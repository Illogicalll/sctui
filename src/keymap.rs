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

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Action {
    Quit,
    Help,
    NextTab,
    PrevTab,
    SubTabLeft,
    SubTabRight,
    Up,
    Down,
    PageUp,
    PageDown,
    SecondaryUp,
    SecondaryDown,
    TertiaryUp,
    TertiaryDown,
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
    AddToQueue,
    PlayNext,
    ToggleQueue,
    ToggleHistory,
    Search,
    ToggleVisualizer,
    AddToPlaylist,
    NewPlaylist,
    RemoveFromPlaylist,
    DeletePlaylist,
}

impl Action {
    pub const ALL: [Action; 36] = [
        Action::Quit,
        Action::Help,
        Action::NextTab,
        Action::PrevTab,
        Action::SubTabLeft,
        Action::SubTabRight,
        Action::Up,
        Action::Down,
        Action::PageUp,
        Action::PageDown,
        Action::SecondaryUp,
        Action::SecondaryDown,
        Action::TertiaryUp,
        Action::TertiaryDown,
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
        Action::AddToQueue,
        Action::PlayNext,
        Action::ToggleQueue,
        Action::ToggleHistory,
        Action::Search,
        Action::ToggleVisualizer,
        Action::AddToPlaylist,
        Action::NewPlaylist,
        Action::RemoveFromPlaylist,
        Action::DeletePlaylist,
    ];

    /// Name used in the config file.
    pub fn name(self) -> &'static str {
        match self {
            Action::Quit => "quit",
            Action::Help => "help",
            Action::NextTab => "next_tab",
            Action::PrevTab => "prev_tab",
            Action::SubTabLeft => "sub_tab_left",
            Action::SubTabRight => "sub_tab_right",
            Action::Up => "up",
            Action::Down => "down",
            Action::PageUp => "page_up",
            Action::PageDown => "page_down",
            Action::SecondaryUp => "secondary_up",
            Action::SecondaryDown => "secondary_down",
            Action::TertiaryUp => "tertiary_up",
            Action::TertiaryDown => "tertiary_down",
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
            Action::AddToQueue => "add_to_queue",
            Action::PlayNext => "play_next",
            Action::ToggleQueue => "toggle_queue",
            Action::ToggleHistory => "toggle_history",
            Action::Search => "search",
            Action::ToggleVisualizer => "toggle_visualizer",
            Action::AddToPlaylist => "add_to_playlist",
            Action::NewPlaylist => "new_playlist",
            Action::RemoveFromPlaylist => "remove_from_playlist",
            Action::DeletePlaylist => "delete_playlist",
        }
    }

    /// Shown in the help popup and as comments in the dumped config.
    pub fn description(self) -> &'static str {
        match self {
            Action::Quit => "Quit (Esc also closes an open popup first)",
            Action::Help => "Key reference and editor",
            Action::NextTab => "Next main tab / next visualiser mode",
            Action::PrevTab => "Previous main tab",
            Action::SubTabLeft => "Previous sub-tab / search filter",
            Action::SubTabRight => "Next sub-tab / search filter",
            Action::Up => "Move selection up",
            Action::Down => "Move selection down",
            Action::PageUp => "Move selection up by 10",
            Action::PageDown => "Move selection down by 10",
            Action::SecondaryUp => "Second pane up (or up by 10 when there is only one pane)",
            Action::SecondaryDown => "Second pane down (or down by 10 when there is only one pane)",
            Action::TertiaryUp => "Move the third pane's selection up (likes of a person)",
            Action::TertiaryDown => "Move the third pane's selection down (likes of a person)",
            Action::PlayPause => "Play / pause",
            Action::PlaySelected => "Play the selected track",
            Action::StartStation => "Start a station from the selected track",
            Action::NextTrack => "Next track",
            Action::PrevTrack => "Previous track",
            Action::SeekForward => "Seek forward 10s",
            Action::SeekBackward => "Seek back 10s",
            Action::VolumeUp => "Volume up",
            Action::VolumeDown => "Volume down",
            Action::ToggleShuffle => "Toggle shuffle",
            Action::ToggleRepeat => "Toggle repeat",
            Action::ToggleLike => "Like / unlike, follow / unfollow the selection",
            Action::AddToQueue => "Add the selected track to the queue",
            Action::PlayNext => "Play the selected track next",
            Action::ToggleQueue => "Toggle the queue popup",
            Action::ToggleHistory => "Toggle listening history (Enter replays a track)",
            Action::Search => "Search: type a query (Search tab) / filter the list (Library)",
            Action::ToggleVisualizer => "Toggle the visualiser",
            Action::AddToPlaylist => "Add the selected track to a playlist (or a new one)",
            Action::NewPlaylist => "Create an empty playlist (Playlists tab)",
            Action::RemoveFromPlaylist => "Remove the selected track from your open playlist",
            Action::DeletePlaylist => "Delete the selected playlist of yours",
        }
    }

    fn from_name(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// The default bindings. Every chord is unique; a test enforces it.
pub const DEFAULTS: &[(Action, &[&str])] = &[
    (Action::Quit, &["esc"]),
    (Action::Help, &["?"]),
    (Action::NextTab, &["tab"]),
    (Action::PrevTab, &["shift+tab"]),
    (Action::SubTabLeft, &["left", "h"]),
    (Action::SubTabRight, &["right", "l"]),
    (Action::Up, &["up", "k"]),
    (Action::Down, &["down", "j"]),
    (Action::PageUp, &["pageup"]),
    (Action::PageDown, &["pagedown"]),
    (Action::SecondaryUp, &["shift+up", "shift+k"]),
    (Action::SecondaryDown, &["shift+down", "shift+j"]),
    (Action::TertiaryUp, &["alt+up"]),
    (Action::TertiaryDown, &["alt+down"]),
    (Action::PlayPause, &["space"]),
    (Action::PlaySelected, &["enter"]),
    (Action::StartStation, &["shift+enter"]),
    (Action::NextTrack, &["shift+right", "n"]),
    (Action::PrevTrack, &["shift+left", "b"]),
    (Action::SeekForward, &["alt+right", "shift+l"]),
    (Action::SeekBackward, &["alt+left", "shift+h"]),
    (Action::VolumeUp, &["+", "="]),
    (Action::VolumeDown, &["-"]),
    (Action::ToggleShuffle, &["shift+s"]),
    (Action::ToggleRepeat, &["shift+r"]),
    (Action::ToggleLike, &["shift+f"]),
    (Action::AddToQueue, &["shift+a"]),
    (Action::PlayNext, &["shift+n"]),
    (Action::ToggleQueue, &["shift+q"]),
    (Action::ToggleHistory, &["shift+p"]),
    (Action::Search, &["/"]),
    (Action::ToggleVisualizer, &["shift+v"]),
    (Action::AddToPlaylist, &["shift+t"]),
    (Action::NewPlaylist, &["shift+c"]),
    (Action::RemoveFromPlaylist, &["shift+d"]),
    (Action::DeletePlaylist, &["shift+x"]),
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

pub struct Keymap {
    bindings: HashMap<Chord, Action>,
    chords: BTreeMap<Action, Vec<Chord>>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut chords = BTreeMap::new();
        for (action, specs) in DEFAULTS {
            let parsed = specs
                .iter()
                .map(|s| Chord::parse(s).expect("default chord parses"))
                .collect();
            chords.insert(*action, parsed);
        }
        let mut km = Keymap { bindings: HashMap::new(), chords };
        km.rebuild();
        km
    }
}

impl Keymap {
    pub fn action(&self, key: &KeyEvent) -> Option<Action> {
        self.bindings.get(&Chord::from_key(key)).copied()
    }

    pub fn chords(&self, action: Action) -> &[Chord] {
        self.chords.get(&action).map(Vec::as_slice).unwrap_or(&[])
    }

    /// `"Shift+Right, n"` for the help popup; `"unbound"` when nothing is set.
    pub fn labels(&self, action: Action) -> String {
        let chords = self.chords(action);
        if chords.is_empty() {
            return "unbound".to_string();
        }
        chords.iter().map(Chord::label).collect::<Vec<_>>().join(", ")
    }

    /// Rebuild chord → action. First action (in `Action::ALL` order) wins a
    /// contested chord; the loser is reported.
    fn rebuild(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();
        self.bindings.clear();
        for action in Action::ALL {
            let Some(chords) = self.chords.get(&action) else { continue };
            for chord in chords {
                if let Some(taken) = self.bindings.get(chord) {
                    warnings.push(format!(
                        "{} is bound to both {} and {}; keeping {}",
                        chord.label(),
                        taken.name(),
                        action.name(),
                        taken.name()
                    ));
                } else {
                    self.bindings.insert(*chord, action);
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
                warnings.push(format!("unknown action '{name}' in [keys]"));
                continue;
            };
            let specs = match spec {
                Spec::One(s) => vec![s],
                Spec::Many(v) => v,
            };
            let mut chords = Vec::new();
            for s in specs {
                match Chord::parse(&s) {
                    Ok(c) => chords.push(c),
                    Err(e) => warnings.push(format!("{}: {e}", action.name())),
                }
            }
            self.chords.insert(action, chords);
        }
        warnings.extend(self.rebuild());
        warnings
    }

    pub fn config_path() -> std::path::PathBuf {
        crate::auth::config_dir().join("config.toml")
    }

    /// Defaults plus the user's `config.toml` if there is one.
    pub fn load() -> (Keymap, Vec<String>) {
        let mut km = Keymap::default();
        let warnings = match std::fs::read_to_string(Self::config_path()) {
            Ok(text) => km.apply_toml(&text),
            Err(_) => Vec::new(),
        };
        (km, warnings)
    }

    /// Which action owns `chord`, if any.
    pub fn bound_to(&self, chord: Chord) -> Option<Action> {
        self.bindings.get(&chord).copied()
    }

    /// Add a chord to `action`. Refused with the owning action if another one
    /// already has it; binding a chord the action already has is a no-op.
    pub fn add_chord(&mut self, action: Action, chord: Chord) -> Result<(), Action> {
        if let Some(other) = self.bindings.get(&chord)
            && *other != action
        {
            return Err(*other);
        }
        let list = self.chords.entry(action).or_default();
        if !list.contains(&chord) {
            list.push(chord);
        }
        self.rebuild();
        Ok(())
    }

    pub fn clear(&mut self, action: Action) {
        self.chords.insert(action, Vec::new());
        self.rebuild();
    }

    /// Restore `action`'s default chords. A default that another action now
    /// owns stays with that action; the message says so.
    pub fn reset(&mut self, action: Action) -> Vec<String> {
        let defaults: Vec<Chord> = DEFAULTS
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, specs)| specs.iter().map(|s| Chord::parse(s).expect("default chord parses")).collect())
            .unwrap_or_default();
        let kept: Vec<Chord> = defaults
            .iter()
            .copied()
            .filter(|c| !matches!(self.bindings.get(c), Some(other) if *other != action))
            .collect();
        let taken: Vec<String> = defaults
            .iter()
            .filter(|c| !kept.contains(c))
            .map(|c| format!("{} stays with {}", c.label(), self.bindings[c].name()))
            .collect();
        self.chords.insert(action, kept);
        self.rebuild();
        taken
    }

    /// Write the current bindings to `config.toml`.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_toml())
    }

    /// A complete config file with the defaults, for `sctui --dump-config`.
    pub fn default_toml() -> String {
        Keymap::default().to_toml()
    }

    /// The current bindings as a complete, commented config file.
    pub fn to_toml(&self) -> String {
        let km = self;
        let mut out = String::from(
            "# sctui key bindings. Save as ~/.config/sctui/config.toml (or $XDG_CONFIG_HOME/sctui/).\n\
             # Each action takes one chord or a list; an empty list unbinds it.\n\
             # Chords: modifiers shift / ctrl / alt joined with +, then a key: a character, space,\n\
             # enter, tab, esc, backspace, delete, insert, home, end, pageup, pagedown,\n\
             # up, down, left, right, f1..f12. An uppercase letter means shift. Bare + is the plus key.\n\
             # Note: shift+enter needs a terminal with the kitty keyboard protocol.\n\n[keys]\n",
        );
        for action in Action::ALL {
            let specs: Vec<String> = km.chords(action).iter().map(|c| format!("\"{}\"", c.spec())).collect();
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
    }

    #[test]
    fn defaults_cover_every_action_without_conflicts() {
        let mut km = Keymap::default();
        assert!(km.rebuild().is_empty(), "default chords must be unique");
        for action in Action::ALL {
            assert!(!km.chords(action).is_empty(), "{} has no default key", action.name());
        }
        assert_eq!(km.action(&key(KeyCode::Char('j'), KeyModifiers::NONE)), Some(Action::Down));
        assert_eq!(km.action(&key(KeyCode::Char('J'), KeyModifiers::SHIFT)), Some(Action::SecondaryDown));
        assert_eq!(km.action(&key(KeyCode::Char('?'), KeyModifiers::SHIFT)), Some(Action::Help));
        assert_eq!(km.action(&key(KeyCode::Esc, KeyModifiers::NONE)), Some(Action::Quit));
        assert_eq!(km.action(&key(KeyCode::Char('h'), KeyModifiers::SHIFT)), Some(Action::SeekBackward));
    }

    #[test]
    fn user_config_overrides_unbinds_and_warns() {
        let mut km = Keymap::default();
        let warnings = km.apply_toml(
            "[keys]\nplay_pause = \"p\"\nnext_track = [\"ctrl+n\", \"n\"]\ndelete_playlist = []\nbogus = \"x\"\nvolume_up = \"esc\"\n",
        );
        assert_eq!(km.action(&key(KeyCode::Char('p'), KeyModifiers::NONE)), Some(Action::PlayPause));
        assert_eq!(km.action(&key(KeyCode::Char(' '), KeyModifiers::NONE)), None, "old chord dropped");
        assert_eq!(km.action(&key(KeyCode::Char('n'), KeyModifiers::CONTROL)), Some(Action::NextTrack));
        assert!(km.chords(Action::DeletePlaylist).is_empty());
        assert_eq!(km.labels(Action::DeletePlaylist), "unbound");
        assert!(warnings.iter().any(|w| w.contains("unknown action 'bogus'")));
        // esc stays with quit (earlier in Action::ALL); the conflict is reported.
        assert_eq!(km.action(&key(KeyCode::Esc, KeyModifiers::NONE)), Some(Action::Quit));
        assert!(warnings.iter().any(|w| w.contains("Esc is bound to both quit and volume_up")));
        assert!(km.apply_toml("this is not toml").iter().any(|w| w.contains("could not be parsed")));
    }

    #[test]
    fn editing_api_adds_clears_resets_and_refuses_conflicts() {
        let mut km = Keymap::default();
        let p = Chord::parse("p").unwrap();
        assert_eq!(km.add_chord(Action::PlayPause, p), Ok(()));
        assert_eq!(km.bound_to(p), Some(Action::PlayPause));
        assert_eq!(km.add_chord(Action::PlayPause, p), Ok(()), "same chord again is a no-op");
        assert_eq!(km.chords(Action::PlayPause).len(), 2);
        assert_eq!(km.add_chord(Action::NextTrack, p), Err(Action::PlayPause));
        km.clear(Action::PlayPause);
        assert!(km.chords(Action::PlayPause).is_empty());
        assert_eq!(km.bound_to(p), None);
        assert!(km.reset(Action::PlayPause).is_empty());
        assert_eq!(km.chords(Action::PlayPause), Keymap::default().chords(Action::PlayPause));
        // A default chord captured by another action stays there on reset.
        km.clear(Action::Help);
        let q = Chord::parse("?").unwrap();
        assert_eq!(km.add_chord(Action::Quit, q), Ok(()));
        let notes = km.reset(Action::Help);
        assert!(km.chords(Action::Help).is_empty());
        assert!(notes[0].contains("stays with quit"));
        // to_toml reflects edits and reloads identically.
        let text = km.to_toml();
        let mut again = Keymap::default();
        assert!(again.apply_toml(&text).is_empty());
        assert_eq!(again.chords(Action::Quit), km.chords(Action::Quit));
        assert!(again.chords(Action::Help).is_empty());
    }

    #[test]
    fn dumped_config_round_trips() {
        let text = Keymap::default_toml();
        let mut km = Keymap::default();
        assert!(km.apply_toml(&text).is_empty(), "dump must re-load cleanly");
        for action in Action::ALL {
            assert_eq!(km.chords(action), Keymap::default().chords(action));
        }
    }
}
