//! Port of `shadecolors4()`, `widget_alpha_factor()` and `widget_color_disabled()` from
//! `source/blender/editors/interface/interface_widgets.cc` (lines 968, 1236, 3225).

pub type Col4 = [u8; 4];

/// `BUT_DISABLED` (`UI_interface_c.hh:358`).
pub const BUT_DISABLED: u32 = 1 << 14;
/// `BUT_INACTIVE` (`UI_interface_c.hh:365`).
pub const BUT_INACTIVE: u32 = 1 << 19;
/// `UI_SEARCH_FILTER_NO_MATCH` (`interface_intern.hh:104`).
pub const UI_SEARCH_FILTER_NO_MATCH: u32 = 1 << 6;

/// The colour members of `uiWidgetColors` touched by `widget_color_disabled()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WidgetColors {
    pub outline: Col4,
    pub inner: Col4,
    pub inner_sel: Col4,
    pub item: Col4,
    pub text: Col4,
    pub text_sel: Col4,
    pub outline_sel: Col4,
}

/// `shadecolors4()`: returns `(coltop, coldown)`; alpha is copied unchanged.
pub fn shadecolors4(color: Col4, shadetop: i16, shadedown: i16) -> (Col4, Col4) {
    let shade = |s: i16| {
        let c = |v: u8| (v as i32 + s as i32).clamp(0, 255) as u8;
        [c(color[0]), c(color[1]), c(color[2]), color[3]]
    };
    (shade(shadetop), shade(shadedown))
}

/// `widget_alpha_factor()`.
pub fn widget_alpha_factor(but_flag: u32) -> f32 {
    if but_flag & (BUT_INACTIVE | BUT_DISABLED) != 0 {
        if but_flag & UI_SEARCH_FILTER_NO_MATCH != 0 {
            return 0.25;
        }
        return 0.5;
    }
    if but_flag & UI_SEARCH_FILTER_NO_MATCH != 0 {
        return 0.5;
    }
    1.0
}

/// `widget_color_disabled()`: scales the alpha of the listed colours (`uchar *= float` truncates).
pub fn widget_color_disabled(wcol: &mut WidgetColors, but_flag: u32) {
    let factor = widget_alpha_factor(but_flag);
    for c in [
        &mut wcol.outline,
        &mut wcol.outline_sel,
        &mut wcol.inner,
        &mut wcol.inner_sel,
        &mut wcol.item,
        &mut wcol.text,
        &mut wcol.text_sel,
    ] {
        c[3] = (c[3] as f32 * factor) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shade_clamps_and_keeps_alpha() {
        let (top, down) = shadecolors4([250, 5, 100, 77], 10, -10);
        assert_eq!(top, [255, 15, 110, 77]);
        assert_eq!(down, [240, 0, 90, 77]);
    }

    #[test]
    fn alpha_factor_combinations() {
        assert_eq!(widget_alpha_factor(0), 1.0);
        assert_eq!(widget_alpha_factor(BUT_DISABLED), 0.5);
        assert_eq!(widget_alpha_factor(BUT_INACTIVE), 0.5);
        assert_eq!(widget_alpha_factor(UI_SEARCH_FILTER_NO_MATCH), 0.5);
        assert_eq!(widget_alpha_factor(BUT_DISABLED | UI_SEARCH_FILTER_NO_MATCH), 0.25);
        assert_eq!(widget_alpha_factor(BUT_INACTIVE | UI_SEARCH_FILTER_NO_MATCH), 0.25);
    }

    #[test]
    fn disabled_scales_all_alphas_only() {
        let mut w = WidgetColors {
            outline: [1, 2, 3, 255],
            inner: [4, 5, 6, 101],
            inner_sel: [7, 8, 9, 200],
            item: [1, 1, 1, 255],
            text: [2, 2, 2, 255],
            text_sel: [3, 3, 3, 255],
            outline_sel: [4, 4, 4, 10],
        };
        widget_color_disabled(&mut w, BUT_DISABLED);
        assert_eq!(w.outline, [1, 2, 3, 127]);
        assert_eq!(w.inner, [4, 5, 6, 50]);
        assert_eq!(w.inner_sel[3], 100);
        assert_eq!(w.outline_sel[3], 5);
        assert_eq!(w.text_sel, [3, 3, 3, 127]);
    }

    #[test]
    fn enabled_leaves_colors() {
        let mut w = WidgetColors { inner: [9, 9, 9, 123], ..Default::default() };
        let before = w;
        widget_color_disabled(&mut w, 0);
        assert_eq!(w, before);
    }
}
