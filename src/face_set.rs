//! Sculpt face sets: the per-face id layer and its viewport colours.
//!
//! The layer is the int face attribute `.sculpt_face_set`, created lazily with every face set to 1
//! (sculpt_face_set.cc:158-178 `create_face_sets_mesh` / `ensure_face_sets_mesh`), which also sets
//! `Mesh::face_sets_color_default` to 1. Ids start at 1; `face_set_none_id` is 0 (sculpt_intern.hh:135).
//! The overlay colour is `BKE_paint_face_set_overlay_color_get()` (paint.cc:2789) over `BLI_hash_int_01()`
//! (BLI_hash_c.hh:94). BMesh and multires variants are not ported.

use crate::math_color::hsv_to_rgb;

/// `face_set_none_id`.
pub const NONE_ID: i32 = 0;
/// The id every face gets when the layer is created.
pub const DEFAULT_ID: i32 = 1;
/// `FACE_SET_MIN_FADE` (sculpt_face_set.hh:76).
pub const MIN_FADE: f32 = 0.05;

/// `BLI_hash_int_2d()`: Jenkins lookup3 `final()` on two keys (BLI_hash_c.hh:53).
pub fn hash_int_2d(kx: u32, ky: u32) -> u32 {
    let init = 0xdead_beef_u32.wrapping_add(2 << 2).wrapping_add(13);
    let (mut a, mut b, mut c) = (init, init, init);
    a = a.wrapping_add(kx);
    b = b.wrapping_add(ky);
    /* `final(a, b, c)` */
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    c
}

/// `BLI_hash_int()`.
pub fn hash_int(k: u32) -> u32 {
    hash_int_2d(k, 0)
}

/// `BLI_hash_int_01()`: the hash as a float in 0..=1.
pub fn hash_int_01(k: u32) -> f32 {
    hash_int(k) as f32 * (1.0 / 0xFFFF_FFFF_u32 as f32)
}

/// `unit_float_to_uchar_clamp()` (math_base_inline.cc:579).
fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// `BKE_paint_face_set_overlay_color_get()`: the overlay colour `[r, g, b]` of a face set id for a colour seed.
/// (Blender leaves the fourth channel of its `uchar[4]` unset; only red, green and blue are used.)
pub fn overlay_color(face_set: i32, seed: i32) -> [u8; 3] {
    const GOLDEN_RATIO_CONJUGATE: f32 = 0.618_034;
    let mut hue = GOLDEN_RATIO_CONJUGATE * (face_set + seed % 10) as f32;
    hue -= hue.floor();
    let sat = hash_int_01(face_set.wrapping_add(seed).wrapping_add(1) as u32);
    let val = hash_int_01(face_set.wrapping_add(seed).wrapping_add(2) as u32);
    let [r, g, b] = hsv_to_rgb(hue, 0.6 + sat * 0.25, 1.0 - val * 0.35);
    [unit_float_to_uchar_clamp(r), unit_float_to_uchar_clamp(g), unit_float_to_uchar_clamp(b)]
}

/// `create_face_sets_mesh()` / `ensure_face_sets_mesh()`: adds the layer with every face set to 1 if it does not
/// exist yet and sets `color_default` to 1. Returns whether it was created.
pub fn ensure(layer: &mut Option<Vec<i32>>, face_count: usize, color_default: &mut i32) -> bool {
    if layer.is_some() {
        return false;
    }
    *layer = Some(vec![DEFAULT_ID; face_count]);
    *color_default = DEFAULT_ID;
    true
}

/// `find_next_available_id(const Mesh &)`: one past the largest id (at least 2); no layer gives 2.
pub fn find_next_available_id(layer: Option<&[i32]>) -> i32 {
    layer.map_or(1, |ids| ids.iter().copied().fold(1, i32::max)) + 1
}

/// `initialize_none_to_id()`: faces with `face_set_none_id` take `new_id`. Without a layer nothing happens.
pub fn initialize_none_to_id(layer: Option<&mut Vec<i32>>, new_id: i32) {
    if let Some(ids) = layer {
        for id in ids.iter_mut().filter(|id| **id == NONE_ID) {
            *id = new_id;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_is_created_lazily_with_id_one() {
        let mut layer = None;
        let mut default = 7;
        assert!(ensure(&mut layer, 6, &mut default));
        assert_eq!(layer.as_deref(), Some(&[1, 1, 1, 1, 1, 1][..]));
        assert_eq!(default, 1);
        layer.as_mut().unwrap()[2] = 5;
        default = 3;
        assert!(!ensure(&mut layer, 6, &mut default), "an existing layer is kept");
        assert_eq!((layer.as_ref().unwrap()[2], default), (5, 3));
    }

    #[test]
    fn next_id_is_one_past_the_maximum() {
        assert_eq!(find_next_available_id(None), 2);
        assert_eq!(find_next_available_id(Some(&[])), 2);
        assert_eq!(find_next_available_id(Some(&[1, 1, 1])), 2);
        assert_eq!(find_next_available_id(Some(&[1, 4, 2])), 5);
        assert_eq!(find_next_available_id(Some(&[-3, 0])), 2);
    }

    #[test]
    fn none_faces_take_the_new_id() {
        let mut ids = vec![0, 3, 0, 1];
        initialize_none_to_id(Some(&mut ids), 9);
        assert_eq!(ids, [9, 3, 9, 1]);
        initialize_none_to_id(None, 9);
    }

    #[test]
    fn hash_matches_the_c_code() {
        /* Reference values from compiling BLI_hash_int_2d() (BLI_hash_c.hh:53) as C. */
        assert_eq!(hash_int(0), 3_695_015_161);
        assert_eq!(hash_int(1), 1_267_069_554);
        assert!((hash_int_01(0) - 3_695_015_161.0 / 4_294_967_295.0).abs() < 1e-6);
    }

    #[test]
    fn overlay_colours_match_the_c_code() {
        /* (face set, seed) -> rgb from `BKE_paint_face_set_overlay_color_get()` transcribed to C. */
        assert_eq!(overlay_color(1, 0), [60, 93, 175]);
        assert_eq!(overlay_color(2, 0), [126, 192, 34]);
        assert_eq!(overlay_color(5, 3), [203, 55, 104]);
        assert_eq!(overlay_color(12, 17), [139, 78, 211]);
    }

    #[test]
    fn overlay_colour_is_stable_and_saturated() {
        let c1 = overlay_color(1, 0);
        assert_eq!(c1, overlay_color(1, 0));
        assert_ne!(overlay_color(1, 0), overlay_color(2, 0));
        /* The seed changes the colour, but only its last digit feeds the hue (`seed % 10`). */
        assert_ne!(overlay_color(1, 0), overlay_color(1, 1));
        for id in 1..40 {
            let [r, g, b] = overlay_color(id, 3);
            let (mx, mn) = (r.max(g).max(b) as f32 / 255.0, r.min(g).min(b) as f32 / 255.0);
            /* Saturation 0.6..0.85 and value 0.65..1.0. */
            assert!(mx >= 0.64 && mx <= 1.0, "value {mx}");
            assert!((mx - mn) / mx >= 0.59 && (mx - mn) / mx <= 0.86, "saturation");
        }
    }
}
