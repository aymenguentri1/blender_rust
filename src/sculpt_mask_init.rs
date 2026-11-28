//! `sculpt.mask_init` (`editors/sculpt_paint/mesh/sculpt_mask_init.cc`, `sculpt_mask_init_exec`): creates a new mask
//! for the whole mesh. Random per Vertex writes `BLI_hash_int_01(vert + seed)`, Random per Loose Part writes
//! `BLI_hash_int_01(island + seed)`; hidden vertices (`.hide_vert`) are skipped, as `write_mask_mesh` only visits
//! the visible ones. Random per Face Set needs the face set layer, which objects do not store yet.

use crate::face_set::hash_int_01;
use crate::mesh::Mesh;

/// `InitMode` (the face set mode is not available).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitMode {
    RandomPerVertex,
    RandomPerLoosePart,
}

/// The connected component of every vertex (`islands::vert_id_get`): ids count up in order of the lowest vertex.
pub fn loose_parts(mesh: &Mesh) -> Vec<u32> {
    let n = mesh.verts.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for f in &mesh.faces {
        for w in f.windows(2) {
            let (a, b) = (find(&mut parent, w[0] as usize), find(&mut parent, w[1] as usize));
            if a != b {
                parent[a.max(b)] = a.min(b);
            }
        }
    }
    let mut ids = vec![u32::MAX; n];
    let mut next = 0;
    (0..n)
        .map(|i| {
            let r = find(&mut parent, i);
            if ids[r] == u32::MAX {
                ids[r] = next;
                next += 1;
            }
            ids[r]
        })
        .collect()
}

/// The mask after `sculpt.mask_init`: `old` keeps its value at hidden vertices.
pub fn mask_init(mesh: &Mesh, old: &[f32], hide_vert: Option<&[bool]>, mode: InitMode, seed: u32) -> Vec<f32> {
    let parts = (mode == InitMode::RandomPerLoosePart).then(|| loose_parts(mesh));
    (0..mesh.verts.len())
        .map(|v| {
            if hide_vert.is_some_and(|h| h.get(v).copied().unwrap_or(false)) {
                return old.get(v).copied().unwrap_or(0.0);
            }
            let key = match &parts {
                Some(p) => p[v],
                None => v as u32,
            };
            hash_int_01(key.wrapping_add(seed))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_quads() -> Mesh {
        let verts = (0..8).map(|i| [i as f32, 0.0, 0.0]).collect();
        Mesh { verts, faces: vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7]] }
    }

    #[test]
    fn per_vertex_uses_the_hash_of_index_plus_seed() {
        let m = two_quads();
        let out = mask_init(&m, &[], None, InitMode::RandomPerVertex, 7);
        for (v, &x) in out.iter().enumerate() {
            assert_eq!(x, hash_int_01(v as u32 + 7));
            assert!((0.0..=1.0).contains(&x));
        }
    }

    #[test]
    fn per_loose_part_is_constant_on_a_part() {
        let m = two_quads();
        assert_eq!(loose_parts(&m), [0, 0, 0, 0, 1, 1, 1, 1]);
        let out = mask_init(&m, &[], None, InitMode::RandomPerLoosePart, 3);
        assert!(out[..4].iter().all(|&x| x == hash_int_01(3)));
        assert!(out[4..].iter().all(|&x| x == hash_int_01(4)));
    }

    #[test]
    fn hidden_vertices_keep_their_mask() {
        let m = two_quads();
        let hide = [true, false, false, false, false, false, false, false];
        let old = vec![0.5; 8];
        let out = mask_init(&m, &old, Some(&hide), InitMode::RandomPerVertex, 1);
        assert_eq!(out[0], 0.5);
        assert_eq!(out[1], hash_int_01(2));
    }
}
