//! The Rename Active Object popup of F2 over the viewport (`WM_OT_call_panel` of `TOPBAR_PT_name`).
//!
//! A small menu panel (same look as `add_menu`) with a title row and a text field holding the active object's
//! name. Typing inserts at the caret; Backspace / Delete, Left / Right and Home / End edit; Enter applies and
//! Escape (or a click) cancels. Deviations: no text selection, no clipboard, no double-click word select, and
//! the field takes the printable characters of the key event only.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const FIELD: Rgba = Rgba::hex(0x282828ff);
const FIELD_OUTLINE: Rgba = Rgba::hex(0x4772b3ff);
const TITLE_TEXT: Rgba = Rgba::hex(0x999999ff);
const TEXT: Rgba = Rgba::hex(0xffffffff);
const TITLE: &str = "Rename Active Object";
const TITLE_MODIFIER: &str = "Rename Modifier";
/// `MAX_NAME` of Blender is 64 bytes including the terminator.
pub const MAX_NAME_BYTES: usize = 63;

/// The text being edited; `caret` is a byte index on a char boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    /// Window position the popup opened at (its top left corner, y up).
    pub pos: [i32; 2],
    pub text: String,
    pub caret: usize,
    /// `Some(i)`: renames modifier `i` of the active object instead of the object.
    pub modifier: Option<usize>,
}

/// What a key did to the popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Editing,
    Apply,
    Cancel,
}

impl Rename {
    /// Opens with the whole current name and the caret at its end.
    pub fn open(pos: [i32; 2], name: &str) -> Rename {
        Rename { pos, text: name.to_string(), caret: name.len(), modifier: None }
    }

    /// The popup of a modifier's name field.
    pub fn open_modifier(pos: [i32; 2], name: &str, modifier: usize) -> Rename {
        Rename { modifier: Some(modifier), ..Rename::open(pos, name) }
    }

    /// Inserts printable characters at the caret (control characters are ignored, the name stays within
    /// `MAX_NAME_BYTES`).
    pub fn insert(&mut self, s: &str) {
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.text.len() + c.len_utf8() > MAX_NAME_BYTES {
                break;
            }
            self.text.insert(self.caret, c);
            self.caret += c.len_utf8();
        }
    }

    fn prev(&self) -> usize {
        self.text[..self.caret].char_indices().next_back().map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.text[self.caret..].chars().next().map_or(self.caret, |c| self.caret + c.len_utf8())
    }

    pub fn backspace(&mut self) {
        let p = self.prev();
        self.text.replace_range(p..self.caret, "");
        self.caret = p;
    }

    pub fn delete(&mut self) {
        let n = self.next();
        self.text.replace_range(self.caret..n, "");
    }

    pub fn left(&mut self) {
        self.caret = self.prev();
    }

    pub fn right(&mut self) {
        self.caret = self.next();
    }

    pub fn home(&mut self) {
        self.caret = 0;
    }

    pub fn end(&mut self) {
        self.caret = self.text.len();
    }

    /// The name to apply: trimmed, `None` when nothing is left.
    pub fn result(&self) -> Option<String> {
        let t = self.text.trim();
        (!t.is_empty()).then(|| t.to_string())
    }
}

pub struct Layout {
    pub panel: Rcti,
    pub title: Rcti,
    pub field: Rcti,
}

/// The panel for a popup opened at `pos` in a `win_w` x `win_h` window, moved back into the window.
pub fn layout(user: &UserDef, pos: [i32; 2], win_w: i32, win_h: i32) -> Layout {
    let unit = user.widget_unit;
    let margin = (unit / 5).max(2);
    let width = (font::label_width(user, TITLE) + 2 * margin).max(12 * unit);
    let height = 2 * unit + 2 * margin;
    let mut xmin = pos[0];
    let mut ymax = pos[1];
    if xmin + width - 1 > win_w - 1 {
        xmin = win_w - width;
    }
    if ymax - height + 1 < 0 {
        ymax = height - 1;
    }
    if ymax > win_h - 1 {
        ymax = win_h - 1;
    }
    xmin = xmin.max(0);
    let panel = Rcti { xmin, xmax: xmin + width - 1, ymin: ymax - height + 1, ymax };
    let row = |i: i32| Rcti {
        xmin: panel.xmin + margin,
        xmax: panel.xmax - margin,
        ymax: panel.ymax - margin - i * unit,
        ymin: panel.ymax - margin - (i + 1) * unit + 1,
    };
    Layout { panel, title: row(0), field: row(1) }
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, l: &Layout, edit: &Rename) {
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, l.panel, OUTLINE.to_xrgb());
    let inner = Rcti { xmin: l.panel.xmin + b, xmax: l.panel.xmax - b, ymin: l.panel.ymin + b, ymax: l.panel.ymax - b };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
    font::draw_label(buf, width, height, user, l.title, if edit.modifier.is_some() { TITLE_MODIFIER } else { TITLE }, TITLE_TEXT);
    fill_rect(buf, width, height, l.field, FIELD_OUTLINE.to_xrgb());
    let f = Rcti { xmin: l.field.xmin + b, xmax: l.field.xmax - b, ymin: l.field.ymin + b, ymax: l.field.ymax - b };
    fill_rect(buf, width, height, f, FIELD.to_xrgb());
    font::draw_label(buf, width, height, user, l.field, &edit.text, TEXT);
    /* The caret sits after the text before it (label padding included). */
    let pad = font::label_width(user, "") / 2;
    let x = l.field.xmin + pad + font::string_width(user, &edit.text[..edit.caret]);
    let caret = Rcti { xmin: x, xmax: x + b - 1, ymin: f.ymin + b + 1, ymax: f.ymax - b - 1 };
    fill_rect(buf, width, height, caret, TEXT.to_xrgb());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_editing() {
        let mut r = Rename::open([0, 0], "Cube");
        assert_eq!(r.caret, 4);
        r.insert("s");
        r.left();
        r.left();
        r.insert("-");
        assert_eq!(r.text, "Cub-es");
        r.backspace();
        r.delete();
        assert_eq!(r.text, "Cubs");
        r.home();
        r.delete();
        assert_eq!(r.text, "ubs");
        r.end();
        r.right();
        assert_eq!(r.caret, 3);
        r.backspace();
        r.backspace();
        r.backspace();
        assert_eq!(r.text, "");
        assert_eq!(r.result(), None);
    }

    #[test]
    fn multibyte_and_limits() {
        let mut r = Rename::open([0, 0], "");
        r.insert("é\u{7}b");
        assert_eq!(r.text, "éb");
        r.left();
        r.left();
        assert_eq!(r.caret, 0);
        r.end();
        r.insert(&"x".repeat(100));
        assert_eq!(r.text.len(), MAX_NAME_BYTES);
        r.home();
        r.backspace();
        assert_eq!(r.text.len(), MAX_NAME_BYTES);
    }

    #[test]
    fn result_is_trimmed() {
        assert_eq!(Rename::open([0, 0], "  Hat ").result().as_deref(), Some("Hat"));
        assert_eq!(Rename::open([0, 0], "   ").result(), None);
    }

    #[test]
    fn panel_stays_in_the_window() {
        let u = UserDef::from_window_scale(1.0);
        let l = layout(&u, [990, 5], 1000, 800);
        assert!(l.panel.xmax <= 999 && l.panel.ymin >= 0);
        assert!(l.panel.isect_pt(l.field.xmin, l.field.ymin) && l.panel.isect_pt(l.field.xmax, l.field.ymax));
    }
}
