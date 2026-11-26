//! The "Screen" keymap, ported item by item (same order) from `km_screen()` in `blender_default.py`.
//! Param: `legacy` (default false).

use crate::keymap::{KeyMapItem as K, Keymap, Prop};

fn dir(idname: &'static str, kind: &'static str, d: &'static str) -> K {
    K::new(idname, kind, "PRESS").prop("direction", Prop::Str(d))
}

fn render(kind: &'static str, props: &[&'static str]) -> K {
    let mut k = K::new("render.render", kind, "PRESS");
    for p in props {
        k = k.prop(p, Prop::Bool(true));
    }
    k
}

/// `km_screen(params)`.
pub fn km_screen(legacy: bool) -> Keymap {
    let mut items = vec![
        // Animation
        K::new("screen.animation_step", "TIMER0", "ANY").any_modifier(),
        K::new("screen.region_blend", "TIMERREGION", "ANY").any_modifier(),
        // Full screen and cycling
        dir("screen.space_context_cycle", "TAB", "NEXT").ctrl(),
        dir("screen.space_context_cycle", "TAB", "PREV").shift().ctrl(),
        dir("screen.workspace_cycle", "PAGE_DOWN", "NEXT").ctrl(),
        dir("screen.workspace_cycle", "PAGE_UP", "PREV").ctrl(),
        // Quad view
        K::new("screen.region_quadview", "Q", "PRESS").ctrl().alt(),
        // Repeat last
        K::new("screen.repeat_last", "R", "PRESS").shift().repeat(),
        // Files
        K::new("file.execute", "RET", "PRESS"),
        K::new("file.execute", "NUMPAD_ENTER", "PRESS"),
        K::new("file.cancel", "ESC", "PRESS"),
        // Asset Catalog undo is only available in the asset browser, and should take priority over `ed.undo`.
        K::new("asset.catalog_undo", "Z", "PRESS").ctrl().repeat(),
        K::new("asset.catalog_redo", "Z", "PRESS").ctrl().shift().repeat(),
        // Undo
        K::new("ed.undo", "Z", "PRESS").ctrl().repeat(),
        K::new("ed.redo", "Z", "PRESS").shift().ctrl().repeat(),
        // Render
        render("F12", &["use_viewport"]),
        render("F12", &["animation", "use_viewport"]).ctrl(),
        render("F12", &["use_sequencer_scene", "use_viewport"]).alt(),
        render("F12", &["animation", "use_sequencer_scene", "use_viewport"]).ctrl().alt(),
        K::new("render.view_cancel", "ESC", "PRESS"),
        K::new("render.view_show", "F11", "PRESS"),
        K::new("render.play_rendered_anim", "F11", "PRESS").ctrl(),
    ];

    if !legacy {
        items.extend([
            K::new("screen.screen_full_area", "SPACE", "PRESS").ctrl(),
            K::new("screen.screen_full_area", "SPACE", "PRESS").ctrl().alt().prop("use_hide_panels", Prop::Bool(true)),
            K::new("screen.redo_last", "F9", "PRESS"),
        ]);
    } else {
        // Old keymap
        items.extend([
            K::new("ed.undo_history", "Z", "PRESS").ctrl().alt(),
            K::new("screen.screen_full_area", "UP_ARROW", "PRESS").ctrl(),
            K::new("screen.screen_full_area", "DOWN_ARROW", "PRESS").ctrl(),
            K::new("screen.screen_full_area", "SPACE", "PRESS").shift(),
            K::new("screen.screen_full_area", "F10", "PRESS").alt().prop("use_hide_panels", Prop::Bool(true)),
            K::new("screen.screen_set", "RIGHT_ARROW", "PRESS").ctrl().prop("delta", Prop::Int(1)),
            K::new("screen.screen_set", "LEFT_ARROW", "PRESS").ctrl().prop("delta", Prop::Int(-1)),
            K::new("screen.screenshot", "F3", "PRESS").ctrl(),
            K::new("screen.repeat_history", "R", "PRESS").ctrl().alt(),
            K::new("screen.region_flip", "F5", "PRESS"),
            K::new("screen.redo_last", "F6", "PRESS"),
            K::new("script.reload", "F8", "PRESS"),
        ]);
    }

    // Preferences.
    items.push(if !legacy {
        K::new("screen.userpref_show", "COMMA", "PRESS").ctrl()
    } else {
        K::new("screen.userpref_show", "U", "PRESS").ctrl().alt()
    });
    Keymap::new("Screen", items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Event, KeyMapItem};

    fn press(km: &Keymap, kind: &'static str, shift: bool, ctrl: bool, alt: bool, repeat: bool) -> Option<KeyMapItem> {
        let mut e = Event::new(kind, "PRESS");
        (e.shift, e.ctrl, e.alt, e.is_repeat) = (shift, ctrl, alt, repeat);
        km.match_item(&e).cloned()
    }

    #[test]
    fn undo_redo_and_catalog_priority() {
        let km = km_screen(false);
        assert_eq!(press(&km, "Z", false, true, false, false).unwrap().idname, "asset.catalog_undo"); // listed before ed.undo
        let items: Vec<_> = km.items.iter().map(|k| k.idname).collect();
        assert!(items.iter().position(|n| *n == "asset.catalog_undo") < items.iter().position(|n| *n == "ed.undo"));
        let redo = km.items.iter().find(|k| k.idname == "ed.redo").unwrap();
        assert_eq!((redo.kind, redo.shift, redo.ctrl, redo.repeat), ("Z", crate::keymap::Mod::Held, crate::keymap::Mod::Held, true));
        let undo = km.items.iter().find(|k| k.idname == "ed.undo").unwrap();
        assert!(undo.repeat && undo.ctrl == crate::keymap::Mod::Held && undo.shift == crate::keymap::Mod::Off);
    }

    #[test]
    fn render_and_cycling_properties() {
        let km = km_screen(false);
        let r = press(&km, "F12", false, true, true, false).unwrap();
        assert_eq!(r.properties.iter().map(|p| p.0).collect::<Vec<_>>(), vec!["animation", "use_sequencer_scene", "use_viewport"]);
        assert_eq!(press(&km, "F12", false, false, false, false).unwrap().properties.len(), 1);
        assert_eq!(press(&km, "TAB", true, true, false, false).unwrap().property("direction"), Some(&Prop::Str("PREV")));
        assert_eq!(press(&km, "PAGE_DOWN", false, true, false, false).unwrap().idname, "screen.workspace_cycle");
        assert_eq!(press(&km, "R", true, false, false, true).unwrap().idname, "screen.repeat_last"); // allows key repeat
        assert!(press(&km, "ESC", false, false, false, false).unwrap().idname == "file.cancel"); // first ESC item wins
    }

    #[test]
    fn legacy_and_preferences() {
        let (new, old) = (km_screen(false), km_screen(true));
        assert_eq!(press(&new, "F9", false, false, false, false).unwrap().idname, "screen.redo_last");
        assert_eq!(press(&old, "F9", false, false, false, false), None);
        assert_eq!(press(&old, "F6", false, false, false, false).unwrap().idname, "screen.redo_last");
        assert_eq!(press(&new, "COMMA", false, true, false, false).unwrap().idname, "screen.userpref_show");
        assert_eq!(press(&old, "COMMA", false, true, false, false), None);
        assert_eq!(press(&old, "U", false, true, true, false).unwrap().idname, "screen.userpref_show");
        assert_eq!(new.items.len(), 22 + 3 + 1);
        assert_eq!(old.items.len(), 22 + 12 + 1);
        assert_eq!(press(&new, "SPACE", false, true, false, false).unwrap().idname, "screen.screen_full_area");
    }
}
