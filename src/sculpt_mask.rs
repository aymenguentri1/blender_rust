//! Mask brush (`mesh/brushes/mask.cc`: `apply_factors`, `clamp_mask`, `calc_faces`; `brush_strength()` for
//! `SCULPT_BRUSH_TYPE_MASK` in sculpt.cc). Each vertex under the brush gets
//! `mask += factor * current * strength`, where `current` is `1 - mask` when the strength is positive (painting
//! moves the mask toward 1) and `mask` otherwise (erasing scales it down), then the mask is clamped to `0..=1`.
//! Deviations: the brush overlap and pressure are 1.

/// `brush_strength()` for the Mask brush in draw mode: `alpha * flip * pressure * overlap`.
pub fn brush_strength(alpha: f32, invert: bool) -> f32 {
    alpha * if invert { -1.0 } else { 1.0 }
}

/// The new mask value of one vertex (`apply_factors()` after `invert_mask()`, then `clamp_mask()`).
pub fn masked(mask: f32, factor: f32, strength: f32) -> f32 {
    let current = if strength > 0.0 { 1.0 - mask } else { mask };
    (mask + factor * current * strength).clamp(0.0, 1.0)
}

/// One dab over `hit`, the `(vertex, brush factor)` pairs under the brush (the mask itself does not mask the brush).
pub fn dab(mask: &mut [f32], hit: &[(usize, f32)], strength: f32) {
    for &(i, factor) in hit {
        if let Some(m) = mask.get_mut(i) {
            *m = masked(*m, factor, strength);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_flips_when_inverted() {
        assert_eq!(brush_strength(0.5, false), 0.5);
        assert_eq!(brush_strength(0.5, true), -0.5);
    }

    #[test]
    fn painting_moves_toward_one_by_the_remaining_amount() {
        assert!((masked(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((masked(0.5, 1.0, 0.5) - 0.75).abs() < 1e-6);
        assert!((masked(0.5, 0.5, 0.5) - 0.625).abs() < 1e-6);
        assert_eq!(masked(0.9, 1.0, 5.0), 1.0, "clamped to 1");
    }

    #[test]
    fn erasing_scales_the_mask_down_and_clamps_at_zero() {
        assert!((masked(0.8, 1.0, -0.5) - 0.4).abs() < 1e-6);
        assert_eq!(masked(0.0, 1.0, -0.5), 0.0, "nothing to erase");
        assert_eq!(masked(0.5, 1.0, -5.0), 0.0, "clamped to 0");
    }

    #[test]
    fn dab_touches_only_the_hit_vertices() {
        let mut mask = vec![0.0, 0.0, 0.0];
        dab(&mut mask, &[(0, 1.0), (2, 0.5)], 1.0);
        assert_eq!(mask, vec![1.0, 0.0, 0.5]);
        dab(&mut mask, &[(0, 1.0), (2, 0.5), (9, 1.0)], -1.0);
        assert_eq!(mask, vec![0.0, 0.0, 0.25], "out of range indices are ignored");
    }
}
