use std::sync::{Arc, Mutex};
mod api;
mod auth;
mod tui;

fn main() -> anyhow::Result<()> {
    // try to load token, else start auth
    let token = match auth::load_token() {
        Some(token) => token,
        None => auth::authenticate()?,
    };

    let token = Arc::new(Mutex::new(token));

    auth::start_auto_refresh(Arc::clone(&token));

    let mut api = api::API::init(Arc::clone(&token));

    tui::run(&mut api).map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}
