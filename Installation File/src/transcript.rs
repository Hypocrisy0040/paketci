// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! Terminal çıktısından düz metin satırları çıkarır (geriye kaydırma ve günlük için).

use std::collections::VecDeque;

#[derive(Default, PartialEq, Clone, Copy)]
enum State {
    #[default]
    Text,
    Esc,
    Csi,
    Osc,
    OscEsc,
    Charset,
}

/// Renk ve imleç kodlarını atar; `\r` ile yeniden yazılan ilerleme satırlarından
/// yalnızca sonuncusunu bırakır.
#[derive(Default)]
pub struct Cleaner {
    state: State,
    line: Vec<u8>,
    csi: Vec<u8>,
    carriage: bool,
    /// Kaydedilen imleç konumuna çizilen durum satırı (apt ilerlemesi gibi)
    overlay: bool,
    alternate: bool,
}

impl Cleaner {
    fn hidden(&self) -> bool {
        self.overlay || self.alternate
    }

    fn flush(&mut self, emit: &mut impl FnMut(String)) {
        emit(String::from_utf8_lossy(&self.line).into_owned());
        self.line.clear();
    }

    fn text(&mut self, b: u8, emit: &mut impl FnMut(String)) {
        match b {
            0x1b => self.state = State::Esc,
            b'\n' => {
                self.carriage = false;
                if !self.hidden() {
                    self.flush(emit);
                }
            }
            b'\r' => self.carriage = true,
            0x08 => {
                while let Some(c) = self.line.pop() {
                    if c & 0xC0 != 0x80 {
                        break;
                    }
                }
            }
            b'\t' | 0x20..=0xff if !self.hidden() => {
                if self.carriage {
                    self.line.clear();
                    self.carriage = false;
                }
                if b == b'\t' {
                    self.line.extend_from_slice(b"    ");
                } else if b != 0x7f {
                    self.line.push(b);
                }
            }
            _ => {}
        }
    }

    fn csi_end(&mut self, end: u8, emit: &mut impl FnMut(String)) {
        match (end, self.csi.as_slice()) {
            (b'K', b"2") if !self.hidden() => self.line.clear(),
            (b'H' | b'f', _) if !self.hidden() && !self.line.is_empty() => self.flush(emit),
            (b'h', b"?1049" | b"?1047" | b"?47") => self.alternate = true,
            (b'l', b"?1049" | b"?1047" | b"?47") => self.alternate = false,
            _ => {}
        }
        self.csi.clear();
    }

    pub fn feed(&mut self, bytes: &[u8], emit: &mut impl FnMut(String)) {
        for &b in bytes {
            match self.state {
                State::Text => self.text(b, emit),
                State::Esc => {
                    self.state = State::Text;
                    match b {
                        b'[' => self.state = State::Csi,
                        b']' => self.state = State::Osc,
                        b'(' | b')' | b'*' | b'+' | b'#' | b'%' => self.state = State::Charset,
                        b'7' => self.overlay = true,
                        b'8' => self.overlay = false,
                        _ => {}
                    }
                }
                State::Csi => {
                    if (0x40..=0x7e).contains(&b) {
                        self.state = State::Text;
                        self.csi_end(b, emit);
                    } else {
                        self.csi.push(b);
                    }
                }
                State::Osc => match b {
                    0x07 => self.state = State::Text,
                    0x1b => self.state = State::OscEsc,
                    _ => {}
                },
                State::OscEsc => self.state = State::Text,
                State::Charset => self.state = State::Text,
            }
        }
    }

    /// Satır sonu gelmeden biten çıktıyı da verir.
    pub fn finish(&mut self, emit: &mut impl FnMut(String)) {
        if !self.line.is_empty() {
            self.flush(emit);
        }
    }
}

fn wrap(line: &str, cols: usize) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars.chunks(cols.max(1)).map(|c| c.iter().collect()).collect()
}

/// Sondan `scroll` satır geriden bakan ekranı üretir. Sınırlanmış `scroll` değerini de döndürür.
pub fn tail_view(lines: &VecDeque<String>, scroll: usize, rows: usize, cols: usize) -> (Vec<String>, usize) {
    let need = scroll.saturating_add(rows);
    let mut reversed: Vec<String> = Vec::new();
    for line in lines.iter().rev() {
        reversed.extend(wrap(line, cols).into_iter().rev());
        if reversed.len() >= need {
            break;
        }
    }
    let scroll = scroll.min(reversed.len().saturating_sub(rows));
    let end = scroll.saturating_add(rows).min(reversed.len());
    let view = reversed[scroll..end].iter().rev().cloned().collect();
    (view, scroll)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(input: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        let mut c = Cleaner::default();
        c.feed(input, &mut |l| out.push(l));
        c.finish(&mut |l| out.push(l));
        out
    }

    #[test]
    fn strips_colors() {
        assert_eq!(clean(b"\x1b[1;36m$ apt\x1b[0m\r\nok\r\n"), ["$ apt", "ok"]);
    }

    #[test]
    fn keeps_last_progress() {
        assert_eq!(clean(b"%10\r%50\r%100\r\nbitti\r\n"), ["%100", "bitti"]);
    }

    #[test]
    fn drops_overlay_and_alternate_screen() {
        assert_eq!(clean(b"a\r\n\x1b7\x1b[24;0fProgress: 20%\x1b8b\r\n"), ["a", "b"]);
        assert_eq!(clean(b"a\r\n\x1b[?1049hdialog\r\n\x1b[?1049lb\r\n"), ["a", "b"]);
    }

    #[test]
    fn handles_utf8_split_and_backspace() {
        let mut out = Vec::new();
        let mut c = Cleaner::default();
        let word = "ş".as_bytes();
        c.feed(&word[..1], &mut |l| out.push(l));
        c.feed(&word[1..], &mut |l| out.push(l));
        c.feed(b"\r\n", &mut |l| out.push(l));
        assert_eq!(out, ["ş"]);
        assert_eq!(clean("aş\x08b\r\n".as_bytes()), ["ab"]);
    }

    #[test]
    fn view_scrolls_and_clamps() {
        let lines: VecDeque<String> = (1..=5).map(|i| i.to_string()).collect();
        assert_eq!(tail_view(&lines, 0, 2, 80), (vec!["4".to_string(), "5".to_string()], 0));
        assert_eq!(tail_view(&lines, 1, 2, 80), (vec!["3".to_string(), "4".to_string()], 1));
        assert_eq!(tail_view(&lines, 99, 2, 80), (vec!["1".to_string(), "2".to_string()], 3));
    }

    #[test]
    fn view_wraps_long_lines() {
        let lines: VecDeque<String> = vec!["abcde".to_string()].into();
        assert_eq!(tail_view(&lines, 0, 3, 2).0, ["ab", "cd", "e"]);
    }
}
