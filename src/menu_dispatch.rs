//! What the rows of the sculpt header menus do: maps an operator row (`idname` and its properties, as drawn by
//! `sculpt_menu::Row`) to the command this port runs, or to nothing when the operator is not available (the row is
//! then drawn disabled).
//!
//! The Mask menu rows follow `paint.mask_flood_fill` (`mode` VALUE with `value`, or INVERT; paint_mask.cc) and
//! `sculpt.mask_filter` (`filter_type`). Other operators are not available yet.

use crate::scene::Scene;
use crate::sculpt::MaskFilter;
use crate::sculpt_hide::{self, VisAction};
use crate::sculpt_mask_init::InitMode;
use crate::sculpt_menu::Row;
use crate::sculpt_pivot::PivotMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    MaskFill,
    MaskClear,
    MaskInvert,
    MaskFilter(MaskFilter),
    HideShowAll(VisAction),
    HideShowMasked(VisAction),
    VisibilityInvert,
    MaskInit(crate::sculpt_mask_init::InitMode),
    FaceSetsInit(crate::face_set_init::InitMode),
    FaceSetsRandomizeColors,
    SetPivot(crate::sculpt_pivot::PivotMode),
    FaceSetsCreate(crate::face_set_create::CreateMode),
    /// `sculpt.mesh_filter`: modal, started by the caller (`Scene::mesh_filter_begin`).
    MeshFilter(crate::sculpt_mesh_filter::FilterType),
    /// `object.select_all` (`action` SELECT / DESELECT / INVERT).
    SelectAll(crate::scene::SelectAction),
    /// `object.select_more` (true) / `object.select_less` (false).
    SelectMoreLess(bool),
    /// `object.select_hierarchy`.
    SelectHierarchy { to_parent: bool, extend: bool },
    /// `object.select_random` with the default `ratio` 0.5 (`WM_operator_properties_select_random`).
    SelectRandom,
    /// `object.location_clear` / `rotation_clear` / `scale_clear` (`clear_delta` False).
    ClearTransform(crate::scene::ClearKind),
    /// `object.transform_apply` with its location / rotation / scale options.
    ApplyTransform(crate::scene::ApplyMask),
    /// `object.parent_clear` with `type` CLEAR_INVERSE.
    ParentClearInverse,
    /// `object.duplicate_move`.
    Duplicate,
    /// `object.join`.
    Join,
    /// `object.delete`.
    Delete,
}

fn prop(props: &[(&str, &str)], name: &str) -> Option<String> {
    props
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

pub fn command_of(idname: &str, props: &[(&str, &str)]) -> Option<Command> {
    match idname {
        "paint.mask_flood_fill" => match prop(props, "mode")?.as_str() {
            "INVERT" => Some(Command::MaskInvert),
            "VALUE" => match prop(props, "value")?.parse::<f32>().ok()? {
                v if v == 1.0 => Some(Command::MaskFill),
                v if v == 0.0 => Some(Command::MaskClear),
                _ => None,
            },
            _ => None,
        },
        "sculpt.mask_filter" => match prop(props, "filter_type")?.as_str() {
            "SMOOTH" => Some(Command::MaskFilter(MaskFilter::Smooth)),
            "SHARPEN" => Some(Command::MaskFilter(MaskFilter::Sharpen)),
            "GROW" => Some(Command::MaskFilter(MaskFilter::Grow)),
            "SHRINK" => Some(Command::MaskFilter(MaskFilter::Shrink)),
            "CONTRAST_INCREASE" => Some(Command::MaskFilter(MaskFilter::ContrastIncrease)),
            "CONTRAST_DECREASE" => Some(Command::MaskFilter(MaskFilter::ContrastDecrease)),
            _ => None,
        },
        "paint.hide_show_all" => vis_action(props).map(Command::HideShowAll),
        "paint.hide_show_masked" => vis_action(props).map(Command::HideShowMasked),
        "paint.visibility_invert" => Some(Command::VisibilityInvert),
        "sculpt.set_pivot_position" => match crate::sculpt_pivot::PivotMode::from_rna(&prop(props, "mode")?)? {
            m @ (PivotMode::Origin | PivotMode::Unmasked | PivotMode::MaskBorder) => Some(Command::SetPivot(m)),
            _ => None,
        },
        "sculpt.face_sets_randomize_colors" => Some(Command::FaceSetsRandomizeColors),
        "sculpt.face_sets_create" => {
            use crate::face_set_create::CreateMode as C;
            match prop(props, "mode")?.as_str() {
                "MASKED" => Some(Command::FaceSetsCreate(C::Masked)),
                "VISIBLE" => Some(Command::FaceSetsCreate(C::Visible)),
                "ALL" => Some(Command::FaceSetsCreate(C::All)),
                _ => None, /* SELECTION needs the edit-mode face selection */
            }
        }
        "sculpt.face_sets_init" => crate::face_set_init::InitMode::from_rna(&prop(props, "mode")?).map(Command::FaceSetsInit),
        "sculpt.mesh_filter" => match prop(props, "type")?.as_str() {
            "INFLATE" => Some(Command::MeshFilter(crate::sculpt_mesh_filter::FilterType::Inflate)),
            "SPHERE" => Some(Command::MeshFilter(crate::sculpt_mesh_filter::FilterType::Sphere)),
            "RANDOM" => Some(Command::MeshFilter(crate::sculpt_mesh_filter::FilterType::Random)),
            _ => None,
        },
        "object.select_all" => match prop(props, "action")?.as_str() {
            "SELECT" => Some(Command::SelectAll(crate::scene::SelectAction::Select)),
            "DESELECT" => Some(Command::SelectAll(crate::scene::SelectAction::Deselect)),
            "INVERT" => Some(Command::SelectAll(crate::scene::SelectAction::Invert)),
            _ => None,
        },
        "object.select_more" => Some(Command::SelectMoreLess(true)),
        "object.select_less" => Some(Command::SelectMoreLess(false)),
        "object.select_random" => Some(Command::SelectRandom),
        "object.location_clear" | "object.rotation_clear" | "object.scale_clear" if prop(props, "clear_delta")? == "False" => {
            use crate::scene::ClearKind;
            Some(Command::ClearTransform(match idname {
                "object.location_clear" => ClearKind::Location,
                "object.rotation_clear" => ClearKind::Rotation,
                _ => ClearKind::Scale,
            }))
        }
        "object.transform_apply" => {
            let flag = |n: &str| Some(prop(props, n)? == "True");
            Some(Command::ApplyTransform(crate::scene::ApplyMask { location: flag("location")?, rotation: flag("rotation")?, scale: flag("scale")? }))
        }
        "object.parent_clear" if prop(props, "type")? == "CLEAR_INVERSE" => Some(Command::ParentClearInverse),
        "object.duplicate_move" => Some(Command::Duplicate),
        "object.join" => Some(Command::Join),
        "object.delete" => Some(Command::Delete),
        "object.select_hierarchy" => Some(Command::SelectHierarchy {
            to_parent: match prop(props, "direction")?.as_str() {
                "PARENT" => true,
                "CHILD" => false,
                _ => return None,
            },
            extend: prop(props, "extend")? == "True",
        }),
        "sculpt.mask_init" => match prop(props, "mode")?.as_str() {
            "RANDOM_PER_VERTEX" => Some(Command::MaskInit(InitMode::RandomPerVertex)),
            "RANDOM_PER_LOOSE_PART" => Some(Command::MaskInit(InitMode::RandomPerLoosePart)),
            _ => None,
        },
        _ => None,
    }
}

/// The `action` enum of the hide operators (`VisAction`, paint_hide.cc).
fn vis_action(props: &[(&str, &str)]) -> Option<VisAction> {
    match prop(props, "action")?.as_str() {
        "HIDE" => Some(VisAction::Hide),
        "SHOW" => Some(VisAction::Show),
        _ => None,
    }
}

/// The command of a menu row, if it is an operator row this port can run.
pub fn command_of_row(row: &Row) -> Option<Command> {
    match row {
        Row::Operator { idname, props, .. } => command_of(idname, props),
        _ => None,
    }
}

/// `BLI_time_now_seconds()`, the seed of `sculpt_mask_init_exec`.
fn mask_seed() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0)
}

/// Runs `cmd` on the active object; true when the scene changed.
pub fn apply(scene: &mut Scene, cmd: Command) -> bool {
    apply_with_symmetry(scene, cmd, [false; 3])
}

/// `apply()` with the sculpt symmetry axes (`Mesh::symmetry`), which the pivot operator reads.
pub fn apply_with_symmetry(scene: &mut Scene, cmd: Command, symm: [bool; 3]) -> bool {
    match cmd {
        Command::SetPivot(m) => scene.set_pivot_active(m, symm),
        Command::MaskFill => scene.fill_mask_active(),
        Command::MaskClear => scene.clear_mask_active(),
        Command::MaskInvert => scene.invert_mask_active(),
        Command::MaskFilter(f) => scene.mask_filter_active(f),
        Command::HideShowAll(a) => scene.visibility_edit_active(|m, v, _| sculpt_hide::hide_show_all(m, v, a)),
        Command::HideShowMasked(a) => scene.visibility_edit_active(|m, v, mask| sculpt_hide::hide_show_masked(m, v, a, Some(mask))),
        Command::MeshFilter(_) => false,
        Command::SelectAll(a) => scene.select_all(a),
        Command::SelectMoreLess(select) => scene.select_more_less(select),
        Command::SelectHierarchy { to_parent, extend } => scene.select_hierarchy(to_parent, extend),
        Command::SelectRandom => scene.select_random(0.5) > 0,
        Command::ClearTransform(k) => scene.clear_transform(k) > 0,
        Command::ApplyTransform(m) => scene.apply_transform(m) > 0,
        Command::ParentClearInverse => scene.parent_clear_inverse() > 0,
        Command::Duplicate => scene.duplicate_selected() > 0,
        Command::Join => scene.join_selected() > 0,
        Command::Delete => scene.delete_selected() > 0,
        Command::FaceSetsInit(m) => scene.face_sets_init_active(m),
        Command::FaceSetsRandomizeColors => scene.face_sets_randomize_colors_active(),
        Command::FaceSetsCreate(m) => scene.face_sets_create_active(m),
        Command::MaskInit(m) => scene.mask_init_active(m, mask_seed()),
        Command::VisibilityInvert => scene.visibility_edit_active(|m, v, _| sculpt_hide::invert_visibility(m, v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mask_menu;
    use crate::mesh::Primitive;

    fn scene() -> Scene {
        let mut scene = Scene::startup();
        let id = scene.add_primitive(Primitive::UvSphere, [0.0, 0.0, 0.0]);
        scene.active = Some(id);
        scene
    }

    fn cmd(label: &str) -> Option<Command> {
        mask_menu::ROWS
            .iter()
            .find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label))
            .and_then(command_of_row)
    }

    #[test]
    fn contrast_rows_map_to_commands() {
        assert_eq!(cmd("Increase Contrast"), Some(Command::MaskFilter(MaskFilter::ContrastIncrease)));
        assert_eq!(cmd("Decrease Contrast"), Some(Command::MaskFilter(MaskFilter::ContrastDecrease)));
    }

    #[test]
    fn mask_rows_map_to_commands() {
        assert_eq!(cmd("Fill Mask"), Some(Command::MaskFill));
        assert_eq!(cmd("Clear Mask"), Some(Command::MaskClear));
        assert_eq!(cmd("Invert Mask"), Some(Command::MaskInvert));
        assert_eq!(
            cmd("Smooth Mask"),
            Some(Command::MaskFilter(MaskFilter::Smooth))
        );
        assert_eq!(
            cmd("Sharpen Mask"),
            Some(Command::MaskFilter(MaskFilter::Sharpen))
        );
        assert_eq!(
            cmd("Grow Mask"),
            Some(Command::MaskFilter(MaskFilter::Grow))
        );
        assert_eq!(
            cmd("Shrink Mask"),
            Some(Command::MaskFilter(MaskFilter::Shrink))
        );
    }

    #[test]
    fn hide_rows_map_and_change_visibility() {
        use crate::sculpt_menu;
        let find = |label: &str| {
            sculpt_menu::rows(true).into_iter().find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label)).and_then(|r| command_of_row(&r))
        };
        assert_eq!(find("Show All"), Some(Command::HideShowAll(VisAction::Show)));
        assert_eq!(find("Invert Visible"), Some(Command::VisibilityInvert));
        assert_eq!(find("Hide Masked"), Some(Command::HideShowMasked(VisAction::Hide)));
        let mut s = scene();
        assert!(!apply(&mut s, Command::HideShowAll(VisAction::Show)), "nothing hidden");
        assert!(!apply(&mut s, Command::HideShowMasked(VisAction::Hide)), "nothing masked");
        assert!(apply(&mut s, Command::HideShowAll(VisAction::Hide)));
        assert!(s.active_object().unwrap().vis.hide_vert.as_ref().unwrap().iter().all(|&h| h));
        assert!(apply(&mut s, Command::HideShowAll(VisAction::Show)));
        assert_eq!(s.active_object().unwrap().vis, Default::default());
        assert!(apply(&mut s, Command::VisibilityInvert));
        assert!(s.undo() && s.active_object().unwrap().vis == Default::default());
    }

    #[test]
    fn random_mask_rows_run_mask_init() {
        let row = |l: &str| mask_menu::RANDOM_MASK.iter().find(|r| matches!(r, Row::Operator { label, .. } if *label == l)).and_then(command_of_row);
        assert_eq!(row("Per Vertex"), Some(Command::MaskInit(InitMode::RandomPerVertex)));
        assert_eq!(row("Per Loose Part"), Some(Command::MaskInit(InitMode::RandomPerLoosePart)));
        assert_eq!(row("Per Face Set"), None, "no face set layer yet");
        let mut s = scene();
        assert!(apply(&mut s, Command::MaskInit(InitMode::RandomPerVertex)));
        assert!(s.has_mask());
        assert!(s.undo() && !s.has_mask());
    }

    #[test]
    fn inflate_filter_row_maps_to_the_modal() {
        assert_eq!(command_of("sculpt.mesh_filter", &[("type", "INFLATE")]), Some(Command::MeshFilter(crate::sculpt_mesh_filter::FilterType::Inflate)));
        assert_eq!(command_of("sculpt.mesh_filter", &[("type", "SMOOTH")]), None);
    }

    #[test]
    fn sphere_and_random_filter_rows_map_to_the_modal() {
        use crate::sculpt_mesh_filter::FilterType;
        assert_eq!(command_of("sculpt.mesh_filter", &[("type", "SPHERE")]), Some(Command::MeshFilter(FilterType::Sphere)));
        assert_eq!(command_of("sculpt.mesh_filter", &[("type", "RANDOM")]), Some(Command::MeshFilter(FilterType::Random)));
    }

    #[test]
    fn face_sets_init_rows_run_and_create_the_layer() {
        use crate::face_set_init::InitMode as M;
        for (label, mode) in [("By Loose Parts", M::LooseParts), ("By Face Set Boundaries", M::FaceSetBoundaries), ("By Materials", M::Materials), ("By Normals", M::Normals), ("By UV Seams", M::UvSeams), ("By Edge Creases", M::Creases), ("By Edge Bevel Weight", M::BevelWeight), ("By Sharp Edges", M::SharpEdges)] {
            let row = crate::face_sets_menu::FACE_SETS_INIT.iter().find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label)).unwrap();
            assert_eq!(command_of_row(row), Some(Command::FaceSetsInit(mode)), "{label}");
        }
        let mut s = scene();
        assert!(apply(&mut s, Command::FaceSetsInit(M::LooseParts)));
        let o = &s.objects[s.index_of(s.active.unwrap()).unwrap()];
        assert_eq!(o.face_sets.len(), o.mesh().unwrap().faces.len());
        assert!(o.face_sets.iter().all(|&f| f == 1), "a sphere is one loose part");
    }

    #[test]
    fn face_sets_create_rows_make_a_new_set() {
        use crate::face_set_create::CreateMode as C;
        assert_eq!(command_of("sculpt.face_sets_create", &[("mode", "ALL")]), Some(Command::FaceSetsCreate(C::All)));
        assert_eq!(command_of("sculpt.face_sets_create", &[("mode", "MASKED")]), Some(Command::FaceSetsCreate(C::Masked)));
        assert_eq!(command_of("sculpt.face_sets_create", &[("mode", "SELECTION")]), None);
        let mut s = scene();
        assert!(!apply(&mut s, Command::FaceSetsCreate(C::Masked)), "no mask: nothing changes");
        assert!(apply(&mut s, Command::FaceSetsCreate(C::All)));
        let o = &s.objects[s.index_of(s.active.unwrap()).unwrap()];
        assert!(o.face_sets.iter().all(|&f| f == 2 || f == 1) && !o.face_sets.is_empty());
        assert!(apply(&mut s, Command::FaceSetsCreate(C::Visible)), "nothing hidden: the existing layer is removed");
        assert!(s.objects[s.index_of(s.active.unwrap()).unwrap()].face_sets.is_empty());
        assert!(!apply(&mut s, Command::FaceSetsCreate(C::Visible)), "no layer left to remove");
    }

    #[test]
    fn randomize_colors_needs_a_face_set_layer_then_advances_the_seed() {
        let mut s = scene();
        assert!(!apply(&mut s, Command::FaceSetsRandomizeColors));
        assert!(apply(&mut s, Command::FaceSetsInit(crate::face_set_init::InitMode::LooseParts)));
        assert!(apply(&mut s, Command::FaceSetsRandomizeColors));
        let o = &s.objects[s.index_of(s.active.unwrap()).unwrap()];
        assert_eq!(o.face_sets_color_seed, 1);
    }

    #[test]
    fn pivot_rows_without_a_cursor_run() {
        for (label, mode) in [("Pivot to Origin", PivotMode::Origin), ("Pivot to Unmasked", PivotMode::Unmasked), ("Pivot to Mask Border", PivotMode::MaskBorder)] {
            let row = crate::sculpt_submenus::SET_PIVOT.iter().find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label)).unwrap();
            assert_eq!(command_of_row(row), Some(Command::SetPivot(mode)), "{label}");
        }
        for label in ["Pivot to Active Vertex", "Pivot to Surface Under Cursor"] {
            let row = crate::sculpt_submenus::SET_PIVOT.iter().find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label)).unwrap();
            assert_eq!(command_of_row(row), None, "{label}");
        }
        let mut s = scene();
        assert!(apply(&mut s, Command::SetPivot(PivotMode::Unmasked)), "a sphere's mean is near but not exactly the origin pivot");
        let id = s.active.unwrap();
        let o = &s.objects[s.index_of(id).unwrap()];
        assert!(o.sculpt_pivot.iter().all(|c| c.abs() < 0.2));
    }

    #[test]
    fn unavailable_rows_have_no_command() {
        for l in [
            "Box Mask",
            "Mask Extract",
            "Mask by Color",
        ] {
            assert_eq!(cmd(l), None, "{l}");
        }
    }

    #[test]
    fn fill_clear_invert_change_the_mask() {
        let mut s = scene();
        let n = s.active_object().unwrap().mesh().unwrap().verts.len();
        assert!(apply(&mut s, Command::MaskFill));
        assert_eq!(s.active_object().unwrap().mask, vec![1.0; n]);
        assert!(!apply(&mut s, Command::MaskFill), "already full");
        assert!(apply(&mut s, Command::MaskInvert));
        assert!(!s.has_mask());
        assert!(apply(&mut s, Command::MaskFill) && apply(&mut s, Command::MaskClear));
        assert!(!s.has_mask());
        assert!(s.undo());
        assert!(s.has_mask(), "undo brings the filled mask back");
    }

    #[test]
    fn filters_need_a_mask() {
        let mut s = scene();
        assert!(!apply(&mut s, Command::MaskFilter(MaskFilter::Grow)));
        apply(&mut s, Command::MaskFill);
        assert!(apply(&mut s, Command::MaskFilter(MaskFilter::Smooth)));
    }

    #[test]
    fn select_menu_rows_map_to_commands() {
        use crate::scene::SelectAction as A;
        let c = |id: &str, p: &[(&str, &str)]| command_of(id, p);
        assert_eq!(c("object.select_all", &[("action", "SELECT")]), Some(Command::SelectAll(A::Select)));
        assert_eq!(c("object.select_all", &[("action", "DESELECT")]), Some(Command::SelectAll(A::Deselect)));
        assert_eq!(c("object.select_all", &[("action", "INVERT")]), Some(Command::SelectAll(A::Invert)));
        assert_eq!(c("object.select_more", &[]), Some(Command::SelectMoreLess(true)));
        assert_eq!(c("object.select_less", &[]), Some(Command::SelectMoreLess(false)));
        assert_eq!(
            c("object.select_hierarchy", &[("extend", "True"), ("direction", "CHILD")]),
            Some(Command::SelectHierarchy { to_parent: false, extend: true })
        );
        assert_eq!(c("object.select_pattern", &[]), None);
        assert_eq!(c("view3d.select_box", &[]), None);
    }

    #[test]
    fn select_all_none_invert_change_the_selection() {
        use crate::scene::SelectAction as A;
        let mut s = scene();
        s.select_all(A::Deselect);
        assert_eq!(s.selected_count(), 0);
        assert!(apply(&mut s, Command::SelectAll(A::Select)));
        let n = s.objects.iter().filter(|o| !o.hidden).count();
        assert_eq!(s.selected_count(), n);
        assert!(apply(&mut s, Command::SelectAll(A::Invert)));
        assert_eq!(s.selected_count(), 0);
        assert!(!apply(&mut s, Command::SelectAll(A::Deselect)));
    }

    #[test]
    fn select_random_picks_half_by_default_and_hierarchy_needs_an_active_parent() {
        let mut s = scene();
        for _ in 0..3 {
            s.add_primitive(Primitive::UvSphere, [1.0, 0.0, 0.0]);
        }
        s.select_all(crate::scene::SelectAction::Deselect);
        assert!(apply(&mut s, Command::SelectRandom));
        assert!(s.selected_count() > 0);
        assert!(!apply(&mut s, Command::SelectHierarchy { to_parent: true, extend: false }));
    }

    #[test]
    fn object_menu_rows_map_to_commands() {
        use crate::object_menu::{APPLY, CLEAR};
        let find = |rows: &[Row], l: &str| rows.iter().find(|r| matches!(r, Row::Operator { label, .. } if *label == l)).cloned().unwrap();
        assert_eq!(command_of_row(&find(CLEAR, "Location")), Some(Command::ClearTransform(crate::scene::ClearKind::Location)));
        assert_eq!(command_of_row(&find(APPLY, "Rotation & Scale")), Some(Command::ApplyTransform(crate::scene::ApplyMask { location: false, rotation: true, scale: true })));
        assert_eq!(command_of_row(&find(CLEAR, "Origin")), None);
    }
}
