use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

    /// Stamp `obtained_at` with now and persist to disk. The token comes back even if
    /// it could not be written: the refresh token it replaced is spent, so this is the
    /// only one that works. The failure is left for `take_save_warning`, and the next
    /// refresh saves again as every refresh does.
    pub(super) fn save(self) -> Self {
        let (token, failure) = self.save_to(&token_path());
        if let Some(e) = failure {
            *SAVE_WARNING.lock().unwrap_or_else(|e| e.into_inner()) =
                Some(format!("Could not save the login token (still signed in for now): {e}"));
        }
        token
    }

    fn save_to(mut self, path: &Path) -> (Self, Option<std::io::Error>) {
        self.obtained_at = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let written = serde_json::to_string_pretty(&self)
            .map_err(std::io::Error::other)
            .and_then(|json| write_private(path, json.as_bytes()));
        (self, written.err())
    }
}

/// Why the newest token is not on disk, until someone shows it to the user.
static SAVE_WARNING: Mutex<Option<String>> = Mutex::new(None);

/// The last token-save failure not yet reported, if any. Tokens are saved from
/// background threads while the TUI owns the terminal, so the UI collects it here.
pub fn take_save_warning() -> Option<String> {
    SAVE_WARNING.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// Write `contents` to `path` so that only this user can read it and a crash never
/// leaves half a file: a 0600 temp file in the same directory, renamed over the
/// target (which also replaces a file an older version left world-readable).
fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.exists()) {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".tmp{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    // Left behind by a crashed run with the same pid; `create_new` must not follow it.
    let _ = fs::remove_file(&tmp);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options
        .open(&tmp)
        .and_then(|mut file| {
            file.write_all(contents)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// `$XDG_CONFIG_HOME/sctui`, else `~/.config/sctui` (`%USERPROFILE%\.config\sctui` on Windows).
pub fn config_dir() -> PathBuf {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    config_home.join("sctui")
}

fn token_path() -> PathBuf {
    config_dir().join("token.json")
}

/// Stored token, possibly expired. Caller decides whether to refresh.
pub fn load_token() -> Option<Token> {
    let data = fs::read_to_string(token_path()).ok()?;
    serde_json::from_str(&data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #6: the token file is private to the user and replaced whole.
    #[test]
    fn saved_token_is_private_and_round_trips() {
        let dir = std::env::temp_dir().join(format!("sctui-token-test-{}-{:x}", std::process::id(), rand::random::<u64>()));
        let path = dir.join("nested").join("token.json");
        let token = || Token { access_token: "access".into(), refresh_token: "refresh".into(), obtained_at: 0 };

        let (saved, failure) = token().save_to(&path);
        assert!(failure.is_none());
        let read: Token = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!((read.access_token.as_str(), read.refresh_token.as_str()), ("access", "refresh"));
        assert_eq!(read.obtained_at, saved.obtained_at);
        assert!(saved.obtained_at > 0);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&path), 0o600);
            assert_eq!(mode(path.parent().unwrap()), 0o700, "a config dir we create is private too");
            // A file an older version wrote world-readable is tightened by the next save.
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
            assert!(token().save_to(&path).1.is_none());
            assert_eq!(mode(&path), 0o600);
        }
        let left: Vec<_> = fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["token.json"], "no temp file is left behind");
        let _ = fs::remove_dir_all(&dir);
    }

    /// S4: a token that cannot be written is still handed back, stamped, to be used.
    #[test]
    fn a_token_that_cannot_be_saved_is_kept() {
        let dir = std::env::temp_dir().join(format!("sctui-token-test-{}-{:x}", std::process::id(), rand::random::<u64>()));
        fs::create_dir(&dir).unwrap();
        // The config "directory" is a file, so nothing can be created in it.
        let blocker = dir.join("not-a-dir");
        fs::write(&blocker, b"").unwrap();
        let token = Token { access_token: "new-access".into(), refresh_token: "new-refresh".into(), obtained_at: 0 };

        let (kept, failure) = token.save_to(&blocker.join("token.json"));
        assert!(failure.is_some());
        assert_eq!((kept.access_token.as_str(), kept.refresh_token.as_str()), ("new-access", "new-refresh"));
        assert!(kept.obtained_at > 0 && !kept.is_expired());
        let _ = fs::remove_dir_all(&dir);
    }
}
