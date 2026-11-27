//! Elastic Deform brush, Grab type (`mesh/brushes/elastic_deform.cc`: `calc_translations`, `calc_faces`,
//! `do_elastic_deform_brush`). Every vertex within the brush radius of the stroke start moves by the Kelvinlet
//! grab of the mouse delta, `BKE_kelvinlet_grab(...) * bstrength * 20`, from its original position.
//! Deviation: the Twist / Scale / Biscale / Triscale types and the PBVH node bounds (Blender also moves the whole
//! nodes that overlap the sphere) are not ported; only vertices inside the radius are held.

use crate::kelvinlet::KelvinletParams;
use crate::mesh::{add, scale, Mesh, V3};
use crate::sculpt::{mask_at, mirror_signs, mirrored};

/// `elastic_deform_volume_preservation` of the sculpt brushes (versioning_280.cc:5481).
pub const VOLUME_PRESERVATION: f32 = 0.5;

/// `calc_translations()` for `BRUSH_ELASTIC_DEFORM_GRAB`: the displacement of the vertex at `co`.
pub fn translation(radius: f32, strength: f32, co: V3, location: V3, delta: V3) -> V3 {
    /* `force` only matters for the scale and twist types; shear modulus is 1. */
    let params = KelvinletParams::new(radius, 0.0, 1.0, VOLUME_PRESERVATION);
    scale(params.grab(co, location, delta), strength * 20.0)
}

/// One pass per mirror: the sign, the centre and the (vertex, mask factor) pairs it holds.
type Pass = (V3, V3, Vec<(usize, f32)>);

/// The vertices a stroke holds: fixed when it starts, moved from their original positions by the mouse delta.
#[derive(Clone, Debug)]
pub struct Elastic {
    passes: Vec<Pass>,
    orig: Vec<V3>,
    radius: f32,
    pub start_px: [f32; 2],
    pub depth: f32,
}

impl Elastic {
    pub fn begin(mesh: &Mesh, mask: &[f32], center: V3, radius: f32, symmetry: [bool; 3], start_px: [f32; 2], depth: f32) -> Elastic {
        let mut taken = vec![false; mesh.verts.len()];
        let mut passes = Vec::new();
        for s in mirror_signs(symmetry) {
            let c = mirrored(center, s);
            let mut set = Vec::new();
            for (i, v) in mesh.verts.iter().enumerate() {
                let d = [v[0] - c[0], v[1] - c[1], v[2] - c[2]];
                if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < radius * radius && !taken[i] {
                    let f = (1.0 - mask_at(mask, i)).clamp(0.0, 1.0);
                    if f > 0.0 {
                        taken[i] = true;
                        set.push((i, f));
                    }
                }
            }
            passes.push((s, c, set));
        }
        Elastic { passes, orig: mesh.verts.clone(), radius, start_px, depth }
    }

    /// `delta` is the mouse movement since the stroke started, in the local space of the mesh.
    pub fn apply(&self, mesh: &mut Mesh, strength: f32, delta: V3) {
        for (s, c, set) in &self.passes {
            let d = mirrored(delta, *s);
            for &(i, f) in set {
                let t = translation(self.radius, strength, self.orig[i], *c, d);
                mesh.verts[i] = add(self.orig[i], scale(t, f));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize) -> Mesh {
        let mut m = Mesh::default();
        m.verts = (0..n).map(|i| [i as f32 * 0.1, 0.0, 0.0]).collect();
        m
    }

    #[test]
    fn centre_vertex_moves_along_delta_and_decays_with_distance() {
        let mut m = line(10);
        let e = Elastic::begin(&m, &[], [0.0; 3], 1.0, [false; 3], [0.0; 2], 0.0);
        e.apply(&mut m, 0.5, [0.0, 0.0, 0.2]);
        let dz: Vec<f32> = m.verts.iter().map(|v| v[2]).collect();
        assert!(dz[0] > 0.0 && m.verts[0][0] == 0.0 && m.verts[0][1] == 0.0);
        assert!(dz.windows(2).all(|w| w[1] < w[0] && w[1] > 0.0));
        /* strength * 20 * grab kernel at the centre (radius 1, shear 1, nu 0.5), delta 0.2. */
        let k = KelvinletParams::new(1.0, 0.0, 1.0, 0.5).grab([0.0; 3], [0.0; 3], [0.0, 0.0, 0.2])[2];
        assert!((dz[0] - k * 10.0).abs() < 1e-6);
    }

    #[test]
    fn masked_vertices_stay_and_outside_radius_is_not_held() {
        let mut m = line(20);
        let mut mask = vec![0.0; 20];
        mask[1] = 1.0;
        let e = Elastic::begin(&m, &mask, [0.0; 3], 1.0, [false; 3], [0.0; 2], 0.0);
        e.apply(&mut m, 0.5, [0.0, 0.0, 0.2]);
        assert_eq!(m.verts[1][2], 0.0);
        assert!(m.verts[2][2] > 0.0);
        assert_eq!(m.verts[19][2], 0.0); /* x = 1.9 > radius */
    }
}
