// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::i18n::t;
use crate::model::Cmd;
use crate::transcript::{tail_view, Cleaner};
use crate::util::{capture, escalator, is_root, xdg_dir};

const HISTORY_LINES: usize = 10_000;
const LOG_MAX_BYTES: u64 = 2_000_000;

fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:@+,%".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn failed_clause() -> String {
    format!(
        r#" || {{ rc=$?; printf '\033[1;31m{}\033[0m\n' "$rc"; exit "$rc"; }}"#,
        t("Komut başarısız oldu (kod %s); kalan adımlar atlandı.", "Command failed (code %s); remaining steps skipped."),
    )
}

/// Komut grubunu tek kabuk betiğine çevirir. Hepsi aynı oturumda çalışmalı:
/// sudo parola onayını oturum başına tuttuğu için ayrı oturumlar parolayı her
/// komutta yeniden sorardı.
fn build_script(cmds: &[Cmd], root: bool, esc: Option<&str>) -> String {
    let failed = failed_clause();
    let mut script = String::new();
    for c in cmds {
        script.push_str(&format!(r"printf '\033[1;36m$ %s\033[0m\n' {}", shell_quote(&c.display())));
        script.push('\n');
        let mut words: Vec<String> = Vec::new();
        if c.root && !root {
            match esc {
                Some(e) => words.push(e.to_string()),
                None => {
                    let msg = t("Hata: root yetkisi gerekiyor ama sudo/doas bulunamadı.", "Error: root privileges are required but sudo/doas was not found.");
                    script.push_str(&format!(r"printf '\033[1;31m{msg}\033[0m\n'; exit 1"));
                    script.push('\n');
                    return script;
                }
            }
        }
        words.push(shell_quote(&c.program));
        words.extend(c.args.iter().map(|a| shell_quote(a)));
        script.push_str(&words.join(" "));
        script.push_str(&failed);
        script.push('\n');
    }
    let done = t("İşlem tamamlandı.", "Operation completed.");
    script.push_str(&format!(r"printf '\033[1;32m{done}\033[0m\n'"));
    script.push('\n');
    script
}

/// Klavye olayını terminalin beklediği bayt dizisine çevirir.
fn key_bytes(k: KeyEvent, app_cursor: bool) -> Vec<u8> {
    let arrow = |c: u8| vec![0x1b, if app_cursor { b'O' } else { b'[' }, c];
    let mut out = match k.code {
        KeyCode::Char(c) if k.modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii_alphabetic() => {
            vec![c.to_ascii_uppercase() as u8 - b'@']
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => arrow(b'A'),
        KeyCode::Down => arrow(b'B'),
        KeyCode::Right => arrow(b'C'),
        KeyCode::Left => arrow(b'D'),
        KeyCode::Home => arrow(b'H'),
        KeyCode::End => arrow(b'F'),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        _ => vec![],
    };
    if k.modifiers.contains(KeyModifiers::ALT) && !out.is_empty() {
        out.insert(0, 0x1b);
    }
    out
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }
}

/// Günlük dosyasını açar; büyüdüyse eskisini `.1` olarak ayırır.
fn open_log() -> Option<(File, PathBuf)> {
    let dir = xdg_dir("XDG_STATE_HOME", ".local/state")?.join("paketci");
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("paketci.log");
    if fs::metadata(&path).map(|m| m.len() > LOG_MAX_BYTES).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("paketci.log.1"));
    }
    let file = OpenOptions::new().create(true).append(true).mode(0o600).open(&path).ok()?;
    Some((file, path))
}

/// TUI içindeki sanal terminalde çalışan komut grubu. Parola istemleri,
/// ilerleme çubukları ve onay soruları gerçek terminaldeki gibi çalışır.
pub struct Run {
    parser: Arc<Mutex<vt100::Parser>>,
    history: Arc<Mutex<VecDeque<String>>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    size: (u16, u16),
    success: Option<bool>,
    scroll: usize,
    log_path: Option<PathBuf>,
}

impl Run {
    pub fn start(cmds: &[Cmd], rows: u16, cols: u16) -> Result<Self> {
        let pair = native_pty_system().openpty(pty_size(rows, cols))?;
        let mut sh = CommandBuilder::new("sh");
        sh.arg("-c");
        sh.arg(build_script(cmds, is_root(), escalator()));
        sh.env("TERM", "xterm-256color");
        if let Ok(dir) = std::env::current_dir() {
            sh.cwd(dir);
        }
        let child = pair.slave.spawn_command(sh)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
        let history = Arc::new(Mutex::new(VecDeque::new()));

        let (mut log, log_path) = match open_log() {
            Some((file, path)) => (Some(file), Some(path)),
            None => (None, None),
        };
        if let Some(f) = &mut log {
            let date = capture("date", &["+%F %T"]).unwrap_or_default();
            let _ = writeln!(f, "\n=== {} ===", date.trim());
        }

        let out = Arc::clone(&parser);
        let lines = Arc::clone(&history);
        thread::spawn(move || {
            let mut emit = |line: String| {
                if let Some(f) = &mut log {
                    let _ = writeln!(f, "{line}");
                }
                let mut h = lines.lock().unwrap_or_else(|e| e.into_inner());
                if h.len() == HISTORY_LINES {
                    h.pop_front();
                }
                h.push_back(line);
            };
            let mut cleaner = Cleaner::default();
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                out.lock().unwrap_or_else(|e| e.into_inner()).process(&buf[..n]);
                cleaner.feed(&buf[..n], &mut emit);
            }
            cleaner.finish(&mut emit);
        });

        Ok(Run {
            parser,
            history,
            master: pair.master,
            writer,
            child,
            size: (rows, cols),
            success: None,
            scroll: 0,
            log_path,
        })
    }

    pub fn parser(&self) -> MutexGuard<'_, vt100::Parser> {
        self.parser.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Komut sürüyorsa `None`, bittiyse başarılı olup olmadığı.
    pub fn success(&self) -> Option<bool> {
        self.success
    }

    pub fn log_path(&self) -> Option<&PathBuf> {
        self.log_path.as_ref()
    }

    pub fn poll(&mut self) {
        if self.success.is_some() {
            return;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => self.success = Some(status.success()),
            Ok(None) => {}
            Err(_) => self.success = Some(false),
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == 0 || cols == 0 || self.size == (rows, cols) {
            return;
        }
        self.size = (rows, cols);
        let _ = self.master.resize(pty_size(rows, cols));
        self.parser().set_size(rows, cols);
    }

    /// Geriye kaydırma tuşuysa işleyip `true` döner. Komut sürerken yön tuşları
    /// komuta gitmeli, tam ekran pencerelerde (debconf gibi) sayfa tuşları da.
    pub fn scroll_key(&mut self, k: KeyEvent) -> bool {
        if self.parser().screen().alternate_screen() {
            return false;
        }
        let page = self.size.0 as usize;
        let done = self.success.is_some();
        match k.code {
            KeyCode::PageUp => self.scroll = self.scroll.saturating_add(page),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(page),
            KeyCode::Up | KeyCode::Char('k') if done => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Down | KeyCode::Char('j') if done => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Home | KeyCode::Char('g') if done => self.scroll = usize::MAX / 2,
            KeyCode::End | KeyCode::Char('G') if done => self.scroll = 0,
            _ => return false,
        }
        true
    }

    /// Geriye kaydırılmışsa gösterilecek satırlar; canlı ekrandaysa `None`.
    pub fn history_view(&mut self, rows: u16, cols: u16) -> Option<Vec<String>> {
        if self.scroll == 0 {
            return None;
        }
        let (view, scroll) = {
            let h = self.history.lock().unwrap_or_else(|e| e.into_inner());
            tail_view(&h, self.scroll, rows as usize, cols as usize)
        };
        self.scroll = scroll;
        (scroll > 0).then_some(view)
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn send_key(&mut self, k: KeyEvent) {
        self.scroll = 0;
        let app_cursor = self.parser().screen().application_cursor();
        let bytes = key_bytes(k, app_cursor);
        if !bytes.is_empty() {
            let _ = self.writer.write_all(&bytes);
            let _ = self.writer.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("firefox"), "firefox");
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn script_runs_all_steps_with_escalator() {
        let cmds = [Cmd::root("apt", &["update"]), Cmd::user("flatpak", &["update", "--user"])];
        let s = build_script(&cmds, false, Some("doas"));
        assert!(s.contains("doas apt update || {"));
        assert!(s.contains("\nflatpak update --user || {"));
    }

    #[test]
    fn script_skips_escalator_for_root() {
        let s = build_script(&[Cmd::root("apt", &["update"])], true, None);
        assert!(s.contains("\napt update || {"));
    }

    #[test]
    fn script_stops_without_escalator() {
        let cmds = [Cmd::root("apt", &["update"]), Cmd::user("flatpak", &["update"])];
        let s = build_script(&cmds, false, None);
        assert!(s.contains("sudo/doas"));
        assert!(!s.contains("flatpak update"));
    }

    #[test]
    fn keys() {
        let none = KeyModifiers::NONE;
        assert_eq!(key_bytes(key(KeyCode::Char('ş'), none), false), "ş".as_bytes());
        assert_eq!(key_bytes(key(KeyCode::Char('c'), KeyModifiers::CONTROL), false), vec![3]);
        assert_eq!(key_bytes(key(KeyCode::Enter, none), false), b"\r");
        assert_eq!(key_bytes(key(KeyCode::Up, none), false), b"\x1b[A");
        assert_eq!(key_bytes(key(KeyCode::Up, none), true), b"\x1bOA");
    }
}
