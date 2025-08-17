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
widths to match. Additionally, I opted to clamp the _Duration_ column to 10% width, to save more space for columns that would likely
contain much longer strings of text (_e.g. title or artist(s)_).

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
