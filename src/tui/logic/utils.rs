use std::collections::VecDeque;

use rand::seq::SliceRandom;

use crate::api::{Album, Artist, Playlist, Track};

pub fn build_search_matches(
    selected_subtab: usize,
    query: &str,
    likes: &Vec<Track>,
    playlists: &Vec<Playlist>,
    albums: &Vec<Album>,
    following: &Vec<Artist>,
) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    match selected_subtab {
        0 => likes
            .iter()
            .enumerate()
            .filter(|(_, track)| {
                track.title.to_lowercase().contains(&q)
                    || track.artists.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect(),
        1 => playlists
            .iter()
            .enumerate()
            .filter(|(_, playlist)| playlist.title.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect(),
        2 => albums
            .iter()
            .enumerate()
            .filter(|(_, album)| {
                album.title.to_lowercase().contains(&q)
                    || album.artists.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect(),
        3 => following
            .iter()
            .enumerate()
            .filter(|(_, artist)| artist.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn build_queue(
    current_idx: usize,
    tracks: &[Track],
    shuffle_enabled: bool,
) -> VecDeque<usize> {
    if tracks.is_empty() {
        return VecDeque::new();
    }

    if shuffle_enabled {
        let mut indices: Vec<usize> = tracks
            .iter()
            .enumerate()
            .filter(|(i, track)| *i != current_idx && track.is_playable())
            .map(|(i, _)| i)
            .collect();
        indices.shuffle(&mut rand::thread_rng());
        VecDeque::from(indices)
    } else {
        let indices: Vec<usize> = (current_idx + 1..tracks.len())
            .filter(|&i| tracks[i].is_playable())
            .collect();
        VecDeque::from(indices)
    }
}
