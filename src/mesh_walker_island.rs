//! `BMW_ISLAND_MANIFOLD` face walker (`bmesh_walkers_impl.cc`: `bmw_IslandWalker_begin` / `_step_ex(only_manifold)`).
//!
//! From a seed face the walk (breadth first) crosses each edge of the current face to the first not yet visited,
//! masked face on that edge. With `only_manifold` an edge is crossed only when exactly two masked faces use it (this
//! face plus one other), so an edge shared by three faces (a T junction) does not connect them. Faces failing the mask
//! are never started from and never entered (`bmw_mask_check_face`); `Mesh` has no edge mask, so every edge passes.

use crate::mesh::Mesh;
use crate::mesh_query::Topology;
use std::collections::VecDeque;

/// Walks the island containing `seed` (empty when `seed` fails the mask). Order is the walker's yield order.
pub fn island_walk(topo: &Topology, mesh: &Mesh, seed: usize, mask: Option<&[bool]>, only_manifold: bool) -> Vec<usize> {
    let ok = |f: usize| mask.map_or(true, |m| m.get(f).copied().unwrap_or(false));
    if seed >= mesh.faces.len() || !ok(seed) {
        return Vec::new();
    }
    let mut visited = vec![false; mesh.faces.len()];
    visited[seed] = true;
    let (mut queue, mut out) = (VecDeque::from([seed]), Vec::new());
    while let Some(cur) = queue.pop_front() {
        out.push(cur);
        for c in 0..mesh.faces[cur].len() {
            let radial = topo.radial_faces(cur, c);
            if only_manifold && !radial.is_empty() {
                let usable = 1 + radial.iter().filter(|&&f| ok(f)).take(2).count();
                if usable != 2 {
                    continue;
                }
            }
            if let Some(&f) = radial.iter().find(|&&f| ok(f) && f != cur && !visited[f]) {
                visited[f] = true;
                queue.push_back(f);
            }
        }
    }
    out
}

/// All manifold islands over the masked faces, seeded in face order (each face belongs to exactly one island).
pub fn manifold_islands(mesh: &Mesh, mask: Option<&[bool]>) -> Vec<Vec<usize>> {
    let topo = Topology::new(mesh);
    let mut seen = vec![false; mesh.faces.len()];
    let mut islands = Vec::new();
    for f in 0..mesh.faces.len() {
        if seen[f] {
            continue;
        }
        let island = island_walk(&topo, mesh, f, mask, true);
        if island.is_empty() {
            continue;
        }
        for &g in &island {
            seen[g] = true;
        }
        islands.push(island);
    }
    islands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(faces: &[&[u32]]) -> Mesh {
        Mesh { verts: vec![[0.0; 3]; 12], faces: faces.iter().map(|f| f.to_vec()).collect() }
    }

    #[test]
    fn separate_quads_two_islands() {
        let mesh = m(&[&[0, 1, 2, 3], &[4, 5, 6, 7]]);
        assert_eq!(manifold_islands(&mesh, None), vec![vec![0], vec![1]]);
    }

    #[test]
    fn shared_edge_quads_one_island() {
        let mesh = m(&[&[0, 1, 2, 3], &[1, 4, 5, 2]]);
        assert_eq!(manifold_islands(&mesh, None), vec![vec![0, 1]]);
    }

    #[test]
    fn t_junction_does_not_connect() {
        // three faces on edge (0,1)
        let mesh = m(&[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4]]);
        assert_eq!(manifold_islands(&mesh, None), vec![vec![0], vec![1], vec![2]]);
        // masking out the third face makes the edge manifold again
        let mask = [true, true, false];
        assert_eq!(manifold_islands(&mesh, Some(&mask)), vec![vec![0, 1]]);
    }

    #[test]
    fn non_manifold_walk_crosses_t_junction() {
        let mesh = m(&[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4]]);
        let topo = Topology::new(&mesh);
        let mut w = island_walk(&topo, &mesh, 0, None, false);
        w.sort();
        assert!(w.len() >= 2 && w.contains(&1));
    }

    #[test]
    fn cube_is_one_island_and_mask_splits() {
        let cube = Mesh::cube(1.0);
        assert_eq!(manifold_islands(&cube, None).len(), 1);
        let mask = vec![false; cube.faces.len()];
        assert!(manifold_islands(&cube, Some(&mask)).is_empty());
    }
}
