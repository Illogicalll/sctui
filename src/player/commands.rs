use crate::api::Track;

pub enum PlayerCommand {
    Play(Track),
    PlayFromPosition(Track, u64),
    PreloadNext(Track),
    Pause,
    Resume,
    VolumeUp,
    VolumeDown,
    NextSong,
    PrevSong,
    FastForward,
    Rewind,
}
