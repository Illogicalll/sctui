# Dev Diary

The following will be an collection of my thoughts (and frustrations) throughout the process of developing the
project

<details>

<summary><b>Chapter 0: Pre-Project</b></summary>

## Why?

Well first and foremost, **fun**. Building things has been and always will be a passion of mine, the satisfaction of
seeing something you envisioned come to life is immeasurable to me.

For this project in particular, I had grown envious of [spotify-player](https://github.com/aome510/spotify-player)
and wanted a similar TUI for SoundCloud, [my streaming platform of choice](https://soundcloud.com/illogicalll). I had
seen many TUIs built with Rust and figured it would be a perfect excuse to get stuck in with the language for the
first time. Although I knew I would almost definitely be in over my head, I figured it would be the perfect
environment to learn.

##  How?

As briefly discussed above, I opted to use Rust for this project. In order to build the TUI in as simple a manner as
possible, I had a browse on [crates.io](https://crates.io/) (Rust's package registry) and came across
[tui](https://crates.io/crates/tui). Unfortunately, its last update was back in 2022. However, after a quick look I
found [ratatui](https://crates.io/crates/ratatui), a successor to tui which had been forked back in 2023 and
maintained steadily since.

In order to handle interactions with SoundCloud, I set out to use the
[official API](https://developers.soundcloud.com/docs/api/guide). The first spanner in the works came from trying to
register my app to gain access to the API. Upon following the official documentation to a google form, it appeared
it was no longer possible to submit applications.

![Deprecated SoundCloud API Application Form](/media/form.png)

Instead I was instructed to use _"support channels, e.g. the chat bot"_, which was rather vague and confusing
especially since the [support section](https://developers.soundcloud.com/support) of the developers page had no such
feature.

![SoundCloud Developers Support Page](/media/support.png)

Eventually, I dug up a
[Reddit post](https://www.reddit.com/r/soundcloud/comments/1l5uxno/how_do_you_register_the_soundcloud_api/) with a
link that led me to a chatbot which helped me create a ticket with the API team. A couple days later my application
was approved and the project was finally underway!

</details>

<details>

<summary><b>Chapter 1: Auth</b></summary>

## Rust OAuth 2.1 Adventures

I had plenty of experience with APIs from previous work and projects, however this was both my first solo from-scratch
OAuth 2.1 system and my first time using Rust so it took a little while to get working. Luckily, the provided
documentation was very clear and provided great examples such as this
[collection of PKCE tools](https://example-app.com/pkce).

With my request formed, I needed a way to receive and handle the callback of the authentication process locally. To
do this I used [tiny_http](https://crates.io/crates/tiny_http/) to create a self-hosted server that would capture and
parse the server response, before passing that into a POST request back to the soundcloud server for the final token.

Then, all that was left was to define token refresh logic and a thread that would ensure that during the program's
execution the user's auth token would never expire. This implementation did mean that if the app was opened after the
token had expired the user would be put through the auth process again, but I was just excited to have working auth
and decided to investigate potential solutions to it later.

## What Next?

Once authenticated, the program will be in possession of the user's token. The plan from here was to have a separate
`api.rs` file which could access the `token` variable that would abstract the API interaction from the UI file(s).
However, with the boring auth and API setup behind me I was itching to jump into the [ratatui docs](https://ratatui.rs/)
and do some UI work.

</details>

<details>

<summary><b>Chapter 2: UI Requirements</b></summary>

## What do I Want This Thing to Look Like?

Having grand visions in your head can be easy, but fully capturing them and making them come to life can prove a
significant challenge (_especially when you are using a UI library you have never used in a language you just started
learning_).

With this in mind, I wanted to take things slowly so as to not end up with an unintelligble mess of code that I would
never be able to return to. In order to do this I needed to start right at the bottom.

## What Do I Never Use?

My logic here was that if I could manage to strip out everything I disliked/never used on the platform, then I would
be left with the bare minimum. This would then perfectly align with minimalism and functionalism, two key principles
in my understanding of what makes a good TUI. So, I started with a list:

- Like 90% of the homepage
  - No shade to the actual developers at soundcloud but "events near you", "curated to your taste" and "artists to
    watch out for" are just some of the many sections on the homepage that I have never once even considered interacting
    with

![Useless (for me) Features on SoundCloud's Home Page](/media/home.png)

- The overview tab of the library
  - I am a fan of more forced navigation rather than having multiple ways to access things. I also would rather have
    something be an additional click away if it means it keeps it more organised and natural. Therefore, instead of
    having an overview page with likes amongst other things, I would much prefer to keep everything in its own
    self-titled sub-tab. The only core components to a user's library from my point of view are:
    - liked tracks
    - playlists
    - saved albums
      Tabs such as:
    - saved stations
    - followed users
    - listening history
      Do have their uses but I don't find myself using them very often. Nonetheless I will likely end up including them
      as features with a lower priority due to the fact that there would be nowhere else to view that data (other than
      the website itself of course)

![The Unused Overview Tab of my Library](/media/library.png)

- Upload
  - I'm not an artist but even if you are an artist, why would you be trying to upload your song through a TUI
    anyway lmao
- My Profile
  - This obviously has its use case but I think its far from necessary in this type of application

## What's Left?

So what does that leave us with?

- A homepage with next to nothing on it
- A feed page of the recent activity of users you follow
- A library page with multiple tabs
- A search page

Given that there was next to nothing left to display on the homepage, I just decided to just scrap it completely and
have 3 tabs (in order of usefulness):

- Library
- Search
- Feed

Content for the time being with the simplicity I had ended up with, I decided to move on to the next stage.

</details>

<details>

<summary><b>Chapter 3: UI Design</b></summary>

## Getting Something on the Screen

For all the time I had spent, the program still did nothing at this point other than authenticate (which I was still
patting myself on the back for). To actually get something tangible my initial thought was to do some mockups. I did
come across [a great tool](https://asciiflow.com/#/) for ascii drawings, but I figured instead of spending hours on
drawings and then scratching my head trying to implement them in a framework and language I have barely ever
used before, I would just get stuck in and see what happens.

For some quick inspiration, I explored some of the [example apps](https://ratatui.rs/examples/apps/) on display on the
ratatui site. In particular, the first _'Demo'_ app was particularly good at demonstrating the different possibilities,
instantly crowding my brain with possibilities for my own TUI. While this might not be the most 'proper' approach to
things, this is a personal project so who cares lol.

## General Layout

After playing with ratatui's `Constraint` and `Block` layout system for a while I settled for a basic 3-tiered design with the following:

- A header with a tab selector
- An area for content, based on the current tab
- A now playing section

![First Draft of Application Layout](/media/ui_1.png)

I felt that this was reminiscent enough of the actual UI on the soundcloud website, which also has fixed tabs at the top and now playing
at the bottom, with a variable content area sandwiched between:

![An Example of SoundClouds UI](/media/sc_ui.png)

## The Library Tab

As discussed above I wanted the library tab to have its own set of sub-tabs. To achieve this, I divided the content area once again for a
second row of tabs:

```rs
let subchunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(chunks[1]);
```

Then I rendered the sub-tabs up top and a table below.

![Subtabs and Table on the Library Tab](/media/ui_2.png)

The table also needed some extra logic to handle different numbers of columns for different sub-tabs, as well as different column
widths to match. ~~Additionally, I opted to clamp the _Duration_ column to 10% width, to save more space for columns that would likely
contain much longer strings of text (_e.g. title or artist(s)_).~~ I ended up removing this logic and opting for
fixed defined column widths as this method introduced unnecessary complexity.

```rs
fn styled_header(cells: &[&str]) -> Row<'static> {
  // map an array of strings to a row of styled cells
  // this avoids repeating code in the column definitions
}

let (header, num_columns) = match selected_subtab {
  // define a tuple that holds the headers (for the styling function above)
  // and the number of columns (for the column width definition below)
  // based on the currently selected sub-tab
};

let column_widths: Vec<Constraint> = if num_columns > 0 {
    if num_columns > 2 { // all tables with more than 2 columns have a duration column
      let other_width = 90 / (num_columns as u16 - 1);
      let mut widths = vec![Constraint::Percentage(other_width); num_columns - 1]; // calculate widths of other columns
      widths.push(Constraint::Percentage(10)); // clamp duration to 10%
      widths
  } else { // otherwise calculate normally
      let width = 100 / num_columns as u16;
      (0..num_columns)
          .map(|_| Constraint::Percentage(width))
          .collect()
  }
} else {
  vec![]
};
```

You may have also noticed in the screenshot above that longer titles are truncated when the window becomes too narrow. To keep things
short(_ish_), I won't go into any further depth but if you are interested the full UI file with comments is available
[here](./src/tui.rs).

## The Search Tab

Two of the most important features to any search are the **query** and the **filters** (honourable mention: sort by, but I'm going to leave that one for now).
After placing them both next to each other, I decided that this cluttered the space a bit too much:

![First Draft of Search UI](/media/search_1.png)

Instead, I decided to break up the space a bit more by sandwiching the search results inbetween the search bar and the filters:

![Second Draft of Search UI](/media/search_2.png)

Additionally, I centered the filters to help further distinguish the filter tabs from the main page tabs:

![Third Draft of Search UI](/media/search_3.png)

Unfortunately `ratatui` doesn't natively support tab centering, so I had to go with my own implementation:

```rs
// terminal area divided by how many search filters there are
let tab_width = width / NUM_SEARCHFILTERS;

fn center_text_in_width(text: &str, width: usize) -> String {
  // account for width of dislayed text
  let total_padding = width - text.chars().count();
  let padding = (total_padding / 2) - 1;

  // return padded text
  format!("{}{}{}", " ".repeat(padding), text, " ".repeat(padding))
}

// map over array of search filters, returning Spans of padded text
let searchfilter: Vec<Span<'static>> = searchfilters
    .iter()
    .map(|filter| Span::raw(center_text_in_width(filter, tab_width)))
    .collect();
```

## The Feed Tab

Last but not least was the feed tab, where activity of followed accounts resides. This one was a bit more of a challenge to
implement as I wanted to keep the interface clear and functional, but also be able to represent all types of account activity and
allow the user to interact with it (play posted/reposted songs or albums) instead of just having it be a static list of events.

In the end I went for a vertical split (two panes next to each other), with the left pane hosting the main activity feed and the right having further
information on the current selection:

![Draft of the Feed Tab Design](/media/feed.png)

The column headings 'User, Action, Media Type, Age' were the best I could come up with to encapsulate all types of activity displayed on the feed page of
the official website.

## Now Playing

Since the application is still just a non-functional shell at this point, I figured there wasn't much point mocking up animated elements and such when
there is no current system to even handle 'playing' a track. Because of this, I will return to this area of the TUI in a later chapter.

</details>

<details>

<summary><b>Chapter 4: API Functions</b></summary>

## Getting our Data

Before I hook anything up to the UI I just made, I want to write logic that will be able to bring in any data I need and return it in the
format I want. I find it best to approach this going tab by tab and then going over each feature to check we will have what we need.

At first I started defining a list of public functions with the token arc mutex in the parameters. While this is the simplest
approach, it would have resulted in the token having to be passed in every time an API call is made. This would have plagued the
UI code with references to authorisation/token code, which I wanted to keep completely separate.

After deleting what little progress I had made, I instead went for a different approach:

`api.rs`:

```rs
// define an API struct that holds a shared, thread-safe token
pub struct API {
    token: Arc<Mutex<Token>>,
}

impl API {
  // when instantiated, the token will be internally accessible with self.token
  pub fn init(token: Arc<Mutex<Token>>) -> Self {
      Self {
          token,
      }
  }

  // define API functions here
  pub fn api_function_name(&mut self) -> anyhow::Result<Value> {
    // ...
  }
```

`main.rs`:

```rs
// define an API object and pass in a reference to the thread-safe token
let mut api = api::API::init(Arc::clone(&token));

// pass the object into the UI file
tui::run(&mut api).map_err(|e| anyhow::anyhow!(e))?;
```

This implementation means that the `tui.rs` file can remain completely oblivious to the existence of tokens and authorisation logic,
while still being able to fetch whatever data it needs.

## Likes

So here we go then, the first API function. Honestly, it ended up taking slightly longer to think get working
than I would have liked given all the supporting setup I did. But such is the reality of building anything I
suppose.

The first major blocker was that there didn't seem to be a way to fetch the Album of a track. I found this
rather ridiculous and spent a long time figuring out a workaround. In the end all I could come up with was to
return the top result of the playlists a track is in that has the `playlist_type` property set to `album`.

However, this would obviously result in a ton more API calls, and to add insult to injury this feature was only
available on Soundcloud's V2 API which I didn't have access to. In the end I opted to substitute the Album column
for a stream count column. It still bugs me but at least I knew there wasn't really much I could do about it.

Another problem I ran into was that Go+ (Soundcloud's premium tier allowing access to mainstream music, offline
listening, _etc._) tracks had no `stream_url`. This ultimately meant that, while I could display the tracks
metadata on the table, the user would not actually be able to play the track. After some reading up on the
documentation, it didn't seem there was a way around this either, so I simply opted to hide those tracks.

With those setbacks aside, it was time to design the API function. The Soundcloud API enforces the reasonable
requirement of pagination (to avoid large requests). With this in mind I made space for a
`liked_tracks_next_href` variable in the API struct, allowing it to persist between function
calls:

```rs
pub struct API {
    token: Arc<Mutex<Token>>,
    liked_tracks_next_href: Option<String>,
}
```

My thinking for the `get_liked_tracks()` function would be to attempt to fetch from the `liked_tracks_next_href` first, and
if it doesn't exist yet call from the default URL that fetches the first 40 tracks (40 seemed like a good balance
of not leaving blank space even in tall windows but still not taking too long to fetch).

This, paired with some formatting functions to handle duration and stream count readability resulted in a relatively
straight-forward first API function.

After hooking it all up to the TUI I realised that scrolling to the bottom didn't make the table scroll. This turned
out to be as my table was not a stateful widget yet. Luckily ratatui makes it pretty straight forward, and
after some adjustments to update the table state and then calling `render_stateful_widget(...)` as opposed to `render_widget(...)`,
scrolling was fully functional.

Lastly I needed to simply re-call the `get_liked_tracks()` function whenever the user was close to reaching the
bottom of the list:

```rs
if max_rows >= REFRESH_THRESHOLD && selected_row + REFRESH_THRESHOLD >= max_rows {
    if let Ok(new_likes) = api.get_liked_tracks() {
        likes.extend(new_likes.into_iter());
    }
}
```

As seen below the data is now dynamically pulled in through the API:

![Likes Working](/media/likes_working.png)

As a side note I ended up removing the dynamic even column width calculation function as it was unnecessarily
complicated, instead opting for defining fixed percentage widths.

## Playlists

Soundcloud offers a `/me/playlists` end point that returns a specified number of playlists and all the tracks in them at once. This is great, but playlists
can contain hundreds or even thousands of songs and this could lead to quite a lot of lag in the application. To avoid this I set the handy `show_tracks`
parameter to `false`, which then gave me only the playlist metadata, taking significantly less time to execute. The plan would then be to follow the link
contained in the `tracks_uri` field if the user wants to navigate to that specific playlist.

One that is rather counter-intuitive though is the fact that `/me/playlists/` doesn't actually return **ALL** your saved playlists, only the ones that
**YOU** made. To include the playlists you saved that other people made you also need to call `/me/likes/playlists` (which will also retrieve all the albums
because on soundcloud albums = playlists, so it needs to be filtered). The question then arises how to we interweave these two responses to form a singular,
cohesive list of saved playlists. There is no `date_saved` so ultimately I settled on `created_at`, which seemed like the next best option.

Another thing I discovered while investigating lag is the slight hitch that occurs when the user holds the down arrow to continuously scroll down. This
was obviously occurring due to the fact that the application was trying to fetch more playlists mid-frame. This led to a _slight_ (complete) overhaul of
how the API functions within the `tui.rs` file in order to make it run on a seperate thread.

If different threads are going to call the API, we obviously need to make it thread safe which means more `Arc`s and more `Mutex`es (yay). Now I have this
abomination in `main.rs`

```rs
let mut api = Arc::new(Mutex::new(api::API::init(Arc::clone(&token))));
```

In order to receive data between frames we need to be able to set up a channel that we can _'check up on'_ in between frames:

```rs
let (tx_playlists, rx_playlists): (Sender<Vec<Playlist>>, Receiver<Vec<Playlist>>) = mpsc::channel();
```

We define a transmitter `tx` for pushing a `Vec<Playlist>` into the channel, and a receiver `rx` for reading the data from the channel.
The API thread(s) fetch playlists from the API and use the sender to pass results back. Meanwhile, the main render loop uses `try_recv()` on the receiver to
check up on the channel between framees for new data:

```rs
loop {
  while let Ok(new_playlists) = rx_playlists.try_recv() {
      playlists.extend(new_playlists.into_iter());
  }

  terminal.draw(|frame| {
    // ...
```

This process was also applied to the liked songs logic and will be applied to all future API usage to avoid the frame hitching problem.

## Albums, Stations, Following + History

These are basically all just following the same pattern outlined above, so I will skip the details here.

The only noteworthy inclusion is that unless I am missing something (I probably am) there doesn't seem to be a way
to retrieve the saved stations or the listening history.

Oh well, I never used the saved stations feature before and while I could implement the listening history myself
(with a simple array), it is not a priority right now.

</details>

<details>

<summary><b>Chapter 5: Playing Audio</b></summary>

## Clueless

I spent like a solid day blindly fumbling with `rodio` (an audio playback library) and `tokio` (an asynchronous runtime)
when I barely understood either. In the end I got so frustrated I deleted all the audio playback code and just started
fresh.

## Clear Head

After taking a break from the project I finally had a fresh start on the playback system. Previously, I had tried to
give the implement the `Track` struct with a `play()` method that I could call from `tui.rs`. This had many downsides
and, in hindsight, was doomed to fail from the get go.

This time I opted to create a new `player.rs` file which would spawn an entirely separate thread, whose sole purpose
was to receive commands sent from `tui.rs` and handle audio playback functionality.

```rs
pub enum PlayerCommand {
  // types of commands the player can receive
}

// transmitter that can communicate with the player thread
pub struct Player {
  tx: Sender<PlayerCommand>,
}

impl Player {
  pub fn new(token: Arc<Mutex<Token>>) -> Self {
    // spawn the player thread
  }

  pub fn play(&self, url: String) {
    // transmit the stream_url to the player
  }
}

// the player logic that runs in the thread and awaits commands
fn player_loop(rx: Receiver<PlayerCommand>, token: Arc<Mutex<Token>>) {
  let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

  for msg in rx {
      match msg {
        // commands and logic go here
      }
  }
}
```

## Handling Audio Download and Playback

In my first attempt I had attempted to build it all from scratch myself and while I'm sure I could have eventually
got it working, it just wasn't worth the hassle.

Instead, I found the super useful [stream-download-rs](https://github.com/aschey/stream-download-rs) crate which handled
much of the headache for me.

This also meant that I could easily play tracks of any length (which is especially useful for soundcloud as many
DJs will upload entire sets) without having to download the entire file first or load it all into memory.

However, this approach would cause me a lot of frustrations when attempting to implement seeking (skipping back and forth
within a track). In the end I got it _semi_-working but it would either take forever to load after the skip or just
sometimes crash so I decided that, given it wasn't a priority, I would come back to it later.

## Later

I finally got around to properly implementing seeking functionality. In the end, the challenge was that `rodio`'s `Decoder` doesn't support seeking in streaming audio, so I had to work around this limitation.

The solution I came up with was to restart playback from a new position using HTTP Range requests. When the user presses `OPTION/ALT + Right` (fast forward) or `OPTION/ALT + Left` (rewind), the app:

1. Calculates the new position (±10 seconds)
2. Stops the current playback
3. Estimates the byte position based on time (assuming ~128 kbps = ~16 KB per second)
4. Sends an HTTP Range header (`bytes={start_byte}-`) to request audio from that position
5. Restarts playback from the new position

The byte position estimation isn't perfect since it assumes a constant bitrate, but it works well enough for most tracks. The actual audio now seeks forward/backward properly, not just the progress bar.

```rs
// estimate byte position: assume ~128 kbps = ~16 KB per second
if seek_position > 0 {
    let estimated_bytes_per_second = 16_000;
    let start_byte = (seek_position as u64 / 1000) * estimated_bytes_per_second;
    let range_header = format!("bytes={}-", start_byte);
    if let Ok(range_value) = HeaderValue::from_str(&range_header) {
        headers.insert("Range", range_value);
    }
}
```

## Shuffle and Repeat

After getting the basic playback working, I wanted to add shuffle and repeat functionality like most music players have. The implementation was pretty straightforward - just a couple of boolean flags to track state and some logic to handle the different playback modes.

For shuffle, I track a playback history so that when the user presses `SHIFT + Left` to go back, they go to the previously played song (which might be different from the previous index if shuffle was enabled). This history is maintained regardless of whether shuffle is enabled or not, so backward navigation always works intuitively.

When shuffle is enabled:
- `SHIFT + Right` picks a random song from the list (avoiding the current song)
- Auto-play when a song finishes also picks a random song
- The current song is added to history before playing the next one

For repeat, it's even simpler - when enabled, the same song just plays again when it finishes. If both shuffle and repeat are enabled, repeat takes priority (which makes sense - if you want to repeat a song, you probably want to repeat that specific song, not a random one).

The UI indicators in the now playing section show `✔︎` when enabled and `×` when disabled, updating in real-time as you toggle them with `SHIFT + S` (shuffle) and `SHIFT + R` (repeat).

Although now that I think about it the random song choosing might have to be reworked slightly as I want to add a view to see the queue which means it would have to be precalculated. Oh well, shouldn't be too much of a change.

</details>

<details>

<summary><b>Chapter 6: Now Playing UI</b></summary>

Finally away from the headache of the audio playback, I could focus on some easy stuff again: The now playing
display.

I went through a couple designs and ideas in my head and I decided that (on top of the regular title, artist
and progress bar) I really wanted a sine wave animation to emphasise playing status and the cover art to show
up too.

Originally I had the art on the left, the info in the middle, and the wave on the right. But the art was a
nightmare to center in its own box and handle resizing with so ultimately I settled for something else:

![Now Playing](/media/now_playing.png)

Honestly, I'm glad the original design didn't work because I think this one with the waves on either side
actually looks better!

## Queue Popup (and Making Shuffle Predictable)

Slightly adjacent to the Now Playing UI, but its related so I thought I would include it here.

TL;DR, I immediately had to rework the shuffle logic like I said I would. In order to show the upcoming queue, I needed the "random" choices to be deterministic, otherwise the list would just change every time it was rendered. The fix was to build a `queue` data structure up front and then pop from it when the next song is needed.

This also made the skip controls behave much nicer because `SHIFT + Right` now pulls from the queue instead of rolling a new random number every time.

Once I had a real queue, I added a popup with `SHIFT + Q` that shows the upcoming songs in a small table. It includes the Title, Artist and Duration (no stream count as that isn't essential information)

![Queue](/media/queue.png)

On top of that, the first row shows the previously played song in a greyed out format and the second row shows the currently playing song (highlighted). This makes it super easy to see where you are in the history and what is coming next.

</details>

<details>

<summary><b>Chapter 7: Quality of Life</b></summary>

With all these feature changes, I figured it was time to take a step back and consider the user-friendliness of the application as a whole.

This led to me making a few additions that may not necessarily add song playback focused features, but useful ones nonetheless.

## Help Menu

At this point there are enough hotkeys and weird little combos that even I forget them half the time.

In order to maintain the usability of the application, I added a help popup triggered with `SHIFT + H` that lays everything out in a clean table.

![Help](/media/help.png)

## Quit Confirmation

Right at the start of development, I chose the Escape key as the button to quit the application.

While this makes perfect sense, I found myself pressing it accidentally without a second thought when trying to quit out of the 'Queue' and 'Help' views as a muscle memory reaction.

This resulted in me closing the application when I didn't mean to, which was quite frustrating.

To prevent this, I added a small confirmation menu that acts as a safeguard against accidental closing of the application.

![Confirmation](/media/quit.png)

## Jumping

Previously, the only option to navigate up and down a list of songs, playlists, etc. was to use the arrow keys to go up and down one at a time.

While this worked perfectly fine, long lists such as a large amount of liked songs became a pain to navigate as you had to hold down a key and wait forever to get to where you wanted to be.

To help this issue, I added the `OPTION/ALT + DOWN` / `OPTION/ALT + UP` keys, to jump 10 items at a time. It was honestly crazy how much of a difference this tiny change made to the usability of the application.

## Search

Another usability headache was just finding *anything* once your lists start getting big. Sure the jumping functionality helped you navigate, but navigating is no use if you don't even know where to look.

I didn't want to add a whole search tab for Library, as that would be complicated with different types of results (tracks, playlists, albums, artists). So instead I went with an inline search bar that appears between the sub-tabs and the list when you press `SHIFT + F`.

![Search](/media/search.png)

The tricky part was that the list behind the search bar needed to *actually filter* while you type, without breaking the underlying data (which is also used by the queue). The fix was to render a filtered view while still keeping the full list around for indexing:

```rs
let filter_active =
    search_popup_visible && selected_tab == 0 && !search_query.trim().is_empty();
let filtered_likes = if filter_active && selected_subtab == 0 {
    likes
        .iter()
        .filter(|track| {
            let q = search_query.trim().to_lowercase();
            !q.is_empty()
                && (track.title.to_lowercase().contains(&q)
                    || track.artists.to_lowercase().contains(&q))
        })
        .cloned()
        .collect::<Vec<_>>()
} else {
    Vec::new()
};
```

That solved the visual part, but it introduced a new bug: when you hit `SHIFT + A` to add a song to the queue, it used the *unfiltered* index. I had to map the selection back to the underlying list instead:

```rs
let search_active = search_popup_visible && !search_query.trim().is_empty();
if search_active {
    if let Some(&idx) = search_matches.get(selected_row) {
        manual_queue.push_back(idx);
    }
} else {
    manual_queue.push_back(selected_row);
}
```

With all that in place, the feature was working as intended!

## Development Quality of Life

All of these changes to benefit the end-user got me thinking... *what about me??*

This whole time I had been extending the same 5 files and it was beginning to be almost impossible to work with.

I knew that refactoring all this work would be quite a challenge, but it was one I was willing to accept purely because I knew how much time it would save me down the line as I continued to extend this application.

After many frustrations, I finally ended up with a structure that turned the 5 files I had into 5 folders, with their functionalities divided up into files within them.

I would explain the structure in more detail, but to be honest you can literally just go look for yourself in like 2 clicks if you are reading this so I won't bother. Oh well, glad this nightmare is over though.

</details>

<details>

<summary><b>Chapter 8: The Other Library Sub-Tabs</b></summary>

Up until now, the only actually functional tab in the application was the 'Likes' sub-tab within the 'Library' main tab.

With all the recent progress in that area, I figured it was *finally* time to move on to building out the rest of the application

## Playlists

This sub-tab had originally been blocked-out as a single list similar to 'Likes'.

While I did consider leaving it that way, and making the `ENTER` key open the selected playlist in a new list, I figured this would ultimately be slightly confusing. This was because there would be no obivous way to go back to the main playlist view.

If I could, I wanted to stay away from opening new windows and instead have everything be immediately visible to the user when they switch to the view.

With this in mind, I split the area into two panes: playlists on the left (1/3 width), tracks on the right (2/3 width).

![Playlists](/media/playlists.png)

One challenge to overcome was switching out the queue. Once you can play from a playlist, the user would obviously expect other songs in that playlist to come next (not whatever was playing before). This required a source-aware queue instead of assuming a single list:

```rs
let active_tracks = match state.playback_source {
    PlaybackSource::Likes => &data.likes,
    PlaybackSource::Playlist => &data.playlist_tracks,
};
state.auto_queue = build_queue(current_idx, active_tracks.len(), state.shuffle_enabled);
```
Another challenge was that there were now two different selectors to control. One for the playlists, and one for songs within those playlists.

In the end I had to shuffle round some of the existing key combinations to make this navigation feel as natural as possible.

- `SHIFT + Up/Down` = select playlists (used to be volume up and down)
- `Up/Down` = select tracks
- `OPTION + Up/Down` = jump 10 tracks at a time

## Albums

The goal here was the same as with the playlists: keep context visible without forcing the user into a separate screen.

I went with a narrower right pane than playlists as there are a lot more columns to fit all the album metadata than for the playlists. The layout is now 65/35, with the full album table on the left and the album's tracklist on the right.

![Albums](/media/albums.png)

The async track fetching pattern is shared with playlists, including cancellation when the user scrolls quickly through albums.
This keeps the UI responsive and avoids stacking up requests for albums that are no longer selected:

```rs
if let Some(handle) = state.album_tracks_task.take() {
    handle.abort();
}
state.album_tracks_request_id = state.album_tracks_request_id.wrapping_add(1);
```

## Following

Following ended up being the most complex sub-tab so far because it has three independent panes and two different track lists.

The layout is now 20/40/40: users on the left, their published tracks in the middle, and their liked tracks on the right. I did really want to add their playlists too, but I couldn't think of a simplistic way to incorporate that feature other than to have a new hotkey to toggle between tracks/liked and playlists/playlist tracks. This could be something to implement in the future, but for now it is not a priority.

![Following](/media/following.png)

The most difficult part of all of these different views was making everything work properly with queueing. A track can be queued from any panel, but playback should continue from the original context.

I ended up storing a richer queue entry that includes its source and a snapshot of the list it came from, so manual items can play without reshaping the queue.

</details>

<details>

<summary><strong>Chapter 9: A Break From Useful Features</strong></summary>

Hooking up all the other Library views was quite a mentally straining process, so once I finished them I figured why not have some fun for a bit with some functionally useless but visually cool features?

## Visualiser

The visualiser started as a simple oscilloscope inspired by [scope-tui](https://github.com/alemidev/scope-tui) but grew into a much bigger UX problem than I expected.

The goal was not scientific accuracy, but to make the waveform feel "alive" and consistent across quiet and loud tracks.

The solution was to tap samples directly from the playback pipeline and then apply a light RMS-based gain with smoothing. That keeps the waveform readable without letting quiet sections blow up or loud peaks dominate the whole screen:

```rs
let rms = (samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32).sqrt();
let target_gain = (0.9 / rms.max(VOLUME_FLOOR)).min(MAX_GAIN);
```

![Visualiser](/media/visualiser.gif)

I was a bit concerned about performance but so far in my testing it doesn't seem to have slowed the application down significantly/

</details>
