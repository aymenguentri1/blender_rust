//! Vector icons drawn in software.
//!
//! Blender draws its icons from `release/datafiles/icons/*.dat` / the `blender_icons.svg` sheet
//! (`ICON_*`, `UI_icon_draw()` in editors/interface/interface_icons.cc). There is no icon sheet here,
//! so every icon is a handful of signed-distance primitives on a 16 x 16 design grid (y down), which
//! is the size of a Blender icon (`ICON_DEFAULT_HEIGHT`, scaled by `UI_SCALE_FAC`).
//!
//! Icons are single coloured: shapes carry an alpha so a face can be lighter than its outline.
//! Coverage is anti-aliased from the distance to the shape and the shapes are combined with "over".
//! Deviation: the outlines are this port's own drawing, not Blender's artwork.

use crate::area::Rcti;
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::widget_draw::blend_px;

/// Size of the design grid in icon units.
pub const GRID: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// `ICON_VIEW3D`, the 3D viewport editor type.
    View3D,
    /// `ICON_OBJECT_DATAMODE`.
    ObjectMode,
    /// `ICON_DOWNARROW_HLT`.
    DownArrow,
    /// `ICON_ORIENTATION_GLOBAL`.
    OrientationGlobal,
    /// `ICON_PIVOT_MEDIAN`, `ICON_PIVOT_CURSOR`, `ICON_PIVOT_INDIVIDUAL`.
    PivotMedian,
    PivotCursor,
    PivotIndividual,
    /// `ICON_SNAP_OFF` (the magnet).
    Snap,
    /// `ICON_PROP_OFF`.
    Proportional,
    /// `ICON_OVERLAY`.
    Overlays,
    /// `ICON_XRAY`.
    Xray,
    /// `ICON_SHADING_WIRE`, `ICON_SHADING_SOLID`, `ICON_MATERIAL`, `ICON_SHADING_RENDERED`.
    ShadingWire,
    ShadingSolid,
    ShadingMaterial,
    ShadingRendered,
    /// `ICON_DISCLOSURE_TRI_RIGHT`: the arrow of a collapsed panel (`DownArrow` is the expanded one).
    RightArrow,
    /// `ICON_RESTRICT_VIEW_OFF`: the monitor of a modifier's "Show in Viewport" toggle.
    Monitor,
    /// `ICON_RESTRICT_RENDER_OFF`: the camera of a modifier's "Show in Render" toggle.
    Camera,
    /// `ICON_GRIP`: the six dots of a modifier panel's drag handle.
    Grip,
    /// `ICON_EDITMODE_HLT`: the square with corner vertices of a modifier's "Show in Edit Mode" toggle.
    EditMode,
    /// The modifier type icons of the panel header (`ICON_MOD_*`); `ModWrench` is the generic `ICON_MODIFIER`.
    ModWrench,
    ModArray,
    ModMirror,
    ModSolidify,
    ModBevel,
    ModSmooth,
    ModDeform,
    ModTriangulate,
    ModBoolean,
    ModWireframe,
    ModDecimate,
    ModRemesh,
}

#[derive(Clone, Debug)]
enum Prim {
    Line { a: [f32; 2], b: [f32; 2], w: f32 },
    Disc { c: [f32; 2], r: f32 },
    Ring { c: [f32; 2], r: f32, w: f32 },
    /// An ellipse outline, `r = [rx, ry]`.
    Ellipse { c: [f32; 2], r: [f32; 2], w: f32 },
    Poly(Vec<[f32; 2]>),
    PolyLine { pts: Vec<[f32; 2]>, w: f32, closed: bool },
}

#[derive(Clone, Debug)]
struct Shape {
    prim: Prim,
    alpha: f32,
}

impl Shape {
    fn a(mut self, alpha: f32) -> Shape {
        self.alpha = alpha;
        self
    }
}

fn line(x0: f32, y0: f32, x1: f32, y1: f32, w: f32) -> Shape {
    Shape { prim: Prim::Line { a: [x0, y0], b: [x1, y1], w }, alpha: 1.0 }
}
fn disc(x: f32, y: f32, r: f32) -> Shape {
    Shape { prim: Prim::Disc { c: [x, y], r }, alpha: 1.0 }
}
fn ring(x: f32, y: f32, r: f32, w: f32) -> Shape {
    Shape { prim: Prim::Ring { c: [x, y], r, w }, alpha: 1.0 }
}
fn ellipse(x: f32, y: f32, rx: f32, ry: f32, w: f32) -> Shape {
    Shape { prim: Prim::Ellipse { c: [x, y], r: [rx, ry], w }, alpha: 1.0 }
}
fn poly(pts: &[[f32; 2]]) -> Shape {
    Shape { prim: Prim::Poly(pts.to_vec()), alpha: 1.0 }
}
fn polyline(pts: &[[f32; 2]], w: f32, closed: bool) -> Shape {
    Shape { prim: Prim::PolyLine { pts: pts.to_vec(), w, closed }, alpha: 1.0 }
}

/// Points of the arc of the circle `(cx, cy, r)` from `a0` to `a1` degrees (0 = +x, 90 = down).
fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, steps: usize) -> Vec<[f32; 2]> {
    (0..=steps)
        .map(|i| {
            let t = (a0 + (a1 - a0) * i as f32 / steps as f32).to_radians();
            [cx + r * t.cos(), cy + r * t.sin()]
        })
        .collect()
}

fn cube_outline(w: f32) -> Vec<Shape> {
    vec![
        polyline(&[[8.0, 1.5], [14.0, 4.75], [14.0, 11.25], [8.0, 14.5], [2.0, 11.25], [2.0, 4.75]], w, true),
        polyline(&[[2.0, 4.75], [8.0, 8.0], [14.0, 4.75]], w, false),
        line(8.0, 8.0, 8.0, 14.5, w),
    ]
}

fn shapes(icon: Icon) -> Vec<Shape> {
    match icon {
        Icon::View3D => {
            let mut v = vec![poly(&[[8.0, 1.5], [14.0, 4.75], [8.0, 8.0], [2.0, 4.75]]).a(0.45)];
            v.extend(cube_outline(1.3));
            v
        }
        Icon::ObjectMode => {
            let mut v = vec![poly(&[[8.0, 1.5], [14.0, 4.75], [8.0, 8.0], [2.0, 4.75]]).a(0.45)];
            v.extend(cube_outline(1.3));
            v
        }
        Icon::DownArrow => vec![poly(&[[4.0, 6.0], [12.0, 6.0], [8.0, 10.5]])],
        Icon::OrientationGlobal => vec![
            ring(8.0, 8.0, 6.2, 1.3),
            ellipse(8.0, 8.0, 2.8, 6.2, 1.1),
            line(1.8, 8.0, 14.2, 8.0, 1.1),
        ],
        Icon::PivotMedian => vec![
            disc(8.0, 8.0, 2.0),
            disc(3.5, 12.0, 1.6),
            disc(12.5, 12.0, 1.6),
            disc(8.0, 3.2, 1.6),
            polyline(&[[3.5, 12.0], [12.5, 12.0], [8.0, 3.2]], 0.9, true).a(0.5),
        ],
        Icon::PivotCursor => vec![
            ring(8.0, 8.0, 4.4, 1.4),
            line(8.0, 1.5, 8.0, 5.0, 1.3),
            line(8.0, 11.0, 8.0, 14.5, 1.3),
            line(1.5, 8.0, 5.0, 8.0, 1.3),
            line(11.0, 8.0, 14.5, 8.0, 1.3),
        ],
        Icon::PivotIndividual => vec![
            ring(4.8, 8.0, 3.2, 1.2),
            disc(4.8, 8.0, 1.1),
            ring(11.2, 8.0, 3.2, 1.2),
            disc(11.2, 8.0, 1.1),
        ],
        Icon::Snap => {
            let mut u = vec![[4.5, 2.0], [4.5, 8.0]];
            u.extend(arc(8.0, 8.0, 3.5, 180.0, 0.0, 12));
            u.push([11.5, 2.0]);
            vec![
                polyline(&u, 3.0, false),
                line(4.5, 2.0, 4.5, 4.2, 1.2).a(0.35),
                line(11.5, 2.0, 11.5, 4.2, 1.2).a(0.35),
            ]
        }
        Icon::Proportional => vec![ring(8.0, 8.0, 6.3, 1.2), ring(8.0, 8.0, 3.4, 1.2), disc(8.0, 8.0, 1.2)],
        Icon::Overlays => vec![ring(6.0, 8.0, 4.6, 1.3), ring(10.0, 8.0, 4.6, 1.3)],
        Icon::Xray => {
            let mut v = vec![poly(&[[8.0, 1.5], [14.0, 4.75], [8.0, 8.0], [2.0, 4.75]]).a(0.25)];
            v.push(polyline(&[[8.0, 1.5], [14.0, 4.75], [14.0, 11.25], [8.0, 14.5], [2.0, 11.25], [2.0, 4.75]], 1.3, true));
            v.push(polyline(&[[2.0, 4.75], [8.0, 8.0], [14.0, 4.75]], 1.0, false).a(0.55));
            v.push(line(8.0, 8.0, 8.0, 14.5, 1.0).a(0.55));
            v
        }
        Icon::ShadingWire => vec![
            ring(8.0, 8.0, 6.2, 1.2),
            ellipse(8.0, 8.0, 2.8, 6.2, 1.0),
            ellipse(8.0, 8.0, 6.2, 2.4, 1.0),
            line(8.0, 1.8, 8.0, 14.2, 1.0),
        ],
        Icon::ShadingSolid => vec![disc(8.0, 8.0, 6.2)],
        Icon::ShadingMaterial => vec![
            disc(8.0, 8.0, 6.2).a(0.5),
            ring(8.0, 8.0, 6.2, 1.3),
            disc(5.6, 5.6, 2.1),
            disc(10.4, 10.4, 2.1),
        ],
        Icon::RightArrow => vec![poly(&[[6.0, 3.8], [6.0, 12.2], [10.8, 8.0]])],
        Icon::Monitor => vec![
            polyline(&[[2.0, 3.0], [14.0, 3.0], [14.0, 10.5], [2.0, 10.5]], 1.4, true),
            poly(&[[3.6, 4.6], [12.4, 4.6], [12.4, 8.9], [3.6, 8.9]]).a(0.3),
            line(8.0, 10.5, 8.0, 12.6, 1.4),
            line(4.8, 12.9, 11.2, 12.9, 1.4),
        ],
        Icon::Camera => vec![
            polyline(&[[1.8, 4.0], [10.2, 4.0], [10.2, 12.0], [1.8, 12.0]], 1.4, true),
            poly(&[[3.4, 5.6], [8.6, 5.6], [8.6, 10.4], [3.4, 10.4]]).a(0.3),
            poly(&[[10.2, 7.0], [14.4, 4.6], [14.4, 11.4], [10.2, 9.0]]),
        ],
        Icon::EditMode => vec![
            polyline(&[[3.4, 3.4], [12.6, 3.4], [12.6, 12.6], [3.4, 12.6]], 1.2, true).a(0.8),
            poly(&[[4.6, 4.6], [11.4, 4.6], [11.4, 11.4], [4.6, 11.4]]).a(0.25),
            disc(3.4, 3.4, 1.9),
            disc(12.6, 3.4, 1.9),
            disc(12.6, 12.6, 1.9),
            disc(3.4, 12.6, 1.9),
        ],
        Icon::Grip => vec![
            disc(6.0, 3.8, 1.25),
            disc(10.0, 3.8, 1.25),
            disc(6.0, 8.0, 1.25),
            disc(10.0, 8.0, 1.25),
            disc(6.0, 12.2, 1.25),
            disc(10.0, 12.2, 1.25),
        ],
        Icon::ModWrench => vec![
            line(3.6, 12.4, 9.0, 7.0, 2.4),
            polyline(&arc(10.6, 5.4, 3.2, 110.0, 400.0, 14), 1.7, false),
        ],
        Icon::ModArray => vec![
            polyline(&[[1.2, 5.5], [5.0, 5.5], [5.0, 10.5], [1.2, 10.5]], 1.3, true),
            polyline(&[[6.2, 5.5], [10.0, 5.5], [10.0, 10.5], [6.2, 10.5]], 1.3, true).a(0.75),
            polyline(&[[11.2, 5.5], [15.0, 5.5], [15.0, 10.5], [11.2, 10.5]], 1.3, true).a(0.5),
        ],
        Icon::ModMirror => vec![
            line(8.0, 1.5, 8.0, 14.5, 1.2),
            poly(&[[1.8, 4.6], [6.6, 8.0], [1.8, 11.4]]).a(0.55),
            polyline(&[[14.2, 4.6], [9.4, 8.0], [14.2, 11.4]], 1.3, true),
        ],
        Icon::ModSolidify => vec![
            polyline(&[[2.0, 2.0], [14.0, 2.0], [14.0, 14.0], [2.0, 14.0]], 1.4, true),
            poly(&[[5.0, 5.0], [11.0, 5.0], [11.0, 11.0], [5.0, 11.0]]).a(0.45),
        ],
        Icon::ModBevel => vec![
            poly(&[[2.0, 14.0], [2.0, 6.0], [6.0, 2.0], [14.0, 2.0], [14.0, 14.0]]).a(0.3),
            polyline(&[[2.0, 14.0], [2.0, 6.0], [6.0, 2.0], [14.0, 2.0], [14.0, 14.0]], 1.4, true),
        ],
        Icon::ModSmooth => vec![polyline(&[[1.5, 9.5], [3.6, 6.2], [6.0, 5.4], [8.0, 8.0], [10.0, 10.6], [12.4, 9.8], [14.5, 6.5]], 1.7, false)],
        Icon::ModDeform => vec![
            polyline(&arc(8.0, 17.0, 10.0, 235.0, 305.0, 10), 1.4, false),
            polyline(&arc(8.0, 12.0, 10.0, 235.0, 305.0, 10), 1.4, false).a(0.75),
            polyline(&arc(8.0, 7.0, 10.0, 235.0, 305.0, 10), 1.4, false).a(0.5),
        ],
        Icon::ModTriangulate => vec![
            polyline(&[[2.0, 2.0], [14.0, 2.0], [14.0, 14.0], [2.0, 14.0]], 1.4, true),
            line(2.0, 14.0, 14.0, 2.0, 1.3),
        ],
        Icon::ModBoolean => vec![ring(5.8, 8.0, 4.6, 1.4), ring(10.2, 8.0, 4.6, 1.4).a(0.75)],
        Icon::ModWireframe => cube_outline(1.2),
        Icon::ModDecimate => vec![
            polyline(&[[8.0, 2.0], [14.0, 13.5], [2.0, 13.5]], 1.4, true),
            line(8.0, 2.0, 8.0, 13.5, 1.2).a(0.7),
        ],
        Icon::ModRemesh => {
            let mut v = vec![poly(&[[8.0, 1.5], [14.0, 4.75], [8.0, 8.0], [2.0, 4.75]]).a(0.35)];
            v.extend(cube_outline(1.2));
            v
        }
        Icon::ShadingRendered => vec![disc(8.0, 8.0, 6.2).a(0.7), ring(8.0, 8.0, 6.2, 1.3), disc(5.8, 5.8, 2.3)],
    }
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (pa, ba) = ([p[0] - a[0], p[1] - a[1]], [b[0] - a[0], b[1] - a[1]]);
    let len2 = ba[0] * ba[0] + ba[1] * ba[1];
    let h = if len2 > 0.0 { ((pa[0] * ba[0] + pa[1] * ba[1]) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let d = [pa[0] - ba[0] * h, pa[1] - ba[1] * h];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// Signed distance to a prim in design units (negative inside / on the stroke).
fn sdf(prim: &Prim, p: [f32; 2]) -> f32 {
    match prim {
        Prim::Line { a, b, w } => seg_dist(p, *a, *b) - w * 0.5,
        Prim::Disc { c, r } => ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() - r,
        Prim::Ring { c, r, w } => (((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() - r).abs() - w * 0.5,
        Prim::Ellipse { c, r, w } => {
            let q = [(p[0] - c[0]) / r[0], (p[1] - c[1]) / r[1]];
            ((q[0] * q[0] + q[1] * q[1]).sqrt() - 1.0).abs() * r[0].min(r[1]) - w * 0.5
        }
        Prim::Poly(pts) => {
            let n = pts.len();
            let mut d = f32::MAX;
            let mut inside = false;
            for i in 0..n {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                d = d.min(seg_dist(p, a, b));
                if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                    inside = !inside;
                }
            }
            if inside {
                -d
            } else {
                d
            }
        }
        Prim::PolyLine { pts, w, closed } => {
            let n = pts.len();
            let count = if *closed { n } else { n.saturating_sub(1) };
            (0..count).map(|i| seg_dist(p, pts[i], pts[(i + 1) % n])).fold(f32::MAX, f32::min) - w * 0.5
        }
    }
}

/// Side length in pixels of an icon at the current UI scale: 16 px at 1x.
pub fn icon_size(user: &UserDef) -> i32 {
    ((GRID * user.scale_factor).round() as i32).max(1)
}

/// Draws `icon` centred in `rect` (window pixels, y up) with `color` (its alpha scales every shape).
pub fn draw_icon(buf: &mut [u32], width: usize, height: usize, user: &UserDef, rect: Rcti, icon: Icon, color: Rgba) {
    if width == 0 || height == 0 || buf.len() < width * height {
        return;
    }
    let size = icon_size(user);
    let k = size as f32 / GRID;
    let x0 = rect.xmin + (rect.size_x() + 1 - size) / 2;
    let y0 = rect.ymin + (rect.size_y() + 1 - size) / 2;
    let list = shapes(icon);
    let base_a = color.a as f32 / 255.0;
    for py in 0..size {
        let wy = y0 + py;
        if wy < 0 || wy >= height as i32 {
            continue;
        }
        let row = (height - 1 - wy as usize) * width;
        for px in 0..size {
            let wx = x0 + px;
            if wx < 0 || wx >= width as i32 {
                continue;
            }
            /* Design space is y down, window space is y up: the top of the icon is its largest y. */
            let p = [(px as f32 + 0.5) / k, (size - 1 - py) as f32 / k + 0.5 / k];
            let mut cov = 0.0f32;
            for s in &list {
                let c = (0.5 - sdf(&s.prim, p) * k).clamp(0.0, 1.0) * s.alpha;
                cov = cov + c * (1.0 - cov);
            }
            let a = cov * base_a;
            if a > 0.0 {
                let px = &mut buf[row + wx as usize];
                *px = blend_px(*px, color, a);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bg() -> u32 {
        0x0020_2020
    }

    /* A solid sphere fills its centre and leaves the corner of the icon box alone. */
    #[test]
    fn solid_sphere_covers_centre_not_corner() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (20usize, 20usize);
        let mut buf = vec![bg(); w * h];
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        draw_icon(&mut buf, w, h, &user, rect, Icon::ShadingSolid, Rgba::hex(0xffffffff));
        assert_ne!(buf[10 * w + 10], bg());
        assert_eq!(buf[0], bg());
    }

    /* A zero alpha colour draws nothing. */
    #[test]
    fn transparent_colour_draws_nothing() {
        let user = UserDef::from_window_scale(1.0);
        let (w, h) = (20usize, 20usize);
        let mut buf = vec![bg(); w * h];
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        draw_icon(&mut buf, w, h, &user, rect, Icon::View3D, Rgba::hex(0xffffff00));
        assert!(buf.iter().all(|&p| p == bg()));
    }

    /* Every icon draws something. */
    #[test]
    fn every_icon_draws_pixels() {
        let user = UserDef::from_window_scale(1.0);
        let rect = Rcti { xmin: 0, xmax: 19, ymin: 0, ymax: 19 };
        for icon in [
            Icon::View3D,
            Icon::ObjectMode,
            Icon::DownArrow,
            Icon::OrientationGlobal,
            Icon::PivotMedian,
            Icon::PivotCursor,
            Icon::PivotIndividual,
            Icon::Snap,
            Icon::Proportional,
            Icon::Overlays,
            Icon::Xray,
            Icon::ShadingWire,
            Icon::ShadingSolid,
            Icon::ShadingMaterial,
            Icon::ShadingRendered,
        ] {
            let mut buf = vec![bg(); 400];
            draw_icon(&mut buf, 20, 20, &user, rect, icon, Rgba::hex(0xffffffff));
            assert!(buf.iter().any(|&p| p != bg()), "{icon:?}");
        }
    }
}
