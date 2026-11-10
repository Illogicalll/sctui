use ratatui::layout::Rect;

/// A popup `percent_x` by `percent_y` of `r`, centred, but never smaller than 50x10
/// while `r` has that much room: a percentage of a small terminal is too little to read.
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let side = |len: u16, percent: u16, floor: u16| {
        ((len as u32 * percent.min(100) as u32 / 100) as u16).max(floor.min(len))
    };
    centered_rect_fixed(side(r.width, percent_x, 50), side(r.height, percent_y, 10), r)
}

pub fn centered_rect_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w) / 2);
    let y = area.y + (area.height.saturating_sub(h) / 2);
    Rect { x, y, width: w, height: h }
}
