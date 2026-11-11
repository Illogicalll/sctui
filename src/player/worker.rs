use crate::auth::Token;
use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, Sender},
};

use super::commands::PlayerCommand;
use super::stream::{PlaybackEngine, Shared, open_output_stream};

/// Pulls the next command to act on. Spam-skipping next/prev queues a burst of `Play`
/// commands faster than one can load, so a run of them already waiting in the channel is
/// collapsed down to the last one — only the track the user actually lands on should be
/// loaded/started. A non-`Play` command pulled ahead while coalescing is handed back via
/// `pending` and returned first on the next call, so nothing skips ahead of it out of order.
fn next_command(rx: &Receiver<PlayerCommand>, pending: &mut Option<PlayerCommand>) -> Option<PlayerCommand> {
    let msg = match pending.take() {
        Some(msg) => msg,
        None => rx.recv().ok()?,
    };
    Some(if matches!(msg, PlayerCommand::Play(..)) {
        let mut latest = msg;
        while let Ok(next) = rx.try_recv() {
            if matches!(next, PlayerCommand::Play(..)) {
                latest = next;
            } else {
                *pending = Some(next);
                break;
            }
        }
        latest
    } else {
        msg
    })
}

pub(crate) fn player_loop(
    rx: Receiver<PlayerCommand>,
    results: Sender<PlayerCommand>,
    token: Arc<Mutex<Token>>,
    shared: Shared,
    ready: Sender<anyhow::Result<()>>,
) {
    // The output stream stays on this thread for as long as the loop runs;
    // `Player::new` waits to hear whether it could be opened at all.
    let opened = open_output_stream()
        .and_then(|stream| Ok((PlaybackEngine::new(stream.mixer().clone(), token, shared, results)?, stream)));
    let (mut engine, _stream) = match opened {
        Ok(opened) => opened,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    // Nothing here touches the network: plays and preloads fetch on their own
    // threads and report back with `Loaded` / `Preloaded`, so pause, resume
    // and volume are answered straight away.
    let mut pending: Option<PlayerCommand> = None;
    while let Some(msg) = next_command(&rx, &mut pending) {
        match msg {
            PlayerCommand::Play(track, change) => engine.play(track, 0, change),
            PlayerCommand::Loaded { seq, result } => engine.finish_load(seq, result),
            PlayerCommand::PreloadNext(track) => engine.preload(track),
            PlayerCommand::Preloaded(hls) => engine.finish_preload(hls),
            PlayerCommand::SetCrossfade { ms, on_user_skips } => {
                engine.set_crossfade(ms, on_user_skips)
            }
            PlayerCommand::Pause => engine.pause(),
            PlayerCommand::Resume => engine.resume(),
            PlayerCommand::VolumeUp => engine.change_volume(0.1),
            PlayerCommand::VolumeDown => engine.change_volume(-0.1),
            PlayerCommand::FastForward => engine.seek_by(true),
            PlayerCommand::Rewind => engine.seek_by(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Track;
    use crate::player::TrackChange;
    use std::sync::mpsc;

    fn track(urn: &str) -> Track {
        Track {
            title: String::new(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: urn.into(),
        }
    }

    fn urn(cmd: &PlayerCommand) -> &str {
        match cmd {
            PlayerCommand::Play(t, _) => &t.track_urn,
            _ => panic!("expected Play"),
        }
    }

    #[test]
    fn a_burst_of_plays_collapses_to_the_last_one() {
        let (tx, rx) = mpsc::channel();
        tx.send(PlayerCommand::Play(track("a"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Play(track("b"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Play(track("c"), TrackChange::UserSkip)).unwrap();

        let mut pending = None;
        let msg = next_command(&rx, &mut pending).unwrap();
        assert_eq!(urn(&msg), "c");
        assert!(pending.is_none());
        assert!(rx.try_recv().is_err(), "the whole burst should be drained");
    }

    #[test]
    fn a_non_play_command_after_a_burst_is_returned_next_in_order() {
        let (tx, rx) = mpsc::channel();
        tx.send(PlayerCommand::Play(track("a"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Play(track("b"), TrackChange::UserSkip)).unwrap();
        tx.send(PlayerCommand::Pause).unwrap();
        tx.send(PlayerCommand::Play(track("c"), TrackChange::UserSkip)).unwrap();

        let mut pending = None;
        let first = next_command(&rx, &mut pending).unwrap();
        assert_eq!(urn(&first), "b", "a and b collapse; pause stops the drain");
        assert!(matches!(pending, Some(PlayerCommand::Pause)));

        let second = next_command(&rx, &mut pending).unwrap();
        assert!(matches!(second, PlayerCommand::Pause));
        assert!(pending.is_none());

        let third = next_command(&rx, &mut pending).unwrap();
        assert_eq!(urn(&third), "c");
    }

    #[test]
    fn single_play_with_nothing_queued_passes_through_unchanged() {
        let (tx, rx) = mpsc::channel();
        tx.send(PlayerCommand::Play(track("only"), TrackChange::UserSkip)).unwrap();
        let mut pending = None;
        assert_eq!(urn(&next_command(&rx, &mut pending).unwrap()), "only");
    }

    #[test]
    fn closed_channel_ends_the_loop() {
        let (tx, rx) = mpsc::channel::<PlayerCommand>();
        drop(tx);
        let mut pending = None;
        assert!(next_command(&rx, &mut pending).is_none());
    }
}
