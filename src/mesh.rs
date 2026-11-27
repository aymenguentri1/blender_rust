//! Polygon meshes, the primitive generators of the Add menu and Catmull-Clark subdivision.
//!
//! Modelled on Blender:
//!  - the primitive defaults of `MESH_OT_primitive_*_add` (editors/mesh/editmesh_add.cc): circle, cylinder
//!    and cone 32 vertices, UV sphere 32 segments and 16 rings, ico sphere 2 subdivisions, grid 10 x 10,
//!    plane / cube 2 m, torus 48 x 12 segments with a major radius of 1 m and a minor one of 0.25 m
//!  - `bmo_create_uvsphere_exec()`, `bmo_create_icosphere_exec()`, `bmo_create_cone_exec()`
//!    (bmesh/operators/bmo_primitive.cc): the vertex counts match (a UV sphere has `segments * (rings - 1) +
//!    2` vertices, an ico sphere of 2 subdivisions has 42)
//!  - the Subdivision Surface modifier, `MOD_subsurf` (Catmull-Clark, "simple" boundary rule
//!    `Keep Corners` is not modelled: boundaries use the standard crease rule)
//!
//! Faces are counter-clockwise seen from outside (the normal is the Newell normal); a mesh has no loose
//! edges, so the Circle primitive is a filled n-gon (Blender's `Triangle Fan` / `NGon` fill types; its default
//! `Nothing` fill, a bare edge loop, cannot be shaded).

pub type V3 = [f32; 3];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub verts: Vec<V3>,
    pub faces: Vec<Vec<u32>>,
}

/// A unique edge with the first face that uses it (the wire shader takes the normal of an adjacent face).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshEdge {
    pub a: u32,
    pub b: u32,
    pub face: u32,
    /// Number of faces that use the edge: 1 is a boundary edge, 2 a manifold one.
    pub faces: u32,
}

pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub(crate) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
pub(crate) fn normalized(v: V3) -> V3 {
    let l = dot(v, v).sqrt();
    if l > 1e-20 {
        scale(v, 1.0 / l)
    } else {
        v
    }
}

/// The kinds of the Add > Mesh menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Plane,
    Cube,
    Circle,
    UvSphere,
    IcoSphere,
    Cylinder,
    Cone,
    Torus,
    Grid,
}

impl Primitive {
    pub const ALL: [Primitive; 9] = [
        Primitive::Plane,
        Primitive::Cube,
        Primitive::Circle,
        Primitive::UvSphere,
        Primitive::IcoSphere,
        Primitive::Cylinder,
        Primitive::Cone,
        Primitive::Torus,
        Primitive::Grid,
    ];

    /// The menu label, which is also the name of the new object.
    pub fn label(self) -> &'static str {
        match self {
            Primitive::Plane => "Plane",
            Primitive::Cube => "Cube",
            Primitive::Circle => "Circle",
            Primitive::UvSphere => "UV Sphere",
            Primitive::IcoSphere => "Ico Sphere",
            Primitive::Cylinder => "Cylinder",
            Primitive::Cone => "Cone",
            Primitive::Torus => "Torus",
            Primitive::Grid => "Grid",
        }
    }

    /// The object name Blender gives the new object (`ED_object_add_type()` uses the data name: `Sphere`
    /// for both spheres).
    pub fn object_name(self) -> &'static str {
        match self {
            Primitive::UvSphere => "Sphere",
            Primitive::IcoSphere => "Icosphere",
            other => other.label(),
        }
    }

    pub fn build(self) -> Mesh {
        match self {
            Primitive::Plane => Mesh::plane(2.0),
            Primitive::Cube => Mesh::cube(2.0),
            Primitive::Circle => Mesh::circle(32, 1.0),
            Primitive::UvSphere => Mesh::uv_sphere(32, 16, 1.0),
            Primitive::IcoSphere => Mesh::ico_sphere(2, 1.0),
            Primitive::Cylinder => Mesh::cone(32, 1.0, 1.0, 2.0),
            Primitive::Cone => Mesh::cone(32, 1.0, 0.0, 2.0),
            Primitive::Torus => Mesh::torus(48, 12, 1.0, 0.25),
            Primitive::Grid => Mesh::grid(10, 10, 2.0),
        }
    }
}

impl Mesh {
    /// A square in the XY plane, facing +Z.
    pub fn plane(size: f32) -> Mesh {
        let h = size / 2.0;
        Mesh {
            verts: vec![[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]],
            faces: vec![vec![0, 1, 2, 3]],
        }
    }

    /// The default cube: `size` m on each side at the origin, in the face order of `workbench::FACES`.
    pub fn cube(size: f32) -> Mesh {
        let h = size / 2.0;
        let corners: [[V3; 4]; 6] = [
            [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]],
            [[-1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0]],
            [[-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0]],
            [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, -1.0, 1.0]],
            [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]],
            [[-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0], [1.0, -1.0, -1.0]],
        ];
        let mut m = Mesh::default();
        let index_of = |m: &mut Mesh, p: V3| -> u32 {
            let p = scale(p, h);
            if let Some(i) = m.verts.iter().position(|&q| q == p) {
                return i as u32;
            }
            m.verts.push(p);
            (m.verts.len() - 1) as u32
        };
        for quad in corners {
            let face = quad.iter().map(|&c| index_of(&mut m, c)).collect();
            m.faces.push(face);
        }
        m
    }

    /// A filled circle in the XY plane (one n-gon), facing +Z.
    pub fn circle(vertices: usize, radius: f32) -> Mesh {
        let n = vertices.max(3);
        let verts = (0..n)
            .map(|k| {
                let a = std::f32::consts::TAU * k as f32 / n as f32;
                [radius * a.cos(), radius * a.sin(), 0.0]
            })
            .collect();
        Mesh { verts, faces: vec![(0..n as u32).collect()] }
    }

    /// `bmo_create_uvsphere_exec()`: poles on the Z axis, `rings - 1` rings of `segments` vertices between.
    pub fn uv_sphere(segments: usize, rings: usize, radius: f32) -> Mesh {
        let (seg, rings) = (segments.max(3), rings.max(2));
        let mut verts = vec![[0.0, 0.0, radius]];
        for r in 1..rings {
            let theta = std::f32::consts::PI * r as f32 / rings as f32;
            for s in 0..seg {
                let phi = std::f32::consts::TAU * s as f32 / seg as f32;
                verts.push([
                    radius * theta.sin() * phi.cos(),
                    radius * theta.sin() * phi.sin(),
                    radius * theta.cos(),
                ]);
            }
        }
        verts.push([0.0, 0.0, -radius]);
        let ring = |r: usize, s: usize| (1 + (r - 1) * seg + s % seg) as u32;
        let bottom = (verts.len() - 1) as u32;
        let mut faces = Vec::new();
        for s in 0..seg {
            faces.push(vec![0, ring(1, s), ring(1, s + 1)]);
        }
        for r in 1..rings - 1 {
            for s in 0..seg {
                faces.push(vec![ring(r + 1, s), ring(r + 1, s + 1), ring(r, s + 1), ring(r, s)]);
            }
        }
        for s in 0..seg {
            faces.push(vec![ring(rings - 1, s + 1), ring(rings - 1, s), bottom]);
        }
        Mesh { verts, faces }
    }

    /// `bmo_create_icosphere_exec()`: an icosahedron whose triangles are split into four
    /// `subdivisions - 1` times, every vertex pushed out to `radius`.
    pub fn ico_sphere(subdivisions: usize, radius: f32) -> Mesh {
        let t = (1.0 + 5.0f32.sqrt()) / 2.0;
        let mut verts: Vec<V3> = [
            [-1.0, t, 0.0],
            [1.0, t, 0.0],
            [-1.0, -t, 0.0],
            [1.0, -t, 0.0],
            [0.0, -1.0, t],
            [0.0, 1.0, t],
            [0.0, -1.0, -t],
            [0.0, 1.0, -t],
            [t, 0.0, -1.0],
            [t, 0.0, 1.0],
            [-t, 0.0, -1.0],
            [-t, 0.0, 1.0],
        ]
        .iter()
        .map(|&v| scale(normalized(v), radius))
        .collect();
        let mut tris: Vec<[u32; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for _ in 1..subdivisions.max(1) {
            let mut cache = std::collections::HashMap::new();
            let mut mid = |verts: &mut Vec<V3>, a: u32, b: u32| -> u32 {
                let key = (a.min(b), a.max(b));
                *cache.entry(key).or_insert_with(|| {
                    let p = scale(normalized(add(verts[a as usize], verts[b as usize])), radius);
                    verts.push(p);
                    (verts.len() - 1) as u32
                })
            };
            let mut next = Vec::with_capacity(tris.len() * 4);
            for [a, b, c] in tris {
                let (ab, bc, ca) = (mid(&mut verts, a, b), mid(&mut verts, b, c), mid(&mut verts, c, a));
                next.extend([[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            tris = next;
        }
        let mut m = Mesh { verts, faces: tris.iter().map(|t| t.to_vec()).collect() };
        m.orient_outward();
        m
    }

    /// `bmo_create_cone_exec()` along Z, centred at the origin: `radius1` at the bottom, `radius2` at the top,
    /// both ends capped with an n-gon (a zero radius collapses that end to a single vertex).
    pub fn cone(vertices: usize, radius1: f32, radius2: f32, depth: f32) -> Mesh {
        let n = vertices.max(3);
        let h = depth / 2.0;
        let ring = |r: f32, z: f32| -> Vec<V3> {
            (0..n)
                .map(|k| {
                    let a = std::f32::consts::TAU * k as f32 / n as f32;
                    [r * a.cos(), r * a.sin(), z]
                })
                .collect()
        };
        let mut verts = Vec::new();
        let mut faces = Vec::new();
        let (bottom_apex, top_apex) = (radius1 <= 0.0, radius2 <= 0.0);
        let bottom: Vec<u32> = if bottom_apex {
            verts.push([0.0, 0.0, -h]);
            vec![0u32; n]
        } else {
            verts.extend(ring(radius1, -h));
            (0..n as u32).collect()
        };
        let top: Vec<u32> = if top_apex {
            verts.push([0.0, 0.0, h]);
            vec![(verts.len() - 1) as u32; n]
        } else {
            let base = verts.len() as u32;
            verts.extend(ring(radius2, h));
            (base..base + n as u32).collect()
        };
        for k in 0..n {
            let k1 = (k + 1) % n;
            if top_apex {
                faces.push(vec![bottom[k], bottom[k1], top[k]]);
            } else if bottom_apex {
                faces.push(vec![bottom[k], top[k1], top[k]]);
            } else {
                faces.push(vec![bottom[k], bottom[k1], top[k1], top[k]]);
            }
        }
        if !top_apex {
            faces.push(top.clone());
        }
        if !bottom_apex {
            faces.push(bottom.iter().rev().copied().collect());
        }
        Mesh { verts, faces }
    }

    /// `add_torus` (`MESH_OT_primitive_torus_add`): `major_segments` around Z, `minor_segments` around the tube.
    pub fn torus(major_segments: usize, minor_segments: usize, major_radius: f32, minor_radius: f32) -> Mesh {
        let (mj, mn) = (major_segments.max(3), minor_segments.max(3));
        let mut verts = Vec::with_capacity(mj * mn);
        for i in 0..mj {
            let u = std::f32::consts::TAU * i as f32 / mj as f32;
            for j in 0..mn {
                let v = std::f32::consts::TAU * j as f32 / mn as f32;
                let r = major_radius + minor_radius * v.cos();
                verts.push([r * u.cos(), r * u.sin(), minor_radius * v.sin()]);
            }
        }
        let at = |i: usize, j: usize| ((i % mj) * mn + j % mn) as u32;
        let mut faces = Vec::with_capacity(mj * mn);
        for i in 0..mj {
            for j in 0..mn {
                faces.push(vec![at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        Mesh { verts, faces }
    }

    /// `bmo_create_grid_exec()`: `x` by `y` quads covering `size` m, centred, facing +Z.
    pub fn grid(x_segments: usize, y_segments: usize, size: f32) -> Mesh {
        let (nx, ny) = (x_segments.max(1), y_segments.max(1));
        let mut verts = Vec::with_capacity((nx + 1) * (ny + 1));
        for j in 0..=ny {
            for i in 0..=nx {
                verts.push([
                    size * (i as f32 / nx as f32 - 0.5),
                    size * (j as f32 / ny as f32 - 0.5),
                    0.0,
                ]);
            }
        }
        let at = |i: usize, j: usize| (j * (nx + 1) + i) as u32;
        let mut faces = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                faces.push(vec![at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        Mesh { verts, faces }
    }

    /// Flips the faces that look towards the centre of a convex, origin centred mesh.
    fn orient_outward(&mut self) {
        for f in 0..self.faces.len() {
            let n = self.face_normal(f);
            let c = self.face_center(f);
            if dot(n, c) < 0.0 {
                self.faces[f].reverse();
            }
        }
    }

    /// The Newell normal of face `f`, normalised.
    pub fn face_normal(&self, f: usize) -> V3 {
        normalized(self.face_area_normal(f))
    }

    /// The Newell vector of face `f`: the normal scaled by twice the area.
    pub(crate) fn face_area_normal(&self, f: usize) -> V3 {
        let face = &self.faces[f];
        let mut n = [0.0f32; 3];
        for i in 0..face.len() {
            let (a, b) = (self.verts[face[i] as usize], self.verts[face[(i + 1) % face.len()] as usize]);
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        n
    }

    pub fn face_center(&self, f: usize) -> V3 {
        let face = &self.faces[f];
        let sum = face.iter().fold([0.0f32; 3], |s, &i| add(s, self.verts[i as usize]));
        scale(sum, 1.0 / face.len().max(1) as f32)
    }

    /// Area weighted vertex normals, the smooth shading normals.
    pub fn vertex_normals(&self) -> Vec<V3> {
        let mut normals = vec![[0.0f32; 3]; self.verts.len()];
        for f in 0..self.faces.len() {
            let n = self.face_area_normal(f);
            for &i in &self.faces[f] {
                normals[i as usize] = add(normals[i as usize], n);
            }
        }
        normals.into_iter().map(normalized).collect()
    }

    /// Every unique edge, in order of first appearance.
    pub fn edges(&self) -> Vec<MeshEdge> {
        let mut index: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
        let mut edges: Vec<MeshEdge> = Vec::new();
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                if a == b {
                    continue;
                }
                let key = (a.min(b), a.max(b));
                match index.get(&key) {
                    Some(&e) => edges[e].faces += 1,
                    None => {
                        index.insert(key, edges.len());
                        edges.push(MeshEdge { a: key.0, b: key.1, face: f as u32, faces: 1 });
                    }
                }
            }
        }
        edges
    }

    /// Triangle fan of every face: the face index and the three vertex indices.
    pub fn triangles(&self) -> Vec<(u32, [u32; 3])> {
        let mut out = Vec::new();
        for (f, face) in self.faces.iter().enumerate() {
            for k in 1..face.len().saturating_sub(1) {
                out.push((f as u32, [face[0], face[k], face[k + 1]]));
            }
        }
        out
    }

    pub fn bounds(&self) -> Option<(V3, V3)> {
        let first = *self.verts.first()?;
        Some(self.verts.iter().fold((first, first), |(mn, mx), v| {
            (std::array::from_fn(|k| mn[k].min(v[k])), std::array::from_fn(|k| mx[k].max(v[k])))
        }))
    }

    /// `levels` rounds of Catmull-Clark subdivision (the Subdivision Surface modifier).
    pub fn subdivided(&self, levels: u32) -> Mesh {
        let mut m = self.clone();
        for _ in 0..levels {
            m = m.catmull_clark();
        }
        m
    }

    fn catmull_clark(&self) -> Mesh {
        let nv = self.verts.len();
        let face_points: Vec<V3> = (0..self.faces.len()).map(|f| self.face_center(f)).collect();

        /* Edge table in first-seen order: end points, the first two faces and the face count. `corner_edge` holds
        the edge of every face corner (corner `i` of a face runs from vertex `i` to `i + 1`), so the quads below need
        no second lookup. */
        let corners: usize = self.faces.iter().map(|f| f.len()).sum();
        let mut edge_index: std::collections::HashMap<u64, u32, FastHash> =
            std::collections::HashMap::with_capacity_and_hasher(corners / 2 + 1, FastHash);
        let mut edges: Vec<EdgeUse> = Vec::with_capacity(corners / 2 + 1);
        let mut corner_edge: Vec<u32> = Vec::with_capacity(corners);
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                let (lo, hi) = (a.min(b), a.max(b));
                let e = *edge_index.entry(((lo as u64) << 32) | hi as u64).or_insert_with(|| {
                    edges.push(EdgeUse { a: lo, b: hi, faces: [0; 2], n: 0 });
                    (edges.len() - 1) as u32
                });
                let u = &mut edges[e as usize];
                if u.n < 2 {
                    u.faces[u.n as usize] = f as u32;
                }
                u.n += 1;
                corner_edge.push(e);
            }
        }
        let ne = edges.len();

        /* Edge points: the average of the end points and of the two face points; boundary edges keep the midpoint. */
        let mut edge_points = Vec::with_capacity(ne);
        for e in &edges {
            let (pa, pb) = (self.verts[e.a as usize], self.verts[e.b as usize]);
            edge_points.push(if e.n == 2 {
                scale(add(add(pa, pb), add(face_points[e.faces[0] as usize], face_points[e.faces[1] as usize])), 0.25)
            } else {
                scale(add(pa, pb), 0.5)
            });
        }

        /* New positions of the original vertices. */
        let mut sum_f = vec![[0.0f32; 3]; nv];
        let mut sum_r = vec![[0.0f32; 3]; nv];
        let mut face_count = vec![0u32; nv];
        let mut edge_count = vec![0u32; nv];
        let mut boundary_sum = vec![[0.0f32; 3]; nv];
        let mut boundary_count = vec![0u32; nv];
        for (f, face) in self.faces.iter().enumerate() {
            for &v in face {
                face_count[v as usize] += 1;
                sum_f[v as usize] = add(sum_f[v as usize], face_points[f]);
            }
        }
        for e in &edges {
            let (a, b) = (e.a, e.b);
            let mid = scale(add(self.verts[a as usize], self.verts[b as usize]), 0.5);
            for (v, other) in [(a, b), (b, a)] {
                sum_r[v as usize] = add(sum_r[v as usize], mid);
                edge_count[v as usize] += 1;
                if e.n == 1 {
                    boundary_sum[v as usize] = add(boundary_sum[v as usize], self.verts[other as usize]);
                    boundary_count[v as usize] += 1;
                }
            }
        }
        let mut verts = Vec::with_capacity(nv + ne + self.faces.len());
        for v in 0..nv {
            let p = self.verts[v];
            if edge_count[v] == 0 {
                verts.push(p);
            } else if boundary_count[v] > 0 {
                /* Boundary (crease) rule: 3/4 the vertex and 1/8 of each of the two boundary neighbours. */
                verts.push(scale(add(scale(p, 6.0), boundary_sum[v]), 1.0 / 8.0));
            } else {
                let n = face_count[v] as f32;
                let f = scale(sum_f[v], 1.0 / n);
                let r = scale(sum_r[v], 1.0 / edge_count[v] as f32);
                verts.push(scale(add(add(f, scale(r, 2.0)), scale(p, n - 3.0)), 1.0 / n));
            }
        }
        verts.extend(edge_points);
        verts.extend(face_points);

        let mut faces = Vec::with_capacity(corners);
        let mut c = 0;
        for (f, face) in self.faces.iter().enumerate() {
            let k = face.len();
            for i in 0..k {
                let (cur, prev_edge, next_edge) = (face[i], corner_edge[c + (i + k - 1) % k], corner_edge[c + i]);
                faces.push(vec![cur, nv as u32 + next_edge, (nv + ne + f) as u32, nv as u32 + prev_edge]);
            }
            c += k;
        }
        Mesh { verts, faces }
    }
}

/// An edge of the subdivision table: its (sorted) end points and the first two faces using it.
struct EdgeUse {
    a: u32,
    b: u32,
    faces: [u32; 2],
    n: u32,
}

/// Multiplicative hasher for the packed edge key (the std SipHash costs more than the subdivision itself).
#[derive(Clone, Copy, Default)]
struct FastHash;
struct FastHasher(u64);
impl std::hash::BuildHasher for FastHash {
    type Hasher = FastHasher;
    fn build_hasher(&self) -> FastHasher {
        FastHasher(0)
    }
}
impl std::hash::Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Mesh {
    fn catmull_clark_reference(&self) -> Mesh {
        let nv = self.verts.len();
        let face_points: Vec<V3> = (0..self.faces.len()).map(|f| self.face_center(f)).collect();

        /* Edge table: the faces on both sides of every edge. */
        let mut edge_index = std::collections::HashMap::new();
        let mut edge_faces: Vec<(u32, u32, Vec<usize>)> = Vec::new();
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                let key = (a.min(b), a.max(b));
                let e = *edge_index.entry(key).or_insert_with(|| {
                    edge_faces.push((key.0, key.1, Vec::new()));
                    edge_faces.len() - 1
                });
                edge_faces[e].2.push(f);
            }
        }
        let ne = edge_faces.len();

        /* Edge points: the average of the end points and of the two face points; boundary edges keep the midpoint. */
        let mut edge_points = Vec::with_capacity(ne);
        for (a, b, fs) in &edge_faces {
            let mid = scale(add(self.verts[*a as usize], self.verts[*b as usize]), 0.5);
            edge_points.push(if fs.len() == 2 {
                scale(add(add(self.verts[*a as usize], self.verts[*b as usize]), add(face_points[fs[0]], face_points[fs[1]])), 0.25)
            } else {
                mid
            });
        }

        /* New positions of the original vertices. */
        let mut sum_f = vec![[0.0f32; 3]; nv];
        let mut sum_r = vec![[0.0f32; 3]; nv];
        let mut face_count = vec![0u32; nv];
        let mut edge_count = vec![0u32; nv];
        let mut boundary_sum = vec![[0.0f32; 3]; nv];
        let mut boundary_count = vec![0u32; nv];
        for face in &self.faces {
            for &v in face {
                face_count[v as usize] += 1;
            }
        }
        for (f, face) in self.faces.iter().enumerate() {
            for &v in face {
                sum_f[v as usize] = add(sum_f[v as usize], face_points[f]);
            }
        }
        for (a, b, fs) in &edge_faces {
            let mid = scale(add(self.verts[*a as usize], self.verts[*b as usize]), 0.5);
            for (v, other) in [(*a, *b), (*b, *a)] {
                sum_r[v as usize] = add(sum_r[v as usize], mid);
                edge_count[v as usize] += 1;
                if fs.len() == 1 {
                    boundary_sum[v as usize] = add(boundary_sum[v as usize], self.verts[other as usize]);
                    boundary_count[v as usize] += 1;
                }
            }
        }
        let mut verts = Vec::with_capacity(nv + ne + self.faces.len());
        for v in 0..nv {
            let p = self.verts[v];
            if edge_count[v] == 0 {
                verts.push(p);
            } else if boundary_count[v] > 0 {
                /* Boundary (crease) rule: 3/4 the vertex and 1/8 of each of the two boundary neighbours. */
                verts.push(scale(add(scale(p, 6.0), boundary_sum[v]), 1.0 / 8.0));
            } else {
                let n = face_count[v] as f32;
                let f = scale(sum_f[v], 1.0 / n);
                let r = scale(sum_r[v], 1.0 / edge_count[v] as f32);
                verts.push(scale(add(add(f, scale(r, 2.0)), scale(p, n - 3.0)), 1.0 / n));
            }
        }
        verts.extend(edge_points);
        verts.extend(face_points);

        let mut faces = Vec::new();
        for (f, face) in self.faces.iter().enumerate() {
            let k = face.len();
            let edge_of = |a: u32, b: u32| (nv + edge_index[&(a.min(b), a.max(b))]) as u32;
            for i in 0..k {
                let (prev, cur, next) = (face[(i + k - 1) % k], face[i], face[(i + 1) % k]);
                faces.push(vec![cur, edge_of(cur, next), (nv + ne + f) as u32, edge_of(prev, cur)]);
            }
        }
        Mesh { verts, faces }
    }
    }

    fn sphere_like() -> Mesh {
        let mut m = Mesh::cube(2.0).subdivided(2);
        for v in m.verts.iter_mut() {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            *v = [v[0] / l, v[1] / l * 1.3, v[2] / l];
        }
        m
    }

    #[test]
    fn catmull_clark_matches_the_reference() {
        let mut open = Mesh::cube(2.0);
        open.faces.pop(); /* boundary edges */
        for m in [Mesh::cube(2.0), open, sphere_like()] {
            let (a, b) = (m.catmull_clark(), m.catmull_clark_reference());
            assert_eq!(a.faces, b.faces);
            assert_eq!(a.verts, b.verts);
        }
    }

    fn faces_point_outward(m: &Mesh) -> bool {
        (0..m.faces.len()).all(|f| dot(m.face_normal(f), m.face_center(f)) > 0.0)
    }

    #[test]
    fn primitive_vertex_counts_match_blender() {
        assert_eq!(Mesh::uv_sphere(32, 16, 1.0).verts.len(), 482);
        assert_eq!(Mesh::uv_sphere(32, 16, 1.0).faces.len(), 512);
        assert_eq!(Mesh::ico_sphere(2, 1.0).verts.len(), 42);
        assert_eq!(Mesh::ico_sphere(2, 1.0).faces.len(), 80);
        assert_eq!(Mesh::cone(32, 1.0, 1.0, 2.0).verts.len(), 64);
        assert_eq!(Mesh::cone(32, 1.0, 0.0, 2.0).verts.len(), 33);
        assert_eq!(Mesh::torus(48, 12, 1.0, 0.25).verts.len(), 576);
        assert_eq!(Mesh::grid(10, 10, 2.0).verts.len(), 121);
        assert_eq!(Mesh::grid(10, 10, 2.0).faces.len(), 100);
        assert_eq!(Mesh::cube(2.0).verts.len(), 8);
        assert_eq!(Mesh::cube(2.0).faces.len(), 6);
    }

    #[test]
    fn closed_convex_primitives_face_outward() {
        assert!(faces_point_outward(&Mesh::cube(2.0)));
        assert!(faces_point_outward(&Mesh::uv_sphere(32, 16, 1.0)));
        assert!(faces_point_outward(&Mesh::ico_sphere(2, 1.0)));
        assert!(faces_point_outward(&Mesh::cone(32, 1.0, 1.0, 2.0)));
        assert!(faces_point_outward(&Mesh::cone(32, 1.0, 0.0, 2.0)));
    }

    #[test]
    fn torus_faces_point_away_from_the_tube_centre() {
        let m = Mesh::torus(24, 8, 1.0, 0.25);
        for f in 0..m.faces.len() {
            let c = m.face_center(f);
            let ring = normalized([c[0], c[1], 0.0]);
            let tube = sub(c, scale(ring, 1.0));
            assert!(dot(m.face_normal(f), tube) > 0.0, "face {f}");
        }
    }

    #[test]
    fn plane_and_grid_face_up() {
        assert_eq!(Mesh::plane(2.0).face_normal(0), [0.0, 0.0, 1.0]);
        let g = Mesh::grid(4, 4, 2.0);
        assert!((0..g.faces.len()).all(|f| g.face_normal(f)[2] > 0.99));
    }

    #[test]
    fn cube_edges_are_twelve_and_manifold() {
        let edges = Mesh::cube(2.0).edges();
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|e| e.faces == 2));
        assert_eq!(Mesh::plane(2.0).edges().iter().filter(|e| e.faces == 1).count(), 4);
    }

    #[test]
    fn closed_meshes_satisfy_euler() {
        for m in [Mesh::cube(2.0), Mesh::uv_sphere(32, 16, 1.0), Mesh::ico_sphere(2, 1.0), Mesh::cone(16, 1.0, 1.0, 2.0)] {
            let (v, e, f) = (m.verts.len() as i32, m.edges().len() as i32, m.faces.len() as i32);
            assert_eq!(v - e + f, 2);
        }
        let t = Mesh::torus(24, 8, 1.0, 0.25);
        assert_eq!(t.verts.len() as i32 - t.edges().len() as i32 + t.faces.len() as i32, 0);
    }

    /* One Catmull-Clark level of a cube: 8 + 12 + 6 vertices, 6 * 4 quads, and the limit of the cube
     * corners moves inwards (the new corner is (F + 2R + 0 * P) / 3 = (7/9, 7/9, 7/9) for the 2 m cube). */
    #[test]
    fn subdividing_a_cube() {
        let m = Mesh::cube(2.0).subdivided(1);
        assert_eq!(m.verts.len(), 26);
        assert_eq!(m.faces.len(), 24);
        assert!(m.faces.iter().all(|f| f.len() == 4));
        let corner = m.verts[0];
        assert!(corner.iter().all(|c| (c.abs() - 5.0 / 9.0).abs() < 1e-5), "{corner:?}");
        assert!(faces_point_outward(&m));
    }

    #[test]
    fn subdivision_keeps_the_mesh_closed_and_rounder() {
        let m = Mesh::cube(2.0).subdivided(2);
        assert_eq!(m.faces.len(), 96);
        assert!(m.edges().iter().all(|e| e.faces == 2));
        let (_, max) = m.bounds().unwrap();
        assert!(max[0] < 1.0);
    }

    #[test]
    fn subdividing_an_open_mesh_keeps_its_boundary_inside_the_plane() {
        let m = Mesh::plane(2.0).subdivided(1);
        assert_eq!(m.faces.len(), 4);
        assert!(m.verts.iter().all(|v| v[2] == 0.0));
    }

    #[test]
    fn vertex_normals_of_a_sphere_point_outward() {
        let m = Mesh::uv_sphere(16, 8, 1.0);
        for (p, n) in m.verts.iter().zip(m.vertex_normals()) {
            assert!(dot(normalized(*p), n) > 0.99);
        }
    }

    #[test]
    fn triangle_fans_cover_polygons() {
        assert_eq!(Mesh::cube(2.0).triangles().len(), 12);
        assert_eq!(Mesh::circle(32, 1.0).triangles().len(), 30);
    }
}
