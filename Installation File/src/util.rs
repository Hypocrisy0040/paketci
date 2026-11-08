// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Baştaki "~" işaretini ev dizinine çevirir.
pub fn expand_home(path: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match (path.strip_prefix('~'), home) {
        (Some(""), Some(h)) => h,
        (Some(rest), Some(h)) if rest.starts_with('/') => h.join(&rest[1..]),
        _ => PathBuf::from(path),
    }
}

/// XDG dizini; değişken yoksa ev dizini altındaki varsayılanı kullanır.
pub fn xdg_dir(var: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(fallback)))
}

/// PATH içinde çalıştırılabilir bir dosya var mı?
pub fn which(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

/// Komutu çalıştırıp stdout'u döndürür (çıkış kodunu yok sayar; bazı araçlar
/// "güncelleme var" durumunda sıfırdan farklı kod döndürür).
pub fn capture(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn regex_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        if ".+*?()[]{}|^$\\".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

pub fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .map(|l| l.split_whitespace().nth(1) == Some("0"))
        })
        .unwrap_or(false)
}

/// sudo / doas gibi yetki yükseltme aracı.
pub fn escalator() -> Option<&'static str> {
    ["sudo", "doas"].into_iter().find(|b| which(b))
}

/// "name-1.2.3" -> ("name", "1.2.3") (son tireden böler; xbps için)
pub fn split_last_dash(s: &str) -> (String, String) {
    match s.rsplit_once('-') {
        Some((n, v)) => (n.to_string(), v.to_string()),
        None => (s.to_string(), String::new()),
    }
}
