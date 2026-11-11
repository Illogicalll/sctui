use std::sync::{Arc, Mutex, mpsc};
mod api;
mod auth;
mod config;
mod keymap;
mod theme;
mod player;
mod media;
mod tui;
mod update;
use player::Player;

const USAGE: &str = "\
sctui - a soundcloud client for the terminal

USAGE: sctui [OPTIONS]

OPTIONS:
  -V, --version          print version and exit
  -h, --help             print this help and exit
      --dump-config      print the default config (theme + key bindings) to stdout and exit
      --no-update-check  don't check GitHub for a newer release on startup
      --setup            run the setup wizard (layout, theme, key tour) again";

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("sctui {}", update::CURRENT);
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--dump-config") {
        print!("{}", config::template());
        return Ok(());
    }
    let loaded = config::load();
    if !loaded.warnings.is_empty() {
        for w in &loaded.warnings {
            eprintln!("config: {w}");
        }
        eprintln!("(continuing with the remaining bindings)");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    if !args.iter().any(|a| a == "--no-update-check") {
        update::maybe_self_update();
    }

    // stored token if still valid; refresh it if expired; browser login only as last resort
    let token = auth::startup_token()?;

    let token = Arc::new(Mutex::new(token));
    let (reauth_tx, reauth_rx) = mpsc::channel();

    auth::start_auto_refresh(Arc::clone(&token), reauth_tx.clone());

    let mut api = Arc::new(Mutex::new(api::API::init(Arc::clone(&token))));

    // Before the TUI takes the terminal: with no audio output there is nothing to run.
    let player = Player::new(Arc::clone(&token))?;

    // spawn a thread to handle re-authentication requests
    let token_clone = Arc::clone(&token);
    std::thread::spawn(move || {
        for _ in reauth_rx.iter() {
            // re-authenticate (this will block waiting for user interaction)
            let result = auth::authenticate();
            // The refresher re-asked every minute while that blocked; one login answers them all.
            reauth_rx.try_iter().for_each(drop);
            match result {
                Ok(new_token) => {
                    // `API` and `Player` share this Arc, so this is all they need.
                    // Rebuilding `API` here would reset its page cursors and every
                    // library list would be fetched, and appended, a second time.
                    *token_clone.lock().unwrap() = new_token;
                }
                Err(_) => {
                    // re-authentication failed, will try again on next refresh failure
                }
            }
        }
    });

    tui::run(&mut api, player, loaded, args.iter().any(|a| a == "--setup")).map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}
