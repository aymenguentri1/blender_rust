//! The frame of a panel (`draw_aligned_panel`, `panel_draw_aligned_backdrop`, `panel_draw_aligned_widgets`,
//! `panel_draw_border`, interface_panel.cc:1139-1440) as plain geometry and colours, so it is tested without a window.
//! Constants: `PNL_HEADER = UI_UNIT_Y * 1.25` (interface_intern.hh:156), `UI_PANEL_MARGIN_X/Y`
//! (UI_interface_c.hh:461), the `panel_*` theme colours and `panel_roundness = 0.4` (userdef_default_theme.c:283).
//! Panels are drawn at aspect 1 (no zoom). Not modelled: the drop shadow (region overlap / dragging), the pin icon,
//! search highlight and layout-panel backdrops.

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::userdef::UserDef;

pub const PANEL_HEADER: Rgba = Rgba::hex(0x3d3d3dff);
pub const PANEL_BACK: Rgba = Rgba::hex(0x3d3d3dff);
pub const PANEL_SUB_BACK: Rgba = Rgba::hex(0x0000001f);
pub const PANEL_OUTLINE: Rgba = Rgba::hex(0xffffff11);
pub const PANEL_TITLE: Rgba = Rgba::hex(0xe6e6e6ff);
pub const PANEL_ACTIVE: Rgba = Rgba::hex(0x4772b3ff);
pub const PANEL_ROUNDNESS: f32 = 0.4;

/// Corners of `draw_roundbox_corner_set`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corners {
    All,
    Top,
    Bottom,
}

/// A filled rounded box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundBox {
    pub rect: Rcti,
    pub radius: f32,
    pub corners: Corners,
    pub color: Rgba,
}

/// The collapse arrow / drag grip: the icon origin and its alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconPlace {
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub header_rect: Rcti,
    /// Backdrop (open panels) and header boxes, in drawing order.
    pub boxes: Vec<RoundBox>,
    /// The outline: the whole panel when open, the header alone when closed (`panel_draw_border`).
    pub border: RoundBox,
    pub arrow: IconPlace,
    pub grip: IconPlace,
    /// The label rectangle (`title_rect`).
    pub title: Rcti,
}

/// `PNL_HEADER`.
pub fn header_height(user: &UserDef) -> i32 {
    (user.widget_unit as f32 * 1.25 + 0.001).floor() as i32
}

/// `UI_PANEL_MARGIN_X`, `UI_PANEL_MARGIN_Y`.
pub fn margins(user: &UserDef) -> (i32, i32) {
    ((user.widget_unit as f32 * 0.4) as i32, (user.widget_unit as f32 * 0.1) as i32)
}

/// `draw_aligned_panel` for a top-level panel with header and background; `rect` is the body (below the header),
/// `active` is `panel_custom_data_active_get`.
pub fn frame(user: &UserDef, rect: Rcti, open: bool, active: bool) -> Frame {
    let unit = user.widget_unit;
    let radius = PANEL_ROUNDNESS * unit as f32 * 0.5;
    let header_rect = Rcti { xmin: rect.xmin, xmax: rect.xmax, ymin: rect.ymax, ymax: rect.ymax + header_height(user) };
    let mut boxes = Vec::new();
    if open {
        boxes.push(RoundBox { rect, radius, corners: Corners::Bottom, color: PANEL_BACK });
    }
    boxes.push(RoundBox { rect: header_rect, radius, corners: if open { Corners::Top } else { Corners::All }, color: PANEL_HEADER });
    let border_rect = Rcti { ymin: if open { rect.ymin } else { header_rect.ymin }, ymax: header_rect.ymax, ..rect };
    let border = RoundBox { rect: border_rect, radius, corners: Corners::All, color: if active { PANEL_ACTIVE } else { PANEL_OUTLINE } };

    let (hw, hh) = (header_rect.size_x(), header_rect.size_y());
    let size_y = hh as f32;
    let mut arrow_alpha = 0.8;
    if hw < unit * 4 {
        arrow_alpha *= ((hw - unit) as f32 / (unit * 3) as f32).max(0.0);
    }
    let arrow = IconPlace { x: header_rect.xmin as f32 + size_y * 0.2, y: header_rect.ymin as f32 + size_y * if open { 0.14 } else { 0.17 }, alpha: arrow_alpha };
    let mut grip_alpha = 0.5;
    if hw < unit * 5 {
        grip_alpha *= ((hw - unit) as f32 / (unit * 4) as f32).max(0.0);
    }
    let grip = IconPlace { x: header_rect.xmax as f32 - unit as f32 * 1.15, y: header_rect.ymin as f32 + (hh as f32 - hh as f32 * 0.7) * 0.5, alpha: grip_alpha };
    /* `labelofs` is 0 for these panels. */
    let title = Rcti {
        xmin: header_rect.xmin + (unit as f32 * 1.1).round() as i32,
        xmax: header_rect.xmax - unit,
        ymin: header_rect.ymin - 2,
        ymax: header_rect.ymax,
    };
    Frame { header_rect, boxes, border, arrow, grip, title }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    const BODY: Rcti = Rcti { xmin: 10, xmax: 309, ymin: 100, ymax: 199 };

    #[test]
    fn the_header_sits_on_the_body_and_is_one_and_a_quarter_units_high() {
        let f = frame(&user(), BODY, true, false);
        assert_eq!(header_height(&user()), 25);
        assert_eq!(f.header_rect, Rcti { xmin: 10, xmax: 309, ymin: 199, ymax: 224 });
        assert_eq!(margins(&user()), (8, 2));
    }

    #[test]
    fn open_panels_have_a_bottom_rounded_body_and_top_rounded_header() {
        let f = frame(&user(), BODY, true, false);
        assert_eq!(f.boxes.len(), 2);
        assert_eq!((f.boxes[0].rect, f.boxes[0].corners, f.boxes[0].color), (BODY, Corners::Bottom, PANEL_BACK));
        assert_eq!((f.boxes[1].corners, f.boxes[1].color), (Corners::Top, PANEL_HEADER));
        assert_eq!(f.boxes[1].radius, 4.0);
        assert_eq!(f.border.rect, Rcti { ymin: 100, ymax: 224, ..BODY });
        assert_eq!(f.border.color, PANEL_OUTLINE);
        assert_eq!(f.arrow.y, 199.0 + 25.0 * 0.14);
    }

    #[test]
    fn closed_panels_are_just_a_fully_rounded_header() {
        let f = frame(&user(), BODY, false, true);
        assert_eq!(f.boxes.len(), 1);
        assert_eq!(f.boxes[0].corners, Corners::All);
        assert_eq!(f.border.rect, f.header_rect);
        assert_eq!(f.border.color, PANEL_ACTIVE);
        assert_eq!(f.arrow.y, 199.0 + 25.0 * 0.17);
    }

    #[test]
    fn arrow_label_and_grip_follow_the_header() {
        let f = frame(&user(), BODY, true, false);
        assert_eq!((f.arrow.x, f.arrow.alpha, f.grip.alpha), (10.0 + 25.0 * 0.2, 0.8, 0.5));
        assert_eq!(f.grip.x, 309.0 - 23.0);
        assert_eq!(f.title, Rcti { xmin: 10 + 22, xmax: 309 - 20, ymin: 197, ymax: 224 });
        /* Dimmed when the width runs out. */
        let narrow = frame(&user(), Rcti { xmax: 10 + 59, ..BODY }, true, false);
        assert!(narrow.arrow.alpha < 0.8 && narrow.grip.alpha < 0.5);
    }
}
