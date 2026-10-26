use anyhow::{Result, bail};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::relay_post;
use super::token::{Token, REFRESH_TIME};

static REFRESH_BUFFER: u64 = 300;
/// The refresh token that last failed to refresh, how many times in a row, and when.
static LAST_FAILURE: Mutex<(String, u32, u64)> = Mutex::new((String::new(), 0, 0));

pub fn refresh_token(old_token: &Token) -> Result<Token> {
    let refresh_token = &old_token.refresh_token;
    with_backoff(refresh_token, || relay_post("/refresh", &[("refresh_token", refresh_token)]))
}

/// Runs `refresh` unless this refresh token failed too recently: 1 minute after a failure,
/// doubling to an hour. Every refresh ends up here, and while the token is expired one is
/// attempted before every API call (the UI asks for the open tab's next page on each 200ms
/// tick), so without this a token SoundCloud had rejected was retried ~5 times a second until
/// the relay hit Cloudflare's daily request cap.
fn with_backoff(refresh_token: &str, refresh: impl FnOnce() -> Result<Token>) -> Result<Token> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut last = LAST_FAILURE.lock().unwrap();
    // A different token (from a fresh login) starts with a clean slate.
    let failures = if last.0 == refresh_token { last.1 } else { 0 };
    if failures > 0 && now < last.2 + (30u64 << failures.min(7)).min(3600) {
        bail!("token refresh failed {failures} time(s) in a row, waiting before retrying");
    }
    let result = refresh();
    *last = match result {
        Ok(_) => Default::default(),
        Err(_) => (refresh_token.to_owned(), failures + 1, now),
    };
    result
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
                match refresh_token(&token_guard) {
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
        if !token_guard.is_expired() {
            // Another thread refreshed it while we waited for the lock.
            return Ok(());
        }
        match refresh_token(&token_guard) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_refresh_is_not_retried_straight_away() {
        assert!(with_backoff("dead", || bail!("invalid_grant")).is_err());
        // The retry must not reach the relay at all.
        assert!(with_backoff("dead", || panic!("retried without waiting")).is_err());
        // A fresh login's token is not held back by the dead one's failures.
        let fresh = with_backoff("fresh", || bail!("tried"));
        assert!(fresh.is_err_and(|e| e.to_string() == "tried"));
    }
}
