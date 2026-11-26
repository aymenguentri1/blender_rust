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
//!  - the modifier stack (`Object::modifiers`: Mirror, Array, Solidify, then the Subdivision Surface level)
//!  - `OBJECT_OT_join` (Ctrl + J), `OBJECT_OT_transform_apply` (Ctrl + A), `OBJECT_OT_origin_set` (Geometry to
//!    Origin, median center), `OBJECT_OT_convert` (Alt + C, applies the modifiers), and the mesh operators
//!    Flip Normals (Alt + N), Recalculate Outside (Shift + N), Triangulate (Ctrl + T), Merge by Distance
//!  - the Bevel modifier, `mesh.subdivide`, `mesh.separate` (loose parts), `VIEW3D_OT_snap_cursor_to_selected`,
//!    `snap_selected_to_cursor` and Origin to 3D Cursor
//!  - `ED_undo_push()`: every operator that changes data pushes a step; here a step is a copy of the object
//!    list (selection changes are not undo steps)
//!
//! Matrices are column-major like Blender's `float[4][4]`: `m[col][row]`.

use crate::mesh::{Mesh, Primitive, V3};
use crate::modifiers::{self, Modifier};
use crate::view3d::{Mat4, RegionView3D};

pub mod ops;
mod parent_drop;
pub use ops::{snap_to_grid, AlignMode, AlignTo, ApplyMask, GroupKind, Randomize};

pub type Mat3 = [[f32; 3]; 3];

const IDENTITY4: Mat4 = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
const MAX_PARENT_DEPTH: u32 = 64;

const UNDO_MAX: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectData {
    Mesh(Mesh),
    /// Drawn as a frustum by `overlay_extra`.
    Camera,
    /// Drawn as a circle (and the shape of its type) by `overlay_extra`.
    Light(LightKind),
    /// `OB_EMPTY` with its `empty_drawtype`, drawn by `overlay_extra`.
    Empty(EmptyKind),
}

/// `Light.type` (`eLightType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    Point,
    Sun,
    Spot,
    Area,
}

impl LightKind {
    pub const ALL: [LightKind; 4] = [LightKind::Point, LightKind::Sun, LightKind::Spot, LightKind::Area];

    /// `get_light_defname()`: the name of a new light object.
    pub fn label(self) -> &'static str {
        match self {
            LightKind::Point => "Point",
            LightKind::Sun => "Sun",
            LightKind::Spot => "Spot",
            LightKind::Area => "Area",
        }
    }
}

/// `Object.empty_display_type` (the Image type is not modelled); the size is always 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyKind {
    PlainAxes,
    Arrows,
    SingleArrow,
    Circle,
    Cube,
    Sphere,
    Cone,
}

impl EmptyKind {
    pub const ALL: [EmptyKind; 7] = [
        EmptyKind::PlainAxes,
        EmptyKind::Arrows,
        EmptyKind::SingleArrow,
        EmptyKind::Circle,
        EmptyKind::Cube,
        EmptyKind::Sphere,
        EmptyKind::Cone,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EmptyKind::PlainAxes => "Plain Axes",
            EmptyKind::Arrows => "Arrows",
            EmptyKind::SingleArrow => "Single Arrow",
            EmptyKind::Circle => "Circle",
            EmptyKind::Cube => "Cube",
            EmptyKind::Sphere => "Sphere",
            EmptyKind::Cone => "Cone",
        }
    }
}

/// The Ctrl + C buffer: objects as they were when copied.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub objects: Vec<Object>,
    pub active: Option<u32>,
}

/// The outliner restriction columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Restrict {
    Viewport,
    Select,
    Render,
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
    /// `OB_HIDE_SELECT` (outliner arrow column): the object cannot be selected in the viewport.
    pub hide_select: bool,
    /// `OB_HIDE_RENDER` (outliner camera column): the object is left out of renders.
    pub hide_render: bool,
    /// `Mesh::polys` smooth flag: shade with vertex normals instead of face normals.
    pub smooth: bool,
    /// Subdivision Surface modifier levels (0 is no modifier).
    pub subsurf: u32,
    /// The modifier stack, evaluated in order before the Subdivision Surface level.
    pub modifiers: Vec<Modifier>,
    /// The modifier the editing keys act on (clamped to the stack when used).
    pub active_modifier: usize,
    /// Panel state per modifier (see `modifiers::ModFlags`); may be shorter than the stack, the rest is default.
    pub mod_flags: Vec<modifiers::ModFlags>,
    /// The parent object (by id). `world = parent_world * parent_inv * local` like `BKE_object_get_world_matrix`.
    pub parent: Option<u32>,
    /// `Object::parentinv`: the inverse of the parent's world matrix when the parent was set, so that parenting
    /// does not move the child.
    pub parent_inv: Mat4,
    /// The parent's world matrix as of the last `Scene::refresh_parents()` (identity without a parent).
    pub parent_world: Mat4,
    /// The sculpt mask (`CD_PAINT_MASK`): 0 to 1 per vertex of the base mesh, 1 is fully masked (protected from
    /// brushes). Empty when nothing is masked; a length that differs from the vertex count counts as no mask.
    pub mask: Vec<f32>,
    /// `Object::mat`/`totcol`/`actcol`: indices into `Scene::materials`.
    pub material_slots: crate::material::MaterialSlots,
    eval: Option<Mesh>,
    /// What the cached `eval` was built against besides the object itself (the targets of its Shrinkwrap and Boolean
    /// modifiers and the transforms involved); 0 when the stack has no such dependency.
    eval_sig: u64,
}

impl Object {
    pub fn world_matrix(&self) -> Mat4 {
        let local = self.local_matrix();
        if self.parent.is_some() {
            mat4_mul(&self.parent_world, &mat4_mul(&self.parent_inv, &local))
        } else {
            local
        }
    }

    /// The inverse of the linear part of the parent space (`parent_world * parent_inv`): turns a world space
    /// offset into an offset of `loc`. Identity without a parent.
    pub fn parent_lin_inv(&self) -> Mat3 {
        if self.parent.is_none() {
            return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        }
        let m = mat4_mul(&self.parent_world, &self.parent_inv);
        let inv = crate::view3d::invert_m4_m4(&m);
        [[inv[0][0], inv[0][1], inv[0][2]], [inv[1][0], inv[1][1], inv[1][2]], [inv[2][0], inv[2][1], inv[2][2]]]
    }

    /// The matrix of the object's own location, rotation and scale (`BKE_object_to_mat4()`).
    pub fn local_matrix(&self) -> Mat4 {
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

    /// The size of the evaluated mesh bounds in the object's own (unscaled) space; zero without a mesh.
    pub fn local_size(&self) -> V3 {
        match self.evaluated().and_then(|m| m.bounds()) {
            Some((lo, hi)) => [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]],
            None => [0.0; 3],
        }
    }

    /// `Object.dimensions`: the bounds size times the scale, `None` for an object without a mesh.
    pub fn dimensions(&self) -> Option<V3> {
        self.mesh()?;
        let s = self.local_size();
        Some([s[0] * self.scale[0].abs(), s[1] * self.scale[1].abs(), s[2] * self.scale[2].abs()])
    }

    pub fn mesh(&self) -> Option<&Mesh> {
        match &self.data {
            ObjectData::Mesh(m) => Some(m),
            _ => None,
        }
    }

    /// The mesh as drawn: the base mesh, or its subdivision once `Scene::update_eval()` has built it.
    pub fn evaluated(&self) -> Option<&Mesh> {
        if self.has_eval() {
            if let Some(m) = self.eval.as_ref() {
                return Some(m);
            }
        }
        self.mesh()
    }

    /// The name modifier `i` shows in its panel: the one typed into the header or the kind's name.
    pub fn modifier_name(&self, i: usize) -> Option<String> {
        let m = self.modifiers.get(i)?;
        Some(self.mod_flags.get(i).and_then(|f| f.name.clone()).unwrap_or_else(|| m.label().to_string()))
    }

    /// Is modifier `i` shown in the viewport?
    pub fn modifier_shown(&self, i: usize) -> bool {
        !self.mod_flags.get(i).is_some_and(|f| f.hide_viewport)
    }

    /// Is modifier `i` enabled in edit mode (`show_in_editmode`)?
    pub fn modifier_editmode(&self, i: usize) -> bool {
        !self.mod_flags.get(i).is_some_and(|f| f.hide_editmode)
    }

    /// Is modifier `i` enabled for rendering (`show_render`)?
    pub fn modifier_render(&self, i: usize) -> bool {
        !self.mod_flags.get(i).is_some_and(|f| f.hide_render)
    }

    /// Is the panel of modifier `i` collapsed?
    pub fn modifier_collapsed(&self, i: usize) -> bool {
        self.mod_flags.get(i).is_some_and(|f| f.collapsed)
    }

    /// Does the object draw something other than its base mesh (a modifier or a subdivision level)?
    pub fn has_eval(&self) -> bool {
        self.subsurf > 0 || !self.modifiers.is_empty()
    }

    fn rebuild_eval(&mut self, ctx: &modifiers::EvalCtx) {
        self.eval = match &self.data {
            ObjectData::Mesh(m) if self.has_eval() => {
                let stacked = modifiers::evaluate_flagged(m, &self.modifiers, &self.mod_flags, ctx);
                Some(if self.subsurf > 0 { stacked.subdivided(self.subsurf) } else { stacked })
            }
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
    local_view: Option<Vec<u32>>,
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
    /// Seed of the next Select Random (it counts up so that repeating the operator picks another set).
    select_seed: u32,
    /// `Scene::r.cfra`, the current frame (the Build modifier depends on it); kept out of the undo steps.
    frame: i32,
    /// Local View (`VIEW3D_OT_localview`): the ids of the objects this mode hid, `None` outside of it. They are kept
    /// apart from the objects the user hid, so leaving gives back exactly the former.
    local_view: Option<Vec<u32>>,
    /// `Main::materials`, referenced by index from the object slots (not part of the undo steps yet).
    pub materials: Vec<crate::material::Material>,
}

impl Default for Scene {
    fn default() -> Self {
        Self::startup()
    }
}

impl Scene {
    /// The factory startup scene.
    pub fn startup() -> Scene {
        let mut s = Scene { objects: Vec::new(), active: None, next_id: 0, undo: Vec::new(), redo: Vec::new(), select_seed: 0, frame: 1, local_view: None, materials: vec![crate::material::Material::new("Material")] };
        let deg = std::f32::consts::PI / 180.0;
        let camera = s.new_object("Camera", ObjectData::Camera);
        camera.loc = [7.358_891, -6.925_791, 4.958_309];
        camera.rot = [63.559_3 * deg, 0.0, 46.691_9 * deg];
        let cube = s.new_object("Cube", ObjectData::Mesh(Mesh::cube(2.0)));
        cube.selected = true;
        /* The startup cube carries the default "Material" in its one slot. */
        cube.material_slots.slot_add(true);
        cube.material_slots.assign(Some(0), 1);
        let light = s.new_object("Light", ObjectData::Light(LightKind::Point));
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
            hide_select: false,
            hide_render: false,
            smooth: false,
            subsurf: 0,
            modifiers: Vec::new(),
            active_modifier: 0,
            mod_flags: Vec::new(),
            parent: None,
            parent_inv: IDENTITY4,
            parent_world: IDENTITY4,
            mask: Vec::new(),
            material_slots: Default::default(),
            eval: None,
            eval_sig: 0,
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

    /// The base mesh of object `id` for sculpting; drops its evaluated mesh (the caller runs `update_eval()`).
    pub fn sculpt_mesh_mut(&mut self, id: u32) -> Option<&mut Mesh> {
        let i = self.index_of(id)?;
        let o = &mut self.objects[i];
        o.eval = None;
        match &mut o.data {
            ObjectData::Mesh(m) => Some(m),
            _ => None,
        }
    }

    /// The base mesh of object `id` with its mask for sculpting, the mask resized to the vertex count (zeros when
    /// it did not fit). Drops the evaluated mesh like `sculpt_mesh_mut()`.
    pub fn sculpt_mesh_mask_mut(&mut self, id: u32) -> Option<(&mut Mesh, &mut Vec<f32>)> {
        let i = self.index_of(id)?;
        let o = &mut self.objects[i];
        o.eval = None;
        match &mut o.data {
            ObjectData::Mesh(m) => {
                if o.mask.len() != m.verts.len() {
                    o.mask = vec![0.0; m.verts.len()];
                }
                Some((m, &mut o.mask))
            }
            _ => None,
        }
    }

    /// `paint.mask_flood_fill(mode='VALUE', value=0)` (Alt + M): clears the mask of the active object. One undo step;
    /// false when there was no mask.
    pub fn clear_mask_active(&mut self) -> bool {
        let Some(i) = self.active.and_then(|id| self.index_of(id)) else { return false };
        if !self.objects[i].mask.iter().any(|&m| m > 0.0) {
            return false;
        }
        self.push_undo();
        self.objects[i].mask.clear();
        true
    }

    /// `paint.mask_flood_fill(mode='INVERT')` (Ctrl + I): every mask value `m` becomes `1 - m` (an empty mask
    /// masks the whole mesh). One undo step.
    pub fn invert_mask_active(&mut self) -> bool {
        let Some(i) = self.active.and_then(|id| self.index_of(id)) else { return false };
        let n = match self.objects[i].mesh() {
            Some(m) => m.verts.len(),
            None => return false,
        };
        self.push_undo();
        let o = &mut self.objects[i];
        if o.mask.len() != n {
            o.mask = vec![0.0; n];
        }
        for m in o.mask.iter_mut() {
            *m = 1.0 - *m;
        }
        o.eval = None;
        true
    }

    /// `sculpt.mask_filter`: one pass of `filter` over the mask of the active object. One undo step; false (and
    /// no step) when nothing is masked.
    pub fn mask_filter_active(&mut self, filter: crate::sculpt::MaskFilter) -> bool {
        let Some(i) = self.active.and_then(|id| self.index_of(id)) else { return false };
        if !self.objects[i].mask.iter().any(|&m| m > 0.0) {
            return false;
        }
        let Some(adj) = self.objects[i].mesh().map(crate::sculpt::adjacency) else { return false };
        if adj.adj.len() != self.objects[i].mask.len() {
            return false;
        }
        self.push_undo();
        crate::sculpt::filter_mask(&mut self.objects[i].mask, &adj, filter);
        true
    }

    /// Whether the active object has any masked vertex.
    pub fn has_mask(&self) -> bool {
        self.active_object().is_some_and(|o| o.mask.iter().any(|&m| m > 0.0))
    }

    /// Subdivides the base mesh of the active object once (Catmull-Clark) so that it has enough vertices to sculpt
    /// on. One undo step; refused above 400000 vertices.
    pub fn subdivide_active_base(&mut self) -> bool {
        let Some(id) = self.active else { return false };
        let Some(i) = self.index_of(id) else { return false };
        let new = match &self.objects[i].data {
            ObjectData::Mesh(m) if !m.faces.is_empty() && m.verts.len() * 3 < 400_000 => m.subdivided(1),
            _ => return false,
        };
        self.push_undo();
        self.objects[i].eval = None;
        self.objects[i].data = ObjectData::Mesh(new);
        true
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
        self.refresh_parents();
        for i in 0..self.objects.len() {
            if !self.objects[i].has_eval() {
                continue;
            }
            let (ctx, sig) = self.eval_ctx(i);
            if self.objects[i].eval.is_none() || self.objects[i].eval_sig != sig {
                self.objects[i].rebuild_eval(&ctx);
                self.objects[i].eval_sig = sig;
            }
        }
    }

    /// The scene context of the modifier stack of object `i` (the Shrinkwrap targets in its local space) and a
    /// signature of everything that context depends on, so that moving a target rebuilds the object.
    fn eval_ctx(&self, i: usize) -> (modifiers::EvalCtx, u64) {
        use std::hash::{Hash, Hasher};
        let mut ctx = modifiers::EvalCtx { frame: self.frame as f32, ..Default::default() };
        let owner = &self.objects[i];
        let timed = owner.modifiers.iter().any(|m| matches!(m, Modifier::Build { .. }));
        let targets: Vec<u32> = owner
            .modifiers
            .iter()
            .filter_map(|m| match m {
                Modifier::Shrinkwrap { target: Some(t), .. } | Modifier::Boolean { target: Some(t), .. } => Some(*t),
                _ => None,
            })
            .collect();
        if targets.is_empty() && !timed {
            return (ctx, 0);
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        if timed {
            self.frame.hash(&mut h);
        }
        let to_local = crate::view3d::invert_m4_m4(&owner.world_matrix());
        let bits = |m: &Mat4| m.iter().flatten().map(|f| f.to_bits()).collect::<Vec<u32>>();
        bits(&to_local).hash(&mut h);
        for t in targets {
            let Some(o) = self.index_of(t).map(|j| &self.objects[j]).filter(|o| o.id != owner.id) else { continue };
            let Some(mesh) = o.mesh() else { continue };
            let tw = o.world_matrix();
            t.hash(&mut h);
            bits(&tw).hash(&mut h);
            for v in &mesh.verts {
                v.map(f32::to_bits).hash(&mut h);
            }
            mesh.faces.hash(&mut h);
            let local = mesh.mapped(|p| mat4_mul_point(&to_local, mat4_mul_point(&tw, p)), false);
            ctx.targets.insert(t, local);
        }
        (ctx, h.finish().max(1))
    }

    // ----- undo ---------------------------------------------------------------------------------

    fn snapshot(&self) -> Snapshot {
        Snapshot { objects: self.objects.clone(), active: self.active, next_id: self.next_id, local_view: self.local_view.clone() }
    }

    fn restore(&mut self, s: Snapshot) {
        self.objects = s.objects;
        self.active = s.active;
        self.next_id = s.next_id;
        self.local_view = s.local_view;
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

    /// `VIEW3D_OT_select_box`: the visible objects touched by the region rectangle `rect` (`[xmin, xmax, ymin,
    /// ymax]`, y up) join, leave or replace the selection. `through` (X-ray or wire shading) tests the projected
    /// triangles; otherwise a grid of pick rays finds what is visible. Cameras, lights and empties are hit by
    /// their origin. The active object is left alone. Returns whether the selection changed.
    pub fn select_box(&mut self, rv3d: &RegionView3D, winx: i32, winy: i32, rect: [i32; 4], mode: crate::select_box::BoxMode, through: bool) -> bool {
        let mut hits: Vec<u32> = Vec::new();
        if !through {
            let step = crate::select_box::sample_step(rect);
            let mut y = rect[2];
            while y <= rect[3] {
                let mut x = rect[0];
                while x <= rect[1] {
                    if let Some(id) = self.pick(rv3d, winx, winy, [x, y]) {
                        if !hits.contains(&id) {
                            hits.push(id);
                        }
                    }
                    x += step;
                }
                y += step;
            }
        }
        for o in self.objects.iter().filter(|o| !o.hidden) {
            let m = o.world_matrix();
            let mesh = o.evaluated().filter(|_| matches!(o.data, ObjectData::Mesh(_)));
            let hit = match mesh {
                Some(mesh) => {
                    if !through {
                        continue;
                    }
                    let screen: Vec<Option<[i32; 2]>> = mesh.verts.iter().map(|&v| rv3d.project_int_clip_near(winx, winy, mat4_mul_point(&m, v))).collect();
                    let inside = |p: [i32; 2]| p[0] >= rect[0] && p[0] <= rect[1] && p[1] >= rect[2] && p[1] <= rect[3];
                    screen.iter().flatten().any(|&p| inside(p))
                        || mesh.triangles().iter().any(|(_, t)| {
                            let pts = [screen[t[0] as usize], screen[t[1] as usize], screen[t[2] as usize]];
                            match pts {
                                [Some(a), Some(b), Some(c)] => crate::select_box::triangle_hits_rect(&[[a[0] as f32, a[1] as f32], [b[0] as f32, b[1] as f32], [c[0] as f32, c[1] as f32]], rect),
                                _ => false,
                            }
                        })
                }
                None => rv3d.project_int_clip_near(winx, winy, mat4_mul_point(&m, [0.0, 0.0, 0.0])).is_some_and(|p| p[0] >= rect[0] && p[0] <= rect[1] && p[1] >= rect[2] && p[1] <= rect[3]),
            };
            if hit && !hits.contains(&o.id) {
                hits.push(o.id);
            }
        }
        let mut changed = false;
        if mode == crate::select_box::BoxMode::Set {
            changed |= self.select_all(SelectAction::Deselect);
        }
        for o in self.objects.iter_mut().filter(|o| hits.contains(&o.id)) {
            let new = mode != crate::select_box::BoxMode::Sub;
            changed |= o.selected != new;
            o.selected = new;
        }
        changed
    }

    // ----- operators ----------------------------------------------------------------------------

    /// `OBJECT_OT_*_add`: the new object is the only selected one and becomes active.
    pub fn add_primitive(&mut self, prim: Primitive, location: V3) -> u32 {
        self.add_item(crate::add_menu::AddItem::Mesh(prim), location)
    }

    /// `OBJECT_OT_*_add` for every entry of the Add menu (mesh primitives, `object.empty_add` (every type but Image),
    /// `object.camera_add`, `object.light_add` (every type)): the new object is aligned to the world, selected and
    /// active, and the others are deselected.
    pub fn add_item(&mut self, item: crate::add_menu::AddItem, location: V3) -> u32 {
        use crate::add_menu::AddItem;
        self.push_undo();
        self.select_all(SelectAction::Deselect);
        let data = match item {
            AddItem::Mesh(p) => ObjectData::Mesh(p.build()),
            AddItem::Empty(k) => ObjectData::Empty(k),
            AddItem::Camera => ObjectData::Camera,
            AddItem::Light(k) => ObjectData::Light(k),
        };
        let o = self.new_object(item.object_name(), data);
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
        self.refresh_parents();
        let active = self.active;
        let gone: Vec<u32> = self.objects.iter().filter(|o| o.selected && !o.hidden).map(|o| o.id).collect();
        for i in 0..self.objects.len() {
            if self.objects[i].parent.is_some_and(|p| gone.contains(&p)) && !gone.contains(&self.objects[i].id) {
                self.bake_world_and_unparent(i);
            }
        }
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
        let active = self.active;
        self.spawn_copies(&sources, active)
    }

    /// New selected copies of `sources` (the active one becomes the copy of `active`); the old selection is
    /// cleared. A copy whose parent was copied too is parented to that copy (`BKE_object_duplicate` remaps it);
    /// a parent that no longer exists is dropped. Pushes no undo step.
    fn spawn_copies(&mut self, sources: &[Object], active: Option<u32>) -> usize {
        for o in &mut self.objects {
            o.selected = false;
        }
        let mut copies: Vec<(u32, u32)> = Vec::new();
        for src in sources {
            let name = src.name.clone();
            let copy = self.new_object(&name, src.data.clone());
            copy.loc = src.loc;
            copy.rot = src.rot;
            copy.scale = src.scale;
            copy.smooth = src.smooth;
            copy.subsurf = src.subsurf;
            copy.modifiers = src.modifiers.clone();
            copy.mod_flags = src.mod_flags.clone();
            copy.selected = true;
            copy.parent = src.parent;
            copy.parent_inv = src.parent_inv;
            let id = copy.id;
            copies.push((src.id, id));
            if active == Some(src.id) {
                self.active = Some(id);
            }
        }
        for &(_, new) in &copies {
            let Some(i) = self.index_of(new) else { continue };
            if let Some(p) = self.objects[i].parent {
                if let Some(&(_, np)) = copies.iter().find(|(o, _)| *o == p) {
                    self.objects[i].parent = Some(np);
                } else if self.index_of(p).is_none() {
                    self.objects[i].parent = None;
                    self.objects[i].parent_inv = IDENTITY4;
                }
            }
        }
        self.refresh_parents();
        sources.len()
    }

    /// `VIEW3D_OT_copybuffer` (Ctrl + C): the selected objects (and which one is active) go to the clipboard.
    pub fn copy_selected(&self) -> Option<Clipboard> {
        let objects: Vec<Object> = self.selected().cloned().collect();
        if objects.is_empty() {
            return None;
        }
        let active = self.active.filter(|a| objects.iter().any(|o| o.id == *a));
        Some(Clipboard { objects, active })
    }

    /// `VIEW3D_OT_pastebuffer` (Ctrl + V): adds the clipboard objects at their copied transforms, named like new
    /// objects (`Cube.001`), selected and active as copied, as one undo step. A parent that was not copied is kept
    /// while it still exists in the scene. Returns the number of objects pasted.
    pub fn paste(&mut self, clip: &Clipboard) -> usize {
        if clip.objects.is_empty() {
            return 0;
        }
        self.push_undo();
        let n = self.spawn_copies(&clip.objects, clip.active);
        /* A buffer without an active object leaves no active object among the pasted ones. */
        if clip.active.is_none() {
            self.active = None;
        }
        n
    }

    /// `rna_Object_name_set` for object `id` (the outliner rename field): same rules as `rename_active`.
    pub fn rename_object(&mut self, id: u32, name: &str) -> bool {
        let name = name.trim();
        let Some(i) = self.index_of(id) else { return false };
        if name.is_empty() || self.objects[i].name == name {
            return false;
        }
        self.push_undo();
        let unique = self.unique_name(name);
        self.objects[i].name = unique;
        true
    }

    /// `rna_Object_name_set` for the active object (F2): the name is made unique among the objects (`Cube.001`
    /// style) and is one undo step. Returns whether the name changed.
    pub fn rename_active(&mut self, name: &str) -> bool {
        let name = name.trim();
        let Some(id) = self.active else { return false };
        let Some(i) = self.index_of(id) else { return false };
        if name.is_empty() || self.objects[i].name == name {
            return false;
        }
        self.push_undo();
        let unique = self.unique_name(name);
        self.objects[i].name = unique;
        true
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

    /// Outliner restriction toggles (`outliner_draw.cc` restrict columns): flips `OB_HIDE_VIEWPORT` (eye),
    /// `OB_HIDE_SELECT` or `OB_HIDE_RENDER` of object `id`. A hidden or unselectable object is deselected.
    pub fn toggle_restrict(&mut self, id: u32, which: Restrict) -> bool {
        let Some(i) = self.index_of(id) else { return false };
        self.push_undo();
        let o = &mut self.objects[i];
        match which {
            Restrict::Viewport => o.hidden = !o.hidden,
            Restrict::Select => o.hide_select = !o.hide_select,
            Restrict::Render => o.hide_render = !o.hide_render,
        }
        if o.hidden || o.hide_select {
            o.selected = false;
        }
        true
    }

    /// `OBJECT_OT_hide_view_set` with `unselected` (Shift + H): hides every visible object that is not selected.
    /// Returns how many were hidden.
    pub fn hide_unselected(&mut self) -> usize {
        let n = self.objects.iter().filter(|o| !o.hidden && !o.selected).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| !o.hidden && !o.selected) {
            o.hidden = true;
        }
        n
    }

    /// `OBJECT_OT_hide_view_clear`: shows every hidden object and selects it.
    pub fn reveal_all(&mut self) -> usize {
        /* Objects that Local View hides stay out of reach until it is left. */
        let kept = self.local_view.clone().unwrap_or_default();
        let n = self.objects.iter().filter(|o| o.hidden && !kept.contains(&o.id)).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.hidden && !kept.contains(&o.id)) {
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


    // ----- modifiers ----------------------------------------------------------------------------

    /// Runs `f` on the stack of every selected visible mesh; `f` returns whether it changed the stack.
    fn edit_modifiers(&mut self, mut f: impl FnMut(&mut Vec<Modifier>) -> bool) -> usize {
        if self.selected().filter(|o| o.mesh().is_some()).count() == 0 {
            return 0;
        }
        self.push_undo();
        let mut changed = 0;
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden && o.mesh().is_some()) {
            let before = o.modifiers.len();
            let old = o.modifiers.clone();
            if f(&mut o.modifiers) {
                modifiers::resync_flags(&old, &o.modifiers, &mut o.mod_flags);
                o.eval = None;
                changed += 1;
                /* A modifier that was just added becomes the active one. */
                if o.modifiers.len() > before {
                    o.active_modifier = o.modifiers.len() - 1;
                }
            }
        }
        if changed == 0 {
            self.pop_undo();
        }
        changed
    }

    /// Runs `f(stack, active)` on every selected visible mesh that has modifiers, `active` being the index of
    /// its active modifier (clamped to the stack); `f` may change the index and returns whether it changed the
    /// stack. Undoable.
    fn edit_active_modifier(&mut self, mut f: impl FnMut(&mut Vec<Modifier>, &mut usize) -> bool) -> usize {
        let has = |o: &Object| o.selected && !o.hidden && o.mesh().is_some() && !o.modifiers.is_empty();
        if !self.objects.iter().any(&has) {
            return 0;
        }
        self.push_undo();
        let mut changed = 0;
        for o in self.objects.iter_mut().filter(|o| has(o)) {
            let mut active = o.active_modifier.min(o.modifiers.len() - 1);
            let old = o.modifiers.clone();
            if f(&mut o.modifiers, &mut active) {
                modifiers::resync_flags(&old, &o.modifiers, &mut o.mod_flags);
                o.eval = None;
                changed += 1;
            }
            o.active_modifier = active.min(o.modifiers.len().saturating_sub(1));
        }
        if changed == 0 {
            self.pop_undo();
        }
        changed
    }

    /// Makes the next (`delta` 1) or previous (-1) modifier of the selected meshes the active one, wrapping
    /// around. Returns how many objects have a stack. Not an undo step.
    pub fn select_modifier(&mut self, delta: i32) -> usize {
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden && o.mesh().is_some() && !o.modifiers.is_empty()) {
            let len = o.modifiers.len() as i32;
            let cur = o.active_modifier.min(o.modifiers.len() - 1) as i32;
            o.active_modifier = (cur + delta).rem_euclid(len) as usize;
            n += 1;
        }
        n
    }

    /// Raises or lowers the main value of the active modifier of the selected meshes by `steps` notches
    /// (`Modifier::adjust`).
    pub fn adjust_modifier(&mut self, steps: i32) -> usize {
        self.edit_active_modifier(|stack, active| stack[*active].adjust(steps))
    }

    /// Moves the active modifier one place down (`delta` 1) or up (-1) in the stack; the active one follows it.
    pub fn move_modifier(&mut self, delta: i32) -> usize {
        self.edit_active_modifier(|stack, active| {
            let to = *active as i32 + delta;
            if to < 0 || to >= stack.len() as i32 {
                return false;
            }
            stack.swap(*active, to as usize);
            *active = to as usize;
            true
        })
    }

    /// Removes the active modifier of the selected meshes.
    pub fn remove_modifier(&mut self) -> usize {
        self.edit_active_modifier(|stack, active| {
            stack.remove(*active);
            true
        })
    }

    // The properties editor panel acts on the active object (not on every selected one), like Blender's buttons.

    fn edit_active_object(&mut self, f: impl FnOnce(&mut Object) -> bool) -> bool {
        self.edit_active_object_impl(f, true)
    }

    /// `edit_active_object`; `resync` lines the per-modifier flags up by comparing the stacks, which cannot tell
    /// identical neighbours apart, so an edit that moves or removes modifiers keeps the flags right itself and
    /// passes false.
    fn edit_active_object_impl(&mut self, f: impl FnOnce(&mut Object) -> bool, resync: bool) -> bool {
        let Some(i) = self.active.and_then(|id| self.index_of(id)) else { return false };
        if self.objects[i].mesh().is_none() {
            return false;
        }
        self.push_undo();
        let o = &mut self.objects[i];
        let old = o.modifiers.clone();
        o.mod_flags.resize(old.len(), modifiers::ModFlags::default());
        let changed = f(o);
        if changed {
            if resync {
                modifiers::resync_flags(&old, &o.modifiers, &mut o.mod_flags);
            }
            o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
            o.eval = None;
        } else {
            self.pop_undo();
        }
        changed
    }

    /// Appends `m` to the active object's stack and makes it the active modifier.
    pub fn panel_add_modifier(&mut self, m: Modifier) -> bool {
        self.edit_active_object(|o| {
            o.modifiers.push(m);
            o.active_modifier = o.modifiers.len() - 1;
            true
        })
    }

    /// Makes modifier `i` of the active object the active one (not an undo step, as in Blender).
    pub fn panel_select_modifier(&mut self, i: usize) -> bool {
        let Some(idx) = self.active.and_then(|id| self.index_of(id)) else { return false };
        let o = &mut self.objects[idx];
        if i >= o.modifiers.len() || o.active_modifier == i {
            return false;
        }
        o.active_modifier = i;
        true
    }

    /// Raises / lowers the main value of modifier `i` of the active object by `steps` notches.
    pub fn panel_adjust_modifier(&mut self, i: usize, steps: i32) -> bool {
        self.edit_active_object(|o| {
            let Some(m) = o.modifiers.get_mut(i) else { return false };
            o.active_modifier = i;
            m.adjust(steps)
        })
    }

    /// Moves modifier `i` of the active object one place down (`delta` 1) or up (-1).
    pub fn panel_move_modifier(&mut self, i: usize, delta: i32) -> bool {
        self.edit_active_object_impl(
            |o| {
                let to = i as i32 + delta;
                if i >= o.modifiers.len() || to < 0 || to >= o.modifiers.len() as i32 {
                    return false;
                }
                o.modifiers.swap(i, to as usize);
                o.mod_flags.swap(i, to as usize);
                o.active_modifier = to as usize;
                true
            },
            false,
        )
    }

    /// The Shrinkwrap / Boolean target picker: moves modifier `i` of the active object to the next other mesh object
    /// (by id) and, after the last one, to no target.
    pub fn panel_cycle_target(&mut self, i: usize) -> bool {
        let Some(active) = self.active else { return false };
        let mut ids: Vec<u32> = self.objects.iter().filter(|o| o.id != active && o.mesh().is_some()).map(|o| o.id).collect();
        ids.sort();
        self.edit_active_object(|o| {
            let target = match o.modifiers.get_mut(i) {
                Some(Modifier::Shrinkwrap { target, .. }) | Some(Modifier::Boolean { target, .. }) => target,
                _ => return false,
            };
            *target = match *target {
                None => ids.first().copied(),
                Some(t) => ids.iter().position(|&x| x == t).and_then(|p| ids.get(p + 1).copied()),
            };
            o.active_modifier = i;
            true
        })
    }

    /// The name of the Shrinkwrap / Boolean target of modifier `i` of the active object.
    pub fn target_name(&self, i: usize) -> Option<String> {
        match self.active_object()?.modifiers.get(i)? {
            Modifier::Shrinkwrap { target: Some(t), .. } | Modifier::Boolean { target: Some(t), .. } => self.index_of(*t).map(|j| self.objects[j].name.clone()),
            _ => None,
        }
    }

    /// `OBJECT_OT_modifier_apply`: bakes modifier `i` of the active object into its mesh and takes it off the stack.
    /// Like Blender it is applied on its own to the base mesh, so the modifiers above it in the stack are not\
    /// part of the result. The other modifiers stay and now act on the baked mesh.
    pub fn panel_apply_modifier(&mut self, i: usize) -> bool {
        let Some(idx) = self.active.and_then(|id| self.index_of(id)) else { return false };
        let o = &self.objects[idx];
        let (Some(mesh), Some(m)) = (o.mesh(), o.modifiers.get(i)) else { return false };
        /* Blender skips a modifier that is disabled in the viewport ("Modifier is disabled, skipping apply"). */
        if !o.modifier_shown(i) {
            return false;
        }
        let (ctx, _) = self.eval_ctx(idx);
        let baked = m.apply_with(mesh, &ctx);
        self.edit_active_object_impl(
            |o| {
                o.data = ObjectData::Mesh(baked.clone());
                o.modifiers.remove(i);
                o.mod_flags.remove(i);
                o.active_modifier = o.active_modifier.min(o.modifiers.len().saturating_sub(1));
                true
            },
            false,
        )
    }

    /// `OBJECT_OT_make_links_data` (Modifiers): replaces the modifier stack and the Subdivision Surface level of
    /// every other selected visible mesh with a copy of the active object's. Returns how many objects changed;
    /// one undo step.
    pub fn copy_modifiers_to_selected(&mut self) -> usize {
        let Some(src) = self.active_object().filter(|o| o.mesh().is_some()) else { return 0 };
        let (id, stack, subsurf, flags) = (src.id, src.modifiers.clone(), src.subsurf, src.mod_flags.clone());
        let differs = |o: &Object| o.modifiers != stack || o.subsurf != subsurf || o.mod_flags != flags;
        let wants = |o: &Object| o.selected && !o.hidden && o.id != id && o.mesh().is_some() && differs(o);
        if !self.objects.iter().any(&wants) {
            return 0;
        }
        self.push_undo();
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| wants(o)) {
            o.modifiers = stack.clone();
            o.mod_flags = flags.clone();
            o.subsurf = subsurf;
            o.active_modifier = o.active_modifier.min(o.modifiers.len().saturating_sub(1));
            o.eval = None;
            n += 1;
        }
        n
    }

    /// `OBJECT_OT_modifier_copy`: inserts a copy of modifier `i` of the active object right after it and makes the
    /// copy the active modifier.
    pub fn panel_duplicate_modifier(&mut self, i: usize) -> bool {
        self.edit_active_object_impl(
            |o| {
                let Some(m) = o.modifiers.get(i).cloned() else { return false };
                let mut flags = o.mod_flags[i].clone();
                /* `BKE_modifier_copy_ex` + `BKE_modifier_unique_name`: the copy is Array.001 and so on. */
                let base = o.modifier_name(i).unwrap_or_default();
                let stem = split_suffix(&base).0.to_string();
                let taken = |n: &str| (0..o.modifiers.len()).any(|j| o.modifier_name(j).as_deref() == Some(n));
                let unique = (1..100_000).map(|n| format!("{stem}.{n:03}")).find(|c| !taken(c)).unwrap_or(base);
                flags.name = (unique != m.label()).then_some(unique);
                o.modifiers.insert(i + 1, m);
                o.mod_flags.insert(i + 1, flags);
                o.active_modifier = i + 1;
                true
            },
            false,
        )
    }

    /// Ctrl + Shift + Alt + D: duplicates the active modifier of the active object.
    pub fn duplicate_active_modifier(&mut self) -> bool {
        let Some(o) = self.active_object() else { return false };
        if o.modifiers.is_empty() {
            return false;
        }
        let i = o.active_modifier.min(o.modifiers.len() - 1);
        self.panel_duplicate_modifier(i)
    }

    /// Ctrl + Shift + Alt + A: applies the active modifier of the active object (`panel_apply_modifier`).
    pub fn apply_active_modifier(&mut self) -> bool {
        let Some(o) = self.active_object() else { return false };
        if o.modifiers.is_empty() {
            return false;
        }
        let i = o.active_modifier.min(o.modifiers.len() - 1);
        self.panel_apply_modifier(i)
    }

    /// `rna_Modifier_name_set` (double-click on the name in the panel): the name is made unique among the modifiers
    /// of the object (`Array.001` style). Naming it back to its kind's name clears the custom name. One undo step.
    /// Returns whether the name changed.
    pub fn panel_rename_modifier(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        self.edit_active_object(|o| {
            let Some(current) = o.modifier_name(i) else { return false };
            if name.is_empty() || name == current {
                return false;
            }
            let taken = |n: &str| (0..o.modifiers.len()).any(|j| j != i && o.modifier_name(j).as_deref() == Some(n));
            let mut unique = name.to_string();
            if taken(&unique) {
                let stem = split_suffix(name).0.to_string();
                unique = (1..100_000).map(|n| format!("{stem}.{n:03}")).find(|c| !taken(c)).unwrap_or(unique);
            }
            if unique == current {
                return false;
            }
            o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
            o.mod_flags[i].name = (unique != o.modifiers[i].label()).then_some(unique);
            true
        })
    }

    /// Collapses / expands the panel of modifier `i` of the active object. Not an undo step and no geometry change,
    /// as in Blender (the expand flag is UI state).
    pub fn panel_toggle_collapse(&mut self, i: usize) -> bool {
        let Some(idx) = self.active.and_then(|id| self.index_of(id)) else { return false };
        let o = &mut self.objects[idx];
        if i >= o.modifiers.len() {
            return false;
        }
        o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
        o.mod_flags[i].collapsed = !o.mod_flags[i].collapsed;
        true
    }

    /// Typing a number into the value field of modifier `i` of the active object (`Modifier::set_main_value`); it
    /// becomes the active modifier. One undo step; false when nothing changed.
    pub fn panel_set_modifier_value(&mut self, i: usize, v: f32) -> bool {
        self.edit_active_object(|o| {
            let Some(m) = o.modifiers.get_mut(i) else { return false };
            o.active_modifier = i;
            m.set_main_value(v)
        })
    }

    /// Drags modifier `i` of the active object to index `to` (`OBJECT_OT_modifier_move_to_index`), the others shift
    /// along; it becomes the active modifier. One undo step; false when it stays where it is.
    pub fn panel_move_modifier_index(&mut self, i: usize, to: usize) -> bool {
        self.edit_active_object_impl(
            |o| {
                if i >= o.modifiers.len() || to >= o.modifiers.len() || i == to {
                    return false;
                }
                let m = o.modifiers.remove(i);
                o.modifiers.insert(to, m);
                o.mod_flags.resize(o.modifiers.len() + 1, modifiers::ModFlags::default());
                let f = o.mod_flags.remove(i);
                o.mod_flags.insert(to, f);
                o.mod_flags.truncate(o.modifiers.len());
                o.active_modifier = to;
                true
            },
            false,
        )
    }

    /// `OBJECT_OT_modifier_move_to_index` (Move to First / Move to Last): moves modifier `i` of the active object to
    /// the start or the end of the stack and keeps it the active one.
    pub fn panel_move_modifier_to(&mut self, i: usize, first: bool) -> bool {
        self.edit_active_object_impl(
            |o| {
                let to = if first { 0 } else { o.modifiers.len().saturating_sub(1) };
                if i >= o.modifiers.len() || i == to {
                    return false;
                }
                let m = o.modifiers.remove(i);
                o.modifiers.insert(to, m);
                let f = o.mod_flags.remove(i);
                o.mod_flags.insert(to, f);
                o.active_modifier = to;
                true
            },
            false,
        )
    }

    /// `OBJECT_OT_modifier_copy_to_selected`: appends a copy of modifier `i` of the active object (with its name and
    /// viewport flag, made unique) to every other selected visible mesh. Returns how many objects got it; one undo step.
    pub fn panel_copy_modifier_to_selected(&mut self, i: usize) -> usize {
        let Some(src) = self.active_object().filter(|o| o.mesh().is_some()) else { return 0 };
        let (id, Some(m), Some(name)) = (src.id, src.modifiers.get(i).cloned(), src.modifier_name(i)) else { return 0 };
        let mut flags = src.mod_flags.get(i).cloned().unwrap_or_default();
        flags.collapsed = false;
        let wants = |o: &Object| o.selected && !o.hidden && o.id != id && o.mesh().is_some();
        if !self.objects.iter().any(&wants) {
            return 0;
        }
        self.push_undo();
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| wants(o)) {
            o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
            let stem = split_suffix(&name).0.to_string();
            let taken = |c: &str| (0..o.modifiers.len()).any(|j| o.modifier_name(j).as_deref() == Some(c));
            let unique = if taken(&name) { (1..100_000).map(|k| format!("{stem}.{k:03}")).find(|c| !taken(c)).unwrap_or(name.clone()) } else { name.clone() };
            let mut f = flags.clone();
            f.name = (unique != m.label()).then_some(unique);
            o.modifiers.push(m.clone());
            o.mod_flags.push(f);
            o.active_modifier = o.modifiers.len() - 1;
            o.eval = None;
            n += 1;
        }
        n
    }

    /// Ctrl + click on a modifier's triangle (`UI_panel_drag_collapse` / `ui_handle_panel_header`): collapses every
    /// other panel and opens modifier `i`; when it already is the only open one, opens them all. UI state only.
    pub fn panel_collapse_others(&mut self, i: usize) -> bool {
        let Some(idx) = self.active.and_then(|id| self.index_of(id)) else { return false };
        let o = &mut self.objects[idx];
        if i >= o.modifiers.len() {
            return false;
        }
        o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
        let only_open = (0..o.modifiers.len()).all(|j| o.mod_flags[j].collapsed == (j != i));
        for (j, f) in o.mod_flags.iter_mut().enumerate() {
            f.collapsed = !only_open && j != i;
        }
        true
    }

    /// The monitor toggle of modifier `i` of the active object (`show_viewport`). Undoable.
    pub fn panel_toggle_viewport(&mut self, i: usize) -> bool {
        self.edit_active_object(|o| {
            if i >= o.modifiers.len() {
                return false;
            }
            o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
            o.mod_flags[i].hide_viewport = !o.mod_flags[i].hide_viewport;
            true
        })
    }

    /// The X / Y / Z buttons of a Mirror modifier `i` of the active object. Unlike the Ctrl + Alt chords, switching off
    /// the last axis keeps the modifier (it then changes nothing), as in Blender. Undoable.
    pub fn panel_toggle_mirror_axis(&mut self, i: usize, axis: usize) -> bool {
        self.edit_active_object(|o| {
            let Some(Modifier::Mirror { axes, .. }) = o.modifiers.get_mut(i) else { return false };
            let Some(on) = axes.get_mut(axis) else { return false };
            *on = !*on;
            o.active_modifier = i;
            true
        })
    }

    /// The edit-mode toggle of modifier `i` of the active object (`show_in_editmode`). Undoable; nothing else changes.
    pub fn panel_toggle_editmode(&mut self, i: usize) -> bool {
        self.edit_active_object(|o| {
            if i >= o.modifiers.len() {
                return false;
            }
            o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
            o.mod_flags[i].hide_editmode = !o.mod_flags[i].hide_editmode;
            true
        })
    }

    /// The camera toggle of modifier `i` of the active object (`show_render`). Undoable; the viewport is unchanged.
    pub fn panel_toggle_render(&mut self, i: usize) -> bool {
        self.edit_active_object(|o| {
            if i >= o.modifiers.len() {
                return false;
            }
            o.mod_flags.resize(o.modifiers.len(), modifiers::ModFlags::default());
            o.mod_flags[i].hide_render = !o.mod_flags[i].hide_render;
            true
        })
    }

    /// Removes modifier `i` of the active object.
    pub fn panel_remove_modifier(&mut self, i: usize) -> bool {
        self.edit_active_object_impl(
            |o| {
                if i >= o.modifiers.len() {
                    return false;
                }
                o.modifiers.remove(i);
                o.mod_flags.remove(i);
                o.active_modifier = o.active_modifier.min(o.modifiers.len().saturating_sub(1));
                true
            },
            false,
        )
    }

    /// Adds a Mirror modifier for `axis` (0 = X, 1 = Y, 2 = Z) to the selected meshes, or flips that axis of
    /// the one they have; a Mirror with no axis left is removed.
    pub fn toggle_mirror(&mut self, axis: usize) -> usize {
        let axis = axis.min(2);
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Mirror { .. })) {
                Some(i) => {
                    if let Modifier::Mirror { axes, .. } = &mut stack[i] {
                        axes[axis] = !axes[axis];
                        if !axes.iter().any(|a| *a) {
                            stack.remove(i);
                        }
                    }
                }
                None => stack.push(Modifier::mirror(axis)),
            }
            true
        })
    }

    /// Cycles the offset of the first Array modifier of each selected mesh through `ARRAY_PRESETS` (relative X, Y,
    /// Z, then 1 m constant along X, Y, Z); a custom offset goes back to relative X. Returns how many changed.
    pub fn cycle_array_offset(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            let Some(Modifier::Array { relative, constant, .. }) = stack.iter_mut().find(|m| matches!(m, Modifier::Array { .. })) else { return false };
            let next = modifiers::array_preset(*relative, *constant).map_or(0, |i| (i + 1) % modifiers::ARRAY_PRESETS.len());
            *relative = modifiers::ARRAY_PRESETS[next].1;
            *constant = modifiers::ARRAY_PRESETS[next].2;
            true
        })
    }

    /// Cycles Fit Length of the first Array modifier of each selected mesh: Fixed Count, then 2, 4 and 8 m, back to
    /// Fixed Count (a custom length starts at 2 m). Returns how many changed.
    pub fn cycle_array_fit(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            let Some(Modifier::Array { fit, .. }) = stack.iter_mut().find(|m| matches!(m, Modifier::Array { .. })) else { return false };
            *fit = match *fit {
                f if f <= 0.0 => 2.0,
                2.0 => 4.0,
                4.0 => 8.0,
                8.0 => 0.0,
                _ => 2.0,
            };
            true
        })
    }

    /// Toggles Merge (1 mm) on the first Array modifier of each selected mesh.
    pub fn toggle_array_merge(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            let Some(Modifier::Array { merge, .. }) = stack.iter_mut().find(|m| matches!(m, Modifier::Array { .. })) else { return false };
            *merge = if *merge > 0.0 { 0.0 } else { 0.001 };
            true
        })
    }

    /// Sets the count of the Array modifier of the selected meshes: 2 or more adds or updates it, less removes it.
    pub fn set_array(&mut self, count: u32) -> usize {
        self.edit_modifiers(|stack| {
            let at = stack.iter().position(|m| matches!(m, Modifier::Array { .. }));
            match (at, count >= 2) {
                (Some(i), true) => {
                    if let Modifier::Array { count: c, .. } = &mut stack[i] {
                        if *c == count {
                            return false;
                        }
                        *c = count;
                    }
                    true
                }
                (Some(i), false) => {
                    stack.remove(i);
                    true
                }
                (None, true) => {
                    stack.push(Modifier::array(count));
                    true
                }
                (None, false) => false,
            }
        })
    }

    /// Adds a Solidify modifier to the selected meshes, or removes the one they have.
    pub fn toggle_solidify(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Solidify { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::solidify()),
            }
            true
        })
    }

    /// Toggles Even Thickness on the first Solidify modifier of each selected mesh. Returns how many changed.
    pub fn toggle_solidify_even(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            let Some(Modifier::Solidify { even, .. }) = stack.iter_mut().find(|m| matches!(m, Modifier::Solidify { .. })) else { return false };
            *even = !*even;
            true
        })
    }

    /// Cycles the segments of the first Bevel modifier of each selected mesh: 1, 2, 3, 4, 6, 8, then 1 again.
    pub fn cycle_bevel_segments(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            let Some(Modifier::Bevel { segments, .. }) = stack.iter_mut().find(|m| matches!(m, Modifier::Bevel { .. })) else { return false };
            *segments = match *segments {
                1 => 2,
                2 => 3,
                3 => 4,
                4 => 6,
                6 => 8,
                _ => 1,
            };
            true
        })
    }

    /// Adds a Bevel modifier to the selected meshes, or removes the one they have.
    pub fn toggle_bevel(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Bevel { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::bevel()),
            }
            true
        })
    }

    /// Adds a Smooth modifier to the selected meshes, or removes the one they have.
    pub fn toggle_smooth_modifier(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Smooth { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::smooth()),
            }
            true
        })
    }

    /// Cycles the Direction of the first Displace modifier of each selected mesh: Normal, X, Y, Z, Normal.
    /// Returns how many changed.
    pub fn cycle_displace_direction(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            let Some(Modifier::Displace { dir, .. }) = stack.iter_mut().find(|m| matches!(m, Modifier::Displace { .. })) else { return false };
            *dir = dir.next();
            true
        })
    }

    /// Adds a Displace modifier to the selected meshes, or removes the one they have.
    pub fn toggle_displace(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Displace { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::displace()),
            }
            true
        })
    }

    /// Adds a Triangulate modifier to the selected meshes, or removes the one they have.
    pub fn toggle_triangulate_modifier(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Triangulate)) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::Triangulate),
            }
            true
        })
    }

    /// Adds a Wireframe modifier to the selected meshes, or removes the one they have.
    pub fn toggle_wireframe(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Wireframe { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::wireframe()),
            }
            true
        })
    }

    /// Cast: adds it as Sphere, then each call moves it on to Cylinder and finally removes it.
    pub fn cycle_cast(&mut self) -> usize {
        use crate::mesh_deform::CastShape;
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Cast { .. })) {
                Some(i) => {
                    let shape = if let Modifier::Cast { shape, .. } = &stack[i] { *shape } else { unreachable!() };
                    match shape.next() {
                        Some(next) => stack[i] = Modifier::cast(next),
                        None => {
                            stack.remove(i);
                        }
                    }
                }
                None => stack.push(Modifier::cast(CastShape::Sphere)),
            }
            true
        })
    }

    /// Adds an Edge Split modifier to the selected meshes, or removes the one they have.
    pub fn toggle_edge_split(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::EdgeSplit { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::edge_split()),
            }
            true
        })
    }

    /// Decimate: adds it at a ratio of 0.5, then each call lowers it to 0.25, 0.1 and finally removes it.
    pub fn cycle_decimate(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Decimate { .. })) {
                Some(i) => {
                    let ratio = if let Modifier::Decimate { ratio } = &stack[i] { *ratio } else { unreachable!() };
                    if ratio > 0.3 {
                        stack[i] = Modifier::decimate(0.25);
                    } else if ratio > 0.15 {
                        stack[i] = Modifier::decimate(0.1);
                    } else {
                        stack.remove(i);
                    }
                }
                None => stack.push(Modifier::decimate(0.5)),
            }
            true
        })
    }

    /// Adds a Laplacian Smooth modifier to the selected meshes, or removes the one they have.
    pub fn toggle_laplacian_smooth(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::LaplacianSmooth { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::laplacian_smooth()),
            }
            true
        })
    }

    /// Sets the current frame; true when an object with a Build modifier now has to be evaluated again (the
    /// cached result is dropped by the signature in `update_eval()`).
    pub fn set_frame(&mut self, frame: i32) -> bool {
        if self.frame == frame {
            return false;
        }
        self.frame = frame;
        self.objects.iter().any(|o| o.modifiers.iter().any(|m| matches!(m, Modifier::Build { .. })))
    }

    /// Adds a Build modifier to the selected meshes, or removes the one they have.
    pub fn toggle_build(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Build { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::build()),
            }
            true
        })
    }

    /// Adds a Remesh modifier to the selected meshes, or removes the one they have.
    pub fn toggle_remesh(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Remesh { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::remesh()),
            }
            true
        })
    }

    /// Adds a Screw modifier to the selected meshes, or removes the one they have.
    pub fn toggle_screw(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Screw { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::screw()),
            }
            true
        })
    }

    /// Adds a Wave modifier to the selected meshes, or removes the one they have.
    pub fn toggle_wave(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Wave { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::wave()),
            }
            true
        })
    }

    /// Adds a Weld modifier to the selected meshes, or removes the one they have.
    pub fn toggle_weld(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::Weld { .. })) {
                Some(i) => {
                    stack.remove(i);
                }
                None => stack.push(Modifier::weld()),
            }
            true
        })
    }

    /// Simple Deform: adds it as Twist, then each call moves it on to Taper, Stretch and finally removes it.
    pub fn cycle_simple_deform(&mut self) -> usize {
        self.edit_modifiers(|stack| {
            match stack.iter().position(|m| matches!(m, Modifier::SimpleDeform { .. })) {
                Some(i) => {
                    let mode = if let Modifier::SimpleDeform { mode, .. } = &stack[i] { *mode } else { unreachable!() };
                    match mode.next() {
                        Some(next) => stack[i] = Modifier::simple_deform(next),
                        None => {
                            stack.remove(i);
                        }
                    }
                }
                None => stack.push(Modifier::simple_deform(crate::mesh_deform::DeformMode::Twist)),
            }
            true
        })
    }

    /// The array count of the first selected mesh with an Array modifier (1 when there is none).
    pub fn array_count(&self) -> u32 {
        self.selected()
            .flat_map(|o| o.modifiers.iter())
            .find_map(|m| if let Modifier::Array { count, .. } = m { Some(*count) } else { None })
            .unwrap_or(1)
    }

    /// `OBJECT_OT_convert` to mesh: bakes the modifier stack and the subdivision level into the mesh.
    pub fn apply_modifiers(&mut self) -> usize {
        let n = self.selected().filter(|o| o.mesh().is_some() && o.has_eval()).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for i in 0..self.objects.len() {
            let o = &self.objects[i];
            if !(o.selected && !o.hidden && o.mesh().is_some() && o.has_eval()) {
                continue;
            }
            let (ctx, _) = self.eval_ctx(i);
            let o = &mut self.objects[i];
            o.rebuild_eval(&ctx);
            if let Some(baked) = o.eval.take() {
                o.data = ObjectData::Mesh(baked);
            }
            o.modifiers.clear();
            o.mod_flags.clear();
            o.subsurf = 0;
        }
        n
    }

    // ----- object data operators ------------------------------------------------------------------

    /// `OBJECT_OT_transform_apply` (location, rotation and scale): bakes the object transform into the mesh and
    /// resets it. Returns how many meshes changed (see `apply_transform` for the single options).
    pub fn apply_transforms(&mut self) -> usize {
        self.apply_transform(ApplyMask::ALL)
    }

    /// `OBJECT_OT_origin_set` (Origin to Geometry, median center): moves the origin of the selected meshes to the
    /// mean of their vertices without moving the mesh in the world.
    pub fn origin_to_geometry(&mut self) -> usize {
        let n = self.selected().filter(|o| o.mesh().is_some()).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            let ObjectData::Mesh(mesh) = &o.data else { continue };
            let Some(c) = mesh.median_center() else { continue };
            let moved = mesh.mapped(|v| [v[0] - c[0], v[1] - c[1], v[2] - c[2]], false);
            let w = mat3_mul_v3(&euler_to_mat3(o.rot), [c[0] * o.scale[0], c[1] * o.scale[1], c[2] * o.scale[2]]);
            o.data = ObjectData::Mesh(moved);
            o.loc = [o.loc[0] + w[0], o.loc[1] + w[1], o.loc[2] + w[2]];
            o.eval = None;
        }
        n
    }

    /// `OBJECT_OT_join`: merges the selected meshes into the active one (which must be selected), in its local
    /// space, and deletes the others. Returns how many objects were joined into the active one.
    pub fn join_selected(&mut self) -> usize {
        let Some(active) = self.active.and_then(|id| self.index_of(id)) else { return 0 };
        if !self.objects[active].selected || self.objects[active].mesh().is_none() {
            return 0;
        }
        let others: Vec<usize> = (0..self.objects.len())
            .filter(|&i| i != active && self.objects[i].selected && !self.objects[i].hidden && self.objects[i].mesh().is_some())
            .collect();
        if others.is_empty() {
            return 0;
        }
        self.push_undo();
        let (a_loc, a_rot, a_scale) = (self.objects[active].loc, self.objects[active].rot, self.objects[active].scale);
        let a_inv_rot = mat3_transpose(&euler_to_mat3(a_rot));
        let safe = |s: f32| if s.abs() < 1e-8 { 1.0 } else { s };
        let mut joined = self.objects[active].mesh().cloned().unwrap_or_default();
        let mut remove_ids = Vec::new();
        for &i in &others {
            let o = &self.objects[i];
            let Some(mesh) = o.mesh() else { continue };
            let (o_loc, o_rot, o_scale) = (o.loc, euler_to_mat3(o.rot), o.scale);
            let flip = (o_scale[0] * o_scale[1] * o_scale[2] < 0.0) != (a_scale[0] * a_scale[1] * a_scale[2] < 0.0);
            let part = mesh.mapped(
                |v| {
                    let w = mat3_mul_v3(&o_rot, [v[0] * o_scale[0], v[1] * o_scale[1], v[2] * o_scale[2]]);
                    let w = [w[0] + o_loc[0] - a_loc[0], w[1] + o_loc[1] - a_loc[1], w[2] + o_loc[2] - a_loc[2]];
                    let l = mat3_mul_v3(&a_inv_rot, w);
                    [l[0] / safe(a_scale[0]), l[1] / safe(a_scale[1]), l[2] / safe(a_scale[2])]
                },
                flip,
            );
            joined.append(&part);
            remove_ids.push(o.id);
        }
        self.objects[active].data = ObjectData::Mesh(joined);
        self.objects[active].eval = None;
        self.objects.retain(|o| !remove_ids.contains(&o.id));
        remove_ids.len()
    }

    /// Runs `f` on the base mesh of every selected mesh object; `f` returns whether it changed the mesh.
    fn edit_meshes(&mut self, mut f: impl FnMut(&mut Mesh) -> bool) -> usize {
        if self.selected().filter(|o| o.mesh().is_some()).count() == 0 {
            return 0;
        }
        self.push_undo();
        let mut changed = 0;
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            if let ObjectData::Mesh(m) = &mut o.data {
                if f(m) {
                    o.eval = None;
                    changed += 1;
                }
            }
        }
        if changed == 0 {
            self.pop_undo();
        }
        changed
    }

    /// `mesh.flip_normals` on the selected meshes.
    pub fn flip_normals(&mut self) -> usize {
        self.edit_meshes(|m| {
            m.flip_normals();
            !m.faces.is_empty()
        })
    }

    /// `mesh.normals_make_consistent` (outside) on the selected meshes.
    pub fn recalc_normals(&mut self) -> usize {
        self.edit_meshes(|m| m.recalc_normals_outside() > 0)
    }

    /// `mesh.quads_convert_to_tris` on the selected meshes.
    pub fn triangulate(&mut self) -> usize {
        self.edit_meshes(|m| {
            if m.faces.iter().all(|f| f.len() <= 3) {
                return false;
            }
            *m = m.triangulated();
            true
        })
    }

    /// `mesh.remove_doubles` on the selected meshes. Returns how many meshes lost vertices.
    pub fn merge_by_distance(&mut self, distance: f32) -> usize {
        self.edit_meshes(|m| {
            let (merged, removed) = m.merged_by_distance(distance);
            if removed == 0 {
                return false;
            }
            *m = merged;
            true
        })
    }


    // ----- snap, separate, subdivide ---------------------------------------------------------------

    /// `VIEW3D_OT_snap_cursor_to_selected`: where the 3D cursor goes (the median of the selected origins).
    pub fn cursor_target(&self) -> Option<V3> {
        self.median_origin()
    }

    /// `VIEW3D_OT_snap_selected_to_cursor` with `use_offset`: moves the selection so its median lands on `cursor`.
    pub fn selection_to_cursor(&mut self, cursor: V3) -> usize {
        let Some(median) = self.median_origin() else { return 0 };
        let delta = [cursor[0] - median[0], cursor[1] - median[1], cursor[2] - median[2]];
        if delta.iter().all(|d| d.abs() < 1e-7) {
            return 0;
        }
        self.push_undo();
        let mut n = 0;
        let skip: Vec<bool> = (0..self.objects.len()).map(|i| self.has_selected_ancestor(i)).collect();
        for (i, o) in self.objects.iter_mut().enumerate().filter(|(_, o)| o.selected && !o.hidden) {
            n += 1;
            if skip[i] {
                continue;
            }
            let d = mat3_mul_v3(&o.parent_lin_inv(), delta);
            o.loc = [o.loc[0] + d[0], o.loc[1] + d[1], o.loc[2] + d[2]];
        }
        n
    }

    /// `OBJECT_OT_origin_set` (Origin to 3D Cursor): the origin of the selected meshes moves to `point` and the
    /// meshes stay where they are in the world.
    pub fn origin_to_point(&mut self, point: V3) -> usize {
        let n = self.selected().filter(|o| o.mesh().is_some()).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        let safe = |s: f32| if s.abs() < 1e-8 { 1.0 } else { s };
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            let ObjectData::Mesh(mesh) = &o.data else { continue };
            let inv = mat3_transpose(&euler_to_mat3(o.rot));
            let d = mat3_mul_v3(&inv, [point[0] - o.loc[0], point[1] - o.loc[1], point[2] - o.loc[2]]);
            let l = [d[0] / safe(o.scale[0]), d[1] / safe(o.scale[1]), d[2] / safe(o.scale[2])];
            let moved = mesh.mapped(|v| [v[0] - l[0], v[1] - l[1], v[2] - l[2]], false);
            o.data = ObjectData::Mesh(moved);
            o.loc = point;
            o.eval = None;
        }
        n
    }

    /// `mesh.separate` by loose parts: every selected mesh made of several pieces keeps the first one and the
    /// others become new objects with the same transform, shading and modifiers. The pieces end up selected.
    /// Returns how many objects were created.
    pub fn separate_loose(&mut self) -> usize {
        let plan: Vec<(u32, Vec<Mesh>)> = self
            .selected()
            .filter_map(|o| {
                let parts = o.mesh()?.loose_parts();
                (parts.len() > 1).then_some((o.id, parts))
            })
            .collect();
        if plan.is_empty() {
            return 0;
        }
        self.push_undo();
        let mut created = 0;
        for (id, mut parts) in plan {
            let Some(i) = self.index_of(id) else { continue };
            let first = parts.remove(0);
            let src = self.objects[i].clone();
            self.objects[i].data = ObjectData::Mesh(first);
            self.objects[i].eval = None;
            for part in parts {
                let o = self.new_object(&src.name, ObjectData::Mesh(part));
                o.loc = src.loc;
                o.rot = src.rot;
                o.scale = src.scale;
                o.smooth = src.smooth;
                o.subsurf = src.subsurf;
                o.modifiers = src.modifiers.clone();
                o.mod_flags = src.mod_flags.clone();
                o.selected = true;
                created += 1;
            }
        }
        created
    }

    /// Sets one component of the active object's location (m), rotation (degrees) or scale, like typing in
    /// a field of the Transform panel (`rna_Object_location_set` etc.). Returns whether anything changed;
    /// a change is one undo step. Non finite values and an `axis` over 2 are refused.
    pub fn set_transform_value(&mut self, prop: TransformProp, axis: usize, value: f32) -> bool {
        if axis > 2 || !value.is_finite() || value.abs() > 1.0e7 {
            return false;
        }
        let Some(i) = self.active.and_then(|id| self.index_of(id)).filter(|&i| !self.objects[i].hidden) else {
            return false;
        };
        let current = match prop {
            TransformProp::Location => self.objects[i].loc[axis],
            TransformProp::Rotation => self.objects[i].rot[axis].to_degrees(),
            TransformProp::Scale => self.objects[i].scale[axis],
            TransformProp::Dimensions => match self.objects[i].dimensions() {
                Some(d) if value > 0.0 && self.objects[i].local_size()[axis] > 1e-9 => d[axis],
                _ => return false,
            },
        };
        if (current - value).abs() < 1e-6 {
            return false;
        }
        self.push_undo();
        let o = &mut self.objects[i];
        match prop {
            TransformProp::Location => o.loc[axis] = value,
            TransformProp::Rotation => o.rot[axis] = value.to_radians(),
            TransformProp::Scale => o.scale[axis] = value,
            /* The sign of the scale (a mirrored axis) stays. */
            TransformProp::Dimensions => {
                let sign = if o.scale[axis] < 0.0 { -1.0 } else { 1.0 };
                o.scale[axis] = sign * value / o.local_size()[axis];
            }
        }
        o.eval = None;
        true
    }

    /// `mesh.convex_hull` on the selected meshes: each mesh is replaced by the hull of its vertices, with
    /// coplanar triangles joined into n-gons. Meshes that span no volume are left alone.
    pub fn convex_hull(&mut self) -> usize {
        self.edit_meshes(|m| match m.convex_hull() {
            Some(h) if h.verts.len() != m.verts.len() || h.faces.len() != m.faces.len() => {
                *m = h;
                true
            }
            _ => false,
        })
    }

    /// `TRANSFORM_OT_mirror` (Ctrl + M) in global `axis` (0 = X, 1 = Y, 2 = Z) with the median point as pivot:
    /// the locations are reflected through the plane of the median, and the orientation and scale are flipped
    /// the way Blender does it (`M * R * S = (M R M) * (M S)`: the rotation is conjugated by the reflection and
    /// the scale along the local axis of the same number becomes negative).
    pub fn mirror_objects(&mut self, axis: usize) -> usize {
        if axis > 2 {
            return 0;
        }
        let Some(median) = self.median_origin() else { return 0 };
        self.push_undo();
        let mut n = 0;
        let mut flip = [[0.0f32; 3]; 3];
        for (i, row) in flip.iter_mut().enumerate() {
            row[i] = if i == axis { -1.0 } else { 1.0 };
        }
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            o.loc[axis] = 2.0 * median[axis] - o.loc[axis];
            let r = euler_to_mat3(o.rot);
            o.rot = mat3_to_euler(&mat3_mul(&flip, &mat3_mul(&r, &flip)));
            o.scale[axis] = -o.scale[axis];
            o.eval = None;
            n += 1;
        }
        n
    }

    /// `OBJECT_OT_select_grouped` (type): selects every visible object of the type of the active one. Returns
    /// how many objects were newly selected.
    pub fn select_grouped_type(&mut self) -> usize {
        let Some(active) = self.active_object() else { return 0 };
        let kind = std::mem::discriminant(&active.data);
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| !o.hidden && !o.selected) {
            if std::mem::discriminant(&o.data) == kind {
                o.selected = true;
                n += 1;
            }
        }
        n
    }

    /// `OBJECT_OT_select_random`: selects about `ratio` of the visible objects. The seed counts up on every
    /// call. Returns how many objects were newly selected.
    pub fn select_random(&mut self, ratio: f32) -> usize {
        let seed = self.select_seed;
        self.select_seed = self.select_seed.wrapping_add(1);
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| !o.hidden && !o.selected) {
            /* A small integer hash (lowbias32) of the object id and the seed, mapped to [0, 1). */
            let mut h = o.id.wrapping_mul(0x9E37_79B9).wrapping_add(seed.wrapping_mul(0x85EB_CA6B)).wrapping_add(0x1234_5678);
            h ^= h >> 16;
            h = h.wrapping_mul(0x7FEB_352D);
            h ^= h >> 15;
            h = h.wrapping_mul(0x846C_A68B);
            h ^= h >> 16;
            if (h as f32 / 4_294_967_296.0) < ratio {
                o.selected = true;
                n += 1;
            }
        }
        n
    }

    fn push_tree(&self, i: usize, level: i32, seen: &mut Vec<bool>, rows: &mut Vec<OutlinerRow>) {
        if seen[i] {
            return;
        }
        seen[i] = true;
        let o = &self.objects[i];
        rows.push(OutlinerRow { level, name: o.name.clone(), kind: RowKind::Object(o.id) });
        let mut kids: Vec<usize> = (0..self.objects.len()).filter(|&j| self.objects[j].parent == Some(o.id)).collect();
        kids.sort_by_key(|&j| self.objects[j].name.to_lowercase());
        for k in kids {
            self.push_tree(k, level + 1, seen, rows);
        }
    }

    // ----- parenting -----------------------------------------------------------------------------

    /// The world matrix of object `i` computed through its chain of parents (no cache involved).
    fn full_world(&self, i: usize, depth: u32) -> Mat4 {
        let o = &self.objects[i];
        let local = o.local_matrix();
        match o.parent.and_then(|p| self.index_of(p)) {
            Some(p) if depth < MAX_PARENT_DEPTH => {
                mat4_mul(&self.full_world(p, depth + 1), &mat4_mul(&o.parent_inv, &local))
            }
            _ => local,
        }
    }

    /// Updates `Object::parent_world` of every object; `update_eval()` calls it before the modifiers are built.
    pub fn refresh_parents(&mut self) {
        let worlds: Vec<Mat4> = (0..self.objects.len())
            .map(|i| match self.objects[i].parent.and_then(|p| self.index_of(p)) {
                Some(p) => self.full_world(p, 1),
                None => IDENTITY4,
            })
            .collect();
        for (o, w) in self.objects.iter_mut().zip(worlds) {
            o.parent_world = w;
        }
    }

    /// Is object `a` (index) a descendant of the object with id `ancestor`?
    fn is_descendant(&self, a: usize, ancestor: u32) -> bool {
        let mut cur = self.objects[a].parent;
        for _ in 0..MAX_PARENT_DEPTH {
            match cur {
                Some(p) if p == ancestor => return true,
                Some(p) => cur = self.index_of(p).and_then(|j| self.objects[j].parent),
                None => return false,
            }
        }
        false
    }

    /// Does a parent, grandparent, ... of object `i` belong to the selection? Such an object moves with it.
    pub fn has_selected_ancestor(&self, i: usize) -> bool {
        let mut cur = self.objects[i].parent;
        for _ in 0..MAX_PARENT_DEPTH {
            let Some(p) = cur.and_then(|p| self.index_of(p)) else { return false };
            if self.objects[p].selected && !self.objects[p].hidden {
                return true;
            }
            cur = self.objects[p].parent;
        }
        false
    }

    /// Gives object `i` its world transform as its own location, rotation and scale and drops the parent
    /// (`BKE_object_apply_mat4`). Shear (a non-uniformly scaled rotated parent) cannot be represented and is lost.
    fn bake_world_and_unparent(&mut self, i: usize) {
        let w = self.full_world(i, 0);
        let o = &mut self.objects[i];
        let (loc, rot, scale) = decompose(&w);
        o.loc = loc;
        o.rot = rot;
        o.scale = scale;
        o.parent = None;
        o.parent_inv = IDENTITY4;
        o.parent_world = IDENTITY4;
        o.eval = None;
    }

    /// `OBJECT_OT_parent_set` (Ctrl + P, type Object with Keep Transform): the active object becomes the parent
    /// of the other selected objects, which stay where they are. Returns how many were parented.
    pub fn parent_set(&mut self) -> usize {
        let Some(active) = self.active.filter(|&id| self.index_of(id).is_some_and(|i| !self.objects[i].hidden)) else {
            return 0;
        };
        let ai = self.index_of(active).unwrap();
        let children: Vec<usize> = (0..self.objects.len())
            .filter(|&i| {
                let o = &self.objects[i];
                o.selected && !o.hidden && o.id != active && !self.is_descendant(ai, o.id)
            })
            .collect();
        if children.is_empty() {
            return 0;
        }
        self.push_undo();
        for &i in &children {
            self.bake_world_and_unparent(i);
            let inv = crate::view3d::invert_m4_m4(&self.full_world(ai, 0));
            let o = &mut self.objects[i];
            o.parent = Some(active);
            o.parent_inv = inv;
        }
        self.refresh_parents();
        children.len()
    }

    /// `OBJECT_OT_parent_clear`: removes the parent of the selected objects. With `keep_transform` they stay
    /// where they are (type Clear and Keep Transformation), otherwise they fall back to their own local
    /// transform (type Clear Parent). Returns how many had a parent.
    pub fn parent_clear(&mut self, keep_transform: bool) -> usize {
        let targets: Vec<usize> =
            (0..self.objects.len()).filter(|&i| self.objects[i].selected && !self.objects[i].hidden && self.objects[i].parent.is_some()).collect();
        if targets.is_empty() {
            return 0;
        }
        self.push_undo();
        self.refresh_parents();
        for &i in &targets {
            if keep_transform {
                self.bake_world_and_unparent(i);
            } else {
                let o = &mut self.objects[i];
                o.parent = None;
                o.parent_inv = IDENTITY4;
                o.parent_world = IDENTITY4;
                o.eval = None;
            }
        }
        self.refresh_parents();
        targets.len()
    }

    /// `OBJECT_OT_select_hierarchy` (Ctrl + [ parent, Ctrl + ] child; with `extend` the old selection stays):
    /// moves the selection and the active object to the parent or the first child (by name) of the active one.
    pub fn select_hierarchy(&mut self, to_parent: bool, extend: bool) -> bool {
        let Some(ai) = self.active.and_then(|id| self.index_of(id)) else { return false };
        let target = if to_parent {
            self.objects[ai].parent.and_then(|p| self.index_of(p))
        } else {
            let id = self.objects[ai].id;
            (0..self.objects.len())
                .filter(|&j| self.objects[j].parent == Some(id))
                .min_by_key(|&j| self.objects[j].name.to_lowercase())
        };
        let Some(t) = target.filter(|&t| !self.objects[t].hidden) else { return false };
        if !extend {
            for o in &mut self.objects {
                o.selected = false;
            }
        }
        self.objects[t].selected = true;
        self.active = Some(self.objects[t].id);
        true
    }

    /// `mesh.subdivide` (one cut, no smoothing) on the selected meshes.
    pub fn subdivide_simple(&mut self) -> usize {
        self.edit_meshes(|m| {
            if m.faces.is_empty() {
                return false;
            }
            *m = m.subdivided_simple();
            true
        })
    }

    // ----- queries ------------------------------------------------------------------------------

    /// The median of the selected objects' origins, the default transform pivot.
    pub fn median_origin(&self) -> Option<V3> {
        let sel: Vec<&Object> = self.selected().collect();
        if sel.is_empty() {
            return None;
        }
        let sum = sel.iter().fold([0.0f32; 3], |s, o| {
            let w = o.world_matrix()[3];
            [s[0] + w[0], s[1] + w[1], s[2] + w[2]]
        });
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
                    if best.is_none_or(|(bt, _)| t < bt) {
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
        let mut rows = vec![
            OutlinerRow { level: 0, name: "Scene Collection".into(), kind: RowKind::SceneCollection },
            OutlinerRow { level: 1, name: "Collection".into(), kind: RowKind::ActiveCollection },
        ];
        let mut seen = vec![false; self.objects.len()];
        let mut roots: Vec<usize> =
            (0..self.objects.len()).filter(|&i| self.objects[i].parent.and_then(|p| self.index_of(p)).is_none()).collect();
        roots.sort_by_key(|&i| self.objects[i].name.to_lowercase());
        for r in roots {
            self.push_tree(r, 2, &mut seen, &mut rows);
        }
        /* Objects in a parent cycle (not reachable from a root) are listed flat so none disappears. */
        for i in 0..self.objects.len() {
            if !seen[i] {
                rows.push(OutlinerRow { level: 2, name: self.objects[i].name.clone(), kind: RowKind::Object(self.objects[i].id) });
            }
        }
        rows.push(OutlinerRow { level: 1, name: "XGen Guides".into(), kind: RowKind::Collection });
        rows
    }
}

/// The three transform properties of the Transform panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformProp {
    Location,
    /// Edited in degrees, stored in radians.
    Rotation,
    Scale,
    /// The size of the evaluated mesh bounds times the scale (meshes only); editing it sets the scale.
    Dimensions,
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

pub fn mat3_transpose(m: &Mat3) -> Mat3 {
    std::array::from_fn(|c| std::array::from_fn(|r| m[r][c]))
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
/// `a * b` of column-major matrices.
pub fn mat4_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [[0.0f32; 4]; 4];
    for (c, col) in out.iter_mut().enumerate() {
        for (r, v) in col.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

/// Splits a matrix into location, Euler rotation and scale (`mat4_to_loc_rot_size()`); a mirroring matrix
/// gets a negative X scale.
pub fn decompose(m: &Mat4) -> (V3, V3, V3) {
    let len = |c: usize| (m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2]).sqrt();
    let mut scale = [len(0), len(1), len(2)];
    let col = |c: usize, s: f32| -> V3 {
        if s > 1e-12 {
            [m[c][0] / s, m[c][1] / s, m[c][2] / s]
        } else {
            [0.0; 3]
        }
    };
    let (x, y, z) = (col(0, scale[0]), col(1, scale[1]), col(2, scale[2]));
    if dot(cross(x, y), z) < 0.0 {
        scale[0] = -scale[0];
    }
    let sx = if scale[0] < 0.0 { -1.0 } else { 1.0 };
    let r: Mat3 = [[x[0] * sx, x[1] * sx, x[2] * sx], y, z];
    ([m[3][0], m[3][1], m[3][2]], mat3_to_euler(&r), scale)
}

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
pub(crate) fn ray_triangle(origin: V3, dir: V3, a: V3, b: V3, c: V3) -> Option<f32> {
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
    #[test]
    fn the_startup_cube_has_the_default_material() {
        let s = Scene::startup();
        assert_eq!(s.materials.len(), 1);
        assert_eq!(s.materials[0].name, "Material");
        let cube = s.objects.iter().find(|o| o.name == "Cube").unwrap();
        assert_eq!((cube.material_slots.slots.len(), cube.material_slots.active, cube.material_slots.get(1)), (1, 1, Some(0)));
        let cam = s.objects.iter().find(|o| o.name == "Camera").unwrap();
        assert!(cam.material_slots.slots.is_empty());
    }

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
    fn restrict_toggles() {
        let mut s = Scene::startup();
        let id = s.objects.iter().find(|o| o.selected).unwrap().id;
        assert!(s.toggle_restrict(id, Restrict::Render));
        assert!(s.objects[s.index_of(id).unwrap()].hide_render);
        assert!(s.objects[s.index_of(id).unwrap()].selected);
        assert!(s.toggle_restrict(id, Restrict::Select));
        assert!(!s.objects[s.index_of(id).unwrap()].selected);
        assert!(s.toggle_restrict(id, Restrict::Viewport));
        assert!(s.objects[s.index_of(id).unwrap()].hidden);
        assert!(s.toggle_restrict(id, Restrict::Viewport));
        assert!(!s.objects[s.index_of(id).unwrap()].hidden);
        assert!(!s.toggle_restrict(9999, Restrict::Render));
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
    fn hide_unselected_keeps_the_selection_and_undoes() {
        let mut s = Scene::startup();
        assert_eq!(s.hide_unselected(), 2, "the camera and the light");
        assert_eq!(s.selected_count(), 1);
        assert!(s.objects.iter().filter(|o| !o.hidden).all(|o| o.name == "Cube"));
        assert_eq!(s.hide_unselected(), 0, "nothing left to hide");
        assert!(s.undo());
        assert!(s.objects.iter().all(|o| !o.hidden));
        /* With nothing selected everything is hidden, and Alt + H brings it all back selected. */
        s.select_all(SelectAction::Deselect);
        assert_eq!(s.hide_unselected(), 3);
        assert_eq!(s.reveal_all(), 3);
        assert_eq!(s.selected_count(), 3);
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

    /* Box select: a box over the cube selects it (solid and through), a box in a corner does not, Set clears the
     * old selection, Add keeps it and Sub removes only the hit. */
    #[test]
    fn box_select_modes() {
        use crate::select_box::BoxMode;
        let (w, h) = (200, 100);
        let mut rv3d = RegionView3D::default();
        rv3d.viewquat = crate::view3d::STARTUP_VIEWQUAT;
        rv3d.update(&View3D::default(), w, h);
        let mut s = Scene::startup();
        let cube = s.objects[1].id;
        for through in [false, true] {
            s.select_all(SelectAction::Deselect);
            assert!(s.select_box(&rv3d, w, h, [80, 120, 30, 70], BoxMode::Set, through));
            assert!(s.objects.iter().find(|o| o.id == cube).unwrap().selected, "through={through}");
            s.select_box(&rv3d, w, h, [0, 10, 0, 10], BoxMode::Set, through);
            assert!(s.objects.iter().all(|o| !o.selected), "Set with nothing under the box clears, through={through}");
            s.select_box(&rv3d, w, h, [80, 120, 30, 70], BoxMode::Add, through);
            s.select_box(&rv3d, w, h, [0, 10, 0, 10], BoxMode::Add, through);
            assert!(s.objects.iter().find(|o| o.id == cube).unwrap().selected, "Add keeps, through={through}");
            s.select_box(&rv3d, w, h, [80, 120, 30, 70], BoxMode::Sub, through);
            assert!(!s.objects.iter().find(|o| o.id == cube).unwrap().selected, "Sub removes, through={through}");
        }
    }

    #[test]
    fn copy_paste_adds_selected_copies() {
        let mut s = Scene::startup();
        let n = s.objects.len();
        let cube = s.objects.iter().find(|o| o.name == "Cube").unwrap().id;
        s.select_all(SelectAction::Deselect);
        assert!(s.copy_selected().is_none());
        s.select_pick(Some(cube), false);
        let clip = s.copy_selected().unwrap();
        assert_eq!(s.paste(&clip), 1);
        assert_eq!(s.objects.len(), n + 1);
        let pasted = s.objects.last().unwrap();
        assert_eq!(pasted.name, "Cube.001");
        assert!(pasted.selected && s.active == Some(pasted.id));
        assert!(!s.objects.iter().find(|o| o.id == cube).unwrap().selected);
        /* Pasting twice gives two more objects; undo takes one paste away. */
        assert_eq!(s.paste(&clip), 1);
        assert_eq!(s.objects.last().unwrap().name, "Cube.002");
        assert!(s.undo());
        assert_eq!(s.objects.len(), n + 1);
    }

    #[test]
    fn paste_drops_a_deleted_parent() {
        let mut s = Scene::startup();
        let cube = s.objects.iter().find(|o| o.name == "Cube").unwrap().id;
        let sphere = s.add_primitive(Primitive::UvSphere, [3.0, 0.0, 0.0]);
        s.select_all(SelectAction::Deselect);
        s.select_pick(Some(sphere), false);
        s.select_pick(Some(cube), true);
        s.active = Some(cube);
        assert!(s.parent_set() > 0);
        s.select_all(SelectAction::Deselect);
        s.select_pick(Some(sphere), false);
        let clip = s.copy_selected().unwrap();
        s.select_all(SelectAction::Deselect);
        s.select_pick(Some(cube), false);
        s.delete_selected();
        s.paste(&clip);
        assert_eq!(s.objects.last().unwrap().parent, None);
    }

    #[test]
    fn rename_active_is_unique_and_undoable() {
        let mut s = Scene::startup();
        let cube = s.active.unwrap();
        let other = s.objects.iter().find(|o| o.id != cube && matches!(o.data, ObjectData::Light(_))).unwrap().name.clone();
        assert!(!s.rename_active("Cube"));
        assert!(!s.rename_active("  "));
        assert!(s.rename_active(" Hat "));
        assert_eq!(s.objects.iter().find(|o| o.id == cube).unwrap().name, "Hat");
        assert!(s.rename_active(&other));
        assert_eq!(s.objects.iter().find(|o| o.id == cube).unwrap().name, format!("{other}.001"));
        assert!(s.undo());
        assert_eq!(s.objects.iter().find(|o| o.id == cube).unwrap().name, "Hat");
    }

    #[test]
    fn ray_triangle_is_double_sided() {
        let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(ray_triangle([0.2, 0.2, 1.0], [0.0, 0.0, -1.0], a, b, c).is_some());
        assert!(ray_triangle([0.2, 0.2, -1.0], [0.0, 0.0, 1.0], a, b, c).is_some());
        assert!(ray_triangle([2.0, 2.0, 1.0], [0.0, 0.0, -1.0], a, b, c).is_none());
        assert!(ray_triangle([0.2, 0.2, 1.0], [0.0, 0.0, 1.0], a, b, c).is_none());
    }
    fn cube_id(s: &Scene) -> u32 {
        s.objects.iter().find(|o| o.name == "Cube").unwrap().id
    }

    #[test]
    fn modifiers_build_the_evaluated_mesh_and_stack_with_subsurf() {
        let mut s = Scene::startup();
        assert_eq!(s.set_array(3), 1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 18);
        assert_eq!(s.array_count(), 3);
        s.set_subsurf(1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 18 * 4);
        assert_eq!(s.set_array(3), 0, "no change, no undo step");
        assert_eq!(s.set_array(1), 1);
        s.update_eval();
        assert!(s.active_object().unwrap().modifiers.is_empty());
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 24);
    }

    #[test]
    fn mirror_toggles_axes_and_removes_itself() {
        let mut s = Scene::startup();
        s.toggle_mirror(0);
        s.toggle_mirror(1);
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
        s.toggle_mirror(0);
        s.toggle_mirror(1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn solidify_toggles_and_undoes() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_solidify(), 1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 12);
        assert!(s.undo());
        assert!(s.active_object().unwrap().modifiers.is_empty());
        assert!(s.redo());
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
    }

    #[test]
    fn duplicate_copies_the_modifiers() {
        let mut s = Scene::startup();
        s.toggle_solidify();
        s.duplicate_selected();
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
    }

    #[test]
    fn apply_modifiers_bakes_them_in() {
        let mut s = Scene::startup();
        s.set_array(2);
        s.set_subsurf(1);
        assert_eq!(s.apply_modifiers(), 1);
        let o = s.active_object().unwrap();
        assert!(o.modifiers.is_empty() && o.subsurf == 0);
        assert_eq!(o.mesh().unwrap().faces.len(), 48);
        assert_eq!(s.apply_modifiers(), 0);
    }

    #[test]
    fn apply_transforms_bakes_location_rotation_and_scale() {
        let mut s = Scene::startup();
        let id = cube_id(&s);
        let i = s.index_of(id).unwrap();
        s.objects[i].loc = [1.0, 2.0, 3.0];
        s.objects[i].scale = [2.0, 1.0, 1.0];
        s.objects[i].rot = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        let before = s.objects[i].world_bounds().unwrap();
        assert_eq!(s.apply_transforms(), 1);
        let o = &s.objects[i];
        assert_eq!((o.loc, o.rot, o.scale), ([0.0; 3], [0.0; 3], [1.0; 3]));
        let after = o.world_bounds().unwrap();
        assert!(approx(before.0, after.0) && approx(before.1, after.1), "{before:?} {after:?}");
        assert_eq!(s.apply_transforms(), 0);
    }

    #[test]
    fn apply_a_negative_scale_keeps_the_faces_outward() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        s.objects[i].scale = [-1.0, 1.0, 1.0];
        s.apply_transforms();
        assert!(s.objects[i].mesh().unwrap().signed_volume() > 0.0);
    }

    #[test]
    fn origin_to_geometry_keeps_the_mesh_in_place() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        if let ObjectData::Mesh(m) = &mut s.objects[i].data {
            *m = m.mapped(|v| [v[0] + 4.0, v[1], v[2]], false);
        }
        s.objects[i].scale = [2.0, 2.0, 2.0];
        let before = s.objects[i].world_bounds().unwrap();
        assert_eq!(s.origin_to_geometry(), 1);
        let o = &s.objects[i];
        assert!(approx(o.loc, [8.0, 0.0, 0.0]), "{:?}", o.loc);
        assert!(approx(o.mesh().unwrap().median_center().unwrap(), [0.0; 3]));
        let after = o.world_bounds().unwrap();
        assert!(approx(before.0, after.0) && approx(before.1, after.1));
    }

    #[test]
    fn join_merges_into_the_active_object_in_its_space() {
        let mut s = Scene::startup();
        let a = s.index_of(cube_id(&s)).unwrap();
        s.objects[a].loc = [10.0, 0.0, 0.0];
        s.objects[a].scale = [2.0, 2.0, 2.0];
        let id = s.add_primitive(Primitive::Cube, [20.0, 0.0, 0.0]);
        s.select_pick(Some(cube_id(&s)), true);
        let b = s.index_of(id).unwrap();
        s.objects[b].selected = true;
        s.active = Some(cube_id(&s));
        assert_eq!(s.join_selected(), 1);
        assert!(s.index_of(id).is_none());
        let o = s.active_object().unwrap();
        assert_eq!(o.mesh().unwrap().faces.len(), 12);
        /* Both cubes are where they were: x from 8 to 12 and from 19 to 21. */
        let (lo, hi) = o.world_bounds().unwrap();
        assert!(approx(lo, [8.0, -2.0, -2.0]) && approx(hi, [21.0, 2.0, 2.0]), "{lo:?} {hi:?}");
        assert!(s.undo());
        assert!(s.index_of(id).is_some());
    }

    #[test]
    fn join_needs_two_meshes_and_a_mesh_active() {
        let mut s = Scene::startup();
        assert_eq!(s.join_selected(), 0);
        s.select_all(SelectAction::Select);
        s.active = s.objects.iter().find(|o| o.name == "Light").map(|o| o.id);
        assert_eq!(s.join_selected(), 0);
        assert_eq!(s.objects.len(), 3);
    }

    #[test]
    fn mesh_operators_flip_recalc_triangulate_merge() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        assert_eq!(s.recalc_normals(), 0, "nothing to fix, no undo step");
        assert!(!s.can_undo());
        assert_eq!(s.flip_normals(), 1);
        assert!(s.objects[i].mesh().unwrap().signed_volume() < 0.0);
        assert_eq!(s.recalc_normals(), 1);
        assert!(s.objects[i].mesh().unwrap().signed_volume() > 0.0);
        assert_eq!(s.triangulate(), 1);
        assert_eq!(s.objects[i].mesh().unwrap().faces.len(), 12);
        assert_eq!(s.triangulate(), 0);
        assert_eq!(s.merge_by_distance(0.001), 0);
        assert!(s.undo() && s.undo() && s.undo());
        assert_eq!(s.objects[i].mesh().unwrap().faces.len(), 6);
    }
    #[test]
    fn bevel_toggles_in_the_stack() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_bevel(), 1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 26);
        assert_eq!(s.toggle_bevel(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn deform_smooth_and_weld_toggle_in_the_stack() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_smooth_modifier(), 1);
        assert_eq!(s.toggle_weld(), 1);
        /* Twist, Bend, Taper, Stretch. */
        for _ in 0..4 {
            assert_eq!(s.cycle_simple_deform(), 1);
        }
        assert_eq!(s.active_object().unwrap().modifiers.len(), 3);
        assert_eq!(s.cycle_simple_deform(), 1);
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        assert_eq!(s.toggle_smooth_modifier(), 1);
        assert_eq!(s.toggle_weld(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn displace_and_triangulate_toggle_and_evaluate() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_displace(), 1);
        assert_eq!(s.toggle_triangulate_modifier(), 1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 12);
        assert!(s.undo());
        assert!(s.undo());
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn cast_cycles_and_wireframe_toggles() {
        let mut s = Scene::startup();
        assert_eq!(s.cycle_cast(), 1);
        assert_eq!(s.cycle_cast(), 1);
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
        assert_eq!(s.cycle_cast(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
        assert_eq!(s.toggle_wireframe(), 1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 96);
        assert_eq!(s.toggle_wireframe(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn laplacian_smooth_and_remesh_toggle() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_remesh(), 1);
        assert_eq!(s.toggle_laplacian_smooth(), 1);
        s.update_eval();
        assert!(s.active_object().unwrap().evaluated().unwrap().faces.len() > 6);
        assert_eq!(s.toggle_remesh(), 1);
        assert_eq!(s.toggle_laplacian_smooth(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn build_reveals_the_mesh_as_the_frame_advances() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_build(), 1);
        s.update_eval();
        let faces = |s: &Scene| s.active_object().unwrap().evaluated().unwrap().faces.len();
        assert_eq!(faces(&s), 0);
        assert!(s.set_frame(51));
        s.update_eval();
        assert_eq!(faces(&s), 3);
        assert!(!s.set_frame(51), "same frame: nothing to redo");
        assert!(s.set_frame(101));
        s.update_eval();
        assert_eq!(faces(&s), 6);
        /* Convert to Mesh bakes the current frame. */
        assert!(s.set_frame(51));
        assert_eq!(s.apply_modifiers(), 1);
        assert_eq!(s.active_object().unwrap().mesh().unwrap().faces.len(), 3);
        assert_eq!(s.toggle_build(), 1);
    }

    #[test]
    fn boolean_cuts_with_its_target_and_follows_it() {
        let mut s = Scene::startup();
        let cube = s.active;
        let cutter = s.add_primitive(crate::mesh::Primitive::Cube, [1.0, 1.0, 1.0]);
        s.active = cube;
        assert!(s.panel_add_modifier(Modifier::boolean()));
        assert!(s.panel_cycle_target(0));
        assert_eq!(s.target_name(0), Some(s.objects[s.index_of(cutter).unwrap()].name.clone()));
        s.update_eval();
        let cut = s.active_object().unwrap().evaluated().unwrap().clone();
        assert!(cut.faces.len() > 6, "the corner is cut out");
        /* Moving the cutter away restores the plain cube. */
        let j = s.index_of(cutter).unwrap();
        s.objects[j].loc = [20.0, 0.0, 0.0];
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 6);
        /* Applying bakes the cut into the mesh and removes the modifier. */
        s.objects[j].loc = [1.0, 1.0, 1.0];
        s.update_eval();
        assert!(s.panel_apply_modifier(0));
        let o = s.active_object().unwrap();
        assert!(o.modifiers.is_empty());
        assert_eq!(o.mesh().unwrap().faces.len(), cut.faces.len());
    }

    #[test]
    fn shrinkwrap_follows_its_target() {
        let mut s = Scene::startup();
        let cube = s.active;
        let sphere = s.add_primitive(crate::mesh::Primitive::UvSphere, [0.0; 3]);
        s.active = cube;
        assert!(s.panel_add_modifier(Modifier::shrinkwrap()));
        assert!(s.panel_cycle_target(0));
        assert_eq!(s.target_name(0), s.objects[s.index_of(sphere).unwrap()].name.clone().into());
        s.update_eval();
        let before = s.active_object().unwrap().evaluated().unwrap().verts.clone();
        assert_ne!(before, s.active_object().unwrap().mesh().unwrap().verts);
        /* Moving the target rebuilds the wrapped mesh without touching the owner. */
        let j = s.index_of(sphere).unwrap();
        s.objects[j].loc = [5.0, 0.0, 0.0];
        s.update_eval();
        assert_ne!(s.active_object().unwrap().evaluated().unwrap().verts, before);
        /* The picker ends on no target, then starts over. */
        assert!(s.panel_cycle_target(0));
        assert_eq!(s.target_name(0), None);
    }

    #[test]
    fn screw_and_wave_toggle() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_screw(), 1);
        assert_eq!(s.toggle_wave(), 1);
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        s.update_eval();
        assert!(s.active_object().unwrap().evaluated().is_some());
        assert_eq!(s.toggle_screw(), 1);
        assert_eq!(s.toggle_wave(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn decimate_cycles_and_edge_split_toggles() {
        let mut s = Scene::startup();
        for _ in 0..3 {
            assert_eq!(s.cycle_decimate(), 1);
        }
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
        assert_eq!(s.cycle_decimate(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
        assert_eq!(s.toggle_edge_split(), 1);
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().verts.len(), 24);
        assert_eq!(s.toggle_edge_split(), 1);
        assert!(s.active_object().unwrap().modifiers.is_empty());
    }

    #[test]
    fn the_active_modifier_can_be_selected_adjusted_moved_and_removed() {
        let mut s = Scene::startup();
        assert_eq!(s.adjust_modifier(1), 0);
        s.toggle_bevel();
        s.toggle_displace();
        s.cycle_cast();
        /* The newest modifier is active. */
        assert_eq!(s.active_object().unwrap().active_modifier, 2);
        assert_eq!(s.adjust_modifier(2), 1);
        assert_eq!(s.active_object().unwrap().modifiers[2], Modifier::Cast { shape: crate::mesh_deform::CastShape::Sphere, factor: 0.7, radius: 0.0 });
        assert_eq!(s.select_modifier(1), 1);
        assert_eq!(s.active_object().unwrap().active_modifier, 0);
        assert_eq!(s.select_modifier(-1), 1);
        assert_eq!(s.active_object().unwrap().active_modifier, 2);
        /* Move the Cast to the front: it goes up twice, the third time there is nowhere to go. */
        assert_eq!(s.move_modifier(-1), 1);
        assert_eq!(s.move_modifier(-1), 1);
        assert_eq!(s.move_modifier(-1), 0);
        let o = s.active_object().unwrap();
        assert_eq!((o.active_modifier, o.modifiers[0].label(), o.modifiers[2].label()), (0, "Cast", "Displace"));
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().modifiers[1].label(), "Cast");
        assert_eq!(s.remove_modifier(), 1);
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        assert_eq!(s.remove_modifier(), 1);
        assert_eq!(s.remove_modifier(), 1);
        assert_eq!(s.remove_modifier(), 0);
    }

    #[test]
    fn subdivide_simple_is_an_undoable_mesh_edit() {
        let mut s = Scene::startup();
        assert_eq!(s.subdivide_simple(), 1);
        assert_eq!(s.active_object().unwrap().mesh().unwrap().faces.len(), 24);
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().mesh().unwrap().faces.len(), 6);
    }

    #[test]
    fn separate_loose_creates_objects_with_the_same_transform() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        s.objects[i].loc = [1.0, 2.0, 3.0];
        s.objects[i].smooth = true;
        assert_eq!(s.separate_loose(), 0, "one piece: nothing to do");
        assert!(!s.can_undo());
        if let ObjectData::Mesh(m) = &mut s.objects[i].data {
            m.append(&Mesh::cube(1.0).mapped(|v| [v[0] + 5.0, v[1], v[2]], false));
        }
        assert_eq!(s.separate_loose(), 1);
        let cubes: Vec<&Object> = s.objects.iter().filter(|o| o.name.starts_with("Cube")).collect();
        assert_eq!(cubes.len(), 2);
        assert!(cubes.iter().all(|o| o.loc == [1.0, 2.0, 3.0] && o.smooth && o.mesh().unwrap().faces.len() == 6));
        assert_eq!(cubes[1].name, "Cube.001");
        assert!(s.undo());
        assert_eq!(s.objects.iter().filter(|o| o.name.starts_with("Cube")).count(), 1);
    }

    #[test]
    fn snap_selection_to_cursor_keeps_relative_offsets() {
        let mut s = Scene::startup();
        let a = s.index_of(cube_id(&s)).unwrap();
        let id = s.add_primitive(Primitive::Cube, [4.0, 0.0, 0.0]);
        s.objects[a].selected = true;
        let b = s.index_of(id).unwrap();
        s.objects[a].loc = [0.0; 3];
        assert!(approx(s.cursor_target().unwrap(), [2.0, 0.0, 0.0]));
        assert_eq!(s.selection_to_cursor([10.0, 0.0, 0.0]), 2);
        assert!(approx(s.objects[a].loc, [8.0, 0.0, 0.0]) && approx(s.objects[b].loc, [12.0, 0.0, 0.0]));
        assert_eq!(s.selection_to_cursor([10.0, 0.0, 0.0]), 0, "already there");
    }

    #[test]
    fn origin_to_cursor_keeps_the_mesh_in_place() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        s.objects[i].rot = [0.0, 0.0, 0.7];
        s.objects[i].scale = [2.0, 1.0, 3.0];
        let before = s.objects[i].world_bounds().unwrap();
        assert_eq!(s.origin_to_point([1.0, 1.0, 1.0]), 1);
        assert_eq!(s.objects[i].loc, [1.0, 1.0, 1.0]);
        let after = s.objects[i].world_bounds().unwrap();
        assert!(approx(before.0, after.0) && approx(before.1, after.1), "{before:?} {after:?}");
    }

    #[test]
    fn convex_hull_replaces_the_mesh_and_undoes() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        if let ObjectData::Mesh(m) = &mut s.objects[i].data {
            m.verts.push([0.0; 3]);
        }
        assert_eq!(s.convex_hull(), 1);
        assert_eq!(s.objects[i].mesh().unwrap().verts.len(), 8);
        assert_eq!(s.convex_hull(), 0, "already convex");
        assert!(s.undo());
        assert_eq!(s.objects[i].mesh().unwrap().verts.len(), 9);
    }

    #[test]
    fn mirror_reflects_about_the_median_and_flips_scale() {
        let mut s = Scene::startup();
        let a = s.index_of(cube_id(&s)).unwrap();
        let id = s.add_primitive(Primitive::Cube, [4.0, 0.0, 0.0]);
        s.objects[a].selected = true;
        let b = s.index_of(id).unwrap();
        s.objects[a].loc = [0.0; 3];
        assert_eq!(s.mirror_objects(0), 2);
        assert!(approx(s.objects[a].loc, [4.0, 0.0, 0.0]) && approx(s.objects[b].loc, [0.0, 0.0, 0.0]));
        assert_eq!(s.objects[a].scale, [-1.0, 1.0, 1.0]);
        assert!(s.undo());
        assert!(approx(s.objects[a].loc, [0.0; 3]) && s.objects[a].scale == [1.0; 3]);
    }

    #[test]
    fn mirror_keeps_the_world_matrix_a_reflection_of_the_old_one() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        s.objects[i].rot = [0.3, 0.5, 0.9];
        s.objects[i].scale = [1.0, 2.0, 3.0];
        s.objects[i].loc = [0.0; 3];
        let before = s.objects[i].world_matrix();
        assert_eq!(s.mirror_objects(1), 1);
        let after = s.objects[i].world_matrix();
        /* Reflecting the old matrix in world Y flips its Y row. */
        for c in 0..3 {
            assert!((after[c][0] - before[c][0]).abs() < 1e-5, "x row, column {c}");
            assert!((after[c][1] + before[c][1]).abs() < 1e-5, "y row, column {c}");
            assert!((after[c][2] - before[c][2]).abs() < 1e-5, "z row, column {c}");
        }
    }

    #[test]
    fn select_grouped_by_type_and_random() {
        let mut s = Scene::startup();
        s.add_primitive(Primitive::Cube, [3.0, 0.0, 0.0]);
        s.add_primitive(Primitive::Plane, [6.0, 0.0, 0.0]);
        let plane = s.active.unwrap();
        s.select_all(SelectAction::Deselect);
        s.active = Some(plane);
        assert_eq!(s.select_grouped_type(), 3, "the other meshes (the camera and the light stay out)");
        assert_eq!(s.selected_count(), 3);
        s.select_all(SelectAction::Deselect);
        assert_eq!(s.select_random(1.0), s.objects.len());
        s.select_all(SelectAction::Deselect);
        assert_eq!(s.select_random(0.0), 0);
        s.select_all(SelectAction::Deselect);
        let first: Vec<bool> = {
            s.select_random(0.5);
            s.objects.iter().map(|o| o.selected).collect()
        };
        s.select_all(SelectAction::Deselect);
        s.select_random(0.5);
        let second: Vec<bool> = s.objects.iter().map(|o| o.selected).collect();
        assert!(first.iter().any(|&x| x) || second.iter().any(|&x| x));
    }

    /// A cube at (4, 0, 0) parented to the startup cube (which is moved and turned first).
    fn parented_pair() -> (Scene, usize, usize) {
        let mut s = Scene::startup();
        let parent = s.index_of(cube_id(&s)).unwrap();
        let cid = s.add_primitive(Primitive::Cube, [4.0, 0.0, 0.0]);
        let child = s.index_of(cid).unwrap();
        s.objects[parent].loc = [0.0, 1.0, 0.0];
        s.objects[parent].rot = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        s.objects[parent].scale = [2.0, 2.0, 2.0];
        s.refresh_parents();
        s.objects[parent].selected = true;
        s.active = Some(s.objects[parent].id);
        (s, parent, child)
    }

    #[test]
    fn parenting_keeps_the_child_in_place_and_it_follows_the_parent() {
        let (mut s, p, c) = parented_pair();
        let before = mat4_mul_point(&s.objects[c].world_matrix(), [1.0, 0.0, 0.0]);
        assert_eq!(s.parent_set(), 1);
        assert_eq!(s.objects[c].parent, Some(s.objects[p].id));
        assert!(approx(mat4_mul_point(&s.objects[c].world_matrix(), [1.0, 0.0, 0.0]), before), "no jump");
        /* Moving the parent by +2 in X moves the child by the same. */
        s.objects[p].loc[0] += 2.0;
        s.refresh_parents();
        let after = mat4_mul_point(&s.objects[c].world_matrix(), [1.0, 0.0, 0.0]);
        assert!(approx(after, [before[0] + 2.0, before[1], before[2]]), "{after:?}");
        /* Turning the parent a further quarter turn about Z swings the child about the parent origin. */
        s.objects[p].loc = [0.0, 1.0, 0.0];
        s.objects[p].rot[2] += std::f32::consts::FRAC_PI_2;
        s.refresh_parents();
        let swung = mat4_mul_point(&s.objects[c].world_matrix(), [0.0; 3]);
        let rel = [4.0 - 0.0, 0.0 - 1.0];
        assert!(approx(swung, [-rel[1] + 0.0, rel[0] + 1.0, 0.0]), "{swung:?}");
    }

    #[test]
    fn parent_clear_keeps_or_drops_the_transform_and_undoes() {
        let (mut s, p, c) = parented_pair();
        s.parent_set();
        s.objects[p].loc[0] += 3.0;
        s.refresh_parents();
        let world = s.objects[c].world_matrix();
        s.select_all(SelectAction::Deselect);
        s.objects[c].selected = true;
        assert_eq!(s.parent_clear(true), 1);
        assert!(s.objects[c].parent.is_none());
        let now = s.objects[c].world_matrix();
        for col in 0..4 {
            for row in 0..4 {
                assert!((now[col][row] - world[col][row]).abs() < 1e-4, "{now:?} {world:?}");
            }
        }
        assert!(s.undo());
        assert!(s.objects[c].parent.is_some());
        assert_eq!(s.parent_clear(false), 1);
        assert!(approx(s.objects[c].loc, [4.0, 0.0, 0.0]), "back to the local transform");
        assert_eq!(s.parent_clear(false), 0);
    }

    #[test]
    fn parenting_refuses_cycles_and_deleting_the_parent_frees_the_child_in_place() {
        let (mut s, p, c) = parented_pair();
        s.parent_set();
        /* Making the child the parent of its own parent is refused. */
        s.select_all(SelectAction::Deselect);
        s.objects[p].selected = true;
        s.active = Some(s.objects[c].id);
        assert_eq!(s.parent_set(), 0);
        /* Delete the parent. */
        let world = mat4_mul_point(&s.objects[c].world_matrix(), [1.0, 1.0, 1.0]);
        s.select_all(SelectAction::Deselect);
        s.objects[p].selected = true;
        assert_eq!(s.delete_selected(), 1);
        let c = s.index_of(s.objects.iter().find(|o| o.parent.is_none() && o.name == "Cube.001").unwrap().id).unwrap();
        assert!(approx(mat4_mul_point(&s.objects[c].world_matrix(), [1.0, 1.0, 1.0]), world));
    }

    #[test]
    fn duplicating_a_family_parents_the_copies_to_each_other() {
        let (mut s, p, c) = parented_pair();
        s.parent_set();
        s.objects[c].selected = true;
        assert_eq!(s.duplicate_selected(), 2);
        let parent_copy = s.objects.iter().find(|o| o.name == "Cube.002").map(|o| o.id);
        let child_copy = s.objects.iter().find(|o| o.name == "Cube.003").unwrap();
        assert_ne!(child_copy.parent, Some(s.objects[p].id));
        assert_eq!(child_copy.parent, parent_copy);
    }

    #[test]
    fn the_outliner_nests_children_and_select_hierarchy_walks_it() {
        let (mut s, p, c) = parented_pair();
        s.parent_set();
        let rows = s.outliner_rows();
        let names: Vec<(i32, &str)> = rows.iter().map(|r| (r.level, r.name.as_str())).collect();
        let at = names.iter().position(|&(_, n)| n == "Cube").unwrap();
        assert_eq!(names[at + 1], (3, "Cube.001"));
        let (pid, cid) = (s.objects[p].id, s.objects[c].id);
        assert!(s.select_hierarchy(false, false));
        assert_eq!((s.active, s.selected_count()), (Some(cid), 1));
        assert!(!s.select_hierarchy(false, false), "no grandchild");
        assert!(s.select_hierarchy(true, true));
        assert_eq!((s.active, s.selected_count()), (Some(pid), 2));
    }

    #[test]
    fn decompose_inverts_the_world_matrix() {
        let mut s = Scene::startup();
        let i = s.index_of(cube_id(&s)).unwrap();
        s.objects[i].loc = [1.0, -2.0, 3.0];
        s.objects[i].rot = [0.2, -0.4, 0.9];
        s.objects[i].scale = [-2.0, 1.5, 0.5];
        let (loc, rot, scale) = decompose(&s.objects[i].world_matrix());
        let mut o = s.objects[i].clone();
        o.loc = loc;
        o.rot = rot;
        o.scale = scale;
        let (a, b) = (s.objects[i].world_matrix(), o.world_matrix());
        for c in 0..4 {
            for r in 0..4 {
                assert!((a[c][r] - b[c][r]).abs() < 1e-4, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn add_item_creates_selected_active_objects_of_each_kind() {
        use crate::add_menu::AddItem;
        let mut s = Scene::startup();
        for (item, name) in [(AddItem::Empty(EmptyKind::PlainAxes), "Empty"), (AddItem::Camera, "Camera.001"), (AddItem::Light(LightKind::Point), "Point")] {
            let id = s.add_item(item, [1.0, 2.0, 3.0]);
            let o = &s.objects[s.index_of(id).unwrap()];
            assert_eq!(o.name, name);
            assert_eq!(o.loc, [1.0, 2.0, 3.0]);
            assert!(o.selected && s.active == Some(id) && s.selected_count() == 1);
            assert!(o.mesh().is_none());
        }
        assert!(matches!(s.objects.last().unwrap().data, ObjectData::Light(LightKind::Point)));
        assert!(s.undo(), "adding is one undo step");
        assert_eq!(s.objects.len(), 5);
        let mesh = s.add_item(AddItem::Mesh(Primitive::Cube), [0.0; 3]);
        assert!(s.objects[s.index_of(mesh).unwrap()].mesh().is_some());
    }

    #[test]
    fn copy_modifiers_replaces_the_stacks_of_the_other_selected_meshes() {
        let mut s = Scene::startup();
        let orig = s.active.unwrap();
        assert_eq!(s.copy_modifiers_to_selected(), 0, "nothing else is selected");
        s.duplicate_selected();
        let copy = s.active.unwrap();
        assert_ne!(copy, orig);
        assert!(s.panel_add_modifier(Modifier::array(3)));
        assert!(s.panel_add_modifier(Modifier::weld()));
        let ci = s.index_of(copy).unwrap();
        s.objects[ci].subsurf = 2;
        s.select_all(SelectAction::Select);
        s.active = Some(copy);
        assert_eq!(s.copy_modifiers_to_selected(), 1);
        let o = &s.objects[s.index_of(orig).unwrap()];
        assert_eq!(o.modifiers, vec![Modifier::array(3), Modifier::weld()]);
        assert_eq!(o.subsurf, 2);
        assert_eq!(s.copy_modifiers_to_selected(), 0, "already identical: no undo step");
        assert!(s.undo());
        let o = &s.objects[s.index_of(orig).unwrap()];
        assert!(o.modifiers.is_empty() && o.subsurf == 0);
    }
}
