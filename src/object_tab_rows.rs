//! Property-split rows of `OBJECT_PT_transform` (properties_object.py:36): each row is a `split(PROPERTY_SPLIT_FACTOR)`
//! of label column and value column plus a one-unit column on the right (the `lock_*` toggle, or the menu next to
//! Mode). Items of the panel root are `templatespace` apart (block_layout, interface_layout.cc:6022), the body
//! starts `panelspace` below the panel top (offset_panel_block, interface_panel.cc:886). Vector values are an
//! aligned column of `UI_UNIT_Y` rows (item_array, :2230). Rects are Blender's y-up `Rcti`. Nothing is drawn here.

#![allow(dead_code)]

use crate::area::Rcti;
use crate::object_tab_layout::RotationMode;
use crate::userdef::UserDef;

/// `Layout::PROPERTY_SPLIT_FACTOR` (UI_interface_layout.hh:123).
pub const SPLIT_FACTOR: f32 = 0.4;
/// `uiStyle::panelspace` and `templatespace` at 1x (interface_style.cc:88, 92).
const PANEL_SPACE: f32 = 8.0;
const TEMPLATE_SPACE: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitRow {
    pub prop: &'static str,
    /// Label column; the label sits on its first line.
    pub label: Rcti,
    /// Value column, `lines` buttons stacked and aligned.
    pub value: Rcti,
    /// Right-hand column of one unit: `lock_*` toggles (one per line) or the Mode menu.
    pub side: Rcti,
    pub lines: i32,
}

fn scaled(user: &UserDef, v: f32) -> i32 {
    (v * user.scale_factor) as i32
}

/// Splits `row` (width `w`, one unit reserved for the side column) like `LayoutSplit` with `percentage`.
fn split_row(user: &UserDef, prop: &'static str, x0: i32, x1: i32, top: i32, lines: i32) -> SplitRow {
    let unit = user.widget_unit;
    let side_x = x1 - unit + 1;
    let w = side_x - x0;
    let lw = (w as f32 * SPLIT_FACTOR) as i32;
    let (ymax, ymin) = (top, top - lines * unit + 1);
    let col = |xmin: i32, xmax: i32| Rcti { xmin, xmax, ymin, ymax };
    SplitRow { prop, label: col(x0, x0 + lw - 1), value: col(x0 + lw, side_x - 1), side: col(side_x, x1), lines }
}

/// Height of the four row groups plus `templatespace` between them, without the child panel.
pub fn rows_height(user: &UserDef, mode: RotationMode) -> i32 {
    let lines = [3, if mode == RotationMode::Quaternion { 4 } else { 3 }, 1, 3];
    lines.iter().sum::<i32>() * user.widget_unit + 3 * scaled(user, TEMPLATE_SPACE)
}

/// The rows of the Transform body `body` (x range of the panel, `ymax` its top edge): Location, Rotation, Mode, Scale.
pub fn transform_rows(user: &UserDef, body: Rcti, mode: RotationMode) -> Vec<SplitRow> {
    let gap = scaled(user, TEMPLATE_SPACE);
    let rot = match mode {
        RotationMode::Quaternion => ("rotation_quaternion", 4),
        RotationMode::AxisAngle => ("rotation_axis_angle", 4),
        RotationMode::Euler => ("rotation_euler", 3),
    };
    let mut top = body.ymax - scaled(user, PANEL_SPACE);
    let mut out = Vec::new();
    for (prop, lines) in [("location", 3), rot, ("rotation_mode", 1), ("scale", 3)] {
        let (x0, x1) = (body.xmin + scaled(user, PANEL_SPACE), body.xmax - scaled(user, PANEL_SPACE));
        out.push(split_row(user, prop, x0, x1, top, lines));
        top -= lines * user.widget_unit + gap;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: Rcti = Rcti { xmin: 36, xmax: 329, ymin: 0, ymax: 600 };

    #[test]
    fn rows_stack_with_templatespace_and_unit_lines() {
        let user = UserDef::from_window_scale(1.0);
        let r = transform_rows(&user, BODY, RotationMode::Euler);
        assert_eq!(r.iter().map(|x| x.lines).collect::<Vec<_>>(), [3, 3, 1, 3]);
        assert_eq!(r[0].value.size_y() + 1, 3 * 20);
        assert_eq!(r[0].value.ymin - 5 - 1, r[1].value.ymax, "templatespace between rows");
        assert_eq!(r[2].label.ymax - r[2].label.ymin + 1, 20);
    }

    #[test]
    fn columns_follow_the_split_factor_and_a_unit_side() {
        let user = UserDef::from_window_scale(1.0);
        let r = transform_rows(&user, BODY, RotationMode::Euler)[0];
        assert_eq!(r.side.xmax - r.side.xmin + 1, 20);
        assert_eq!(r.label.xmax + 1, r.value.xmin);
        assert_eq!(r.value.xmax + 1, r.side.xmin);
        let w = r.side.xmin - r.label.xmin;
        assert_eq!(r.label.xmax - r.label.xmin + 1, (w as f32 * 0.4) as i32);
    }

    #[test]
    fn quaternion_has_four_lines_and_the_body_adds_up_to_the_measured_256() {
        let user = UserDef::from_window_scale(1.0);
        assert_eq!(transform_rows(&user, BODY, RotationMode::Quaternion)[1].lines, 4);
        /* 200 + 3*5 + Delta Transform header 25 + 2*panelspace = 256 (measured body, object_tab_panels tests). */
        assert_eq!(rows_height(&user, RotationMode::Euler) + 25 + 2 * 8, 256);
    }
}
