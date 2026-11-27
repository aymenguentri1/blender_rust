//! `widget_active_color()` (interface_widgets.cc), the colours of a widget under the cursor
//! (`UI_HOVER`), with its helpers `color_mul_hsl_v3()`, `color_blend_v4_v4v4()` (same file),
//! `srgb_to_grayscale_byte()`, `rgb_uchar_to_float()` (math_color_inline.cc) and
//! `unit_float_to_uchar_clamp()` (math_base_inline.cc).

use crate::math_color::{hsl_to_rgb_v, rgb_to_hsl_v};
use crate::widget_colors::{Col4, WidgetColors};

/// Rec.709 luminance of an sRGB colour with byte weights that add up to 255.
pub fn srgb_to_grayscale_byte(c: Col4) -> u8 {
    ((54 * c[0] as u32 + 182 * c[1] as u32 + 19 * c[2] as u32) / 255) as u8
}

pub fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// Byte blend with the factor quantized to 0..255; the division by 256 is Blender's.
fn color_blend_v4_v4v4(col1: Col4, col2: Col4, fac: f32) -> Col4 {
    let f = unit_float_to_uchar_clamp(fac) as u32;
    std::array::from_fn(|i| ((f * col1[i] as u32 + (255 - f) * col2[i] as u32) / 256) as u8)
}

/// Scales hue, saturation and lightness; alpha is kept.
fn color_mul_hsl_v3(c: &mut Col4, h: f32, s: f32, l: f32) {
    let rgb = std::array::from_fn(|i| c[i] as f32 * (1.0 / 255.0));
    let hsl = rgb_to_hsl_v(rgb);
    let out = hsl_to_rgb_v([hsl[0] * h, hsl[1] * s, hsl[2] * l]);
    for i in 0..3 {
        c[i] = unit_float_to_uchar_clamp(out[i]);
    }
}

pub fn widget_active_color(wcol: &mut WidgetColors) {
    /* Light text on a darker inner: a dark theme. */
    let dark = srgb_to_grayscale_byte(wcol.text) > srgb_to_grayscale_byte(wcol.inner);
    color_mul_hsl_v3(&mut wcol.inner, 1.0, 1.15, if dark { 1.2 } else { 1.1 });
    wcol.outline = color_blend_v4_v4v4(wcol.outline, wcol.outline_sel, 0.5);
    color_mul_hsl_v3(&mut wcol.outline_sel, 1.0, 1.15, 1.15);
    color_mul_hsl_v3(&mut wcol.text, 1.0, 1.15, if dark { 1.25 } else { 0.8 });
}

/// Expected values come from the C functions compiled on their own.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_widgets::WCOL_REGULAR;

    #[test]
    fn dark_theme_hover() {
        let mut w = WCOL_REGULAR;
        widget_active_color(&mut w);
        assert_eq!(w.outline, [60, 60, 60, 254]);
        assert_eq!(w.outline_sel, [70, 70, 70, 255]);
        assert_eq!(w.inner, [101, 101, 101, 255]);
        assert_eq!(w.text, [255, 255, 255, 255]);
        assert_eq!((w.inner_sel, w.item), (WCOL_REGULAR.inner_sel, WCOL_REGULAR.item));
    }

    #[test]
    fn light_theme_hover_keeps_alpha() {
        let mut w = WidgetColors {
            outline: [10, 20, 30, 100],
            outline_sel: [200, 100, 50, 200],
            inner: [180, 190, 200, 77],
            text: [20, 40, 60, 255],
            ..Default::default()
        };
        widget_active_color(&mut w);
        assert_eq!(w.outline, [104, 59, 39, 149]);
        assert_eq!(w.outline_sel, [221, 118, 67, 200]);
        assert_eq!(w.inner, [201, 209, 217, 77]);
        assert_eq!(w.text, [14, 32, 50, 255]);
    }

    #[test]
    fn grayscale_weights_cover_the_byte_range() {
        assert_eq!(srgb_to_grayscale_byte([255; 4]), 255);
        assert_eq!(srgb_to_grayscale_byte([0, 255, 0, 0]), 182);
    }
}
