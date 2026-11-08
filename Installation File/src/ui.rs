// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

use ratatui::prelude::*;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap};

use crate::app::{App, ImpactState, Mode, Popup, Tab};
use crate::backends::impact::format_size;
use crate::i18n::{t, LANGUAGES};
use crate::model::{Package, Source};

fn source_color(s: Source) -> Color {
    match s {
        Source::Native => Color::Green,
        Source::Aur => Color::Magenta,
        Source::Flatpak => Color::Cyan,
        Source::Snap => Color::Yellow,
    }
}

fn source_label(s: Option<Source>) -> &'static str {
    s.map(|s| s.badge()).unwrap_or(t("HEPSİ", "ALL"))
}

const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(f: &mut Frame, app: &mut App) {
    if app.run.is_some() {
        draw_run(f, app);
        return;
    }
    let show_input = matches!(app.tab, Tab::Search | Tab::Installed);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(if show_input { 3 } else { 0 }),
            Constraint::Min(5),
            Constraint::Length(4),
            Constraint::Length(2),
        ])
        .split(f.size());

    draw_tabs(f, app, chunks[0]);
    if show_input {
        draw_input(f, app, chunks[1]);
    }
    draw_list(f, app, chunks[2]);
    draw_detail(f, app, chunks[3]);
    draw_footer(f, app, chunks[4]);
    draw_popup(f, app);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = [t("1 Ara", "1 Search"), t("2 Kurulu", "2 Installed"), t("3 Güncellemeler", "3 Updates"), t("4 Bakım", "4 Maintenance")];
    let mut titles: Vec<String> = titles.iter().map(|s| s.to_string()).collect();
    if !app.updates.is_empty() {
        titles[2] = format!("{} ({})", titles[2], app.updates.len());
    }
    let title = format!(" paketçi — {} [{}] ", app.ctx.distro.pretty, app.ctx.summary());
    let mut block = Block::default().borders(Borders::ALL).title(title);
    if app.reboot_required {
        let warning = Span::styled(
            t(" ⚠ Yeniden başlatma gerekli ", " ⚠ Restart required "),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        );
        block = block.title(Title::from(warning).alignment(Alignment::Right));
    }
    let tabs = Tabs::new(titles)
        .block(block)
        .select(app.tab.index())
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider(" ");
    f.render_widget(tabs, area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let idx = if app.tab == Tab::Installed { 1 } else { 0 };
    let editing = app.mode == Mode::Input;
    let label = if app.tab == Tab::Installed { t("Filtre", "Filter") } else { t("Ara", "Search") };
    let title = format!(" {} — {}: {} (s) ", label, t("kaynak", "source"), source_label(app.source_filter));
    let style = if editing { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) };
    let block = Block::default().borders(Borders::ALL).border_style(style).title(title);
    let text = &app.inputs[idx];
    let shown = if text.is_empty() && !editing { t("/ ile yazmaya başlayın", "Press / to start typing").to_string() } else { text.clone() };
    let p = Paragraph::new(shown).block(block).style(if text.is_empty() && !editing {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    });
    f.render_widget(p, area);
    if editing {
        let x = area.x + 1 + text.chars().count() as u16;
        f.set_cursor(x.min(area.x + area.width.saturating_sub(2)), area.y + 1);
    }
}

fn package_row(app: &App, p: &Package) -> Row<'static> {
    let sel = app.selected.contains(&p.key());
    let mark = if sel {
        Span::styled("[x]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else if p.installed {
        Span::styled(" ✔ ", Style::default().fg(Color::Green))
    } else {
        Span::raw("   ")
    };
    let origin = if p.origin.is_empty() || p.source == Source::Aur { String::new() } else { format!(" {}", p.origin) };
    let badge = Line::from(vec![
        Span::styled(p.source.badge().to_string(), Style::default().fg(source_color(p.source)).add_modifier(Modifier::BOLD)),
        Span::styled(origin, Style::default().fg(Color::DarkGray)),
    ]);
    let name_style = if sel { Style::default().fg(Color::Yellow) } else { Style::default().add_modifier(Modifier::BOLD) };
    Row::new(vec![
        Cell::from(mark),
        Cell::from(Span::styled(p.display.clone(), name_style)),
        Cell::from(Span::styled(p.version.clone(), Style::default().fg(Color::Blue))),
        Cell::from(badge),
        Cell::from(Span::styled(p.description.clone(), Style::default().fg(Color::Gray))),
    ])
}

fn draw_list(f: &mut Frame, app: &mut App, area: Rect) {
    let tab = app.tab;
    let len = app.list_len(tab);
    // imleci sınırla
    {
        let st = &mut app.states[tab.index()];
        match (len, st.selected()) {
            (0, _) => st.select(None),
            (n, Some(i)) if i >= n => st.select(Some(n - 1)),
            (_, None) => st.select(Some(0)),
            _ => {}
        }
    }

    if tab == Tab::Maintenance {
        let rows: Vec<Row> = app
            .tasks
            .iter()
            .map(|t| {
                Row::new(vec![
                    Cell::from(Span::styled(t.title.clone(), Style::default().add_modifier(Modifier::BOLD))),
                    Cell::from(Span::styled(t.desc.clone(), Style::default().fg(Color::Gray))),
                ])
            })
            .collect();
        let table = Table::new(rows, [Constraint::Length(46), Constraint::Min(10)])
            .block(Block::default().borders(Borders::ALL).title(t(" Bakım görevleri (Enter: çalıştır) ", " Maintenance tasks (Enter: run) ")))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");
        f.render_stateful_widget(table, area, &mut app.states[tab.index()]);
        return;
    }

    let title = match tab {
        Tab::Search => format!(" {} ({}) ", t("Sonuçlar", "Results"), len),
        Tab::Installed => format!(" {} ({}) ", t("Kurulu paketler", "Installed packages"), len),
        _ => format!(" {} ({}) ", t("Güncellemeler", "Updates"), len),
    };
    let block = Block::default().borders(Borders::ALL).title(title);

    if len == 0 {
        let loading = app.list_loading(tab);
        let msg = match tab {
            Tab::Search if loading => t("Aranıyor…", "Searching…"),
            Tab::Search => t(
                "Sonuç yok. / ile bir paket adı yazın (yazım hatası yapsanız bile en yakın eşleşmeler gösterilir).",
                "No results. Press / and type a package name (closest matches are shown even with typos).",
            ),
            Tab::Installed if loading => t("Kurulu paketler okunuyor…", "Reading installed packages…"),
            Tab::Installed => t("Eşleşen kurulu paket yok.", "No matching installed packages."),
            _ if loading => t("Güncellemeler denetleniyor…", "Checking for updates…"),
            _ => t(
                "Bekleyen güncelleme yok. c: yeniden denetle, R: depoları yenileyip denetle.",
                "No pending updates. c: check again, R: refresh repositories and check.",
            ),
        };
        let p = Paragraph::new(msg).block(block).style(Style::default().fg(Color::DarkGray)).wrap(Wrap { trim: true });
        f.render_widget(p, area);
        return;
    }

    let rows: Vec<Row> = app.visible(tab).into_iter().map(|p| package_row(app, p)).collect();
    let header = Row::new(vec!["", t("Paket", "Package"), t("Sürüm", "Version"), t("Kaynak", "Source"), t("Açıklama", "Description")])
        .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD));
    let table = Table::new(
        rows,
        [Constraint::Length(3), Constraint::Length(30), Constraint::Length(20), Constraint::Length(18), Constraint::Min(10)],
    )
    .header(header)
    .block(block)
    .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
    .highlight_symbol("▶ ");
    f.render_stateful_widget(table, area, &mut app.states[tab.index()]);
}

fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(t(" Ayrıntı ", " Details "));
    let text: Text = if app.tab == Tab::Maintenance {
        let sel = app.states[app.tab.index()].selected().and_then(|i| app.tasks.get(i));
        match sel {
            Some(t) => Text::from(t.cmds.iter().map(|c| Line::from(format!("$ {}", c.display()))).collect::<Vec<_>>()),
            None => Text::default(),
        }
    } else {
        match app.current() {
            Some(p) => {
                let mut head = vec![Span::styled(p.display.clone(), Style::default().add_modifier(Modifier::BOLD))];
                if p.display != p.name {
                    head.push(Span::styled(format!("  ({})", p.name), Style::default().fg(Color::DarkGray)));
                }
                head.push(Span::raw("  "));
                head.push(Span::styled(p.source.badge(), Style::default().fg(source_color(p.source))));
                if !p.origin.is_empty() {
                    head.push(Span::styled(format!(" · {}", p.origin), Style::default().fg(Color::DarkGray)));
                }
                if p.installed {
                    head.push(Span::styled(t("  ✔ kurulu", "  ✔ installed"), Style::default().fg(Color::Green)));
                }
                Text::from(vec![Line::from(head), Line::from(p.description.clone())])
            }
            None => Text::default(),
        }
    };
    f.render_widget(Paragraph::new(text).block(block).wrap(Wrap { trim: true }), area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let spin = if app.busy() { format!("{} ", SPIN[app.tick % SPIN.len()]) } else { String::new() };
    let hints = if app.mode == Mode::Input {
        t("Enter: ara/uygula • Esc: bırak • Ctrl+U: temizle", "Enter: search/apply • Esc: leave • Ctrl+U: clear")
    } else {
        match app.tab {
            Tab::Search => t(
                "↑↓ gez • Enter/i kur • Space seç • s kaynak • d ayrıntı • / ara • u güncelle • ? yardım • q çık",
                "↑↓ move • Enter/i install • Space select • s source • d details • / search • u update • ? help • q quit",
            ),
            Tab::Installed => t(
                "↑↓ gez • Enter/r kaldır • Space seç • / filtre • s kaynak • d ayrıntı • ? yardım • q çık",
                "↑↓ move • Enter/r remove • Space select • / filter • s source • d details • ? help • q quit",
            ),
            Tab::Updates => t(
                "Space seç • Enter seçilileri (yoksa hepsini) güncelle • u hepsi • c denetle • R depoları yenile • q çık",
                "Space select • Enter update selected (or all) • u all • c check • R refresh repos • q quit",
            ),
            Tab::Maintenance => t("↑↓ gez • Enter çalıştır • Tab sekme • q çık", "↑↓ move • Enter run • Tab next tab • q quit"),
        }
    };
    let lines = vec![
        Line::from(vec![Span::styled(spin, Style::default().fg(Color::Cyan)), Span::raw(app.status.clone())]),
        Line::from(Span::styled(hints, Style::default().fg(Color::DarkGray))),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn run_layout(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(1)])
        .split(area)
}

/// Komut panelinin iç boyutu (satır, sütun); sanal terminal buna göre açılır.
pub fn run_size(area: Rect) -> (u16, u16) {
    let inner = Block::default().borders(Borders::ALL).inner(run_layout(area)[1]);
    (inner.height.max(1), inner.width.max(1))
}

fn term_color(c: vt100::Color) -> Color {
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

fn term_style(c: &vt100::Cell) -> Style {
    let mut s = Style::default().fg(term_color(c.fgcolor())).bg(term_color(c.bgcolor()));
    if c.bold() {
        s = s.add_modifier(Modifier::BOLD);
    }
    if c.italic() {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if c.underline() {
        s = s.add_modifier(Modifier::UNDERLINED);
    }
    if c.inverse() {
        s = s.add_modifier(Modifier::REVERSED);
    }
    s
}

fn draw_run(f: &mut Frame, app: &mut App) {
    let chunks = run_layout(f.size());
    draw_tabs(f, app, chunks[0]);
    let tick = app.tick;
    let Some(run) = app.run.as_mut() else { return };

    let inner = Block::default().borders(Borders::ALL).inner(chunks[1]);
    run.resize(inner.height, inner.width);
    let history = run.history_view(inner.height, inner.width);

    let running = run.success().is_none();
    let (mut title, color) = match run.success() {
        None => (format!(" {} {} ", SPIN[(tick / 3) % SPIN.len()], t("Çalışıyor", "Running")), Color::Cyan),
        Some(true) => (format!(" ✔ {} ", t("Tamamlandı", "Completed")), Color::Green),
        Some(false) => (format!(" ✘ {} ", t("Başarısız oldu", "Failed")), Color::Red),
    };
    if run.scroll() > 0 {
        title.push_str(&t(format!("— ↑ {} satır geride ", run.scroll()), format!("— ↑ {} lines back ", run.scroll())));
    }
    let hint = if running {
        t(
            "Yazdıklarınız çalışan komuta gider • Parola yazılırken görünmez • PgUp/PgDn: kaydır • Ctrl+C: iptal",
            "Your typing goes to the running command • Passwords stay hidden • PgUp/PgDn: scroll • Ctrl+C: cancel",
        )
        .to_string()
    } else {
        let mut h = t("↑↓ PgUp/PgDn: kaydır • Enter/Esc/q: listeye dön", "↑↓ PgUp/PgDn: scroll • Enter/Esc/q: back to list").to_string();
        if let Some(path) = run.log_path() {
            h.push_str(&format!(" • {}: {}", t("Günlük", "Log"), path.display()));
        }
        h
    };
    f.render_widget(Block::default().borders(Borders::ALL).border_style(Style::default().fg(color)).title(title), chunks[1]);

    if let Some(lines) = history {
        let text: Vec<Line> = lines.into_iter().map(Line::from).collect();
        f.render_widget(Paragraph::new(text), inner);
    } else {
        let parser = run.parser();
        let screen = parser.screen();
        let buf = f.buffer_mut();
        for y in 0..inner.height {
            for x in 0..inner.width {
                let Some(c) = screen.cell(y, x) else { continue };
                if c.is_wide_continuation() {
                    continue;
                }
                let sym = c.contents();
                let target = buf.get_mut(inner.x + x, inner.y + y);
                target.set_symbol(if sym.is_empty() { " " } else { sym.as_str() });
                target.set_style(term_style(c));
            }
        }
        if running && !screen.hide_cursor() {
            let (row, col) = screen.cursor_position();
            if row < inner.height && col < inner.width {
                f.set_cursor(inner.x + col, inner.y + row);
            }
        }
    }
    f.render_widget(Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray))), chunks[2]);
}

fn name_list(names: &[String]) -> String {
    const MAX: usize = 40;
    let mut s = names.iter().take(MAX).cloned().collect::<Vec<_>>().join(", ");
    if names.len() > MAX {
        let more = names.len() - MAX;
        s.push_str(&t(format!(" ve {more} paket daha"), format!(" and {more} more")));
    }
    s
}

fn impact_lines(state: &ImpactState) -> Vec<Line<'static>> {
    let bold = |text: String, color: Color| Span::styled(text, Style::default().fg(color).add_modifier(Modifier::BOLD));
    let note = |text: &'static str| vec![Line::from(""), Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))];
    match state {
        ImpactState::Hidden => vec![],
        ImpactState::Loading => note(t("Etkisi hesaplanıyor…", "Calculating the impact…")),
        ImpactState::Unavailable => note(t(
            "Etki önizlemesi alınamadı; paket yöneticisi işlem sırasında özet gösterip onay sorar.",
            "No impact preview available; the package manager will show a summary and ask during the operation.",
        )),
        ImpactState::Ready(i) => {
            let mut lines = vec![Line::from("")];
            if !i.install.is_empty() {
                lines.push(Line::from(vec![
                    bold(format!("{} ({}): ", t("Kurulacak/güncellenecek", "To install/update"), i.install.len()), Color::Green),
                    Span::raw(name_list(&i.install)),
                ]));
            }
            if !i.remove.is_empty() {
                lines.push(Line::from(vec![
                    bold(format!("{} ({}): ", t("Kaldırılacak", "To remove"), i.remove.len()), Color::Red),
                    Span::raw(name_list(&i.remove)),
                ]));
            }
            if let Some(bytes) = i.download {
                lines.push(Line::from(vec![
                    bold(format!("{}: ", t("İndirilecek boyut", "Download size")), Color::Cyan),
                    Span::raw(format_size(bytes)),
                ]));
            }
            lines
        }
    }
}

fn centered(w_pct: u16, h_pct: u16, area: Rect) -> Rect {
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100 - h_pct) / 2), Constraint::Percentage(h_pct), Constraint::Percentage((100 - h_pct) / 2)])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100 - w_pct) / 2), Constraint::Percentage(w_pct), Constraint::Percentage((100 - w_pct) / 2)])
        .split(v[1])[1]
}

fn draw_popup(f: &mut Frame, app: &App) {
    let Some(popup) = &app.popup else { return };
    match popup {
        Popup::Confirm { title, lines, impact, .. } => {
            let area = centered(80, 70, f.size());
            f.render_widget(Clear, area);
            let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Yellow)).title(format!(" {} ", title));
            let inner = block.inner(area);
            f.render_widget(block, area);
            let parts = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);
            let mut text: Vec<Line> = lines.iter().map(|l| Line::from(l.clone())).collect();
            text.extend(impact_lines(impact));
            f.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), parts[0]);
            let hint = Span::styled(
                t("Enter/y: onayla • Esc/n: iptal", "Enter/y: confirm • Esc/n: cancel"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            );
            f.render_widget(Paragraph::new(hint), parts[1]);
        }
        Popup::PackageFile { import, input, confirm_overwrite } => {
            let area = centered(70, 40, f.size());
            f.render_widget(Clear, area);
            let title = if *import { t(" Paket listesini içe aktar ", " Import package list ") } else { t(" Paket listesini dışa aktar ", " Export package list ") };
            let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)).title(title);
            let inner = block.inner(area);
            f.render_widget(block, area);
            let mut lines = vec![
                Line::from(t("Dosya yolu:", "File path:")),
                Line::from(Span::styled(input.clone(), Style::default().add_modifier(Modifier::BOLD))),
                Line::from(""),
            ];
            if *confirm_overwrite {
                lines.push(Line::from(Span::styled(
                    t("Bu dosya zaten var. Üzerine yazmak için tekrar Enter'a basın.", "This file already exists. Press Enter again to overwrite it."),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    t("Enter: devam • Esc: iptal • Ctrl+U: temizle", "Enter: continue • Esc: cancel • Ctrl+U: clear"),
                    Style::default().fg(Color::DarkGray),
                )));
            }
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
            let x = inner.x + input.chars().count() as u16;
            if inner.width > 0 && inner.height > 1 && x < inner.x + inner.width {
                f.set_cursor(x, inner.y + 1);
            }
        }
        Popup::Language { selected, required } => {
            let area = centered(50, 50, f.size());
            f.render_widget(Clear, area);
            let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)).title(" Dil / Language ");
            let mut lines = vec![Line::from("Dil seçin / Choose a language"), Line::from("")];
            for (i, (_, name)) in LANGUAGES.iter().enumerate() {
                lines.push(if i == *selected {
                    Line::from(Span::styled(format!("▶ {}  {}", i + 1, name), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                } else {
                    Line::from(format!("  {}  {}", i + 1, name))
                });
            }
            lines.push(Line::from(""));
            let mut hint = "↑↓: gez/move • Enter: seç/select".to_string();
            if !*required {
                hint.push_str(" • Esc: iptal/cancel");
            }
            lines.push(Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))));
            f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
        }
        Popup::Detail(p) => {
            let area = centered(70, 50, f.size());
            f.render_widget(Clear, area);
            let row = |k: &str, v: String| {
                Line::from(vec![Span::styled(format!("{:<10}", k), Style::default().fg(Color::DarkGray)), Span::raw(v)])
            };
            let mut lines = vec![
                row(t("Ad", "Name"), p.display.clone()),
                row(t("Kimlik", "ID"), p.name.clone()),
                row(t("Sürüm", "Version"), p.version.clone()),
                row(t("Kaynak", "Source"), format!("{} {}", p.source.badge(), p.origin)),
                row(
                    t("Durum", "Status"),
                    if p.installed { t("kurulu", "installed").into() } else { t("kurulu değil", "not installed").into() },
                ),
                Line::from(""),
                Line::from(p.description.clone()),
            ];
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(t("Kapatmak için herhangi bir tuş", "Press any key to close"), Style::default().fg(Color::DarkGray))));
            let block = Block::default().borders(Borders::ALL).title(t(" Paket ayrıntısı ", " Package details "));
            f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: true }), area);
        }
        Popup::Help => {
            let area = centered(70, 80, f.size());
            f.render_widget(Clear, area);
            let items = [
                ("1-4 / Tab", t("Sekmeler: Ara, Kurulu, Güncellemeler, Bakım", "Tabs: Search, Installed, Updates, Maintenance")),
                ("/", t("Arama / filtre kutusuna yaz (Enter: ara, Esc: bırak)", "Type in the search/filter box (Enter: search, Esc: leave)")),
                ("↑ ↓ j k", t("Listede gez (PgUp/PgDn, g/G)", "Move through the list (PgUp/PgDn, g/G)")),
                ("Space", t("Paketi seç / seçimi kaldır (toplu işlem için)", "Select / deselect a package (for batch actions)")),
                ("Enter / i", t("Ara sekmesinde: seçili paketleri kur", "Search tab: install selected packages")),
                ("Enter / r", t("Kurulu sekmesinde: seçili paketleri kaldır", "Installed tab: remove selected packages")),
                ("Enter", t("Güncellemeler sekmesinde: seçilileri, seçim yoksa hepsini güncelle", "Updates tab: update selected, or all if none selected")),
                ("s", t("Kaynak filtresi: hepsi → repo → AUR → Flatpak → Snap", "Source filter: all → repo → AUR → Flatpak → Snap")),
                ("d", t("Paket ayrıntıları", "Package details")),
                ("u", t("Her şeyi güncelle (her sekmede çalışır)", "Update everything (works in every tab)")),
                ("c / R", t("Güncellemeler sekmesinde: denetle / depoları yenile", "Updates tab: check / refresh repositories")),
                ("PgUp/PgDn", t("Komut çıktısında geriye kaydır", "Scroll back through command output")),
                ("Esc", t("Seçimi temizle", "Clear selection")),
                ("q / Ctrl+C", t("Çık", "Quit")),
            ];
            let mut lines: Vec<Line> = items
                .iter()
                .map(|(k, v)| Line::from(vec![Span::styled(format!("{:<12}", k), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), Span::raw(*v)]))
                .collect();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                t("✔ kurulu • [x] seçili • Kapatmak için herhangi bir tuş", "✔ installed • [x] selected • Press any key to close"),
                Style::default().fg(Color::DarkGray),
            )));
            let block = Block::default().borders(Borders::ALL).title(t(" Yardım ", " Help "));
            f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: true }), area);
        }
    }
}
