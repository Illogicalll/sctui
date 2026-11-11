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

/// Drop everything inside `()`, `[]` and `{}`.
fn strip_brackets(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                out.push(' ');
            }
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Strip the decorations SoundCloud titles carry: bracketed asides, a leading
/// track number ("09 - "), and trailing credits ("ft. X", "prod. Y").
pub(crate) fn clean_title(title: &str) -> String {
    let mut out = strip_brackets(title);
    // "09 - Song", "3. Song": one or two digits, then a separator.
    let trimmed = out.trim_start();
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if (1..=2).contains(&digits) {
        let rest = trimmed[digits..].trim_start();
        if let Some(after) = rest.strip_prefix(['-', '.', ')', '–', '—']).filter(|a| a.starts_with(' ')) {
            out = after.to_string();
        }
    }
    // ASCII lowering keeps byte offsets, so the cut lands on the same character.
    let lower = out.to_ascii_lowercase();
    let cut = [" official", " lyrics", " free download", " free dl", " out now", " prod.", " prod ", " ft.", " ft ", " feat.", " feat ", " featuring ", " w/ "]
        .iter()
        .filter_map(|m| lower.find(m))
        .min();
    if let Some(i) = cut {
        out.truncate(i);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c: char| " -–—|~/".contains(c)).to_string()
}

/// The pieces of a title either side of " - ", " | " and friends, cleaned, with
/// bare track numbers dropped.
fn parts(title: &str) -> Vec<String> {
    let mut marked = strip_brackets(title);
    for sep in [" - ", " – ", " — ", " | ", " ~ ", " // "] {
        marked = marked.replace(sep, "\u{1}");
    }
    marked
        .split('\u{1}')
        .map(clean_title)
        .filter(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

/// `(artist, song)` pairs to try, most specific first. The uploader is often not
/// the artist and "Song - Artist" is as common as "Artist - Song", so both orders
/// are tried, then the uploader with each reading of the title.
pub(crate) fn candidates(track: &Track) -> Vec<(String, String)> {
    let parts = parts(&track.title);
    let whole = clean_title(&track.title);
    let uploader = track.artists.trim().to_string();
    // "deadmau5, Kaskade" → "deadmau5".
    let lead = uploader.split([',', '&']).next().unwrap_or("").trim().to_string();
    let mut out: Vec<(String, String)> = Vec::new();
    if parts.len() >= 2 {
        out.push((parts[0].clone(), parts[1].clone()));
        out.push((parts[1].clone(), parts[0].clone()));
    }
    if let Some(last) = parts.last() {
        out.push((uploader.clone(), last.clone()));
    }
    out.push((uploader.clone(), whole.clone()));
    if lead != uploader {
        out.push((lead, parts.last().cloned().unwrap_or(whole)));
    }
    out.retain(|(a, s)| !a.is_empty() && !s.is_empty());
    let mut seen = Vec::new();
    out.retain(|c| {
        let key = (norm(&c.0), norm(&c.1));
        !seen.contains(&key) && {
            seen.push(key);
            true
        }
    });
    out
}

/// Lowercase words only: brackets gone, "&" spelled out, apostrophes and dots
/// closed up ("a.s.o." → "aso", "god's" → "gods"), other punctuation a space.
fn norm(s: &str) -> String {
    let mut out = String::new();
    for c in strip_brackets(s).chars() {
        match c {
            '\'' | '’' | '`' | '.' => {}
            '&' => out.push_str(" and "),
            c if c.is_alphanumeric() => out.extend(c.to_lowercase()),
            _ => out.push(' '),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Words that tell editions apart, not songs.
const NOISE: [&str; 16] = [
    "remaster", "remastered", "version", "edit", "mix", "radio", "original", "album", "single", "explicit", "clean",
    "mono", "stereo", "deluxe", "bonus", "track",
];

fn is_noise(word: &str) -> bool {
    NOISE.contains(&word) || (word.len() == 4 && (word.starts_with("19") || word.starts_with("20")) && word.chars().all(|c| c.is_ascii_digit()))
}

/// The words of `s` that identify a song. Edition words are dropped unless they
/// are all there is.
fn words(s: &str) -> Vec<String> {
    let all: Vec<String> = norm(s).split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect();
    let kept: Vec<String> = all.iter().filter(|w| !is_noise(w)).cloned().collect();
    if kept.is_empty() { all } else { kept }
}

/// Share of `needles` found in `hay`. Nothing to look for counts as no match.
fn coverage(needles: &[String], hay: &[String]) -> f32 {
    if needles.is_empty() {
        return 0.0;
    }
    needles.iter().filter(|n| hay.contains(n)).count() as f32 / needles.len() as f32
}

/// What we know about the playing track, in the form results are compared against.
struct Wanted {
    /// Words of the cleaned title (every part of it).
    core: Vec<String>,
    /// Every word in the raw title (bracketed credits included) and the uploader's name.
    all: Vec<String>,
    duration_s: f64,
}

impl Wanted {
    fn of(track: &Track) -> Wanted {
        let core = words(&parts(&track.title).join(" "));
        let raw: String = track.title.chars().map(|c| if "()[]{}".contains(c) { ' ' } else { c }).collect();
        let mut all = words(&raw);
        all.extend(words(&track.artists));
        Wanted { core, all, duration_s: track.duration_ms as f64 / 1000.0 }
    }

    /// How well a result fits, or `None` when it is not the same song.
    /// Three kinds of evidence: the title, the artist and the length. Any two that
    /// agree are enough; one alone ("Dreams" by somebody) is not.
    fn score(&self, title: &str, artist: &str, duration_s: f64, synced: bool) -> Option<f32> {
        let title_words = words(title);
        let artist_words = words(artist);
        // Every word of the result's title is in ours…
        let precise = coverage(&title_words, &self.all);
        // …and it (with its artist) accounts for our title.
        let mut theirs = title_words.clone();
        theirs.extend(artist_words.iter().cloned());
        let recall = coverage(&self.core, &theirs);
        let artist_fit = coverage(&artist_words, &self.all);
        let gap = (duration_s - self.duration_s).abs();
        let length_fit = if self.duration_s <= 0.0 || duration_s <= 0.0 {
            0.5
        } else if gap <= 2.5 {
            1.0
        } else if gap <= 5.5 {
            0.8
        } else if gap <= 15.0 {
            0.4
        } else if gap <= 40.0 {
            0.1
        } else {
            0.0
        };
        let needed = if title_words.len() <= 3 { 0.99 } else { 0.8 };
        if precise < needed {
            return None;
        }
        let same_title = recall >= 0.6;
        let same_artist = artist_fit >= 0.5;
        let same_length = length_fit >= 0.8;
        let agree = (same_artist && (same_title || same_length)) || (recall >= 0.99 && same_length);
        if !agree {
            return None;
        }
        Some(precise + 2.0 * recall + 3.0 * artist_fit + 2.0 * length_fit + if synced && gap <= SYNC_TOLERANCE_S { 1.0 } else { 0.0 })
    }
}

/// Synced lines from a recording this much longer or shorter than ours would
/// scroll out of step, so they are shown as plain text instead.
const SYNC_TOLERANCE_S: f64 = 10.0;

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
        _ => frac.get(..3)?.parse::<u64>().ok()?,
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

/// One LRCLIB record: lyrics, or a note that the track is instrumental.
struct Entry {
    id: i64,
    title: String,
    artist: String,
    duration_s: f64,
    /// `None` for a record marked instrumental.
    lyrics: Option<Lyrics>,
}

fn entry(v: &serde_json::Value) -> Option<Entry> {
    let text = |k: &str| v.get(k).and_then(|s| s.as_str()).unwrap_or("").to_string();
    let lyrics = from_json(v);
    let instrumental = v.get("instrumental").and_then(|b| b.as_bool()).unwrap_or(false);
    if lyrics.is_none() && !instrumental {
        return None;
    }
    Some(Entry {
        id: v.get("id").and_then(|i| i.as_i64()).unwrap_or(0),
        title: text("trackName"),
        artist: text("artistName"),
        duration_s: v.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0),
        lyrics,
    })
}

/// What the lyrics view shows for a track LRCLIB lists as having no words.
pub const INSTRUMENTAL: &str = "♪ Instrumental ♪";

/// The searches to run for a track, most specific first.
fn queries(track: &Track) -> Vec<Vec<(&'static str, String)>> {
    let mut out: Vec<Vec<(&'static str, String)>> = candidates(track)
        .into_iter()
        .take(3)
        .map(|(artist, song)| vec![("track_name", song), ("artist_name", artist)])
        .collect();
    // Free text over every field: catches "Song Artist" titles with no separator.
    let whole = parts(&track.title).join(" ");
    if !whole.is_empty() {
        out.push(vec![("q", whole.clone())]);
        out.push(vec![("q", format!("{whole} {}", track.artists))]);
    }
    out
}

/// The best acceptable entry and its score. Words beat an "instrumental" note for
/// the same song (some records are marked instrumental by mistake).
fn best<'a>(wanted: &Wanted, entries: &'a [Entry]) -> Option<(f32, &'a Entry)> {
    entries
        .iter()
        .filter_map(|e| {
            let synced = e.lyrics.as_ref().is_some_and(Lyrics::is_synced);
            let score = wanted.score(&e.title, &e.artist, e.duration_s, synced)?;
            Some((if e.lyrics.is_some() { score } else { score - 3.0 }, e))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
}

/// A cover or re-upload under a name we cannot tie to the artist: if the title
/// matches exactly, has at least two words, and every record with that title is by
/// one and the same artist, the words are that song's. A title shared by several
/// artists ("All I Am") proves nothing and is left alone.
fn unique_title<'a>(wanted: &Wanted, entries: &'a [Entry]) -> Option<&'a Entry> {
    if wanted.core.len() < 2 {
        return None;
    }
    let same: Vec<&Entry> = entries.iter().filter(|e| e.lyrics.is_some() && words(&e.title) == wanted.core).collect();
    let artist = norm(&same.first()?.artist);
    if same.len() < 2 || same.iter().any(|e| norm(&e.artist) != artist) {
        return None;
    }
    let synced = |e: &&&Entry| e.lyrics.as_ref().is_some_and(Lyrics::is_synced);
    same.iter().find(synced).or(same.first()).copied()
}

/// The lyrics to show for `wanted` out of everything the searches returned.
fn pick(wanted: &Wanted, entries: &[Entry]) -> Option<Lyrics> {
    let (e, sure_of_length) = match best(wanted, entries) {
        Some((_, e)) => (e, true),
        None => (unique_title(wanted, entries)?, false),
    };
    let Some(mut lyrics) = e.lyrics.clone() else {
        return Some(Lyrics { synced: Vec::new(), plain: vec![INSTRUMENTAL.to_string()] });
    };
    let known = wanted.duration_s > 0.0 && e.duration_s > 0.0;
    let in_step = sure_of_length && (!known || (e.duration_s - wanted.duration_s).abs() <= SYNC_TOLERANCE_S);
    if lyrics.is_synced() && !in_step {
        // A different cut of the song: right words, wrong clock.
        if lyrics.plain.iter().all(|l| l.trim().is_empty()) {
            lyrics.plain = lyrics.synced.iter().map(|(_, line)| line.clone()).collect();
        }
        lyrics.synced.clear();
    }
    Some(lyrics)
}

/// Title, artist and length all agree and the lines are timed: nothing better to find.
const CERTAIN: f32 = 8.5;

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| reqwest::Client::builder().user_agent(USER_AGENT).timeout(std::time::Duration::from_secs(8)).build().ok())
        .as_ref()
}

/// One search. `Err` means the service could not be asked (offline, timeout, still
/// rate-limited after waiting), which is different from "asked, found nothing".
async fn search(client: &reqwest::Client, query: &[(&'static str, String)]) -> Result<Vec<Entry>, String> {
    for attempt in 0..3 {
        let resp = client.get("https://lrclib.net/api/search").query(query).send().await.map_err(|e| e.to_string())?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // LRCLIB allows a burst and then asks for about a second.
            let wait = resp.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(1);
            tokio::time::sleep(std::time::Duration::from_secs(wait.clamp(1, 3) + attempt)).await;
            continue;
        }
        if !resp.status().is_success() {
            return Err(format!("LRCLIB answered {}", resp.status()));
        }
        let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        return Ok(v.as_array().map(|items| items.iter().filter_map(entry).collect()).unwrap_or_default());
    }
    Err("LRCLIB is rate-limiting".to_string())
}

/// Best match for `track`. `Ok(None)` when LRCLIB has nothing that is plausibly
/// this song; `Err` when it could not be asked, so the caller can try again later
/// instead of remembering a miss.
///
/// Every search result is scored on title, artist and length (see `Wanted::score`);
/// the best one wins, with timed lines preferred when the recording is the same length.
pub async fn fetch_lyrics(track: Track) -> Result<Option<Lyrics>, String> {
    let client = client().ok_or("no HTTP client")?;
    let wanted = Wanted::of(&track);
    let mut entries: Vec<Entry> = Vec::new();
    let mut failed: Option<String> = None;

    for query in queries(&track) {
        match search(client, &query).await {
            Ok(found) => {
                for e in found {
                    if !entries.iter().any(|have| have.id == e.id && e.id != 0) {
                        entries.push(e);
                    }
                }
            }
            Err(why) => failed = Some(why),
        }
        if best(&wanted, &entries).is_some_and(|(score, _)| score >= CERTAIN) {
            break;
        }
    }

    match (pick(&wanted, &entries), failed) {
        (Some(lyrics), _) => Ok(Some(lyrics)),
        // Some searches did not get through; the ones that failed might have found it.
        (None, Some(why)) => Err(why),
        (None, None) => Ok(None),
    }
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
        assert_eq!(clean_title("09 - Halcyon + On + On"), "Halcyon + On + On");
        assert_eq!(clean_title("300 dreams"), "300 dreams", "a number that is part of the title stays");
        assert_eq!(clean_title("2hollis - nice [cross rebirth]"), "2hollis - nice");
        // A non-ASCII letter before the marker used to shift the cut (and could panic).
        assert_eq!(clean_title("İstanbul ft. Someone"), "İstanbul");

        let pair = |a: &str, s: &str| (a.to_string(), s.to_string());
        assert_eq!(
            candidates(&track("Underworld - Dark And Long (Remaster)", "SNAUT")),
            vec![pair("Underworld", "Dark And Long"), pair("Dark And Long", "Underworld"), pair("SNAUT", "Dark And Long"), pair("SNAUT", "Underworld - Dark And Long")]
        );
        assert_eq!(candidates(&track("L-ON-D-ON", "Bassvictim")), vec![pair("Bassvictim", "L-ON-D-ON")]);
        // A track number is not an artist.
        assert_eq!(candidates(&track("11 - Somebody", "a.s.o.")), vec![pair("a.s.o.", "Somebody")]);
        // Several uploaders: also try the first alone.
        assert!(candidates(&track("I Remember (Vocal Mix)", "deadmau5, Kaskade")).contains(&pair("deadmau5", "I Remember")));
    }

    fn timed(dur: u64, title: &str, artists: &str) -> Wanted {
        let mut t = track(title, artists);
        t.duration_ms = dur * 1000;
        Wanted::of(&t)
    }

    fn rec(id: i64, title: &str, artist: &str, duration_s: f64, synced: bool) -> Entry {
        let lyrics = Lyrics {
            synced: if synced { vec![(0, format!("{title} line"))] } else { Vec::new() },
            plain: vec![format!("{title} line")],
        };
        Entry { id, title: title.into(), artist: artist.into(), duration_s, lyrics: Some(lyrics) }
    }

    #[test]
    fn two_kinds_of_evidence_are_needed() {
        // Title and artist agree; the length does not have to (a longer cut).
        assert!(timed(653, "Underworld - Dark And Long", "SNAUT").score("Dark & Long", "Underworld", 455.0, true).is_some());
        // "Song - Artist" order, uploaded by someone else.
        assert!(timed(286, "Disparate Youth - Santigold", "venadrine").score("Disparate Youth", "Santigold", 285.0, true).is_some());
        // No separator at all: artist and credit are in the title.
        assert!(timed(212, "Highway Patrol Yung Lean (Ft. Bladee)", "ʎɔᴉ").score("Highway Patrol", "Yung Lean", 212.0, true).is_some());
        // Punctuation and case do not matter.
        assert!(timed(190, "11 - Somebody", "a.s.o.").score("Somebody", "A.S.O.", 190.0, true).is_some());
        assert!(timed(162, "god's chariots - oklou", "oklou").score("God’s Chariots", "Oklou", 162.0, false).is_some());
        // Extra credit in our title, edition words in theirs.
        assert!(timed(160, "Illegal + Nia Archives", "PinkPantheress").score("Illegal", "PinkPantheress", 160.0, true).is_some());
        assert!(timed(254, "Starman (2012 Remaster)", "David Bowie").score("Starman - 2012 Remaster", "David Bowie", 254.0, true).is_some());

        // Same title by somebody else, different length: not our song.
        assert!(timed(188, "300 dreams", "After").score("Dreams", "Fleetwood Mac", 257.0, true).is_none());
        assert!(timed(182, "All I Am", "Ninajirachi").score("All I Am", "Jess Glynne", 205.0, true).is_none());
        // Same artist, different song.
        assert!(timed(214, "Star Eyes", "Magdalena Bay").score("Second Sleep", "Magdalena Bay", 288.0, true).is_none());
        // A result whose title has words ours lacks.
        assert!(timed(250, "Everlong", "Foo Fighters").score("Everlong Acoustic Cover", "Last Day Of Summer", 250.0, true).is_none());
    }

    #[test]
    fn the_best_record_wins_and_out_of_step_lines_lose_their_clock() {
        let wanted = timed(250, "Everlong", "Foo Fighters");
        let entries = vec![
            rec(1, "Everlong (Official HD Video)", "Foo Fighters", 291.0, true),
            rec(2, "Everlong", "Foo Fighters", 251.0, false),
            rec(3, "Everlong", "Foo Fighters", 250.0, true),
            rec(4, "Everlong", "Somebody Else", 180.0, true),
        ];
        assert_eq!(best(&wanted, &entries).unwrap().1.id, 3, "same length and timed beats the rest");
        assert!(pick(&wanted, &entries).unwrap().is_synced());

        // Only a much longer cut exists: the words are right, the timing is not.
        let lyrics = pick(&wanted, &entries[..1]).unwrap();
        assert!(!lyrics.is_synced());
        assert_eq!(lyrics.plain, vec!["Everlong (Official HD Video) line"]);

        // Nothing plausible.
        assert!(pick(&wanted, &entries[3..]).is_none());
    }

    #[test]
    fn instrumentals_say_so_unless_words_exist() {
        let wanted = timed(120, "Loukeman - Snoopy", "Odin");
        let note = Entry { id: 1, title: "Snoopy".into(), artist: "Loukeman".into(), duration_s: 120.0, lyrics: None };
        assert_eq!(pick(&wanted, std::slice::from_ref(&note)).unwrap().plain, vec![INSTRUMENTAL]);
        let with_words = vec![note, rec(2, "Snoopy", "Loukeman", 121.0, false)];
        assert_eq!(pick(&wanted, &with_words).unwrap().plain, vec!["Snoopy line"]);
    }

    #[test]
    fn a_cover_gets_the_song_only_when_the_title_is_one_artists() {
        // "Misguided ghosts" re-sung by an uploader LRCLIB has never heard of.
        let wanted = timed(147, "Misguided ghosts", "deer park");
        let paramore = vec![rec(1, "Misguided Ghosts", "Paramore", 182.0, true), rec(2, "Misguided Ghosts", "Paramore", 182.0, true)];
        let lyrics = pick(&wanted, &paramore).expect("a title only one artist has");
        assert!(!lyrics.is_synced(), "a cover is not timed like the original");

        // One record is not corroboration, a shared title is not evidence, one word is not a title.
        assert!(pick(&wanted, &paramore[..1]).is_none());
        let shared = vec![rec(1, "All I Am", "Jess Glynne", 205.0, true), rec(2, "All I Am", "Lynyrd Skynyrd", 300.0, true)];
        assert!(pick(&timed(182, "All I Am", "Ninajirachi"), &shared).is_none());
        let one_word = vec![rec(1, "Dreams", "Fleetwood Mac", 257.0, true), rec(2, "Dreams", "Fleetwood Mac", 257.0, true)];
        assert!(pick(&timed(100, "Dreams", "nobody"), &one_word).is_none());
    }

    #[test]
    fn stamps_with_odd_fractions_do_not_panic() {
        assert_eq!(parse_stamp("00:12.5"), Some(12_500));
        assert_eq!(parse_stamp("00:12.1234"), Some(12_123));
        assert_eq!(parse_stamp("00:12.00é"), None);
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
        let found = rt.block_on(fetch_lyrics(t)).expect("LRCLIB reachable").expect("LRCLIB has this one");
        assert!(found.is_synced(), "expected synced lines, got {} plain lines", found.plain.len());
        assert!(found.synced.len() > 10);
        assert!(rt.block_on(fetch_lyrics(tests_track("zzqx unlikely title 9f8a", "nobody"))).unwrap().is_none());
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

/// Hit-rate check against a list of real tracks; needs network.
/// `SCTUI_LYRICS_EVAL=/path/list.tsv cargo test -- --ignored lyrics_eval --nocapture`
/// where each line is `title<TAB>uploader<TAB>mm:ss`.
#[cfg(test)]
mod eval {
    use super::*;

    #[test]
    #[ignore]
    fn lyrics_eval() {
        let Ok(path) = std::env::var("SCTUI_LYRICS_EVAL") else { return };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (mut hit, mut synced, mut total) = (0, 0, 0);
        for line in std::fs::read_to_string(path).unwrap().lines().filter(|l| !l.trim().is_empty()) {
            let mut f = line.split('\t');
            let (title, artists, dur) = (f.next().unwrap(), f.next().unwrap(), f.next().unwrap());
            let (m, s) = dur.split_once(':').unwrap();
            let track = Track {
                title: title.into(),
                artists: artists.into(),
                duration: dur.into(),
                duration_ms: (m.parse::<u64>().unwrap() * 60 + s.parse::<u64>().unwrap()) * 1000,
                playback_count: String::new(),
                artwork_url: String::new(),
                access: String::new(),
                track_urn: String::new(),
            };
            total += 1;
            match rt.block_on(fetch_lyrics(track)) {
                Ok(Some(l)) => {
                    hit += 1;
                    synced += l.is_synced() as usize;
                    let first = l.synced.iter().map(|(_, t)| t.as_str()).chain(l.plain.iter().map(String::as_str)).find(|t| !t.trim().is_empty()).unwrap_or("");
                    println!("HIT  {} | {title} | {artists} | {first:.60}", if l.is_synced() { "sync " } else { "plain" });
                }
                Ok(None) => println!("MISS       | {title} | {artists}"),
                Err(why) => println!("FAIL       | {title} | {artists} | {why}"),
            }
        }
        println!("{hit}/{total} found, {synced} synced");
    }
}
