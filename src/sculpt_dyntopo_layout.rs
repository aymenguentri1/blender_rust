//! Row model of `VIEW3D_PT_sculpt_dyntopo` (scripts/startup/bl_ui/space_view3d_toolbar.py).
//! Enum items come from `detail_refine_items` / `detail_type_items` (rna_sculpt_paint.cc); operator names from
//! `SCULPT_OT_detail_flood_fill`, `SCULPT_OT_dynamic_topology_toggle` (mesh/sculpt_detail.cc, sculpt_dyntopo.cc).
//! Pure data; the popover draws it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailType {
    Relative,
    Constant,
    Brush,
    Manual,
}

impl DetailType {
    pub const ALL: [DetailType; 4] = [DetailType::Relative, DetailType::Constant, DetailType::Brush, DetailType::Manual];
    pub fn label(self) -> &'static str {
        match self {
            DetailType::Relative => "Relative Detail",
            DetailType::Constant => "Constant Detail",
            DetailType::Brush => "Brush Detail",
            DetailType::Manual => "Manual Detail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefineMethod {
    Subdivide,
    Collapse,
    SubdivideCollapse,
}

impl RefineMethod {
    pub const ALL: [RefineMethod; 3] = [RefineMethod::Subdivide, RefineMethod::Collapse, RefineMethod::SubdivideCollapse];
    pub fn label(self) -> &'static str {
        match self {
            RefineMethod::Subdivide => "Subdivide Edges",
            RefineMethod::Collapse => "Collapse Edges",
            RefineMethod::SubdivideCollapse => "Subdivide Collapse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// Header checkbox: `sculpt.dynamic_topology_toggle`.
    HeaderToggle { on: bool },
    /// `row.prop(sculpt, id)` (+ `sculpt.sample_detail_size` eyedropper, mode DYNTOPO, when `eyedropper`).
    Prop { id: &'static str, text: &'static str, active: bool, eyedropper: bool },
    /// `sub.prop(sculpt, "detail_refine_method" | "detail_type_method", text=..)`.
    Enum { id: &'static str, text: &'static str, active: bool },
    /// `col.operator("sculpt.detail_flood_fill")`.
    FloodFill { active: bool },
}

/// `dyntopo` is `sculpt_object.use_dynamic_topology_sculpting`, `brush_has_dyntopo` is
/// `brush.sculpt_capabilities.has_dyntopo`.
pub fn layout(dyntopo: bool, brush_has_dyntopo: bool, detail_type: DetailType) -> Vec<Row> {
    let sub = brush_has_dyntopo || detail_type == DetailType::Manual;
    let manual_or_const = matches!(detail_type, DetailType::Constant | DetailType::Manual);
    let mut r = vec![Row::HeaderToggle { on: dyntopo }];
    let active = dyntopo && sub;
    r.push(match detail_type {
        DetailType::Constant | DetailType::Manual => {
            Row::Prop { id: "constant_detail_resolution", text: "Resolution", active, eyedropper: true }
        }
        DetailType::Brush => Row::Prop { id: "detail_percent", text: "Detail Percentage", active, eyedropper: false },
        DetailType::Relative => Row::Prop { id: "detail_size", text: "Detail Size", active, eyedropper: false },
    });
    r.push(Row::Enum { id: "detail_refine_method", text: "Refine Method", active });
    r.push(Row::Enum { id: "detail_type_method", text: "Detailing", active });
    if manual_or_const {
        r.push(Row::FloodFill { active: dyntopo });
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_uses_detail_size() {
        let r = layout(true, true, DetailType::Relative);
        assert_eq!(r.len(), 4);
        assert!(matches!(r[1], Row::Prop { id: "detail_size", eyedropper: false, active: true, .. }));
    }

    #[test]
    fn brush_uses_percent() {
        let r = layout(true, true, DetailType::Brush);
        assert!(matches!(r[1], Row::Prop { id: "detail_percent", .. }));
        assert!(!r.iter().any(|x| matches!(x, Row::FloodFill { .. })));
    }

    #[test]
    fn constant_and_manual_have_eyedropper_and_flood_fill() {
        for t in [DetailType::Constant, DetailType::Manual] {
            let r = layout(true, true, t);
            assert!(matches!(r[1], Row::Prop { id: "constant_detail_resolution", eyedropper: true, .. }));
            assert_eq!(r.last(), Some(&Row::FloodFill { active: true }));
        }
    }

    #[test]
    fn inactive_rules() {
        // dyntopo off: everything inactive.
        let r = layout(false, true, DetailType::Constant);
        assert!(matches!(r[1], Row::Prop { active: false, .. }));
        assert_eq!(r.last(), Some(&Row::FloodFill { active: false }));
        // brush without dyntopo capability: sub inactive unless Manual; flood fill stays active.
        assert!(matches!(layout(true, false, DetailType::Relative)[1], Row::Prop { active: false, .. }));
        assert!(matches!(layout(true, false, DetailType::Manual)[1], Row::Prop { active: true, .. }));
        assert_eq!(layout(true, false, DetailType::Constant).last(), Some(&Row::FloodFill { active: true }));
    }

    #[test]
    fn enum_labels() {
        assert_eq!(RefineMethod::SubdivideCollapse.label(), "Subdivide Collapse");
        assert_eq!(DetailType::ALL.map(|t| t.label())[3], "Manual Detail");
    }
}
