// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use crate::model::*;
use crate::util::capture;

pub fn search(q: &str) -> Vec<Package> {
    capture("snap", &["find", q]).map(|s| parse_find(&s)).unwrap_or_default()
}

pub fn parse_find(text: &str) -> Vec<Package> {
    text.lines()
        .skip(1) // başlık satırı
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.len() < 4 {
                return None;
            }
            let mut p = Package::new(Source::Snap, t[0]);
            p.version = t[1].to_string();
            p.origin = "snapcraft".into();
            if t[3].contains("classic") {
                p.extra = "classic".into();
            }
            p.description = t[4..].join(" ");
            Some(p)
        })
        .collect()
}

pub fn installed() -> Vec<Package> {
    capture("snap", &["list"])
        .map(|s| {
            s.lines()
                .skip(1)
                .filter_map(|l| {
                    let t: Vec<&str> = l.split_whitespace().collect();
                    if t.len() < 2 {
                        return None;
                    }
                    let mut p = Package::new(Source::Snap, t[0]);
                    p.version = t[1].to_string();
                    p.origin = "snapcraft".into();
                    let notes = t.get(5).copied().unwrap_or("");
                    if notes.contains("classic") {
                        p.extra = "classic".into();
                    } else if notes.contains("base") || notes.contains("snapd") {
                        p.extra = "base".into();
                    }
                    p.installed = true;
                    Some(p)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn updates() -> Vec<Package> {
    capture("snap", &["refresh", "--list"])
        .map(|s| {
            s.lines()
                .skip(1)
                .filter_map(|l| {
                    let t: Vec<&str> = l.split_whitespace().collect();
                    if t.len() < 2 {
                        return None;
                    }
                    let mut p = Package::new(Source::Snap, t[0]);
                    p.version = format!("→ {}", t[1]);
                    p.origin = "snapcraft".into();
                    p.installed = true;
                    Some(p)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn install_cmd(p: &Package) -> Cmd {
    if p.extra == "classic" {
        Cmd::root("snap", &["install", "--classic", &p.name])
    } else {
        Cmd::root("snap", &["install", &p.name])
    }
}

pub fn remove_cmd(p: &Package) -> Cmd {
    Cmd::root("snap", &["remove", &p.name])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_output() {
        let t = "Name  Version  Publisher   Notes    Summary\nvlc   3.0.20   videolan**  -        The ultimate media player\ncode  1.90     vscode**    classic  Code editing\n";
        let v = parse_find(t);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].description, "The ultimate media player");
        assert_eq!(v[1].extra, "classic");
    }
}
