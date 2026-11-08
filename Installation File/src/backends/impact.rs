// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! Onay penceresi için işlemin etkisini root gerektirmeden hesaplar.
//! dnf ve zypper simülasyon için de root istediğinden önizleme yapılmaz; onlar
//! işlem sırasında kendi özetlerini gösterip onay sorar.

use crate::distro::Family;
use crate::util::{capture, split_last_dash};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Install,
    Remove,
    Update,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Impact {
    /// Kurulacak veya güncellenecek paketler (bağımlılıklar dahil)
    pub install: Vec<String>,
    pub remove: Vec<String>,
    pub download: Option<u64>,
}

impl Impact {
    fn is_empty(&self) -> bool {
        self.install.is_empty() && self.remove.is_empty()
    }
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

pub fn parse_apt_sim(text: &str) -> Impact {
    let mut i = Impact::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match (it.next(), it.next()) {
            (Some("Inst"), Some(n)) => i.install.push(n.to_string()),
            (Some("Remv" | "Purg"), Some(n)) => i.remove.push(n.to_string()),
            _ => {}
        }
    }
    i
}

fn sum_apt_sizes(text: &str) -> Option<u64> {
    let sizes: Vec<u64> = text
        .lines()
        .filter_map(|l| l.strip_prefix("Size: ")?.trim().parse().ok())
        .collect();
    if sizes.is_empty() {
        None
    } else {
        Some(sizes.iter().sum())
    }
}

/// `pacman --print-format "%n %v %s"` (kaldırmada boyut sütunu yoktur)
pub fn parse_pacman_print(text: &str, remove: bool) -> Impact {
    let mut i = Impact::default();
    let mut total = 0u64;
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 2 {
            continue;
        }
        if remove {
            i.remove.push(t[0].to_string());
        } else {
            i.install.push(t[0].to_string());
            total += t.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        }
    }
    if !remove && !i.install.is_empty() {
        i.download = Some(total);
    }
    i
}

pub fn parse_apk_sim(text: &str) -> Impact {
    let mut i = Impact::default();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('(').and_then(|l| l.split_once(") ")).map(|(_, r)| r) else {
            continue;
        };
        let mut it = rest.split_whitespace();
        match (it.next(), it.next()) {
            (Some("Installing" | "Upgrading" | "Downgrading" | "Reinstalling" | "Replacing"), Some(n)) => {
                i.install.push(n.to_string())
            }
            (Some("Purging" | "Deleting"), Some(n)) => i.remove.push(n.to_string()),
            _ => {}
        }
    }
    i
}

/// `xbps-* -n`: "pkgver eylem mimari depo kurulu-boyut indirme-boyutu"
pub fn parse_xbps_sim(text: &str) -> Impact {
    let mut i = Impact::default();
    let mut total = 0u64;
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 2 {
            continue;
        }
        let (name, _) = split_last_dash(t[0]);
        match t[1] {
            "install" | "update" | "reinstall" | "downgrade" => {
                i.install.push(name);
                total += t.get(5).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            }
            "remove" => i.remove.push(name),
            _ => {}
        }
    }
    if total > 0 {
        i.download = Some(total);
    }
    i
}

/// Dağıtımın paket yöneticisiyle işlemi simüle eder. Desteklenmiyorsa veya
/// sonuç alınamazsa `None` döner.
pub fn simulate(family: Family, action: Action, names: &[String]) -> Option<Impact> {
    let run = |program: &str, base: &[&str]| {
        let mut args = base.to_vec();
        args.extend(names.iter().map(String::as_str));
        capture(program, &args)
    };
    let impact = match (family, action) {
        (Family::Debian, Action::Install) => parse_apt_sim(&run("apt-get", &["-s", "install"])?),
        (Family::Debian, Action::Update) => parse_apt_sim(&run("apt-get", &["-s", "install", "--only-upgrade"])?),
        (Family::Debian, Action::Remove) => parse_apt_sim(&run("apt-get", &["-s", "remove"])?),
        (Family::Arch, Action::Install) => {
            parse_pacman_print(&run("pacman", &["-Sp", "--needed", "--print-format", "%n %v %s"])?, false)
        }
        (Family::Arch, Action::Remove) => {
            parse_pacman_print(&run("pacman", &["-Rnsp", "--print-format", "%n %v"])?, true)
        }
        (Family::Alpine, Action::Install) => parse_apk_sim(&run("apk", &["add", "--simulate"])?),
        (Family::Alpine, Action::Update) => parse_apk_sim(&run("apk", &["add", "--simulate", "--upgrade"])?),
        (Family::Alpine, Action::Remove) => parse_apk_sim(&run("apk", &["del", "--simulate"])?),
        (Family::Void, Action::Install) => parse_xbps_sim(&run("xbps-install", &["-n"])?),
        (Family::Void, Action::Update) => parse_xbps_sim(&run("xbps-install", &["-nu"])?),
        (Family::Void, Action::Remove) => parse_xbps_sim(&run("xbps-remove", &["-nR"])?),
        _ => return None,
    };
    if impact.is_empty() {
        return None;
    }
    let mut impact = impact;
    if family == Family::Debian && !impact.install.is_empty() {
        let mut args = vec!["show", "--no-all-versions"];
        args.extend(impact.install.iter().map(String::as_str));
        impact.download = capture("apt-cache", &args).and_then(|s| sum_apt_sizes(&s));
    }
    Some(impact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_simulation() {
        let i = parse_apt_sim(
            "NOTE: This is only a simulation!\nInst libfoo (1.0 Ubuntu:24.04 [amd64])\nInst vim [2:8.2] (2:9.0 Ubuntu:24.04 [amd64])\nRemv nano [7.2]\nConf vim (2:9.0 Ubuntu:24.04 [amd64])\n",
        );
        assert_eq!(i.install, ["libfoo", "vim"]);
        assert_eq!(i.remove, ["nano"]);
    }

    #[test]
    fn apt_sizes() {
        assert_eq!(sum_apt_sizes("Package: a\nSize: 100\nInstalled-Size: 9\n\nPackage: b\nSize: 50\n"), Some(150));
        assert_eq!(sum_apt_sizes("Package: a\n"), None);
    }

    #[test]
    fn pacman_print() {
        let i = parse_pacman_print("glibc 2.40-1 1000\nfirefox 130.0-1 2000\n", false);
        assert_eq!(i.install, ["glibc", "firefox"]);
        assert_eq!(i.download, Some(3000));
        let r = parse_pacman_print("firefox 130.0-1\nlibfoo 1.0-1\n", true);
        assert_eq!(r.remove, ["firefox", "libfoo"]);
        assert_eq!(r.download, None);
    }

    #[test]
    fn apk_simulation() {
        let i = parse_apk_sim("(1/3) Installing foo (1.2-r0)\n(2/3) Upgrading bar (1.0-r0 -> 1.1-r0)\n(3/3) Purging baz (2.0-r0)\nOK: 10 MiB in 5 packages\n");
        assert_eq!(i.install, ["foo", "bar"]);
        assert_eq!(i.remove, ["baz"]);
    }

    #[test]
    fn xbps_simulation() {
        let i = parse_xbps_sim("foo-1.2_1 install x86_64 https://repo 4000 1500\nbar-baz-2.0_3 update x86_64 https://repo 100 500\nold-1.0_1 remove x86_64 https://repo 0 0\n");
        assert_eq!(i.install, ["foo", "bar-baz"]);
        assert_eq!(i.remove, ["old"]);
        assert_eq!(i.download, Some(2000));
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1_500), "1.5 KB");
        assert_eq!(format_size(12_300_000), "12.3 MB");
    }
}
