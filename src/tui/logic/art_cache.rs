//! Where cover art lives on disk, and for how long. The OS media panel needs the
//! image as a local file (see `media.rs`), but SoundCloud's API terms allow caching
//! for the session only, so each run keeps its art in a directory of its own and
//! deletes it on the way out.

use std::fs;
use std::path::{Path, PathBuf};

/// This run's art directory under `root` (the temp dir): `sctui-<pid>`.
pub(crate) fn run_dir(root: &Path) -> PathBuf {
    root.join(format!("sctui-{}", std::process::id()))
}

/// This run's art directory, removed again when dropped: on quit, on an error
/// return and when a panic unwinds. Nothing here is fatal; without the directory
/// the art writes fail and the media panel goes without a cover.
pub(crate) struct RunDir(PathBuf);

impl RunDir {
    pub(crate) fn create(root: &Path) -> Self {
        let dir = run_dir(root);
        // Whatever is already there was left by a dead process that had this pid.
        remove(&dir);
        let _ = crate::update::create_private_dir(&dir);
        RunDir(dir)
    }
}

impl Drop for RunDir {
    fn drop(&mut self) {
        remove(&self.0);
    }
}

/// Delete what earlier runs left under `root`: the flat `sctui-art-*.img` files from
/// before per-run directories, and `sctui-<pid>` directories whose process is gone.
/// Only those two name shapes, directly inside `root`, are ever touched.
pub(crate) fn sweep(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix("sctui-")) else { continue };
        if rest.strip_prefix("art-").is_some_and(|r| r.ends_with(".img")) {
            // Unlinks a symlink rather than its target, and fails on a directory.
            let _ = fs::remove_file(entry.path());
        } else if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
            // A reused pid makes a dead run's directory look alive; a later launch gets it.
            if rest.parse().is_ok_and(is_dead) {
                remove(&entry.path());
            }
        }
    }
}

/// Remove the directory at `path` and everything in it. A symlink is never
/// followed: one at `path` is left alone, one inside is unlinked.
fn remove(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
        let _ = fs::remove_dir_all(path);
    }
}

/// Whether no process has `pid`, as far as the OS will say.
#[cfg(unix)]
fn is_dead(pid: i32) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const ESRCH: i32 = 3;
    // Signal 0 only checks that the process exists. Pid 0 would mean our own process
    // group, which is alive; "exists but is not ours" (EPERM) is alive too.
    pid > 0 && unsafe { kill(pid, 0) } == -1 && std::io::Error::last_os_error().raw_os_error() == Some(ESRCH)
}

/// No dependency-free liveness check here, so no directory is ever proven dead.
#[cfg(not(unix))]
fn is_dead(_pid: i32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("sctui-art-test-{tag}-{}-{:x}", std::process::id(), rand::random::<u64>()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// A pid that no process has: a child that has exited and been reaped.
    #[cfg(unix)]
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        child.wait().unwrap();
        child.id()
    }

    /// #56: the sweep deletes legacy art files and dead runs' directories, and nothing else.
    #[test]
    fn sweep_removes_only_stale_art() {
        let root = scratch("sweep");
        let legacy = root.join("sctui-art-0123456789abcdef.img");
        fs::write(&legacy, b"x").unwrap();
        let live = run_dir(&root);
        fs::create_dir(&live).unwrap();
        fs::write(live.join("sctui-art-1.img"), b"x").unwrap();
        let unrelated = [
            format!("sctui-update-{}-00000000deadbeef", std::process::id()),
            format!("sctui-token-test-{}-beef", std::process::id()),
            "sctui-".to_string(),
            "sctui-12ab".to_string(),
            "sctui-art-test-1".to_string(),
            "other-1".to_string(),
        ];
        for name in &unrelated {
            fs::create_dir(root.join(name)).unwrap();
        }
        // Not a directory, so not a run directory, whatever its name says.
        let file = root.join("sctui-4294967295");
        fs::write(&file, b"x").unwrap();
        #[cfg(unix)]
        let dead = {
            let dead = root.join(format!("sctui-{}", dead_pid()));
            fs::create_dir(&dead).unwrap();
            fs::write(dead.join("sctui-art-1.img"), b"x").unwrap();
            // A symlink inside a dead run's directory is unlinked, not followed.
            std::os::unix::fs::symlink(&live, dead.join("link")).unwrap();
            dead
        };

        sweep(&root);

        assert!(!legacy.exists(), "legacy flat file removed");
        assert!(live.join("sctui-art-1.img").exists(), "a running instance keeps its art");
        for name in &unrelated {
            assert!(root.join(name).is_dir(), "{name} left alone");
        }
        assert!(file.exists());
        #[cfg(unix)]
        assert!(!dead.exists(), "dead run's directory removed");
        let _ = fs::remove_dir_all(&root);
    }

    /// #56: the run directory is private, replaces a leftover with the same pid, and
    /// is gone once the guard drops.
    #[test]
    fn run_dir_is_private_and_removed_on_drop() {
        let root = scratch("run");
        let dir = run_dir(&root);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("stale.img"), b"x").unwrap();

        let guard = RunDir::create(&root);
        assert!(dir.is_dir());
        assert!(!dir.join("stale.img").exists(), "a leftover with this pid is not adopted");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        }
        fs::write(dir.join("sctui-art-1.img"), b"x").unwrap();
        drop(guard);
        assert!(!dir.exists(), "removed at shutdown");
        let _ = fs::remove_dir_all(&root);
    }
}
