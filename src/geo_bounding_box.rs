//! Geometry Nodes "Bounding Box" (nodes/geometry/nodes/node_geo_bounding_box.cc): Min/Max outputs from the
//! bounds of the positions (zero when there is no geometry) and the box mesh's `scale = max - min`,
//! `center = min + scale / 2`. With "Use Radius" each point extends the bounds by its radius.
//! Deviation: only the bounds math; the cuboid mesh (`create_cuboid_mesh`) is built by the caller.

#![allow(dead_code)]

type V3 = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: V3,
    pub max: V3,
}

/// Bounds of positions, optionally grown by per-point radii (`None` for empty input).
pub fn compute_bounds(positions: &[V3], radii: Option<&[f32]>) -> Option<Bounds> {
    let first = positions.first()?;
    let mut b = Bounds { min: *first, max: *first };
    for (i, p) in positions.iter().enumerate() {
        let r = radii.and_then(|r| r.get(i)).copied().unwrap_or(0.0);
        for k in 0..3 {
            b.min[k] = b.min[k].min(p[k] - r);
            b.max[k] = b.max[k].max(p[k] + r);
        }
    }
    Some(b)
}

/// The Min / Max outputs: zero vectors when there is no bounds.
pub fn min_max_outputs(bounds: Option<Bounds>) -> (V3, V3) {
    bounds.map_or(([0.0; 3], [0.0; 3]), |b| (b.min, b.max))
}

/// `(scale, center)` of the box mesh; `None` clears the geometry.
pub fn box_scale_center(bounds: Option<Bounds>) -> Option<(V3, V3)> {
    let b = bounds?;
    let scale: V3 = std::array::from_fn(|i| b.max[i] - b.min[i]);
    let center: V3 = std::array::from_fn(|i| b.min[i] + scale[i] / 2.0);
    Some((scale, center))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_max_of_point_set() {
        let b = compute_bounds(&[[1.0, -2.0, 3.0], [-1.0, 4.0, 0.0], [0.0, 0.0, 5.0]], None).unwrap();
        assert_eq!(b, Bounds { min: [-1.0, -2.0, 0.0], max: [1.0, 4.0, 5.0] });
    }

    #[test]
    fn empty_input_gives_zero_outputs_and_no_box() {
        let b = compute_bounds(&[], None);
        assert!(b.is_none());
        assert_eq!(min_max_outputs(b), ([0.0; 3], [0.0; 3]));
        assert!(box_scale_center(b).is_none());
    }

    #[test]
    fn single_point_is_a_degenerate_box() {
        let b = compute_bounds(&[[2.0, 3.0, 4.0]], None);
        assert_eq!(min_max_outputs(b), ([2.0, 3.0, 4.0], [2.0, 3.0, 4.0]));
        assert_eq!(box_scale_center(b), Some(([0.0; 3], [2.0, 3.0, 4.0])));
    }

    #[test]
    fn radius_grows_the_bounds() {
        let p = [[0.0; 3], [4.0, 0.0, 0.0]];
        let b = compute_bounds(&p, Some(&[1.0, 2.0])).unwrap();
        assert_eq!(b, Bounds { min: [-1.0, -2.0, -2.0], max: [6.0, 2.0, 2.0] });
        assert_eq!(compute_bounds(&p, None).unwrap().max, [4.0, 0.0, 0.0]);
    }

    #[test]
    fn box_scale_and_center() {
        let b = Some(Bounds { min: [-1.0, 0.0, 2.0], max: [3.0, 2.0, 6.0] });
        assert_eq!(box_scale_center(b), Some(([4.0, 2.0, 4.0], [1.0, 1.0, 4.0])));
    }
}
