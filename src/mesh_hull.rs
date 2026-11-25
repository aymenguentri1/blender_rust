//! The Convex Hull operator (`mesh.convex_hull`).
//!
//! Modelled on Blender's `bmo_convex_hull` (`extern/` quickhull in `intern/`): the hull of all vertices of the
//! mesh replaces the mesh, and coplanar triangles are joined into n-gons (`use_join_triangles`), so the hull of
//! a box is six quads. Vertices inside the hull or on its faces and edges are dropped (`delete_unused`).
//! The hull is built incrementally: a tetrahedron from four extreme points, then every other point that lies
//! outside removes the faces it can see and is joined to the horizon. Deviations: it is O(points x faces), which
//! is fine for the meshes of this viewport, and the hull of a flat or collinear point set is not made (the
//! operator reports nothing to do) where Blender would build a flat n-gon.

use crate::mesh::Mesh;
use std::collections::{HashMap, HashSet};

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P, b: P) -> P {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: P) -> f64 {
    dot(a, a).sqrt()
}

/// Unit normal and plane offset of the triangle, counter clockwise seen from outside.
fn plane(pts: &[P], t: [usize; 3]) -> (P, f64) {
    let n = cross(sub(pts[t[1]], pts[t[0]]), sub(pts[t[2]], pts[t[0]]));
    let l = len(n);
    let n = if l > 0.0 { [n[0] / l, n[1] / l, n[2] / l] } else { [0.0; 3] };
    (n, dot(n, pts[t[0]]))
}

impl Mesh {
    /// The convex hull of the vertices as a mesh with outward counter clockwise faces, or `None` when the
    /// points do not span a volume.
    pub fn convex_hull(&self) -> Option<Mesh> {
        let pts: Vec<P> = self.verts.iter().map(|v| [v[0] as f64, v[1] as f64, v[2] as f64]).collect();
        if pts.len() < 4 {
            return None;
        }
        let (lo, hi) = self.bounds()?;
        let extent = (0..3).map(|k| (hi[k] - lo[k]) as f64).fold(0.0, f64::max);
        if extent <= 0.0 {
            return None;
        }
        let eps = extent * 1e-7;

        /* The starting tetrahedron: far apart points, then the one farthest from the line and from the plane. */
        let i0 = (0..pts.len()).min_by(|&a, &b| pts[a][0].total_cmp(&pts[b][0]))?;
        let i1 = (0..pts.len()).max_by(|&a, &b| len(sub(pts[a], pts[i0])).total_cmp(&len(sub(pts[b], pts[i0]))))?;
        if len(sub(pts[i1], pts[i0])) <= eps {
            return None;
        }
        let axis = sub(pts[i1], pts[i0]);
        let line_dist = |i: usize| len(cross(axis, sub(pts[i], pts[i0]))) / len(axis);
        let i2 = (0..pts.len()).max_by(|&a, &b| line_dist(a).total_cmp(&line_dist(b)))?;
        if line_dist(i2) <= eps {
            return None;
        }
        let (n, d) = plane(&pts, [i0, i1, i2]);
        let plane_dist = |i: usize| dot(n, pts[i]) - d;
        let i3 = (0..pts.len()).max_by(|&a, &b| plane_dist(a).abs().total_cmp(&plane_dist(b).abs()))?;
        if plane_dist(i3).abs() <= eps {
            return None;
        }
        /* The base must face away from the fourth point. */
        let (i1, i2) = if plane_dist(i3) > 0.0 { (i2, i1) } else { (i1, i2) };
        let mut tris: Vec<[usize; 3]> = vec![[i0, i1, i2], [i0, i3, i1], [i1, i3, i2], [i2, i3, i0]];
        let start = [i0, i1, i2, i3];

        for p in 0..pts.len() {
            if start.contains(&p) {
                continue;
            }
            let visible: Vec<bool> = tris
                .iter()
                .map(|&t| {
                    let (n, d) = plane(&pts, t);
                    dot(n, pts[p]) - d > eps
                })
                .collect();
            if !visible.iter().any(|&v| v) {
                continue;
            }
            let seen: HashSet<(usize, usize)> = tris
                .iter()
                .zip(&visible)
                .filter(|(_, &v)| v)
                .flat_map(|(t, _)| (0..3).map(move |k| (t[k], t[(k + 1) % 3])))
                .collect();
            let mut next: Vec<[usize; 3]> = tris.iter().zip(&visible).filter(|(_, &v)| !v).map(|(t, _)| *t).collect();
            for &(a, b) in &seen {
                if !seen.contains(&(b, a)) {
                    next.push([a, b, p]);
                }
            }
            tris = next;
        }

        let faces = join_coplanar(&pts, &tris, eps);

        /* Keep only the vertices that are corners of the hull. */
        let mut remap: HashMap<usize, u32> = HashMap::new();
        let mut out = Mesh::default();
        let mut order: Vec<usize> = faces.iter().flatten().copied().collect();
        order.sort_unstable();
        order.dedup();
        for v in order {
            remap.insert(v, out.verts.len() as u32);
            out.verts.push(self.verts[v]);
        }
        out.faces = faces.iter().map(|f| f.iter().map(|v| remap[v]).collect()).collect();
        Some(out)
    }
}

/// Joins edge-connected triangles on one plane into polygons; a group whose outline is not one simple loop
/// stays as triangles.
fn join_coplanar(pts: &[P], tris: &[[usize; 3]], eps: f64) -> Vec<Vec<usize>> {
    let planes: Vec<(P, f64)> = tris.iter().map(|&t| plane(pts, t)).collect();
    let mut edge_owner: HashMap<(usize, usize), usize> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        for k in 0..3 {
            edge_owner.insert((t[k], t[(k + 1) % 3]), i);
        }
    }
    let mut group = vec![usize::MAX; tris.len()];
    let mut groups = 0;
    for s in 0..tris.len() {
        if group[s] != usize::MAX {
            continue;
        }
        let mut stack = vec![s];
        group[s] = groups;
        while let Some(i) = stack.pop() {
            let t = tris[i];
            for k in 0..3 {
                let Some(&j) = edge_owner.get(&(t[(k + 1) % 3], t[k])) else { continue };
                let same = group[j] == usize::MAX
                    && dot(planes[i].0, planes[j].0) > 1.0 - 1e-9
                    && (planes[i].1 - planes[j].1).abs() <= eps * 10.0;
                if same {
                    group[j] = groups;
                    stack.push(j);
                }
            }
        }
        groups += 1;
    }

    let mut out = Vec::new();
    for g in 0..groups {
        let members: Vec<usize> = (0..tris.len()).filter(|&i| group[i] == g).collect();
        if members.len() == 1 {
            out.push(tris[members[0]].to_vec());
            continue;
        }
        let edges: HashSet<(usize, usize)> =
            members.iter().flat_map(|&i| (0..3).map(move |k| (tris[i][k], tris[i][(k + 1) % 3]))).collect();
        let mut next: HashMap<usize, usize> = HashMap::new();
        let mut simple = true;
        for &(a, b) in &edges {
            if !edges.contains(&(b, a)) && next.insert(a, b).is_some() {
                simple = false;
            }
        }
        let mut poly = Vec::new();
        if simple && !next.is_empty() {
            let first = *next.keys().min().unwrap();
            let mut cur = first;
            loop {
                poly.push(cur);
                match next.get(&cur) {
                    Some(&n) => cur = n,
                    None => break,
                }
                if cur == first || poly.len() > next.len() {
                    break;
                }
            }
        }
        if simple && poly.len() == next.len() && poly.len() >= 3 {
            out.push(poly);
        } else {
            out.extend(members.iter().map(|&i| tris[i].to_vec()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hull_of_a_cube_with_inner_points_is_six_quads() {
        let mut m = Mesh::cube(2.0);
        m.verts.push([0.0, 0.0, 0.0]);
        m.verts.push([0.5, 0.2, -0.3]);
        m.verts.push([1.0, 0.0, 0.0]); /* in the middle of a face: dropped */
        let h = m.convex_hull().unwrap();
        assert_eq!(h.verts.len(), 8);
        assert_eq!(h.faces.len(), 6);
        assert!(h.faces.iter().all(|f| f.len() == 4));
        assert!((h.signed_volume() - 8.0).abs() < 1e-4, "{}", h.signed_volume());
    }

    #[test]
    fn the_hull_of_a_sphere_keeps_its_volume_and_is_closed() {
        let m = Mesh::uv_sphere(16, 8, 1.0);
        let h = m.convex_hull().unwrap();
        assert!((h.signed_volume() - m.signed_volume()).abs() < 1e-3);
        assert!(h.edges().iter().all(|e| e.faces == 2), "closed and manifold");
    }

    #[test]
    fn the_hull_fills_a_concavity() {
        /* Two cubes side by side with a gap above them: an L shaped union. */
        let mut m = Mesh::cube(2.0);
        m.append(&Mesh::cube(2.0).mapped(|v| [v[0] + 2.0, v[1], v[2]], false));
        m.append(&Mesh::cube(2.0).mapped(|v| [v[0], v[1], v[2] + 2.0], false));
        let h = m.convex_hull().unwrap();
        assert!(h.signed_volume() > 3.0 * 8.0 + 1.0);
        assert!(h.edges().iter().all(|e| e.faces == 2));
    }

    #[test]
    fn flat_or_tiny_point_sets_have_no_hull() {
        assert!(Mesh::plane(2.0).convex_hull().is_none());
        assert!(Mesh::default().convex_hull().is_none());
        let line = Mesh { verts: vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]], faces: vec![] };
        assert!(line.convex_hull().is_none());
    }
}
