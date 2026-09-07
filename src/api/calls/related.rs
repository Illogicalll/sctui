use std::sync::{Arc, Mutex};

use crate::auth::Token;

use super::super::utils::{access_token, parse_str, parse_track, response_items};
use crate::api::{Artist, Track};

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

/// Artists SoundCloud considers similar to `user_urn`. There is no public "artist station"
/// endpoint (that's a private api-v2 resource); this is the closest a public-API client gets,
/// used to build a station-like mix from the seed artist plus a few of these.
pub async fn fetch_related_users(
    token: Arc<Mutex<Token>>,
    user_urn: String,
) -> anyhow::Result<Vec<Artist>> {
    let access_token = access_token(&token);

    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("https://api.soundcloud.com/users/{user_urn}/related"))
        .query(&[("limit", "20"), ("linked_partitioning", "true")])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response_items(&resp)
        .into_iter()
        .map(|v| Artist { name: parse_str(&v, "username"), urn: parse_str(&v, "urn") })
        .collect())
}
