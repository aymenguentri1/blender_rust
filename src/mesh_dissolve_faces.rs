//! Dissolve Faces of an edit-mesh selection (`MESH_OT_dissolve_faces`).
//!
//! Ported from `editmesh_tools.cc` `edbm_dissolve_faces_exec()` (6209), `bmo_dissolve.cc` `bmo_dissolve_faces_exec()`
//! (225) and `bmesh_core.cc` `BM_faces_join()` (1252, `do_del = true`):
//!  - the selected faces are grouped into regions by walking manifold edges (an edge used by exactly two faces, both
//!    selected, `BMW_ISLAND_MANIFOLD`); a region of one face is left alone;
//!  - a region is joined into one face whose boundary is the region's edges used by a single region face, starting at the
//!    first such edge of the region's first face and following its direction; the joined faces and the interior edges
//!    and vertices (every face of the vertex is in the region, none of its edges on the boundary) are deleted;
//!  - a region fails (its faces stay) when an edge has more than two region faces or the boundary is not one closed
//!    loop; a joined face equal to an existing face reuses that face (`f_double`);
//!  - `use_verts`: a vertex that did not have exactly two edges before and has exactly two now is dissolved out of its faces.
//!    The joined faces are appended after the untouched faces. Not ported: loop data, multires, custom normals; a region
//!    whose windings disagree is treated as a failed join (`BM_face_create_ngon` works on undirected edges).

use crate::mesh::Mesh;
use std::collections::{HashMap, HashSet};

pub struct Dissolved {
    pub mesh: Mesh,
    /// Indices of the joined faces in `mesh` (`region.out`).
    pub new_faces: Vec<u32>,
}

fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn degrees(faces: &[Vec<u32>], nv: usize) -> Vec<usize> {
    let mut e: HashSet<(u32, u32)> = HashSet::new();
    for f in faces {
        for k in 0..f.len() {
            e.insert(key(f[k], f[(k + 1) % f.len()]));
        }
    }
    let mut d = vec![0; nv];
    for (a, b) in e {
        d[a as usize] += 1;
        d[b as usize] += 1;
    }
    d
}

/// `dissolve_faces` on the faces `faces` (indices; repeats and out of range ignored). `None` when none is valid.
pub fn dissolve_faces(mesh: &Mesh, faces: &[u32], use_verts: bool) -> Option<Dissolved> {
    let nf = mesh.faces.len();
    let nv = mesh.verts.len();
    let mut sel = vec![false; nf];
    for &f in faces {
        if let Some(s) = sel.get_mut(f as usize) {
            *s = true;
        }
    }
    if !sel.iter().any(|&s| s) {
        return None;
    }
    let deg_before = degrees(&mesh.faces, nv);
    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, f) in mesh.faces.iter().enumerate() {
        for k in 0..f.len() {
            edge_faces.entry(key(f[k], f[(k + 1) % f.len()])).or_default().push(i);
        }
    }
    /* Regions: breadth first over manifold edges between selected faces, in mesh order. */
    let mut seen = vec![false; nf];
    let mut regions: Vec<Vec<usize>> = Vec::new();
    for start in 0..nf {
        if !sel[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        let mut region = vec![start];
        let mut q = 0;
        while q < region.len() {
            let f = &mesh.faces[region[q]];
            q += 1;
            for k in 0..f.len() {
                let list = &edge_faces[&key(f[k], f[(k + 1) % f.len()])];
                if list.len() == 2 {
                    for &o in list {
                        if sel[o] && !seen[o] {
                            seen[o] = true;
                            region.push(o);
                        }
                    }
                }
            }
        }
        if region.len() >= 2 {
            regions.push(region);
        }
    }
    let mut dead_face = vec![false; nf];
    let mut dead_vert = vec![false; nv];
    let mut joined: Vec<Vec<u32>> = Vec::new();
    for region in &regions {
        let inr: HashSet<usize> = region.iter().copied().collect();
        let rlen = |a: u32, b: u32| edge_faces[&key(a, b)].iter().filter(|i| inr.contains(i)).count();
        let mut boundary: Vec<(u32, u32)> = Vec::new();
        let mut ok = true;
        for &fi in region {
            let f = &mesh.faces[fi];
            for k in 0..f.len() {
                let (a, b) = (f[k], f[(k + 1) % f.len()]);
                match rlen(a, b) {
                    1 => boundary.push((a, b)),
                    2 => {}
                    _ => ok = false,
                }
            }
        }
        if !ok || boundary.is_empty() {
            continue;
        }
        let next: HashMap<u32, u32> = boundary.iter().copied().collect();
        if next.len() != boundary.len() {
            continue;
        }
        let (v1, mut v) = (boundary[0].0, boundary[0].1);
        let mut poly = vec![v1];
        while v != v1 && poly.len() <= boundary.len() {
            poly.push(v);
            let Some(&n) = next.get(&v) else { break };
            v = n;
        }
        if v != v1 || poly.len() != boundary.len() {
            continue;
        }
        /* Interior vertices: all faces in the region and no boundary edge. */
        let on_boundary: HashSet<u32> = boundary.iter().flat_map(|&(a, b)| [a, b]).collect();
        for &fi in region {
            for &vi in &mesh.faces[fi] {
                if !on_boundary.contains(&vi) && mesh.faces.iter().enumerate().all(|(j, g)| !g.contains(&vi) || inr.contains(&j)) {
                    dead_vert[vi as usize] = true;
                }
            }
        }
        for &fi in region {
            dead_face[fi] = true;
        }
        let mut set = poly.clone();
        set.sort_unstable();
        let double = mesh.faces.iter().enumerate().any(|(j, g)| {
            let mut s = g.clone();
            s.sort_unstable();
            !inr.contains(&j) && s == set
        });
        if !double {
            joined.push(poly);
        }
    }
    let mut faces_out: Vec<Vec<u32>> = mesh.faces.iter().enumerate().filter(|(i, _)| !dead_face[*i]).map(|(_, f)| f.clone()).collect();
    let first_new = faces_out.len();
    faces_out.extend(joined);
    if use_verts {
        let now = degrees(&faces_out, nv);
        for v in 0..nv {
            if dead_vert[v] || deg_before[v] == 2 || now[v] != 2 {
                continue;
            }
            if faces_out.iter().any(|f| f.contains(&(v as u32)) && f.len() <= 3) {
                continue;
            }
            for f in &mut faces_out {
                f.retain(|&x| x != v as u32);
            }
            dead_vert[v] = true;
        }
    }
    let mut remap = vec![u32::MAX; nv];
    let mut out = Mesh { verts: Vec::new(), faces: Vec::new() };
    for i in 0..nv {
        if !dead_vert[i] {
            remap[i] = out.verts.len() as u32;
            out.verts.push(mesh.verts[i]);
        }
    }
    for f in &faces_out {
        out.faces.push(f.iter().map(|&v| remap[v as usize]).collect());
    }
    let new_faces = (first_new..out.faces.len()).map(|i| i as u32).collect();
    Some(Dissolved { mesh: out, new_faces })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: usize, h: usize) -> Mesh {
        let mut m = Mesh { verts: Vec::new(), faces: Vec::new() };
        for y in 0..=h {
            for x in 0..=w {
                m.verts.push([x as f32, y as f32, 0.0]);
            }
        }
        let id = |x: usize, y: usize| (y * (w + 1) + x) as u32;
        for y in 0..h {
            for x in 0..w {
                m.faces.push(vec![id(x, y), id(x + 1, y), id(x + 1, y + 1), id(x, y + 1)]);
            }
        }
        m
    }

    #[test]
    fn two_adjacent_quads_become_one_hexagon_and_the_shared_edge_is_gone() {
        let d = dissolve_faces(&grid(2, 1), &[0, 1], false).unwrap();
        assert_eq!(d.mesh.faces, vec![vec![0, 1, 2, 5, 4, 3]]);
        assert_eq!(d.new_faces, vec![0]);
        assert_eq!(d.mesh.verts.len(), 6, "no vertex was interior");
    }

    #[test]
    fn a_single_face_or_no_selection_is_a_no_op() {
        let m = grid(2, 1);
        let d = dissolve_faces(&m, &[0], false).unwrap();
        assert_eq!((d.mesh.faces.clone(), d.mesh.verts.len(), d.new_faces.len()), (m.faces.clone(), 6, 0));
        assert!(dissolve_faces(&m, &[], false).is_none());
    }

    #[test]
    fn a_full_two_by_two_block_loses_its_interior_vertex() {
        let d = dissolve_faces(&grid(2, 2), &[0, 1, 2, 3], false).unwrap();
        assert_eq!(d.mesh.faces.len(), 1);
        assert_eq!(d.mesh.faces[0].len(), 8);
        assert_eq!(d.mesh.verts.len(), 8, "the center vertex is deleted");
    }

    #[test]
    fn unselected_faces_stay_first_and_a_ring_with_a_hole_is_not_joined() {
        let m = grid(3, 3);
        let ring = [0, 1, 2, 3, 5, 6, 7, 8];
        let d = dissolve_faces(&m, &ring, false).unwrap();
        assert_eq!(d.mesh.faces.len(), 9, "two boundary loops: the join fails, every face stays");
        let d = dissolve_faces(&m, &[0, 1], false).unwrap();
        assert_eq!(d.mesh.faces.len(), 8);
        assert_eq!(d.mesh.faces[0], m.faces[2], "untouched faces keep their order");
        assert_eq!(d.new_faces, vec![7]);
    }

    #[test]
    fn use_verts_dissolves_vertices_left_with_two_edges() {
        let mut m = grid(2, 1);
        m.verts.push([1.0, 2.0, 0.0]);
        m.verts.push([0.0, 2.0, 0.0]);
        /* A quad above the left cell shares edge 3-4; verts 1 and 4 have three edges before the dissolve. */
        m.faces.push(vec![3, 4, 7, 6]);
        let plain = dissolve_faces(&m, &[0, 1], false).unwrap();
        assert_eq!(plain.mesh.verts.len(), 8);
        let d = dissolve_faces(&m, &[0, 1], true).unwrap();
        assert_eq!(d.mesh.verts.len(), 7, "only vertex 1 (bottom middle) had three edges and now has two");
        assert!(d.mesh.faces.iter().all(|f| f.len() >= 3));
    }
}
