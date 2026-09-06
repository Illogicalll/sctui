use std::sync::{Arc, Mutex};

use crate::auth::Token;

use super::super::utils::{access_token, parse_track, response_items};
use crate::api::Track;

/// Tracks SoundCloud considers related to `track_urn`, playable ones only.
/// Feeds stations and the end-of-queue autoplay.
pub async fn fetch_related_tracks(
    token: Arc<Mutex<Token>>,
    track_urn: String,
) -> anyhow::Result<Vec<Track>> {
    let access_token = access_token(&token);

    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("https://api.soundcloud.com/tracks/{track_urn}/related"))
        .query(&[
            ("access", "playable"),
            ("limit", "30"),
            ("linked_partitioning", "true"),
        ])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response_items(&resp).into_iter().map(|v| parse_track(&v)).collect())
}
