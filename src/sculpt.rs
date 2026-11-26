//! Sculpt Mode (`editors/sculpt_paint/sculpt.cc`, `paint_stroke.cc`), the part that is modelled here.
//!
//! A stroke is a series of dabs placed every `spacing` (10 % of the brush diameter) along the mouse path, each
//! one found by casting the pixel ray against the base mesh of the active object. A dab moves the vertices
//! inside the brush radius by `strength * falloff(distance)`, with the Smooth curve preset of the brush
//! (`3 t^2 - 2 t^3`, `t = 1 - distance / radius`).
//!
//! Brushes: Draw, Smooth, Grab, Inflate, Flatten, Pinch and Crease. Ctrl inverts the brush (the ring turns light
//! blue like `cursor_color_subtract`), Shift smooths while it is held. Symmetry mirrors every dab across the
//! local X / Y / Z planes (X is on, as in a new Blender sculpt session).
//!
//! Deviations: the displacement factors are tuned by eye (Blender derives them from the brush overlap), there is
//! no pressure, masking, face sets, dynamic topology or multires, the brush curve is always Smooth, and a mesh
//! with modifiers is sculpted on its base mesh (Blender does the same, but shows the result through the stack).

use crate::area::Rcti;
use crate::font;
use crate::mesh::{add, cross, dot, normalized, scale, sub, Mesh, V3};
use crate::overlay_cursor::draw_segment;
use crate::scene::{mat4_mul_dir, mat4_mul_point, ray_triangle, Object, Scene};
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::view3d::{invert_m4_m4, Mat4, RegionView3D};

/// `Brush::sculpt_brush_type` of the default brushes that are modelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brush {
    Draw,
    Smooth,
    Grab,
    Inflate,
    Flatten,
    Pinch,
    Crease,
    Clay,
}

impl Brush {
    pub const ALL: [Brush; 8] = [Brush::Draw, Brush::Smooth, Brush::Grab, Brush::Inflate, Brush::Flatten, Brush::Pinch, Brush::Crease, Brush::Clay];

    pub fn label(self) -> &'static str {
        match self {
            Brush::Draw => "Draw",
            Brush::Smooth => "Smooth",
            Brush::Grab => "Grab",
            Brush::Inflate => "Inflate",
            Brush::Flatten => "Flatten",
            Brush::Pinch => "Pinch",
            Brush::Crease => "Crease",
            Brush::Clay => "Clay Strips",
        }
    }
}

/// Displacement of a Draw dab at full strength and falloff, in brush radii.
const DRAW_FACTOR: f32 = 0.06;
/// Part of the distance to the target (plane, neighbour average, centre) a dab closes at full strength.
const BLEND_FACTOR: f32 = 0.3;
const PINCH_FACTOR: f32 = 0.2;
/// `Brush::spacing` (percent of the brush diameter) of the dotted brushes.
const SPACING: f32 = 0.10;
const MIN_RADIUS_PX: f32 = 5.0;
const MAX_RADIUS_PX: f32 = 500.0;
const MAX_DABS_PER_EVENT: usize = 48;

/// `Brush::curve` with the Smooth preset.
pub fn falloff(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 || distance >= radius {
        return 0.0;
    }
    let t = 1.0 - distance / radius;
    t * t * (3.0 - 2.0 * t)
}

/// The brush settings (`Brush::size`, `Brush::alpha`) and the symmetry of the mesh (`Mesh::symmetry`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub brush: Brush,
    /// `Brush::size`: the radius in pixels (Blender stores the diameter, shown as Radius in 4.x).
    pub radius_px: f32,
    pub strength: f32,
    pub symmetry: [bool; 3],
}

impl Default for Settings {
    fn default() -> Self {
        Settings { brush: Brush::Draw, radius_px: 50.0, strength: 0.5, symmetry: [true, false, false] }
    }
}

/// The vertices of `mesh` within `radius` of `center`, with their falloff.
fn affected(mesh: &Mesh, center: V3, radius: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    for (i, &v) in mesh.verts.iter().enumerate() {
        let d = sub(v, center);
        let d2 = dot(d, d);
        if d2 < radius * radius {
            let f = falloff(d2.sqrt(), radius);
            if f > 0.0 {
                out.push((i, f));
            }
        }
    }
    out
}

/// The vertex neighbours through the edges of the faces.
pub fn adjacency(mesh: &Mesh) -> Vec<Vec<u32>> {
    let mut adj = vec![Vec::<u32>::new(); mesh.verts.len()];
    for face in &mesh.faces {
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            if !adj[a as usize].contains(&b) {
                adj[a as usize].push(b);
            }
            if !adj[b as usize].contains(&a) {
                adj[b as usize].push(a);
            }
        }
    }
    adj
}

/// The mirror combinations of the enabled axes (`+++` first), as sign triples.
fn mirror_signs(sym: [bool; 3]) -> Vec<[f32; 3]> {
    let mut out = vec![[1.0f32; 3]];
    for axis in 0..3 {
        if sym[axis] {
            let n = out.len();
            for k in 0..n {
                let mut s = out[k];
                s[axis] = -1.0;
                out.push(s);
            }
        }
    }
    out
}

fn mirrored(v: V3, s: [f32; 3]) -> V3 {
    [v[0] * s[0], v[1] * s[1], v[2] * s[2]]
}

/// One dab of `brush` (not Grab) at `center`, in the local space of `mesh`, with every mirror of it.
#[allow(clippy::too_many_arguments)]
pub fn dab(mesh: &mut Mesh, adj: &[Vec<u32>], brush: Brush, center: V3, radius: f32, strength: f32, invert: bool, symmetry: [bool; 3]) {
    if brush == Brush::Grab {
        return;
    }
    for s in mirror_signs(symmetry) {
        dab_one(mesh, adj, brush, mirrored(center, s), radius, strength, invert);
    }
}

fn dab_one(mesh: &mut Mesh, adj: &[Vec<u32>], brush: Brush, center: V3, radius: f32, strength: f32, invert: bool) {
    let hit = affected(mesh, center, radius);
    if hit.is_empty() {
        return;
    }
    let dir = if invert { -1.0 } else { 1.0 };
    let normals = mesh.vertex_normals();
    /* `calc_area_normal()`: the falloff weighted mean of the vertex normals under the brush. */
    let mut area = [0.0f32; 3];
    for &(i, f) in &hit {
        area = add(area, scale(normals[i], f));
    }
    let area = normalized(area);
    let push = radius * strength * DRAW_FACTOR;
    match brush {
        Brush::Draw => {
            for &(i, f) in &hit {
                mesh.verts[i] = add(mesh.verts[i], scale(area, push * f * dir));
            }
        }
        Brush::Inflate => {
            for &(i, f) in &hit {
                mesh.verts[i] = add(mesh.verts[i], scale(normals[i], push * f * dir));
            }
        }
        Brush::Smooth => {
            let moved: Vec<(usize, V3)> = hit
                .iter()
                .filter(|&&(i, _)| !adj[i].is_empty())
                .map(|&(i, f)| {
                    let mut sum = [0.0f32; 3];
                    for &n in &adj[i] {
                        sum = add(sum, mesh.verts[n as usize]);
                    }
                    let avg = scale(sum, 1.0 / adj[i].len() as f32);
                    let v = mesh.verts[i];
                    (i, add(v, scale(sub(avg, v), (f * strength).min(1.0))))
                })
                .collect();
            for (i, p) in moved {
                mesh.verts[i] = p;
            }
        }
        Brush::Flatten => {
            let (mut sum, mut wsum) = ([0.0f32; 3], 0.0f32);
            for &(i, f) in &hit {
                sum = add(sum, scale(mesh.verts[i], f));
                wsum += f;
            }
            let plane = scale(sum, 1.0 / wsum.max(1e-12));
            for &(i, f) in &hit {
                let dist = dot(sub(mesh.verts[i], plane), area);
                mesh.verts[i] = sub(mesh.verts[i], scale(area, dist * (f * strength * BLEND_FACTOR).min(1.0)));
            }
        }
        Brush::Pinch => pinch(mesh, &hit, center, area, strength * dir),
        Brush::Crease => {
            /* Digs a groove (the inverted brush raises a ridge) and pulls the sides in. */
            for &(i, f) in &hit {
                mesh.verts[i] = add(mesh.verts[i], scale(area, -push * f * dir));
            }
            pinch(mesh, &hit, center, area, strength * dir);
        }
        Brush::Clay => {
            /* `do_clay_strips_brush()`, simplified: builds up towards a plane a little above the surface, so
             * only what lies below it rises (the inverted brush cuts what lies above a plane below it). */
            let (mut sum, mut wsum) = ([0.0f32; 3], 0.0f32);
            for &(i, f) in &hit {
                sum = add(sum, scale(mesh.verts[i], f));
                wsum += f;
            }
            let plane = add(scale(sum, 1.0 / wsum.max(1e-12)), scale(area, push * 0.5 * dir));
            for &(i, f) in &hit {
                let dist = dot(sub(mesh.verts[i], plane), area) * dir;
                if dist < 0.0 {
                    mesh.verts[i] = sub(mesh.verts[i], scale(area, dist * dir * (f * strength * BLEND_FACTOR).min(1.0)));
                }
            }
        }
        Brush::Grab => {}
    }
}

fn pinch(mesh: &mut Mesh, hit: &[(usize, f32)], center: V3, normal: V3, amount: f32) {
    for &(i, f) in hit {
        let d = sub(center, mesh.verts[i]);
        let tangent = sub(d, scale(normal, dot(d, normal)));
        mesh.verts[i] = add(mesh.verts[i], scale(tangent, f * amount * PINCH_FACTOR));
    }
}

/// The vertices a Grab stroke holds (`sculpt_grab`): fixed when the stroke starts, moved by the mouse delta.
#[derive(Clone, Debug)]
struct Grab {
    passes: Vec<([f32; 3], Vec<(usize, f32)>)>,
    orig: Vec<V3>,
    start_px: [f32; 2],
    depth: f32,
}

impl Grab {
    fn begin(mesh: &Mesh, center: V3, radius: f32, symmetry: [bool; 3], start_px: [f32; 2], depth: f32) -> Grab {
        let mut taken = vec![false; mesh.verts.len()];
        let mut passes = Vec::new();
        for s in mirror_signs(symmetry) {
            let mut set = affected(mesh, mirrored(center, s), radius);
            /* A vertex that two mirrors both reach (on the plane) moves with the first. */
            set.retain(|&(i, _)| !std::mem::replace(&mut taken[i], true));
            passes.push((s, set));
        }
        Grab { passes, orig: mesh.verts.clone(), start_px, depth }
    }

    fn apply(&self, mesh: &mut Mesh, delta: V3) {
        for (s, set) in &self.passes {
            let d = mirrored(delta, *s);
            for &(i, f) in set {
                mesh.verts[i] = add(self.orig[i], scale(d, f));
            }
        }
    }
}

/// The running stroke.
#[derive(Clone, Debug)]
struct Active {
    id: u32,
    adj: Vec<Vec<u32>>,
    brush: Brush,
    invert: bool,
    last: [f32; 2],
    /// Distance walked since the last dab.
    carry: f32,
    grab: Option<Grab>,
    world: Mat4,
    inv: Mat4,
}

/// The radius / strength modal (`F` / `Shift + F`, `paint.brush_radial_control`).
#[derive(Clone, Copy, Debug)]
pub struct Adjust {
    pub strength: bool,
    /// The window position of the cursor when the key was pressed.
    pub start: [i32; 2],
    init_radius: f32,
    init_strength: f32,
}

impl Adjust {
    /// The window position of the ring centre while adjusting the radius: the cursor sits on its edge.
    pub fn center(&self) -> [i32; 2] {
        [self.start[0] - self.init_radius as i32, self.start[1]]
    }
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub world: V3,
    pub local: V3,
}

/// The nearest front facing hit of the pixel ray `px` (region pixels, y up) with the base mesh of `obj`.
pub fn cast(obj: &Object, rv3d: &RegionView3D, rw: i32, rh: i32, px: [f32; 2]) -> Option<Hit> {
    let mesh = obj.mesh()?;
    let near = rv3d.unproject_v3(rw, rh, px[0], px[1], 0.0)?;
    let far = rv3d.unproject_v3(rw, rh, px[0], px[1], 1.0)?;
    let dir = normalized(sub(far, near));
    if dot(dir, dir) == 0.0 {
        return None;
    }
    let world = obj.world_matrix();
    let pts: Vec<V3> = mesh.verts.iter().map(|&v| mat4_mul_point(&world, v)).collect();
    let mut best: Option<f32> = None;
    for (_, [a, b, c]) in mesh.triangles() {
        let (pa, pb, pc) = (pts[a as usize], pts[b as usize], pts[c as usize]);
        if dot(cross(sub(pb, pa), sub(pc, pa)), dir) >= 0.0 {
            continue;
        }
        if let Some(t) = ray_triangle(near, dir, pa, pb, pc) {
            if best.map_or(true, |bt| t < bt) {
                best = Some(t);
            }
        }
    }
    let t = best?;
    let hit = add(near, scale(dir, t));
    let inv = invert_m4_m4(&world);
    Some(Hit { world: hit, local: mat4_mul_point(&inv, hit) })
}

/// The window depth (0..1) of a world position.
fn window_depth(rv3d: &RegionView3D, co: V3) -> f32 {
    let p = &rv3d.persmat;
    let z = p[0][2] * co[0] + p[1][2] * co[1] + p[2][2] * co[2] + p[3][2];
    let w = p[0][3] * co[0] + p[1][3] * co[1] + p[2][3] * co[2] + p[3][3];
    if w == 0.0 {
        0.5
    } else {
        (z / w + 1.0) * 0.5
    }
}

/// Sculpt Mode of the session: the settings, the running stroke and the running radius / strength modal.
#[derive(Clone, Debug, Default)]
pub struct Session {
    pub settings: Settings,
    stroke: Option<Active>,
    pub adjust: Option<Adjust>,
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    pub fn in_stroke(&self) -> bool {
        self.stroke.is_some()
    }

    /// `sculpt.brush_stroke` on the press: the first dab. Returns whether the mesh changed.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_stroke(&mut self, scene: &mut Scene, rv3d: &RegionView3D, rw: i32, rh: i32, mval: [i32; 2], invert: bool, smooth_held: bool) -> bool {
        let Some(id) = scene.active else { return false };
        let Some(index) = scene.index_of(id) else { return false };
        let px = [mval[0] as f32 + 0.5, mval[1] as f32 + 0.5];
        let obj = &scene.objects[index];
        let Some(hit) = cast(obj, rv3d, rw, rh, px) else { return false };
        let (world, inv) = (obj.world_matrix(), invert_m4_m4(&obj.world_matrix()));
        let adj = match obj.mesh() {
            Some(m) => adjacency(m),
            None => return false,
        };
        let brush = if smooth_held { Brush::Smooth } else { self.settings.brush };
        scene.push_undo();
        let mut active = Active { id, adj, brush, invert, last: px, carry: 0.0, grab: None, world, inv };
        let changed = if brush == Brush::Grab {
            let radius = self.local_radius(rv3d, &active, hit.world);
            let depth = window_depth(rv3d, hit.world);
            if let Some(mesh) = scene.sculpt_mesh_mut(id) {
                active.grab = Some(Grab::begin(mesh, hit.local, radius, self.settings.symmetry, px, depth));
            }
            true
        } else {
            self.dab_at(scene, rv3d, &active, &hit)
        };
        self.stroke = Some(active);
        changed
    }

    fn local_radius(&self, rv3d: &RegionView3D, a: &Active, world_hit: V3) -> f32 {
        let world_r = self.settings.radius_px * rv3d.pixel_size_no_ui_scale(world_hit);
        let s = (0..3).map(|k| {
            let mut e = [0.0f32; 3];
            e[k] = 1.0;
            let d = mat4_mul_dir(&a.inv, e);
            dot(d, d).sqrt()
        });
        world_r * s.sum::<f32>() / 3.0
    }

    fn dab_at(&self, scene: &mut Scene, rv3d: &RegionView3D, a: &Active, hit: &Hit) -> bool {
        let radius = self.local_radius(rv3d, a, hit.world);
        let Some(mesh) = scene.sculpt_mesh_mut(a.id) else { return false };
        dab(mesh, &a.adj, a.brush, hit.local, radius, self.settings.strength, a.invert, self.settings.symmetry);
        true
    }

    /// The mouse moved during a stroke: dabs along the path (or the grab delta). Returns whether the mesh changed.
    pub fn move_stroke(&mut self, scene: &mut Scene, rv3d: &RegionView3D, rw: i32, rh: i32, mval: [i32; 2]) -> bool {
        let Some(mut a) = self.stroke.take() else { return false };
        let cur = [mval[0] as f32 + 0.5, mval[1] as f32 + 0.5];
        let mut changed = false;
        if let Some(grab) = a.grab.as_ref() {
            let p0 = rv3d.unproject_v3(rw, rh, grab.start_px[0], grab.start_px[1], grab.depth);
            let p1 = rv3d.unproject_v3(rw, rh, cur[0], cur[1], grab.depth);
            if let (Some(p0), Some(p1)) = (p0, p1) {
                let delta = mat4_mul_dir(&a.inv, sub(p1, p0));
                if let Some(mesh) = scene.sculpt_mesh_mut(a.id) {
                    grab.apply(mesh, delta);
                    changed = true;
                }
            }
            a.last = cur;
        } else {
            let spacing = (self.settings.radius_px * 2.0 * SPACING).max(1.0);
            let d = ((cur[0] - a.last[0]).powi(2) + (cur[1] - a.last[1]).powi(2)).sqrt();
            if d > 0.0 {
                let dir = [(cur[0] - a.last[0]) / d, (cur[1] - a.last[1]) / d];
                let mut travelled = spacing - a.carry;
                let mut dabs = 0;
                while travelled <= d && dabs < MAX_DABS_PER_EVENT {
                    let p = [a.last[0] + dir[0] * travelled, a.last[1] + dir[1] * travelled];
                    let obj = scene.index_of(a.id).map(|i| &scene.objects[i]);
                    if let Some(hit) = obj.and_then(|o| cast(o, rv3d, rw, rh, p)) {
                        changed |= self.dab_at(scene, rv3d, &a, &hit);
                    }
                    travelled += spacing;
                    dabs += 1;
                }
                a.carry = if dabs >= MAX_DABS_PER_EVENT { 0.0 } else { d - (travelled - spacing) };
                a.last = cur;
            }
        }
        self.stroke = Some(a);
        changed
    }

    pub fn end_stroke(&mut self) {
        self.stroke = None;
    }

    /// `F` / `Shift + F`: starts the radius / strength modal at the window position `cursor`.
    pub fn start_adjust(&mut self, strength: bool, cursor: [i32; 2]) {
        self.adjust = Some(Adjust { strength, start: cursor, init_radius: self.settings.radius_px, init_strength: self.settings.strength });
    }

    /// The cursor moved during the modal.
    pub fn adjust_move(&mut self, cursor: [i32; 2]) {
        let Some(a) = self.adjust else { return };
        if a.strength {
            let dx = (cursor[0] - a.start[0]) as f32;
            self.settings.strength = (a.init_strength + dx / 200.0).clamp(0.0, 1.0);
        } else {
            let c = a.center();
            let (dx, dy) = ((cursor[0] - c[0]) as f32, (cursor[1] - c[1]) as f32);
            self.settings.radius_px = (dx * dx + dy * dy).sqrt().clamp(MIN_RADIUS_PX, MAX_RADIUS_PX);
        }
    }

    /// Confirms (`cancel` false) or cancels the modal.
    pub fn adjust_end(&mut self, cancel: bool) {
        if cancel {
            if let Some(a) = self.adjust {
                self.settings.radius_px = a.init_radius;
                self.settings.strength = a.init_strength;
            }
        }
        self.adjust = None;
    }

    /// Alt + X / Y / Z.
    pub fn toggle_symmetry(&mut self, axis: usize) {
        self.settings.symmetry[axis] = !self.settings.symmetry[axis];
    }

    /// The brush ring of `paint_draw_cursor()`: the outer circle at the radius and an inner one at the radius
    /// times the strength; the inverted brush is light blue (`cursor_color_subtract`), the add colour is white.
    /// `cursor` is the window position; `region` the viewport rect.
    pub fn draw_cursor(&self, buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef, cursor: [i32; 2], invert: bool) {
        let size = (region.size_x() + 1, region.size_y() + 1);
        let center = match self.adjust {
            Some(a) if !a.strength => a.center(),
            _ => cursor,
        };
        let c = [(center[0] - region.xmin) as f32 + 0.5, (center[1] - region.ymin) as f32 + 0.5];
        let (outer, inner) = if invert { ([0.5, 0.5, 1.0], [0.3, 0.3, 0.7]) } else { ([1.0, 1.0, 1.0], [0.6, 0.6, 0.6]) };
        let lw = user.pixelsize as f32;
        let ring = |buf: &mut [u32], r: f32, color: [f32; 3]| {
            const SEG: usize = 64;
            let pt = |i: usize| {
                let a = std::f32::consts::TAU * i as f32 / SEG as f32;
                [c[0] + r * a.cos(), c[1] + r * a.sin()]
            };
            for i in 0..SEG {
                draw_segment(buf, width, height, region, size, pt(i), pt(i + 1), color, lw);
            }
        };
        ring(buf, self.settings.radius_px, outer);
        let inner_r = self.settings.radius_px * self.settings.strength;
        if inner_r >= 2.0 {
            ring(buf, inner_r, inner);
        }
    }

    /// The line of the viewport text (`Sculpt Mode  Draw  Radius 50 px  Strength 0.50  Sym X`).
    pub fn status_text(&self) -> String {
        let s = &self.settings;
        let mut sym = String::new();
        for (i, name) in ["X", "Y", "Z"].iter().enumerate() {
            if s.symmetry[i] {
                sym.push_str(name);
            }
        }
        let sym = if sym.is_empty() { "off".to_string() } else { sym };
        format!("Sculpt Mode | {} | Radius {:.0} px | Strength {:.2} | Symmetry {}", s.brush.label(), s.radius_px, s.strength, sym)
    }

    pub fn draw_status(&self, buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef) {
        let text = self.status_text();
        let w = font::label_width(user, &text);
        let h = user.widget_unit;
        let rect = Rcti { xmin: region.xmin + 4, xmax: region.xmin + 4 + w, ymin: region.ymin + 4, ymax: region.ymin + 4 + h };
        font::draw_label(buf, width, height, user, rect, &text, Rgba::hex(0xffffffcc));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Mesh {
        Mesh::grid(20, 20, 2.0)
    }

    fn height_at(m: &Mesh, x: f32, y: f32) -> f32 {
        m.verts.iter().min_by(|a, b| ((a[0] - x).powi(2) + (a[1] - y).powi(2)).partial_cmp(&((b[0] - x).powi(2) + (b[1] - y).powi(2))).unwrap()).unwrap()[2]
    }

    #[test]
    fn falloff_is_smooth() {
        assert_eq!(falloff(0.0, 1.0), 1.0);
        assert_eq!(falloff(1.0, 1.0), 0.0);
        assert!((falloff(0.5, 1.0) - 0.5).abs() < 1e-6);
        assert_eq!(falloff(2.0, 1.0), 0.0);
    }

    #[test]
    fn draw_raises_and_inverted_draw_digs() {
        let adj;
        let mut m = grid();
        adj = adjacency(&m);
        let up = m.vertex_normals()[0][2];
        let sign = if up >= 0.0 { 1.0 } else { -1.0 };
        dab(&mut m, &adj, Brush::Draw, [0.0, 0.0, 0.0], 0.5, 1.0, false, [false; 3]);
        assert!(height_at(&m, 0.0, 0.0) * sign > 0.01);
        assert_eq!(height_at(&m, 0.9, 0.9), 0.0, "outside the radius");
        let mut m2 = grid();
        dab(&mut m2, &adj, Brush::Draw, [0.0, 0.0, 0.0], 0.5, 1.0, true, [false; 3]);
        assert!(height_at(&m2, 0.0, 0.0) * sign < -0.01);
    }

    #[test]
    fn symmetry_mirrors_the_dab() {
        let mut m = grid();
        let adj = adjacency(&m);
        dab(&mut m, &adj, Brush::Draw, [0.5, 0.0, 0.0], 0.3, 1.0, false, [true, false, false]);
        let (a, b) = (height_at(&m, 0.5, 0.0), height_at(&m, -0.5, 0.0));
        assert!(a != 0.0 && (a - b).abs() < 1e-6);
    }

    #[test]
    fn smooth_flattens_a_bump() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        m.verts[i][2] = 0.3;
        for _ in 0..4 {
            dab(&mut m, &adj, Brush::Smooth, [0.0, 0.0, 0.0], 0.5, 1.0, false, [false; 3]);
        }
        assert!(m.verts[i][2] < 0.2);
    }

    #[test]
    fn flatten_pulls_towards_the_plane() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        m.verts[i][2] = 0.2;
        dab(&mut m, &adj, Brush::Flatten, [0.0, 0.0, 0.0], 0.5, 1.0, false, [false; 3]);
        assert!(m.verts[i][2].abs() < 0.2);
    }

    #[test]
    fn clay_fills_a_dent_and_leaves_a_bump() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        let up = m.vertex_normals()[i][2].signum();
        m.verts[i][2] = -0.2 * up;
        dab(&mut m, &adj, Brush::Clay, [0.0, 0.0, 0.0], 0.5, 1.0, false, [false; 3]);
        assert!((m.verts[i][2] * up).abs() < 0.2, "the dent is filled");
        let mut b = grid();
        b.verts[i][2] = 0.2 * up;
        dab(&mut b, &adj, Brush::Clay, [0.0, 0.0, 0.0], 0.5, 1.0, false, [false; 3]);
        assert_eq!(b.verts[i][2], 0.2 * up, "a bump above the plane is left alone");
    }

    #[test]
    fn pinch_moves_vertices_to_the_centre() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| (v[0] - 0.2).abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        dab(&mut m, &adj, Brush::Pinch, [0.0, 0.0, 0.0], 0.5, 1.0, false, [false; 3]);
        assert!(m.verts[i][0] < 0.2);
    }

    #[test]
    fn grab_moves_the_held_vertices() {
        let m0 = grid();
        let mut m = m0.clone();
        let g = Grab::begin(&m0, [0.0, 0.0, 0.0], 0.5, [false; 3], [0.0, 0.0], 0.5);
        g.apply(&mut m, [0.0, 0.0, 1.0]);
        assert!((height_at(&m, 0.0, 0.0) - 1.0).abs() < 1e-5);
        assert_eq!(height_at(&m, 0.9, 0.9), 0.0);
        g.apply(&mut m, [0.0, 0.0, 0.0]);
        assert_eq!(m, m0);
    }

    #[test]
    fn adjust_sets_radius_and_cancel_restores_it() {
        let mut s = Session::new();
        s.start_adjust(false, [300, 200]);
        s.adjust_move([400, 200]);
        assert!((s.settings.radius_px - 150.0).abs() < 1.0);
        s.adjust_end(true);
        assert_eq!(s.settings.radius_px, 50.0);
        s.start_adjust(true, [300, 200]);
        s.adjust_move([340, 200]);
        assert!((s.settings.strength - 0.7).abs() < 1e-6);
        s.adjust_end(false);
        assert!((s.settings.strength - 0.7).abs() < 1e-6);
    }

    #[test]
    fn status_text_names_the_brush() {
        let s = Session::new();
        assert!(s.status_text().contains("Draw") && s.status_text().contains("Symmetry X"));
    }

    /* A whole stroke through the scene: the press on the sphere lifts it under the cursor, the drag goes on
     * and one undo step takes the stroke back. */
    #[test]
    fn a_stroke_deforms_the_mesh_and_undoes_in_one_step() {
        use crate::mesh::Primitive;
        use crate::view3d::View3D;
        let mut scene = Scene::startup();
        let id = scene.add_primitive(Primitive::UvSphere, [0.0, 0.0, 0.0]);
        scene.active = Some(id);
        let before = scene.active_object().unwrap().mesh().unwrap().clone();
        let (w, h) = (400, 300);
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w, h);
        let mut session = Session::new();
        session.settings.radius_px = 40.0;
        assert!(session.begin_stroke(&mut scene, &rv3d, w, h, [w / 2, h / 2], false, false));
        assert!(session.in_stroke());
        session.move_stroke(&mut scene, &rv3d, w, h, [w / 2 + 30, h / 2]);
        session.end_stroke();
        let after = scene.active_object().unwrap().mesh().unwrap().clone();
        assert_ne!(before, after);
        assert!(scene.undo());
        assert_eq!(scene.active_object().unwrap().mesh().unwrap(), &before);
    }

    #[test]
    fn a_press_on_nothing_does_not_stroke() {
        use crate::view3d::View3D;
        let mut scene = Scene::startup();
        let (w, h) = (400, 300);
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w, h);
        let mut session = Session::new();
        assert!(!session.begin_stroke(&mut scene, &rv3d, w, h, [2, 2], false, false));
        assert!(!session.in_stroke() && !scene.can_undo());
    }
}
