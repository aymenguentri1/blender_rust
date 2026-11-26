//! The modal object transform: grab (G), rotate (R) and scale (S).
//!
//! Modelled on Blender's `transform.translate`, `transform.rotate` and `transform.resize`
//! (editors/transform/):
//!  - translate: `ED_view3d_win_to_delta()` of the movement since the press (`convertViewVec()`), scaled by
//!    `ED_view3d_calc_zfac()` of the pivot, exactly as the 3D cursor drag does; with an axis constraint the mouse
//!    movement is projected on the axis as it appears on screen
//!  - rotate: the angle swept around the pivot's screen position, about the view axis or the constrained global
//!    axis (the angle flips when that axis points away from the viewer, so a counter-clockwise mouse movement
//!    always turns the object counter-clockwise)
//!  - resize: the ratio of the distances from the pivot to the cursor now and at the press
//!  - X / Y / Z constrain to a global axis, Shift + X / Y / Z to the plane perpendicular to it (pressing the same
//!    one again removes the constraint; the local orientation of the second press is not modelled)
//!  - numeric input (`NumInput`): digits, `.`, `-` and Backspace replace the mouse value
//!  - Ctrl snaps to 1 m, 5 degrees and 0.1 (`transform_snap_increment`)
//!  - Shift is precision mode: the mouse movement counts a tenth while it is held (`t->mouse.precision`), and
//!    releasing it keeps what was reached instead of jumping back to the raw cursor position
//!  - the pivot is the median point of the selected objects' origins (`V3D_AROUND_CENTER_MEDIAN`)
//!  - confirm with Enter, Space or the left mouse button, cancel with Escape or the right mouse button

use crate::mesh::V3;
use std::cell::Cell;
use crate::scene::{axis_angle_to_mat3, euler_to_mat3, mat3_mul, mat3_mul_v3, mat3_to_euler, Mat3, Scene};
use crate::view3d::RegionView3D;

/// How much of the mouse movement counts while Shift is held.
pub const PRECISION_FACTOR: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    fn index(self) -> usize {
        self as usize
    }
    fn unit(self) -> V3 {
        let mut v = [0.0; 3];
        v[self.index()] = 1.0;
        v
    }
    fn name(self) -> &'static str {
        ["X", "Y", "Z"][self.index()]
    }
}

struct Item {
    id: u32,
    loc: V3,
    /// Where the origin is in the world and the matrix that turns a world offset into an offset of `loc`
    /// (identity and `loc` itself for an object without a parent).
    world: V3,
    to_local: Mat3,
    rot: V3,
    scale: V3,
}

/// `scene.tool_settings.transform_pivot_point` (the modes that make sense for whole objects).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PivotPoint {
    /// `V3D_AROUND_CENTER_MEDIAN`: the median of the selected origins.
    #[default]
    Median,
    /// `V3D_AROUND_CURSOR`: the 3D cursor.
    Cursor,
    /// `V3D_AROUND_LOCAL_ORIGINS`: every object rotates and scales about its own origin.
    Individual,
}

impl PivotPoint {
    /// The next mode of the cycle Median -> 3D Cursor -> Individual Origins.
    pub fn next(self) -> PivotPoint {
        match self {
            PivotPoint::Median => PivotPoint::Cursor,
            PivotPoint::Cursor => PivotPoint::Individual,
            PivotPoint::Individual => PivotPoint::Median,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PivotPoint::Median => "Median Point",
            PivotPoint::Cursor => "3D Cursor",
            PivotPoint::Individual => "Individual Origins",
        }
    }
}

pub struct Transform {
    pub mode: Mode,
    pub axis: Option<Axis>,
    /// Shift + axis: the plane perpendicular to `axis` instead of the axis.
    pub plane: bool,
    pub numeric: String,
    /// Ctrl is held: snap to increments.
    pub snap: bool,
    /// Shift is held: the mouse movement counts `PRECISION_FACTOR`.
    pub precision: bool,
    /// The press position in region pixels (y up).
    start: [f32; 2],
    /// The cursor position the last `apply()` saw, and the effective position it produced: they move together
    /// except in precision mode, where the effective one moves slower. Interior mutability because `apply` is `&self`.
    last_mval: Cell<[i32; 2]>,
    eff_mval: Cell<[f32; 2]>,
    items: Vec<Item>,
    pivot: V3,
    /// Individual Origins: each item rotates and scales about its own origin.
    individual: bool,
    zfac: f32,
    pushed_undo: bool,
}

fn project(rv3d: &RegionView3D, winx: i32, winy: i32, co: V3) -> Option<[f32; 2]> {
    let p = &rv3d.persmat;
    let c: [f32; 4] = std::array::from_fn(|i| p[0][i] * co[0] + p[1][i] * co[1] + p[2][i] * co[2] + p[3][i]);
    if c[3] <= 1e-6 {
        return None;
    }
    Some([(c[0] / c[3] * 0.5 + 0.5) * winx as f32, (c[1] / c[3] * 0.5 + 0.5) * winy as f32])
}

/// `InputSpringFlip()` with `calcSpringFactor()` (transform_input.cc): the distance of the cursor
/// from the pivot over the distance of the start point (1 when that is 0), negated once the
/// cursor is on the other side of the pivot than the start point (`center` truncated to int
/// for that test, as Blender does).
fn spring_flip_ratio(center: [f32; 2], start: [f32; 2], mval: [f32; 2]) -> f32 {
    let mut factor = ((center[0] - start[0]).powi(2) + (center[1] - start[1]).powi(2)).sqrt();
    if factor == 0.0 {
        factor = 1.0;
    }
    let ratio = ((center[0] - mval[0]).powi(2) + (center[1] - mval[1]).powi(2)).sqrt() / factor;
    let (cx, cy) = (center[0].trunc(), center[1].trunc());
    let flip = (cx - mval[0]) * (cx - start[0].trunc()) + (cy - mval[1]) * (cy - start[1].trunc());
    if flip < 0.0 { -ratio } else { ratio }
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// `compatible_eul()`: the equivalent euler angles closest to `old`, so a rotation does not flip by 360 degrees.
fn euler_compatible(m: &Mat3, old: V3) -> V3 {
    let e = mat3_to_euler(m);
    let alt = [e[0] + std::f32::consts::PI, std::f32::consts::PI - e[1], e[2] + std::f32::consts::PI];
    let near = |e: V3| -> V3 { std::array::from_fn(|k| old[k] + wrap_angle(e[k] - old[k])) };
    let (a, b) = (near(e), near(alt));
    let cost = |v: V3| (0..3).map(|k| (v[k] - old[k]).abs()).sum::<f32>();
    if cost(b) < cost(a) {
        b
    } else {
        a
    }
}

/// `it.loc` after the origin moved by the world space offset `delta`.
fn moved(it: &Item, delta: V3) -> V3 {
    let d = mat3_mul_v3(&it.to_local, delta);
    std::array::from_fn(|k| it.loc[k] + d[k])
}

impl Transform {
    /// Starts the operator on the selected objects; `None` when nothing is selected. `mval` is the cursor in
    /// region pixels (y up). `push_undo` is false when an operator that already pushed a step starts it (Shift + D).
    pub fn begin(mode: Mode, scene: &mut Scene, mval: [i32; 2], rv3d: &RegionView3D, push_undo: bool) -> Option<Transform> {
        Self::begin_with_pivot(mode, scene, mval, rv3d, push_undo, PivotPoint::Median, [0.0; 3])
    }

    /// `begin()` with a pivot point setting; `cursor` is the 3D cursor, used by `PivotPoint::Cursor`.
    pub fn begin_with_pivot(mode: Mode, scene: &mut Scene, mval: [i32; 2], rv3d: &RegionView3D, push_undo: bool, pivot_point: PivotPoint, cursor: V3) -> Option<Transform> {
        let median = scene.median_origin()?;
        let pivot = if pivot_point == PivotPoint::Cursor { cursor } else { median };
        /* An object whose parent is selected too moves with it and is left out (`transform_object_deselect`). */
        let items: Vec<Item> = (0..scene.objects.len())
            .filter(|&i| scene.objects[i].selected && !scene.objects[i].hidden && !scene.has_selected_ancestor(i))
            .map(|i| {
                let o = &scene.objects[i];
                let w = o.world_matrix()[3];
                Item { id: o.id, loc: o.loc, world: [w[0], w[1], w[2]], to_local: o.parent_lin_inv(), rot: o.rot, scale: o.scale }
            })
            .collect();
        if push_undo {
            scene.push_undo();
        }
        Some(Transform {
            mode,
            axis: None,
            plane: false,
            numeric: String::new(),
            snap: false,
            precision: false,
            start: [mval[0] as f32, mval[1] as f32],
            last_mval: Cell::new(mval),
            eff_mval: Cell::new([mval[0] as f32, mval[1] as f32]),
            items,
            pivot,
            individual: pivot_point == PivotPoint::Individual,
            zfac: rv3d.calc_zfac(pivot),
            pushed_undo: push_undo,
        })
    }

    /// X / Y / Z (`plane` with Shift): sets the constraint, or removes it when it is already set.
    pub fn set_axis(&mut self, axis: Axis, plane: bool) {
        if self.axis == Some(axis) && self.plane == plane {
            self.axis = None;
            self.plane = false;
        } else {
            self.axis = Some(axis);
            self.plane = plane;
        }
    }

    /// A key of the numeric input; returns whether it was used.
    pub fn numeric_key(&mut self, c: char) -> bool {
        match c {
            '0'..='9' => self.numeric.push(c),
            '.' if !self.numeric.contains('.') => self.numeric.push('.'),
            '-' => {
                if self.numeric.starts_with('-') {
                    self.numeric.remove(0);
                } else {
                    self.numeric.insert(0, '-');
                }
            }
            _ => return false,
        }
        true
    }

    pub fn numeric_backspace(&mut self) -> bool {
        self.numeric.pop().is_some()
    }

    /// The typed value: `None` while nothing (or only a sign / point) is typed.
    fn typed(&self) -> Option<f32> {
        self.numeric.parse::<f32>().ok()
    }

    /// Writes the transform for the cursor at `mval` into the objects (always from their initial state) and
    /// returns the text of the viewport header.
    pub fn apply(&self, scene: &mut Scene, mval: [i32; 2], rv3d: &RegionView3D, winx: i32, winy: i32) -> String {
        /* The movement since the last call counts in full, or a tenth of it in precision mode. */
        let last = self.last_mval.replace(mval);
        let gain = if self.precision { PRECISION_FACTOR } else { 1.0 };
        let eff = self.eff_mval.get();
        let mval = [eff[0] + (mval[0] - last[0]) as f32 * gain, eff[1] + (mval[1] - last[1]) as f32 * gain];
        self.eff_mval.set(mval);
        let text = match self.mode {
            Mode::Translate => self.apply_translate(scene, mval, rv3d, winx, winy),
            Mode::Rotate => self.apply_rotate(scene, mval, rv3d, winx, winy),
            Mode::Scale => self.apply_scale(scene, mval, rv3d, winx, winy),
        };
        let constraint = match (self.axis, self.plane) {
            (Some(a), false) => format!(" along global {}", a.name()),
            (Some(a), true) => format!(" along global {} plane", a.name()),
            _ => String::new(),
        };
        format!("{text}{constraint}")
    }

    fn set_items(&self, scene: &mut Scene, f: impl Fn(&Item) -> (V3, V3, V3)) {
        for item in &self.items {
            if let Some(i) = scene.index_of(item.id) {
                let (loc, rot, scale) = f(item);
                let o = &mut scene.objects[i];
                o.loc = loc;
                o.rot = rot;
                o.scale = scale;
            }
        }
    }

    fn apply_translate(&self, scene: &mut Scene, mval: [f32; 2], rv3d: &RegionView3D, winx: i32, winy: i32) -> String {
        let (dx, dy) = (mval[0] - self.start[0], mval[1] - self.start[1]);
        let free = rv3d.win_to_delta(winx, winy, self.zfac, dx, dy);
        let mut vec = free;
        if let Some(axis) = self.axis {
            let u = axis.unit();
            if self.plane {
                vec[axis.index()] = 0.0;
            } else {
                /* The axis as it appears on screen: how many pixels one metre along it moves the pivot. */
                let screen = match (project(rv3d, winx, winy, self.pivot), project(rv3d, winx, winy, std::array::from_fn(|k| self.pivot[k] + u[k]))) {
                    (Some(a), Some(b)) => [b[0] - a[0], b[1] - a[1]],
                    _ => [0.0, 0.0],
                };
                let len2 = screen[0] * screen[0] + screen[1] * screen[1];
                let t = if len2 > 1e-6 { (dx * screen[0] + dy * screen[1]) / len2 } else { 0.0 };
                vec = [u[0] * t, u[1] * t, u[2] * t];
            }
        }
        if let Some(v) = self.typed() {
            vec = match self.axis {
                Some(axis) if !self.plane => {
                    let mut o = [0.0; 3];
                    o[axis.index()] = v;
                    o
                }
                _ => [v, 0.0, 0.0],
            };
        } else if self.snap {
            vec = vec.map(|c| c.round());
        }
        self.set_items(scene, |it| (moved(it, vec), it.rot, it.scale));
        format!("D: {:.4}  {:.4}  {:.4} m", vec[0], vec[1], vec[2])
    }

    /// The axis a rotation turns about: the constrained global axis or the view axis (towards the viewer).
    fn rotation_axis(&self, rv3d: &RegionView3D) -> V3 {
        match self.axis {
            Some(a) if !self.plane => a.unit(),
            _ => {
                let z = rv3d.viewinv[2];
                let l = (z[0] * z[0] + z[1] * z[1] + z[2] * z[2]).sqrt().max(1e-20);
                [z[0] / l, z[1] / l, z[2] / l]
            }
        }
    }

    fn apply_rotate(&self, scene: &mut Scene, mval: [f32; 2], rv3d: &RegionView3D, winx: i32, winy: i32) -> String {
        let axis = self.rotation_axis(rv3d);
        let view_z = rv3d.viewinv[2];
        let mut angle = match self.typed() {
            Some(deg) => deg.to_radians(),
            None => match project(rv3d, winx, winy, self.pivot) {
                Some(c) => {
                    let a0 = (self.start[1] - c[1]).atan2(self.start[0] - c[0]);
                    let a1 = (mval[1] - c[1]).atan2(mval[0] - c[0]);
                    let mut a = wrap_angle(a1 - a0);
                    /* An axis that points away from the viewer turns the other way round on screen. */
                    if dot(axis, [view_z[0], view_z[1], view_z[2]]) < 0.0 {
                        a = -a;
                    }
                    if self.snap {
                        let step = 5.0f32.to_radians();
                        a = (a / step).round() * step;
                    }
                    a
                }
                None => 0.0,
            },
        };
        if !angle.is_finite() {
            angle = 0.0;
        }
        let r = axis_angle_to_mat3(axis, angle);
        let (pivot, individual) = (self.pivot, self.individual);
        self.set_items(scene, |it| {
            let pivot = if individual { it.world } else { pivot };
            let rel = [it.world[0] - pivot[0], it.world[1] - pivot[1], it.world[2] - pivot[2]];
            let rotated = mat3_mul_v3(&r, rel);
            let loc = moved(it, std::array::from_fn(|k| rotated[k] - rel[k]));
            let rot = euler_compatible(&mat3_mul(&r, &euler_to_mat3(it.rot)), it.rot);
            (loc, rot, it.scale)
        });
        match self.axis {
            Some(_) if !self.plane => format!("Rotate: {:.2}\u{b0}", angle.to_degrees()),
            _ => format!("Rotate: {:.2}\u{b0} around view axis", angle.to_degrees()),
        }
    }

    fn apply_scale(&self, scene: &mut Scene, mval: [f32; 2], rv3d: &RegionView3D, winx: i32, winy: i32) -> String {
        let factor = match self.typed() {
            Some(v) => v,
            None => match project(rv3d, winx, winy, self.pivot) {
                Some(c) => {
                    let f = spring_flip_ratio(c, self.start, mval);
                    if self.snap {
                        (f * 10.0).round() / 10.0
                    } else {
                        f
                    }
                }
                None => 1.0,
            },
        };
        let mut f = [factor; 3];
        if let Some(axis) = self.axis {
            for (k, fk) in f.iter_mut().enumerate() {
                let on_axis = k == axis.index();
                if on_axis == self.plane {
                    *fk = 1.0;
                }
            }
        }
        let (pivot, individual) = (self.pivot, self.individual);
        self.set_items(scene, |it| {
            let pivot = if individual { it.world } else { pivot };
            let loc = moved(it, std::array::from_fn(|k| (it.world[k] - pivot[k]) * (f[k] - 1.0)));
            let scale = std::array::from_fn(|k| it.scale[k] * f[k]);
            (loc, it.rot, scale)
        });
        format!("Scale: {:.4}", factor)
    }

    /// Escape / right mouse button: puts the objects back (and drops the undo step this operator pushed).
    pub fn cancel(self, scene: &mut Scene) {
        self.set_items(scene, |it| (it.loc, it.rot, it.scale));
        if self.pushed_undo {
            scene.pop_undo();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Primitive;
    use crate::view3d::View3D;

    const W: i32 = 200;
    const H: i32 = 100;

    /// The default view of `RegionView3D` (identity quaternion) is the Top view: +X is right on screen, +Y is up,
    /// and the view axis (towards the viewer) is +Z. The origin projects to the centre of the 200 x 100 region.
    fn view() -> RegionView3D {
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), W, H);
        rv3d
    }

    fn start(mode: Mode) -> (Scene, Transform, RegionView3D) {
        let rv3d = view();
        let mut scene = Scene::startup();
        let t = Transform::begin(mode, &mut scene, [100, 50], &rv3d, true).unwrap();
        (scene, t, rv3d)
    }

    fn cube(scene: &Scene) -> &crate::scene::Object {
        scene.objects.iter().find(|o| o.name == "Cube").unwrap()
    }

    fn two_cubes() -> Scene {
        let mut scene = Scene::startup();
        let id = scene.add_primitive(crate::mesh::Primitive::Cube, [4.0, 0.0, 0.0]);
        for o in &mut scene.objects {
            o.selected = o.mesh().is_some();
        }
        scene.active = Some(id);
        scene
    }

    fn obj<'a>(scene: &'a Scene, name: &str) -> &'a crate::scene::Object {
        scene.objects.iter().find(|o| o.name == name).unwrap()
    }

    #[test]
    fn spring_ratio_grows_with_distance_and_flips_past_the_pivot() {
        let (c, s) = ([100.0, 50.0], [150.0, 50.0]);
        assert_eq!(spring_flip_ratio(c, s, [200.0, 50.0]), 2.0);
        assert_eq!(spring_flip_ratio(c, s, [125.0, 50.0]), 0.5);
        assert_eq!(spring_flip_ratio(c, s, [50.0, 50.0]), -1.0);
        assert_eq!(spring_flip_ratio(c, s, [100.0, 100.0]), 1.0);
    }

    #[test]
    fn spring_ratio_with_start_on_the_pivot_uses_factor_one() {
        assert_eq!(spring_flip_ratio([10.0, 10.0], [10.0, 10.0], [13.0, 14.0]), 5.0);
    }

    #[test]
    fn scale_about_the_median_moves_the_origins_apart() {
        let rv3d = view();
        let mut scene = two_cubes();
        let mut t = Transform::begin(Mode::Scale, &mut scene, [100, 50], &rv3d, true).unwrap();
        t.numeric = "2".into();
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        /* Origins at 0 and 4, median 2: they go to -2 and 6. */
        assert!((obj(&scene, "Cube").loc[0] + 2.0).abs() < 1e-4);
        assert!((obj(&scene, "Cube.001").loc[0] - 6.0).abs() < 1e-4);
    }

    #[test]
    fn scale_about_individual_origins_keeps_the_locations() {
        let rv3d = view();
        let mut scene = two_cubes();
        let mut t = Transform::begin_with_pivot(Mode::Scale, &mut scene, [100, 50], &rv3d, true, PivotPoint::Individual, [0.0; 3]).unwrap();
        t.numeric = "2".into();
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert!(obj(&scene, "Cube").loc[0].abs() < 1e-4);
        assert!((obj(&scene, "Cube.001").loc[0] - 4.0).abs() < 1e-4);
        assert_eq!(obj(&scene, "Cube").scale, [2.0; 3]);
        assert_eq!(obj(&scene, "Cube.001").scale, [2.0; 3]);
    }

    #[test]
    fn scale_about_the_3d_cursor() {
        let rv3d = view();
        let mut scene = two_cubes();
        let mut t = Transform::begin_with_pivot(Mode::Scale, &mut scene, [100, 50], &rv3d, true, PivotPoint::Cursor, [4.0, 0.0, 0.0]).unwrap();
        t.numeric = "2".into();
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        /* The cube at the cursor stays, the one at the origin moves to -4. */
        assert!((obj(&scene, "Cube").loc[0] + 4.0).abs() < 1e-4);
        assert!((obj(&scene, "Cube.001").loc[0] - 4.0).abs() < 1e-4);
    }

    #[test]
    fn rotate_about_individual_origins_only_turns_the_objects() {
        let rv3d = view();
        let mut scene = two_cubes();
        let mut t = Transform::begin_with_pivot(Mode::Rotate, &mut scene, [150, 50], &rv3d, true, PivotPoint::Individual, [0.0; 3]).unwrap();
        t.axis = Some(Axis::Z);
        t.numeric = "90".into();
        t.apply(&mut scene, [150, 50], &rv3d, W, H);
        assert!((obj(&scene, "Cube.001").loc[0] - 4.0).abs() < 1e-4);
        assert!((obj(&scene, "Cube.001").rot[2].abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn pivot_point_cycles_through_the_three_modes() {
        let mut p = PivotPoint::default();
        assert_eq!(p, PivotPoint::Median);
        p = p.next();
        assert_eq!((p, p.label()), (PivotPoint::Cursor, "3D Cursor"));
        p = p.next();
        assert_eq!(p, PivotPoint::Individual);
        assert_eq!(p.next(), PivotPoint::Median);
    }

    #[test]
    fn nothing_selected_does_not_start() {
        let mut scene = Scene::startup();
        scene.select_all(crate::scene::SelectAction::Deselect);
        assert!(Transform::begin(Mode::Translate, &mut scene, [0, 0], &view(), true).is_none());
        assert!(!scene.can_undo());
    }

    #[test]
    fn precision_mode_counts_a_tenth_of_the_mouse_movement() {
        let (mut full_scene, full, rv3d) = start(Mode::Translate);
        full.apply(&mut full_scene, [150, 50], &rv3d, W, H);
        let x_full = cube(&full_scene).loc[0];
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.precision = true;
        t.apply(&mut scene, [150, 50], &rv3d, W, H);
        let x = cube(&scene).loc[0];
        assert!(x_full > 0.1);
        assert!((x - x_full * PRECISION_FACTOR).abs() < 1e-4, "{x} vs {x_full}");
    }

    #[test]
    fn releasing_shift_keeps_the_value_instead_of_jumping_to_the_cursor() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.precision = true;
        t.apply(&mut scene, [200, 50], &rv3d, W, H);
        let slow = cube(&scene).loc[0];
        t.precision = false;
        t.apply(&mut scene, [200, 50], &rv3d, W, H);
        assert!((cube(&scene).loc[0] - slow).abs() < 1e-6, "no jump when Shift is released");
        /* Further movement counts in full again: 10 px is a tenth of the 100 px of the first move. */
        t.apply(&mut scene, [210, 50], &rv3d, W, H);
        let (mut ref_scene, ref_t, rv) = start(Mode::Translate);
        ref_t.apply(&mut ref_scene, [110, 50], &rv, W, H);
        let ten_px = cube(&ref_scene).loc[0];
        assert!((cube(&scene).loc[0] - (slow + ten_px)).abs() < 1e-4);
    }

    #[test]
    fn precision_mode_slows_rotation_and_scale_too() {
        /* Pressed 50 px right of the pivot (the centre of the region), since a press on the pivot has no reference. */
        let from_right = |mode: Mode, precision: bool, to: [i32; 2]| {
            let rv3d = view();
            let mut scene = Scene::startup();
            let mut t = Transform::begin(mode, &mut scene, [150, 50], &rv3d, true).unwrap();
            t.precision = precision;
            t.apply(&mut scene, to, &rv3d, W, H);
            let o = cube(&scene);
            (o.scale[0], o.rot[2].abs())
        };
        let (full, _) = from_right(Mode::Scale, false, [250, 50]);
        let (slow, _) = from_right(Mode::Scale, true, [250, 50]);
        assert!((full - 3.0).abs() < 1e-3, "{full}");
        assert!(slow > 1.0 && slow < 1.5, "{slow}");

        let (_, fast) = from_right(Mode::Rotate, false, [150, 100]);
        let (_, slow) = from_right(Mode::Rotate, true, [150, 100]);
        assert!((fast - std::f32::consts::FRAC_PI_4).abs() < 1e-3, "{fast}");
        assert!(slow > 0.0 && slow < fast * 0.2, "{slow} {fast}");
    }

    #[test]
    fn precision_ignores_nothing_when_the_cursor_stands_still() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.precision = true;
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(cube(&scene).loc, [0.0; 3]);
    }

    #[test]
    fn grab_moves_in_the_view_plane() {
        let (mut scene, t, rv3d) = start(Mode::Translate);
        t.apply(&mut scene, [150, 50], &rv3d, W, H);
        let loc = cube(&scene).loc;
        assert!(loc[0] > 0.1, "{loc:?}");
        assert!(loc[1].abs() < 1e-4 && loc[2].abs() < 1e-4, "{loc:?}");
        /* Moving the mouse up moves the object up on screen (+Y in the top view). */
        t.apply(&mut scene, [100, 80], &rv3d, W, H);
        let loc = cube(&scene).loc;
        assert!(loc[1] > 0.1 && loc[0].abs() < 1e-4 && loc[2].abs() < 1e-4, "{loc:?}");
    }

    #[test]
    fn the_object_follows_the_cursor_exactly() {
        /* `win_to_delta()` with the zfac of the pivot: the pivot's projection moves by the mouse movement. */
        let (mut scene, t, rv3d) = start(Mode::Translate);
        t.apply(&mut scene, [130, 60], &rv3d, W, H);
        let loc = cube(&scene).loc;
        let p = project(&rv3d, W, H, loc).unwrap();
        assert!((p[0] - 130.0).abs() < 0.1 && (p[1] - 60.0).abs() < 0.1, "{p:?}");
    }

    #[test]
    fn axis_constraint_keeps_the_other_axes() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.set_axis(Axis::X, false);
        t.apply(&mut scene, [140, 90], &rv3d, W, H);
        let loc = cube(&scene).loc;
        assert!(loc[0] > 0.1 && loc[1] == 0.0 && loc[2] == 0.0, "{loc:?}");
    }

    #[test]
    fn plane_constraint_removes_one_axis() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.set_axis(Axis::X, true);
        t.apply(&mut scene, [140, 90], &rv3d, W, H);
        assert_eq!(cube(&scene).loc[0], 0.0);
        assert!(cube(&scene).loc[1] > 0.1);
    }

    #[test]
    fn pressing_the_same_axis_again_clears_it() {
        let (_, mut t, _) = start(Mode::Translate);
        t.set_axis(Axis::Y, false);
        assert_eq!(t.axis, Some(Axis::Y));
        t.set_axis(Axis::Y, false);
        assert_eq!(t.axis, None);
        t.set_axis(Axis::Y, false);
        t.set_axis(Axis::Y, true);
        assert_eq!((t.axis, t.plane), (Some(Axis::Y), true));
    }

    #[test]
    fn numeric_grab_along_an_axis() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.set_axis(Axis::Y, false);
        for c in "2.5".chars() {
            assert!(t.numeric_key(c));
        }
        let text = t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(cube(&scene).loc, [0.0, 2.5, 0.0]);
        assert!(text.contains("2.5000") && text.contains("along global Y"), "{text}");
        t.numeric_key('-');
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(cube(&scene).loc[1], -2.5);
        assert!(t.numeric_backspace() && t.numeric_backspace() && t.numeric_backspace());
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(cube(&scene).loc, [0.0; 3], "an empty field falls back to the mouse");
    }

    #[test]
    fn numeric_input_rejects_other_keys_and_a_second_point() {
        let (_, mut t, _) = start(Mode::Translate);
        assert!(!t.numeric_key('x'));
        assert!(t.numeric_key('1') && t.numeric_key('.') && !t.numeric_key('.'));
        assert_eq!(t.numeric, "1.");
    }

    #[test]
    fn ctrl_snaps_a_grab_to_whole_metres() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.set_axis(Axis::X, false);
        t.snap = true;
        t.apply(&mut scene, [123, 50], &rv3d, W, H);
        let x = cube(&scene).loc[0];
        assert_eq!(x, x.round());
    }

    #[test]
    fn numeric_rotation_about_an_axis() {
        let (mut scene, mut t, rv3d) = start(Mode::Rotate);
        t.set_axis(Axis::Z, false);
        t.numeric_key('9');
        t.numeric_key('0');
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        let rot = cube(&scene).rot;
        assert!((rot[2] - std::f32::consts::FRAC_PI_2).abs() < 1e-4 && rot[0].abs() < 1e-4, "{rot:?}");
    }

    #[test]
    fn rotation_turns_counter_clockwise_with_the_mouse() {
        /* The pivot is at the screen centre (100, 50): moving the mouse from the right of it to above it is a
         * counter-clockwise quarter turn, a positive rotation about the view axis (towards the viewer). */
        let rv3d = view();
        let mut scene = Scene::startup();
        let t = Transform::begin(Mode::Rotate, &mut scene, [150, 50], &rv3d, true).unwrap();
        t.apply(&mut scene, [100, 100], &rv3d, W, H);
        let m = euler_to_mat3(cube(&scene).rot);
        let v = rv3d.viewinv[2];
        /* The rotation axis is the view axis: it is unchanged by the rotation. */
        let r = mat3_mul_v3(&m, [v[0], v[1], v[2]]);
        assert!((r[0] - v[0]).abs() < 1e-4 && (r[1] - v[1]).abs() < 1e-4 && (r[2] - v[2]).abs() < 1e-4);
        /* A +90 degree turn about the view axis sends +X (screen right) to screen up (+Y in the top view). */
        let up = mat3_mul_v3(&m, [1.0, 0.0, 0.0]);
        assert!(up[1] > 0.99, "{up:?}");
    }

    #[test]
    fn rotation_turns_clockwise_when_the_mouse_does() {
        let rv3d = view();
        let mut scene = Scene::startup();
        let t = Transform::begin(Mode::Rotate, &mut scene, [150, 50], &rv3d, true).unwrap();
        t.apply(&mut scene, [100, 0], &rv3d, W, H);
        let down = mat3_mul_v3(&euler_to_mat3(cube(&scene).rot), [1.0, 0.0, 0.0]);
        assert!(down[1] < -0.99, "{down:?}");
    }

    /* An axis pointing away from the viewer turns the other way, so the object still follows the mouse: seen from
     * below (the Bottom view) a counter-clockwise mouse movement is a negative rotation about +Z. */
    #[test]
    fn a_constrained_axis_facing_away_flips_the_angle() {
        let mut rv3d = RegionView3D::default();
        rv3d.view_axis(crate::view3d::View::Bottom);
        rv3d.update(&View3D::default(), W, H);
        let mut scene = Scene::startup();
        let mut t = Transform::begin(Mode::Rotate, &mut scene, [150, 50], &rv3d, true).unwrap();
        t.set_axis(Axis::Z, false);
        t.apply(&mut scene, [100, 100], &rv3d, W, H);
        let z = cube(&scene).rot[2];
        assert!((z + std::f32::consts::FRAC_PI_2).abs() < 1e-3, "{z}");
    }

    #[test]
    fn rotation_of_a_selection_orbits_the_median_point() {
        let rv3d = view();
        let mut scene = Scene::startup();
        scene.select_all(crate::scene::SelectAction::Deselect);
        let a = scene.add_primitive(Primitive::Cube, [2.0, 0.0, 0.0]);
        let b = scene.add_primitive(Primitive::Cube, [-2.0, 0.0, 0.0]);
        /* `b` is selected by the add; Shift-selecting `a` adds it. */
        scene.select_pick(Some(a), true);
        assert_eq!(scene.selected_count(), 2);
        let mut t = Transform::begin(Mode::Rotate, &mut scene, [100, 50], &rv3d, true).unwrap();
        t.set_axis(Axis::Z, false);
        for c in "90".chars() {
            t.numeric_key(c);
        }
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        let la = scene.objects[scene.index_of(a).unwrap()].loc;
        let lb = scene.objects[scene.index_of(b).unwrap()].loc;
        assert!(la[0].abs() < 1e-4 && (la[1] - 2.0).abs() < 1e-4, "{la:?}");
        assert!(lb[0].abs() < 1e-4 && (lb[1] + 2.0).abs() < 1e-4, "{lb:?}");
    }

    #[test]
    fn scale_follows_the_distance_to_the_pivot() {
        let rv3d = view();
        let mut scene = Scene::startup();
        let t = Transform::begin(Mode::Scale, &mut scene, [150, 50], &rv3d, true).unwrap();
        t.apply(&mut scene, [200, 50], &rv3d, W, H);
        let s = cube(&scene).scale;
        assert!((s[0] - 2.0).abs() < 1e-4 && s[0] == s[1] && s[1] == s[2], "{s:?}");
        t.apply(&mut scene, [125, 50], &rv3d, W, H);
        assert!((cube(&scene).scale[0] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn scale_on_an_axis_and_numeric() {
        let (mut scene, mut t, rv3d) = start(Mode::Scale);
        t.set_axis(Axis::Z, false);
        t.numeric_key('3');
        let text = t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(cube(&scene).scale, [1.0, 1.0, 3.0]);
        assert!(text.starts_with("Scale: 3.0000"), "{text}");
        t.set_axis(Axis::Z, true);
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(cube(&scene).scale, [3.0, 3.0, 1.0]);
    }

    #[test]
    fn ctrl_snaps_a_scale_to_a_tenth() {
        let (mut scene, mut t, rv3d) = start(Mode::Scale);
        t.snap = true;
        t.apply(&mut scene, [100 + 37, 50], &rv3d, W, H);
        let s = cube(&scene).scale[0];
        assert!(((s * 10.0).round() - s * 10.0).abs() < 1e-4, "{s}");
    }

    #[test]
    fn scaling_several_objects_scales_their_distance_from_the_median() {
        let rv3d = view();
        let mut scene = Scene::startup();
        scene.select_all(crate::scene::SelectAction::Deselect);
        let a = scene.add_primitive(Primitive::Cube, [1.0, 0.0, 0.0]);
        let b = scene.add_primitive(Primitive::Cube, [-1.0, 0.0, 0.0]);
        scene.select_pick(Some(a), true);
        assert_eq!(scene.selected_count(), 2);
        let mut t = Transform::begin(Mode::Scale, &mut scene, [100, 50], &rv3d, true).unwrap();
        t.numeric_key('2');
        t.apply(&mut scene, [100, 50], &rv3d, W, H);
        assert_eq!(scene.objects[scene.index_of(a).unwrap()].loc, [2.0, 0.0, 0.0]);
        assert_eq!(scene.objects[scene.index_of(b).unwrap()].loc, [-2.0, 0.0, 0.0]);
    }

    #[test]
    fn cancel_restores_the_objects_and_the_undo_stack() {
        let (mut scene, mut t, rv3d) = start(Mode::Translate);
        t.set_axis(Axis::X, false);
        t.apply(&mut scene, [180, 50], &rv3d, W, H);
        assert_ne!(cube(&scene).loc, [0.0; 3]);
        t.cancel(&mut scene);
        assert_eq!(cube(&scene).loc, [0.0; 3]);
        assert!(!scene.can_undo());
    }

    #[test]
    fn confirm_leaves_one_undo_step() {
        let (mut scene, t, rv3d) = start(Mode::Translate);
        t.apply(&mut scene, [150, 50], &rv3d, W, H);
        assert_ne!(cube(&scene).loc, [0.0; 3]);
        assert!(scene.undo());
        assert_eq!(cube(&scene).loc, [0.0; 3]);
        assert!(!scene.can_undo());
    }

    #[test]
    fn cancel_after_duplicate_keeps_the_duplicate() {
        let rv3d = view();
        let mut scene = Scene::startup();
        scene.duplicate_selected();
        let mut t = Transform::begin(Mode::Translate, &mut scene, [100, 50], &rv3d, false).unwrap();
        t.set_axis(Axis::X, false);
        t.apply(&mut scene, [180, 50], &rv3d, W, H);
        t.cancel(&mut scene);
        assert_eq!(scene.objects.len(), 4);
        assert_eq!(scene.active_object().unwrap().loc, [0.0; 3]);
        assert!(scene.undo());
        assert_eq!(scene.objects.len(), 3, "grab and duplicate share one undo step");
    }

    #[test]
    fn rotation_stays_close_to_the_previous_euler_angles() {
        let m = euler_to_mat3([3.0, 0.2, -3.0]);
        let e = euler_compatible(&m, [3.0, 0.2, -3.0]);
        assert!((e[0] - 3.0).abs() < 1e-3 && (e[2] + 3.0).abs() < 1e-3, "{e:?}");
        /* An angle past 180 degrees continues instead of wrapping back. */
        let m = euler_to_mat3([0.0, 0.0, 3.3]);
        let e = euler_compatible(&m, [0.0, 0.0, 3.0]);
        assert!((e[2] - 3.3).abs() < 1e-3, "{e:?}");
    }

    #[test]
    fn grabbing_a_child_moves_it_in_world_space_and_a_selected_parent_carries_it() {
        let mut scene = Scene::startup();
        let a = cube(&scene).id;
        let b = scene.add_primitive(crate::mesh::Primitive::Cube, [5.0, 0.0, 0.0]);
        let (ai, bi) = (scene.index_of(a).unwrap(), scene.index_of(b).unwrap());
        scene.objects[ai].rot = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        scene.objects[ai].scale = [2.0, 2.0, 2.0];
        scene.refresh_parents();
        scene.active = Some(a);
        scene.select_all(crate::scene::SelectAction::Deselect);
        scene.objects[ai].selected = true;
        scene.objects[bi].selected = true;
        assert_eq!(scene.parent_set(), 1);
        /* Both selected: only the parent is an item. */
        let rv3d = view();
        let t = Transform::begin(Mode::Translate, &mut scene, [0, 0], &rv3d, true).unwrap();
        assert_eq!(t.items.len(), 1);
        t.cancel(&mut scene);
        /* Only the child selected: a world +X offset of 1 becomes -0.5 along the child's local Y... through the
         * parent's quarter turn and scale of 2, so the world position moves by exactly 1 in X. */
        scene.objects[ai].selected = false;
        let before = scene.objects[bi].world_matrix()[3];
        let mut t = Transform::begin(Mode::Translate, &mut scene, [0, 0], &rv3d, true).unwrap();
        t.numeric = "1".into();
        t.axis = Some(Axis::X);
        t.apply(&mut scene, [0, 0], &rv3d, W, H);
        scene.refresh_parents();
        let after = scene.objects[bi].world_matrix()[3];
        assert!((after[0] - before[0] - 1.0).abs() < 1e-4 && (after[1] - before[1]).abs() < 1e-4, "{before:?} {after:?}");
    }
}
