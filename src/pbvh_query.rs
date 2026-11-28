//! PBVH-backed sphere query for sculpt dabs (source/blender/editors/sculpt_paint/mesh/sculpt.cc `node_in_sphere`,
//! pbvh.cc `search_nodes`): only vertices of leaves whose bounds touch the brush sphere are tested.

use crate::mesh::Mesh;
use crate::pbvh_build::{build_mesh_nodes, MeshNode};
use crate::pbvh_partition::V3;
use crate::pbvh_search::search_gather;

/// Leaf size used for the stroke tree.
pub const LEAF_LIMIT: usize = 10_000 / 100;

pub struct Pbvh {
    pub nodes: Vec<MeshNode>,
    /// Face indices reordered by the build; leaves reference ranges of it.
    pub faces: Vec<i32>,
    pub bounds: Vec<(V3, V3)>,
}

fn merge(a: (V3, V3), b: (V3, V3)) -> (V3, V3) {
    let mut lo = a.0;
    let mut hi = a.1;
    for k in 0..3 {
        lo[k] = lo[k].min(b.0[k]);
        hi[k] = hi[k].max(b.1[k]);
    }
    (lo, hi)
}

impl Pbvh {
    /// Build from `mesh` (face centers = mean of the face vertices, node bounds = bounds of their vertices).
    pub fn build(mesh: &Mesh, leaf_limit: usize) -> Pbvh {
        let centers: Vec<V3> = mesh
            .faces
            .iter()
            .map(|f| {
                let mut c = [0.0f32; 3];
                for &v in f {
                    for k in 0..3 {
                        c[k] += mesh.verts[v as usize][k];
                    }
                }
                let n = f.len().max(1) as f32;
                [c[0] / n, c[1] / n, c[2] / n]
            })
            .collect();
        let mut faces: Vec<i32> = (0..mesh.faces.len() as i32).collect();
        let nodes = build_mesh_nodes(&[], leaf_limit, &centers, &mut faces);
        let mut bounds = vec![([f32::MAX; 3], [f32::MIN; 3]); nodes.len()];
        // Children always sit after their parent, so a reverse sweep sees children first.
        for i in (0..nodes.len()).rev() {
            if nodes[i].leaf {
                for &f in &faces[nodes[i].faces.clone()] {
                    for &v in &mesh.faces[f as usize] {
                        let p = mesh.verts[v as usize];
                        bounds[i] = merge(bounds[i], (p, p));
                    }
                }
            } else if nodes[i].children_offset != 0 {
                let c = nodes[i].children_offset;
                bounds[i] = merge(bounds[c], bounds[c + 1]);
            }
        }
        Pbvh { nodes, faces, bounds }
    }

    /// `update_node_bounds_mesh`: bounds of the vertices of the faces of leaf `i`.
    fn leaf_bounds(&self, mesh: &Mesh, i: usize) -> (V3, V3) {
        let mut b = ([f32::MAX; 3], [f32::MIN; 3]);
        for &f in &self.faces[self.nodes[i].faces.clone()] {
            for &v in &mesh.faces[f as usize] {
                let p = mesh.verts[v as usize];
                b = merge(b, (p, p));
            }
        }
        b
    }

    /// `Tree::update_bounds_mesh` + `flush_bounds_to_parents`: recompute the bounds of the dirty leaves, then
    /// re-merge parents while their bounds keep changing.
    pub fn update_bounds_mesh(&mut self, mesh: &Mesh, dirty_leaves: &[usize]) {
        let mut pending: Vec<usize> = Vec::new();
        for &i in dirty_leaves {
            if self.nodes[i].leaf {
                self.bounds[i] = self.leaf_bounds(mesh, i);
                if self.nodes[i].parent >= 0 {
                    pending.push(self.nodes[i].parent as usize);
                }
            }
        }
        while let Some(n) = pending.pop() {
            let old = self.bounds[n];
            let c = self.nodes[n].children_offset;
            self.bounds[n] = merge(self.bounds[c], self.bounds[c + 1]);
            if self.bounds[n] != old && self.nodes[n].parent >= 0 {
                pending.push(self.nodes[n].parent as usize);
            }
        }
    }

    /// `node_in_sphere`: nearest point of the node bounds to `center` is closer than `radius`.
    pub fn node_in_sphere(&self, node: usize, center: V3, radius: f32) -> bool {
        let (lo, hi) = self.bounds[node];
        let mut d2 = 0.0;
        for k in 0..3 {
            let d = center[k] - center[k].clamp(lo[k], hi[k]);
            d2 += d * d;
        }
        d2 < radius * radius
    }

    /// `node_in_cylinder`: Blender computes the ray-to-bounds distance but still returns `true`
    /// (`dist_sq < radius^2 || true`, "TODO: Solve issues and enable distance check").
    pub fn node_in_cylinder(&self, node: usize, ray: &crate::math_ray_aabb::DistRayAabbPrecalc, radius: f32) -> bool {
        let (lo, hi) = self.bounds[node];
        let (dist_sq, _, _) = crate::math_ray_aabb::dist_squared_ray_to_aabb(ray, lo, hi);
        let _ = dist_sq < radius * radius;
        true
    }

    /// Sorted, unique vertex indices of the faces of the leaves touching the sphere.
    pub fn verts_near(&self, mesh: &Mesh, center: V3, radius: f32) -> Vec<usize> {
        let leaves = search_gather(&self.nodes, |i| self.node_in_sphere(i, center, radius));
        let mut verts = Vec::new();
        for l in leaves {
            for &f in &self.faces[self.nodes[l].faces.clone()] {
                verts.extend(mesh.faces[f as usize].iter().map(|&v| v as usize));
            }
        }
        verts.sort_unstable();
        verts.dedup();
        verts
    }
}

/// Same result as `sculpt::affected`, testing only the vertices of the leaves near the sphere.
pub fn affected_pbvh(mesh: &Mesh, pbvh: &Pbvh, mask: &[f32], center: V3, radius: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    for i in pbvh.verts_near(mesh, center, radius) {
        let v = mesh.verts[i];
        let d = [v[0] - center[0], v[1] - center[1], v[2] - center[2]];
        let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if d2 < radius * radius {
            let f = crate::sculpt::falloff(d2.sqrt(), radius) * (1.0 - crate::sculpt::mask_at(mask, i)).clamp(0.0, 1.0);
            if f > 0.0 {
                out.push((i, f));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn matches_brute_force_affected() {
        let mesh = grid(20);
        let pbvh = Pbvh::build(&mesh, 8);
        let mut mask = vec![0.0; mesh.verts.len()];
        mask[50] = 0.5;
        for (c, r) in [([5.0, 5.0, 0.0], 3.0), ([0.0, 0.0, 0.0], 2.5), ([19.5, 3.2, 0.1], 4.0), ([100.0, 0.0, 0.0], 2.0)] {
            assert_eq!(affected_pbvh(&mesh, &pbvh, &mask, c, r), crate::sculpt::affected(&mesh, &mask, c, r));
        }
    }

    #[test]
    fn bounds_refresh_matches_rebuild_after_moving_vertices() {
        let mut mesh = grid(12);
        let mut pbvh = Pbvh::build(&mesh, 6);
        for v in [5usize, 6, 18, 19] {
            mesh.verts[v][2] += 7.0;
        }
        let leaves: Vec<usize> = (0..pbvh.nodes.len()).filter(|&i| pbvh.nodes[i].leaf).collect();
        pbvh.update_bounds_mesh(&mesh, &leaves);
        let fresh = Pbvh::build(&mesh, 6);
        // Same face order (centers moved only in z, so rebuild may reorder): compare root bounds and per-leaf extents.
        assert_eq!(pbvh.bounds[0], fresh.bounds[0]);
        for i in 0..pbvh.nodes.len() {
            if pbvh.nodes[i].leaf {
                assert_eq!(pbvh.bounds[i], pbvh.leaf_bounds(&mesh, i));
            } else {
                let c = pbvh.nodes[i].children_offset;
                assert_eq!(pbvh.bounds[i], merge(pbvh.bounds[c], pbvh.bounds[c + 1]));
            }
        }
    }

    #[test]
    fn cylinder_test_matches_blender_always_true() {
        let mesh = grid(4);
        let pbvh = Pbvh::build(&mesh, 4);
        let ray = crate::math_ray_aabb::dist_ray_aabb_precalc([100.0, 100.0, 100.0], [0.0, 0.0, 1.0]);
        assert!(pbvh.node_in_cylinder(0, &ray, 0.1));
    }

    #[test]
    fn far_sphere_touches_no_leaf_and_node_test_is_strict() {
        let mesh = grid(8);
        let pbvh = Pbvh::build(&mesh, 4);
        assert!(pbvh.verts_near(&mesh, [50.0, 50.0, 0.0], 1.0).is_empty());
        assert!(!pbvh.node_in_sphere(0, [-1.0, 0.0, 0.0], 1.0));
        assert!(pbvh.node_in_sphere(0, [-1.0, 0.0, 0.0], 1.01));
    }
}
