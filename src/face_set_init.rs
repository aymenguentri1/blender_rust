//! `SCULPT_OT_face_sets_init` (sculpt_face_set.cc:613-895, `init_op_exec` + `init_flood_fill`): new face sets from loose
//! parts, materials, normals, UV seams, creases, sharp edges, bevel weight or the existing face set boundaries.
//! Mesh PBVH only. Edge attributes are indexed like `Mesh::edges()` (unique edges in order of first appearance).
//! Undo, PBVH tagging and the `face_set_overlay_check()` side effect are not ported.

#![allow(dead_code)] // not wired into the app yet

use crate::mesh::Mesh;
use std::collections::{HashMap, VecDeque};

/// `InitMode` (the RNA ids are LOOSE_PARTS, MATERIALS, NORMALS, UV_SEAMS, CREASES, SHARP_EDGES, BEVEL_WEIGHT,
/// FACE_SET_BOUNDARIES).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitMode {
    LooseParts,
    Materials,
    Normals,
    UvSeams,
    Creases,
    SharpEdges,
    BevelWeight,
    FaceSetBoundaries,
}

impl InitMode {
    pub fn from_rna(id: &str) -> Option<InitMode> {
        Some(match id {
            "LOOSE_PARTS" => InitMode::LooseParts,
            "MATERIALS" => InitMode::Materials,
            "NORMALS" => InitMode::Normals,
            "UV_SEAMS" => InitMode::UvSeams,
            "CREASES" => InitMode::Creases,
            "SHARP_EDGES" => InitMode::SharpEdges,
            "BEVEL_WEIGHT" => InitMode::BevelWeight,
            "FACE_SET_BOUNDARIES" => InitMode::FaceSetBoundaries,
            _ => return None,
        })
    }
}

/// RNA default of `threshold` (range 0..=1).
pub const THRESHOLD_DEFAULT: f32 = 0.5;

/// Attributes the operator reads; `None` is an absent attribute (reads as 0 / false).
#[derive(Default)]
pub struct InitInput<'a> {
    pub hide_poly: Option<&'a [bool]>,
    pub material_index: Option<&'a [i32]>,
    pub uv_seam: Option<&'a [bool]>,
    pub crease_edge: Option<&'a [f32]>,
    pub sharp_edge: Option<&'a [bool]>,
    pub bevel_weight_edge: Option<&'a [f32]>,
    pub threshold: f32,
}

fn face_normal(mesh: &Mesh, f: usize) -> [f32; 3] {
    /* Newell normal, normalized (`mesh->face_normals()`). */
    let face = &mesh.faces[f];
    let mut n = [0.0f32; 3];
    for i in 0..face.len() {
        let a = mesh.verts[face[i] as usize];
        let b = mesh.verts[face[(i + 1) % face.len()] as usize];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if l > 0.0 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0; 3] }
}

/// `init_flood_fill()`: breadth-first regions over edge-adjacent faces for which `test(from, edge, to)` holds. Hidden
/// faces are skipped, ids skip the face sets of hidden faces (`gather_hidden_face_sets()`).
fn flood_fill(mesh: &Mesh, hide_poly: Option<&[bool]>, face_sets: &mut [i32], test: impl Fn(usize, usize, usize) -> bool) {
    let hidden = |f: usize| hide_poly.is_some_and(|h| h[f]);
    let mut edge_index: HashMap<(u32, u32), usize> = HashMap::new();
    let mut edge_faces: Vec<Vec<usize>> = Vec::new();
    let mut face_edges: Vec<Vec<usize>> = Vec::with_capacity(mesh.faces.len());
    for (f, face) in mesh.faces.iter().enumerate() {
        let mut fe = Vec::new();
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            if a == b {
                continue;
            }
            let next = edge_faces.len();
            let e = *edge_index.entry((a.min(b), a.max(b))).or_insert(next);
            if e == edge_faces.len() {
                edge_faces.push(Vec::new());
            }
            edge_faces[e].push(f);
            fe.push(e);
        }
        face_edges.push(fe);
    }
    let hidden_sets: Vec<i32> = (0..mesh.faces.len()).filter(|&f| hidden(f)).map(|f| face_sets[f]).collect();
    let mut visited = vec![false; mesh.faces.len()];
    let mut next_set = 1;
    for i in 0..mesh.faces.len() {
        if hidden(i) || visited[i] {
            continue;
        }
        while hidden_sets.contains(&next_set) {
            next_set += 1;
        }
        face_sets[i] = next_set;
        visited[i] = true;
        let mut queue = VecDeque::from([i]);
        while let Some(face_i) = queue.pop_front() {
            for &e in &face_edges[face_i] {
                for &nb in &edge_faces[e] {
                    if nb == face_i || visited[nb] || hidden(nb) || !test(face_i, e, nb) {
                        continue;
                    }
                    face_sets[nb] = next_set;
                    visited[nb] = true;
                    queue.push_back(nb);
                }
            }
        }
        next_set += 1;
    }
}

/// `init_op_exec()`. `face_sets` is `.sculpt_face_set` (`None` = absent; the layer is created as all 1 first, except
/// that FaceSetBoundaries compares the all-0 copy `duplicate_face_sets()` made before that). Returns the new layer.
pub fn face_sets_init(mesh: &Mesh, face_sets: Option<Vec<i32>>, mode: InitMode, input: &InitInput) -> Vec<i32> {
    let n = mesh.faces.len();
    let copy = face_sets.clone().unwrap_or_else(|| vec![0; n]);
    let mut sets = face_sets.unwrap_or_else(|| vec![1; n]);
    let hide = input.hide_poly;
    let t = input.threshold;
    let edge_f32 = |a: Option<&[f32]>, e: usize| a.and_then(|v| v.get(e)).copied().unwrap_or(0.0);
    let edge_bool = |a: Option<&[bool]>, e: usize| a.and_then(|v| v.get(e)).copied().unwrap_or(false);
    match mode {
        InitMode::LooseParts => flood_fill(mesh, hide, &mut sets, |_, _, _| true),
        InitMode::Materials => {
            for i in 0..n {
                if hide.is_some_and(|h| h[i]) {
                    continue;
                }
                sets[i] = input.material_index.and_then(|m| m.get(i)).copied().unwrap_or(0) + 1;
            }
        }
        InitMode::Normals => {
            let normals: Vec<[f32; 3]> = (0..n).map(|f| face_normal(mesh, f)).collect();
            flood_fill(mesh, hide, &mut sets, |a, _, b| {
                let (x, y) = (normals[a], normals[b]);
                (x[0] * y[0] + x[1] * y[1] + x[2] * y[2]).abs() > t
            });
        }
        InitMode::UvSeams => flood_fill(mesh, hide, &mut sets, |_, e, _| !edge_bool(input.uv_seam, e)),
        InitMode::Creases => flood_fill(mesh, hide, &mut sets, |_, e, _| edge_f32(input.crease_edge, e) < t),
        InitMode::SharpEdges => flood_fill(mesh, hide, &mut sets, |_, e, _| !edge_bool(input.sharp_edge, e)),
        InitMode::BevelWeight => flood_fill(mesh, hide, &mut sets, |_, e, _| edge_f32(input.bevel_weight_edge, e) < t),
        InitMode::FaceSetBoundaries => flood_fill(mesh, hide, &mut sets, |a, _, b| copy[a] == copy[b]),
    }
    sets
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads sharing edge 1-2 (edge index 1 in first-appearance order: 0-1, 1-2, 2-3, 3-0, ...), plus a
    /// disconnected third quad.
    fn mesh() -> Mesh {
        Mesh {
            verts: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [9.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 1.0, 0.0], [9.0, 1.0, 0.0]],
            faces: vec![vec![0, 1, 2, 3], vec![1, 4, 5, 2], vec![6, 7, 8, 9]],
        }
    }

    fn inp() -> InitInput<'static> {
        InitInput { threshold: THRESHOLD_DEFAULT, ..Default::default() }
    }

    #[test]
    fn rna_ids_map() {
        assert_eq!(InitMode::from_rna("UV_SEAMS"), Some(InitMode::UvSeams));
        assert_eq!(InitMode::from_rna("FACE_SET_BOUNDARIES"), Some(InitMode::FaceSetBoundaries));
        assert_eq!(InitMode::from_rna("nope"), None);
    }

    #[test]
    fn loose_parts_give_one_set_per_connected_part() {
        assert_eq!(face_sets_init(&mesh(), None, InitMode::LooseParts, &inp()), vec![1, 1, 2]);
    }

    #[test]
    fn materials_use_index_plus_one_and_skip_hidden_faces() {
        let hide = [false, true, false];
        let mat = [0, 3, 2];
        let i = InitInput { hide_poly: Some(&hide), material_index: Some(&mat), ..inp() };
        assert_eq!(face_sets_init(&mesh(), Some(vec![7, 7, 7]), InitMode::Materials, &i), vec![1, 7, 3]);
        assert_eq!(face_sets_init(&mesh(), None, InitMode::Materials, &inp()), vec![1, 1, 1]);
    }

    #[test]
    fn normals_split_when_the_dot_is_not_above_the_threshold() {
        let mut m = mesh();
        m.verts[4] = [1.0, 0.0, 1.0];
        m.verts[5] = [1.0, 1.0, 1.0]; /* face 1 stands up: dot 0 */
        assert_eq!(face_sets_init(&m, None, InitMode::Normals, &inp()), vec![1, 2, 3]);
        assert_eq!(face_sets_init(&mesh(), None, InitMode::Normals, &inp()), vec![1, 1, 2]);
    }

    #[test]
    fn edge_modes_cut_at_the_shared_edge() {
        let m = mesh();
        let edges = m.edges();
        let shared = edges.iter().position(|e| (e.a, e.b) == (1, 2)).unwrap();
        let mut seam = vec![false; edges.len()];
        seam[shared] = true;
        let i = InitInput { uv_seam: Some(&seam), ..inp() };
        assert_eq!(face_sets_init(&m, None, InitMode::UvSeams, &i), vec![1, 2, 3]);
        let i = InitInput { sharp_edge: Some(&seam), ..inp() };
        assert_eq!(face_sets_init(&m, None, InitMode::SharpEdges, &i), vec![1, 2, 3]);
        let mut w = vec![0.0; edges.len()];
        w[shared] = 0.5; /* `< threshold` is false at exactly the threshold */
        let i = InitInput { crease_edge: Some(&w), ..inp() };
        assert_eq!(face_sets_init(&m, None, InitMode::Creases, &i), vec![1, 2, 3]);
        let i = InitInput { bevel_weight_edge: Some(&w), threshold: 0.6, ..inp() };
        assert_eq!(face_sets_init(&m, None, InitMode::BevelWeight, &i), vec![1, 1, 2]);
    }

    #[test]
    fn face_set_boundaries_regroup_equal_neighbours_and_skip_hidden_ids() {
        let m = mesh();
        assert_eq!(face_sets_init(&m, Some(vec![5, 5, 5]), InitMode::FaceSetBoundaries, &inp()), vec![1, 1, 2]);
        assert_eq!(face_sets_init(&m, Some(vec![5, 6, 6]), InitMode::FaceSetBoundaries, &inp()), vec![1, 2, 3]);
        assert_eq!(face_sets_init(&m, None, InitMode::FaceSetBoundaries, &inp()), vec![1, 1, 2]);
        /* Hidden face 2 keeps set 1, so visible parts start at 2. */
        let hide = [false, false, true];
        let i = InitInput { hide_poly: Some(&hide), ..inp() };
        assert_eq!(face_sets_init(&m, Some(vec![1, 1, 1]), InitMode::LooseParts, &i), vec![2, 2, 1]);
    }
}
