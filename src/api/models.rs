use chrono::{DateTime, FixedOffset};

#[derive(Debug, Clone)]
pub struct Track {
    pub title: String,
    pub artists: String,
    pub duration: String,
    pub duration_ms: u64,
    pub playback_count: String,
    pub artwork_url: String,
    pub stream_url: String,
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
