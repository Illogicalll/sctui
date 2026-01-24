use reqwest::blocking::Client;

use crate::auth::try_refresh_token;

use super::super::utils::{parse_next_href, parse_str};
use crate::api::{API, Artist};

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
