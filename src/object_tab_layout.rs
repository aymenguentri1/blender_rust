//! Layout of the Object tab of the properties editor: panel order, labels and fields of
//! `OBJECT_PT_transform`, `OBJECT_PT_delta_transform`, `OBJECT_PT_relations`, `OBJECT_PT_collections`
//! and `OBJECT_PT_visibility` (scripts/startup/bl_ui/properties_object.py:36, :87, :133, :186, :413).
//! Pure data, no drawing. Labels not given by `text=` come from RNA (rna_object.cc), e.g. use_camera_lock_parent (:3168).
//! `transform_rows` adds the `lock_*` toggles (DECORATE_UNLOCKED icons, emboss=False) beside Location/Rotation/Scale
//! and the `ANIM_MT_rotation_mode_convert` menu (DOWNARROW_HLT) next to Mode. Panels of the Object tab with their `bl_order` (OBJECT_PT_motion_paths :374, OBJECT_PT_display :219 order 10,
//! OBJECT_PT_animation :632, OBJECT_PT_custom_props :636, rna_prop_ui.py:259 order 1000); only their headers are listed.
//! Not modelled: Parent Inverse Transform, Instancing, Line Art, Shading, Light Linking.

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationMode {
    Euler,
    Quaternion,
    AxisAngle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelId {
    Transform,
    DeltaTransform,
    Relations,
    Collections,
    MotionPaths,
    Visibility,
    Display,
    Animation,
    CustomProps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    pub id: PanelId,
    pub label: &'static str,
    pub parent: Option<PanelId>,
    pub default_closed: bool,
    /// `bl_order`: panels are sorted by it, ties keep registration order.
    pub order: i32,
}

/// Registration order of the panels in the file (children follow their parent in the UI).
pub const PANELS: [Panel; 9] = [
    Panel { id: PanelId::Transform, label: "Transform", parent: None, default_closed: false, order: 0 },
    Panel { id: PanelId::DeltaTransform, label: "Delta Transform", parent: Some(PanelId::Transform), default_closed: true, order: 0 },
    Panel { id: PanelId::Relations, label: "Relations", parent: None, default_closed: true, order: 0 },
    Panel { id: PanelId::Collections, label: "Collections", parent: None, default_closed: true, order: 0 },
    Panel { id: PanelId::MotionPaths, label: "Motion Paths", parent: None, default_closed: true, order: 0 },
    Panel { id: PanelId::Visibility, label: "Visibility", parent: None, default_closed: true, order: 0 },
    Panel { id: PanelId::Display, label: "Viewport Display", parent: None, default_closed: true, order: 10 },
    Panel { id: PanelId::Animation, label: "Animation", parent: None, default_closed: true, order: 999 },
    Panel { id: PanelId::CustomProps, label: "Custom Properties", parent: None, default_closed: true, order: 1000 },
];

/// A property row: RNA identifier and the label Blender shows (`text=` or the RNA name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub prop: &'static str,
    pub text: &'static str,
}

const fn f(prop: &'static str, text: &'static str) -> Field {
    Field { prop, text }
}

/// `OBJECT_PT_transform.draw`: Location, Rotation by mode, Mode, Scale.
pub fn transform_fields(mode: RotationMode) -> Vec<Field> {
    let rot = match mode {
        RotationMode::Quaternion => f("rotation_quaternion", "Rotation"),
        RotationMode::AxisAngle => f("rotation_axis_angle", "Rotation"),
        RotationMode::Euler => f("rotation_euler", "Rotation"),
    };
    vec![f("location", "Location"), rot, f("rotation_mode", "Mode"), f("scale", "Scale")]
}

/// A row of `OBJECT_PT_transform.draw` with the decorator buttons drawn after the field
/// (`row.prop(ob, "lock_*", text="", emboss=False, icon='DECORATE_UNLOCKED')`, properties_object.py:46-82).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub field: Field,
    /// RNA identifiers of the lock toggles; stacked in a column when there are two (quaternion, axis-angle).
    pub locks: Vec<&'static str>,
    /// Menu button after the field (`ANIM_MT_rotation_mode_convert`, icon DOWNARROW_HLT).
    pub menu: Option<&'static str>,
}

/// `OBJECT_PT_transform.draw` rows with their lock toggles and the convert menu.
pub fn transform_rows(mode: RotationMode) -> Vec<Row> {
    let locks = |mode: RotationMode, prop: &str| -> Vec<&'static str> {
        match (prop, mode) {
            ("location", _) => vec!["lock_location"],
            ("scale", _) => vec!["lock_scale"],
            ("rotation_mode", _) => vec![],
            (_, RotationMode::Euler) => vec!["lock_rotation"],
            (_, _) => vec!["lock_rotation_w", "lock_rotation"],
        }
    };
    transform_fields(mode)
        .into_iter()
        .map(|field| {
            let menu = (field.prop == "rotation_mode").then_some("ANIM_MT_rotation_mode_convert");
            Row { field, locks: locks(mode, field.prop), menu }
        })
        .collect()
}

/// `OBJECT_PT_delta_transform.draw`: no rotation row in axis-angle mode.
pub fn delta_fields(mode: RotationMode) -> Vec<Field> {
    let mut v = vec![f("delta_location", "Location")];
    match mode {
        RotationMode::Quaternion => v.push(f("delta_rotation_quaternion", "Rotation")),
        RotationMode::AxisAngle => {}
        RotationMode::Euler => v.push(f("delta_rotation_euler", "Rotation")),
    }
    v.push(f("delta_scale", "Scale"));
    v
}

/// `OBJECT_PT_relations.draw`: three columns of the grid flow, separated groups.
pub fn relations_columns() -> [Vec<Field>; 3] {
    [
        vec![
            f("parent", "Parent"),
            f("parent_type", "Parent Type"),
            f("use_camera_lock_parent", "Camera Parent Lock"),
        ],
        vec![f("track_axis", "Tracking Axis"), f("up_axis", "Up Axis")],
        vec![f("pass_index", "Pass Index")],
    ]
}

/// Whether the Visibility panel shows for a render engine (`COMPAT_ENGINES`).
pub fn visibility_poll(engine: &str) -> bool {
    matches!(engine, "BLENDER_RENDER" | "BLENDER_EEVEE" | "BLENDER_WORKBENCH")
}

/// A heading plus its checkbox rows; `invert` marks `invert_checkbox=True`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub heading: Option<&'static str>,
    pub rows: Vec<(Field, bool)>,
}

/// `OBJECT_PT_visibility.draw` without engine-specific ray visibility groups (Workbench path).
pub fn visibility_groups() -> Vec<Group> {
    vec![
        Group {
            heading: None,
            rows: vec![(f("hide_select", "Selectable"), true), (f("hide_surface_pick", "Surface Picking"), true)],
        },
        Group {
            heading: Some("Show In"),
            rows: vec![(f("hide_viewport", "Viewports"), true), (f("hide_render", "Renders"), true)],
        },
        Group { heading: Some("Mask"), rows: vec![(f("is_holdout", "Holdout"), false)] },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_order_and_labels() {
        let l: Vec<_> = PANELS.iter().map(|p| p.label).collect();
        assert_eq!(l, ["Transform", "Delta Transform", "Relations", "Collections", "Motion Paths", "Visibility", "Viewport Display", "Animation", "Custom Properties"]);
        assert!(!PANELS[0].default_closed && PANELS[1..].iter().all(|p| p.default_closed));
        assert_eq!(PANELS[1].parent, Some(PanelId::Transform));
    }

    #[test]
    fn the_list_is_already_sorted_by_bl_order() {
        assert!(PANELS.windows(2).all(|w| w[0].order <= w[1].order));
    }

    #[test]
    fn rotation_row_follows_mode() {
        assert_eq!(transform_fields(RotationMode::Euler)[1].prop, "rotation_euler");
        assert_eq!(transform_fields(RotationMode::Quaternion)[1].prop, "rotation_quaternion");
        assert_eq!(transform_fields(RotationMode::AxisAngle)[1].prop, "rotation_axis_angle");
    }

    #[test]
    fn rows_carry_lock_decorators_and_the_convert_menu() {
        let r = transform_rows(RotationMode::Euler);
        assert_eq!(r.iter().map(|r| r.locks.clone()).collect::<Vec<_>>(), [vec!["lock_location"], vec!["lock_rotation"], vec![], vec!["lock_scale"]]);
        assert_eq!(r[2].menu, Some("ANIM_MT_rotation_mode_convert"));
        assert!(r[0].menu.is_none());
        assert_eq!(transform_rows(RotationMode::Quaternion)[1].locks, ["lock_rotation_w", "lock_rotation"]);
        assert_eq!(transform_rows(RotationMode::AxisAngle)[1].locks, ["lock_rotation_w", "lock_rotation"]);
    }

    #[test]
    fn delta_has_no_rotation_for_axis_angle() {
        assert_eq!(delta_fields(RotationMode::AxisAngle).len(), 2);
        assert_eq!(delta_fields(RotationMode::Euler)[1].prop, "delta_rotation_euler");
    }

    #[test]
    fn relations_columns_match_blender() {
        let c = relations_columns();
        assert_eq!(c[0][2].text, "Camera Parent Lock");
        assert_eq!(c[1][0].text, "Tracking Axis");
        assert_eq!(c[2][0].prop, "pass_index");
    }

    #[test]
    fn visibility_engines_and_groups() {
        assert!(visibility_poll("BLENDER_EEVEE") && !visibility_poll("CYCLES"));
        let g = visibility_groups();
        assert_eq!(g[1].heading, Some("Show In"));
        assert!(g[0].rows.iter().all(|(_, inv)| *inv));
    }
}
