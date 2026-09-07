//! `~/.config/sctui/config.toml`: `[theme]` and `[keys]`. Read once at startup,
//! rewritten by the in-app key editor and theme picker.

use std::path::PathBuf;

use serde::Deserialize;

use crate::keymap::Keymap;
use crate::theme::{self, Overrides, Theme};

pub fn path() -> PathBuf {
    crate::auth::config_dir().join("config.toml")
}

pub struct Loaded {
    pub keymap: Keymap,
    pub theme_name: String,
    pub theme_overrides: Overrides,
    pub warnings: Vec<String>,
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    theme: ThemeSection,
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

    Loaded { keymap, theme_name: base.name.to_string(), theme_overrides: file.theme.colors, warnings }
}

/// Write the file: theme choice, any colour overrides, and only the key
/// bindings that differ from the defaults.
pub fn save(keymap: &Keymap, theme_name: &str, overrides: &Overrides) -> std::io::Result<()> {
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
    out.push_str(&Keymap::default().keys_section(false));
    out
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
    }

    #[test]
    fn theme_section_parses_with_overrides() {
        let file: File = toml::from_str("[theme]\nname = \"nord\"\n[theme.colors]\naccent = \"#ff0000\"\n[keys]\nquit = \"q\"\n").unwrap();
        assert_eq!(file.theme.name.as_deref(), Some("nord"));
        assert_eq!(file.theme.colors.0.get("accent").map(String::as_str), Some("#ff0000"));
    }
}
