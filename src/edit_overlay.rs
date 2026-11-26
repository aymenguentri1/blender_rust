//! Edit Mode overlay colours and sizes for vertices, edges and faces.
//!
//! Ported from `draw/engines/overlay/shaders/overlay_edit_mesh.bsl.hh` (`Resources::vertex_color`,
//! `edge_color_inner`, `edge_vertex_color`, `face_color`, vertex point size) and `overlay_mesh.hh` (pass setup:
//! `select_vert_/edge_/face_`, `wire_shading`). Theme defaults are `U_theme_default.space_view3d` in
//! `release/datafiles/userdef/userdef_default_theme.c`.
//! Not ported: crease/bevel/seam/sharp/freestyle edge outer colours, face dots, retopology offset, facing
//! (fresnel) blend, occlusion fading, GPU passes.

use crate::edit_select::{Elem, Selection, SelectMode};

pub type Rgba = [f32; 4];

/// `RGBA(0xRRGGBBAA)` from the theme file.
pub fn rgba(hex: u32) -> Rgba {
    [
        ((hex >> 24) & 0xff) as f32 / 255.0,
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    ]
}

/// The theme fields the edit-mesh overlay reads (`space_view3d` defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub wire_edit: Rgba,
    pub vertex: Rgba,
    pub vertex_select: Rgba,
    pub edge_select: Rgba,
    pub edge_mode_select: Rgba,
    pub face: Rgba,
    pub face_select: Rgba,
    pub face_mode_select: Rgba,
    pub edit_mesh_active: Rgba,
    pub vertex_size: f32,
    pub edge_width: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            wire_edit: rgba(0x000000ff),
            vertex: rgba(0x000000ff),
            vertex_select: rgba(0xff7a00ff),
            edge_select: rgba(0xff9900ff),
            edge_mode_select: rgba(0xffd800ff),
            face: rgba(0xffffff02),
            face_select: rgba(0xffa30033),
            face_mode_select: rgba(0xffb70033),
            edit_mesh_active: rgba(0xffffff33),
            vertex_size: 3.0,
            edge_width: 1.0,
        }
    }
}

/// Per-element state the shader unpacks from the vertex data (`EditVertData` flags).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub selected: bool,
    pub active: bool,
}

/// Overlay state: which select modes are on and whether the wireframe shading is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub mode: SelectMode,
    pub wire_shading: bool,
}

impl Overlay {
    /// `vertex_color`: active is opaque `edit_mesh_active`, selected is `vert_select`, else `vert`.
    pub fn vertex_color(&self, t: &Theme, f: Flags) -> Rgba {
        if f.active {
            let a = t.edit_mesh_active;
            return [a[0], a[1], a[2], 1.0];
        }
        if f.selected {
            return t.vertex_select;
        }
        t.vertex
    }

    /// `out_point_size = theme.sizes.vert * 2` (without crease).
    pub fn vertex_point_size(&self, t: &Theme) -> f32 {
        t.vertex_size * 2.0
    }

    /// `edge_color_inner` (face/edge select modes): wire, selected, then active, alpha forced to 1.
    pub fn edge_color(&self, t: &Theme, f: Flags) -> Rgba {
        let sel = if self.mode.edge { t.edge_mode_select } else { t.edge_select };
        let mut c = t.wire_edit;
        if f.selected {
            c = sel;
        }
        if f.active {
            c = t.edit_mesh_active;
        }
        c[3] = 1.0;
        c
    }

    /// `edge_vertex_color` (vertex select mode): an edge is lit when a vertex of it is selected or active.
    pub fn edge_vertex_color(&self, t: &Theme, v_sel_or_active: bool) -> Rgba {
        let sel = if self.mode.edge { t.edge_mode_select } else { t.edge_select };
        let mut c = if v_sel_or_active { sel } else { t.wire_edit };
        c[3] = 1.0;
        c
    }

    /// `face_color`, including the `wire_shading` alpha scale and the "don't always fill" alpha rule.
    pub fn face_color(&self, t: &Theme, f: Flags) -> Rgba {
        let sel = if self.mode.face { t.face_mode_select } else { t.face_select };
        let mut c = t.face;
        if f.selected {
            c = sel;
        }
        if self.mode.face && f.active {
            let a = t.edit_mesh_active;
            c = [
                sel[0] + (a[0] - sel[0]) * 0.5,
                sel[1] + (a[1] - sel[1]) * 0.5,
                sel[2] + (a[2] - sel[2]) * 0.5,
                sel[3],
            ];
        }
        if self.wire_shading {
            if f.selected {
                c[3] *= 0.6;
            }
        } else if !(self.mode.face || f.selected || f.active) {
            c[3] = 0.0;
        }
        c
    }

    /// Colour of every vertex, edge and face of an edit mesh for the current selection.
    pub fn colors(&self, t: &Theme, sel: &Selection, edges: &[[usize; 2]]) -> MeshColors {
        let active_face = sel.active_face;
        let vert_flags = |i: usize| Flags { selected: sel.verts.get(i).copied().unwrap_or(false), active: false };
        MeshColors {
            verts: (0..sel.verts.len()).map(|i| self.vertex_color(t, vert_flags(i))).collect(),
            edges: edges
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    if self.mode.vert {
                        let lit = vert_flags(e[0]).selected || vert_flags(e[1]).selected;
                        self.edge_vertex_color(t, lit)
                    } else {
                        let selected = sel.edges.get(i).copied().unwrap_or(false);
                        self.edge_color(t, Flags { selected, active: false })
                    }
                })
                .collect(),
            faces: (0..sel.faces.len())
                .map(|i| {
                    let f = Flags { selected: sel.faces[i], active: active_face == Some(i) };
                    self.face_color(t, f)
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshColors {
    pub verts: Vec<Rgba>,
    pub edges: Vec<Rgba>,
    pub faces: Vec<Rgba>,
}

/// Which element kind an `Elem` is, for callers that colour a single picked element.
pub fn elem_flags(sel: &Selection, e: Elem) -> Flags {
    match e {
        Elem::Vert(i) => Flags { selected: sel.verts.get(i).copied().unwrap_or(false), active: false },
        Elem::Edge(i) => Flags { selected: sel.edges.get(i).copied().unwrap_or(false), active: false },
        Elem::Face(i) => Flags { selected: sel.faces.get(i).copied().unwrap_or(false), active: sel.active_face == Some(i) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ov(v: bool, e: bool, f: bool, wire: bool) -> Overlay {
        Overlay { mode: SelectMode { vert: v, edge: e, face: f }, wire_shading: wire }
    }
    const SEL: Flags = Flags { selected: true, active: false };
    const ACT: Flags = Flags { selected: true, active: true };
    const NONE: Flags = Flags { selected: false, active: false };

    #[test]
    fn rgba_hex() {
        assert_eq!(rgba(0xff7a00ff), [1.0, 122.0 / 255.0, 0.0, 1.0]);
    }

    #[test]
    fn vertex_colors() {
        let (t, o) = (Theme::default(), ov(true, false, false, false));
        assert_eq!(o.vertex_color(&t, NONE), t.vertex);
        assert_eq!(o.vertex_color(&t, SEL), t.vertex_select);
        assert_eq!(o.vertex_color(&t, ACT), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(o.vertex_point_size(&t), 6.0);
    }

    #[test]
    fn edge_colors_by_mode() {
        let t = Theme::default();
        assert_eq!(ov(false, false, true, false).edge_color(&t, SEL), t.edge_select);
        assert_eq!(ov(false, true, false, false).edge_color(&t, SEL), t.edge_mode_select);
        assert_eq!(ov(false, true, false, false).edge_color(&t, NONE), t.wire_edit);
        assert_eq!(ov(false, true, false, false).edge_color(&t, ACT)[..3], t.edit_mesh_active[..3]);
        assert_eq!(ov(false, true, false, false).edge_color(&t, ACT)[3], 1.0);
        assert_eq!(ov(true, false, false, false).edge_vertex_color(&t, true), t.edge_select);
        assert_eq!(ov(true, false, false, false).edge_vertex_color(&t, false), t.wire_edit);
    }

    #[test]
    fn face_alpha_rules() {
        let t = Theme::default();
        // Unselected face outside face mode is not filled.
        assert_eq!(ov(true, false, false, false).face_color(&t, NONE)[3], 0.0);
        // In face mode the base face colour keeps its alpha.
        assert_eq!(ov(false, false, true, false).face_color(&t, NONE), t.face);
        // Selected face uses the mode colour in face mode.
        assert_eq!(ov(false, false, true, false).face_color(&t, SEL), t.face_mode_select);
        assert_eq!(ov(true, false, false, false).face_color(&t, SEL), t.face_select);
        // Wire shading lowers selected alpha to 60%.
        let a = ov(true, false, false, true).face_color(&t, SEL)[3];
        assert!((a - t.face_select[3] * 0.6).abs() < 1e-6);
    }

    #[test]
    fn active_face_blends_with_active_colour() {
        let t = Theme::default();
        let c = ov(false, false, true, false).face_color(&t, ACT);
        assert!((c[0] - (t.face_mode_select[0] + 1.0) / 2.0).abs() < 1e-6);
        assert_eq!(c[3], t.face_mode_select[3]);
    }

    #[test]
    fn mesh_colors_follow_selection() {
        let t = Theme::default();
        let sel = Selection { verts: vec![true, false, false], edges: vec![false, false], faces: vec![false], active_face: None };
        let c = ov(true, false, false, false).colors(&t, &sel, &[[0, 1], [1, 2]]);
        assert_eq!(c.verts[0], t.vertex_select);
        assert_eq!(c.verts[1], t.vertex);
        assert_eq!(c.edges[0], t.edge_select);
        assert_eq!(c.edges[1], t.wire_edit);
        assert_eq!(c.faces[0][3], 0.0);
    }
}
