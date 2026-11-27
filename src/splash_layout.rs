//! Geometry of the startup splash popup (`wm_block_splash_create`, wm_splash_screen.cc:380-560): the splash image,
//! the version label, the menu column and the popup bounds centred in the window. Sources: `splash_width =
//! style->widget.points * 45 * UI_SCALE_FAC` clamped to 70% of the window, `splash_height = width * img_y / img_x`,
//! the image button at `(0, 0.5 * U.widget_unit)`, the label at `(width - 8s, height - 13s)` one unit high with right
//! aligned text, the menu layout at `x = 26s`, `y = 0`, `width = splash_width - 52s` (s = `UI_SCALE_FAC`; the `110s` argument is
//! its `em`, not its top, so the menu hangs below the image), and `block_bounds_set_centered(block, 0)`.
//! Not modelled: the project menu (only shown with projects), the banner image, the Rosetta warning. The height of
//! the menu column is an input: it comes from the layout of `WM_MT_splash` (space of its rows), not from this file.
//! Window coordinates are y up.

use crate::area::Rcti;
use crate::font::UI_DEFAULT_TEXT_POINTS;
use crate::userdef::UserDef;

#[derive(Clone, Debug, PartialEq)]
pub struct SplashLayout {
    /// The whole popup, centred in the window.
    pub popup: Rcti,
    /// The splash image button.
    pub image: Rcti,
    /// The version label (right aligned text).
    pub version: Rcti,
    /// The area of the menu layout: its top is the block origin, it hangs below the image.
    pub menu: Rcti,
}

/// The splash width in pixels: `points * 45 * UI_SCALE_FAC`, at most 70% of the window.
pub fn splash_width(user: &UserDef, win_w: i32) -> i32 {
    let w = (UI_DEFAULT_TEXT_POINTS as f32 * 45.0 * user.scale_factor) as i32;
    w.min((win_w as f32 * 0.7) as i32)
}

/// The popup for an image of `img` (x, y) pixels and a menu column `menu_h` high, in a `win_w` x `win_h` window.
pub fn layout(user: &UserDef, win_w: i32, win_h: i32, img: [i32; 2], menu_h: i32) -> SplashLayout {
    let s = user.scale_factor;
    let unit = user.widget_unit;
    let width = splash_width(user, win_w);
    let height = (width as i64 * img[1] as i64 / img[0].max(1) as i64) as i32;
    /* Block coordinates, origin at the bottom left of the block; the image sits half a unit above it. */
    let image = Rcti { xmin: 0, xmax: width, ymin: unit / 2, ymax: unit / 2 + height };
    let (label_right, label_y) = ((width as f32 - 8.0 * s) as i32, (height as f32 - 13.0 * s) as i32);
    let version = Rcti { xmin: 0, xmax: label_right, ymin: label_y, ymax: label_y + unit };
    let margin = (26.0 * s) as i32;
    let menu = Rcti { xmin: margin, xmax: width - margin, ymin: -menu_h, ymax: 0 };
    /* `block_bounds_set_centered`: the bounds of all buttons, centred in the window. */
    let mut bounds = image;
    bounds.union(&version);
    bounds.union(&menu);
    let (bw, bh) = (bounds.size_x(), bounds.size_y());
    let (ox, oy) = ((win_w - bw) / 2 - bounds.xmin, (win_h - bh) / 2 - bounds.ymin);
    let shift = |mut r: Rcti| {
        r.translate(ox, oy);
        r
    };
    SplashLayout { popup: shift(bounds), image: shift(image), version: shift(version), menu: shift(menu) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_width_follows_the_font_size_and_scale_and_is_capped_by_the_window() {
        let u1 = UserDef::from_window_scale(1.0);
        let u2 = UserDef::from_window_scale(2.0);
        assert_eq!(splash_width(&u1, 1920), 495);
        assert_eq!(splash_width(&u2, 3840), 990);
        assert_eq!(splash_width(&u1, 600), 420, "70% of a small window");
    }

    #[test]
    fn the_image_keeps_its_aspect_and_the_popup_is_centred() {
        let u = UserDef::from_window_scale(1.0);
        let l = layout(&u, 1920, 1080, [1000, 500], 200);
        assert_eq!((l.image.size_x(), l.image.size_y()), (495, 247));
                /* The menu column starts at the block origin, half a unit below the image. */
        assert_eq!(l.menu.size_x(), 495 - 52);
        assert_eq!(l.menu.xmin - l.image.xmin, 26);
        assert_eq!(l.image.ymin - l.menu.ymax, 10, "the menu starts half a unit below the image");
        assert!(l.popup.xmin >= 0 && l.popup.xmax <= 1920 && l.popup.ymin >= 0 && l.popup.ymax <= 1080);
        assert!(((l.popup.xmin + l.popup.xmax) / 2 - 1920 / 2).abs() <= 1, "centred");
        for r in [l.image, l.version] {
            assert!(l.popup.xmin <= r.xmin && r.xmax <= l.popup.xmax && l.popup.ymin <= r.ymin && r.ymax <= l.popup.ymax);
        }
    }

    #[test]
    fn the_version_label_sits_in_the_top_right_corner_of_the_image() {
        let u = UserDef::from_window_scale(1.0);
        let l = layout(&u, 1920, 1080, [1000, 500], 200);
        assert_eq!(l.image.xmax - l.version.xmax, 8, "8 px in from the right edge");
        assert_eq!(l.version.size_y(), u.widget_unit);
        assert_eq!(l.version.ymin - l.image.ymin, 247 - 13 - 10);
    }

    #[test]
    fn everything_doubles_at_2x() {
        let l1 = layout(&UserDef::from_window_scale(1.0), 1920, 1080, [1000, 500], 200);
        let u2 = UserDef::from_window_scale(2.0);
        let l2 = layout(&u2, 3840, 2160, [1000, 500], 400);
        assert_eq!((l1.image.size_x(), l2.image.size_x()), (495, 990));
        assert_eq!(l2.menu.size_x(), 990 - 104);
        assert_eq!(l2.menu.xmin - l2.image.xmin, 52);
        assert_eq!(l2.version.size_y(), u2.widget_unit);
    }
}
