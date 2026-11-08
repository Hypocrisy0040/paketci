// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use std::collections::HashMap;
use std::fs;

use crate::util::which;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Arch,
    Debian,
    Fedora,
    Suse,
    Alpine,
    Void,
    Unknown,
}

impl Family {
    pub fn manager(self) -> &'static str {
        match self {
            Family::Arch => "pacman",
            Family::Debian => "apt",
            Family::Fedora => "dnf",
            Family::Suse => "zypper",
            Family::Alpine => "apk",
            Family::Void => "xbps",
            Family::Unknown => "?",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Distro {
    pub id: String,
    pub pretty: String,
    pub family: Family,
}

pub fn parse_os_release(text: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
            m.insert(k.trim().to_string(), v.to_string());
        }
    }
    m
}

pub fn family_from_ids(ids: &[String]) -> Family {
    for id in ids {
        let id = id.as_str();
        let fam = match id {
            "arch" | "manjaro" | "endeavouros" | "cachyos" | "garuda" | "artix" | "arcolinux"
            | "archcraft" | "arco" => Family::Arch,
            "debian" | "ubuntu" | "linuxmint" | "pop" | "elementary" | "kali" | "raspbian"
            | "zorin" | "devuan" | "neon" | "mx" | "parrot" | "deepin" => Family::Debian,
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "nobara" | "amzn" | "ol" => {
                Family::Fedora
            }
            "alpine" | "postmarketos" => Family::Alpine,
            "void" => Family::Void,
            _ if id.starts_with("opensuse") || id == "sles" || id == "suse" => Family::Suse,
            _ => continue,
        };
        return fam;
    }
    Family::Unknown
}

fn detect_by_binary() -> Family {
    if which("pacman") {
        Family::Arch
    } else if which("apt-get") {
        Family::Debian
    } else if which("dnf") {
        Family::Fedora
    } else if which("zypper") {
        Family::Suse
    } else if which("apk") {
        Family::Alpine
    } else if which("xbps-install") {
        Family::Void
    } else {
        Family::Unknown
    }
}

pub fn detect() -> Distro {
    let text = fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default();
    let m = parse_os_release(&text);
    let id = m.get("ID").cloned().unwrap_or_else(|| "linux".into());
    let mut ids = vec![id.to_lowercase()];
    if let Some(like) = m.get("ID_LIKE") {
        ids.extend(like.split_whitespace().map(|s| s.to_lowercase()));
    }
    let mut family = family_from_ids(&ids);
    if family == Family::Unknown {
        family = detect_by_binary();
    }
    let pretty = m.get("PRETTY_NAME").cloned().unwrap_or_else(|| id.clone());
    Distro { id, pretty, family }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_ubuntu_derivative() {
        let m = parse_os_release("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\nPRETTY_NAME=\"Linux Mint 21\"\n");
        assert_eq!(m["ID"], "linuxmint");
        let ids = vec!["foo".to_string(), "ubuntu".to_string()];
        assert_eq!(family_from_ids(&ids), Family::Debian);
    }

    #[test]
    fn families() {
        let f = |s: &str| family_from_ids(&[s.to_string()]);
        assert_eq!(f("manjaro"), Family::Arch);
        assert_eq!(f("opensuse-tumbleweed"), Family::Suse);
        assert_eq!(f("rocky"), Family::Fedora);
        assert_eq!(f("void"), Family::Void);
        assert_eq!(f("nixos"), Family::Unknown);
    }
}
