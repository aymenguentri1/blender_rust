//! Row layout of `draw_mesh_automasking_settings(topbar=True)` (scripts/startup/bl_ui/properties_paint_common.py).
//! Pure list of rows with the Python's conditional visibility; the popover draws it.
//! Deviation: only the topbar variant (no collapsible panel, no split-factor hack for the steps slider).

use crate::sculpt_automask_settings::AutomaskSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskOp {
    /// `sculpt.mask_from_boundary`, `boundary_mode = 'MESH'`.
    BoundaryMesh,
    /// `sculpt.mask_from_boundary`, `boundary_mode = 'FACE_SETS'`.
    BoundaryFaceSets,
    /// `sculpt.mask_from_cavity`.
    Cavity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Label(&'static str),
    Separator,
    /// `layout.prop(settings, id, text=text)`; `active` is the row's `layout.active`; `op` is the
    /// "Create Mask" button placed on the same row.
    Prop { id: &'static str, text: Option<&'static str>, active: bool, op: Option<MaskOp> },
    /// `template_curve_mapping(settings, "cavity_curve", brush=True)`.
    CavityCurve,
}

fn prop(id: &'static str, text: &'static str) -> Row {
    Row::Prop { id, text: Some(text), active: true, op: None }
}

fn prop_op(id: &'static str, text: &'static str, op: Option<MaskOp>) -> Row {
    Row::Prop { id, text: Some(text), active: true, op }
}

pub fn layout(s: &AutomaskSettings, use_face_set: bool, use_operators: bool) -> Vec<Row> {
    let mut r = vec![Row::Label("Auto-Masking"), prop("use_automasking_topology", "Topology")];
    if use_face_set {
        r.push(prop("use_automasking_face_sets", "Face Sets"));
    }
    r.push(Row::Separator);

    let op = |on: bool, op: MaskOp| (use_operators && on).then_some(op);
    r.push(prop_op("use_automasking_boundary_edges", "Mesh Boundary", op(s.boundary_edges, MaskOp::BoundaryMesh)));
    if use_face_set {
        r.push(prop_op("use_automasking_boundary_face_sets", "Face Sets Boundary", op(s.boundary_face_sets, MaskOp::BoundaryFaceSets)));
    }
    if s.boundary_edges || s.boundary_face_sets {
        r.push(Row::Prop { id: "boundary_edges_propagation_steps", text: None, active: true, op: None });
    }
    r.push(Row::Separator);

    r.push(prop_op("use_automasking_cavity", "Cavity", None));
    let cavity_active = s.cavity || s.cavity_inverted;
    if use_operators && cavity_active {
        if let Some(Row::Prop { op, .. }) = r.last_mut() {
            *op = Some(MaskOp::Cavity);
        }
    }
    r.push(prop("use_automasking_cavity_inverted", "Cavity (inverted)"));
    if cavity_active {
        r.push(prop("cavity_factor", "Factor"));
        r.push(prop("cavity_blur_steps", "Blur"));
        r.push(prop("use_automasking_custom_cavity_curve", "Custom Curve"));
        if s.custom_cavity_curve {
            r.push(Row::CavityCurve);
        }
    }
    r.push(Row::Separator);

    r.push(prop("use_automasking_view_normal", "View Normal"));
    if s.view_normal {
        r.push(prop("use_automasking_view_occlusion", "Occlusion"));
        let active = !s.view_occlusion;
        r.push(Row::Prop { id: "view_normal_limit", text: Some("Limit"), active, op: None });
        r.push(Row::Prop { id: "view_normal_falloff", text: Some("Falloff"), active, op: None });
    }

    r.push(prop("use_automasking_start_normal", "Area Normal"));
    if s.start_normal {
        r.push(prop("start_normal_limit", "Limit"));
        r.push(prop("start_normal_falloff", "Falloff"));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(rows: &[Row]) -> Vec<&'static str> {
        rows.iter().filter_map(|r| if let Row::Prop { id, .. } = r { Some(*id) } else { None }).collect()
    }

    #[test]
    fn defaults_show_only_the_toggles() {
        let rows = layout(&AutomaskSettings::default(), true, true);
        assert_eq!(rows[0], Row::Label("Auto-Masking"));
        assert_eq!(
            ids(&rows),
            ["use_automasking_topology", "use_automasking_face_sets", "use_automasking_boundary_edges", "use_automasking_boundary_face_sets",
             "use_automasking_cavity", "use_automasking_cavity_inverted", "use_automasking_view_normal", "use_automasking_start_normal"]
        );
    }

    #[test]
    fn face_set_rows_need_use_face_set() {
        let ids = ids(&layout(&AutomaskSettings::default(), false, false));
        assert!(!ids.contains(&"use_automasking_face_sets") && !ids.contains(&"use_automasking_boundary_face_sets"));
    }

    #[test]
    fn steps_only_with_a_boundary_flag() {
        let mut s = AutomaskSettings::default();
        assert!(!ids(&layout(&s, true, false)).contains(&"boundary_edges_propagation_steps"));
        s.boundary_face_sets = true;
        assert!(ids(&layout(&s, true, false)).contains(&"boundary_edges_propagation_steps"));
        s.boundary_face_sets = false;
        s.boundary_edges = true;
        assert!(ids(&layout(&s, true, false)).contains(&"boundary_edges_propagation_steps"));
    }

    #[test]
    fn create_mask_buttons_need_operators_and_the_flag() {
        let mut s = AutomaskSettings::default();
        s.boundary_edges = true;
        s.cavity_inverted = true;
        let has = |rows: &[Row], o| rows.iter().any(|r| matches!(r, Row::Prop { op: Some(x), .. } if *x == o));
        let with = layout(&s, true, true);
        assert!(has(&with, MaskOp::BoundaryMesh) && has(&with, MaskOp::Cavity) && !has(&with, MaskOp::BoundaryFaceSets));
        let without = layout(&s, true, false);
        assert!(!has(&without, MaskOp::BoundaryMesh) && !has(&without, MaskOp::Cavity));
    }

    #[test]
    fn cavity_block_and_curve() {
        let mut s = AutomaskSettings::default();
        s.cavity_inverted = true;
        let rows = layout(&s, false, false);
        assert!(ids(&rows).contains(&"cavity_factor") && ids(&rows).contains(&"use_automasking_custom_cavity_curve"));
        assert!(!rows.contains(&Row::CavityCurve));
        s.custom_cavity_curve = true;
        assert!(layout(&s, false, false).contains(&Row::CavityCurve));
    }

    #[test]
    fn view_normal_limit_is_inactive_with_occlusion() {
        let mut s = AutomaskSettings::default();
        assert!(!ids(&layout(&s, false, false)).contains(&"use_automasking_view_occlusion"));
        s.view_normal = true;
        s.view_occlusion = true;
        let rows = layout(&s, false, false);
        assert!(ids(&rows).contains(&"use_automasking_view_occlusion"));
        assert!(rows.contains(&Row::Prop { id: "view_normal_limit", text: Some("Limit"), active: false, op: None }));
        s.view_occlusion = false;
        assert!(layout(&s, false, false).contains(&Row::Prop { id: "view_normal_falloff", text: Some("Falloff"), active: true, op: None }));
    }

    #[test]
    fn area_normal_sliders_only_when_enabled() {
        let mut s = AutomaskSettings::default();
        assert!(!ids(&layout(&s, false, false)).contains(&"start_normal_limit"));
        s.start_normal = true;
        assert!(ids(&layout(&s, false, false)).contains(&"start_normal_falloff"));
    }
}
