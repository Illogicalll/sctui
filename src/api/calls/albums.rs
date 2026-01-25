use reqwest::blocking::Client;
use reqwest;

use crate::auth::{Token, try_refresh_token};

use super::super::utils::{format_duration, format_playback_count, parse_next_href, parse_str, parse_u64};
use crate::api::{API, Album, Track};
use std::sync::{Arc, Mutex};

impl API {
    pub fn get_albums(&mut self) -> anyhow::Result<Vec<Album>> {
        let _ = try_refresh_token(&self.token);

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
}

pub async fn fetch_album_tracks(
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
        if !url.contains("access=") {
            url.push_str("&access=playable,preview,blocked");
        }
    } else {
        url.push_str("?linked_partitioning=true&limit=200&access=playable,preview,blocked");
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
        let access = parse_str(&track, "access");

        tracks.push(Track {
            title,
            artists,
            duration,
            duration_ms,
            playback_count,
            artwork_url,
            stream_url,
            access,
        });
    }

    Ok(tracks)
}
