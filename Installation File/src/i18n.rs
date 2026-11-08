// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! Arayüz dili: kayıtlı seçim, sistem dili, o da yoksa kullanıcıya sorma.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::distro::parse_os_release;
use crate::util::xdg_dir;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Tr,
    En,
}

/// Dil seçim penceresinde gösterilen diller.
pub const LANGUAGES: [(Lang, &str); 2] = [(Lang::Tr, "Türkçe"), (Lang::En, "English")];

static TURKISH: AtomicBool = AtomicBool::new(false);

pub fn lang() -> Lang {
    if TURKISH.load(Ordering::Relaxed) {
        Lang::Tr
    } else {
        Lang::En
    }
}

pub fn set(l: Lang) {
    TURKISH.store(l == Lang::Tr, Ordering::Relaxed);
}

/// Etkin dile göre metni seçer.
pub fn t<T>(tr: T, en: T) -> T {
    match lang() {
        Lang::Tr => tr,
        Lang::En => en,
    }
}

/// "tr_TR.UTF-8" -> Türkçe, başka bir dil -> İngilizce. "C" ve "POSIX" dil
/// bilgisi taşımadığı için `None` döner.
fn parse_locale(value: &str) -> Option<Lang> {
    let first = value.split(':').next().unwrap_or("").trim().to_lowercase();
    let code = first.split(['_', '.', '@', '-']).next().unwrap_or("");
    if code.is_empty() || code == "c" || code == "posix" {
        return None;
    }
    Some(if code == "tr" { Lang::Tr } else { Lang::En })
}

fn detect_env(get: impl Fn(&str) -> Option<String>) -> Option<Lang> {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| get(k))
        .find_map(|v| parse_locale(&v))
}

fn detect_locale_file(values: &HashMap<String, String>) -> Option<Lang> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| values.get(*k))
        .find_map(|v| parse_locale(v))
}

fn detect_system() -> Option<Lang> {
    detect_env(|k| std::env::var(k).ok()).or_else(|| {
        // Masaüstü menüsünden açılınca ortam değişkenleri eksik olabilir
        ["/etc/locale.conf", "/etc/default/locale"].iter().find_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            detect_locale_file(&parse_os_release(&text))
        })
    })
}

fn config_path() -> Option<PathBuf> {
    Some(xdg_dir("XDG_CONFIG_HOME", ".config")?.join("paketci").join("config"))
}

fn load_saved() -> Option<Lang> {
    let text = fs::read_to_string(config_path()?).ok()?;
    match parse_os_release(&text).get("lang").map(String::as_str) {
        Some("tr") => Some(Lang::Tr),
        Some("en") => Some(Lang::En),
        _ => None,
    }
}

/// Kullanıcının seçimini kaydeder; sonraki açılışlarda sistem dilinin önüne geçer.
pub fn save(l: Lang) -> std::io::Result<()> {
    let path = config_path().ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, format!("lang={}\n", if l == Lang::Tr { "tr" } else { "en" }))
}

/// Dili belirler. Hiçbir yerden bulunamazsa `true` döner; o zaman kullanıcıya sorulur.
pub fn init() -> bool {
    match load_saved().or_else(detect_system) {
        Some(l) => {
            set(l);
            false
        }
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    #[test]
    fn locale_values() {
        assert_eq!(parse_locale("tr_TR.UTF-8"), Some(Lang::Tr));
        assert_eq!(parse_locale("tr"), Some(Lang::Tr));
        assert_eq!(parse_locale("tr:en_US"), Some(Lang::Tr));
        assert_eq!(parse_locale("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(parse_locale("de_DE@euro"), Some(Lang::En));
        assert_eq!(parse_locale("C"), None);
        assert_eq!(parse_locale("C.UTF-8"), None);
        assert_eq!(parse_locale("POSIX"), None);
        assert_eq!(parse_locale(""), None);
    }

    #[test]
    fn env_priority() {
        assert_eq!(detect_env(env(&[("LANG", "en_US.UTF-8"), ("LC_ALL", "tr_TR.UTF-8")])), Some(Lang::Tr));
        assert_eq!(detect_env(env(&[("LC_ALL", "C"), ("LANG", "tr_TR.UTF-8")])), Some(Lang::Tr));
        assert_eq!(detect_env(env(&[("LANGUAGE", "fr:tr"), ("LANG", "tr_TR.UTF-8")])), Some(Lang::En));
        assert_eq!(detect_env(env(&[("LANG", "C.UTF-8")])), None);
        assert_eq!(detect_env(env(&[])), None);
    }

    #[test]
    fn locale_file() {
        assert_eq!(detect_locale_file(&parse_os_release("LANG=\"tr_TR.UTF-8\"\n")), Some(Lang::Tr));
        assert_eq!(detect_locale_file(&parse_os_release("# yorum\nLANG=C\n")), None);
    }
}
