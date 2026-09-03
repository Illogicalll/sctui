use reqwest::Method;

use crate::auth::{Token, try_refresh_token};
use std::sync::{Arc, Mutex};

/// Sends `method` to `https://api.soundcloud.com/{path}`; the like/follow endpoints have no body.
pub(crate) async fn engage(
    token: Arc<Mutex<Token>>,
    method: Method,
    path: String,
) -> anyhow::Result<()> {
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
