// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use crate::util::{escalator, is_root};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Source {
    Native,
    Aur,
    Flatpak,
    Snap,
}

impl Source {
    pub fn badge(self) -> &'static str {
        match self {
            Source::Native => "REPO",
            Source::Aur => "AUR",
            Source::Flatpak => "FLATPAK",
            Source::Snap => "SNAP",
        }
    }
    pub fn order(self) -> u8 {
        match self {
            Source::Native => 0,
            Source::Aur => 1,
            Source::Flatpak => 2,
            Source::Snap => 3,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Package {
    /// Kurulumda kullanılan kimlik (Flatpak için uygulama kimliği)
    pub name: String,
    /// Kullanıcıya gösterilen ad
    pub display: String,
    pub version: String,
    pub description: String,
    pub source: Source,
    /// Depo / remote adı
    pub origin: String,
    /// Kaynağa özel ek bilgi (snap: "classic", flatpak: "user"/"system")
    pub extra: String,
    pub installed: bool,
    pub score: f64,
}

impl Package {
    pub fn new(source: Source, name: &str) -> Self {
        Self {
            name: name.to_string(),
            display: name.to_string(),
            version: String::new(),
            description: String::new(),
            source,
            origin: String::new(),
            extra: String::new(),
            installed: false,
            score: 0.0,
        }
    }

    pub fn key(&self) -> (Source, String) {
        (self.source, self.name.clone())
    }

    /// Aynı uygulamanın farklı kaynaklardaki sürümlerini yan yana getirmek için.
    pub fn group_key(&self) -> String {
        match self.source {
            Source::Flatpak => self
                .name
                .rsplit('.')
                .next()
                .unwrap_or(&self.name)
                .to_lowercase(),
            _ => {
                let n = self.name.to_lowercase();
                for suf in ["-bin", "-git", "-appimage"] {
                    if let Some(s) = n.strip_suffix(suf) {
                        return s.to_string();
                    }
                }
                n
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    pub root: bool,
}

impl Cmd {
    pub fn root(program: &str, args: &[&str]) -> Self {
        Self::build(true, program, args)
    }
    pub fn user(program: &str, args: &[&str]) -> Self {
        Self::build(false, program, args)
    }
    pub fn shell(root: bool, script: &str) -> Self {
        Self::build(root, "sh", &["-c", script])
    }
    fn build(root: bool, program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            root,
        }
    }
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.root && !is_root() {
            s.push_str(escalator().unwrap_or("sudo"));
            s.push(' ');
        }
        s.push_str(&self.program);
        for a in &self.args {
            s.push(' ');
            if a.contains(char::is_whitespace) {
                s.push('\'');
                s.push_str(a);
                s.push('\'');
            } else {
                s.push_str(a);
            }
        }
        s
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskAction {
    Commands,
    SwitchLanguage,
    ExportPackages,
    ImportPackages,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub title: String,
    pub desc: String,
    pub cmds: Vec<Cmd>,
    pub action: TaskAction,
}
