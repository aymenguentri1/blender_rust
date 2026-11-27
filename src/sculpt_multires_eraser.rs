//! Multires displacement eraser (editors/sculpt_paint/mesh/brushes/multires_displacement_eraser.cc): each grid
//! point moves toward its limit-surface position by `falloff * min(bstrength, 1)`. Ports the translation maths of
//! `calc_node()` and the strength clamp of `do_displacement_eraser_brush()` (:63-91, :104).
//! Deviation: the limit positions and falloff factors come from the caller (no `SubdivCCG` evaluation).

/// `std::min(ss.cache->bstrength, 1.0f)`.
pub fn eraser_strength(bstrength: f32) -> f32 {
    bstrength.min(1.0)
}

/// `scale_factors(factors, strength)`, then `translations[i] = (limit[i] - position[i]) * factor[i]`.
pub fn translations(
    positions: &[[f32; 3]],
    limit_positions: &[[f32; 3]],
    factors: &[f32],
    strength: f32,
) -> Vec<[f32; 3]> {
    positions
        .iter()
        .zip(limit_positions)
        .zip(factors)
        .map(|((p, l), &f)| std::array::from_fn(|i| (l[i] - p[i]) * (f * strength)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: [[f32; 3]; 1] = [[1.0, 2.0, 3.0]];
    const L: [[f32; 3]; 1] = [[0.0, 0.0, 0.0]];

    fn apply(f: f32) -> [f32; 3] {
        let t = translations(&P, &L, &[f], 1.0)[0];
        std::array::from_fn(|i| P[0][i] + t[i])
    }

    #[test]
    fn weight_one_restores_limit_zero_keeps_sculpted_half_is_midpoint() {
        assert_eq!(apply(1.0), [0.0; 3]);
        assert_eq!(apply(0.0), [1.0, 2.0, 3.0]);
        assert_eq!(apply(0.5), [0.5, 1.0, 1.5]);
    }

    #[test]
    fn strength_is_clamped_to_one_and_scales_the_factor() {
        assert_eq!(eraser_strength(3.0), 1.0);
        assert_eq!(eraser_strength(0.25), 0.25);
        let t = translations(&P, &L, &[1.0], eraser_strength(0.5))[0];
        assert_eq!(t, [-0.5, -1.0, -1.5]);
    }
}
