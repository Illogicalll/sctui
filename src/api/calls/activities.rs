use std::collections::HashMap;

use reqwest::blocking::Client;

use super::super::utils::{access_token, parse_datetime, parse_str, parse_track, response_items};
use crate::api::{API, Activity, Page};

impl API {
    /// Next page of the stream of posts and reposts from followed users.
    pub fn get_activities(&mut self) -> anyhow::Result<Vec<Activity>> {
        let access_token = access_token(&self.token);

        let Some(url) = self.feed_page.take_url(
            "https://api.soundcloud.com/me/activities?limit=200&linked_partitioning=true",
        ) else {
            return Ok(Vec::new());
        };

        let client = Client::new();
        let resp: serde_json::Value = client
            .get(&url)
            .bearer_auth(&access_token)
            .send()?
            .error_for_status()?
            .json()?;

        self.feed_page = Page::from_response(&resp);

        let mut activities: Vec<Activity> =
            response_items(&resp).iter().filter_map(parse_activity).collect();

        // Reposters arrive as bare urns. Look each new one up once; on failure show the urn.
        for urn in activities.iter().filter_map(|a| a.reposter_urn.as_deref()) {
            if self.user_names.contains_key(urn) {
                continue;
            }
            let name = client
                .get(format!("https://api.soundcloud.com/users/{}", urn.replace(':', "%3A")))
                .bearer_auth(&access_token)
                .send()
                .and_then(|r| r.error_for_status())
                .and_then(|r| r.json::<serde_json::Value>())
                .map(|u| parse_str(&u, "username"))
                .unwrap_or_else(|_| urn.to_string());
            self.user_names.insert(urn.to_string(), name);
        }
        resolve_reposters(&mut activities, &self.user_names);

        Ok(activities)
    }
}

/// Swap the uploader for the reposter's name where `names` (urn → username) knows it.
pub(crate) fn resolve_reposters(activities: &mut [Activity], names: &HashMap<String, String>) {
    for activity in activities {
        if let Some(name) = activity.reposter_urn.as_ref().and_then(|urn| names.get(urn)) {
            activity.user = name.clone();
        }
    }
}

/// `None` for activity kinds the feed does not show (comments, unknown origins).
pub(crate) fn parse_activity(item: &serde_json::Value) -> Option<Activity> {
    let origin = item.get("origin")?;
    let (media, track, key) = match parse_str(origin, "kind").as_str() {
        "track" => {
            let track = parse_track(origin);
            let key = track.track_urn.clone();
            ("Track", Some(track), key)
        }
        "playlist" => {
            let playlist_type = parse_str(origin, "playlist_type");
            let media = if playlist_type.is_empty() || playlist_type.eq_ignore_ascii_case("playlist") {
                "Playlist"
            } else {
                "Album"
            };
            (media, None, parse_str(origin, "tracks_uri"))
        }
        _ => return None,
    };
    let action = if parse_str(item, "type").ends_with(":repost") {
        "Repost"
    } else {
        "Post"
    };
    let user = origin
        .get("user")
        .map(|u| parse_str(u, "username"))
        .unwrap_or_default();
    let reposter_urn = item
        .get("reposter")
        .and_then(|r| r.as_str())
        .map(str::to_string);

    Some(Activity {
        user,
        reposter_urn,
        action,
        media,
        created_at: parse_datetime(item, "created_at"),
        track,
        key,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_activity, resolve_reposters};
    use crate::api::utils::format_age;
    use serde_json::json;
    use std::collections::HashMap;

    fn track_origin() -> serde_json::Value {
        json!({
            "kind": "track",
            "title": "Await",
            "urn": "soundcloud:tracks:1",
            "duration": 90000,
            "playback_count": 1200,
            "access": "playable",
            "user": { "username": "DJ_Dave" }
        })
    }

    #[test]
    fn track_post_uses_uploader_and_carries_the_track() {
        let item = json!({
            "type": "track",
            "created_at": "2026/09/04 00:00:00 +0000",
            "origin": track_origin()
        });
        let a = parse_activity(&item).unwrap();
        assert_eq!(a.user, "DJ_Dave");
        assert_eq!(a.action, "Post");
        assert_eq!(a.media, "Track");
        assert_eq!(a.key, "soundcloud:tracks:1");
        assert_eq!(a.track.as_ref().unwrap().title, "Await");
        assert_eq!(a.created_at.to_rfc3339(), "2026-09-04T00:00:00+00:00");
    }

    #[test]
    fn track_repost_keeps_reposter_urn_for_later_resolution() {
        let item = json!({
            "type": "track:repost",
            "created_at": "2026/09/02 20:11:22 +0000",
            "reposter": "soundcloud:users:206646939",
            "origin": track_origin()
        });
        let a = parse_activity(&item).unwrap();
        assert_eq!(a.action, "Repost");
        assert_eq!(a.media, "Track");
        assert_eq!(a.reposter_urn.as_deref(), Some("soundcloud:users:206646939"));
        assert_eq!(a.user, "DJ_Dave", "uploader until the reposter name is known");
    }

    #[test]
    fn resolve_reposters_swaps_in_known_names_only() {
        let repost = json!({
            "type": "track:repost",
            "created_at": "2026/09/02 20:11:22 +0000",
            "reposter": "soundcloud:users:1",
            "origin": track_origin()
        });
        let unknown = json!({
            "type": "track:repost",
            "created_at": "2026/09/02 20:11:22 +0000",
            "reposter": "soundcloud:users:2",
            "origin": track_origin()
        });
        let post = json!({ "type": "track", "created_at": "2026/09/04 00:00:00 +0000", "origin": track_origin() });
        let mut activities: Vec<_> = [repost, unknown, post].iter().filter_map(parse_activity).collect();
        let names = HashMap::from([("soundcloud:users:1".to_string(), "selecta.".to_string())]);
        resolve_reposters(&mut activities, &names);
        assert_eq!(activities[0].user, "selecta.");
        assert_eq!(activities[1].user, "DJ_Dave");
        assert_eq!(activities[2].user, "DJ_Dave");
    }

    #[test]
    fn playlist_post_keys_on_tracks_uri_and_tells_album_from_playlist() {
        let mut item = json!({
            "type": "playlist",
            "created_at": "2026/08/25 22:36:33 +0000",
            "origin": {
                "kind": "playlist",
                "playlist_type": "ALBUM",
                "title": "LP",
                "tracks_uri": "https://api.soundcloud.com/playlists/5/tracks",
                "user": { "username": "James P" }
            }
        });
        let a = parse_activity(&item).unwrap();
        assert_eq!(a.user, "James P");
        assert_eq!(a.media, "Album");
        assert_eq!(a.key, "https://api.soundcloud.com/playlists/5/tracks");
        assert!(a.track.is_none());

        item["origin"]["playlist_type"] = json!("PLAYLIST");
        assert_eq!(parse_activity(&item).unwrap().media, "Playlist");
        item["origin"]["playlist_type"] = json!("");
        assert_eq!(parse_activity(&item).unwrap().media, "Playlist");
    }

    #[test]
    fn unknown_origin_kind_is_skipped() {
        let item = json!({ "type": "comment", "created_at": "2026/09/04 00:00:00 +0000", "origin": { "kind": "comment" } });
        assert!(parse_activity(&item).is_none());
        assert!(parse_activity(&json!({ "type": "track" })).is_none());
    }

    #[test]
    fn age_picks_the_largest_whole_unit() {
        let d = 86_400;
        assert_eq!(format_age(30), "now");
        assert_eq!(format_age(5 * 60), "5m");
        assert_eq!(format_age(3 * 3600), "3h");
        assert_eq!(format_age(2 * d), "2d");
        assert_eq!(format_age(13 * d), "1w");
        assert_eq!(format_age(45 * d), "1mo");
        assert_eq!(format_age(400 * d), "1y");
    }
}
