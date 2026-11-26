//! OBJ text writer for one mesh object: `v`, `vt`, `vn`, `f` lines with Blender's number formats and normal/UV
//! deduplication (io/wavefront_obj/exporter/obj_export_file_writer.cc:194-450, obj_export_io.hh:67-123,
//! obj_export_mesh.cc:273-360). Not ported: smoothing groups, vertex/material groups, vertex colors, world transform
//! and axis conversion, edges and curves, index offsets across several objects (single object: offsets are 0).

#![allow(dead_code)]

use crate::mesh::{Mesh, V3};
use std::collections::HashMap;
use std::fmt::Write;

/// `normals_domain()` choices of `store_normal_coords_and_indices` that a plain `Mesh` can supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalDomain {
    /// One normal per face (flat shading).
    Face,
    /// One normal per vertex (smooth shading).
    Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjParams {
    pub export_normals: bool,
    pub export_uv: bool,
    pub normal_domain: NormalDomain,
}

impl Default for ObjParams {
    /// `OBJExportParams` defaults: normals and UVs on.
    fn default() -> Self {
        ObjParams { export_normals: true, export_uv: true, normal_domain: NormalDomain::Face }
    }
}

/// `round_float_to_n_digits` (obj_export_mesh.cc:308).
#[allow(clippy::excessive_precision)] /* the constant is Blender's `0.49999999f` */
fn round_digits(f: f32, digits: i32) -> f32 {
    let scale = 10f32.powi(digits);
    (scale * f - 0.499_999_99).ceil() / scale
}

/// `VectorSet`-style unique list keyed by the bit pattern of the components.
#[derive(Default)]
struct Unique<const N: usize> {
    items: Vec<[f32; N]>,
    index: HashMap<[u32; N], usize>,
}

impl<const N: usize> Unique<N> {
    fn index_of_or_add(&mut self, v: [f32; N]) -> usize {
        let key = v.map(f32::to_bits);
        *self.index.entry(key).or_insert_with(|| {
            self.items.push(v);
            self.items.len() - 1
        })
    }
}

/// Writes `o name`, all `v`/`vt`/`vn` lines and then the `f` lines. `uvs` holds one UV per face corner in face order
/// (`None` or empty: no UV layer). Spaces in the name become underscores (`write_object_name`).
pub fn write_obj(name: &str, mesh: &Mesh, uvs: Option<&[[f32; 2]]>, params: &ObjParams, flip: bool) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "o {}", name.replace(' ', "_"));
    for v in &mesh.verts {
        let _ = writeln!(out, "v {:.6} {:.6} {:.6}", v[0], v[1], v[2]);
    }

    /* `store_uv_coords_and_indices`: unique UVs in first-seen order. */
    let mut uv_unique = Unique::<2>::default();
    let mut corner_uv: Vec<usize> = Vec::new();
    let corners: usize = mesh.faces.iter().map(Vec::len).sum();
    let uvs = uvs.filter(|u| !u.is_empty());
    if let Some(uvs) = uvs {
        assert_eq!(uvs.len(), corners, "one UV per face corner");
        corner_uv = uvs.iter().map(|&uv| uv_unique.index_of_or_add(uv)).collect();
    }
    let has_uv = params.export_uv && !uv_unique.items.is_empty();
    if has_uv {
        for uv in &uv_unique.items {
            let _ = writeln!(out, "vt {:.6} {:.6}", uv[0], uv[1]);
        }
    }

    /* `store_normal_coords_and_indices`: normalized, rounded to 4 digits, unique. */
    let mut n_unique = Unique::<3>::default();
    let mut corner_n: Vec<usize> = Vec::new();
    if params.export_normals {
        let mut add = |n: V3| n_unique.index_of_or_add(n.map(|c| round_digits(c, 4)));
        match params.normal_domain {
            NormalDomain::Face => {
                for (f, face) in mesh.faces.iter().enumerate() {
                    let i = add(mesh.face_normal(f));
                    corner_n.extend(std::iter::repeat_n(i, face.len()));
                }
            }
            NormalDomain::Point => {
                let vn = mesh.vertex_normals();
                let idx: Vec<usize> = vn.iter().map(|&n| add(n)).collect();
                for face in &mesh.faces {
                    corner_n.extend(face.iter().map(|&v| idx[v as usize]));
                }
            }
        }
        for n in &n_unique.items {
            let _ = writeln!(out, "vn {:.4} {:.4} {:.4}", n[0], n[1], n[2]);
        }
    }

    /* `write_face_elements` + `write_vert_*_indices` (1-based; `flip` keeps the first corner, then goes backwards). */
    let mut corner = 0;
    for face in &mesh.faces {
        let n = face.len();
        out.push('f');
        for k in 0..n {
            let j = if flip && k != 0 { n - k } else { k };
            let v = face[j] + 1;
            let c = corner + j;
            let _ = match (params.export_normals, has_uv) {
                (true, true) => write!(out, " {}/{}/{}", v, corner_uv[c] + 1, corner_n[c] + 1),
                (true, false) => write!(out, " {}//{}", v, corner_n[c] + 1),
                (false, true) => write!(out, " {}/{}", v, corner_uv[c] + 1),
                (false, false) => write!(out, " {}", v),
            };
        }
        out.push('\n');
        corner += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri_quad() -> Mesh {
        Mesh {
            verts: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            faces: vec![vec![0, 1, 2, 3], vec![0, 4, 1]],
        }
    }

    #[test]
    fn rounds_like_blender() {
        assert_eq!(round_digits(0.70710678, 4), 0.7071);
        assert_eq!(round_digits(-0.00001, 4), 0.0);
        assert_eq!(round_digits(1.0, 4), 1.0);
    }

    #[test]
    fn vertices_use_six_digits_and_faces_are_one_based() {
        let p = ObjParams { export_normals: false, export_uv: false, ..Default::default() };
        let s = write_obj("My Cube", &tri_quad(), None, &p, false);
        assert!(s.starts_with("o My_Cube\nv 0.000000 0.000000 0.000000\nv 1.000000 0.000000 0.000000\n"));
        assert!(s.ends_with("f 1 2 3 4\nf 1 5 2\n"));
    }

    #[test]
    fn flat_normals_are_deduplicated() {
        let m = Mesh { verts: tri_quad().verts, faces: vec![vec![0, 1, 2], vec![0, 2, 3]] };
        let s = write_obj("a", &m, None, &ObjParams::default(), false);
        assert_eq!(s.matches("vn -0.0000 -0.0000 1.0000\n").count(), 1, "{s}"); /* ceil(-0.49999999) is -0, as in Blender */
        assert_eq!(s.matches("\nvn ").count(), 1);
        assert!(s.ends_with("f 1//1 2//1 3//1\nf 1//1 3//1 4//1\n"), "{s}");
    }

    #[test]
    fn uvs_are_deduplicated_and_written_with_normals() {
        let m = Mesh { verts: tri_quad().verts, faces: vec![vec![0, 1, 2], vec![0, 2, 3]] };
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let s = write_obj("a", &m, Some(&uv), &ObjParams::default(), false);
        assert_eq!(s.matches("\nvt ").count(), 4);
        assert!(s.contains("vt 0.000000 1.000000\n"));
        assert!(s.ends_with("f 1/1/1 2/2/1 3/3/1\nf 1/1/1 3/3/1 4/4/1\n"), "{s}");
        let p = ObjParams { export_normals: false, ..Default::default() };
        assert!(write_obj("a", &m, Some(&uv), &p, false).ends_with("f 1/1 2/2 3/3\nf 1/1 3/3 4/4\n"));
        let p = ObjParams { export_uv: false, ..Default::default() };
        assert!(!write_obj("a", &m, Some(&uv), &p, false).contains("vt "));
    }

    #[test]
    fn flip_keeps_first_corner_and_reverses_the_rest() {
        let p = ObjParams { export_normals: false, export_uv: false, ..Default::default() };
        let s = write_obj("a", &tri_quad(), None, &p, true);
        assert!(s.ends_with("f 1 4 3 2\nf 1 2 5\n"), "{s}");
    }

    #[test]
    fn smooth_normals_are_per_vertex() {
        let p = ObjParams { normal_domain: NormalDomain::Point, export_uv: false, ..Default::default() };
        let s = write_obj("a", &Mesh::cube(2.0), None, &p, false);
        assert_eq!(s.matches("\nvn ").count(), 8);
    }
}
