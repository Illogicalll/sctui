use anyhow::Result;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::token::{Token, REFRESH_TIME};

static REFRESH_BUFFER: u64 = 300;

pub fn refresh_token(old_token: &Token) -> Result<Token> {
    dotenvy::dotenv().ok();
    let client_id = std::env::var("SOUNDCLOUD_CLIENT_ID")?;
    let client_secret = std::env::var("SOUNDCLOUD_CLIENT_SECRET")?;
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("refresh_token", &old_token.refresh_token),
    ];
    let mut resp = reqwest::blocking::Client::new()
        .post("https://secure.soundcloud.com/oauth/token")
        .header("accept", "application/json; charset=utf-8")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()?
        .error_for_status()?
        .json::<Token>()?;

    resp.obtained_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    fs::write("token.json", serde_json::to_string_pretty(&resp)?)?;

    Ok(resp)
}

pub fn start_auto_refresh(token: Arc<Mutex<Token>>, reauth_tx: std::sync::mpsc::Sender<()>) {
    std::thread::spawn(move || {
        loop {
            let should_refresh = {
                let token_guard = token.lock().unwrap();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let time_until_expiry =
                    (token_guard.obtained_at + REFRESH_TIME).saturating_sub(now);
                time_until_expiry <= REFRESH_BUFFER
            };

            if should_refresh {
                let mut token_guard = token.lock().unwrap();
                match refresh_token(&*token_guard) {
                    Ok(new_token) => {
                        *token_guard = new_token;
                    }
                    Err(_) => {
                        drop(token_guard);
                        let _ = reauth_tx.send(());
                        std::thread::sleep(std::time::Duration::from_secs(60));
                        continue;
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    });
}

pub fn try_refresh_token(token: &Arc<Mutex<Token>>) -> Result<()> {
    let token_guard = token.lock().unwrap();
    if token_guard.is_expired() {
        drop(token_guard);
        let mut token_guard = token.lock().unwrap();
        match refresh_token(&*token_guard) {
            Ok(new_token) => {
                *token_guard = new_token;
                Ok(())
            }
            Err(e) => Err(e),
        }
    } else {
        Ok(())
    }
}
