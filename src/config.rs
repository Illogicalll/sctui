//! `~/.config/sctui/config.toml`: `[theme]`, `[settings]` and `[keys]`. Read once at
//! startup, rewritten by the in-app key editor, theme picker and settings page.

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
    pub warnings: Vec<String>,
}

/// Options edited from the settings page of the `?` overlay.
#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(default)]
pub struct Settings {
    pub hide_unplayable: bool,
    pub hide_feed_tab: bool,
    pub crossfade: bool,
    #[serde(deserialize_with = "de_secs")]
    pub crossfade_secs: f32,
    pub crossfade_user_skips: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hide_unplayable: false,
            hide_feed_tab: false,
            crossfade: false,
            crossfade_secs: 5.0,
            crossfade_user_skips: true,
        }
    }
}

/// Accepts a whole number as well as a decimal, so a config written before
/// half-second steps existed (`crossfade_secs = 5`) still loads. A type error here
/// would fail the whole `[settings]` table and silently reset every option in it.
fn de_secs<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Secs {
        Decimal(f32),
        Whole(i64),
    }
    Ok(match Secs::deserialize(deserializer)? {
        Secs::Decimal(secs) => secs,
        Secs::Whole(secs) => secs as f32,
    })
}

/// One row of the settings page: its label and the field it edits.
#[derive(Clone, Copy)]
pub enum SettingRow {
    /// Flipped with Enter or Space.
    Toggle(&'static str, fn(&mut Settings) -> &mut bool),
    /// The same, but only meaningful while crossfading is on: shown dimmed
    /// alongside the duration whenever it is not.
    CrossfadeToggle(&'static str, fn(&mut Settings) -> &mut bool),
    /// A number of seconds in half-second steps, adjusted with left/right.
    Secs(&'static str, fn(&mut Settings) -> &mut f32),
}

impl Settings {
    /// The settings page, in display order.
    pub const ROWS: [SettingRow; 5] = [
        SettingRow::Toggle("Hide unplayable tracks", |s| &mut s.hide_unplayable),
        SettingRow::Toggle("Hide feed tab", |s| &mut s.hide_feed_tab),
        SettingRow::Toggle("Crossfade between tracks", |s| &mut s.crossfade),
        SettingRow::Secs("Crossfade duration", |s| &mut s.crossfade_secs),
        SettingRow::CrossfadeToggle("Crossfade applies to user-skips", |s| {
            &mut s.crossfade_user_skips
        }),
    ];

    pub const CROSSFADE_SECS_MIN: f32 = 0.5;
    pub const CROSSFADE_SECS_MAX: f32 = 12.0;
    /// Left/right moves the duration by this much.
    pub const CROSSFADE_SECS_STEP: f32 = 0.5;

    /// The duration as the settings page treats it: nonsense from a hand-edited file
    /// falls back to the default, and anything off the half-second grid snaps onto it.
    pub fn crossfade_secs_snapped(&self) -> f32 {
        let secs = if self.crossfade_secs.is_finite() {
            self.crossfade_secs
        } else {
            Self::default().crossfade_secs
        };
        let step = Self::CROSSFADE_SECS_STEP;
        ((secs / step).round() * step).clamp(Self::CROSSFADE_SECS_MIN, Self::CROSSFADE_SECS_MAX)
    }

    /// How long one track should overlap the next, or 0 when crossfading is off
    /// (the player then keeps only its own click-avoidance fade).
    pub fn crossfade_ms(&self) -> u64 {
        if self.crossfade {
            (self.crossfade_secs_snapped() * 1000.0).round() as u64
        } else {
            0
        }
    }
}

impl SettingRow {
    pub fn label(&self) -> &'static str {
        match self {
            SettingRow::Toggle(label, _)
            | SettingRow::CrossfadeToggle(label, _)
            | SettingRow::Secs(label, _) => label,
        }
    }

    /// The value column, and whether the row is actually in effect: a duration
    /// nothing is using yet is dimmed the way an off toggle is.
    pub fn display(&self, mut settings: Settings) -> (String, bool) {
        match self {
            SettingRow::Toggle(_, field) => {
                let on = *field(&mut settings);
                ((if on { "on" } else { "off" }).to_string(), on)
            }
            SettingRow::CrossfadeToggle(_, field) => {
                let on = *field(&mut settings);
                (
                    (if on { "on" } else { "off" }).to_string(),
                    on && settings.crossfade,
                )
            }
            SettingRow::Secs(_, field) => {
                (format!("{:.1}s", field(&mut settings)), settings.crossfade)
            }
        }
    }
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    theme: ThemeSection,
    #[serde(default)]
    settings: Settings,
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
        warnings,
    }
}

/// Write the file: theme choice, any colour overrides, the settings toggles, and
/// only the key bindings that differ from the defaults.
pub fn save(
    keymap: &Keymap,
    theme_name: &str,
    overrides: &Overrides,
    settings: &Settings,
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
    out.push_str("# Options also editable in the app (? then Tab). crossfade_secs is 0.5-12, in half-second steps.\n[settings]\n");
    out.push_str(&settings_section(&Settings::default()));
    out.push('\n');
    out.push_str(&Keymap::default().keys_section(false));
    out
}

/// The body of `[settings]`, one `key = bool` per line.
fn settings_section(settings: &Settings) -> String {
    toml::to_string(settings).unwrap_or_default()
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
        assert!(!file.settings.crossfade);
        assert_eq!(file.settings.crossfade_secs, Settings::default().crossfade_secs);
        assert!(file.settings.crossfade_user_skips);
    }

    #[test]
    fn settings_round_trip() {
        let mut settings = Settings::default();
        for row in &Settings::ROWS {
            match row {
                SettingRow::Toggle(_, field) | SettingRow::CrossfadeToggle(_, field) => {
                    *field(&mut settings) = true
                }
                SettingRow::Secs(_, field) => *field(&mut settings) = 9.5,
            }
        }
        let text = format!("[settings]\n{}", settings_section(&settings));
        let file: File = toml::from_str(&text).unwrap();
        assert!(file.settings.hide_unplayable);
        assert!(file.settings.hide_feed_tab);
        assert!(file.settings.crossfade);
        assert_eq!(file.settings.crossfade_secs, 9.5);
        assert!(file.settings.crossfade_user_skips);
    }

    /// A config file written before crossfading existed has neither key, so the
    /// defaults have to fill in; a hand-edited nonsense duration must not reach
    /// the player.
    #[test]
    fn crossfade_duration_defaults_and_clamps() {
        let file: File = toml::from_str("[settings]\nhide_feed_tab = true\n").unwrap();
        assert_eq!(file.settings.crossfade_secs, 5.0);
        assert!(file.settings.crossfade_user_skips, "a skip crossfades once crossfading is on");
        assert_eq!(file.settings.crossfade_ms(), 0, "off means no crossfade");

        let mut settings = file.settings;
        settings.crossfade = true;
        assert_eq!(settings.crossfade_ms(), 5_000);
        settings.crossfade_secs = 200.0;
        assert_eq!(settings.crossfade_ms(), 12_000);
        settings.crossfade_secs = 0.0;
        assert_eq!(settings.crossfade_ms(), 500);
        settings.crossfade_secs = f32::NAN;
        assert_eq!(settings.crossfade_ms(), 5_000, "nonsense falls back to the default");
    }

    /// Half-second steps: the value survives a round trip through the file, an older
    /// whole-number config still loads, and anything off the grid snaps onto it.
    #[test]
    fn crossfade_duration_takes_half_seconds() {
        let mut settings = Settings {
            crossfade: true,
            crossfade_secs: 2.5,
            ..Default::default()
        };
        assert_eq!(settings.crossfade_ms(), 2_500);

        let text = format!("[settings]\n{}", settings_section(&settings));
        let file: File = toml::from_str(&text).unwrap();
        assert_eq!(file.settings.crossfade_secs, 2.5, "a decimal survives the round trip");

        // Written before half-second steps existed: a bare integer must still parse,
        // and must not take the rest of [settings] down with it.
        let older: File =
            toml::from_str("[settings]\ncrossfade = true\ncrossfade_secs = 7\n").unwrap();
        assert_eq!(older.settings.crossfade_secs, 7.0);
        assert_eq!(older.settings.crossfade_ms(), 7_000);
        assert!(older.settings.crossfade, "the rest of the table still loaded");

        settings.crossfade_secs = 3.7;
        assert_eq!(settings.crossfade_secs_snapped(), 3.5, "snapped onto the half-second grid");
    }

    /// The user-skip row says nothing while there is no crossfade to apply, so
    /// it is dimmed with the duration until the toggle above it is on.
    #[test]
    fn the_user_skip_row_is_dimmed_until_crossfading_is_on() {
        let row = Settings::ROWS
            .iter()
            .find(|r| matches!(r, SettingRow::CrossfadeToggle(..)))
            .expect("the user-skip row is on the settings page");
        let mut settings = Settings::default();
        assert_eq!(row.display(settings), ("on".to_string(), false));
        settings.crossfade = true;
        assert_eq!(row.display(settings), ("on".to_string(), true));
        settings.crossfade_user_skips = false;
        assert_eq!(row.display(settings), ("off".to_string(), false));
    }

    #[test]
    fn theme_section_parses_with_overrides() {
        let file: File = toml::from_str("[theme]\nname = \"nord\"\n[theme.colors]\naccent = \"#ff0000\"\n[keys]\nquit = \"q\"\n").unwrap();
        assert_eq!(file.theme.name.as_deref(), Some("nord"));
        assert_eq!(file.theme.colors.0.get("accent").map(String::as_str), Some("#ff0000"));
    }
}
