//! Row geometry of `OBJECT_PT_transform` (properties_object.py:36) in Euler mode: Location, Rotation, Mode, Scale.
//! The panel body layout is `block_layout(Vertical, Panel, x = panelspace, w = w - 2 * panelspace)` (area.cc:3323),
//! its root column spaces items by `templatespace` (interface_layout.cc:6021, style 5); every `prop` is a
//! property-split row (`ui_split::split_row`, interface_layout.cc:2164) inside a row that also holds the lock toggle
//! or the rotation-mode menu, one `UI_UNIT_X` button after the prop's decorator column. The body is
//! `-co.y + 2 * panelspace` high (area.cc:3345-3351) and includes the child panels' headers (`ed_panel_draw`).
//! Not modelled: quaternion / axis-angle rows. Pure geometry, no drawing.

#![allow(dead_code)]

use crate::ui_split::{split_row, SplitRow};
use crate::userdef::UserDef;

/// `uiStyle::templatespace` (5) and `panelspace` (8) scaled like `style_get_dpi` (interface_style.cc:431).
pub fn template_space(user: &UserDef) -> i32 {
    (user.scale_factor * 5.0) as i32
}
pub fn panel_space(user: &UserDef) -> i32 {
    (user.scale_factor * 8.0) as i32
}

/// One button row: the split of the prop, then the trailing lock / menu button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub label: &'static str,
    /// `(ymin, ymax)` in continuous y, y up.
    pub y: (i32, i32),
    pub split: SplitRow,
    /// `(x, width)` of the lock toggle (`DECORATE_UNLOCKED`) or the rotation-mode menu (`DOWNARROW_HLT`).
    pub trailing: (i32, i32),
}

const fn l(label: &'static str) -> &'static str {
    label
}

/// Lines of the panel from the content top `top` (continuous y, y up), content x `x` and width `w`.
/// RNA array labels: the first carries the prefix (`Location X`), the others only the axis (interface_layout.cc:2174-2181).
pub fn lines(user: &UserDef, x: i32, w: i32, top: i32) -> Vec<Line> {
    let unit = user.widget_unit;
    let gap = template_space(user);
    let groups: [&[&'static str]; 4] = [
        &["Location X", "Y", "Z"],
        &["Rotation X", "Y", "Z"],
        &["Mode"],
        &["Scale X", "Y", "Z"],
    ];
    let mut out = Vec::new();
    let mut y = top;
    for (gi, g) in groups.iter().enumerate() {
        if gi > 0 {
            y -= gap;
        }
        for label in g.iter() {
            out.push(Line {
                label: l(label),
                y: (y - unit, y),
                split: split_row(x, w - unit, 0, unit, true),
                trailing: (x + w - unit, unit),
            });
            y -= unit;
        }
    }
    out
}

/// Panel body height including the headers of the closed child panels (`child_headers` of them, each
/// `panel_frame::header_height`): `-co.y + 2 * panelspace + children`.
pub fn body_height(user: &UserDef, child_header: i32, child_headers: i32) -> i32 {
    let unit = user.widget_unit;
    let gap = template_space(user);
    10 * unit + 3 * gap + 2 * panel_space(user) + child_header * child_headers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel_frame;

    #[test]
    fn body_is_the_measured_256_at_1x() {
        let user = UserDef::from_window_scale(1.0);
        assert_eq!(body_height(&user, panel_frame::header_height(&user), 1), 256);
    }

    #[test]
    fn lines_stack_with_template_space_between_items() {
        let user = UserDef::from_window_scale(1.0);
        let v = lines(&user, 36, 294, 0);
        assert_eq!(v.len(), 10);
        assert_eq!(v[0].y, (-20, 0));
        assert_eq!(v[2].y, (-60, -40), "aligned rows touch");
        assert_eq!(v[3].y, (-85, -65), "5 px between Location and Rotation");
        assert_eq!(v[6].label, "Mode");
        assert_eq!(v[9].y.0, -(10 * 20 + 15));
    }

    #[test]
    fn split_and_trailing_button_share_the_row() {
        let user = UserDef::from_window_scale(1.0);
        let r = lines(&user, 36, 294, 0)[0];
        assert_eq!(r.trailing, (36 + 274, 20));
        assert_eq!(r.split.label, (36, 101)); // int((274 - 20) * 0.4)
        assert_eq!(r.split.decorate, Some((36 + 254, 20)));
        assert_eq!(r.split.decorate.unwrap().0 + 20, r.trailing.0);
    }

    #[test]
    fn scales_with_the_window() {
        let user = UserDef::from_window_scale(2.0);
        assert_eq!(template_space(&user), 10);
        assert_eq!(body_height(&user, panel_frame::header_height(&user), 1), 2 * 256);
    }
}
