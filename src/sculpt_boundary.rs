//! Sculpt boundary vertex search (`mesh/sculpt_boundary.cc`: `check_counts`, `is_vert_in_editable_boundary_mesh`,
//! `get_closest_boundary_vert_mesh`, `indices_init_mesh`; `sculpt.cc`: `create_boundary_info`, `vert_is_boundary`,
//! `vert_neighbors_get_mesh`; `sculpt_flood_fill.cc`: `FillDataMesh::execute`).
//! Deviation: the flood fill, like Blender's, does not mark the initial vertex visited, so the start vertex is reached
//! again from its first boundary neighbour and listed twice (its distance keeps the first value, `Map::add`). Only the base mesh
//! (no grids / BMesh) is ported; `fake_neighbors` are not used.

use std::collections::{HashMap, VecDeque};

use crate::mesh::{sub, Mesh, V3};

fn len(a: V3, b: V3) -> f32 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Topology and visibility data the boundary code reads (`vert_to_face`, `hide_poly`, `hide_vert`, boundary bits).
pub struct Topo<'a> {
    pub mesh: &'a Mesh,
    pub vert_to_face: Vec<Vec<usize>>,
    pub hide_poly: Vec<bool>,
    pub hide_vert: Vec<bool>,
    pub boundary_verts: Vec<bool>,
}

/// `create_boundary_info()`: a vertex is on the boundary when an edge it belongs to is used by fewer than two faces.
pub fn boundary_info(mesh: &Mesh) -> Vec<bool> {
    let mut count: HashMap<(u32, u32), u32> = HashMap::new();
    for f in &mesh.faces {
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            *count.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let mut bits = vec![false; mesh.verts.len()];
    for (&(a, b), &n) in &count {
        if n < 2 {
            bits[a as usize] = true;
            bits[b as usize] = true;
        }
    }
    bits
}

impl<'a> Topo<'a> {
    /// Nothing hidden.
    pub fn new(mesh: &'a Mesh) -> Self {
        Self::with_hidden(mesh, vec![false; mesh.faces.len()], vec![false; mesh.verts.len()])
    }

    pub fn with_hidden(mesh: &'a Mesh, hide_poly: Vec<bool>, hide_vert: Vec<bool>) -> Self {
        let mut vert_to_face = vec![Vec::new(); mesh.verts.len()];
        for (fi, f) in mesh.faces.iter().enumerate() {
            for &v in f {
                vert_to_face[v as usize].push(fi);
            }
        }
        Topo { mesh, vert_to_face, hide_poly, hide_vert, boundary_verts: boundary_info(mesh) }
    }

    /// `vert_neighbors_get_mesh()`: both adjacent vertices of every visible face around `v`, without duplicates.
    pub fn neighbors(&self, v: usize) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        for &fi in &self.vert_to_face[v] {
            if self.hide_poly[fi] {
                continue;
            }
            let f = &self.mesh.faces[fi];
            let Some(k) = f.iter().position(|&x| x as usize == v) else { continue };
            for n in [f[(k + f.len() - 1) % f.len()] as usize, f[(k + 1) % f.len()] as usize] {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
        out
    }

    /// `boundary::vert_is_boundary()`: a vertex next to a hidden face counts as boundary.
    pub fn is_boundary(&self, v: usize) -> bool {
        if self.vert_to_face[v].iter().any(|&f| self.hide_poly[f]) {
            return true;
        }
        self.boundary_verts[v]
    }

    /// `FillDataMesh::execute()`: breadth first over visible neighbours; `func(from, to)` returns whether to continue.
    fn flood(&self, initial: usize, mut func: impl FnMut(usize, usize) -> bool) {
        let mut visited = vec![false; self.mesh.verts.len()];
        let mut queue = VecDeque::from([initial]);
        while let Some(from) = queue.pop_front() {
            for to in self.neighbors(from) {
                if visited[to] || self.hide_vert[to] {
                    continue;
                }
                visited[to] = true;
                if func(from, to) {
                    queue.push_back(to);
                }
            }
        }
    }

    /// `is_vert_in_editable_boundary_mesh()` (with `check_counts`): corners and non-manifold boundaries are not editable.
    pub fn is_in_editable_boundary(&self, v: usize) -> bool {
        if self.hide_vert[v] {
            return false;
        }
        let (mut neighbor_count, mut boundary_count) = (0, 0);
        for n in self.neighbors(v) {
            if !self.hide_vert[n] {
                neighbor_count += 1;
                if self.is_boundary(n) {
                    boundary_count += 1;
                }
            }
        }
        neighbor_count > 2 && boundary_count <= 2
    }

    /// `get_closest_boundary_vert_mesh()`: the boundary vertex with the fewest topology steps within `radius`.
    pub fn closest_boundary_vert(&self, initial: usize, radius: f32) -> Option<usize> {
        if self.is_boundary(initial) {
            return Some(initial);
        }
        let origin = self.mesh.verts[initial];
        let radius_sq = radius * radius;
        let mut steps = vec![0i32; self.mesh.verts.len()];
        let (mut best, mut best_steps) = (None, i32::MAX);
        self.flood(initial, |from, to| {
            steps[to] = steps[from] + 1;
            if self.is_boundary(to) && steps[to] < best_steps {
                best_steps = steps[to];
                best = Some(to);
            }
            let d = sub(origin, self.mesh.verts[to]);
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < radius_sq
        });
        best
    }
}

/// The collected boundary (`SculptBoundary::verts`, `distance`, `edges`).
#[derive(Debug, Default)]
pub struct Boundary {
    pub verts: Vec<usize>,
    pub distance: HashMap<usize, f32>,
    pub edges: Vec<(V3, V3)>,
}

/// `indices_init_mesh()`: walk the boundary from `initial`, accumulating the edge length distance.
pub fn indices_init(topo: &Topo, initial: usize) -> Boundary {
    let mut b = Boundary::default();
    b.verts.push(initial);
    b.distance.insert(initial, 0.0);
    topo.flood(initial, |from, to| {
        if !topo.is_boundary(to) {
            return false;
        }
        let (a, c) = (topo.mesh.verts[from], topo.mesh.verts[to]);
        let d = b.distance.get(&from).copied().unwrap_or(0.0) + len(a, c);
        b.verts.push(to);
        b.distance.entry(to).or_insert(d);
        b.edges.push((a, c));
        topo.is_in_editable_boundary(to)
    });
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_boundary_is_the_outer_ring() {
        let m = Mesh::grid(4, 4, 2.0);
        let bits = boundary_info(&m);
        assert_eq!(bits.iter().filter(|&&b| b).count(), 16);
    }

    #[test]
    fn closed_mesh_has_no_boundary() {
        let m = Mesh::cube(2.0);
        assert!(boundary_info(&m).iter().all(|&b| !b));
        assert_eq!(Topo::new(&m).closest_boundary_vert(0, 100.0), None);
    }

    #[test]
    fn closest_boundary_vert_is_found_inside_the_radius_only() {
        let m = Mesh::grid(4, 4, 2.0);
        let t = Topo::new(&m);
        let centre = (0..m.verts.len()).find(|&i| m.verts[i][0].abs() < 1e-4 && m.verts[i][1].abs() < 1e-4).unwrap();
        assert_eq!(t.closest_boundary_vert(centre, 0.1), None);
        let hit = t.closest_boundary_vert(centre, 5.0).unwrap();
        assert!(t.is_boundary(hit));
        assert_eq!(t.closest_boundary_vert(hit, 0.0), Some(hit));
    }

    #[test]
    fn indices_walk_one_side_up_to_the_corners() {
        let m = Mesh::grid(4, 4, 2.0);
        let t = Topo::new(&m);
        let start = (0..m.verts.len()).find(|&i| t.is_boundary(i) && t.is_in_editable_boundary(i)).unwrap();
        let b = indices_init(&t, start);
        // The walk stops at the two corners of the side (`check_counts`), which are listed but not expanded.
        let distinct: std::collections::HashSet<_> = b.verts.iter().collect();
        assert_eq!(distinct.len(), 5);
        assert_eq!(b.verts.iter().filter(|&&v| v == start).count(), 2); // the start is listed again, as in Blender
        assert_eq!(b.distance[&start], 0.0);
        assert!(b.verts.iter().all(|&v| t.is_boundary(v)));
        assert_eq!(b.edges.len(), b.verts.len() - 1);
        assert_eq!(b.distance.len(), 5);
        assert!((b.distance.values().cloned().fold(0.0, f32::max) - 1.5).abs() < 1e-4 || (b.distance.values().cloned().fold(0.0, f32::max) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn hidden_face_makes_its_verts_boundary() {
        let m = Mesh::grid(2, 2, 2.0);
        let mut hide = vec![false; m.faces.len()];
        hide[0] = true;
        let t = Topo::with_hidden(&m, hide, vec![false; m.verts.len()]);
        for &v in &m.faces[0] {
            assert!(t.is_boundary(v as usize));
        }
    }

    #[test]
    fn corner_vertex_is_not_editable() {
        let m = Mesh::grid(2, 2, 2.0);
        let t = Topo::new(&m);
        let corner = (0..m.verts.len()).find(|&i| m.verts[i][0] < -0.9 && m.verts[i][1] < -0.9).unwrap();
        assert!(!t.is_in_editable_boundary(corner));
    }
}
