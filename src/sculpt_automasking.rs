//! Sculpt automasking factors (`mesh/sculpt_automasking.cc`: `calc_vert_factors` face set / boundary edge / boundary face
//! set branches, `fill_constrained_topology_automasking_factors_mesh`, `init_boundary_masking_mesh`; `sculpt.cc`:
//! `vert_has_face_set`, `vert_has_unique_face_set`).
//! Deviations: base mesh only; the symmetric start vertices of the topology fill (`find_symm_verts_mesh`) and the symmetry
//! aware radius test are supplied by the caller; island, normal, cavity and occlusion modes are not ported here.

use crate::face_set::NONE_ID;
use crate::sculpt_boundary::Topo;

/// `vert_has_face_set()`: without a face set layer only the "none" id matches.
pub fn vert_has_face_set(topo: &Topo, face_sets: &[i32], v: usize, face_set: i32) -> bool {
    if face_sets.is_empty() {
        return face_set == NONE_ID;
    }
    topo.vert_to_face[v].iter().any(|&f| face_sets[f] == face_set)
}

/// `vert_has_unique_face_set()`: all faces around the vertex share one face set.
pub fn vert_has_unique_face_set(topo: &Topo, face_sets: &[i32], v: usize) -> bool {
    if face_sets.is_empty() {
        return true;
    }
    let mut it = topo.vert_to_face[v].iter().map(|&f| face_sets[f]);
    match it.next() {
        None => true,
        Some(first) => it.all(|s| s == first),
    }
}

/// `BRUSH_AUTOMASKING_FACE_SETS`, `_BOUNDARY_EDGES`, `_BOUNDARY_FACE_SETS`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flags {
    pub face_sets: bool,
    pub boundary_edges: bool,
    pub boundary_face_sets: bool,
}

/// The per-vertex zeroing branches of `calc_vert_factors()`. `initial_face_set` is the face set under the stroke start;
/// `ignore_unique` is the Draw Face Sets exception of the boundary face set branch.
pub fn apply_vert_factor(
    topo: &Topo,
    face_sets: &[i32],
    flags: Flags,
    initial_face_set: i32,
    ignore_unique: bool,
    v: usize,
    factor: &mut f32,
) {
    if flags.face_sets && initial_face_set != NONE_ID && !vert_has_face_set(topo, face_sets, v, initial_face_set) {
        *factor = 0.0;
    }
    if flags.boundary_edges && topo.is_boundary(v) {
        *factor = 0.0;
    }
    if flags.boundary_face_sets && !ignore_unique && !vert_has_unique_face_set(topo, face_sets, v) {
        *factor = 0.0;
    }
}

/// `fill_constrained_topology_automasking_factors_mesh()`: connected vertices reached through the brush radius get 1.
/// `initial` are the start vertices, `inside` the radius (and symmetry) test of the destination vertex.
pub fn constrained_topology(topo: &Topo, initial: &[usize], inside: impl Fn(usize) -> bool, factors: &mut [f32]) {
    let mut visited = vec![false; factors.len()];
    let mut queue: std::collections::VecDeque<usize> = initial.iter().copied().collect();
    while let Some(from) = queue.pop_front() {
        for to in topo.neighbors(from) {
            if visited[to] || topo.hide_vert[to] {
                continue;
            }
            visited[to] = true;
            factors[from] = 1.0;
            factors[to] = 1.0;
            if inside(to) {
                queue.push_back(to);
            }
        }
    }
}

/// `BoundaryAutomaskMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryMode {
    Edges,
    FaceSets,
}

/// `init_boundary_masking_mesh()`: vertices within `propagation_steps` of a boundary fade out quadratically.
pub fn init_boundary_masking(topo: &Topo, face_sets: &[i32], mode: BoundaryMode, propagation_steps: i32, factors: &mut [f32]) {
    const INF: i32 = -1;
    let n = factors.len();
    let mut dist = vec![INF; n];
    for (i, d) in dist.iter_mut().enumerate() {
        let on = match mode {
            BoundaryMode::Edges => topo.is_boundary(i),
            BoundaryMode::FaceSets => !vert_has_unique_face_set(topo, face_sets, i),
        };
        if on {
            *d = 0;
        }
    }
    for it in 0..propagation_steps {
        for i in 0..n {
            if dist[i] != INF {
                continue;
            }
            for nb in topo.neighbors(i) {
                if dist[nb] == it {
                    dist[i] = it + 1;
                }
            }
        }
    }
    for i in 0..n {
        if dist[i] == INF {
            continue;
        }
        let p = 1.0 - dist[i] as f32 / propagation_steps as f32;
        factors[i] *= 1.0 - p * p;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;

    /// 4x4 grid, left half of the faces is set 1, right half set 2.
    fn split_grid() -> (Mesh, Vec<i32>) {
        let m = Mesh::grid(4, 4, 2.0);
        let sets = m
            .faces
            .iter()
            .map(|f| {
                let cx: f32 = f.iter().map(|&v| m.verts[v as usize][0]).sum::<f32>() / f.len() as f32;
                if cx < 0.0 { 1 } else { 2 }
            })
            .collect();
        (m, sets)
    }

    #[test]
    fn unique_face_set_is_false_on_the_seam_only() {
        let (m, sets) = split_grid();
        let t = Topo::new(&m);
        for v in 0..m.verts.len() {
            let on_seam = m.verts[v][0].abs() < 1e-4;
            assert_eq!(vert_has_unique_face_set(&t, &sets, v), !on_seam);
        }
        assert!(vert_has_unique_face_set(&t, &[], 0));
        assert!(vert_has_face_set(&t, &[], 0, NONE_ID));
        assert!(!vert_has_face_set(&t, &[], 0, 1));
    }

    #[test]
    fn face_set_mode_zeroes_vertices_outside_the_initial_set() {
        let (m, sets) = split_grid();
        let t = Topo::new(&m);
        let flags = Flags { face_sets: true, ..Default::default() };
        for v in 0..m.verts.len() {
            let mut f = 1.0;
            apply_vert_factor(&t, &sets, flags, 1, false, v, &mut f);
            // Seam vertices touch both sets, so only x > 0 is cut.
            assert_eq!(f == 0.0, m.verts[v][0] > 1e-4);
        }
        let mut f = 1.0;
        apply_vert_factor(&t, &sets, flags, NONE_ID, false, 0, &mut f);
        assert_eq!(f, 1.0);
    }

    #[test]
    fn boundary_edges_mode_zeroes_the_open_boundary() {
        let m = Mesh::grid(4, 4, 2.0);
        let t = Topo::new(&m);
        let flags = Flags { boundary_edges: true, ..Default::default() };
        let zeroed = (0..m.verts.len())
            .filter(|&v| {
                let mut f = 1.0;
                apply_vert_factor(&t, &[], flags, NONE_ID, false, v, &mut f);
                f == 0.0
            })
            .count();
        assert_eq!(zeroed, 16);
    }

    #[test]
    fn boundary_face_sets_mode_zeroes_the_seam_unless_ignored() {
        let (m, sets) = split_grid();
        let t = Topo::new(&m);
        let flags = Flags { boundary_face_sets: true, ..Default::default() };
        let seam = (0..m.verts.len()).find(|&v| m.verts[v][0].abs() < 1e-4).unwrap();
        let mut f = 1.0;
        apply_vert_factor(&t, &sets, flags, 1, false, seam, &mut f);
        assert_eq!(f, 0.0);
        let mut f = 1.0;
        apply_vert_factor(&t, &sets, flags, 1, true, seam, &mut f);
        assert_eq!(f, 1.0);
    }

    #[test]
    fn topology_fill_stops_at_the_radius() {
        let m = Mesh::grid(8, 1, 4.0);
        let t = Topo::new(&m);
        let mut f = vec![0.0; m.verts.len()];
        let start = (0..m.verts.len()).min_by(|&a, &b| m.verts[a][0].partial_cmp(&m.verts[b][0]).unwrap()).unwrap();
        let x0 = m.verts[start][0];
        constrained_topology(&t, &[start], |v| m.verts[v][0] - x0 < 1.2, &mut f);
        let reached = f.iter().filter(|&&x| x == 1.0).count();
        assert!(reached > 2 && reached < m.verts.len());
    }

    #[test]
    fn boundary_masking_fades_toward_the_boundary() {
        let m = Mesh::grid(8, 8, 4.0);
        let t = Topo::new(&m);
        let mut f = vec![1.0; m.verts.len()];
        init_boundary_masking(&t, &[], BoundaryMode::Edges, 2, &mut f);
        let at = |x: f32, y: f32| (0..m.verts.len()).find(|&v| (m.verts[v][0] - x).abs() < 1e-4 && (m.verts[v][1] - y).abs() < 1e-4).unwrap();
        assert_eq!(f[at(-2.0, 0.0)], 0.0); // on the boundary: p = 1
        assert!((f[at(-1.5, 0.0)] - 0.75).abs() < 1e-5); // one step: 1 - 0.5^2
        assert_eq!(f[at(0.0, 0.0)], 1.0); // beyond the propagation range
    }
}
