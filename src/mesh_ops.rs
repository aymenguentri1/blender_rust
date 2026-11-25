//! Mesh operations behind the modifiers and the object-mode mesh operators.
//!
//! Modelled on Blender:
//!  - `MOD_mirror`: the mesh plus its reflection across the object's origin plane of an axis; vertices
//!    within the merge threshold of the plane (default 0.001 m) are shared, and the reflected faces are
//!    wound the other way so they keep facing outwards
//!  - `MOD_array`: `count` copies, each shifted by the relative offset (a factor of the mesh bounds,
//!    default 1, 0, 0) plus the constant offset
//!  - `MOD_solidify` (simple mode): a shell of `thickness` along the vertex normals, offset -1 keeps the
//!    original surface on the outside and +1 on the inside; boundary edges get a rim of quads
//!  - `OBJECT_OT_join`, `mesh.flip_normals`, `mesh.normals_make_consistent`, `mesh.quads_convert_to_tris`
//!    and `mesh.remove_doubles` (Merge by Distance)

use crate::mesh::{add, cross, dot, scale, sub, Mesh, V3};
use std::collections::HashMap;

impl Mesh {
    /// Maps every vertex through `f`; `flip` reverses the winding (use it when `f` mirrors space, a negative
    /// determinant, so the faces keep facing outwards).
    pub fn mapped(&self, f: impl Fn(V3) -> V3, flip: bool) -> Mesh {
        let mut m = Mesh { verts: self.verts.iter().map(|&v| f(v)).collect(), faces: self.faces.clone() };
        if flip {
            m.flip_normals();
        }
        m
    }

    /// Appends `other` to this mesh (the vertex indices of its faces are shifted).
    pub fn append(&mut self, other: &Mesh) {
        let base = self.verts.len() as u32;
        self.verts.extend_from_slice(&other.verts);
        self.faces.extend(other.faces.iter().map(|f| f.iter().map(|&i| i + base).collect::<Vec<u32>>()));
    }

    /// `mesh.flip_normals`: reverses the winding of every face.
    pub fn flip_normals(&mut self) {
        for f in &mut self.faces {
            f.reverse();
        }
    }

    /// The mean of the vertices, the `Median Center` of Set Origin.
    pub fn median_center(&self) -> Option<V3> {
        if self.verts.is_empty() {
            return None;
        }
        let sum = self.verts.iter().fold([0.0f32; 3], |s, &v| add(s, v));
        Some(scale(sum, 1.0 / self.verts.len() as f32))
    }

    /// The Mirror modifier across the plane `axis` (0 = X, 1 = Y, 2 = Z) through the origin.
    pub fn mirrored(&self, axis: usize, merge: bool, threshold: f32) -> Mesh {
        let n = self.verts.len();
        let mut out = self.clone();
        /* The index of the reflection of every vertex: itself when it lies on the plane and merging is on. */
        let mut mirror_of = Vec::with_capacity(n);
        for (i, v) in self.verts.iter().enumerate() {
            if merge && v[axis].abs() <= threshold {
                mirror_of.push(i as u32);
            } else {
                let mut r = *v;
                r[axis] = -r[axis];
                mirror_of.push(out.verts.len() as u32);
                out.verts.push(r);
            }
        }
        for face in &self.faces {
            if face.iter().all(|&i| mirror_of[i as usize] == i) {
                continue; /* A face on the plane would be a duplicate of itself. */
            }
            out.faces.push(face.iter().rev().map(|&i| mirror_of[i as usize]).collect());
        }
        out
    }

    /// The Array modifier with a relative offset (a factor of the bounds size) and a constant one.
    pub fn arrayed(&self, count: u32, relative: V3, constant: V3) -> Mesh {
        let count = count.max(1);
        let size = self.bounds().map_or([0.0; 3], |(mn, mx)| sub(mx, mn));
        let step = add([relative[0] * size[0], relative[1] * size[1], relative[2] * size[2]], constant);
        let mut out = self.clone();
        for k in 1..count {
            let shift = scale(step, k as f32);
            let copy = self.mapped(|v| add(v, shift), false);
            out.append(&copy);
        }
        out
    }

    /// The Solidify modifier (simple mode). `offset` is in -1..=1 as in Blender.
    pub fn solidified(&self, thickness: f32, offset: f32) -> Mesh {
        let normals = self.vertex_normals();
        let outer = thickness * (offset + 1.0) * 0.5;
        let n = self.verts.len() as u32;
        let mut out = Mesh::default();
        for (v, nrm) in self.verts.iter().zip(&normals) {
            out.verts.push(add(*v, scale(*nrm, outer)));
        }
        for (v, nrm) in self.verts.iter().zip(&normals) {
            out.verts.push(add(*v, scale(*nrm, outer - thickness)));
        }
        out.faces = self.faces.clone();
        for f in &self.faces {
            out.faces.push(f.iter().rev().map(|&i| i + n).collect());
        }
        /* Rim: a directed edge whose reverse is not in the mesh is on the boundary. */
        let mut directed: HashMap<(u32, u32), u32> = HashMap::new();
        for f in &self.faces {
            for i in 0..f.len() {
                *directed.entry((f[i], f[(i + 1) % f.len()])).or_insert(0) += 1;
            }
        }
        for f in &self.faces {
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                if !directed.contains_key(&(b, a)) {
                    out.faces.push(vec![a, a + n, b + n, b]);
                }
            }
        }
        out
    }

    /// `mesh.quads_convert_to_tris`: fan triangulation of every face with more than three corners.
    pub fn triangulated(&self) -> Mesh {
        let mut out = Mesh { verts: self.verts.clone(), faces: Vec::with_capacity(self.faces.len()) };
        for f in &self.faces {
            if f.len() <= 3 {
                out.faces.push(f.clone());
            } else {
                for k in 1..f.len() - 1 {
                    out.faces.push(vec![f[0], f[k], f[k + 1]]);
                }
            }
        }
        out
    }

    /// `mesh.remove_doubles`: welds vertices closer than `distance`, drops the faces that collapse and the
    /// vertices that no face and no merge needs any more. Returns the new mesh and the number of vertices removed.
    pub fn merged_by_distance(&self, distance: f32) -> (Mesh, usize) {
        let cell = distance.max(1e-9);
        let key = |v: V3| -> (i64, i64, i64) {
            ((v[0] / cell).floor() as i64, (v[1] / cell).floor() as i64, (v[2] / cell).floor() as i64)
        };
        let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
        let mut target: Vec<u32> = Vec::with_capacity(self.verts.len());
        let mut kept: Vec<u32> = Vec::new();
        for (i, &v) in self.verts.iter().enumerate() {
            let (cx, cy, cz) = key(v);
            let mut found = None;
            'search: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(list) = grid.get(&(cx + dx, cy + dy, cz + dz)) {
                            for &j in list {
                                let d = sub(self.verts[j as usize], v);
                                if dot(d, d) <= distance * distance {
                                    found = Some(j);
                                    break 'search;
                                }
                            }
                        }
                    }
                }
            }
            match found {
                Some(j) => target.push(target[j as usize]),
                None => {
                    target.push(kept.len() as u32);
                    kept.push(i as u32);
                    grid.entry((cx, cy, cz)).or_default().push(i as u32);
                }
            }
        }
        let mut out = Mesh { verts: kept.iter().map(|&i| self.verts[i as usize]).collect(), faces: Vec::new() };
        for f in &self.faces {
            let mut nf: Vec<u32> = Vec::with_capacity(f.len());
            for &i in f {
                let t = target[i as usize];
                if nf.last() != Some(&t) {
                    nf.push(t);
                }
            }
            while nf.len() > 1 && nf.first() == nf.last() {
                nf.pop();
            }
            let mut uniq = nf.clone();
            uniq.sort_unstable();
            uniq.dedup();
            if nf.len() >= 3 && uniq.len() == nf.len() {
                out.faces.push(nf);
            }
        }
        let removed = self.verts.len() - out.verts.len();
        (out, removed)
    }

    /// `mesh.normals_make_consistent`: gives every connected piece one winding and, when the piece is a closed
    /// surface, turns it so the faces point outwards (positive enclosed volume). Returns how many faces flipped.
    pub fn recalc_normals_outside(&mut self) -> usize {
        let nf = self.faces.len();
        let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                edge_faces.entry((a.min(b), a.max(b))).or_default().push(f);
            }
        }
        let directed = |face: &Vec<u32>, a: u32, b: u32, flipped: bool| -> bool {
            /* Does the face (with its flip state) run a -> b? */
            (0..face.len()).any(|i| {
                let (p, q) = (face[i], face[(i + 1) % face.len()]);
                if flipped { (q, p) == (a, b) } else { (p, q) == (a, b) }
            })
        };
        let mut flip: Vec<Option<bool>> = vec![None; nf];
        let mut flipped_total = 0;
        for seed in 0..nf {
            if flip[seed].is_some() {
                continue;
            }
            flip[seed] = Some(false);
            let mut comp = vec![seed];
            let mut stack = vec![seed];
            let mut closed = true;
            while let Some(f) = stack.pop() {
                let face = self.faces[f].clone();
                let fl = flip[f].unwrap();
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    let neighbours = &edge_faces[&(a.min(b), a.max(b))];
                    if neighbours.len() != 2 {
                        closed = false;
                    }
                    /* In this face's current orientation the edge runs (x, y); a consistent neighbour runs (y, x). */
                    let (x, y) = if fl { (b, a) } else { (a, b) };
                    for &g in neighbours {
                        if g == f || flip[g].is_some() {
                            continue;
                        }
                        let same = directed(&self.faces[g], x, y, false);
                        flip[g] = Some(same);
                        comp.push(g);
                        stack.push(g);
                    }
                }
            }
            if closed {
                let mut volume = 0.0f32;
                for &f in &comp {
                    let mut face = self.faces[f].clone();
                    if flip[f] == Some(true) {
                        face.reverse();
                    }
                    for k in 1..face.len() - 1 {
                        let (p, q, r) = (self.verts[face[0] as usize], self.verts[face[k] as usize], self.verts[face[k + 1] as usize]);
                        volume += dot(p, cross(q, r));
                    }
                }
                if volume < 0.0 {
                    for &f in &comp {
                        flip[f] = Some(!flip[f].unwrap());
                    }
                }
            }
        }
        for (f, fl) in flip.iter().enumerate() {
            if *fl == Some(true) {
                self.faces[f].reverse();
                flipped_total += 1;
            }
        }
        flipped_total
    }

    /// The signed volume of the mesh (positive for outward facing closed surfaces).
    pub fn signed_volume(&self) -> f32 {
        let mut v = 0.0;
        for (_, t) in self.triangles() {
            let (a, b, c) = (self.verts[t[0] as usize], self.verts[t[1] as usize], self.verts[t[2] as usize]);
            v += dot(a, cross(b, c)) / 6.0;
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_of_a_half_cube_welds_the_plane() {
        /* A 1 x 2 x 2 box from x = 0 to 1: mirroring in X gives the full 2 x 2 x 2 cube minus nothing: 12 vertices. */
        let mut half = Mesh::cube(2.0).mapped(|v| [v[0] * 0.5 + 0.5, v[1], v[2]], false);
        half.faces.retain(|f| !f.iter().all(|&i| half.verts[i as usize][0].abs() < 1e-6)); /* open the x = 0 side */
        let m = half.mirrored(0, true, 0.001);
        assert_eq!(m.verts.len(), 12);
        assert_eq!(m.faces.len(), 10);
        assert!(m.edges().iter().all(|e| e.faces == 2), "the welded mirror is closed");
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn mirror_without_merge_duplicates_every_vertex() {
        let m = Mesh::cube(2.0).mapped(|v| add(v, [3.0, 0.0, 0.0]), false).mirrored(0, true, 0.001);
        assert_eq!(m.verts.len(), 16);
        assert_eq!(m.faces.len(), 12);
        assert!(m.signed_volume() > 0.0);
        assert!((m.signed_volume() - 16.0).abs() < 1e-3);
    }

    #[test]
    fn array_uses_the_relative_offset_of_the_bounds() {
        let m = Mesh::cube(2.0).arrayed(3, [1.0, 0.0, 0.0], [0.0; 3]);
        assert_eq!(m.verts.len(), 24);
        assert_eq!(m.faces.len(), 18);
        let (mn, mx) = m.bounds().unwrap();
        assert_eq!((mn[0], mx[0]), (-1.0, 5.0));
        let m = Mesh::cube(2.0).arrayed(2, [0.0; 3], [0.0, 0.0, 5.0]);
        assert_eq!(m.bounds().unwrap().1[2], 6.0);
        assert_eq!(Mesh::cube(2.0).arrayed(0, [1.0, 0.0, 0.0], [0.0; 3]), Mesh::cube(2.0));
    }

    #[test]
    fn solidify_a_closed_mesh_makes_two_shells() {
        let c = Mesh::cube(2.0);
        let m = c.solidified(0.1, -1.0);
        assert_eq!((m.verts.len(), m.faces.len()), (16, 12));
        assert!(m.edges().iter().all(|e| e.faces == 2));
        /* Offset -1 keeps the original surface outside. */
        assert!((m.bounds().unwrap().1[0] - 1.0).abs() < 1e-6);
        let out = c.solidified(0.1, 1.0);
        assert!(out.bounds().unwrap().1[0] > 1.05);
    }

    #[test]
    fn solidify_a_plane_adds_a_rim_and_stays_closed() {
        let m = Mesh::plane(2.0).solidified(0.2, 0.0);
        assert_eq!((m.verts.len(), m.faces.len()), (8, 2 + 4));
        assert!(m.edges().iter().all(|e| e.faces == 2), "{:?}", m.edges());
        assert!(m.signed_volume() > 0.0);
        assert!((m.signed_volume() - 2.0 * 2.0 * 0.2).abs() < 1e-4);
        assert_eq!(m.bounds().unwrap().1[2], 0.1);
    }

    #[test]
    fn flipping_reverses_the_normals() {
        let mut m = Mesh::cube(2.0);
        let v = m.signed_volume();
        m.flip_normals();
        assert!((m.signed_volume() + v).abs() < 1e-5);
    }

    #[test]
    fn recalc_fixes_mixed_windings_and_inside_out_meshes() {
        let mut m = Mesh::cube(2.0);
        m.faces[2].reverse();
        let changed = m.recalc_normals_outside();
        assert_eq!(changed, 1);
        assert!((m.signed_volume() - 8.0).abs() < 1e-4);

        let mut inside_out = Mesh::uv_sphere(16, 8, 1.0);
        inside_out.flip_normals();
        assert_eq!(inside_out.recalc_normals_outside(), inside_out.faces.len());
        assert!(inside_out.signed_volume() > 0.0);

        let mut ok = Mesh::cube(2.0);
        assert_eq!(ok.recalc_normals_outside(), 0);
    }

    #[test]
    fn recalc_handles_two_pieces_independently() {
        let mut a = Mesh::cube(2.0);
        let mut b = Mesh::cube(2.0).mapped(|v| add(v, [5.0, 0.0, 0.0]), false);
        b.flip_normals();
        a.append(&b);
        assert_eq!(a.recalc_normals_outside(), 6);
        assert!((a.signed_volume() - 16.0).abs() < 1e-3);
    }

    #[test]
    fn triangulate_counts() {
        let m = Mesh::cube(2.0).triangulated();
        assert_eq!(m.faces.len(), 12);
        assert!(m.faces.iter().all(|f| f.len() == 3));
        assert!((m.signed_volume() - 8.0).abs() < 1e-4);
        assert_eq!(Mesh::circle(32, 1.0).triangulated().faces.len(), 30);
    }

    #[test]
    fn merge_by_distance_welds_a_split_cube() {
        let mut c = Mesh::cube(2.0);
        /* Split it into six separate quads: 24 vertices. */
        let mut split = Mesh::default();
        for f in &c.faces {
            let base = split.verts.len() as u32;
            for &i in f {
                split.verts.push(c.verts[i as usize]);
            }
            split.faces.push((0..f.len() as u32).map(|k| base + k).collect());
        }
        assert_eq!(split.verts.len(), 24);
        let (merged, removed) = split.merged_by_distance(0.001);
        assert_eq!((merged.verts.len(), merged.faces.len(), removed), (8, 6, 16));
        assert!(merged.edges().iter().all(|e| e.faces == 2));
        assert!((merged.signed_volume() - 8.0).abs() < 1e-4);
        let (same, removed) = c.merged_by_distance(0.001);
        assert_eq!(removed, 0);
        assert_eq!(same, c);
        c.verts[1] = c.verts[0];
        let (collapsed, removed) = c.merged_by_distance(0.001);
        assert_eq!(removed, 1);
        assert!(collapsed.faces.iter().all(|f| f.len() >= 3));
    }

    #[test]
    fn mapped_with_a_mirroring_map_keeps_faces_outward() {
        let m = Mesh::cube(2.0).mapped(|v| [-v[0], v[1], v[2]], true);
        assert!((m.signed_volume() - 8.0).abs() < 1e-4);
    }
}
