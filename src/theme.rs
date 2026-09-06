//! Colour themes. Eight roles cover the whole UI; the terminal background is
//! never painted, so transparent terminals stay transparent.
//!
//! Config (`~/.config/sctui/config.toml`):
//!
//! ```toml
//! [theme]
//! name = "tokyonight"          # one of Theme::BUILTIN, or "default"
//! [theme.colors]               # optional per-role overrides, "#rrggbb" or a named colour
//! accent = "#ff9e64"
//! ```

use std::sync::RwLock;

use ratatui::style::Color;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    /// Primary text.
    pub fg: Color,
    /// Secondary text: unplayable rows, artists, times.
    pub muted: Color,
    /// Hints, rules, unlit elements.
    pub dim: Color,
    /// Selected tab, progress, current lyric, left channel.
    pub accent: Color,
    /// Table headers, right channel, visualiser far end.
    pub secondary: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    /// Prompts that want attention (key capture).
    pub warning: Color,
    /// RGB endpoints for visualiser gradients (ANSI names have no RGB).
    pub gradient: [(u8, u8, u8); 2],
}

pub const ROLES: [&str; 8] = ["fg", "muted", "dim", "accent", "secondary", "selection_bg", "selection_fg", "warning"];

const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}
const fn tup(hex: u32) -> (u8, u8, u8) {
    ((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    /// The look sctui shipped with: plain ANSI colours, so it follows the
    /// terminal's own palette.
    pub const DEFAULT: Theme = Theme {
        name: "default",
        fg: Color::White,
        muted: Color::Gray,
        dim: Color::DarkGray,
        accent: Color::Cyan,
        secondary: Color::Magenta,
        selection_bg: Color::LightBlue,
        selection_fg: Color::White,
        warning: Color::Yellow,
        gradient: [(0, 255, 255), (255, 0, 255)],
    };

    pub const BUILTIN: [Theme; 9] = [
        Theme::DEFAULT,
        Theme {
            name: "tokyonight",
            fg: rgb(0xc0caf5),
            muted: rgb(0x565f89),
            dim: rgb(0x3b4261),
            accent: rgb(0x7aa2f7),
            secondary: rgb(0xbb9af7),
            selection_bg: rgb(0x33467c),
            selection_fg: rgb(0xc0caf5),
            warning: rgb(0xe0af68),
            gradient: [tup(0x7aa2f7), tup(0xbb9af7)],
        },
        Theme {
            name: "catppuccin-mocha",
            fg: rgb(0xcdd6f4),
            muted: rgb(0x6c7086),
            dim: rgb(0x45475a),
            accent: rgb(0x89b4fa),
            secondary: rgb(0xcba6f7),
            selection_bg: rgb(0x585b70),
            selection_fg: rgb(0xcdd6f4),
            warning: rgb(0xf9e2af),
            gradient: [tup(0x89dceb), tup(0xf5c2e7)],
        },
        Theme {
            name: "dracula",
            fg: rgb(0xf8f8f2),
            muted: rgb(0x6272a4),
            dim: rgb(0x44475a),
            accent: rgb(0x8be9fd),
            secondary: rgb(0xff79c6),
            selection_bg: rgb(0x44475a),
            selection_fg: rgb(0xf8f8f2),
            warning: rgb(0xf1fa8c),
            gradient: [tup(0x8be9fd), tup(0xff79c6)],
        },
        Theme {
            name: "gruvbox-dark",
            fg: rgb(0xebdbb2),
            muted: rgb(0x928374),
            dim: rgb(0x504945),
            accent: rgb(0x83a598),
            secondary: rgb(0xd3869b),
            selection_bg: rgb(0x504945),
            selection_fg: rgb(0xebdbb2),
            warning: rgb(0xfabd2f),
            gradient: [tup(0x8ec07c), tup(0xfe8019)],
        },
        Theme {
            name: "nord",
            fg: rgb(0xd8dee9),
            muted: rgb(0x4c566a),
            dim: rgb(0x434c5e),
            accent: rgb(0x88c0d0),
            secondary: rgb(0xb48ead),
            selection_bg: rgb(0x434c5e),
            selection_fg: rgb(0xeceff4),
            warning: rgb(0xebcb8b),
            gradient: [tup(0x88c0d0), tup(0xb48ead)],
        },
        Theme {
            name: "solarized-dark",
            fg: rgb(0x93a1a1),
            muted: rgb(0x586e75),
            dim: rgb(0x073642),
            accent: rgb(0x2aa198),
            secondary: rgb(0xd33682),
            selection_bg: rgb(0x073642),
            selection_fg: rgb(0xeee8d5),
            warning: rgb(0xb58900),
            gradient: [tup(0x2aa198), tup(0xd33682)],
        },
        Theme {
            name: "one-dark",
            fg: rgb(0xabb2bf),
            muted: rgb(0x5c6370),
            dim: rgb(0x3e4451),
            accent: rgb(0x61afef),
            secondary: rgb(0xc678dd),
            selection_bg: rgb(0x3e4451),
            selection_fg: rgb(0xabb2bf),
            warning: rgb(0xe5c07b),
            gradient: [tup(0x56b6c2), tup(0xc678dd)],
        },
        Theme {
            name: "rose-pine",
            fg: rgb(0xe0def4),
            muted: rgb(0x6e6a86),
            dim: rgb(0x403d52),
            accent: rgb(0x9ccfd8),
            secondary: rgb(0xc4a7e7),
            selection_bg: rgb(0x403d52),
            selection_fg: rgb(0xe0def4),
            warning: rgb(0xf6c177),
            gradient: [tup(0x9ccfd8), tup(0xebbcba)],
        },
    ];

    pub fn by_name(name: &str) -> Option<Theme> {
        Theme::BUILTIN.iter().copied().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Apply `[theme.colors]` overrides. Unknown roles or colours are reported.
    pub fn with_overrides(mut self, overrides: &Overrides) -> (Theme, Vec<String>) {
        let mut warnings = Vec::new();
        for (role, value) in &overrides.0 {
            let Some(color) = parse_color(value) else {
                warnings.push(format!("theme colour '{role}': cannot parse '{value}'"));
                continue;
            };
            match role.as_str() {
                "fg" => self.fg = color,
                "muted" => self.muted = color,
                "dim" => self.dim = color,
                "accent" => {
                    self.accent = color;
                    if let Color::Rgb(r, g, b) = color {
                        self.gradient[0] = (r, g, b);
                    }
                }
                "secondary" => {
                    self.secondary = color;
                    if let Color::Rgb(r, g, b) = color {
                        self.gradient[1] = (r, g, b);
                    }
                }
                "selection_bg" => self.selection_bg = color,
                "selection_fg" => self.selection_fg = color,
                "warning" => self.warning = color,
                other => warnings.push(format!("theme colour '{other}' is not a role ({})", ROLES.join(", "))),
            }
        }
        (self, warnings)
    }

    /// Colour `t` of the way from the accent end to the secondary end.
    pub fn gradient(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let [(r0, g0, b0), (r1, g1, b1)] = self.gradient;
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8;
        Color::Rgb(lerp(r0, r1), lerp(g0, g1), lerp(b0, b1))
    }
}

/// `[theme.colors]` as written in the config: role → colour string.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Overrides(pub std::collections::BTreeMap<String, String>);

/// `#rrggbb`, `rrggbb`, or a ratatui colour name (`cyan`, `light blue`, `gray`...).
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let v = u32::from_str_radix(hex, 16).ok()?;
        return Some(rgb(v));
    }
    s.parse::<Color>().ok()
}

static CURRENT: RwLock<Theme> = RwLock::new(Theme::DEFAULT);

/// The active theme. Cheap; call at render time rather than caching.
pub fn current() -> Theme {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

pub fn set(theme: Theme) {
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = theme;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_names_are_unique_and_resolvable() {
        let mut names: Vec<&str> = Theme::BUILTIN.iter().map(|t| t.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Theme::BUILTIN.len());
        assert_eq!(Theme::by_name("TokyoNight").map(|t| t.name), Some("tokyonight"));
        assert!(Theme::by_name("nope").is_none());
    }

    #[test]
    fn colours_parse_and_overrides_apply() {
        assert_eq!(parse_color("#7aa2f7"), Some(Color::Rgb(0x7a, 0xa2, 0xf7)));
        assert_eq!(parse_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("light blue"), Some(Color::LightBlue));
        assert_eq!(parse_color("zzz"), None);
        let mut o = Overrides::default();
        o.0.insert("accent".into(), "#ff9e64".into());
        o.0.insert("bogus".into(), "#000000".into());
        o.0.insert("dim".into(), "notacolour".into());
        let (t, warnings) = Theme::by_name("tokyonight").unwrap().with_overrides(&o);
        assert_eq!(t.accent, Color::Rgb(0xff, 0x9e, 0x64));
        assert_eq!(t.gradient[0], (0xff, 0x9e, 0x64), "gradient follows an RGB accent");
        assert_eq!(warnings.len(), 2);
        assert_eq!(t.gradient(0.0), Color::Rgb(0xff, 0x9e, 0x64));
        assert_eq!(Theme::DEFAULT.gradient(1.0), Color::Rgb(255, 0, 255));
    }
}
