use std::sync::{Arc, Mutex, mpsc};
mod api;
mod auth;
mod player;
mod tui;
mod update;
use player::Player;

const USAGE: &str = "\
sctui - a soundcloud client for the terminal

USAGE: sctui [OPTIONS]

OPTIONS:
  -V, --version          print version and exit
  -h, --help             print this help and exit
      --no-update-check  don't check GitHub for a newer release on startup";

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
    if !args.iter().any(|a| a == "--no-update-check") {
        update::maybe_self_update();
    }

    // stored token if still valid; refresh it if expired; browser login only as last resort
    let token = match auth::load_token() {
        Some(token) if !token.is_expired() => token,
        Some(token) => auth::refresh_token(&token).or_else(|_| auth::authenticate())?,
        None => auth::authenticate()?,
    };

    let token = Arc::new(Mutex::new(token));
    let (reauth_tx, reauth_rx) = mpsc::channel();

    auth::start_auto_refresh(Arc::clone(&token), reauth_tx.clone());

    let mut api = Arc::new(Mutex::new(api::API::init(Arc::clone(&token))));

    let player = Player::new(Arc::clone(&token));

    // spawn a thread to handle re-authentication requests
    let token_clone = Arc::clone(&token);
    let api_clone = Arc::clone(&api);
    std::thread::spawn(move || {
        for _ in reauth_rx {
            // re-authenticate (this will block waiting for user interaction)
            match auth::authenticate() {
                Ok(new_token) => {
                    *token_clone.lock().unwrap() = new_token;
                    // update API with new token
                    *api_clone.lock().unwrap() = api::API::init(Arc::clone(&token_clone));
                }
                Err(_) => {
                    // re-authentication failed, will try again on next refresh failure
                }
            }
        }
    });

    tui::run(&mut api, player).map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}
