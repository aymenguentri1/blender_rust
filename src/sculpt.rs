//! Sculpt Mode (`editors/sculpt_paint/sculpt.cc`, `paint_stroke.cc`), the part that is modelled here.
//!
//! A stroke is a series of dabs placed every `spacing` (10 % of the brush diameter) along the mouse path, each
//! one found by casting the pixel ray against the base mesh of the active object. A dab moves the vertices
//! inside the brush radius by `strength * falloff(distance)`, with the Smooth curve preset of the brush
//! (`3 t^2 - 2 t^3`, `t = 1 - distance / radius`).
//!
//! Brushes: Draw, Smooth, Grab, Inflate, Flatten, Pinch, Crease, Clay Strips, Fill, Scrape, Mask, Layer, Snake Hook, Nudge and Blob. The mask
//! (`Object::mask`, 0 to 1 per vertex) scales every brush down by `1 - mask`, so a masked vertex stays put, and is
//! painted by the Mask brush (Ctrl erases), cleared with Alt + M and inverted with Ctrl + I. Ctrl inverts the brush (the ring turns light
//! blue like `cursor_color_subtract`), Shift smooths while it is held. Symmetry mirrors every dab across the
//! local X / Y / Z planes (X is on, as in a new Blender sculpt session).
//!
//! Deviations: the displacement factors are tuned by eye (Blender derives them from the brush overlap), there is
//! no pressure, face sets, dynamic topology or multires, the brush curve is always Smooth, and a mesh
//! with modifiers is sculpted on its base mesh (Blender does the same, but shows the result through the stack).

use crate::area::Rcti;
use crate::font;
use crate::mesh::{add, cross, dot, normalized, scale, sub, Mesh, V3};
use crate::overlay_cursor::draw_segment;
use crate::scene::{mat4_mul_dir, mat4_mul_point, ray_triangle, Object, Scene};
use crate::theme::Rgba;
use crate::userdef::UserDef;
use crate::sculpt_enhance;
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
    Fill,
    Scrape,
    Mask,
    Layer,
    SnakeHook,
    Nudge,
    Blob,
    Thumb,
    DrawSharp,
}

impl Brush {
    #[allow(dead_code)]
    pub const ALL: [Brush; 17] = [Brush::Draw, Brush::Smooth, Brush::Grab, Brush::Inflate, Brush::Flatten, Brush::Pinch, Brush::Crease, Brush::Clay, Brush::Fill, Brush::Scrape, Brush::Mask, Brush::Layer, Brush::SnakeHook, Brush::Nudge, Brush::Blob, Brush::Thumb, Brush::DrawSharp];

    /// The key of the brush in Sculpt Mode, as the brush popup lists it (`Shift X` for `Shift + X`).
    pub fn key_label(self) -> &'static str {
        match self {
            Brush::Draw => "X",
            Brush::Smooth => "S",
            Brush::Grab => "G",
            Brush::Inflate => "I",
            Brush::Flatten => "Shift T",
            Brush::Pinch => "P",
            Brush::Crease => "Shift C",
            Brush::Clay => "C",
            Brush::Fill => "Shift X",
            Brush::Scrape => "Shift S",
            Brush::Mask => "M",
            Brush::Layer => "L",
            Brush::SnakeHook => "K",
            Brush::Nudge => "N",
            Brush::Blob => "Shift B",
            Brush::Thumb | Brush::DrawSharp => "",
        }
    }

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
            Brush::Fill => "Fill",
            Brush::Scrape => "Scrape",
            Brush::Mask => "Mask",
            Brush::Layer => "Layer",
            Brush::SnakeHook => "Snake Hook",
            Brush::Nudge => "Nudge",
            Brush::Blob => "Blob",
            Brush::Thumb => "Thumb",
            Brush::DrawSharp => "Draw Sharp",
        }
    }
}

/// Displacement of a Draw dab at full strength and falloff, in brush radii.
const DRAW_FACTOR: f32 = 0.06;
/// Part of the distance to the target (plane, neighbour average, centre) a dab closes at full strength.
const BLEND_FACTOR: f32 = 0.3;
const PINCH_FACTOR: f32 = 0.2;
/// Mask painted by a Mask dab at full strength and falloff.
const MASK_FACTOR: f32 = 0.25;
/// Height a Layer stroke raises the surface to, in brush radii at full strength.
const LAYER_HEIGHT: f32 = 0.25;
/// Distance a Nudge dab drags the surface along the stroke, in brush radii at full strength and falloff.
const NUDGE_FACTOR: f32 = 0.15;
/// Displacement of a Blob dab, relative to a Draw dab.
const BLOB_FACTOR: f32 = 1.5;
/// `Brush::spacing` (percent of the brush diameter) of the dotted brushes.
const SPACING: f32 = 0.10;
pub const MIN_RADIUS_PX: f32 = 5.0;
pub const MAX_RADIUS_PX: f32 = 500.0;
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
    /// `Brush::direction` = SUBTRACT (the Subtract button of the tool header): the brush works inverted, as if
    /// Ctrl were held.
    pub subtract: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { brush: Brush::Draw, radius_px: 50.0, strength: 0.5, symmetry: [true, false, false], subtract: false }
    }
}

/// The mask of vertex `i` (0 where the mask is shorter than the mesh, i.e. absent).
fn mask_at(mask: &[f32], i: usize) -> f32 {
    mask.get(i).copied().unwrap_or(0.0)
}

/// The vertices of `mesh` within `radius` of `center`, with their falloff times `1 - mask`.
pub(crate) fn affected(mesh: &Mesh, mask: &[f32], center: V3, radius: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    for (i, &v) in mesh.verts.iter().enumerate() {
        let d = sub(v, center);
        let d2 = dot(d, d);
        if d2 < radius * radius {
            let f = falloff(d2.sqrt(), radius) * (1.0 - mask_at(mask, i)).clamp(0.0, 1.0);
            if f > 0.0 {
                out.push((i, f));
            }
        }
    }
    out
}

/// The topology a stroke needs, built once when it starts (the dabs only move vertices): the vertex neighbours
/// through the edges of the faces, and the faces around each vertex (for normals of just the vertices under a dab).
#[derive(Clone, Debug, Default)]
pub struct Topo {
    pub adj: Vec<Vec<u32>>,
    pub vfaces: Vec<Vec<u32>>,
}

pub fn adjacency(mesh: &Mesh) -> Topo {
    let mut adj = vec![Vec::<u32>::new(); mesh.verts.len()];
    let mut vfaces = vec![Vec::<u32>::new(); mesh.verts.len()];
    for (f, face) in mesh.faces.iter().enumerate() {
        for &v in face {
            vfaces[v as usize].push(f as u32);
        }
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
    Topo { adj, vfaces }
}

/// The smooth shading normal of vertex `i` (what `Mesh::vertex_normals()` gives, for one vertex).
fn vertex_normal(mesh: &Mesh, topo: &Topo, i: usize) -> V3 {
    let mut n = [0.0f32; 3];
    for &f in &topo.vfaces[i] {
        n = add(n, mesh.face_area_normal(f as usize));
    }
    normalized(n)
}

/// The mirror combinations of the enabled axes (`+++` first), as sign triples.
pub(crate) fn mirror_signs(sym: [bool; 3]) -> Vec<[f32; 3]> {
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

pub(crate) fn mirrored(v: V3, s: [f32; 3]) -> V3 {
    [v[0] * s[0], v[1] * s[1], v[2] * s[2]]
}

/// One dab of `brush` (not Grab) at `center`, in the local space of `mesh`, with every mirror of it, on a mesh
/// without a mask.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub fn dab(mesh: &mut Mesh, adj: &Topo, brush: Brush, center: V3, radius: f32, strength: f32, invert: bool, symmetry: [bool; 3]) {
    let mut mask = vec![0.0; mesh.verts.len()];
    dab_masked(mesh, &mut mask, adj, brush, center, radius, strength, invert, symmetry);
}

/// `dab()` on a mesh with a mask (one value per vertex); the Mask brush paints it, the others are scaled by it.
#[allow(clippy::too_many_arguments)]
pub fn dab_masked(mesh: &mut Mesh, mask: &mut [f32], adj: &Topo, brush: Brush, center: V3, radius: f32, strength: f32, invert: bool, symmetry: [bool; 3]) {
    if matches!(brush, Brush::Grab | Brush::SnakeHook | Brush::Layer | Brush::Nudge | Brush::Thumb | Brush::DrawSharp) {
        return;
    }
    for s in mirror_signs(symmetry) {
        dab_one(mesh, mask, adj, brush, mirrored(center, s), radius, strength, invert);
    }
}

#[allow(clippy::too_many_arguments)]
fn dab_one(mesh: &mut Mesh, mask: &mut [f32], adj: &Topo, brush: Brush, center: V3, radius: f32, strength: f32, invert: bool) {
    if brush == Brush::Mask {
        /* `do_mask_brush()`: paints the mask up (Ctrl: erases it) under the brush, the mask itself is not masked. */
        let amount = strength * MASK_FACTOR * if invert { -1.0 } else { 1.0 };
        for (i, f) in affected(mesh, &[], center, radius) {
            if let Some(m) = mask.get_mut(i) {
                *m = (*m + f * amount).clamp(0.0, 1.0);
            }
        }
        return;
    }
    let hit = affected(mesh, mask, center, radius);
    if hit.is_empty() {
        return;
    }
    let dir = if invert { -1.0 } else { 1.0 };
    /* Only the vertices under the brush need a normal. */
    let normals: Vec<V3> = hit.iter().map(|&(i, _)| vertex_normal(mesh, adj, i)).collect();
    /* `calc_area_normal()`: the falloff weighted mean of the vertex normals under the brush. */
    let mut area = [0.0f32; 3];
    for (k, &(_, f)) in hit.iter().enumerate() {
        area = add(area, scale(normals[k], f));
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
            for (k, &(i, f)) in hit.iter().enumerate() {
                mesh.verts[i] = add(mesh.verts[i], scale(normals[k], push * f * dir));
            }
        }
        Brush::Smooth => {
            let moved: Vec<(usize, V3)> = hit
                .iter()
                .filter(|&&(i, _)| !adj.adj[i].is_empty())
                .map(|&(i, f)| {
                    let mut sum = [0.0f32; 3];
                    for &n in &adj.adj[i] {
                        sum = add(sum, mesh.verts[n as usize]);
                    }
                    let avg = scale(sum, 1.0 / adj.adj[i].len() as f32);
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
        Brush::Fill | Brush::Scrape => {
            /* `do_fill_brush()` / `do_scrape_brush()`: Flatten that only raises what lies below the plane (Fill)
             * or only lowers what lies above it (Scrape); the inverted brush swaps the two. */
            let (mut sum, mut wsum) = ([0.0f32; 3], 0.0f32);
            for &(i, f) in &hit {
                sum = add(sum, scale(mesh.verts[i], f));
                wsum += f;
            }
            let plane = scale(sum, 1.0 / wsum.max(1e-12));
            let fill = (brush == Brush::Fill) != invert;
            for &(i, f) in &hit {
                let dist = dot(sub(mesh.verts[i], plane), area);
                if (fill && dist < 0.0) || (!fill && dist > 0.0) {
                    mesh.verts[i] = sub(mesh.verts[i], scale(area, dist * (f * strength * BLEND_FACTOR).min(1.0)));
                }
            }
        }
        Brush::Blob => {
            /* `do_blob_brush()` (simplified): inflates with a push away from the dab centre as well, so the
             * surface swells into a ball instead of a plateau. */
            for (k, &(i, f)) in hit.iter().enumerate() {
                let radial = normalized(sub(mesh.verts[i], center));
                let d = normalized(add(normals[k], scale(radial, 0.5)));
                mesh.verts[i] = add(mesh.verts[i], scale(d, push * BLOB_FACTOR * f * dir));
            }
        }
        Brush::Grab | Brush::Mask | Brush::Layer | Brush::SnakeHook | Brush::Nudge | Brush::Thumb | Brush::DrawSharp => {}
    }
}

/// The falloff weighted mean of the vertex normals under a dab (`calc_area_normal()`).
fn area_normal(mesh: &Mesh, adj: &Topo, hit: &[(usize, f32)]) -> V3 {
    let mut area = [0.0f32; 3];
    for &(i, f) in hit {
        area = add(area, scale(vertex_normal(mesh, adj, i), f));
    }
    normalized(area)
}

/// One Nudge dab (`do_nudge_brush()`): drags the vertices under the brush along `dir`, the stroke direction in
/// the local space of the mesh, projected onto the surface.
#[allow(clippy::too_many_arguments)]
pub fn nudge(mesh: &mut Mesh, mask: &[f32], adj: &Topo, center: V3, radius: f32, strength: f32, dir: V3, symmetry: [bool; 3]) {
    for s in mirror_signs(symmetry) {
        let hit = affected(mesh, mask, mirrored(center, s), radius);
        if hit.is_empty() {
            continue;
        }
        let area = area_normal(mesh, adj, &hit);
        let d = mirrored(dir, s);
        let tangent = sub(d, scale(area, dot(d, area)));
        for &(i, f) in &hit {
            mesh.verts[i] = add(mesh.verts[i], scale(tangent, radius * strength * NUDGE_FACTOR * f));
        }
    }
}

/// One Thumb dab (`do_thumb_brush()`): Nudge that picks the vertices and their falloff from the surface as it was
/// when the stroke began (`orig`), so the brush drags the same patch along instead of sweeping up new vertices.
/// The offset is the stroke direction projected onto the surface, `cross(cross(normal, delta), normal)`.
#[allow(clippy::too_many_arguments)]
pub fn thumb(mesh: &mut Mesh, orig: &[V3], mask: &[f32], adj: &Topo, center: V3, radius: f32, strength: f32, dir: V3, symmetry: [bool; 3]) {
    for s in mirror_signs(symmetry) {
        let c = mirrored(center, s);
        let mut hit = Vec::new();
        for (i, &v) in orig.iter().enumerate().take(mesh.verts.len()) {
            let d = sub(v, c);
            let d2 = dot(d, d);
            if d2 < radius * radius {
                let f = falloff(d2.sqrt(), radius) * (1.0 - mask_at(mask, i)).clamp(0.0, 1.0);
                if f > 0.0 {
                    hit.push((i, f));
                }
            }
        }
        if hit.is_empty() {
            continue;
        }
        let area = area_normal(mesh, adj, &hit);
        let d = mirrored(dir, s);
        let tangent = sub(d, scale(area, dot(d, area)));
        for &(i, f) in &hit {
            mesh.verts[i] = add(mesh.verts[i], scale(tangent, radius * strength * NUDGE_FACTOR * f));
        }
    }
}

/// The state of a Layer stroke (`SCULPT_vertex_layer_disp`): the surface as it was when the stroke began and how
/// far each vertex has been raised along its original normal. The raise is capped, so the stroke builds a plateau
/// of constant height however often it passes.
#[derive(Clone, Debug)]
struct Layer {
    orig: Vec<V3>,
    normals: Vec<V3>,
    disp: Vec<f32>,
}

impl Layer {
    fn begin(mesh: &Mesh) -> Layer {
        Layer { orig: mesh.verts.clone(), normals: mesh.vertex_normals(), disp: vec![0.0; mesh.verts.len()] }
    }

    /// One Layer dab (`do_layer_brush()`); the inverted brush digs a layer instead.
    #[allow(clippy::too_many_arguments)]
    fn dab(&mut self, mesh: &mut Mesh, mask: &[f32], center: V3, radius: f32, strength: f32, invert: bool, symmetry: [bool; 3]) {
        let dir = if invert { -1.0 } else { 1.0 };
        let max = radius * strength * LAYER_HEIGHT;
        let step = radius * strength * DRAW_FACTOR;
        for s in mirror_signs(symmetry) {
            for (i, f) in affected(mesh, mask, mirrored(center, s), radius) {
                let d = (self.disp[i] + step * f * dir).clamp(-max, max);
                /* A vertex only ever moves one way during a stroke (the other direction cannot undo it). */
                let d = if invert { d.min(0.0).min(self.disp[i]) } else { d.max(0.0).max(self.disp[i]) };
                self.disp[i] = d;
                mesh.verts[i] = add(self.orig[i], scale(self.normals[i], d));
            }
        }
    }
}

/// The surface when a Draw Sharp stroke began (`do_draw_sharp_brush()`, `orig_position_data`).
#[derive(Clone, Debug)]
struct Sharp {
    orig: Vec<V3>,
    normals: Vec<V3>,
}

impl Sharp {
    fn begin(mesh: &Mesh) -> Sharp {
        Sharp { orig: mesh.verts.clone(), normals: mesh.vertex_normals() }
    }

    /// Draw whose falloff and normal come from the stroke start surface, so the crease stays sharp however far the
    /// surface has moved. Dabs still add up, as in Blender.
    #[allow(clippy::too_many_arguments)]
    fn dab(&self, mesh: &mut Mesh, mask: &[f32], center: V3, radius: f32, strength: f32, invert: bool, symmetry: [bool; 3]) {
        let push = radius * strength * DRAW_FACTOR * if invert { -1.0 } else { 1.0 };
        for s in mirror_signs(symmetry) {
            let c = mirrored(center, s);
            let mut hit = Vec::new();
            let mut area = [0.0f32; 3];
            for (i, &v) in self.orig.iter().enumerate().take(mesh.verts.len()) {
                let d = sub(v, c);
                let d2 = dot(d, d);
                if d2 < radius * radius {
                    let f = falloff(d2.sqrt(), radius) * (1.0 - mask_at(mask, i)).clamp(0.0, 1.0);
                    if f > 0.0 {
                        area = add(area, scale(self.normals[i], f));
                        hit.push((i, f));
                    }
                }
            }
            let offset = scale(normalized(area), push);
            for (i, f) in hit {
                mesh.verts[i] = add(mesh.verts[i], scale(offset, f));
            }
        }
    }
}

/// The surface a Snake Hook stroke drags (`do_snake_hook_brush()`): the brush centre follows the mouse and pulls
/// the vertices under it along, so the surface stretches into a tentacle.
#[derive(Clone, Debug)]
struct Hook {
    center: V3,
    radius: f32,
}

impl Hook {
    /// Moves the brush by `delta` (local space), dragging the vertices under its previous position with it.
    fn apply(&mut self, mesh: &mut Mesh, mask: &[f32], delta: V3, symmetry: [bool; 3]) {
        let mut taken = vec![false; mesh.verts.len()];
        for s in mirror_signs(symmetry) {
            let d = mirrored(delta, s);
            for (i, f) in affected(mesh, mask, mirrored(self.center, s), self.radius) {
                /* A vertex that two mirrors both reach (on the plane) moves with the first. */
                if !std::mem::replace(&mut taken[i], true) {
                    mesh.verts[i] = add(mesh.verts[i], scale(d, f));
                }
            }
        }
        self.center = add(self.center, delta);
    }
}

fn pinch(mesh: &mut Mesh, hit: &[(usize, f32)], center: V3, normal: V3, amount: f32) {
    for &(i, f) in hit {
        let d = sub(center, mesh.verts[i]);
        let tangent = sub(d, scale(normal, dot(d, normal)));
        mesh.verts[i] = add(mesh.verts[i], scale(tangent, f * amount * PINCH_FACTOR));
    }
}

/// `sculpt.mask_filter(filter_type=...)`: what happens to the mask of each vertex, from its neighbours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskFilter {
    /// The mean of the neighbours.
    Smooth,
    /// `3 m^2 - 2 m^3`: pushes values towards 0 and 1.
    Sharpen,
    /// The largest of the vertex and its neighbours.
    Grow,
    /// The smallest of the vertex and its neighbours.
    Shrink,
}

/// One pass of `filter` over `mask` (one value per vertex of the mesh `adj` was built from).
pub fn filter_mask(mask: &mut [f32], adj: &Topo, filter: MaskFilter) {
    let old = mask.to_vec();
    for (i, m) in mask.iter_mut().enumerate() {
        let near = &adj.adj[i];
        *m = match filter {
            MaskFilter::Sharpen => old[i] * old[i] * (3.0 - 2.0 * old[i]),
            MaskFilter::Smooth if !near.is_empty() => near.iter().map(|&n| old[n as usize]).sum::<f32>() / near.len() as f32,
            MaskFilter::Grow => near.iter().fold(old[i], |a, &n| a.max(old[n as usize])),
            MaskFilter::Shrink => near.iter().fold(old[i], |a, &n| a.min(old[n as usize])),
            MaskFilter::Smooth => old[i],
        }
        .clamp(0.0, 1.0);
    }
}

/// One Grab pass: the brush centre and the (vertex, falloff) pairs it holds.
type GrabPass = ([f32; 3], Vec<(usize, f32)>);

/// The vertices a Grab stroke holds (`sculpt_grab`): fixed when the stroke starts, moved by the mouse delta.
#[derive(Clone, Debug)]
struct Grab {
    passes: Vec<GrabPass>,
    orig: Vec<V3>,
    start_px: [f32; 2],
    depth: f32,
}

impl Grab {
    fn begin(mesh: &Mesh, mask: &[f32], center: V3, radius: f32, symmetry: [bool; 3], start_px: [f32; 2], depth: f32) -> Grab {
        let mut taken = vec![false; mesh.verts.len()];
        let mut passes = Vec::new();
        for s in mirror_signs(symmetry) {
            let mut set = affected(mesh, mask, mirrored(center, s), radius);
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
    adj: Topo,
    brush: Brush,
    invert: bool,
    last: [f32; 2],
    /// Distance walked since the last dab.
    carry: f32,
    grab: Option<Grab>,
    layer: Option<Layer>,
    sharp: Option<Sharp>,
    hook: Option<Hook>,
    /// The surface when a Thumb stroke began.
    orig: Option<Vec<V3>>,
    /// Enhance Details (inverted Smooth): the directions taken when the stroke began.
    details: Option<Vec<V3>>,
    /// The stroke direction in the local space of the mesh (Nudge), from the last mouse movement.
    dir: V3,
    /// The window depth of the first hit: the mouse moves in the plane at that depth (Snake Hook, Nudge).
    depth: f32,
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

/// The nearest front facing hit of the pixel ray `px` (region pixels, y up) with the base mesh of `obj`. The ray is
/// moved into the local space of the mesh once, so no vertex is transformed, and the triangle fans are walked in place.
pub fn cast(obj: &Object, rv3d: &RegionView3D, rw: i32, rh: i32, px: [f32; 2]) -> Option<Hit> {
    let mesh = obj.mesh()?;
    let near = rv3d.unproject_v3(rw, rh, px[0], px[1], 0.0)?;
    let far = rv3d.unproject_v3(rw, rh, px[0], px[1], 1.0)?;
    let world = obj.world_matrix();
    let inv = invert_m4_m4(&world);
    let (o, f) = (mat4_mul_point(&inv, near), mat4_mul_point(&inv, far));
    let dir = sub(f, o);
    if dot(dir, dir) == 0.0 {
        return None;
    }
    let mut best: Option<f32> = None;
    for face in &mesh.faces {
        for k in 1..face.len().saturating_sub(1) {
            let (pa, pb, pc) = (mesh.verts[face[0] as usize], mesh.verts[face[k] as usize], mesh.verts[face[k + 1] as usize]);
            if dot(cross(sub(pb, pa), sub(pc, pa)), dir) >= 0.0 {
                continue;
            }
            if let Some(t) = ray_triangle(o, dir, pa, pb, pc) {
                if best.is_none_or(|bt| t < bt) {
                    best = Some(t);
                }
            }
        }
    }
    let local = add(o, scale(dir, best?));
    Some(Hit { world: mat4_mul_point(&world, local), local })
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
        let inv = invert_m4_m4(&obj.world_matrix());
        let adj = match obj.mesh() {
            Some(m) => adjacency(m),
            None => return false,
        };
        let brush = if smooth_held { Brush::Smooth } else { self.settings.brush };
        scene.push_undo();
        let depth = window_depth(rv3d, hit.world);
        let invert = invert != self.settings.subtract;
        let mut active = Active { id, adj, brush, invert, last: px, carry: 0.0, grab: None, layer: None, sharp: None, hook: None, orig: None, details: None, dir: [0.0; 3], depth, inv };
        let changed = if brush == Brush::Grab {
            let radius = self.local_radius(rv3d, &active, hit.world);
            if let Some((mesh, mask)) = scene.sculpt_mesh_mask_mut(id) {
                active.grab = Some(Grab::begin(mesh, mask, hit.local, radius, self.settings.symmetry, px, active.depth));
            }
            true
        } else if brush == Brush::SnakeHook {
            let radius = self.local_radius(rv3d, &active, hit.world);
            active.hook = Some(Hook { center: hit.local, radius });
            true
        } else {
            if brush == Brush::Thumb {
                active.orig = scene.sculpt_mesh_mut(id).map(|m| m.verts.clone());
            }
            if brush == Brush::Smooth && active.invert {
                active.details = scene.sculpt_mesh_mut(id).map(|m| sculpt_enhance::detail_directions(m, &active.adj));
            }
            if brush == Brush::DrawSharp {
                active.sharp = scene.sculpt_mesh_mut(id).map(|m| Sharp::begin(m));
            }
            if brush == Brush::Layer {
                active.layer = scene.sculpt_mesh_mut(id).map(|m| Layer::begin(m));
            }
            self.dab_at(scene, rv3d, &mut active, &hit)
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

    fn dab_at(&self, scene: &mut Scene, rv3d: &RegionView3D, a: &mut Active, hit: &Hit) -> bool {
        let radius = self.local_radius(rv3d, a, hit.world);
        let Some((mesh, mask)) = scene.sculpt_mesh_mask_mut(a.id) else { return false };
        let sym = self.settings.symmetry;
        if let Some(layer) = a.layer.as_mut() {
            layer.dab(mesh, mask, hit.local, radius, self.settings.strength, a.invert, sym);
            return true;
        }
        if let Some(sharp) = a.sharp.as_ref() {
            sharp.dab(mesh, mask, hit.local, radius, self.settings.strength, a.invert, sym);
            return true;
        }
        if let Some(details) = a.details.as_ref() {
            sculpt_enhance::enhance_details(mesh, mask, details, hit.local, radius, self.settings.strength, sym);
            return true;
        }
        if let (Brush::Thumb, Some(orig)) = (a.brush, a.orig.as_ref()) {
            thumb(mesh, orig, mask, &a.adj, hit.local, radius, self.settings.strength, a.dir, sym);
            return true;
        }
        if a.brush == Brush::Nudge {
            nudge(mesh, mask, &a.adj, hit.local, radius, self.settings.strength, a.dir, sym);
            return true;
        }
        dab_masked(mesh, mask, &a.adj, a.brush, hit.local, radius, self.settings.strength, a.invert, self.settings.symmetry);
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
        } else if a.hook.is_some() {
            let p0 = rv3d.unproject_v3(rw, rh, a.last[0], a.last[1], a.depth);
            let p1 = rv3d.unproject_v3(rw, rh, cur[0], cur[1], a.depth);
            if let (Some(p0), Some(p1)) = (p0, p1) {
                let delta = mat4_mul_dir(&a.inv, sub(p1, p0));
                let sym = self.settings.symmetry;
                if let (Some(hook), Some((mesh, mask))) = (a.hook.as_mut(), scene.sculpt_mesh_mask_mut(a.id)) {
                    hook.apply(mesh, mask, delta, sym);
                    changed = true;
                }
            }
            a.last = cur;
        } else {
            if matches!(a.brush, Brush::Nudge | Brush::Thumb) {
                let p0 = rv3d.unproject_v3(rw, rh, a.last[0], a.last[1], a.depth);
                let p1 = rv3d.unproject_v3(rw, rh, cur[0], cur[1], a.depth);
                if let (Some(p0), Some(p1)) = (p0, p1) {
                    a.dir = normalized(mat4_mul_dir(&a.inv, sub(p1, p0)));
                }
            }
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
                        changed |= self.dab_at(scene, rv3d, &mut a, &hit);
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
    #[allow(clippy::too_many_arguments)]
    pub fn draw_cursor(&self, buf: &mut [u32], width: usize, height: usize, region: Rcti, user: &UserDef, cursor: [i32; 2], invert: bool) {
        let invert = invert != self.settings.subtract;
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

    #[allow(dead_code)]
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
    fn thumb_drags_along_the_surface_and_never_along_the_normal() {
        let mut m = grid();
        let orig = m.verts.clone();
        let adj = adjacency(&m);
        thumb(&mut m, &orig, &[], &adj, [0.0; 3], 0.5, 1.0, normalized([1.0, 0.0, 1.0]), [false; 3]);
        let i = (0..m.verts.len()).min_by(|&a, &b| dot(orig[a], orig[a]).partial_cmp(&dot(orig[b], orig[b])).unwrap()).unwrap();
        assert!(m.verts[i][0] > orig[i][0] + 1e-4);
        assert!(m.verts.iter().zip(&orig).all(|(a, b)| (a[2] - b[2]).abs() < 1e-6));
        let far = (0..m.verts.len()).find(|&k| dot(orig[k], orig[k]) > 0.64).unwrap();
        assert_eq!(m.verts[far], orig[far]);
    }

    #[test]
    fn thumb_keeps_the_stroke_start_patch() {
        let mut m = grid();
        let orig = m.verts.clone();
        let adj = adjacency(&m);
        for _ in 0..30 {
            thumb(&mut m, &orig, &[], &adj, [0.0; 3], 0.4, 1.0, [1.0, 0.0, 0.0], [false; 3]);
        }
        /* A vertex outside the starting patch is never picked up, however far the patch was dragged. */
        let far = (0..m.verts.len()).find(|&k| dot(orig[k], orig[k]) > 0.25).unwrap();
        assert_eq!(m.verts[far], orig[far]);
    }

    #[test]
    fn thumb_symmetry_mirrors_the_drag() {
        let mut m = grid();
        let orig = m.verts.clone();
        let adj = adjacency(&m);
        thumb(&mut m, &orig, &[], &adj, [0.5, 0.0, 0.0], 0.4, 1.0, [1.0, 0.0, 0.0], [true, false, false]);
        let right = (0..m.verts.len()).filter(|&k| (orig[k][0] - 0.5).abs() < 0.06 && orig[k][1].abs() < 0.06).map(|k| m.verts[k][0] - orig[k][0]).fold(f32::MIN, f32::max);
        let left = (0..m.verts.len()).filter(|&k| (orig[k][0] + 0.5).abs() < 0.06 && orig[k][1].abs() < 0.06).map(|k| m.verts[k][0] - orig[k][0]).fold(f32::MAX, f32::min);
        assert!(right > 1e-4 && left < -1e-4, "{right} {left}");
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
    fn fill_raises_and_scrape_lowers() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        let up = m.vertex_normals()[i][2].signum();
        let mut dent = m.clone();
        dent.verts[i][2] = -0.2 * up;
        dab(&mut dent, &adj, Brush::Fill, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert!((dent.verts[i][2] * up).abs() < 0.2);
        let mut bump = m.clone();
        bump.verts[i][2] = 0.2 * up;
        dab(&mut bump, &adj, Brush::Fill, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert_eq!(bump.verts[i][2], 0.2 * up);
        dab(&mut bump, &adj, Brush::Scrape, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert!(bump.verts[i][2] * up < 0.2);
        m.verts[i][2] = 0.0;
    }

    #[test]
    fn mask_filters_grow_shrink_smooth_sharpen() {
        let m = grid();
        let adj = adjacency(&m);
        let c = centre_vertex(&m);
        let mut base = vec![0.0f32; m.verts.len()];
        base[c] = 1.0;
        let count = |v: &[f32]| v.iter().filter(|&&x| x > 0.0).count();

        let mut grown = base.clone();
        filter_mask(&mut grown, &adj, MaskFilter::Grow);
        assert_eq!(count(&grown), 1 + adj.adj[c].len(), "the vertex and its neighbours");
        assert!(adj.adj[c].iter().all(|&n| grown[n as usize] == 1.0));

        filter_mask(&mut grown, &adj, MaskFilter::Shrink);
        assert_eq!(grown, base, "shrink undoes a grow of a single vertex");
        let mut gone = base.clone();
        filter_mask(&mut gone, &adj, MaskFilter::Shrink);
        assert_eq!(count(&gone), 0, "a lone masked vertex shrinks away");

        let mut smooth = base.clone();
        filter_mask(&mut smooth, &adj, MaskFilter::Smooth);
        assert_eq!(smooth[c], 0.0, "the mean of unmasked neighbours");
        let n = adj.adj[c][0] as usize;
        assert!(smooth[n] > 0.0 && smooth[n] < 1.0, "the neighbours pick up part of it");

        let mut soft = vec![0.25f32, 0.5, 0.75, 0.0, 1.0];
        let a2 = Topo { adj: vec![vec![]; 5], vfaces: vec![vec![]; 5] };
        filter_mask(&mut soft, &a2, MaskFilter::Sharpen);
        assert!(soft[0] < 0.25 && soft[2] > 0.75 && (soft[1] - 0.5).abs() < 1e-6);
        assert_eq!((soft[3], soft[4]), (0.0, 1.0));
        filter_mask(&mut soft, &a2, MaskFilter::Smooth);
        assert!(soft.iter().all(|v| (0.0..=1.0).contains(v)), "no neighbours: unchanged and in range");
    }

    #[test]
    fn mask_brush_paints_and_ctrl_erases() {
        let mut m = grid();
        let adj = adjacency(&m);
        let mut mask = vec![0.0; m.verts.len()];
        let i = m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        let far = m.verts.iter().position(|v| (v[0] - 0.9).abs() < 1e-3 && (v[1] - 0.9).abs() < 1e-3).unwrap();
        for _ in 0..3 {
            dab_masked(&mut m, &mut mask, &adj, Brush::Mask, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        }
        assert!(mask[i] > 0.5 && mask[i] <= 1.0);
        assert_eq!(mask[far], 0.0);
        let before = mask[i];
        dab_masked(&mut m, &mut mask, &adj, Brush::Mask, [0.0; 3], 0.5, 1.0, true, [false; 3]);
        assert!(mask[i] < before);
        assert_eq!(m, grid(), "the mask brush does not move the mesh");
    }

    #[test]
    fn masked_vertices_do_not_move() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        let sign = m.vertex_normals()[i][2].signum();
        let mut mask = vec![0.0; m.verts.len()];
        mask[i] = 1.0;
        dab_masked(&mut m, &mut mask, &adj, Brush::Draw, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert_eq!(m.verts[i][2], 0.0, "fully masked");
        let other = m.verts.iter().position(|v| (v[0] - 0.2).abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        assert!(m.verts[other][2] * sign > 0.0, "an unmasked neighbour rises");
        let mut half = grid();
        let mut hm = vec![0.0; half.verts.len()];
        hm[i] = 0.5;
        let mut full = grid();
        dab(&mut full, &adj, Brush::Draw, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        dab_masked(&mut half, &mut hm, &adj, Brush::Draw, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert!((half.verts[i][2] - full.verts[i][2] * 0.5).abs() < 1e-5, "half masked moves half as far");
    }

    #[test]
    fn grab_skips_masked_vertices() {
        let m0 = grid();
        let i = m0.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        let mut mask = vec![0.0; m0.verts.len()];
        mask[i] = 1.0;
        let mut m = m0.clone();
        let g = Grab::begin(&m0, &mask, [0.0; 3], 0.5, [false; 3], [0.0, 0.0], 0.5);
        g.apply(&mut m, [0.0, 0.0, 1.0]);
        assert_eq!(m.verts[i], m0.verts[i]);
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
        let g = Grab::begin(&m0, &[], [0.0, 0.0, 0.0], 0.5, [false; 3], [0.0, 0.0], 0.5);
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

    fn sphere_scene() -> (Scene, RegionView3D, i32, i32) {
        use crate::mesh::Primitive;
        use crate::view3d::View3D;
        let mut scene = Scene::startup();
        let id = scene.add_primitive(Primitive::UvSphere, [0.0, 0.0, 0.0]);
        scene.active = Some(id);
        let (w, h) = (400, 300);
        let mut rv3d = RegionView3D::default();
        rv3d.update(&View3D::default(), w, h);
        (scene, rv3d, w, h)
    }

    #[test]
    fn a_mask_stroke_paints_the_mask_and_leaves_the_mesh() {
        let (mut scene, rv3d, w, h) = sphere_scene();
        let before = scene.active_object().unwrap().mesh().unwrap().clone();
        let mut session = Session::new();
        session.settings.radius_px = 40.0;
        session.settings.brush = Brush::Mask;
        assert!(session.begin_stroke(&mut scene, &rv3d, w, h, [w / 2, h / 2], false, false));
        session.move_stroke(&mut scene, &rv3d, w, h, [w / 2 + 30, h / 2]);
        session.end_stroke();
        assert!(scene.has_mask());
        assert_eq!(scene.active_object().unwrap().mesh().unwrap(), &before);
        assert!(scene.undo());
        assert!(!scene.has_mask(), "one undo step takes the stroke's mask back");
    }

    #[test]
    fn scene_mask_filters_grow_shrink_and_undo() {
        let (mut scene, rv3d, w, h) = sphere_scene();
        assert!(!scene.mask_filter_active(MaskFilter::Grow), "nothing masked, nothing to do");
        let mut session = Session::new();
        session.settings.radius_px = 30.0;
        session.settings.brush = Brush::Mask;
        session.begin_stroke(&mut scene, &rv3d, w, h, [w / 2, h / 2], false, false);
        session.end_stroke();
        let count = |s: &Scene| s.active_object().unwrap().mask.iter().filter(|&&m| m > 0.0).count();
        let painted = count(&scene);
        assert!(painted > 0);
        assert!(scene.mask_filter_active(MaskFilter::Grow));
        assert!(count(&scene) > painted, "grow reaches more vertices");
        assert!(scene.mask_filter_active(MaskFilter::Shrink));
        assert!(count(&scene) <= painted + 1);
        assert!(scene.undo() && scene.undo());
        assert_eq!(count(&scene), painted, "two undo steps back to the painted mask");
    }

    #[test]
    fn clear_and_invert_mask() {
        let (mut scene, _, _, _) = sphere_scene();
        assert!(!scene.clear_mask_active(), "nothing to clear");
        assert!(scene.invert_mask_active());
        let n = scene.active_object().unwrap().mesh().unwrap().verts.len();
        assert_eq!(scene.active_object().unwrap().mask, vec![1.0; n], "inverting an empty mask masks everything");
        assert!(scene.invert_mask_active());
        assert!(!scene.has_mask());
        assert!(scene.invert_mask_active() && scene.clear_mask_active());
        assert!(!scene.has_mask());
        assert!(scene.undo());
        assert!(scene.has_mask(), "undo brings the mask back");
    }

    #[test]
    fn a_fully_masked_object_cannot_be_sculpted() {
        let (mut scene, rv3d, w, h) = sphere_scene();
        scene.invert_mask_active();
        let before = scene.active_object().unwrap().mesh().unwrap().clone();
        let mut session = Session::new();
        session.settings.radius_px = 40.0;
        session.begin_stroke(&mut scene, &rv3d, w, h, [w / 2, h / 2], false, false);
        session.move_stroke(&mut scene, &rv3d, w, h, [w / 2 + 30, h / 2]);
        session.end_stroke();
        assert_eq!(scene.active_object().unwrap().mesh().unwrap(), &before);
    }

    fn centre_vertex(m: &Mesh) -> usize {
        m.verts.iter().position(|v| v[0].abs() < 1e-3 && v[1].abs() < 1e-3).unwrap()
    }

    #[test]
    fn blob_swells_and_ctrl_blob_dents() {
        let adj = adjacency(&grid());
        let mut m = grid();
        let i = centre_vertex(&m);
        let up = m.vertex_normals()[i][2].signum();
        dab(&mut m, &adj, Brush::Blob, [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert!(m.verts[i][2] * up > 0.01);
        assert_eq!(height_at(&m, 0.9, 0.9), 0.0);
        let mut d = grid();
        dab(&mut d, &adj, Brush::Blob, [0.0; 3], 0.5, 1.0, true, [false; 3]);
        assert!(d.verts[i][2] * up < -0.01);
    }

    #[test]
    fn layer_builds_a_plateau_of_capped_height() {
        let m0 = grid();
        let i = centre_vertex(&m0);
        let up = m0.vertex_normals()[i][2].signum();
        let (radius, strength) = (0.5f32, 1.0f32);
        let mut m = m0.clone();
        let mut layer = Layer::begin(&m);
        let mut last = 0.0;
        for _ in 0..200 {
            layer.dab(&mut m, &[], [0.0; 3], radius, strength, false, [false; 3]);
            let h = m.verts[i][2] * up;
            assert!(h >= last - 1e-6, "never lowers");
            last = h;
        }
        let cap = radius * strength * LAYER_HEIGHT;
        assert!((last - cap).abs() < 1e-4, "height {last} cap {cap}");
        /* Dabbing on a vertex that is already at the cap does not go on raising it. */
        layer.dab(&mut m, &[], [0.0; 3], radius, strength, false, [false; 3]);
        assert!((m.verts[i][2] * up - cap).abs() < 1e-4);
        /* A new stroke starts from the raised surface, so the cap applies on top of it. */
        let mut layer2 = Layer::begin(&m);
        layer2.dab(&mut m, &[], [0.0; 3], radius, strength, false, [false; 3]);
        assert!(m.verts[i][2] * up > cap);
        /* The inverted brush digs. */
        let mut dug = m0.clone();
        let mut l3 = Layer::begin(&dug);
        for _ in 0..50 {
            l3.dab(&mut dug, &[], [0.0; 3], radius, strength, true, [false; 3]);
        }
        assert!(dug.verts[i][2] * up < -0.01);
    }

    #[test]
    fn nudge_drags_along_the_stroke_on_the_surface() {
        let mut m = grid();
        let adj = adjacency(&m);
        let i = m.verts.iter().position(|v| (v[0] - 0.2).abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        /* The direction has a component along the normal, which a nudge ignores. */
        nudge(&mut m, &[], &adj, [0.0; 3], 0.5, 1.0, normalized([1.0, 0.0, 1.0]), [false; 3]);
        assert!(m.verts[i][0] > 0.2 + 1e-4, "moved along the stroke");
        assert_eq!(m.verts[i][2], 0.0, "stays on the surface");
        assert_eq!(height_at(&m, 0.9, 0.9), 0.0);
        let mut zero = grid();
        nudge(&mut zero, &[], &adj, [0.0; 3], 0.5, 1.0, [0.0; 3], [false; 3]);
        assert_eq!(zero, grid(), "no direction, no movement");
        let mut masked = grid();
        let mask = vec![1.0; masked.verts.len()];
        nudge(&mut masked, &mask, &adj, [0.0; 3], 0.5, 1.0, [1.0, 0.0, 0.0], [false; 3]);
        assert_eq!(masked, grid());
    }

    #[test]
    fn snake_hook_pulls_the_centre_along_and_the_rim_less() {
        let mut m = grid();
        let i = centre_vertex(&m);
        let near = m.verts.iter().position(|v| (v[0] - 0.3).abs() < 1e-3 && v[1].abs() < 1e-3).unwrap();
        let mut hook = Hook { center: [0.0; 3], radius: 0.5 };
        for _ in 0..5 {
            hook.apply(&mut m, &[], [0.0, 0.0, 0.1], [false; 3]);
        }
        assert!((m.verts[i][2] - 0.5).abs() < 1e-4, "the centre follows the brush exactly, got {}", m.verts[i][2]);
        assert!(m.verts[near][2] > 0.0 && m.verts[near][2] < m.verts[i][2]);
        assert_eq!(height_at(&m, 0.9, 0.9), 0.0);
        assert!((hook.center[2] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn snake_hook_with_symmetry_moves_a_plane_vertex_once() {
        let mut m = grid();
        let i = centre_vertex(&m);
        let mut hook = Hook { center: [0.0; 3], radius: 0.5 };
        hook.apply(&mut m, &[], [0.0, 0.0, 0.1], [true, false, false]);
        assert!((m.verts[i][2] - 0.1).abs() < 1e-5, "{}", m.verts[i][2]);
    }

    #[test]
    fn new_brushes_stroke_through_the_scene_and_undo_in_one_step() {
        for brush in [Brush::Layer, Brush::SnakeHook, Brush::Nudge, Brush::Blob, Brush::Thumb, Brush::DrawSharp] {
            let (mut scene, rv3d, w, h) = sphere_scene();
            let before = scene.active_object().unwrap().mesh().unwrap().clone();
            let mut session = Session::new();
            session.settings.radius_px = 40.0;
            session.settings.brush = brush;
            assert!(session.begin_stroke(&mut scene, &rv3d, w, h, [w / 2, h / 2], false, false), "{brush:?}");
            for k in 1..6 {
                session.move_stroke(&mut scene, &rv3d, w, h, [w / 2 + k * 8, h / 2 - k * 3]);
            }
            session.end_stroke();
            let after = scene.active_object().unwrap().mesh().unwrap().clone();
            assert_ne!(before, after, "{brush:?} changed nothing");
            assert!(after.verts.iter().all(|v| v.iter().all(|c| c.is_finite())), "{brush:?}");
            assert!(scene.undo());
            assert_eq!(scene.active_object().unwrap().mesh().unwrap(), &before, "{brush:?}");
        }
    }

    #[test]
    fn inverted_smooth_enhances_details() {
        let (mut scene, rv3d, w, h) = sphere_scene();
        let run = |scene: &mut Scene, invert| {
            let mut session = Session::new();
            session.settings.brush = Brush::Smooth;
            session.settings.radius_px = 40.0;
            assert!(session.begin_stroke(scene, &rv3d, w, h, [w / 2, h / 2], invert, false));
            session.end_stroke();
            let mesh = scene.active_object().unwrap().mesh().unwrap().clone();
            assert!(scene.undo());
            mesh
        };
        let before = scene.active_object().unwrap().mesh().unwrap().clone();
        let (smooth, enhanced) = (run(&mut scene, false), run(&mut scene, true));
        assert_ne!(enhanced, before);
        assert_ne!(enhanced, smooth);
    }

    #[test]
    fn draw_sharp_takes_falloff_from_the_stroke_start() {
        let mut m = grid();
        let c = centre_vertex(&m);
        let sharp = Sharp::begin(&m);
        sharp.dab(&mut m, &[], [0.0; 3], 0.5, 1.0, false, [false; 3]);
        let once = m.verts.clone();
        assert!((once[c][2] - 0.5 * DRAW_FACTOR).abs() < 1e-5);
        /* The second dab adds the same amounts although the surface has moved. */
        sharp.dab(&mut m, &[], [0.0; 3], 0.5, 1.0, false, [false; 3]);
        assert!(m.verts.iter().zip(&once).all(|(a, b)| (a[2] - 2.0 * b[2]).abs() < 1e-5));
        assert!(once.iter().any(|v| v[2] > 0.0 && v[2] < once[c][2]), "falloff weighted");
        let mut dug = grid();
        Sharp::begin(&dug).dab(&mut dug, &[], [0.0; 3], 0.5, 1.0, true, [false; 3]);
        assert!((dug.verts[c][2] + once[c][2]).abs() < 1e-6);
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
