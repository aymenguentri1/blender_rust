//! Multires displacement smear brush (`mesh/brushes/multires_displacement_smear.cc`: `calc_node` per grid point,
//! `store_node_prev_displacement`, strength clamp in `do_displacement_smear_brush`; `BRUSH_SMEAR_DEFORM_*` in
//! DNA_brush_enums.h). Each point blends the previous displacement (position minus limit surface) of its neighbours that
//! lie against the stroke direction, then moves toward `limit + blended displacement` by the brush factor.
//! Deviation: limit positions, neighbours and falloff factors come from the caller (no `SubdivCCG` evaluation).

use crate::mesh::{add, scale, sub, V3};

/// `eBrushSmearDeformType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmearDeform {
    Drag,
    Pinch,
    Expand,
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `math::normalize()`: the zero vector stays zero.
fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        scale(a, 1.0 / l)
    } else {
        [0.0; 3]
    }
}

/// `std::clamp(ss.cache->bstrength, 0, 1)`.
pub fn smear_strength(bstrength: f32) -> f32 {
    bstrength.clamp(0.0, 1.0)
}

/// `store_node_prev_displacement()` for one point: `position - limit`.
pub fn prev_displacement(position: V3, limit: V3) -> V3 {
    sub(position, limit)
}

/// `current_disp` of `calc_node()`: the stroke direction the displacement is dragged along.
pub fn current_disp(deform: SmearDeform, location: V3, last_location: V3, position: V3) -> V3 {
    match deform {
        SmearDeform::Drag => sub(location, last_location),
        SmearDeform::Pinch => sub(location, position),
        SmearDeform::Expand => sub(position, location),
    }
}

/// One grid point of `calc_node()`. `neighbors` holds `(limit_surface_co, prev_displacement)` of every neighbour.
/// Returns the new position: `interpolate(position, limit + blended, factor * strength)`.
pub fn smear_point(
    position: V3,
    limit: V3,
    prev_disp: V3,
    neighbors: &[(V3, V3)],
    current_disp: V3,
    factor: f32,
    strength: f32,
) -> V3 {
    let dir = normalize(current_disp);
    let mut blended = prev_disp;
    let mut weights = 1.0f32;
    for &(n_limit, n_prev) in neighbors {
        let vert_disp = normalize(sub(n_limit, limit));
        let d = dot(dir, vert_disp);
        if d >= 0.0 {
            continue;
        }
        let w = (-d).clamp(0.0, 1.0);
        blended = add(blended, scale(n_prev, w));
        weights += w;
    }
    let new_co = add(limit, scale(blended, 1.0 / weights));
    let t = factor * strength;
    add(scale(position, 1.0 - t), scale(new_co, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: V3, b: V3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn strength_is_clamped_to_unit_range() {
        assert_eq!(smear_strength(2.0), 1.0);
        assert_eq!(smear_strength(-1.0), 0.0);
        assert_eq!(smear_strength(0.4), 0.4);
    }

    #[test]
    fn current_disp_follows_the_deform_type() {
        let (loc, last, pos) = ([2.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 3.0, 0.0]);
        assert_eq!(current_disp(SmearDeform::Drag, loc, last, pos), [1.0, 0.0, 0.0]);
        assert_eq!(current_disp(SmearDeform::Pinch, loc, last, pos), [2.0, -3.0, 0.0]);
        assert_eq!(current_disp(SmearDeform::Expand, loc, last, pos), [-2.0, 3.0, 0.0]);
    }

    #[test]
    fn displacement_is_smeared_from_the_neighbour_behind_the_stroke() {
        // Stroke goes +X; the neighbour at -X (behind) carries a displacement that is pulled in.
        let limit = [0.0; 3];
        let neighbors = [([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0])];
        let p = smear_point([0.0; 3], limit, [0.0; 3], &neighbors, [1.0, 0.0, 0.0], 1.0, 1.0);
        assert!(near(p, [0.0, 0.0, 0.5]));
    }

    #[test]
    fn neighbours_ahead_of_the_stroke_are_ignored() {
        let neighbors = [([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])];
        let p = smear_point([0.0; 3], [0.0; 3], [0.0; 3], &neighbors, [1.0, 0.0, 0.0], 1.0, 1.0);
        assert!(near(p, [0.0; 3]));
    }

    #[test]
    fn factor_and_strength_blend_toward_the_target() {
        let neighbors = [([-1.0, 0.0, 0.0], [0.0, 0.0, 2.0])];
        let full = smear_point([0.0; 3], [0.0; 3], [0.0; 3], &neighbors, [1.0, 0.0, 0.0], 1.0, 1.0);
        let half = smear_point([0.0; 3], [0.0; 3], [0.0; 3], &neighbors, [1.0, 0.0, 0.0], 0.5, 1.0);
        let none = smear_point([0.0; 3], [0.0; 3], [0.0; 3], &neighbors, [1.0, 0.0, 0.0], 1.0, 0.0);
        assert!(near(full, [0.0, 0.0, 1.0]));
        assert!(near(half, [0.0, 0.0, 0.5]));
        assert!(near(none, [0.0; 3]));
    }

    #[test]
    fn zero_stroke_direction_ignores_all_neighbours() {
        let neighbors = [([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0])];
        let p = smear_point([0.0; 3], [0.0; 3], [0.0; 3], &neighbors, [0.0; 3], 1.0, 1.0);
        assert!(near(p, [0.0; 3]));
    }

    #[test]
    fn prev_displacement_is_position_minus_limit() {
        assert_eq!(prev_displacement([1.0, 2.0, 3.0], [1.0, 1.0, 1.0]), [0.0, 1.0, 2.0]);
    }
}
