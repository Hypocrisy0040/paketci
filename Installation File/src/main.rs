// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Hypocrisy0040

mod app;
mod backends;
mod distro;
mod exec;
mod fuzzy;
mod history;
mod i18n;
mod model;
mod pkglist;
mod transcript;
mod ui;
mod util;

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;

const HELP_TR: &str = "paketçi — tüm Linux dağıtımları için evrensel paket yöneticisi (TUI)

Kullanım:
  paketci [ARAMA TERİMİ]     Terimle başlatır ve hemen arar
  paketci --help | --version

Uygulama içinde ? tuşu ile kısayolları görebilirsiniz.";

const HELP_EN: &str = "paketçi — a universal package manager (TUI) for all Linux distributions

Usage:
  paketci [SEARCH TERM]      Starts and immediately searches for the term
  paketci --help | --version

Press ? inside the application to see the shortcuts.";

fn main() -> Result<()> {
    let ask_language = i18n::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", i18n::t(HELP_TR, HELP_EN));
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("paketci {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let initial = args.join(" ");

    let orig = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        orig(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let res = run(&mut terminal, initial, ask_language);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    res
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, initial: String, ask_language: bool) -> Result<()> {
    let mut app = App::new(initial, ask_language);
    while !app.quit {
        app.poll_messages();
        terminal.draw(|f| ui::draw(f, &mut app))?;
        let wait = if app.run.is_some() {
            30
        } else if app.busy() {
            80
        } else {
            250
        };
        if event::poll(Duration::from_millis(wait))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    app.on_key(k);
                }
            }
        }
        app.tick = app.tick.wrapping_add(1);
        if let Some(cmds) = app.pending_run.take() {
            let (rows, cols) = ui::run_size(terminal.size()?);
            app.start_run(&cmds, rows, cols);
        }
    }
    Ok(())
}
