//! Port of the HSV / HSL conversions of `source/blender/blenlib/intern/math_color.cc`
//! (`hsv_to_rgb`, `rgb_to_hsv`, `hsl_to_rgb`, `rgb_to_hsl` and their `_v` variants, `rgb_to_yuv`,
//! `yuv_to_rgb`, `rgb_to_ycc`, `ycc_to_rgb`).
//! Tests are the `RGBToHSVRoundtrip` / `RGBToHSLRoundtrip` / `RGBToYUVRoundtrip` /
//! `RGBToYCCRoundtrip` cases of `tests/BLI_math_color_test.cc`,
//! plus primary colours checked against the formulas of the source.
//! The sRGB <-> linear conversions of the same file live in `overlay_grid.rs` /
//! `overlay_grid_draw.rs`.

/// `CLAMP(a, 0, 1)` of the source.
fn clamp01(a: f32) -> f32 {
    a.clamp(0.0, 1.0)
}

/// `hsv_to_rgb()`: returns `[r, g, b]`.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let nr = clamp01((h * 6.0 - 3.0).abs() - 1.0);
    let ng = clamp01(2.0 - (h * 6.0 - 2.0).abs());
    let nb = clamp01(2.0 - (h * 6.0 - 4.0).abs());

    [((nr - 1.0) * s + 1.0) * v, ((ng - 1.0) * s + 1.0) * v, ((nb - 1.0) * s + 1.0) * v]
}

/// `hsv_to_rgb_v()`.
pub fn hsv_to_rgb_v(hsv: [f32; 3]) -> [f32; 3] {
    hsv_to_rgb(hsv[0], hsv[1], hsv[2])
}

/// `hsl_to_rgb()`: returns `[r, g, b]`.
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let nr = clamp01((h * 6.0 - 3.0).abs() - 1.0);
    let ng = clamp01(2.0 - (h * 6.0 - 2.0).abs());
    let nb = clamp01(2.0 - (h * 6.0 - 4.0).abs());

    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;

    [(nr - 0.5) * chroma + l, (ng - 0.5) * chroma + l, (nb - 0.5) * chroma + l]
}

/// `hsl_to_rgb_v()`.
pub fn hsl_to_rgb_v(hsl: [f32; 3]) -> [f32; 3] {
    hsl_to_rgb(hsl[0], hsl[1], hsl[2])
}

/// `rgb_to_hsv()`: returns `[h, s, v]`.
pub fn rgb_to_hsv(mut r: f32, mut g: f32, mut b: f32) -> [f32; 3] {
    let mut k = 0.0f32;

    if g < b {
        std::mem::swap(&mut g, &mut b);
        k = -1.0;
    }
    let mut min_gb = b;
    if r < g {
        std::mem::swap(&mut r, &mut g);
        k = -2.0 / 6.0 - k;
        min_gb = g.min(b);
    }

    let chroma = r - min_gb;

    [(k + (g - b) / (6.0 * chroma + 1e-20)).abs(), chroma / (r + 1e-20), r]
}

/// `rgb_to_hsv_v()`.
pub fn rgb_to_hsv_v(rgb: [f32; 3]) -> [f32; 3] {
    rgb_to_hsv(rgb[0], rgb[1], rgb[2])
}

/// `rgb_to_hsl()`: returns `[h, s, l]`.
pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> [f32; 3] {
    let cmax = r.max(g).max(b);
    let cmin = r.min(g).min(b);
    let l = (1.0f32).min((cmax + cmin) / 2.0);

    let (mut h, s);
    if cmax == cmin {
        /* achromatic */
        h = 0.0;
        s = 0.0;
    } else {
        let d = cmax - cmin;
        s = if l > 0.5 { d / (2.0 - cmax - cmin) } else { d / (cmax + cmin) };
        h = if cmax == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if cmax == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
    }
    h /= 6.0;

    [h, s, l]
}

/// `rgb_to_hsl_v()`.
pub fn rgb_to_hsl_v(rgb: [f32; 3]) -> [f32; 3] {
    rgb_to_hsl(rgb[0], rgb[1], rgb[2])
}

/// `BLI_YUV_ITU_BT601` / `BLI_YUV_ITU_BT709`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YuvColorspace {
    ItuBt601,
    ItuBt709,
}

/// `BLI_YCC_ITU_BT601` / `BLI_YCC_ITU_BT709` / `BLI_YCC_JFIF_0_255`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YccColorspace {
    ItuBt601,
    ItuBt709,
    Jfif0_255,
}

/// `rgb_to_yuv()`: returns `[y, u, v]`.
pub fn rgb_to_yuv(r: f32, g: f32, b: f32, colorspace: YuvColorspace) -> [f32; 3] {
    match colorspace {
        YuvColorspace::ItuBt601 => [
            0.299 * r + 0.587 * g + 0.114 * b,
            -0.147 * r - 0.289 * g + 0.436 * b,
            0.615 * r - 0.515 * g - 0.100 * b,
        ],
        YuvColorspace::ItuBt709 => [
            0.2126 * r + 0.7152 * g + 0.0722 * b,
            -0.09991 * r - 0.33609 * g + 0.436 * b,
            0.615 * r - 0.55861 * g - 0.05639 * b,
        ],
    }
}

/// `yuv_to_rgb()`: returns `[r, g, b]`.
pub fn yuv_to_rgb(y: f32, u: f32, v: f32, colorspace: YuvColorspace) -> [f32; 3] {
    match colorspace {
        YuvColorspace::ItuBt601 => [y + 1.140 * v, y - 0.394 * u - 0.581 * v, y + 2.032 * u],
        YuvColorspace::ItuBt709 => {
            [y + 1.28033 * v, y - 0.21482 * u - 0.38059 * v, y + 2.12798 * u]
        }
    }
}

/// `rgb_to_ycc()`: returns `[y, cb, cr]` (0-255 range for JFIF, 16-235 / 16-240 for ITU).
pub fn rgb_to_ycc(r: f32, g: f32, b: f32, colorspace: YccColorspace) -> [f32; 3] {
    let sr = 255.0 * r;
    let sg = 255.0 * g;
    let sb = 255.0 * b;

    match colorspace {
        YccColorspace::ItuBt601 => [
            (0.257 * sr) + (0.504 * sg) + (0.098 * sb) + 16.0,
            (-0.148 * sr) - (0.291 * sg) + (0.439 * sb) + 128.0,
            (0.439 * sr) - (0.368 * sg) - (0.071 * sb) + 128.0,
        ],
        YccColorspace::ItuBt709 => [
            (0.183 * sr) + (0.614 * sg) + (0.062 * sb) + 16.0,
            (-0.101 * sr) - (0.338 * sg) + (0.439 * sb) + 128.0,
            (0.439 * sr) - (0.399 * sg) - (0.040 * sb) + 128.0,
        ],
        YccColorspace::Jfif0_255 => [
            (0.299 * sr) + (0.587 * sg) + (0.114 * sb),
            (-0.16874 * sr) - (0.33126 * sg) + (0.5 * sb) + 128.0,
            (0.5 * sr) - (0.41869 * sg) - (0.08131 * sb) + 128.0,
        ],
    }
}

/// `ycc_to_rgb()`: returns `[r, g, b]` in the range 0 - 1.
pub fn ycc_to_rgb(y: f32, cb: f32, cr: f32, colorspace: YccColorspace) -> [f32; 3] {
    let [r, g, b] = match colorspace {
        YccColorspace::ItuBt601 => [
            1.164 * (y - 16.0) + 1.596 * (cr - 128.0),
            1.164 * (y - 16.0) - 0.813 * (cr - 128.0) - 0.392 * (cb - 128.0),
            1.164 * (y - 16.0) + 2.017 * (cb - 128.0),
        ],
        YccColorspace::ItuBt709 => [
            1.164 * (y - 16.0) + 1.793 * (cr - 128.0),
            1.164 * (y - 16.0) - 0.534 * (cr - 128.0) - 0.213 * (cb - 128.0),
            1.164 * (y - 16.0) + 2.115 * (cb - 128.0),
        ],
        YccColorspace::Jfif0_255 => [
            y + 1.402 * cr - 179.456,
            y - 0.34414 * cb - 0.71414 * cr + 135.45984,
            y + 1.772 * cb - 226.816,
        ],
    };
    [r / 255.0, g / 255.0, b / 255.0]
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_to_yuv_roundtrip() {
        let orig_rgb = [0.1f32, 0.2, 0.3];
        let yuv = rgb_to_yuv(orig_rgb[0], orig_rgb[1], orig_rgb[2], YuvColorspace::ItuBt709);
        let rgb = yuv_to_rgb(yuv[0], yuv[1], yuv[2], YuvColorspace::ItuBt709);
        assert!(v3_near(orig_rgb, rgb, 1e-4), "{rgb:?}");

        /* BT601 is not in the Blender test; same input, checked at the YCC tolerance. */
        let yuv = rgb_to_yuv(orig_rgb[0], orig_rgb[1], orig_rgb[2], YuvColorspace::ItuBt601);
        let rgb = yuv_to_rgb(yuv[0], yuv[1], yuv[2], YuvColorspace::ItuBt601);
        assert!(v3_near(orig_rgb, rgb, 1e-3), "{rgb:?}");
    }

    #[test]
    fn rgb_to_ycc_roundtrip() {
        let orig_rgb = [0.1f32, 0.2, 0.3];
        for colorspace in [YccColorspace::ItuBt601, YccColorspace::ItuBt709, YccColorspace::Jfif0_255] {
            let ycc = rgb_to_ycc(orig_rgb[0], orig_rgb[1], orig_rgb[2], colorspace);
            let rgb = ycc_to_rgb(ycc[0], ycc[1], ycc[2], colorspace);
            assert!(v3_near(orig_rgb, rgb, 1e-3), "{colorspace:?}: {rgb:?}");
        }
    }

    /// `EXPECT_V3_NEAR`.
    fn v3_near(a: [f32; 3], b: [f32; 3], epsilon: f32) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() <= epsilon)
    }

    #[test]
    fn rgb_to_hsv_roundtrip() {
        let orig_rgb = [0.1f32, 0.2, 0.3];
        let rgb = hsv_to_rgb_v(rgb_to_hsv_v(orig_rgb));
        assert!(v3_near(orig_rgb, rgb, 1e-5), "{rgb:?}");
    }

    #[test]
    fn rgb_to_hsl_roundtrip() {
        let orig_rgb = [0.1f32, 0.2, 0.3];
        let rgb = hsl_to_rgb_v(rgb_to_hsl_v(orig_rgb));
        assert!(v3_near(orig_rgb, rgb, 1e-5), "{rgb:?}");
    }

    #[test]
    fn primary_colors() {
        assert!(v3_near(rgb_to_hsv(1.0, 0.0, 0.0), [0.0, 1.0, 1.0], 1e-6));
        assert!(v3_near(rgb_to_hsv(0.0, 1.0, 0.0), [1.0 / 3.0, 1.0, 1.0], 1e-6));
        assert!(v3_near(rgb_to_hsv(0.0, 0.0, 1.0), [2.0 / 3.0, 1.0, 1.0], 1e-6));
        assert!(v3_near(rgb_to_hsl(1.0, 0.0, 0.0), [0.0, 1.0, 0.5], 1e-6));
        assert!(v3_near(rgb_to_hsl(0.5, 0.5, 0.5), [0.0, 0.0, 0.5], 1e-6));
        assert!(v3_near(hsv_to_rgb(0.0, 0.0, 0.7), [0.7, 0.7, 0.7], 1e-6));
    }
}
