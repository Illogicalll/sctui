use chrono::{DateTime, FixedOffset, Utc};
use reqwest;

use crate::auth::{Token, try_refresh_token};

use super::super::utils::{format_duration, parse_str, parse_track, parse_u64, response_items};
use crate::api::{Album, Artist, Playlist, Track};
use std::sync::{Arc, Mutex};

fn parse_created_at(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value)
        .or_else(|_| DateTime::parse_from_str(value, "%Y/%m/%d %H:%M:%S %z"))
        .unwrap_or_else(|_| Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()))
}

pub async fn fetch_search_tracks(
    token: Arc<Mutex<Token>>,
    query: String,
) -> anyhow::Result<Vec<Track>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };

    let resp: serde_json::Value = reqwest::Client::new()
        .get("https://api.soundcloud.com/tracks")
        .query(&[
            ("q", query.as_str()),
            ("linked_partitioning", "true"),
            ("limit", "50"),
            ("access", "playable,preview,blocked"),
        ])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(response_items(&resp).into_iter().map(|v| parse_track(&v)).collect())
}

pub async fn fetch_search_albums(
    token: Arc<Mutex<Token>>,
    query: String,
) -> anyhow::Result<Vec<Album>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };

    let resp: serde_json::Value = reqwest::Client::new()
        .get("https://api.soundcloud.com/playlists")
        .query(&[
            ("q", query.as_str()),
            ("linked_partitioning", "true"),
            ("limit", "50"),
            ("show_tracks", "false"),
        ])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut albums = Vec::new();
    for playlist in response_items(&resp) {
        let playlist_type = parse_str(&playlist, "playlist_type");
        if !playlist_type.eq_ignore_ascii_case("album") {
            continue;
        }

        let title = parse_str(&playlist, "title");
        let artists = parse_str(
            playlist.get("user").unwrap_or(&serde_json::Value::Null),
            "username",
        );
        let release_year = parse_u64(&playlist, "release_year").to_string();
        let track_count = parse_u64(&playlist, "track_count").to_string();
        let duration = format_duration(parse_u64(&playlist, "duration"));
        let tracks_uri = parse_str(&playlist, "tracks_uri");

        albums.push(Album {
            title,
            artists,
            release_year,
            duration,
            track_count,
            tracks_uri,
        });
    }

    Ok(albums)
}

pub async fn fetch_search_playlists(
    token: Arc<Mutex<Token>>,
    query: String,
) -> anyhow::Result<Vec<Playlist>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };

    let resp: serde_json::Value = reqwest::Client::new()
        .get("https://api.soundcloud.com/playlists")
        .query(&[
            ("q", query.as_str()),
            ("linked_partitioning", "true"),
            ("limit", "50"),
            ("show_tracks", "false"),
        ])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut playlists = Vec::new();
    for playlist in response_items(&resp) {
        let playlist_type = parse_str(&playlist, "playlist_type");
        if playlist_type.eq_ignore_ascii_case("album") {
            continue;
        }

        let title = parse_str(&playlist, "title");
        let track_count = parse_u64(&playlist, "track_count").to_string();
        let duration = format_duration(parse_u64(&playlist, "duration"));
        let created_at = parse_created_at(&parse_str(&playlist, "created_at"));
        let tracks_uri = parse_str(&playlist, "tracks_uri");

        playlists.push(Playlist {
            title,
            track_count,
            duration,
            created_at,
            tracks_uri,
            is_owned: false,
        });
    }

    playlists.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(playlists)
}

pub async fn fetch_search_people(
    token: Arc<Mutex<Token>>,
    query: String,
) -> anyhow::Result<Vec<Artist>> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };

    let resp: serde_json::Value = reqwest::Client::new()
        .get("https://api.soundcloud.com/users")
        .query(&[
            ("q", query.as_str()),
            ("linked_partitioning", "true"),
            ("limit", "50"),
        ])
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut people = Vec::new();
    for user in response_items(&resp) {
        let name = parse_str(&user, "username");
        let urn = parse_str(&user, "urn");
        if name.is_empty() || urn.is_empty() {
            continue;
        }
        people.push(Artist { name, urn });
    }

    Ok(people)
}

