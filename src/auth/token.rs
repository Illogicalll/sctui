use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
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

    /// Stamp `obtained_at` with now and persist to disk.
    pub(super) fn save(mut self) -> anyhow::Result<Self> {
        self.obtained_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let path = token_path();
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, serde_json::to_string_pretty(&self)?)?;
        Ok(self)
    }
}

/// `$XDG_CONFIG_HOME/sctui/token.json`, else `~/.config/sctui/token.json`.
fn token_path() -> PathBuf {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    config_home.join("sctui").join("token.json")
}

/// Stored token, possibly expired. Caller decides whether to refresh.
pub fn load_token() -> Option<Token> {
    let data = fs::read_to_string(token_path()).ok()?;
    serde_json::from_str(&data).ok()
}
