//! Surface Smooth brush maths (`mesh/brushes/surface_smooth.cc` `do_surface_smooth_brush_mesh`, and
//! `surface_smooth_laplacian_step` / `surface_smooth_displace_step` of `mesh/sculpt_smooth.cc`): the HC smooth
//! algorithm of "Improved Laplacian Smoothing of Noisy Surface Meshes".
//!
//! Each iteration moves every vertex towards the average of its neighbours (`laplacian_step`), then pulls it back
//! with the averaged difference to the shape-preserving position (`displace_step`). Both moves are scaled by the
//! brush factors, clamped to 0..1 (`clamp_factors`). The brush falloff itself is computed by the caller.
//! One difference: both steps read a snapshot of the positions (Blender reads the positions already moved by the
//! nodes processed before), and the Laplacian displacement of vertices outside the brush is computed, not kept at 0.

use crate::mesh::{add, scale, sub, V3};

/// `Brush::surface_smooth_shape_preservation` (alpha), `surface_smooth_current_vertex` (beta) and
/// `surface_smooth_iterations`.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceSmooth {
    pub shape_preservation: f32,
    pub current_vertex: f32,
    pub iterations: usize,
}

/// `neighbor_data_average_mesh`: the mean of `src` over each vertex' neighbours, zero without neighbours
/// (`attribute_math::mix_indices` on an empty span).
pub fn neighbor_average(src: &[V3], neighbors: &[Vec<usize>]) -> Vec<V3> {
    neighbors
        .iter()
        .map(|n| {
            let w = 1.0 / n.len() as f32;
            n.iter().fold([0.0; 3], |acc, &i| add(acc, scale(src[i], w)))
        })
        .collect()
}

/// `surface_smooth_laplacian_step`: `(laplacian_disp, translations)`.
pub fn laplacian_step(positions: &[V3], orig: &[V3], average: &[V3], alpha: f32) -> (Vec<V3>, Vec<V3>) {
    let lap = (0..positions.len())
        .map(|i| sub(average[i], add(scale(orig[i], alpha), scale(positions[i], 1.0 - alpha))))
        .collect();
    let trans = (0..positions.len()).map(|i| sub(average[i], positions[i])).collect();
    (lap, trans)
}

/// `surface_smooth_displace_step`.
pub fn displace_step(laplacian_disp: &[V3], average_laplacian_disp: &[V3], beta: f32) -> Vec<V3> {
    (0..laplacian_disp.len())
        .map(|i| scale(add(scale(average_laplacian_disp[i], 1.0 - beta), scale(laplacian_disp[i], beta)), -1.0))
        .collect()
}

/// `do_surface_smooth_brush_mesh`: the new positions. `orig` are the positions at the start of the stroke.
pub fn surface_smooth(positions: &[V3], orig: &[V3], neighbors: &[Vec<usize>], factors: &[f32], p: SurfaceSmooth) -> Vec<V3> {
    let factors: Vec<f32> = factors.iter().map(|f| f.clamp(0.0, 1.0)).collect();
    let mut pos = positions.to_vec();
    for _ in 0..p.iterations {
        let avg = neighbor_average(&pos, neighbors);
        let (lap, mut trans) = laplacian_step(&pos, orig, &avg, p.shape_preservation);
        for i in 0..pos.len() {
            trans[i] = scale(trans[i], factors[i]);
            pos[i] = add(pos[i], trans[i]);
        }
        let avg_lap = neighbor_average(&lap, neighbors);
        let trans = displace_step(&lap, &avg_lap, p.current_vertex);
        for i in 0..pos.len() {
            pos[i] = add(pos[i], scale(trans[i], factors[i]));
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    /* Three vertices in a line: a - b - c. */
    fn line(z: [f32; 3]) -> (Vec<V3>, Vec<Vec<usize>>) {
        (vec![[0.0, 0.0, z[0]], [1.0, 0.0, z[1]], [2.0, 0.0, z[2]]], vec![vec![1], vec![0, 2], vec![1]])
    }

    fn zs(p: &[V3]) -> Vec<f32> {
        p.iter().map(|v| v[2]).collect()
    }

    fn run(z: [f32; 3], beta: f32, iterations: usize, factor: f32) -> Vec<f32> {
        let (pos, nb) = line(z);
        zs(&surface_smooth(&pos, &pos, &nb, &[factor; 3], SurfaceSmooth { shape_preservation: 0.0, current_vertex: beta, iterations }))
    }

    #[test]
    fn flat_mesh_is_unchanged() {
        assert_eq!(run([0.0; 3], 0.5, 3, 1.0), vec![0.0; 3]);
        assert_eq!(run([2.0; 3], 0.3, 2, 1.0), vec![2.0; 3]);
    }

    #[test]
    fn beta_half_is_plain_laplacian_smoothing() {
        /* The spike in the middle goes to its neighbours' mean, the ends to the middle. */
        assert_eq!(run([0.0, 1.0, 0.0], 0.5, 1, 1.0), vec![1.0, 0.0, 1.0]);
    }

    #[test]
    fn beta_one_cancels_the_laplacian_move() {
        assert_eq!(run([0.0, 1.0, 0.0], 1.0, 1, 1.0), vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn zero_factor_and_zero_iterations_keep_positions() {
        assert_eq!(run([0.0, 1.0, 0.0], 0.5, 1, 0.0), vec![0.0, 1.0, 0.0]);
        assert_eq!(run([0.0, 1.0, 0.0], 0.5, 0, 1.0), vec![0.0, 1.0, 0.0]);
    }

    #[test]
    fn factors_are_clamped_to_one() {
        assert_eq!(run([0.0, 1.0, 0.0], 0.5, 1, 5.0), run([0.0, 1.0, 0.0], 0.5, 1, 1.0));
    }

    #[test]
    fn laplacian_step_uses_shape_preservation() {
        let (lap, trans) = laplacian_step(&[[0.0, 0.0, 1.0]], &[[0.0, 0.0, 3.0]], &[[0.0, 0.0, 0.0]], 0.5);
        assert_eq!(lap[0][2], -2.0); // 0 - (3 * 0.5 + 1 * 0.5)
        assert_eq!(trans[0][2], -1.0);
    }

    #[test]
    fn isolated_vertex_averages_to_zero() {
        assert_eq!(neighbor_average(&[[1.0, 2.0, 3.0]], &[vec![]]), vec![[0.0; 3]]);
    }
}
