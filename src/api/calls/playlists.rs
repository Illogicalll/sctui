use chrono::{DateTime, FixedOffset, Utc};
use reqwest::blocking::Client;

use crate::auth::try_refresh_token;

use super::super::utils::{format_duration, parse_next_href, parse_str, parse_u64};
use crate::api::{API, Playlist};

impl API {
    pub fn get_playlists(&mut self) -> anyhow::Result<Vec<Playlist>> {
        let _ = try_refresh_token(&self.token);

        let token_guard = self.token.lock().unwrap();

        let should_fetch_my =
            !self.my_first_playlist_page_fetched || self.my_playlists_next_href.is_some();
        let should_fetch_others =
            !self.others_first_playlist_page_fetched || self.others_playlists_next_href.is_some();

        if !should_fetch_my && !should_fetch_others {
            return Ok(Vec::new());
        }

        let urls = vec![
            self.my_playlists_next_href.clone().unwrap_or_else(|| {
                "https://api.soundcloud.com/me/playlists?linked_partitioning=true&limit=40&show_tracks=false".to_string()
            }),
            self.others_playlists_next_href.clone().unwrap_or_else(|| {
                "https://api.soundcloud.com/me/likes/playlists?limit=40&linked_partitioning=true"
                    .to_string()
            }),
        ];

        let mut playlists = Vec::new();

        for (i, url) in urls.iter().enumerate() {
            if (i == 0 && !should_fetch_my) || (i == 1 && !should_fetch_others) {
                continue;
            }
            let resp: serde_json::Value = Client::new()
                .get(url)
                .bearer_auth(&token_guard.access_token)
                .send()?
                .error_for_status()?
                .json()?;

            let next_href = parse_next_href(&resp);
            if i == 0 {
                self.my_playlists_next_href = next_href;
                self.my_first_playlist_page_fetched = true;
            } else {
                self.others_playlists_next_href = next_href;
                self.others_first_playlist_page_fetched = true;
            }

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

                    let title = parse_str(&playlist, "title");
                    let track_count = parse_u64(&playlist, "track_count").to_string();
                    let duration = format_duration(parse_u64(&playlist, "duration"));
                    let created_at = DateTime::parse_from_str(
                        &parse_str(&playlist, "created_at"),
                        "%Y/%m/%d %H:%M:%S %z",
                    )
                    .unwrap_or_else(|_| {
                        Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap())
                    });
                    let tracks_uri = parse_str(&playlist, "tracks_uri");

                    playlists.push(Playlist {
                        title,
                        track_count,
                        duration,
                        created_at,
                        tracks_uri,
                    });
                }
            }
        }

        playlists.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(playlists)
    }
}
