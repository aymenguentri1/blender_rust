//! Smooth Mask brush maths (`mesh/brushes/smooth_mask.cc` `iteration_strengths` and `do_smooth_brush_mesh`, and
//! `mix_new_masks` / `clamp_mask` of `mesh/paint_mask.cc`).
//!
//! Per iteration every mask value is replaced by the mean of its neighbours (`neighbor_data_average_mesh`, zero
//! without neighbours), mixed in by `factor * iteration strength` and clamped to 0..1. The brush factors (falloff,
//! hide, front faces) are computed by the caller.

/// `iteration_strengths`: `strength` (clamped to 1) spread over at most 4 iterations; every full iteration has
/// strength 1 and the last one the remainder.
pub fn iteration_strengths(strength: f32) -> Vec<f32> {
    const MAX_ITERATIONS: usize = 4;
    let clamped = strength.min(1.0);
    let count = (clamped * MAX_ITERATIONS as f32) as usize;
    let last = MAX_ITERATIONS as f32 * (clamped - count as f32 / MAX_ITERATIONS as f32);
    let mut result = vec![1.0; count];
    result.push(last);
    result
}

/// `mix_new_masks` + `clamp_mask`.
fn mix(mask: f32, new: f32, factor: f32) -> f32 {
    (mask + (new - mask) * factor).clamp(0.0, 1.0)
}

/// `do_smooth_brush_mesh`: smooths `mask` in place. `factors` is the brush factor of every vertex.
pub fn smooth_mask(mask: &mut [f32], neighbors: &[Vec<usize>], factors: &[f32], brush_strength: f32) {
    for strength in iteration_strengths(brush_strength) {
        let averages: Vec<f32> = neighbors
            .iter()
            .map(|n| n.iter().map(|&i| mask[i]).sum::<f32>() * if n.is_empty() { 0.0 } else { 1.0 / n.len() as f32 })
            .collect();
        for i in 0..mask.len() {
            mask[i] = mix(mask[i], averages[i], factors[i] * strength);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /* a - b - c in a line. */
    fn line() -> Vec<Vec<usize>> {
        vec![vec![1], vec![0, 2], vec![1]]
    }

    #[test]
    fn strengths_split_into_four_iterations() {
        assert_eq!(iteration_strengths(1.0), vec![1.0, 1.0, 1.0, 1.0, 0.0]);
        assert_eq!(iteration_strengths(0.5), vec![1.0, 1.0, 0.0]);
        assert_eq!(iteration_strengths(0.125), vec![0.5]);
        assert_eq!(iteration_strengths(3.0), iteration_strengths(1.0));
    }

    #[test]
    fn middle_spike_goes_to_neighbour_mean() {
        let mut m = [0.0, 1.0, 0.0];
        smooth_mask(&mut m, &line(), &[1.0; 3], 0.25);
        assert_eq!(m, [1.0, 0.0, 1.0]);
    }

    #[test]
    fn factor_mixes_and_zero_keeps() {
        let mut m = [0.0, 1.0, 0.0];
        smooth_mask(&mut m, &line(), &[0.5; 3], 0.25);
        assert_eq!(m, [0.5, 0.5, 0.5]);
        let mut m = [0.0, 1.0, 0.0];
        smooth_mask(&mut m, &line(), &[0.0; 3], 1.0);
        assert_eq!(m, [0.0, 1.0, 0.0]);
    }

    #[test]
    fn uniform_mask_is_unchanged_and_isolated_vertex_fades() {
        let mut m = [0.4, 0.4, 0.4];
        smooth_mask(&mut m, &line(), &[1.0; 3], 1.0);
        assert_eq!(m, [0.4; 3]);
        let mut m = [0.8];
        smooth_mask(&mut m, &[vec![]], &[1.0], 0.25);
        assert_eq!(m, [0.0]);
    }
}
