use anyhow::{Result, anyhow};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{Rng, distributions::Alphanumeric};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tiny_http::{Response, Server};
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,

    #[serde(default)]
    pub obtained_at: u64,
}

static CODE_VERIFIER_LEN: usize = 64;
static STATE_LEN: usize = 56;
static REFRESH_TIME: u64 = 2700;

// give a token the ability to be 'expired'
impl Token {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.obtained_at + REFRESH_TIME
    }
}

// called by main.rs on program start
pub fn load_token() -> Option<Token> {
    let data = fs::read_to_string("token.json").ok()?;
    let token: Token = serde_json::from_str(&data).ok()?;
    if token.is_expired() {
        None
    } else {
        Some(token)
    }
}

// helper functions for OAuth
fn generate_code_verifier() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(CODE_VERIFIER_LEN)
        .map(char::from)
        .collect()
}

fn generate_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn generate_state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(STATE_LEN)
        .map(char::from)
        .collect()
}

// soundcloud OAuth 2.1 user authentication
pub fn authenticate() -> Result<Token> {
    // define auth url
    dotenvy::dotenv().ok();
    let client_id = std::env::var("SOUNDCLOUD_CLIENT_ID")?;
    let client_secret = std::env::var("SOUNDCLOUD_CLIENT_SECRET")?;
    let redirect_uri = "http://127.0.0.1:8080/callback";
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);
    let state = generate_state();
    let auth_url = format!(
        "https://secure.soundcloud.com/authorize?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&code_challenge={code_challenge}&code_challenge_method=S256&state={state}",
        client_id = client_id,
        redirect_uri = redirect_uri,
        code_challenge = code_challenge,
        state = state
    );
    webbrowser::open(&auth_url)?;

    // server to receive callback with auth code
    let server = Server::http("127.0.0.1:8080").map_err(|e| {
        anyhow!(
            "Failed to start server, check port 8080 is open and/or free: {}",
            e
        )
    })?;

    // parse callback
    let received_data = Arc::new(Mutex::new(None));
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
                {
                    let mut data = received_data.lock().unwrap();
                    *data = Some(code.clone());
                }
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

    // exchange code and code_verifier for token
    let code = {
        let data = received_data.lock().unwrap();
        data.clone()
            .ok_or_else(|| anyhow!("No authorization code received"))?
    };
    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("code_verifier", &code_verifier),
    ];
    let mut resp = reqwest::blocking::Client::new()
        .post("https://secure.soundcloud.com/oauth/token")
        .form(&params)
        .send()?
        .error_for_status()?
        .json::<Token>()?;

    // set time token was obtained and save it
    resp.obtained_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    fs::write("token.json", serde_json::to_string_pretty(&resp)?)?;

    Ok(resp)
}

// soundcloud OAuth 2.1 token refresh
pub fn refresh_token(old_token: &Token) -> Result<Token> {
    // define refresh url
    dotenvy::dotenv().ok();
    let client_id = std::env::var("SOUNDCLOUD_CLIENT_ID")?;
    let client_secret = std::env::var("SOUNDCLOUD_CLIENT_SECRET")?;
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
        ("refresh_token", &old_token.refresh_token),
    ];
    let mut resp = reqwest::blocking::Client::new()
        .post("https://secure.soundcloud.com/oauth/token")
        .header("accept", "application/json; charset=utf-8")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()?
        .error_for_status()?
        .json::<Token>()?;

    // set time token was obtained and save it
    resp.obtained_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    fs::write("token.json", serde_json::to_string_pretty(&resp)?)?;

    Ok(resp)
}

// make sure the token never expires
pub fn start_auto_refresh(token: Arc<Mutex<Token>>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(REFRESH_TIME));
            let mut token_guard = token.lock().unwrap();
            if let Ok(new_token) = refresh_token(&*token_guard) {
                *token_guard = new_token;
            }
        }
    });
}
