//! Port of `widget_color_blend_from_flags()` from
//! `source/blender/editors/interface/interface_widgets.cc` (line 3257).
//! Flag values from `UI_interface_c.hh` (`BUT_ANIMATED` 360, `BUT_ANIMATED_KEY` 361,
//! `BUT_DRIVEN` 362, `BUT_OVERRIDDEN` 397, `BUT_ANIMATED_CHANGED` 517).

use crate::widget_colors::Col4;

pub const BUT_ANIMATED: u64 = 1 << 15;
pub const BUT_ANIMATED_KEY: u64 = 1 << 16;
pub const BUT_DRIVEN: u64 = 1 << 17;
pub const BUT_OVERRIDDEN: u64 = 1 << 31;
/// `but_drawflag` bit.
pub const BUT_ANIMATED_CHANGED: u32 = 1 << 24;

/// Only whether the emboss is `EmbossType::None` matters here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbossType {
    None,
    Other,
}

/// The `*_sel` members of `uiWidgetStateColors` used for blending.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WidgetStateColors {
    pub inner_anim_sel: Col4,
    pub inner_key_sel: Col4,
    pub inner_driven_sel: Col4,
    pub inner_overridden_sel: Col4,
    pub inner_changed_sel: Col4,
}

/// `widget_color_blend_from_flags()`: the state colour to blend with, or `None`.
pub fn widget_color_blend_from_flags(
    wcol_state: &WidgetStateColors,
    but_flag: u64,
    but_drawflag: u32,
    emboss: EmbossType,
) -> Option<Col4> {
    if emboss == EmbossType::None {
        return None;
    }
    if but_drawflag & BUT_ANIMATED_CHANGED != 0 {
        return Some(wcol_state.inner_changed_sel);
    }
    if but_flag & BUT_ANIMATED_KEY != 0 {
        return Some(wcol_state.inner_key_sel);
    }
    if but_flag & BUT_ANIMATED != 0 {
        return Some(wcol_state.inner_anim_sel);
    }
    if but_flag & BUT_DRIVEN != 0 {
        return Some(wcol_state.inner_driven_sel);
    }
    if but_flag & BUT_OVERRIDDEN != 0 {
        return Some(wcol_state.inner_overridden_sel);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: WidgetStateColors = WidgetStateColors {
        inner_anim_sel: [1, 0, 0, 255],
        inner_key_sel: [2, 0, 0, 255],
        inner_driven_sel: [3, 0, 0, 255],
        inner_overridden_sel: [4, 0, 0, 255],
        inner_changed_sel: [5, 0, 0, 255],
    };
    const E: EmbossType = EmbossType::Other;

    #[test]
    fn no_flags_is_none() {
        assert_eq!(widget_color_blend_from_flags(&S, 0, 0, E), None);
    }

    #[test]
    fn each_flag_picks_its_colour() {
        assert_eq!(widget_color_blend_from_flags(&S, BUT_ANIMATED, 0, E).unwrap()[0], 1);
        assert_eq!(widget_color_blend_from_flags(&S, BUT_ANIMATED_KEY, 0, E).unwrap()[0], 2);
        assert_eq!(widget_color_blend_from_flags(&S, BUT_DRIVEN, 0, E).unwrap()[0], 3);
        assert_eq!(widget_color_blend_from_flags(&S, BUT_OVERRIDDEN, 0, E).unwrap()[0], 4);
        assert_eq!(widget_color_blend_from_flags(&S, 0, BUT_ANIMATED_CHANGED, E).unwrap()[0], 5);
    }

    #[test]
    fn priority_changed_key_anim_driven_overridden() {
        let all = BUT_ANIMATED | BUT_ANIMATED_KEY | BUT_DRIVEN | BUT_OVERRIDDEN;
        assert_eq!(widget_color_blend_from_flags(&S, all, BUT_ANIMATED_CHANGED, E).unwrap()[0], 5);
        assert_eq!(widget_color_blend_from_flags(&S, all, 0, E).unwrap()[0], 2);
        assert_eq!(widget_color_blend_from_flags(&S, BUT_ANIMATED | BUT_DRIVEN, 0, E).unwrap()[0], 1);
        assert_eq!(widget_color_blend_from_flags(&S, BUT_DRIVEN | BUT_OVERRIDDEN, 0, E).unwrap()[0], 3);
    }

    #[test]
    fn emboss_none_returns_none() {
        let all = BUT_ANIMATED | BUT_ANIMATED_KEY | BUT_DRIVEN | BUT_OVERRIDDEN;
        assert_eq!(widget_color_blend_from_flags(&S, all, BUT_ANIMATED_CHANGED, EmbossType::None), None);
    }
}
