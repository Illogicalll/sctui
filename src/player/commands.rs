use crate::api::Track;

pub enum PlayerCommand {
    Play(Track),
    PreloadNext(Track),
    Pause,
    Resume,
    /// Crossfade length in milliseconds, 0 for no crossfade.
    SetCrossfade(u64),
    VolumeUp,
    VolumeDown,
    FastForward,
    Rewind,
}
