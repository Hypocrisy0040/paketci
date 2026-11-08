// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! Dışa aktarılan paket listesinin dosya biçimi: `kaynak<TAB>ad<TAB>ek`.

use crate::util::split_last_dash;

const HEADER: &str = "# paketci package list v1";

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Paket yöneticisi ("apt", "pacman"…) veya "aur", "flatpak", "snap"
    pub kind: String,
    pub name: String,
    /// Kaynağa özel bilgi (snap: "classic", flatpak: "user"/"system")
    pub extra: String,
}

pub fn format(entries: &[Entry], distro: &str) -> String {
    let mut lines: Vec<String> = entries.iter().map(|e| format!("{}\t{}\t{}", e.kind, e.name, e.extra)).collect();
    lines.sort();
    lines.dedup();
    format!("{HEADER}\n# {distro}\n{}\n", lines.join("\n"))
}

/// Dosya dışarıdan gelebilir: "-" ile başlayan bir ad komuta seçenek olarak geçerdi.
fn valid_name(name: &str) -> bool {
    name.chars().next().map_or(false, |c| c.is_ascii_alphanumeric())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || "+._:@-".contains(c))
}

/// Geçerli satırları ve geçersiz satır sayısını döndürür.
pub fn parse(text: &str) -> (Vec<Entry>, usize) {
    let mut entries = Vec::new();
    let mut invalid = 0;
    for line in text.lines().map(str::trim_end) {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split('\t');
        match (it.next(), it.next()) {
            (Some(kind), Some(name)) if !kind.is_empty() && valid_name(name) => entries.push(Entry {
                kind: kind.to_string(),
                name: name.to_string(),
                extra: it.next().unwrap_or("").to_string(),
            }),
            _ => invalid += 1,
        }
    }
    (entries, invalid)
}

/// `/etc/apk/world`: "ad", "ad>1.0", "ad@edge"; "!" ile başlayanlar çakışmadır.
pub fn parse_apk_world(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|w| !w.starts_with('!'))
        .map(|w| w.split(['<', '>', '=', '~', '@']).next().unwrap_or("").to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

/// `xbps-query -m`: satır başına "ad-sürüm_devir"
pub fn parse_xbps_manual(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| split_last_dash(l).0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, name: &str, extra: &str) -> Entry {
        Entry { kind: kind.into(), name: name.into(), extra: extra.into() }
    }

    #[test]
    fn round_trip() {
        let list = vec![entry("snap", "code", "classic"), entry("apt", "vim", ""), entry("apt", "vim", "")];
        let text = format(&list, "Ubuntu 24.04");
        assert!(text.starts_with(HEADER));
        let (parsed, invalid) = parse(&text);
        assert_eq!(parsed, [entry("apt", "vim", ""), entry("snap", "code", "classic")]);
        assert_eq!(invalid, 0);
    }

    #[test]
    fn rejects_unsafe_names() {
        let (parsed, invalid) = parse("apt\t--force-yes\t\napt\tok-name+1\t\napt\ta b\t\nbozuk satır\n\n# yorum\n");
        assert_eq!(parsed, [entry("apt", "ok-name+1", "")]);
        assert_eq!(invalid, 3);
    }

    #[test]
    fn apk_world() {
        assert_eq!(parse_apk_world("vim\nfoo>1.0\nbar@edge\n!baz\n"), ["vim", "foo", "bar"]);
    }

    #[test]
    fn xbps_manual() {
        assert_eq!(parse_xbps_manual("foo-1.2_1\nbar-baz-2.0_3\n"), ["foo", "bar-baz"]);
    }
}
