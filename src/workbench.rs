//! Solid mode drawing of the default cube with the workbench studio lighting.
//!
//! Ported from Blender:
//!  - `BKE_studiolight_default()`            (blenkernel/intern/studiolight.cc:752-805): the four lights
//!    of the internal "Default" studio light; the startup 3D viewport has an empty `studio_light`
//!    name, so `BKE_studiolight_find()` falls back to it
//!  - `get_light_data_from_studio_solidlight()`, `SceneResources::init()`
//!    (draw/engines/workbench/workbench_resources.cc:67-165): the
//!    light directions are used as they are (view space) because `V3D_SHADING_WORLD_ORIENTATION` is not
//!    set; the ambient colour is black and specular is on (`V3D_SHADING_SPECULAR_HIGHLIGHT`)
//!  - `get_world_lighting()`                 (workbench/shaders/workbench_world_light.bsl.hh)
//!  - the composite `frag()`                 (workbench/shaders/workbench_composite.bsl.hh): the incident
//!    vector comes from the pixel ray, the normal is the (view space) face normal, back faces flip it
//!  - `Material::pack_data()`, `float_pair_encode()` / `float_pair_decode()`, `material_data_get()`
//!    (workbench_private.hh:143, shaders/workbench_common.bsl.hh,
//!    shaders/workbench_material.bsl.hh): the roughness / metallic quantisation
//!  - `drw_view_incident_vector()`           (draw/intern/shaders/draw_view_lib.glsl:86)
//!
//! The cube is the startup scene's `Cube` (2 m at the origin, flat shaded) with the default material
//! (`DNA_material_types.h`: base colour 0.8, roughness 0.4, metallic 0). Colours are shaded in linear
//! space and stored as sRGB (the "Standard" view transform). Cavity, shadows, outlines and
//! anti-aliasing are not modelled.

use crate::area::Rcti;
use crate::colorspace_lut::{linear_to_srgb8, srgb8_to_linear};
use crate::mesh::Mesh;
use crate::scene::{mat3_mul_v3, mat4_mul_point, normal_matrix, Scene};
use crate::view3d::{Mat4, RegionView3D};

type V3 = [f32; 3];

/// `SolidLight` of the studio light: view space direction, diffuse colour with its wrap, specular colour.
struct SolidLight {
    direction: V3,
    diffuse: V3,
    wrap: f32,
    specular: V3,
}

/// `BKE_studiolight_default()`; the ambient colour is zero.
const LIGHTS: [SolidLight; 4] = [
    SolidLight {
        direction: [-0.352546, 0.170931, -0.920051],
        diffuse: [0.033103; 3],
        wrap: 0.526620,
        specular: [0.266761; 3],
    },
    SolidLight {
        direction: [-0.408163, 0.346939, 0.844415],
        diffuse: [0.521083, 0.538226, 0.538226],
        wrap: 0.0,
        specular: [0.599030; 3],
    },
    SolidLight {
        direction: [0.521739, 0.826087, 0.212999],
        diffuse: [0.038403, 0.034357, 0.049530],
        wrap: 0.478261,
        specular: [0.106102, 0.125981, 0.158523],
    },
    SolidLight {
        direction: [0.624519, -0.562067, -0.542269],
        diffuse: [0.090838, 0.082080, 0.072255],
        wrap: 0.2,
        specular: [0.106535, 0.084771, 0.066080],
    },
];

/// `Material::r, g, b`, `roughness` and `metallic` of the default material.
const BASE_COLOR: V3 = [0.8; 3];
const ROUGHNESS: f32 = 0.4;
const METALLIC: f32 = 0.0;

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn normalize(v: V3) -> V3 {
    let len = dot(v, v).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

/// `fast_rcp()`: `intBitsToFloat(0x7eef370b - floatBitsToInt(v))`.
fn fast_rcp(v: f32) -> f32 {
    f32::from_bits(0x7eef_370b_u32.wrapping_sub(v.to_bits()))
}

/// `wrapped_lighting()`.
fn wrapped_lighting(nl: f32, w: f32) -> f32 {
    let w_1 = w + 1.0;
    ((nl + w) * fast_rcp(w_1 * w_1)).clamp(0.0, 1.0)
}

/// The roughness and metallic the composite pass sees: `pack_data()` stores the square root of the
/// roughness as 8 bit, `float_pair_encode()` keeps 5 bits of roughness and 3 bits of metallic.
fn material_roughness_metallic() -> (f32, f32) {
    let to_uchar = |v: f32| ((v * 255.0 + 0.5) as u32).min(255) as f32 / 255.0;
    let roughness = to_uchar(ROUGHNESS.sqrt());
    let metallic = to_uchar(METALLIC);
    let (iv1, iv2) = ((roughness * 31.0) as i32, (metallic * 7.0) as i32);
    (iv1 as f32 / 31.0, iv2 as f32 / 7.0)
}

/// The part of `get_world_lighting()` that depends only on the material and the lights, computed once per frame
/// instead of per pixel (`exp2`, the wrap factors, the base colours).
struct LightPre {
    direction: V3,
    diffuse: V3,
    specular: V3,
    wrap: f32,
    shininess: f32,
    /// `shininess * 0.125 + 1`.
    shininess_scale: f32,
    /// `wrap + (1 - wrap) * roughness`.
    w: f32,
    /// `wrap * wrap`.
    k: f32,
}

struct Lighting {
    roughness: f32,
    lights: Vec<LightPre>,
    diffuse_color: V3,
    specular_base: V3,
}

impl Lighting {
    fn new(roughness: f32, metallic: f32) -> Lighting {
        let lights = LIGHTS
            .iter()
            .map(|l| {
                /* Reduce gloss for smooth light. */
                let gloss = (1.0 - roughness) * (1.0 - l.wrap);
                let shininess = (10.0 * gloss + 1.0).exp2();
                LightPre {
                    direction: l.direction,
                    diffuse: l.diffuse,
                    specular: l.specular,
                    wrap: l.wrap,
                    shininess,
                    shininess_scale: shininess * 0.125 + 1.0,
                    w: l.wrap + (1.0 - l.wrap) * roughness,
                    k: l.wrap * l.wrap,
                }
            })
            .collect();
        Lighting {
            roughness,
            lights,
            diffuse_color: BASE_COLOR.map(|c| c * (1.0 - metallic)),
            specular_base: BASE_COLOR.map(|c| 0.05 * (1.0 - metallic) + c * metallic),
        }
    }

    /// `get_world_lighting()` with specular on and a black ambient colour; `n` and `i` are in view space.
    fn shade(&self, n: V3, i: V3) -> V3 {
        /* `R = -reflect(I, N)`. */
        let n_dot_i = dot(n, i);
        let r = [2.0 * n_dot_i * n[0] - i[0], 2.0 * n_dot_i * n[1] - i[1], 2.0 * n_dot_i * n[2] - i[2]];

        let mut specular_light = [0.0f32; 3];
        let mut diffuse_light = [0.0f32; 3];
        for l in &self.lights {
            let half_dir = normalize([l.direction[0] + i[0], l.direction[1] + i[1], l.direction[2] + i[2]]);
            let wrapped_nl = dot(l.direction, r);
            let spec_angle = saturate(dot(half_dir, n));
            let spec_nl = saturate(dot(l.direction, n));

            /* `0 ^ shininess` is 0 (shininess is positive): skips the `powf` of the unlit pixels. */
            let pow = if spec_angle > 0.0 { spec_angle.powf(l.shininess) } else { 0.0 };
            let spec_light = pow * spec_nl * l.shininess_scale;

            /* Simulate env. light. */
            let spec_env = wrapped_lighting(wrapped_nl, l.w);
            let spec_light = spec_light * (1.0 - l.k) + spec_env * l.k;

            let diff_light = wrapped_lighting(dot(l.direction, n), l.wrap);
            for c in 0..3 {
                specular_light[c] += spec_light * l.specular[c];
                diffuse_light[c] += diff_light * l.diffuse[c];
            }
        }

        /* `brdf_approx()`. */
        let fresnel = (-8.35 * saturate(n_dot_i)).exp2() * (1.0 - self.roughness);
        let specular_color = self.specular_base.map(|c| c * (1.0 - fresnel) + fresnel);
        let spec_energy = dot(specular_color, [0.33333; 3]);

        std::array::from_fn(|c| {
            diffuse_light[c] * self.diffuse_color[c] * (1.0 - spec_energy) + specular_light[c] * specular_color[c]
        })
    }
}

#[cfg(test)]
fn world_lighting(roughness: f32, metallic: f32, n: V3, i: V3) -> V3 {
    Lighting::new(roughness, metallic).shade(n, i)
}

/// `drw_view_incident_vector()` for the pixel ray through `ndc` (the composite pass uses a point at
/// `z = 0.5` of the screen, so only the ray direction matters). The perspective ray is
/// `((ndc_x + winmat[2][0]) / winmat[0][0], (ndc_y + winmat[2][1]) / winmat[1][1], -1)`.
fn incident_vector(rv3d: &RegionView3D, ndc: [f32; 2]) -> V3 {
    if rv3d.ortho {
        return [0.0, 0.0, 1.0];
    }
    let w = &rv3d.winmat;
    normalize([-(ndc[0] + w[2][0]) / w[0][0], -(ndc[1] + w[2][1]) / w[1][1], 1.0])
}

/// Clips a polygon in clip space against the near plane `z >= -w`.
fn clip_near(poly: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (da, db) = (a[2] + a[3], b[2] + b[3]);
        if da >= 0.0 {
            out.push(a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            let t = da / (da - db);
            out.push(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t));
        }
    }
    out
}

/// Clips a world space polygon against the view clipping planes (`RV3D_CLIPPING`): the part with
/// `dot(plane, (co, 1)) >= 0` stays, like the clip distances of `set_clipping_distances()`
/// (workbench/shaders/workbench_prepass.bsl.hh:36-50).
fn clip_world(poly: &[V3], planes: &[[f32; 4]; 4]) -> Vec<V3> {
    let mut poly = poly.to_vec();
    for pl in planes {
        let dist = |v: V3| pl[0] * v[0] + pl[1] * v[1] + pl[2] * v[2] + pl[3];
        let mut out = Vec::with_capacity(poly.len() + 1);
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (da, db) = (dist(a), dist(b));
            if da >= 0.0 {
                out.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                let t = da / (da - db);
                out.push(std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t));
            }
        }
        poly = out;
    }
    poly
}

fn edge(a: &[f32], b: &[f32], c: &[f32]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// `View3DShading::xray_alpha` default (`SHADING_XRAY_ALPHA()` of a solid shading with `V3D_SHADING_XRAY`).
const SHADING_XRAY_ALPHA: f32 = 0.5;

/// The `TransparentPass` targets (workbench_mesh_passes.cc:240-305): `accumulation_tx` (colour sum),
/// `reveal_tx` (weight sum) and the revealage kept in the alpha of `accumulation_tx`, cleared to 1.
struct Xray {
    alpha: f32,
    accum: Vec<V3>,
    weight: Vec<f32>,
    reveal: Vec<f32>,
}

/// `linear_zdepth()` and `calculate_transparent_weight()` (workbench/shaders/workbench_prepass.bsl.hh:54-90):
/// `frag_z` is the window depth in [0, 1].
fn transparent_weight(frag_z: f32, winmat: &[[f32; 4]; 4]) -> f32 {
    let z = if winmat[3][3] == 0.0 {
        let d = 2.0 * frag_z - 1.0;
        -winmat[3][2] / (d + winmat[2][2])
    } else {
        frag_z * (-2.0 / winmat[2][2])
    };
    /* Eq 7 of McGuire and Bavoil, optimised. */
    let a = z.abs() / 5.0;
    let mut b = z.abs() / 200.0;
    b *= b;
    let w = 10.0 / ((1e-5 + a * a) + b * (b * b));
    w.clamp(1e-2, 3e2)
}


struct Target<'a> {
    buf: &'a mut [u32],
    width: usize,
    height: usize,
    region: Rcti,
    rw: i32,
    rh: i32,
    depth: Vec<f32>,
    /// The object id buffer: `Object::id + 1` of the nearest surface of every pixel, 0 where nothing is drawn.
    ids: Vec<u32>,
    /// The id written by the triangles being drawn.
    cur_id: u32,
    /// The sculpt mask at the corners of the triangles being drawn (all zero when the object has no mask).
    cur_mask: [f32; 3],
    lighting: Lighting,
    /// The opaque triangles of the frame, rasterised by `flush()` in horizontal bands on several threads.
    jobs: Vec<Job>,
    /// `Some` in X-ray mode (`SceneState::xray_mode`): every face goes to the transparent pass.
    xray: Option<Xray>,
}

impl Target<'_> {
    /// `TransparentResolvePass`: `accumulation / weight` blended over the buffer with the alpha
    /// `1 - revealage`, in linear space.
    fn resolve_xray(&mut self) {
        let Some(x) = self.xray.as_ref() else { return };
        for py in 0..self.rh {
            for px in 0..self.rw {
                let di = (py * self.rw + px) as usize;
                let weight = x.weight[di];
                if weight <= 0.0 {
                    continue;
                }
                let a = 1.0 - x.reveal[di];
                if a <= 0.0 {
                    continue;
                }
                let (bx, by) = (self.region.xmin + px, self.region.ymin + py);
                if bx < 0 || by < 0 || bx as usize >= self.width || by as usize >= self.height {
                    continue;
                }
                let idx = (self.height - 1 - by as usize) * self.width + bx as usize;
                let d = self.buf[idx];
                let mut out = 0u32;
                for (k, shift) in [16u32, 8, 0].into_iter().enumerate() {
                    let src = x.accum[di][k] / weight.max(1e-4);
                    let dst = srgb8_to_linear((d >> shift) & 0xff);
                    out |= linear_to_srgb8(src * a + dst * (1.0 - a)) << shift;
                }
                self.buf[idx] = out;
            }
        }
    }
    /// Rasterises one triangle (region pixels, depth in [0, 1]) with the depth test `LESS_EQUAL`.
    /// `ns` are the view space normals of its corners: all equal for flat shading, otherwise they are
    /// interpolated across the triangle (smooth shading). A triangle that is clockwise on screen is a back
    /// face and has its normals flipped.
    fn triangle(&mut self, mut t: [[f32; 3]; 3], mut ns: [V3; 3], rv3d: &RegionView3D) {
        let mut area = edge(&t[0], &t[1], &t[2]);
        if area == 0.0 {
            return;
        }
        if area < 0.0 {
            t.swap(1, 2);
            ns.swap(1, 2);
            self.cur_mask.swap(1, 2);
            area = -area;
            /* `frag_transparent()` does not flip the normal of back faces. */
            if self.xray.is_none() {
                ns = ns.map(|n| [-n[0], -n[1], -n[2]]);
            }
        }
        if self.xray.is_none() {
            self.jobs.push(Job { t, ns, area, id: self.cur_id, mask: self.cur_mask });
            return;
        }
        let flat = ns[0] == ns[1] && ns[1] == ns[2];
        /* The edge functions with their per-triangle terms hoisted (the same arithmetic as `edge()`). */
        let (e0x, e0y, o0x, o0y) = (t[2][0] - t[1][0], t[2][1] - t[1][1], t[1][0], t[1][1]);
        let (e1x, e1y, o1x, o1y) = (t[0][0] - t[2][0], t[0][1] - t[2][1], t[2][0], t[2][1]);
        let (e2x, e2y, o2x, o2y) = (t[1][0] - t[0][0], t[1][1] - t[0][1], t[0][0], t[0][1]);
        /* A flat face seen in an orthographic view has one colour for all its pixels. */
        let flat_color = (flat && rv3d.ortho).then(|| self.lighting.shade(ns[0], [0.0, 0.0, 1.0]));

        let min_x = t[0][0].min(t[1][0]).min(t[2][0]);
        let max_x = t[0][0].max(t[1][0]).max(t[2][0]);
        let min_y = t[0][1].min(t[1][1]).min(t[2][1]);
        let max_y = t[0][1].max(t[1][1]).max(t[2][1]);
        let (x0, x1) = ((min_x.floor() as i32).max(0), (max_x.ceil() as i32).min(self.rw - 1));
        let (y0, y1) = ((min_y.floor() as i32).max(0), (max_y.ceil() as i32).min(self.rh - 1));

        for py in y0..=y1 {
            for px in x0..=x1 {
                let p = [px as f32 + 0.5, py as f32 + 0.5];
                let w0 = e0x * (p[1] - o0y) - e0y * (p[0] - o0x);
                let w1 = e1x * (p[1] - o1y) - e1y * (p[0] - o1x);
                let w2 = e2x * (p[1] - o2y) - e2y * (p[0] - o2x);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = (w0 * t[0][2] + w1 * t[1][2] + w2 * t[2][2]) / area;
                if !(0.0..=1.0).contains(&z) {
                    continue;
                }
                let di = (py * self.rw + px) as usize;
                /* The transparent pass is not depth tested against the cube itself. */
                if self.xray.is_none() && z > self.depth[di] {
                    continue;
                }
                /* The object id of the nearest surface (the outline pass reads it). */
                if z <= self.depth[di] {
                    self.ids[di] = self.cur_id;
                }
                /* `TransparentDepthPass` writes the nearest depth of the X-ray faces. */
                self.depth[di] = self.depth[di].min(z);

                let n = if flat {
                    ns[0]
                } else {
                    normalize(std::array::from_fn(|k| (w0 * ns[0][k] + w1 * ns[1][k] + w2 * ns[2][k]) / area))
                };
                let c = match flat_color {
                    Some(c) => c,
                    None => {
                        let ndc = [p[0] / self.rw as f32 * 2.0 - 1.0, p[1] / self.rh as f32 * 2.0 - 1.0];
                        self.lighting.shade(n, incident_vector(rv3d, ndc))
                    }
                };

                /* `frag_transparent()`: `alpha = material alpha * xray_alpha`, weighted accumulation. */
                if let Some(x) = self.xray.as_mut() {
                    let alpha = x.alpha;
                    let weight = transparent_weight(z, &rv3d.winmat) * alpha;
                    for k in 0..3 {
                        x.accum[di][k] += c[k] * weight;
                    }
                    x.weight[di] += weight;
                    x.reveal[di] *= 1.0 - alpha;
                    continue;
                }

                let (x, y) = (self.region.xmin + px, self.region.ymin + py);
                if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
                    continue;
                }
                let idx = (self.height - 1 - y as usize) * self.width + x as usize;
                self.buf[idx] = (linear_to_srgb8(c[0]) << 16) | (linear_to_srgb8(c[1]) << 8) | linear_to_srgb8(c[2]);
            }
        }
    }
}

/// One opaque triangle waiting for `Target::flush()`: screen space corners (counter clockwise), view space normals
/// (already flipped for back faces), twice its area and the object id.
struct Job {
    t: [[f32; 3]; 3],
    ns: [V3; 3],
    area: f32,
    id: u32,
    /// The sculpt mask at the three corners.
    mask: [f32; 3],
}

/// How much of the colour a fully masked point keeps: Blender draws the mask as a dark grey wash.
const MASK_KEEP: f32 = 0.2;

fn mask_shade(c: V3, m: f32) -> V3 {
    let k = 1.0 - m.clamp(0.0, 1.0) * (1.0 - MASK_KEEP);
    [c[0] * k, c[1] * k, c[2] * k]
}

/// A raw pointer to the window buffer that the band threads write through. Every band owns a distinct range of
/// region rows, and a region row maps to exactly one window row, so no two threads write the same element.
#[derive(Clone, Copy)]
struct OutPtr(*mut u32, usize);
unsafe impl Send for OutPtr {}
unsafe impl Sync for OutPtr {}

/// The rows `y0..y1` of the region: their depth and id rows, and the shared window buffer.
struct Band<'a> {
    y0: i32,
    y1: i32,
    depth: &'a mut [f32],
    ids: &'a mut [u32],
}

/// Everything a band needs that is the same for all bands.
struct BandCtx<'a> {
    rv3d: &'a RegionView3D,
    lighting: &'a Lighting,
    out: OutPtr,
    width: usize,
    height: usize,
    region: Rcti,
    rw: i32,
    rh: i32,
}

/// Rasterises `jobs`, in order, into the rows of `band`: the same per pixel work and the same arithmetic as the X-ray
/// less path of `Target::triangle()`, so any band split gives the same image.
fn raster_band(ctx: &BandCtx, band: &mut Band, jobs: &[Job]) {
    let (rw, rh) = (ctx.rw, ctx.rh);
    for job in jobs {
        let (t, ns, area) = (&job.t, &job.ns, job.area);
        let min_y = t[0][1].min(t[1][1]).min(t[2][1]);
        let max_y = t[0][1].max(t[1][1]).max(t[2][1]);
        let y0 = (min_y.floor() as i32).max(band.y0);
        let y1 = (max_y.ceil() as i32).min(rh - 1).min(band.y1 - 1);
        if y0 > y1 {
            continue;
        }
        let min_x = t[0][0].min(t[1][0]).min(t[2][0]);
        let max_x = t[0][0].max(t[1][0]).max(t[2][0]);
        let (x0, x1) = ((min_x.floor() as i32).max(0), (max_x.ceil() as i32).min(rw - 1));
        let flat = ns[0] == ns[1] && ns[1] == ns[2];
        let (e0x, e0y, o0x, o0y) = (t[2][0] - t[1][0], t[2][1] - t[1][1], t[1][0], t[1][1]);
        let (e1x, e1y, o1x, o1y) = (t[0][0] - t[2][0], t[0][1] - t[2][1], t[2][0], t[2][1]);
        let (e2x, e2y, o2x, o2y) = (t[1][0] - t[0][0], t[1][1] - t[0][1], t[0][0], t[0][1]);
        let masked = job.mask.iter().any(|&m| m > 0.0);
        let flat_color = (flat && ctx.rv3d.ortho && !masked).then(|| ctx.lighting.shade(ns[0], [0.0, 0.0, 1.0]));
        for py in y0..=y1 {
            for px in x0..=x1 {
                let p = [px as f32 + 0.5, py as f32 + 0.5];
                let w0 = e0x * (p[1] - o0y) - e0y * (p[0] - o0x);
                let w1 = e1x * (p[1] - o1y) - e1y * (p[0] - o1x);
                let w2 = e2x * (p[1] - o2y) - e2y * (p[0] - o2x);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = (w0 * t[0][2] + w1 * t[1][2] + w2 * t[2][2]) / area;
                if !(0.0..=1.0).contains(&z) {
                    continue;
                }
                let di = ((py - band.y0) * rw + px) as usize;
                if z > band.depth[di] {
                    continue;
                }
                band.ids[di] = job.id;
                band.depth[di] = band.depth[di].min(z);
                let c = match flat_color {
                    Some(c) => c,
                    None => {
                        let n = if flat {
                            ns[0]
                        } else {
                            normalize(std::array::from_fn(|k| (w0 * ns[0][k] + w1 * ns[1][k] + w2 * ns[2][k]) / area))
                        };
                        let ndc = [p[0] / rw as f32 * 2.0 - 1.0, p[1] / rh as f32 * 2.0 - 1.0];
                        ctx.lighting.shade(n, incident_vector(ctx.rv3d, ndc))
                    }
                };
                let c = if masked { mask_shade(c, (w0 * job.mask[0] + w1 * job.mask[1] + w2 * job.mask[2]) / area) } else { c };
                let (x, y) = (ctx.region.xmin + px, ctx.region.ymin + py);
                if x < 0 || y < 0 || x as usize >= ctx.width || y as usize >= ctx.height {
                    continue;
                }
                let idx = (ctx.height - 1 - y as usize) * ctx.width + x as usize;
                debug_assert!(idx < ctx.out.1);
                let v = (linear_to_srgb8(c[0]) << 16) | (linear_to_srgb8(c[1]) << 8) | linear_to_srgb8(c[2]);
                /* SAFETY: `idx < width * height` (checked above) and only the band that owns row `py` writes it. */
                unsafe { *ctx.out.0.add(idx) = v };
            }
        }
    }
}

/// How many threads the bands are spread over.
fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get()).min(8)
}

impl Target<'_> {
    /// Rasterises the deferred opaque triangles: the region is cut into bands of rows (more bands than threads, so
    /// that the threads share an uneven load), each thread takes the next free band and replays every triangle in
    /// submission order into it.
    fn flush(&mut self, rv3d: &RegionView3D, threads: usize) {
        if self.jobs.is_empty() {
            return;
        }
        let jobs = std::mem::take(&mut self.jobs);
        let (rw, rh) = (self.rw, self.rh);
        let threads = if jobs.len() < 64 || rh < 32 { 1 } else { threads.clamp(1, (rh / 16).max(1) as usize) };
        let band_rows = if threads == 1 { rh } else { ((rh + threads as i32 * 4 - 1) / (threads as i32 * 4)).max(8) };
        let out = OutPtr(self.buf.as_mut_ptr(), self.buf.len());
        let ctx = BandCtx { rv3d, lighting: &self.lighting, out, width: self.width, height: self.height, region: self.region, rw, rh };
        let rows = (band_rows * rw) as usize;
        let mut bands: Vec<Band> = self
            .depth
            .chunks_mut(rows)
            .zip(self.ids.chunks_mut(rows))
            .enumerate()
            .map(|(k, (depth, ids))| {
                let y0 = k as i32 * band_rows;
                Band { y0, y1: (y0 + band_rows).min(rh), depth, ids }
            })
            .collect();
        if threads == 1 {
            for band in &mut bands {
                raster_band(&ctx, band, &jobs);
            }
            return;
        }
        bands.reverse();
        let queue = std::sync::Mutex::new(bands);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop());
                    let Some(mut band) = next else { break };
                    raster_band(&ctx, &mut band, &jobs);
                });
            }
        });
    }
}

/// The result of `draw_scene()` for the overlays: the depth buffer (`rw * rh` values in [0, 1], 1.0 where
/// nothing was drawn, row 0 at the bottom of the region) and the object id buffer (`Object::id + 1`, 0 where
/// nothing was drawn, same layout).
pub struct SceneRender {
    pub depth: Vec<f32>,
    pub ids: Vec<u32>,
}

/// World space normal to view space (`viewmat` rotation).
fn normal_to_view(rv3d: &RegionView3D, n: V3) -> V3 {
    let vm = &rv3d.viewmat;
    std::array::from_fn(|i| vm[0][i] * n[0] + vm[1][i] * n[1] + vm[2][i] * n[2])
}

/// Draws one convex or concave world space polygon with a single normal: clipped by the view clipping border and
/// the near plane, then fanned (the flat shading path).
fn draw_flat_polygon(target: &mut Target, rv3d: &RegionView3D, poly_world: &[V3], n_view: V3) {
    let p = &rv3d.persmat;
    let world = match rv3d.clip {
        Some(planes) => clip_world(poly_world, &planes),
        None => poly_world.to_vec(),
    };
    let clip: Vec<[f32; 4]> = world
        .iter()
        .map(|v| std::array::from_fn(|i| p[0][i] * v[0] + p[1][i] * v[1] + p[2][i] * v[2] + p[3][i]))
        .collect();
    let poly = clip_near(&clip);
    if poly.len() < 3 || poly.iter().any(|c| c[3] <= 0.0) {
        return;
    }
    let (rw, rh) = (target.rw as f32, target.rh as f32);
    let screen: Vec<[f32; 3]> = poly
        .iter()
        .map(|c| [((c[0] / c[3]) * 0.5 + 0.5) * rw, ((c[1] / c[3]) * 0.5 + 0.5) * rh, (c[2] / c[3]) * 0.5 + 0.5])
        .collect();
    for k in 1..screen.len() - 1 {
        target.triangle([screen[0], screen[k], screen[k + 1]], [n_view; 3], rv3d);
    }
}

/// Draws one object's evaluated mesh. Flat shaded faces use their own normal; with `smooth` every triangle fully
/// inside the view uses the interpolated vertex normals, the rest (cut by the near plane or the clipping border)
/// fall back to the face normal.
fn draw_mesh(target: &mut Target, rv3d: &RegionView3D, m: &Mat4, mesh: &Mesh, smooth: bool, mask: Option<&[f32]>) {
    let nm = normal_matrix(m);
    let to_view = |n_local: V3| normal_to_view(rv3d, normalize(mat3_mul_v3(&nm, n_local)));
    let world: Vec<V3> = mesh.verts.iter().map(|&v| mat4_mul_point(m, v)).collect();
    let vnormals = smooth.then(|| mesh.vertex_normals());
    let p = &rv3d.persmat;
    let (rw, rh) = (target.rw as f32, target.rh as f32);

    for (f, face) in mesh.faces.iter().enumerate() {
        let face_n = to_view(mesh.face_normal(f));
        let Some(vn) = vnormals.as_ref() else {
            let poly: Vec<V3> = face.iter().map(|&i| world[i as usize]).collect();
            /* A flat face has one mask: the mean of its corners. */
            let mean = mask.map_or(0.0, |mk| face.iter().map(|&i| mk[i as usize]).sum::<f32>() / face.len() as f32);
            target.cur_mask = [mean; 3];
            draw_flat_polygon(target, rv3d, &poly, face_n);
            target.cur_mask = [0.0; 3];
            continue;
        };
        for k in 1..face.len().saturating_sub(1) {
            let tri = [face[0], face[k], face[k + 1]];
            target.cur_mask = mask.map_or([0.0; 3], |mk| tri.map(|i| mk[i as usize]));
            let clip: [[f32; 4]; 3] = tri.map(|i| {
                let v = world[i as usize];
                std::array::from_fn(|r| p[0][r] * v[0] + p[1][r] * v[1] + p[2][r] * v[2] + p[3][r])
            });
            let inside = rv3d.clip.is_none() && clip.iter().all(|c| c[3] > 0.0 && c[2] + c[3] >= 0.0);
            if inside {
                let screen = clip.map(|c| [((c[0] / c[3]) * 0.5 + 0.5) * rw, ((c[1] / c[3]) * 0.5 + 0.5) * rh, (c[2] / c[3]) * 0.5 + 0.5]);
                let ns = tri.map(|i| to_view(vn[i as usize]));
                target.triangle(screen, ns, rv3d);
            } else {
                let mean = target.cur_mask.iter().sum::<f32>() / 3.0;
                target.cur_mask = [mean; 3];
                draw_flat_polygon(target, rv3d, &tri.map(|i| world[i as usize]), face_n);
            }
        }
    }
    target.cur_mask = [0.0; 3];
}

/// Draws the visible mesh objects of `scene` into `region` (inclusive Blender rect, y up) of the `0x00RRGGBB`
/// buffer, and returns the region's depth and object id buffers for the overlays. `rv3d` must have been updated
/// for the size of the region.
pub fn draw_scene(buf: &mut [u32], width: usize, height: usize, region: Rcti, rv3d: &RegionView3D, xray: bool, scene: &Scene) -> SceneRender {
    draw_scene_threads(buf, width, height, region, rv3d, xray, scene, default_threads())
}

/// `draw_scene()` with an explicit thread count (the image does not depend on it).
#[allow(clippy::too_many_arguments)]
pub fn draw_scene_threads(buf: &mut [u32], width: usize, height: usize, region: Rcti, rv3d: &RegionView3D, xray: bool, scene: &Scene, threads: usize) -> SceneRender {
    let (rw, rh) = (region.size_x() + 1, region.size_y() + 1);
    if rw <= 0 || rh <= 0 {
        return SceneRender { depth: Vec::new(), ids: Vec::new() };
    }
    let (roughness, metallic) = material_roughness_metallic();
    let n = (rw * rh) as usize;
    let mut target = Target {
        buf,
        width,
        height,
        region,
        rw,
        rh,
        depth: vec![1.0; n],
        ids: vec![0; n],
        cur_id: 0,
        cur_mask: [0.0; 3],
        xray: xray.then(|| Xray {
            alpha: SHADING_XRAY_ALPHA,
            accum: vec![[0.0; 3]; n],
            weight: vec![0.0; n],
            reveal: vec![1.0; n],
        }),
        lighting: Lighting::new(roughness, metallic),
        jobs: Vec::new(),
    };

    for o in scene.objects.iter().filter(|o| !o.hidden) {
        let Some(mesh) = o.evaluated() else { continue };
        target.cur_id = o.id + 1;
        let mask = (o.mask.len() == mesh.verts.len() && o.mask.iter().any(|&m| m > 0.0)).then_some(o.mask.as_slice());
        draw_mesh(&mut target, rv3d, &o.world_matrix(), mesh, o.smooth, mask);
    }
    target.flush(rv3d, threads);
    target.resolve_xray();
    SceneRender { depth: target.depth, ids: target.ids }
}

/// The startup scene's cube only (the original entry point of the tests).
#[cfg(test)]
pub fn draw_cube(buf: &mut [u32], width: usize, height: usize, region: Rcti, rv3d: &RegionView3D, xray: bool) -> Vec<f32> {
    draw_scene(buf, width, height, region, rv3d, xray, &Scene::startup()).depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Primitive;
    use crate::view3d::View3D;

    fn render_scene(scene: &Scene) -> (Vec<u32>, SceneRender, usize, usize) {
        let (w, h) = (200usize, 100usize);
        let mut buf = vec![crate::theme::SPACE_VIEW3D_BACK.to_xrgb(); w * h];
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let r = draw_scene(&mut buf, w, h, region, &rv3d, false, scene);
        (buf, r, w, h)
    }

    /* The id buffer holds `Object::id + 1` at the centre, 0 in a corner, and nothing for hidden objects. */
    #[test]
    fn the_id_buffer_names_the_nearest_object() {
        let mut scene = Scene::startup();
        let cube = scene.objects[1].id;
        let (_, r, w, _) = render_scene(&scene);
        assert_eq!(r.ids[50 * w + 100], cube + 1);
        assert_eq!(r.ids[0], 0);
        scene.hide_selected();
        let (_, r, _, _) = render_scene(&scene);
        assert!(r.ids.iter().all(|&i| i == 0) && r.depth.iter().all(|&d| d == 1.0));
    }

    /* A masked sphere is drawn darker than the same sphere without a mask, smooth and flat shaded, and the
     * threaded rasteriser agrees with the single thread one. */
    #[test]
    fn the_mask_darkens_the_surface() {
        let lum = |buf: &[u32], i: usize| ((buf[i] >> 16) & 0xff) + ((buf[i] >> 8) & 0xff) + (buf[i] & 0xff);
        for smooth in [false, true] {
            let mut scene = Scene::startup();
            scene.hide_selected();
            let id = scene.add_primitive(Primitive::UvSphere, [0.0, 0.0, 0.0]);
            scene.active = Some(id);
            if smooth {
                scene.set_smooth(true);
            }
            let (plain, r0, w, _) = render_scene(&scene);
            let i = 50 * w + 100;
            assert_eq!(r0.ids[i], id + 1);
            scene.invert_mask_active();
            let (masked, _, _, _) = render_scene(&scene);
            assert!(lum(&masked, i) * 2 < lum(&plain, i), "smooth {smooth}: masked {} plain {}", lum(&masked, i), lum(&plain, i));
            assert_eq!(masked[0], plain[0], "the background is untouched");
        }
    }

    #[test]
    fn a_sphere_in_front_hides_the_cube() {
        let mut scene = Scene::startup();
        let rv3d = {
            let mut r = RegionView3D::default();
            r.update(&View3D::default(), 200, 100);
            r
        };
        let eye = rv3d.viewinv[2];
        let sphere = scene.add_primitive(Primitive::UvSphere, [eye[0] * 3.0, eye[1] * 3.0, eye[2] * 3.0]);
        let (_, r, w, _) = render_scene(&scene);
        assert_eq!(r.ids[50 * w + 100], sphere + 1);
    }

    /* Smooth shading interpolates the vertex normals, so the shade changes across a sphere's flat facets: with
     * flat shading a facet is one colour, smooth shading gives more distinct colours over the same area. */
    #[test]
    fn smooth_shading_has_a_gradient_flat_does_not() {
        let mut scene = Scene::startup();
        /* The sphere would sit inside the startup cube, which is selected: delete it first. */
        scene.delete_selected();
        let id = scene.add_primitive(Primitive::IcoSphere, [0.0; 3]);
        let distinct = |scene: &Scene| {
            let (buf, r, w, _) = render_scene(scene);
            let mut colours = std::collections::HashSet::new();
            for (i, &obj) in r.ids.iter().enumerate() {
                if obj == id + 1 {
                    colours.insert(buf[i / w * w + i % w]);
                }
            }
            colours.len()
        };
        let flat = distinct(&scene);
        scene.set_smooth(true);
        let smooth = distinct(&scene);
        assert!(smooth > flat * 2, "flat {flat}, smooth {smooth}");
    }

    /* Default view, 200x100 region: the +Z face of the cube covers the centre, so the pixel there is
     * lit (not the background) and its depth is in front of the far plane. */
    #[test]
    fn cube_covers_the_view_centre() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut buf = vec![bg; w * h];
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let depth = draw_cube(&mut buf, w, h, region, &rv3d, false);

        assert_eq!(depth.len(), w * h);
        assert!(depth[50 * w + 100] < 1.0, "{}", depth[50 * w + 100]);
        assert_ne!(buf[(h - 1 - 50) * w + 100], bg);
        assert_eq!(depth[0], 1.0);
        assert_eq!(buf[(h - 1) * w], bg);
    }

    /* The quantised material: roughness sqrt(0.4) -> 161 / 255 -> 19 / 31, metallic 0. */
    #[test]
    fn material_quantisation() {
        let (r, m) = material_roughness_metallic();
        assert!((r - 19.0 / 31.0).abs() < 1e-6, "{r}");
        assert_eq!(m, 0.0);
    }

    /* X-ray: the cube is blended with the background instead of replacing it, and the depth buffer
     * still holds the cube. */
    #[test]
    fn xray_blends_the_cube() {
        let (w, h) = (200usize, 100usize);
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };

        let mut opaque = vec![bg; w * h];
        let depth_opaque = draw_cube(&mut opaque, w, h, region, &rv3d, false);
        let mut xray = vec![bg; w * h];
        let depth_xray = draw_cube(&mut xray, w, h, region, &rv3d, true);

        let centre = (h - 1 - 50) * w + 100;
        assert_ne!(xray[centre], bg);
        assert_ne!(xray[centre], opaque[centre]);
        assert_eq!(depth_xray, depth_opaque);
        assert_eq!(xray[(h - 1) * w], bg);
    }

    /// `cargo test --release bench_draw_scene -- --ignored --nocapture`: milliseconds per frame of a smooth sphere
    /// (about 4000 triangles) and a flat cube at 1280 x 720.
    #[test]
    #[ignore]
    fn bench_draw_scene() {
        let mut scene = Scene::startup();
        let id = scene.add_primitive(crate::mesh::Primitive::UvSphere, [0.0, 0.0, 0.0]);
        for o in scene.objects.iter_mut().filter(|o| o.id == id) {
            o.smooth = true;
            o.scale = [2.0; 3];
        }
        let (w, h) = (1280usize, 720usize);
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let mut buf = vec![0u32; w * h];
        let t = std::time::Instant::now();
        for _ in 0..30 {
            draw_scene(&mut buf, w, h, region, &rv3d, false, &scene);
        }
        println!("draw_scene: {:.2} ms/frame", t.elapsed().as_secs_f64() * 1000.0 / 30.0);
    }

    /// The band split and the thread count do not change the image, the depth or the id buffer.
    #[test]
    fn threaded_rendering_matches_single_threaded() {
        let mut scene = Scene::startup();
        let id = scene.add_primitive(crate::mesh::Primitive::UvSphere, [0.5, 0.0, 0.3]);
        for o in scene.objects.iter_mut().filter(|o| o.id == id) {
            o.smooth = true;
            o.scale = [1.5; 3];
        }
        let (w, h) = (640usize, 480usize);
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w as i32, h as i32);
        let region = Rcti { xmin: 0, xmax: w as i32 - 1, ymin: 0, ymax: h as i32 - 1 };
        let bg = crate::theme::SPACE_VIEW3D_BACK.to_xrgb();
        let run = |threads: usize| {
            let mut buf = vec![bg; w * h];
            let r = draw_scene_threads(&mut buf, w, h, region, &rv3d, false, &scene, threads);
            (buf, r.depth, r.ids)
        };
        let one = run(1);
        assert!(one.0.iter().any(|&p| p != bg), "something is drawn");
        for threads in [2, 4, 7] {
            assert!(run(threads) == one, "{threads} threads");
        }
    }
}
