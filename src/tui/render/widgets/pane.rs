//! One pane of any kind: border, title, position counter, header row, rows, cursor.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
};

use crate::api::{Activity, Album, Artist, List, Playlist, Track};
use crate::theme;
use crate::tui::logic::filtering::{FilteredViews, is_filter_active};
use crate::tui::logic::nav::{self, PaneId, PaneKind};
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::NowPlaying;

use super::{fit, put};

/// The rows a pane shows, borrowed from wherever they live.
pub enum PaneRows<'a> {
    Tracks(&'a [Track]),
    Playlists(&'a [Playlist]),
    Albums(&'a [Album]),
    People(&'a [Artist]),
    Feed(&'a [Activity]),
}

impl PaneRows<'_> {
    pub fn len(&self) -> usize {
        match self {
            PaneRows::Tracks(r) => r.len(),
            PaneRows::Playlists(r) => r.len(),
            PaneRows::Albums(r) => r.len(),
            PaneRows::People(r) => r.len(),
            PaneRows::Feed(r) => r.len(),
        }
    }

    /// The title / name of row `i`: what its first text column shows.
    pub fn label(&self, i: usize) -> Option<&str> {
        Some(match self {
            PaneRows::Tracks(r) => r.get(i)?.title.as_str(),
            PaneRows::Playlists(r) => r.get(i)?.title.as_str(),
            PaneRows::Albums(r) => r.get(i)?.title.as_str(),
            PaneRows::People(r) => r.get(i)?.name.as_str(),
            PaneRows::Feed(r) => r.get(i)?.user.as_str(),
        })
    }
}

/// How a layout wants its panes dressed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PaneStyle {
    /// Classic: a centred title on the few panes that had one (`tracks`, `liked`,
    /// `activity`), nothing on the rest, and the Search tab's heart column. Otherwise
    /// every pane is titled top-left, with its filter text.
    pub boxed_title: bool,
}

/// A pane's title and rows. The pane the Library filter applies to reads the filtered
/// views while a filter is active; every other pane reads `data`.
pub fn pane_view<'a>(
    id: PaneId,
    state: &AppState,
    data: &'a AppData,
    views: &'a FilteredViews,
) -> (String, PaneRows<'a>) {
    let filtered = is_filter_active(state) && nav::filter_pane(state) == Some(id);
    // A child pane is named after the row it belongs to.
    let child_of = |parent: PaneId| {
        let (_, rows) = pane_view(parent, state, data, views);
        rows.label(nav::cursor(state, parent)).unwrap_or("—").to_string()
    };
    let (title, rows) = match id {
        PaneId::Likes => ("Likes".into(), PaneRows::Tracks(if filtered { &views.likes } else { &data.likes })),
        PaneId::Playlists => ("Playlists".into(), PaneRows::Playlists(&data.playlists)),
        PaneId::PlaylistTracks => (
            child_of(PaneId::Playlists),
            PaneRows::Tracks(if filtered { &views.playlist_tracks } else { &data.playlist_tracks }),
        ),
        PaneId::Albums => ("Albums".into(), PaneRows::Albums(if filtered { &views.albums } else { &data.albums })),
        PaneId::AlbumTracks => (child_of(PaneId::Albums), PaneRows::Tracks(&data.album_tracks)),
        PaneId::Following => (
            "Following".into(),
            PaneRows::People(if filtered { &views.following } else { &data.following }),
        ),
        PaneId::FollowingTracks => ("tracks".into(), PaneRows::Tracks(&data.following_tracks)),
        PaneId::FollowingLikes => ("liked".into(), PaneRows::Tracks(&data.following_likes_tracks)),
        PaneId::SearchTracks => ("Results".into(), PaneRows::Tracks(&data.search_tracks)),
        PaneId::SearchAlbums => ("Albums".into(), PaneRows::Albums(&data.search_albums)),
        PaneId::SearchAlbumTracks => (child_of(PaneId::SearchAlbums), PaneRows::Tracks(&data.search_album_tracks)),
        PaneId::SearchPlaylists => ("Playlists".into(), PaneRows::Playlists(&data.search_playlists)),
        PaneId::SearchPlaylistTracks => (
            child_of(PaneId::SearchPlaylists),
            PaneRows::Tracks(&data.search_playlist_tracks),
        ),
        PaneId::SearchPeople => ("People".into(), PaneRows::People(&data.search_people)),
        PaneId::SearchPeopleTracks => ("tracks".into(), PaneRows::Tracks(&data.search_people_tracks)),
        PaneId::SearchPeopleLikes => ("liked".into(), PaneRows::Tracks(&data.search_people_likes_tracks)),
        PaneId::Feed => ("activity".into(), PaneRows::Feed(&data.feed)),
        PaneId::FeedTracks => (child_of(PaneId::Feed), PaneRows::Tracks(&data.feed_tracks)),
    };
    (title, rows)
}

/// Inner width from which a tracks pane shows all five columns, and four.
pub const TRACK_COLS_5: u16 = 72;
pub const TRACK_COLS_4: u16 = 44;

/// Header and width of each text column for a pane `inner` cells wide. Columns drop
/// as the pane narrows (tracks: streams first, then artist).
fn text_columns(kind: PaneKind, inner: u16) -> Vec<(&'static str, Constraint)> {
    use Constraint::{Fill, Length};
    match kind {
        PaneKind::Tracks if inner >= TRACK_COLS_5 => {
            vec![("Title", Fill(2)), ("Artist(s)", Fill(1)), ("Time", Length(8)), ("Streams", Length(8))]
        }
        PaneKind::Tracks if inner >= TRACK_COLS_4 => {
            vec![("Title", Fill(2)), ("Artist(s)", Fill(1)), ("Time", Length(8))]
        }
        PaneKind::Tracks => vec![("Title", Fill(1)), ("Time", Length(8))],
        PaneKind::Playlists if inner >= 44 => {
            vec![("Name", Fill(1)), ("Songs", Length(6)), ("Duration", Length(8))]
        }
        PaneKind::Playlists => vec![("Name", Fill(1)), ("Songs", Length(5))],
        PaneKind::Albums if inner >= 50 => vec![
            ("Title", Fill(2)),
            ("Artist(s)", Fill(1)),
            ("Year", Length(4)),
            ("Songs", Length(5)),
            ("Duration", Length(8)),
        ],
        PaneKind::Albums => vec![("Title", Fill(3)), ("Artist(s)", Fill(2))],
        PaneKind::People => vec![("Name", Fill(1))],
        PaneKind::Feed if inner >= 44 => {
            vec![("User", Fill(1)), ("Action", Length(6)), ("Media", Length(8)), ("Age", Length(5))]
        }
        PaneKind::Feed => vec![("User", Fill(1)), ("Action", Length(6)), ("Age", Length(5))],
    }
}

/// Row `i`'s text for `n` text columns, in the order `text_columns` lists them.
fn texts(rows: &PaneRows, i: usize, n: usize) -> Vec<String> {
    let s = |s: &str| s.to_string();
    match rows {
        PaneRows::Tracks(r) => {
            let t = &r[i];
            match n {
                4 => vec![s(&t.title), s(&t.artists), s(&t.duration), s(&t.playback_count)],
                3 => vec![s(&t.title), s(&t.artists), s(&t.duration)],
                _ => vec![s(&t.title), s(&t.duration)],
            }
        }
        PaneRows::Playlists(r) => {
            let p = &r[i];
            let mut out = vec![s(&p.title), s(&p.track_count)];
            if n == 3 {
                out.push(s(&p.duration));
            }
            out
        }
        PaneRows::Albums(r) => {
            let a = &r[i];
            match n {
                5 => vec![s(&a.title), s(&a.artists), s(&a.release_year), s(&a.track_count), s(&a.duration)],
                _ => vec![s(&a.title), s(&a.artists)],
            }
        }
        PaneRows::People(r) => vec![s(&r[i].name)],
        PaneRows::Feed(r) => {
            let a = &r[i];
            match n {
                4 => vec![s(&a.user), s(a.action), s(a.media), a.age()],
                _ => vec![s(&a.user), s(a.action), a.age()],
            }
        }
    }
}

/// Whether row `i` is one the user has liked / follows (Classic's Search heart column).
fn liked(rows: &PaneRows, i: usize, data: &AppData) -> bool {
    match rows {
        PaneRows::Tracks(r) => data.liked_track_urns.contains(&r[i].track_urn),
        PaneRows::Playlists(r) => data.liked_playlist_uris.contains(&r[i].tracks_uri),
        PaneRows::Albums(r) => data.liked_album_uris.contains(&r[i].tracks_uri),
        PaneRows::People(r) => data.followed_user_urns.contains(&r[i].urn),
        PaneRows::Feed(_) => false,
    }
}

/// The `n / total` shown on a pane's bottom border.
pub(super) fn counter(cursor: usize, len: usize) -> String {
    if len == 0 { " 0 ".to_string() } else { format!(" {} / {} ", cursor.min(len - 1) + 1, len) }
}

/// Draw pane `id` in `area`. The focused pane gets the accent border, a bold title and
/// the selection bar; the others are dimmed and mark their selected row with `›`. The
/// playing track's row carries `▶` (`⏸` while paused). Only the rows on screen are
/// built, so a pane costs the same however long its list is; the pane's `TableState`
/// keeps the scroll offset between frames.
#[allow(clippy::too_many_arguments)]
pub fn draw_pane(
    frame: &mut Frame,
    area: Rect,
    id: PaneId,
    focused: bool,
    style: PaneStyle,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
) {
    let area = area.intersection(frame.area());
    if area.width < 4 || area.height < 3 {
        if focused {
            state.page_rows = 1;
        }
        return;
    }
    let t = theme::current();
    let inner_w = area.width - 2;
    let inner_h = area.height - 2;
    // The header row goes first when there is only room for one line.
    let has_header = inner_h >= 2;
    let body = (inner_h - has_header as u16) as usize;
    let kept_offset = nav::table(data, id).offset();
    let filter_text = (!style.boxed_title && nav::filter_pane(state) == Some(id) && state.search_popup_visible)
        .then(|| format!("  /{}{}", state.search_query, if state.search_typing { "▏" } else { "" }));
    let filtering = is_filter_active(state) && nav::filter_pane(state) == Some(id);

    let shared: &AppData = data;
    let (title, rows) = pane_view(id, state, shared, views);
    // A library list (or the feed) whose first page is still on its way.
    let loading = match id {
        PaneId::Likes => Some(List::Likes),
        PaneId::Playlists => Some(List::Playlists),
        PaneId::Albums => Some(List::Albums),
        PaneId::Following => Some(List::Following),
        PaneId::Feed => Some(List::Feed),
        _ => None,
    }
    .is_some_and(|list| shared.loading[list as usize]);
    let len = rows.len();
    let cursor = nav::cursor(state, id).min(len.saturating_sub(1));
    // Keep the scroll where it was unless the cursor left the window.
    let mut offset = kept_offset.min(len.saturating_sub(body));
    if cursor < offset {
        offset = cursor;
    } else if cursor >= offset + body {
        offset = cursor + 1 - body;
    }

    // Classic keeps the Search tab's heart column on the result lists.
    let hearts = style.boxed_title
        && matches!(
            id,
            PaneId::SearchTracks | PaneId::SearchAlbums | PaneId::SearchPlaylists | PaneId::SearchPeople
        );
    let columns = text_columns(id.kind(), inner_w);
    let lead = 1 + hearts as usize;
    let mut headers = vec![""];
    let mut constraints = vec![Constraint::Length(1)];
    if hearts {
        headers.push("♥");
        constraints.push(Constraint::Length(1));
    }
    headers.extend(columns.iter().map(|c| c.0));
    constraints.extend(columns.iter().map(|c| c.1));
    // The same split `Table` makes, so text is cut to exactly the column it lands in.
    let widths = Layout::horizontal(constraints.clone()).spacing(1).split(Rect::new(0, 0, inner_w, 1));

    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let is_tracks = id.kind() == PaneKind::Tracks;
    let table_rows: Vec<Row> = (offset..(offset + body).min(len))
        .map(|i| {
            let selected = i == cursor;
            let track = match &rows {
                PaneRows::Tracks(r) => Some(&r[i]),
                _ => None,
            };
            let playing = track.is_some_and(|tr| np.has_track && tr.track_urn == np.track.track_urn);
            let unplayable = track.is_some_and(|tr| !tr.is_playable());
            let bar = selected && focused;
            let base = if bar && unplayable {
                Style::new().bg(t.dim).fg(t.muted)
            } else if bar {
                Style::new().bg(t.selection_bg).fg(t.selection_fg).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::new().fg(t.muted).add_modifier(Modifier::BOLD)
            } else if unplayable || !focused {
                Style::new().fg(t.dim)
            } else {
                Style::new().fg(t.fg)
            };
            let marker = match (playing, np.playing) {
                (true, true) => "▶",
                (true, false) => "⏸",
                _ if selected && !focused => "›",
                _ => "",
            };
            let mut cells = vec![Cell::from(marker).style(if playing && !bar { accent } else { base })];
            if hearts {
                cells.push(Cell::from(if liked(&rows, i, shared) { "♥" } else { "" }));
            }
            for (k, text) in texts(&rows, i, columns.len()).into_iter().enumerate() {
                let cell = Cell::from(fit(&text, widths[lead + k].width as usize));
                cells.push(if k == 0 && playing && !bar {
                    cell.style(accent)
                } else if k > 0 && is_tracks && focused && !selected && !unplayable {
                    // Artist, time and streams sit back from the title.
                    cell.style(Style::new().fg(t.muted))
                } else {
                    cell
                });
            }
            Row::new(cells).style(base)
        })
        .collect();

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { t.accent } else { t.dim }))
        .title_bottom(Line::styled(counter(cursor, len), Style::new().fg(t.dim)).right_aligned());
    let title_style = if focused { accent } else { Style::new().fg(t.muted) };
    if !style.boxed_title {
        let text = format!("{title}{}", filter_text.unwrap_or_default());
        block = block.title(Line::styled(format!(" {} ", fit(&text, inner_w.saturating_sub(4) as usize)), title_style));
    } else if matches!(
        id,
        PaneId::FollowingTracks
            | PaneId::FollowingLikes
            | PaneId::SearchPeopleTracks
            | PaneId::SearchPeopleLikes
            | PaneId::Feed
    ) {
        block = block.title(Line::styled(title, title_style).centered());
    }

    let mut table = Table::new(table_rows, constraints).column_spacing(1).block(block);
    if has_header {
        let header_style = if focused {
            Style::new().fg(t.secondary).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(t.dim)
        };
        table = table.header(Row::new(headers).style(header_style));
    }
    frame.render_widget(table, area);
    if len == 0 {
        let y = area.y + 1 + has_header as u16;
        let text = if filtering { "no matches" } else if loading { "loading…" } else { "empty" };
        put(frame, Rect::new(area.x + 2, y, inner_w.saturating_sub(2), 1), Line::styled(text, Style::new().fg(t.dim)));
    }

    *nav::table(data, id).offset_mut() = offset;
    if focused {
        state.page_rows = body.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, text};

    fn likes_at(width: u16) -> String {
        let (mut state, mut data, views, np) = fixture();
        go(&mut state, &mut data, 0, 0, 0);
        text(&draw_buffer(width, 12, |f| {
            draw_pane(f, f.area(), PaneId::Likes, true, PaneStyle { boxed_title: false }, &mut state, &mut data, &views, &np)
        }))
    }

    #[test]
    fn track_columns_drop_with_width() {
        // The pane's inner width is two less than its area.
        let five = likes_at(TRACK_COLS_5 + 2);
        assert!(five.contains("Streams") && five.contains("Artist(s)") && five.contains("Time"), "{five}");
        let four = likes_at(TRACK_COLS_5 + 1);
        assert!(!four.contains("Streams") && four.contains("Artist(s)") && four.contains("Time"), "{four}");
        let four = likes_at(TRACK_COLS_4 + 2);
        assert!(four.contains("Artist(s)"), "{four}");
        let three = likes_at(TRACK_COLS_4 + 1);
        assert!(!three.contains("Artist(s)") && three.contains("Title") && three.contains("Time"), "{three}");
    }

    /// S2: the app starts before any list is in; until a list's first page lands its
    /// pane says so rather than claiming to be empty.
    #[test]
    fn an_unfetched_list_reads_loading_not_empty() {
        use crate::tui::render::testkit::{fixture_empty, locations, render_text};
        let (mut state, _, views, np) = fixture_empty();
        let mut data = AppData::loading();
        let mut likes = |data: &mut AppData| {
            text(&draw_buffer(40, 8, |f| {
                draw_pane(f, f.area(), PaneId::Likes, true, PaneStyle { boxed_title: false }, &mut state, data, &views, &np)
            }))
        };
        let before = likes(&mut data);
        assert!(before.contains("loading…") && !before.contains("empty"), "{before}");
        data.loading[List::Likes as usize] = false;
        let after = likes(&mut data);
        assert!(after.contains("empty") && !after.contains("loading…"), "{after}");

        // The whole app draws at every location with nothing fetched yet.
        let mut data = AppData::loading();
        for (tab, sub, focus) in locations() {
            go(&mut state, &mut data, tab, sub, focus);
            assert!(!render_text(120, 40, &mut state, &mut data, &views, &np).trim().is_empty());
        }
    }

    #[test]
    fn playing_row_has_marker() {
        let (mut state, mut data, views, mut np) = fixture();
        go(&mut state, &mut data, 0, 0, 0);
        let title = np.track.title.clone();
        let mut draw = |np: &NowPlaying| {
            text(&draw_buffer(100, 14, |f| {
                draw_pane(f, f.area(), PaneId::Likes, true, PaneStyle { boxed_title: false }, &mut state, &mut data, &views, np)
            }))
        };
        let playing = draw(&np);
        assert!(playing.contains(&format!("▶ {title}")), "{playing}");
        np.playing = false;
        let paused = draw(&np);
        assert!(paused.contains(&format!("⏸ {title}")) && !paused.contains('▶'), "{paused}");
        np.has_track = false;
        let stopped = draw(&np);
        assert!(!stopped.contains('▶') && !stopped.contains('⏸'), "{stopped}");
    }

    #[test]
    fn counter_and_page_rows_follow_the_cursor() {
        let (mut state, mut data, views, np) = fixture();
        go(&mut state, &mut data, 0, 0, 0);
        nav::move_by(&mut state, &mut data, &views, 41);
        let screen = text(&draw_buffer(80, 12, |f| {
            draw_pane(f, f.area(), PaneId::Likes, true, PaneStyle { boxed_title: false }, &mut state, &mut data, &views, &np)
        }));
        assert!(screen.contains(&format!("42 / {}", data.likes.len())), "{screen}");
        assert!(screen.contains(&data.likes[41].title), "the cursor row scrolled out of view:\n{screen}");
        // 12 rows less two borders and the header.
        assert_eq!(state.page_rows, 9);
    }

    #[test]
    fn filtered_pane_draws_from_views_and_empty_panes_say_so() {
        let (mut state, mut data, mut views, np) = fixture();
        go(&mut state, &mut data, 0, 0, 0);
        state.search_popup_visible = true;
        state.search_query = "only".into();
        views.likes = vec![data.likes[7].clone()];
        let mut draw = |state: &mut AppState, views: &FilteredViews| {
            text(&draw_buffer(80, 12, |f| {
                draw_pane(f, f.area(), PaneId::Likes, true, PaneStyle { boxed_title: false }, state, &mut data, views, &np)
            }))
        };
        let screen = draw(&mut state, &views);
        assert!(screen.contains("1 / 1") && screen.contains("/only"), "{screen}");
        views.likes.clear();
        let screen = draw(&mut state, &views);
        assert!(screen.contains("no matches"), "{screen}");
    }
}
