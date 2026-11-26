//! Edit Mode click select: nearest element under the mouse and what the click does to the selection.
//!
//! Ported from `editors/mesh/editmesh_select.cc` (`findnearest{vert,face}__doClosest`, `find_nearest_edge__doClosest`,
//! `unified_findnearest`, `edbm_select_pick_bmesh`), `ED_view3d_select_dist_px` (view3d_select.cc) and
//! `bmesh/intern/bmesh_marking.cc` (`BM_{vert,edge,face}_select_set`, `BM_mesh_select_mode_flush_ex`).
//! This is the screen-space (X-Ray) picking path. Not ported: the occlusion select buffer of solid mode, region
//! clipping, select history, hidden elements, UV sync.

use crate::mesh::{Mesh, MeshEdge};

/// `ED_view3d_select_dist_px()` is `75 * U.pixelsize`.
pub const SELECT_DIST_PX: f32 = 75.0;
const FIND_NEAR_SELECT_BIAS: f32 = 5.0;
const FIND_NEAR_CYCLE_THRESHOLD_MIN: f32 = 3.0;

/// `em->selectmode` bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectMode {
    pub vert: bool,
    pub edge: bool,
    pub face: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elem {
    Vert(usize),
    Edge(usize),
    Face(usize),
}

/// `eSelectOp` values that make sense for picking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelOp {
    Set,
    Add,
    Sub,
    Xor,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub verts: Vec<bool>,
    pub edges: Vec<bool>,
    pub faces: Vec<bool>,
    pub active_face: Option<usize>,
}

/// Screen positions in region pixels; `None` is an element that does not project (clipped).
pub struct Screen<'a> {
    pub verts: &'a [Option<[f32; 2]>],
    pub face_centers: &'a [Option<[f32; 2]>],
}

#[derive(Clone, Copy)]
struct Hit {
    index: usize,
    dist: f32,
    center: f32,
}

fn manhattan(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs()
}

fn closest_on_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = d[0] * d[0] + d[1] * d[1];
    let t = if len > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + d[0] * t, a[1] + d[1] * t]
}

/// The shared "closest, with select bias and cycling" loop. `items` yields `(index, dist, center_dist, selected)`.
/// `cycle` is `Some(previous index)` when the mouse has not moved since the last pick.
fn nearest(items: impl Iterator<Item = (usize, f32, f32, bool)>, limit: f32, cycle: Option<usize>) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    let mut best_bias = limit;
    let mut cycled: Option<Hit> = None;
    for (index, dist, center, selected) in items {
        let bias = if selected { dist + FIND_NEAR_SELECT_BIAS } else { dist };
        let hit = Hit { index, dist, center };
        if bias < best_bias {
            best_bias = bias;
            best = Some(hit);
        }
        if let Some(prev) = cycle {
            if cycled.is_none() && index > prev && bias < FIND_NEAR_CYCLE_THRESHOLD_MIN {
                cycled = Some(hit);
            }
        }
    }
    cycled.or(best).filter(|h| h.dist < limit)
}

/// `unified_findnearest()`: faces, then edges, then vertices; a face or edge only keeps `dist_margin` for the finer
/// types, and the finest hit wins. `cycle` holds the previously picked element when the mouse has not moved.
pub fn pick(
    mesh_edges: &[MeshEdge],
    screen: &Screen,
    sel: &Selection,
    mode: SelectMode,
    mouse: [f32; 2],
    pixelsize: f32,
    cycle: Option<Option<Elem>>,
) -> Option<Elem> {
    let dist_init = SELECT_DIST_PX * pixelsize;
    let margin = dist_init / 2.0;
    let mut dist = dist_init;
    let mut found = None;
    let on = |v: &[bool], i: usize| v.get(i).copied().unwrap_or(false);

    if mode.face {
        let prev = cycle.map(|p| if let Some(Elem::Face(i)) = p { i } else { 0 });
        let items = screen.face_centers.iter().enumerate().filter_map(|(i, c)| {
            let d = manhattan(mouse, (*c)?);
            Some((i, d, d, on(&sel.faces, i)))
        });
        if let Some(h) = nearest(items, dist, prev) {
            dist = if mode.edge || mode.vert { margin.min(h.center) } else { h.dist };
            found = Some(Elem::Face(h.index));
        }
    }
    if dist > 0.0 && mode.edge {
        let prev = cycle.map(|p| if let Some(Elem::Edge(i)) = p { i } else { 0 });
        let items = mesh_edges.iter().enumerate().filter_map(|(i, e)| {
            let a = (*screen.verts.get(e.a as usize)?)?;
            let b = (*screen.verts.get(e.b as usize)?)?;
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            Some((i, manhattan(mouse, closest_on_segment(mouse, a, b)), manhattan(mouse, mid), on(&sel.edges, i)))
        });
        if let Some(h) = nearest(items, dist, prev) {
            dist = if mode.vert { margin.min(h.center) } else { h.dist };
            found = Some(Elem::Edge(h.index));
        }
    }
    if dist > 0.0 && mode.vert {
        let prev = cycle.map(|p| if let Some(Elem::Vert(i)) = p { i } else { 0 });
        let items = screen.verts.iter().enumerate().filter_map(|(i, c)| {
            Some((i, manhattan(mouse, (*c)?), 0.0, on(&sel.verts, i)))
        });
        if let Some(h) = nearest(items, dist, prev) {
            found = Some(Elem::Vert(h.index));
        }
    }
    found
}

/// Edge indices around each face, in loop order.
fn face_edges(mesh: &Mesh, edges: &[MeshEdge]) -> Vec<Vec<usize>> {
    let index: std::collections::HashMap<(u32, u32), usize> =
        edges.iter().enumerate().map(|(i, e)| ((e.a.min(e.b), e.a.max(e.b)), i)).collect();
    mesh.faces
        .iter()
        .map(|f| {
            (0..f.len())
                .filter_map(|i| {
                    let (a, b) = (f[i], f[(i + 1) % f.len()]);
                    index.get(&(a.min(b), a.max(b))).copied()
                })
                .collect()
        })
        .collect()
}

struct Topo<'a> {
    edges: &'a [MeshEdge],
    faces: Vec<Vec<usize>>,
    mode: SelectMode,
}

impl Topo<'_> {
    fn edge_verts(&self, e: usize) -> [usize; 2] {
        [self.edges[e].a as usize, self.edges[e].b as usize]
    }

    fn vert_has_other_selected_edge(&self, sel: &Selection, v: usize, skip: usize) -> bool {
        (0..self.edges.len()).any(|e| e != skip && sel.edges[e] && self.edge_verts(e).contains(&v))
    }

    fn edge_select_set(&self, sel: &mut Selection, e: usize, on: bool) {
        sel.edges[e] = on;
        for v in self.edge_verts(e) {
            if on || self.mode.vert || !self.vert_has_other_selected_edge(sel, v, e) {
                sel.verts[v] = on;
            }
        }
    }

    fn face_select_set(&self, mesh: &Mesh, sel: &mut Selection, f: usize, on: bool) {
        sel.faces[f] = on;
        let loop_edges = &self.faces[f];
        if on {
            for &e in loop_edges {
                self.edge_select_set(sel, e, true);
            }
        } else if self.mode.vert {
            for &v in &mesh.faces[f] {
                sel.verts[v as usize] = false;
            }
            for &e in loop_edges {
                sel.edges[e] = false;
            }
        } else {
            for &e in loop_edges {
                let shared = (0..self.faces.len()).any(|g| g != f && sel.faces[g] && self.faces[g].contains(&e));
                if self.mode.edge || !shared {
                    sel.edges[e] = false;
                }
            }
            for (&v, &e) in mesh.faces[f].iter().zip(loop_edges) {
                if !self.vert_has_other_selected_edge(sel, v as usize, e) {
                    sel.verts[v as usize] = false;
                }
            }
        }
    }

    /// `BM_mesh_select_mode_flush_ex` without flush down.
    fn flush(&self, sel: &mut Selection) {
        if self.mode.vert {
            for e in 0..self.edges.len() {
                sel.edges[e] = self.edge_verts(e).iter().all(|&v| sel.verts[v]);
            }
        }
        if self.mode.vert || self.mode.edge {
            for (f, loop_edges) in self.faces.iter().enumerate() {
                sel.faces[f] = loop_edges.iter().all(|&e| sel.edges[e]);
            }
        }
    }
}

/// `edbm_select_pick_bmesh()`: applies a click on `hit` (`None` is a click on nothing). Returns whether it changed.
/// `mesh_edges` must be `mesh.edges()`.
pub fn apply_pick(
    mesh: &Mesh,
    mesh_edges: &[MeshEdge],
    sel: &mut Selection,
    mode: SelectMode,
    hit: Option<Elem>,
    op: SelOp,
    deselect_all: bool,
) -> bool {
    sel.verts.resize(mesh.verts.len(), false);
    sel.edges.resize(mesh_edges.len(), false);
    sel.faces.resize(mesh.faces.len(), false);
    let mut changed = false;
    if op == SelOp::Set && (hit.is_some() || deselect_all) {
        sel.verts.fill(false);
        sel.edges.fill(false);
        sel.faces.fill(false);
        changed = true;
    }
    let Some(hit) = hit else { return changed };
    let topo = Topo { edges: mesh_edges, faces: face_edges(mesh, mesh_edges), mode };
    let was = match hit {
        Elem::Vert(i) => sel.verts[i],
        Elem::Edge(i) => sel.edges[i],
        Elem::Face(i) => sel.faces[i],
    };
    let on = match op {
        SelOp::Set | SelOp::Add => true,
        SelOp::Sub => false,
        SelOp::Xor => !was,
    };
    match hit {
        Elem::Vert(i) => sel.verts[i] = on,
        Elem::Edge(i) => topo.edge_select_set(sel, i, on),
        Elem::Face(i) => {
            if op != SelOp::Sub {
                sel.active_face = Some(i);
            }
            topo.face_select_set(mesh, sel, i, on);
        }
    }
    topo.flush(sel);
    true
}

/// `MESH_OT_select_all` `action` (`eSelectAction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelAction {
    Toggle,
    Select,
    Deselect,
    Invert,
}

/// `edbm_select_all_exec()`: `Toggle` deselects when anything is selected, else selects all. `Select` and `Deselect` are
/// `EDBM_flag_enable_all`/`EDBM_flag_disable_all` (every vert, edge and face; the active face is left alone). `Invert` is
/// `EDBM_select_swap()` on the elements of the select mode (vert, else edge, else face) followed by the mode flush.
/// `mesh_edges` must be `mesh.edges()`. Hidden elements are not ported.
pub fn select_all(mesh: &Mesh, mesh_edges: &[MeshEdge], sel: &mut Selection, mode: SelectMode, action: SelAction) {
    sel.verts.resize(mesh.verts.len(), false);
    sel.edges.resize(mesh_edges.len(), false);
    sel.faces.resize(mesh.faces.len(), false);
    let action = match action {
        SelAction::Toggle => {
            if sel.verts.iter().chain(&sel.edges).chain(&sel.faces).any(|&b| b) { SelAction::Deselect } else { SelAction::Select }
        }
        a => a,
    };
    match action {
        SelAction::Select | SelAction::Deselect => {
            let on = action == SelAction::Select;
            sel.verts.fill(on);
            sel.edges.fill(on);
            sel.faces.fill(on);
        }
        _ => {
            let topo = Topo { edges: mesh_edges, faces: face_edges(mesh, mesh_edges), mode };
            if mode.vert {
                for v in sel.verts.iter_mut() {
                    *v = !*v;
                }
            } else if mode.edge {
                for e in 0..mesh_edges.len() {
                    topo.edge_select_set(sel, e, !sel.edges[e]);
                }
            } else {
                for f in 0..mesh.faces.len() {
                    topo.face_select_set(mesh, sel, f, !sel.faces[f]);
                }
            }
            topo.flush(sel);
        }
    }
}

/// One of the three select-mode bits (`SCE_SELECT_VERTEX`, `_EDGE`, `_FACE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeBit {
    Vert,
    Edge,
    Face,
}

/// `MESH_OT_select_mode` `action` (0 disable, 1 enable, 2 toggle; -1, "already set", is `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeAction {
    Disable,
    Enable,
    Toggle,
}

impl SelectMode {
    fn has(self, b: ModeBit) -> bool {
        match b {
            ModeBit::Vert => self.vert,
            ModeBit::Edge => self.edge,
            ModeBit::Face => self.face,
        }
    }

    fn with(mut self, b: ModeBit, on: bool) -> SelectMode {
        match b {
            ModeBit::Vert => self.vert = on,
            ModeBit::Edge => self.edge = on,
            ModeBit::Face => self.face = on,
        }
        self
    }

    fn only(b: ModeBit) -> SelectMode {
        SelectMode { vert: false, edge: false, face: false }.with(b, true)
    }

    fn is_empty(self) -> bool {
        !(self.vert || self.edge || self.face)
    }
}

/// `EDBM_selectmode_toggle_multi()` (without the UV sync and `use_expand` parts): the mode that results from pressing
/// `toggle`, and whether the selection must be converted with [`selectmode_set`]. When it is `false` only the mode value
/// changes ("only update": the bit is already as asked, or it is the only one set and cannot be disabled). Without
/// `extend` the result is just `toggle`.
pub fn selectmode_toggle(old: SelectMode, toggle: ModeBit, action: Option<ModeAction>, extend: bool) -> (SelectMode, bool) {
    let mut new = old;
    let mut only_update = false;
    match action {
        None => {}
        Some(ModeAction::Disable) => {
            if !old.has(toggle) || old == SelectMode::only(toggle) {
                only_update = true;
            } else {
                new = new.with(toggle, false);
            }
        }
        Some(ModeAction::Enable) => {
            if old.has(toggle) {
                only_update = true;
            } else {
                new = new.with(toggle, true);
            }
        }
        Some(ModeAction::Toggle) => {
            if old == SelectMode::only(toggle) {
                only_update = true;
            } else {
                new = new.with(toggle, !old.has(toggle));
            }
        }
    }
    if only_update {
        return (new, false);
    }
    if !extend || new.is_empty() {
        new = SelectMode::only(toggle);
    }
    (new, true)
}

/// `EDBM_selectmode_set()`: converts the selection to the new mode `mode`. Vertex mode flushes up (edges and faces whose
/// vertices are all selected become selected, nothing is deselected); edge mode keeps the selected edges (and their
/// vertices and faces); face mode keeps only the selected faces. Does nothing when nothing is selected.
/// `mesh_edges` must be `mesh.edges()`. Select history and UV sync are not ported.
pub fn selectmode_set(mesh: &Mesh, mesh_edges: &[MeshEdge], sel: &mut Selection, mode: SelectMode) {
    sel.verts.resize(mesh.verts.len(), false);
    sel.edges.resize(mesh_edges.len(), false);
    sel.faces.resize(mesh.faces.len(), false);
    let any = |v: &[bool]| v.iter().any(|&b| b);
    if !(any(&sel.verts) || any(&sel.edges) || any(&sel.faces)) {
        return;
    }
    let topo = Topo { edges: mesh_edges, faces: face_edges(mesh, mesh_edges), mode };
    if mode.vert {
        if any(&sel.verts) {
            /* `BM_mesh_select_flush_from_verts(bm, true)` */
            for e in 0..mesh_edges.len() {
                if topo.edge_verts(e).iter().all(|&v| sel.verts[v]) {
                    sel.edges[e] = true;
                }
            }
            for (f, face) in mesh.faces.iter().enumerate() {
                if face.iter().all(|&v| sel.verts[v as usize]) {
                    sel.faces[f] = true;
                }
            }
        }
    } else if mode.edge {
        sel.verts.fill(false);
        if any(&sel.edges) {
            for e in 0..mesh_edges.len() {
                if sel.edges[e] {
                    topo.edge_select_set(sel, e, true);
                }
            }
            topo.flush(sel);
        }
    } else if mode.face {
        for e in 0..mesh_edges.len() {
            topo.edge_select_set(sel, e, false);
        }
        for f in 0..mesh.faces.len() {
            if sel.faces[f] {
                topo.face_select_set(mesh, sel, f, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERT: SelectMode = SelectMode { vert: true, edge: false, face: false };
    const EDGE: SelectMode = SelectMode { vert: false, edge: true, face: false };
    const FACE: SelectMode = SelectMode { vert: false, edge: false, face: true };
    const ALL: SelectMode = SelectMode { vert: true, edge: true, face: true };

    /// A 200 px square: verts at the corners, face center in the middle.
    const VERTS: [Option<[f32; 2]>; 4] = [Some([0.0, 0.0]), Some([200.0, 0.0]), Some([200.0, 200.0]), Some([0.0, 200.0])];
    const CENTERS: [Option<[f32; 2]>; 1] = [Some([100.0, 100.0])];

    fn quad() -> (Mesh, Vec<MeshEdge>) {
        let m = Mesh { verts: vec![[0.0; 3]; 4], faces: vec![vec![0, 1, 2, 3]] };
        let e = m.edges();
        (m, e)
    }

    fn at(mode: SelectMode, mouse: [f32; 2], sel: &Selection, cycle: Option<Option<Elem>>) -> Option<Elem> {
        let (_, e) = quad();
        pick(&e, &Screen { verts: &VERTS, face_centers: &CENTERS }, sel, mode, mouse, 1.0, cycle)
    }

    fn count(v: &[bool]) -> usize {
        v.iter().filter(|&&b| b).count()
    }

    #[test]
    fn threshold_is_75_manhattan_pixels_exclusive() {
        let s = Selection::default();
        assert_eq!(at(VERT, [-40.0, -34.0], &s, None), Some(Elem::Vert(0)));
        assert_eq!(at(VERT, [-40.0, -35.0], &s, None), None);
        assert_eq!(at(FACE, [100.0, 26.0], &s, None), Some(Elem::Face(0)));
        assert_eq!(at(FACE, [100.0, 25.0], &s, None), None);
    }

    #[test]
    fn edge_distance_is_to_the_segment() {
        let s = Selection::default();
        assert_eq!(at(EDGE, [150.0, 10.0], &s, None), Some(Elem::Edge(0)));
        assert_eq!(at(EDGE, [190.0, 100.0], &s, None), Some(Elem::Edge(1)));
    }

    #[test]
    fn finer_elements_win_only_within_the_margin() {
        let s = Selection::default();
        assert_eq!(at(ALL, [5.0, 5.0], &s, None), Some(Elem::Vert(0)));
        assert_eq!(at(ALL, [100.0, 5.0], &s, None), Some(Elem::Edge(0)));
        assert_eq!(at(ALL, [100.0, 80.0], &s, None), Some(Elem::Face(0)));
    }

    #[test]
    fn selected_elements_are_biased_by_5_pixels() {
        let (_, e) = quad();
        let verts = [Some([0.0, 0.0]), Some([8.0, 0.0])];
        let screen = Screen { verts: &verts, face_centers: &[] };
        let mut s = Selection { verts: vec![true, false], ..Default::default() };
        assert_eq!(pick(&e[..0], &screen, &s, VERT, [3.0, 0.0], 1.0, None), Some(Elem::Vert(1)));
        s.verts[0] = false;
        assert_eq!(pick(&e[..0], &screen, &s, VERT, [3.0, 0.0], 1.0, None), Some(Elem::Vert(0)));
    }

    #[test]
    fn clicking_without_moving_cycles_stacked_verts() {
        let verts = [Some([0.0, 0.0]), Some([0.0, 0.0]), Some([0.0, 0.0])];
        let screen = Screen { verts: &verts, face_centers: &[] };
        let s = Selection::default();
        let next = |p| pick(&[], &screen, &s, VERT, [1.0, 0.0], 1.0, Some(p));
        assert_eq!(next(Some(Elem::Vert(1))), Some(Elem::Vert(2)));
        assert_eq!(next(Some(Elem::Vert(2))), Some(Elem::Vert(0)));
    }

    #[test]
    fn set_replaces_and_toggle_flips() {
        let (m, e) = quad();
        let mut s = Selection::default();
        apply_pick(&m, &e, &mut s, VERT, Some(Elem::Vert(0)), SelOp::Set, false);
        apply_pick(&m, &e, &mut s, VERT, Some(Elem::Vert(1)), SelOp::Xor, false);
        assert_eq!((count(&s.verts), count(&s.edges)), (2, 1));
        apply_pick(&m, &e, &mut s, VERT, Some(Elem::Vert(1)), SelOp::Xor, false);
        assert_eq!((count(&s.verts), count(&s.edges)), (1, 0));
        assert!(!apply_pick(&m, &e, &mut s, VERT, None, SelOp::Set, false));
        assert!(apply_pick(&m, &e, &mut s, VERT, None, SelOp::Set, true));
        assert_eq!(count(&s.verts), 0);
    }

    #[test]
    fn face_and_edge_clicks_flush() {
        let (m, e) = quad();
        let mut s = Selection::default();
        apply_pick(&m, &e, &mut s, FACE, Some(Elem::Face(0)), SelOp::Set, false);
        assert_eq!((count(&s.verts), count(&s.edges), count(&s.faces), s.active_face), (4, 4, 1, Some(0)));
        apply_pick(&m, &e, &mut s, FACE, Some(Elem::Face(0)), SelOp::Xor, false);
        assert_eq!((count(&s.verts), count(&s.edges), count(&s.faces)), (0, 0, 0));
        apply_pick(&m, &e, &mut s, EDGE, Some(Elem::Edge(0)), SelOp::Add, false);
        apply_pick(&m, &e, &mut s, EDGE, Some(Elem::Edge(1)), SelOp::Add, false);
        apply_pick(&m, &e, &mut s, EDGE, Some(Elem::Edge(1)), SelOp::Sub, false);
        assert_eq!((count(&s.verts), count(&s.edges)), (2, 1), "the shared vertex stays with edge 0");
    }

    #[test]
    fn select_all_deselect_and_toggle() {
        let (m, e) = quad();
        let mut sel = Selection::default();
        select_all(&m, &e, &mut sel, VERT, SelAction::Select);
        assert!(sel.verts.iter().chain(&sel.edges).chain(&sel.faces).all(|&b| b));
        select_all(&m, &e, &mut sel, VERT, SelAction::Toggle);
        assert!(!sel.verts.iter().chain(&sel.edges).chain(&sel.faces).any(|&b| b));
        select_all(&m, &e, &mut sel, VERT, SelAction::Toggle);
        assert_eq!(count(&sel.verts), 4);
    }

    #[test]
    fn invert_in_vertex_mode_flushes() {
        let (m, e) = quad();
        let mut sel = Selection::default();
        sel.verts = vec![true, true, false, false];
        select_all(&m, &e, &mut sel, VERT, SelAction::Invert);
        assert_eq!(sel.verts, vec![false, false, true, true]);
        assert_eq!(count(&sel.edges), 1);
        assert_eq!(count(&sel.faces), 0);
        select_all(&m, &e, &mut sel, VERT, SelAction::Invert);
        select_all(&m, &e, &mut sel, VERT, SelAction::Select);
        select_all(&m, &e, &mut sel, VERT, SelAction::Invert);
        assert_eq!((count(&sel.verts), count(&sel.edges), count(&sel.faces)), (0, 0, 0));
    }

    #[test]
    fn invert_in_face_mode_swaps_faces() {
        let (m, e) = quad();
        let mut sel = Selection::default();
        select_all(&m, &e, &mut sel, FACE, SelAction::Invert);
        assert_eq!((count(&sel.faces), count(&sel.edges), count(&sel.verts)), (1, 4, 4));
        select_all(&m, &e, &mut sel, FACE, SelAction::Invert);
        assert_eq!((count(&sel.faces), count(&sel.edges), count(&sel.verts)), (0, 0, 0));
    }

    /// Two quads sharing the edge 1-2: verts 0..5 in a 2x3 grid.
    fn two_quads() -> (Mesh, Vec<MeshEdge>) {
        let m = Mesh { verts: vec![[0.0; 3]; 6], faces: vec![vec![0, 1, 4, 3], vec![1, 2, 5, 4]] };
        let e = m.edges();
        (m, e)
    }

    #[test]
    fn toggle_keeps_one_mode_unless_extending() {
        let all = ALL;
        assert_eq!(selectmode_toggle(VERT, ModeBit::Edge, None, false), (EDGE, true));
        assert_eq!(selectmode_toggle(VERT, ModeBit::Edge, None, true), (VERT, true));
        assert_eq!(selectmode_toggle(VERT, ModeBit::Edge, Some(ModeAction::Toggle), true), (SelectMode { vert: true, edge: true, face: false }, true));
        assert_eq!(selectmode_toggle(VERT, ModeBit::Vert, Some(ModeAction::Toggle), true), (VERT, false), "last bit stays");
        assert_eq!(selectmode_toggle(all, ModeBit::Face, Some(ModeAction::Disable), true).0, SelectMode { vert: true, edge: true, face: false });
        assert_eq!(selectmode_toggle(EDGE, ModeBit::Edge, Some(ModeAction::Enable), true), (EDGE, false));
    }

    #[test]
    fn vert_to_edge_to_face_flushes() {
        let (m, e) = two_quads();
        let mut sel = Selection::default();
        sel.verts = vec![true, true, false, true, true, false];
        selectmode_set(&m, &e, &mut sel, VERT);
        assert_eq!((count(&sel.edges), count(&sel.faces)), (4, 1), "first quad flushed up");
        selectmode_set(&m, &e, &mut sel, EDGE);
        assert_eq!((count(&sel.verts), count(&sel.edges), count(&sel.faces)), (4, 4, 1));
        selectmode_set(&m, &e, &mut sel, FACE);
        assert_eq!((count(&sel.verts), count(&sel.edges), count(&sel.faces)), (4, 4, 1));
    }

    #[test]
    fn edge_mode_drops_loose_verts_and_face_mode_drops_loose_edges() {
        let (m, e) = two_quads();
        let mut sel = Selection::default();
        sel.verts = vec![true, false, false, false, false, true];
        selectmode_set(&m, &e, &mut sel, EDGE);
        assert_eq!(count(&sel.verts), 0, "no selected edge: vertices are deselected");
        let mut sel = Selection::default();
        sel.verts = vec![false, true, true, false, false, false];
        sel.edges = vec![false; e.len()];
        let shared = e.iter().position(|x| (x.a.min(x.b), x.a.max(x.b)) == (1, 2)).unwrap();
        sel.edges[shared] = true;
        selectmode_set(&m, &e, &mut sel, EDGE);
        assert_eq!((count(&sel.verts), count(&sel.edges), count(&sel.faces)), (2, 1, 0));
        selectmode_set(&m, &e, &mut sel, FACE);
        assert_eq!((count(&sel.verts), count(&sel.edges), count(&sel.faces)), (0, 0, 0));
    }

    #[test]
    fn nothing_selected_changes_nothing() {
        let (m, e) = two_quads();
        let mut sel = Selection::default();
        selectmode_set(&m, &e, &mut sel, FACE);
        assert_eq!(count(&sel.verts) + count(&sel.edges) + count(&sel.faces), 0);
    }
}
