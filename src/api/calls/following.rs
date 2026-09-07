use reqwest::blocking::Client;

use crate::auth::Token;

use super::super::utils::{access_token, parse_str, parse_track, response_items};
use crate::api::{API, Artist, Page, Track};
use std::sync::{Arc, Mutex};

impl API {
    pub fn get_following(&mut self) -> anyhow::Result<Vec<Artist>> {
        let access_token = access_token(&self.token);

        let Some(url) = self.following_page.take_url(
            "https://api.soundcloud.com/me/followings?limit=40&linked_partitioning=true",
        ) else {
            return Ok(Vec::new());
        };

        let resp: serde_json::Value = Client::new()
            .get(&url)
            .bearer_auth(&access_token)
            .send()?
            .error_for_status()?
            .json()?;

        self.following_page = Page::from_response(&resp);

        let mut following = Vec::new();

        if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
            for artist in collection {
                let name = parse_str(artist, "username");
                let urn = parse_str(artist, "urn");

                // Most feed reposters are followed users; saves a lookup each in the feed fetch.
                self.user_names.insert(urn.clone(), name.clone());
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

/// `suffix` is `"tracks"` for a user's uploads or `"likes/tracks"` for their likes.
pub(crate) async fn fetch_user_tracks(
    token: Arc<Mutex<Token>>,
    user_urn: String,
    suffix: &str,
) -> anyhow::Result<Vec<Track>> {
    let access_token = access_token(&token);
    let url = build_user_tracks_url(&user_urn, suffix);

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
