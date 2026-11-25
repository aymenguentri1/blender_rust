//! Scene data: objects with transforms, selection, the object operators and undo.
//!
//! Modelled on Blender:
//!  - the startup scene (`Camera`, `Cube`, `Light` in the `Collection`; the cube is selected and active,
//!    which is why the outliner draws it in `TH_ACTIVE_OBJECT`)
//!  - `Object::loc`, `rot` (`ROT_MODE_XYZ`: `R = Rz * Ry * Rx`, `eulO_to_mat3()`) and `scale`
//!    (`BKE_object_to_mat4()`: `T * R * S`)
//!  - `unique_name`: `BLI_uniquename()` adds `.001`, `.002`, ... to a name that is taken
//!  - `OBJECT_OT_select_all`, `OBJECT_OT_delete`, `OBJECT_OT_duplicate`, `OBJECT_OT_hide_view_set`,
//!    `OBJECT_OT_hide_view_clear`, `OBJECT_OT_shade_smooth` / `shade_flat`, `OBJECT_OT_location_clear`,
//!    `OBJECT_OT_rotation_clear`, `OBJECT_OT_scale_clear`, the Subdivision Surface level shortcut
//!    (`OBJECT_OT_subdivision_set`, Ctrl + 0..5)
//!  - `ED_undo_push()`: every operator that changes data pushes a step; here a step is a copy of the object
//!    list (selection changes are not undo steps)
//!
//! Matrices are column-major like Blender's `float[4][4]`: `m[col][row]`.

use crate::mesh::{Mesh, Primitive, V3};
use crate::view3d::{Mat4, RegionView3D};

pub type Mat3 = [[f32; 3]; 3];

const UNDO_MAX: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectData {
    Mesh(Mesh),
    /// Not drawn (there is no camera or light drawing yet), but listed in the outliner.
    Camera,
    Light,
}

#[derive(Clone, Debug)]
pub struct Object {
    /// Stable identifier (never reused), written to the object id buffer as `id + 1`.
    pub id: u32,
    pub name: String,
    pub data: ObjectData,
    pub loc: V3,
    /// Euler angles in radians, XYZ order.
    pub rot: V3,
    pub scale: V3,
    pub selected: bool,
    pub hidden: bool,
    /// `Mesh::polys` smooth flag: shade with vertex normals instead of face normals.
    pub smooth: bool,
    /// Subdivision Surface modifier levels (0 is no modifier).
    pub subsurf: u32,
    eval: Option<Mesh>,
}

impl Object {
    pub fn world_matrix(&self) -> Mat4 {
        let r = euler_to_mat3(self.rot);
        let mut m = [[0.0f32; 4]; 4];
        for c in 0..3 {
            for row in 0..3 {
                m[c][row] = r[c][row] * self.scale[c];
            }
        }
        m[3] = [self.loc[0], self.loc[1], self.loc[2], 1.0];
        m
    }

    pub fn mesh(&self) -> Option<&Mesh> {
        match &self.data {
            ObjectData::Mesh(m) => Some(m),
            _ => None,
        }
    }

    /// The mesh as drawn: the base mesh, or its subdivision once `Scene::update_eval()` has built it.
    pub fn evaluated(&self) -> Option<&Mesh> {
        if self.subsurf > 0 {
            if let Some(m) = self.eval.as_ref() {
                return Some(m);
            }
        }
        self.mesh()
    }

    fn rebuild_eval(&mut self) {
        self.eval = match (&self.data, self.subsurf) {
            (ObjectData::Mesh(m), n) if n > 0 => Some(m.subdivided(n)),
            _ => None,
        };
    }

    /// The bounds of the evaluated mesh in world space.
    pub fn world_bounds(&self) -> Option<(V3, V3)> {
        let (min, max) = self.evaluated()?.bounds()?;
        let m = self.world_matrix();
        let mut out: Option<(V3, V3)> = None;
        for i in 0..8 {
            let corner = [
                if i & 1 == 0 { min[0] } else { max[0] },
                if i & 2 == 0 { min[1] } else { max[1] },
                if i & 4 == 0 { min[2] } else { max[2] },
            ];
            let p = mat4_mul_point(&m, corner);
            out = Some(match out {
                None => (p, p),
                Some((lo, hi)) => (
                    std::array::from_fn(|k| lo[k].min(p[k])),
                    std::array::from_fn(|k| hi[k].max(p[k])),
                ),
            });
        }
        out
    }
}

#[derive(Clone)]
struct Snapshot {
    objects: Vec<Object>,
    active: Option<u32>,
    next_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectAction {
    Select,
    Deselect,
    Invert,
}

#[derive(Clone)]
pub struct Scene {
    pub objects: Vec<Object>,
    /// The active object (`BKE_view_layer_active_object_get()`), by id.
    pub active: Option<u32>,
    next_id: u32,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Default for Scene {
    fn default() -> Self {
        Self::startup()
    }
}

impl Scene {
    /// The factory startup scene.
    pub fn startup() -> Scene {
        let mut s = Scene { objects: Vec::new(), active: None, next_id: 0, undo: Vec::new(), redo: Vec::new() };
        let deg = std::f32::consts::PI / 180.0;
        let camera = s.new_object("Camera", ObjectData::Camera);
        camera.loc = [7.358_891, -6.925_791, 4.958_309];
        camera.rot = [63.559_3 * deg, 0.0, 46.691_9 * deg];
        let cube = s.new_object("Cube", ObjectData::Mesh(Mesh::cube(2.0)));
        cube.selected = true;
        let light = s.new_object("Light", ObjectData::Light);
        light.loc = [4.076_245, 1.005_454, 5.903_862];
        s.active = s.objects.iter().find(|o| o.name == "Cube").map(|o| o.id);
        s
    }

    fn new_object(&mut self, name: &str, data: ObjectData) -> &mut Object {
        let name = self.unique_name(name);
        let id = self.next_id;
        self.next_id += 1;
        self.objects.push(Object {
            id,
            name,
            data,
            loc: [0.0; 3],
            rot: [0.0; 3],
            scale: [1.0; 3],
            selected: false,
            hidden: false,
            smooth: false,
            subsurf: 0,
            eval: None,
        });
        self.objects.last_mut().unwrap()
    }

    /// `BLI_uniquename()` with the `.001` suffix style.
    pub fn unique_name(&self, base: &str) -> String {
        let (stem, _) = split_suffix(base);
        if !self.objects.iter().any(|o| o.name == base) {
            return base.to_string();
        }
        for n in 1..100_000 {
            let candidate = format!("{stem}.{n:03}");
            if !self.objects.iter().any(|o| o.name == candidate) {
                return candidate;
            }
        }
        base.to_string()
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.objects.iter().position(|o| o.id == id)
    }

    pub fn active_object(&self) -> Option<&Object> {
        self.active.and_then(|id| self.index_of(id)).map(|i| &self.objects[i])
    }

    pub fn selected(&self) -> impl Iterator<Item = &Object> {
        self.objects.iter().filter(|o| o.selected && !o.hidden)
    }

    pub fn selected_count(&self) -> usize {
        self.selected().count()
    }

    /// Builds the subdivided meshes the modifier stack needs; call before drawing.
    pub fn update_eval(&mut self) {
        for o in &mut self.objects {
            if o.subsurf > 0 && o.eval.is_none() {
                o.rebuild_eval();
            }
        }
    }

    // ----- undo ---------------------------------------------------------------------------------

    fn snapshot(&self) -> Snapshot {
        Snapshot { objects: self.objects.clone(), active: self.active, next_id: self.next_id }
    }

    fn restore(&mut self, s: Snapshot) {
        self.objects = s.objects;
        self.active = s.active;
        self.next_id = s.next_id;
    }

    /// `ED_undo_push()`: call before the operator changes the scene.
    pub fn push_undo(&mut self) {
        self.undo.push(self.snapshot());
        if self.undo.len() > UNDO_MAX {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Drops the step `push_undo()` made (a cancelled operator).
    pub fn pop_undo(&mut self) {
        self.undo.pop();
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else { return false };
        self.redo.push(self.snapshot());
        self.restore(prev);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(self.snapshot());
        self.restore(next);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // ----- selection ----------------------------------------------------------------------------

    /// `OBJECT_OT_select_all`; hidden objects are not touched. Returns whether anything changed.
    pub fn select_all(&mut self, action: SelectAction) -> bool {
        let mut changed = false;
        for o in self.objects.iter_mut().filter(|o| !o.hidden) {
            let new = match action {
                SelectAction::Select => true,
                SelectAction::Deselect => false,
                SelectAction::Invert => !o.selected,
            };
            changed |= o.selected != new;
            o.selected = new;
        }
        changed
    }

    /// `VIEW3D_OT_select` / the outliner click: `extend` toggles the object (Shift), otherwise only it is
    /// selected; `None` is a click on nothing, which deselects unless `extend`.
    pub fn select_pick(&mut self, hit: Option<u32>, extend: bool) {
        match hit {
            None => {
                if !extend {
                    self.select_all(SelectAction::Deselect);
                }
            }
            Some(id) => {
                let Some(i) = self.index_of(id) else { return };
                if extend {
                    let now = !self.objects[i].selected;
                    self.objects[i].selected = now;
                    if now {
                        self.active = Some(id);
                    } else if self.active == Some(id) {
                        self.active = None;
                    }
                } else {
                    self.select_all(SelectAction::Deselect);
                    self.objects[i].selected = true;
                    self.active = Some(id);
                }
            }
        }
    }

    // ----- operators ----------------------------------------------------------------------------

    /// `OBJECT_OT_*_add`: the new object is the only selected one and becomes active.
    pub fn add_primitive(&mut self, prim: Primitive, location: V3) -> u32 {
        self.push_undo();
        self.select_all(SelectAction::Deselect);
        let o = self.new_object(prim.object_name(), ObjectData::Mesh(prim.build()));
        o.loc = location;
        o.selected = true;
        let id = o.id;
        self.active = Some(id);
        id
    }

    /// `OBJECT_OT_delete`: removes the selected objects. Returns how many.
    pub fn delete_selected(&mut self) -> usize {
        let n = self.selected_count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        let active = self.active;
        self.objects.retain(|o| !(o.selected && !o.hidden));
        if active.is_some_and(|id| self.index_of(id).is_none()) {
            self.active = None;
        }
        n
    }

    /// `OBJECT_OT_duplicate`: the copies are selected (the active one stays active as a copy).
    pub fn duplicate_selected(&mut self) -> usize {
        let sources: Vec<Object> = self.selected().cloned().collect();
        if sources.is_empty() {
            return 0;
        }
        self.push_undo();
        for o in &mut self.objects {
            o.selected = false;
        }
        let active = self.active;
        for src in &sources {
            let name = src.name.clone();
            let copy = self.new_object(&name, src.data.clone());
            copy.loc = src.loc;
            copy.rot = src.rot;
            copy.scale = src.scale;
            copy.smooth = src.smooth;
            copy.subsurf = src.subsurf;
            copy.selected = true;
            if active == Some(src.id) {
                let id = copy.id;
                self.active = Some(id);
            }
        }
        sources.len()
    }

    /// `OBJECT_OT_hide_view_set`: hides the selected objects.
    pub fn hide_selected(&mut self) -> usize {
        let n = self.selected_count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected) {
            o.hidden = true;
            o.selected = false;
        }
        n
    }

    /// `OBJECT_OT_hide_view_clear`: shows every hidden object and selects it.
    pub fn reveal_all(&mut self) -> usize {
        let n = self.objects.iter().filter(|o| o.hidden).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.hidden) {
            o.hidden = false;
            o.selected = true;
        }
        n
    }

    /// `OBJECT_OT_shade_smooth` / `OBJECT_OT_shade_flat` on the selected meshes.
    pub fn set_smooth(&mut self, smooth: bool) -> usize {
        let n = self.selected().filter(|o| o.mesh().is_some()).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden && o.mesh().is_some()) {
            o.smooth = smooth;
        }
        n
    }

    /// `OBJECT_OT_subdivision_set`: the Subdivision Surface level of the selected meshes.
    pub fn set_subsurf(&mut self, levels: u32) -> usize {
        let levels = levels.min(5);
        let n = self.selected().filter(|o| o.mesh().is_some() && o.subsurf != levels).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden && o.mesh().is_some()) {
            if o.subsurf != levels {
                o.subsurf = levels;
                o.eval = None;
            }
        }
        n
    }

    /// `OBJECT_OT_location_clear` (Alt + G), `rotation_clear` (Alt + R), `scale_clear` (Alt + S).
    pub fn clear_transform(&mut self, kind: ClearKind) -> usize {
        let n = self.selected().count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            match kind {
                ClearKind::Location => o.loc = [0.0; 3],
                ClearKind::Rotation => o.rot = [0.0; 3],
                ClearKind::Scale => o.scale = [1.0; 3],
            }
        }
        n
    }

    // ----- queries ------------------------------------------------------------------------------

    /// The median of the selected objects' origins, the default transform pivot.
    pub fn median_origin(&self) -> Option<V3> {
        let sel: Vec<&Object> = self.selected().collect();
        if sel.is_empty() {
            return None;
        }
        let sum = sel.iter().fold([0.0f32; 3], |s, o| [s[0] + o.loc[0], s[1] + o.loc[1], s[2] + o.loc[2]]);
        let n = sel.len() as f32;
        Some([sum[0] / n, sum[1] / n, sum[2] / n])
    }

    /// The world bounds of the visible (or only the selected) mesh objects.
    pub fn bounds(&self, selected_only: bool) -> Option<(V3, V3)> {
        let mut out: Option<(V3, V3)> = None;
        for o in self.objects.iter().filter(|o| !o.hidden && (!selected_only || o.selected)) {
            if let Some((lo, hi)) = o.world_bounds() {
                out = Some(match out {
                    None => (lo, hi),
                    Some((a, b)) => (
                        std::array::from_fn(|k| a[k].min(lo[k])),
                        std::array::from_fn(|k| b[k].max(hi[k])),
                    ),
                });
            }
        }
        out
    }

    /// The nearest visible object under the region pixel `mval` (y up), by casting the pixel ray against the
    /// triangles of the evaluated meshes (back faces count: solid shading draws them).
    pub fn pick(&self, rv3d: &RegionView3D, winx: i32, winy: i32, mval: [i32; 2]) -> Option<u32> {
        let (x, y) = (mval[0] as f32 + 0.5, mval[1] as f32 + 0.5);
        let near = rv3d.unproject_v3(winx, winy, x, y, 0.0)?;
        let far = rv3d.unproject_v3(winx, winy, x, y, 1.0)?;
        let dir = sub(far, near);
        let len = dot(dir, dir).sqrt();
        if len == 0.0 {
            return None;
        }
        let dir = [dir[0] / len, dir[1] / len, dir[2] / len];
        let mut best: Option<(f32, u32)> = None;
        for o in self.objects.iter().filter(|o| !o.hidden) {
            let Some(mesh) = o.evaluated() else { continue };
            let Some((lo, hi)) = o.world_bounds() else { continue };
            if !ray_hits_aabb(near, dir, lo, hi) {
                continue;
            }
            let m = o.world_matrix();
            let world: Vec<V3> = mesh.verts.iter().map(|&v| mat4_mul_point(&m, v)).collect();
            for (_, [a, b, c]) in mesh.triangles() {
                if let Some(t) = ray_triangle(near, dir, world[a as usize], world[b as usize], world[c as usize]) {
                    if best.map_or(true, |(bt, _)| t < bt) {
                        best = Some((t, o.id));
                    }
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// The tree rows of the outliner in the view layer mode: collections, then the objects of the
    /// `Collection` sorted by name.
    pub fn outliner_rows(&self) -> Vec<OutlinerRow> {
        let mut objects: Vec<&Object> = self.objects.iter().collect();
        objects.sort_by_key(|o| o.name.to_lowercase());
        let mut rows = vec![
            OutlinerRow { level: 0, name: "Scene Collection".into(), kind: RowKind::SceneCollection },
            OutlinerRow { level: 1, name: "Collection".into(), kind: RowKind::ActiveCollection },
        ];
        for o in objects {
            rows.push(OutlinerRow { level: 2, name: o.name.clone(), kind: RowKind::Object(o.id) });
        }
        rows.push(OutlinerRow { level: 1, name: "XGen Guides".into(), kind: RowKind::Collection });
        rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearKind {
    Location,
    Rotation,
    Scale,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    SceneCollection,
    /// The active layer collection (drawn with `TH_TEXT_HI`).
    ActiveCollection,
    Collection,
    Object(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlinerRow {
    pub level: i32,
    pub name: String,
    pub kind: RowKind,
}

/// `"Cube.012"` -> `("Cube", Some(12))`.
fn split_suffix(name: &str) -> (&str, Option<u32>) {
    if let Some((stem, tail)) = name.rsplit_once('.') {
        if tail.len() == 3 {
            if let Ok(n) = tail.parse::<u32>() {
                return (stem, Some(n));
            }
        }
    }
    (name, None)
}

// ----- vector and matrix helpers ----------------------------------------------------------------

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn mat3_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut out = [[0.0f32; 3]; 3];
    for c in 0..3 {
        for r in 0..3 {
            out[c][r] = (0..3).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

pub fn mat3_mul_v3(m: &Mat3, v: V3) -> V3 {
    std::array::from_fn(|r| m[0][r] * v[0] + m[1][r] * v[1] + m[2][r] * v[2])
}

/// `eulO_to_mat3()` for `EULER_ORDER_XYZ`: `R = Rz * Ry * Rx`.
pub fn euler_to_mat3(e: V3) -> Mat3 {
    let (sx, cx) = e[0].sin_cos();
    let (sy, cy) = e[1].sin_cos();
    let (sz, cz) = e[2].sin_cos();
    let rx = [[1.0, 0.0, 0.0], [0.0, cx, sx], [0.0, -sx, cx]];
    let ry = [[cy, 0.0, -sy], [0.0, 1.0, 0.0], [sy, 0.0, cy]];
    let rz = [[cz, sz, 0.0], [-sz, cz, 0.0], [0.0, 0.0, 1.0]];
    mat3_mul(&rz, &mat3_mul(&ry, &rx))
}

/// `mat3_normalized_to_eul()`: the XYZ euler angles of a rotation matrix.
pub fn mat3_to_euler(m: &Mat3) -> V3 {
    let cy = (m[0][0] * m[0][0] + m[0][1] * m[0][1]).sqrt();
    if cy > 1e-6 {
        [m[1][2].atan2(m[2][2]), (-m[0][2]).atan2(cy), m[0][1].atan2(m[0][0])]
    } else {
        [(-m[2][1]).atan2(m[1][1]), (-m[0][2]).atan2(cy), 0.0]
    }
}

/// The rotation by `angle` radians about the unit `axis` (`axis_angle_to_mat3()`).
pub fn axis_angle_to_mat3(axis: V3, angle: f32) -> Mat3 {
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    let [x, y, z] = axis;
    [
        [t * x * x + c, t * x * y + s * z, t * x * z - s * y],
        [t * x * y - s * z, t * y * y + c, t * y * z + s * x],
        [t * x * z + s * y, t * y * z - s * x, t * z * z + c],
    ]
}

pub fn mat4_mul_point(m: &Mat4, p: V3) -> V3 {
    std::array::from_fn(|r| m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r])
}

/// The rotation and scale part of `m` applied to a direction.
pub fn mat4_mul_dir(m: &Mat4, d: V3) -> V3 {
    std::array::from_fn(|r| m[0][r] * d[0] + m[1][r] * d[1] + m[2][r] * d[2])
}

/// The inverse transpose of the upper 3x3 of `m`, for transforming normals under non-uniform scale.
pub fn normal_matrix(m: &Mat4) -> Mat3 {
    let a = |c: usize, r: usize| m[c][r];
    let (c0, c1, c2) = ([a(0, 0), a(0, 1), a(0, 2)], [a(1, 0), a(1, 1), a(1, 2)], [a(2, 0), a(2, 1), a(2, 2)]);
    /* The inverse transpose has the cross products of the other two columns as its columns. */
    let (n0, n1, n2) = (cross(c1, c2), cross(c2, c0), cross(c0, c1));
    let det = dot(c0, n0);
    let inv = if det.abs() > 1e-20 { 1.0 / det } else { 1.0 };
    [n0.map(|v| v * inv), n1.map(|v| v * inv), n2.map(|v| v * inv)]
}

fn ray_hits_aabb(origin: V3, dir: V3, lo: V3, hi: V3) -> bool {
    let (mut t0, mut t1) = (f32::NEG_INFINITY, f32::INFINITY);
    for k in 0..3 {
        if dir[k].abs() < 1e-12 {
            if origin[k] < lo[k] - 1e-5 || origin[k] > hi[k] + 1e-5 {
                return false;
            }
        } else {
            let (a, b) = ((lo[k] - 1e-5 - origin[k]) / dir[k], (hi[k] + 1e-5 - origin[k]) / dir[k]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    t0 <= t1 && t1 >= 0.0
}

/// Möller-Trumbore, double sided; the distance along `dir` of the hit in front of the origin.
fn ray_triangle(origin: V3, dir: V3, a: V3, b: V3, c: V3) -> Option<f32> {
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(dir, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, a);
    let u = dot(s, p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t > 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view3d::View3D;

    fn approx(a: V3, b: V3) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
    }

    #[test]
    fn startup_scene_matches_the_default_file() {
        let s = Scene::startup();
        let names: Vec<&str> = s.objects.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Camera", "Cube", "Light"]);
        assert_eq!(s.active_object().map(|o| o.name.as_str()), Some("Cube"));
        assert_eq!(s.selected().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Cube"]);
        assert!(!s.can_undo());
    }

    #[test]
    fn outliner_rows_are_sorted_and_keep_the_startup_layout() {
        let rows = Scene::startup().outliner_rows();
        let names: Vec<(i32, &str)> = rows.iter().map(|r| (r.level, r.name.as_str())).collect();
        assert_eq!(
            names,
            [(0, "Scene Collection"), (1, "Collection"), (2, "Camera"), (2, "Cube"), (2, "Light"), (1, "XGen Guides")]
        );
    }

    #[test]
    fn unique_names_count_up() {
        let mut s = Scene::startup();
        assert_eq!(s.unique_name("Cube"), "Cube.001");
        s.add_primitive(Primitive::Cube, [0.0; 3]);
        assert_eq!(s.objects.last().unwrap().name, "Cube.001");
        s.add_primitive(Primitive::Cube, [0.0; 3]);
        assert_eq!(s.objects.last().unwrap().name, "Cube.002");
        assert_eq!(s.unique_name("Torus"), "Torus");
    }

    #[test]
    fn adding_selects_only_the_new_object() {
        let mut s = Scene::startup();
        let id = s.add_primitive(Primitive::UvSphere, [1.0, 2.0, 3.0]);
        assert_eq!(s.active, Some(id));
        assert_eq!(s.selected_count(), 1);
        let o = s.active_object().unwrap();
        assert_eq!((o.name.as_str(), o.loc), ("Sphere", [1.0, 2.0, 3.0]));
    }

    #[test]
    fn undo_and_redo_restore_the_scene() {
        let mut s = Scene::startup();
        s.add_primitive(Primitive::Torus, [0.0; 3]);
        assert_eq!(s.objects.len(), 4);
        assert!(s.undo());
        assert_eq!(s.objects.len(), 3);
        assert_eq!(s.active_object().unwrap().name, "Cube");
        assert!(s.redo());
        assert_eq!(s.objects.len(), 4);
        assert!(!s.redo());
        assert!(s.undo() && !s.undo());
    }

    #[test]
    fn a_new_operation_clears_the_redo_stack() {
        let mut s = Scene::startup();
        s.add_primitive(Primitive::Plane, [0.0; 3]);
        s.undo();
        assert!(s.can_redo());
        s.add_primitive(Primitive::Cone, [0.0; 3]);
        assert!(!s.can_redo());
    }

    #[test]
    fn delete_removes_selected_and_clears_active() {
        let mut s = Scene::startup();
        assert_eq!(s.delete_selected(), 1);
        assert_eq!(s.objects.len(), 2);
        assert_eq!(s.active, None);
        assert_eq!(s.delete_selected(), 0);
        s.undo();
        assert_eq!(s.objects.len(), 3);
        assert_eq!(s.active_object().unwrap().name, "Cube");
    }

    #[test]
    fn duplicate_selects_the_copy_and_keeps_the_active_role() {
        let mut s = Scene::startup();
        s.objects[1].loc = [1.0, 0.0, 0.0];
        assert_eq!(s.duplicate_selected(), 1);
        assert_eq!(s.objects.len(), 4);
        let copy = s.active_object().unwrap();
        assert_eq!((copy.name.as_str(), copy.loc, copy.selected), ("Cube.001", [1.0, 0.0, 0.0], true));
        assert!(!s.objects.iter().find(|o| o.name == "Cube").unwrap().selected);
    }

    #[test]
    fn hide_and_reveal() {
        let mut s = Scene::startup();
        assert_eq!(s.hide_selected(), 1);
        assert_eq!(s.selected_count(), 0);
        assert!(s.bounds(false).is_none());
        assert_eq!(s.reveal_all(), 1);
        assert_eq!(s.selected_count(), 1);
        assert!(s.bounds(false).is_some());
    }

    #[test]
    fn select_all_invert_and_pick() {
        let mut s = Scene::startup();
        assert!(s.select_all(SelectAction::Invert));
        assert_eq!(s.selected().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Camera", "Light"]);
        let cube = s.objects[1].id;
        s.select_pick(Some(cube), true);
        assert_eq!(s.selected_count(), 3);
        assert_eq!(s.active, Some(cube));
        s.select_pick(Some(cube), true);
        assert_eq!(s.selected_count(), 2);
        assert_eq!(s.active, None);
        s.select_pick(Some(cube), false);
        assert_eq!(s.selected().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Cube"]);
        s.select_pick(None, false);
        assert_eq!(s.selected_count(), 0);
    }

    #[test]
    fn euler_round_trips() {
        for e in [[0.3, -0.7, 1.9], [-2.0, 0.4, 0.1], [0.0, 0.0, 0.0], [1.0, 1.2, -3.0]] {
            let back = mat3_to_euler(&euler_to_mat3(e));
            let (a, b) = (euler_to_mat3(e), euler_to_mat3(back));
            for c in 0..3 {
                for r in 0..3 {
                    assert!((a[c][r] - b[c][r]).abs() < 1e-4, "{e:?} -> {back:?}");
                }
            }
        }
    }

    #[test]
    fn euler_rotates_x_first() {
        /* 90 degrees about X sends +Y to +Z; then 90 degrees about Z sends +Z to +Z. */
        let m = euler_to_mat3([std::f32::consts::FRAC_PI_2, 0.0, std::f32::consts::FRAC_PI_2]);
        assert!(approx(mat3_mul_v3(&m, [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]));
        assert!(approx(mat3_mul_v3(&m, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    }

    #[test]
    fn world_matrix_is_translate_rotate_scale() {
        let o = Scene::startup().objects[1].clone();
        let mut o = o;
        o.loc = [1.0, 2.0, 3.0];
        o.scale = [2.0, 1.0, 1.0];
        o.rot = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        /* (1, 0, 0) scaled to (2, 0, 0), rotated to (0, 2, 0), moved to (1, 4, 3). */
        assert!(approx(mat4_mul_point(&o.world_matrix(), [1.0, 0.0, 0.0]), [1.0, 4.0, 3.0]));
    }

    #[test]
    fn normal_matrix_of_non_uniform_scale() {
        let mut o = Scene::startup().objects[1].clone();
        o.scale = [2.0, 1.0, 1.0];
        let n = mat3_mul_v3(&normal_matrix(&o.world_matrix()), [1.0, 1.0, 0.0]);
        assert!(approx(n, [0.5, 1.0, 0.0]), "{n:?}");
    }

    #[test]
    fn subsurf_builds_the_evaluated_mesh() {
        let mut s = Scene::startup();
        assert_eq!(s.set_subsurf(2), 1);
        s.update_eval();
        let cube = s.active_object().unwrap();
        assert_eq!(cube.evaluated().unwrap().faces.len(), 96);
        assert_eq!(cube.mesh().unwrap().faces.len(), 6);
        assert_eq!(s.set_subsurf(2), 0);
        s.undo();
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 6);
    }

    #[test]
    fn clear_transform_resets_the_selection() {
        let mut s = Scene::startup();
        s.objects[1].loc = [3.0, 0.0, 0.0];
        s.objects[1].rot = [1.0, 0.0, 0.0];
        s.objects[1].scale = [2.0; 3];
        s.clear_transform(ClearKind::Location);
        s.clear_transform(ClearKind::Rotation);
        s.clear_transform(ClearKind::Scale);
        let c = &s.objects[1];
        assert_eq!((c.loc, c.rot, c.scale), ([0.0; 3], [0.0; 3], [1.0; 3]));
    }

    /* The default view looks at the cube at the origin: the pixel at the centre of the region hits it, a corner
     * pixel does not. */
    #[test]
    fn picking_hits_the_cube_in_the_view_centre() {
        let (w, h) = (200, 100);
        let mut rv3d = RegionView3D::default();
        rv3d.viewquat = crate::view3d::STARTUP_VIEWQUAT;
        rv3d.update(&View3D::default(), w, h);
        let s = Scene::startup();
        assert_eq!(s.pick(&rv3d, w, h, [100, 50]), Some(s.objects[1].id));
        assert_eq!(s.pick(&rv3d, w, h, [2, 2]), None);
    }

    #[test]
    fn picking_prefers_the_nearer_object() {
        let (w, h) = (200, 100);
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w, h);
        let mut s = Scene::startup();
        /* `viewinv[2]` points from the view centre (the origin) to the eye: a sphere 3 m along it sits between
         * the eye (10 m away) and the cube and covers the centre pixel. */
        let near = rv3d.viewinv[2];
        let id = s.add_primitive(Primitive::UvSphere, [near[0] * 3.0, near[1] * 3.0, near[2] * 3.0]);
        let hit = s.pick(&rv3d, w, h, [100, 50]);
        assert_eq!(hit, Some(id));
    }

    #[test]
    fn ray_triangle_is_double_sided() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(ray_triangle([0.2, 0.2, 1.0], [0.0, 0.0, -1.0], a, b, c).is_some());
        assert!(ray_triangle([0.2, 0.2, -1.0], [0.0, 0.0, 1.0], a, b, c).is_some());
        assert!(ray_triangle([2.0, 2.0, 1.0], [0.0, 0.0, -1.0], a, b, c).is_none());
        assert!(ray_triangle([0.2, 0.2, 1.0], [0.0, 0.0, 1.0], a, b, c).is_none());
    }
}
