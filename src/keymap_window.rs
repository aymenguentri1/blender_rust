//! The "Window" keymap, ported item by item (same order) from `km_window()` in `blender_default.py`.
//! Params: `legacy` (default false), `spacebar_action` (default `'TOOL'`), `use_alt_click_leader` (default false).

use crate::keymap::{KeyMapItem as K, Keymap, Prop};
use crate::keymap_frames::SpacebarAction;

const SAVE_DIALOG: &str = "show_save_modified_images_dialog";

fn save_dialog(idname: &'static str, kind: &'static str) -> K {
    K::new(idname, kind, "PRESS").prop(SAVE_DIALOG, Prop::Bool(true))
}

/// `op_panel(menu, kmi_args, kmi_data)`.
fn op_panel(panel: &'static str, kind: &'static str, value: &'static str, data: &[(&'static str, Prop)]) -> K {
    let mut k = K::new("wm.call_panel", kind, value).prop("name", Prop::Str(panel));
    for (n, v) in data {
        k = k.prop(n, v.clone());
    }
    k
}

fn scale_sens(kind: &'static str, value: f64) -> K {
    K::new("wm.context_scale_float", kind, "PRESS")
        .prop("data_path", Prop::Str("preferences.inputs.ndof_translation_sensitivity"))
        .prop("value", Prop::Float(value))
}

/// `km_window(params)`.
pub fn km_window(spacebar_action: SpacebarAction, legacy: bool, use_alt_click_leader: bool) -> Keymap {
    let mut items: Vec<K> = Vec::new();

    if legacy {
        // Old shortcuts
        items.extend([
            K::new("wm.save_homefile", "U", "PRESS").ctrl(),
            K::new("wm.open_mainfile", "F1", "PRESS"),
            K::new("wm.link", "O", "PRESS").ctrl().alt(),
            K::new("wm.append", "F1", "PRESS").shift(),
            save_dialog("wm.save_mainfile", "W").ctrl(),
            save_dialog("wm.save_as_mainfile", "F2"),
            save_dialog("wm.save_as_mainfile", "S").ctrl().alt().prop("copy", Prop::Bool(true)),
            K::new("wm.window_new", "W", "PRESS").ctrl().alt(),
            K::new("wm.window_fullscreen_toggle", "F11", "PRESS").alt(),
            K::new("wm.doc_view_manual_ui_context", "F1", "PRESS").alt(),
            K::new("wm.search_menu", "SPACE", "PRESS"),
            K::new("wm.redraw_timer", "T", "PRESS").ctrl().alt(),
            K::new("wm.debug_menu", "D", "PRESS").ctrl().alt(),
        ]);
    }

    // File operations
    items.extend([
        K::op_menu("TOPBAR_MT_file_new", "N", "PRESS").ctrl(),
        K::op_menu("TOPBAR_MT_file_open_recent", "O", "PRESS").shift().ctrl(),
        K::new("wm.open_mainfile", "O", "PRESS").ctrl(),
        save_dialog("wm.save_mainfile", "S").ctrl(),
        save_dialog("wm.save_as_mainfile", "S").shift().ctrl(),
        save_dialog("wm.save_mainfile", "S").ctrl().alt().prop("incremental", Prop::Bool(true)),
        K::new("wm.quit_blender", "Q", "PRESS").ctrl(),
        // Quick menu and toolbar
        K::op_menu("SCREEN_MT_user_menu", "Q", "PRESS"),
    ]);

    // Fast editor switching
    for (k, t) in [
        ("F1", "FILE_BROWSER"),
        ("F2", "CLIP_EDITOR"),
        ("F3", "NODE_EDITOR"),
        ("F4", "CONSOLE"),
        ("F5", "VIEW_3D"),
        ("F6", "GRAPH_EDITOR"),
        ("F7", "PROPERTIES"),
        ("F8", "SEQUENCE_EDITOR"),
        ("F9", "OUTLINER"),
        ("F10", "IMAGE_EDITOR"),
        ("F11", "TEXT_EDITOR"),
        ("F12", "DOPESHEET_EDITOR"),
    ] {
        items.push(K::new("screen.space_type_set_or_cycle", k, "PRESS").shift().prop("space_type", Prop::Str(t)));
    }

    // NDOF settings
    items.extend([
        op_panel("USERPREF_PT_ndof_settings", "NDOF_BUTTON_MENU", "PRESS", &[]),
        scale_sens("NDOF_BUTTON_PLUS", 1.1),
        scale_sens("NDOF_BUTTON_MINUS", 1.0 / 1.1),
        scale_sens("NDOF_BUTTON_PLUS", 1.5).shift(),
        scale_sens("NDOF_BUTTON_MINUS", 2.0 / 3.0).shift(),
        K::new("info.reports_display_update", "TIMER_REPORT", "ANY").any_modifier(),
    ]);

    if !legacy {
        // New shortcuts
        items.extend([
            K::new("wm.doc_view_manual_ui_context", "F1", "PRESS"),
            op_panel("TOPBAR_PT_name", "F2", "PRESS", &[("keep_open", Prop::Bool(false))]),
            K::new("wm.batch_rename", "F2", "PRESS").ctrl(),
            K::new("wm.search_menu", "F3", "PRESS"),
            K::op_menu("TOPBAR_MT_file_context_menu", "F4", "PRESS"),
            // Pass through when no tool-system exists or the fallback isn't available.
            K::new("wm.toolbar_fallback_pie", "W", "PRESS").alt(),
        ]);
        if use_alt_click_leader {
            // Alt as "Leader-Key".
            items.extend([K::new("wm.toolbar_prompt", "LEFT_ALT", "CLICK"), K::new("wm.toolbar_prompt", "RIGHT_ALT", "CLICK")]);
        }
        items.push(match spacebar_action {
            SpacebarAction::Tool => K::new("wm.toolbar", "SPACE", "PRESS"),
            SpacebarAction::Play => K::new("wm.toolbar", "SPACE", "PRESS").shift(),
            SpacebarAction::Search => K::new("wm.search_menu", "SPACE", "PRESS"),
        });
    }
    Keymap::new("Window", items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Event;

    fn press(km: &Keymap, kind: &'static str, shift: bool, ctrl: bool, alt: bool) -> Option<crate::keymap::KeyMapItem> {
        let mut e = Event::new(kind, "PRESS");
        (e.shift, e.ctrl, e.alt) = (shift, ctrl, alt);
        km.match_item(&e).cloned()
    }

    #[test]
    fn file_operations() {
        let km = km_window(SpacebarAction::Tool, false, false);
        let s = press(&km, "S", false, true, false).unwrap();
        assert_eq!((s.idname, s.property(SAVE_DIALOG)), ("wm.save_mainfile", Some(&Prop::Bool(true))));
        assert_eq!(s.property("incremental"), None);
        assert_eq!(press(&km, "S", true, true, false).unwrap().idname, "wm.save_as_mainfile");
        let inc = press(&km, "S", false, true, true).unwrap();
        assert_eq!((inc.idname, inc.property("incremental")), ("wm.save_mainfile", Some(&Prop::Bool(true))));
        assert_eq!(press(&km, "Q", false, true, false).unwrap().idname, "wm.quit_blender");
        assert_eq!(press(&km, "O", false, true, false).unwrap().idname, "wm.open_mainfile");
        assert_eq!(press(&km, "N", false, true, false).unwrap().property("name"), Some(&Prop::Str("TOPBAR_MT_file_new")));
        assert_eq!(press(&km, "Q", false, false, false).unwrap().property("name"), Some(&Prop::Str("SCREEN_MT_user_menu")));
    }

    #[test]
    fn editor_switching_and_function_keys() {
        let km = km_window(SpacebarAction::Tool, false, false);
        let f5 = press(&km, "F5", true, false, false).unwrap();
        assert_eq!((f5.idname, f5.property("space_type")), ("screen.space_type_set_or_cycle", Some(&Prop::Str("VIEW_3D"))));
        assert_eq!(press(&km, "F12", true, false, false).unwrap().property("space_type"), Some(&Prop::Str("DOPESHEET_EDITOR")));
        assert_eq!(press(&km, "F3", false, false, false).unwrap().idname, "wm.search_menu");
        assert_eq!(press(&km, "F2", false, true, false).unwrap().idname, "wm.batch_rename");
        assert_eq!(press(&km, "F2", false, false, false).unwrap().property("keep_open"), Some(&Prop::Bool(false)));
    }

    #[test]
    fn spacebar_action_alt_leader_and_legacy() {
        assert_eq!(press(&km_window(SpacebarAction::Tool, false, false), "SPACE", false, false, false).unwrap().idname, "wm.toolbar");
        assert_eq!(press(&km_window(SpacebarAction::Play, false, false), "SPACE", true, false, false).unwrap().idname, "wm.toolbar");
        assert_eq!(press(&km_window(SpacebarAction::Search, false, false), "SPACE", false, false, false).unwrap().idname, "wm.search_menu");
        let mut e = Event::new("LEFT_ALT", "CLICK");
        e.alt = true;
        assert!(km_window(SpacebarAction::Tool, false, false).match_item(&e).is_none());
        assert_eq!(km_window(SpacebarAction::Tool, false, true).match_item(&e).unwrap().idname, "wm.toolbar_prompt");
        let legacy = km_window(SpacebarAction::Tool, true, false);
        assert_eq!(press(&legacy, "F2", false, false, false).unwrap().idname, "wm.save_as_mainfile");
        assert_eq!(press(&legacy, "SPACE", false, false, false).unwrap().idname, "wm.search_menu");
        assert_eq!(press(&legacy, "W", false, true, false).unwrap().idname, "wm.save_mainfile");
    }

    #[test]
    fn ndof_and_timer_items() {
        let km = km_window(SpacebarAction::Tool, false, false);
        assert_eq!(press(&km, "NDOF_BUTTON_PLUS", false, false, false).unwrap().property("value"), Some(&Prop::Float(1.1)));
        assert_eq!(press(&km, "NDOF_BUTTON_MINUS", true, false, false).unwrap().property("value"), Some(&Prop::Float(2.0 / 3.0)));
        let mut t = Event::new("TIMER_REPORT", "PRESS");
        t.shift = true;
        assert_eq!(km.match_item(&t).unwrap().idname, "info.reports_display_update");
        assert_eq!(km.items.len(), 8 + 12 + 6 + 6 + 1);
    }
}
