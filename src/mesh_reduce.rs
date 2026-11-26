//! Topology-changing modifiers that remove or split geometry: Decimate (collapse) and Edge Split.
//!
//! Modelled on Blender:
//!  - `MOD_decimate` (Collapse): the mesh is triangulated, then the shortest edges are collapsed to their
//!    midpoint until `ratio` of the triangles is left. Blender orders collapses by a quadric error metric;
//!    edge length is used here, which keeps the silhouette less well but needs no extra data. A collapse is
//!    only done when it keeps the surface manifold (the endpoints' common neighbours are exactly the
//!    opposite corners of the faces on the edge). Meshes over 50 000 triangles are left alone so the
//!    viewport stays responsive.
//!  - `MOD_edgesplit`: an edge shared by two faces whose normals differ by more than the split angle
//!    (default 30 degrees) is cut: its vertices are duplicated for each smooth group of faces around them.
//!    Boundary edges are already open and stay as they are.

use crate::mesh::{dot, sub, Mesh, V3};
use std::collections::HashMap;

const DECIMATE_LIMIT: usize = 50_000;

impl Mesh {
    /// The Decimate modifier (Collapse). `ratio` 1 keeps the mesh untouched, 0.5 about half of its triangles.
    pub fn decimated(&self, ratio: f32) -> Mesh {
        if ratio >= 1.0 {
            return self.clone();
        }
        let tri = self.triangulated();
        let total = tri.faces.len();
        if total > DECIMATE_LIMIT || total <= 4 {
            return self.clone();
        }
        let target = ((total as f32 * ratio.max(0.0)).ceil() as usize).max(4);
        let mut verts = tri.verts;
        let mut faces = tri.faces;
        let mut tris = total;
        while tris > target {
            let n = verts.len();
            let mut nb: Vec<Vec<u32>> = vec![Vec::new(); n];
            let mut shared: HashMap<(u32, u32), usize> = HashMap::new();
            for f in &faces {
                for i in 0..3 {
                    let (a, b) = (f[i], f[(i + 1) % 3]);
                    *shared.entry((a.min(b), a.max(b))).or_insert(0) += 1;
                    nb[a as usize].push(b);
                    nb[b as usize].push(a);
                }
            }
            for l in &mut nb {
                l.sort_unstable();
                l.dedup();
            }
            let mut edges: Vec<(f32, u32, u32)> = shared
                .keys()
                .map(|&(a, b)| {
                    let d = sub(verts[a as usize], verts[b as usize]);
                    (dot(d, d), a, b)
                })
                .collect();
            edges.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
            let mut touched = vec![false; n];
            let mut remap: Vec<u32> = (0..n as u32).collect();
            let mut did = false;
            for (_, a, b) in edges {
                if tris <= target {
                    break;
                }
                let (ia, ib) = (a as usize, b as usize);
                if touched[ia] || touched[ib] {
                    continue;
                }
                let faces_on_edge = shared[&(a, b)];
                let common = nb[ia].iter().filter(|v| nb[ib].binary_search(v).is_ok()).count();
                if common != faces_on_edge || tris - faces_on_edge < 4 {
                    continue;
                }
                let (pa, pb) = (verts[ia], verts[ib]);
                verts[ia] = [(pa[0] + pb[0]) * 0.5, (pa[1] + pb[1]) * 0.5, (pa[2] + pb[2]) * 0.5];
                remap[ib] = a;
                touched[ia] = true;
                touched[ib] = true;
                for &v in nb[ia].iter().chain(nb[ib].iter()) {
                    touched[v as usize] = true;
                }
                tris -= faces_on_edge;
                did = true;
            }
            if !did {
                break;
            }
            faces = faces
                .into_iter()
                .map(|f| f.into_iter().map(|v| remap[v as usize]).collect::<Vec<u32>>())
                .filter(|f| f[0] != f[1] && f[1] != f[2] && f[0] != f[2])
                .collect();
            tris = faces.len();
        }
        /* Drop the vertices no face uses any more. */
        let mut new_index = vec![u32::MAX; verts.len()];
        let mut out = Mesh::default();
        for f in &mut faces {
            for v in f.iter_mut() {
                if new_index[*v as usize] == u32::MAX {
                    new_index[*v as usize] = out.verts.len() as u32;
                    out.verts.push(verts[*v as usize]);
                }
                *v = new_index[*v as usize];
            }
        }
        out.faces = faces;
        out
    }

    /// The Edge Split modifier with a split angle in degrees.
    pub fn edge_split(&self, angle_degrees: f32) -> Mesh {
        let cos_limit = angle_degrees.to_radians().cos();
        let normals: Vec<V3> = (0..self.faces.len()).map(|f| self.face_normal(f)).collect();
        let mut offset = Vec::with_capacity(self.faces.len());
        let mut corners = 0usize;
        for f in &self.faces {
            offset.push(corners);
            corners += f.len();
        }
        let mut parent: Vec<usize> = (0..corners).collect();
        fn find(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        let mut edge_faces: HashMap<(u32, u32), Vec<(usize, usize)>> = HashMap::new();
        for (fi, f) in self.faces.iter().enumerate() {
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                edge_faces.entry((a.min(b), a.max(b))).or_default().push((fi, i));
            }
        }
        for (&(a, b), users) in &edge_faces {
            if users.len() != 2 {
                continue;
            }
            let ((f, _), (g, _)) = (users[0], users[1]);
            if dot(normals[f], normals[g]) < cos_limit {
                continue;
            }
            for v in [a, b] {
                let cf = self.faces[f].iter().position(|&x| x == v).unwrap();
                let cg = self.faces[g].iter().position(|&x| x == v).unwrap();
                let (rf, rg) = (find(&mut parent, offset[f] + cf), find(&mut parent, offset[g] + cg));
                if rf != rg {
                    parent[rf] = rg;
                }
            }
        }
        let mut ids: HashMap<(u32, usize), u32> = HashMap::new();
        let mut out = Mesh::default();
        for (fi, f) in self.faces.iter().enumerate() {
            let mut nf = Vec::with_capacity(f.len());
            for (i, &v) in f.iter().enumerate() {
                let root = find(&mut parent, offset[fi] + i);
                let id = *ids.entry((v, root)).or_insert_with(|| {
                    out.verts.push(self.verts[v as usize]);
                    out.verts.len() as u32 - 1
                });
                nf.push(id);
            }
            out.faces.push(nf);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimate_reaches_the_ratio_and_stays_valid() {
        let s = Mesh::uv_sphere(32, 16, 1.0);
        let tris = s.triangulated().faces.len();
        let d = s.decimated(0.5);
        assert!(d.faces.len() <= tris / 2 + 2 && d.faces.len() >= tris / 4, "{} of {tris}", d.faces.len());
        assert!(d.faces.iter().all(|f| f.len() == 3 && f.iter().all(|&i| (i as usize) < d.verts.len())));
        /* No vertex is left without a face. */
        let mut used = vec![false; d.verts.len()];
        d.faces.iter().flatten().for_each(|&i| used[i as usize] = true);
        assert!(used.iter().all(|u| *u));
        /* A closed surface stays closed: every edge has two faces. */
        assert!(d.edges().iter().all(|e| e.faces == 2));
        /* The sphere is still roughly a sphere. */
        assert!(d.verts.iter().all(|v| (v.iter().map(|c| c * c).sum::<f32>().sqrt() - 1.0).abs() < 0.2));
    }

    #[test]
    fn decimate_one_changes_nothing_and_tiny_meshes_are_kept() {
        let cube = Mesh::cube(2.0);
        assert_eq!(cube.decimated(1.0), cube);
        /* A triangular pyramid has 4 triangles: nothing can be collapsed without destroying the solid. */
        let pyramid = Mesh::cone(3, 1.0, 0.0, 1.0);
        assert!(pyramid.triangulated().faces.len() <= 4);
        assert_eq!(pyramid.decimated(0.1), pyramid);
    }

    #[test]
    fn edge_split_cuts_a_cube_but_not_a_smooth_sphere_or_a_plane() {
        let cube = Mesh::cube(2.0);
        let c = cube.edge_split(30.0);
        assert_eq!(c.verts.len(), 24);
        assert_eq!(c.faces.len(), 6);
        assert_eq!(cube.edge_split(95.0), cube);
        let plane = Mesh::grid(4, 4, 2.0);
        assert_eq!(plane.edge_split(30.0).verts.len(), plane.verts.len());
        let ico = Mesh::ico_sphere(2, 1.0);
        assert_eq!(ico.edge_split(45.0).verts.len(), ico.verts.len());
    }
}
