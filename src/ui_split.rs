//! Property-split row geometry (`use_property_split = True`), ported from
//! `uiItemPropertySplitWrapperCreate` (interface_layout.cc:3538) and `LayoutItemSplit::resolve_impl`
//! (interface_layout.cc:5052). A row holds a split (label column | property row) and, when
//! `use_property_decorate` is on, a fixed `UI_UNIT_X` decorator column on its right.
//! `Layout::PROPERTY_SPLIT_FACTOR = 0.4` (UI_interface_layout.hh:123). Pure geometry, no drawing.

#![allow(dead_code)]

pub const PROPERTY_SPLIT_FACTOR: f32 = 0.4;

/// Horizontal spans `(x, width)` of one property-split row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitRow {
    pub label: (i32, i32),
    pub property: (i32, i32),
    pub decorate: Option<(i32, i32)>,
}

/// Lay out a row of width `w` at `x`. `space` is the split's item spacing (0 for aligned rows),
/// `unit_x` is `UI_UNIT_X` (`U.widget_unit`).
pub fn split_row(x: i32, w: i32, space: i32, unit_x: i32, decorate: bool) -> SplitRow {
    let split_w = if decorate { w - unit_x } else { w };
    /* resolve_impl with tot == 2 */
    let wd = split_w - space;
    let label_w = ((wd as f32 * PROPERTY_SPLIT_FACTOR) as i32).max(0);
    let prop_w = (((wd - label_w) as f32) as i32).max(0);
    let prop_x = x + label_w + space;
    SplitRow {
        label: (x, label_w),
        property: (prop_x, prop_w),
        decorate: decorate.then_some((x + split_w, unit_x)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decorated_row_reserves_one_unit_on_the_right() {
        let r = split_row(0, 300, 0, 20, true);
        assert_eq!(r.label, (0, 112)); // int(280 * 0.4)
        assert_eq!(r.property, (112, 168));
        assert_eq!(r.decorate, Some((280, 20)));
    }

    #[test]
    fn undecorated_row_uses_the_full_width() {
        let r = split_row(10, 100, 0, 20, false);
        assert_eq!((r.label, r.property, r.decorate), ((10, 40), (50, 60), None));
    }

    #[test]
    fn spacing_is_taken_out_before_the_percentage() {
        let r = split_row(0, 101, 1, 20, false);
        assert_eq!(r.label, (0, 40));
        assert_eq!(r.property, (41, 60));
    }
}
