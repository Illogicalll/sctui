# sctui UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five runtime-switchable layouts over one navigation model and one vim-style keymap, with hint line, toasts, playing marker, command palette, responsive rules at every terminal size, and a first-run setup wizard.

**Architecture:** A new `nav` module is the single place that maps (location, focused pane) to a cursor, a list and a `TableState`; all input goes through it and all layouts draw from it. Existing data slots in `AppData` and the fetch-on-select machinery stay; the old per-pane cursor fields stay as storage but are only reached through `nav`. Rendering is split into shared widgets (`render/widgets/`) and one file per layout (`render/layouts/`), each a pure function of state and terminal size.

**Tech Stack:** Rust 2024, ratatui 0.29, crossterm, serde/toml. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-10-ui-redesign-design.md`. Behavioural reference: `docs/superpowers/specs/2026-11-10-ui-redesign-prototype.html` (open in a browser; do not port its code).

## Global Constraints

- No new crates. Reuse ratatui `Table`/`TableState`, existing `truncate_with_ellipsis`, `styled_header`, `centered_rect*`, `theme::current()`.
- Repo is not rustfmt-clean on purpose. Do not run `cargo fmt`; do not reformat lines you are not changing.
- Never read or touch `.env`, `token.json`, `worker/.dev.vars`. Never run the app (needs login and audio); verify with `cargo check`, `cargo test`, and `TestBackend` renders.
- Every task ends with `cargo check` warning count not above the count at task start, and `cargo test` green.
- Rendering must never panic at any size from 20×6 to 240×80, with any overlay open (Task 4's test; every later task extends it).
- All selection-based actions act on the focused pane's selected row only. Container actions (delete playlist, like/follow container, artist station) require a container row focused; they never fall through from a tracks pane.
- `h`/`l`/Left/Right move sub-tabs only (wrap). `[`/`]` move pane focus only (no wrap). Esc never quits.
- Thresholds (columns × rows), tunable by a few cells: too small < 36×8; panes side by side 3/2/1 at pane-row width ≥104 / ≥64 / below; track columns 5/4/3 at pane inner width ≥72 / ≥44 / below; player bar 7 rows (Classic only) at rows ≥30, 4 rows at rows ≥20, else 1 line; hint line hidden at rows <14; Classic compact header at rows <24; Sidebar nav column hidden unless focused at columns <90; docked queue at columns ≥120; Columns window 3/2/1 at columns ≥100 / ≥60 / below; Columns track card at columns ≥124; Player panel falls back to Tighten at columns <110; panel cover height 9/6/3/0 at panel rows ≥34 / ≥26 / ≥20 / below.
- Commit after each task on branch `feat/ui-redesign`, conventional-commit subject, ending the message with `Co-authored-by: Claude <noreply@anthropic.com>`. Do not push.

## Scoped decisions made while planning (deviations from the spec's wording)

- **Filter stays one-per-location**, on the pane that is filterable today (Likes tracks, playlist tracks, Albums list, Following list). `/` in Library focuses that pane and edits its filter. Search and Feed have no list filter. The spec's "focused pane's filter" is narrowed to this.
- **Old cursor fields are kept as storage** behind `nav`; they are not deleted in this plan.

## Review Focus

1. Overlay open while the terminal is resized below 36×8: must show the too-small screen, not panic, and the overlay must still be there after resizing back. (Task 4 test.)
2. Empty lists everywhere (new account): every layout and the wizard must render and every action must be a no-op, not an index panic. (Task 4 and Task 11 tests.)
3. Wide and non-ASCII titles (CJK, emoji): truncation must not split a char or overflow a pane border. (Task 4 test.)
4. A config with `[keys]` entries for removed actions or a chord that is a prefix of another binding: startup warns, nothing panics, the rest of the map loads. (Task 3 tests.)
5. A list changing under an active filter or an open confirm (unlike, new page, playlist created): the highlighted row and the acted-on row stay the same item. (Task 2 tests.)

---

## File structure

| File | Responsibility |
|---|---|
| `src/tui/logic/nav.rs` (new) | `PaneId`, `PaneKind`, location → panes, focus, cursor access, movement, table-state sync, nav column |
| `src/tui/logic/toast.rs` (new) | `Toast`, `ToastKind`, push/expire |
| `src/tui/logic/palette.rs` (new) | palette items and fuzzy match |
| `src/tui/logic/wizard.rs` (new) | wizard stages, lesson steps, action observation |
| `src/tui/logic/input/*` | rewired through `nav`; new `palette.rs`, `wizard.rs` handlers; `movement.rs` shrinks |
| `src/keymap.rs` | actions, chord sequences, defaults |
| `src/config.rs` | `Layout`, new settings, parse warnings |
| `src/tui/render/widgets/{pane,pane_row,player,hints,toasts,nav_column}.rs` (new) | shared drawing |
| `src/tui/render/layouts/{mod,classic,tighten,sidebar,columns,panel}.rs` (new) | one pure function per layout |
| `src/tui/render/overlays/{palette,wizard}.rs` (new) | popups |
| `src/tui/render/tabs/*`, `render/now_playing.rs` | deleted once Classic is drawn by `layouts/classic.rs` |
| `src/tui/render/testkit.rs` (new, `#[cfg(test)]`) | fixtures and the render-everything harness |

---

### Task 1: Fix-first bugs

**Files:** `src/tui/render/overlays/confirm.rs`, `src/keymap.rs`, `src/config.rs`, `src/main.rs`, `src/tui/logic/mod.rs`, `src/tui/logic/state.rs`, `src/tui/logic/input/confirm.rs`, `src/tui/logic/input/commands.rs`

- [x] **T1 clamp.** Test `confirm_popup_does_not_panic_on_narrow_terminal`: render `render_confirm` on `TestBackend` at 30×8, 10×4, 1×1 with a 60-char message; no panic. Fix `confirm.rs:15` to `.max(40).min(frame.area().width.max(1))`.
- [x] **#3 TOML escaping.** Test `keys_section_round_trips_quote_and_backslash`: bind `"` and `\` to an action, write `keys_section`, parse it back with `apply_toml`, same chords, no warnings. Write each spec with `toml::Value::String(spec).to_string()`. Same treatment for `[theme.colors]` keys and values in `config.rs`.
- [x] **#4 silent reset.** Test `bad_settings_type_is_reported_not_silently_defaulted`: `crossfade = "yes"` produces a warning string containing `settings`. In `config.rs` load, `match toml::from_str::<File>` and push the error into `warnings`; on failure, set a `load_failed` flag carried in the loaded config so in-app saves are refused (message via `help_message`) until the file parses.
- [x] **#5 re-auth duplication.** In `src/main.rs` reauth success path, replace only the token (`*token_clone.lock().unwrap() = new_token`), do not rebuild `API`. Confirm by reading `API::init` that the token `Arc` is shared; if `API` caches anything token-derived, refresh that only.
- [x] **#7 playlist edit ordering.** In `tui/logic/mod.rs`, drain `playlist_edit_queue` through one long-lived task fed by an unbounded channel so edits run strictly in order; send each result back as a new `Msg::PlaylistEditFailed(String)` on error (handled in Task 5; for now store in `help_message`).
- [x] **T3 confirm identity.** `ConfirmAction::RemoveTrack { tracks_uri: String, track: Track }`, `DeletePlaylist { tracks_uri: String }`; `message` and `perform` resolve the playlist by `tracks_uri` at call time and do nothing if it is gone. Test `confirm_targets_playlist_by_identity_after_insert`: open confirm for playlist B, insert a new playlist at index 0, perform, B is the one removed.
- [x] `cargo test`, `cargo check`, commit `fix: crash and data-loss bugs ahead of UI redesign`.

### Task 2: Navigation model, modes, modal overlays

**Files:** create `src/tui/logic/nav.rs`; modify `src/tui/logic/state.rs`, `src/tui/logic/mod.rs`, `src/tui/logic/filtering.rs`, all of `src/tui/logic/input/*`, `src/keymap.rs` (actions only)

**Produces:**

```rust
// nav.rs
pub enum PaneKind { Tracks, Playlists, Albums, People, Feed }
pub enum PaneId { Likes, Playlists, PlaylistTracks, Albums, AlbumTracks, Following, FollowingTracks, FollowingLikes,
                  SearchTracks, SearchAlbums, SearchAlbumTracks, SearchPlaylists, SearchPlaylistTracks,
                  SearchPeople, SearchPeopleTracks, SearchPeopleLikes, Feed, FeedTracks }
impl PaneId { pub fn kind(self) -> PaneKind; pub fn is_container(self) -> bool }
pub fn panes(state: &AppState) -> &'static [PaneId];          // by (selected_tab, sub-tab)
pub fn focus(state: &AppState) -> usize;                       // clamped index into panes()
pub fn focused(state: &AppState) -> PaneId;
pub fn set_focus(state: &mut AppState, idx: usize);
pub fn focus_left(state: &mut AppState) -> bool;               // false at edge
pub fn focus_right(state: &mut AppState) -> bool;
pub fn cursor(state: &AppState, id: PaneId) -> usize;
pub fn len(state: &AppState, data: &AppData, views: &FilteredViews, id: PaneId) -> usize;   // filtered length
pub fn move_by(state: &mut AppState, data: &mut AppData, views: &FilteredViews, delta: isize);
pub fn move_to(state: &mut AppState, data: &mut AppData, views: &FilteredViews, end: Edge); // Edge::{Top,Bottom}
pub fn sync_tables(state: &AppState, data: &mut AppData);      // every TableState.select(cursor) once per loop
pub fn selected_track(state: &AppState, data: &AppData, views: &FilteredViews) -> Option<Track>; // focused Tracks pane
pub fn set_location(state: &mut AppState, tab: usize, sub: Option<usize>); // saves/restores per-location primary cursor
```

`AppState` gains `focus: [[usize; 4]; 3]`, `primary_rows: [[usize; 4]; 3]`, `page_rows: usize` (visible rows of the focused pane, written by render, default 10). `nav` keeps the legacy flags (`following_tracks_focus`, `search_people_tracks_focus`, `info_pane_selected`) in step with focus so untouched handlers keep working; nothing outside `nav` writes them.

- [x] **Tests first** (in `nav.rs`, using `AppData` built by a new `AppData::empty()` plus pushes): `panes_for_every_location` (the spec's table); `focus_is_per_location_and_clamped`; `bracket_focus_stops_at_edges`; `move_by_only_moves_focused_pane`; `tab_switch_restores_cursor_and_table_offset` (T4: Likes row 300, sub-tab right then left, cursor 300 and `likes_state.selected()==Some(300)`); `filter_closes_on_tab_switch` (T5); `filter_matches_follow_list_changes` (T6: filter playlist tracks, replace the list, matches rebuilt); `artist_station_uses_filtered_row` (T7); `like_on_track_pane_targets_track_not_parent`; `container_action_ignored_from_tracks_pane` (delete playlist with tracks pane focused does nothing).
- [x] Implement `nav.rs`. Cursor storage: primary pane of a location → `selected_row`; child panes → their existing `*_row` fields; Feed tracks → `selected_info_row`.
- [x] `keymap.rs`: add `Action::{PaneLeft, PaneRight, Top, Bottom, HalfPageDown, HalfPageUp}` (config names `pane_left`, `pane_right`, `top`, `bottom`, `half_page_down`, `half_page_up`); remove `SecondaryUp/Down`, `TertiaryUp/Down`. Temporary defaults: `[`, `]`, `home`, `end`, `ctrl+d`, `ctrl+u` (Task 3 sets the final table).
- [x] Rewire `input/mod.rs::run_action`: Up/Down/PageUp/PageDown/HalfPage*/Top/Bottom → `nav`; held-key ramp keeps working with one `Lane`. Delete the secondary/tertiary paths in `movement.rs` and `commands.rs`. Sub-tab and tab switches go through `nav::set_location`.
- [x] Selection actions resolve from focus: `PlaySelected` on a container pane = `focus_right`; on Tracks = play. `ToggleLike` on Tracks = like/unlike that track (new `Engagement::LikeTrack`/`UnlikeTrack` path for any pane), on container = existing container like/follow. `AddToQueue`/`PlayNext`/`AddToPlaylist`/`RemoveFromPlaylist` need a Tracks pane. `DeletePlaylist` needs `PaneId::Playlists` focused. `StartStation` = track or focused artist row. `NewPlaylist` works anywhere.
- [x] Modes in `handle_key_event`: (1) any overlay visible → that overlay's handler consumes every key (no `Option` fall-through; unhandled keys are dropped); queue popup gets a handler (movement scrolls, Esc/its key closes). (2) insert mode (`search_typing`, `search_popup_visible`, playlist-name prompt): every printable char types, incl. capitals; Enter/Esc/Backspace/Ctrl+W/Ctrl+U only. (3) visualiser: Tab cycles; selection actions are no-ops. (4) normal.
- [x] `Action::Quit` default key becomes `q` (temporary, Task 3 finalises); Esc in normal mode clears the active Library filter and otherwise does nothing.
- [x] In the main loop, call `filtering::rebuild_matches_if_active` then `nav::sync_tables` once per iteration before render.
- [x] `cargo test`, `cargo check`, commit `feat: single focus-based navigation model and modal input`.

### Task 3: Keymap with sequences and the new defaults

**Files:** `src/keymap.rs`, `src/config.rs`, `src/tui/logic/input/mod.rs`, `src/tui/logic/input/help_editor.rs`, `src/tui/render/overlays/help.rs`, `src/tui/logic/state.rs`

**Produces:**

```rust
pub struct Seq(pub Vec<Chord>);                         // 1..=3 chords; spec() joins with ' '
pub enum KeyResult { Action(Action), Pending, None }
impl Keymap { pub fn feed(&self, pending: &mut Vec<Chord>, key: &KeyEvent) -> KeyResult;
              pub fn labels(&self, a: Action) -> String; pub fn first_label(&self, a: Action) -> String; }
```

`AppState` gains `pending_keys: Vec<Chord>`, `pending_since: Option<Instant>` (cleared after 1 s by the loop).

- [x] Tests: `gg_is_a_sequence` (`g` → Pending, `g` → `Action::Top`); `pending_falls_back_to_single_key` (`g` then `j` → `Action::Down`); `defaults_have_no_duplicate_or_prefix_conflicts`; `config_sequence_parses` (`top = ["g g", "home"]`); `removed_actions_warn` (`secondary_up = "K"` → warning naming it, rest loads); `help_and_quit_cannot_be_fully_unbound`.
- [x] Add actions `TabLibrary`, `TabSearch`, `TabFeed` (`tab_library`, `tab_search`, `tab_feed`; Feed is a no-op when hidden).
- [x] Set `DEFAULTS` to exactly the spec's keymap table (omit `:`/Ctrl+P and `o` until Tasks 7 and 10 add those actions). Capital letter = Shift. `shift+enter` stays as a second binding for station.
- [x] `help_editor.rs`: capture one chord, or a sequence when the user presses a second key within 1 s; refuse to remove the last binding of `Help` or `Quit` with a message.
- [x] `cargo test`, commit `feat: vim-style default keymap with key sequences`.

### Task 4: Shared widgets, Classic as a skin, responsive rules, render-everything test

**Files:** create `src/tui/render/widgets/{mod,pane,pane_row,player}.rs`, `src/tui/render/layouts/{mod,classic}.rs`, `src/tui/render/testkit.rs`; modify `src/tui/render/mod.rs`, `src/tui/logic/mod.rs`, overlays; delete `src/tui/render/tabs/*`, `src/tui/render/now_playing.rs` when unused

**Produces:**

```rust
// render/mod.rs
pub struct NowPlaying { pub track: Track, pub volume: f32, pub playing: bool, pub has_track: bool }
pub fn render(frame: &mut Frame, state: &mut AppState, data: &mut AppData, views: &FilteredViews,
              np: &NowPlaying, cover: Option<&mut ThreadProtocol>, wave: &Arc<Mutex<VecDeque<f32>>>);
// widgets/pane.rs
pub enum PaneRows<'a> { Tracks(&'a [Track]), Playlists(&'a [Playlist]), Albums(&'a [Album]), People(&'a [Artist]), Feed(&'a [Activity]) }
pub struct PaneStyle { pub boxed_title: bool }   // Classic vs modern title placement
pub fn pane_view<'a>(id: PaneId, state: &AppState, data: &'a AppData, views: &'a FilteredViews) -> (String /*title*/, PaneRows<'a>);
pub fn draw_pane(frame: &mut Frame, area: Rect, id: PaneId, focused: bool, style: PaneStyle,
                 state: &mut AppState, data: &mut AppData, views: &FilteredViews, playing_urn: Option<&str>);
// widgets/pane_row.rs
pub fn visible_window(n: usize, focus: usize, first: usize, max: usize) -> (usize, usize);
pub fn draw_pane_row(frame, area, state, data, views, style, weights: Option<&[u16]>, playing_urn);
// widgets/player.rs
pub enum PlayerSize { Classic7, Bar4, Line1 }
pub fn player_size(layout_is_classic: bool, rows: u16) -> PlayerSize;
pub fn draw_player(frame, area, size, state, np, cover);
// layouts/mod.rs
pub fn draw(frame, state, data, views, np, cover);   // dispatches on state.settings.layout (Task 6); Classic only for now
```

- [x] `testkit.rs`: `fixture() -> (AppState, AppData, FilteredViews, NowPlaying)` with ~60 likes, 5 playlists, 4 albums, 6 people, 5 feed items, including one CJK title, one emoji title and one 200-char title; `fixture_empty()`; `render_text(w, h, &mut state, &mut data, ...) -> String`; `for_all_sizes(f)` over widths `[20,36,40,59,60,63,64,79,80,89,90,99,100,103,104,109,110,119,120,123,124,160,240]` × heights `[6,8,13,14,19,20,23,24,29,30,33,34,45,80]`.
- [x] Tests: `every_location_renders_at_every_size` (all 3 tabs × all sub-tabs × each focus index × both fixtures; assert no panic and that the focused pane's selected title is on screen when size ≥ 36×8); `every_overlay_renders_at_every_size` (quit, confirm, help ×3 pages, theme picker, playlist picker ×2 modes, queue, history); `too_small_shows_message`; `playing_row_has_marker` (`▶`, `⏸` when paused); `unfocused_pane_has_no_selection_bar`; `pane_count_follows_width` (Following at 110 → 3 panes, 80 → 2, 50 → 1 and the focused one is visible); `track_columns_drop_with_width`; `visible_window` unit cases from the prototype's `window_` function; `player_size_thresholds`.
- [x] Implement widgets. Pane = ratatui `Table` in a rounded `Block`; focused: accent border, bold title, `selection_bg` bar; unfocused: `dim` text, no bar, `›` in the first cell of its selected row. Position counter `n / total` bottom-right. Write the body height to `state.page_rows` for the focused pane. Classic style keeps centred titles (`tracks`, `liked`, `activity`) and its width ratios via `weights`.
- [x] `layouts/classic.rs`: boxed tabs, boxed sub-tabs, search/filter boxes, pane row, player (7/4/1), sine only at columns ≥ 90, compact header rows at rows < 24, Search heart column kept.
- [x] Main loop builds `NowPlaying` from `Player` and passes `Some(&mut cover)`; overlays use `centered_rect_fixed`-style clamping everywhere.
- [x] `cargo test`, commit `feat: shared pane/player widgets, responsive Classic, render-at-every-size test`.

### Task 5: Hint line and toasts

**Files:** create `src/tui/logic/toast.rs`, `src/tui/render/widgets/{hints,toasts}.rs`; modify `state.rs`, `logic/mod.rs`, input handlers, `config.rs`, `layouts/classic.rs`

**Produces:** `pub enum ToastKind { Ok, Err }`; `impl AppState { pub fn toast(&mut self, msg: impl Into<String>); pub fn toast_err(&mut self, msg: impl Into<String>); }`; `toast::expire(&mut Vec<Toast>, now)`; `hints::pairs(state, data) -> Vec<(String, &'static str)>`; `hints::draw(frame, area, state, data)`; `Settings.hint_line: bool` (default true, row on the Settings page).

- [x] Tests: `at_most_three_toasts`; `ok_expires_after_3s_err_after_6s` (inject `Instant`); `toast_box_clamps_to_width`; `hints_change_with_focus_kind` (tracks pane lists play/queue/like; playlists pane lists open/new/delete); `hints_use_live_keymap` (rebind queue to `z`, hint shows `z`); `hints_trim_to_width_without_splitting_a_pair`; `pending_keys_shown_right_aligned`; `hint_line_hidden_under_14_rows`.
- [x] Toast call sites: like/unlike, follow/unfollow, add to queue (`Added to queue · N`), play next, add to playlist, remove, create, delete, shuffle/repeat, station start, theme kept; errors from `Msg::PlaylistEditFailed`, engagement failures (send a failure `Msg` and roll back the optimistic set: T11), and fetch failures.
- [x] Draw order: layout, overlays, toasts (top-right, stacked down from row 1), hint line last row.
- [x] Extend the render-everything test with 3 toasts present and with `hint_line` off.
- [x] Commit `feat: context hint line and toasts`.

### Task 6: Layout setting and Tighten

**Files:** `src/config.rs`, `src/tui/render/layouts/{mod,tighten}.rs`, `src/tui/render/overlays/help.rs`, `src/tui/logic/input/help_editor.rs`

**Produces:** `#[serde(rename_all = "kebab-case")] pub enum Layout { Classic, Tighten, Sidebar, Columns, PlayerPanel }` with `ALL`, `label()`, `next()/prev()`; `Settings.layout` (default `Classic`), `Settings.dock_queue: bool`, `Settings.onboarded: bool`; `SettingRow::Choice` for Layout (Left/Right cycles); `widgets::header::draw_compact(frame, area, state, data) -> u16` (2 rows: `sctui │ 1 Library 2 Search 3 Feed`, then sub-tab pills with filter/query right-aligned; also used by Classic's compact mode and by Player panel).

- [x] Tests: `layout_round_trips_in_toml`; `unknown_layout_warns_and_defaults`; `settings_row_cycles_layout`; `tighten_chrome_is_at_most_seven_rows_at_120x32`; render-everything test now loops over `Layout::ALL` (unimplemented layouts dispatch to Tighten until their task lands).
- [x] Tighten: compact header, pane row, 4-row or 1-line player, hint line.
- [x] Commit `feat: layout setting and Tighten layout`.

### Task 7: Command palette

**Files:** create `src/tui/logic/palette.rs`, `src/tui/logic/input/palette.rs`, `src/tui/render/overlays/palette.rs`; modify `keymap.rs`, `state.rs`, `input/mod.rs`, `overlays/mod.rs`

**Produces:** `Action::Palette` (`palette`, defaults `:` and `ctrl+p`); `pub enum Command { Action(Action), Layout(Layout), Theme(&'static str), ToggleHints, SetupWizard }`; `pub fn items(state) -> Vec<(String /*label*/, String /*keys*/, Command)>`; `pub fn fuzzy(query: &str, label: &str) -> Option<i32>` (subsequence; bonus for adjacency and word starts); `AppState.palette: Option<PaletteState { query: String, selected: usize }>`.

- [x] Tests: `fuzzy_subsequence_and_ranking` (`"lay sb"` ranks `Layout: sidebar` first; `"zzz"` matches nothing); `empty_query_lists_everything`; `palette_runs_selected_command_and_closes`; `palette_is_insert_mode` (typing `Q` adds `Q`, does not open the queue); `every_action_is_listed_except_movement_and_close`; render-everything with the palette open.
- [x] Commit `feat: command palette`.

### Task 8: Sidebar layout

**Files:** create `src/tui/render/widgets/nav_column.rs`, `src/tui/render/layouts/sidebar.rs`; modify `nav.rs`, `state.rs`, input handlers

**Produces (nav.rs):** `AppState.nav_focused: bool`; `pub struct NavItem { pub label: String, pub tab: usize, pub sub: Option<usize>, pub playlist_uri: Option<String> }`; `pub fn nav_items(state, data, with_playlists: bool) -> Vec<NavItem>`; `pub fn nav_index(state, data, items) -> usize`; `pub fn nav_go(state, data, item)`; `pub fn has_nav(layout) -> bool`; `pub fn first_pane(state) -> usize` (1 on Library › Playlists in Sidebar, else 0).

- [x] Tests: `bracket_left_from_first_pane_focuses_nav_only_in_nav_layouts`; `nav_move_changes_location`; `selecting_playlist_entry_sets_playlists_location_and_hides_list_pane`; `delete_and_like_act_on_nav_playlist_entry`; `nav_hidden_under_90_cols_unless_focused`; `dock_queue_only_from_120_cols`; render-everything for Sidebar incl. `nav_focused`.
- [x] Layout: nav column (26 cols), optional one-row strip (Search pills + query; location name when the column is hidden), pane row, optional docked `Up next` column (display-only), full-width player, hint line.
- [x] Commit `feat: Sidebar layout`.

### Task 9: Columns layout

**Files:** create `src/tui/render/layouts/columns.rs`; modify `widgets/nav_column.rs`

- [x] Tests: `breadcrumb_names_location_and_focused_path`; `window_keeps_focused_column_visible_at_3_2_1`; `track_card_only_from_124_cols_and_tracks_focus`; `track_card_has_no_cover_unless_selected_is_playing`; render-everything for Columns.
- [x] Layout: breadcrumb row, columns = nav column (sections only) + panes through `visible_window`, track card (text: title, artist, duration, streams, applicable keys), player, hint line.
- [x] Commit `feat: Columns layout`.

### Task 10: Player panel layout

**Files:** create `src/tui/render/layouts/panel.rs`; modify `keymap.rs`, `state.rs`, `input/commands.rs`

**Produces:** `Action::PanelPage` (`panel_page`, default `o`); `pub enum PanelPage { UpNext, Lyrics, Spectrum, History }` with `next()`; `AppState.panel_page`.

- [x] Tests: `panel_falls_back_to_tighten_under_110_cols`; `cover_height_thresholds`; `o_cycles_pages`; `q_and_h_still_open_popups_in_panel_layout`; render-everything for PlayerPanel × each page.
- [x] Layout: left = compact header + pane row + hint line; right = panel (min(46, 36% of width)): cover, title, artist · source, progress, state line, page tabs, page body (Up next list reusing the queue popup's row builder; Lyrics reusing `visualizer/lyrics.rs`; Spectrum reusing `visualizer/spectrum.rs`; History list). No bottom player bar.
- [x] Commit `feat: Player panel layout`.

### Task 11: Setup wizard

**Files:** create `src/tui/logic/wizard.rs`, `src/tui/logic/input/wizard.rs`, `src/tui/render/overlays/wizard.rs`; modify `src/main.rs`, `src/config.rs`, `state.rs`, `input/mod.rs`, `palette.rs`

**Produces:** `pub enum Stage { Welcome, Layout, Theme, Practice, Done }`; `pub struct Wizard { pub stage: Stage, pub step: usize, pub selected: usize, pub sub: u8 }`; `pub const LESSONS: [Lesson; 10]` (`goal`, `why`, `action_for_key_label`, completion predicate over observed actions/state); `impl Wizard { pub fn observe(&mut self, action: Action, state_snapshot: &Observed) }`; `AppState.wizard: Option<Wizard>`, `AppState.tour_prompt: bool`; CLI flag `--setup`.

- [x] Tests: `fresh_install_starts_wizard` (no config file → `wizard.is_some()`); `existing_config_without_onboarded_gets_tour_prompt_not_wizard_and_keeps_classic`; `layout_stage_previews_live_and_enter_keeps`; `theme_stage_reverts_on_esc`; `each_lesson_completes_on_its_action_even_when_rebound`; `two_part_lessons_need_both_parts` (bottom then top; filter then clear; focus in then back); `esc_skips_and_sets_onboarded`; `skipped_wizard_defaults_to_tighten`; `empty_library_skips_track_lessons`; `lesson_steps_never_enqueue_engagement_or_playlist_edit`; `card_becomes_two_row_strip_when_narrow`; render-everything with the wizard at each stage.
- [x] Lessons, in order: move down ×3; bottom then top; next sub-tab; focus right then left; play; pause then resume; add to queue; filter then clear; open palette and run a command; open keys popup and close it.
- [x] Practice stage is the real app with a lesson card (goal bold, one-line why, key from `first_label`, `n / 10`) pinned right (or the two-row strip above the hint line under 100 columns). Layout/Theme stages draw the real layout behind a list on the left.
- [x] Finishing or skipping saves `onboarded = true`. Upgrade prompt: one confirm-style popup "Keys have changed. Take the two-minute tour?"; Yes starts at Practice. Palette command `Setup wizard`.
- [x] Commit `feat: first-run setup wizard`.

As built (differences from the wording above): `Wizard` also carries `revert_layout` and `ticked`; `observe` takes `Option<Action>` plus the state before and after the key, because the filter, palette and keys-popup lessons finish on keys that are not actions (typed text, Esc); the palette lesson's second half is `Wizard::ran_command`, called where the palette hands back a command. The card reads keys with `hint_label` (the hint line's short form), not `first_label`. The upgrade prompt's Yes/No cursor reuses `confirm_selected`. During practice Esc first does its normal job (close a popup, leave insert mode, clear the filter, leave the visualiser) and skips the tour only when there is nothing else for it to do. The lesson is a 44×7 card bottom-right from 100×24, otherwise the two-row strip. The focus lesson accepts `]` and `[` in either order (in Sidebar the way "in" starts from the column).

### Task 12: Docs and final verification

- [x] README: replace the Configuration/Keybindings text with the new keymap table, layouts, palette, wizard; note the default keys changed.
- [x] `cargo test`, `cargo check`, `cargo clippy` (no new warnings), `cargo build --release`.
- [x] Walk the spec section by section and tick each requirement against a test or a file; list anything unmet.
- [x] Commit `docs: describe layouts, keymap, palette and setup wizard`.
