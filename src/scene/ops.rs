//! More object operators, as a child module of `scene` so that they can reach the private parts of `Object`.
//!
//! Modelled on Blender:
//!  - `OBJECT_OT_align_objects` (`relative_to`: Scene Origin / 3D Cursor / Selection / Active, `align_mode`:
//!    Negative Sides / Centers / Positive Sides, one global axis at a time)
//!  - `OBJECT_OT_transform_randomize` (location, rotation and scale; a seed that counts up on every call)
//!  - `OBJECT_OT_select_grouped` for Children, Immediate Children, Parent and Siblings (`Type` lives in `scene`)
//!  - `OBJECT_OT_origin_set` with `type = ORIGIN_CENTER_OF_VOLUME`: the volume centroid of a closed mesh
//!  - the statistics overlay (`Scene::statistics`): objects, vertices, faces and triangles
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
}
