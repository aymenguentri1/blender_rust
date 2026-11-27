//! Vertex normal cache for the workbench: like `Mesh::batch_cache` in Blender
//! (`draw_cache_impl_mesh.cc`), the derived data is kept between redraws and only rebuilt when the mesh
//! changed. `Mesh` has no dirty tag here, so the mesh content is fingerprinted instead.

use crate::mesh::Mesh;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

type V3 = [f32; 3];

/// FNV-1a style fingerprint of the vertex positions and face topology.
pub fn fingerprint(mesh: &Mesh) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut put = |x: u32| {
        h ^= x as u64;
        h = h.wrapping_mul(0x100000001b3);
    };
    put(mesh.verts.len() as u32);
    for v in &mesh.verts {
        for c in v {
            put(c.to_bits());
        }
    }
    for f in &mesh.faces {
        put(f.len() as u32 | 0x8000_0000);
        for &i in f {
            put(i);
        }
    }
    h
}

#[derive(Default)]
pub struct NormalCache {
    entries: HashMap<u32, (u64, Arc<Vec<V3>>)>,
}

impl NormalCache {
    /// The vertex normals of `mesh` (object `id`), recomputed only when the mesh changed since the last call.
    pub fn get(&mut self, id: u32, mesh: &Mesh) -> Arc<Vec<V3>> {
        let fp = fingerprint(mesh);
        if let Some((old, n)) = self.entries.get(&id) {
            if *old == fp {
                return n.clone();
            }
        }
        let n = Arc::new(mesh.vertex_normals());
        self.entries.insert(id, (fp, n.clone()));
        n
    }

    /// Forgets the objects that are not in `live`.
    pub fn retain(&mut self, live: &[u32]) {
        self.entries.retain(|k, _| live.contains(k));
    }
}

/// The process wide cache used by `workbench::draw_mesh`.
pub fn shared() -> &'static Mutex<NormalCache> {
    static C: OnceLock<Mutex<NormalCache>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(NormalCache::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_until_the_mesh_changes() {
        let mut c = NormalCache::default();
        let mut m = Mesh::plane(2.0);
        let a = c.get(1, &m);
        let b = c.get(1, &m);
        assert!(Arc::ptr_eq(&a, &b));
        m.verts[0][2] += 1.0;
        let d = c.get(1, &m);
        assert!(!Arc::ptr_eq(&a, &d));
        assert_eq!(*d, m.vertex_normals());
    }

    #[test]
    fn topology_change_invalidates_and_retain_drops() {
        let mut c = NormalCache::default();
        let mut m = Mesh::plane(2.0);
        let a = c.get(7, &m);
        m.faces[0].reverse();
        assert!(!Arc::ptr_eq(&a, &c.get(7, &m)));
        c.retain(&[]);
        assert!(c.entries.is_empty());
    }
}
