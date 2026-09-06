//! Writes against the user's own playlists. SoundCloud's `PUT /playlists/{urn}`
//! replaces the whole track list, so add/remove first read every track URN
//! (following pagination) and then write the edited list back.

use std::sync::{Arc, Mutex};

use anyhow::Context;
use serde_json::json;

use super::super::utils::{parse_next_href, parse_str, response_items};
use super::playlists::parse_playlist;
use crate::api::Playlist;
use crate::auth::{Token, try_refresh_token};

fn access_token(token: &Arc<Mutex<Token>>) -> String {
    let _ = try_refresh_token(token);
    token.lock().unwrap().access_token.clone()
}

fn playlist_url(playlist_id: u64) -> String {
    format!("https://api.soundcloud.com/playlists/soundcloud:playlists:{playlist_id}")
}

/// Every track URN in the playlist, in order, across all pages.
async fn fetch_all_track_urns(
    token: &Arc<Mutex<Token>>,
    tracks_uri: &str,
) -> anyhow::Result<Vec<String>> {
    let access = access_token(token);
    let mut url = if tracks_uri.starts_with("http") {
        tracks_uri.to_string()
    } else {
        format!("https://api.soundcloud.com{tracks_uri}")
    };
    url.push_str(if url.contains('?') { "&" } else { "?" });
    url.push_str("linked_partitioning=true&limit=200&access=playable,preview,blocked");

    let client = reqwest::Client::new();
    let mut urns = Vec::new();
    loop {
        let resp: serde_json::Value = client
            .get(&url)
            .bearer_auth(&access)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        urns.extend(response_items(&resp).iter().map(|v| parse_str(v, "urn")));
        match parse_next_href(&resp) {
            Some(next) => url = next,
            None => break,
        }
    }
    Ok(urns)
}

/// The list SoundCloud should hold after adding (`add == true`) or removing `urn`.
pub(crate) fn edited_urns(mut existing: Vec<String>, urn: &str, add: bool) -> Vec<String> {
    existing.retain(|u| u != urn);
    if add {
        existing.push(urn.to_string());
    }
    existing
}

async fn put_tracks(
    token: &Arc<Mutex<Token>>,
    playlist_id: u64,
    urns: &[String],
) -> anyhow::Result<()> {
    let body = json!({
        "playlist": { "tracks": urns.iter().map(|u| json!({ "urn": u })).collect::<Vec<_>>() }
    });
    reqwest::Client::new()
        .put(playlist_url(playlist_id))
        .bearer_auth(access_token(token))
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .context("updating playlist tracks")?;
    Ok(())
}

pub async fn add_track_to_playlist(
    token: Arc<Mutex<Token>>,
    playlist_id: u64,
    tracks_uri: String,
    track_urn: String,
) -> anyhow::Result<()> {
    let urns = fetch_all_track_urns(&token, &tracks_uri).await?;
    put_tracks(&token, playlist_id, &edited_urns(urns, &track_urn, true)).await
}

pub async fn remove_track_from_playlist(
    token: Arc<Mutex<Token>>,
    playlist_id: u64,
    tracks_uri: String,
    track_urn: String,
) -> anyhow::Result<()> {
    let urns = fetch_all_track_urns(&token, &tracks_uri).await?;
    put_tracks(&token, playlist_id, &edited_urns(urns, &track_urn, false)).await
}

/// Creates a private playlist, empty or holding `track_urn`, and returns it as
/// the library represents playlists.
pub async fn create_playlist(
    token: Arc<Mutex<Token>>,
    title: String,
    track_urn: Option<String>,
) -> anyhow::Result<Playlist> {
    let tracks: Vec<serde_json::Value> = track_urn.into_iter().map(|u| json!({ "urn": u })).collect();
    let body = json!({
        "playlist": { "title": title, "sharing": "private", "tracks": tracks }
    });
    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://api.soundcloud.com/playlists")
        .bearer_auth(access_token(&token))
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .context("creating playlist")?
        .json()
        .await?;
    Ok(parse_playlist(&resp, true))
}

pub async fn delete_playlist(token: Arc<Mutex<Token>>, playlist_id: u64) -> anyhow::Result<()> {
    reqwest::Client::new()
        .delete(playlist_url(playlist_id))
        .bearer_auth(access_token(&token))
        .send()
        .await?
        .error_for_status()
        .context("deleting playlist")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::edited_urns;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn add_appends_once_and_remove_drops_all_copies() {
        assert_eq!(edited_urns(v(&["a", "b"]), "c", true), v(&["a", "b", "c"]));
        assert_eq!(edited_urns(v(&["a", "c", "b"]), "c", true), v(&["a", "b", "c"]));
        assert_eq!(edited_urns(v(&["a", "c", "b", "c"]), "c", false), v(&["a", "b"]));
        assert_eq!(edited_urns(v(&[]), "c", false), v(&[]));
    }
}
