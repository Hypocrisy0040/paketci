// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use crate::backends::impact::{self, Action, Impact};
use crate::backends::{self, Ctx};
use crate::exec::Run;
use crate::fuzzy;
use crate::history;
use crate::i18n::{self, t, Lang};
use crate::model::*;
use crate::pkglist;
use crate::util::expand_home;

const MAX_LISTED_PACKAGES: usize = 15;
const MAX_COMMAND_CHARS: usize = 300;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Search,
    Installed,
    Updates,
    Maintenance,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Search, Tab::Installed, Tab::Updates, Tab::Maintenance];
    pub fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Input,
}

pub enum ImpactState {
    /// Depo paketi yok, gösterilecek etki de yok
    Hidden,
    Loading,
    Ready(Impact),
    Unavailable,
}

pub enum Popup {
    /// `learn`: işlem başarılı olursa aramada öne çıkarılacak paketler
    Confirm { title: String, lines: Vec<String>, cmds: Vec<Cmd>, impact: ImpactState, learn: Vec<String> },
    Detail(Package),
    Help,
    /// `required`: dil bulunamadı, seçim yapmadan kapatılamaz
    Language { selected: usize, required: bool },
    PackageFile { import: bool, input: String, confirm_overwrite: bool },
}

pub enum Msg {
    SearchPart { gen: u64, pkgs: Vec<Package>, note: Option<String> },
    Installed(Vec<Package>),
    Updates(Vec<Package>),
    Impact { gen: u64, impact: Option<Impact> },
    Reboot(bool),
    Exported(Result<(usize, String), String>),
}

pub struct App {
    pub ctx: Ctx,
    pub tab: Tab,
    pub mode: Mode,
    /// [arama sorgusu, kurulu paket filtresi]
    pub inputs: [String; 2],
    pub results: Vec<Package>,
    pub installed: Vec<Package>,
    pub updates: Vec<Package>,
    pub tasks: Vec<Task>,
    pub states: [TableState; 4],
    pub selected: HashSet<(Source, String)>,
    pub source_filter: Option<Source>,
    pub popup: Option<Popup>,
    pub status: String,
    pub query: String,
    pub notes: Vec<String>,
    pub tick: usize,
    pub quit: bool,
    pub pending_run: Option<Vec<Cmd>>,
    /// Çalışan veya sonucu ekranda duran komut grubu
    pub run: Option<Run>,
    run_learn: Vec<String>,
    pub reboot_required: bool,
    gen: u64,
    pending_search: usize,
    loading_installed: bool,
    loading_updates: bool,
    updates_loaded: bool,
    /// Denetim sürerken sistem değişti; bitince yeniden denetlenecek
    reload_updates: bool,
    impact_gen: u64,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

impl App {
    pub fn new(initial: String, ask_language: bool) -> Self {
        let ctx = Ctx::new();
        let (tx, rx) = channel();
        let tasks = backends::tasks(&ctx);
        let mut app = App {
            ctx,
            tab: Tab::Search,
            mode: Mode::Input,
            inputs: [initial.clone(), String::new()],
            results: vec![],
            installed: vec![],
            updates: vec![],
            tasks,
            states: Default::default(),
            selected: HashSet::new(),
            source_filter: None,
            popup: None,
            status: String::new(),
            query: String::new(),
            notes: vec![],
            tick: 0,
            quit: false,
            pending_run: None,
            run: None,
            run_learn: Vec::new(),
            reboot_required: false,
            gen: 0,
            pending_search: 0,
            loading_installed: false,
            loading_updates: false,
            updates_loaded: false,
            reload_updates: false,
            impact_gen: 0,
            tx,
            rx,
        };
        app.status = if app.ctx.sources().is_empty() {
            t("Desteklenen paket yöneticisi bulunamadı", "No supported package manager found").into()
        } else {
            t("Aramak istediğiniz paketin adını yazıp Enter'a basın", "Type a package name and press Enter").into()
        };
        if ask_language {
            app.popup = Some(Popup::Language { selected: 0, required: true });
        }
        app.load_installed();
        app.load_updates();
        app.check_reboot();
        if !initial.trim().is_empty() {
            app.mode = Mode::Normal;
            app.start_search();
        }
        app
    }

    pub fn busy(&self) -> bool {
        self.pending_search > 0 || self.loading_installed || self.loading_updates
    }

    pub fn list_loading(&self, tab: Tab) -> bool {
        match tab {
            Tab::Search => self.pending_search > 0,
            Tab::Installed => self.loading_installed,
            _ => self.loading_updates,
        }
    }

    fn choose_language(&mut self, l: Lang) {
        i18n::set(l);
        self.tasks = backends::tasks(&self.ctx);
        self.popup = None;
        self.status = match i18n::save(l) {
            Ok(()) => t("Dil: Türkçe", "Language: English").to_string(),
            Err(e) => t(format!("Dil seçimi kaydedilemedi: {e}"), format!("Could not save the language choice: {e}")),
        };
    }

    // ------------------------------------------------------------ arka plan işleri

    fn start_search(&mut self) {
        let q = fuzzy::normalize(&self.inputs[0]);
        if q.is_empty() {
            return;
        }
        let sources = self.ctx.sources();
        if sources.is_empty() {
            return;
        }
        self.gen += 1;
        self.results.clear();
        self.selected.clear();
        self.notes.clear();
        self.states[Tab::Search.index()].select(Some(0));
        self.pending_search = sources.len();
        self.query = q.clone();
        self.status = t(format!("“{}” aranıyor…", q), format!("Searching for “{}”…", q));
        for s in sources {
            let ctx = self.ctx.clone();
            let tx = self.tx.clone();
            let gen = self.gen;
            let q = q.clone();
            thread::spawn(move || {
                let (pkgs, note) = backends::search_source(&ctx, s, &q);
                let _ = tx.send(Msg::SearchPart { gen, pkgs, note });
            });
        }
    }

    fn load_installed(&mut self) {
        if self.loading_installed {
            return;
        }
        self.loading_installed = true;
        let ctx = self.ctx.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(Msg::Installed(backends::list_installed_all(&ctx)));
        });
    }

    fn check_reboot(&self) {
        let family = self.ctx.distro.family;
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(Msg::Reboot(backends::native::reboot_required(family)));
        });
    }

    fn load_updates(&mut self) {
        if self.loading_updates {
            self.reload_updates = true;
            return;
        }
        self.loading_updates = true;
        self.updates_loaded = true;
        if self.tab == Tab::Updates {
            self.status = t("Güncellemeler denetleniyor…", "Checking for updates…").into();
        }
        let ctx = self.ctx.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(Msg::Updates(backends::list_updates(&ctx)));
        });
    }

    pub fn poll_messages(&mut self) {
        if let Some(run) = &mut self.run {
            run.poll();
        }
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::SearchPart { gen, pkgs, note } => {
                    if gen != self.gen {
                        continue;
                    }
                    self.results.extend(pkgs);
                    if let Some(n) = note {
                        self.notes.push(n);
                    }
                    self.regroup();
                    self.pending_search = self.pending_search.saturating_sub(1);
                    if self.pending_search == 0 {
                        self.finish_search();
                    }
                }
                Msg::Installed(v) => {
                    self.installed = v;
                    self.loading_installed = false;
                }
                Msg::Updates(v) => {
                    self.updates = v;
                    self.loading_updates = false;
                    if self.tab == Tab::Updates {
                        let n = self.updates.len();
                        self.status = if n == 0 {
                            t("Sistem güncel", "System is up to date").into()
                        } else {
                            t(
                                format!("{n} güncelleme var — u ile hepsini güncelleyin"),
                                format!("{n} updates available — press u to update everything"),
                            )
                        };
                    }
                    if self.reload_updates {
                        self.reload_updates = false;
                        self.load_updates();
                    }
                }
                Msg::Reboot(required) => self.reboot_required = required,
                Msg::Exported(result) => {
                    self.status = match result {
                        Ok((n, path)) => t(format!("{n} paket kaydedildi: {path}"), format!("{n} packages saved to {path}")),
                        Err(e) => e,
                    };
                }
                Msg::Impact { gen, impact } => {
                    if gen == self.impact_gen {
                        if let Some(Popup::Confirm { impact: state, .. }) = &mut self.popup {
                            *state = impact.map_or(ImpactState::Unavailable, ImpactState::Ready);
                        }
                    }
                }
            }
        }
    }

    /// Aynı uygulamanın farklı kaynaklardaki sürümlerini yan yana sıralar.
    fn regroup(&mut self) {
        let mut best: HashMap<String, f64> = HashMap::new();
        for p in &self.results {
            let e = best.entry(p.group_key()).or_insert(0.0);
            if p.score > *e {
                *e = p.score;
            }
        }
        self.results.sort_by(|a, b| {
            let (ka, kb) = (a.group_key(), b.group_key());
            let (ga, gb) = (best[&ka], best[&kb]);
            gb.partial_cmp(&ga)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(ka.cmp(&kb))
                .then(a.source.order().cmp(&b.source.order()))
                .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
        });
    }

    fn finish_search(&mut self) {
        let n = self.results.len();
        let mut s = if n == 0 {
            t(format!("“{}” için sonuç bulunamadı", self.query), format!("No results for “{}”", self.query))
        } else {
            let exact = self.results.iter().any(|p| p.score >= 0.70);
            if exact {
                t(format!("{} sonuç", n), format!("{} results", n))
            } else {
                let guess = &self.results[0];
                t(
                    format!(
                        "“{}” ile tam eşleşme yok — bunu mu demek istediniz: {}? ({} benzer sonuç)",
                        self.query, guess.display, n
                    ),
                    format!("No exact match for “{}” — did you mean: {}? ({} similar results)", self.query, guess.display, n),
                )
            }
        };
        for note in &self.notes {
            s.push_str(" · ");
            s.push_str(note);
        }
        self.status = s;
    }

    pub fn start_run(&mut self, cmds: &[Cmd], rows: u16, cols: u16) {
        match Run::start(cmds, rows, cols) {
            Ok(run) => self.run = Some(run),
            Err(e) => {
                self.run_learn.clear();
                self.status = t(format!("Komut başlatılamadı: {e}"), format!("Could not start the command: {e}"));
            }
        }
    }

    fn after_run(&mut self) {
        self.selected.clear();
        self.ctx.tools = backends::Tools::detect(&self.ctx.distro);
        self.tasks = backends::tasks(&self.ctx);
        self.load_installed();
        self.check_reboot();
        if !self.query.is_empty() {
            self.inputs[0] = self.query.clone();
            self.start_search();
        }
        if self.updates_loaded {
            self.load_updates();
        }
        if !self.busy() {
            self.status = t("İşlem tamamlandı", "Operation completed").into();
        }
    }

    // ------------------------------------------------------------ listeler

    pub fn visible(&self, tab: Tab) -> Vec<&Package> {
        let f = self.source_filter;
        let src_ok = |p: &&Package| f.map_or(true, |s| p.source == s);
        match tab {
            Tab::Search => self.results.iter().filter(src_ok).collect(),
            Tab::Installed => {
                let q = self.inputs[1].to_lowercase();
                self.installed
                    .iter()
                    .filter(src_ok)
                    .filter(|p| {
                        q.is_empty()
                            || p.name.to_lowercase().contains(&q)
                            || p.display.to_lowercase().contains(&q)
                            || p.description.to_lowercase().contains(&q)
                    })
                    .collect()
            }
            Tab::Updates => self.updates.iter().collect(),
            Tab::Maintenance => vec![],
        }
    }

    pub fn list_len(&self, tab: Tab) -> usize {
        match tab {
            Tab::Maintenance => self.tasks.len(),
            _ => self.visible(tab).len(),
        }
    }

    pub fn current(&self) -> Option<Package> {
        let i = self.states[self.tab.index()].selected()?;
        self.visible(self.tab).get(i).map(|p| (*p).clone())
    }

    fn targets(&self) -> Vec<Package> {
        let list = self.visible(self.tab);
        if !self.selected.is_empty() {
            let v: Vec<Package> = list
                .iter()
                .filter(|p| self.selected.contains(&p.key()))
                .map(|p| (*p).clone())
                .collect();
            if !v.is_empty() {
                return v;
            }
        }
        self.current().into_iter().collect()
    }

    fn move_cursor(&mut self, delta: isize) {
        let len = self.list_len(self.tab);
        if len == 0 {
            return;
        }
        let st = &mut self.states[self.tab.index()];
        let cur = st.selected().unwrap_or(0) as isize;
        st.select(Some((cur + delta).clamp(0, len as isize - 1) as usize));
    }

    fn goto(&mut self, tab: Tab) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        self.selected.clear();
        self.mode = Mode::Normal;
        if tab == Tab::Updates && !self.updates_loaded {
            self.load_updates();
        }
    }

    fn cycle_filter(&mut self) {
        let mut opts: Vec<Option<Source>> = vec![None];
        opts.extend(self.ctx.sources().into_iter().map(Some));
        let pos = opts.iter().position(|o| *o == self.source_filter).unwrap_or(0);
        self.source_filter = opts[(pos + 1) % opts.len()];
        self.states[self.tab.index()].select(Some(0));
    }

    // ------------------------------------------------------------ eylemler

    fn confirm_install(&mut self, pkgs: Vec<Package>) {
        if pkgs.is_empty() {
            return;
        }
        let cmds = backends::install_cmds(&self.ctx, &pkgs);
        if cmds.is_empty() {
            self.status = t("Bu paket için kurulum komutu üretilemedi", "Could not build an install command for this package").into();
            return;
        }
        self.open_confirm(t("Kurulum onayı", "Confirm installation"), &pkgs, cmds, Action::Install);
        if let Some(Popup::Confirm { learn, .. }) = &mut self.popup {
            *learn = pkgs.iter().map(|p| p.name.clone()).collect();
        }
    }

    fn confirm_remove(&mut self, pkgs: Vec<Package>) {
        if pkgs.is_empty() {
            return;
        }
        let cmds = backends::remove_cmds(&self.ctx, &pkgs);
        if cmds.is_empty() {
            return;
        }
        self.open_confirm(t("Kaldırma onayı", "Confirm removal"), &pkgs, cmds, Action::Remove);
    }

    fn open_confirm(&mut self, title: &str, pkgs: &[Package], cmds: Vec<Cmd>, action: Action) {
        let mut lines: Vec<String> = pkgs
            .iter()
            .take(MAX_LISTED_PACKAGES)
            .map(|p| {
                let mut s = format!("  • {} [{}]", p.display, p.source.badge());
                if p.installed && action == Action::Install {
                    s.push_str(t(" (zaten kurulu)", " (already installed)"));
                }
                s
            })
            .collect();
        if pkgs.len() > MAX_LISTED_PACKAGES {
            let more = pkgs.len() - MAX_LISTED_PACKAGES;
            lines.push(t(format!("  … ve {more} paket daha"), format!("  … and {more} more")));
        }
        lines.push(String::new());
        lines.push(t("Çalıştırılacak komutlar:", "Commands to run:").into());
        lines.extend(cmds.iter().map(|c| {
            let text = c.display();
            match text.char_indices().nth(MAX_COMMAND_CHARS) {
                Some((end, _)) => format!("  $ {}…", &text[..end]),
                None => format!("  $ {text}"),
            }
        }));

        // AUR paketlerini de pacman kaldırır
        let names: Vec<String> = pkgs
            .iter()
            .filter(|p| p.source == Source::Native || (action == Action::Remove && p.source == Source::Aur))
            .map(|p| p.name.clone())
            .collect();
        let impact = if names.is_empty() {
            ImpactState::Hidden
        } else {
            self.impact_gen += 1;
            let gen = self.impact_gen;
            let family = self.ctx.distro.family;
            let tx = self.tx.clone();
            thread::spawn(move || {
                let _ = tx.send(Msg::Impact { gen, impact: impact::simulate(family, action, &names) });
            });
            ImpactState::Loading
        };
        self.popup = Some(Popup::Confirm { title: title.into(), lines, cmds, impact, learn: Vec::new() });
    }

    fn confirm_update_all(&mut self) {
        let cmds = backends::update_all_cmds(&self.ctx);
        if cmds.is_empty() {
            self.status = t("Güncelleme komutu bulunamadı", "No update command found").into();
            return;
        }
        let mut lines = vec![
            t("Tüm kaynaklar güncellenecek.", "All sources will be updated.").to_string(),
            String::new(),
            t("Çalıştırılacak komutlar:", "Commands to run:").into(),
        ];
        lines.extend(cmds.iter().map(|c| format!("  $ {}", c.display())));
        self.popup = Some(Popup::Confirm {
            title: t("Sistem güncelleme onayı", "Confirm system update").into(),
            lines,
            cmds,
            impact: ImpactState::Hidden,
            learn: Vec::new(),
        });
    }

    fn confirm_update_selected(&mut self) {
        let mut pkgs = self.targets();
        // Güncelleme listesi Flatpak kurulum yerini (kullanıcı/sistem) içermez
        for p in pkgs.iter_mut().filter(|p| p.source == Source::Flatpak) {
            if let Some(i) = self.installed.iter().find(|i| i.source == Source::Flatpak && i.name == p.name) {
                p.extra = i.extra.clone();
            }
        }
        match backends::update_selected_cmds(&self.ctx, &pkgs) {
            Ok(cmds) if !cmds.is_empty() => {
                self.open_confirm(t("Güncelleme onayı", "Confirm update"), &pkgs, cmds, Action::Update);
            }
            Ok(_) => self.status = t("Güncelleme komutu bulunamadı", "No update command found").into(),
            Err(message) => self.status = message,
        }
    }

    fn run_task(&mut self) {
        let Some(i) = self.states[Tab::Maintenance.index()].selected() else { return };
        let Some(task) = self.tasks.get(i).cloned() else { return };
        match task.action {
            TaskAction::Commands => {}
            TaskAction::SwitchLanguage => {
                let selected = i18n::LANGUAGES.iter().position(|(l, _)| *l == i18n::lang()).unwrap_or(0);
                self.popup = Some(Popup::Language { selected, required: false });
                return;
            }
            TaskAction::ExportPackages | TaskAction::ImportPackages => {
                self.popup = Some(Popup::PackageFile {
                    import: task.action == TaskAction::ImportPackages,
                    input: t("~/paketci-paketler.txt", "~/paketci-packages.txt").to_string(),
                    confirm_overwrite: false,
                });
                return;
            }
        }
        let mut lines = vec![task.desc.clone(), String::new(), t("Çalıştırılacak komutlar:", "Commands to run:").into()];
        lines.extend(task.cmds.iter().map(|c| format!("  $ {}", c.display())));
        self.popup = Some(Popup::Confirm { title: task.title, lines, cmds: task.cmds, impact: ImpactState::Hidden, learn: Vec::new() });
    }

    fn export_packages(&mut self, path: PathBuf) {
        self.status = t("Paket listesi hazırlanıyor…", "Preparing the package list…").into();
        let ctx = self.ctx.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let entries = backends::export_entries(&ctx);
            let result = if entries.is_empty() {
                Err(t("Paket listesi alınamadı", "Could not read the package list").to_string())
            } else {
                fs::write(&path, pkglist::format(&entries, &ctx.distro.pretty))
                    .map(|_| (entries.len(), path.display().to_string()))
                    .map_err(|e| t(format!("Dosya yazılamadı: {e}"), format!("Could not write the file: {e}")))
            };
            let _ = tx.send(Msg::Exported(result));
        });
    }

    fn import_packages(&mut self, path: &Path) {
        if self.loading_installed {
            self.status = t("Kurulu paketler henüz okunuyor, birazdan tekrar deneyin", "Installed packages are still loading, try again shortly").into();
            return;
        }
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.status = t(format!("Dosya okunamadı: {e}"), format!("Could not read the file: {e}"));
                return;
            }
        };
        let (entries, invalid) = pkglist::parse(&text);
        if entries.is_empty() {
            self.status = t("Dosyada geçerli paket bulunamadı", "No valid packages found in the file").into();
            return;
        }
        let plan = backends::plan_import(&self.ctx, &entries, &self.installed);
        let cmds = backends::install_cmds(&self.ctx, &plan.pkgs);
        if cmds.is_empty() {
            self.status = t(
                format!("Kurulacak yeni paket yok ({} zaten kurulu, {} atlandı)", plan.already, plan.foreign + plan.unavailable + invalid),
                format!("No new packages to install ({} already installed, {} skipped)", plan.already, plan.foreign + plan.unavailable + invalid),
            );
            return;
        }
        self.open_confirm(t("İçe aktarma onayı", "Confirm import"), &plan.pkgs, cmds, Action::Install);

        let mut notes = vec![t(format!("Kurulacak: {} paket", plan.pkgs.len()), format!("To install: {} packages", plan.pkgs.len()))];
        if plan.already > 0 {
            notes.push(t(format!("Zaten kurulu: {}", plan.already), format!("Already installed: {}", plan.already)));
        }
        if plan.foreign > 0 {
            notes.push(t(format!("Atlandı, başka dağıtımdan: {}", plan.foreign), format!("Skipped, from another distribution: {}", plan.foreign)));
        }
        if plan.unavailable > 0 {
            notes.push(t(format!("Atlandı, kaynağı bu sistemde yok: {}", plan.unavailable), format!("Skipped, source not available here: {}", plan.unavailable)));
        }
        if invalid > 0 {
            notes.push(t(format!("Atlandı, geçersiz satır: {invalid}"), format!("Skipped, invalid lines: {invalid}")));
        }
        notes.push(String::new());
        if let Some(Popup::Confirm { lines, .. }) = &mut self.popup {
            lines.splice(0..0, notes);
        }
    }

    fn package_file_key(&mut self, k: KeyEvent) {
        let Some(Popup::PackageFile { import, input, confirm_overwrite }) = &mut self.popup else { return };
        match k.code {
            KeyCode::Esc => self.popup = None,
            KeyCode::Enter => {
                let path = expand_home(input.trim());
                if input.trim().is_empty() {
                    return;
                }
                if *import {
                    self.popup = None;
                    self.import_packages(&path);
                } else if path.exists() && !*confirm_overwrite {
                    *confirm_overwrite = true;
                } else {
                    self.popup = None;
                    self.export_packages(path);
                }
            }
            KeyCode::Backspace => {
                input.pop();
                *confirm_overwrite = false;
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                input.clear();
                *confirm_overwrite = false;
            }
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                *confirm_overwrite = false;
            }
            _ => {}
        }
    }

    fn primary(&mut self) {
        match self.tab {
            Tab::Search => {
                let t = self.targets();
                self.confirm_install(t);
            }
            Tab::Installed => {
                let t = self.targets();
                self.confirm_remove(t);
            }
            Tab::Updates if self.selected.is_empty() => self.confirm_update_all(),
            Tab::Updates => self.confirm_update_selected(),
            Tab::Maintenance => self.run_task(),
        }
    }

    // ------------------------------------------------------------ klavye

    pub fn on_key(&mut self, k: KeyEvent) {
        // Komut sürerken her tuş (Ctrl+C dahil) komuta gider
        if let Some(run) = &mut self.run {
            if run.scroll_key(k) {
                return;
            }
            if run.success().is_none() {
                run.send_key(k);
                return;
            }
        }
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.run.is_some() {
            if matches!(k.code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
                let learned = std::mem::take(&mut self.run_learn);
                if self.run.as_ref().and_then(|r| r.success()) == Some(true) && !learned.is_empty() {
                    self.ctx.history = history::record(&learned);
                }
                self.run = None;
                self.after_run();
            }
            return;
        }
        if self.popup.is_some() {
            self.popup_key(k);
            return;
        }
        match self.mode {
            Mode::Input => self.input_key(k),
            Mode::Normal => self.normal_key(k),
        }
    }

    fn popup_key(&mut self, k: KeyEvent) {
        match &self.popup {
            Some(Popup::Confirm { .. }) => match k.code {
                code if matches!(code, KeyCode::Enter | KeyCode::Char('y')) || (code == KeyCode::Char('e') && i18n::lang() == Lang::Tr) => {
                    if let Some(Popup::Confirm { cmds, learn, .. }) = self.popup.take() {
                        self.pending_run = Some(cmds);
                        self.run_learn = learn;
                    }
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('h') | KeyCode::Char('q') => {
                    self.popup = None;
                }
                _ => {}
            },
            Some(Popup::PackageFile { .. }) => self.package_file_key(k),
            Some(Popup::Language { .. }) => self.language_key(k),
            _ => self.popup = None,
        }
    }

    fn language_key(&mut self, k: KeyEvent) {
        let Some(Popup::Language { selected, required }) = &mut self.popup else { return };
        let count = i18n::LANGUAGES.len();
        match k.code {
            KeyCode::Up | KeyCode::Char('k') => *selected = (*selected + count - 1) % count,
            KeyCode::Down | KeyCode::Char('j') => *selected = (*selected + 1) % count,
            KeyCode::Enter => {
                let (language, _) = i18n::LANGUAGES[*selected];
                self.choose_language(language);
            }
            KeyCode::Char(c @ '1'..='9') => {
                if let Some((language, _)) = i18n::LANGUAGES.get(c as usize - '1' as usize) {
                    self.choose_language(*language);
                }
            }
            KeyCode::Esc if !*required => self.popup = None,
            _ => {}
        }
    }

    fn input_key(&mut self, k: KeyEvent) {
        let idx = if self.tab == Tab::Installed { 1 } else { 0 };
        match k.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                if self.tab == Tab::Search {
                    self.start_search();
                }
            }
            KeyCode::Backspace => {
                self.inputs[idx].pop();
            }
            KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => self.inputs[idx].clear(),
            KeyCode::Char(c) if !k.modifiers.contains(KeyModifiers::CONTROL) => self.inputs[idx].push(c),
            _ => {}
        }
        if idx == 1 {
            self.states[Tab::Installed.index()].select(Some(0));
        }
    }

    fn normal_key(&mut self, k: KeyEvent) {
        let has_list = matches!(self.tab, Tab::Search | Tab::Installed);
        match k.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') | KeyCode::F(1) => self.popup = Some(Popup::Help),
            KeyCode::Char('/') if has_list => self.mode = Mode::Input,
            KeyCode::Tab => {
                let n = (self.tab.index() + 1) % Tab::ALL.len();
                self.goto(Tab::ALL[n]);
            }
            KeyCode::BackTab => {
                let n = (self.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len();
                self.goto(Tab::ALL[n]);
            }
            KeyCode::Char(c @ '1'..='4') => self.goto(Tab::ALL[(c as u8 - b'1') as usize]),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::PageDown => self.move_cursor(10),
            KeyCode::PageUp => self.move_cursor(-10),
            KeyCode::Home | KeyCode::Char('g') => self.move_cursor(isize::MIN / 2),
            KeyCode::End | KeyCode::Char('G') => self.move_cursor(isize::MAX / 2),
            KeyCode::Esc => self.selected.clear(),
            KeyCode::Char(' ') if has_list || self.tab == Tab::Updates => {
                if let Some(p) = self.current() {
                    let key = p.key();
                    if !self.selected.remove(&key) {
                        self.selected.insert(key);
                    }
                    self.move_cursor(1);
                }
            }
            KeyCode::Enter => self.primary(),
            KeyCode::Char('i') if self.tab == Tab::Search => self.primary(),
            KeyCode::Char('r') | KeyCode::Char('x') | KeyCode::Delete if self.tab == Tab::Installed => self.primary(),
            KeyCode::Char('s') if has_list => self.cycle_filter(),
            KeyCode::Char('d') if self.tab != Tab::Maintenance => {
                if let Some(p) = self.current() {
                    self.popup = Some(Popup::Detail(p));
                }
            }
            KeyCode::Char('u') => self.confirm_update_all(),
            KeyCode::Char('c') if self.tab == Tab::Updates => self.load_updates(),
            KeyCode::Char('R') if self.tab == Tab::Updates => {
                let cmds = backends::refresh_cmds(&self.ctx);
                if cmds.is_empty() {
                    self.load_updates();
                } else {
                    self.pending_run = Some(cmds);
                    self.updates_loaded = true;
                }
            }
            _ => {}
        }
    }
}
