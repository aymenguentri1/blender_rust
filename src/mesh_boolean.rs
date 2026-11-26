//! The Boolean modifier (`MOD_boolean`): Difference, Union and Intersect of a mesh with another mesh object.
//!
//! Blender's Fast solver and this one both want closed, consistently wound meshes. This one is a BSP tree
//! CSG (the classic clip-and-merge of Naylor / `csg.js`) computed in `f64`:
//!  - both meshes are triangulated and every triangle becomes a polygon with its plane
//!  - the operation is a fixed sequence of `clip_to` / `invert` calls on the two trees
//!  - the surviving convex pieces are welded back into a `Mesh` (pieces stay n-gons, no re-merging of the
//!    coplanar fragments, so the result has more faces than Blender's)
//!
//! Safety nets: disjoint bounding boxes skip the tree entirely, the tree depth is capped so that recursion
//! stays shallow on degenerate input, and inputs above `MAX_TRIANGLES` are left alone (the modifier then
//! returns the mesh unchanged instead of freezing the viewport).

use crate::mesh::{Mesh, V3};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    Difference,
    Union,
    Intersect,
}

impl BoolOp {
    pub fn label(self) -> &'static str {
        match self {
            BoolOp::Difference => "Difference",
            BoolOp::Union => "Union",
            BoolOp::Intersect => "Intersect",
        }
    }

    /// Blender's menu order is Intersect, Union, Difference; the value keys walk it forwards.
    pub fn next(self) -> BoolOp {
        match self {
            BoolOp::Difference => BoolOp::Union,
            BoolOp::Union => BoolOp::Intersect,
            BoolOp::Intersect => BoolOp::Difference,
        }
    }

    pub fn prev(self) -> BoolOp {
        match self {
            BoolOp::Difference => BoolOp::Intersect,
            BoolOp::Union => BoolOp::Difference,
            BoolOp::Intersect => BoolOp::Union,
        }
    }
}

/// Above this many triangles in both operands together the modifier does nothing.
pub const MAX_TRIANGLES: usize = 6000;
/// Deepest BSP level; polygons below it are kept together in one node.
const MAX_DEPTH: usize = 200;
/// Distance under which a point is on a plane (the meshes are in metres).
const EPS: f64 = 1e-6;
/// Vertices closer than this are one vertex in the result.
const WELD: f64 = 1e-5;

type P3 = [f64; 3];

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn lerp(a: P3, b: P3, t: f64) -> P3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

#[derive(Clone, Copy)]
struct Plane {
    n: P3,
    w: f64,
}

impl Plane {
    fn from_points(a: P3, b: P3, c: P3) -> Option<Plane> {
        let n = cross(sub(b, a), sub(c, a));
        let l = dot(n, n).sqrt();
        if !(l > 1e-14) {
            return None;
        }
        let n = [n[0] / l, n[1] / l, n[2] / l];
        Some(Plane { n, w: dot(n, a) })
    }

    fn flip(&mut self) {
        self.n = [-self.n[0], -self.n[1], -self.n[2]];
        self.w = -self.w;
    }
}

#[derive(Clone)]
struct Poly {
    v: Vec<P3>,
    plane: Plane,
}

impl Poly {
    fn flip(&mut self) {
        self.v.reverse();
        self.plane.flip();
    }
}

const COPLANAR: u8 = 0;
const FRONT: u8 = 1;
const BACK: u8 = 2;
const SPANNING: u8 = 3;

/// Sorts `poly` against `plane` into the four buckets, cutting it when it spans the plane.
fn split_polygon(
    plane: &Plane,
    poly: &Poly,
    coplanar_front: &mut Vec<Poly>,
    coplanar_back: &mut Vec<Poly>,
    front: &mut Vec<Poly>,
    back: &mut Vec<Poly>,
) {
    let mut ptype = COPLANAR;
    let types: Vec<u8> = poly
        .v
        .iter()
        .map(|&p| {
            let t = dot(plane.n, p) - plane.w;
            let ty = if t < -EPS {
                BACK
            } else if t > EPS {
                FRONT
            } else {
                COPLANAR
            };
            ptype |= ty;
            ty
        })
        .collect();
    match ptype {
        COPLANAR => {
            if dot(plane.n, poly.plane.n) > 0.0 {
                coplanar_front.push(poly.clone())
            } else {
                coplanar_back.push(poly.clone())
            }
        }
        FRONT => front.push(poly.clone()),
        BACK => back.push(poly.clone()),
        _ => {
            let n = poly.v.len();
            let (mut f, mut b) = (Vec::new(), Vec::new());
            for i in 0..n {
                let j = (i + 1) % n;
                let (ti, tj) = (types[i], types[j]);
                let (vi, vj) = (poly.v[i], poly.v[j]);
                if ti != BACK {
                    f.push(vi);
                }
                if ti != FRONT {
                    b.push(vi);
                }
                if (ti | tj) == SPANNING {
                    let d = dot(plane.n, sub(vj, vi));
                    let t = (plane.w - dot(plane.n, vi)) / d;
                    let v = lerp(vi, vj, t);
                    f.push(v);
                    b.push(v);
                }
            }
            if f.len() >= 3 {
                front.push(Poly { v: f, plane: poly.plane });
            }
            if b.len() >= 3 {
                back.push(Poly { v: b, plane: poly.plane });
            }
        }
    }
}

#[derive(Default)]
struct Node {
    plane: Option<Plane>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
    polys: Vec<Poly>,
}

impl Node {
    fn from_polys(polys: Vec<Poly>) -> Node {
        let mut n = Node::default();
        n.build(polys, 0);
        n
    }

    fn invert(&mut self) {
        for p in &mut self.polys {
            p.flip();
        }
        if let Some(pl) = &mut self.plane {
            pl.flip();
        }
        if let Some(f) = &mut self.front {
            f.invert();
        }
        if let Some(b) = &mut self.back {
            b.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }

    /// The parts of `polys` outside the solid this tree describes.
    fn clip_polygons(&self, polys: Vec<Poly>) -> Vec<Poly> {
        let Some(plane) = &self.plane else { return polys };
        let (mut front, mut back) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            split_polygon(plane, p, &mut cf, &mut cb, &mut front, &mut back);
            front.append(&mut cf);
            back.append(&mut cb);
        }
        let mut out = match &self.front {
            Some(f) => f.clip_polygons(front),
            None => front,
        };
        if let Some(b) = &self.back {
            out.extend(b.clip_polygons(back));
        }
        out
    }

    fn clip_to(&mut self, other: &Node) {
        self.polys = other.clip_polygons(std::mem::take(&mut self.polys));
        if let Some(f) = &mut self.front {
            f.clip_to(other);
        }
        if let Some(b) = &mut self.back {
            b.clip_to(other);
        }
    }

    fn all_polygons(&self) -> Vec<Poly> {
        let mut out = self.polys.clone();
        if let Some(f) = &self.front {
            out.extend(f.all_polygons());
        }
        if let Some(b) = &self.back {
            out.extend(b.all_polygons());
        }
        out
    }

    fn build(&mut self, polys: Vec<Poly>, depth: usize) {
        if polys.is_empty() {
            return;
        }
        if self.plane.is_none() {
            /* The middle polygon rather than the first one: the meshes come ordered along their rings or rows,
             * and the first plane would peel them off one at a time into a deep, lopsided tree. */
            self.plane = Some(polys[polys.len() / 2].plane);
        }
        if depth >= MAX_DEPTH {
            self.polys.extend(polys);
            return;
        }
        let plane = self.plane.unwrap();
        let (mut front, mut back) = (Vec::new(), Vec::new());
        for p in &polys {
            let (mut cf, mut cb) = (Vec::new(), Vec::new());
            split_polygon(&plane, p, &mut cf, &mut cb, &mut front, &mut back);
            self.polys.append(&mut cf);
            self.polys.append(&mut cb);
        }
        if !front.is_empty() {
            self.front.get_or_insert_with(Default::default).build(front, depth + 1);
        }
        if !back.is_empty() {
            self.back.get_or_insert_with(Default::default).build(back, depth + 1);
        }
    }
}

fn to_polys(mesh: &Mesh) -> Vec<Poly> {
    let tri = mesh.triangulated();
    tri.faces
        .iter()
        .filter(|f| f.len() == 3)
        .filter_map(|f| {
            let p = |i: usize| tri.verts[f[i] as usize].map(f64::from);
            let (a, b, c) = (p(0), p(1), p(2));
            Plane::from_points(a, b, c).map(|plane| Poly { v: vec![a, b, c], plane })
        })
        .collect()
}

/// Welds the polygons into a mesh; pieces that collapse below three corners are dropped.
fn to_mesh(polys: &[Poly]) -> Mesh {
    let mut mesh = Mesh::default();
    let mut index: HashMap<[i64; 3], u32> = HashMap::new();
    for poly in polys {
        let mut face: Vec<u32> = Vec::with_capacity(poly.v.len());
        for p in &poly.v {
            let key = [(p[0] / WELD).round() as i64, (p[1] / WELD).round() as i64, (p[2] / WELD).round() as i64];
            let id = *index.entry(key).or_insert_with(|| {
                mesh.verts.push([p[0] as f32, p[1] as f32, p[2] as f32]);
                (mesh.verts.len() - 1) as u32
            });
            if face.last() != Some(&id) {
                face.push(id);
            }
        }
        while face.len() > 1 && face.first() == face.last() {
            face.pop();
        }
        let mut seen = face.clone();
        seen.sort_unstable();
        seen.dedup();
        if face.len() >= 3 && seen.len() == face.len() {
            mesh.faces.push(face);
        }
    }
    mesh
}

fn overlap(a: (V3, V3), b: (V3, V3)) -> bool {
    (0..3).all(|k| a.0[k] <= b.1[k] + EPS as f32 && b.0[k] <= a.1[k] + EPS as f32)
}

/// `self` and `other` side by side in one mesh.
fn joined(a: &Mesh, b: &Mesh) -> Mesh {
    let mut m = a.clone();
    let base = m.verts.len() as u32;
    m.verts.extend_from_slice(&b.verts);
    m.faces.extend(b.faces.iter().map(|f| f.iter().map(|i| i + base).collect::<Vec<u32>>()));
    m
}

impl Mesh {
    /// The Boolean modifier: `self` combined with `other` (already in the space of `self`). An empty operand
    /// or a result beyond `MAX_TRIANGLES` leaves `self` as it is (an empty `other` intersects to nothing).
    pub fn boolean(&self, other: &Mesh, op: BoolOp) -> Mesh {
        let (Some(ba), Some(bb)) = (self.bounds(), other.bounds()) else {
            return if other.faces.is_empty() && op == BoolOp::Intersect { Mesh::default() } else { self.clone() };
        };
        if !overlap(ba, bb) {
            return match op {
                BoolOp::Difference => self.clone(),
                BoolOp::Union => joined(self, other),
                BoolOp::Intersect => Mesh::default(),
            };
        }
        let (pa, pb) = (to_polys(self), to_polys(other));
        if pa.is_empty() || pb.is_empty() || pa.len() + pb.len() > MAX_TRIANGLES {
            return self.clone();
        }
        let (mut a, mut b) = (Node::from_polys(pa), Node::from_polys(pb));
        match op {
            BoolOp::Union => {
                a.clip_to(&b);
                b.clip_to(&a);
                b.invert();
                b.clip_to(&a);
                b.invert();
                a.build(b.all_polygons(), 0);
            }
            BoolOp::Difference => {
                a.invert();
                a.clip_to(&b);
                b.clip_to(&a);
                b.invert();
                b.clip_to(&a);
                b.invert();
                a.build(b.all_polygons(), 0);
                a.invert();
            }
            BoolOp::Intersect => {
                a.invert();
                b.clip_to(&a);
                b.invert();
                a.clip_to(&b);
                b.clip_to(&a);
                a.build(b.all_polygons(), 0);
                a.invert();
            }
        }
        to_mesh(&a.all_polygons())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enclosed volume by the divergence theorem (fan of every face).
    fn volume(m: &Mesh) -> f32 {
        let mut v = 0.0;
        for f in &m.faces {
            for k in 1..f.len() - 1 {
                let (a, b, c) = (m.verts[f[0] as usize], m.verts[f[k] as usize], m.verts[f[k + 1] as usize]);
                v += crate::mesh::dot(a, crate::mesh::cross(b, c)) / 6.0;
            }
        }
        v
    }

    fn shifted(m: &Mesh, d: V3) -> Mesh {
        m.mapped(|p| [p[0] + d[0], p[1] + d[1], p[2] + d[2]], false)
    }

    #[test]
    fn overlapping_cubes_have_the_expected_volumes() {
        /* Two 2 m cubes overlapping in a 1 x 2 x 2 slab: 8 + 8 - 4, 4 and 8 - 4. */
        let a = Mesh::cube(2.0);
        let b = shifted(&a, [1.0, 0.0, 0.0]);
        assert!((volume(&a) - 8.0).abs() < 1e-4);
        assert!((volume(&a.boolean(&b, BoolOp::Union)) - 12.0).abs() < 1e-3);
        assert!((volume(&a.boolean(&b, BoolOp::Intersect)) - 4.0).abs() < 1e-3);
        assert!((volume(&a.boolean(&b, BoolOp::Difference)) - 4.0).abs() < 1e-3);
    }

    #[test]
    fn a_cut_from_the_corner_removes_an_eighth() {
        let a = Mesh::cube(2.0);
        let b = shifted(&Mesh::cube(2.0), [1.0, 1.0, 1.0]);
        let d = a.boolean(&b, BoolOp::Difference);
        assert!((volume(&d) - 7.0).abs() < 1e-3);
    }

    #[test]
    fn disjoint_operands_short_cut() {
        let a = Mesh::cube(2.0);
        let far = shifted(&a, [10.0, 0.0, 0.0]);
        assert_eq!(a.boolean(&far, BoolOp::Difference), a);
        assert_eq!(a.boolean(&far, BoolOp::Intersect), Mesh::default());
        let u = a.boolean(&far, BoolOp::Union);
        assert_eq!(u.faces.len(), 12);
        assert!((volume(&u) - 16.0).abs() < 1e-3);
    }

    #[test]
    fn a_sphere_with_a_bite_is_smaller() {
        let sphere = Mesh::uv_sphere(16, 8, 1.0);
        let cutter = shifted(&Mesh::cube(1.0), [0.9, 0.0, 0.0]);
        let (vs, vd) = (volume(&sphere), volume(&sphere.boolean(&cutter, BoolOp::Difference)));
        assert!(vd < vs - 0.01 && vd > 0.0, "{vs} -> {vd}");
        let both = volume(&sphere.boolean(&cutter, BoolOp::Union));
        let inter = volume(&sphere.boolean(&cutter, BoolOp::Intersect));
        /* Inclusion-exclusion: |A u B| = |A| + |B| - |A n B|. */
        assert!((both - (vs + volume(&cutter) - inter)).abs() < 1e-2);
    }

    #[test]
    fn default_spheres_stay_fast_and_consistent() {
        /* The Add menu's UV spheres (32 x 16, 960 triangles each): the worst realistic size for the BSP. */
        let a = Mesh::uv_sphere(32, 16, 1.0);
        let b = shifted(&a, [0.6, 0.2, 0.1]);
        let t = std::time::Instant::now();
        let (u, i, d) = (a.boolean(&b, BoolOp::Union), a.boolean(&b, BoolOp::Intersect), a.boolean(&b, BoolOp::Difference));
        assert!(t.elapsed().as_secs_f32() < 5.0, "too slow: {:?}", t.elapsed());
        let (va, vb) = (volume(&a), volume(&b));
        assert!((volume(&u) - (va + vb - volume(&i))).abs() < 0.02 * va, "inclusion-exclusion");
        assert!((volume(&d) - (va - volume(&i))).abs() < 0.02 * va, "difference");
        /* Identical operands (every plane coplanar) must not recurse forever. */
        let same = a.boolean(&a, BoolOp::Union);
        assert!(!same.faces.is_empty());
    }

    #[test]
    fn an_empty_operand_is_harmless() {
        let a = Mesh::cube(2.0);
        assert_eq!(a.boolean(&Mesh::default(), BoolOp::Difference), a);
        assert_eq!(a.boolean(&Mesh::default(), BoolOp::Union), a);
        assert_eq!(Mesh::default().boolean(&a, BoolOp::Union), Mesh::default());
    }

    #[test]
    fn the_operation_keys_cycle() {
        let mut op = BoolOp::Difference;
        for _ in 0..3 {
            op = op.next();
        }
        assert_eq!(op, BoolOp::Difference);
        assert_eq!(BoolOp::Difference.prev().next(), BoolOp::Difference);
    }
}
