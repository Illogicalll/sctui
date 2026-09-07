use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{Rng, distributions::Alphanumeric};
use sha2::{Digest, Sha256};
use tiny_http::{Response, Server};
use reqwest::Url;

use super::relay_post;
use super::token::Token;

/// Public OAuth client identifier. It names the app, not any account, and is useless without the
/// client secret, which only the auth relay holds. It is visible in every user's browser URL bar.
const CLIENT_ID: &str = "XTV3fHQiqa6P4CVB5JH9Y452wVTGFyvu";
/// Must match `REDIRECT_URI` in `worker/src/index.js`.
const REDIRECT_URI: &str = "http://127.0.0.1:8080/callback";

fn random_alnum(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}

fn generate_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

pub fn authenticate() -> Result<Token> {
    let code_verifier = random_alnum(64);
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = random_alnum(56);
    let auth_url = format!(
        "https://secure.soundcloud.com/authorize?client_id={CLIENT_ID}&redirect_uri={REDIRECT_URI}&response_type=code&code_challenge={code_challenge}&code_challenge_method=S256&state={state}"
    );
    webbrowser::open(&auth_url)?;

    let server = Server::http("127.0.0.1:8080").map_err(|e| {
        anyhow!(
            "Failed to start server, check port 8080 is open and/or free: {}",
            e
        )
    })?;

    let mut received_code = None;
    for request in server.incoming_requests() {
        let url_str = format!("http://127.0.0.1:8080{}", request.url());
        let parsed = Url::parse(&url_str)?;
        if parsed.path() == "/callback" {
            let query_pairs = parsed.query_pairs();
            let code_opt = query_pairs
                .clone()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.into_owned());
            let state_opt = query_pairs
                .clone()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.into_owned());
            if let (Some(code), Some(returned_state)) = (code_opt, state_opt) {
                if returned_state != state {
                    let response =
                        Response::from_string("Invalid state parameter").with_status_code(400);
                    request.respond(response)?;
                    return Err(anyhow!("CSRF state mismatch"));
                }
                received_code = Some(code);
                let response =
                    Response::from_string("Authentication successful! You can close this window.");
                request.respond(response)?;
                break;
            } else {
                let response = Response::from_string("Missing code or state").with_status_code(400);
                request.respond(response)?;
                return Err(anyhow!("Missing code or state in callback"));
            }
        } else {
            let response = Response::from_string("Not found").with_status_code(404);
            request.respond(response)?;
        }
    }

    let code = received_code.ok_or_else(|| anyhow!("No authorization code received"))?;
    // The relay adds client_id, client_secret, grant_type and redirect_uri.
    relay_post("/token", &[("code", &code), ("code_verifier", &code_verifier)])
}
