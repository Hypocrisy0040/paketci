// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

//! Dağıtımın kendi paket yöneticisi (pacman, apt, dnf, zypper, apk, xbps).

use crate::distro::Family;
use crate::model::*;
use crate::util::*;

const ARCHES: &[&str] = &[
    "x86_64", "noarch", "i686", "i386", "aarch64", "armv7hl", "ppc64le", "s390x", "src",
];

fn terms(q: &str, escape: bool) -> Vec<String> {
    q.split_whitespace()
        .map(|t| if escape { regex_escape(t) } else { t.to_string() })
        .collect()
}

fn with_terms<'a>(base: &[&'a str], t: &'a [String]) -> Vec<&'a str> {
    let mut v: Vec<&str> = base.to_vec();
    v.extend(t.iter().map(|s| s.as_str()));
    v
}

pub fn search(family: Family, q: &str) -> Vec<Package> {
    match family {
        Family::Arch => {
            let t = terms(q, true);
            capture("pacman", &with_terms(&["-Ss"], &t))
                .map(|s| parse_pacman(&s, Source::Native))
        }
        Family::Debian => {
            let t = terms(q, true);
            capture("apt-cache", &with_terms(&["search"], &t)).map(|s| parse_apt(&s))
        }
        Family::Fedora => {
            let t = terms(q, false);
            capture("dnf", &with_terms(&["search", "-q"], &t)).map(|s| parse_dnf(&s))
        }
        Family::Suse => {
            let t = terms(q, false);
            capture(
                "zypper",
                &with_terms(&["--non-interactive", "--no-refresh", "search", "-d"], &t),
            )
            .map(|s| parse_zypper(&s))
        }
        Family::Alpine => {
            let t = terms(q, false);
            capture("apk", &with_terms(&["search", "-v", "-d"], &t)).map(|s| parse_apk(&s))
        }
        Family::Void => {
            let t = terms(q, false);
            capture("xbps-query", &with_terms(&["-Rs"], &t)).map(|s| parse_xbps(&s))
        }
        Family::Unknown => None,
    }
    .unwrap_or_default()
}

pub fn installed(family: Family) -> Vec<Package> {
    let mk = |name: &str, ver: &str, desc: &str| {
        let mut p = Package::new(Source::Native, name);
        p.version = ver.to_string();
        p.description = desc.to_string();
        p.installed = true;
        p.origin = family.manager().to_string();
        p
    };
    match family {
        Family::Arch => capture("pacman", &["-Q"])
            .map(|s| {
                s.lines()
                    .filter_map(|l| {
                        let mut it = l.split_whitespace();
                        Some(mk(it.next()?, it.next().unwrap_or(""), ""))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Family::Debian => capture(
            "dpkg-query",
            &["-W", "-f=${Package}\t${Version}\t${db:Status-Abbrev}\t${binary:Summary}\n"],
        )
        .map(|s| {
            s.lines()
                .filter_map(|l| {
                    let c: Vec<&str> = l.split('\t').collect();
                    if c.len() >= 3 && c[2].trim().starts_with("ii") {
                        Some(mk(c[0], c[1], c.get(3).copied().unwrap_or("")))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default(),
        Family::Fedora | Family::Suse => capture(
            "rpm",
            &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{SUMMARY}\n"],
        )
        .map(|s| {
            s.lines()
                .filter_map(|l| {
                    let c: Vec<&str> = l.split('\t').collect();
                    if c.len() >= 2 && !c[0].starts_with("gpg-pubkey") {
                        Some(mk(c[0], c[1], c.get(2).copied().unwrap_or("")))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default(),
        Family::Alpine => capture("apk", &["info", "-v"])
            .map(|s| {
                s.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| {
                        let (n, v) = split_apk_nv(l.trim());
                        mk(&n, &v, "")
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Family::Void => capture("xbps-query", &["-l"])
            .map(|s| {
                s.lines()
                    .filter_map(|l| {
                        let mut it = l.split_whitespace();
                        if it.next()? != "ii" {
                            return None;
                        }
                        let (n, v) = split_last_dash(it.next()?);
                        let d = it.collect::<Vec<_>>().join(" ");
                        Some(mk(&n, &v, &d))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Family::Unknown => vec![],
    }
}

pub fn updates(family: Family) -> Vec<Package> {
    let mk = |name: &str, ver: String| {
        let mut p = Package::new(Source::Native, name);
        p.version = ver;
        p.installed = true;
        p.origin = family.manager().to_string();
        p
    };
    match family {
        Family::Arch => {
            let text = if which("checkupdates") {
                capture("checkupdates", &[])
            } else {
                capture("pacman", &["-Qu"])
            };
            text.map(|s| parse_arrow_updates(&s, Source::Native)).unwrap_or_default()
        }
        Family::Debian => capture("apt", &["list", "--upgradable"])
            .map(|s| parse_apt_upgradable(&s))
            .unwrap_or_default(),
        Family::Fedora => capture("dnf", &["check-update", "-q"])
            .map(|s| parse_dnf_updates(&s))
            .unwrap_or_default(),
        Family::Suse => capture("zypper", &["--non-interactive", "--no-refresh", "list-updates"])
            .map(|s| {
                s.lines()
                    .filter_map(|l| {
                        let c: Vec<&str> = l.split('|').map(str::trim).collect();
                        if c.len() >= 5 && c[2] != "Name" {
                            Some(mk(c[2], format!("{} → {}", c[3], c[4])))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Family::Alpine => capture("apk", &["version", "-l", "<"])
            .map(|s| {
                s.lines()
                    .filter_map(|l| {
                        let t: Vec<&str> = l.split_whitespace().collect();
                        if t.len() >= 3 && t[1] == "<" {
                            let (n, old) = split_apk_nv(t[0]);
                            Some(mk(&n, format!("{} → {}", old, t[2])))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Family::Void => capture("xbps-install", &["-un"])
            .map(|s| {
                s.lines()
                    .filter_map(|l| {
                        let t: Vec<&str> = l.split_whitespace().collect();
                        if t.len() >= 2 && t[1] == "update" {
                            let (n, v) = split_last_dash(t[0]);
                            Some(mk(&n, format!("→ {}", v)))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Family::Unknown => vec![],
    }
}

// ---------------------------------------------------------------- komutlar

fn cmd_with(prog: &str, base: &[&str], names: &[String]) -> Cmd {
    let mut args: Vec<&str> = base.to_vec();
    args.extend(names.iter().map(|s| s.as_str()));
    Cmd::root(prog, &args)
}

pub fn install_cmd(f: Family, names: &[String]) -> Option<Cmd> {
    Some(match f {
        Family::Arch => cmd_with("pacman", &["-S", "--needed"], names),
        Family::Debian => cmd_with("apt", &["install"], names),
        Family::Fedora => cmd_with("dnf", &["install"], names),
        Family::Suse => cmd_with("zypper", &["install"], names),
        Family::Alpine => cmd_with("apk", &["add"], names),
        Family::Void => cmd_with("xbps-install", &["-S"], names),
        Family::Unknown => return None,
    })
}

pub fn remove_cmd(f: Family, names: &[String]) -> Option<Cmd> {
    Some(match f {
        Family::Arch => cmd_with("pacman", &["-Rns"], names),
        Family::Debian => cmd_with("apt", &["remove"], names),
        Family::Fedora => cmd_with("dnf", &["remove"], names),
        Family::Suse => cmd_with("zypper", &["remove"], names),
        Family::Alpine => cmd_with("apk", &["del"], names),
        Family::Void => cmd_with("xbps-remove", &["-R"], names),
        Family::Unknown => return None,
    })
}

pub fn update_cmds(f: Family, aur_helper: Option<&str>) -> Vec<Cmd> {
    match f {
        Family::Arch => match aur_helper {
            // yay/paru -Syu hem depoları hem AUR'u günceller
            Some(h) => vec![Cmd::user(h, &["-Syu"])],
            None => vec![Cmd::root("pacman", &["-Syu"])],
        },
        Family::Debian => vec![Cmd::root("apt", &["update"]), Cmd::root("apt", &["upgrade"])],
        Family::Fedora => vec![Cmd::root("dnf", &["upgrade", "--refresh"])],
        Family::Suse => vec![Cmd::root("zypper", &["refresh"]), Cmd::root("zypper", &["update"])],
        Family::Alpine => vec![Cmd::root("apk", &["update"]), Cmd::root("apk", &["upgrade"])],
        Family::Void => vec![Cmd::root("xbps-install", &["-Su"])],
        Family::Unknown => vec![],
    }
}

/// Yarım kalan kurulumları tamamlar, bağımlılık sorunlarını giderir. Arch'ta
/// yalnızca denetler: otomatik bir onarım komutu yok, zorlamak sistemi bozabilir.
pub fn repair_cmds(f: Family) -> Vec<Cmd> {
    match f {
        Family::Debian => vec![Cmd::root("dpkg", &["--configure", "-a"]), Cmd::root("apt", &["--fix-broken", "install"])],
        Family::Arch => vec![Cmd::user("pacman", &["-Dk"])],
        Family::Fedora => vec![Cmd::root("dnf", &["distro-sync"])],
        Family::Suse => vec![Cmd::root("zypper", &["verify"])],
        Family::Alpine => vec![Cmd::root("apk", &["fix"])],
        Family::Void => vec![Cmd::root("xbps-reconfigure", &["-a"]), Cmd::root("xbps-pkgdb", &["-a"])],
        Family::Unknown => vec![],
    }
}

fn output_lines(text: Option<String>) -> Vec<String> {
    text.map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

/// Kullanıcının kendi kurduğu paketler (otomatik gelen bağımlılıklar hariç).
pub fn manual_packages(f: Family) -> Vec<String> {
    match f {
        Family::Arch => output_lines(capture("pacman", &["-Qqen"])),
        Family::Debian => output_lines(capture("apt-mark", &["showmanual"])),
        Family::Fedora => output_lines(capture("dnf", &["repoquery", "-q", "--userinstalled", "--qf", "%{name}\n"])),
        Family::Suse => {
            let auto: std::collections::HashSet<String> =
                output_lines(std::fs::read_to_string("/var/lib/zypp/AutoInstalled").ok()).into_iter().collect();
            output_lines(capture("rpm", &["-qa", "--qf", "%{NAME}\n"]))
                .into_iter()
                .filter(|n| !auto.contains(n) && !n.starts_with("gpg-pubkey"))
                .collect()
        }
        Family::Alpine => std::fs::read_to_string("/etc/apk/world")
            .map(|s| crate::pkglist::parse_apk_world(&s))
            .unwrap_or_default(),
        Family::Void => capture("xbps-query", &["-m"])
            .map(|s| crate::pkglist::parse_xbps_manual(&s))
            .unwrap_or_default(),
        Family::Unknown => vec![],
    }
}

/// Eski çekirdeğin modüllerini silen dağıtımlarda çalışan çekirdek artık kurulu
/// değilse yeniden başlatma gerekir. Modül yoksa (kapsayıcı, WSL) karar verilmez.
fn kernel_outdated(running: &str, installed: &[String]) -> bool {
    !running.is_empty() && !installed.is_empty() && !installed.iter().any(|m| m == running)
}

fn running_kernel_removed() -> bool {
    let running = capture("uname", &["-r"]).unwrap_or_default();
    let installed: Vec<String> = ["/usr/lib/modules", "/lib/modules"]
        .iter()
        .find_map(|dir| std::fs::read_dir(dir).ok())
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    kernel_outdated(running.trim(), &installed)
}

/// Void eski çekirdekleri sakladığı için güvenilir bir işaret yok; `false` döner.
pub fn reboot_required(f: Family) -> bool {
    let says_required = |text: Option<String>| text.map_or(false, |s| s.contains("Reboot is required"));
    match f {
        Family::Debian => std::path::Path::new("/run/reboot-required").exists() || std::path::Path::new("/var/run/reboot-required").exists(),
        Family::Arch | Family::Alpine => running_kernel_removed(),
        Family::Fedora => says_required(capture("dnf", &["needs-restarting", "-r"])),
        Family::Suse => says_required(capture("zypper", &["--non-interactive", "needs-rebooting"])),
        Family::Void | Family::Unknown => false,
    }
}

fn sorted_unique(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

/// `pacman -F`: "repo/ad sürüm" satırını girintili dosya yolları izler.
pub fn parse_pacman_files(text: &str) -> Vec<String> {
    let mut current = None;
    let mut names = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) {
            let path = line.trim();
            if path.starts_with("usr/bin/") || path.starts_with("usr/sbin/") {
                names.extend(current.clone());
            }
        } else {
            current = line.split_whitespace().next().and_then(|t| t.rsplit('/').next()).map(String::from);
        }
    }
    sorted_unique(names)
}

/// `apt-file search`: "paket: /usr/bin/komut"
pub fn parse_apt_file(text: &str) -> Vec<String> {
    sorted_unique(text.lines().filter_map(|l| l.split_once(": ").map(|(n, _)| n.to_string())).collect())
}

/// `dnf provides`: "ad-epoch:sürüm-devir.mimari : özet" ve ayrıntı satırları
pub fn parse_dnf_provides(text: &str) -> Vec<String> {
    let names = text
        .lines()
        .filter(|l| !["Repo", "Matched", "Filename", "Provide", "Last"].iter().any(|p| l.starts_with(p)))
        .filter_map(|l| l.split_once(" : ").map(|(head, _)| head.trim()))
        .filter_map(|head| head.rsplitn(3, '-').nth(2).map(String::from))
        .collect();
    sorted_unique(names)
}

/// `zypper search --provides`: "S | Ad | Özet | Tür" tablosu
pub fn parse_zypper_provides(text: &str) -> Vec<String> {
    let names = text
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('|').map(str::trim).collect();
            (c.len() >= 4 && c[3] == "package").then(|| c[1].to_string())
        })
        .collect();
    sorted_unique(names)
}

/// Verilen komutu sağlayan paketler. Debian'da `apt-file`, Arch'ta güncel
/// `pacman -Fy` veritabanı gerekir; Void'de hızlı bir yol olmadığı için boş döner.
pub fn providers(f: Family, command: &str) -> Vec<String> {
    let safe = !command.starts_with('-') && !command.is_empty() && command.chars().all(|c| c.is_ascii_alphanumeric() || "+._-".contains(c));
    if !safe {
        return vec![];
    }
    let path = format!("/usr/bin/{command}");
    match f {
        Family::Arch => capture("pacman", &["-F", command]).map(|s| parse_pacman_files(&s)),
        Family::Debian if which("apt-file") => {
            let pattern = format!("^/(usr/)?s?bin/{}$", regex_escape(command));
            capture("apt-file", &["search", "--regexp", &pattern]).map(|s| parse_apt_file(&s))
        }
        Family::Fedora => capture("dnf", &["provides", "-q", &path]).map(|s| parse_dnf_provides(&s)),
        Family::Suse => capture("zypper", &["--non-interactive", "--no-refresh", "search", "--provides", "--match-exact", &path])
            .map(|s| parse_zypper_provides(&s)),
        Family::Alpine => capture("apk", &["search", "--exact", &format!("cmd:{command}")])
            .map(|s| sorted_unique(output_lines(Some(s)).iter().map(|l| split_apk_nv(l).0).collect())),
        _ => None,
    }
    .unwrap_or_default()
}

/// Yalnızca verilen paketleri günceller. Arch'ta bilinçli olarak yok: tek tek
/// güncelleme kısmi güncelleme demektir ve sistemi bozabilir.
pub fn upgrade_cmd(f: Family, names: &[String]) -> Option<Cmd> {
    Some(match f {
        Family::Debian => cmd_with("apt", &["install", "--only-upgrade"], names),
        Family::Fedora => cmd_with("dnf", &["upgrade"], names),
        Family::Suse => cmd_with("zypper", &["update"], names),
        Family::Alpine => cmd_with("apk", &["add", "--upgrade"], names),
        Family::Void => cmd_with("xbps-install", &["-Su"], names),
        Family::Arch | Family::Unknown => return None,
    })
}

/// Depo meta verisini yeniler. Arch'ta bilinçli olarak boş: `pacman -Sy`
/// kısmi güncellemeye yol açar, `checkupdates` zaten geçici veritabanı kullanır.
pub fn refresh_cmds(f: Family) -> Vec<Cmd> {
    match f {
        Family::Debian => vec![Cmd::root("apt", &["update"])],
        Family::Fedora => vec![Cmd::root("dnf", &["makecache"])],
        Family::Suse => vec![Cmd::root("zypper", &["refresh"])],
        Family::Alpine => vec![Cmd::root("apk", &["update"])],
        Family::Void => vec![Cmd::root("xbps-install", &["-S"])],
        Family::Arch | Family::Unknown => vec![],
    }
}

pub fn autoremove_cmd(f: Family) -> Option<Cmd> {
    Some(match f {
        Family::Arch => Cmd::shell(
            true,
            "orphans=$(pacman -Qdtq); if [ -z \"$orphans\" ]; then echo 'Yetim paket yok.'; else pacman -Rns $orphans; fi",
        ),
        Family::Debian => Cmd::root("apt", &["autoremove"]),
        Family::Fedora => Cmd::root("dnf", &["autoremove"]),
        Family::Void => Cmd::root("xbps-remove", &["-o"]),
        _ => return None,
    })
}

pub fn clean_cmd(f: Family) -> Option<Cmd> {
    Some(match f {
        Family::Arch => Cmd::root("pacman", &["-Sc"]),
        Family::Debian => Cmd::root("apt", &["clean"]),
        Family::Fedora => Cmd::root("dnf", &["clean", "all"]),
        Family::Suse => Cmd::root("zypper", &["clean", "--all"]),
        Family::Alpine => Cmd::root("apk", &["cache", "clean"]),
        Family::Void => Cmd::root("xbps-remove", &["-O"]),
        Family::Unknown => return None,
    })
}

// ---------------------------------------------------------------- ayrıştırıcılar

pub fn parse_pacman(text: &str, source: Source) -> Vec<Package> {
    let mut out = Vec::new();
    let mut cur: Option<Package> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            if let Some(p) = cur.as_mut() {
                if !p.description.is_empty() {
                    p.description.push(' ');
                }
                p.description.push_str(line.trim());
            }
            continue;
        }
        if let Some(p) = cur.take() {
            out.push(p);
        }
        let mut it = line.split_whitespace();
        let Some(rn) = it.next() else { continue };
        let Some((repo, name)) = rn.split_once('/') else { continue };
        let mut p = Package::new(source, name);
        p.version = it.next().unwrap_or("").to_string();
        p.origin = if source == Source::Aur { "AUR".into() } else { repo.to_string() };
        p.installed = line.to_lowercase().contains("[installed");
        cur = Some(p);
    }
    if let Some(p) = cur {
        out.push(p);
    }
    out
}

pub fn parse_apt(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let (n, d) = l.split_once(" - ")?;
            let mut p = Package::new(Source::Native, n.trim());
            p.description = d.trim().to_string();
            Some(p)
        })
        .collect()
}

fn split_arch(s: &str) -> Option<(&str, &str)> {
    let (n, a) = s.rsplit_once('.')?;
    ARCHES.contains(&a).then_some((n, a))
}

pub fn parse_dnf(text: &str) -> Vec<Package> {
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('=') || l.contains("Matched") {
            continue;
        }
        let mut it = l.splitn(2, char::is_whitespace);
        let Some(first) = it.next() else { continue };
        let Some((name, _)) = split_arch(first) else { continue };
        let rest = it.next().unwrap_or("").trim();
        let rest = rest.strip_prefix(':').unwrap_or(rest).trim();
        let mut p = Package::new(Source::Native, name);
        p.description = rest.to_string();
        out.push(p);
    }
    out
}

pub fn parse_zypper(text: &str) -> Vec<Package> {
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.contains('|') {
            continue;
        }
        let c: Vec<&str> = line.split('|').map(str::trim).collect();
        if c.len() < 3 || c[1] == "Name" {
            continue;
        }
        if c.len() >= 4 && c[3] != "package" {
            continue;
        }
        let mut p = Package::new(Source::Native, c[1]);
        p.description = c[2].to_string();
        out.push(p);
    }
    out
}

/// "firefox-131.0.1-r0" -> ("firefox", "131.0.1-r0")
pub fn split_apk_nv(s: &str) -> (String, String) {
    let base = match s.rfind("-r") {
        Some(i) if i + 2 < s.len() && s[i + 2..].chars().all(|c| c.is_ascii_digit()) => &s[..i],
        _ => s,
    };
    let idx = base
        .char_indices()
        .filter(|(i, c)| {
            *c == '-' && base[i + 1..].chars().next().map_or(false, |d| d.is_ascii_digit())
        })
        .map(|(i, _)| i)
        .last();
    match idx {
        Some(i) => (s[..i].to_string(), s[i + 1..].to_string()),
        None => (s.to_string(), String::new()),
    }
}

pub fn parse_apk(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let (nv, desc) = match l.split_once(" - ") {
                Some((a, b)) => (a.trim(), b.trim()),
                None => (l.trim(), ""),
            };
            if nv.is_empty() {
                return None;
            }
            let (n, v) = split_apk_nv(nv);
            let mut p = Package::new(Source::Native, &n);
            p.version = v;
            p.description = desc.to_string();
            Some(p)
        })
        .collect()
}

pub fn parse_xbps(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            if !l.starts_with('[') || l.len() < 4 {
                return None;
            }
            let installed = l.starts_with("[*]");
            let rest = l[3..].trim();
            let mut it = rest.splitn(2, char::is_whitespace);
            let (n, v) = split_last_dash(it.next()?);
            let mut p = Package::new(Source::Native, &n);
            p.version = v;
            p.description = it.next().unwrap_or("").trim().to_string();
            p.installed = installed;
            Some(p)
        })
        .collect()
}

pub fn parse_arrow_updates(text: &str, source: Source) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.len() >= 4 && t[2] == "->" {
                let mut p = Package::new(source, t[0]);
                p.version = format!("{} → {}", t[1], t[3]);
                p.installed = true;
                Some(p)
            } else {
                None
            }
        })
        .collect()
}

pub fn parse_apt_upgradable(text: &str) -> Vec<Package> {
    text.lines()
        .filter_map(|l| {
            if l.starts_with("Listing") {
                return None;
            }
            let (name, rest) = l.split_once('/')?;
            let t: Vec<&str> = rest.split_whitespace().collect();
            let new = t.get(1)?;
            let old = l
                .split("upgradable from: ")
                .nth(1)
                .map(|s| s.trim_end_matches(']').trim())
                .unwrap_or("?");
            let mut p = Package::new(Source::Native, name);
            p.version = format!("{} → {}", old, new);
            p.installed = true;
            Some(p)
        })
        .collect()
}

pub fn parse_dnf_updates(text: &str) -> Vec<Package> {
    let mut out = Vec::new();
    for l in text.lines() {
        if l.starts_with("Obsoleting") {
            break;
        }
        let t: Vec<&str> = l.split_whitespace().collect();
        if t.len() >= 3 {
            if let Some((name, _)) = split_arch(t[0]) {
                let mut p = Package::new(Source::Native, name);
                p.version = format!("→ {}", t[1]);
                p.installed = true;
                out.push(p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_providers() {
        assert_eq!(
            parse_pacman_files("extra/imagemagick 7.1-1 (imagemagick)\n    usr/bin/convert\nextra/foo 1-1\n    usr/share/foo/convert\n"),
            ["imagemagick"]
        );
        assert_eq!(parse_apt_file("imagemagick-6.q16: /usr/bin/convert\n"), ["imagemagick-6.q16"]);
        assert_eq!(
            parse_dnf_provides("ImageMagick-1:7.1.1.15-1.fc39.x86_64 : An X application\nRepo        : fedora\nFilename    : /usr/bin/convert\n"),
            ["ImageMagick"]
        );
        assert_eq!(
            parse_zypper_provides("S  | Name        | Summary | Type\n---+-------------+---------+--------\n   | ImageMagick | Viewer  | package\n"),
            ["ImageMagick"]
        );
        assert!(providers(Family::Arch, "-x").is_empty());
        assert!(providers(Family::Arch, "a b").is_empty());
    }

    #[test]
    fn kernel_check() {
        let installed = vec!["6.10.1-arch1-1".to_string()];
        assert!(kernel_outdated("6.9.7-arch1-1", &installed));
        assert!(!kernel_outdated("6.10.1-arch1-1", &installed));
        assert!(!kernel_outdated("6.9.7-arch1-1", &[]));
    }

    #[test]
    fn pacman_format() {
        let t = "extra/firefox 131.0-1 [installed]\n    Fast, Private & Safe Web Browser\nextra/firefox-i18n-tr 131.0-1\n    Turkish language pack\n";
        let v = parse_pacman(t, Source::Native);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "firefox");
        assert_eq!(v[0].origin, "extra");
        assert!(v[0].installed);
        assert_eq!(v[1].description, "Turkish language pack");
    }

    #[test]
    fn yay_aur_format() {
        let t = "aur/yay-bin 12.3.5-1 (+1500 40.1) (Installed)\n    Yet another yogurt\n";
        let v = parse_pacman(t, Source::Aur);
        assert_eq!(v[0].origin, "AUR");
        assert_eq!(v[0].source, Source::Aur);
    }

    #[test]
    fn apt_format() {
        let v = parse_apt("firefox - Safe and easy web browser from Mozilla\nvlc - multimedia player\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "firefox");
        assert_eq!(v[1].description, "multimedia player");
    }

    #[test]
    fn dnf_formats() {
        let t = "========= Name Matched: firefox =========\nfirefox.x86_64 : Mozilla Firefox Web browser\nfirefox-langpacks.noarch : Firefox langpacks\n";
        let v = parse_dnf(t);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].name, "firefox");
        assert_eq!(v[1].name, "firefox-langpacks");
        // dnf5 tarzı (iki nokta yok)
        let v5 = parse_dnf("Matched fields: name\n firefox.x86_64 Mozilla Firefox\n");
        assert_eq!(v5[0].description, "Mozilla Firefox");
    }

    #[test]
    fn zypper_format() {
        let t = "S  | Name    | Summary         | Type\n---+---------+-----------------+--------\ni+ | firefox | Mozilla Firefox | package\n   | firefox | Mozilla Firefox | srcpackage\n";
        let v = parse_zypper(t);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "firefox");
    }

    #[test]
    fn apk_split_and_parse() {
        assert_eq!(split_apk_nv("firefox-131.0.1-r0"), ("firefox".into(), "131.0.1-r0".into()));
        assert_eq!(split_apk_nv("libfoo-2-dev-2.1-r3"), ("libfoo-2-dev".into(), "2.1-r3".into()));
        let v = parse_apk("firefox-131.0.1-r0 - Web browser\n");
        assert_eq!(v[0].name, "firefox");
        assert_eq!(v[0].description, "Web browser");
    }

    #[test]
    fn xbps_format() {
        let v = parse_xbps("[-] firefox-131.0_1       Mozilla Firefox web browser\n[*] vlc-3.0.20_3 VLC player\n");
        assert_eq!(v[0].name, "firefox");
        assert_eq!(v[0].version, "131.0_1");
        assert!(!v[0].installed && v[1].installed);
    }

    #[test]
    fn update_formats() {
        let a = parse_arrow_updates("linux 6.10.1-1 -> 6.10.2-1\n", Source::Native);
        assert_eq!(a[0].version, "6.10.1-1 → 6.10.2-1");
        let d = parse_apt_upgradable("Listing...\ncurl/noble-updates 8.5.0-2 amd64 [upgradable from: 8.4.0-1]\n");
        assert_eq!(d[0].name, "curl");
        assert_eq!(d[0].version, "8.4.0-1 → 8.5.0-2");
        let f = parse_dnf_updates("\nkernel.x86_64   6.10.3-200.fc40   updates\nObsoleting Packages\nfoo.x86_64 1 x\n");
        assert_eq!(f.len(), 1);
    }
}
