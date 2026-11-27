//! Contents of the topbar menus (`TOPBAR_MT_blender`, `_edit`, `_render`, `_window`, `_help`, space_topbar.py).
//! A menu is a list of `Item`s in the order of its `draw()`: operators (label, icon, shortcut), separators,
//! submenus and property checkboxes. Labels without `text=` in the python are the operator names (wm_operators.cc,
//! screen_ops.cc, ed_undo.cc). Deviations: shortcuts are the default keymap strings written out, not looked up;
//! `show_developer_ui` is off, `use_multiview` is off and the sequencer rows are left out (no sequencer scene).

/// One row of a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// `layout.operator(idname, text, icon)`.
    Op {
        idname: &'static str,
        label: &'static str,
        icon: Option<&'static str>,
        shortcut: Option<&'static str>,
        enabled: bool,
    },
    /// `layout.separator()`.
    Separator,
    /// `layout.label(text=..)`.
    Label { label: &'static str },
    /// `layout.menu(id)`: a row that opens a submenu.
    Sub {
        id: &'static str,
        label: &'static str,
        icon: Option<&'static str>,
        shortcut: Option<&'static str>,
    },
    /// `layout.prop(.., bool)`: a row with a checkbox.
    Check {
        prop: &'static str,
        label: &'static str,
        checked: bool,
    },
}

/// The state the menus read (`context`, `tool_settings`, `scene.render`, `screen`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ctx {
    pub can_undo: bool,
    pub can_redo: bool,
    pub lock_object_mode: bool,
    pub lock_interface: bool,
    pub show_statusbar: bool,
    /// `sys.platform` starts with `win`: the Window menu has Toggle System Console.
    pub windows: bool,
    /// `bpy.data.is_saved`: Revert and Save Incremental need a saved file.
    pub is_saved: bool,
    /// `bpy.data.use_autopack`.
    pub use_autopack: bool,
}

impl Default for Ctx {
    fn default() -> Self {
        Ctx {
            can_undo: true,
            can_redo: false,
            lock_object_mode: true,
            lock_interface: false,
            show_statusbar: true,
            windows: cfg!(windows),
            is_saved: false,
            use_autopack: false,
        }
    }
}

const fn op(idname: &'static str, label: &'static str) -> Item {
    Item::Op {
        idname,
        label,
        icon: None,
        shortcut: None,
        enabled: true,
    }
}

impl Item {
    const fn icon(self, icon: &'static str) -> Item {
        match self {
            Item::Op {
                idname,
                label,
                shortcut,
                enabled,
                ..
            } => Item::Op {
                idname,
                label,
                icon: Some(icon),
                shortcut,
                enabled,
            },
            Item::Sub {
                id,
                label,
                shortcut,
                ..
            } => Item::Sub {
                id,
                label,
                icon: Some(icon),
                shortcut,
            },
            other => other,
        }
    }
    const fn key(self, shortcut: &'static str) -> Item {
        match self {
            Item::Sub {
                id, label, icon, ..
            } => Item::Sub {
                id,
                label,
                icon,
                shortcut: Some(shortcut),
            },
            Item::Op {
                idname,
                label,
                icon,
                enabled,
                ..
            } => Item::Op {
                idname,
                label,
                icon,
                shortcut: Some(shortcut),
                enabled,
            },
            other => other,
        }
    }
    const fn enabled(self, on: bool) -> Item {
        match self {
            Item::Op {
                idname,
                label,
                icon,
                shortcut,
                ..
            } => Item::Op {
                idname,
                label,
                icon,
                shortcut,
                enabled: on,
            },
            other => other,
        }
    }
}

const fn sub(id: &'static str, label: &'static str) -> Item {
    Item::Sub {
        id,
        label,
        icon: None,
        shortcut: None,
    }
}

/// `TOPBAR_MT_blender`.
pub fn blender() -> Vec<Item> {
    vec![
        op("wm.splash", "Splash Screen"),
        op("wm.splash_about", "About Blender"),
        Item::Separator,
        op(
            "preferences.app_template_install",
            "Install Application Template...",
        ),
        Item::Separator,
        sub("TOPBAR_MT_blender_system", "System"),
    ]
}

/// `TOPBAR_MT_edit`.
pub fn edit(c: &Ctx) -> Vec<Item> {
    vec![
        op("ed.undo", "Undo")
            .icon("loop_back")
            .key("Ctrl Z")
            .enabled(c.can_undo),
        op("ed.redo", "Redo")
            .icon("loop_forwards")
            .key("Shift Ctrl Z")
            .enabled(c.can_redo),
        sub("TOPBAR_MT_undo_history", "Undo History"),
        Item::Separator,
        op("screen.redo_last", "Adjust Last Operation...").key("F9"),
        op("screen.repeat_last", "Repeat Last").key("Shift R"),
        op("screen.repeat_history", "Repeat History..."),
        Item::Separator,
        op("wm.search_menu", "Menu Search...")
            .icon("viewzoom")
            .key("F3"),
        Item::Separator,
        op("wm.call_panel", "Rename Active Item...").key("F2"),
        op("wm.batch_rename", "Batch Rename...").key("Ctrl F2"),
        Item::Separator,
        Item::Check {
            prop: "tool_settings.lock_object_mode",
            label: "Lock Object Modes",
            checked: c.lock_object_mode,
        },
        Item::Separator,
        op("screen.userpref_show", "Preferences...")
            .icon("preferences")
            .key("Ctrl ,"),
    ]
}

/// `TOPBAR_MT_render`.
pub fn render(c: &Ctx) -> Vec<Item> {
    vec![
        op("render.render", "Render Image")
            .icon("render_still")
            .key("F12"),
        op("render.render_animation", "Render Animation")
            .icon("render_animation")
            .key("Ctrl F12"),
        Item::Separator,
        op("sound.mixdown", "Render Audio..."),
        Item::Separator,
        op("render.view_show", "View Render").key("F11"),
        op("render.play_rendered_anim", "View Animation").key("Ctrl F11"),
        Item::Separator,
        Item::Check {
            prop: "render.use_lock_interface",
            label: "Lock Interface",
            checked: c.lock_interface,
        },
    ]
}

/// `TOPBAR_MT_window`.
pub fn window(c: &Ctx) -> Vec<Item> {
    let mut v = vec![
        op("wm.window_new", "New Window"),
        op("wm.window_new_main", "New Main Window"),
        Item::Separator,
        op("wm.window_fullscreen_toggle", "Toggle Window Fullscreen").icon("fullscreen_enter"),
        Item::Separator,
        op("screen.workspace_cycle_next", "Next Workspace").key("Ctrl Page Down"),
        op("screen.workspace_cycle_prev", "Previous Workspace").key("Ctrl Page Up"),
        Item::Separator,
        Item::Check {
            prop: "screen.show_statusbar",
            label: "Show Status Bar",
            checked: c.show_statusbar,
        },
        Item::Separator,
        op("screen.screenshot", "Save Screenshot..."),
        op("screen.screenshot_area", "Save Screenshot (Editor)..."),
    ];
    if c.windows {
        v.push(Item::Separator);
        v.push(op("wm.console_toggle", "Toggle System Console").icon("console"));
    }
    v
}

/// `TOPBAR_MT_help`.
pub fn help() -> Vec<Item> {
    vec![
        op("wm.url_open_preset.manual", "Manual").icon("url"),
        op("wm.url_open.support", "Support"),
        op("wm.url_open.community", "User Communities"),
        op("wm.url_open.get_involved", "Get Involved"),
        op("wm.url_open_preset.release_notes", "Release Notes"),
        Item::Separator,
        op("wm.url_open_preset.bug", "Report a Bug").icon("url"),
        op("wm.sysinfo", "Save System Info..."),
    ]
}

/// `TOPBAR_MT_file`.
pub fn file(c: &Ctx) -> Vec<Item> {
    vec![
        sub("TOPBAR_MT_file_new", "New").icon("file_new"),
        op("wm.open_mainfile", "Open...")
            .icon("file_folder")
            .key("Ctrl O"),
        sub("TOPBAR_MT_file_open_recent", "Open Recent").key("Shift Ctrl O"),
        op("wm.revert_mainfile", "Revert").enabled(c.is_saved),
        sub("TOPBAR_MT_file_recover", "Recover"),
        Item::Separator,
        op("wm.save_mainfile", "Save")
            .icon("file_tick")
            .key("Ctrl S"),
        op("wm.save_as_mainfile", "Save As...").key("Shift Ctrl S"),
        op("wm.save_as_mainfile.copy", "Save Copy..."),
        op("wm.save_mainfile.incremental", "Save Incremental")
            .key("Ctrl Alt S")
            .enabled(c.is_saved),
        Item::Separator,
        op("wm.link", "Link...").icon("link_blend"),
        op("wm.append", "Append...").icon("append_blend"),
        sub("TOPBAR_MT_file_previews", "Data Previews"),
        Item::Separator,
        sub("TOPBAR_MT_file_project", "Project").icon("project"),
        Item::Separator,
        sub("TOPBAR_MT_file_import", "Import").icon("import"),
        sub("TOPBAR_MT_file_export", "Export").icon("export"),
        Item::Separator,
        sub("TOPBAR_MT_file_external_data", "External Data"),
        sub("TOPBAR_MT_file_cleanup", "Clean Up"),
        Item::Separator,
        sub("TOPBAR_MT_file_defaults", "Defaults"),
        Item::Separator,
        op("wm.quit_blender", "Quit").icon("quit").key("Ctrl Q"),
    ]
}

/// `TOPBAR_MT_file_new` with no application templates installed: only the General entry.
pub fn file_new() -> Vec<Item> {
    vec![op("wm.read_homefile", "General").icon("file_new")]
}

/// `TOPBAR_MT_file_recover`.
pub fn file_recover() -> Vec<Item> {
    vec![
        op("wm.recover_last_session", "Last Session").icon("recover_last"),
        op("wm.recover_auto_save", "Auto Save..."),
    ]
}

/// `TOPBAR_MT_file_defaults` with no application template.
pub fn file_defaults() -> Vec<Item> {
    vec![
        op("wm.save_homefile", "Save Startup File"),
        op("wm.read_factory_settings", "Load Factory Settings"),
    ]
}

/// `TOPBAR_MT_file_project` with an empty recent projects list.
pub fn file_project() -> Vec<Item> {
    vec![
        op("project.new", "New...").icon("add"),
        op("project.open", "Open...").icon("file_folder"),
        op("project.close", "Close"),
        Item::Separator,
        Item::Label {
            label: "No Recent Projects",
        },
    ]
}

/// `TOPBAR_MT_blender_system`.
pub fn blender_system() -> Vec<Item> {
    vec![
        op("script.reload", "Reload Scripts"),
        Item::Separator,
        op("wm.memory_statistics", "Memory Statistics"),
        op("wm.debug_menu", "Debug Menu"),
        sub("WM_OT_redraw_timer", "Redraw Timer"),
        Item::Separator,
        op("screen.spacedata_cleanup", "Clean Up Space Data"),
        op("wm.operator_presets_cleanup", "Clean Up Operator Presets"),
    ]
}

/// `TOPBAR_MT_file_cleanup`: starts with a separator in the python source.
pub fn file_cleanup() -> Vec<Item> {
    vec![
        Item::Separator,
        op("outliner.orphans_purge", "Purge Unused Data..."),
        op("outliner.orphans_manage", "Manage Unused Data..."),
    ]
}

/// `TOPBAR_MT_file_previews`.
pub fn file_previews() -> Vec<Item> {
    vec![
        op("wm.previews_ensure", "Refresh Data-Block Previews"),
        op("wm.previews_batch_generate", "Batch-Generate Previews..."),
        Item::Separator,
        op("wm.previews_clear", "Clear Data-Block Previews..."),
        op("wm.previews_batch_clear", "Batch-Clear Previews..."),
    ]
}

/// `TOPBAR_MT_file_external_data`: the pack rows are inactive while auto-pack is on.
pub fn file_external_data(c: &Ctx) -> Vec<Item> {
    let icon = if c.use_autopack {
        "checkbox_hlt"
    } else {
        "checkbox_dehlt"
    };
    vec![
        op("file.autopack_toggle", "Automatically Pack Resources").icon(icon),
        op("file.pack_all", "Pack Resources").enabled(!c.use_autopack),
        op("file.unpack_all", "Unpack Resources...").enabled(!c.use_autopack),
        Item::Separator,
        op("file.pack_libraries", "Pack Linked Libraries"),
        op("file.unpack_libraries", "Unpack Linked Libraries"),
        Item::Separator,
        op("file.make_paths_relative", "Make Paths Relative"),
        op("file.make_paths_absolute", "Make Paths Absolute"),
        Item::Separator,
        op("file.report_missing_files", "Report Missing Files"),
        op("file.find_missing_files", "Find Missing Files..."),
    ]
}

/// `TOPBAR_MT_file_import`. Deviation: only the Wavefront importer exists in this app.
pub fn file_import() -> Vec<Item> {
    vec![op("wm.obj_import", "Wavefront (.obj)")]
}

/// `TOPBAR_MT_file_export`; Export All Collections is inactive without export collections. Deviation: only Wavefront.
pub fn file_export() -> Vec<Item> {
    vec![
        op("wm.collection_export_all", "Export All Collections").enabled(false),
        Item::Separator,
        op("wm.obj_export", "Wavefront (.obj)"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[Item]) -> Vec<&'static str> {
        items
            .iter()
            .map(|i| match i {
                Item::Op { label, .. } | Item::Sub { label, .. } | Item::Check { label, .. } => {
                    *label
                }
                Item::Separator => "-",
                Item::Label { label } => *label,
            })
            .collect()
    }

    #[test]
    fn blender_menu_order() {
        assert_eq!(
            labels(&blender()),
            [
                "Splash Screen",
                "About Blender",
                "-",
                "Install Application Template...",
                "-",
                "System"
            ]
        );
    }

    #[test]
    fn edit_menu_order_and_shortcuts() {
        let m = edit(&Ctx::default());
        assert_eq!(
            labels(&m),
            [
                "Undo",
                "Redo",
                "Undo History",
                "-",
                "Adjust Last Operation...",
                "Repeat Last",
                "Repeat History...",
                "-",
                "Menu Search...",
                "-",
                "Rename Active Item...",
                "Batch Rename...",
                "-",
                "Lock Object Modes",
                "-",
                "Preferences..."
            ]
        );
        assert!(
            matches!(
                m[1],
                Item::Op {
                    enabled: false,
                    shortcut: Some("Shift Ctrl Z"),
                    ..
                }
            ),
            "Redo is disabled without redo steps"
        );
        assert!(matches!(
            m[0],
            Item::Op {
                enabled: true,
                icon: Some("loop_back"),
                ..
            }
        ));
        assert!(matches!(m[13], Item::Check { checked: true, .. }));
    }

    #[test]
    fn render_menu_order_and_lock_interface() {
        let m = render(&Ctx::default());
        assert_eq!(
            labels(&m),
            [
                "Render Image",
                "Render Animation",
                "-",
                "Render Audio...",
                "-",
                "View Render",
                "View Animation",
                "-",
                "Lock Interface"
            ]
        );
        assert!(matches!(m[8], Item::Check { checked: false, .. }));
        assert!(matches!(
            m[1],
            Item::Op {
                shortcut: Some("Ctrl F12"),
                ..
            }
        ));
    }

    #[test]
    fn window_menu_has_the_console_row_only_on_windows() {
        let win = window(&Ctx {
            windows: true,
            ..Ctx::default()
        });
        let other = window(&Ctx {
            windows: false,
            ..Ctx::default()
        });
        assert_eq!(labels(&win).last(), Some(&"Toggle System Console"));
        assert_eq!(labels(&other).last(), Some(&"Save Screenshot (Editor)..."));
        assert_eq!(
            labels(&other),
            [
                "New Window",
                "New Main Window",
                "-",
                "Toggle Window Fullscreen",
                "-",
                "Next Workspace",
                "Previous Workspace",
                "-",
                "Show Status Bar",
                "-",
                "Save Screenshot...",
                "Save Screenshot (Editor)..."
            ]
        );
    }

    #[test]
    fn help_menu_order() {
        assert_eq!(
            labels(&help()),
            [
                "Manual",
                "Support",
                "User Communities",
                "Get Involved",
                "Release Notes",
                "-",
                "Report a Bug",
                "Save System Info..."
            ]
        );
    }

    #[test]
    fn file_menu_order_icons_and_disabled_rows() {
        let m = file(&Ctx::default());
        assert_eq!(
            labels(&m),
            [
                "New",
                "Open...",
                "Open Recent",
                "Revert",
                "Recover",
                "-",
                "Save",
                "Save As...",
                "Save Copy...",
                "Save Incremental",
                "-",
                "Link...",
                "Append...",
                "Data Previews",
                "-",
                "Project",
                "-",
                "Import",
                "Export",
                "-",
                "External Data",
                "Clean Up",
                "-",
                "Defaults",
                "-",
                "Quit"
            ]
        );
        assert!(
            matches!(m[3], Item::Op { enabled: false, .. }),
            "Revert needs a saved file"
        );
        assert!(matches!(
            m[9],
            Item::Op {
                enabled: false,
                shortcut: Some("Ctrl Alt S"),
                ..
            }
        ));
        assert!(matches!(
            m[2],
            Item::Sub {
                shortcut: Some("Shift Ctrl O"),
                ..
            }
        ));
        assert!(matches!(
            m[25],
            Item::Op {
                icon: Some("quit"),
                shortcut: Some("Ctrl Q"),
                ..
            }
        ));
        let saved = file(&Ctx {
            is_saved: true,
            ..Ctx::default()
        });
        assert!(
            matches!(saved[3], Item::Op { enabled: true, .. })
                && matches!(saved[9], Item::Op { enabled: true, .. })
        );
    }

    #[test]
    fn system_menu_matches_the_screenshot_rows() {
        assert_eq!(
            labels(&blender_system()),
            [
                "Reload Scripts",
                "-",
                "Memory Statistics",
                "Debug Menu",
                "Redraw Timer",
                "-",
                "Clean Up Space Data",
                "Clean Up Operator Presets"
            ]
        );
    }

    #[test]
    fn file_submenus_match_the_python_lists() {
        assert_eq!(labels(&file_recover()), ["Last Session", "Auto Save..."]);
        assert_eq!(
            labels(&file_defaults()),
            ["Save Startup File", "Load Factory Settings"]
        );
        assert_eq!(
            labels(&file_project()),
            ["New...", "Open...", "Close", "-", "No Recent Projects"]
        );
        assert_eq!(
            labels(&file_cleanup()),
            ["-", "Purge Unused Data...", "Manage Unused Data..."]
        );
        assert_eq!(labels(&file_previews()).len(), 5);
        assert_eq!(labels(&file_external_data(&Ctx::default())).len(), 12);
    }

    #[test]
    fn autopack_toggles_the_icon_and_the_pack_rows() {
        let off = file_external_data(&Ctx::default());
        let on = file_external_data(&Ctx {
            use_autopack: true,
            ..Ctx::default()
        });
        assert!(
            matches!(
                off[0],
                Item::Op {
                    icon: Some("checkbox_dehlt"),
                    ..
                }
            ) && matches!(off[1], Item::Op { enabled: true, .. })
        );
        assert!(
            matches!(
                on[0],
                Item::Op {
                    icon: Some("checkbox_hlt"),
                    ..
                }
            ) && matches!(on[1], Item::Op { enabled: false, .. })
        );
    }
}
