//! Lookup tables for the per-pixel blend of the viewport overlays, replacing two `powf()` calls
//! per channel: `srgb_to_linearrgb()` of an 8 bit value is exact, `linearrgb_to_srgb()` is
//! sampled at `LIN_TO_SRGB_STEPS` points and rounded to 8 bit.

use std::sync::OnceLock;

use crate::overlay_grid::srgb_to_linearrgb;
use crate::overlay_grid_draw::linearrgb_to_srgb;

const LIN_TO_SRGB_STEPS: usize = 16384;

/// `srgb_to_linearrgb(v / 255)` for an 8 bit channel value.
pub fn srgb8_to_linear(v: u32) -> f32 {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            *e = srgb_to_linearrgb(i as f32 / 255.0);
        }
        t
    });
    table[(v & 0xff) as usize]
}

/// `linearrgb_to_srgb(lin)` as an 8 bit channel value; `lin` is clamped to [0, 1].
pub fn linear_to_srgb8(lin: f32) -> u32 {
    static TABLE: OnceLock<Vec<u8>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        (0..LIN_TO_SRGB_STEPS)
            .map(|i| {
                let lin = i as f32 / (LIN_TO_SRGB_STEPS - 1) as f32;
                (linearrgb_to_srgb(lin) * 255.0 + 0.5).min(255.0) as u8
            })
            .collect()
    });
    table[(lin.clamp(0.0, 1.0) * (LIN_TO_SRGB_STEPS - 1) as f32 + 0.5) as usize] as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_match_the_conversions() {
        assert_eq!(srgb8_to_linear(0), 0.0);
        assert!((srgb8_to_linear(255) - 1.0).abs() < 1e-6);
        assert!((srgb8_to_linear(61) - srgb_to_linearrgb(61.0 / 255.0)).abs() < 1e-9);
        for v in 0..=255u32 {
            assert_eq!(linear_to_srgb8(srgb8_to_linear(v)), v, "{v}");
        }
        assert_eq!(linear_to_srgb8(-1.0), 0);
        assert_eq!(linear_to_srgb8(2.0), 255);
    }
}
