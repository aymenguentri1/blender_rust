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
//!  - `MOD_bevel` (one segment, `Offset` width): every face shrinks by the width, every edge between two
//!    faces becomes a chamfer quad and every vertex a cap polygon; the width is clamped to under half the
//!    shortest edge (Blender's `Clamp Overlap`)
//!  - `mesh.subdivide` (one cut, smoothness 0), `mesh.separate` by loose parts
//!  - `OBJECT_OT_join`, `mesh.flip_normals`, `mesh.normals_make_consistent`, `mesh.quads_convert_to_tris`
//!    and `mesh.remove_doubles` (Merge by Distance)

use crate::mesh::{add, cross, dot, normalized, scale, sub, Mesh, V3};
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

    /// Fit Length: how many copies of the Array fit in `fit` metres, `None` when the length is off (0) or the offset
    /// has no length. As in Blender it is the length over the offset length rounded down, at least 1 and at most 1000.
    pub fn array_fit_count(&self, relative: V3, constant: V3, fit: f32) -> Option<u32> {
        if !(fit > 0.0) {
            return None;
        }
        let size = self.bounds().map_or([0.0; 3], |(mn, mx)| sub(mx, mn));
        let step = add([relative[0] * size[0], relative[1] * size[1], relative[2] * size[2]], constant);
        let len = dot(step, step).sqrt();
        (len > 1e-6).then(|| (((fit + 1e-4) / len).floor() as i64).clamp(1, 1000) as u32)
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

    /// `mesh.subdivide` with one cut and no smoothing: every n-gon becomes n quads around its center.
    pub fn subdivided_simple(&self) -> Mesh {
        let mut out = Mesh { verts: self.verts.clone(), faces: Vec::new() };
        let mut mids: HashMap<(u32, u32), u32> = HashMap::new();
        for (f, face) in self.faces.iter().enumerate() {
            let k = face.len();
            let center = out.verts.len() as u32;
            out.verts.push(self.face_center(f));
            let mut mid = |a: u32, b: u32, out: &mut Mesh| -> u32 {
                *mids.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    out.verts.push(scale(add(self.verts[a as usize], self.verts[b as usize]), 0.5));
                    (out.verts.len() - 1) as u32
                })
            };
            for i in 0..k {
                let (prev, cur, next) = (face[(i + k - 1) % k], face[i], face[(i + 1) % k]);
                let (m_next, m_prev) = (mid(cur, next, &mut out), mid(prev, cur, &mut out));
                out.faces.push(vec![cur, m_next, center, m_prev]);
            }
        }
        out
    }

    /// The connected pieces of the mesh (vertices that no face uses are dropped), ordered by their first face.
    pub fn loose_parts(&self) -> Vec<Mesh> {
        let mut parent: Vec<u32> = (0..self.verts.len() as u32).collect();
        fn find(parent: &mut Vec<u32>, mut x: u32) -> u32 {
            while parent[x as usize] != x {
                parent[x as usize] = parent[parent[x as usize] as usize];
                x = parent[x as usize];
            }
            x
        }
        for face in &self.faces {
            for &v in &face[1..] {
                let (a, b) = (find(&mut parent, face[0]), find(&mut parent, v));
                if a != b {
                    parent[b as usize] = a;
                }
            }
        }
        let mut order: Vec<u32> = Vec::new();
        let mut parts: HashMap<u32, (Mesh, HashMap<u32, u32>)> = HashMap::new();
        for face in &self.faces {
            let root = find(&mut parent, face[0]);
            let (mesh, remap) = parts.entry(root).or_insert_with(|| {
                order.push(root);
                (Mesh::default(), HashMap::new())
            });
            let new_face: Vec<u32> = face
                .iter()
                .map(|&v| {
                    *remap.entry(v).or_insert_with(|| {
                        mesh.verts.push(self.verts[v as usize]);
                        (mesh.verts.len() - 1) as u32
                    })
                })
                .collect();
            mesh.faces.push(new_face);
        }
        order.into_iter().map(|r| parts.remove(&r).unwrap().0).collect()
    }

    /// The Bevel modifier with one segment: a chamfer of `width` on every edge between two faces and a cap on
    /// every closed vertex. Open boundaries are not bevelled (their faces still shrink).
    #[cfg(test)]
    pub fn beveled(&self, width: f32) -> Mesh {
        self.beveled_segments(width, 1)
    }

    /// The Bevel modifier with `segments` segments: the chamfer strip of every edge and the rim of every cap get
    /// `segments - 1` more vertices on a quadratic curve that bulges towards the original edge (one segment is the
    /// plain chamfer).
    pub fn beveled_segments(&self, width: f32, segments: u32) -> Mesh {
        let segments = segments.clamp(1, 32) as usize;
        let shortest = self.edges().iter().map(|e| {
            let d = sub(self.verts[e.a as usize], self.verts[e.b as usize]);
            dot(d, d).sqrt()
        }).fold(f32::INFINITY, f32::min);
        let w = width.min(0.49 * shortest);
        if !(w > 1e-9) || !shortest.is_finite() {
            return self.clone();
        }
        /* One new vertex per face corner. */
        let mut base = Vec::with_capacity(self.faces.len());
        let mut out = Mesh::default();
        for (f, face) in self.faces.iter().enumerate() {
            base.push(out.verts.len() as u32);
            let n = self.face_normal(f);
            let k = face.len();
            for i in 0..k {
                let v = self.verts[face[i] as usize];
                let a = normalized(sub(self.verts[face[(i + k - 1) % k] as usize], v));
                let b = normalized(sub(self.verts[face[(i + 1) % k] as usize], v));
                let c = cross(a, b);
                let sin = dot(c, c).sqrt();
                let p = if sin < 1e-3 {
                    v
                } else {
                    /* Convex corners move along the bisector into the face, reflex ones the other way. */
                    let dir = if dot(c, n) <= 0.0 { 1.0 } else { -1.0 };
                    add(v, scale(add(a, b), dir * w / sin))
                };
                out.verts.push(p);
            }
            out.faces.push((0..k as u32).map(|i| base[f] + i).collect());
        }
        let corner = |f: usize, i: usize| base[f] + (i % self.faces[f].len()) as u32;
        /* Directed edges -> (face, index); an edge used twice in the same direction is non-manifold. */
        let mut directed: HashMap<(u32, u32), Option<(usize, usize)>> = HashMap::new();
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let key = (face[i], face[(i + 1) % face.len()]);
                directed.entry(key).and_modify(|e| *e = None).or_insert(Some((f, i)));
            }
        }
        /* The vertices strictly between the two corners of an edge end, in the order of the strip: the key is the
         * edge (a < b) and whether the end is `b`. */
        let mut inner: HashMap<(u32, u32, bool), Vec<u32>> = HashMap::new();
        let mut rim = |out: &mut Mesh, key: (u32, u32, bool), p: u32, q: u32, v: V3| -> Vec<u32> {
            let mut row = vec![p];
            let mut mids = Vec::new();
            for k in 1..segments {
                let t = k as f32 / segments as f32;
                let (pp, qq) = (out.verts[p as usize], out.verts[q as usize]);
                let pos: V3 = std::array::from_fn(|c| (1.0 - t) * (1.0 - t) * pp[c] + 2.0 * t * (1.0 - t) * v[c] + t * t * qq[c]);
                let id = out.verts.len() as u32;
                out.verts.push(pos);
                mids.push(id);
            }
            row.extend(mids.iter().copied());
            row.push(q);
            inner.insert(key, mids);
            row
        };
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                if a > b {
                    continue;
                }
                if let (Some(Some((_, _))), Some(Some((g, j)))) = (directed.get(&(a, b)), directed.get(&(b, a))) {
                    let row_a = rim(&mut out, (a, b, false), corner(f, i), corner(*g, *j + 1), self.verts[a as usize]);
                    let row_b = rim(&mut out, (a, b, true), corner(f, i + 1), corner(*g, *j), self.verts[b as usize]);
                    for k in 0..segments {
                        out.faces.push(vec![row_b[k], row_a[k], row_a[k + 1], row_b[k + 1]]);
                    }
                }
            }
        }
        /* The cap of every vertex: walk the faces around it through their incoming edges. */
        let mut corners_of: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
        for (f, face) in self.faces.iter().enumerate() {
            for (i, &v) in face.iter().enumerate() {
                corners_of.entry(v).or_default().push((f, i));
            }
        }
        let mut verts_sorted: Vec<&u32> = corners_of.keys().collect();
        verts_sorted.sort();
        for &v in verts_sorted {
            let list = &corners_of[&v];
            let (f0, i0) = list[0];
            let mut cap = vec![corner(f0, i0)];
            let mut corners = 1;
            let (mut f, mut i) = (f0, i0);
            let mut closed = false;
            for _ in 0..list.len() {
                let face = &self.faces[f];
                let prev = face[(i + face.len() - 1) % face.len()];
                match directed.get(&(v, prev)) {
                    Some(Some((g, j))) => {
                        /* The rim of the edge between the two corners, from this face's corner to the next one. */
                        let (a, b) = if v < prev { (v, prev) } else { (prev, v) };
                        if let Some(mids) = inner.get(&(a, b, v == b)) {
                            if v == b {
                                cap.extend(mids.iter().copied());
                            } else {
                                cap.extend(mids.iter().rev().copied());
                            }
                        }
                        (f, i) = (*g, *j);
                        if (f, i) == (f0, i0) {
                            closed = true;
                            break;
                        }
                        cap.push(corner(f, i));
                        corners += 1;
                    }
                    _ => break,
                }
            }
            if closed && corners == list.len() && corners >= 3 {
                out.faces.push(cap);
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
    #[test]
    fn simple_subdivide_counts_and_keeps_the_shape() {
        let m = Mesh::cube(2.0).subdivided_simple();
        assert_eq!((m.verts.len(), m.faces.len()), (26, 24));
        assert!(m.faces.iter().all(|f| f.len() == 4));
        assert!(m.edges().iter().all(|e| e.faces == 2));
        assert!((m.signed_volume() - 8.0).abs() < 1e-4);
        let t = Mesh::circle(8, 1.0).subdivided_simple();
        assert_eq!(t.faces.len(), 8);
        let twice = Mesh::cube(2.0).subdivided_simple().subdivided_simple();
        assert_eq!(twice.faces.len(), 96);
    }

    #[test]
    fn loose_parts_split_disconnected_pieces() {
        let mut m = Mesh::cube(2.0);
        m.append(&Mesh::cube(1.0).mapped(|v| add(v, [5.0, 0.0, 0.0]), false));
        m.append(&Mesh::plane(1.0).mapped(|v| add(v, [0.0, 5.0, 0.0]), false));
        m.verts.push([9.0, 9.0, 9.0]); /* a loose vertex is dropped */
        let parts = m.loose_parts();
        assert_eq!(parts.len(), 3);
        assert_eq!((parts[0].verts.len(), parts[0].faces.len()), (8, 6));
        assert_eq!((parts[1].verts.len(), parts[1].faces.len()), (8, 6));
        assert_eq!((parts[2].verts.len(), parts[2].faces.len()), (4, 1));
        assert!((parts[1].signed_volume() - 1.0).abs() < 1e-4);
        assert_eq!(Mesh::cube(2.0).loose_parts().len(), 1);
        assert!(Mesh::default().loose_parts().is_empty());
    }

    #[test]
    fn bevel_a_cube_is_a_closed_chamfered_solid() {
        let m = Mesh::cube(2.0).beveled(0.2);
        /* 24 corners, 6 + 12 + 8 faces, and Euler: 24 - 48 + 26 = 2. */
        assert_eq!((m.verts.len(), m.faces.len()), (24, 26));
        let edges = m.edges();
        assert!(edges.iter().all(|e| e.faces == 2));
        assert_eq!(m.verts.len() as i32 - edges.len() as i32 + m.faces.len() as i32, 2);
        let v = m.signed_volume();
        assert!(v > 0.0 && v < 8.0, "{v}");
        /* The original faces keep their planes, shrunk by the width on every side. */
        let (mn, mx) = m.bounds().unwrap();
        assert!(mn.iter().chain(mx.iter()).all(|c| (c.abs() - 1.0).abs() < 1e-5));
        /* Each of the 12 chamfers cuts at most a prism with a right triangle of legs 0.2 and length 2. */
        assert!(8.0 - v < 12.0 * 0.5 * 0.2 * 0.2 * 2.0 + 1e-3);
        assert!(m.faces.iter().filter(|f| f.len() == 3).count() == 8, "triangular vertex caps");
    }

    #[test]
    fn bevel_width_is_clamped_and_zero_is_a_noop() {
        let huge = Mesh::cube(2.0).beveled(50.0);
        assert!(huge.signed_volume() > 0.0);
        assert!(huge.verts.iter().all(|v| v.iter().all(|c| c.abs() <= 1.0 + 1e-5)));
        assert_eq!(Mesh::cube(2.0).beveled(0.0), Mesh::cube(2.0));
        assert_eq!(Mesh::default().beveled(0.1), Mesh::default());
    }

    #[test]
    fn bevel_an_open_mesh_only_insets() {
        let m = Mesh::plane(2.0).beveled(0.1);
        assert_eq!((m.verts.len(), m.faces.len()), (4, 1));
        assert!((m.bounds().unwrap().1[0] - 0.9).abs() < 1e-5);
    }

    #[test]
    fn bevel_keeps_a_sphere_closed_and_outward() {
        let m = Mesh::ico_sphere(2, 1.0).beveled(0.02);
        assert!(m.edges().iter().all(|e| e.faces == 2));
        assert!(m.signed_volume() > 0.0);
    }

    #[test]
    fn a_segmented_bevel_of_a_cube_is_closed_and_round() {
        let c = Mesh::cube(2.0);
        let one = c.beveled_segments(0.2, 1);
        assert_eq!(one, c.beveled(0.2));
        for n in [2usize, 3, 4] {
            let m = c.beveled_segments(0.2, n as u32);
            assert_eq!(m.verts.len(), 24 + 12 * 2 * (n - 1), "{n} segments");
            assert_eq!(m.faces.len(), 6 + 12 * n + 8, "{n} segments");
            assert!(m.edges().iter().all(|e| e.faces == 2), "{n} segments: not closed");
            assert!(m.signed_volume() > 0.0 && m.signed_volume() < 8.0, "{n}: {}", m.signed_volume());
        }
        /* More segments follow the curve, so the volume grows towards the original (the curve bulges outwards). */
        let v1 = c.beveled_segments(0.2, 1).signed_volume();
        let v4 = c.beveled_segments(0.2, 4).signed_volume();
        assert!(v4 > v1, "{v4} <= {v1}");
    }

    #[test]
    fn segmented_bevel_keeps_the_bounds_and_clamps_the_segments() {
        let c = Mesh::cube(2.0);
        let m = c.beveled_segments(0.2, 5);
        let (lo, hi) = m.bounds().unwrap();
        assert!((0..3).all(|k| (lo[k] + 1.0).abs() < 1e-4 && (hi[k] - 1.0).abs() < 1e-4));
        assert_eq!(c.beveled_segments(0.2, 0), c.beveled(0.2), "0 counts as 1");
        assert_eq!(c.beveled_segments(0.0, 4), c);
    }
}
