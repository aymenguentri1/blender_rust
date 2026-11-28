//! Popup menu panel for a list of `sculpt_menu::Row`s (the Sculpt, Mask and Face Sets menus of the viewport header).
//!
//! Sizing follows the popup block code of Blender: `UI_MENU_PADDING` is `0.2 * UI_UNIT_Y` (interface_intern.hh),
//! the panel is at least `UI_MENU_WIDTH_MIN` = `9 * UI_UNIT_Y` wide, every row is `UI_UNIT_Y` high and a menu
//! separator is `int(7 * UI_SCALE_FAC)` high (`Layout::separator`, interface_layout.cc). Looks like `add_menu.rs`
//! (`wcol_menu_back`, `inner_sel` under the cursor). Deviation: square corners.

use crate::area::{fill_rect, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::sculpt_menu::Row;
use crate::theme::Rgba;
use crate::userdef::UserDef;

const OUTLINE: Rgba = Rgba::hex(0x242424ff);
const INNER: Rgba = Rgba::hex(0x181818ff);
const INNER_SEL: Rgba = Rgba::hex(0x4772b3ff);
const SEP_LINE: Rgba = Rgba::hex(0x363636ff);
const ITEM_TEXT: Rgba = Rgba::hex(0xddddddff);
const ITEM_TEXT_SEL: Rgba = Rgba::hex(0xffffffff);
const ITEM_TEXT_DISABLED: Rgba = Rgba::hex(0x777777ff);

pub struct Layout {
    pub panel: Rcti,
    /// `(row rect, index into the rows)` for every row that is not a separator.
    pub items: Vec<(Rcti, usize)>,
    /// The separator rows.
    pub separators: Vec<Rcti>,
}

fn separator_height(user: &UserDef) -> i32 {
    (7.0 * user.scale_factor) as i32
}

fn label_of(row: &Row) -> &'static str {
    match row {
        Row::Separator => "",
        Row::Menu { label, .. } | Row::Operator { label, .. } => label,
    }
}

/// The panel for `rows` opened with its top left corner at window position `pos` (y up) in a `win_w` x `win_h` window.
pub fn layout(user: &UserDef, rows: &[Row], pos: [i32; 2], win_w: i32, win_h: i32) -> Layout {
    let unit = user.widget_unit;
    let margin = (0.2 * unit as f32) as i32;
    let sep_h = separator_height(user);
    /* One unit of icon (checkbox) column left of every label and one unit of arrow column right of it. */
    let widest = rows
        .iter()
        .map(|r| font::label_width(user, label_of(r)))
        .max()
        .unwrap_or(0);
    let width = (widest + 2 * unit + 2 * margin).max(9 * unit);
    let height: i32 = rows
        .iter()
        .map(|r| {
            if matches!(r, Row::Separator) {
                sep_h
            } else {
                unit
            }
        })
        .sum::<i32>()
        + 2 * margin;

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
    let panel = Rcti {
        xmin,
        xmax: xmin + width - 1,
        ymin: ymax - height + 1,
        ymax,
    };

    let mut y = panel.ymax - margin;
    let (mut items, mut separators) = (Vec::new(), Vec::new());
    for (i, r) in rows.iter().enumerate() {
        let h = if matches!(r, Row::Separator) {
            sep_h
        } else {
            unit
        };
        let rect = Rcti {
            xmin: panel.xmin + margin,
            xmax: panel.xmax - margin,
            ymax: y,
            ymin: y - h + 1,
        };
        if matches!(r, Row::Separator) {
            separators.push(rect);
        } else {
            items.push((rect, i));
        }
        y -= h;
    }
    Layout {
        panel,
        items,
        separators,
    }
}

/// The row index under the window position `cursor`.
pub fn item_at(layout: &Layout, cursor: [i32; 2]) -> Option<usize> {
    layout
        .items
        .iter()
        .find(|(r, _)| r.isect_pt_v(cursor))
        .map(|&(_, i)| i)
}

/// Draws the menu into the `0x00RRGGBB` buffer; `enabled(row)` tells whether the operator can run (disabled rows are
/// drawn dim and never highlighted).
pub fn draw(
    buf: &mut [u32],
    width: usize,
    height: usize,
    user: &UserDef,
    rows: &[Row],
    layout: &Layout,
    cursor: [i32; 2],
    enabled: &dyn Fn(&Row) -> bool,
) {
    let b = user.pixelsize.max(1);
    fill_rect(buf, width, height, layout.panel, OUTLINE.to_xrgb());
    let inner = Rcti {
        xmin: layout.panel.xmin + b,
        xmax: layout.panel.xmax - b,
        ymin: layout.panel.ymin + b,
        ymax: layout.panel.ymax - b,
    };
    fill_rect(buf, width, height, inner, INNER.to_xrgb());
    for sep in &layout.separators {
        let mid = (sep.ymin + sep.ymax) / 2;
        fill_rect(
            buf,
            width,
            height,
            Rcti {
                ymin: mid,
                ymax: mid + b - 1,
                ..*sep
            },
            SEP_LINE.to_xrgb(),
        );
    }
    let hover = item_at(layout, cursor);
    let unit = user.widget_unit;
    for &(rect, i) in &layout.items {
        let row = &rows[i];
        let on = enabled(row);
        let selected = on && hover == Some(i);
        if selected {
            fill_rect(buf, width, height, rect, INNER_SEL.to_xrgb());
        }
        let color = if !on {
            ITEM_TEXT_DISABLED
        } else if selected {
            ITEM_TEXT_SEL
        } else {
            ITEM_TEXT
        };
        if let Row::Operator {
            checkbox: Some(checked),
            ..
        } = row
        {
            let icon = if *checked {
                Icon::CheckboxHlt
            } else {
                Icon::CheckboxDehlt
            };
            icons::draw_icon(
                buf,
                width,
                height,
                user,
                Rcti {
                    xmax: rect.xmin + unit - 1,
                    ..rect
                },
                icon,
                color,
            );
        }
        font::draw_label(
            buf,
            width,
            height,
            user,
            Rcti {
                xmin: rect.xmin + unit,
                ..rect
            },
            label_of(row),
            color,
        );
        if matches!(row, Row::Menu { .. }) {
            icons::draw_icon(
                buf,
                width,
                height,
                user,
                Rcti {
                    xmin: rect.xmax - unit + 1,
                    ..rect
                },
                Icon::RightArrow,
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    fn rows() -> Vec<Row> {
        crate::sculpt_menu::rows(false)
    }

    #[test]
    fn panel_is_wide_enough_and_rows_are_one_unit() {
        let u = user();
        let r = rows();
        let l = layout(&u, &r, [100, 500], 1200, 800);
        assert!(l.panel.size_x() >= 9 * u.widget_unit);
        for (rect, _) in &l.items {
            assert_eq!(rect.size_y() + 1, u.widget_unit);
        }
        assert_eq!(l.items.len() + l.separators.len(), r.len());
    }

    #[test]
    fn separators_are_thinner_than_rows() {
        let u = user();
        let l = layout(&u, &rows(), [100, 500], 1200, 800);
        assert!(!l.separators.is_empty());
        for s in &l.separators {
            assert_eq!(s.size_y() + 1, (7.0 * u.scale_factor) as i32);
            assert!(s.size_y() + 1 < u.widget_unit);
        }
    }

    #[test]
    fn rows_stack_downwards_in_order_without_gaps() {
        let u = user();
        let r = rows();
        let l = layout(&u, &r, [100, 500], 1200, 800);
        let mut rects: Vec<(usize, Rcti)> = l
            .items
            .iter()
            .map(|&(rc, i)| (i, rc))
            .chain(l.separators.iter().map(|rc| (usize::MAX, *rc)))
            .collect();
        rects.sort_by_key(|(_, rc)| -rc.ymax);
        for w in rects.windows(2) {
            assert_eq!(w[0].1.ymin - 1, w[1].1.ymax);
        }
        let idx: Vec<usize> = l.items.iter().map(|&(_, i)| i).collect();
        assert!(idx.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn item_at_finds_the_row_under_the_cursor() {
        let u = user();
        let l = layout(&u, &rows(), [100, 500], 1200, 800);
        let (rect, i) = l.items[2];
        assert_eq!(
            item_at(&l, [rect.xmin + 3, (rect.ymin + rect.ymax) / 2]),
            Some(i)
        );
        assert_eq!(item_at(&l, [0, 0]), None);
        let sep = l.separators[0];
        assert_eq!(item_at(&l, [sep.xmin + 3, sep.ymin]), None);
    }

    #[test]
    fn panel_is_clamped_into_the_window() {
        let u = user();
        let l = layout(&u, &rows(), [1190, 20], 1200, 800);
        assert!(l.panel.xmax <= 1199 && l.panel.xmin >= 0);
        assert!(l.panel.ymin >= 0 && l.panel.ymax <= 799);
    }

    #[test]
    fn hover_highlights_only_enabled_rows() {
        let u = user();
        let r = rows();
        let (w, h) = (1200usize, 800usize);
        let l = layout(&u, &r, [100, 500], w as i32, h as i32);
        let (rect, _) = l.items[0];
        let cur = [rect.xmin + 3, (rect.ymin + rect.ymax) / 2];
        let px = |buf: &Vec<u32>| buf[(h as i32 - 1 - cur[1]) as usize * w + cur[0] as usize];
        let mut a = vec![0u32; w * h];
        draw(&mut a, w, h, &u, &r, &l, cur, &|_| true);
        let mut b = vec![0u32; w * h];
        draw(&mut b, w, h, &u, &r, &l, cur, &|_| false);
        assert_eq!(px(&a), INNER_SEL.to_xrgb());
        assert_ne!(px(&b), INNER_SEL.to_xrgb());
    }
}
