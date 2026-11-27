//! Smooth brush (`mesh/brushes/smooth.cc`: `do_smooth_brush_mesh`, `apply_positions_faces`; `brush_strength()` for
//! `SCULPT_BRUSH_TYPE_SMOOTH` in sculpt.cc). The strength (clamped to 1) is spread over up to four iterations
//! (`iteration_strengths`, shared with Smooth Mask). In each one every vertex under the brush moves toward the mean of its
//! neighbours by `factor * iteration strength`; a vertex without neighbours stays (`neighbor_data_average_mesh_check_loose`).
//! The new positions of one iteration are all computed before any is applied.
//! Deviations: the brush factors are computed once per dab (Blender recomputes them per iteration), boundary and
//! face set neighbour filtering are not ported.

use crate::mesh::{add, scale, sub, V3};
use crate::sculpt_smooth_mask::iteration_strengths;

/// `brush_strength()` for the Smooth brush: `flip * alpha * pressure`, `alpha` is the squared root alpha, pressure is 1.
/// The inverted brush is Enhance Details, so the sign is not applied here.
pub fn brush_strength(root_alpha: f32) -> f32 {
    root_alpha * root_alpha
}

/// `neighbor_data_average_mesh_check_loose()` for one vertex: the neighbour mean, the vertex itself when loose.
fn average(verts: &[V3], i: usize, neighbors: &[u32]) -> V3 {
    if neighbors.is_empty() {
        return verts[i];
    }
    let w = 1.0 / neighbors.len() as f32;
    neighbors.iter().fold([0.0; 3], |acc, &n| add(acc, scale(verts[n as usize], w)))
}

/// One dab: `hit` holds the `(vertex, brush factor)` pairs, `neighbors` the neighbours of every vertex.
pub fn smooth(verts: &mut [V3], neighbors: &[Vec<u32>], hit: &[(usize, f32)], brush_strength: f32) {
    for strength in iteration_strengths(brush_strength.max(0.0)) {
        let moved: Vec<(usize, V3)> = hit
            .iter()
            .map(|&(i, f)| (i, add(verts[i], scale(sub(average(verts, i, &neighbors[i]), verts[i]), f * strength))))
            .collect();
        for (i, p) in moved {
            verts[i] = p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /* a - b - c in a line, b is a spike. */
    fn line() -> (Vec<V3>, Vec<Vec<u32>>) {
        (vec![[0.0; 3], [1.0, 0.0, 1.0], [2.0, 0.0, 0.0]], vec![vec![1], vec![0, 2], vec![1]])
    }

    #[test]
    fn strength_is_the_squared_alpha() {
        assert_eq!(brush_strength(0.5), 0.25);
    }

    #[test]
    fn a_full_strength_dab_moves_the_spike_to_the_neighbour_mean() {
        let (mut v, n) = line();
        smooth(&mut v, &n, &[(1, 1.0)], 1.0);
        assert_eq!(v[1], [1.0, 0.0, 0.0]);
        assert_eq!(v[0], [0.0; 3], "vertices outside the hit stay");
    }

    #[test]
    fn factor_scales_the_move_and_iterations_compound() {
        let (mut v, n) = line();
        /* strength 0.25 = one iteration of strength 1 (count 1) plus a zero remainder. */
        smooth(&mut v, &n, &[(1, 0.5)], 0.25);
        assert!((v[1][2] - 0.5).abs() < 1e-6);
        let (mut w, n) = line();
        /* strength 0.125 = a single iteration of strength 0.5. */
        smooth(&mut w, &n, &[(1, 1.0)], 0.125);
        assert!((w[1][2] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn loose_vertices_and_zero_strength_do_not_move() {
        let mut v = vec![[1.0, 2.0, 3.0]];
        smooth(&mut v, &[vec![]], &[(0, 1.0)], 1.0);
        assert_eq!(v[0], [1.0, 2.0, 3.0]);
        let (mut w, n) = line();
        smooth(&mut w, &n, &[(1, 1.0)], -1.0);
        assert_eq!(w, line().0);
    }
}
