use reqwest::blocking::Client;

use crate::auth::try_refresh_token;

use super::super::utils::{format_duration, parse_next_href, parse_str, parse_u64};
use crate::api::{API, Album};

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
