use chrono::{DateTime, FixedOffset, Utc};
use reqwest::blocking::Client;
use reqwest;

use crate::auth::{Token, try_refresh_token};

use super::super::utils::{format_duration, format_playback_count, parse_next_href, parse_str, parse_u64};
use crate::api::{API, Playlist, Track};
use std::sync::{Arc, Mutex};

impl API {
    pub fn get_playlists(&mut self) -> anyhow::Result<Vec<Playlist>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        let should_fetch_my =
            !self.my_first_playlist_page_fetched || self.my_playlists_next_href.is_some();
        let should_fetch_others =
            !self.others_first_playlist_page_fetched || self.others_playlists_next_href.is_some();

        if !should_fetch_my && !should_fetch_others {
            return Ok(Vec::new());
        }

        let urls = vec![
            self.my_playlists_next_href.clone().unwrap_or_else(|| {
                "https://api.soundcloud.com/me/playlists?linked_partitioning=true&limit=40&show_tracks=false".to_string()
            }),
            self.others_playlists_next_href.clone().unwrap_or_else(|| {
                "https://api.soundcloud.com/me/likes/playlists?limit=40&linked_partitioning=true"
                    .to_string()
            }),
        ];

        let mut playlists = Vec::new();

        for (i, url) in urls.iter().enumerate() {
            if (i == 0 && !should_fetch_my) || (i == 1 && !should_fetch_others) {
                continue;
            }
            let resp: serde_json::Value = Client::new()
                .get(url)
                .bearer_auth(&token_guard.access_token)
                .send()?
                .error_for_status()?
                .json()?;

            let next_href = parse_next_href(&resp);
            if i == 0 {
                self.my_playlists_next_href = next_href;
                self.my_first_playlist_page_fetched = true;
            } else {
                self.others_playlists_next_href = next_href;
                self.others_first_playlist_page_fetched = true;
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

    pub fn get_playlist_tracks(&mut self, tracks_uri: &str) -> anyhow::Result<Vec<Track>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        let mut url = if tracks_uri.starts_with("http") {
            tracks_uri.to_string()
        } else {
            format!("https://api.soundcloud.com{}", tracks_uri)
        };
        if url.contains('?') {
            if !url.contains("linked_partitioning") {
                url.push_str("&linked_partitioning=true");
            }
            if !url.contains("limit=") {
                url.push_str("&limit=200");
            }
        } else {
            url.push_str("?linked_partitioning=true&limit=200");
        }

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        drop(token_guard);

        let items = if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            collection.clone()
        } else if let Some(array) = resp.as_array() {
            array.clone()
        } else {
            Vec::new()
        };

        let mut tracks = Vec::new();
        for track in items {
            let title = parse_str(&track, "title");

            let artists = parse_str(&track, "metadata_artist");
            let artists = if !artists.is_empty() {
                artists
            } else {
                parse_str(
                    track.get("user").unwrap_or(&serde_json::Value::Null),
                    "username",
                )
            };
            let duration = format_duration(parse_u64(&track, "duration"));
            let duration_ms = parse_u64(&track, "duration");

            let playback_count = parse_u64(&track, "playback_count");
            let playback_count = format_playback_count(playback_count);

            let artwork_url = parse_str(&track, "artwork_url");
            let stream_url = parse_str(&track, "stream_url");

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

        Ok(tracks)
    }
}

pub async fn fetch_playlist_tracks(
    token: Arc<Mutex<Token>>,
    tracks_uri: String,
) -> anyhow::Result<Vec<Track>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };

    let mut url = if tracks_uri.starts_with("http") {
        tracks_uri
    } else {
        format!("https://api.soundcloud.com{}", tracks_uri)
    };
    if url.contains('?') {
        if !url.contains("linked_partitioning") {
            url.push_str("&linked_partitioning=true");
        }
        if !url.contains("limit=") {
            url.push_str("&limit=200");
        }
    } else {
        url.push_str("?linked_partitioning=true&limit=200");
    }

    let resp: serde_json::Value = reqwest::Client::new()
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let items = if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
        collection.clone()
    } else if let Some(array) = resp.as_array() {
        array.clone()
    } else {
        Vec::new()
    };

    let mut tracks = Vec::new();
    for track in items {
        let title = parse_str(&track, "title");

        let artists = parse_str(&track, "metadata_artist");
        let artists = if !artists.is_empty() {
            artists
        } else {
            parse_str(
                track.get("user").unwrap_or(&serde_json::Value::Null),
                "username",
            )
        };
        let duration = format_duration(parse_u64(&track, "duration"));
        let duration_ms = parse_u64(&track, "duration");

        let playback_count = parse_u64(&track, "playback_count");
        let playback_count = format_playback_count(playback_count);

        let artwork_url = parse_str(&track, "artwork_url");
        let stream_url = parse_str(&track, "stream_url");

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

    Ok(tracks)
}
