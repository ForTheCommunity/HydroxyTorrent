mod api;
mod app;
pub mod config;
mod models;
mod theme;
mod ui;

use std::{
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

fn parse_cli_args() -> (Option<String>, Option<String>, Option<String>) {
    let mut url = std::env::var("HYDROXY_URL").ok();
    let mut user = std::env::var("HYDROXY_USER").ok();
    let mut pass = std::env::var("HYDROXY_PASS").ok();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" | "-u" => url = args.next(),
            "--user" | "-U" => user = args.next(),
            "--pass" | "--password" | "-p" => pass = args.next(),
            "--help" | "-h" => {
                println!("HydroxyTorrent Remote Control TUI (hydroxy_rc_tui)");
                println!();
                println!("Usage: hydroxy_rc_tui [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -u, --url <URL>        Daemon RPC URL (default: from config.toml or http://127.0.0.1:10000)");
                println!("  -U, --user <USERNAME>  RPC Username");
                println!("  -p, --pass <PASSWORD>  RPC Password");
                println!("  -h, --help             Print help");
                std::process::exit(0);
            }
            _ => {}
        }
    }

    (url, user, pass)
}

#[tokio::main]
async fn main() -> Result<()> {
    let (cli_url, cli_user, cli_pass) = parse_cli_args();

    // Install panic hook so terminal raw mode is always restored even on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cli_url, cli_user, cli_pass);
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    // Initial fetch
    app.refresh_all().await;
    let mut last_poll = Instant::now();
    let poll_interval = Duration::from_millis(1500);

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if app.should_quit {
            break;
        }

        let timeout = poll_interval.saturating_sub(last_poll.elapsed());
        let wait = timeout.min(Duration::from_millis(50));

        if event::poll(wait)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key).await;
                }
            }
        }

        if last_poll.elapsed() >= poll_interval {
            app.refresh_all().await;
            last_poll = Instant::now();
        }
    }

    Ok(())
}
