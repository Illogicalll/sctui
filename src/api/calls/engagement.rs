use reqwest;

use crate::auth::{Token, try_refresh_token};
use std::sync::{Arc, Mutex};

pub async fn like_track(token: Arc<Mutex<Token>>, track_id: u64) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/likes/tracks/{}", track_id);

    reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub async fn unlike_track(token: Arc<Mutex<Token>>, track_id: u64) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/likes/tracks/{}", track_id);

    reqwest::Client::new()
        .delete(url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub async fn like_playlist(token: Arc<Mutex<Token>>, playlist_id: u64) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/likes/playlists/{}", playlist_id);

    reqwest::Client::new()
        .post(url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub async fn unlike_playlist(token: Arc<Mutex<Token>>, playlist_id: u64) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/likes/playlists/{}", playlist_id);

    reqwest::Client::new()
        .delete(url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub async fn follow_user(token: Arc<Mutex<Token>>, user_id: u64) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/me/followings/{}", user_id);

    reqwest::Client::new()
        .put(url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

pub async fn unfollow_user(token: Arc<Mutex<Token>>, user_id: u64) -> anyhow::Result<()> {
    let _ = try_refresh_token(&token);

    let access_token = { token.lock().unwrap().access_token.clone() };
    let url = format!("https://api.soundcloud.com/me/followings/{}", user_id);

    reqwest::Client::new()
        .delete(url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

