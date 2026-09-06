//! Lyrics from LRCLIB (free, keyless). Synced lines when it has them, plain
//! text otherwise. SoundCloud titles are messy ("Artist - Song (feat. X) [FREE
//! DL]") and the uploader is often not the artist, so a couple of cleaned
//! candidates are tried, then a duration-matched search.

use crate::api::Track;

const USER_AGENT: &str = concat!("sctui/", env!("CARGO_PKG_VERSION"), " (https://github.com/Illogicalll/sctui)");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lyrics {
    /// `(timestamp ms, line)` sorted by time. Empty when only plain text exists.
    pub synced: Vec<(u64, String)>,
    pub plain: Vec<String>,
}

impl Lyrics {
    pub fn is_synced(&self) -> bool {
        !self.synced.is_empty()
    }
}

/// Strip the decorations SoundCloud titles carry.
pub(crate) fn clean_title(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let lower = out.to_lowercase();
    for marker in [" official", " audio", " lyrics", " free download", " prod.", " prod ", " ft.", " ft ", " feat.", " feat "] {
        if let Some(i) = lower.find(marker) {
            out.truncate(i);
            break;
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| " -–|".contains(c)).to_string()
}

/// `(artist, song)` pairs to try, most specific first.
pub(crate) fn candidates(track: &Track) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some((artist, song)) = track.title.split_once(" - ") {
        out.push((artist.trim().to_string(), clean_title(song)));
    }
    out.push((track.artists.clone(), clean_title(&track.title)));
    out.retain(|(a, s)| !a.is_empty() && !s.is_empty());
    out.dedup();
    out
}

/// Parse LRC text. Lines may carry several tags (`[00:12.00][01:03.50]text`).
pub(crate) fn parse_lrc(text: &str) -> Vec<(u64, String)> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim_start();
        let mut stamps = Vec::new();
        while let Some(end) = rest.strip_prefix('[').and_then(|r| r.find(']')) {
            let tag = &rest[1..=end];
            let Some(ms) = parse_stamp(tag) else { break };
            stamps.push(ms);
            rest = rest[end + 2..].trim_start();
        }
        for ms in stamps {
            lines.push((ms, rest.trim().to_string()));
        }
    }
    lines.sort_by_key(|(ms, _)| *ms);
    lines
}

/// `mm:ss.xx`, `mm:ss.xxx` or `mm:ss` → milliseconds.
fn parse_stamp(tag: &str) -> Option<u64> {
    let (m, s) = tag.split_once(':')?;
    let m: u64 = m.trim().parse().ok()?;
    let (s, frac) = s.split_once('.').unwrap_or((s, "0"));
    let s: u64 = s.trim().parse().ok()?;
    let frac_ms = match frac.len() {
        0 => 0,
        1 => frac.parse::<u64>().ok()? * 100,
        2 => frac.parse::<u64>().ok()? * 10,
        _ => frac[..3].parse::<u64>().ok()?,
    };
    Some((m * 60 + s) * 1000 + frac_ms)
}

fn from_json(v: &serde_json::Value) -> Option<Lyrics> {
    let synced = v.get("syncedLyrics").and_then(|s| s.as_str()).map(parse_lrc).unwrap_or_default();
    let plain: Vec<String> = v
        .get("plainLyrics")
        .and_then(|s| s.as_str())
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default();
    if synced.is_empty() && plain.iter().all(|l| l.trim().is_empty()) {
        return None;
    }
    Some(Lyrics { synced, plain })
}

/// Best match for `track`, or `None` when LRCLIB has nothing plausible.
/// LRCLIB often holds several entries per song; a synced one anywhere wins
/// over a plain one from the exact endpoint.
pub async fn fetch_lyrics(track: Track) -> Option<Lyrics> {
    let client = reqwest::Client::builder().user_agent(USER_AGENT).build().ok()?;
    let duration_s = (track.duration_ms as f64 / 1000.0).round() as i64;
    let mut plain_fallback: Option<Lyrics> = None;

    for (artist, song) in candidates(&track) {
        let mut found: Vec<Lyrics> = Vec::new();

        let get = client
            .get("https://lrclib.net/api/get")
            .query(&[
                ("track_name", song.as_str()),
                ("artist_name", artist.as_str()),
                ("duration", &duration_s.to_string()),
            ])
            .send()
            .await;
        if let Ok(resp) = get
            && resp.status().is_success()
            && let Ok(v) = resp.json::<serde_json::Value>().await
        {
            found.extend(from_json(&v));
        }

        let search = client
            .get("https://lrclib.net/api/search")
            .query(&[("q", format!("{artist} {song}"))])
            .send()
            .await;
        if let Ok(resp) = search
            && resp.status().is_success()
            && let Ok(v) = resp.json::<serde_json::Value>().await
            && let Some(items) = v.as_array()
        {
            // Only entries within a few seconds of our length.
            found.extend(items.iter().filter_map(|item| {
                let d = item.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0).round() as i64;
                ((d - duration_s).abs() <= 3).then(|| from_json(item)).flatten()
            }));
        }

        if let Some(synced) = found.iter().find(|l| l.is_synced()) {
            return Some(synced.clone());
        }
        if plain_fallback.is_none() {
            plain_fallback = found.into_iter().next();
        }
    }
    plain_fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artists: &str) -> Track {
        Track {
            title: title.into(),
            artists: artists.into(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: String::new(),
        }
    }

    #[test]
    fn titles_are_cleaned_and_split() {
        assert_eq!(clean_title("hypnosis (feat. sevinchi) [FREE DL]"), "hypnosis");
        assert_eq!(clean_title("tomorrow ft. Temachii"), "tomorrow");
        assert_eq!(clean_title("Dark And Long (Official Audio)"), "Dark And Long");
        assert_eq!(
            candidates(&track("Underworld - Dark And Long (Remaster)", "SNAUT")),
            vec![("Underworld".to_string(), "Dark And Long".to_string()), ("SNAUT".to_string(), "Underworld - Dark And Long".to_string())]
        );
        assert_eq!(candidates(&track("L-ON-D-ON", "Bassvictim")), vec![("Bassvictim".to_string(), "L-ON-D-ON".to_string())]);
    }

    #[test]
    fn lrc_parses_multi_tag_lines_and_sorts() {
        let lrc = "[00:12.00]First line\n[00:05.50][01:00.5]Repeated\n[00:20]\ngarbage\n[00:30.123]Third";
        let lines = parse_lrc(lrc);
        assert_eq!(
            lines,
            vec![
                (5_500, "Repeated".to_string()),
                (12_000, "First line".to_string()),
                (20_000, String::new()),
                (30_123, "Third".to_string()),
                (60_500, "Repeated".to_string()),
            ]
        );
    }

    #[test]
    fn json_falls_back_to_plain_and_rejects_empty() {
        let v: serde_json::Value = serde_json::json!({"syncedLyrics": null, "plainLyrics": "la la\nla"});
        let l = from_json(&v).unwrap();
        assert!(!l.is_synced());
        assert_eq!(l.plain, vec!["la la", "la"]);
        assert!(from_json(&serde_json::json!({"syncedLyrics": "", "plainLyrics": " \n"})).is_none());
    }
}

/// Live LRCLIB lookup; needs network. `cargo test -- --ignored lrclib_live`.
#[cfg(test)]
mod live_tests {
    use super::*;

    #[test]
    #[ignore]
    fn lrclib_live_lookup_finds_a_known_track() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut t = tests_track("L-ON-D-ON", "Bassvictim");
        t.duration_ms = 188_000;
        let found = rt.block_on(fetch_lyrics(t)).expect("LRCLIB has this one");
        assert!(found.is_synced(), "expected synced lines, got {} plain lines", found.plain.len());
        assert!(found.synced.len() > 10);
        assert!(rt.block_on(fetch_lyrics(tests_track("zzqx unlikely title 9f8a", "nobody"))).is_none());
    }

    fn tests_track(title: &str, artists: &str) -> Track {
        Track {
            title: title.into(),
            artists: artists.into(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: String::new(),
        }
    }
}
