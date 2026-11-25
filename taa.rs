//! Samples and filter weights of the workbench temporal anti-aliasing.
//!
//! Ported from Blender:
//!  - `RandomNumberGenerator`                (blenlib/BLI_rand.hh:20-151): the 48 bit linear
//!    congruential generator behind `BLI_rng_new()` / `BLI_rng_get_double()`
//!  - `BLI_jitterate1()`, `BLI_jitterate2()`, `BLI_jitter_init()`
//!                                           (blenlib/intern/jitter_2d.cc:19-173)
//!  - `TaaSamples::init_samples()`           (draw/engines/workbench/workbench_effect_antialiasing.cc:14-55):
//!    the samples are moved so that the one closest to the centre is the first, recentred to
//!    [-1, 1) and sorted by the farthest distance from the previous one
//!  - `filter_blackman_harris()`, `setup_taa_weights()`
//!                                           (workbench_effect_antialiasing.cc:84-112): the weights of the
//!    3x3 neighbourhood for a sample offset, with a filter width of 2
//!
//! The default `U.viewport_aa` is 8 (`DNA_userdef_types.h:1311`), so only the 8 sample table is built.

use crate::area::Rcti;
use crate::colorspace_lut::{linear_to_srgb8, srgb8_to_linear};
use crate::view3d::Mat4;


/// `RandomNumberGenerator` seeded with `seed()`.
struct Rng {
    x: u64,
}

impl Rng {
    fn new(seed: u32) -> Self {
        Self { x: ((seed as u64) << 16) | 0x330E }
    }

    /// `get_int32()`: the 31 high bits of the 48 bit state after one step.
    fn get_int32(&mut self) -> i32 {
        self.x = 0x5_DEEC_E66D_u64.wrapping_mul(self.x).wrapping_add(0xB) & 0x0000_FFFF_FFFF_FFFF;
        (self.x >> 17) as i32
    }

    /// `get_double()`: a value in [0, 1).
    fn get_double(&mut self) -> f64 {
        self.get_int32() as f64 / 2_147_483_648.0
    }
}

/// `BLI_jitterate1()`.
fn jitterate1(jit1: &mut [[f32; 2]], jit2: &mut [[f32; 2]], radius1: f32) {
    let num = jit1.len();
    for i in (0..num).rev() {
        let (mut dvecx, mut dvecy) = (0.0f32, 0.0f32);
        let (mut x, mut y) = (jit1[i][0], jit1[i][1]);
        for j in (0..num).rev() {
            if i == j {
                continue;
            }
            let mut vecx = jit1[j][0] - x - 1.0;
            let mut vecy = jit1[j][1] - y - 1.0;
            for _ in 0..3 {
                for step in 0..3 {
                    if vecx.abs() < radius1 && vecy.abs() < radius1 {
                        let len = (vecx * vecx + vecy * vecy).sqrt();
                        if len > 0.0 && len < radius1 {
                            let len = len / radius1;
                            dvecx += vecx / len;
                            dvecy += vecy / len;
                        }
                    }
                    /* `vecx += 1`, `vecx += 1`, then `vecx -= 2`. */
                    vecx += if step < 2 { 1.0 } else { -2.0 };
                }
                vecy += 1.0;
            }
        }

        x -= dvecx / 18.0;
        y -= dvecy / 18.0;
        x -= x.floor();
        y -= y.floor();
        jit2[i] = [x, y];
    }
    jit1.copy_from_slice(&jit2[..num]);
}

/// `BLI_jitterate2()`.
fn jitterate2(jit1: &mut [[f32; 2]], jit2: &mut [[f32; 2]], radius2: f32) {
    let num = jit1.len();
    for i in (0..num).rev() {
        let (mut dvecx, mut dvecy) = (0.0f32, 0.0f32);
        let (mut x, mut y) = (jit1[i][0], jit1[i][1]);
        for j in (0..num).rev() {
            if i == j {
                continue;
            }
            let mut vecx = jit1[j][0] - x - 1.0;
            let mut vecy = jit1[j][1] - y - 1.0;

            for _ in 0..3 {
                if vecx.abs() < radius2 {
                    dvecx += vecx * radius2;
                }
                vecx += 1.0;
            }
            for _ in 0..3 {
                if vecy.abs() < radius2 {
                    dvecy += vecy * radius2;
                }
                vecy += 1.0;
            }
        }

        x -= dvecx / 2.0;
        y -= dvecy / 2.0;
        x -= x.floor();
        y -= y.floor();
        jit2[i] = [x, y];
    }
    jit1.copy_from_slice(&jit2[..num]);
}

/// `BLI_jitter_init()`: `num` points in [-0.5, 0.5).
fn jitter_init(num: usize) -> Vec<[f32; 2]> {
    let mut jitarr = vec![[0.0f32; 2]; num];
    if num == 0 {
        return jitarr;
    }
    let mut jit2 = vec![[0.0f32; 2]; num];

    let number_fl = num as f32;
    let number_fl_sqrt = number_fl.sqrt();
    let rad1 = 1.0 / number_fl_sqrt;
    let rad2 = 1.0 / number_fl;
    let rad3 = number_fl_sqrt / number_fl;

    let mut rng = Rng::new(31_415_926 + num as u32);

    let mut x = 0.0f32;
    for (i, point) in jitarr.iter_mut().enumerate() {
        point[0] = x + rad1 * (0.5 - rng.get_double()) as f32;
        point[1] = i as f32 / number_fl + rad1 * (0.5 - rng.get_double()) as f32;
        x += rad3;
        x -= x.floor();
    }

    for _ in 0..24 {
        jitterate1(&mut jitarr, &mut jit2, rad1);
        jitterate1(&mut jitarr, &mut jit2, rad1);
        jitterate2(&mut jitarr, &mut jit2, rad2);
    }

    /* Finally, move jitter to be centered around (0, 0). */
    for point in &mut jitarr {
        point[0] -= 0.5;
        point[1] -= 0.5;
    }
    jitarr
}

fn length_squared(v: [f32; 2]) -> f32 {
    v[0] * v[0] + v[1] * v[1]
}

/// `TaaSamples::init_samples()` for `num` samples.
fn init_samples(num: usize) -> Vec<[f32; 2]> {
    let mut samples = jitter_init(num);

    /* Find closest element to center. */
    let mut closest_index = 0;
    let mut closest_squared_distance = 1.0f32;
    for (i, sample) in samples.iter().enumerate() {
        let squared_dist = length_squared(*sample);
        if squared_dist < closest_squared_distance {
            closest_squared_distance = squared_dist;
            closest_index = i;
        }
    }
    let closest_sample = samples[closest_index];

    for sample in &mut samples {
        /* Move jitter samples so that closest sample is in center. */
        sample[0] -= closest_sample[0];
        sample[1] -= closest_sample[1];
        /* Avoid samples outside range (wrap around), then recenter the distribution [-1..1]. */
        *sample = [(sample[0] + 0.5) % 1.0, (sample[1] + 0.5) % 1.0];
        *sample = [sample[0] * 2.0 - 1.0, sample[1] * 2.0 - 1.0];
    }

    /* Swap center sample to the start of the array. */
    if closest_index != 0 {
        samples.swap(0, closest_index);
    }

    /* Sort list based on farthest distance with previous. */
    for i in 0..num.saturating_sub(2) {
        let mut squared_dist = 0.0f32;
        let mut index = i;
        for j in (i + 1)..num {
            let d = length_squared([samples[i][0] - samples[j][0], samples[i][1] - samples[j][1]]);
            if d > squared_dist {
                squared_dist = d;
                index = j;
            }
        }
        samples.swap(i + 1, index);
    }
    samples
}

/// `get_taa_samples().x8`: the sample offsets of the default 8 sample anti-aliasing, in [-1, 1).
pub fn taa_samples_x8() -> Vec<[f32; 2]> {
    init_samples(8)
}

/// `filter_blackman_harris()`.
fn filter_blackman_harris(x: f32, width: f32) -> f32 {
    if x > width * 0.5 {
        return 0.0;
    }
    let x = 2.0 * std::f32::consts::PI * (x / width + 0.5).clamp(0.0, 1.0);
    0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos()
}

/// `setup_taa_weights()`: the weights of the 3x3 neighbourhood (x outer, y inner, from -1 to 1) for a
/// sample `offset`, and their sum.
pub fn taa_weights(offset: [f32; 2]) -> ([f32; 9], f32) {
    let filter_width = 2.0;
    let mut weights = [0.0f32; 9];
    let mut sum = 0.0f32;
    let mut i = 0;
    for x in -1..=1 {
        for y in -1..=1 {
            let co = [x as f32 - offset[0], y as f32 - offset[1]];
            let weight = filter_blackman_harris(length_squared(co).sqrt(), filter_width);
            sum += weight;
            weights[i] = weight;
            i += 1;
        }
    }
    (weights, sum)
}

/// `SceneState::samples_len` for the default `U.viewport_aa` of 8.
pub const SAMPLES_LEN: usize = 8;

/// The accumulation of `AntiAliasingPass` (`taa_accumulation_ps_`, `smaa_resolve_ps_` with a mix factor
/// of 0): the weighted sum of the log2 colours (`workbench_taa` shader) of the samples drawn since the
/// view last changed, divided by the accumulated weight and brought back with `exp2(x) - 1`. The SMAA
/// stage that Blender mixes in while fewer than 4 samples are accumulated is not ported, so the first
/// frames are not smoothed; the finished result (8 samples) is the same. Neighbours outside the
/// region are clamped to the edge.
pub struct TaaState {
    key: Option<(Mat4, i32, i32, bool)>,
    samples: Vec<[f32; 2]>,
    /// `SceneState::sample`: the sample drawn next, `SAMPLES_LEN` once TAA is finished.
    pub sample: usize,
    weight_accum: f32,
    /// Region pixels, y up, row-major.
    accum: Vec<[f32; 3]>,
}

impl Default for TaaState {
    fn default() -> Self {
        Self { key: None, samples: taa_samples_x8(), sample: 0, weight_accum: 0.0, accum: Vec::new() }
    }
}

impl TaaState {
    /// The sample to draw now; the accumulation restarts at sample 0 when the view (`persmat`), the
    /// region size or the X-ray mode changed (`SceneState::reset_taa`).
    pub fn begin(&mut self, persmat: Mat4, rw: i32, rh: i32, xray: bool) -> usize {
        let key = (persmat, rw, rh, xray);
        if self.key != Some(key) {
            self.key = Some(key);
            self.sample = 0;
        }
        self.sample
    }

    /// `get_taa_samples().x8[sample]`.
    pub fn offset(&self, sample: usize) -> [f32; 2] {
        self.samples[sample]
    }

    /// `taa_accumulation_ps_` for the current sample: `img` is the sample drawn into a `rw` x `rh`
    /// `0x00RRGGBB` buffer (row 0 at the top) with the jitter `offset` applied to the projection.
    pub fn accumulate(&mut self, img: &[u32], rw: usize, rh: usize, offset: [f32; 2]) {
        let (weights, sum) = taa_weights(offset);
        if self.sample == 0 {
            self.accum.clear();
            self.accum.resize(rw * rh, [0.0; 3]);
            self.weight_accum = 0.0;
        }
        if self.accum.len() != rw * rh || img.len() != rw * rh {
            return;
        }
        if rw == 0 || rh == 0 {
            return;
        }
        /* Use log2 space to avoid highlights creating too much aliasing. */
        static LOG_LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
        let lut = LOG_LUT.get_or_init(|| std::array::from_fn(|v| (srgb8_to_linear(v as u32) + 1.0).log2()));
        let log: Vec<[f32; 3]> = img
            .iter()
            .map(|&c| [lut[((c >> 16) & 0xff) as usize], lut[((c >> 8) & 0xff) as usize], lut[(c & 0xff) as usize]])
            .collect();
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(rh);
        let rows_per = (rh + threads - 1) / threads;
        let log = &log;
        let weights = &weights;
        std::thread::scope(|s| {
            for (ci, chunk) in self.accum.chunks_mut(rows_per * rw).enumerate() {
                s.spawn(move || {
                    let py0 = ci * rows_per;
                    for (r, row) in chunk.chunks_mut(rw).enumerate() {
                        let py = py0 + r;
                        for (px, a) in row.iter_mut().enumerate() {
                            let mut acc = [0.0f32; 3];
                            let mut i = 0;
                            for x in -1i32..=1 {
                                for y in -1i32..=1 {
                                    let w = weights[i];
                                    i += 1;
                                    if w == 0.0 {
                                        continue;
                                    }
                                    let sx = (px as i32 + x).clamp(0, rw as i32 - 1) as usize;
                                    let sy = (py as i32 + y).clamp(0, rh as i32 - 1) as usize;
                                    let l = &log[(rh - 1 - sy) * rw + sx];
                                    for k in 0..3 {
                                        acc[k] += l[k] * w;
                                    }
                                }
                            }
                            for k in 0..3 {
                                a[k] += acc[k];
                            }
                        }
                    }
                });
            }
        });
        self.weight_accum += sum;
        self.sample += 1;
    }

    /// `smaa_resolve_ps_` with `mix_factor = 0`: writes the accumulated image into `region`
    /// (inclusive Blender rect, y up) of the `0x00RRGGBB` buffer.
    pub fn resolve(&self, buf: &mut [u32], width: usize, height: usize, region: Rcti) {
        let (rw, rh) = (region.size_x() + 1, region.size_y() + 1);
        if rw <= 0 || rh <= 0 || self.weight_accum <= 0.0 || self.accum.len() != (rw * rh) as usize {
            return;
        }
        let (rwu, rhu) = (rw as usize, rh as usize);
        let inv_w = 1.0 / self.weight_accum;
        let mut resolved = vec![0u32; rwu * rhu];
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(rhu);
        let rows_per = (rhu + threads - 1) / threads;
        let accum = &self.accum;
        std::thread::scope(|s| {
            for (ci, chunk) in resolved.chunks_mut(rows_per * rwu).enumerate() {
                s.spawn(move || {
                    let base = ci * rows_per * rwu;
                    for (i, o) in chunk.iter_mut().enumerate() {
                        let a = &accum[base + i];
                        let mut out = 0u32;
                        for (k, shift) in [16u32, 8, 0].into_iter().enumerate() {
                            /* Exit log2 space. */
                            let lin = (a[k] * inv_w).exp2() - 1.0;
                            out |= linear_to_srgb8(lin) << shift;
                        }
                        *o = out;
                    }
                });
            }
        });
        for py in 0..rh {
            for px in 0..rw {
                let (x, y) = (region.xmin + px, region.ymin + py);
                if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                    continue;
                }
                buf[(height - 1 - y as usize) * width + x as usize] = resolved[(py * rw + px) as usize];
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /* The first sample is the one closest to the centre, which is moved to exactly (0, 0); all
     * samples are in [-1, 1) and distinct. */
    #[test]
    fn samples_x8_layout() {
        let s = taa_samples_x8();
        assert_eq!(s.len(), 8);
        assert_eq!(s[0], [0.0, 0.0]);
        for (i, a) in s.iter().enumerate() {
            assert!(a[0] >= -1.0 && a[0] < 1.0 && a[1] >= -1.0 && a[1] < 1.0, "{:?}", a);
            for b in &s[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    /* The generator is the deterministic `drand48` style sequence of `BLI_rng_new()`. */
    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut a, mut b) = (Rng::new(31_415_934), Rng::new(31_415_934));
        for _ in 0..16 {
            let v = a.get_double();
            assert_eq!(v, b.get_double());
            assert!((0.0..1.0).contains(&v));
        }
    }

    /* The Blackman-Harris window is 1 at the centre and 0 beyond half the filter width; a sample
     * on the pixel centre weighs the centre texel 1 and the four direct neighbours 0.5 filter radius
     * further out. */
    #[test]
    fn weights_for_centered_sample() {
        assert!((filter_blackman_harris(0.0, 2.0) - 1.0).abs() < 1e-5);
        assert_eq!(filter_blackman_harris(1.5, 2.0), 0.0);
        let (w, sum) = taa_weights([0.0, 0.0]);
        assert!((w[4] - 1.0).abs() < 1e-5, "{}", w[4]);
        assert!(w[1] > 0.0 && w[1] < w[4]);
        assert!((sum - w.iter().sum::<f32>()).abs() < 1e-6);
    }

    /* A uniform image stays unchanged through the log2 accumulation of all 8 samples, and the
     * accumulation restarts when the view key changes. */
    #[test]
    fn uniform_image_is_unchanged() {
        let (w, h) = (8usize, 6usize);
        let img = vec![0x003d_3d3d_u32; w * h];
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let persmat = [[0.0f32; 4]; 4];
        let mut taa = TaaState::default();
        for s in 0..SAMPLES_LEN {
            assert_eq!(taa.begin(persmat, w as i32, h as i32, false), s);
            let offset = taa.offset(s);
            taa.accumulate(&img, w, h, offset);
        }
        assert_eq!(taa.begin(persmat, w as i32, h as i32, false), SAMPLES_LEN);
        let mut out = vec![0u32; w * h];
        taa.resolve(&mut out, w, h, region);
        for &p in &out {
            assert!((((p >> 16) & 0xff) as i32 - 0x3d).abs() <= 1, "{:#08x}", p);
        }
        assert_eq!(taa.begin(persmat, w as i32, h as i32, true), 0);
    }
}
