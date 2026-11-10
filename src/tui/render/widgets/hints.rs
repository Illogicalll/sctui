//! The hint line: one dim row at the bottom naming the keys that apply right now, read
//! from the live keymap. What it lists follows the mode, in the order input is routed:
//! an open overlay, insert mode, the visualiser, then the focused pane's kind.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Clear,
};

use crate::config::Layout;
use crate::keymap::{Action, Seq};
use crate::theme;
use crate::tui::logic::nav::{self, PaneId, PaneKind};
use crate::tui::logic::state::{AppData, AppState, HelpPage};

use super::{put, put_right, text_width};

/// Terminal rows below which the hint line is not drawn.
pub const ROWS_HINTS: u16 = 14;

/// Rows a layout leaves free at the bottom of a terminal `height` rows tall: 1, or 0
/// when the hint line is switched off or there is no room for it.
pub fn rows(state: &AppState, height: u16) -> u16 {
    (state.settings.hint_line && height >= ROWS_HINTS) as u16
}

/// The keys that apply in the current mode, most useful first, each with a one-word
/// description. Unbound actions are left out.
pub fn pairs(state: &AppState, data: &AppData) -> Vec<(String, &'static str)> {
    let km = &state.keymap;
    let k = |action: Action, what: &'static str| (km.hint_label(action), what);
    let two = |a: Action, b: Action, what: &'static str| {
        let keys: Vec<String> = [a, b].into_iter().map(|a| km.hint_label(a)).filter(|l| !l.is_empty()).collect();
        (keys.join(" "), what)
    };
    let fixed = |keys: &str, what: &'static str| (keys.to_string(), what);
    let rows = |what| two(Action::Down, Action::Up, what);
    let sides = |what| two(Action::SubTabLeft, Action::SubTabRight, what);
    let (enter, esc) = (|what| fixed("enter", what), |what| fixed("esc", what));

    let mut out = if state.palette.is_some() {
        vec![enter("run"), fixed("↑ ↓", "move"), esc("close")]
    } else if state.quit_confirm_visible {
        vec![sides("choose"), enter("confirm"), esc("cancel")]
    } else if state.help_visible {
        match state.help_page {
            _ if state.help_capture.is_some() => vec![esc("cancel")],
            HelpPage::Keys => {
                vec![fixed("tab", "page"), rows("move"), enter("add key"), fixed("backspace", "remove"), fixed("r", "default"), esc("close")]
            }
            HelpPage::Settings => vec![fixed("tab", "page"), rows("move"), enter("toggle"), sides("adjust"), esc("close")],
            HelpPage::Equalizer => vec![fixed("tab", "page"), sides("band"), rows("gain"), fixed("r", "flat"), esc("close")],
        }
    } else if state.theme_picker_visible {
        vec![rows("preview"), enter("keep"), esc("revert")]
    } else if state.confirm.is_some() {
        vec![sides("choose"), enter("confirm"), esc("cancel")]
    } else if state.playlist_picker_title.is_some() {
        vec![enter("create"), fixed("tab", "visibility"), esc("cancel")]
    } else if state.playlist_picker_visible {
        vec![rows("move"), enter("add"), esc("close")]
    } else if state.history_visible {
        vec![rows("move"), enter("play"), esc("close")]
    } else if state.queue_visible {
        vec![rows("scroll"), esc("close")]
    } else if state.search_typing {
        let keep = if state.selected_tab == 1 { "search" } else { "keep filter" };
        vec![enter(keep), esc("cancel"), fixed("ctrl+w", "delete word")]
    } else if state.visualizer_mode {
        vec![fixed("tab", "mode"), k(Action::ToggleVisualizer, "back"), k(Action::PlayPause, "pause"), k(Action::NextTrack, "next")]
    } else if nav::in_nav(state) {
        vec![rows("move"), two(Action::PaneRight, Action::PlaySelected, "open"), k(Action::Palette, "commands"), k(Action::Help, "keys")]
    } else {
        let id = nav::focused(state);
        let mut out = Vec::new();
        // `[` also reaches the navigation column where there is one.
        if nav::panes(state).len() - nav::first_pane(state) > 1 || nav::has_nav(state.settings.layout) {
            out.push(two(Action::PaneLeft, Action::PaneRight, "pane"));
        }
        match id.kind() {
            PaneKind::Tracks => {
                out.extend([
                    k(Action::PlaySelected, "play"),
                    k(Action::AddToQueue, "queue"),
                    k(Action::PlayNext, "next"),
                    k(Action::ToggleLike, "like"),
                    k(Action::AddToPlaylist, "playlist"),
                ]);
                let own = |uri: &str| data.playlists.iter().any(|p| p.is_owned && p.tracks_uri == uri);
                if id == PaneId::PlaylistTracks && data.playlist_tracks_uri.as_deref().is_some_and(own) {
                    out.push(k(Action::RemoveFromPlaylist, "remove"));
                }
                out.push(k(Action::StartStation, "station"));
            }
            kind => {
                out.push(k(Action::PlaySelected, "open"));
                match kind {
                    PaneKind::Playlists => {
                        out.push(k(Action::NewPlaylist, "new"));
                        if id == PaneId::Playlists {
                            out.push(k(Action::DeletePlaylist, "delete"));
                        }
                        out.push(k(Action::ToggleLike, "like"));
                    }
                    PaneKind::People => out.extend([k(Action::ToggleLike, "follow"), k(Action::StartStation, "station")]),
                    PaneKind::Feed => {}
                    _ => out.push(k(Action::ToggleLike, "like")),
                }
            }
        }
        match state.selected_tab {
            // Sidebar's column is how its Library sections are reached.
            0 if state.settings.layout == Layout::Sidebar => out.push(k(Action::Search, "filter")),
            0 => out.extend([sides("sub-tab"), k(Action::Search, "filter")]),
            1 => out.extend([sides("sub-tab"), k(Action::Search, "search")]),
            _ => {}
        }
        out.extend([k(Action::Palette, "commands"), k(Action::Help, "keys")]);
        out
    };
    out.retain(|(keys, _)| !keys.is_empty());
    out
}

/// Draw the hint line in `area` (one row): key in accent bold, description dim, three
/// spaces between pairs, as many whole pairs as fit. Keys of a sequence typed so far
/// (`g …`) sit at the right end.
pub fn draw(frame: &mut Frame, area: Rect, state: &AppState, data: &AppData) {
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    // Over whatever a popup left on this row.
    frame.render_widget(Clear, area);
    let t = theme::current();
    let key = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);
    // The key editor shows the keys it is capturing itself.
    let pending = match state.pending_keys.is_empty() || state.help_capture.is_some() {
        true => String::new(),
        false => format!("{} …", Seq(state.pending_keys.clone()).hint_label()),
    };
    let limit = (area.width as usize).saturating_sub(text_width(&pending) + 2);
    let mut spans = vec![Span::raw(" ")];
    let mut x = 1;
    for (keys, what) in pairs(state, data) {
        let need = text_width(&keys) + 1 + text_width(what);
        if x + need > limit {
            break;
        }
        x += need + 3;
        spans.push(Span::styled(keys, key));
        spans.push(Span::styled(format!(" {what}   "), dim));
    }
    put(frame, area, Line::from(spans));
    if !pending.is_empty() {
        let style = Style::new().fg(t.warning).add_modifier(Modifier::BOLD);
        put_right(frame, Rect { width: area.width - 1, ..area }, &pending, style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Chord;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, render_buffer, render_text, text};

    fn described(state: &AppState, data: &AppData) -> Vec<&'static str> {
        pairs(state, data).into_iter().map(|(_, what)| what).collect()
    }

    /// The hint line alone, `width` cells wide.
    fn line(width: u16, state: &AppState, data: &AppData) -> String {
        text(&draw_buffer(width, 1, |f| draw(f, f.area(), state, data))).trim_end_matches('\n').to_string()
    }

    #[test]
    fn hints_change_with_focus_kind() {
        let (mut state, mut data, ..) = fixture();
        // Library › Likes: one tracks pane, so no pane keys.
        assert_eq!(described(&state, &data), ["play", "queue", "next", "like", "playlist", "station", "sub-tab", "filter", "commands", "keys"]);
        // Library › Playlists, the list: container keys, and nothing that needs a track.
        go(&mut state, &mut data, 0, 1, 0);
        assert_eq!(described(&state, &data), ["pane", "open", "new", "delete", "like", "sub-tab", "filter", "commands", "keys"]);
        // Its tracks: remove only when the open playlist is the user's own.
        go(&mut state, &mut data, 0, 1, 1);
        assert!(!described(&state, &data).contains(&"remove"));
        data.playlist_tracks_uri = Some(data.playlists[0].tracks_uri.clone());
        let hints = described(&state, &data);
        assert!(hints.starts_with(&["pane", "play", "queue"]) && hints.contains(&"remove"), "{hints:?}");
        data.playlist_tracks_uri = Some(data.playlists[3].tracks_uri.clone());
        assert!(!described(&state, &data).contains(&"remove"), "a liked playlist is not editable");
        // People follow; found playlists cannot be deleted; feed items can only be opened.
        go(&mut state, &mut data, 0, 3, 0);
        assert_eq!(described(&state, &data)[..4], ["pane", "open", "follow", "station"]);
        go(&mut state, &mut data, 1, 2, 0);
        assert_eq!(described(&state, &data), ["pane", "open", "new", "like", "sub-tab", "search", "commands", "keys"]);
        go(&mut state, &mut data, 2, 0, 0);
        assert_eq!(described(&state, &data), ["pane", "open", "commands", "keys"]);

        // A navigation column is one more thing `[` reaches, so the pane keys show even
        // with one pane; Sidebar's Library has no sub-tab row to step through.
        go(&mut state, &mut data, 0, 0, 0);
        state.settings.layout = Layout::Sidebar;
        assert_eq!(described(&state, &data), ["pane", "play", "queue", "next", "like", "playlist", "station", "filter", "commands", "keys"]);
        go(&mut state, &mut data, 1, 0, 0);
        assert_eq!(described(&state, &data)[6..], ["station", "sub-tab", "search", "commands", "keys"]);
        // The column itself: move, open, and the two ways to everything else.
        state.nav_focused = true;
        assert_eq!(described(&state, &data), ["move", "open", "commands", "keys"]);
        state.settings.layout = Layout::Classic;
        assert_eq!(described(&state, &data)[0], "play", "no column, no column hints");
        go(&mut state, &mut data, 2, 0, 0);

        // Modes come before the pane: insert, visualiser, then each overlay.
        state.search_typing = true;
        assert_eq!(described(&state, &data), ["keep filter", "cancel", "delete word"]);
        state.search_typing = false;
        state.visualizer_mode = true;
        assert_eq!(described(&state, &data), ["mode", "back", "pause", "next"]);
        state.queue_visible = true;
        assert_eq!(described(&state, &data), ["scroll", "close"]);
        state.theme_picker_visible = true;
        assert_eq!(described(&state, &data), ["preview", "keep", "revert"]);
        state.help_visible = true;
        assert_eq!(described(&state, &data)[..2], ["page", "move"]);
        state.quit_confirm_visible = true;
        assert_eq!(described(&state, &data), ["choose", "confirm", "cancel"]);
        state.palette = Some(Default::default());
        assert_eq!(pairs(&state, &data), [("enter".to_string(), "run"), ("↑ ↓".to_string(), "move"), ("esc".to_string(), "close")]);
    }

    #[test]
    fn hints_use_live_keymap() {
        let (mut state, data, ..) = fixture();
        assert!(line(120, &state, &data).starts_with(" enter play   a queue   A next   f like   p playlist   S station   h l sub-tab"));
        state.keymap.apply_toml("[keys]\nadd_to_queue = \"z\"\nplay_next = []\nsub_tab_left = []\n");
        let hints = line(120, &state, &data);
        assert!(hints.starts_with(" enter play   z queue   f like"), "{hints:?}");
        assert!(hints.contains("   l sub-tab   "), "one of a pair unbound: the other still shows: {hints:?}");
    }

    #[test]
    fn hints_trim_to_width_without_splitting_a_pair() {
        let (state, data, ..) = fixture();
        let full = line(240, &state, &data).trim_end().to_string();
        assert!(full.ends_with(": commands   ? keys"), "{full:?}");
        let all = pairs(&state, &data);
        for width in 1..=full.chars().count() as u16 + 4 {
            let shown = line(width, &state, &data);
            let shown = shown.trim_end();
            // Always a whole number of pairs: a prefix of the full line that ends on a description.
            let whole = (0..=all.len()).any(|n| {
                let joined: Vec<String> = all[..n].iter().map(|(k, d)| format!("{k} {d}")).collect();
                shown.trim_start() == joined.join("   ")
            });
            assert!(whole, "{width} cols cut a pair: {shown:?}");
            assert!(shown.chars().count() < width as usize || shown.is_empty(), "{width} cols: no margin: {shown:?}");
        }
        assert_eq!(line(22, &state, &data).trim_end(), " enter play");
        assert_eq!(line(23, &state, &data).trim_end(), " enter play   a queue");
    }

    #[test]
    fn pending_keys_shown_right_aligned() {
        let (mut state, data, ..) = fixture();
        state.pending_keys = vec![Chord::parse("g").unwrap()];
        let buf = draw_buffer(60, 1, |f| draw(f, f.area(), &state, &data));
        let shown = text(&buf);
        assert!(shown.trim_end_matches('\n').ends_with("g … "), "{shown:?}");
        let t = theme::current();
        assert_eq!((buf[(1, 0)].fg, buf[(7, 0)].fg, buf[(56, 0)].fg), (t.accent, t.dim, t.warning));
        // The hints stop short of it rather than running underneath.
        let before = shown.trim_end_matches('\n').trim_end_matches("g … ");
        assert!(before.ends_with("   ") && before.trim_end().ends_with("p playlist"), "{shown:?}");
        // The key editor is capturing those keys, and says so itself.
        state.help_capture = Some(Action::Quit);
        state.help_visible = true;
        assert!(!line(60, &state, &data).contains('…'));
    }

    #[test]
    fn hint_line_hidden_under_14_rows() {
        let (mut state, mut data, views, np) = fixture();
        let last = |screen: String| screen.lines().last().unwrap().to_string();
        let shown = last(render_text(80, 14, &mut state, &mut data, &views, &np));
        assert!(shown.starts_with(" enter play   a queue"), "{shown:?}");
        let hidden = last(render_text(80, 13, &mut state, &mut data, &views, &np));
        assert!(!hidden.contains("enter play"), "{hidden:?}");
        assert_eq!((rows(&state, 14), rows(&state, 13)), (1, 0));
        // The setting turns it off at any height, and the layout takes the row back.
        state.settings.hint_line = false;
        let off = render_text(80, 24, &mut state, &mut data, &views, &np);
        assert!(!off.contains("enter play"), "{off}");
        assert_eq!(rows(&state, 24), 0);
        // It is drawn last: over the visualiser and under no popup, in its own colours.
        state.settings.hint_line = true;
        (state.visualizer_mode, state.help_visible) = (true, true);
        let buf = render_buffer(80, 24, &mut state, &mut data, &views, &np);
        assert!(last(text(&buf)).starts_with(" tab page"), "{}", text(&buf));
        assert_eq!(buf[(1, 23)].fg, theme::current().accent);
    }
}
