// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use crate::model::{Package, Source};

/// Yaygın adlar ve dağıtımlarda karşılık gelen paketler (öncelik sırasıyla).
const ALIASES: &[(&str, &[&str])] = &[
    ("vscode", &["code", "code-oss", "visual-studio-code-bin"]),
    ("visualstudiocode", &["code", "code-oss", "visual-studio-code-bin"]),
    ("chrome", &["chromium", "google-chrome", "google-chrome-stable", "chromium-browser"]),
    ("googlechrome", &["google-chrome", "google-chrome-stable", "chromium"]),
    ("node", &["nodejs"]),
    ("python", &["python3"]),
    ("pip", &["python3-pip", "python-pip"]),
    ("java", &["default-jre", "jre-openjdk", "java-latest-openjdk"]),
    ("jdk", &["default-jdk", "jdk-openjdk", "java-latest-openjdk-devel"]),
    ("golang", &["go", "golang"]),
    ("go", &["golang"]),
    ("rust", &["rustc", "rust"]),
    ("g++", &["g++", "gcc-c++", "gcc"]),
    ("nvim", &["neovim"]),
    ("vi", &["vim"]),
    ("docker", &["docker.io", "docker-ce", "docker"]),
    ("obs", &["obs-studio"]),
    ("telegram", &["telegram-desktop"]),
    ("libre", &["libreoffice"]),
    ("office", &["libreoffice"]),
    ("word", &["libreoffice-writer", "libreoffice"]),
    ("excel", &["libreoffice-calc", "libreoffice"]),
    ("powerpoint", &["libreoffice-impress", "libreoffice"]),
    ("photoshop", &["gimp", "krita"]),
    ("paint", &["pinta", "kolourpaint"]),
    ("mysql", &["mariadb-server", "mariadb", "mysql-server"]),
    ("postgres", &["postgresql"]),
    ("ssh", &["openssh-client", "openssh"]),
    ("sshd", &["openssh-server", "openssh"]),
    ("ifconfig", &["net-tools"]),
    ("vlcplayer", &["vlc"]),
    ("winrar", &["unrar", "p7zip"]),
    ("7zip", &["p7zip", "p7zip-full", "7zip"]),
    ("torrent", &["qbittorrent", "transmission-gtk"]),
];

/// Asıl programı arayan kişinin çoğunlukla istemediği yan paketler.
const SECONDARY_SUFFIXES: &[&str] = &[
    "-dev", "-devel", "-doc", "-docs", "-dbg", "-dbgsym", "-debuginfo", "-debugsource", "-data", "-common", "-libs",
    "-headers", "-static", "-src",
];
const SECONDARY_PARTS: &[&str] = &["-l10n", "-locale", "-langpack", "-i18n", "-lang-"];

const KEY_ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

/// Küçük harfe çevirir ve Türkçe harfleri Latin karşılıklarına indirir. Paket
/// adları ASCII'dir; `İ` düz küçültülünce görünmez bir nokta da eklenirdi.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'İ' | 'I' | 'ı' => out.push('i'),
            'Ç' | 'ç' => out.push('c'),
            'Ş' | 'ş' => out.push('s'),
            'Ğ' | 'ğ' => out.push('g'),
            'Ö' | 'ö' => out.push('o'),
            'Ü' | 'ü' => out.push('u'),
            '\u{307}' => {}
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

pub fn normalize(s: &str) -> String {
    fold(s)
        .split_whitespace()
        .map(|w| w.trim_start_matches('-'))
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Harf ve rakam dışındakileri atar: "vs-code" ve "vs code" -> "vscode"
fn compact(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Sorguya karşılık gelen bilinen paket adları.
pub fn aliases(q: &str) -> &'static [&'static str] {
    let c = compact(q);
    ALIASES
        .iter()
        .find(|(key, _)| *key == q || *key == c)
        .map(|(_, targets)| *targets)
        .unwrap_or(&[])
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if s.chars().count() >= 3 && !v.contains(&s) {
        v.push(s);
    }
}

fn affixes(word: &str) -> Vec<String> {
    let c: Vec<char> = word.chars().collect();
    let n = c.len();
    let mut out = Vec::new();
    if n >= 4 {
        // ardışık tekrarlanan harfleri teke indir: "firefoox" -> "firefox"
        let mut collapsed = String::new();
        let mut last = None;
        for ch in &c {
            if Some(*ch) != last {
                collapsed.push(*ch);
            }
            last = Some(*ch);
        }
        if collapsed.chars().count() != n {
            out.push(collapsed);
        }
        // ön ek ve son ek: yazım hatası hangi uçtaysa diğer uç doğru kalır
        let k = ((n + 1) / 2).max(3).min(n - 1);
        out.push(c[..k].iter().collect());
        out.push(c[n - k..].iter().collect());
    }
    out
}

/// Asıl sorgunun yanında denenecek alternatif arama ifadeleri.
pub fn variants(q: &str) -> Vec<String> {
    let mut v = vec![q.to_string()];
    let words: Vec<&str> = q.split([' ', '-', '_']).filter(|w| !w.is_empty()).collect();
    if words.len() > 1 {
        // "libre office" -> "libreoffice", "libre-office"
        push_unique(&mut v, words.concat());
        push_unique(&mut v, words.join("-"));
        for w in words {
            push_unique(&mut v, w.to_string());
        }
    } else {
        for a in affixes(q) {
            push_unique(&mut v, a);
        }
    }
    v.truncate(6);
    v
}

fn key_position(c: char) -> Option<(i32, i32)> {
    KEY_ROWS
        .iter()
        .enumerate()
        .find_map(|(row, keys)| keys.find(c).map(|col| (row as i32, col as i32)))
}

fn keys_adjacent(a: char, b: char) -> bool {
    match (key_position(a), key_position(b)) {
        (Some((ra, ca)), Some((rb, cb))) => (ra - rb).abs() <= 1 && (ca - cb).abs() <= 1,
        _ => false,
    }
}

/// Komşu tuş karışıklığını yarım hata sayan düzenleme uzaklığı benzerliği.
fn keyboard_similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    let mut d = vec![vec![0.0_f64; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i as f64;
    }
    for j in 0..=b.len() {
        d[0][j] = j as f64;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let replace = if a[i - 1] == b[j - 1] {
                0.0
            } else if keys_adjacent(a[i - 1], b[j - 1]) {
                0.5
            } else {
                1.0
            };
            let mut cost = (d[i - 1][j] + 1.0).min(d[i][j - 1] + 1.0).min(d[i - 1][j - 1] + replace);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                cost = cost.min(d[i - 2][j - 2] + 1.0);
            }
            d[i][j] = cost;
        }
    }
    1.0 - d[a.len()][b.len()] / longest as f64
}

fn candidates(p: &Package) -> Vec<String> {
    let mut c = vec![fold(&p.name)];
    if p.source == Source::Flatpak {
        if let Some(last) = p.name.rsplit('.').next() {
            c.push(fold(last));
        }
    }
    let d = fold(&p.display);
    if !c.contains(&d) {
        c.push(d);
    }
    c
}

fn name_score(q: &str, c: &str) -> f64 {
    let ratio = q.len() as f64 / c.len().max(1) as f64;
    if c == q {
        1.0
    } else if c.starts_with(q) {
        0.88 + 0.1 * ratio
    } else if c.contains(q) {
        0.70 + 0.1 * ratio
    } else {
        let a = keyboard_similarity(q, c);
        let b = strsim::jaro_winkler(q, c) * 0.9;
        0.68 * a.max(b)
    }
}

/// Geliştirme, belge ve dil paketlerini asıl programın altına indirir.
fn secondary_factor(q: &str, name: &str) -> f64 {
    let wanted = |part: &str| q.contains(part.trim_matches('-'));
    if SECONDARY_SUFFIXES.iter().any(|s| name.ends_with(s) && !wanted(s)) || SECONDARY_PARTS.iter().any(|s| name.contains(s) && !wanted(s)) {
        0.88
    } else if name.starts_with("lib") && !q.contains("lib") {
        0.93
    } else {
        1.0
    }
}

/// 0.0 - 1.0 arası benzerlik puanı. `q` normalize edilmiş olmalı.
pub fn score(q: &str, p: &Package) -> f64 {
    let cands = candidates(p);
    let cq = compact(q);
    let mut best: f64 = 0.0;
    for c in &cands {
        let mut s = name_score(q, c);
        if cq.len() >= 2 {
            s = s.max(name_score(&cq, &compact(c)));
        }
        best = best.max(s);
    }
    best *= secondary_factor(q, &fold(&p.name));
    for (i, alias) in aliases(q).iter().enumerate() {
        if cands.iter().any(|c| c == alias) {
            best = best.max(0.92 - 0.01 * i as f64);
        }
    }
    let desc = fold(&p.description);
    if q.contains(' ') {
        let toks: Vec<&str> = q.split_whitespace().collect();
        let hay = format!("{} {}", cands.join(" "), desc);
        let found = toks.iter().filter(|t| hay.contains(**t)).count();
        best = best.max(0.8 * found as f64 / toks.len() as f64);
    }
    if desc.contains(q) {
        best = best.max(0.45);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str) -> Package {
        Package::new(Source::Native, name)
    }

    #[test]
    fn normalize_works() {
        assert_eq!(normalize("  --FireFox   Browser "), "firefox browser");
    }

    #[test]
    fn turkish_letters_fold() {
        assert_eq!(normalize("FİREFOX"), "firefox");
        assert_eq!(normalize("fırefox"), "firefox");
        assert_eq!(normalize("ÇŞĞÖÜ"), "csgou");
        assert_eq!(normalize("fi\u{307}refox"), "firefox");
    }

    #[test]
    fn typo_variants_cover_both_ends() {
        let v = variants("firfox");
        assert_eq!(v[0], "firfox");
        assert!(v.contains(&"fir".to_string()));
        assert!(v.contains(&"fox".to_string()));
        assert!(variants("firefoox").contains(&"firefox".to_string()));
    }

    #[test]
    fn separators_are_tried_and_ignored() {
        let v = variants("libre office");
        assert!(v.contains(&"libreoffice".to_string()));
        assert!(v.contains(&"libre-office".to_string()));
        assert!(variants("vs-code").contains(&"vscode".to_string()));
        assert_eq!(score("libre office", &pkg("libreoffice")), 1.0);
        assert_eq!(score("vlc-plugin", &pkg("vlcplugin")), 1.0);
    }

    #[test]
    fn typo_ranks_right_package_first() {
        let q = "firfox";
        let good = score(q, &pkg("firefox"));
        assert!(good >= 0.55, "{good}");
        assert!(good > score(q, &pkg("firewalld")));
        assert!(score(q, &pkg("firewalld")) < 0.55);
    }

    #[test]
    fn exact_beats_prefix_beats_contains() {
        let q = "vlc";
        assert!(score(q, &pkg("vlc")) > score(q, &pkg("vlc-plugin")));
        assert!(score(q, &pkg("vlc-plugin")) > score(q, &pkg("libvlc")));
    }

    #[test]
    fn secondary_packages_rank_lower_unless_asked() {
        let q = "firefox";
        assert!(score(q, &pkg("firefox-esr")) > score(q, &pkg("firefox-dev")));
        assert!(score(q, &pkg("firefox-esr")) > score(q, &pkg("firefox-l10n-tr")));
        assert_eq!(score("firefox-dev", &pkg("firefox-dev")), 1.0);
    }

    #[test]
    fn aliases_find_real_package_names() {
        assert!(score("vscode", &pkg("code")) > 0.9);
        assert!(score("vs code", &pkg("code")) > 0.9);
        assert!(score("chrome", &pkg("chromium")) > score("chrome", &pkg("chrome-gnome-shell")));
        assert!(score("vscode", &pkg("code")) > score("vscode", &pkg("code-oss")));
        assert_eq!(aliases("g++"), &["g++", "gcc-c++", "gcc"]);
        assert!(aliases("firefox").is_empty());
    }

    #[test]
    fn adjacent_keys_cost_less() {
        assert!(score("firefix", &pkg("firefox")) > score("firefax", &pkg("firefox")));
        assert!(keys_adjacent('o', 'i'));
        assert!(!keys_adjacent('a', 'p'));
    }
}
