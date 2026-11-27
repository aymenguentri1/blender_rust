//! Top bar menu labels (`TOPBAR_MT_editor_menus`, space_topbar.py: the Blender icon, File, Edit, Render, Window, Help):
//! hover highlight, click opens the pull-down under the label, clicking the open label or outside closes it, Escape
//! closes it, and hovering another label while open switches menus (`handler_region_menu()` with
//! `can_activate_other_menu()`, interface_handlers.cc). Screen coordinates, y down, scale factor passed in.

use crate::menu_submenu::Rect;

/// One header label: its rect and whether it shows text (the Blender icon menu has none).
#[derive(Clone, Copy, Debug)]
pub struct Label {
    pub rect: Rect,
    pub has_text: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Move([f32; 2]),
    /// `LEFTMOUSE` press.
    Press([f32; 2]),
    Escape,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuBar {
    /// Label under the cursor (highlight).
    pub hover: Option<usize>,
    /// Label whose menu is open.
    pub open: Option<usize>,
}

/// The pull-down block under a label (`UI_DIR_DOWN`): left aligned with the label, top at its bottom edge.
pub fn menu_rect(label: Rect, width: f32, height: f32) -> Rect {
    Rect::new(label.xmin, label.xmin + width, label.ymax, label.ymax + height)
}

/// `can_activate_other_menu()`: whether the cursor at `x` may switch from open label `cur` to `other`.
fn can_activate_other(cur: &Label, other: &Label, x: f32, open_width: f32, scale: f32) -> bool {
    if open_width > 600.0 * scale {
        return false; /* A super wide open menu never switches to neighbours. */
    }
    let mut safety = 4.0 * scale;
    if other.has_text {
        safety += 4.0 * scale;
    }
    let (left, right);
    if other.rect.xmin < cur.rect.xmin {
        /* Right to left. */
        if cur.rect.xmin - other.rect.xmax > 24.0 * scale {
            return true;
        }
        right = cur.rect.xmax;
        left = other.rect.xmax - safety;
    } else {
        /* Left to right. */
        if other.rect.xmin - cur.rect.xmax > 24.0 * scale {
            return true;
        }
        left = cur.rect.xmin;
        right = other.rect.xmin + safety;
    }
    x < left.trunc() || x > right.trunc()
}

impl MenuBar {
    /// Apply an event. `menu` is the open pull-down's rect (from `menu_rect`), `scale` the UI scale factor.
    /// Returns true when the event was consumed by the bar (opened, switched or closed a menu).
    pub fn handle(&mut self, labels: &[Label], menu: Option<Rect>, scale: f32, ev: Event) -> bool {
        let over = |p: [f32; 2]| labels.iter().position(|l| l.rect.contains(p));
        match ev {
            Event::Move(p) => {
                let inside_menu = menu.is_some_and(|m| m.contains(p));
                self.hover = if inside_menu { None } else { over(p) };
                if let (Some(cur), Some(other), false) = (self.open, self.hover, inside_menu) {
                    let width = menu.map_or(0.0, |m| m.xmax - m.xmin);
                    if other != cur && can_activate_other(&labels[cur], &labels[other], p[0], width, scale) {
                        self.open = Some(other);
                        return true;
                    }
                }
                false
            }
            Event::Press(p) => {
                if self.open.is_some() && menu.is_some_and(|m| m.contains(p)) {
                    return false; /* The menu handles its own rows. */
                }
                match (self.open, over(p)) {
                    (Some(cur), Some(i)) if i == cur => self.open = None, /* Pressing the open label exits. */
                    (_, Some(i)) => self.open = Some(i),
                    (Some(_), None) => self.open = None, /* Click outside closes. */
                    (None, None) => return false,
                }
                true
            }
            Event::Escape => self.open.take().is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Icon (0..20), File (30..70), Edit (74..114), Render (118..174) on a 20 px high bar.
    fn labels() -> Vec<Label> {
        [(0.0, 20.0, false), (30.0, 70.0, true), (74.0, 114.0, true), (118.0, 174.0, true)]
            .into_iter()
            .map(|(a, b, t)| Label { rect: Rect::new(a, b, 0.0, 20.0), has_text: t })
            .collect()
    }

    fn menu_for(bar: &MenuBar, l: &[Label]) -> Option<Rect> {
        bar.open.map(|i| menu_rect(l[i].rect, 200.0, 300.0))
    }

    fn feed(bar: &mut MenuBar, l: &[Label], ev: Event) -> bool {
        let m = menu_for(bar, l);
        bar.handle(l, m, 1.0, ev)
    }

    #[test]
    fn menu_opens_under_the_label() {
        let r = menu_rect(Rect::new(30.0, 70.0, 0.0, 20.0), 200.0, 300.0);
        assert_eq!((r.xmin, r.xmax, r.ymin, r.ymax), (30.0, 230.0, 20.0, 320.0));
    }

    #[test]
    fn hover_highlights_without_opening() {
        let (mut b, l) = (MenuBar::default(), labels());
        feed(&mut b, &l, Event::Move([50.0, 10.0]));
        assert_eq!((b.hover, b.open), (Some(1), None));
        feed(&mut b, &l, Event::Move([300.0, 10.0]));
        assert_eq!(b.hover, None);
    }

    #[test]
    fn click_opens_and_clicking_again_closes() {
        let (mut b, l) = (MenuBar::default(), labels());
        assert!(feed(&mut b, &l, Event::Press([50.0, 10.0])));
        assert_eq!(b.open, Some(1));
        assert!(feed(&mut b, &l, Event::Press([50.0, 10.0])));
        assert_eq!(b.open, None);
    }

    #[test]
    fn hovering_another_label_switches_when_open() {
        let (mut b, l) = (MenuBar::default(), labels());
        feed(&mut b, &l, Event::Press([50.0, 10.0]));
        /* Edit is 4 px right of File: still within the safety zone, so no switch yet (xmin 74 + 8 = 82). */
        assert!(!feed(&mut b, &l, Event::Move([78.0, 10.0])));
        assert_eq!(b.open, Some(1));
        assert!(feed(&mut b, &l, Event::Move([90.0, 10.0])));
        assert_eq!(b.open, Some(2));
        /* Back to File: right to left, past `other.xmax - safety` is still inside the cur label, then past it. */
        assert!(feed(&mut b, &l, Event::Move([60.0, 10.0])));
        assert_eq!(b.open, Some(1));
    }

    #[test]
    fn far_apart_labels_switch_immediately() {
        let (mut b, l) = (MenuBar::default(), labels());
        feed(&mut b, &l, Event::Press([10.0, 10.0]));
        assert_eq!(b.open, Some(0));
        assert!(feed(&mut b, &l, Event::Move([140.0, 10.0])));
        assert_eq!(b.open, Some(3));
    }

    #[test]
    fn no_switch_while_the_cursor_is_in_the_menu_and_wide_menus_never_switch() {
        let (mut b, l) = (MenuBar::default(), labels());
        feed(&mut b, &l, Event::Press([50.0, 10.0]));
        feed(&mut b, &l, Event::Move([100.0, 100.0]));
        assert_eq!((b.open, b.hover), (Some(1), None));
        let wide = Some(Rect::new(30.0, 700.0, 20.0, 320.0));
        assert!(!b.handle(&l, wide, 1.0, Event::Move([140.0, 10.0])));
        assert_eq!(b.open, Some(1));
    }

    #[test]
    fn click_outside_and_escape_close() {
        let (mut b, l) = (MenuBar::default(), labels());
        feed(&mut b, &l, Event::Press([50.0, 10.0]));
        assert!(feed(&mut b, &l, Event::Press([500.0, 400.0])));
        assert_eq!(b.open, None);
        feed(&mut b, &l, Event::Press([50.0, 10.0]));
        assert!(feed(&mut b, &l, Event::Escape));
        assert_eq!(b.open, None);
        assert!(!feed(&mut b, &l, Event::Escape));
        assert!(!feed(&mut b, &l, Event::Press([500.0, 400.0])));
    }

    #[test]
    fn click_inside_the_open_menu_is_left_to_the_menu() {
        let (mut b, l) = (MenuBar::default(), labels());
        feed(&mut b, &l, Event::Press([50.0, 10.0]));
        assert!(!feed(&mut b, &l, Event::Press([100.0, 100.0])));
        assert_eq!(b.open, Some(1));
    }
}
