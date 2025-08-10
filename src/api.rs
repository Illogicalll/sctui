use crate::auth::Token;
use reqwest::blocking::Client;

pub fn get_me(token: &Token) -> anyhow::Result<String> {
    let resp = Client::new()
        .get("https://api.soundcloud.com/me")
        .bearer_auth(&token.access_token)
        .send()?
        .error_for_status()?
        .text()?;
    Ok(resp)
}
