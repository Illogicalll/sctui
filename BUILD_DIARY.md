# Dev Diary

The following will be an collection of my thoughts (and frustrations) throughout the process of developing the
project

<details>

<summary>Chapter 0: Pre-Project</summary>

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

<summary>Chapter 1: Auth</summary>

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

Once authenticated, the program will be in possession of the user's token. The plan from here was to have a separate
`api.rs` file which could access the `token` variable that would abstract the API interaction from the UI file(s).
However, with the boring auth and API setup behind me I was itching to jump into the [ratatui docs](https://ratatui.rs/)
and do some UI work.

</details>
