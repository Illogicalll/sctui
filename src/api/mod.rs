mod utils;
mod models;
mod calls;

use std::sync::{Arc, Mutex};

use crate::auth::Token;

pub use calls::albums::fetch_album_tracks;
pub use calls::following::{fetch_following_liked_tracks, fetch_following_tracks};
pub use calls::playlists::fetch_playlist_tracks;
pub use calls::search::{
    fetch_search_albums, fetch_search_people, fetch_search_playlists, fetch_search_tracks,
};
pub use models::{Album, Artist, Playlist, Track};

pub struct API {
    token: Arc<Mutex<Token>>,
    liked_tracks_next_href: Option<String>,
    first_liked_tracks_page_fetched: bool,
    my_playlists_next_href: Option<String>,
    my_first_playlist_page_fetched: bool,
    others_playlists_next_href: Option<String>,
    others_first_playlist_page_fetched: bool,
    albums_next_href: Option<String>,
    first_albums_page_fetched: bool,
    following_next_href: Option<String>,
    first_following_page_fetched: bool,
}

impl API {
    pub fn init(token: Arc<Mutex<Token>>) -> Self {
        Self {
            token,
            liked_tracks_next_href: None,
            first_liked_tracks_page_fetched: false,
            my_playlists_next_href: None,
            my_first_playlist_page_fetched: false,
            others_playlists_next_href: None,
            others_first_playlist_page_fetched: false,
            albums_next_href: None,
            first_albums_page_fetched: false,
            following_next_href: None,
            first_following_page_fetched: false,
        }
    }

    pub fn token_clone(&self) -> Arc<Mutex<Token>> {
        Arc::clone(&self.token)
    }
}
