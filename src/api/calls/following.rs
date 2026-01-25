use reqwest::blocking::Client;
use reqwest;

use crate::auth::{Token, try_refresh_token};

use super::super::utils::{format_duration, format_playback_count, parse_next_href, parse_str, parse_u64};
use crate::api::{API, Artist, Track};
use std::sync::{Arc, Mutex};

impl API {
    pub fn get_following(&mut self) -> anyhow::Result<Vec<Artist>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        if self.following_next_href.is_none() && !self.first_following_page_fetched {
            self.first_following_page_fetched = true;
        } else if self.following_next_href.is_none() {
            return Ok(Vec::new());
        }

        let url = self.following_next_href.clone().unwrap_or_else(|| {
            "https://api.soundcloud.com/me/followings?limit=40&linked_partitioning=true"
                .to_string()
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

fn build_user_tracks_url(user_urn: &str, suffix: &str) -> String {
    let encoded_urn = user_urn.replace(':', "%3A");
    format!(
        "https://api.soundcloud.com/users/{}/{}?linked_partitioning=true&limit=200&access=playable,preview,blocked",
        encoded_urn, suffix
    )
}

pub async fn fetch_following_tracks(
    token: Arc<Mutex<Token>>,
    user_urn: String,
) -> anyhow::Result<Vec<Track>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = build_user_tracks_url(&user_urn, "tracks");

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

pub async fn fetch_following_liked_tracks(
    token: Arc<Mutex<Token>>,
    user_urn: String,
) -> anyhow::Result<Vec<Track>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = build_user_tracks_url(&user_urn, "likes/tracks");

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
