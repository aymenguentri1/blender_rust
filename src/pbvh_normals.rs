//! Partial normals update per PBVH node (source/blender/blenkernel/intern/pbvh.cc: `calc_node_face_normals`,
//! `normals_calc_verts_simple`, `update_normals_mesh`).
//!
//! Faces of the tagged leaves are exactly the faces with changed normals; every vertex of those faces changes
//! (Blender's owned + shared vertices of the tagged nodes), so those vertex normals are rebuilt from the cached
//! face normals.

use crate::mesh::Mesh;
use crate::pbvh_partition::V3;
use crate::pbvh_query::Pbvh;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NormalsCache {
    pub face: Vec<V3>,
    pub vert: Vec<V3>,
}

fn unit_or_zero(v: V3) -> (V3, f32) {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len == 0.0 { ([0.0; 3], 0.0) } else { ([v[0] / len, v[1] / len, v[2] / len], len) }
}

/// `normals_calc_verts_simple` for one vertex: sum of the unit face normals, normalized, `(0, 0, 1)` when degenerate.
fn vert_normal(vfaces: &[Vec<u32>], face_normals: &[V3], v: usize) -> V3 {
    let mut n = [0.0f32; 3];
    for &f in &vfaces[v] {
        let fnrm = face_normals[f as usize];
        n = [n[0] + fnrm[0], n[1] + fnrm[1], n[2] + fnrm[2]];
    }
    let (u, len) = unit_or_zero(n);
    if len == 0.0 { [0.0, 0.0, 1.0] } else { u }
}

/// Full computation (the dirty-cache branch of `update_normals_mesh`).
pub fn calc_all(mesh: &Mesh, vfaces: &[Vec<u32>]) -> NormalsCache {
    let face: Vec<V3> = (0..mesh.faces.len()).map(|f| unit_or_zero(mesh.face_area_normal(f)).0).collect();
    let vert = (0..mesh.verts.len()).map(|v| vert_normal(vfaces, &face, v)).collect();
    NormalsCache { face, vert }
}

/// `update_normals_mesh` with a clean cache: recompute face normals of the tagged leaves, then the normals of all
/// their vertices. Returns the vertices that were recomputed (sorted, unique).
pub fn update_nodes(mesh: &Mesh, vfaces: &[Vec<u32>], pbvh: &Pbvh, leaves: &[usize], cache: &mut NormalsCache) -> Vec<usize> {
    let mut verts = Vec::new();
    for &l in leaves {
        for &f in &pbvh.faces[pbvh.nodes[l].faces.clone()] {
            cache.face[f as usize] = unit_or_zero(mesh.face_area_normal(f as usize)).0;
            verts.extend(mesh.faces[f as usize].iter().map(|&v| v as usize));
        }
    }
    verts.sort_unstable();
    verts.dedup();
    for &v in &verts {
        cache.vert[v] = vert_normal(vfaces, &cache.face, v);
    }
    verts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::adjacency;

    fn grid(n: usize) -> Mesh {
        let mut verts = Vec::new();
        for y in 0..=n {
            for x in 0..=n {
                verts.push([x as f32, y as f32, ((x * y) % 3) as f32 * 0.1]);
            }
        }
        let mut faces = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let a = (y * (n + 1) + x) as u32;
                faces.push(vec![a, a + 1, a + n as u32 + 2, a + n as u32 + 1]);
            }
        }
        Mesh { verts, faces }
    }

    #[test]
    fn partial_update_equals_full_recompute() {
        let mut mesh = grid(16);
        let topo = adjacency(&mesh);
        let mut cache = calc_all(&mesh, &topo.vfaces);
        let center = [8.0, 8.0, 0.0];
        let radius = 3.0;
        for v in 0..mesh.verts.len() {
            let d = [mesh.verts[v][0] - center[0], mesh.verts[v][1] - center[1]];
            if d[0] * d[0] + d[1] * d[1] < radius * radius {
                mesh.verts[v][2] += 2.0;
            }
        }
        let mut pbvh = Pbvh::build(&mesh, 8);
        let leaves = pbvh.leaves_near(center, radius + 2.0);
        pbvh.update_bounds_mesh(&mesh, &leaves);
        let touched = update_nodes(&mesh, &topo.vfaces, &pbvh, &leaves, &mut cache);
        assert!(!touched.is_empty() && touched.len() < mesh.verts.len());
        assert_eq!(cache, calc_all(&mesh, &topo.vfaces));
    }

    #[test]
    fn degenerate_vertex_gets_up_normal() {
        let mesh = Mesh { verts: vec![[0.0; 3]; 3], faces: vec![vec![0, 1, 2]] };
        let vfaces = vec![vec![0u32], vec![0], vec![0]];
        assert_eq!(calc_all(&mesh, &vfaces).vert[0], [0.0, 0.0, 1.0]);
    }
}
