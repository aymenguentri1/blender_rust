//! Poke Faces of an edit-mesh selection (`MESH_OT_poke`).
//!
//! Ported from `editmesh_tools.cc` `edbm_poke_face_exec()` (5384) and `bmo_poke.cc` `bmo_poke_exec()` (32), with the
//! centers of `bmesh_polygon.cc` (`BM_face_calc_center_median_weighted` 668, `BM_face_calc_center_median` 655,
//! `BM_face_calc_center_bounds` 618):
//!  - each input face gets a new center vertex and is replaced by a fan of triangles `(v_i, v_i+1, center)`;
//!  - the center moves along the face normal by `offset * offset_fac`, `offset_fac` being 1, or with
//!    `use_relative_offset` the mean distance from the unmoved center to the face vertices;
//!  - the new vertices are appended in input face order, the new triangles appended after the untouched faces
//!    (the killed faces leave the mesh).
//!    Not ported: loop attribute / multires interpolation (`Mesh` has none).

use crate::mesh::Mesh;

/// `BMOP_POKE_*`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CenterMode {
    MedianWeighted,
    Median,
    Bounds,
}

pub struct Poked {
    pub mesh: Mesh,
    /// Indices of the created center vertices (`verts.out`).
    pub new_verts: Vec<u32>,
    /// Indices of the created triangles (`faces.out`).
    pub new_faces: Vec<u32>,
}

fn len(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn center(mesh: &Mesh, f: usize, mode: CenterMode) -> [f32; 3] {
    let face = &mesh.faces[f];
    let co = |k: usize| mesh.verts[face[k] as usize];
    let n = face.len();
    match mode {
        CenterMode::Median => mesh.face_center(f),
        CenterMode::Bounds => {
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for k in 0..n {
                for a in 0..3 {
                    lo[a] = lo[a].min(co(k)[a]);
                    hi[a] = hi[a].max(co(k)[a]);
                }
            }
            [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5]
        }
        CenterMode::MedianWeighted => {
            let mut c = [0.0f32; 3];
            let mut totw = 0.0;
            let mut w_prev = len(co(n - 1), co(0));
            for k in 0..n {
                let w_curr = len(co(k), co((k + 1) % n));
                let w = w_curr + w_prev;
                for a in 0..3 {
                    c[a] += co(k)[a] * w;
                }
                totw += w;
                w_prev = w_curr;
            }
            if totw != 0.0 {
                for a in &mut c {
                    *a *= 1.0 / totw;
                }
            }
            c
        }
    }
}

/// `poke` on the faces `faces` (indices; repeats and out of range ignored). `None` when nothing valid is selected
/// (`bm->totfacesel == 0` -> `continue`).
pub fn poke_faces(mesh: &Mesh, faces: &[u32], offset: f32, use_relative_offset: bool, mode: CenterMode) -> Option<Poked> {
    let mut input = vec![false; mesh.faces.len()];
    for &f in faces {
        if let Some(x) = input.get_mut(f as usize) {
            *x = true;
        }
    }
    if !input.iter().any(|&x| x) {
        return None;
    }
    let mut out = Mesh { verts: mesh.verts.clone(), faces: Vec::new() };
    for (i, f) in mesh.faces.iter().enumerate() {
        if !input[i] {
            out.faces.push(f.clone());
        }
    }
    let (mut new_verts, mut new_faces) = (Vec::new(), Vec::new());
    for i in (0..mesh.faces.len()).filter(|&i| input[i]) {
        let face = &mesh.faces[i];
        let c = center(mesh, i, mode);
        let vc = out.verts.len() as u32;
        let mut offset_fac = if use_relative_offset { 0.0 } else { 1.0 };
        for k in 0..face.len() {
            new_faces.push(out.faces.len() as u32);
            out.faces.push(vec![face[k], face[(k + 1) % face.len()], vc]);
            if use_relative_offset {
                offset_fac += len(c, mesh.verts[face[k] as usize]);
            }
        }
        if use_relative_offset {
            offset_fac /= face.len() as f32;
        }
        let no = mesh.face_normal(i);
        let s = offset * offset_fac;
        out.verts.push([c[0] + no[0] * s, c[1] + no[1] * s, c[2] + no[2] * s]);
        new_verts.push(vc);
    }
    Some(Poked { mesh: out, new_verts, new_faces })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5)
    }

    fn quad() -> Mesh {
        Mesh {
            verts: vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0], [5.0, 0.0, 0.0]],
            faces: vec![vec![0, 1, 2, 3], vec![1, 4, 2]],
        }
    }

    #[test]
    fn a_quad_becomes_four_triangles_around_a_new_center() {
        let p = poke_faces(&quad(), &[0], 0.0, false, CenterMode::Median).unwrap();
        assert_eq!(p.new_verts, vec![5]);
        assert!(close(p.mesh.verts[5], [1.0, 1.0, 0.0]));
        assert_eq!(p.mesh.faces[0], vec![1, 4, 2], "the unselected face is kept, first");
        assert_eq!(p.new_faces, vec![1, 2, 3, 4]);
        assert_eq!(p.mesh.faces[1], vec![0, 1, 5]);
        assert_eq!(p.mesh.faces[4], vec![3, 0, 5]);
        assert_eq!(p.mesh.faces.len(), 5);
    }

    #[test]
    fn the_offset_moves_the_center_along_the_normal_and_relative_scales_it() {
        let p = poke_faces(&quad(), &[0], 0.5, false, CenterMode::Median).unwrap();
        assert!(close(p.mesh.verts[5], [1.0, 1.0, 0.5]), "the quad's normal is +Z");
        let p = poke_faces(&quad(), &[0], 0.5, true, CenterMode::Median).unwrap();
        let r = 2f32.sqrt();
        assert!(close(p.mesh.verts[5], [1.0, 1.0, 0.5 * r]), "mean center-to-vertex distance is sqrt 2");
    }

    #[test]
    fn center_modes_differ_on_a_skewed_face() {
        /* Triangle with a long edge: weighted median leans towards the long edge, bounds is the box middle. */
        let m = Mesh { verts: vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 1.0, 0.0]], faces: vec![vec![0, 1, 2]] };
        let med = poke_faces(&m, &[0], 0.0, false, CenterMode::Median).unwrap().mesh.verts[3];
        let bnd = poke_faces(&m, &[0], 0.0, false, CenterMode::Bounds).unwrap().mesh.verts[3];
        let wgt = poke_faces(&m, &[0], 0.0, false, CenterMode::MedianWeighted).unwrap().mesh.verts[3];
        assert!(close(med, [4.0 / 3.0, 1.0 / 3.0, 0.0]));
        assert!(close(bnd, [2.0, 0.5, 0.0]));
        assert!(wgt[0] != med[0] && wgt[0] != bnd[0]);
    }

    #[test]
    fn repeats_out_of_range_and_empty_selection() {
        assert!(poke_faces(&quad(), &[], 0.0, false, CenterMode::Median).is_none());
        assert!(poke_faces(&quad(), &[9], 0.0, false, CenterMode::Median).is_none());
        let p = poke_faces(&quad(), &[1, 1], 0.0, false, CenterMode::Median).unwrap();
        assert_eq!((p.new_verts.len(), p.mesh.faces.len()), (1, 4));
    }
}
