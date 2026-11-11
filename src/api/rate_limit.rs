//! SoundCloud's rate limit. A 429 says when the limit resets in its JSON body
//! (`reset_time`; there is no `Retry-After`). From then until that time every request
//! made through `send_gated` is refused here, before it is sent: the API terms forbid
//! working around a limit, and retrying into one is the quickest way to do that.

use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Duration, Local, Utc};
use reqwest::StatusCode;

/// How long to hold when a 429 does not say when the limit resets, or names a time
/// already past (a skewed clock).
// ponytail: a fixed 5 minutes. The first request after it probes again and a further
// 429 holds for another 5; back off exponentially if SoundCloud is ever seen sending
// 429s without a usable `reset_time`.
const FALLBACK_HOLD_SECS: i64 = 300;

/// A request SoundCloud answered with 429, or one not sent because of the hold since.
#[derive(Debug)]
pub struct RateLimited {
    pub until: DateTime<Utc>,
}

impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rate limited by SoundCloud until {}", self.until.with_timezone(&Local).format("%H:%M:%S"))
    }
}

impl std::error::Error for RateLimited {}

/// The unix second the limit lifts at; anything not in the future means no limit.
pub(crate) struct RateLimit(AtomicI64);

static LIMIT: RateLimit = RateLimit(AtomicI64::new(0));

impl RateLimit {
    /// When the limit lifts, if one is in force at `now`.
    pub(crate) fn until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp(self.0.load(Ordering::SeqCst), 0).filter(|until| *until > now)
    }

    /// A 429 with `body` arrived at `now`: nothing more is sent until its `reset_time`.
    pub(crate) fn note(&self, body: &str, now: DateTime<Utc>) -> RateLimited {
        let until = reset_time(body)
            .filter(|at| *at > now)
            .unwrap_or(now + Duration::seconds(FALLBACK_HOLD_SECS));
        // Never earlier than a limit already noted.
        self.0.fetch_max(until.timestamp(), Ordering::SeqCst);
        RateLimited { until }
    }

    fn check(&self, now: DateTime<Utc>) -> Result<(), RateLimited> {
        self.until(now).map_or(Ok(()), |until| Err(RateLimited { until }))
    }
}

/// `reset_time` (`2026/09/04 14:32:10 +0000`) from a 429's body. The key is looked for
/// as text: it sits a few levels down, and where exactly is not worth depending on.
fn reset_time(body: &str) -> Option<DateTime<Utc>> {
    const KEY: &str = "\"reset_time\"";
    let value = body[body.find(KEY)? + KEY.len()..].split('"').nth(1)?;
    DateTime::parse_from_str(value, "%Y/%m/%d %H:%M:%S %z").ok().map(|at| at.to_utc())
}

/// When SoundCloud's rate limit lifts, while one is in force.
pub fn until() -> Option<DateTime<Local>> {
    LIMIT.until(Utc::now()).map(Into::into)
}

/// `send` and `error_for_status` in one, behind the rate limit: nothing is sent while
/// it is in force, and a 429 puts it in force. Either way the error is a [`RateLimited`].
pub(crate) trait SendGated {
    fn send_gated(self) -> anyhow::Result<reqwest::blocking::Response>;
}

impl SendGated for reqwest::blocking::RequestBuilder {
    fn send_gated(self) -> anyhow::Result<reqwest::blocking::Response> {
        LIMIT.check(Utc::now())?;
        let resp = self.send()?;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(LIMIT.note(&resp.text().unwrap_or_default(), Utc::now()).into());
        }
        Ok(resp.error_for_status()?)
    }
}

/// [`SendGated`] for the async client.
pub(crate) trait SendGatedAsync {
    async fn send_gated(self) -> anyhow::Result<reqwest::Response>;
}

impl SendGatedAsync for reqwest::RequestBuilder {
    async fn send_gated(self) -> anyhow::Result<reqwest::Response> {
        LIMIT.check(Utc::now())?;
        let resp = self.send().await?;
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(LIMIT.note(&resp.text().await.unwrap_or_default(), Utc::now()).into());
        }
        Ok(resp.error_for_status()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 4, h, m, s).unwrap()
    }

    /// The body SoundCloud documents for a 429.
    fn body(reset_time: &str) -> String {
        format!(
            r#"{{"errors":[{{"meta":{{"rate_limit":{{"group":"plays","max_nr_of_requests":15000,"time_window":"PT24H"}},"remaining_requests":0,"reset_time":"{reset_time}"}}}}]}}"#
        )
    }

    #[test]
    fn reset_time_is_read_from_the_body_whatever_its_offset() {
        assert_eq!(reset_time(&body("2026/09/04 14:32:10 +0000")), Some(at(14, 32, 10)));
        assert_eq!(reset_time(&body("2026/09/04 16:32:10 +0200")), Some(at(14, 32, 10)));
        assert_eq!(reset_time(&body("2026/09/04 09:02:10 -0530")), Some(at(14, 32, 10)));
        assert_eq!(reset_time(r#"{"reset_time": "2026/09/04 14:32:10 +0000"}"#), Some(at(14, 32, 10)));
    }

    #[test]
    fn a_missing_or_malformed_reset_time_is_no_time_at_all() {
        for body in [
            String::new(),
            "<html>Too Many Requests</html>".to_string(),
            r#"{"errors":[{"meta":{"remaining_requests":0}}]}"#.to_string(),
            r#"{"reset_time":null,"group":"plays"}"#.to_string(),
            body("tomorrow"),
            body("2026-09-04T14:32:10Z"),
            body("2026/09/04 14:32:10"),
        ] {
            assert_eq!(reset_time(&body), None, "{body}");
        }
    }

    /// Paging, search and stream starts all send through this one gate.
    #[test]
    fn requests_are_held_until_reset_time_and_released_at_it() {
        let limit = RateLimit(AtomicI64::new(0));
        let now = at(14, 0, 0);
        assert!(limit.check(now).is_ok(), "no limit to begin with");

        let refused = limit.note(&body("2026/09/04 14:32:10 +0000"), now);
        assert_eq!(refused.until, at(14, 32, 10));
        assert_eq!(limit.check(now).unwrap_err().until, at(14, 32, 10));
        assert!(limit.check(at(14, 32, 9)).is_err(), "not before the reset");
        assert!(limit.check(at(14, 32, 10)).is_ok(), "released by itself");
        assert_eq!(limit.until(at(14, 32, 10)), None);
    }

    #[test]
    fn without_a_usable_reset_time_the_hold_is_five_minutes() {
        let now = at(14, 0, 0);
        for body in [String::new(), body("nonsense"), body("2026/09/04 13:00:00 +0000")] {
            let limit = RateLimit(AtomicI64::new(0));
            assert_eq!(limit.note(&body, now).until, at(14, 5, 0), "{body}");
            assert!(limit.check(at(14, 4, 59)).is_err() && limit.check(at(14, 5, 0)).is_ok());
        }
        // A later 429 never shortens a hold already in force.
        let limit = RateLimit(AtomicI64::new(0));
        limit.note(&body("2026/09/04 14:32:10 +0000"), now);
        limit.note("", now);
        assert!(limit.check(at(14, 30, 0)).is_err());
    }

    /// The error survives the context the player wraps round it, which is how the
    /// engine and the segment pump tell a rate limit from any other failure.
    #[test]
    fn the_error_is_still_recognisable_under_added_context() {
        use anyhow::Context;
        let refused: anyhow::Result<()> = Err(RateLimited { until: at(14, 32, 10) }.into());
        let err = refused.context("download returned error status").context("outer").unwrap_err();
        assert!(err.downcast_ref::<RateLimited>().is_some());
        assert!(format!("{err:#}").contains("Rate limited by SoundCloud until "));
    }

    /// Over real HTTP: the 429 is the last request made.
    #[test]
    fn a_429_stops_the_requests_that_follow_it_from_being_sent() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/tracks/1/streams", server.server_addr());
        let served = std::thread::spawn(move || {
            let mut requests = 0;
            while let Ok(Some(request)) = server.recv_timeout(std::time::Duration::from_millis(300)) {
                requests += 1;
                let reset = (Utc::now() + Duration::hours(1)).format("%Y/%m/%d %H:%M:%S %z").to_string();
                let _ = request.respond(tiny_http::Response::from_string(body(&reset)).with_status_code(429));
            }
            requests
        });

        let client = reqwest::blocking::Client::builder().no_proxy().build().unwrap();
        let first = client.get(&url).send_gated().unwrap_err();
        assert!(first.downcast_ref::<RateLimited>().is_some(), "{first:#}");
        for _ in 0..5 {
            assert!(client.get(&url).send_gated().unwrap_err().downcast_ref::<RateLimited>().is_some());
        }
        assert!(until().is_some());
        // The limit is process-wide; nothing else under test may be left behind it.
        LIMIT.0.store(0, Ordering::SeqCst);
        assert_eq!(served.join().unwrap(), 1, "only the request that was answered 429 went out");
    }
}
