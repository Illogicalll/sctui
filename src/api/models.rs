use chrono::{DateTime, FixedOffset, Utc};

use super::utils::format_age;

/// One row of the Feed tab: something a followed user posted or reposted.
#[derive(Debug, Clone)]
pub struct Activity {
    /// Reposter for reposts once resolved, uploader otherwise.
    pub user: String,
    /// SoundCloud gives reposters as bare urns; the fetch resolves them to `user`.
    pub reposter_urn: Option<String>,
    /// "Post" or "Repost".
    pub action: &'static str,
    /// "Track", "Playlist" or "Album".
    pub media: &'static str,
    pub created_at: DateTime<FixedOffset>,
    /// Present for track items; shown in the info pane without a fetch.
    pub track: Option<Track>,
    /// Track urn, or the set's `tracks_uri` to fetch its tracks. Keys the info-pane fetch.
    pub key: String,
}

impl Activity {
    pub fn age(&self) -> String {
        let secs = (Utc::now() - self.created_at.with_timezone(&Utc)).num_seconds();
        format_age(secs.max(0) as u64)
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub title: String,
    pub artists: String,
    pub duration: String,
    pub duration_ms: u64,
    pub playback_count: String,
    pub artwork_url: String,
    pub access: String,
    pub track_urn: String,
}

impl Track {
    pub fn is_playable(&self) -> bool {
        self.access.is_empty() || self.access == "playable"
    }
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub title: String,
    pub track_count: String,
    pub duration: String,
    pub created_at: DateTime<FixedOffset>,
    pub tracks_uri: String,
    /// True if this playlist comes from `/me/playlists` (owned by the logged-in user).
    /// False if it comes from `/me/likes/playlists` (liked playlists).
    pub is_owned: bool,
}

#[derive(Debug, Clone)]
pub struct Album {
    pub title: String,
    pub artists: String,
    pub release_year: String,
    pub duration: String,
    pub track_count: String,
    pub tracks_uri: String,
}

#[derive(Debug, Clone)]
pub struct Artist {
    pub name: String,
    pub urn: String,
}
