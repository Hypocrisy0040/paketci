// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

pub mod aur;
pub mod flatpak;
pub mod impact;
pub mod native;
pub mod snap;

use std::collections::{HashMap, HashSet};

use crate::distro::{Distro, Family};
use crate::fuzzy;
use crate::history::{self, History};
use crate::i18n::t;
use crate::model::*;
use crate::pkglist::Entry;
use crate::util::which;

#[derive(Clone)]
pub struct Tools {
    pub aur_helper: Option<String>,
    pub flatpak: bool,
    pub snap: bool,
}

impl Tools {
    pub fn detect(d: &Distro) -> Self {
        let aur_helper = if d.family == Family::Arch {
            ["paru", "yay", "pikaur"]
                .iter()
                .find(|h| which(h))
                .map(|h| h.to_string())
        } else {
            None
        };
        Tools { aur_helper, flatpak: which("flatpak"), snap: which("snap") }
    }
}

#[derive(Clone)]
pub struct Ctx {
    pub distro: Distro,
    pub tools: Tools,
    pub history: History,
}

impl Ctx {
    pub fn new() -> Self {
        let distro = crate::distro::detect();
        let tools = Tools::detect(&distro);
        Ctx { distro, tools, history: history::load() }
    }

    pub fn sources(&self) -> Vec<Source> {
        let mut v = Vec::new();
        if self.distro.family != Family::Unknown {
            v.push(Source::Native);
        }
        if self.tools.aur_helper.is_some() {
            v.push(Source::Aur);
        }
        if self.tools.flatpak {
            v.push(Source::Flatpak);
        }
        if self.tools.snap {
            v.push(Source::Snap);
        }
        v
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.distro.family != Family::Unknown {
            parts.push(self.distro.family.manager().to_string());
        }
        if let Some(h) = &self.tools.aur_helper {
            parts.push(h.clone());
        }
        if self.tools.flatpak {
            parts.push("flatpak".into());
        }
        if self.tools.snap {
            parts.push("snap".into());
        }
        parts.join(" • ")
    }
}

// ---------------------------------------------------------------- arama

fn raw_search(ctx: &Ctx, s: Source, q: &str) -> Vec<Package> {
    match s {
        Source::Native => {
            let mut v = native::search(ctx.distro.family, q);
            for p in &mut v {
                if p.origin.is_empty() {
                    p.origin = ctx.distro.family.manager().to_string();
                }
            }
            v
        }
        Source::Aur => ctx
            .tools
            .aur_helper
            .as_deref()
            .map(|h| aur::search(h, q))
            .unwrap_or_default(),
        Source::Flatpak => flatpak::search(q),
        Source::Snap => snap::search(q),
    }
}

fn merge(map: &mut HashMap<String, Package>, found: Vec<Package>) {
    for p in found {
        map.entry(p.name.clone()).or_insert(p);
    }
}

/// Tek bir kaynakta, yazım hatalarına dayanıklı arama yapar.
pub fn search_source(ctx: &Ctx, source: Source, query: &str) -> (Vec<Package>, Option<String>) {
    let q = fuzzy::normalize(query);
    if q.is_empty() {
        return (vec![], None);
    }
    let mut map: HashMap<String, Package> = HashMap::new();
    let first = raw_search(ctx, source, &q);
    let strong = first.iter().any(|p| fuzzy::score(&q, p) >= 0.70);
    merge(&mut map, first);
    // Takma adlar her zaman denenir: "chrome" güçlü ama alakasız bir sonuç bulabilir
    for alias in fuzzy::aliases(&q) {
        merge(&mut map, raw_search(ctx, source, alias));
    }
    let mut providers = Vec::new();
    if !strong {
        // Güçlü bir isim eşleşmesi yok: yazım hatası olabilir, alternatifleri dene
        for v in fuzzy::variants(&q).into_iter().skip(1) {
            merge(&mut map, raw_search(ctx, source, &v));
        }
        if source == Source::Native {
            providers = native::providers(ctx.distro.family, &q);
            for name in &providers {
                map.entry(name.clone()).or_insert_with(|| {
                    let mut p = Package::new(Source::Native, name);
                    p.origin = ctx.distro.family.manager().to_string();
                    p.description = t(format!("`{q}` komutunu sağlar"), format!("Provides the `{q}` command"));
                    p
                });
            }
        }
    }

    let inst: HashMap<String, String> = list_installed(ctx, source)
        .into_iter()
        .map(|p| (p.name, p.version))
        .collect();

    let mut out: Vec<Package> = map
        .into_values()
        .filter_map(|mut p| {
            p.score = fuzzy::score(&q, &p);
            if providers.contains(&p.name) {
                p.score = p.score.max(0.85);
            }
            let hay = fuzzy::fold(&format!("{} {} {}", p.name, p.display, p.description));
            let keep = p.score >= 0.55 || hay.contains(&q);
            if !keep {
                return None;
            }
            if let Some(count) = ctx.history.get(&p.name) {
                p.score += history::boost(*count);
            }
            if let Some(v) = inst.get(&p.name) {
                p.installed = true;
                if p.version.is_empty() {
                    p.version = v.clone();
                }
            }
            Some(p)
        })
        .collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(200);

    let note = if source == Source::Flatpak && out.is_empty() && !flatpak::has_remotes() {
        Some(t("Flatpak deposu ekli değil (Bakım sekmesinden Flathub'ı ekleyin)", "No Flatpak remote configured (add Flathub from the Maintenance tab)").to_string())
    } else {
        None
    };
    (out, note)
}

// ---------------------------------------------------------------- listeler

pub fn list_installed(ctx: &Ctx, s: Source) -> Vec<Package> {
    match s {
        Source::Native => native::installed(ctx.distro.family),
        Source::Aur => aur::installed(),
        Source::Flatpak => flatpak::installed(),
        Source::Snap => snap::installed(),
    }
}

pub fn list_installed_all(ctx: &Ctx) -> Vec<Package> {
    let mut all = Vec::new();
    let mut native = native::installed(ctx.distro.family);
    if ctx.distro.family == Family::Arch {
        let foreign: std::collections::HashSet<String> =
            aur::installed().into_iter().map(|p| p.name).collect();
        for p in &mut native {
            if foreign.contains(&p.name) {
                p.source = Source::Aur;
                p.origin = "AUR".into();
            }
        }
    }
    all.extend(native);
    if ctx.tools.flatpak {
        all.extend(flatpak::installed());
    }
    if ctx.tools.snap {
        all.extend(snap::installed());
    }
    all.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    all
}

pub fn list_updates(ctx: &Ctx) -> Vec<Package> {
    let mut all = native::updates(ctx.distro.family);
    if let Some(h) = &ctx.tools.aur_helper {
        all.extend(aur::updates(h));
    }
    if ctx.tools.flatpak {
        all.extend(flatpak::updates());
    }
    if ctx.tools.snap {
        all.extend(snap::updates());
    }
    all
}

// ---------------------------------------------------------------- komutlar

fn names_of(pkgs: &[Package], pred: impl Fn(Source) -> bool) -> Vec<String> {
    pkgs.iter().filter(|p| pred(p.source)).map(|p| p.name.clone()).collect()
}

pub fn install_cmds(ctx: &Ctx, pkgs: &[Package]) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    let native = names_of(pkgs, |s| s == Source::Native);
    if !native.is_empty() {
        cmds.extend(native::install_cmd(ctx.distro.family, &native));
    }
    let aur = names_of(pkgs, |s| s == Source::Aur);
    if !aur.is_empty() {
        if let Some(h) = &ctx.tools.aur_helper {
            cmds.push(aur::install_cmd(h, &aur));
        }
    }
    for p in pkgs.iter().filter(|p| p.source == Source::Flatpak) {
        cmds.push(flatpak::install_cmd(p));
    }
    for p in pkgs.iter().filter(|p| p.source == Source::Snap) {
        cmds.push(snap::install_cmd(p));
    }
    cmds
}

pub fn remove_cmds(ctx: &Ctx, pkgs: &[Package]) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    // AUR'dan kurulanlar da pacman ile kaldırılır
    let native = names_of(pkgs, |s| s == Source::Native || s == Source::Aur);
    if !native.is_empty() {
        cmds.extend(native::remove_cmd(ctx.distro.family, &native));
    }
    for p in pkgs.iter().filter(|p| p.source == Source::Flatpak) {
        cmds.push(flatpak::remove_cmd(p));
    }
    for p in pkgs.iter().filter(|p| p.source == Source::Snap) {
        cmds.push(snap::remove_cmd(p));
    }
    cmds
}

pub fn update_all_cmds(ctx: &Ctx) -> Vec<Cmd> {
    let mut c = native::update_cmds(ctx.distro.family, ctx.tools.aur_helper.as_deref());
    if ctx.tools.flatpak {
        c.extend(flatpak::update_cmds());
    }
    if ctx.tools.snap {
        c.push(Cmd::root("snap", &["refresh"]));
    }
    c
}

/// Yalnızca seçilen paketleri günceller. Arch depo paketleri seçiliyse hata
/// metni döner: kısmi güncelleme sistemi bozabilir.
pub fn update_selected_cmds(ctx: &Ctx, pkgs: &[Package]) -> Result<Vec<Cmd>, String> {
    let mut cmds = Vec::new();
    let native = names_of(pkgs, |s| s == Source::Native);
    if !native.is_empty() {
        if ctx.distro.family == Family::Arch {
            return Err(t(
                "Arch'ta depo paketleri tek tek güncellenemez (kısmi güncelleme sistemi bozabilir); u ile hepsini güncelleyin",
                "Arch repo packages cannot be updated individually (partial upgrades can break the system); press u to update everything",
            )
            .to_string());
        }
        cmds.extend(native::upgrade_cmd(ctx.distro.family, &native));
    }
    let aur = names_of(pkgs, |s| s == Source::Aur);
    if !aur.is_empty() {
        if let Some(h) = &ctx.tools.aur_helper {
            cmds.push(aur::install_cmd(h, &aur));
        }
    }
    for p in pkgs.iter().filter(|p| p.source == Source::Flatpak) {
        cmds.push(flatpak::update_cmd(p));
    }
    for p in pkgs.iter().filter(|p| p.source == Source::Snap) {
        cmds.push(Cmd::root("snap", &["refresh", &p.name]));
    }
    Ok(cmds)
}

/// Sistemdeki kullanıcı kurulumu paketleri dosya kayıtlarına çevirir.
pub fn export_entries(ctx: &Ctx) -> Vec<Entry> {
    let entry = |kind: &str, name: String, extra: String| Entry { kind: kind.to_string(), name, extra };
    let manager = ctx.distro.family.manager();
    let mut v: Vec<Entry> = native::manual_packages(ctx.distro.family)
        .into_iter()
        .map(|n| entry(manager, n, String::new()))
        .collect();
    if ctx.distro.family == Family::Arch {
        v.extend(aur::manual_packages().into_iter().map(|n| entry("aur", n, String::new())));
    }
    if ctx.tools.flatpak {
        v.extend(flatpak::installed().into_iter().map(|p| entry("flatpak", p.name, p.extra)));
    }
    if ctx.tools.snap {
        v.extend(snap::installed().into_iter().filter(|p| p.extra != "base").map(|p| entry("snap", p.name, p.extra)));
    }
    v
}

pub struct ImportPlan {
    pub pkgs: Vec<Package>,
    pub already: usize,
    /// Başka bir dağıtımın paketleri; adları burada geçerli olmayabilir
    pub foreign: usize,
    /// Kaynağı bu sistemde kurulu olmayan paketler (Flatpak, Snap, AUR yardımcısı)
    pub unavailable: usize,
}

pub fn plan_import(ctx: &Ctx, entries: &[Entry], installed: &[Package]) -> ImportPlan {
    let present: HashSet<(Source, &str)> = installed.iter().map(|p| (p.source, p.name.as_str())).collect();
    let mut plan = ImportPlan { pkgs: Vec::new(), already: 0, foreign: 0, unavailable: 0 };
    let manager = ctx.distro.family.manager();
    for e in entries {
        let source = match e.kind.as_str() {
            k if k == manager => Source::Native,
            "aur" if ctx.distro.family != Family::Arch => {
                plan.foreign += 1;
                continue;
            }
            "aur" if ctx.tools.aur_helper.is_some() => Source::Aur,
            "flatpak" if ctx.tools.flatpak => Source::Flatpak,
            "snap" if ctx.tools.snap => Source::Snap,
            "aur" | "flatpak" | "snap" => {
                plan.unavailable += 1;
                continue;
            }
            _ => {
                plan.foreign += 1;
                continue;
            }
        };
        // Arch'ta AUR paketi depo paketi olarak da listelenmiş olabilir
        let name = e.name.as_str();
        if present.contains(&(source, name)) || (source == Source::Aur && present.contains(&(Source::Native, name))) {
            plan.already += 1;
            continue;
        }
        let mut p = Package::new(source, name);
        p.extra = e.extra.clone();
        plan.pkgs.push(p);
    }
    plan
}

pub fn refresh_cmds(ctx: &Ctx) -> Vec<Cmd> {
    native::refresh_cmds(ctx.distro.family)
}

pub fn tasks(ctx: &Ctx) -> Vec<Task> {
    let f = ctx.distro.family;
    let task = |title: &str, desc: &str, cmds: Vec<Cmd>| Task {
        title: title.to_string(),
        desc: desc.to_string(),
        cmds,
        action: TaskAction::Commands,
    };
    let mut v = Vec::new();

    let all = update_all_cmds(ctx);
    if !all.is_empty() {
        v.push(task(
            t("Her şeyi güncelle", "Update everything"),
            t("Dağıtım paketleri, AUR, Flatpak ve Snap", "Distribution packages, AUR, Flatpak and Snap"),
            all,
        ));
    }
    let refresh = native::refresh_cmds(f);
    if !refresh.is_empty() {
        v.push(task(
            t("Depo bilgilerini yenile", "Refresh repositories"),
            t("Paket listelerini güncelle (yükleme yapmaz)", "Update package lists (installs nothing)"),
            refresh,
        ));
    }
    if let Some(c) = native::autoremove_cmd(f) {
        v.push(task(
            t("Kullanılmayan paketleri temizle", "Remove unused packages"),
            t("Artık gerekmeyen bağımlılıkları kaldır", "Remove dependencies that are no longer needed"),
            vec![c],
        ));
    }
    let repair = native::repair_cmds(f);
    if !repair.is_empty() {
        let desc = if f == Family::Arch {
            t("Paket veritabanını denetler, sorunları listeler (Arch'ta otomatik onarım yapılmaz)", "Checks the package database and lists problems (Arch has no safe automatic repair)")
        } else {
            t("Yarım kalan kurulumları tamamlar, bağımlılık sorunlarını giderir", "Finishes interrupted installs and fixes dependency problems")
        };
        v.push(task(t("Bozuk kurulumu onar", "Repair broken installation"), desc, repair));
    }
    if let Some(c) = native::clean_cmd(f) {
        v.push(task(
            t("Paket önbelleğini temizle", "Clean package cache"),
            t("İndirilmiş paket dosyalarını sil", "Delete downloaded package files"),
            vec![c],
        ));
    }
    if ctx.tools.flatpak {
        v.push(task(
            t("Flatpak güncelle", "Update Flatpak"),
            t("Tüm Flatpak uygulamalarını güncelle", "Update all Flatpak applications"),
            flatpak::update_cmds(),
        ));
        v.push(task(
            t("Kullanılmayan Flatpak çalışma zamanlarını sil", "Remove unused Flatpak runtimes"),
            "flatpak uninstall --unused",
            vec![Cmd::root("flatpak", &["uninstall", "--unused", "--system", "-y"])],
        ));
        v.push(task(
            t("Flathub deposunu ekle", "Add Flathub remote"),
            t("Flatpak için Flathub uzak deposunu ekle", "Add the Flathub remote for Flatpak"),
            vec![flatpak::add_flathub_cmd()],
        ));
    } else if let Some(i) = native::install_cmd(f, &["flatpak".to_string()]) {
        v.push(task(
            t("Flatpak desteğini kur", "Install Flatpak support"),
            t("Flatpak'i kurar ve Flathub'ı ekler", "Installs Flatpak and adds Flathub"),
            vec![i, flatpak::add_flathub_cmd()],
        ));
    }
    if ctx.tools.snap {
        v.push(task(
            t("Snap paketlerini güncelle", "Update Snap packages"),
            "snap refresh",
            vec![Cmd::root("snap", &["refresh"])],
        ));
    }
    if f == Family::Arch && ctx.tools.aur_helper.is_none() {
        v.push(task(
            t("AUR yardımcısı (yay) kur", "Install AUR helper (yay)"),
            t("AUR paketlerini aramak/kurmak için yay-bin derler", "Builds yay-bin to search and install AUR packages"),
            aur::bootstrap_cmds(),
        ));
    }
    if ctx.distro.family != Family::Unknown {
        v.push(Task {
            action: TaskAction::ExportPackages,
            ..task(
                t("Paket listesini dışa aktar", "Export package list"),
                t("Kendi kurduğunuz paketleri bir dosyaya kaydeder", "Saves the packages you installed to a file"),
                vec![],
            )
        });
        v.push(Task {
            action: TaskAction::ImportPackages,
            ..task(
                t("Paket listesini içe aktar", "Import package list"),
                t("Dosyadaki paketlerden eksik olanları kurar", "Installs the missing packages from a file"),
                vec![],
            )
        });
    }
    v.push(Task {
        action: TaskAction::SwitchLanguage,
        ..task(
            t("Dil (şu an: Türkçe)", "Language (current: English)"),
            t("Arayüz dilini seçin", "Choose the interface language"),
            vec![],
        )
    });
    v
}
