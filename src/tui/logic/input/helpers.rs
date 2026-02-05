use crate::tui::logic::state::{AppState, QueuedTrack};

pub(crate) fn insert_manual_queue(state: &mut AppState, queued: QueuedTrack) {
    let mut items: Vec<QueuedTrack> = state.manual_queue.drain(..).collect();
    let insert_idx = match items.iter().rposition(|item| item.user_added) {
        Some(idx) => idx + 1,
        None => 0,
    };
    items.insert(insert_idx, queued);
    state.manual_queue = items.into_iter().collect();
}
