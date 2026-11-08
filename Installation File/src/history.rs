// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! Kullanıcının kurduğu paketleri sayar; aramada bu paketler biraz öne çıkar.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::util::xdg_dir;

pub type History = HashMap<String, u32>;

fn path() -> Option<PathBuf> {
    Some(xdg_dir("XDG_STATE_HOME", ".local/state")?.join("paketci").join("history"))
}

fn parse(text: &str) -> History {
    text.lines()
        .filter_map(|l| {
            let (count, name) = l.split_once('\t')?;
            Some((name.to_string(), count.parse().ok()?))
        })
        .collect()
}

fn format(history: &History) -> String {
    let mut lines: Vec<String> = history.iter().map(|(name, count)| format!("{count}\t{name}")).collect();
    lines.sort();
    lines.join("\n") + "\n"
}

pub fn load() -> History {
    path().and_then(|p| fs::read_to_string(p).ok()).map(|s| parse(&s)).unwrap_or_default()
}

fn add(history: &mut History, names: &[String]) {
    for name in names {
        *history.entry(name.clone()).or_insert(0) += 1;
    }
}

/// Başarıyla kurulan paketleri kaydeder ve güncel geçmişi döndürür.
pub fn record(names: &[String]) -> History {
    let mut history = load();
    add(&mut history, names);
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(p, format(&history));
    }
    history
}

/// Yalnızca benzer puanlı sonuçların sırasını değiştirecek kadar küçük tutulur.
pub fn boost(count: u32) -> f64 {
    (0.02 + 0.02 * count as f64).min(0.08)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_counting() {
        let mut h = parse("2\tvim\nbozuk\nx\tfoo\n");
        assert_eq!(h.len(), 1);
        add(&mut h, &["vim".to_string(), "vlc".to_string()]);
        assert_eq!(h["vim"], 3);
        assert_eq!(parse(&format(&h)), h);
    }

    #[test]
    fn boost_is_capped() {
        assert!(boost(1) < boost(2));
        assert_eq!(boost(100), 0.08);
    }
}
