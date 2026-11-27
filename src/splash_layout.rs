//! Geometry of the startup splash popup, ported from `wm_block_splash_create()`
//! (`source/blender/windowmanager/intern/wm_splash_screen.cc:391`) and
//! `wm_block_splash_image()` (same file, :240). Block space: origin bottom-left, y up.

use crate::area::Rcti;
use crate::font::UI_DEFAULT_TEXT_POINTS;
use crate::userdef::UserDef;

/// Rects of the splash block, all in block coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplashLayout {
    pub width: i32,
    pub height: i32,
    /// The splash image button (`uiDefButImage`).
    pub image: Rcti,
    /// Right aligned version label (`wm_block_splash_add_label`).
    pub version: Rcti,
    /// Floating project menu, only drawn with an active or recent project.
    pub project: Rcti,
    /// Layout of `WM_MT_splash` / `WM_MT_splash_quick_setup` (x, y, width, height of `block_layout`).
    pub menu: Rcti,
}

/// `Rcti` of an `x, y, w, h` button (inclusive max like `BLI_rcti`).
fn xywh(x: i32, y: i32, w: i32, h: i32) -> Rcti {
    Rcti { xmin: x, xmax: x + w - 1, ymin: y, ymax: y + h - 1 }
}

/// `window_px_x` is `WM_window_native_pixel_x()`; `image` the splash image size in pixels.
pub fn layout(user: &UserDef, window_px_x: i32, image: (i32, i32)) -> SplashLayout {
    let fac = user.scale_factor;
    let unit = user.widget_unit;

    /* `style->widget.points * 45 * UI_SCALE_FAC`, then `CLAMP_MAX(.., window_x * 0.7f)`. */
    let mut width = (UI_DEFAULT_TEXT_POINTS * 45.0 * fac) as i32;
    width = width.min((window_px_x as f32 * 0.7) as i32);
    /* `height = (width * ibuf->y) / ibuf->x` */
    let height = if image.0 > 0 { width * image.1 / image.0 } else { 0 };

    let image_y = (0.5 * unit as f32) as i32;
    let label_right = width - (8.0 * fac) as i32;
    let label_y = height - (13.0 * fac) as i32;

    let project_margin = unit / 2;
    let project_width = unit * 8;
    let project_y = image_y + project_margin + unit;

    let margin_x = (fac * 26.0) as i32;

    SplashLayout {
        width,
        height,
        image: xywh(0, image_y, width, height),
        version: xywh(0, label_y, label_right, unit),
        project: xywh(width - project_width - project_margin, project_y, project_width, 0),
        menu: xywh(margin_x, 0, width - margin_x * 2, (fac * 110.0) as i32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IMG: (i32, i32) = (1000, 600);

    #[test]
    fn panel_and_item_rects_at_1x() {
        let l = layout(&UserDef::from_window_scale(1.0), 1920, IMG);
        assert_eq!((l.width, l.height), (495, 297));
        assert_eq!(l.image, Rcti { xmin: 0, xmax: 494, ymin: 10, ymax: 306 });
        assert_eq!(l.version, Rcti { xmin: 0, xmax: 486, ymin: 284, ymax: 303 });
        assert_eq!((l.project.xmin, l.project.ymin), (495 - 160 - 10, 40));
        assert_eq!(l.menu, Rcti { xmin: 26, xmax: 468, ymin: 0, ymax: 109 });
    }

    #[test]
    fn panel_and_item_rects_at_2x() {
        let l = layout(&UserDef::from_window_scale(2.0), 3840, IMG);
        assert_eq!((l.width, l.height), (990, 594));
        assert_eq!(l.image.ymin, 20);
        assert_eq!(l.version, Rcti { xmin: 0, xmax: 973, ymin: 568, ymax: 607 });
        assert_eq!((l.project.xmin, l.project.ymin), (990 - 320 - 20, 80));
        assert_eq!(l.menu, Rcti { xmin: 52, xmax: 937, ymin: 0, ymax: 219 });
    }

    #[test]
    fn width_is_clamped_to_70_percent_of_the_window() {
        let l = layout(&UserDef::from_window_scale(1.0), 500, IMG);
        assert_eq!(l.width, 350);
        assert_eq!(l.height, 210);
    }
}
