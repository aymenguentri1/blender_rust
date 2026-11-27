//! Popup menu drawing ported from `interface_widgets.cc`: `widget_menu_back`, `widget_menu_itembut`,
//! `widget_state_menu_item`, `widget_draw_text_icon` (menu rows), `widget_draw_submenu_tria`, `draw_separator`.
//! Check boxes are the `ICON_CHECKBOX_*` icons `interface_layout.cc` gives boolean menu rows.
//! Not ported yet: the soft shadow, text clipping and the underlined accelerator key.

use crate::area::Rcti;
use crate::font;
use crate::icons::{self, Icon};
use crate::theme::Rgba;
use crate::theme_widgets::{WCOL_MENU_BACK, WCOL_MENU_ITEM};
use crate::userdef::UserDef;
use crate::widget_colors::{Col4, WidgetColors};
use crate::widget_draw::{blend_px, draw_round_box, draw_round_box_corners};

/// The button flags `widget_state_menu_item` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct ItemState {
    pub hover: bool,
    pub disabled: bool,
    pub inactive: bool,
    /// `BUT_ACTIVE_DEFAULT | UI_SELECT_DRAW`.
    pub current: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Item<'a> {
    pub label: &'a str,
    pub shortcut: Option<&'a str>,
    pub icon: Option<Icon>,
    /// A boolean row: replaces `icon` with the check box icon.
    pub checked: Option<bool>,
    pub submenu: bool,
    pub state: ItemState,
}

/// `color_blend_3ubv_3ubv`: RGB only, truncating.
fn blend3(col: &mut Col4, other: Col4, fac: f32) {
    for i in 0..3 {
        col[i] = ((1.0 - fac) * col[i] as f32 + fac * other[i] as f32) as u8;
    }
}

/// `widget_state_menu_item`.
pub fn item_colors(s: ItemState) -> WidgetColors {
    let mut w = WCOL_MENU_ITEM;
    if s.disabled && s.hover {
        w.text[3] = 128;
        let text = w.text;
        blend3(&mut w.inner, text, 0.5);
        w.inner[3] = 64;
    } else if s.disabled {
        w.text[3] = 128;
    } else if s.inactive {
        if s.hover {
            let text = w.text;
            blend3(&mut w.inner, text, 0.2);
            w.text[..3].copy_from_slice(&w.text_sel[..3]);
            w.inner[3] = 255;
        }
        let inner = w.inner;
        blend3(&mut w.text, inner, 0.5);
    } else if s.current {
        w.inner = w.inner_sel;
        w.outline = w.outline_sel;
        w.text = w.text_sel;
    } else if s.hover {
        let (text, outline_sel) = (w.text, w.outline_sel);
        blend3(&mut w.inner, text, 0.2);
        blend3(&mut w.outline, outline_sel, 0.5);
        w.text[..3].copy_from_slice(&w.text_sel[..3]);
        w.inner[3] = 255;
        w.text[3] = 255;
    }
    w
}

fn rgba(c: Col4) -> Rgba {
    Rgba { r: c[0], g: c[1], b: c[2], a: c[3] }
}

/// `widget_menu_back` corners, `[bottom left, bottom right, top left, top right]`. `pulldown` is `None` for a
/// `BLOCK_POPUP` block, else `(opens down, to the left, to the right)` of a menu opened from a button.
pub fn back_corners(pulldown: Option<(bool, bool, bool)>) -> [bool; 4] {
    let Some((down, left, right)) = pulldown else { return [true; 4] };
    let mut c = [down, down, !down, !down];
    if left {
        c[3] = true;
        c[1] = true;
    }
    if right {
        c[2] = true;
        c[0] = true;
    }
    c
}

pub fn draw_back(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti, corners: [bool; 4]) {
    let w = WCOL_MENU_BACK;
    draw_round_box_corners(buf, width, height, user, rect, rgba(w.inner), rgba(w.outline), w.roundness, corners);
}

/// One menu row. `has_submenu` is `BLOCK_CONTAINS_SUBMENU_BUT`: it indents every shortcut.
pub fn draw_item(buf: &mut [u32], width: usize, height: usize, user: &UserDef, row: Rcti, item: &Item, has_submenu: bool) {
    let wcol = item_colors(item.state);
    let unit = user.widget_unit as f32;
    let scale = user.scale_factor;
    let mut rect = row;
    let side = (0.125 * unit) as i32;
    rect.xmin += side;
    rect.xmax -= side;
    draw_round_box(buf, width, height, user, rect, rgba(wcol.inner), rgba(wcol.outline), wcol.roundness);

    let text = rgba(wcol.text);
    let icon = match item.checked {
        Some(true) => Some(Icon::CheckboxHlt),
        Some(false) => Some(Icon::CheckboxDehlt),
        None => item.icon,
    };
    if icon.is_some() || item.submenu {
        if !item.label.is_empty() {
            rect.xmin += (0.2 * unit) as i32;
        }
        let size = icons::icon_size(user);
        if let Some(icon) = icon {
            let x = rect.xmin + (4.0 * scale).round() as i32;
            let at = Rcti { xmin: x, xmax: x + size - 1, ymin: rect.ymin, ymax: rect.ymax };
            icons::draw_icon(buf, width, height, user, at, icon, text);
        }
        if item.submenu {
            draw_submenu_tria(buf, width, height, user, rect, wcol.text);
        }
        rect.xmin += (size as f32 + 2.0 * scale).round() as i32;
    }
    if has_submenu {
        rect.xmax -= (6.0 * scale) as i32;
    }
    /* `UI_TEXT_MARGIN_X * U.widget_unit` */
    rect.xmin += (0.4 * unit) as i32;
    font::draw_label_pad(buf, width, height, user, rect, item.label, text, 0);
    if let Some(hint) = item.shortcut {
        /* Right aligned at `UI_TEXT_CLIP_MARGIN`, half the text alpha. */
        rect.xmax -= (0.25 * unit) as i32;
        let color = Rgba { a: text.a / 2, ..text };
        let pad = rect.xmax + 1 - font::string_width(user, hint) - rect.xmin;
        font::draw_label_pad(buf, width, height, user, rect, hint, color, pad);
    }
}

/// `widget_draw_submenu_tria` + `draw_anti_tria_rect(.., 'h', ..)`: a right pointing triangle, 4 x 4 samples.
fn draw_submenu_tria(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti, col: Col4) {
    let th = icons::icon_size(user);
    let tw = th - 2 * user.pixelsize;
    let xs = rect.xmax + 1 - tw;
    let ys = (rect.ymin + rect.ymax + 1 - th) / 2;
    /* `BLI_rctf_scale(0.4)` keeps the centre. */
    let (cx, cy) = (xs as f32 + tw as f32 * 0.5, ys as f32 + th as f32 * 0.5);
    let (hw, hh) = (tw as f32 * 0.2, th as f32 * 0.2);
    let alpha = col[3] as f32 / 255.0 * 0.8;
    for y in ys.max(0)..(ys + th).min(height as i32) {
        for x in xs.max(0)..(xs + tw).min(width as i32) {
            let mut hit = 0;
            for s in 0..16 {
                let dx = x as f32 + ((s % 4) as f32 + 0.5) / 4.0 - (cx - hw);
                let dy = y as f32 + ((s / 4) as f32 + 0.5) / 4.0 - cy;
                if dx >= 0.0 && dy.abs() <= hh * (1.0 - dx / (2.0 * hw)) {
                    hit += 1;
                }
            }
            if hit > 0 {
                let px = &mut buf[(height - 1 - y as usize) * width + x as usize];
                *px = blend_px(*px, rgba(col), alpha * hit as f32 / 16.0);
            }
        }
    }
}

/// `draw_separator`: a one pixel line in the item text colour with alpha 30, inset `7 * UI_SCALE_FAC` in a menu.
pub fn draw_separator(buf: &mut [u32], width: usize, height: usize, user: &UserDef, row: Rcti) {
    let inset = (7.0 * user.scale_factor) as i32;
    let y = (row.ymin + row.ymax + 1) / 2;
    if y < 0 || y as usize >= height {
        return;
    }
    let color = Rgba { a: 30, ..rgba(WCOL_MENU_ITEM.text) };
    for x in (row.xmin + inset).max(0)..=(row.xmax - inset).min(width as i32 - 1) {
        let px = &mut buf[(height - 1 - y as usize) * width + x as usize];
        *px = blend_px(*px, color, 30.0 / 255.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 200;
    const H: usize = 20;
    const BG: u32 = 0x0018_1818;
    const ROW: Rcti = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 19 };

    fn render(item: &Item, has_submenu: bool) -> Vec<u32> {
        let mut buf = vec![BG; W * H];
        draw_item(&mut buf, W, H, &UserDef::from_window_scale(1.0), ROW, item, has_submenu);
        buf
    }
    fn red(p: u32) -> u32 {
        p >> 16 & 0xff
    }

    #[test]
    fn state_colors() {
        let hover = item_colors(ItemState { hover: true, ..Default::default() });
        /* 0.8 * 0x18 + 0.2 * 0xdd, truncated. */
        assert_eq!(hover.inner, [63, 63, 63, 255]);
        assert_eq!(hover.text, [255; 4]);
        let current = item_colors(ItemState { current: true, ..Default::default() });
        assert_eq!(current.inner, WCOL_MENU_ITEM.inner_sel);
        let off = item_colors(ItemState { disabled: true, ..Default::default() });
        assert_eq!(off.text, [0xdd, 0xdd, 0xdd, 128]);
        let off_hover = item_colors(ItemState { disabled: true, hover: true, ..Default::default() });
        assert_eq!(off_hover.inner[3], 64);
    }

    #[test]
    fn hovered_and_current_rows_are_filled() {
        let at = 10 * W + 100;
        assert_eq!(render(&Item::default(), false)[at], BG);
        let hover = Item { state: ItemState { hover: true, ..Default::default() }, ..Default::default() };
        assert_eq!(render(&hover, false)[at], 0x003f_3f3f);
        let current = Item { state: ItemState { current: true, ..Default::default() }, ..Default::default() };
        assert_eq!(render(&current, false)[at], 0x0047_72b3);
    }

    #[test]
    fn disabled_text_is_dimmer() {
        if font::string_width(&UserDef::from_window_scale(1.0), "Open") == 0 {
            return;
        }
        let brightest = |disabled| {
            let item = Item { label: "Open", state: ItemState { disabled, ..Default::default() }, ..Default::default() };
            render(&item, false).iter().map(|&p| red(p)).max().unwrap()
        };
        assert!(brightest(false) > brightest(true));
        assert!(brightest(true) <= (0xdd + 0x18) / 2 + 1);
    }

    #[test]
    fn submenu_arrow_and_shortcut_are_on_the_right() {
        let changed = |item: &Item| {
            let buf = render(item, item.submenu);
            (0..W).filter(|&x| (0..H).any(|y| buf[y * W + x] != BG)).collect::<Vec<_>>()
        };
        let arrow = changed(&Item { submenu: true, ..Default::default() });
        assert!(!arrow.is_empty() && arrow.iter().all(|&x| x > 170), "{arrow:?}");
        if font::string_width(&UserDef::from_window_scale(1.0), "F2") > 0 {
            let hint = changed(&Item { shortcut: Some("F2"), ..Default::default() });
            assert!(!hint.is_empty() && hint.iter().all(|&x| x > 170 && x < 193), "{hint:?}");
        }
    }

    #[test]
    fn check_box_and_icon_are_on_the_left() {
        let on = render(&Item { checked: Some(true), ..Default::default() }, false);
        let off = render(&Item { checked: Some(false), ..Default::default() }, false);
        assert_ne!(on, off);
        assert!((0..H).all(|y| (40..W).all(|x| on[y * W + x] == BG)));
    }

    #[test]
    fn separator_line_is_drawn_inset() {
        let mut buf = vec![BG; W * 7];
        let row = Rcti { xmin: 0, xmax: 199, ymin: 0, ymax: 6 };
        draw_separator(&mut buf, W, 7, &UserDef::from_window_scale(1.0), row);
        let line = 3 * W;
        assert!(red(buf[line + 100]) > red(BG));
        assert_eq!((buf[line + 6], buf[line + 194]), (BG, BG));
        assert_eq!(buf.iter().filter(|&&p| p != BG).count(), 186);
    }

    #[test]
    fn back_corners_follow_direction() {
        assert_eq!(back_corners(None), [true; 4]);
        assert_eq!(back_corners(Some((true, false, false))), [true, true, false, false]);
        assert_eq!(back_corners(Some((true, false, true))), [true, true, true, false]);
        let mut buf = vec![0u32; 40 * 40];
        let rect = Rcti { xmin: 0, xmax: 39, ymin: 0, ymax: 39 };
        draw_back(&mut buf, 40, 40, &UserDef::from_window_scale(1.0), rect, [true; 4]);
        assert_eq!(buf[20 * 40 + 20], 0x0018_1818);
    }
}
