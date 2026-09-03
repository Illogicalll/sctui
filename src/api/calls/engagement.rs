use reqwest::Method;

use crate::auth::{Token, try_refresh_token};
use std::sync::{Arc, Mutex};

async fn engage(token: Arc<Mutex<Token>>, method: Method, path: String) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/{}", path);

    reqwest::Client::new()
        .request(method, url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub async fn like_track(token: Arc<Mutex<Token>>, track_id: u64) -> anyhow::Result<()> {
    engage(token, Method::POST, format!("likes/tracks/{}", track_id)).await
}

pub async fn unlike_track(token: Arc<Mutex<Token>>, track_id: u64) -> anyhow::Result<()> {
    engage(token, Method::DELETE, format!("likes/tracks/{}", track_id)).await
}

pub async fn like_playlist(token: Arc<Mutex<Token>>, playlist_id: u64) -> anyhow::Result<()> {
    engage(token, Method::POST, format!("likes/playlists/{}", playlist_id)).await
}

pub async fn unlike_playlist(token: Arc<Mutex<Token>>, playlist_id: u64) -> anyhow::Result<()> {
    engage(token, Method::DELETE, format!("likes/playlists/{}", playlist_id)).await
}

pub async fn follow_user(token: Arc<Mutex<Token>>, user_id: u64) -> anyhow::Result<()> {
    engage(token, Method::PUT, format!("me/followings/{}", user_id)).await
}

pub async fn unfollow_user(token: Arc<Mutex<Token>>, user_id: u64) -> anyhow::Result<()> {
    engage(token, Method::DELETE, format!("me/followings/{}", user_id)).await
}

