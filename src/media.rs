//! OS media integration: the Now Playing widget and the play/pause/skip
//! commands behind media keys, headphones and the Control Centre.
//!
//! macOS only for now (MPNowPlayingInfoCenter + MPRemoteCommandCenter via
//! `souvlaki`). Other platforms get a no-op with the same API so the main loop
//! needs no `cfg`s.

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

pub use imp::Media;

#[cfg(target_os = "macos")]
mod imp {
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
        PlatformConfig, SeekDirection,
    };

    use super::MediaCommand;
    use crate::api::Track;

    pub struct Media {
        controls: Option<MediaControls>,
    }

    impl Media {
        /// Registers with the system. Commands arrive on the returned receiver.
        /// Failure just disables the integration.
        pub fn new() -> (Self, Receiver<MediaCommand>) {
            let (tx, rx) = mpsc::channel();
            let controls = MediaControls::new(PlatformConfig {
                display_name: "sctui",
                dbus_name: "sctui",
                hwnd: None,
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
                            // No seek-to-position in the player yet; volume and URIs are
                            // not exposed by macOS anyway.
                            _ => return,
                        };
                        let _ = tx.send(command);
                    })
                    .ok()
                    .map(|_| controls)
            });
            (Self { controls }, rx)
        }

        /// Push title / artist / artwork / length to the Now Playing widget.
        pub fn set_track(&mut self, track: &Track) {
            if let Some(controls) = self.controls.as_mut() {
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(track.title.as_str()),
                    artist: Some(track.artists.as_str()),
                    album: None,
                    cover_url: (!track.artwork_url.is_empty()).then_some(track.artwork_url.as_str()),
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

        /// MPRemoteCommandCenter delivers its callbacks through the main dispatch
        /// queue, which only drains while the main thread's run loop runs. The TUI
        /// loop owns the main thread, so it calls this once per iteration.
        pub fn pump(&self) {
            if self.controls.is_some() {
                CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, Duration::ZERO, true);
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::sync::mpsc::{self, Receiver};

    use super::MediaCommand;
    use crate::api::Track;

    pub struct Media {
        // Keeps the channel open so `try_recv` reports Empty rather than Disconnected.
        _tx: mpsc::Sender<MediaCommand>,
    }

    impl Media {
        pub fn new() -> (Self, Receiver<MediaCommand>) {
            let (tx, rx) = mpsc::channel();
            (Self { _tx: tx }, rx)
        }
        pub fn set_track(&mut self, _track: &Track) {}
        pub fn set_playback(&mut self, _playing: bool, _position_ms: u64) {}
        pub fn set_stopped(&mut self) {}
        pub fn pump(&self) {}
    }
}
