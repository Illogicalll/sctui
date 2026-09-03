use crate::api::Track;

pub enum PlayerCommand {
    Play(Track),
    PreloadNext(Track),
    Pause,
    Resume,
    VolumeUp,
    VolumeDown,
    FastForward,
    Rewind,
}
