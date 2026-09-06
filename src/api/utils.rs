use chrono::{DateTime, FixedOffset, Utc};
use std::sync::{Arc, Mutex};

use super::models::Track;
use crate::auth::{Token, try_refresh_token};

/// Refreshes the token if needed and returns a clone of the current access token.
pub(crate) fn access_token(token: &Arc<Mutex<Token>>) -> String {
    let _ = try_refresh_token(token);
    token.lock().unwrap().access_token.clone()
}

/// Largest whole unit of an elapsed time: "now", "5m", "3h", "2d", "1w", "1mo", "1y".
pub(crate) fn format_age(secs: u64) -> String {
    const UNITS: [(u64, &str); 6] = [
        (31_536_000, "y"),
        (2_592_000, "mo"),
        (604_800, "w"),
        (86_400, "d"),
        (3_600, "h"),
        (60, "m"),
    ];
    UNITS
        .iter()
        .find(|(len, _)| secs >= *len)
        .map(|(len, name)| format!("{}{name}", secs / len))
        .unwrap_or_else(|| "now".to_string())
}

/// SoundCloud's `2026/09/04 00:00:00 +0000` timestamps; now on parse failure.
pub(crate) fn parse_datetime(obj: &serde_json::Value, key: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_str(&parse_str(obj, key), "%Y/%m/%d %H:%M:%S %z")
        .unwrap_or_else(|_| Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()))
}

pub(crate) fn format_playback_count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.2}K", n as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.2}M", n as f64 / 1_000_000.0),
        1_000_000_000..=999_999_999_999 => format!("{:.2}B", n as f64 / 1_000_000_000.0),
        _ => format!("{:.2}T", n as f64 / 1_000_000_000_000.0),
    }
}

pub(crate) fn format_duration(duration_ms: u64) -> String {
    let duration_sec = duration_ms / 1000;
    let hours = duration_sec / 3600;
    let minutes = (duration_sec % 3600) / 60;
    let seconds = duration_sec % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

pub(crate) fn parse_str(obj: &serde_json::Value, key: &str) -> String {
    sanitize_display(obj.get(key).and_then(|v| v.as_str()).unwrap_or(""))
}

/// Drops the invisible code points that make terminals and ratatui disagree
/// about how many cells a string occupies: variation selectors (`◼️`, `☑️`),
/// zero-width joiners and spaces, emoji skin-tone modifiers. A mismatch shifts
/// the rest of the row on screen and leaves ghost characters behind, because
/// ratatui's diff never sees the shift.
pub(crate) fn sanitize_display(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            !matches!(c,
                '\u{FE00}'..='\u{FE0F}'   // variation selectors
                | '\u{200B}'..='\u{200F}' // zero-width space/joiners, marks
                | '\u{2060}'               // word joiner
                | '\u{FEFF}'               // BOM
                | '\u{1F3FB}'..='\u{1F3FF}' // emoji skin-tone modifiers
            )
        })
        .collect()
}

#[cfg(test)]
mod sanitize_tests {
    use super::sanitize_display;

    #[test]
    fn strips_width_hazards_and_keeps_text() {
        assert_eq!(sanitize_display("He incognito\u{25FC}\u{FE0F} - 2"), "He incognito\u{25FC} - 2");
        assert_eq!(sanitize_display("T H E \u{2611}\u{FE0F}"), "T H E \u{2611}");
        assert_eq!(sanitize_display("\u{1F64F}\u{1F3FD}\u{2728}"), "\u{1F64F}\u{2728}");
        assert_eq!(sanitize_display("a\u{200D}b\u{200B}c"), "abc");
        assert_eq!(sanitize_display("AÚN NO HE TERMINADO"), "AÚN NO HE TERMINADO");
        assert_eq!(sanitize_display("HE/HIM\u{1F49A}"), "HE/HIM\u{1F49A}");
    }
}

pub(crate) fn parse_u64(obj: &serde_json::Value, key: &str) -> u64 {
    obj.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

pub(crate) fn parse_next_href(resp: &serde_json::Value) -> Option<String> {
    resp.get("next_href")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub(crate) fn parse_track(obj: &serde_json::Value) -> Track {
    let title = parse_str(obj, "title");

    let artists = parse_str(obj, "metadata_artist");
    let artists = if !artists.is_empty() {
        artists
    } else {
        parse_str(obj.get("user").unwrap_or(&serde_json::Value::Null), "username")
    };

    let duration_ms = parse_u64(obj, "duration");
    let duration = format_duration(duration_ms);

    let playback_count = format_playback_count(parse_u64(obj, "playback_count"));

    // SoundCloud hands out the 100x100 "-large" thumbnail; the CDN also serves
    // 500x500 under "-t500x500", which is what the big cover-art views need.
    let artwork_url = parse_str(obj, "artwork_url").replace("-large.", "-t500x500.");
    let access = parse_str(obj, "access");
    let track_urn = parse_str(obj, "urn");

    Track {
        title,
        artists,
        duration,
        duration_ms,
        playback_count,
        artwork_url,
        access,
        track_urn,
    }
}

pub(crate) fn response_items(resp: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
        collection.clone()
    } else if let Some(array) = resp.as_array() {
        array.clone()
    } else {
        Vec::new()
    }
}
