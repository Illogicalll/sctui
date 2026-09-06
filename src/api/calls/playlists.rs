use reqwest::blocking::Client;

use crate::auth::Token;

use super::super::utils::{
    access_token, format_duration, parse_datetime, parse_str, parse_track, parse_u64, response_items,
};
use crate::api::{API, Page, Playlist, Track};
use std::sync::{Arc, Mutex};

/// A playlist object from any `/playlists` endpoint, as the library shows it.
pub(crate) fn parse_playlist(playlist: &serde_json::Value, is_owned: bool) -> Playlist {
    Playlist {
        title: parse_str(playlist, "title"),
        track_count: parse_u64(playlist, "track_count").to_string(),
        duration: format_duration(parse_u64(playlist, "duration")),
        created_at: parse_datetime(playlist, "created_at"),
        tracks_uri: parse_str(playlist, "tracks_uri"),
        is_owned,
    }
}

impl API {
    pub fn get_playlists(&mut self) -> anyhow::Result<Vec<Playlist>> {
        let access_token = access_token(&self.token);

        let urls = [
            self.my_playlists_page.url(
                "https://api.soundcloud.com/me/playlists?linked_partitioning=true&limit=40&show_tracks=false",
            ),
            self.others_playlists_page.url(
                "https://api.soundcloud.com/me/likes/playlists?limit=40&linked_partitioning=true",
            ),
        ];

        if urls.iter().all(Option::is_none) {
            return Ok(Vec::new());
        }

        let mut playlists = Vec::new();

        for (i, url) in urls.iter().enumerate() {
            let Some(url) = url else { continue };
            let resp: serde_json::Value = Client::new()
                .get(url)
                .bearer_auth(&access_token)
                .send()?
                .error_for_status()?
                .json()?;

            let page = Page::from_response(&resp);
            if i == 0 {
                self.my_playlists_page = page;
            } else {
                self.others_playlists_page = page;
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

                    playlists.push(parse_playlist(playlist, i == 0));
                }
            }
        }

        playlists.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(playlists)
    }
}

pub async fn fetch_playlist_tracks(
    token: Arc<Mutex<Token>>,
    tracks_uri: String,
) -> anyhow::Result<Vec<Track>> {
    let access_token = access_token(&token);

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

    Ok(response_items(&resp).into_iter().map(|v| parse_track(&v)).collect())
}
