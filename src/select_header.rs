//! The tool settings header of the 3D viewport in Object Mode with the Select Box tool: the `mode` enum of
//! `view3d.select_box` as a row of icon-only buttons (`draw_settings` of `builtin.select_box`,
//! space_toolsystem_toolbar.py: `row.prop(props, "mode", text="", expand=True, icon_only=True)`).
//! Layout and drawing only; the mode is the operator's default (`SEL_OP_SET`) and always lit.

use crate::area::{global_header_layout_start, Rcti};
use crate::icons::{self, Icon};
use crate::theme::{self, Rgba};
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box_sides;

/// `select_mode_items` of `WM_operator_properties_select_operation` (wm_operator_props.cc), in order.
pub const MODES: [Icon; 5] = [Icon::SelectSet, Icon::SelectExtend, Icon::SelectSubtract, Icon::SelectDifference, Icon::SelectIntersect];

const ICON_COLOR: Rgba = Rgba::hex(0xe6e6e6ff);
const ICON_COLOR_SEL: Rgba = Rgba::hex(0xffffffff);

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub icon: Icon,
    pub rect: Rcti,
    pub selected: bool,
}

/// Lays the buttons out in the tool header region `header` (window coordinates). An icon-only enum button is one
/// `UI_UNIT_X` wide (`text_icon_width` with no text); the buttons of an aligned row touch.
pub fn layout(user: &UserDef, header: Rcti) -> Vec<Item> {
    let (offset, top_y, height) = global_header_layout_start(user, header.size_y() + 1);
    let (y0, y1) = (header.ymin + top_y - height, header.ymin + top_y - 1);
    let u = user.widget_unit;
    let x = header.xmin + offset;
    MODES
        .iter()
        .enumerate()
        .map(|(i, &icon)| {
            let xmin = x + i as i32 * u;
            Item { icon, rect: Rcti { xmin, xmax: xmin + u - 1, ymin: y0, ymax: y1 }, selected: i == 0 }
        })
        .collect()
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, items: &[Item]) {
    let last = items.len().saturating_sub(1);
    for (i, it) in items.iter().enumerate() {
        let inner = if it.selected { theme::WCOL_TOGGLE_INNER_SEL } else { theme::WCOL_TOOL_INNER };
        draw_round_box_sides(buf, width, height, user, it.rect, inner, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS, i == 0, i == last);
        icons::draw_icon(buf, width, height, user, it.rect, it.icon, if it.selected { ICON_COLOR_SEL } else { ICON_COLOR });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::area::{tool_header_rect, view3d_header_rect};

    fn region(user: &UserDef) -> Rcti {
        tool_header_rect(user, Rcti { xmin: 0, xmax: 1579, ymin: 0, ymax: 940 }).unwrap()
    }

    #[test]
    fn five_buttons_in_enum_order_one_unit_wide_touching() {
        let user = UserDef::from_window_scale(1.0);
        let items = layout(&user, region(&user));
        let icons: Vec<Icon> = items.iter().map(|i| i.icon).collect();
        assert_eq!(icons, MODES.to_vec());
        for it in &items {
            assert_eq!(it.rect.size_x() + 1, user.widget_unit);
        }
        assert!(items.windows(2).all(|p| p[1].rect.xmin == p[0].rect.xmax + 1));
        assert_eq!(items[0].rect.xmin, region(&user).xmin + (4.0 * user.scale_factor) as i32);
    }

    #[test]
    fn only_set_is_lit_and_buttons_sit_inside_the_region() {
        let user = UserDef::from_window_scale(1.0);
        let r = region(&user);
        let items = layout(&user, r);
        assert_eq!(items.iter().filter(|i| i.selected).count(), 1);
        assert!(items[0].selected);
        assert!(items.iter().all(|i| r.isect(&i.rect) == Some(i.rect)));
    }

    #[test]
    fn region_height_equals_header_height() {
        let user = UserDef::from_window_scale(1.0);
        let main = Rcti { xmin: 0, xmax: 1579, ymin: 0, ymax: 940 };
        assert_eq!(tool_header_rect(&user, main).unwrap().size_y(), view3d_header_rect(&user, main).unwrap().size_y());
    }
}
