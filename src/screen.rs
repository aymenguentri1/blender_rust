//! Screen layout: top bar, status bar and the main editor area.
//!
//! Ported from Blender:
//!  - `screen_global_header_size()`, `screen_global_topbar_area_refresh()`,
//!    `screen_global_statusbar_area_refresh()`  (editors/screen/screen_edit.cc:1284-1318)
//!  - `ED_area_global_size_y()`                  (editors/screen/area.cc:4408)
//!  - `WM_window_screen_rect_calc()`             (windowmanager/intern/wm_window.cc:3229)
//!  - `screen_geom_vertices_scale()` and
//!    `screen_geom_vertices_scale_pass()`        (editors/screen/screen_geometry.cc:251, 138)
//!  - `area_calc_totrct()`                       (editors/screen/area.cc:2096)
//!
//! Only the normal screen state with one main area is handled (no split areas,
//! no collapsed status bar, no fullscreen / temp screens).
//!
//! The layout is pure integer math on `Copy` types with no allocation. It is
//! computed when the window size or DPI changes and cached by the caller, so a
//! redraw does not recompute it.

use crate::area::{area_calc_totrct, area_headersize, Rcti};
use crate::userdef::UserDef;

/* The startup Layout screen has the 3D viewport, outliner, properties editor and timeline. Its vertices
 * are stored in the binary startup.blend, which is not in the source, so the splits are fractions of the
 * screen measured from the default Blender window (they scale like `screen_geom_vertices_scale_pass()`):
 * the right column starts at this fraction of the width, the timeline is this fraction of the height
 * from the bottom and the outliner this fraction from the top. */
const SPLIT_RIGHT_COLUMN: f32 = 0.822;
const SPLIT_TIMELINE: f32 = 0.0745;
const SPLIT_OUTLINER: f32 = 0.186;

/// Final `area->totrct` of every area, in Blender coordinates (y up, inclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenLayout {
    pub topbar: Rcti,
    pub statusbar: Rcti,
    /// The 3D viewport.
    pub main: Rcti,
    pub outliner: Rcti,
    pub properties: Rcti,
    pub timeline: Rcti,
    /// The area vertex rectangles (the scissor of the borders) of main, outliner, properties, timeline.
    vertices: [Rcti; 4],
}

/// Which area of the screen is under a point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaKind {
    TopBar,
    StatusBar,
    Main,
    Outliner,
    Properties,
    Timeline,
}

impl ScreenLayout {
    /// The editor areas as (kind, `totrct`, area vertex rectangle).
    pub fn editor_areas(&self) -> [(AreaKind, Rcti, Rcti); 4] {
        [
            (AreaKind::Main, self.main, self.vertices[0]),
            (AreaKind::Outliner, self.outliner, self.vertices[1]),
            (AreaKind::Properties, self.properties, self.vertices[2]),
            (AreaKind::Timeline, self.timeline, self.vertices[3]),
        ]
    }
    /// The area whose `totrct` contains `xy` (window coordinates, y up), like
    /// `BKE_screen_find_area_xy()` over the area `totrct` rectangles; `None` over the gaps.
    pub fn area_at(&self, xy: [i32; 2]) -> Option<AreaKind> {
        if self.topbar.isect_pt_v(xy) {
            Some(AreaKind::TopBar)
        } else if self.statusbar.isect_pt_v(xy) {
            Some(AreaKind::StatusBar)
        } else {
            self.editor_areas().into_iter().find(|(_, totrct, _)| totrct.isect_pt_v(xy)).map(|(kind, _, _)| kind)
        }
    }
}

/// `screen_global_header_size()`: `int(ceilf(ED_area_headersize() / UI_SCALE_FAC))`.
#[inline]
fn screen_global_header_size(user: &UserDef) -> i32 {
    (area_headersize(user) as f32 / user.scale_factor).ceil() as i32
}

/// `ED_area_global_size_y()`: `round_fl_to_int(cur_fixed_height * UI_SCALE_FAC)`.
#[inline]
fn area_global_size_y(user: &UserDef, cur_fixed_height: i32) -> i32 {
    (cur_fixed_height as f32 * user.scale_factor).round() as i32
}

/// Computes the screen layout for a window of `win_w` x `win_h` pixels
/// (`WM_window_rect_calc()` is `0..win_w`, `0..win_h`).
pub fn screen_layout(user: &UserDef, win_w: i32, win_h: i32) -> ScreenLayout {
    let window_rect = Rcti { xmin: 0, xmax: win_w, ymin: 0, ymax: win_h };

    let header = screen_global_header_size(user);

    /* screen_global_topbar_area_refresh(): size, size_min and size_max are all `size`. */
    let top_size = header as i16 as i32;
    let top_init = Rcti {
        xmin: window_rect.xmin,
        xmax: window_rect.xmax - 1,
        ymin: (window_rect.ymax - 1) - top_size,
        ymax: window_rect.ymax - 1,
    };

    /* screen_global_statusbar_area_refresh(): not collapsed, so size == size_max. */
    let bot_size = (0.85f32 * header as f32) as i16 as i32;
    let bot_init = Rcti {
        xmin: window_rect.xmin,
        xmax: window_rect.xmax - 1,
        ymin: window_rect.ymin,
        ymax: window_rect.ymin + bot_size,
    };

    /* WM_window_screen_rect_calc(): subtract the global areas from the window rect. */
    let top_height = area_global_size_y(user, top_size) - 1;
    let bot_height = area_global_size_y(user, bot_size) - 1;
    let screen_rect = Rcti {
        xmin: window_rect.xmin,
        xmax: window_rect.xmax,
        ymin: window_rect.ymin + bot_height,
        ymax: window_rect.ymax - top_height,
    };

    /* screen_geom_vertices_scale(): global areas get their exact fixed size. The
     * comparisons use the vertices set by the refresh functions above. */
    let mut top_h = top_height;
    if top_init.ymin > window_rect.ymin {
        top_h += user.pixelsize;
    }
    if top_init.ymax < (window_rect.ymax - 1) {
        top_h += user.pixelsize;
    }
    let top_vertices = Rcti {
        xmin: window_rect.xmin,
        xmax: window_rect.xmax - 1,
        ymin: (window_rect.ymax - 1) - top_h,
        ymax: window_rect.ymax - 1,
    };

    let mut bot_h = bot_height;
    if bot_init.ymin > window_rect.ymin {
        bot_h += user.pixelsize;
    }
    if bot_init.ymax < (window_rect.ymax - 1) {
        bot_h += user.pixelsize;
    }
    let bot_vertices = Rcti {
        xmin: window_rect.xmin,
        xmax: window_rect.xmax - 1,
        ymin: window_rect.ymin,
        ymax: window_rect.ymin + bot_h,
    };

    /* screen_geom_vertices_scale_pass(): the single main area is scaled to fit
     * `screen_rect` (vertices span xmin..xmax-1 and ymin..ymax-1). */
    let mut main_vertices = Rcti {
        xmin: screen_rect.xmin,
        xmax: screen_rect.xmax - 1,
        ymin: screen_rect.ymin,
        ymax: screen_rect.ymax - 1,
    };

    /* "Make each window at least ED_area_headersize() high": the area is aligned to the
     * top screen edge, so `min = headersize + border_width`; the lower edge moves up. */
    let border_width = (user.border_width as f32 * user.scale_factor).ceil() as i32;
    let min = area_headersize(user) + border_width;
    let height = main_vertices.size_y() + 1;
    if height < min {
        main_vertices.ymin = main_vertices.ymax - min;
    }

    /* The screen is split into four areas that share their edge vertices. */
    let v = main_vertices;
    let span_x = (v.xmax - v.xmin) as f32;
    let span_y = (v.ymax - v.ymin) as f32;
    let split_x = v.xmin + (SPLIT_RIGHT_COLUMN * span_x).round() as i32;
    let split_timeline = v.ymin + (SPLIT_TIMELINE * span_y).round() as i32;
    let split_outliner = v.ymax - (SPLIT_OUTLINER * span_y).round() as i32;
    let vertices = [
        Rcti { xmin: v.xmin, xmax: split_x, ymin: split_timeline, ymax: v.ymax },
        Rcti { xmin: split_x, xmax: v.xmax, ymin: split_outliner, ymax: v.ymax },
        Rcti { xmin: split_x, xmax: v.xmax, ymin: v.ymin, ymax: split_outliner },
        Rcti { xmin: v.xmin, xmax: split_x, ymin: v.ymin, ymax: split_timeline },
    ];

    /* area_calc_totrct(): global areas never get padding; the editor areas are not the only
     * area (`areabase.is_single()` is false), so they are padded except at the window edges. */
    let totrct = |r: Rcti| area_calc_totrct(user, r, window_rect, false, false);
    ScreenLayout {
        topbar: area_calc_totrct(user, top_vertices, window_rect, true, false),
        statusbar: area_calc_totrct(user, bot_vertices, window_rect, true, false),
        main: totrct(vertices[0]),
        outliner: totrct(vertices[1]),
        properties: totrct(vertices[2]),
        timeline: totrct(vertices[3]),
        vertices,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /* At 1x: widget_unit 20, header size 26, top bar 26 + 1, status bar
     * 0.85 * 26 -> 22 (+1), border_width 2. Values worked out from the C code above. */
    #[test]
    fn layout_at_1x() {
        let user = UserDef::from_window_scale(1.0);
        assert_eq!(user.widget_unit, 20);
        let l = screen_layout(&user, 1920, 1080);
        assert_eq!(l.topbar, Rcti { xmin: 0, xmax: 1919, ymin: 1053, ymax: 1079 });
        assert_eq!(l.statusbar, Rcti { xmin: 0, xmax: 1919, ymin: 0, ymax: 22 });
        /* Vertices: split_x = round(0.822 * 1919) = 1577, timeline top = 21 + round(0.0745 * 1033) = 98,
         * outliner bottom = 1054 - round(0.186 * 1033) = 862; each side shared with another area is padded by 2. */
        assert_eq!(l.main, Rcti { xmin: 2, xmax: 1575, ymin: 100, ymax: 1052 });
        assert_eq!(l.outliner, Rcti { xmin: 1579, xmax: 1917, ymin: 864, ymax: 1052 });
        assert_eq!(l.properties, Rcti { xmin: 1579, xmax: 1917, ymin: 23, ymax: 860 });
        assert_eq!(l.timeline, Rcti { xmin: 2, xmax: 1575, ymin: 23, ymax: 96 });
        assert_eq!(l.area_at([100, 500]), Some(AreaKind::Main));
        assert_eq!(l.area_at([100, 1070]), Some(AreaKind::TopBar));
        assert_eq!(l.area_at([100, 5]), Some(AreaKind::StatusBar));
        assert_eq!(l.area_at([-1, 500]), None);
        assert_eq!(l.area_at([1700, 1000]), Some(AreaKind::Outliner));
        assert_eq!(l.area_at([1700, 500]), Some(AreaKind::Properties));
        assert_eq!(l.area_at([100, 50]), Some(AreaKind::Timeline));
        assert_eq!(l.area_at([1576, 500]), None);
    }
}
