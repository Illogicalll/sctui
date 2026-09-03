use reqwest::blocking::Client;

use crate::auth::try_refresh_token;

use super::super::utils::parse_track;
use crate::api::{API, Page, Track};

impl API {
    pub fn get_liked_tracks(&mut self) -> anyhow::Result<Vec<Track>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        let Some(url) = self.liked_tracks_page.take_url(
            "https://api.soundcloud.com/me/likes/tracks?limit=40&access=playable,preview,blocked&linked_partitioning=true",
        ) else {
            return Ok(Vec::new());
        };

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&token_guard.access_token)
            .send()?
            .error_for_status()?
            .json()?;

        drop(token_guard);

        self.liked_tracks_page = Page::from_response(&resp);

        let mut tracks = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for track in collection {
                tracks.push(parse_track(track));
            }
        }

        Ok(tracks)
    }
}
