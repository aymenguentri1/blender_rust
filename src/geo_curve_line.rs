//! Geometry Nodes "Curve Line" (nodes/geometry/nodes/node_geo_curve_primitive_line.cc): a poly curve of 2 points,
//! either from start/end (Points mode) or start + normalized direction * length (Direction mode).
//! Deviation: returns the two positions only; the Curves container is built by the caller.

#![allow(dead_code)]

type V3 = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineMode {
    Points,
    Direction,
}

/// `create_point_line_curve()`.
pub fn point_line(start: V3, end: V3) -> [V3; 2] {
    [start, end]
}

/// `math::normalize()`: the zero vector stays zero.
fn normalize(v: V3) -> V3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0.0 { [0.0; 3] } else { [v[0] / l, v[1] / l, v[2] / l] }
}

/// `create_direction_line_curve()`: `normalize(direction) * length + start`.
pub fn direction_line(start: V3, direction: V3, length: f32) -> [V3; 2] {
    let d = normalize(direction);
    [start, std::array::from_fn(|i| d[i] * length + start[i])]
}

/// Node defaults: Start (0,0,0), End (0,0,1), Direction (0,0,1), Length 1; mode Points.
pub fn line(mode: LineMode, start: V3, end: V3, direction: V3, length: f32) -> [V3; 2] {
    match mode {
        LineMode::Points => point_line(start, end),
        LineMode::Direction => direction_line(start, direction, length),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_mode_gives_start_and_end() {
        assert_eq!(line(LineMode::Points, [1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [9.0; 3], 9.0), [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    }

    #[test]
    fn direction_is_normalized_then_scaled_by_length() {
        let l = line(LineMode::Direction, [1.0, 0.0, 0.0], [0.0; 3], [0.0, 3.0, 0.0], 2.0);
        assert_eq!(l, [[1.0, 0.0, 0.0], [1.0, 2.0, 0.0]]);
        let m = direction_line([0.0; 3], [3.0, 4.0, 0.0], 10.0);
        assert!((m[1][0] - 6.0).abs() < 1e-5 && (m[1][1] - 8.0).abs() < 1e-5);
    }

    #[test]
    fn zero_direction_collapses_to_start_and_negative_length_reverses() {
        assert_eq!(direction_line([1.0, 1.0, 1.0], [0.0; 3], 5.0), [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]]);
        assert_eq!(direction_line([0.0; 3], [0.0, 0.0, 2.0], -1.0)[1], [0.0, 0.0, -1.0]);
    }
}
