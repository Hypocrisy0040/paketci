// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use crate::model::*;
use crate::util::capture;

fn first_remote(origin: &str) -> &str {
    origin.split(',').next().filter(|s| !s.is_empty()).unwrap_or("flathub")
}

pub fn search(q: &str) -> Vec<Package> {
    capture(
        "flatpak",
        &["search", "--columns=application,name,description,version,remotes", q],
    )
    .map(|s| parse_search(&s))
    .unwrap_or_default()
}

pub fn parse_search(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() < 2 || c[0] == "Application ID" || !c[0].contains('.') {
                return None;
            }
            let mut p = Package::new(Source::Flatpak, c[0].trim());
            p.display = c[1].trim().to_string();
            p.description = c.get(2).map(|s| s.trim()).unwrap_or("").to_string();
            p.version = c.get(3).map(|s| s.trim()).unwrap_or("").to_string();
            p.origin = c.get(4).map(|s| first_remote(s.trim()).to_string()).unwrap_or_default();
            Some(p)
        })
        .collect()
}

pub fn installed() -> Vec<Package> {
    capture(
        "flatpak",
        &["list", "--app", "--columns=application,name,version,installation"],
    )
    .map(|s| {
        s.lines()
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() < 2 || !c[0].contains('.') {
                    return None;
                }
                let mut p = Package::new(Source::Flatpak, c[0].trim());
                p.display = c[1].trim().to_string();
                p.version = c.get(2).map(|s| s.trim()).unwrap_or("").to_string();
                p.extra = c.get(3).map(|s| s.trim()).unwrap_or("system").to_string();
                p.origin = "flatpak".into();
                p.installed = true;
                Some(p)
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn updates() -> Vec<Package> {
    capture(
        "flatpak",
        &["remote-ls", "--updates", "--app", "--columns=application,name,version"],
    )
    .map(|s| {
        s.lines()
            .filter_map(|l| {
                let c: Vec<&str> = l.split('\t').collect();
                if c.len() < 2 || !c[0].contains('.') {
                    return None;
                }
                let mut p = Package::new(Source::Flatpak, c[0].trim());
                p.display = c[1].trim().to_string();
                p.version = format!("→ {}", c.get(2).map(|s| s.trim()).unwrap_or(""));
                p.origin = "flatpak".into();
                p.installed = true;
                Some(p)
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn has_remotes() -> bool {
    capture("flatpak", &["remotes"])
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

pub fn install_cmd(p: &Package) -> Cmd {
    if p.extra == "user" {
        Cmd::user("flatpak", &["install", "--user", "-y", first_remote(&p.origin), &p.name])
    } else {
        Cmd::root("flatpak", &["install", "--system", "-y", first_remote(&p.origin), &p.name])
    }
}

pub fn remove_cmd(p: &Package) -> Cmd {
    if p.extra == "user" {
        Cmd::user("flatpak", &["uninstall", "--user", "-y", &p.name])
    } else {
        Cmd::root("flatpak", &["uninstall", "--system", "-y", &p.name])
    }
}

pub fn update_cmds() -> Vec<Cmd> {
    vec![
        Cmd::root("flatpak", &["update", "--system", "-y"]),
        Cmd::user("flatpak", &["update", "--user", "-y"]),
    ]
}

pub fn update_cmd(p: &Package) -> Cmd {
    if p.extra == "user" {
        Cmd::user("flatpak", &["update", "--user", "-y", &p.name])
    } else {
        Cmd::root("flatpak", &["update", "--system", "-y", &p.name])
    }
}

pub fn add_flathub_cmd() -> Cmd {
    Cmd::root(
        "flatpak",
        &["remote-add", "--if-not-exists", "flathub", "https://dl.flathub.org/repo/flathub.flatpakrepo"],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_output() {
        let t = "org.mozilla.firefox\tFirefox\tFast, Private & Safe Web Browser\t131.0\tflathub\n";
        let v = parse_search(t);
        assert_eq!(v[0].name, "org.mozilla.firefox");
        assert_eq!(v[0].display, "Firefox");
        assert_eq!(v[0].origin, "flathub");
        assert_eq!(v[0].group_key(), "firefox");
    }
}
