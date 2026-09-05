//! Startup update check and in-place self-update from GitHub Releases.
//!
//! Mirrors what `install.sh` / `install.ps1` do: fetch the archive for this
//! platform, verify its sha256, extract with the system `tar`, swap the binary.

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, bail};
use sha2::{Digest, Sha256};

const REPO: &str = "Illogicalll/sctui";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Check GitHub for a newer release and offer to install it. Never fails the
/// launch: the check itself is silent on error, an update failure is printed
/// and the current binary carries on.
pub fn maybe_self_update() {
    cleanup_old_binary();

    let Some(tag) = latest_tag() else { return };
    let (Some(latest), Some(current)) = (parse_version(&tag), parse_version(CURRENT)) else {
        return;
    };
    if latest <= current || skipped_tag().as_deref() == Some(tag.as_str()) {
        return;
    }
    if !io::stdin().is_terminal() {
        return;
    }

    print!("sctui {tag} is available (you have v{CURRENT}). Update? [y]es / [n]o / [s]kip this version: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return;
    }
    match line.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => match install(&tag) {
            Ok(exe) => relaunch(&exe),
            Err(e) => eprintln!(
                "update failed: {e:#}\nlaunching the current version; update manually with the installer in the README."
            ),
        },
        "s" | "skip" => {
            let _ = fs::write(skip_path(), &tag);
        }
        _ => {}
    }
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("sctui/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(4))
        .build()
        .expect("reqwest client")
}

fn get(url: &str, timeout: Duration) -> anyhow::Result<Vec<u8>> {
    let resp = client()
        .get(url)
        .timeout(timeout)
        .send()
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url}"))?;
    Ok(resp.bytes()?.to_vec())
}

fn latest_tag() -> Option<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = get(&url, Duration::from_secs(4)).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&body).ok()?;
    json.get("tag_name")?.as_str().map(str::to_owned)
}

/// `v1.2.3` or `1.2.3` → (1, 2, 3). Anything else (pre-releases included) → None.
fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut parts = s.trim().trim_start_matches('v').split('.');
    let v = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(v)
}

/// Rust target triple for the running platform, matching the release asset names.
fn target() -> Option<&'static str> {
    use std::env::consts::{ARCH, OS};
    Some(match (OS, ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        _ => return None,
    })
}

fn skip_path() -> PathBuf {
    crate::auth::config_dir().join("skip-version")
}

fn skipped_tag() -> Option<String> {
    fs::read_to_string(skip_path())
        .ok()
        .map(|s| s.trim().to_owned())
}

/// Download, verify, extract and swap in the release `tag`. Returns the path
/// of the (now replaced) executable.
fn install(tag: &str) -> anyhow::Result<PathBuf> {
    let target = target().context("no prebuilt release for this platform")?;
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let name = format!("sctui-{target}");
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");

    let tmp = std::env::temp_dir().join(format!("sctui-update-{}", std::process::id()));
    fs::create_dir_all(&tmp)?;
    let archive = tmp.join(format!("{name}.{ext}"));

    eprintln!("downloading {name}.{ext}...");
    let bytes = get(&format!("{base}/{name}.{ext}"), Duration::from_secs(120))?;
    let sums = get(&format!("{base}/{name}.{ext}.sha256"), Duration::from_secs(10))?;
    verify_sha256(&bytes, &String::from_utf8_lossy(&sums))?;
    fs::write(&archive, &bytes)?;

    // System tar extracts .tar.gz everywhere and .zip on Windows 10+.
    let status = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(&tmp)
        .status()
        .context("running tar")?;
    if !status.success() {
        bail!("tar exited with {status}");
    }

    let bin_name = if cfg!(windows) { "sctui.exe" } else { "sctui" };
    let new_bin = tmp.join(&name).join(bin_name);
    let exe = std::env::current_exe()?;
    replace_binary(&new_bin, &exe)
        .with_context(|| format!("replacing {}", exe.display()))?;

    let _ = fs::remove_dir_all(&tmp);
    eprintln!("updated to {tag}");
    Ok(exe)
}

fn verify_sha256(bytes: &[u8], sums_file: &str) -> anyhow::Result<()> {
    let expected = sums_file
        .split_whitespace()
        .next()
        .context("empty checksum file")?
        .to_ascii_lowercase();
    let actual = format!("{:x}", Sha256::digest(bytes));
    if expected != actual {
        bail!("checksum mismatch (expected {expected}, got {actual})");
    }
    Ok(())
}

/// Stage next to the target (same filesystem), then rename over it. Windows
/// won't let a running exe be overwritten, so the old one is renamed aside
/// first and removed on the next launch by `cleanup_old_binary`.
fn replace_binary(new_bin: &Path, exe: &Path) -> anyhow::Result<()> {
    let staging = exe.with_extension("new");
    fs::copy(new_bin, &staging)?;
    fs::set_permissions(&staging, fs::metadata(exe)?.permissions())?;
    if cfg!(windows) {
        let old = exe.with_extension("old.exe");
        let _ = fs::remove_file(&old);
        fs::rename(exe, &old)?;
    }
    fs::rename(&staging, exe)?;
    Ok(())
}

fn cleanup_old_binary() {
    if cfg!(windows) && let Ok(exe) = std::env::current_exe() {
        let _ = fs::remove_file(exe.with_extension("old.exe"));
    }
}

/// Run the freshly installed binary with the same arguments and exit with its
/// status. Falls through (continuing on the old code) if it can't be spawned.
fn relaunch(exe: &Path) {
    match Command::new(exe).args(std::env::args_os().skip(1)).status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(0)),
        Err(e) => eprintln!("could not relaunch {}: {e}", exe.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing_and_ordering() {
        assert_eq!(parse_version("v0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v0.2.0-beta"), None);
        assert_eq!(parse_version("v1.2"), None);
        assert_eq!(parse_version("v1.2.3.4"), None);
        assert!(parse_version("v0.10.0") > parse_version("v0.9.9"));
        assert!(parse_version(CURRENT).is_some(), "Cargo version must be plain x.y.z");
    }

    #[test]
    fn this_platform_has_a_release_target() {
        assert!(target().is_some());
    }

    #[test]
    fn sha256_verification() {
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(verify_sha256(b"abc", &format!("{abc}  sctui.tar.gz")).is_ok());
        assert!(verify_sha256(b"abc", &abc.to_uppercase()).is_ok());
        assert!(verify_sha256(b"abd", abc).is_err());
        assert!(verify_sha256(b"abc", "").is_err());
    }
}
