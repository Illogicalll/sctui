use anyhow::{Result, bail};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::relay_post;
use super::token::{Token, REFRESH_TIME};

static REFRESH_BUFFER: u64 = 300;
/// How long a refresh that failed for no fault of the token is held off.
const TRANSIENT_RETRY_SECS: u64 = 10;
/// The refresh token that last failed to refresh, how many times in a row it was
/// rejected, and the time before which it is not tried again.
static LAST_FAILURE: Mutex<(String, u32, u64)> = Mutex::new((String::new(), 0, 0));

/// Why a refresh failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    /// The relay or SoundCloud answered and said no (`invalid_grant` and the like):
    /// this refresh token is dead and only a new login helps.
    Rejected,
    /// Nothing wrong with the token as far as anyone can tell: no connection, a
    /// timeout, a 5xx, a rate limit. Trying again later can work.
    Transient,
}

impl Failure {
    /// From the HTTP status of the failed request, `None` when there was no response.
    fn from_status(status: Option<u16>) -> Self {
        match status {
            Some(408 | 429) => Failure::Transient,
            Some(400..=499) => Failure::Rejected,
            _ => Failure::Transient,
        }
    }

    fn of(error: &anyhow::Error) -> Self {
        let status = error.downcast_ref::<reqwest::Error>().and_then(|e| e.status());
        Self::from_status(status.map(|s| s.as_u16()))
    }

    /// The rejection count and earliest retry time after this failure at `now`, given
    /// `rejections` earlier ones in a row. A rejection waits 1 minute, doubling to an
    /// hour; anything else leaves that count alone and waits a few seconds.
    fn backoff(self, rejections: u32, now: u64) -> (u32, u64) {
        match self {
            Failure::Rejected => {
                let rejections = rejections + 1;
                (rejections, now + (30u64 << rejections.min(7)).min(3600))
            }
            Failure::Transient => (rejections, now + TRANSIENT_RETRY_SECS),
        }
    }
}

pub fn refresh_token(old_token: &Token) -> Result<Token> {
    let refresh_token = &old_token.refresh_token;
    with_backoff(refresh_token, || relay_post("/refresh", &[("refresh_token", refresh_token)]))
}

/// Runs `refresh` unless this refresh token failed too recently (see `Failure::backoff`).
/// Every refresh ends up here, and while the token is expired one is attempted before
/// every API call, so without this a token SoundCloud had rejected was retried ~5 times
/// a second until the relay hit Cloudflare's daily request cap.
fn with_backoff(refresh_token: &str, refresh: impl FnOnce() -> Result<Token>) -> Result<Token> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut last = LAST_FAILURE.lock().unwrap();
    // A different token (from a fresh login) starts with a clean slate.
    let (rejections, not_before) = if last.0 == refresh_token { (last.1, last.2) } else { (0, 0) };
    if now < not_before {
        bail!("token refresh failed recently, waiting before retrying");
    }
    let result = refresh();
    *last = match &result {
        Ok(_) => Default::default(),
        Err(e) => {
            let (rejections, not_before) = Failure::of(e).backoff(rejections, now);
            (refresh_token.to_owned(), rejections, not_before)
        }
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
                    Err(e) => {
                        drop(token_guard);
                        // Only a token that was turned down needs a new login. A dropped
                        // connection (or the hold-off after either kind of failure) is
                        // simply tried again on the next pass.
                        if Failure::of(&e) == Failure::Rejected {
                            let _ = reauth_tx.send(());
                        }
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

    /// #14: only an answer that turns the token down asks for a new login and backs
    /// off for long; a dead connection or a server error is retried shortly.
    #[test]
    fn only_a_rejected_token_needs_a_login_and_a_long_backoff() {
        for status in [400, 401, 403] {
            assert_eq!(Failure::from_status(Some(status)), Failure::Rejected, "{status}");
        }
        for status in [None, Some(408), Some(429), Some(500), Some(502), Some(503)] {
            assert_eq!(Failure::from_status(status), Failure::Transient, "{status:?}");
        }
        // An error with no HTTP response behind it (no route, timeout, the hold-off itself).
        assert_eq!(Failure::of(&anyhow::anyhow!("network unreachable")), Failure::Transient);

        // Rejections: 60s, doubling, capped at an hour.
        assert_eq!(Failure::Rejected.backoff(0, 1000), (1, 1060));
        assert_eq!(Failure::Rejected.backoff(1, 1000), (2, 1120));
        assert_eq!(Failure::Rejected.backoff(5, 1000), (6, 2920));
        assert_eq!(Failure::Rejected.backoff(6, 1000), (7, 4600));
        assert_eq!(Failure::Rejected.backoff(40, 1000), (41, 4600));
        // Anything else: a short wait that never grows and never counts as a rejection.
        assert_eq!(Failure::Transient.backoff(0, 1000), (0, 1000 + TRANSIENT_RETRY_SECS));
        assert_eq!(Failure::Transient.backoff(3, 1000), (3, 1000 + TRANSIENT_RETRY_SECS));
    }
}
