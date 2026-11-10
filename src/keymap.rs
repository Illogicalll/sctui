//! Configurable key bindings.
//!
//! Every command is an [`Action`]; a [`Keymap`] maps normalised key chords to
//! actions. Defaults live in [`DEFAULTS`] and can be overridden per action in
//! `~/.config/sctui/config.toml`:
//!
//! ```toml
//! [keys]
//! play_pause = "space"
//! next_track = ["shift+right", "n"]
//! delete_playlist = []          # unbind
//! ```
//!
//! Chord grammar: optional modifiers (`shift`, `ctrl`, `alt`; `option`/`opt`
//! are aliases for `alt`) joined by `+`, then a key: a single character,
//! `space`, `enter`, `tab`, `esc`, `backspace`, `delete`, `insert`, `home`,
//! `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`, `f1`..`f12`.
//! An uppercase letter means shift. A bare `+` is the plus key.
//!
//! A binding is one chord or a sequence of up to three, separated by spaces
//! (`top = ["g g", "home"]`). No binding may be the start of another.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use std::time::Duration;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Action {
    Quit,
    Help,
    NextTab,
    PrevTab,
    TabLibrary,
    TabSearch,
    TabFeed,
    SubTabLeft,
    SubTabRight,
    Up,
    Down,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    Top,
    Bottom,
    PaneLeft,
    PaneRight,
    PlayPause,
    PlaySelected,
    StartStation,
    NextTrack,
    PrevTrack,
    SeekForward,
    SeekBackward,
    VolumeUp,
    VolumeDown,
    ToggleShuffle,
    ToggleRepeat,
    ToggleLike,
    ToggleLikePlaying,
    AddToQueue,
    PlayNext,
    ToggleQueue,
    ToggleHistory,
    Search,
    ToggleVisualizer,
    AddToPlaylist,
    AddToPlaylistPlaying,
    NewPlaylist,
    RemoveFromPlaylist,
    DeletePlaylist,
    Themes,
    Palette,
}

impl Action {
    pub const ALL: [Action; 45] = [
        Action::Quit,
        Action::Help,
        Action::NextTab,
        Action::PrevTab,
        Action::TabLibrary,
        Action::TabSearch,
        Action::TabFeed,
        Action::SubTabLeft,
        Action::SubTabRight,
        Action::Up,
        Action::Down,
        Action::PageUp,
        Action::PageDown,
        Action::HalfPageUp,
        Action::HalfPageDown,
        Action::Top,
        Action::Bottom,
        Action::PaneLeft,
        Action::PaneRight,
        Action::PlayPause,
        Action::PlaySelected,
        Action::StartStation,
        Action::NextTrack,
        Action::PrevTrack,
        Action::SeekForward,
        Action::SeekBackward,
        Action::VolumeUp,
        Action::VolumeDown,
        Action::ToggleShuffle,
        Action::ToggleRepeat,
        Action::ToggleLike,
        Action::ToggleLikePlaying,
        Action::AddToQueue,
        Action::PlayNext,
        Action::ToggleQueue,
        Action::ToggleHistory,
        Action::Search,
        Action::ToggleVisualizer,
        Action::AddToPlaylist,
        Action::AddToPlaylistPlaying,
        Action::NewPlaylist,
        Action::RemoveFromPlaylist,
        Action::DeletePlaylist,
        Action::Themes,
        Action::Palette,
    ];

    /// Name used in the config file.
    pub fn name(self) -> &'static str {
        match self {
            Action::Quit => "quit",
            Action::Help => "help",
            Action::NextTab => "next_tab",
            Action::PrevTab => "prev_tab",
            Action::TabLibrary => "tab_library",
            Action::TabSearch => "tab_search",
            Action::TabFeed => "tab_feed",
            Action::SubTabLeft => "sub_tab_left",
            Action::SubTabRight => "sub_tab_right",
            Action::Up => "up",
            Action::Down => "down",
            Action::PageUp => "page_up",
            Action::PageDown => "page_down",
            Action::HalfPageUp => "half_page_up",
            Action::HalfPageDown => "half_page_down",
            Action::Top => "top",
            Action::Bottom => "bottom",
            Action::PaneLeft => "pane_left",
            Action::PaneRight => "pane_right",
            Action::PlayPause => "play_pause",
            Action::PlaySelected => "play_selected",
            Action::StartStation => "start_station",
            Action::NextTrack => "next_track",
            Action::PrevTrack => "prev_track",
            Action::SeekForward => "seek_forward",
            Action::SeekBackward => "seek_backward",
            Action::VolumeUp => "volume_up",
            Action::VolumeDown => "volume_down",
            Action::ToggleShuffle => "toggle_shuffle",
            Action::ToggleRepeat => "toggle_repeat",
            Action::ToggleLike => "toggle_like",
            Action::ToggleLikePlaying => "toggle_like_playing",
            Action::AddToQueue => "add_to_queue",
            Action::PlayNext => "play_next",
            Action::ToggleQueue => "toggle_queue",
            Action::ToggleHistory => "toggle_history",
            Action::Search => "search",
            Action::ToggleVisualizer => "toggle_visualizer",
            Action::AddToPlaylist => "add_to_playlist",
            Action::AddToPlaylistPlaying => "add_to_playlist_playing",
            Action::NewPlaylist => "new_playlist",
            Action::RemoveFromPlaylist => "remove_from_playlist",
            Action::DeletePlaylist => "delete_playlist",
            Action::Themes => "themes",
            Action::Palette => "palette",
        }
    }

    /// Shown in the help popup and as comments in the dumped config.
    pub fn description(self) -> &'static str {
        match self {
            Action::Quit => "Quit",
            Action::Help => "Key reference and editor",
            Action::NextTab => "Next main tab / next visualiser mode",
            Action::PrevTab => "Previous main tab",
            Action::TabLibrary => "Go to Library",
            Action::TabSearch => "Go to Search",
            Action::TabFeed => "Go to Feed",
            Action::SubTabLeft => "Previous sub-tab / search filter",
            Action::SubTabRight => "Next sub-tab / search filter",
            Action::Up => "Move up in the focused pane",
            Action::Down => "Move down in the focused pane",
            Action::PageUp => "Move up a page",
            Action::PageDown => "Move down a page",
            Action::HalfPageUp => "Move up half a page",
            Action::HalfPageDown => "Move down half a page",
            Action::Top => "Go to the first row",
            Action::Bottom => "Go to the last row",
            Action::PaneLeft => "Focus the pane to the left",
            Action::PaneRight => "Focus the pane to the right",
            Action::PlayPause => "Play / pause",
            Action::PlaySelected => "Play the selected track / step into the selected row",
            Action::StartStation => "Start a station from the selected track (or artist)",
            Action::NextTrack => "Next track",
            Action::PrevTrack => "Previous track",
            Action::SeekForward => "Seek forward 10s",
            Action::SeekBackward => "Seek back 10s",
            Action::VolumeUp => "Volume up",
            Action::VolumeDown => "Volume down",
            Action::ToggleShuffle => "Toggle shuffle",
            Action::ToggleRepeat => "Toggle repeat",
            Action::ToggleLike => "Like / unlike, follow / unfollow the selection",
            Action::ToggleLikePlaying => "Like / unlike the currently playing track",
            Action::AddToQueue => "Add the selected track to the queue",
            Action::PlayNext => "Play the selected track next",
            Action::ToggleQueue => "Toggle the queue popup",
            Action::ToggleHistory => "Toggle listening history (Enter replays a track)",
            Action::Search => "Search: type a query (Search tab) / filter the list (Library)",
            Action::ToggleVisualizer => "Toggle the visualiser",
            Action::AddToPlaylist => "Add the selected track to a playlist (or a new one)",
            Action::AddToPlaylistPlaying => "Add the currently playing track to a playlist (or a new one)",
            Action::NewPlaylist => "Create an empty playlist",
            Action::RemoveFromPlaylist => "Remove the selected track from your open playlist",
            Action::DeletePlaylist => "Delete the selected playlist of yours",
            Action::Themes => "Choose a colour theme",
            Action::Palette => "Command palette",
        }
    }

    fn from_name(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// Config names of actions that no longer exist; a `[keys]` entry for one is
/// ignored with a warning that says so.
const REMOVED: [&str; 4] = ["secondary_up", "secondary_down", "tertiary_up", "tertiary_down"];

/// How long a half-typed sequence waits for its next key.
pub const SEQ_TIMEOUT: Duration = Duration::from_secs(1);

/// The default bindings. None is a duplicate or the start of another; a test
/// enforces it.
pub const DEFAULTS: &[(Action, &[&str])] = &[
    (Action::Quit, &["q"]),
    (Action::Help, &["?"]),
    (Action::NextTab, &["tab"]),
    (Action::PrevTab, &["shift+tab"]),
    (Action::TabLibrary, &["1"]),
    (Action::TabSearch, &["2"]),
    (Action::TabFeed, &["3"]),
    (Action::SubTabLeft, &["h", "left"]),
    (Action::SubTabRight, &["l", "right"]),
    (Action::Up, &["k", "up"]),
    (Action::Down, &["j", "down"]),
    (Action::PageUp, &["ctrl+b", "pageup"]),
    (Action::PageDown, &["ctrl+f", "pagedown"]),
    (Action::HalfPageUp, &["ctrl+u"]),
    (Action::HalfPageDown, &["ctrl+d"]),
    (Action::Top, &["g g", "home"]),
    (Action::Bottom, &["G", "end"]),
    (Action::PaneLeft, &["["]),
    (Action::PaneRight, &["]"]),
    (Action::PlayPause, &["space"]),
    (Action::PlaySelected, &["enter"]),
    (Action::StartStation, &["S", "shift+enter"]),
    (Action::NextTrack, &[">"]),
    (Action::PrevTrack, &["<"]),
    (Action::SeekForward, &["."]),
    (Action::SeekBackward, &[","]),
    (Action::VolumeUp, &["+", "="]),
    (Action::VolumeDown, &["-"]),
    (Action::ToggleShuffle, &["s"]),
    (Action::ToggleRepeat, &["r"]),
    (Action::ToggleLike, &["f"]),
    (Action::ToggleLikePlaying, &["F"]),
    (Action::AddToQueue, &["a"]),
    (Action::PlayNext, &["A"]),
    (Action::ToggleQueue, &["Q"]),
    (Action::ToggleHistory, &["H"]),
    (Action::Search, &["/"]),
    (Action::ToggleVisualizer, &["v"]),
    (Action::AddToPlaylist, &["p"]),
    (Action::AddToPlaylistPlaying, &["P"]),
    (Action::NewPlaylist, &["c"]),
    (Action::RemoveFromPlaylist, &["d"]),
    (Action::DeletePlaylist, &["x"]),
    (Action::Themes, &["t"]),
    (Action::Palette, &[":", "ctrl+p"]),
];

/// A normalised key press: letters are lowercase with Shift as a modifier,
/// Shift is dropped from symbols (`?` is `?`, not Shift+`/`), BackTab is
/// Shift+Tab, and only Shift/Ctrl/Alt are kept.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Chord {
    code: KeyCode,
    mods: KeyModifiers,
}

impl Chord {
    fn new(code: KeyCode, mods: KeyModifiers) -> Chord {
        let mut mods = mods & (KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT);
        let code = match code {
            KeyCode::BackTab => {
                mods |= KeyModifiers::SHIFT;
                KeyCode::Tab
            }
            KeyCode::Char(c) if c.is_alphabetic() => {
                if c.is_uppercase() {
                    mods |= KeyModifiers::SHIFT;
                }
                KeyCode::Char(c.to_lowercase().next().unwrap_or(c))
            }
            KeyCode::Char(c) => {
                mods.remove(KeyModifiers::SHIFT);
                KeyCode::Char(c)
            }
            other => other,
        };
        Chord { code, mods }
    }

    pub fn from_key(key: &KeyEvent) -> Chord {
        Chord::new(key.code, key.modifiers)
    }

    pub fn parse(spec: &str) -> Result<Chord, String> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Err("empty key".to_string());
        }
        let (mods_str, key_str) = if let Some(stripped) = spec.strip_suffix('+') {
            (stripped.trim_end_matches('+'), "+")
        } else if let Some(i) = spec.rfind('+') {
            (&spec[..i], &spec[i + 1..])
        } else {
            ("", spec)
        };

        let mut mods = KeyModifiers::NONE;
        for m in mods_str.split('+').filter(|m| !m.is_empty()) {
            mods |= match m.trim().to_ascii_lowercase().as_str() {
                "shift" => KeyModifiers::SHIFT,
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "opt" | "option" | "meta" => KeyModifiers::ALT,
                other => return Err(format!("unknown modifier '{other}' in '{spec}'")),
            };
        }

        let lower = key_str.to_ascii_lowercase();
        let code = match lower.as_str() {
            "space" => KeyCode::Char(' '),
            "enter" | "return" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "backtab" => KeyCode::BackTab,
            "esc" | "escape" => KeyCode::Esc,
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "insert" | "ins" => KeyCode::Insert,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" | "pgup" => KeyCode::PageUp,
            "pagedown" | "pgdn" => KeyCode::PageDown,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "plus" => KeyCode::Char('+'),
            "minus" => KeyCode::Char('-'),
            "slash" => KeyCode::Char('/'),
            "question" => KeyCode::Char('?'),
            _ => {
                let mut chars = key_str.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => KeyCode::Char(c),
                    _ => {
                        if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
                            && (1..=12).contains(&n)
                        {
                            KeyCode::F(n)
                        } else {
                            return Err(format!("unknown key '{key_str}' in '{spec}'"));
                        }
                    }
                }
            }
        };
        Ok(Chord::new(code, mods))
    }

    /// Human label, e.g. `Shift+Enter`, `Alt+Right`, `K`, `?`, `Space`.
    pub fn label(&self) -> String {
        let mut s = String::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            s.push_str("Ctrl+");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            s.push_str("Alt+");
        }
        let shift = self.mods.contains(KeyModifiers::SHIFT);
        match self.code {
            KeyCode::Char(c) if c.is_alphabetic() => {
                if shift {
                    s.push_str("Shift+");
                    s.extend(c.to_uppercase());
                } else {
                    s.push(c);
                }
            }
            other => {
                if shift {
                    s.push_str("Shift+");
                }
                s.push_str(&match other {
                    KeyCode::Char(' ') => "Space".to_string(),
                    KeyCode::Char(c) => c.to_string(),
                    KeyCode::Enter => "Enter".into(),
                    KeyCode::Tab => "Tab".into(),
                    KeyCode::Esc => "Esc".into(),
                    KeyCode::Backspace => "Backspace".into(),
                    KeyCode::Delete => "Delete".into(),
                    KeyCode::Insert => "Insert".into(),
                    KeyCode::Home => "Home".into(),
                    KeyCode::End => "End".into(),
                    KeyCode::PageUp => "PageUp".into(),
                    KeyCode::PageDown => "PageDown".into(),
                    KeyCode::Up => "Up".into(),
                    KeyCode::Down => "Down".into(),
                    KeyCode::Left => "Left".into(),
                    KeyCode::Right => "Right".into(),
                    KeyCode::F(n) => format!("F{n}"),
                    other => format!("{other:?}"),
                });
            }
        }
        s
    }

    /// Short form for the hint line: the config spelling, with Shift+letter as the
    /// capital (`G`, `[`, `enter`, `ctrl+d`).
    pub fn hint_label(&self) -> String {
        match self.code {
            KeyCode::Char(c) if c.is_alphabetic() && self.mods == KeyModifiers::SHIFT => c.to_uppercase().collect(),
            _ => self.spec(),
        }
    }

    /// Config-file spelling, e.g. `shift+enter`, `alt+right`, `shift+k`, `?`.
    fn spec(&self) -> String {
        let mut s = String::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            s.push_str("ctrl+");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            s.push_str("alt+");
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            s.push_str("shift+");
        }
        s.push_str(&match self.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("f{n}"),
            other => format!("{other:?}").to_ascii_lowercase(),
        });
        s
    }
}

/// One binding: 1 to 3 chords pressed in order (`g g`), written space-separated.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Seq(pub Vec<Chord>);

impl Seq {
    pub const MAX: usize = 3;

    pub fn parse(spec: &str) -> Result<Seq, String> {
        let chords = spec.split_whitespace().map(Chord::parse).collect::<Result<Vec<_>, _>>()?;
        match chords.len() {
            0 => Err("empty key".to_string()),
            1..=Seq::MAX => Ok(Seq(chords)),
            _ => Err(format!("'{spec}' is longer than {} keys", Seq::MAX)),
        }
    }

    /// Human label, e.g. `g g`, `Shift+Enter`.
    pub fn label(&self) -> String {
        self.0.iter().map(Chord::label).collect::<Vec<_>>().join(" ")
    }

    /// Short form for the hint line, e.g. `g g`, `G`, `shift+enter`.
    pub fn hint_label(&self) -> String {
        self.0.iter().map(Chord::hint_label).collect::<Vec<_>>().join(" ")
    }

    /// Config-file spelling, e.g. `g g`, `shift+enter`.
    fn spec(&self) -> String {
        self.0.iter().map(Chord::spec).collect::<Vec<_>>().join(" ")
    }
}

/// What a key press came to, given the keys already waiting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyResult {
    Action(Action),
    /// The keys so far start a longer binding; wait for the next one.
    Pending,
    None,
}

pub struct Keymap {
    /// The bindings in effect. No key is the start of another, so a lookup is
    /// never ambiguous.
    bindings: HashMap<Vec<Chord>, Action>,
    seqs: BTreeMap<Action, Vec<Seq>>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut seqs = BTreeMap::new();
        for (action, specs) in DEFAULTS {
            seqs.insert(*action, default_seqs(specs));
        }
        let mut km = Keymap { bindings: HashMap::new(), seqs };
        km.rebuild();
        km
    }
}

fn default_seqs(specs: &[&str]) -> Vec<Seq> {
    specs.iter().map(|s| Seq::parse(s).expect("default binding parses")).collect()
}

/// The binding in `bindings` that equals `seq`, starts it, or is started by it.
fn overlap<'a>(bindings: &'a HashMap<Vec<Chord>, Action>, seq: &[Chord]) -> Option<(&'a [Chord], Action)> {
    bindings
        .iter()
        .find(|(b, _)| b.starts_with(seq) || seq.starts_with(b))
        .map(|(b, a)| (b.as_slice(), *a))
}

impl Keymap {
    /// Take one key press. `pending` holds the keys of a sequence typed so far and
    /// is left holding them only when the result is `Pending`. Keys that stop
    /// matching are dropped and the new key is tried on its own, so `g j` moves down.
    pub fn feed(&self, pending: &mut Vec<Chord>, key: &KeyEvent) -> KeyResult {
        pending.push(Chord::from_key(key));
        loop {
            if self.bindings.keys().any(|b| b.len() > pending.len() && b.starts_with(pending.as_slice())) {
                return KeyResult::Pending;
            }
            if let Some(action) = self.bindings.get(pending.as_slice()) {
                pending.clear();
                return KeyResult::Action(*action);
            }
            let last = pending.pop();
            if pending.is_empty() {
                return KeyResult::None;
            }
            pending.clear();
            pending.extend(last);
        }
    }

    /// What a single key press means with nothing pending.
    #[cfg(test)]
    pub fn action(&self, key: &KeyEvent) -> Option<Action> {
        match self.feed(&mut Vec::new(), key) {
            KeyResult::Action(action) => Some(action),
            _ => None,
        }
    }

    pub fn seqs(&self, action: Action) -> &[Seq] {
        self.seqs.get(&action).map(Vec::as_slice).unwrap_or(&[])
    }

    /// `"g g, Home"` for the help popup; `"unbound"` when nothing is set.
    pub fn labels(&self, action: Action) -> String {
        let seqs = self.seqs(action);
        if seqs.is_empty() {
            return "unbound".to_string();
        }
        seqs.iter().map(Seq::label).collect::<Vec<_>>().join(", ")
    }

    /// The first binding's label, for hints (`"g g"`); empty when unbound.
    pub fn first_label(&self, action: Action) -> String {
        self.seqs(action).first().map(Seq::label).unwrap_or_default()
    }

    /// The first binding in the hint line's short form (`G`, `g g`, `[`); empty when unbound.
    pub fn hint_label(&self, action: Action) -> String {
        self.seqs(action).first().map(Seq::hint_label).unwrap_or_default()
    }

    /// Rebuild binding → action. Shorter bindings go in first, then `Action::ALL`
    /// order; one that repeats or continues a binding already in is left out and
    /// reported, so a single key is never shadowed by a sequence.
    fn rebuild(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        self.bindings.clear();
        let mut all: Vec<(&Seq, Action)> = Action::ALL
            .into_iter()
            .flat_map(|a| self.seqs.get(&a).into_iter().flatten().map(move |s| (s, a)))
            .collect();
        all.sort_by_key(|(seq, _)| seq.0.len());
        for (seq, action) in all {
            match overlap(&self.bindings, &seq.0) {
                Some((taken_keys, taken)) if taken_keys == seq.0.as_slice() => warnings.push(format!(
                    "{} is bound to both {} and {}; keeping {}",
                    seq.label(),
                    taken.name(),
                    action.name(),
                    taken.name()
                )),
                Some((taken_keys, taken)) => warnings.push(format!(
                    "{} is ignored for {}: it starts with {}, which is bound to {}",
                    seq.label(),
                    action.name(),
                    Seq(taken_keys.to_vec()).label(),
                    taken.name()
                )),
                None => {
                    self.bindings.insert(seq.0.clone(), action);
                }
            }
        }
        warnings
    }

    /// Apply the `[keys]` table of a config document. Returns warnings; the
    /// map is always left usable.
    pub fn apply_toml(&mut self, text: &str) -> Vec<String> {
        #[derive(Deserialize)]
        struct Config {
            #[serde(default)]
            keys: BTreeMap<String, Spec>,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Spec {
            One(String),
            Many(Vec<String>),
        }

        let config: Config = match toml::from_str(text) {
            Ok(c) => c,
            Err(e) => return vec![format!("config.toml could not be parsed: {e}")],
        };
        let mut warnings = Vec::new();
        for (name, spec) in config.keys {
            let Some(action) = Action::from_name(&name) else {
                warnings.push(if REMOVED.contains(&name.as_str()) {
                    format!("action '{name}' in [keys] was removed and is ignored")
                } else {
                    format!("unknown action '{name}' in [keys]")
                });
                continue;
            };
            let specs = match spec {
                Spec::One(s) => vec![s],
                Spec::Many(v) => v,
            };
            let mut seqs = Vec::new();
            for s in specs {
                match Seq::parse(&s) {
                    Ok(seq) => seqs.push(seq),
                    Err(e) => warnings.push(format!("{}: {e}", action.name())),
                }
            }
            self.seqs.insert(action, seqs);
        }
        warnings.extend(self.rebuild());
        warnings
    }

    /// Add a binding to `action`. Refused with the owning action if a binding in
    /// effect is the same keys, starts with them, or is the start of them; one the
    /// action already has is a no-op.
    pub fn add(&mut self, action: Action, seq: Seq) -> Result<(), Action> {
        if self.seqs(action).contains(&seq) {
            return Ok(());
        }
        if let Some((_, other)) = overlap(&self.bindings, &seq.0) {
            return Err(other);
        }
        self.seqs.entry(action).or_default().push(seq);
        self.rebuild();
        Ok(())
    }

    /// Remove `action`'s oldest binding and return it. Refused, with the reason,
    /// when there is none, or when it is the only one left for Help or Quit: the
    /// app must always be able to show its keys and to exit.
    pub fn unbind_one(&mut self, action: Action) -> Result<Seq, String> {
        let count = self.seqs(action).len();
        if count == 0 {
            return Err(format!("{} has no key to remove", action.description()));
        }
        if count == 1 && matches!(action, Action::Help | Action::Quit) {
            return Err(format!("{} must keep a key; add another one first", action.description()));
        }
        let removed = self.seqs.entry(action).or_default().remove(0);
        self.rebuild();
        Ok(removed)
    }

    #[cfg(test)]
    pub fn clear(&mut self, action: Action) {
        self.seqs.insert(action, Vec::new());
        self.rebuild();
    }

    /// Restore `action`'s default bindings. A default that clashes with another
    /// action's binding stays with that action; the message says so.
    pub fn reset(&mut self, action: Action) -> Vec<String> {
        let defaults = DEFAULTS.iter().find(|(a, _)| *a == action).map(|(_, specs)| default_seqs(specs));
        self.seqs.insert(action, Vec::new());
        self.rebuild();
        let mut notes = Vec::new();
        for seq in defaults.unwrap_or_default() {
            let label = seq.label();
            if let Err(other) = self.add(action, seq) {
                notes.push(format!("{label} stays with {}", other.name()));
            }
        }
        notes
    }

    /// The `[keys]` section of the config file, commented; with
    /// `only_overrides`, just the actions that differ from the defaults
    /// (unlisted = default).
    pub fn keys_section(&self, only_overrides: bool) -> String {
        let km = self;
        let defaults = Keymap::default();
        let mut out = String::from(
            "# Key bindings. Each action takes one binding or a list; an empty list unbinds it.\n\
             # Chords: modifiers shift / ctrl / alt joined with +, then a key: a character, space,\n\
             # enter, tab, esc, backspace, delete, insert, home, end, pageup, pagedown,\n\
             # up, down, left, right, f1..f12. An uppercase letter means shift. Bare + is the plus key.\n\
             # A binding can be a sequence of up to three chords separated by spaces, e.g. \"g g\".\n\
             # Note: shift+enter needs a terminal with the kitty keyboard protocol.\n[keys]\n",
        );
        if only_overrides {
            out.push_str("# Only actions listed here are changed; everything else keeps its default.\n\n");
        }
        for action in Action::ALL {
            if only_overrides && km.seqs(action) == defaults.seqs(action) {
                continue;
            }
            let specs: Vec<String> = km.seqs(action).iter().map(|c| toml::Value::String(c.spec()).to_string()).collect();
            let _ = writeln!(out, "# {}\n{} = [{}]\n", action.description(), action.name(), specs.join(", "));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn chords_parse_and_normalise() {
        assert_eq!(Chord::parse("shift+enter").unwrap(), Chord::from_key(&key(KeyCode::Enter, KeyModifiers::SHIFT)));
        assert_eq!(Chord::parse("K").unwrap(), Chord::parse("shift+k").unwrap());
        assert_eq!(Chord::parse("shift+k").unwrap(), Chord::from_key(&key(KeyCode::Char('K'), KeyModifiers::NONE)));
        assert_eq!(Chord::parse("?").unwrap(), Chord::from_key(&key(KeyCode::Char('?'), KeyModifiers::SHIFT)));
        assert_eq!(Chord::parse("+").unwrap(), Chord::from_key(&key(KeyCode::Char('+'), KeyModifiers::SHIFT)));
        assert_eq!(Chord::parse("shift++").unwrap(), Chord::parse("+").unwrap());
        assert_eq!(Chord::parse("shift+tab").unwrap(), Chord::from_key(&key(KeyCode::BackTab, KeyModifiers::NONE)));
        assert_eq!(Chord::parse("Option+Right").unwrap(), Chord::from_key(&key(KeyCode::Right, KeyModifiers::ALT)));
        assert_eq!(Chord::parse("F5").unwrap(), Chord::from_key(&key(KeyCode::F(5), KeyModifiers::NONE)));
        assert!(Chord::parse("hyper+x").is_err());
        assert!(Chord::parse("nosuchkey").is_err());
        assert!(Chord::parse("").is_err());
        assert_eq!(Chord::parse("shift+enter").unwrap().label(), "Shift+Enter");
        assert_eq!(Chord::parse("shift+k").unwrap().label(), "Shift+K");
        assert_eq!(Chord::parse("space").unwrap().label(), "Space");
        assert_eq!(Chord::parse("alt+right").unwrap().spec(), "alt+right");
        let km = Keymap::default();
        let hints = [Action::Bottom, Action::Top, Action::PaneLeft, Action::PlaySelected, Action::HalfPageDown, Action::PlayPause];
        assert_eq!(hints.map(|a| km.hint_label(a)), ["G", "g g", "[", "enter", "ctrl+d", "space"]);
    }

    #[test]
    fn defaults_cover_every_action_without_conflicts() {
        let mut km = Keymap::default();
        assert!(km.rebuild().is_empty(), "default bindings must not clash");
        for action in Action::ALL {
            assert!(!km.seqs(action).is_empty(), "{} has no default key", action.name());
        }
        let plain = |c: char| km.action(&key(KeyCode::Char(c), KeyModifiers::NONE));
        let shifted = |c: char| km.action(&key(KeyCode::Char(c), KeyModifiers::SHIFT));
        let ctrl = |c: char| km.action(&key(KeyCode::Char(c), KeyModifiers::CONTROL));
        assert_eq!(plain('j'), Some(Action::Down));
        assert_eq!(shifted('?'), Some(Action::Help));
        assert_eq!(plain('q'), Some(Action::Quit));
        assert_eq!(km.action(&key(KeyCode::Esc, KeyModifiers::NONE)), None, "Esc is never a command");
        // Plain letters act on the selection, capitals on what is playing or a popup.
        for (c, lower, upper) in [
            ('a', Action::AddToQueue, Action::PlayNext),
            ('f', Action::ToggleLike, Action::ToggleLikePlaying),
            ('p', Action::AddToPlaylist, Action::AddToPlaylistPlaying),
            ('s', Action::ToggleShuffle, Action::StartStation),
            ('q', Action::Quit, Action::ToggleQueue),
            ('h', Action::SubTabLeft, Action::ToggleHistory),
        ] {
            assert_eq!(plain(c), Some(lower));
            assert_eq!(shifted(c.to_ascii_uppercase()), Some(upper));
        }
        for (c, action) in [
            ('1', Action::TabLibrary), ('2', Action::TabSearch), ('3', Action::TabFeed),
            ('>', Action::NextTrack), ('<', Action::PrevTrack), ('.', Action::SeekForward),
            (',', Action::SeekBackward), ('r', Action::ToggleRepeat), ('d', Action::RemoveFromPlaylist),
            ('c', Action::NewPlaylist), ('x', Action::DeletePlaylist), ('v', Action::ToggleVisualizer),
            ('t', Action::Themes), ('[', Action::PaneLeft), (']', Action::PaneRight),
        ] {
            assert_eq!(plain(c), Some(action), "{c}");
        }
        // One focused pane: the second- and third-pane keys are gone, focus moves instead.
        assert_eq!(shifted('J'), None);
        assert_eq!(km.action(&key(KeyCode::Char('j'), KeyModifiers::ALT)), None);
        assert_eq!(km.action(&key(KeyCode::Home, KeyModifiers::NONE)), Some(Action::Top));
        assert_eq!(km.action(&key(KeyCode::End, KeyModifiers::NONE)), Some(Action::Bottom));
        assert_eq!(shifted('G'), Some(Action::Bottom));
        assert_eq!(km.action(&key(KeyCode::Enter, KeyModifiers::SHIFT)), Some(Action::StartStation));
        assert_eq!(ctrl('d'), Some(Action::HalfPageDown));
        assert_eq!(ctrl('u'), Some(Action::HalfPageUp));
        assert_eq!(ctrl('f'), Some(Action::PageDown));
        assert_eq!(ctrl('b'), Some(Action::PageUp));
        for (name, action) in [
            ("pane_left", Action::PaneLeft), ("pane_right", Action::PaneRight), ("top", Action::Top),
            ("bottom", Action::Bottom), ("half_page_down", Action::HalfPageDown), ("half_page_up", Action::HalfPageUp),
            ("tab_library", Action::TabLibrary), ("tab_search", Action::TabSearch), ("tab_feed", Action::TabFeed),
        ] {
            assert_eq!(Action::from_name(name), Some(action));
        }
        assert_eq!(Action::from_name("secondary_up"), None);
        assert_eq!(km.labels(Action::Top), "g g, Home");
        assert_eq!(km.first_label(Action::Top), "g g");
    }

    fn feed(km: &Keymap, pending: &mut Vec<Chord>, c: char) -> KeyResult {
        km.feed(pending, &key(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn gg_is_a_sequence() {
        let km = Keymap::default();
        let mut pending = Vec::new();
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Pending);
        assert_eq!(pending, [Chord::parse("g").unwrap()]);
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Action(Action::Top));
        assert!(pending.is_empty());
        assert_eq!(km.action(&key(KeyCode::Char('g'), KeyModifiers::NONE)), None, "one g alone does nothing");
    }

    #[test]
    fn pending_falls_back_to_single_key() {
        let km = Keymap::default();
        let mut pending = Vec::new();
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Pending);
        assert_eq!(feed(&km, &mut pending, 'j'), KeyResult::Action(Action::Down));
        assert!(pending.is_empty());
        // An unbound key after a pending one is nothing, and leaves nothing waiting.
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Pending);
        assert_eq!(feed(&km, &mut pending, 'z'), KeyResult::None);
        assert!(pending.is_empty());

        // Three keys: the key that breaks a sequence may itself start one again.
        let mut km = Keymap::default();
        assert_eq!(km.add(Action::Bottom, Seq::parse("z z g").unwrap()), Ok(()));
        assert_eq!(feed(&km, &mut pending, 'z'), KeyResult::Pending);
        assert_eq!(feed(&km, &mut pending, 'z'), KeyResult::Pending);
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Action(Action::Bottom));
        assert_eq!(feed(&km, &mut pending, 'z'), KeyResult::Pending);
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Pending, "z dropped, g starts g g");
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Action(Action::Top));
    }

    #[test]
    fn defaults_have_no_duplicate_or_prefix_conflicts() {
        let all: Vec<(Action, Seq)> = DEFAULTS
            .iter()
            .flat_map(|(a, specs)| specs.iter().map(move |s| (*a, Seq::parse(s).unwrap())))
            .collect();
        for (i, (a, x)) in all.iter().enumerate() {
            for (b, y) in &all[i + 1..] {
                assert!(
                    !x.0.starts_with(&y.0) && !y.0.starts_with(&x.0),
                    "{} ({}) clashes with {} ({})",
                    x.label(), a.name(), y.label(), b.name()
                );
            }
        }
        assert_eq!(DEFAULTS.len(), Action::ALL.len());
        assert!(Keymap::default().rebuild().is_empty());
    }

    #[test]
    fn config_sequence_parses() {
        let mut km = Keymap::default();
        let warnings = km.apply_toml("[keys]\ntop = [\"g g\", \"home\"]\nbottom = \"z  Z b\"\nhelp = \"a b c d\"\n");
        assert_eq!(km.seqs(Action::Top), [Seq::parse("g g").unwrap(), Seq::parse("home").unwrap()]);
        assert_eq!(km.seqs(Action::Bottom)[0].spec(), "z shift+z b");
        assert_eq!(km.labels(Action::Bottom), "z Shift+Z b");
        let mut pending = Vec::new();
        assert_eq!(feed(&km, &mut pending, 'z'), KeyResult::Pending);
        assert_eq!(km.feed(&mut pending, &key(KeyCode::Char('Z'), KeyModifiers::SHIFT)), KeyResult::Pending);
        assert_eq!(feed(&km, &mut pending, 'b'), KeyResult::Action(Action::Bottom));
        // More than three keys is refused; that entry is dropped and the rest loads.
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("help") && warnings[0].contains("longer than 3"));
        // Written back out and read again, the sequences are unchanged.
        let mut again = Keymap::default();
        assert!(again.apply_toml(&km.keys_section(true)).is_empty());
        assert_eq!(again.seqs(Action::Bottom), km.seqs(Action::Bottom));
    }

    #[test]
    fn removed_actions_warn() {
        let mut km = Keymap::default();
        let warnings = km.apply_toml(
            "[keys]\nsecondary_up = \"K\"\ntertiary_down = [\"alt+j\"]\nplay_pause = \"z\"\n",
        );
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("'secondary_up'") && w.contains("removed")));
        assert!(warnings.iter().any(|w| w.contains("'tertiary_down'") && w.contains("removed")));
        assert_eq!(km.action(&key(KeyCode::Char('z'), KeyModifiers::NONE)), Some(Action::PlayPause));
        assert_eq!(km.action(&key(KeyCode::Char('K'), KeyModifiers::SHIFT)), None);
    }

    /// A single key always beats a sequence that starts with it, whichever the config
    /// names first; the loser is reported and everything else loads.
    #[test]
    fn prefix_conflict_in_config_warns_and_keeps_the_single_key() {
        let mut km = Keymap::default();
        let warnings = km.apply_toml("[keys]\nadd_to_queue = \"g\"\nplay_pause = [\"space\", \"j j\"]\n");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("g g is ignored for top") && w.contains("add_to_queue")));
        assert!(warnings.iter().any(|w| w.contains("j j is ignored for play_pause") && w.contains("down")));
        let mut pending = Vec::new();
        assert_eq!(feed(&km, &mut pending, 'g'), KeyResult::Action(Action::AddToQueue));
        assert_eq!(feed(&km, &mut pending, 'j'), KeyResult::Action(Action::Down));
        assert_eq!(km.action(&key(KeyCode::Home, KeyModifiers::NONE)), Some(Action::Top));
        assert_eq!(km.action(&key(KeyCode::Char(' '), KeyModifiers::NONE)), Some(Action::PlayPause));
    }

    #[test]
    fn help_and_quit_cannot_be_fully_unbound() {
        let mut km = Keymap::default();
        for action in [Action::Help, Action::Quit] {
            let before = km.seqs(action).to_vec();
            let refusal = km.unbind_one(action).unwrap_err();
            assert!(refusal.contains("must keep a key"), "{refusal}");
            assert_eq!(km.seqs(action), before);
        }
        // With a second key the first can go, and then the new one is the last.
        assert_eq!(km.add(Action::Quit, Seq::parse("ctrl+q").unwrap()), Ok(()));
        assert_eq!(km.unbind_one(Action::Quit), Ok(Seq::parse("q").unwrap()));
        assert!(km.unbind_one(Action::Quit).is_err());
        assert_eq!(km.action(&key(KeyCode::Char('q'), KeyModifiers::CONTROL)), Some(Action::Quit));
        // Any other action can be emptied, oldest key first.
        assert_eq!(km.unbind_one(Action::Top), Ok(Seq::parse("g g").unwrap()));
        assert_eq!(km.unbind_one(Action::Top), Ok(Seq::parse("home").unwrap()));
        assert!(km.unbind_one(Action::Top).unwrap_err().contains("no key"));
        assert_eq!(km.labels(Action::Top), "unbound");
        assert_eq!(km.first_label(Action::Top), "");
    }

    #[test]
    fn user_config_overrides_unbinds_and_warns() {
        let mut km = Keymap::default();
        let warnings = km.apply_toml(
            "[keys]\nplay_pause = \"p\"\nnext_track = [\"ctrl+n\", \"n\"]\ndelete_playlist = []\nbogus = \"x\"\nvolume_up = \"q\"\nsecondary_up = \"K\"\n",
        );
        assert_eq!(km.action(&key(KeyCode::Char('p'), KeyModifiers::NONE)), Some(Action::PlayPause));
        assert_eq!(km.action(&key(KeyCode::Char(' '), KeyModifiers::NONE)), None, "old chord dropped");
        assert_eq!(km.action(&key(KeyCode::Char('n'), KeyModifiers::CONTROL)), Some(Action::NextTrack));
        assert!(km.seqs(Action::DeletePlaylist).is_empty());
        assert_eq!(km.labels(Action::DeletePlaylist), "unbound");
        assert!(warnings.iter().any(|w| w.contains("unknown action 'bogus'")));
        // An action that no longer exists is named as removed, and the rest still loads.
        assert!(warnings.iter().any(|w| w.contains("action 'secondary_up' in [keys] was removed")));
        // q stays with quit (earlier in Action::ALL); the conflict is reported.
        assert_eq!(km.action(&key(KeyCode::Char('q'), KeyModifiers::NONE)), Some(Action::Quit));
        assert!(warnings.iter().any(|w| w.contains("q is bound to both quit and volume_up")));
        // p moved to play_pause, so add_to_playlist's default p is the one reported.
        assert!(warnings.iter().any(|w| w.contains("p is bound to both play_pause and add_to_playlist")));
        assert!(km.apply_toml("this is not toml").iter().any(|w| w.contains("could not be parsed")));
    }

    #[test]
    fn editing_api_adds_clears_resets_and_refuses_conflicts() {
        let mut km = Keymap::default();
        let z = Seq::parse("z").unwrap();
        assert_eq!(km.add(Action::PlayPause, z.clone()), Ok(()));
        assert_eq!(km.action(&key(KeyCode::Char('z'), KeyModifiers::NONE)), Some(Action::PlayPause));
        assert_eq!(km.add(Action::PlayPause, z.clone()), Ok(()), "same binding again is a no-op");
        assert_eq!(km.seqs(Action::PlayPause).len(), 2);
        assert_eq!(km.add(Action::NextTrack, z.clone()), Err(Action::PlayPause));
        // A sequence that starts with a bound key, or a key that starts a bound sequence.
        assert_eq!(km.add(Action::NextTrack, Seq::parse("z n").unwrap()), Err(Action::PlayPause));
        assert_eq!(km.add(Action::NextTrack, Seq::parse("g").unwrap()), Err(Action::Top));
        assert_eq!(km.add(Action::NextTrack, Seq::parse("g n").unwrap()), Ok(()), "g g and g n share a start");
        km.clear(Action::PlayPause);
        assert!(km.seqs(Action::PlayPause).is_empty());
        assert_eq!(km.action(&key(KeyCode::Char('z'), KeyModifiers::NONE)), None);
        assert!(km.reset(Action::PlayPause).is_empty());
        assert_eq!(km.seqs(Action::PlayPause), Keymap::default().seqs(Action::PlayPause));
        // A default chord captured by another action stays there on reset.
        km.clear(Action::Help);
        let q = Seq::parse("?").unwrap();
        assert_eq!(km.add(Action::Quit, q), Ok(()));
        let notes = km.reset(Action::Help);
        assert!(km.seqs(Action::Help).is_empty());
        assert!(notes[0].contains("stays with quit"));
        // Saved form lists only the edited actions and reloads identically.
        let text = km.keys_section(true);
        assert!(text.contains("quit = "), "edited action is written");
        assert!(text.contains("help = []"), "unbound action is written");
        assert!(!text.contains("\nnext_tab = "), "untouched action is left to the defaults");
        let mut again = Keymap::default();
        assert!(again.apply_toml(&text).is_empty());
        assert_eq!(again.seqs(Action::Quit), km.seqs(Action::Quit));
        assert!(again.seqs(Action::Help).is_empty());
    }

    #[test]
    fn keys_section_round_trips_quote_and_backslash() {
        let mut km = Keymap::default();
        km.clear(Action::PlayPause);
        for spec in ["\"", "\\"] {
            assert_eq!(km.add(Action::PlayPause, Seq::parse(spec).unwrap()), Ok(()));
        }
        let mut again = Keymap::default();
        let warnings = again.apply_toml(&km.keys_section(true));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(again.seqs(Action::PlayPause), km.seqs(Action::PlayPause));
        assert_eq!(again.seqs(Action::PlayPause).len(), 2);
    }

    #[test]
    fn dumped_config_round_trips() {
        let text = crate::config::template();
        let mut km = Keymap::default();
        assert!(km.apply_toml(&text).is_empty(), "dump must re-load cleanly");
        for action in Action::ALL {
            assert_eq!(km.seqs(action), Keymap::default().seqs(action));
        }
    }
}
