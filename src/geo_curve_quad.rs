//! Geometry Nodes "Quadrilateral" (nodes/geometry/nodes/node_geo_curve_primitive_quadrilateral.cc): a cyclic poly
//! curve of 4 points for Rectangle, Parallelogram, Trapezoid, Kite and Points modes. Sizes clamp to >= 0 as in
//! `node_geo_exec` (offsets and Top Height do not). Deviation: positions only.

#![allow(dead_code)]

type V3 = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quad {
    Rectangle { width: f32, height: f32 },
    Parallelogram { width: f32, height: f32, offset: f32 },
    Trapezoid { bottom_width: f32, top_width: f32, offset: f32, height: f32 },
    Kite { width: f32, bottom_height: f32, top_height: f32 },
    Points([V3; 4]),
}

/// The four corner positions for a mode, in Blender's order.
pub fn quad_positions(q: Quad) -> [V3; 4] {
    let m = |v: f32| v.max(0.0);
    match q {
        Quad::Rectangle { width, height } => {
            let (w, h) = (m(width), m(height));
            [[w / 2.0, h / 2.0, 0.0], [-w / 2.0, h / 2.0, 0.0], [-w / 2.0, -h / 2.0, 0.0], [w / 2.0, -h / 2.0, 0.0]]
        }
        Quad::Parallelogram { width, height, offset } => {
            let (w, h) = (m(width), m(height));
            [
                [w / 2.0 + offset / 2.0, h / 2.0, 0.0],
                [-w / 2.0 + offset / 2.0, h / 2.0, 0.0],
                [-w / 2.0 - offset / 2.0, -h / 2.0, 0.0],
                [w / 2.0 - offset / 2.0, -h / 2.0, 0.0],
            ]
        }
        Quad::Trapezoid { bottom_width, top_width, offset, height } => {
            let (b, t, h) = (m(bottom_width), m(top_width), m(height));
            [[t / 2.0 + offset, h / 2.0, 0.0], [-t / 2.0 + offset, h / 2.0, 0.0], [-b / 2.0, -h / 2.0, 0.0], [b / 2.0, -h / 2.0, 0.0]]
        }
        Quad::Kite { width, bottom_height, top_height } => {
            let w = m(width);
            [[0.0, -m(bottom_height), 0.0], [w / 2.0, 0.0, 0.0], [0.0, top_height, 0.0], [-w / 2.0, 0.0, 0.0]]
        }
        Quad::Points(p) => p,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_corners_and_clamp() {
        assert_eq!(
            quad_positions(Quad::Rectangle { width: 4.0, height: 2.0 }),
            [[2.0, 1.0, 0.0], [-2.0, 1.0, 0.0], [-2.0, -1.0, 0.0], [2.0, -1.0, 0.0]]
        );
        assert_eq!(quad_positions(Quad::Rectangle { width: -1.0, height: -1.0 })[0], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn parallelogram_shifts_top_and_bottom_oppositely() {
        let p = quad_positions(Quad::Parallelogram { width: 2.0, height: 2.0, offset: 1.0 });
        assert_eq!(p[0], [1.5, 1.0, 0.0]);
        assert_eq!(p[1], [-0.5, 1.0, 0.0]);
        assert_eq!(p[2], [-1.5, -1.0, 0.0]);
        assert_eq!(p[3], [0.5, -1.0, 0.0]);
    }

    #[test]
    fn trapezoid_uses_top_bottom_widths_and_top_offset() {
        let p = quad_positions(Quad::Trapezoid { bottom_width: 4.0, top_width: 2.0, offset: 0.5, height: 2.0 });
        assert_eq!(p, [[1.5, 1.0, 0.0], [-0.5, 1.0, 0.0], [-2.0, -1.0, 0.0], [2.0, -1.0, 0.0]]);
    }

    #[test]
    fn kite_and_points() {
        let k = quad_positions(Quad::Kite { width: 2.0, bottom_height: 1.0, top_height: -3.0 });
        assert_eq!(k, [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, -3.0, 0.0], [-1.0, 0.0, 0.0]]); // top height not clamped
        let pts = [[1.0, 2.0, 3.0]; 4];
        assert_eq!(quad_positions(Quad::Points(pts)), pts);
    }
}
