use reqwest::blocking::Client;
use reqwest;

use crate::auth::{Token, try_refresh_token};

use super::super::utils::{parse_str, parse_track, response_items};
use crate::api::{API, Artist, Page, Track};
use std::sync::{Arc, Mutex};

impl API {
    pub fn get_following(&mut self) -> anyhow::Result<Vec<Artist>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        let Some(url) = self.following_page.take_url(
            "https://api.soundcloud.com/me/followings?limit=40&linked_partitioning=true",
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

        self.following_page = Page::from_response(&resp);

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

async fn fetch_user_tracks(
    token: Arc<Mutex<Token>>,
    user_urn: String,
    suffix: &str,
) -> anyhow::Result<Vec<Track>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
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

pub async fn fetch_following_tracks(
    token: Arc<Mutex<Token>>,
    user_urn: String,
) -> anyhow::Result<Vec<Track>> {
    fetch_user_tracks(token, user_urn, "tracks").await
}

pub async fn fetch_following_liked_tracks(
    token: Arc<Mutex<Token>>,
    user_urn: String,
) -> anyhow::Result<Vec<Track>> {
    fetch_user_tracks(token, user_urn, "likes/tracks").await
}
