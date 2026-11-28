//! `sculpt.mesh_filter` (`editors/sculpt_paint/mesh/sculpt_filter_mesh.cc`), the one-shot filters: Inflate, Scale,
//! Sphere and Random (`calc_inflate_filter`, `calc_scale_filter`, `calc_sphere_filter`, `calc_random_filter`).
//! The factor of a vertex is `(1 - mask) * strength` (0 when hidden); the translation is added to the original
//! position (the modal operator restores the stroke start before every step for these filters). Smooth, Relax,
//! Sharpen and the other continuous filters are not ported here.
//! `sculpt_mesh_update_strength`: `strength = start_strength * (start_x - x) * -0.001 * UI_SCALE_FAC`.

use crate::face_set::hash_int_2d;
use crate::mesh::{normalized, scale, Mesh, V3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    Scale,
    Inflate,
    Sphere,
    Random,
}

/// The strength of the filter for a mouse x (`sculpt_mesh_update_strength`).
pub fn strength_from_mouse(start_strength: f32, start_x: f32, x: f32, ui_scale: f32) -> f32 {
    start_strength * -(start_x - x) * 0.001 * ui_scale
}

/// `randomize_factors`: the hash of the position bits and the seed, as `-0.5..0.5`.
fn random_factor(p: V3, seed: u32) -> f32 {
    let h = hash_int_2d(p[0].to_bits(), p[1].to_bits()) ^ hash_int_2d(p[2].to_bits(), seed);
    h as f32 * (1.0 / 0xFFFF_FFFFu32 as f32) - 0.5
}

/// The filtered vertex positions: `orig` moved by the filter at `strength`. `mask` (empty = none) and `hide_vert`
/// scale the factor down.
pub fn apply(mesh: &Mesh, orig: &[V3], mask: &[f32], hide_vert: Option<&[bool]>, kind: FilterType, strength: f32, seed: u32) -> Vec<V3> {
    let normals = if kind == FilterType::Inflate || kind == FilterType::Random {
        let m = Mesh { verts: orig.to_vec(), faces: mesh.faces.clone() };
        m.vertex_normals()
    } else {
        Vec::new()
    };
    orig.iter()
        .enumerate()
        .map(|(i, &p)| {
            let hidden = hide_vert.is_some_and(|h| h.get(i).copied().unwrap_or(false));
            let mut f = if hidden { 0.0 } else { 1.0 - mask.get(i).copied().unwrap_or(0.0) };
            f *= strength;
            let t = match kind {
                FilterType::Inflate => scale(normals[i], f),
                FilterType::Scale => scale(p, f),
                FilterType::Sphere => {
                    let n = normalized(p);
                    scale([(n[0] - p[0]) * 0.5, (n[1] - p[1]) * 0.5, (n[2] - p[2]) * 0.5], f.abs())
                }
                FilterType::Random => scale(normals[i], f * random_factor(p, seed)),
            };
            [p[0] + t[0], p[1] + t[1], p[2] + t[2]]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Primitive;

    fn cube() -> Mesh {
        Primitive::Cube.build()
    }

    #[test]
    fn strength_follows_the_mouse() {
        assert!((strength_from_mouse(1.0, 100.0, 300.0, 1.0) - 0.2).abs() < 1e-6);
        assert!((strength_from_mouse(1.0, 100.0, 0.0, 2.0) + 0.2).abs() < 1e-6);
    }

    #[test]
    fn scale_multiplies_positions_and_respects_mask() {
        let m = cube();
        let mut mask = vec![0.0; m.verts.len()];
        mask[0] = 1.0;
        let out = apply(&m, &m.verts, &mask, None, FilterType::Scale, 0.5, 0);
        assert_eq!(out[0], m.verts[0], "masked vertex stays");
        for i in 1..m.verts.len() {
            for a in 0..3 {
                assert!((out[i][a] - m.verts[i][a] * 1.5).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn inflate_moves_along_normals_and_hidden_stay() {
        let m = cube();
        let n = m.vertex_normals();
        let mut hide = vec![false; m.verts.len()];
        hide[1] = true;
        let out = apply(&m, &m.verts, &[], Some(&hide), FilterType::Inflate, 0.25, 0);
        assert_eq!(out[1], m.verts[1]);
        for a in 0..3 {
            assert!((out[0][a] - (m.verts[0][a] + n[0][a] * 0.25)).abs() < 1e-5);
        }
    }

    #[test]
    fn sphere_pulls_towards_the_unit_sphere() {
        let m = cube();
        let out = apply(&m, &m.verts, &[], None, FilterType::Sphere, 1.0, 0);
        let len = |p: V3| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        for (a, b) in m.verts.iter().zip(&out) {
            assert!((len(*b) - 1.0).abs() < (len(*a) - 1.0).abs() + 1e-6);
        }
    }

    #[test]
    fn random_is_deterministic_per_seed() {
        let m = cube();
        let a = apply(&m, &m.verts, &[], None, FilterType::Random, 1.0, 5);
        assert_eq!(a, apply(&m, &m.verts, &[], None, FilterType::Random, 1.0, 5));
        assert_ne!(a, apply(&m, &m.verts, &[], None, FilterType::Random, 1.0, 6));
        assert_ne!(a, m.verts);
    }
}
