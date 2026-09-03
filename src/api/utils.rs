use super::models::Track;

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
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
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

    let artwork_url = parse_str(obj, "artwork_url");
    let stream_url = parse_str(obj, "stream_url");
    let access = parse_str(obj, "access");
    let track_urn = parse_str(obj, "urn");

    Track {
        title,
        artists,
        duration,
        duration_ms,
        playback_count,
        artwork_url,
        stream_url,
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
