//! Relax brush maths: `iteration_strengths` of `mesh/brushes/relax.cc` and `calc_relaxed_translations_faces` of
//! `mesh/sculpt_smooth.cc` (`translation_to_plane`, `calc_boundary_normal_corner`).
//!
//! A vertex moves towards the mean of its neighbours, projected onto the tangent plane of its normal (so the
//! surface relaxes without shrinking). Vertices with 2 or fewer neighbours do not move. A boundary vertex only
//! uses the neighbours it shares a boundary edge with. Face sets and hidden faces are not handled here.

use crate::mesh::{add, dot, normalized, scale, sub, V3};

/// `iteration_strengths`: the four strengths of one Relax Face Sets dab (each squared by the caller).
pub fn iteration_strengths(strength: f32, stroke_iteration: usize) -> [f32; 4] {
    if stroke_iteration % 3 == 0 {
        return [strength; 4];
    }
    let modified = (strength * 1.5).min(1.0);
    [modified, modified, strength, strength]
}

/// `translation_to_plane`: the move from `current` to the closest point of `smoothed` on the plane through
/// `current` with `normal`.
fn translation_to_plane(current: V3, normal: V3, smoothed: V3) -> V3 {
    let d = sub(smoothed, current);
    sub(d, scale(normal, dot(d, normal)))
}

/// `calc_relaxed_translations_faces` for one vertex. `all_neighbors` is the vertex' full neighbour count (a vertex
/// with 2 or fewer is a corner and does not move); `neighbors` are the neighbour positions used, already reduced to
/// the boundary-edge neighbours when `is_boundary`. Zero for a vertex that must not move.
pub fn relaxed_translation(position: V3, normal: V3, all_neighbors: usize, neighbors: &[V3], is_boundary: bool, factor: f32) -> V3 {
    if factor == 0.0 || all_neighbors <= 2 || neighbors.is_empty() {
        return [0.0; 3];
    }
    let n = if is_boundary && neighbors.len() == 2 {
        let sum = neighbors.iter().fold([0.0; 3], |acc, &p| add(acc, normalized(sub(p, position))));
        let n = normalized(sum);
        if n == [0.0; 3] {
            return [0.0; 3];
        }
        n
    } else {
        normal
    };
    let w = 1.0 / neighbors.len() as f32;
    let smoothed = neighbors.iter().fold([0.0; 3], |acc, &p| add(acc, scale(p, w)));
    scale(translation_to_plane(position, n, smoothed), factor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: [V3; 4] = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]];

    #[test]
    fn strengths_every_third_step_are_plain() {
        assert_eq!(iteration_strengths(0.4, 0), [0.4; 4]);
        assert_eq!(iteration_strengths(0.4, 3), [0.4; 4]);
        assert_eq!(iteration_strengths(0.4, 1), [0.6, 0.6, 0.4, 0.4]);
    }

    #[test]
    fn strengths_are_capped_at_one() {
        assert_eq!(iteration_strengths(0.9, 2), [1.0, 1.0, 0.9, 0.9]);
    }

    #[test]
    fn move_is_tangent_to_the_normal() {
        /* Vertex above a flat ring: the plane projection leaves it where it is in z, tangent move is 0. */
        let t = relaxed_translation([0.0, 0.0, 1.0], [0.0, 0.0, 1.0], 4, &RING, false, 1.0);
        assert_eq!(t, [0.0; 3]);
        /* Off-centre in the plane: moves back to the centre. */
        let t = relaxed_translation([0.5, 0.0, 0.0], [0.0, 0.0, 1.0], 4, &RING, false, 1.0);
        assert_eq!(t, [-0.5, 0.0, 0.0]);
    }

    #[test]
    fn factor_scales_and_zero_skips() {
        let half = relaxed_translation([0.5, 0.0, 0.0], [0.0, 0.0, 1.0], 4, &RING, false, 0.5);
        assert_eq!(half, [-0.25, 0.0, 0.0]);
        assert_eq!(relaxed_translation([0.5, 0.0, 0.0], [0.0, 0.0, 1.0], 4, &RING, false, 0.0), [0.0; 3]);
    }

    #[test]
    fn corner_vertices_do_not_move() {
        let two = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        assert_eq!(relaxed_translation([0.2, 0.2, 0.0], [0.0, 0.0, 1.0], 2, &two, false, 1.0), [0.0; 3]);
        assert_eq!(relaxed_translation([0.2, 0.2, 0.0], [0.0, 0.0, 1.0], 3, &[], true, 1.0), [0.0; 3]);
    }

    #[test]
    fn boundary_vertex_with_two_edges_uses_the_neighbour_bisector() {
        /* Opposite neighbours cancel: no normal, no move. */
        let line = [[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        assert_eq!(relaxed_translation([0.0, 0.5, 0.0], [0.0, 0.0, 1.0], 4, &line, true, 1.0), [0.0; 3]);
        /* Corner: neighbours at +x and +y give the bisector normal, the vertex slides along it only. */
        let corner = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let t = relaxed_translation([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 4, &corner, true, 1.0);
        assert!(t[2].abs() < 1e-6 && (t[0] + t[1]).abs() < 1e-6 && t[0].abs() > 0.0);
    }
}
