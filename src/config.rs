//! `~/.config/sctui/config.toml`: `[theme]`, `[settings]`, `[eq]` and `[keys]`. Read
//! once at startup, rewritten by the in-app key editor, theme picker, settings page
//! and equaliser page.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::keymap::Keymap;
use crate::theme::{self, Overrides, Theme};

pub fn path() -> PathBuf {
    crate::auth::config_dir().join("config.toml")
}

pub struct Loaded {
    pub keymap: Keymap,
    pub theme_name: String,
    pub theme_overrides: Overrides,
    pub settings: Settings,
    pub eq: Equalizer,
    pub warnings: Vec<String>,
}

/// Per-band equaliser gains in dB, edited from the EQ page of the `?` overlay.
/// The length has to match `player::eq::BANDS`; `player::eq::set` won't compile
/// if it drifts.
#[derive(Deserialize, Serialize, Default, Clone, Copy)]
#[serde(default)]
pub struct Equalizer {
    pub gains: [i8; 6],
}

impl Equalizer {
    /// Pull hand-edited gains back into the range the audio path accepts. Without
    /// this a `gains = [-100, ...]` in the file would be clamped for playback but
    /// still drawn (and saved) as -100.
    fn clamped(mut self) -> Self {
        let max = crate::player::eq::MAX_GAIN_DB;
        for gain in &mut self.gains {
            *gain = (*gain).clamp(-max, max);
        }
        self
    }
}

/// On/off options, edited from the settings page of the `?` overlay.
#[derive(Deserialize, Serialize, Default, Clone, Copy)]
#[serde(default)]
pub struct Settings {
    pub hide_unplayable: bool,
    pub hide_feed_tab: bool,
}

/// One row of the settings page: its label and the field it toggles.
pub type SettingRow = (&'static str, fn(&mut Settings) -> &mut bool);

impl Settings {
    /// The settings page, in display order.
    pub const ROWS: [SettingRow; 2] = [
        ("Hide unplayable tracks", |s| &mut s.hide_unplayable),
        ("Hide feed tab", |s| &mut s.hide_feed_tab),
    ];
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    theme: ThemeSection,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    eq: Equalizer,
}

#[derive(Deserialize, Default)]
struct ThemeSection {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    colors: Overrides,
}

/// Defaults plus whatever the file overrides. Also installs the theme.
pub fn load() -> Loaded {
    let text = std::fs::read_to_string(path()).unwrap_or_default();
    let mut warnings = Vec::new();

    let mut keymap = Keymap::default();
    warnings.extend(keymap.apply_toml(&text));

    // A parse failure was already reported by the keymap parse.
    let file: File = toml::from_str(&text).unwrap_or_default();
    let theme_name = file.theme.name.unwrap_or_else(|| Theme::DEFAULT.name.to_string());
    let base = Theme::by_name(&theme_name).unwrap_or_else(|| {
        warnings.push(format!(
            "unknown theme '{theme_name}' (have: {}); using default",
            Theme::BUILTIN.iter().map(|t| t.name).collect::<Vec<_>>().join(", ")
        ));
        Theme::DEFAULT
    });
    let (resolved, theme_warnings) = base.with_overrides(&file.theme.colors);
    warnings.extend(theme_warnings);
    theme::set(resolved);

    Loaded {
        keymap,
        theme_name: base.name.to_string(),
        theme_overrides: file.theme.colors,
        settings: file.settings,
        eq: file.eq.clamped(),
        warnings,
    }
}

/// Write the file: theme choice, any colour overrides, the settings toggles, the
/// equaliser gains, and only the key bindings that differ from the defaults.
pub fn save(
    keymap: &Keymap,
    theme_name: &str,
    overrides: &Overrides,
    settings: &Settings,
    eq: &Equalizer,
) -> std::io::Result<()> {
    let path = path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut out = String::from(
        "# sctui configuration. Edit here or from the app (? for keys, Shift+O for themes).\n\n[theme]\n",
    );
    out.push_str(&format!("name = \"{theme_name}\"\n"));
    if !overrides.0.is_empty() {
        out.push_str("\n[theme.colors]\n");
        for (role, value) in &overrides.0 {
            out.push_str(&format!("{role} = \"{value}\"\n"));
        }
    }
    out.push_str("\n[settings]\n");
    out.push_str(&settings_section(settings));
    out.push_str("\n[eq]\n");
    out.push_str(&eq_section(eq));
    out.push('\n');
    out.push_str(&keymap.keys_section(true));
    std::fs::write(path, out)
}

/// Full template with every default, for `sctui --dump-config`.
pub fn template() -> String {
    let mut out = String::from(
        "# sctui configuration. Save as ~/.config/sctui/config.toml (or $XDG_CONFIG_HOME/sctui/).\n\n",
    );
    out.push_str("# Colour theme: one of ");
    out.push_str(&Theme::BUILTIN.iter().map(|t| t.name).collect::<Vec<_>>().join(", "));
    out.push_str(".\n# [theme.colors] overrides single roles with \"#rrggbb\" or a colour name: ");
    out.push_str(&theme::ROLES.join(", "));
    out.push_str(".\n[theme]\nname = \"default\"\n# [theme.colors]\n# accent = \"#7aa2f7\"\n\n");
    out.push_str("# On/off options, also editable in the app (? then Tab).\n[settings]\n");
    out.push_str(&settings_section(&Settings::default()));
    out.push_str("\n# Equaliser gains in dB, one per band (60Hz 230Hz 910Hz 3kHz 8kHz 16kHz),\n");
    out.push_str("# -12 to +12. Also editable in the app (? then Tab twice).\n[eq]\n");
    out.push_str(&eq_section(&Equalizer::default()));
    out.push('\n');
    out.push_str(&Keymap::default().keys_section(false));
    out
}

/// The body of `[settings]`, one `key = bool` per line.
fn settings_section(settings: &Settings) -> String {
    toml::to_string(settings).unwrap_or_default()
}

/// The body of `[eq]`, one `gains = [...]` line.
fn eq_section(eq: &Equalizer) -> String {
    toml::to_string(eq).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_reloads_as_defaults() {
        let text = template();
        let mut km = Keymap::default();
        assert!(km.apply_toml(&text).is_empty());
        let file: File = toml::from_str(&text).unwrap();
        assert_eq!(file.theme.name.as_deref(), Some("default"));
        assert!(file.theme.colors.0.is_empty());
        assert!(!file.settings.hide_unplayable);
        assert!(!file.settings.hide_feed_tab);
        assert_eq!(file.eq.gains, [0; 6]);
    }

    #[test]
    fn eq_round_trip() {
        let eq = Equalizer { gains: [-12, -3, 0, 4, 9, 12] };
        let file: File = toml::from_str(&format!("[eq]\n{}", eq_section(&eq))).unwrap();
        assert_eq!(file.eq.gains, eq.gains);

        let wild: File = toml::from_str("[eq]\ngains = [-100, 100, 0, 0, 0, 0]\n").unwrap();
        assert_eq!(wild.eq.clamped().gains, [-12, 12, 0, 0, 0, 0]);
    }

    #[test]
    fn settings_round_trip() {
        let mut settings = Settings::default();
        for (_, field) in Settings::ROWS {
            *field(&mut settings) = true;
        }
        let text = format!("[settings]\n{}", settings_section(&settings));
        let file: File = toml::from_str(&text).unwrap();
        assert!(file.settings.hide_unplayable);
        assert!(file.settings.hide_feed_tab);
    }

    #[test]
    fn theme_section_parses_with_overrides() {
        let file: File = toml::from_str("[theme]\nname = \"nord\"\n[theme.colors]\naccent = \"#ff0000\"\n[keys]\nquit = \"q\"\n").unwrap();
        assert_eq!(file.theme.name.as_deref(), Some("nord"));
        assert_eq!(file.theme.colors.0.get("accent").map(String::as_str), Some("#ff0000"));
    }
}
