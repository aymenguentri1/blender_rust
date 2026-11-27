//! The transport buttons of the timeline header (`playback_controls()` in `space_time.py`): the aligned row of
//! `screen.frame_jump`, `screen.time_jump`, `screen.keyframe_jump` and `screen.animation_play` icon buttons.
//! Visual only (no operators yet), animation not playing: Play Reverse and Play, not Pause.
//! Deviation: Blender centres the group with `separator_spacer()`s between the autokey, snapping and frame
//! widgets, none of which are drawn yet, so the group is centred in the header instead.

use crate::area::{global_header_layout_start, Rcti};
use crate::icons::{self, Icon};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box_sides;

/// Icons of the aligned row, left to right (`row.operator(..., icon=...)` order of `playback_controls()`).
pub const TRANSPORT: [Icon; 8] = [
    Icon::Rew,
    Icon::FramePrev,
    Icon::PrevKeyframe,
    Icon::PlayReverse,
    Icon::Play,
    Icon::NextKeyframe,
    Icon::FrameNext,
    Icon::Ff,
];

const ICON_COLOR: Rgba = Rgba::hex(0xffffffff);

/// Rect of every transport button inside the timeline header `header` (window coordinates): icon-only operator
/// buttons are `UI_UNIT_X` wide and packed without spacing (`row = layout.row(align=True)`).
pub fn layout(user: &UserDef, header: Rcti) -> Vec<(Icon, Rcti)> {
    let (_, top_y, height) = global_header_layout_start(user, header.size_y() + 1);
    let w = user.widget_unit;
    let total = w * TRANSPORT.len() as i32;
    let x0 = header.xmin + (header.size_x() + 1 - total) / 2;
    TRANSPORT
        .iter()
        .enumerate()
        .map(|(i, &icon)| {
            let xmin = x0 + w * i as i32;
            (icon, Rcti { xmin, xmax: xmin + w - 1, ymin: header.ymin + top_y - height, ymax: header.ymin + top_y - 1 })
        })
        .collect()
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, header: Rcti) {
    let items = layout(user, header);
    let last = items.len() - 1;
    for (i, (icon, rect)) in items.into_iter().enumerate() {
        draw_round_box_sides(buf, width, height, user, rect, theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS, i == 0, i == last);
        icons::draw_icon(buf, width, height, user, rect, icon, ICON_COLOR);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_are_packed_and_centred() {
        let user = UserDef::from_window_scale(1.0);
        let header = Rcti { xmin: 0, xmax: 999, ymin: 1000, ymax: 1025 };
        let l = layout(&user, header);
        assert_eq!(l.len(), 8);
        assert_eq!(l[0].0, Icon::Rew);
        assert_eq!(l[7].0, Icon::Ff);
        for w in l.windows(2) {
            assert_eq!(w[1].1.xmin, w[0].1.xmax + 1);
        }
        let (left, right) = (l[0].1.xmin, 999 - l[7].1.xmax);
        assert!((left - right).abs() <= 1);
        assert_eq!((l[0].1.ymin, l[0].1.ymax), (1003, 1022));
    }
}
