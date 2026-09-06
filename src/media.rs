//! OS media integration: the Now Playing widget and the play/pause/skip
//! commands behind media keys, headphones and the system media panel.
//!
//! One backend per OS, all through `souvlaki`:
//! - macOS: MPNowPlayingInfoCenter + MPRemoteCommandCenter. Callbacks come via
//!   the main dispatch queue, so the TUI loop pumps the main CFRunLoop.
//! - Windows: System Media Transport Controls, which need a window. A hidden
//!   one is created on the main thread and its message queue is pumped.
//! - Linux: MPRIS over D-Bus (zbus). souvlaki runs its own thread; no pump.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
    PlatformConfig, SeekDirection,
};

use crate::api::Track;

/// A command the OS asked us to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    Toggle,
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    SeekForward,
    SeekBackward,
}

pub struct Media {
    controls: Option<MediaControls>,
    #[cfg(target_os = "windows")]
    _window: Option<win::HiddenWindow>,
}

impl Media {
    /// Registers with the system. Commands arrive on the returned receiver.
    /// Any failure just disables the integration.
    pub fn new() -> (Self, Receiver<MediaCommand>) {
        let (tx, rx) = mpsc::channel();

        #[cfg(target_os = "windows")]
        let window = win::HiddenWindow::create();
        #[cfg(target_os = "windows")]
        let hwnd = window.as_ref().map(|w| w.hwnd_ptr());
        #[cfg(not(target_os = "windows"))]
        let hwnd = None;

        let controls = MediaControls::new(PlatformConfig {
            display_name: "sctui",
            dbus_name: "sctui",
            hwnd,
        })
        .ok()
        .and_then(|mut controls| {
            controls
                .attach(move |event| {
                    let command = match event {
                        MediaControlEvent::Toggle => MediaCommand::Toggle,
                        MediaControlEvent::Play => MediaCommand::Play,
                        MediaControlEvent::Pause => MediaCommand::Pause,
                        MediaControlEvent::Stop | MediaControlEvent::Quit => MediaCommand::Stop,
                        MediaControlEvent::Next => MediaCommand::Next,
                        MediaControlEvent::Previous => MediaCommand::Previous,
                        MediaControlEvent::Seek(SeekDirection::Forward)
                        | MediaControlEvent::SeekBy(SeekDirection::Forward, _) => {
                            MediaCommand::SeekForward
                        }
                        MediaControlEvent::Seek(SeekDirection::Backward)
                        | MediaControlEvent::SeekBy(SeekDirection::Backward, _) => {
                            MediaCommand::SeekBackward
                        }
                        // No seek-to-position in the player yet; volume, URIs and
                        // Raise have no meaning for a TUI.
                        _ => return,
                    };
                    let _ = tx.send(command);
                })
                .ok()
                .map(|_| controls)
        });

        (
            Self {
                controls,
                #[cfg(target_os = "windows")]
                _window: window,
            },
            rx,
        )
    }

    /// Push title / artist / length and, when a local copy exists, the artwork.
    /// Only local files are passed: souvlaki's macOS backend aborts the process
    /// when `NSImage` fails to load a remote URL.
    pub fn set_track(&mut self, track: &Track, cover_file: Option<&Path>) {
        if let Some(controls) = self.controls.as_mut() {
            let cover_url = cover_file.map(|p| format!("file://{}", p.display()));
            let _ = controls.set_metadata(MediaMetadata {
                title: Some(track.title.as_str()),
                artist: Some(track.artists.as_str()),
                album: None,
                cover_url: cover_url.as_deref(),
                duration: (track.duration_ms > 0).then(|| Duration::from_millis(track.duration_ms)),
            });
        }
    }

    pub fn set_playback(&mut self, playing: bool, position_ms: u64) {
        if let Some(controls) = self.controls.as_mut() {
            let progress = Some(MediaPosition(Duration::from_millis(position_ms)));
            let _ = controls.set_playback(if playing {
                MediaPlayback::Playing { progress }
            } else {
                MediaPlayback::Paused { progress }
            });
        }
    }

    pub fn set_stopped(&mut self) {
        if let Some(controls) = self.controls.as_mut() {
            let _ = controls.set_playback(MediaPlayback::Stopped);
        }
    }

    /// Deliver pending OS callbacks. Called once per TUI loop iteration on the
    /// main thread, which is where both macOS and Windows want them handled.
    pub fn pump(&self) {
        if self.controls.is_none() {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
            CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, Duration::ZERO, true);
        }
        #[cfg(target_os = "windows")]
        win::pump_messages();
    }
}

/// SMTC binds to a window handle. A console has none, so make an invisible one.
#[cfg(target_os = "windows")]
mod win {
    use std::ffi::c_void;

    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, HMENU, MSG,
        PM_REMOVE, PeekMessageW, RegisterClassW, TranslateMessage, WINDOW_EX_STYLE,
        WINDOW_STYLE, WNDCLASSW,
    };
    use windows::core::PCWSTR;

    unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    pub struct HiddenWindow {
        hwnd: HWND,
        // The class name must outlive the registered class.
        _class_name: Vec<u16>,
    }

    impl HiddenWindow {
        pub fn create() -> Option<Self> {
            let class_name: Vec<u16> = "sctui_media\0".encode_utf16().collect();
            unsafe {
                let instance: HINSTANCE = GetModuleHandleW(PCWSTR::null()).ok()?;
                let class = WNDCLASSW {
                    lpfnWndProc: Some(wnd_proc),
                    hInstance: instance,
                    lpszClassName: PCWSTR(class_name.as_ptr()),
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 {
                    return None;
                }
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    PCWSTR(class_name.as_ptr()),
                    PCWSTR(class_name.as_ptr()),
                    WINDOW_STYLE::default(),
                    0,
                    0,
                    0,
                    0,
                    HWND(0),
                    HMENU(0),
                    instance,
                    None,
                );
                (hwnd.0 != 0).then_some(Self { hwnd, _class_name: class_name })
            }
        }

        pub fn hwnd_ptr(&self) -> *mut c_void {
            self.hwnd.0 as *mut c_void
        }
    }

    impl Drop for HiddenWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }

    pub fn pump_messages() {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, HWND(0), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

/// Process-level checks of souvlaki's macOS artwork loader. Run explicitly:
/// `cargo test -- --ignored souvlaki_`. The "remote" one is expected to abort
/// the test process (that is the bug these tests document); the "local" one
/// must survive.
#[cfg(all(test, target_os = "macos"))]
mod souvlaki_tests {
    use super::*;
    use std::time::Duration;

    fn track(cover: Option<&std::path::Path>) -> (Media, Track) {
        let (media, _rx) = Media::new();
        let t = Track {
            title: "t".into(),
            artists: "a".into(),
            duration: "0:10".into(),
            duration_ms: 10_000,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: "x".into(),
        };
        let _ = cover;
        (media, t)
    }

    #[test]
    #[ignore]
    fn souvlaki_local_file_cover_survives() {
        let Ok(dir) = std::env::var("SCTUI_FIXTURE_DIR") else { return };
        let path = std::path::PathBuf::from(dir).join("art500.jpg");
        let (mut media, t) = track(Some(&path));
        media.set_track(&t, Some(&path));
        std::thread::sleep(Duration::from_secs(3)); // artwork loads on a GCD queue
    }

    #[test]
    #[ignore]
    fn souvlaki_remote_404_cover_aborts() {
        let (mut media, t) = track(None);
        if let Some(controls) = media.controls.as_mut() {
            let _ = controls.set_metadata(MediaMetadata {
                title: Some("t"),
                artist: Some("a"),
                album: None,
                cover_url: Some("https://i1.sndcdn.com/artworks-does-not-exist-t500x500.jpg"),
                duration: None,
            });
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}
