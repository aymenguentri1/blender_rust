//! The scene and view layer fields of the top bar (`TOPBAR_HT_upper_bar.draw_right`, space_topbar.py:53-61):
//! `template_ID(window, "scene", new, unlink)` (`template_ID`, interface_template_id.cc:1176) and
//! `template_search(window, "view_layer", ...)` (interface_template_search.cc). Each is one aligned row: the
//! browse button (`UI_UNIT_X`), the name field (`template_search_textbut_width`, interface_templates.cc:30; the
//! scene one is `UI_UNIT_X` wider for the pin, interface_template_id.cc:1248), the new button (`ICON_DUPLICATE`)
//! and the unlink button (`ICON_X`), both `UI_UNIT_X`. Layout only plus a pure draw helper.

use crate::icons::Icon;

/// `TEMPLATE_SEARCH_TEXTBUT_MIN_WIDTH` in widget units (interface_templates_intern.hh:30).
const MIN_UNITS: i32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Browse,
    Name,
    New,
    Unlink,
}

/// `template_search_textbut_width()`: text width plus a 0.75 unit margin, clamped to 4..16 units.
pub fn textbut_width(unit: i32, text_w: i32) -> i32 {
    (text_w + (unit as f32 * 0.75) as i32).clamp(unit * MIN_UNITS, unit * MIN_UNITS * 4)
}

/// Widths of the parts of an ID field in layout order. `pin` widens the name by one unit (scene of a window).
pub fn parts(unit: i32, text_w: i32, pin: bool) -> [(Part, i32); 4] {
    let name = textbut_width(unit, text_w) + if pin { unit } else { 0 };
    [(Part::Browse, unit), (Part::Name, name), (Part::New, unit), (Part::Unlink, unit)]
}

pub fn total_width(parts: &[(Part, i32)]) -> i32 {
    parts.iter().map(|p| p.1).sum()
}

/// The rects of the parts of one field, packed left to right from `x` (`row.place_packed()`), `ymin..=ymax` tall.
pub fn rects(parts: &[(Part, i32)], x: i32, ymin: i32, ymax: i32) -> Vec<(Part, crate::area::Rcti)> {
    let mut x = x;
    parts
        .iter()
        .map(|&(p, w)| {
            let r = crate::area::Rcti { xmin: x, xmax: x + w - 1, ymin, ymax };
            x += w;
            (p, r)
        })
        .collect()
}

/// The icon of a part: the field's own ID icon on the browse button.
pub fn icon(part: Part, id_icon: Icon) -> Option<Icon> {
    match part {
        Part::Browse => Some(id_icon),
        Part::Name => None,
        Part::New => Some(Icon::Duplicate),
        Part::Unlink => Some(Icon::X),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_width_is_clamped_to_four_to_sixteen_units() {
        assert_eq!(textbut_width(20, 10), 80);
        assert_eq!(textbut_width(20, 100), 115);
        assert_eq!(textbut_width(20, 900), 320);
    }

    #[test]
    fn scene_name_is_one_unit_wider_for_the_pin() {
        let a = parts(20, 40, false);
        let b = parts(20, 40, true);
        assert_eq!(b[1].1, a[1].1 + 20);
        assert_eq!(total_width(&a), 20 + 80 + 20 + 20);
    }

    #[test]
    fn view_layer_rects_follow_in_order_without_gaps() {
        /* `template_search(window, "view_layer", ...)` with new and unlink: browse, name, new, unlink. */
        let r = rects(&parts(20, 40, false), 100, 5, 24);
        let order: Vec<Part> = r.iter().map(|x| x.0).collect();
        assert_eq!(order, vec![Part::Browse, Part::Name, Part::New, Part::Unlink]);
        for w in r.windows(2) {
            assert_eq!(w[0].1.xmax + 1, w[1].1.xmin);
        }
        assert_eq!((r[0].1.xmin, r[3].1.xmax), (100, 100 + 140 - 1));
    }

    #[test]
    fn only_the_name_has_no_icon() {
        assert_eq!(icon(Part::Name, Icon::TabScene), None);
        assert_eq!(icon(Part::Unlink, Icon::TabScene), Some(Icon::X));
    }
}
