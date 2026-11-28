//! VIEW3D_MT_view: the View menu of the 3D viewport header, as pure layout rows.
//!
//! Ported from `scripts/startup/bl_ui/space_view3d.py` (`class VIEW3D_MT_view`, `draw`). The region toggles are
//! `layout.prop(view, "show_region_*")` checkboxes: the row `idname` is the RNA property and `checkbox` its value; the
//! labels are the RNA UI names (`DEF_SHOW_REGION_PROPERTY` in `rna_space.cc`).

use crate::sculpt_menu::Row;

/// The state `VIEW3D_MT_view.draw` reads from the context and the space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    pub show_region_toolbar: bool,
    pub show_region_ui: bool,
    pub show_region_tool_header: bool,
    pub show_region_asset_shelf: bool,
    pub show_region_hud: bool,
    pub show_viewer: bool,
    /// `context.mode in {'PAINT_TEXTURE', 'PAINT_VERTEX', 'PAINT_WEIGHT', 'SCULPT'}`.
    pub stroke_mode: bool,
    /// `bool(view.region_quadviews)`.
    pub quad_view: bool,
}

fn op(idname: &'static str, label: &'static str, props: &'static [(&'static str, &'static str)]) -> Row {
    Row::Operator { idname, label, props, checkbox: None }
}

fn prop(idname: &'static str, label: &'static str, value: bool) -> Row {
    Row::Operator { idname, label, props: &[], checkbox: Some(value) }
}

/// Rows of the menu, in the order of `VIEW3D_MT_view.draw`.
pub fn rows(s: &ViewState) -> Vec<Row> {
    let menu = |idname, label| Row::Menu { idname, label };
    let mut r = vec![
        prop("show_region_toolbar", "Toolbar", s.show_region_toolbar),
        prop("show_region_ui", "Sidebar", s.show_region_ui),
        prop("show_region_tool_header", "Tool Settings", s.show_region_tool_header),
        prop("show_region_asset_shelf", "Asset Shelf", s.show_region_asset_shelf),
        prop("show_region_hud", "Adjust Last Operation", s.show_region_hud),
        Row::Separator,
    ];
    r.push(op("view3d.view_selected", if s.stroke_mode { "Center Last Stroke" } else { "Frame Selected" }, &[]));
    if s.quad_view {
        r.push(op("view3d.view_selected", "Frame Selected (Quad View)", &[("use_all_regions", "True")]));
    }
    r.extend([
        op("view3d.view_all", "Frame All", &[("center", "False")]),
        op("view3d.view_persportho", "Perspective/Orthographic", &[]),
        menu("VIEW3D_MT_view_local", "Local View"),
        prop("show_viewer", "Viewer Node", s.show_viewer),
        Row::Separator,
        menu("VIEW3D_MT_view_cameras", "Cameras"),
        Row::Separator,
        menu("VIEW3D_MT_view_viewpoint", "Viewpoint"),
        menu("VIEW3D_MT_view_navigation", "Navigation"),
        menu("VIEW3D_MT_view_align", "Align View"),
        Row::Separator,
        menu("VIEW3D_MT_view_regions", "View Regions"),
        Row::Separator,
        op("screen.animation_play", "Play Animation", &[]),
        Row::Separator,
        op("render.opengl", "Render Viewport Preview", &[]),
        op("render.opengl", "Render Playblast", &[("animation", "True")]),
        op("render.opengl", "Render Playblast on Keyframes", &[("animation", "True"), ("render_keyed_only", "True")]),
        Row::Separator,
        menu("INFO_MT_area", "Area"),
    ]);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(r: &Row) -> &'static str {
        match r {
            Row::Separator => "-",
            Row::Menu { label, .. } | Row::Operator { label, .. } => label,
        }
    }

    fn labels(s: &ViewState) -> Vec<&'static str> {
        rows(s).iter().map(label).collect()
    }

    #[test]
    fn labels_in_blender_order() {
        assert_eq!(
            labels(&ViewState::default()),
            [
                "Toolbar", "Sidebar", "Tool Settings", "Asset Shelf", "Adjust Last Operation", "-",
                "Frame Selected", "Frame All", "Perspective/Orthographic", "Local View", "Viewer Node", "-",
                "Cameras", "-", "Viewpoint", "Navigation", "Align View", "-", "View Regions", "-",
                "Play Animation", "-", "Render Viewport Preview", "Render Playblast",
                "Render Playblast on Keyframes", "-", "Area",
            ]
        );
    }

    #[test]
    fn stroke_modes_say_center_last_stroke() {
        let l = labels(&ViewState { stroke_mode: true, ..Default::default() });
        assert_eq!(l[6], "Center Last Stroke");
        assert!(!l.contains(&"Frame Selected"));
    }

    #[test]
    fn quad_view_adds_the_all_regions_row_after_frame_selected() {
        let r = rows(&ViewState { quad_view: true, ..Default::default() });
        assert_eq!(label(&r[7]), "Frame Selected (Quad View)");
        match &r[7] {
            Row::Operator { idname, props, .. } => {
                assert_eq!(*idname, "view3d.view_selected");
                assert_eq!(*props, &[("use_all_regions", "True")]);
            }
            _ => panic!("not an operator row"),
        }
        assert_eq!(rows(&ViewState::default()).len() + 1, r.len());
    }

    #[test]
    fn checkboxes_follow_the_state() {
        let r = rows(&ViewState { show_region_ui: true, show_viewer: true, ..Default::default() });
        let on: Vec<_> = r
            .iter()
            .filter_map(|x| match x {
                Row::Operator { label, checkbox: Some(true), .. } => Some(*label),
                _ => None,
            })
            .collect();
        assert_eq!(on, ["Sidebar", "Viewer Node"]);
    }

    #[test]
    fn view_all_does_not_center_the_cursor_and_playblasts_animate() {
        let r = rows(&ViewState::default());
        let find = |l: &str| r.iter().find(|x| label(x) == l).cloned().unwrap();
        assert!(matches!(find("Frame All"), Row::Operator { props: [("center", "False")], .. }));
        assert!(matches!(
            find("Render Playblast on Keyframes"),
            Row::Operator { props: [("animation", "True"), ("render_keyed_only", "True")], .. }
        ));
    }
}
