use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const REFRESH_TIME: u64 = 2700;

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,

    #[serde(default)]
    pub obtained_at: u64,
}

impl Token {
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.obtained_at + REFRESH_TIME
    }
}

pub fn load_token() -> Option<Token> {
    let data = fs::read_to_string("token.json").ok()?;
    let token: Token = serde_json::from_str(&data).ok()?;
    if token.is_expired() {
        None
    } else {
        Some(token)
    }
}
