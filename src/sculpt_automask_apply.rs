//! Applies the automasking factors to a sculpt dab (`mesh/sculpt_automasking.cc`: `factor_get` of the topology, boundary
//! edge, boundary face set and boundary fade branches, via `sculpt_automasking.rs`).
//! A dab moves vertices; each vertex then keeps `factor` of its displacement (`translation * factor`, which is what
//! multiplying the brush factor does for every translating brush). With every flag off the factors are all 1 and the
//! dab is untouched.
//! Deviations: the face set layer is not stored on the mesh yet (empty slice: those branches are no-ops); the topology
//! fill starts at the vertex nearest to the dab center only (no symmetric start vertices).

use crate::mesh::{Mesh, V3};
use crate::sculpt_automask_settings::AutomaskSettings;
use crate::sculpt_automasking::{self as am, BoundaryMode, Flags};
use crate::sculpt_boundary::Topo;

/// The owned topology data of a stroke (`Topo` borrows the mesh, this does not).
#[derive(Clone, Debug, Default)]
pub struct Cache {
    vert_to_face: Vec<Vec<usize>>,
    boundary: Vec<bool>,
}

impl Cache {
    pub fn new(mesh: &Mesh) -> Cache {
        let t = Topo::new(mesh);
        Cache { vert_to_face: t.vert_to_face, boundary: t.boundary_verts }
    }
}

/// Whether any flag handled here is on.
pub fn is_active(s: &AutomaskSettings) -> bool {
    s.topology || s.face_sets || s.boundary_edges || s.boundary_face_sets
}

/// The automasking factor of every vertex for a dab at `center` with `radius`.
pub fn factors(mesh: &Mesh, cache: &mut Cache, face_sets: &[i32], s: &AutomaskSettings, center: V3, radius: f32) -> Vec<f32> {
    let n = mesh.verts.len();
    if cache.boundary.len() != n {
        *cache = Cache::new(mesh);
    }
    let topo = Topo {
        mesh,
        vert_to_face: std::mem::take(&mut cache.vert_to_face),
        hide_poly: vec![false; mesh.faces.len()],
        hide_vert: vec![false; n],
        boundary_verts: std::mem::take(&mut cache.boundary),
    };
    let mut out = vec![1.0; n];
    if s.topology {
        out = vec![0.0; n];
        let d2 = |i: usize| {
            let v = mesh.verts[i];
            (v[0] - center[0]).powi(2) + (v[1] - center[1]).powi(2) + (v[2] - center[2]).powi(2)
        };
        if let Some(start) = (0..n).min_by(|&a, &b| d2(a).total_cmp(&d2(b))) {
            am::constrained_topology(&topo, &[start], |i| d2(i) < radius * radius, &mut out);
        }
    }
    let flags = Flags { face_sets: s.face_sets, boundary_edges: s.boundary_edges, boundary_face_sets: s.boundary_face_sets };
    for (v, f) in out.iter_mut().enumerate() {
        am::apply_vert_factor(&topo, face_sets, flags, crate::face_set::NONE_ID, false, v, f);
    }
    let steps = s.boundary_edges_propagation_steps();
    if s.boundary_edges {
        am::init_boundary_masking(&topo, face_sets, BoundaryMode::Edges, steps, &mut out);
    }
    if s.boundary_face_sets {
        am::init_boundary_masking(&topo, face_sets, BoundaryMode::FaceSets, steps, &mut out);
    }
    cache.vert_to_face = topo.vert_to_face;
    cache.boundary = topo.boundary_verts;
    out
}

/// `after = before + (after - before) * factor` for every vertex.
pub fn blend(before: &[V3], after: &mut [V3], factors: &[f32]) {
    for ((b, a), &f) in before.iter().zip(after.iter_mut()).zip(factors) {
        if f < 1.0 {
            for k in 0..3 {
                a[k] = b[k] + (a[k] - b[k]) * f;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4x4 vertex grid (open boundary around it).
    fn grid() -> Mesh {
        let mut verts = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                verts.push([x as f32, y as f32, 0.0]);
            }
        }
        let mut faces = Vec::new();
        for y in 0..3u32 {
            for x in 0..3u32 {
                let i = y * 4 + x;
                faces.push(vec![i, i + 1, i + 5, i + 4]);
            }
        }
        Mesh { verts, faces }
    }

    fn lift(mesh: &Mesh) -> Vec<V3> {
        mesh.verts.iter().map(|v| [v[0], v[1], v[2] + 1.0]).collect()
    }

    #[test]
    fn flags_off_leave_the_dab_untouched() {
        let m = grid();
        let s = AutomaskSettings::default();
        assert!(!is_active(&s));
        let f = factors(&m, &mut Cache::default(), &[], &s, [1.5, 1.5, 0.0], 10.0);
        assert!(f.iter().all(|&x| x == 1.0));
        let mut after = lift(&m);
        let want = after.clone();
        blend(&m.verts, &mut after, &f);
        assert_eq!(after, want);
    }

    #[test]
    fn mesh_boundary_leaves_boundary_vertices_unmoved() {
        let m = grid();
        let mut s = AutomaskSettings::default();
        s.boundary_edges = true;
        let f = factors(&m, &mut Cache::default(), &[], &s, [1.5, 1.5, 0.0], 10.0);
        let mut after = lift(&m);
        blend(&m.verts, &mut after, &f);
        for (i, v) in after.iter().enumerate() {
            let boundary = [0, 1, 2, 3, 4, 7, 8, 11, 12, 13, 14, 15].contains(&i);
            assert_eq!(v[2] == 0.0, boundary, "vertex {i}");
        }
    }

    #[test]
    fn topology_only_reaches_what_is_connected_within_the_radius() {
        let m = grid();
        let mut s = AutomaskSettings::default();
        s.topology = true;
        let f = factors(&m, &mut Cache::default(), &[], &s, [0.0, 0.0, 0.0], 1.2);
        assert_eq!(f[0], 1.0);
        assert_eq!(f[1], 1.0);
        assert_eq!(f[15], 0.0, "far corner is outside the radius");
    }

    #[test]
    fn cache_survives_between_dabs() {
        let m = grid();
        let mut s = AutomaskSettings::default();
        s.boundary_edges = true;
        let mut c = Cache::default();
        let a = factors(&m, &mut c, &[], &s, [1.5, 1.5, 0.0], 10.0);
        let b = factors(&m, &mut c, &[], &s, [1.5, 1.5, 0.0], 10.0);
        assert_eq!(a, b);
    }
}
