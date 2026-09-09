use crate::api::Track;

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

pub enum PlayerCommand {
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
}
