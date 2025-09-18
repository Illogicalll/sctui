use crate::auth::Token;
use chrono::{DateTime, FixedOffset, Utc};
use reqwest::blocking::Client;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Track {
    pub title: String,
    pub artists: String,
    pub duration: String,
    pub duration_ms: u64,
    pub playback_count: String,
    pub artwork_url: String,
    pub stream_url: String,
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

pub struct API {
    token: Arc<Mutex<Token>>,
    liked_tracks_next_href: Option<String>,
    first_liked_tracks_page_fetched: bool,
    my_playlists_next_href: Option<String>,
    my_first_playlist_page_fetched: bool,
    others_playlists_next_href: Option<String>,
    others_first_playlist_page_fetched: bool,
    albums_next_href: Option<String>,
    first_albums_page_fetched: bool,
    following_next_href: Option<String>,
    first_following_page_fetched: bool,
}

fn format_playback_count(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.2}K", n as f64 / 1_000.0),
        1_000_000..=999_999_999 => format!("{:.2}M", n as f64 / 1_000_000.0),
        1_000_000_000..=999_999_999_999 => format!("{:.2}B", n as f64 / 1_000_000_000.0),
        _ => format!("{:.2}T", n as f64 / 1_000_000_000_000.0),
    }
}

fn format_duration(duration_ms: u64) -> String {
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

fn parse_str(obj: &serde_json::Value, key: &str) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn parse_u64(obj: &serde_json::Value, key: &str) -> u64 {
    obj.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn parse_next_href(resp: &serde_json::Value) -> Option<String> {
    return resp
        .get("next_href")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
}

impl API {
    pub fn init(token: Arc<Mutex<Token>>) -> Self {
        Self {
            token,
            liked_tracks_next_href: None,
            first_liked_tracks_page_fetched: false,
            my_playlists_next_href: None,
            my_first_playlist_page_fetched: false,
            others_playlists_next_href: None,
            others_first_playlist_page_fetched: false,
            albums_next_href: None,
            first_albums_page_fetched: false,
            following_next_href: None,
            first_following_page_fetched: false,
        }
    }

    pub fn get_liked_tracks(&mut self) -> anyhow::Result<Vec<Track>> {
        let token_guard = self.token.lock().unwrap();

        if self.liked_tracks_next_href.is_none() && !self.first_liked_tracks_page_fetched {
            self.first_liked_tracks_page_fetched = true;
        } else if self.liked_tracks_next_href.is_none() {
            return Ok(Vec::new());
        }

        let url = self.liked_tracks_next_href.clone().unwrap_or_else(|| {
        "https://api.soundcloud.com/me/likes/tracks?limit=100&access=playable&linked_partitioning=true".to_string()
    });

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        drop(token_guard);

        self.liked_tracks_next_href = parse_next_href(&resp);

        let mut tracks = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for track in collection {
                let title = parse_str(track, "title");

                let artists = parse_str(track, "metadata_artist");
                let artists = if !artists.is_empty() {
                    artists
                } else {
                    parse_str(
                        track.get("user").unwrap_or(&serde_json::Value::Null),
                        "username",
                    )
                };
                let duration = format_duration(parse_u64(track, "duration"));
                let duration_ms = parse_u64(track, "duration");

                let playback_count = parse_u64(track, "playback_count");
                let playback_count = format_playback_count(playback_count);

                let artwork_url = parse_str(track, "artwork_url");

                let stream_url = parse_str(track, "stream_url");

                tracks.push(Track {
                    title,
                    artists,
                    duration,
                    duration_ms,
                    playback_count,
                    artwork_url,
                    stream_url,
                });
            }
        }

        Ok(tracks)
    }

    pub fn get_playlists(&mut self) -> anyhow::Result<Vec<Playlist>> {
        let token_guard = self.token.lock().unwrap();

        if self.my_playlists_next_href.is_none() && !self.my_first_playlist_page_fetched {
            self.my_first_playlist_page_fetched = true;
        } else if self.others_playlists_next_href.is_none()
            && !self.others_first_playlist_page_fetched
        {
            self.others_first_playlist_page_fetched = true;
        } else if self.my_playlists_next_href.is_none() && self.others_playlists_next_href.is_none()
        {
            return Ok(Vec::new());
        }

        let urls = vec![
        self.my_playlists_next_href.clone().unwrap_or_else(|| {
            "https://api.soundcloud.com/me/playlists?linked_partitioning=true&limit=40&show_tracks=false".to_string()
        }),
        self.others_playlists_next_href.clone().unwrap_or_else(|| {
            "https://api.soundcloud.com/me/likes/playlists?limit=40&linked_partitioning=true".to_string()
        }),
    ];

        let mut playlists = Vec::new();

        for (i, url) in urls.iter().enumerate() {
            let resp: serde_json::Value = Client::new()
                .get(url)
                .bearer_auth(&token_guard.access_token)
                .send()?
                .error_for_status()?
                .json()?;

            let next_href = parse_next_href(&resp);
            if i == 0 {
                self.my_playlists_next_href = next_href;
            } else {
                self.others_playlists_next_href = next_href;
            }

            if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
                for playlist in collection {
                    if i == 1
                        && playlist
                            .get("playlist_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            != "PLAYLIST"
                    {
                        continue;
                    }

                    let title = parse_str(&playlist, "title");
                    let track_count = parse_u64(&playlist, "track_count").to_string();
                    let duration = format_duration(parse_u64(&playlist, "duration"));
                    let created_at = DateTime::parse_from_str(
                        &parse_str(&playlist, "created_at"),
                        "%Y/%m/%d %H:%M:%S %z",
                    )
                    .unwrap_or_else(|_| {
                        Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap())
                    });
                    let tracks_uri = parse_str(&playlist, "tracks_uri");

                    playlists.push(Playlist {
                        title,
                        track_count,
                        duration,
                        created_at,
                        tracks_uri,
                    });
                }
            }
        }

        playlists.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(playlists)
    }

    pub fn get_albums(&mut self) -> anyhow::Result<Vec<Album>> {
        let token_guard = self.token.lock().unwrap();

        if self.albums_next_href.is_none() && !self.first_albums_page_fetched {
            self.first_albums_page_fetched = true;
        } else if self.albums_next_href.is_none() {
            return Ok(Vec::new());
        }

        let url = self.albums_next_href.clone().unwrap_or_else(|| {
            "https://api.soundcloud.com/me/likes/playlists?limit=40&linked_partitioning=true"
                .to_string()
        });

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        drop(token_guard);

        self.albums_next_href = parse_next_href(&resp);

        let mut albums = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for album in collection {
                if parse_str(album, "playlist_type") != "album" {
                    continue;
                }

                let title = parse_str(album, "title");
                let artists = parse_str(
                    album.get("user").unwrap_or(&serde_json::Value::Null),
                    "username",
                );
                let release_year = parse_u64(album, "release_year").to_string();
                let track_count = parse_u64(album, "track_count").to_string();
                let duration = format_duration(parse_u64(album, "duration"));
                let tracks_uri = parse_str(album, "tracks_uri");

                albums.push(Album {
                    title,
                    artists,
                    release_year,
                    track_count,
                    duration,
                    tracks_uri,
                });
            }
        }

        Ok(albums)
    }

    pub fn get_following(&mut self) -> anyhow::Result<Vec<Artist>> {
        let token_guard = self.token.lock().unwrap();

        if self.following_next_href.is_none() && !self.first_following_page_fetched {
            self.first_following_page_fetched = true;
        } else if self.following_next_href.is_none() {
            return Ok(Vec::new());
        }

        let url = self.following_next_href.clone().unwrap_or_else(|| {
            "https://api.soundcloud.com/me/followings?limit=40&linked_partitioning=true".to_string()
        });

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        drop(token_guard);

        self.following_next_href = parse_next_href(&resp);

        let mut following = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for artist in collection {
                let name = parse_str(artist, "username");
                let urn = parse_str(artist, "urn");

                following.push(Artist { name, urn });
            }
        }

        Ok(following)
    }
}
