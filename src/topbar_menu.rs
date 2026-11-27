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
    /// `layout.menu(id)`: a row that opens a submenu.
    Sub {
        id: &'static str,
        label: &'static str,
        icon: Option<&'static str>,
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
            Item::Sub { id, label, .. } => Item::Sub {
                id,
                label,
                icon: Some(icon),
            },
            other => other,
        }
    }
    const fn key(self, shortcut: &'static str) -> Item {
        match self {
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
}
