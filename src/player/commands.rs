use crate::api::Track;
use crate::player::stream::{CachedHls, Loaded};

/// What prompted a change of track. A natural handover always crossfades (when
/// crossfading is on at all); a user skip only does when the user asked for
/// that; a seek never does — it keeps its own click-avoidance fade.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackChange {
    /// The track ran out and its successor is taking over.
    Natural,
    /// The user picked a different track: next, previous, or Enter on a row.
    UserSkip,
    /// Same track, new position.
    Seek,
}

pub(crate) enum PlayerCommand {
    Play(Track, TrackChange),
    PreloadNext(Track),
    Pause,
    Resume,
    /// Crossfade length in milliseconds (0 for no crossfade), and whether it
    /// applies to user skips as well as natural handovers.
    SetCrossfade { ms: u64, on_user_skips: bool },
    VolumeUp,
    VolumeDown,
    FastForward,
    Rewind,
    /// From a fetch thread back to the player thread: the network half of the
    /// play or seek numbered `seq` has finished.
    Loaded { seq: u64, result: anyhow::Result<Loaded> },
    /// Likewise for a preload of the next track.
    Preloaded(CachedHls),
}
