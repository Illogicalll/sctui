mod utils;
mod models;
mod calls;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::auth::Token;

pub(crate) use calls::engagement::engage;
pub(crate) use calls::following::fetch_user_tracks;
pub use calls::playlist_edit::{
    add_track_to_playlist, create_playlist, delete_playlist, remove_track_from_playlist,
};
pub use calls::playlists::{fetch_playlist_tracks, has_playable_tracks};
pub use calls::related::{fetch_related_tracks, fetch_related_users};
pub use calls::search::{
    fetch_search_albums, fetch_search_people, fetch_search_playlists, fetch_search_tracks,
};
pub use models::{Activity, Album, Artist, Playlist, Track};
pub mod lyrics;
pub mod rate_limit;
pub use lyrics::{Lyrics, fetch_lyrics};
pub(crate) use utils::{MAX_PAGES, format_duration};

/// The lists `API` pages through, a page per call.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum List {
    Likes,
    Playlists,
    Albums,
    Following,
    Feed,
}

/// Cursor over one `linked_partitioning` list. It moves only when a page has been
/// read, so a request that fails leaves it where it was and the same page is asked
/// for again next time. Callers must not fetch one list twice at once.
enum Page {
    Start,
    Next(String),
    Done,
}

impl Page {
    /// URL of the next page to fetch, or `None` once the list is exhausted.
    fn url(&self, base: &str) -> Option<String> {
        match self {
            Page::Start => Some(base.to_string()),
            Page::Next(href) => Some(href.clone()),
            Page::Done => None,
        }
    }

    fn from_response(resp: &serde_json::Value) -> Page {
        utils::parse_next_href(resp).map_or(Page::Done, Page::Next)
    }
}

pub struct API {
    token: Arc<Mutex<Token>>,
    liked_tracks_page: Page,
    my_playlists_page: Page,
    others_playlists_page: Page,
    albums_page: Page,
    following_page: Page,
    feed_page: Page,
    /// User urn → username, for feed reposters.
    user_names: HashMap<String, String>,
}

impl API {
    pub fn init(token: Arc<Mutex<Token>>) -> Self {
        Self {
            token,
            liked_tracks_page: Page::Start,
            my_playlists_page: Page::Start,
            others_playlists_page: Page::Start,
            albums_page: Page::Start,
            following_page: Page::Start,
            feed_page: Page::Start,
            user_names: HashMap::new(),
        }
    }

    /// Whether `list` has a page left to fetch.
    pub fn has_more(&self, list: List) -> bool {
        let pages: &[&Page] = match list {
            List::Likes => &[&self.liked_tracks_page],
            List::Playlists => &[&self.my_playlists_page, &self.others_playlists_page],
            List::Albums => &[&self.albums_page],
            List::Following => &[&self.following_page],
            List::Feed => &[&self.feed_page],
        };
        pages.iter().any(|page| !matches!(page, Page::Done))
    }

    pub fn token_clone(&self) -> Arc<Mutex<Token>> {
        Arc::clone(&self.token)
    }
}
