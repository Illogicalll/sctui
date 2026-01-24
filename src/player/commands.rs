use crate::api::Track;

pub enum PlayerCommand {
    Play(Track),
    PlayFromPosition(Track, u64),
    Pause,
    Resume,
    VolumeUp,
    VolumeDown,
    NextSong,
    PrevSong,
    FastForward,
    Rewind,
}
