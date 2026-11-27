//! The panel stack of the Object tab: one `panel_frame` box per top-level panel of `object_tab_layout::PANELS`
//! (`ui_panels_end` stacking with `UI_PANEL_MARGIN_Y` between them, interface_panel.cc), with the arrow, label and
//! grip of `panel_draw_aligned_widgets`. Body heights come from the caller, the content of each panel is laid out
//! elsewhere. Coordinates of the rects are Blender's continuous ones (see `panel_frame::draw`).

#![allow(dead_code)]

use crate::area::Rcti;
use crate::font;
use crate::icons::{self, Icon};
use crate::object_tab_layout::{PanelId, PANELS};
use crate::panel_frame::{self, Frame};
use crate::theme::Rgba;
use crate::userdef::UserDef;

#[derive(Clone, Debug, PartialEq)]
pub struct PanelBox {
    pub id: PanelId,
    pub open: bool,
    /// The body rect given to `panel_frame::frame` (zero height when closed).
    pub body: Rcti,
    pub frame: Frame,
}

/// Open state of the panels: the ones whose `default_closed` is false until the user toggles them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenState(pub Vec<PanelId>);

impl Default for OpenState {
    fn default() -> Self {
        OpenState(PANELS.iter().filter(|p| !p.default_closed && p.parent.is_none()).map(|p| p.id).collect())
    }
}

impl OpenState {
    pub fn is_open(&self, id: PanelId) -> bool {
        self.0.contains(&id)
    }

    /// A click on a header.
    pub fn toggle(&mut self, id: PanelId) {
        if let Some(i) = self.0.iter().position(|&p| p == id) {
            self.0.remove(i);
        } else {
            self.0.push(id);
        }
    }
}

/// Stacks the top-level panels in the window region `window`, the first header's top edge at the continuous y
/// `top` (`ContextRows::panels_top + 1`); `body_height(id)` is the body of an open panel.
pub fn layout(user: &UserDef, window: Rcti, top: i32, state: &OpenState, body_height: &dyn Fn(PanelId) -> i32) -> Vec<PanelBox> {
    let (mx, my) = panel_frame::margins(user);
    let (x0, x1) = (window.xmin + mx, window.xmax + 1 - mx);
    let hh = panel_frame::header_height(user);
    let mut top = top;
    let mut out = Vec::new();
    for p in PANELS.iter().filter(|p| p.parent.is_none()) {
        let open = state.is_open(p.id);
        let ymax = top - hh;
        let ymin = if open { ymax - body_height(p.id) } else { ymax };
        let body = Rcti { xmin: x0, xmax: x1, ymin, ymax };
        out.push(PanelBox { id: p.id, open, body, frame: panel_frame::frame(user, body, open, false) });
        top = ymin - my;
    }
    out
}

/// The panel whose header contains `xy` (window coordinates, y up).
pub fn header_at(boxes: &[PanelBox], xy: [i32; 2]) -> Option<PanelId> {
    boxes.iter().find(|b| xy[0] >= b.frame.header_rect.xmin && xy[0] < b.frame.header_rect.xmax && xy[1] >= b.frame.header_rect.ymin && xy[1] < b.frame.header_rect.ymax).map(|b| b.id)
}

fn label(id: PanelId) -> &'static str {
    PANELS.iter().find(|p| p.id == id).map_or("", |p| p.label)
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, boxes: &[PanelBox]) {
    let unit = user.widget_unit;
    for b in boxes {
        panel_frame::draw(buf, width, height, user, &b.frame);
        let f = &b.frame;
        let icon_rect = |p: panel_frame::IconPlace| Rcti { xmin: p.x as i32, xmax: p.x as i32 + unit - 1, ymin: p.y as i32, ymax: p.y as i32 + unit - 1 };
        let with_alpha = |c: Rgba, a: f32| Rgba { a: (c.a as f32 * a) as u8, ..c };
        let arrow = if b.open { Icon::DownArrow } else { Icon::RightArrow };
        icons::draw_icon(buf, width, height, user, icon_rect(f.arrow), arrow, with_alpha(panel_frame::PANEL_TITLE, f.arrow.alpha));
        icons::draw_icon(buf, width, height, user, icon_rect(f.grip), Icon::Grip, with_alpha(panel_frame::PANEL_TITLE, f.grip.alpha));
        font::draw_label(buf, width, height, user, f.title, label(b.id), panel_frame::PANEL_TITLE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Rcti = Rcti { xmin: 28, xmax: 337, ymin: 0, ymax: 757 };
    /// Pixel row of the 787 px screenshot for a y-up value.
    fn px(y: i32) -> i32 {
        786 - y
    }
    /// The Transform body of the measured screenshot (pixel rows 129..384).
    fn body(id: PanelId) -> i32 {
        if id == PanelId::Transform { 256 } else { 0 }
    }

    #[test]
    fn the_stack_matches_the_measured_outline_rows() {
        let user = UserDef::from_window_scale(1.0);
        let top = 786 - 104 + 1;
        let b = layout(&user, WINDOW, top, &OpenState::default(), &body);
        assert_eq!(b.iter().map(|b| b.id).collect::<Vec<_>>(), [PanelId::Transform, PanelId::Relations, PanelId::Collections, PanelId::MotionPaths, PanelId::Visibility, PanelId::Display, PanelId::Animation, PanelId::CustomProps]);
        /* Top outline rows and bottom outline rows measured at x=100 of the screenshot. */
        let tops = [104, 387, 414, 441, 468, 495, 522, 549];
        let bottoms = [384, 411, 438, 465, 492, 519, 546, 573];
        for (i, b) in b.iter().enumerate() {
            let (t, bot) = (px(b.frame.border.rect.ymax - 1), px(b.frame.border.rect.ymin));
            assert_eq!((t, bot), (tops[i], bottoms[i]), "{:?}", b.id);
        }
        assert_eq!((b[0].body.xmin, b[0].body.xmax - 1), (36, 329));
    }

    #[test]
    fn toggling_a_header_opens_and_closes_its_panel() {
        let user = UserDef::from_window_scale(1.0);
        let mut st = OpenState::default();
        assert!(st.is_open(PanelId::Transform) && !st.is_open(PanelId::Relations));
        let top = 786 - 104 + 1;
        let b = layout(&user, WINDOW, top, &st, &body);
        let rel = b.iter().find(|b| b.id == PanelId::Relations).unwrap();
        let click = [100, rel.frame.header_rect.ymin + 5];
        assert_eq!(header_at(&b, click), Some(PanelId::Relations));
        assert_eq!(header_at(&b, [5, 100]), None);
        st.toggle(PanelId::Relations);
        let big = |id| if id == PanelId::Transform { 256 } else { 60 };
        let b2 = layout(&user, WINDOW, top, &st, &big);
        assert!(b2[1].open);
        assert_eq!(b2[2].frame.header_rect.ymax, b2[1].body.ymin - 2, "the next panel follows the open body");
        st.toggle(PanelId::Transform);
        assert!(!layout(&user, WINDOW, top, &st, &big)[0].open);
    }

    #[test]
    fn the_frames_are_those_of_panel_frame_at_2x() {
        let user = UserDef::from_window_scale(2.0);
        let win = Rcti { xmax: 28 + 618, ..WINDOW };
        let b = layout(&user, win, 600, &OpenState::default(), &|_| 100);
        assert_eq!(b[0].frame, panel_frame::frame(&user, b[0].body, true, false));
        assert_eq!(b[1].frame, panel_frame::frame(&user, b[1].body, false, false));
        assert_eq!(b[1].frame.header_rect.size_y(), panel_frame::header_height(&user));
    }
}
