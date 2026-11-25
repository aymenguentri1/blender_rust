//! Port of the area geometry from Blender:
//!  - `area_calc_totrct()`  (editors/screen/area.cc:2096)
//!  - `ED_area_headersize()` (editors/screen/area.cc:4397)
//!  - `HEADER_PADDING_Y`     (makesdna/DNA_screen_types.h:626)
//!
//! Coordinates follow Blender: origin bottom-left, `rcti` bounds are inclusive.
//! Only the normal screen state is handled (not SCREENFULL / SCREENMAXIMIZED /
//! temp screens).

use crate::theme::Rgba;
use crate::userdef::UserDef;

pub const HEADER_PADDING_Y: i32 = 6;

/// Clamps an inclusive Blender rect (y up) to a `width` x `height` buffer; returns
/// `(x0, x1, y0, y1)` inclusive, or `None` when nothing is left.
fn clip_rect_to_buffer(width: usize, height: usize, rect: Rcti) -> Option<(usize, usize, usize, usize)> {
    if width == 0 || height == 0 {
        return None;
    }
    let buffer = Rcti { xmin: 0, xmax: width as i32 - 1, ymin: 0, ymax: height as i32 - 1 };
    let r = rect.isect(&buffer)?;
    Some((r.xmin as usize, r.xmax as usize, r.ymin as usize, r.ymax as usize))
}

/// Blender's `rcti`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rcti {
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
}

/// `BLI_rcti_isect_pt()` (blenlib/intern/rct.cc): inclusive point-in-rectangle test.
impl Rcti {
    pub fn isect_pt(&self, x: i32, y: i32) -> bool {
        x >= self.xmin && x <= self.xmax && y >= self.ymin && y <= self.ymax
    }

    /// `BLI_rcti_isect_pt_v()` (blenlib/intern/rct.cc): `isect_pt()` for an `xy` pair, as used by
    /// `ED_region_contains_xy()` (editors/screen/area_query.cc:146) on `region->winrct`.
    pub fn isect_pt_v(&self, xy: [i32; 2]) -> bool {
        self.isect_pt(xy[0], xy[1])
    }

    /// `BLI_rcti_size_x()` (blenlib/intern/rct.cc): `xmax - xmin` (no +1).
    pub fn size_x(&self) -> i32 {
        self.xmax - self.xmin
    }

    /// `BLI_rcti_size_y()` (blenlib/intern/rct.cc): `ymax - ymin` (no +1).
    pub fn size_y(&self) -> i32 {
        self.ymax - self.ymin
    }

    /// `BLI_rcti_cent_x()`: `(xmin + xmax) / 2` (integer division).
    pub fn cent_x(&self) -> i32 {
        (self.xmin + self.xmax) / 2
    }

    /// `BLI_rcti_cent_y()`: `(ymin + ymax) / 2` (integer division).
    pub fn cent_y(&self) -> i32 {
        (self.ymin + self.ymax) / 2
    }

    /// `BLI_rcti_is_empty()`: `xmax < xmin || ymax < ymin`.
    pub fn is_empty(&self) -> bool {
        self.xmax < self.xmin || self.ymax < self.ymin
    }

    /// `BLI_rcti_pad()`: grows the rectangle by `pad_x` / `pad_y` on every side.
    pub fn pad(&mut self, pad_x: i32, pad_y: i32) {
        self.xmin -= pad_x;
        self.xmax += pad_x;
        self.ymin -= pad_y;
        self.ymax += pad_y;
    }

    /// `BLI_rcti_translate()`: moves the rectangle by `x` / `y`.
    pub fn translate(&mut self, x: i32, y: i32) {
        self.xmin += x;
        self.xmax += x;
        self.ymin += y;
        self.ymax += y;
    }

    /// `BLI_rcti_union()`: grows `self` to also cover `other`.
    pub fn union(&mut self, other: &Rcti) {
        self.xmin = self.xmin.min(other.xmin);
        self.xmax = self.xmax.max(other.xmax);
        self.ymin = self.ymin.min(other.ymin);
        self.ymax = self.ymax.max(other.ymax);
    }

    /// `BLI_rcti_isect()`: the overlap of `self` and `other`, `None` when they do not overlap.
    pub fn isect(&self, other: &Rcti) -> Option<Rcti> {
        let r = Rcti {
            xmin: self.xmin.max(other.xmin),
            xmax: self.xmax.min(other.xmax),
            ymin: self.ymin.max(other.ymin),
            ymax: self.ymax.min(other.ymax),
        };
        if r.is_empty() { None } else { Some(r) }
    }
}

#[cfg(test)]
mod rcti_tests {
    use super::*;

    #[test]
    fn isect_pt_is_inclusive() {
        let r = Rcti { xmin: 0, xmax: 9, ymin: 5, ymax: 14 };
        assert!(r.isect_pt(0, 5));
        assert!(r.isect_pt(9, 14));
        assert!(!r.isect_pt(10, 5));
        assert!(!r.isect_pt(0, 4));
        assert!(r.isect_pt_v([9, 14]));
        assert!(!r.isect_pt_v([10, 14]));
    }

    #[test]
    fn in_view3d_main_excludes_header() {
        let user = UserDef::from_window_scale(1.0);
        let main = Rcti { xmin: 0, xmax: 1919, ymin: 21, ymax: 1054 };
        assert!(in_view3d_main(&user, main, [100, 500]));
        assert!(!in_view3d_main(&user, main, [100, 1050]));
        assert!(!in_view3d_main(&user, main, [100, 5]));
    }

    #[test]
    fn top_header_rect_of_topbar() {
        let user = UserDef::from_window_scale(1.0);
        /* Top bar totrct at 1x (see `screen::layout_at_1x`): 27 px, header size 26 fits. */
        let topbar = Rcti { xmin: 0, xmax: 1919, ymin: 1053, ymax: 1079 };
        assert_eq!(top_header_rect(&user, topbar), Some(Rcti { ymin: 1054, ..topbar }));
        let statusbar = Rcti { xmin: 0, xmax: 1919, ymin: 0, ymax: 22 };
        assert_eq!(statusbar_header_rect(statusbar), statusbar);
        /* 1x: offset 4, buttons 20 px high in a 26 px region, top at 20 + (26 - 20) / 2 = 23. */
        assert_eq!(global_header_layout_start(&user, 26), (4, 23, 20));
        /* A 21 px region leaves room for 19 px buttons. */
        assert_eq!(global_header_layout_start(&user, 21), (4, 20, 19));
        assert_eq!(r.size_x(), 9);
        assert_eq!(r.size_y(), 9);
    }
}

/// `ED_area_headersize()`: `U.widget_unit + int(UI_SCALE_FAC * HEADER_PADDING_Y)`.
pub fn area_headersize(user: &UserDef) -> i32 {
    user.widget_unit + (user.scale_factor * HEADER_PADDING_Y as f32) as i32
}

/// `area_calc_totrct()`.
///
/// `area_rect` is the rectangle spanned by the area vertices (v1.x, v1.y)-(v4.x, v2.y),
/// `window_rect` is `WM_window_rect_calc()`, `is_global` is `ED_area_is_global()`
/// and `is_single` is `screen->areabase.is_single()`.
pub fn area_calc_totrct(
    user: &UserDef,
    area_rect: Rcti,
    window_rect: Rcti,
    is_global: bool,
    is_single: bool,
) -> Rcti {
    let ui_scale_fac = user.scale_factor;

    /* Padding around each area, except at window edges. */
    let px = (user.border_width as f32 * ui_scale_fac).max(ui_scale_fac) as i16 as i32;

    /* Padding at window edges. Cannot be less than border width. */
    let px_edge = (ui_scale_fac * 2.0).min(user.border_width as f32 * ui_scale_fac) as i16 as i32;

    let mut totrct = area_rect;

    /* Scale down totrct by the border size on all sides not at window edges. */
    if !is_global && !is_single {
        totrct.xmin += if totrct.xmin > window_rect.xmin { px } else { px_edge };
        totrct.xmax -= if totrct.xmax < (window_rect.xmax - 1) { px } else { px_edge };
        totrct.ymin += if totrct.ymin > window_rect.ymin { px } else { px_edge };

        if totrct.ymax < (window_rect.ymax - 1) {
            totrct.ymax -= px;
        } else {
            /* Small gap below Top Bar (`!is_single` always holds in this branch). */
            totrct.ymax -= user.pixelsize;
        }
    }

    totrct
}

/// `region_rect_recursive()` for the 3D viewport header (`RGN_TYPE_HEADER`, `RGN_ALIGN_TOP`).
///
/// With the default `USER_REGION_OVERLAP` and a header theme alpha != 255 the region is an
/// overlap region, so it is cut from `overlap_remainder` (the whole `area.totrct`) and does not
/// shrink the window region. Returns `None` for `RGN_FLAG_TOO_SMALL`.
pub fn view3d_header_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    top_header_rect(user, totrct)
}

/// `region_rect_recursive()` for an `RGN_TYPE_HEADER` region with `RGN_ALIGN_TOP` and
/// `prefsizey = HEADERY` (also the top bar's header, space_topbar.cc:367): the strip of height
/// `area_headersize()` at the top of `totrct`, shrunk when it does not fit, `None` for
/// `RGN_FLAG_TOO_SMALL`.
pub fn top_header_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    let mut prefsizey = area_headersize(user);
    /* rct_fits(): BLI_rcti_size_y(rect) + 1 - size. */
    let fits = totrct.size_y() + 1 - prefsizey;
    if prefsizey == 0 || fits < user.pixelsize * -2 {
        return None;
    }
    if fits < 0 {
        prefsizey += fits;
    }
    Some(Rcti { ymin: totrct.ymax - prefsizey + 1, ..totrct })
}

/// `region_rect_recursive()` for the status bar (space_statusbar.cc:45): its only region is an
/// `RGN_TYPE_HEADER` with `RGN_ALIGN_NONE`, which takes the whole remainder, i.e. `area.totrct`.
pub fn statusbar_header_rect(totrct: Rcti) -> Rcti {
    totrct
}

/// `AREAMINX` (makesdna/DNA_screen_types.h:625), the `prefsizex` of the properties navigation bar
/// (space_buttons.cc:1181).
pub const AREAMINX: i32 = 29;

/// `region_rect_recursive()` for the properties editor's `RGN_TYPE_NAV_BAR` (`RGN_ALIGN_LEFT`): it comes
/// after the non-overlapping header, so it takes `int(UI_SCALE_FAC * AREAMINX)` pixels from the left of
/// what is left below the header, over the full remaining height. `None` when the header does not fit.
pub fn properties_nav_bar_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    let header = top_header_rect(user, totrct)?;
    let prefsizex = (user.scale_factor * AREAMINX as f32) as i32;
    let remainder = Rcti { ymax: header.ymin - 1, ..totrct };
    if prefsizex == 0 || remainder.is_empty() {
        return None;
    }
    /* rct_fits(): BLI_rcti_size_x(rect) + 1 - size. */
    let fits = remainder.size_x() + 1 - prefsizex;
    let width = if fits < 0 { prefsizex + fits } else { prefsizex };
    Some(Rcti { xmax: remainder.xmin + width - 1, ..remainder })
}

#[cfg(test)]
mod nav_bar_tests {
    use super::*;

    /* 1x properties area {1579, 1917, 23, 860}: the 26 px header takes y 835..=860, the 29 px wide
     * navigation bar is x 1579..=1607 below it. */
    #[test]
    fn properties_nav_bar_below_header() {
        let user = UserDef::from_window_scale(1.0);
        let totrct = Rcti { xmin: 1579, xmax: 1917, ymin: 23, ymax: 860 };
        assert_eq!(
            properties_nav_bar_rect(&user, totrct),
            Some(Rcti { xmin: 1579, xmax: 1607, ymin: 23, ymax: 834 })
        );
    }
}

/// Where the first button of a global header (top bar, status bar) starts, from
/// `ED_region_header_layout()` (editors/screen/area.cc:4236-4252): `offset = int(4 * UI_SCALE_FAC)`,
/// `buttony = min(UI_UNIT_Y, winy - 2 * UI_SCALE_FAC)` and the buttons are vertically centred.
/// Returns `(x, top_y, button_height)` relative to the region's bottom-left; `winy` is the region
/// height. The offset of non-global headers is `UI_HEADER_OFFSET`, which is not in the supplied
/// source, so it is not ported.
pub fn global_header_layout_start(user: &UserDef, winy: i32) -> (i32, i32, i32) {
    let offset = (4.0 * user.scale_factor) as i32;
    let buttony = user.widget_unit.min(winy - (2.0 * user.scale_factor) as i32);
    (offset, buttony + (winy - buttony) / 2, buttony)
}

/// `UI_TIME_SCRUB_MARGIN_Y` (editors/include/UI_view2d.hh:493): `23 * UI_SCALE_FAC`.
const TIME_SCRUB_MARGIN_Y: f32 = 23.0;

/// `ED_time_scrub_region_rect_get()` (editors/interface/time_scrub_ui.cc:39): the strip at the top of
/// a window region, `ymax = winy` and `ymin = ymax - UI_TIME_SCRUB_MARGIN_Y` (float truncated to int),
/// moved here from region to window coordinates. `window` is the inclusive window region rect.
pub fn time_scrub_rect(user: &UserDef, window: Rcti) -> Rcti {
    let winy = window.size_y() + 1;
    let ymin = (winy as f32 - TIME_SCRUB_MARGIN_Y * user.scale_factor) as i32;
    Rcti { ymin: window.ymin + ymin, ..window }
}

/// `ED_time_scrub_event_in_region()` (editors/interface/time_scrub_ui.cc:377): the rect of the scrub strip
/// in window coordinates, `winrct` with `ymin = ymax - UI_TIME_SCRUB_MARGIN_Y` (float truncated to int).
pub fn time_scrub_event_rect(user: &UserDef, window: Rcti) -> Rcti {
    Rcti { ymin: (window.ymax as f32 - TIME_SCRUB_MARGIN_Y * user.scale_factor) as i32, ..window }
}

#[cfg(test)]
mod time_scrub_tests {
    use super::*;

    /* 1x timeline area {2, 1575, 23, 96}: the header takes y 71..=96, the window region is y 23..=70
     * (48 px) and the 23 px scrub strip is its top, y 48..=70. */
    #[test]
    fn scrub_strip_is_top_of_window_region() {
        let user = UserDef::from_window_scale(1.0);
        let window = Rcti { xmin: 2, xmax: 1575, ymin: 23, ymax: 70 };
        assert_eq!(time_scrub_rect(&user, window), Rcti { xmin: 2, xmax: 1575, ymin: 48, ymax: 70 });
    }
}


/// True when `xy` is inside the 3D viewport area `main` but not over its header region
/// (the `ED_region_contains_xy()` test of the window region, area_query.cc:146).
pub fn in_view3d_main(user: &UserDef, main: Rcti, xy: [i32; 2]) -> bool {
    main.isect_pt_v(xy) && !view3d_header_rect(user, main).is_some_and(|h| h.isect_pt_v(xy))
}

/// Composites an overlap region over the buffer: `ED_region_clear()` clears it to the
/// premultiplied colour `(a*rgb, a)` and the region is blended with `GPU_BLEND_ALPHA_PREMULT`,
/// i.e. `out = src_premultiplied + dst * (1 - a)`. Same inclusive y-up rect as `fill_rect`.
pub fn blend_rect(buf: &mut [u32], width: usize, height: usize, rect: Rcti, color: Rgba) {
    let Some((x0, x1, y0, y1)) = clip_rect_to_buffer(width, height, rect) else { return };
    let a = color.a as u32;
    let premul = |c: u8| (c as u32 * a + 127) / 255;
    let (sr, sg, sb) = (premul(color.r), premul(color.g), premul(color.b));
    let keep = |d: u32| (d * (255 - a) + 127) / 255;
    for y in y0..=y1 {
        let row = height - 1 - y;
        for px in &mut buf[row * width + x0..=row * width + x1] {
            let d = *px;
            let r = (sr + keep((d >> 16) & 0xff)).min(255);
            let g = (sg + keep((d >> 8) & 0xff)).min(255);
            let b = (sb + keep(d & 0xff)).min(255);
            *px = (r << 16) | (g << 8) | b;
        }
    }
}


/// Fill an inclusive Blender rect (y up) into a top-left-origin `0x00RRGGBB` buffer.
pub fn fill_rect(buf: &mut [u32], width: usize, height: usize, rect: Rcti, color: u32) {
    let Some((x0, x1, y0, y1)) = clip_rect_to_buffer(width, height, rect) else { return };
    for y in y0..=y1 {
        let row = height - 1 - y;
        buf[row * width + x0..=row * width + x1].fill(color);
    }
}

impl Rcti {
    /// `BLI_rcti_init_pt_radius()`: the square of half size `radius` around `(x, y)`.
    pub fn init_pt_radius(x: i32, y: i32, radius: i32) -> Rcti {
        Rcti { xmin: x - radius, xmax: x + radius, ymin: y - radius, ymax: y + radius }
    }
}

#[cfg(test)]
mod rcti_ops_tests {
    use super::*;

    #[test]
    fn center_empty_pad_translate() {
        let mut r = Rcti { xmin: 0, xmax: 9, ymin: 4, ymax: 13 };
        assert_eq!((r.cent_x(), r.cent_y()), (4, 8));
        assert!(!r.is_empty());
        assert!(Rcti { xmin: 5, xmax: 4, ymin: 0, ymax: 0 }.is_empty());
        r.pad(2, 1);
        assert_eq!(r, Rcti { xmin: -2, xmax: 11, ymin: 3, ymax: 14 });
        r.translate(10, -3);
        assert_eq!(r, Rcti { xmin: 8, xmax: 21, ymin: 0, ymax: 11 });
    }

    #[test]
    fn union_isect_init_pt_radius() {
        let a = Rcti { xmin: 0, xmax: 9, ymin: 0, ymax: 9 };
        let b = Rcti { xmin: 5, xmax: 15, ymin: 5, ymax: 15 };
        let mut u = a;
        u.union(&b);
        assert_eq!(u, Rcti { xmin: 0, xmax: 15, ymin: 0, ymax: 15 });
        assert_eq!(a.isect(&b), Some(Rcti { xmin: 5, xmax: 9, ymin: 5, ymax: 9 }));
        assert_eq!(a.isect(&Rcti { xmin: 20, xmax: 30, ymin: 0, ymax: 9 }), None);
        assert_eq!(Rcti::init_pt_radius(10, 20, 3), Rcti { xmin: 7, xmax: 13, ymin: 17, ymax: 23 });
    }
}
