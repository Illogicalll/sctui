use reqwest::blocking::Client;

use crate::api::rate_limit::{SendGated, SendGatedAsync};

use crate::auth::Token;

use super::super::utils::{
    MAX_PAGES, access_token, fetch_tracks, format_duration, parse_datetime, parse_str, parse_u64,
    response_items,
};
use crate::api::{API, Page, Playlist, Track};
use std::sync::{Arc, Mutex};

/// A playlist object from any `/playlists` endpoint, as the library shows it.
pub(crate) fn parse_playlist(playlist: &serde_json::Value, is_owned: bool) -> Playlist {
    Playlist {
        title: parse_str(playlist, "title"),
        track_count: parse_u64(playlist, "track_count").to_string(),
        duration: format_duration(parse_u64(playlist, "duration")),
        created_at: parse_datetime(playlist, "created_at"),
        tracks_uri: parse_str(playlist, "tracks_uri"),
        is_owned,
    }
}

impl API {
    pub fn get_playlists(&mut self) -> anyhow::Result<Vec<Playlist>> {
        let access_token = access_token(&self.token);
        self.next_playlists(|url| {
            Ok(Client::new().get(url).bearer_auth(&access_token).send_gated()?.json()?)
        })
    }

    /// The next page of the user's own playlists and of the ones they liked, merged.
    /// `get` fetches one URL. Both cursors move only once both pages are in: if either
    /// request fails nothing is kept, and the next call asks for the same two pages,
    /// so no page of either list is lost.
    fn next_playlists(
        &mut self,
        mut get: impl FnMut(&str) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<Vec<Playlist>> {
        let urls = [
            self.my_playlists_page.url(
                "https://api.soundcloud.com/me/playlists?linked_partitioning=true&limit=40&show_tracks=false",
            ),
            self.others_playlists_page.url(
                "https://api.soundcloud.com/me/likes/playlists?limit=40&linked_partitioning=true",
            ),
        ];

        let mut next_pages = [None, None];
        let mut playlists = Vec::new();

        for (i, url) in urls.iter().enumerate() {
            let Some(url) = url else { continue };
            let resp = get(url)?;
            next_pages[i] = Some(Page::from_response(&resp));

            if let Some(collection) = resp.get("collection").and_then(|v| v.as_array()) {
                for playlist in collection {
                    if i == 1
                        && playlist
                            .get("playlist_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            != "PLAYLIST"
                    {
                        continue;
                    }

                    playlists.push(parse_playlist(playlist, i == 0));
                }
            }
        }

        let [mine, others] = next_pages;
        if let Some(page) = mine {
            self.my_playlists_page = page;
        }
        if let Some(page) = others {
            self.others_playlists_page = page;
        }

        playlists.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(playlists)
    }
}

/// Whether a playlist or album has at least one track the API will actually play.
/// One tiny request: only playable tracks are asked for, and only the first.
pub async fn has_playable_tracks(token: Arc<Mutex<Token>>, tracks_uri: String) -> anyhow::Result<bool> {
    let access_token = access_token(&token);
    let base = if tracks_uri.starts_with("http") { tracks_uri } else { format!("https://api.soundcloud.com{tracks_uri}") };
    let sep = if base.contains('?') { '&' } else { '?' };
    let resp: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}{sep}access=playable&limit=1&linked_partitioning=true"))
        .bearer_auth(access_token)
        .send_gated()
        .await?
        .json()
        .await?;
    Ok(!response_items(&resp).is_empty())
}

/// Every track of a playlist or album, across all its pages.
pub async fn fetch_playlist_tracks(
    token: Arc<Mutex<Token>>,
    tracks_uri: String,
) -> anyhow::Result<Vec<Track>> {
    let mut url = if tracks_uri.starts_with("http") {
        tracks_uri
    } else {
        format!("https://api.soundcloud.com{}", tracks_uri)
    };
    if url.contains('?') {
        if !url.contains("linked_partitioning") {
            url.push_str("&linked_partitioning=true");
        }
        if !url.contains("limit=") {
            url.push_str("&limit=200");
        }
        if !url.contains("access=") {
            url.push_str("&access=playable,preview,blocked");
        }
    } else {
        url.push_str("?linked_partitioning=true&limit=200&access=playable,preview,blocked");
    }

    fetch_tracks(&token, url, MAX_PAGES).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::List;
    use serde_json::json;

    fn page(titles: &[&str], next: Option<&str>) -> serde_json::Value {
        let rows: Vec<_> = titles.iter().map(|t| json!({ "title": t, "playlist_type": "PLAYLIST" })).collect();
        json!({ "collection": rows, "next_href": next })
    }

    /// #13: the liked-playlists request failing must not cost a page of the user's own.
    /// #11: nor does a failure, first page included, retire either list.
    #[test]
    fn a_failed_request_loses_no_playlists() {
        let token = Token { access_token: String::new(), refresh_token: String::new(), obtained_at: 0 };
        let mut api = API::init(Arc::new(Mutex::new(token)));
        let titles = |rows: Vec<Playlist>| {
            let mut titles: Vec<String> = rows.into_iter().map(|p| p.title).collect();
            titles.sort();
            titles
        };

        // Own playlists arrive, then the liked ones fail.
        let failed = api.next_playlists(|url| {
            if url.contains("/me/playlists") { Ok(page(&["mine 1"], Some("mine-2"))) } else { anyhow::bail!("503") }
        });
        assert!(failed.is_err());
        assert!(api.has_more(List::Playlists));

        // The retry asks for the same first pages and gets both.
        let mut asked = Vec::new();
        let rows = api.next_playlists(|url| {
            asked.push(url.to_string());
            Ok(if url.contains("/me/playlists") { page(&["mine 1"], Some("mine-2")) } else { page(&["liked 1"], None) })
        });
        assert_eq!(titles(rows.unwrap()), ["liked 1", "mine 1"]);
        assert!(asked[0].contains("/me/playlists?") && asked[1].contains("/me/likes/playlists?"), "{asked:?}");

        // From here only the list with pages left is asked, until it too runs out.
        let mut asked = Vec::new();
        let rows = api.next_playlists(|url| {
            asked.push(url.to_string());
            Ok(page(&["mine 2"], None))
        });
        assert_eq!((titles(rows.unwrap()), asked), (vec!["mine 2".to_string()], vec!["mine-2".to_string()]));
        assert!(!api.has_more(List::Playlists));
        assert!(api.next_playlists(|_| panic!("nothing left to ask for")).unwrap().is_empty());
    }
}
