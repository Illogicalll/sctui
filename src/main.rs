use std::sync::{Arc, Mutex, mpsc};
mod api;
mod auth;
mod player;
mod tui;
use player::Player;

fn main() -> anyhow::Result<()> {
    // try to load token, else start auth
    let token = match auth::load_token() {
        Some(token) => token,
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
    let player_token_clone = Arc::clone(&token);
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
