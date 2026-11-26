//! More object operators, as a child module of `scene` so that they can reach the private parts of `Object`.
//!
//! Modelled on Blender:
//!  - `OBJECT_OT_align_objects` (`relative_to`: Scene Origin / 3D Cursor / Selection / Active, `align_mode`:
//!    Negative Sides / Centers / Positive Sides, one global axis at a time)
//!  - `OBJECT_OT_transform_randomize` (location, rotation and scale; a seed that counts up on every call)
//!  - `OBJECT_OT_select_grouped` for Children, Immediate Children, Parent and Siblings (`Type` lives in `scene`)
//!  - `OBJECT_OT_origin_set` with `type = ORIGIN_CENTER_OF_VOLUME`: the volume centroid of a closed mesh
//!  - `VIEW3D_OT_snap_selected_to_active` (Selection to Active)
//!  - `OBJECT_OT_origin_set` with `type = ORIGIN_CENTER_OF_VOLUME` is joined by the bounding box center
//!  - `OBJECT_OT_parent_clear` with `type = CLEAR_INVERSE`
//!  - `OBJECT_OT_select_mirror` (`BLI_string_flip_side_name()`: `.L` / `.R`, `_l` / `_r`, `Left` / `Right`, prefixes too)
//!  - `OBJECT_OT_select_linked` (Object Data: every object whose mesh is identical to the active one's, since meshes
//!    are not shared between objects here; Modifiers: the same modifier stack, the stand-in for linked modifiers)
//!  - the statistics overlay (`Scene::statistics`): objects, vertices, faces and triangles
//!  - `OBJECT_OT_transform_apply` with its `location` / `rotation` / `scale` options told apart (`ApplyMask`), keeping
//!    the children of an applied object in place
//!  - `VIEW3D_OT_snap_selected_to_grid` (Selection to Grid)
//!  - `VIEW3D_OT_localview`: shows only the selected objects until it is toggled off
//!  - `OBJECT_OT_parent_no_inverse_set` (Make Parent without Inverse, with and without Keep Transform)
//!  - `OBJECT_OT_make_links_data` with `type = OBDATA` (Link Object Data), and the grid snap of the 3D cursor
//!
//! Deviations: Randomize Transform uses its own hash instead of Blender's random number generator, so a given seed
//! does not give the same numbers, and its defaults (1 m, 15 degrees, 20 %) differ because Blender's are all zero
//! and the operator would do nothing at the keys. Origin to Center of Mass uses the volume centroid (it ignores the
//! surface variant) and falls back to the median of the vertices for an open mesh.

use super::*;

/// What `OBJECT_OT_align_objects` aligns to (`relative_to`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignTo {
    SceneOrigin,
    Cursor(V3),
    Selection,
    Active,
}

/// `align_mode`: which side of the object bounds is moved onto the reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignMode {
    NegativeSides,
    Centers,
    PositiveSides,
}

/// `OBJECT_OT_select_grouped`, the relationship types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    /// Every descendant (`CHILDREN`).
    Children,
    /// The direct children only (`CHILDREN_IMMEDIATE`).
    ChildrenImmediate,
    /// The parent of the active object (`PARENT`).
    Parent,
    /// The other children of the active object's parent (`SIBLINGS`).
    Siblings,
}

/// The amounts of `OBJECT_OT_transform_randomize`: a random offset in `[-amount, amount]` per axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Randomize {
    pub loc: V3,
    /// Radians.
    pub rot: V3,
    /// Relative: the scale is multiplied by `1 + amount * r`.
    pub scale: V3,
    /// One random number for all three scale axes (`scale_even`).
    pub scale_even: bool,
}

impl Default for Randomize {
    fn default() -> Self {
        let deg15 = 15.0f32.to_radians();
        Randomize { loc: [1.0; 3], rot: [deg15; 3], scale: [0.2; 3], scale_even: false }
    }
}

/// Counts shown by the statistics overlay (`ED_info_draw_stats()`), for the visible objects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub objects: usize,
    pub selected: usize,
    pub verts: usize,
    pub faces: usize,
    pub tris: usize,
}

/// `BLI_string_flip_side_name()` for the common cases: the name with its left / right token flipped, `None` when it has
/// none. The token is a prefix or a suffix next to a `.`, `_`, `-` or space, in the case of the original, and a
/// trailing `.001` style number is kept.
pub fn flip_side_name(name: &str) -> Option<String> {
    let (base, num) = match name.rfind('.') {
        Some(i) if i > 0 && name.len() > i + 1 && name[i + 1..].bytes().all(|b| b.is_ascii_digit()) => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let flip = |tok: &str| -> Option<&'static str> {
        Some(match tok {
            "L" => "R",
            "R" => "L",
            "l" => "r",
            "r" => "l",
            "Left" => "Right",
            "Right" => "Left",
            "left" => "right",
            "right" => "left",
            "LEFT" => "RIGHT",
            "RIGHT" => "LEFT",
            _ => return None,
        })
    };
    let is_sep = |c: char| matches!(c, '.' | '_' | '-' | ' ');
    /* A suffix: `Arm.L`. */
    if let Some(i) = base.rfind(is_sep) {
        if let Some(f) = flip(&base[i + 1..]) {
            return Some(format!("{}{}{}", &base[..=i], f, num));
        }
    }
    /* A prefix: `L_Arm`. */
    if let Some(i) = base.find(is_sep) {
        if let Some(f) = flip(&base[..i]) {
            return Some(format!("{}{}{}", f, &base[i..], num));
        }
    }
    None
}

/// A small integer hash (lowbias32) mapped to `[-1, 1)`.
fn rand_signed(id: u32, seed: u32, channel: u32) -> f32 {
    let mut h = id
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(seed.wrapping_mul(0x85EB_CA6B))
        .wrapping_add(channel.wrapping_mul(0xC2B2_AE35))
        .wrapping_add(0x1234_5678);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    (h as f32 / 4_294_967_296.0) * 2.0 - 1.0
}

/// The volume centroid of a closed triangulated surface, `None` when it has no volume (flat or open meshes with
/// a tiny signed volume).
fn volume_centroid(mesh: &Mesh) -> Option<V3> {
    let mut vol = 0.0f64;
    let mut c = [0.0f64; 3];
    for (_, t) in mesh.triangles() {
        let a = mesh.verts[t[0] as usize].map(f64::from);
        let b = mesh.verts[t[1] as usize].map(f64::from);
        let d = mesh.verts[t[2] as usize].map(f64::from);
        /* The signed volume of the tetrahedron with the origin as its apex. */
        let v = (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0]) + a[2] * (b[0] * d[1] - b[1] * d[0])) / 6.0;
        vol += v;
        for k in 0..3 {
            c[k] += v * (a[k] + b[k] + d[k]) / 4.0;
        }
    }
    if vol.abs() < 1e-12 {
        return None;
    }
    Some([(c[0] / vol) as f32, (c[1] / vol) as f32, (c[2] / vol) as f32])
}

impl Scene {
    fn depth_of(&self, i: usize) -> u32 {
        let mut depth = 0;
        let mut cur = self.objects[i].parent;
        while let Some(p) = cur {
            depth += 1;
            if depth > MAX_PARENT_DEPTH {
                break;
            }
            cur = self.index_of(p).and_then(|j| self.objects[j].parent);
        }
        depth
    }

    /// Moves object `i` by a world space offset (the offset of `loc` is the offset in the parent's space).
    fn translate_world(&mut self, i: usize, delta: V3) {
        let lin = self.objects[i].parent_lin_inv();
        let d = mat3_mul_v3(&lin, delta);
        let o = &mut self.objects[i];
        o.loc = [o.loc[0] + d[0], o.loc[1] + d[1], o.loc[2] + d[2]];
        o.eval = None;
    }

    /// The world bounds of object `i`; an object without geometry counts as its origin.
    fn align_bounds(&self, i: usize) -> (V3, V3) {
        let o = &self.objects[i];
        o.world_bounds().unwrap_or_else(|| {
            let p = o.world_matrix()[3];
            ([p[0], p[1], p[2]], [p[0], p[1], p[2]])
        })
    }

    /// `OBJECT_OT_align_objects`: moves the selected objects along the global `axis` (0, 1, 2) so that the
    /// `mode` side of their bounds meets the reference. Returns how many objects moved.
    pub fn align_objects(&mut self, axis: usize, mode: AlignMode, to: AlignTo) -> usize {
        if axis > 2 {
            return 0;
        }
        let sel: Vec<usize> = (0..self.objects.len()).filter(|&i| self.objects[i].selected && !self.objects[i].hidden).collect();
        if sel.is_empty() {
            return 0;
        }
        self.refresh_parents();
        /* The reference range along the axis (a point for the cursor and the origin). */
        let (lo, hi) = match to {
            AlignTo::SceneOrigin => (0.0, 0.0),
            AlignTo::Cursor(p) => (p[axis], p[axis]),
            AlignTo::Selection => sel.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &i| {
                let (a, b) = self.align_bounds(i);
                (lo.min(a[axis]), hi.max(b[axis]))
            }),
            AlignTo::Active => match self.active.and_then(|id| self.index_of(id)) {
                Some(a) => {
                    let (a, b) = self.align_bounds(a);
                    (a[axis], b[axis])
                }
                None => return 0,
            },
        };
        let target = match mode {
            AlignMode::NegativeSides => lo,
            AlignMode::Centers => 0.5 * (lo + hi),
            AlignMode::PositiveSides => hi,
        };
        /* Parents first: moving one moves its children, whose bounds are read after that. */
        let mut order = sel;
        order.sort_by_key(|&i| self.depth_of(i));
        self.push_undo();
        let mut moved = 0;
        for &i in &order {
            self.refresh_parents();
            let (a, b) = self.align_bounds(i);
            let here = match mode {
                AlignMode::NegativeSides => a[axis],
                AlignMode::Centers => 0.5 * (a[axis] + b[axis]),
                AlignMode::PositiveSides => b[axis],
            };
            let d = target - here;
            if d.abs() < 1e-7 {
                continue;
            }
            let mut delta = [0.0; 3];
            delta[axis] = d;
            self.translate_world(i, delta);
            moved += 1;
        }
        if moved == 0 {
            self.pop_undo();
        }
        moved
    }

    /// `OBJECT_OT_transform_randomize`: a random offset of the location, rotation and scale of every selected
    /// object. The seed counts up on every call, so repeating it gives another set. Returns how many objects changed.
    pub fn randomize_transform(&mut self, r: &Randomize) -> usize {
        let n = self.selected().count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        let seed = self.select_seed;
        self.select_seed = self.select_seed.wrapping_add(1);
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            for k in 0..3 {
                o.loc[k] += r.loc[k] * rand_signed(o.id, seed, k as u32);
                o.rot[k] += r.rot[k] * rand_signed(o.id, seed, 3 + k as u32);
            }
            let even = rand_signed(o.id, seed, 6);
            for k in 0..3 {
                let rnd = if r.scale_even { even } else { rand_signed(o.id, seed, 7 + k as u32) };
                o.scale[k] *= (1.0 + r.scale[k] * rnd).max(0.0);
            }
            o.eval = None;
        }
        n
    }

    /// `OBJECT_OT_select_grouped` by relationship. Returns how many objects were newly selected.
    pub fn select_grouped(&mut self, kind: GroupKind) -> usize {
        let Some(ai) = self.active.and_then(|id| self.index_of(id)) else { return 0 };
        let active_id = self.objects[ai].id;
        let parent = self.objects[ai].parent;
        let mut picked: Vec<usize> = Vec::new();
        for j in 0..self.objects.len() {
            let o = &self.objects[j];
            let hit = match kind {
                GroupKind::Children => self.is_descendant(j, active_id),
                GroupKind::ChildrenImmediate => o.parent == Some(active_id),
                GroupKind::Parent => parent == Some(o.id),
                GroupKind::Siblings => j != ai && parent.is_some() && o.parent == parent,
            };
            if hit && !o.hidden && !o.selected {
                picked.push(j);
            }
        }
        for &j in &picked {
            self.objects[j].selected = true;
        }
        picked.len()
    }

    /// `OBJECT_OT_origin_set` (Origin to Center of Mass, Volume): moves the origin of every selected mesh to the
    /// volume centroid, keeping the geometry in place.
    pub fn origin_to_center_of_mass(&mut self) -> usize {
        let n = self.selected().filter(|o| o.mesh().is_some()).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            let ObjectData::Mesh(mesh) = &o.data else { continue };
            let Some(c) = volume_centroid(mesh).or_else(|| mesh.median_center()) else { continue };
            let moved = mesh.mapped(|v| [v[0] - c[0], v[1] - c[1], v[2] - c[2]], false);
            let w = mat3_mul_v3(&euler_to_mat3(o.rot), [c[0] * o.scale[0], c[1] * o.scale[1], c[2] * o.scale[2]]);
            o.data = ObjectData::Mesh(moved);
            o.loc = [o.loc[0] + w[0], o.loc[1] + w[1], o.loc[2] + w[2]];
            o.eval = None;
        }
        n
    }

    /// `VIEW3D_OT_snap_selected_to_active`: moves the other selected objects so their origins land on the active
    /// object's origin. An object that moves with a selected parent is left out, and so is an ancestor of the
    /// active object. Returns how many objects moved.
    pub fn selection_to_active(&mut self) -> usize {
        let Some(ai) = self.active.and_then(|id| self.index_of(id)) else { return 0 };
        self.refresh_parents();
        let target = self.objects[ai].world_matrix()[3];
        let moving: Vec<usize> = (0..self.objects.len())
            .filter(|&i| {
                let o = &self.objects[i];
                i != ai && o.selected && !o.hidden && !self.has_selected_ancestor(i) && !self.is_descendant(ai, o.id)
            })
            .filter(|&i| {
                let w = self.objects[i].world_matrix()[3];
                (0..3).any(|k| (w[k] - target[k]).abs() > 1e-7)
            })
            .collect();
        if moving.is_empty() {
            return 0;
        }
        self.push_undo();
        for &i in &moving {
            let w = self.objects[i].world_matrix()[3];
            self.translate_world(i, [target[0] - w[0], target[1] - w[1], target[2] - w[2]]);
        }
        self.refresh_parents();
        moving.len()
    }

    /// `OBJECT_OT_origin_set` (Origin to Geometry, Bounds Center): the center of the mesh bounding box.
    pub fn origin_to_bounds_center(&mut self) -> usize {
        let n = self.selected().filter(|o| o.mesh().is_some()).count();
        if n == 0 {
            return 0;
        }
        self.push_undo();
        for o in self.objects.iter_mut().filter(|o| o.selected && !o.hidden) {
            let ObjectData::Mesh(mesh) = &o.data else { continue };
            let Some((lo, hi)) = mesh.bounds() else { continue };
            let c = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])];
            let moved = mesh.mapped(|v| [v[0] - c[0], v[1] - c[1], v[2] - c[2]], false);
            let w = mat3_mul_v3(&euler_to_mat3(o.rot), [c[0] * o.scale[0], c[1] * o.scale[1], c[2] * o.scale[2]]);
            o.data = ObjectData::Mesh(moved);
            o.loc = [o.loc[0] + w[0], o.loc[1] + w[1], o.loc[2] + w[2]];
            o.eval = None;
        }
        n
    }

    /// `OBJECT_OT_parent_clear` (Clear Parent Inverse): resets the stored inverse parent matrix of the selected
    /// children, so they follow their parent's transform directly (and usually move). Returns how many changed.
    pub fn parent_clear_inverse(&mut self) -> usize {
        let targets: Vec<usize> = (0..self.objects.len())
            .filter(|&i| {
                let o = &self.objects[i];
                o.selected && !o.hidden && o.parent.is_some() && o.parent_inv != IDENTITY4
            })
            .collect();
        if targets.is_empty() {
            return 0;
        }
        self.push_undo();
        for &i in &targets {
            self.objects[i].parent_inv = IDENTITY4;
            self.objects[i].eval = None;
        }
        self.refresh_parents();
        targets.len()
    }

    /// `OBJECT_OT_select_linked` (Object Data): selects every visible object with a mesh equal to the active
    /// object's. Returns how many objects were newly selected.
    pub fn select_linked_data(&mut self) -> usize {
        let Some(data) = self.active_object().and_then(|o| o.mesh().cloned()) else { return 0 };
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| !o.hidden && !o.selected) {
            if o.mesh() == Some(&data) {
                o.selected = true;
                n += 1;
            }
        }
        n
    }

    /// `OBJECT_OT_select_linked` (Modifiers): selects every visible mesh object whose modifier stack and
    /// Subdivision Surface level equal the active object's (which must have some). Returns how many were newly selected.
    pub fn select_linked_modifiers(&mut self) -> usize {
        let Some((stack, level)) = self.active_object().filter(|o| o.mesh().is_some()).map(|o| (o.modifiers.clone(), o.subsurf)) else {
            return 0;
        };
        if stack.is_empty() && level == 0 {
            return 0;
        }
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| !o.hidden && !o.selected && o.mesh().is_some()) {
            if o.modifiers == stack && o.subsurf == level {
                o.selected = true;
                n += 1;
            }
        }
        n
    }

    /// `OBJECT_OT_select_mirror`: selects the objects whose names are the left / right flip of a selected one's.
    /// With `extend` the current selection stays. Returns how many objects were newly selected.
    pub fn select_mirror(&mut self, extend: bool) -> usize {
        let wanted: Vec<String> = self.objects.iter().filter(|o| o.selected && !o.hidden).filter_map(|o| flip_side_name(&o.name)).collect();
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| !o.hidden) {
            let hit = wanted.contains(&o.name);
            if hit && !o.selected {
                n += 1;
            }
            o.selected = hit || (extend && o.selected);
        }
        n
    }

    /// `object_select_more_less` (`OBJECT_OT_select_more` / `OBJECT_OT_select_less`): every selectable object
    /// whose selected state differs from its parent's, and the parent of such a pair, is selected (`select`) or
    /// deselected. Returns whether any object changed.
    pub fn select_more_less(&mut self, select: bool) -> bool {
        let sel: Vec<(u32, bool)> = self.objects.iter().map(|o| (o.id, o.selected)).collect();
        let state = |id: u32| sel.iter().find(|(i, _)| *i == id).map(|(_, s)| *s);
        let mut doit = vec![false; self.objects.len()];
        for (j, o) in self.objects.iter().enumerate() {
            if o.hidden || o.hide_select {
                continue;
            }
            let Some(pid) = o.parent else { continue };
            let Some(ps) = state(pid) else { continue };
            if o.selected != ps {
                doit[j] = true;
                if let Some(pj) = self.index_of(pid) {
                    doit[pj] = true;
                }
            }
        }
        let mut changed = false;
        for (j, o) in self.objects.iter_mut().enumerate() {
            if doit[j] && !o.hidden && !o.hide_select && o.selected != select {
                o.selected = select;
                changed = true;
            }
        }
        changed
    }

    /// The world position of the active object's origin (`VIEW3D_OT_snap_cursor_to_active`).
    pub fn active_origin(&self) -> Option<V3> {
        let w = self.active_object()?.world_matrix()[3];
        Some([w[0], w[1], w[2]])
    }

    /// The numbers of the statistics overlay for the visible objects (the evaluated meshes, so modifiers count).
    pub fn statistics(&self) -> Stats {
        let mut s = Stats::default();
        for o in self.objects.iter().filter(|o| !o.hidden) {
            s.objects += 1;
            s.selected += usize::from(o.selected);
            if let Some(m) = o.evaluated() {
                s.verts += m.verts.len();
                s.faces += m.faces.len();
                s.tris += m.faces.iter().map(|f| f.len().saturating_sub(2)).sum::<usize>();
            }
        }
        s
    }
}

impl Stats {
    /// The overlay lines, like Blender's `Objects 1/3`, `Vertices 8`, `Edges`, `Faces 6`, `Triangles 12`.
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("Objects  {}/{}", self.selected, self.objects),
            format!("Vertices  {}", self.verts),
            format!("Faces  {}", self.faces),
            format!("Triangles  {}", self.tris),
        ]
    }
}

/// The `location` / `rotation` / `scale` options of `OBJECT_OT_transform_apply`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplyMask {
    pub location: bool,
    pub rotation: bool,
    pub scale: bool,
}

impl ApplyMask {
    pub const ALL: ApplyMask = ApplyMask { location: true, rotation: true, scale: true };
    pub const LOCATION: ApplyMask = ApplyMask { location: true, rotation: false, scale: false };
    pub const ROTATION: ApplyMask = ApplyMask { location: false, rotation: true, scale: false };
    pub const SCALE: ApplyMask = ApplyMask { location: false, rotation: false, scale: true };
}

impl Scene {
    /// `OBJECT_OT_transform_apply`: bakes the chosen parts of the object transform into the mesh and resets them.
    /// What stays on the object is applied after the baked part, so the world position of the vertices is kept
    /// when the remaining rotation and scale commute with it (always for Location, Scale and All; Rotation of a
    /// non-uniformly scaled object shears, as in Blender). The direct children of an applied object keep their
    /// world transform, like Blender, which adjusts their parent inverse. Returns how many meshes changed.
    pub fn apply_transform(&mut self, mask: ApplyMask) -> usize {
        let changes = |o: &Object| {
            (mask.location && o.loc != [0.0; 3]) || (mask.rotation && o.rot != [0.0; 3]) || (mask.scale && o.scale != [1.0; 3])
        };
        let targets: Vec<usize> = (0..self.objects.len())
            .filter(|&i| {
                let o = &self.objects[i];
                o.selected && !o.hidden && o.mesh().is_some() && changes(o)
            })
            .collect();
        if targets.is_empty() {
            return 0;
        }
        self.push_undo();

        /* The world matrices of the children, to give them back afterwards. */
        let target_ids: Vec<u32> = targets.iter().map(|&i| self.objects[i].id).collect();
        let mut children: Vec<(usize, Mat4, u32)> = (0..self.objects.len())
            .filter(|&i| self.objects[i].parent.is_some_and(|p| target_ids.contains(&p)))
            .map(|i| (i, self.full_world(i, 0), self.depth_of(i)))
            .collect();
        children.sort_by_key(|c| c.2);

        const ID3: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        for &i in &targets {
            let o = &mut self.objects[i];
            let ObjectData::Mesh(mesh) = &o.data else { continue };
            let (loc, rot, sc) = (o.loc, euler_to_mat3(o.rot), o.scale);
            let safe = |v: f32| if v.abs() < 1e-8 { 1.0 } else { v };
            /* t = (remaining rotation * remaining scale)^-1 * loc, so that the object origin ends up unmoved. */
            let mut t = [0.0; 3];
            if mask.location {
                t = if mask.rotation { loc } else { mat3_mul_v3(&mat3_transpose(&rot), loc) };
                if !mask.scale {
                    t = [t[0] / safe(sc[0]), t[1] / safe(sc[1]), t[2] / safe(sc[2])];
                }
            }
            let rot_m = if mask.rotation { rot } else { ID3 };
            let sc_m = if mask.scale { sc } else { [1.0; 3] };
            let flip = mask.scale && sc[0] * sc[1] * sc[2] < 0.0;
            let baked = mesh.mapped(
                |v| {
                    let r = mat3_mul_v3(&rot_m, [v[0] * sc_m[0], v[1] * sc_m[1], v[2] * sc_m[2]]);
                    [r[0] + t[0], r[1] + t[1], r[2] + t[2]]
                },
                flip,
            );
            o.data = ObjectData::Mesh(baked);
            if mask.location {
                o.loc = [0.0; 3];
            }
            if mask.rotation {
                o.rot = [0.0; 3];
            }
            if mask.scale {
                o.scale = [1.0; 3];
            }
            o.eval = None;
        }

        /* Parents first, so that a grandchild sees its parent already back in place. */
        for (i, world, _) in children {
            let Some(p) = self.objects[i].parent.and_then(|p| self.index_of(p)) else { continue };
            let inv_parent = crate::view3d::invert_m4_m4(&self.full_world(p, 0));
            let inv_local = crate::view3d::invert_m4_m4(&self.objects[i].local_matrix());
            self.objects[i].parent_inv = mat4_mul(&inv_parent, &mat4_mul(&world, &inv_local));
        }
        self.refresh_parents();
        targets.len()
    }

    /// `VIEW3D_OT_snap_selected_to_grid`: moves the origin of every selected object to the nearest grid point
    /// (`grid` metres apart). Objects that move along with a selected ancestor are left to it. Returns how many
    /// objects moved.
    pub fn snap_selected_to_grid(&mut self, grid: f32) -> usize {
        if grid.is_nan() || grid <= 0.0 {
            return 0;
        }
        let mut moves: Vec<(usize, V3)> = Vec::new();
        for i in 0..self.objects.len() {
            let o = &self.objects[i];
            if !o.selected || o.hidden || self.has_selected_ancestor(i) {
                continue;
            }
            let at = mat4_mul_point(&self.full_world(i, 0), [0.0; 3]);
            let snapped = snap_to_grid(at, grid);
            let d: V3 = std::array::from_fn(|k| snapped[k] - at[k]);
            if d.iter().any(|x| x.abs() > 1e-6) {
                moves.push((i, d));
            }
        }
        if moves.is_empty() {
            return 0;
        }
        self.push_undo();
        for &(i, d) in &moves {
            let o = &mut self.objects[i];
            let l = mat3_mul_v3(&o.parent_lin_inv(), d);
            o.loc = [o.loc[0] + l[0], o.loc[1] + l[1], o.loc[2] + l[2]];
        }
        self.refresh_parents();
        moves.len()
    }

    pub fn in_local_view(&self) -> bool {
        self.local_view.is_some()
    }

    /// `VIEW3D_OT_localview`: enters Local View, which hides every visible object that is not selected, or leaves
    /// it and shows what it hid. The objects the user had hidden stay hidden, and so do the ones hidden while in
    /// Local View. Returns whether the mode changed (entering needs a selected object).
    pub fn toggle_local_view(&mut self) -> bool {
        match self.local_view.take() {
            Some(hidden_by_view) => {
                for o in self.objects.iter_mut().filter(|o| hidden_by_view.contains(&o.id)) {
                    o.hidden = false;
                }
                true
            }
            None => {
                if !self.objects.iter().any(|o| o.selected && !o.hidden) {
                    return false;
                }
                let ids: Vec<u32> = self.objects.iter().filter(|o| !o.hidden && !o.selected).map(|o| o.id).collect();
                for o in self.objects.iter_mut().filter(|o| ids.contains(&o.id)) {
                    o.hidden = true;
                }
                self.local_view = Some(ids);
                true
            }
        }
    }
}

/// Rounds every coordinate to the nearest multiple of `grid` (`snap_curs_to_grid`); a non-positive or NaN grid
/// leaves the point alone.
pub fn snap_to_grid(p: V3, grid: f32) -> V3 {
    if grid.is_nan() || grid <= 0.0 {
        return p;
    }
    p.map(|c| (c / grid).round() * grid)
}

impl Scene {
    /// `OBJECT_OT_parent_no_inverse_set`: the active object becomes the parent of the other selected objects with an
    /// identity parent inverse. Without `keep_transform` they end up at the parent's origin (their location is
    /// cleared, rotation and scale stay as local values); with it their local transform is recomputed from the
    /// parent so that they stay where they are in the world. Parenting to a descendant is refused. Returns how many
    /// objects were parented.
    pub fn parent_set_no_inverse(&mut self, keep_transform: bool) -> usize {
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
        /* All world matrices first: reparenting one selected object moves the selected objects below it. */
        let worlds: Vec<Mat4> = children.iter().map(|&i| self.full_world(i, 0)).collect();
        let inv_parent = crate::view3d::invert_m4_m4(&self.full_world(ai, 0));
        for (&i, world) in children.iter().zip(&worlds) {
            let o = &mut self.objects[i];
            o.parent = Some(active);
            o.parent_inv = IDENTITY4;
            if keep_transform {
                let (loc, rot, scale) = decompose(&mat4_mul(&inv_parent, world));
                o.loc = loc;
                o.rot = rot;
                o.scale = scale;
            } else {
                o.loc = [0.0; 3];
            }
            o.eval = None;
        }
        self.refresh_parents();
        children.len()
    }

    /// `OBJECT_OT_make_links_data` (Link Object Data): the other selected meshes take the geometry of the active
    /// mesh, and its shading flag, which this port keeps on the object. Meshes are not shared between objects here,
    /// so they get copies (the original keeps editing independent). Objects that already match are left alone.
    /// Returns how many objects changed.
    pub fn link_object_data(&mut self) -> usize {
        let Some(src) = self.active_object().filter(|o| o.mesh().is_some()) else { return 0 };
        let (id, mesh, smooth) = (src.id, src.mesh().unwrap().clone(), src.smooth);
        let wants = |o: &Object| o.selected && !o.hidden && o.id != id && o.mesh().is_some_and(|m| *m != mesh || o.smooth != smooth);
        if !self.objects.iter().any(&wants) {
            return 0;
        }
        self.push_undo();
        let mut n = 0;
        for o in self.objects.iter_mut().filter(|o| wants(o)) {
            o.data = ObjectData::Mesh(mesh.clone());
            o.smooth = smooth;
            o.eval = None;
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn cubes() -> Scene {
        /* The startup cube at the origin, plus two more: Cube.001 at x = 5 and Cube.002 at x = -4, scaled by 2. */
        let mut s = Scene::startup();
        let a = s.add_primitive(Primitive::Cube, [5.0, 1.0, 0.0]);
        let b = s.add_primitive(Primitive::Cube, [-4.0, 2.0, 0.0]);
        let ib = s.index_of(b).unwrap();
        s.objects[ib].scale = [2.0, 2.0, 2.0];
        let _ = a;
        for o in &mut s.objects {
            o.selected = o.mesh().is_some();
        }
        s.active = s.objects.iter().find(|o| o.name == "Cube").map(|o| o.id);
        s
    }

    fn center_x(s: &Scene, name: &str) -> f32 {
        let o = s.objects.iter().find(|o| o.name == name).unwrap();
        let (lo, hi) = o.world_bounds().unwrap();
        0.5 * (lo[0] + hi[0])
    }

    #[test]
    fn align_centers_to_the_active_object() {
        let mut s = cubes();
        let moved = s.align_objects(0, AlignMode::Centers, AlignTo::Active);
        assert_eq!(moved, 2);
        for n in ["Cube", "Cube.001", "Cube.002"] {
            assert!(approx(center_x(&s, n), 0.0), "{n}");
        }
        /* The other axes are untouched. */
        assert!(approx(s.objects.iter().find(|o| o.name == "Cube.001").unwrap().loc[1], 1.0));
    }

    #[test]
    fn align_negative_and_positive_sides_to_the_cursor() {
        let mut s = cubes();
        s.align_objects(0, AlignMode::NegativeSides, AlignTo::Cursor([10.0, 0.0, 0.0]));
        for o in s.objects.iter().filter(|o| o.mesh().is_some()) {
            assert!(approx(o.world_bounds().unwrap().0[0], 10.0), "{}", o.name);
        }
        let mut s = cubes();
        s.align_objects(0, AlignMode::PositiveSides, AlignTo::SceneOrigin);
        for o in s.objects.iter().filter(|o| o.mesh().is_some()) {
            assert!(approx(o.world_bounds().unwrap().1[0], 0.0), "{}", o.name);
        }
    }

    #[test]
    fn align_to_the_selection_uses_the_combined_bounds_and_is_one_undo_step() {
        let mut s = cubes();
        /* The bounds span x = -6 .. 6 (Cube.002 is scaled to 4 wide at -4, Cube.001 reaches 6). */
        s.align_objects(0, AlignMode::PositiveSides, AlignTo::Selection);
        for o in s.objects.iter().filter(|o| o.mesh().is_some()) {
            assert!(approx(o.world_bounds().unwrap().1[0], 6.0), "{}", o.name);
        }
        assert!(s.can_undo());
        s.undo();
        assert!(approx(center_x(&s, "Cube.001"), 5.0));
    }

    #[test]
    fn align_does_nothing_when_already_aligned_and_pushes_no_undo() {
        let mut s = Scene::startup();
        assert_eq!(s.align_objects(2, AlignMode::Centers, AlignTo::Active), 0);
        assert!(!s.can_undo());
        assert_eq!(s.align_objects(5, AlignMode::Centers, AlignTo::Active), 0);
    }

    #[test]
    fn align_moves_a_child_of_a_selected_parent_only_when_needed() {
        let mut s = cubes();
        let parent = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        let child = s.objects.iter().position(|o| o.name == "Cube.002").unwrap();
        s.objects[child].parent = Some(parent);
        s.refresh_parents();
        s.align_objects(0, AlignMode::Centers, AlignTo::Active);
        s.refresh_parents();
        for n in ["Cube", "Cube.001", "Cube.002"] {
            assert!(approx(center_x(&s, n), 0.0), "{n}");
        }
    }

    #[test]
    fn randomize_changes_the_selection_and_is_deterministic_per_seed() {
        let mut a = Scene::startup();
        let mut b = Scene::startup();
        assert_eq!(a.randomize_transform(&Randomize::default()), 1);
        b.randomize_transform(&Randomize::default());
        let (oa, ob) = (a.active_object().unwrap().clone(), b.active_object().unwrap().clone());
        assert_eq!((oa.loc, oa.rot, oa.scale), (ob.loc, ob.rot, ob.scale));
        assert!(oa.loc != [0.0; 3]);
        assert!(oa.loc.iter().all(|v| v.abs() <= 1.0));
        assert!(oa.scale.iter().all(|v| (0.8..=1.2).contains(v)));
        /* The next call uses the next seed. */
        let before = oa.loc;
        a.randomize_transform(&Randomize::default());
        assert!(a.active_object().unwrap().loc != before);
        /* Not selected objects stay put. */
        assert_eq!(a.objects.iter().find(|o| o.name == "Camera").unwrap().loc, Scene::startup().objects[0].loc);
    }

    #[test]
    fn randomize_even_scale_keeps_the_proportions() {
        let mut s = Scene::startup();
        let r = Randomize { loc: [0.0; 3], rot: [0.0; 3], scale: [0.5; 3], scale_even: true };
        s.randomize_transform(&r);
        let sc = s.active_object().unwrap().scale;
        assert!(approx(sc[0], sc[1]) && approx(sc[1], sc[2]));
        assert!(approx(s.active_object().unwrap().loc[0], 0.0));
    }

    #[test]
    fn randomize_undoes_in_one_step() {
        let mut s = Scene::startup();
        s.randomize_transform(&Randomize::default());
        s.undo();
        assert_eq!(s.active_object().unwrap().loc, [0.0; 3]);
        assert_eq!(s.active_object().unwrap().scale, [1.0; 3]);
    }

    fn family() -> Scene {
        /* Cube <- A <- B, and Cube <- C. */
        let mut s = Scene::startup();
        let cube = s.active.unwrap();
        let a = s.add_primitive(Primitive::Cube, [1.0, 0.0, 0.0]);
        let b = s.add_primitive(Primitive::Cube, [2.0, 0.0, 0.0]);
        let c = s.add_primitive(Primitive::Cube, [3.0, 0.0, 0.0]);
        for (child, parent) in [(a, cube), (b, a), (c, cube)] {
            let i = s.index_of(child).unwrap();
            s.objects[i].parent = Some(parent);
        }
        s.refresh_parents();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.active = Some(cube);
        s
    }

    fn names(s: &Scene) -> Vec<String> {
        s.selected().map(|o| o.name.clone()).collect()
    }

    #[test]
    fn select_grouped_children() {
        let mut s = family();
        assert_eq!(s.select_grouped(GroupKind::Children), 3);
        assert_eq!(names(&s), ["Cube.001", "Cube.002", "Cube.003"]);
        let mut s = family();
        assert_eq!(s.select_grouped(GroupKind::ChildrenImmediate), 2);
        assert_eq!(names(&s), ["Cube.001", "Cube.003"]);
    }

    #[test]
    fn select_grouped_parent_and_siblings() {
        let mut s = family();
        let a = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        s.active = Some(a);
        assert_eq!(s.select_grouped(GroupKind::Siblings), 1);
        assert_eq!(names(&s), ["Cube.003"]);
        assert_eq!(s.select_grouped(GroupKind::Parent), 1);
        assert!(names(&s).contains(&"Cube".to_string()));
        /* Nothing left to add; the root has no parent or siblings. */
        assert_eq!(s.select_grouped(GroupKind::Parent), 0);
        let root = s.objects.iter().find(|o| o.name == "Cube").unwrap().id;
        s.active = Some(root);
        assert_eq!(s.select_grouped(GroupKind::Siblings), 0);
    }

    #[test]
    fn select_grouped_skips_hidden_objects() {
        let mut s = family();
        let i = s.objects.iter().position(|o| o.name == "Cube.003").unwrap();
        s.objects[i].hidden = true;
        assert_eq!(s.select_grouped(GroupKind::ChildrenImmediate), 1);
    }

    #[test]
    fn volume_centroid_of_a_cube_and_an_offset_box() {
        let c = volume_centroid(&Mesh::cube(2.0)).unwrap();
        assert!(c.iter().all(|v| v.abs() < 1e-5));
        let shifted = Mesh::cube(2.0).mapped(|v| [v[0] + 3.0, v[1] - 1.0, v[2] + 0.5], false);
        let c = volume_centroid(&shifted).unwrap();
        assert!(approx(c[0], 3.0) && approx(c[1], -1.0) && approx(c[2], 0.5));
        /* A flat quad has no volume. */
        let quad = Mesh { verts: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]], faces: vec![vec![0, 1, 2, 3]] };
        assert!(volume_centroid(&quad).is_none());
    }

    #[test]
    fn origin_to_center_of_mass_moves_the_origin_not_the_geometry() {
        let mut s = Scene::startup();
        let i = s.objects.iter().position(|o| o.name == "Cube").unwrap();
        /* A cube whose mesh sits off its origin: the centroid is (2, 0, 0). */
        s.objects[i].data = ObjectData::Mesh(Mesh::cube(2.0).mapped(|v| [v[0] + 2.0, v[1], v[2]], false));
        let before = s.objects[i].world_bounds().unwrap();
        assert_eq!(s.origin_to_center_of_mass(), 1);
        let o = &s.objects[i];
        assert!(approx(o.loc[0], 2.0));
        let after = o.world_bounds().unwrap();
        assert!((0..3).all(|k| approx(before.0[k], after.0[k]) && approx(before.1[k], after.1[k])));
        s.undo();
        assert!(approx(s.objects[i].loc[0], 0.0));
    }

    #[test]
    fn active_origin_is_the_world_position_of_the_active_object() {
        let mut s = cubes();
        let id = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        s.active = Some(id);
        assert_eq!(s.active_origin(), Some([5.0, 1.0, 0.0]));
        s.active = None;
        assert_eq!(s.active_origin(), None);
    }

    #[test]
    fn flipped_names() {
        assert_eq!(flip_side_name("Arm.L").as_deref(), Some("Arm.R"));
        assert_eq!(flip_side_name("Arm.R.001").as_deref(), Some("Arm.L.001"));
        assert_eq!(flip_side_name("leg_l").as_deref(), Some("leg_r"));
        assert_eq!(flip_side_name("Left_Hand").as_deref(), Some("Right_Hand"));
        assert_eq!(flip_side_name("hand-RIGHT").as_deref(), Some("hand-LEFT"));
        assert_eq!(flip_side_name("Cube"), None);
        assert_eq!(flip_side_name("Cube.001"), None);
        assert_eq!(flip_side_name("Lamp"), None);
    }

    fn named(names: &[&str]) -> Scene {
        let mut s = Scene::startup();
        s.objects.clear();
        for n in names {
            s.add_primitive(Primitive::Cube, [0.0; 3]);
            s.objects.last_mut().unwrap().name = n.to_string();
        }
        for o in &mut s.objects {
            o.selected = false;
        }
        s
    }

    #[test]
    fn select_mirror_replaces_or_extends() {
        let mut s = named(&["Arm.L", "Arm.R", "Leg.L", "Body"]);
        s.objects[0].selected = true;
        assert_eq!(s.select_mirror(false), 1);
        assert_eq!(s.selected().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Arm.R"]);
        s.objects[3].selected = true;
        assert_eq!(s.select_mirror(true), 1);
        assert_eq!(s.selected().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["Arm.L", "Arm.R", "Body"]);
        let mut s = named(&["Body", "Head"]);
        s.objects[0].selected = true;
        assert_eq!(s.select_mirror(false), 0);
    }

    #[test]
    fn selection_to_active_moves_the_others_onto_it() {
        let mut s = cubes();
        let a = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        s.active = Some(a);
        assert_eq!(s.selection_to_active(), 2);
        for o in s.objects.iter().filter(|o| o.mesh().is_some()) {
            assert_eq!(o.loc, [5.0, 1.0, 0.0], "{}", o.name);
        }
        assert_eq!(s.selection_to_active(), 0);
        s.undo();
        assert_eq!(s.objects.iter().find(|o| o.name == "Cube").unwrap().loc, [0.0; 3]);
    }

    #[test]
    fn selection_to_active_leaves_out_children_of_selected_parents() {
        let mut s = cubes();
        let parent = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        let child = s.objects.iter().position(|o| o.name == "Cube.002").unwrap();
        s.objects[child].parent = Some(parent);
        s.refresh_parents();
        /* Cube is active at the origin; Cube.001 (the parent) moves there. Cube.002 keeps its local (-4, 2, 0)
         * under it (no inverse was stored), so it goes along instead of being snapped by itself. */
        assert_eq!(s.selection_to_active(), 1);
        s.refresh_parents();
        assert_eq!(s.objects[child].loc, [-4.0, 2.0, 0.0]);
        let w = s.objects[child].world_matrix()[3];
        assert!((w[0] + 4.0).abs() < 1e-4 && (w[1] - 2.0).abs() < 1e-4, "{w:?}");
    }

    #[test]
    fn origin_to_bounds_center() {
        let mut s = Scene::startup();
        let i = s.objects.iter().position(|o| o.name == "Cube").unwrap();
        /* A lopsided mesh: the vertex mean is not the box center. */
        s.objects[i].data = ObjectData::Mesh(Mesh {
            verts: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [4.0, 2.0, 2.0]],
            faces: vec![vec![0, 1, 3]],
        });
        let before = s.objects[i].world_bounds().unwrap();
        assert_eq!(s.origin_to_bounds_center(), 1);
        assert_eq!(s.objects[i].loc, [2.0, 1.0, 1.0]);
        let after = s.objects[i].world_bounds().unwrap();
        assert!((0..3).all(|k| approx(before.0[k], after.0[k]) && approx(before.1[k], after.1[k])));
    }

    #[test]
    fn clear_parent_inverse_resets_it_and_the_child_follows_the_parent() {
        let mut s = cubes();
        let parent = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        for o in &mut s.objects {
            o.selected = false;
        }
        let pi = s.index_of(parent).unwrap();
        s.objects[pi].selected = true;
        let ci = s.objects.iter().position(|o| o.name == "Cube.002").unwrap();
        s.objects[ci].selected = true;
        s.active = Some(parent);
        assert_eq!(s.parent_set(), 1);
        /* Parenting stored the inverse of the parent's matrix, so there is something to clear. */
        assert_eq!(s.parent_clear_inverse(), 1);
        s.refresh_parents();
        /* Local (-4, 2, 0) now sits inside the parent at (5, 1, 0). */
        let w = s.objects[ci].world_matrix()[3];
        assert!((w[0] - 1.0).abs() < 1e-4 && (w[1] - 3.0).abs() < 1e-4, "{w:?}");
        assert_eq!(s.parent_clear_inverse(), 0);
    }

    #[test]
    fn solidify_even_toggles_only_with_a_solidify() {
        let mut s = Scene::startup();
        assert_eq!(s.toggle_solidify_even(), 0, "no Solidify modifier yet");
        s.toggle_solidify();
        let even = |s: &Scene| matches!(s.active_object().unwrap().modifiers[0], Modifier::Solidify { even: true, .. });
        assert!(!even(&s));
        assert_eq!(s.toggle_solidify_even(), 1);
        assert!(even(&s));
        assert_eq!(s.toggle_solidify_even(), 1);
        assert!(!even(&s));
    }

    #[test]
    fn displace_direction_cycles_only_with_a_displace() {
        let mut s = Scene::startup();
        assert_eq!(s.cycle_displace_direction(), 0, "no Displace modifier yet");
        s.toggle_displace();
        let dir = |s: &Scene| match &s.active_object().unwrap().modifiers[0] {
            Modifier::Displace { dir, .. } => *dir,
            _ => unreachable!(),
        };
        use crate::mesh_deform::DisplaceDir::*;
        for want in [X, Y, Z, Normal] {
            assert_eq!(s.cycle_displace_direction(), 1);
            assert_eq!(dir(&s), want);
        }
    }

    #[test]
    fn array_fit_cycles_and_sizes_the_array() {
        let mut s = Scene::startup();
        assert_eq!(s.cycle_array_fit(), 0, "no Array modifier yet");
        s.set_array(2);
        let fit = |s: &Scene| match &s.active_object().unwrap().modifiers[0] {
            Modifier::Array { fit, .. } => *fit,
            _ => unreachable!(),
        };
        for want in [2.0, 4.0, 8.0, 0.0, 2.0] {
            assert_eq!(s.cycle_array_fit(), 1);
            assert_eq!(fit(&s), want);
        }
        /* The default cube is 2 m wide: 4 m fit two copies, 8 m four. */
        s.cycle_array_fit();
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 12);
        s.cycle_array_fit();
        s.update_eval();
        assert_eq!(s.active_object().unwrap().evaluated().unwrap().faces.len(), 24);
    }

    #[test]
    fn array_offset_cycles_through_the_presets_and_wraps() {
        let mut s = Scene::startup();
        assert_eq!(s.cycle_array_offset(), 0, "no Array modifier yet");
        assert!(!s.can_undo());
        s.set_array(3);
        let off = |s: &Scene| match &s.active_object().unwrap().modifiers[0] {
            Modifier::Array { relative, constant, .. } => (*relative, *constant),
            _ => unreachable!(),
        };
        assert_eq!(off(&s), ([1.0, 0.0, 0.0], [0.0; 3]));
        for i in 1..6 {
            assert_eq!(s.cycle_array_offset(), 1);
            assert_eq!(off(&s), (modifiers::ARRAY_PRESETS[i].1, modifiers::ARRAY_PRESETS[i].2));
        }
        s.cycle_array_offset();
        assert_eq!(off(&s), ([1.0, 0.0, 0.0], [0.0; 3]));
        s.undo();
        assert_eq!(off(&s).1, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn array_constant_offset_moves_the_copies_and_merge_toggles() {
        let mut s = Scene::startup();
        s.set_array(2);
        for _ in 0..3 {
            s.cycle_array_offset();
        }
        s.update_eval();
        /* +1 m along X: the copy overlaps the 2 m cube, bounds 3 m wide. */
        let (lo, hi) = s.active_object().unwrap().world_bounds().unwrap();
        assert!(approx(hi[0] - lo[0], 3.0));
        assert_eq!(s.toggle_array_merge(), 1);
        assert!(matches!(s.active_object().unwrap().modifiers[0], Modifier::Array { merge, .. } if merge > 0.0));
        assert_eq!(s.toggle_array_merge(), 1);
        assert!(matches!(s.active_object().unwrap().modifiers[0], Modifier::Array { merge, .. } if merge == 0.0));
    }

    fn deselect_all_but_active(s: &mut Scene) {
        let a = s.active;
        for o in &mut s.objects {
            o.selected = Some(o.id) == a;
        }
    }

    #[test]
    fn select_linked_data_picks_identical_meshes() {
        let mut s = cubes();
        deselect_all_but_active(&mut s);
        /* The three cubes share a mesh (scale and location do not matter); a changed mesh does not match. */
        let i = s.objects.iter().position(|o| o.name == "Cube.002").unwrap();
        s.objects[i].data = ObjectData::Mesh(Mesh::cube(3.0));
        assert_eq!(s.select_linked_data(), 1);
        let names: Vec<&str> = s.selected().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Cube", "Cube.001"]);
        assert_eq!(s.select_linked_data(), 0);
        s.active = s.objects.iter().find(|o| o.name == "Camera").map(|o| o.id);
        assert_eq!(s.select_linked_data(), 0, "a camera has no mesh");
    }

    #[test]
    fn select_linked_data_skips_hidden_objects() {
        let mut s = cubes();
        deselect_all_but_active(&mut s);
        let i = s.objects.iter().position(|o| o.name == "Cube.001").unwrap();
        s.objects[i].hidden = true;
        assert_eq!(s.select_linked_data(), 1);
    }

    #[test]
    fn select_linked_modifiers_compares_the_whole_stack() {
        let mut s = cubes();
        for o in s.objects.iter_mut().filter(|o| o.mesh().is_some()) {
            o.modifiers = vec![Modifier::mirror(0)];
        }
        let i = s.objects.iter().position(|o| o.name == "Cube.002").unwrap();
        s.objects[i].modifiers.push(Modifier::solidify());
        deselect_all_but_active(&mut s);
        assert_eq!(s.select_linked_modifiers(), 1);
        let j = s.objects.iter().position(|o| o.name == "Cube.001").unwrap();
        s.objects[j].subsurf = 2;
        deselect_all_but_active(&mut s);
        assert_eq!(s.select_linked_modifiers(), 0, "a different subdivision level does not match");
    }

    #[test]
    fn select_linked_modifiers_needs_modifiers_on_the_active_object() {
        let mut s = cubes();
        deselect_all_but_active(&mut s);
        assert_eq!(s.select_linked_modifiers(), 0);
    }

    #[test]
    fn bevel_segments_cycle_and_wrap() {
        let mut s = Scene::startup();
        assert_eq!(s.cycle_bevel_segments(), 0, "no Bevel modifier yet");
        s.toggle_bevel();
        let seg = |s: &Scene| match &s.active_object().unwrap().modifiers[0] {
            Modifier::Bevel { segments, .. } => *segments,
            _ => unreachable!(),
        };
        let mut seen = vec![seg(&s)];
        for _ in 0..6 {
            assert_eq!(s.cycle_bevel_segments(), 1);
            seen.push(seg(&s));
        }
        assert_eq!(seen, [1, 2, 3, 4, 6, 8, 1]);
        s.undo();
        assert_eq!(seg(&s), 8);
    }

    #[test]
    fn statistics_follow_the_bevel_segments() {
        let mut s = Scene::startup();
        s.toggle_bevel();
        s.update_eval();
        let flat = s.statistics();
        s.cycle_bevel_segments();
        s.cycle_bevel_segments();
        s.update_eval();
        let round = s.statistics();
        assert!(round.verts > flat.verts && round.faces > flat.faces);
    }

    #[test]
    fn statistics_count_the_visible_objects() {
        let mut s = Scene::startup();
        let st = s.statistics();
        assert_eq!(st, Stats { objects: 3, selected: 1, verts: 8, faces: 6, tris: 12 });
        s.objects[1].hidden = true;
        assert_eq!(s.statistics().verts, 0);
        assert_eq!(st.lines()[0], "Objects  1/3");
    }

    #[test]
    fn statistics_include_the_modifiers() {
        let mut s = Scene::startup();
        s.set_subsurf(1);
        s.update_eval();
        let st = s.statistics();
        assert!(st.verts > 8 && st.faces > 6);
    }

    #[test]
    fn select_more_grows_across_parent_links_and_less_shrinks() {
        let mut s = family();
        let a = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        let ai = s.index_of(a).unwrap();
        s.objects[ai].selected = true;
        /* Cube.001 selected: its parent (Cube) and child (Cube.002) join; the sibling Cube.003 does not. */
        assert!(s.select_more_less(true));
        assert_eq!(names(&s), ["Cube", "Cube.001", "Cube.002"]);
        /* Less: only Cube differs from a neighbour (its unselected child Cube.003), so it alone drops out. */
        assert!(s.select_more_less(false));
        assert_eq!(names(&s), ["Cube.001", "Cube.002"]);
    }

    #[test]
    fn select_more_less_report_no_change_and_skip_unselectable() {
        let mut s = family();
        assert!(!s.select_more_less(true), "nothing selected: no boundary");
        let a = s.objects.iter().find(|o| o.name == "Cube.001").unwrap().id;
        let ai = s.index_of(a).unwrap();
        s.objects[ai].selected = true;
        let b = s.objects.iter().position(|o| o.name == "Cube.002").unwrap();
        s.objects[b].hide_select = true;
        assert!(s.select_more_less(true));
        assert_eq!(names(&s), ["Cube", "Cube.001"]);
        for o in &mut s.objects {
            o.selected = true;
        }
        assert!(!s.select_more_less(true), "everything selected: nothing to add");
    }
}

#[cfg(test)]
mod apply_snap_local_tests {
    use super::*;
    use crate::mesh::Primitive;

    fn close(a: V3, b: V3) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
    }

    /// The world positions of the vertices of object `i`.
    fn world_verts(s: &Scene, i: usize) -> Vec<V3> {
        let o = &s.objects[i];
        let m = o.world_matrix();
        o.mesh().unwrap().verts.iter().map(|&v| mat4_mul_point(&m, v)).collect()
    }

    fn posed(scale: V3) -> (Scene, usize) {
        let mut s = Scene::startup();
        let i = s.index_of(s.active.unwrap()).unwrap();
        s.objects[i].loc = [1.0, 2.0, 3.0];
        s.objects[i].rot = [0.3, -0.5, 1.1];
        s.objects[i].scale = scale;
        (s, i)
    }

    #[test]
    fn each_apply_option_keeps_the_vertices_in_place_and_resets_only_its_part() {
        for (mask, scale) in [
            (ApplyMask::ALL, [2.0, 0.5, 3.0]),
            (ApplyMask::LOCATION, [2.0, 0.5, 3.0]),
            (ApplyMask::SCALE, [2.0, 0.5, 3.0]),
            /* Rotation only commutes with a uniform scale. */
            (ApplyMask::ROTATION, [2.0, 2.0, 2.0]),
            (ApplyMask { location: true, rotation: false, scale: true }, [2.0, 0.5, 3.0]),
            (ApplyMask { location: true, rotation: true, scale: false }, [1.0, 1.0, 1.0]),
        ] {
            let (mut s, i) = posed(scale);
            let before = world_verts(&s, i);
            let (loc, rot, sc) = (s.objects[i].loc, s.objects[i].rot, s.objects[i].scale);
            assert_eq!(s.apply_transform(mask), 1, "{mask:?}");
            let o = &s.objects[i];
            /* What was applied is neutral now, what was not applied is untouched. */
            assert_eq!(o.loc, if mask.location { [0.0; 3] } else { loc }, "{mask:?}");
            assert_eq!(o.rot, if mask.rotation { [0.0; 3] } else { rot }, "{mask:?}");
            assert_eq!(o.scale, if mask.scale { [1.0; 3] } else { sc }, "{mask:?}");
            for (a, b) in before.iter().zip(world_verts(&s, i)) {
                assert!(close(*a, b), "{mask:?}: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn applying_does_nothing_when_the_chosen_part_is_already_neutral() {
        let (mut s, i) = posed([1.0; 3]);
        s.objects[i].rot = [0.0; 3];
        assert_eq!(s.apply_transform(ApplyMask::ROTATION), 0);
        assert_eq!(s.apply_transform(ApplyMask::SCALE), 0);
        assert!(!s.can_undo(), "a no-op leaves no undo step");
        assert_eq!(s.apply_transform(ApplyMask::LOCATION), 1);
        assert!(s.can_undo());
        assert!(s.undo());
        assert_eq!(s.objects[i].loc, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn a_negative_scale_flips_the_faces_when_it_is_applied() {
        let (mut s, i) = posed([-1.0, 1.0, 1.0]);
        let normal_before = |s: &Scene| {
            let m = s.objects[i].mesh().unwrap();
            let w = world_verts(s, i);
            let f = &m.faces[0];
            let (a, b, c) = (w[f[0] as usize], w[f[1] as usize], w[f[2] as usize]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        };
        let n0 = normal_before(&s);
        s.apply_transform(ApplyMask::SCALE);
        let n1 = normal_before(&s);
        /* The mirrored mesh was inside out; applying the scale reverses the winding, so the same world vertices now
         * give the opposite (outward) face normal. */
        assert!(close(n1, [-n0[0], -n0[1], -n0[2]]), "{n0:?} -> {n1:?}");
        assert!(n0.iter().any(|x| x.abs() > 1e-3));
    }

    #[test]
    fn children_stay_where_they_are_when_the_parent_is_applied() {
        let mut s = Scene::startup();
        let pid = s.active.unwrap();
        let pi = s.index_of(pid).unwrap();
        s.objects[pi].loc = [1.0, -2.0, 0.5];
        s.objects[pi].rot = [0.0, 0.0, 0.8];
        s.objects[pi].scale = [2.0, 2.0, 2.0];
        let cid = s.add_primitive(Primitive::Cube, [5.0, 1.0, 0.0]);
        let gid = s.add_primitive(Primitive::Cube, [-3.0, 4.0, 2.0]);
        let (ci, gi) = (s.index_of(cid).unwrap(), s.index_of(gid).unwrap());
        s.objects[ci].parent = Some(pid);
        s.objects[gi].parent = Some(cid);
        s.refresh_parents();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[pi].selected = true;
        s.active = Some(pid);
        let (c0, g0) = (world_verts(&s, ci), world_verts(&s, gi));
        for mask in [ApplyMask::SCALE, ApplyMask::ROTATION, ApplyMask::LOCATION, ApplyMask::ALL] {
            s.objects[pi].loc = [1.0, -2.0, 0.5];
            s.objects[pi].rot = [0.0, 0.0, 0.8];
            s.objects[pi].scale = [2.0, 2.0, 2.0];
            s.refresh_parents();
            let (c_now, g_now) = (world_verts(&s, ci), world_verts(&s, gi));
            s.apply_transform(mask);
            for (a, b) in c_now.iter().zip(world_verts(&s, ci)) {
                assert!(close(*a, b), "{mask:?} moved the child");
            }
            for (a, b) in g_now.iter().zip(world_verts(&s, gi)) {
                assert!(close(*a, b), "{mask:?} moved the grandchild");
            }
        }
        let _ = (c0, g0);
    }

    #[test]
    fn snap_to_grid_rounds_world_origins_and_respects_parents() {
        let mut s = Scene::startup();
        let a = s.add_primitive(Primitive::Cube, [1.4, -2.6, 0.49]);
        let ai = s.index_of(a).unwrap();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[ai].selected = true;
        assert_eq!(s.snap_selected_to_grid(1.0), 1);
        assert_eq!(s.objects[ai].loc, [1.0, -3.0, 0.0]);
        assert_eq!(s.snap_selected_to_grid(1.0), 0, "already on the grid");

        /* A child of an unselected, rotated and scaled parent lands on the grid in world space. */
        let p = s.add_primitive(Primitive::Cube, [0.0; 3]);
        let c = s.add_primitive(Primitive::Cube, [0.3, 0.8, 0.0]);
        let (pi, ci) = (s.index_of(p).unwrap(), s.index_of(c).unwrap());
        s.objects[pi].rot = [0.0, 0.0, 0.7];
        s.objects[pi].scale = [2.0, 3.0, 1.0];
        s.objects[ci].parent = Some(p);
        s.refresh_parents();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[ci].selected = true;
        assert_eq!(s.snap_selected_to_grid(0.5), 1);
        let at = mat4_mul_point(&s.objects[ci].world_matrix(), [0.0; 3]);
        assert!(at.iter().all(|x| ((x / 0.5).round() * 0.5 - x).abs() < 1e-4), "{at:?}");

        assert_eq!(s.snap_selected_to_grid(0.0), 0);
        assert_eq!(s.snap_selected_to_grid(-1.0), 0);
        assert_eq!(s.snap_selected_to_grid(f32::NAN), 0);
    }

    #[test]
    fn snap_to_grid_leaves_children_of_selected_parents_to_the_parent() {
        let mut s = Scene::startup();
        let p = s.add_primitive(Primitive::Cube, [0.4, 0.0, 0.0]);
        let c = s.add_primitive(Primitive::Cube, [1.3, 0.0, 0.0]);
        let (pi, ci) = (s.index_of(p).unwrap(), s.index_of(c).unwrap());
        s.objects[ci].parent = Some(p);
        s.refresh_parents();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[pi].selected = true;
        s.objects[ci].selected = true;
        let rel_before = s.objects[ci].loc;
        assert_eq!(s.snap_selected_to_grid(1.0), 1);
        assert_eq!(s.objects[ci].loc, rel_before);
        assert_eq!(s.objects[pi].loc, [0.0, 0.0, 0.0]);
    }

    fn hidden_names(s: &Scene) -> Vec<String> {
        s.objects.iter().filter(|o| o.hidden).map(|o| o.name.clone()).collect()
    }

    #[test]
    fn local_view_isolates_the_selection_and_restores_exactly_what_it_hid() {
        let mut s = Scene::startup();
        let extra = s.add_primitive(Primitive::Cube, [3.0, 0.0, 0.0]);
        /* The user hid the light before. */
        let li = s.objects.iter().position(|o| o.name == "Light").unwrap();
        s.objects[li].hidden = true;
        let ei = s.index_of(extra).unwrap();
        for (k, o) in s.objects.iter_mut().enumerate() {
            o.selected = k == ei;
        }
        assert!(s.toggle_local_view());
        assert!(s.in_local_view());
        let mut hidden = hidden_names(&s);
        hidden.sort();
        assert_eq!(hidden, ["Camera", "Cube", "Light"]);
        assert!(s.toggle_local_view());
        assert!(!s.in_local_view());
        assert_eq!(hidden_names(&s), ["Light"], "the light stays hidden as the user left it");
    }

    #[test]
    fn local_view_needs_a_selection_and_keeps_objects_hidden_inside_it_hidden() {
        let mut s = Scene::startup();
        let b = s.add_primitive(Primitive::Cube, [3.0, 0.0, 0.0]);
        for o in &mut s.objects {
            o.selected = false;
        }
        assert!(!s.toggle_local_view(), "nothing selected, nothing to isolate");
        assert!(!s.in_local_view());
        let bi = s.index_of(b).unwrap();
        let ci = s.objects.iter().position(|o| o.name == "Cube").unwrap();
        s.objects[bi].selected = true;
        s.objects[ci].selected = true;
        s.toggle_local_view();
        /* Hide one of the two inside, then try to reveal everything: the camera and light stay away. */
        s.objects[bi].selected = true;
        s.objects[ci].selected = false;
        s.hide_selected();
        assert_eq!(s.reveal_all(), 1, "only what the user hid in local view comes back");
        assert!(s.objects.iter().find(|o| o.name == "Camera").unwrap().hidden);
        assert!(s.objects.iter().find(|o| o.name == "Light").unwrap().hidden);
        s.objects[bi].selected = true;
        s.hide_selected();
        s.toggle_local_view();
        assert_eq!(hidden_names(&s), [s.objects[bi].name.clone()], "an object hidden inside stays hidden");
    }

    #[test]
    fn local_view_follows_undo() {
        let mut s = Scene::startup();
        let b = s.add_primitive(Primitive::Cube, [3.0, 0.0, 0.0]);
        let bi = s.index_of(b).unwrap();
        s.toggle_local_view();
        assert!(s.in_local_view());
        s.push_undo();
        s.objects[bi].loc[0] = 9.0;
        assert!(s.undo());
        assert!(s.in_local_view(), "still in local view after undoing a step taken inside it");
        assert_eq!(s.objects[bi].loc[0], 3.0);
    }
}

#[cfg(test)]
mod link_and_parent_tests {
    use super::*;
    use crate::mesh::Primitive;

    fn close(a: V3, b: V3) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
    }

    fn origin(s: &Scene, i: usize) -> V3 {
        mat4_mul_point(&s.objects[i].world_matrix(), [0.0; 3])
    }

    /// A rotated and scaled parent (active) and a child at (4, 0, 0), both selected.
    fn pair() -> (Scene, usize, usize) {
        let mut s = Scene::startup();
        let p = s.active.unwrap();
        let pi = s.index_of(p).unwrap();
        s.objects[pi].loc = [0.0, 1.0, 0.0];
        s.objects[pi].rot = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        s.objects[pi].scale = [2.0, 2.0, 2.0];
        let c = s.add_primitive(Primitive::Cube, [4.0, 0.0, 0.0]);
        let ci = s.index_of(c).unwrap();
        s.objects[ci].rot = [0.0, 0.2, 0.0];
        s.refresh_parents();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[pi].selected = true;
        s.objects[ci].selected = true;
        s.active = Some(p);
        (s, pi, ci)
    }

    #[test]
    fn without_inverse_the_child_lands_on_the_parent_origin() {
        let (mut s, pi, ci) = pair();
        let rot = s.objects[ci].rot;
        assert_eq!(s.parent_set_no_inverse(false), 1);
        let c = &s.objects[ci];
        assert_eq!(c.parent, Some(s.objects[pi].id));
        assert_eq!(c.parent_inv, IDENTITY4);
        assert_eq!((c.loc, c.rot), ([0.0; 3], rot));
        assert!(close(origin(&s, ci), [0.0, 1.0, 0.0]), "{:?}", origin(&s, ci));
        /* It follows the parent from there. */
        s.objects[pi].loc = [3.0, 1.0, 0.0];
        s.refresh_parents();
        assert!(close(origin(&s, ci), [3.0, 1.0, 0.0]));
    }

    #[test]
    fn keep_transform_leaves_the_child_in_place_with_an_identity_inverse() {
        let (mut s, pi, ci) = pair();
        let before = mat4_mul_point(&s.objects[ci].world_matrix(), [1.0, 0.5, -0.5]);
        assert_eq!(s.parent_set_no_inverse(true), 1);
        assert_eq!(s.objects[ci].parent_inv, IDENTITY4);
        assert!(close(mat4_mul_point(&s.objects[ci].world_matrix(), [1.0, 0.5, -0.5]), before));
        /* The local location is the offset in the parent's space: (4,0,0) - (0,1,0) turned back and halved. */
        assert!(close(s.objects[ci].loc, [-0.5, -2.0, 0.0]), "{:?}", s.objects[ci].loc);
        s.objects[pi].loc[0] += 2.0;
        s.refresh_parents();
        /* Moving the parent 2 m in X moves the child from (4, 0, 0) to (6, 0, 0). */
        assert!(close(origin(&s, ci), [6.0, 0.0, 0.0]), "{:?}", origin(&s, ci));
    }

    #[test]
    fn reparenting_a_selected_chain_uses_the_worlds_from_before() {
        /* Cube <- A (selected) <- B (selected): making the new active the parent of both keeps B under A. */
        let mut s = Scene::startup();
        let cube = s.active.unwrap();
        let a = s.add_primitive(Primitive::Cube, [2.0, 0.0, 0.0]);
        let b = s.add_primitive(Primitive::Cube, [5.0, 0.0, 0.0]);
        let new_parent = s.add_primitive(Primitive::Cube, [0.0, 7.0, 0.0]);
        let (ai, bi) = (s.index_of(a).unwrap(), s.index_of(b).unwrap());
        s.objects[bi].parent = Some(a);
        s.refresh_parents();
        for o in &mut s.objects {
            o.selected = false;
        }
        s.objects[ai].selected = true;
        s.objects[bi].selected = true;
        let ni = s.index_of(new_parent).unwrap();
        s.objects[ni].selected = true;
        s.active = Some(new_parent);
        let (wa, wb) = (origin(&s, ai), origin(&s, bi));
        assert_eq!(s.parent_set_no_inverse(true), 2);
        assert!(close(origin(&s, ai), wa) && close(origin(&s, bi), wb), "{:?} {:?}", origin(&s, ai), origin(&s, bi));
        let _ = cube;
    }

    #[test]
    fn parenting_without_inverse_refuses_cycles_nothing_selected_and_undoes() {
        let (mut s, pi, ci) = pair();
        s.parent_set_no_inverse(true);
        /* The parent is now a descendant-free root; making the child the parent of the parent is a cycle. */
        s.objects[pi].selected = true;
        s.objects[ci].selected = true;
        s.active = Some(s.objects[ci].id);
        assert_eq!(s.parent_set_no_inverse(false), 0);
        assert_eq!(s.objects[pi].parent, None);
        for o in &mut s.objects {
            o.selected = false;
        }
        assert_eq!(s.parent_set_no_inverse(false), 0);
        let (mut s, _, ci) = pair();
        let before = s.objects[ci].loc;
        s.parent_set_no_inverse(false);
        assert!(s.undo());
        assert_eq!((s.objects[ci].parent, s.objects[ci].loc), (None, before));
    }

    #[test]
    fn link_object_data_copies_the_active_mesh_to_the_other_selected_meshes() {
        let mut s = Scene::startup();
        let sphere = s.add_primitive(Primitive::UvSphere, [3.0, 0.0, 0.0]);
        let other = s.add_primitive(Primitive::Cone, [-3.0, 0.0, 0.0]);
        let (si, oi, ci) = (s.index_of(sphere).unwrap(), s.index_of(other).unwrap(), s.objects.iter().position(|o| o.name == "Cube").unwrap());
        s.objects[si].smooth = true;
        for o in &mut s.objects {
            o.selected = o.mesh().is_some();
        }
        s.active = Some(sphere);
        let (loc_c, name_c) = (s.objects[ci].loc, s.objects[ci].name.clone());
        assert_eq!(s.link_object_data(), 2);
        let sphere_mesh = s.objects[si].mesh().unwrap().clone();
        assert!(s.objects[ci].mesh().unwrap() == &sphere_mesh && s.objects[oi].mesh().unwrap() == &sphere_mesh);
        assert!(s.objects[ci].smooth && s.objects[oi].smooth);
        /* Transforms and names are the objects' own. */
        assert_eq!((s.objects[ci].loc, s.objects[ci].name.clone()), (loc_c, name_c));
        assert_eq!(s.link_object_data(), 0, "already linked");
        assert!(s.undo());
        assert_ne!(s.objects[ci].mesh().unwrap(), &sphere_mesh);
    }

    #[test]
    fn link_object_data_needs_an_active_mesh_and_skips_other_types() {
        let mut s = Scene::startup();
        let cam = s.objects.iter().position(|o| o.name == "Camera").unwrap();
        for o in &mut s.objects {
            o.selected = true;
        }
        s.active = Some(s.objects[cam].id);
        assert_eq!(s.link_object_data(), 0, "the camera has no mesh to give");
        assert!(!s.can_undo());
        let cube = s.objects.iter().find(|o| o.name == "Cube").unwrap().id;
        s.active = Some(cube);
        assert_eq!(s.link_object_data(), 0, "no other mesh is selected");
        assert!(!s.can_undo());
    }

    #[test]
    fn the_cursor_snaps_to_the_grid() {
        assert_eq!(snap_to_grid([1.4, -2.6, 0.5], 1.0), [1.0, -3.0, 1.0]);
        assert_eq!(snap_to_grid([0.74, 0.76, -0.24], 0.5), [0.5, 1.0, 0.0]);
        assert_eq!(snap_to_grid([1.4, 2.0, 3.0], 0.0), [1.4, 2.0, 3.0]);
        assert_eq!(snap_to_grid([1.4, 2.0, 3.0], -1.0), [1.4, 2.0, 3.0]);
        assert_eq!(snap_to_grid([1.4, 2.0, 3.0], f32::NAN), [1.4, 2.0, 3.0]);
    }
}
