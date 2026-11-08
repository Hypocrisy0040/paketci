// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! AUR (yay / paru / pikaur üzerinden). Yalnızca Arch tabanlı sistemlerde.

use super::native::{parse_arrow_updates, parse_pacman};
use crate::model::*;
use crate::util::capture;

pub fn search(helper: &str, q: &str) -> Vec<Package> {
    let terms: Vec<&str> = q.split_whitespace().collect();
    let mut args = vec!["-Ssa"];
    args.extend(terms);
    capture(helper, &args)
        .map(|s| parse_pacman(&s, Source::Aur))
        .unwrap_or_default()
}

/// Yabancı (depolarda olmayan) paketler = AUR'dan kurulanlar.
pub fn installed() -> Vec<Package> {
    capture("pacman", &["-Qm"])
        .map(|s| {
            s.lines()
                .filter_map(|l| {
                    let mut it = l.split_whitespace();
                    let mut p = Package::new(Source::Aur, it.next()?);
                    p.version = it.next().unwrap_or("").to_string();
                    p.origin = "AUR".into();
                    p.installed = true;
                    Some(p)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Kullanıcının kendi kurduğu AUR paketleri.
pub fn manual_packages() -> Vec<String> {
    capture("pacman", &["-Qqem"])
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

pub fn updates(helper: &str) -> Vec<Package> {
    capture(helper, &["-Qua"])
        .map(|s| {
            let mut v = parse_arrow_updates(&s, Source::Aur);
            for p in &mut v {
                p.origin = "AUR".into();
            }
            v
        })
        .unwrap_or_default()
}

pub fn install_cmd(helper: &str, names: &[String]) -> Cmd {
    let mut args = vec!["-S".to_string(), "--needed".to_string()];
    args.extend(names.iter().cloned());
    Cmd { program: helper.to_string(), args, root: false }
}

/// yay-bin'i AUR'dan derleyip kurar (yardımcı yoksa).
pub fn bootstrap_cmds() -> Vec<Cmd> {
    vec![
        Cmd::root("pacman", &["-S", "--needed", "git", "base-devel"]),
        Cmd::shell(
            false,
            "d=$(mktemp -d) && git clone https://aur.archlinux.org/yay-bin.git \"$d/yay-bin\" && cd \"$d/yay-bin\" && makepkg -si",
        ),
    ]
}
