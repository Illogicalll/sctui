# sctui UI redesign: switchable layouts, one navigation model, new keymap

Date: 2026-11-10
Status: reviewed, awaiting owner approval

## Why

sctui's screen design is the one it started with. Features added since (three-pane
Following, feed, queue, history, playlists editing, settings, equaliser, themes, lyrics)
were fitted into that first design and now feel crammed:

- 13 of the terminal's rows are chrome before any content (16 on Search).
- Panes have no focus. Pane 1 moves with Up/Down, pane 2 with Shift+Up/Down, pane 3 with
  Alt+Up/Down; which pane an action hits depends on which key was pressed last.
- Nearly every action is Shift+letter and nothing on screen says so.
- Lists do not show what is playing. Actions give no feedback.
- Overlays are not modal; keys leak to the view behind them.
- Capital letters cannot be typed into search or filter because they are bound actions.

The owner wants to choose between several layouts at runtime, wants keys that behave
like other TUIs (neovim, tmux), and wants the app to look right at every terminal size.

## Reference material

Two HTML files sit next to this document. They are the agreed look and behaviour;
where this document and the prototype disagree, this document wins, and where this
document is silent, the prototype is the reference.

- `2026-11-10-ui-redesign-prototype.html`: interactive mock of all five layouts with the
  agreed keymap, breakpoints, hint line, toasts, palette and overlays. Open in a browser,
  click the terminal, use the keyboard. Layout and size buttons are along the top.
- `2026-11-10-ui-redesign-mockups.html`: static pictures of the current UI and the four
  new layouts as first proposed.

The prototype is a throwaway. None of its code is to be ported; it only shows behaviour.

## Goals

1. Five layouts, switchable at runtime and remembered: **Classic** (today's look),
   **Tighten**, **Sidebar**, **Columns**, **Player panel**.
2. One navigation model and one keymap shared by all five.
3. A context hint line.
4. A playing marker (`▶`, `⏸` when paused) on the playing track's row in every track list.
5. Non-intrusive notifications (toasts) for actions and errors.
6. A command palette with fuzzy matching.
7. Every layout looks right from very small to very large terminals.
8. A first-run setup wizard: choose a layout, choose a theme, then practise the important
   keys in a guided sandbox.

## Non-goals

Not part of this work, because the owner did not pick them:

- Liked (heart) markers in Library lists. The owner dislikes the heart glyph. The Search
  tab's existing heart column is left as it is.
- Editable queue (reorder, remove), merging queue and history into one popup.
- Splitting settings out of the `?` popup.
- Sorting lists, session-state restore, a mini visualiser in the player bar.
- Mouse support.
- Any change to audio, API, auth or the visualiser modes themselves.

## Anti-goals

- Four or five copies of navigation or input logic. If a layout needs its own cursor
  rules, the design has failed.
- A layout that looks broken, cramped or clipped at any terminal size. The owner ranks
  proper appearance above faithful recreation of today's screen, Classic included.
- Silent loss of a user's existing `[keys]` overrides.

## Decisions the owner made

- All four proposed layouts plus the original, chosen from Settings.
- Keys may be revised freely and should feel like neovim/tmux.
- `h`/`l` (and Left/Right) move between **sub-tabs** only. `[`/`]` move between **panes**
  only. Neither crosses into the other's level. This was tried in the prototype both ways
  and this is the one the owner kept.
- Hint line: yes. Playing marker: yes. Hearts: no. Toasts: yes. Palette: yes.
  Responsive breakpoints for every layout: yes, and important.
- **Classic is a skin over the shared widgets**, not a pixel-faithful copy. It follows the
  same responsive rules as the other layouts, so at 80×24 it will differ from today
  (4-row player bar, two panes on Following, no sine wave, dimmed unfocused panes).
  Proper appearance matters more than fidelity.
- **The wizard's practice stage uses the real app**: the user's own library after login,
  with real playback. No demo data.
- **`o` (cycle panel page) keeps a default key. Next-layout has no key**: layouts are
  changed from Settings (and the palette's per-layout commands).
- A first-run setup wizard, modelled on tuios.dev/learn.

## Decisions made by the designer (owner has seen them in the prototype; not separately confirmed)

- Classic uses the new keymap and focus model; there is no legacy key scheme.
- Classic gains the hint line (can be turned off), the playing marker, toasts, and an
  accent border on the focused pane.
- Toasts appear top-right, one line each, newest at the bottom, at most three.
  Success toasts last about 3 seconds, errors about 6.
- Enter on a list pane (playlists, albums, people, feed activity) moves focus into the
  next pane. It does not play the whole container.
- `q` quits (with the existing confirm). Esc never quits.
- The layout setting is a row on the existing Settings page of the `?` popup, and each
  layout is also a palette command.
- Sub-tab movement with `h`/`l` wraps at the ends. Pane movement with `[`/`]` does not.
- **Container actions need the container focused.** Delete playlist, like/follow a
  container and station-from-artist act only when a container row has focus. They never
  fall through from a tracks pane to its parent. In Sidebar and Columns the navigation
  column's playlist entries count as playlist rows for these actions, so a playlist is
  deleted, liked or opened from the sidebar entry itself.
- **`Q` and `H` always open the queue and history popups, in every layout.** The Player
  panel's pages and Sidebar's docked queue are display-only and are never a focus target;
  the panel page changes only with `o`, the dock only with its setting. (The prototype
  differs here; this document wins.)
- **Cover art is only ever the playing track's.** One image pipeline, as today. The
  Columns track card is text-only unless the selected track is the playing one. The
  4-row player bar shows a small thumbnail of the playing track when wide enough.
- **Search heart column:** kept in Classic (it is part of today's look), absent from the
  four new layouts.

## The navigation model

This is the core of the work. Everything else is drawn from it.

### Location

Where the user is: main tab (Library, Search, Feed) and, within Library and Search, a
sub-tab (Library: Likes, Playlists, Albums, Following. Search: Tracks, Albums,
Playlists, People). Feed has no sub-tabs. Each main tab remembers its own sub-tab.

### Panes

Each location yields an ordered list of one to three panes:

| Location | Panes |
|---|---|
| Library › Likes | tracks |
| Library › Playlists | playlists, tracks of selected playlist |
| Library › Albums | albums, tracks of selected album |
| Library › Following | people, selected person's tracks, selected person's likes |
| Search › Tracks | tracks |
| Search › Albums / Playlists | results, tracks of selected result |
| Search › People | people, tracks, likes |
| Feed | activity, tracks of selected activity |

A pane has a kind (tracks, playlists, albums, people, feed), a title, its rows, a cursor
and scroll offset, and an optional filter string.

### Focus

Exactly one pane has focus per location, remembered per location. All movement and all
selection-based actions (play, queue, like, add to playlist, remove, station) act on the
focused pane's selected row and nothing else.

Sidebar and Columns have one more focusable thing to the left of the panes: the
navigation column (Library sections; in Sidebar also the user's playlists). `[` from the
leftmost pane focuses it; `]` or Enter leaves it. Moving within it changes location
immediately. In Sidebar, selecting a playlist in the navigation column sets location to
Library › Playlists with that playlist selected, and the playlists pane is not drawn
(the sidebar is it), so the leftmost drawn pane is the playlist's tracks.

### Identity, not position

Child panes are keyed by the identity of the parent's selected item (playlist URI, album
URI, user URN), not by its row number, and remember their own cursor per parent item.
Filtered views hand out the same row objects as unfiltered ones. Actions resolve their
target from the selected row at the moment they run. Confirm dialogs carry the identity
of what they will delete, not an index.

This replaces today's many `selected_*_row` fields and the `FollowingTracksFocus` /
`info_pane_selected` flags with one structure, and removes by construction several
defects found in the bug sweep (stale selection after tab switch, filter matches not
rebuilt, station ignoring the filter, confirm deleting the wrong playlist).

### Modes

- **Normal**: keys are commands.
- **Insert**: typing a Library filter, a Search query, the palette query or a new
  playlist name. Every printable key types, including capitals. Enter confirms, Esc
  cancels, Backspace deletes, Ctrl+W deletes a word, Ctrl+U clears. No bound action fires.
- **Overlay**: a popup is open. It is modal: it receives all keys and nothing reaches the
  view behind. Movement actions move within it, Enter selects, Esc closes, and the key
  that opened it closes it.
- **Visualiser**: full-screen as today. Tab cycles the visualiser mode. Playback keys
  work. Selection-based actions do nothing while it is up.

### Filter and search

`/` in Library enters insert mode on the focused pane's filter; the list narrows as the
user types. Enter keeps the filter and returns to normal mode. Esc while typing cancels
it. Esc in normal mode clears the focused pane's filter. `/` on the Search tab edits the
search query; Enter runs the search.

## Keymap

Defaults. All remain user-configurable through `[keys]` and the in-app editor.

| Group | Keys | Action |
|---|---|---|
| Move | `j` `k`, Down Up | Move in focused pane |
| | `[` `]` | Focus pane left / right (Columns: step out / in) |
| | `g g`, Home | Top |
| | `G`, End | Bottom |
| | Ctrl+D / Ctrl+U | Half page down / up |
| | Ctrl+F, PageDown / Ctrl+B, PageUp | Page down / up |
| Views | `h` `l`, Left Right | Previous / next sub-tab or search type |
| | `1` `2` `3` | Library / Search / Feed |
| | Tab / Shift+Tab | Next / previous main tab |
| Find | `/` | Filter (Library) or search query (Search) |
| | `:`, Ctrl+P | Command palette |
| | `?` | Keys and settings popup |
| Playback | Enter | Play selected track / step into selected container |
| | Space | Play / pause |
| | `>` `<` | Next / previous track |
| | `.` `,` | Seek forward / back 10 s |
| | `+` `=` / `-` | Volume up / down |
| | `s` / `r` | Shuffle / repeat |
| | `S`, Shift+Enter | Start station from selection |
| Track | `a` / `A` | Add to queue / play next |
| | `f` / `F` | Like or unlike selection / the playing track |
| | `p` / `P` | Add selection / the playing track to a playlist |
| | `d` | Remove from playlist (confirm) |
| Playlist | `c` / `x` | New playlist / delete playlist (confirm) |
| Panels | `Q` / `H` | Queue / history |
| | `v` | Visualiser |
| | `t` | Theme picker |
| | `o` | Cycle the player panel's page (Player panel layout) |
| App | `q` | Quit (confirm) |
| | Esc | Close overlay / leave insert mode / clear filter |

Consequences:

- **Key sequences.** `g g` needs the keymap to accept a sequence of chords. A chord that
  is a prefix of a longer binding waits for the next key (about a second), and the
  pending keys are shown at the right end of the hint line. The config grammar gains a
  space-separated form, for example `top = ["g g", "home"]`.
- **`f` on a track likes that track**, in any pane. Today it acts on the parent row.
  When a container pane (playlists, albums, people) has focus, `f` likes or follows the
  selected container.
- **`New playlist`** works from anywhere, not only Library › Playlists.
- **Help and Quit cannot be left with no binding** in the key editor.
- **Actions removed:** `secondary_up`, `secondary_down`, `tertiary_up`, `tertiary_down`.
- **Actions kept under their existing config names:** everything else, including
  `sub_tab_left`/`sub_tab_right` (still on `h`/`l`).
- **New actions:** pane left/right, top, bottom, half page up/down, go to each main tab,
  command palette, cycle panel page.

### Existing user configs

`[keys]` overrides are matched by action name and keep working. Entries for removed
actions are ignored with a startup warning that names them. Users who never customised
keys get the new defaults. Because defaults change for most actions, the release notes
must say so plainly, and the first launch after upgrade shows a one-time toast pointing
at `?`.

## Shared pieces

Every layout is a pure function from (state, terminal size) to a drawn frame, built
from the same parts:

- **Pane widget.** Draws one pane: border, title, optional filter text, position counter
  (`7 / 1,204`), header row, rows, cursor. Chooses columns by pane kind and available
  width (tracks: marker, title, artist, time, streams; drops streams, then artist, as
  width shrinks). Focused pane: accent border, bold title, full selection bar.
  Unfocused panes: dimmed text, no bar, a `›` on their selected row.
- **Pane row.** Lays out the location's panes side by side with weights per pane kind,
  and applies the pane-count breakpoint (below).
- **Player readouts**, three sizes: the original 7-row bar (Classic only), a 4-row bar,
  and a 1-line bar. All show state (playing, paused, stopped), title, artist, position,
  duration, shuffle, repeat, volume. The 4-row bar also shows the next track when there
  is room.
- **Hint line.** One dim row at the bottom. Lists the keys that apply to the focused
  thing in the current mode, read from the live keymap, trimmed to the width. A setting
  turns it off.
- **Toasts.** As above. Sources: like/unlike, follow, add to queue, play next, add to
  playlist, remove, create, delete, shuffle/repeat toggles, station start, layout and
  theme changes, and failures of any of those or of a fetch.
- **Overlays.** Queue, history, playlist picker, new-playlist prompt, confirm, theme
  picker, keys/settings/equaliser popup, command palette. All modal.
- **Command palette.** Lists every action with its description and current keys, plus
  one command per layout, one per theme, hint line on/off. Fuzzy subsequence match,
  best match first; Enter runs it.

## Layouts

All sizes below are columns × rows of the terminal.

### Classic

Today's screen at sizes where it fits (see the responsive rules for when it compacts): boxed tab bar with centred `sctui` title, boxed sub-tab bar (Library),
search box above and filter-type box below (Search), pane tables with the original
width ratios, 7-row player bar with the animated sine either side and
`shf / vol / rep` readout. Added: focused-pane accent border, playing marker, hint line,
toasts.

### Tighten

Row 0: `sctui`, the three main tabs with their numbers, and palette/help hints on the
right. Row 1: sub-tabs as pills with the selected one filled, filter or search text
right-aligned on the same row. Then the pane row, the 4-row player bar, the hint line.

### Sidebar

A navigation column on the left (about 26 columns): Likes, Playlists with the user's
playlists listed beneath it, Albums, Following, Search, Feed. Then the pane row. The
player bar and hint line span the full width underneath. On Search, a one-row strip
above the panes carries the type pills and the query. On wide terminals a setting (and
`Q`) docks an "Up next" column on the right instead of opening the queue popup.

### Columns

Row 0 is a breadcrumb (`sctui › Library › Playlists › rave tunes`) with filter or query
text at the right. Below it, Miller columns: the navigation column, then the location's
panes, showing a window of up to three with the focused one in view. `[` steps out,
`]` or Enter steps in. On very wide terminals, when a tracks pane is focused and is the
last column, a track card (cover, title, artist, duration, streams, applicable keys)
fills the right edge.

### Player panel

Left: Tighten's two header rows and the pane row, running to the hint line. Right: a
permanent panel (about a third of the width, capped) with cover art, title, artist and
source, progress, state/shuffle/repeat/volume, and a lower area that pages between
Up next, Lyrics, Spectrum and History (`o` cycles; `Q` and `H` jump to their pages
instead of opening popups). There is no bottom player bar.

## Responsive behaviour

"Looks right at every size" is a requirement, not polish. Rules, as implemented in the
prototype (thresholds may be tuned by a few cells during build, behaviour may not):

| Rule | Threshold |
|---|---|
| Terminal too small: draw a centred "terminal too small" message with the size, nothing else | under 36 columns or 8 rows |
| Panes shown side by side: 3 / 2 / 1. Hidden panes are reached with `[` `]`; the visible window follows focus and prefers to show the focused pane's child. Edge labels (`‹ Playlists`, `liked ›`) name what is off-screen | pane-row width ≥ 104 / ≥ 64 / below |
| Track columns: 5 / 4 / 3 (drop streams, then artist) | pane inner width ≥ 72 / ≥ 44 / below |
| Player: Classic 7-row bar only when rows ≥ 30; 4-row bar when rows ≥ 20; else 1-line bar | rows |
| Hint line hidden | rows < 14 |
| Classic falls back to compact header rows (no boxes) | rows < 24 |
| Sidebar navigation column hidden unless it has focus; a one-row strip names the location | columns < 90 |
| Sidebar docked queue allowed | columns ≥ 120 |
| Columns window: 3 / 2 / 1 columns | columns ≥ 100 / ≥ 60 / below |
| Columns track card | columns ≥ 124 |
| Player panel falls back to Tighten entirely | columns < 110 |
| Player panel cover art height 9 / 6 / 3 / none | panel rows ≥ 34 / 26 / 20 / below |
| Popups clamp to the terminal and never panic | always |

Acceptance: for every layout, at every size from 20×6 to 240×80, with every overlay
open, rendering must not panic, must fill exactly the terminal, and must keep the
focused row visible. This is to be enforced by a test that renders each combination on
ratatui's `TestBackend`.

## First-run setup wizard

The owner asked for this, modelled on the guided lessons at https://tuios.dev/learn
(a bolded goal per step, the key to press, and the app checking what actually happened
before moving on).

### When it runs

- **Fresh install** (no config file yet): the wizard runs the first time the TUI starts,
  which is after the browser login completes.
- **Existing users upgrading** (config file exists, wizard never completed): no wizard.
  They stay on Classic and see a one-time prompt, "Keys have changed. Take the
  two-minute tour?", which starts at the practice stage if accepted. *(Designer's call.)*
- **Any time:** a palette command, "Setup wizard", and a `--setup` flag, rerun it.
- Finishing or skipping records `onboarded = true` in `[settings]`.

### Stages

1. **Welcome.** One screen: what sctui is, Enter to continue, Esc to skip everything.
2. **Layout.** The five layouts as a list; moving the cursor redraws a live preview of
   that layout behind or beside the list, at the user's real terminal size. Enter keeps it.
3. **Theme.** Same pattern with the built-in themes (this is the existing theme picker's
   live preview).
4. **Practice.** The chosen layout and theme, with a lesson card pinned to one edge.
   Each step shows a bolded goal, one sentence of explanation, and the key, read from
   the live keymap. A step completes when the *action* happens (so it still works if the
   user has rebound the key), shows a tick, and advances. The card shows progress
   ("4 / 10"). Esc skips the rest at any point; nothing is forced.
5. **Done.** Where to find everything again: `?` for keys and settings, the palette for
   commands, and how to rerun the wizard.

### Practice steps (the important keys only)

1. Move down a few rows.
2. Jump to the bottom, then the top.
3. Go to the next sub-tab.
4. Move focus into the tracks pane, and back.
5. Play a track.
6. Pause and resume.
7. Add a track to the queue (first toast appears; the card points at it).
8. Filter the list, then clear the filter.
9. Open the command palette and run a command.
10. Open the keys popup and close it.

### Constraints

- The wizard and sandbox obey the same responsive rules as everything else; on a
  terminal too small for the lesson card beside the layout, the card takes the hint
  line's place as a two-row strip.
- Steps are driven by actions, never by raw key codes.
- Practice runs in the real app after login, on the user's own library with real sound.
  The lesson steps use only actions that do not change the SoundCloud account (moving,
  playing, queueing, filtering, opening popups). Other keys still work normally; the
  wizard does not block them.
- If the user's library is empty, steps that need a track are skipped rather than
  blocking the wizard.

## Settings and persistence

New keys in `[settings]`: `layout` (`classic`, `tighten`, `sidebar`, `columns`,
`player-panel`), `hint_line` (bool, default on), `dock_queue` (bool, default off).
Unknown layout names fall back to the default with a startup warning.

New installs choose their layout in the setup wizard; if they skip it the default is
Tighten *(designer's call)*. Existing users stay on Classic. `onboarded` (bool) records
that the wizard was finished or skipped.

## Bugs

A sweep of the whole codebase ran alongside this design. Its findings are numbered in
the conversation that produced this document (`#1`–`#19` player/API/auth/config,
`T1`–`T20` TUI). How they relate to this work:

**Fix first, as separate small changes, before the redesign starts:**

- T1: confirm popup panics under 40 columns (`render/overlays/confirm.rs:15`).
- T3: confirm dialogs carry a positional playlist index (`input/confirm.rs:56`).
- #3: key specs written to TOML unescaped (`keymap.rs:593`).
- #4: a type error in `[theme]`/`[settings]`/`[eq]` silently resets all three
  (`config.rs:197`).
- #5: re-authentication duplicates every library list (`main.rs:79`).
- #7 with T11: playlist edits race and failures are dropped
  (`tui/logic/mod.rs:481`, `api/calls/playlist_edit.rs:81`).

**Fixed by the new navigation model; each needs a regression test rather than a patch:**

- T4 (`input/navigation.rs:23`): switching tab or sub-tab resets the selected row to 0
  but leaves the table scrolled, so the highlight is off-screen and Enter/queue/like act
  on row 0.
- T5 (`input/mod.rs:82`): the Library filter popup stays active after a tab switch and
  swallows typing on the Search tab.
- T6 (`logic/filtering.rs:20`): filter matches are raw indices that are never rebuilt
  when the underlying list changes, so the wrong rows are shown after moving to another
  playlist, an unlike, or a new page arriving.
- T7 (`input/playback.rs:16`): starting an artist station reads the unfiltered list, so
  with a filter active it starts for a different artist than the highlighted one.
- T16 (`keymap.rs:227`): station is bound only to Shift+Enter, which terminals without
  the kitty keyboard protocol deliver as Enter. It gains `S`.
- T18 (`input/help_editor.rs:53`): the key editor allows unbinding Help and Quit.
- Overlays letting keys through to the view behind, and bound Shift+letters being
  untypeable in search and filter fields.

**Enabled by this work:** T11's missing failure feedback has somewhere to go once
toasts exist.

**Out of scope here, tracked separately:** everything else from the sweep (playback
engine, auth, API paging, updater).

## Order of work

Each step leaves the app working and releasable.

1. The "fix first" bugs above.
2. Navigation model, modes, new keymap with key sequences, modal overlays, drawn in
   Classic only. Shared pane widget. Playing marker.
3. Hint line and toasts.
4. Responsive rules for Classic, and the render-everything test.
5. Layout setting and Tighten.
6. Command palette.
7. Sidebar.
8. Columns.
9. Player panel.
10. Setup wizard (needs the layouts, themes, hint line and toasts to exist).

## Left to the builder

- How state is structured internally, provided the navigation model above holds.
- Exact threshold values within a few cells of those listed.
- Exact wording of hints, toasts and palette entries.
- Fuzzy scoring details.
- How the render test enumerates sizes.
- Whether steps 7–9 land as one release or three.
