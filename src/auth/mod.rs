mod oauth;
mod refresh;
mod token;

pub use oauth::authenticate;
pub use refresh::{refresh_token, start_auto_refresh, try_refresh_token};
pub use token::{Token, config_dir, load_token};

/// Cloudflare Worker that holds the SoundCloud client secret (source in `worker/`). It adds the
/// secret to the two token requests and forwards them to SoundCloud; nothing else goes through it.
/// Override at build time with `SCTUI_AUTH_RELAY` to test against a local `wrangler dev`.
const AUTH_RELAY: &str = match option_env!("SCTUI_AUTH_RELAY") {
    Some(url) => url,
    None => "https://sctui.w-murphy.com",
};

/// POST form `params` to the relay and persist the returned token. Sends an explicit User-Agent
/// because the zone's Cloudflare WAF rejects some generic client signatures (error 1010).
fn relay_post(path: &str, params: &[(&str, &str)]) -> anyhow::Result<Token> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("sctui/", env!("CARGO_PKG_VERSION")))
        .build()?
        .post(format!("{AUTH_RELAY}{path}"))
        .form(params)
        .send()?
        .error_for_status()?
        .json::<Token>()?
        .save()
}
