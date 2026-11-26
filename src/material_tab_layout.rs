//! Layout of the Material tab of the properties editor (EEVEE): slot list buttons, edit-mode row, panel order and the
//! fields of `MATERIAL_PT_viewport` (scripts/startup/bl_ui/properties_material.py:44-175, :355). Pure data, no drawing.
//! The Surface panel only shows the node view of the output node's "Surface" input (`panel_node_draw`, :149); its
//! node widgets need the node tree and are not modelled, only the two fallback labels.

#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelId {
    ContextMaterial,
    Preview,
    Surface,
    ViewportDisplay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    pub id: PanelId,
    /// `bl_label`; the context panel has an empty label and `HIDE_HEADER`.
    pub label: &'static str,
    pub default_closed: bool,
    pub hide_header: bool,
}

/// Panels in draw order: `EEVEE_MATERIAL_PT_context_material`, `MATERIAL_PT_preview`, `EEVEE_MATERIAL_PT_surface`,
/// then `MATERIAL_PT_viewport` (`bl_order = 10`).
pub const PANELS: [Panel; 4] = [
    Panel { id: PanelId::ContextMaterial, label: "", default_closed: false, hide_header: true },
    Panel { id: PanelId::Preview, label: "Preview", default_closed: true, hide_header: false },
    Panel { id: PanelId::Surface, label: "Surface", default_closed: false, hide_header: false },
    Panel { id: PanelId::ViewportDisplay, label: "Viewport Display", default_closed: true, hide_header: false },
];

/// `EEVEE_MATERIAL_PT_context_material.poll` (:84): grease pencil objects and materials never show it.
pub fn context_panel_polls(has_object: bool, has_material: bool, object_is_grease_pencil: bool, material_is_grease_pencil: bool) -> bool {
    if material_is_grease_pencil || (has_object && object_is_grease_pencil) {
        return false;
    }
    has_object || has_material
}

/// Rows of the slot list (:100): 3, or 5 when the object has more than one slot (the move buttons need room).
pub fn slot_list_rows(slot_count: usize) -> usize {
    if slot_count > 1 { 5 } else { 3 }
}

/// A button of the column right of the slot list (:117-132), top to bottom; `None` is a separator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotButton {
    Add,
    Remove,
    ContextMenu,
    MoveUp,
    MoveDown,
}

pub fn slot_buttons(slot_count: usize) -> Vec<Option<SlotButton>> {
    let mut v = vec![Some(SlotButton::Add), Some(SlotButton::Remove), None, Some(SlotButton::ContextMenu)];
    if slot_count > 1 {
        v.extend([None, Some(SlotButton::MoveUp), Some(SlotButton::MoveDown)]);
    }
    v
}

/// Operator behind each slot button (`bl_idname`, direction for the move ones).
pub fn slot_button_operator(b: SlotButton) -> (&'static str, Option<&'static str>) {
    match b {
        SlotButton::Add => ("object.material_slot_add", None),
        SlotButton::Remove => ("object.material_slot_remove", None),
        SlotButton::ContextMenu => ("MATERIAL_MT_context_menu", None),
        SlotButton::MoveUp => ("object.material_slot_move", Some("UP")),
        SlotButton::MoveDown => ("object.material_slot_move", Some("DOWN")),
    }
}

/// Edit-mode row under the material selector (:140-145): `(operator, text)`. Font objects only get Assign.
pub fn edit_mode_buttons(is_font: bool) -> Vec<(&'static str, &'static str)> {
    let mut v = vec![("object.material_slot_assign", "Assign")];
    if !is_font {
        v.push(("object.material_slot_select", "Select"));
        v.push(("object.material_slot_deselect", "Deselect"));
    }
    v
}

/// `panel_node_draw` fallback label when the surface input cannot be shown (:156-160).
pub fn surface_fallback_label(has_output_node: bool, has_surface_input: bool) -> Option<&'static str> {
    match (has_output_node, has_surface_input) {
        (false, _) => Some("No output node"),
        (true, false) => Some("Incompatible output node"),
        (true, true) => None,
    }
}

/// `MATERIAL_PT_viewport.draw` (:371): RNA identifier and `text=` (empty = RNA name).
pub fn viewport_display_fields() -> [(&'static str, &'static str); 3] {
    [("diffuse_color", "Color"), ("metallic", ""), ("roughness", "")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panels_are_in_blender_order() {
        let labels: Vec<_> = PANELS.iter().map(|p| p.label).collect();
        assert_eq!(labels, ["", "Preview", "Surface", "Viewport Display"]);
        assert!(PANELS[0].hide_header && PANELS[1].default_closed && !PANELS[2].default_closed && PANELS[3].default_closed);
    }

    #[test]
    fn context_panel_poll_excludes_grease_pencil() {
        assert!(context_panel_polls(true, false, false, false));
        assert!(context_panel_polls(false, true, false, false));
        assert!(!context_panel_polls(false, false, false, false));
        assert!(!context_panel_polls(true, true, true, false));
        assert!(!context_panel_polls(true, true, false, true));
    }

    #[test]
    fn slot_list_is_taller_and_sortable_with_several_slots() {
        assert_eq!((slot_list_rows(0), slot_list_rows(1), slot_list_rows(2)), (3, 3, 5));
        assert_eq!(slot_buttons(1).len(), 4);
        let b = slot_buttons(2);
        assert_eq!(b.len(), 7);
        assert_eq!(&b[4..], [None, Some(SlotButton::MoveUp), Some(SlotButton::MoveDown)]);
        assert_eq!(slot_button_operator(SlotButton::MoveDown), ("object.material_slot_move", Some("DOWN")));
    }

    #[test]
    fn edit_mode_row_and_fallbacks() {
        assert_eq!(edit_mode_buttons(false).iter().map(|b| b.1).collect::<Vec<_>>(), ["Assign", "Select", "Deselect"]);
        assert_eq!(edit_mode_buttons(true), [("object.material_slot_assign", "Assign")]);
        assert_eq!(surface_fallback_label(false, false), Some("No output node"));
        assert_eq!(surface_fallback_label(true, false), Some("Incompatible output node"));
        assert_eq!(surface_fallback_label(true, true), None);
        assert_eq!(viewport_display_fields()[0], ("diffuse_color", "Color"));
    }
}
