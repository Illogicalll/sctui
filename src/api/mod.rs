mod utils;
mod models;
mod calls;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::auth::Token;

pub(crate) use calls::engagement::engage;
pub(crate) use calls::following::fetch_user_tracks;
pub use calls::playlists::fetch_playlist_tracks;
pub use calls::search::{
    fetch_search_albums, fetch_search_people, fetch_search_playlists, fetch_search_tracks,
};
pub use models::{Activity, Album, Artist, Playlist, Track};
pub(crate) use utils::format_duration;

/// Cursor over one `linked_partitioning` list.
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

    /// Like `url`, but `Start` becomes `Done` before the request is made, so a
    /// failed first fetch is not retried.
    fn take_url(&mut self, base: &str) -> Option<String> {
        let url = self.url(base);
        if matches!(self, Page::Start) {
            *self = Page::Done;
        }
        url
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

    pub fn token_clone(&self) -> Arc<Mutex<Token>> {
        Arc::clone(&self.token)
    }
}
