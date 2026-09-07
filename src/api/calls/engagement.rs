use reqwest::Method;

use super::super::utils::access_token;
use crate::auth::Token;
use std::sync::{Arc, Mutex};

/// Sends `method` to `https://api.soundcloud.com/{path}`; the like/follow endpoints have no body.
pub(crate) async fn engage(
    token: Arc<Mutex<Token>>,
    method: Method,
    path: String,
) -> anyhow::Result<()> {
    let access_token = access_token(&token);
    let url = format!("https://api.soundcloud.com/{}", path);

    reqwest::Client::new()
        .request(method, url)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
