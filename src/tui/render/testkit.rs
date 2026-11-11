//! Fixtures and the render-everything harness: every location, layout and overlay drawn
//! on a `TestBackend` at every size in the matrix. A new layout or overlay is one more
//! entry in [`LAYOUTS`] or [`OVERLAYS`]; the tests below loop over both.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ratatui::{Frame, Terminal, backend::TestBackend, buffer::Buffer};

use crate::api::{Activity, Album, Artist, Playlist, Track};
use crate::config::Layout;
use crate::tui::logic::filtering::{FilteredViews, build_filtered_views};
use crate::tui::logic::nav;
use crate::tui::logic::palette::PaletteState;
use crate::tui::logic::state::{AppData, AppState, ConfirmAction, HelpPage, PanelPage, PlaybackSource, QueuedTrack};
use crate::tui::logic::wizard::{Stage, Wizard};

use super::widgets::pane::pane_view;
use super::widgets::text_width;
use super::{NowPlaying, render};

pub const WIDTHS: [u16; 23] = [
    20, 36, 40, 59, 60, 63, 64, 79, 80, 89, 90, 99, 100, 103, 104, 109, 110, 119, 120, 123, 124, 160, 240,
];
pub const HEIGHTS: [u16; 14] = [6, 8, 13, 14, 19, 20, 23, 24, 29, 30, 33, 34, 45, 80];

/// Puts a fixture's state into one named configuration.
pub type Setup = fn(&mut AppState);

/// Switch to `layout`, with or without its navigation column focused. Every entry of
/// [`LAYOUTS`] sets both, so they can be applied in turn.
fn layout(state: &mut AppState, layout: Layout, nav_focused: bool) {
    (state.settings.layout, state.nav_focused) = (layout, nav_focused);
}

/// The Player panel layout on one of its panel's pages.
fn panel(state: &mut AppState, page: PanelPage) {
    layout(state, Layout::PlayerPanel, false);
    state.panel_page = page;
}

/// Every layout (and each of its variants), as a function that switches a fixture to
/// it. The plain variant of a layout is named by its label.
pub const LAYOUTS: &[(&str, Setup)] = &[
    ("classic", |s| layout(s, Layout::Classic, false)),
    ("tighten", |s| layout(s, Layout::Tighten, false)),
    ("columns", |s| layout(s, Layout::Columns, false)),
    ("columns: nav focused", |s| layout(s, Layout::Columns, true)),
    ("player-panel", |s| panel(s, PanelPage::UpNext)),
    ("player-panel: lyrics", |s| panel(s, PanelPage::Lyrics)),
    ("player-panel: spectrum", |s| panel(s, PanelPage::Spectrum)),
    ("player-panel: history", |s| panel(s, PanelPage::History)),
];

/// Every overlay (and each of its pages / modes), as a function that opens it.
pub const OVERLAYS: &[(&str, Setup)] = &[
    ("quit", |s| s.quit_confirm_visible = true),
    ("confirm", |s| s.confirm = Some(ConfirmAction::DeletePlaylist { tracks_uri: playlist_uri(1) })),
    ("help: keys", |s| s.help_visible = true),
    ("help: settings", |s| (s.help_visible, s.help_page) = (true, HelpPage::Settings)),
    ("help: equaliser", |s| (s.help_visible, s.help_page) = (true, HelpPage::Equalizer)),
    ("theme picker", |s| s.theme_picker_visible = true),
    ("playlist picker: list", |s| {
        s.playlist_picker_visible = true;
        s.playlist_picker_track = Some(track(900, WIDE_TITLES[2]));
    }),
    ("playlist picker: new playlist", |s| {
        s.playlist_picker_visible = true;
        s.playlist_picker_title = Some("a new playlist with quite a long name, longer than its box".into());
    }),
    ("queue", |s| s.queue_visible = true),
    ("history", |s| s.history_visible = true),
    ("palette", |s| s.palette = Some(PaletteState { query: "lay".into(), selected: 1 })),
    ("palette: no match", |s| s.palette = Some(PaletteState { query: WIDE_TITLES[0].into(), selected: 9 })),
    ("wizard: welcome", |s| s.wizard = Some(Wizard::at(Stage::Welcome))),
    ("wizard: layout", |s| s.wizard = Some(Wizard { selected: 3, ..Wizard::at(Stage::Layout) })),
    ("wizard: theme", |s| s.wizard = Some(Wizard::at(Stage::Theme))),
    ("wizard: practice", |s| s.wizard = Some(Wizard { step: 3, sub: 1, ticked: Some(2), ..Wizard::at(Stage::Practice) })),
    ("wizard: done", |s| s.wizard = Some(Wizard::at(Stage::Done))),
    ("wizard: upgrade prompt", |s| s.tour_prompt = true),
];

/// Things drawn over every layout that are not overlays: rendered at every size
/// alongside [`OVERLAYS`].
pub const EXTRAS: &[(&str, Setup)] = &[
    ("three toasts", |s| {
        s.toast("Added to queue · 3");
        s.toast_err(format!("Adding to playlist failed: {}", WIDE_TITLES[0]));
        s.toast(format!("Liked {}", WIDE_TITLES[2]));
    }),
    ("hint line off", |s| s.settings.hint_line = false),
    ("toasts over a popup, keys pending", |s| {
        s.toast_err(WIDE_TITLES[1]);
        s.toast("Shuffle on");
        s.help_visible = true;
        s.pending_keys = vec![crate::keymap::Chord::parse("g").unwrap()];
    }),
];

/// Titles that are hard to fit: double-width CJK, emoji, and 200 chars.
pub const WIDE_TITLES: [&str; 3] = [
    "日本語のタイトルはとても長いですね、本当に長いです",
    "🎧 emoji 🔥 in 🎹 the 🌙 title 🎧",
    "A two hundred character title that goes on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on and on, the end.",
];
/// Where the wide titles sit in the fixture's likes.
pub const WIDE_AT: usize = 1;

pub fn track(i: usize, title: &str) -> Track {
    Track {
        title: title.to_string(),
        artists: format!("Artist {}", i % 7),
        duration: format!("{:02}:{:02}", 2 + i % 5, i * 13 % 60),
        duration_ms: ((2 + i % 5) * 60 + i * 13 % 60) as u64 * 1000,
        playback_count: format!("{}.{}K", i * 37 % 900 + 1, i % 10),
        artwork_url: String::new(),
        // Every ninth one is geo-blocked.
        access: if i % 9 == 8 { "blocked".into() } else { "playable".into() },
        track_urn: format!("soundcloud:tracks:{i}"),
        permalink_url: format!("https://soundcloud.com/artist-{}/track-{i}", i % 7),
    }
}

/// `n` tracks named `{prefix} 00`, `{prefix} 01`, …, with urns starting at `base`.
pub fn tracks(base: usize, prefix: &str, n: usize) -> Vec<Track> {
    (0..n).map(|i| track(base + i, &format!("{prefix} {i:02}"))).collect()
}

fn playlist_uri(i: usize) -> String {
    format!("https://api.soundcloud.com/playlists/{i}/tracks")
}

pub fn playlists(prefix: &str, n: usize) -> Vec<Playlist> {
    (0..n)
        .map(|i| Playlist {
            title: format!("{prefix} {i}"),
            track_count: (10 + i * 7).to_string(),
            duration: format!("0{i}:42:00"),
            created_at: Default::default(),
            tracks_uri: playlist_uri(i),
            is_owned: i != 3,
            permalink_url: format!("https://soundcloud.com/me/sets/playlist-{i}"),
        })
        .collect()
}

pub fn albums(prefix: &str, n: usize) -> Vec<Album> {
    (0..n)
        .map(|i| Album {
            title: format!("{prefix} {i}"),
            artists: format!("Artist {i}"),
            release_year: (2019 + i).to_string(),
            duration: format!("00:4{i}:00"),
            track_count: (8 + i).to_string(),
            tracks_uri: format!("https://api.soundcloud.com/playlists/{}/tracks", 100 + i),
            permalink_url: format!("https://soundcloud.com/artist-{i}/sets/album-{i}"),
        })
        .collect()
}

pub fn people(prefix: &str, n: usize) -> Vec<Artist> {
    (0..n).map(|i| Artist { name: format!("{prefix} {i}"), urn: format!("soundcloud:users:{i}"), permalink_url: format!("https://soundcloud.com/person-{i}") }).collect()
}

fn queued(track: &Track) -> QueuedTrack {
    QueuedTrack::new(PlaybackSource::Likes, 0, track, None, None, None, None, true)
}

/// A populated library on Library › Likes: 60 likes (three of them [`WIDE_TITLES`], from
/// row [`WIDE_AT`]), 5 playlists, 4 albums, 6 people, 5 feed items, every child pane and
/// search list filled, `Song 04` playing a minute in, a queue and some history.
pub fn fixture() -> (AppState, AppData, FilteredViews, NowPlaying) {
    let mut likes = tracks(0, "Song", 60);
    for (i, title) in WIDE_TITLES.iter().enumerate() {
        likes[WIDE_AT + i].title = title.to_string();
    }
    let feed = (0..5)
        .map(|i| Activity {
            user: format!("Poster {i}"),
            reposter_urn: None,
            action: if i % 2 == 0 { "Post" } else { "Repost" },
            media: ["Track", "Playlist", "Album"][i % 3],
            created_at: (chrono::Utc::now() - chrono::Duration::hours(1 + 30 * i as i64)).fixed_offset(),
            track: None,
            key: format!("soundcloud:tracks:{}", 700 + i),
            permalink_url: format!("https://soundcloud.com/poster-{i}/post-{i}"),
        })
        .collect();
    let mut data = AppData {
        liked_track_urns: likes.iter().map(|t| t.track_urn.clone()).collect(),
        liked_playlist_uris: [playlist_uri(3)].into(),
        followed_user_urns: ["soundcloud:users:2".to_string()].into(),
        playlists: playlists("Playlist", 5),
        playlist_tracks: tracks(100, "Listed", 30),
        albums: albums("Album", 4),
        album_tracks: tracks(200, "Album cut", 11),
        following: people("Friend", 6),
        following_tracks: tracks(300, "Upload", 14),
        following_likes_tracks: tracks(400, "Their like", 25),
        // Search finds some of the user's own likes, so the heart column has hearts.
        search_tracks: likes[..20].iter().cloned().chain(tracks(500, "Found", 20)).collect(),
        search_albums: albums("Found album", 4),
        search_album_tracks: tracks(520, "Found cut", 9),
        search_playlists: playlists("Found playlist", 5),
        search_playlist_tracks: tracks(540, "Found listed", 12),
        search_people: people("Found person", 6),
        search_people_tracks: tracks(560, "Found upload", 7),
        search_people_likes_tracks: tracks(580, "Found like", 18),
        feed,
        feed_tracks: tracks(700, "Posted", 3),
        // Likes are playing: the queue indexes the copy taken when they started.
        playback_tracks: likes.clone(),
        likes,
        ..Default::default()
    };
    let playing = data.likes[4].clone();
    let state = fixture_state(&data);
    nav::sync_tables(&state, &mut data);
    let views = build_filtered_views(&state, &data);
    let np = NowPlaying { track: playing, volume: 0.7, playing: true, has_track: true };
    (state, data, views, np)
}

/// The state half of [`fixture`], for tests that want a fresh one over the same data
/// (building the data is most of a fixture's cost).
pub fn fixture_state(data: &AppData) -> AppState {
    AppState {
        query: "found".into(),
        progress: 61_000,
        current_playing_index: Some(4),
        auto_queue: VecDeque::from([5, 6, 7]),
        manual_queue: data.likes[10..12].iter().map(queued).collect(),
        playback_history: data.likes[20..23].iter().map(queued).collect(),
        ..Default::default()
    }
}

/// A new account: every list empty, nothing playing.
pub fn fixture_empty() -> (AppState, AppData, FilteredViews, NowPlaying) {
    let state = AppState::default();
    let data = AppData::default();
    let views = build_filtered_views(&state, &data);
    let track = Track {
        title: "No Track Playing - Press <ENTER> on Something to Play!".into(),
        artists: "N/A".into(),
        duration: "0:00".into(),
        duration_ms: 1,
        playback_count: "0".into(),
        artwork_url: String::new(),
        access: "playable".into(),
        track_urn: String::new(),
        permalink_url: String::new(),
    };
    (state, data, views, NowPlaying { track, volume: 1.0, playing: false, has_track: false })
}

/// Every (main tab, sub-tab, focused pane index) there is.
pub fn locations() -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    for (tab, subs) in [(0, 4), (1, 4), (2, 1)] {
        for sub in 0..subs {
            let mut state = AppState::default();
            nav::set_location(&mut state, tab, Some(sub));
            out.extend((0..nav::panes(&state).len()).map(|focus| (tab, sub, focus)));
        }
    }
    out
}

/// Go to a location and focus one of its panes, as the main loop would leave things
/// before a draw.
pub fn go(state: &mut AppState, data: &mut AppData, tab: usize, sub: usize, focus: usize) {
    nav::set_location(state, tab, Some(sub));
    // Fixtures have their search results already.
    state.search_needs_fetch = false;
    nav::set_focus(state, focus);
    nav::sync_tables(state, data);
}

/// The title / name of the focused pane's selected row; `None` when the pane is empty.
/// While the navigation column has focus, its current entry.
pub fn selected_label(state: &AppState, data: &AppData, views: &FilteredViews) -> Option<String> {
    if nav::in_nav(state) {
        let items = nav::nav_items(state);
        return Some(items[nav::nav_index(state, &items)].label.clone());
    }
    let id = nav::focused(state);
    pane_view(id, state, data, views).1.label(nav::cursor(state, id)).map(str::to_string)
}

/// Call `f` with every size in the matrix, biggest first, on every core: a debug-build
/// draw costs about a millisecond and the everything-tests make tens of thousands.
pub fn for_all_sizes(f: impl Fn(u16, u16) + Sync) {
    let sizes: Vec<(u16, u16)> =
        WIDTHS.iter().rev().flat_map(|&w| HEIGHTS.iter().rev().map(move |&h| (w, h))).collect();
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(&(w, h)) = sizes.get(next.fetch_add(1, Ordering::Relaxed)) {
                    f(w, h);
                }
            });
        }
    });
}

/// Run `draw` on a `w`×`h` terminal and hand back what it drew.
pub fn draw_buffer(w: u16, h: u16, draw: impl FnOnce(&mut Frame)) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    // Straight into the frame's buffer: `Terminal::draw` would also diff it against the
    // previous frame and replay that into the backend, which is most of the cost of a
    // draw and none of it is ours.
    draw(&mut terminal.get_frame());
    std::mem::take(terminal.current_buffer_mut())
}

/// The whole app as `render` draws it (no cover art, silent visualiser).
pub fn render_buffer(
    w: u16,
    h: u16,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
) -> Buffer {
    let wave = Arc::new(Mutex::new(VecDeque::new()));
    draw_buffer(w, h, |frame| render(frame, state, data, views, np, None, &wave))
}

/// A buffer's glyphs, one line per row. The cell after a double-width glyph is its
/// other half and is skipped, so wide text reads as it was written.
pub fn text(buf: &Buffer) -> String {
    let mut out = String::with_capacity(buf.area.area() as usize + buf.area.height as usize);
    for row in buf.content.chunks(buf.area.width.max(1) as usize) {
        let mut skip = 0;
        for cell in row {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let symbol = cell.symbol();
            if !symbol.is_ascii() {
                skip = text_width(symbol).saturating_sub(1);
            }
            out.push_str(symbol);
        }
        out.push('\n');
    }
    out
}

pub fn render_text(
    w: u16,
    h: u16,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
) -> String {
    text(&render_buffer(w, h, state, data, views, np))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::logic::nav::Edge;
    use crate::tui::render::{MIN_COLS, MIN_ROWS};

    type Fixture = fn() -> (AppState, AppData, FilteredViews, NowPlaying);
    type FreshState = fn(&AppData) -> AppState;
    /// Each fixture, and how to make another state like the one it came with.
    const FIXTURES: [(&str, Fixture, FreshState); 2] =
        [("populated", fixture, fixture_state), ("empty", fixture_empty, |_| AppState::default())];

    #[test]
    fn every_location_renders_at_every_size() {
        let locations = locations();
        assert_eq!(locations.len(), 18);
        for_all_sizes(|w, h| {
            for (fixture_name, fixture, _) in FIXTURES {
                let (mut state, mut data, views, np) = fixture();
                for (layout, set_layout) in LAYOUTS {
                    set_layout(&mut state);
                    for &(tab, sub, focus) in &locations {
                        go(&mut state, &mut data, tab, sub, focus);
                        // The far end of the list: the pane has to scroll to show it.
                        nav::move_to(&mut state, &mut data, &views, Edge::Bottom);
                        let buf = render_buffer(w, h, &mut state, &mut data, &views, &np);
                        if w < MIN_COLS || h < MIN_ROWS {
                            continue;
                        }
                        let Some(label) = selected_label(&state, &data, &views) else { continue };
                        let start: String = label.chars().take(7).collect();
                        let screen = text(&buf);
                        assert!(
                            screen.contains(&start),
                            "{layout}, {fixture_name}, {w}x{h}, tab {tab} sub {sub} focus {focus}: \
                             selected row {label:?} is not on screen\n{screen}"
                        );
                    }
                }
            }
        });
    }

    /// #54: SoundCloud is credited on every screen that shows its content, in every
    /// layout at every size, the visualiser included: its mark in front of `sctui`, or
    /// leading the top row where there is no `sctui`.
    #[test]
    fn soundcloud_is_credited_in_every_layout_at_every_size() {
        use crate::tui::render::widgets::SOUNDCLOUD_GLYPH;
        for_all_sizes(|w, h| {
            if w < MIN_COLS || h < MIN_ROWS {
                return;
            }
            for (fixture_name, fixture, _) in FIXTURES {
                let (mut state, mut data, views, np) = fixture();
                for (layout, set_layout) in LAYOUTS {
                    set_layout(&mut state);
                    for (tab, visualizer) in [(0, false), (1, false), (2, false), (0, true)] {
                        go(&mut state, &mut data, tab, 0, 0);
                        state.visualizer_mode = visualizer;
                        let at = format!("{layout}, {fixture_name}, {w}x{h}, tab {tab}, visualiser {visualizer}");
                        let screen = render_text(w, h, &mut state, &mut data, &views, &np);
                        let branded = format!("{SOUNDCLOUD_GLYPH} sctui");
                        let beside_brand = if screen.contains("sctui") { screen.contains(&branded) } else { screen.starts_with(&format!(" {SOUNDCLOUD_GLYPH} ≡")) };
                        assert!(beside_brand, "{at}: no credit\n{screen}");
                    }
                }
            }
        });
    }

    /// Where the mark sits, layout by layout.
    #[test]
    fn soundcloud_credit_placement() {
        let (mut state, mut data, views, np) = fixture();
        let mut lines = |state: &mut AppState, w, h| -> Vec<String> {
            render_text(w, h, state, &mut data, &views, &np).lines().map(str::to_string).collect()
        };
        // The visualiser has no header: the mark is in its frame's title.
        state.visualizer_mode = true;
        assert!(lines(&mut state, 120, 32)[0].contains("─\u{f1be} sctui─"));
        state.visualizer_mode = false;
        let mut top = |name: &str, w, h| {
            LAYOUTS.iter().find(|(n, _)| *n == name).unwrap().1(&mut state);
            lines(&mut state, w, h).swap_remove(0)
        };
        assert!(top("classic", 120, 32).contains("─\u{f1be} sctui─"));
        assert!(top("classic", 120, 20).starts_with(" \u{f1be} sctui │  1 Library"));
        assert!(top("tighten", 120, 32).starts_with(" \u{f1be} sctui │  1 Library"));
        assert!(top("player-panel", 120, 32).starts_with(" \u{f1be} sctui │  1 Library"));
        assert!(top("columns", 120, 32).starts_with(" \u{f1be} sctui › Library › Likes"));
    }

    #[test]
    fn every_overlay_renders_at_every_size() {
        for_all_sizes(|w, h| {
            for (_, fixture, fresh_state) in FIXTURES {
                let (_, mut data, views, np) = fixture();
                for (_, set_layout) in LAYOUTS {
                    for (_, open) in OVERLAYS.iter().chain(EXTRAS) {
                        let mut state = fresh_state(&data);
                        set_layout(&mut state);
                        open(&mut state);
                        render_buffer(w, h, &mut state, &mut data, &views, &np);
                    }
                }
            }
        });
    }

    #[test]
    fn every_overlay_is_drawn() {
        let wants = [
            "Want to Quit", "Delete playlist", "Go to Search", "Setting", "Equaliser", " Theme ", "+ New playlist",
            "Visibility:", "Queue", "History", "› lay▏", "no matching command",
            "Welcome to sctui", "Miller columns", "catppuccin-mocha", "Tour 4 / 10", "You're set", "Keys have changed",
        ];
        assert_eq!(wants.len(), OVERLAYS.len());
        for ((name, open), want) in OVERLAYS.iter().zip(wants) {
            let (mut state, mut data, views, np) = fixture();
            open(&mut state);
            let screen = render_text(100, 40, &mut state, &mut data, &views, &np);
            assert!(screen.contains(want), "{name} did not draw {want:?}:\n{screen}");
        }
    }

    #[test]
    fn too_small_shows_message() {
        for (w, h) in [(35, 24), (36, 7), (20, 6), (240, 6)] {
            let (mut state, mut data, views, np) = fixture();
            // An open overlay is not drawn either, and is still open afterwards.
            state.help_visible = true;
            let screen = render_text(w, h, &mut state, &mut data, &views, &np);
            assert!(screen.contains("terminal too small"), "{w}x{h}:\n{screen}");
            assert!(screen.contains(&format!("{w}×{h}")), "{w}x{h}:\n{screen}");
            assert_eq!(screen.replace("terminal too small", "").replace(&format!("{w}×{h}"), "").trim(), "");
            assert!(state.help_visible);
            let screen = render_text(80, 24, &mut state, &mut data, &views, &np);
            assert!(screen.contains("Equaliser") && !screen.contains("too small"), "{screen}");
        }
        // Smaller than the message itself: clipped, not a panic.
        for (w, h) in [(1, 1), (2, 1), (1, 3), (10, 2)] {
            let (mut state, mut data, views, np) = fixture();
            render_buffer(w, h, &mut state, &mut data, &views, &np);
        }
        let (mut state, mut data, views, np) = fixture();
        assert!(!render_text(MIN_COLS, MIN_ROWS, &mut state, &mut data, &views, &np).contains("too small"));
    }

    /// Wide and very long titles are cut to their column: the pane's right border is
    /// intact on every row, whichever of them is selected.
    #[test]
    fn wide_titles_stay_inside_the_pane() {
        for w in WIDTHS.into_iter().filter(|&w| w >= MIN_COLS) {
            for (i, title) in WIDE_TITLES.iter().enumerate() {
                let (mut state, mut data, views, np) = fixture();
                nav::move_by(&mut state, &mut data, &views, (WIDE_AT + i) as isize);
                let buf = render_buffer(w, 24, &mut state, &mut data, &views, &np);
                let screen = text(&buf);
                let start: String = title.chars().take(4).collect();
                assert!(screen.contains(&start), "{w} cols: {title:?} is not shown\n{screen}");
                let rows: Vec<u16> = (0..24).filter(|&y| buf[(0, y)].symbol() == "│").collect();
                assert!(rows.len() > 8, "{screen}");
                for y in rows {
                    assert_eq!(buf[(w - 1, y)].symbol(), "│", "{w} cols, row {y}: border overwritten\n{screen}");
                }
            }
        }
    }

    #[test]
    fn classic_is_boxed_when_tall_and_compact_when_short() {
        let (mut state, mut data, views, np) = fixture();
        go(&mut state, &mut data, 1, 0, 0);
        let tall = render_text(120, 32, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = tall.lines().collect();
        assert!(lines[0].starts_with('╭') && lines[0].contains("sctui"), "{tall}");
        assert!(lines[3].contains("search") && lines[4].contains("found"), "{tall}");
        assert!(tall.contains("filter") && tall.contains("shf:") && tall.contains('♥'), "{tall}");
        // 24 rows keeps the boxes but not the 7-row player; 23 drops the boxes.
        let mid = render_text(120, 24, &mut state, &mut data, &views, &np);
        assert!(mid.starts_with('╭') && !mid.contains("shf:") && mid.contains("vol"), "{mid}");
        let short = render_text(120, 23, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = short.lines().collect();
        assert!(lines[0].starts_with(" \u{f1be} sctui │") && lines[1].contains("Tracks") && lines[1].contains("/ found"), "{short}");
        assert!(lines[2].starts_with('╭'), "{short}");
    }

    #[test]
    fn visualiser_renders_through_render_without_cover_art() {
        use crate::tui::logic::state::VisualizerMode;
        for mode in VisualizerMode::ALL {
            for (w, h) in [(MIN_COLS, MIN_ROWS), (80, 24), (240, 80)] {
                let (mut state, mut data, views, np) = fixture();
                (state.visualizer_mode, state.visualizer_view, state.queue_visible) = (true, mode, true);
                let screen = render_text(w, h, &mut state, &mut data, &views, &np);
                assert!(screen.contains("Queue"), "{mode:?} at {w}x{h} lost its overlay:\n{screen}");
            }
        }
    }
}
