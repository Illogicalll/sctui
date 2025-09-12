use std::sync::{Arc, Mutex};
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

    auth::start_auto_refresh(Arc::clone(&token));

    let mut api = Arc::new(Mutex::new(api::API::init(Arc::clone(&token))));

    let player = Player::new(Arc::clone(&token));

    tui::run(&mut api, player).map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}
