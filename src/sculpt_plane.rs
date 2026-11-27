//! Plane brush (`mesh/brushes/plane.cc`: `calc_local_distances`, `scale_factors_by_height_and_depth`, `calc_translations`,
//! `do_plane_brush`). Vertices are put in the brush local space (`z` along the plane normal, all scaled by the radius).
//! Above the plane (`z >= 0`) the falloff distance is stretched by `height`, below by `depth`; each vertex then moves along
//! the normal onto the plane by `-z * radius * strength * factor`. Inverting either flips the strength or swaps height and
//! depth (`plane_inversion_mode`). Deviations: the Mesh path only, `tip_scale_x` is 1, brush hardness and the plane
//! offset / tilt are not applied, `height`, `depth` and `roundness` are parameters (brush asset values).

use crate::mesh::{dot, sub, V3};

/// `brush.plane_inversion_mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InversionMode {
    InvertDisplacement,
    SwapHeightAndDepth,
}

/// `brush_strength()` for Plane: `alpha * pressure * (1 + overlap) / 2`, halved for an inverted brush in
/// Invert Displacement mode (pressure is 1, `alpha` the squared root alpha).
pub fn brush_strength(root_alpha: f32, flip: bool, mode: InversionMode, overlap: f32) -> f32 {
    let alpha = root_alpha * root_alpha;
    let overlap = (1.0 + overlap) / 2.0;
    if !flip || mode == InversionMode::SwapHeightAndDepth {
        alpha * overlap
    } else {
        0.5 * alpha * overlap
    }
}

/// `(strength, height, depth)` after the inversion (`do_plane_brush`).
pub fn apply_inversion(strength: f32, height: f32, depth: f32, flip: bool, mode: InversionMode) -> (f32, f32, f32) {
    match (flip, mode) {
        (false, _) => (strength, height, depth),
        (true, InversionMode::InvertDisplacement) => (-strength, height, depth),
        (true, InversionMode::SwapHeightAndDepth) => (strength, depth, height),
    }
}

/// The brush local position of `p`: x / y / z are the dot products with the unit axes, divided by the radius.
pub fn local_position(p: V3, center: V3, axes: &[V3; 3], radius: f32) -> V3 {
    let d = sub(p, center);
    [dot(d, axes[0]) / radius, dot(d, axes[1]) / radius, dot(d, axes[2]) / radius]
}

/// `calc_local_distances()` for one vertex.
pub fn local_distance(local: V3, roundness: f32, height: f32, depth: f32) -> f32 {
    let extent = if local[2] >= 0.0 { height } else { depth };
    if extent == 0.0 {
        return 1.0;
    }
    let z = local[2] / extent;
    if roundness == 1.0 {
        return (local[0] * local[0] + local[1] * local[1] + z * z).sqrt();
    }
    let (ax, ay) = (local[0].abs(), local[1].abs());
    if ax.max(ay) > 1.0 {
        return 1.0;
    }
    let hardness = 1.0 - roundness;
    let rcp = if roundness == 0.0 { 0.0 } else { 1.0 / roundness };
    let (ex, ey) = ((ax - hardness).max(0.0) * rcp, (ay - hardness).max(0.0) * rcp);
    (ex * ex + ey * ey + z * z).sqrt()
}

/// `scale_factors_by_height_and_depth()` for one vertex.
pub fn scale_factor(factor: f32, z: f32, height: f32, depth: f32) -> f32 {
    if z > 0.0 && height != 1.0 && height != 0.0 {
        return factor * height;
    }
    if z < 0.0 && depth != 0.0 && depth != 1.0 {
        return factor * depth;
    }
    factor
}

/// One dab. `falloff` maps a local distance (0..1) to the brush curve factor; vertices with distance >= 1 are untouched.
#[allow(clippy::too_many_arguments)]
pub fn dab(verts: &mut [V3], mask: &[f32], center: V3, axes: &[V3; 3], radius: f32, strength: f32, height: f32, depth: f32, roundness: f32, falloff: impl Fn(f32) -> f32) {
    let normal = axes[2];
    for (i, v) in verts.iter_mut().enumerate() {
        let local = local_position(*v, center, axes, radius);
        let dist = local_distance(local, roundness, height, depth);
        if dist >= 1.0 {
            continue;
        }
        let m = (1.0 - mask.get(i).copied().unwrap_or(0.0)).clamp(0.0, 1.0);
        let factor = scale_factor(falloff(dist) * m, local[2], height, depth) * local[2];
        let k = -radius * strength * factor;
        *v = [v[0] + normal[0] * k, v[1] + normal[1] * k, v[2] + normal[2] * k];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AX: [V3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    #[test]
    fn strength_halves_for_inverted_displacement() {
        let m = InversionMode::InvertDisplacement;
        assert_eq!(brush_strength(0.5, false, m, 1.0), 0.25);
        assert_eq!(brush_strength(0.5, true, m, 1.0), 0.125);
        assert_eq!(brush_strength(0.5, true, InversionMode::SwapHeightAndDepth, 1.0), 0.25);
    }

    #[test]
    fn inversion_flips_or_swaps() {
        assert_eq!(apply_inversion(0.5, 1.0, 0.2, false, InversionMode::InvertDisplacement), (0.5, 1.0, 0.2));
        assert_eq!(apply_inversion(0.5, 1.0, 0.2, true, InversionMode::InvertDisplacement), (-0.5, 1.0, 0.2));
        assert_eq!(apply_inversion(0.5, 1.0, 0.2, true, InversionMode::SwapHeightAndDepth), (0.5, 0.2, 1.0));
    }

    #[test]
    fn distances_stretch_by_height_above_and_depth_below() {
        assert!((local_distance([0.0, 0.0, 0.5], 1.0, 1.0, 1.0) - 0.5).abs() < 1e-6);
        assert!((local_distance([0.0, 0.0, 0.5], 1.0, 2.0, 1.0) - 0.25).abs() < 1e-6);
        assert!((local_distance([0.0, 0.0, -0.5], 1.0, 2.0, 0.5) - 1.0).abs() < 1e-6);
        assert_eq!(local_distance([0.0, 0.0, 0.5], 1.0, 0.0, 1.0), 1.0, "no height: nothing above");
        assert_eq!(local_distance([0.0, 0.0, -0.5], 1.0, 1.0, 0.0), 1.0, "no depth: nothing below");
        /* Square tip: outside the unit square is out, inside the hard core has distance 0 in xy. */
        assert_eq!(local_distance([1.5, 0.0, 0.0], 0.5, 1.0, 1.0), 1.0);
        assert!((local_distance([0.4, 0.0, 0.0], 0.5, 1.0, 1.0)).abs() < 1e-6);
    }

    #[test]
    fn factors_scale_by_height_and_depth() {
        assert_eq!(scale_factor(1.0, 0.5, 2.0, 1.0), 2.0);
        assert_eq!(scale_factor(1.0, -0.5, 2.0, 0.5), 0.5);
        assert_eq!(scale_factor(1.0, 0.5, 1.0, 0.5), 1.0);
    }

    #[test]
    fn dab_pulls_both_sides_onto_the_plane() {
        let mut v = vec![[0.0, 0.0, 0.5], [0.0, 0.0, -0.5], [0.0, 0.0, 2.0]];
        dab(&mut v, &[], [0.0; 3], &AX, 1.0, 0.5, 1.0, 1.0, 1.0, |_| 1.0);
        /* z -= z * strength * factor (factor 1): 0.5 -> 0.25, -0.5 -> -0.25. */
        assert!((v[0][2] - 0.25).abs() < 1e-6 && (v[1][2] + 0.25).abs() < 1e-6);
        assert_eq!(v[2][2], 2.0, "outside the radius");
    }

    #[test]
    fn inverted_strength_pushes_away_and_mask_blocks() {
        let mut v = vec![[0.0, 0.0, 0.5], [0.0, 0.0, 0.5]];
        dab(&mut v, &[0.0, 1.0], [0.0; 3], &AX, 1.0, -0.5, 1.0, 1.0, 1.0, |_| 1.0);
        assert!((v[0][2] - 0.75).abs() < 1e-6);
        assert_eq!(v[1][2], 0.5);
        /* Depth 0 leaves what is below untouched. */
        let mut w = vec![[0.0, 0.0, -0.5]];
        dab(&mut w, &[], [0.0; 3], &AX, 1.0, 0.5, 1.0, 0.0, 1.0, |_| 1.0);
        assert_eq!(w[0][2], -0.5);
    }
}
