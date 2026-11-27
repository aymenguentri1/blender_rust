//! Clicking a menu row (`ui_handle_menu_event` / `ui_apply_but_funcs_after`, interface_handlers.cc): an enabled
//! operator row runs and closes the menu, a checkbox toggles its property and closes it, a disabled row or a
//! label/separator does nothing and keeps it open, a submenu row opens its submenu. The wm operators wired by
//! name are WM_OT_quit_blender, WM_OT_splash, WM_OT_splash_about, WM_OT_window_fullscreen_toggle (wm_operators.cc).

use crate::topbar_menu::Item;

/// What the application does for a clicked row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    SplashScreen,
    AboutBlender,
    ToggleFullscreen,
    /// Any other operator, by idname (`wm.save_mainfile`, ...).
    Operator(&'static str),
    /// Flip the boolean property of a checkbox row.
    Toggle(&'static str),
    OpenSubmenu(&'static str),
}

/// Result of a click: the action and whether the menu closes afterwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activation {
    pub action: Action,
    pub close: bool,
}

const KEEP_OPEN: Activation = Activation {
    action: Action::None,
    close: false,
};

pub fn activate(item: &Item) -> Activation {
    match *item {
        Item::Op {
            idname, enabled, ..
        } => {
            if !enabled {
                return KEEP_OPEN;
            }
            let action = match idname {
                "wm.quit_blender" => Action::Quit,
                "wm.splash" => Action::SplashScreen,
                "wm.splash_about" => Action::AboutBlender,
                "wm.window_fullscreen_toggle" => Action::ToggleFullscreen,
                other => Action::Operator(other),
            };
            Activation {
                action,
                close: true,
            }
        }
        Item::Check { prop, .. } => Activation {
            action: Action::Toggle(prop),
            close: true,
        },
        Item::Sub { id, .. } => Activation {
            action: Action::OpenSubmenu(id),
            close: false,
        },
        Item::Separator | Item::Label { .. } => KEEP_OPEN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topbar_menu::{blender, file, window, Ctx};

    fn find(items: &[Item], label: &str) -> Item {
        *items
            .iter()
            .find(|i| match i {
                Item::Op { label: l, .. }
                | Item::Sub { label: l, .. }
                | Item::Check { label: l, .. } => *l == label,
                _ => false,
            })
            .unwrap()
    }

    #[test]
    fn enabled_row_runs_and_closes() {
        let a = activate(&find(&file(&Ctx::default()), "Quit"));
        assert_eq!(
            a,
            Activation {
                action: Action::Quit,
                close: true
            }
        );
    }

    #[test]
    fn wired_window_rows() {
        let b = blender();
        assert_eq!(
            activate(&find(&b, "Splash Screen")).action,
            Action::SplashScreen
        );
        assert_eq!(
            activate(&find(&b, "About Blender")).action,
            Action::AboutBlender
        );
        let w = window(&Ctx::default());
        assert_eq!(
            activate(&find(&w, "Toggle Window Fullscreen")).action,
            Action::ToggleFullscreen
        );
    }

    #[test]
    fn disabled_row_does_nothing() {
        let it = Item::Op {
            idname: "wm.save_mainfile",
            label: "Save",
            icon: None,
            shortcut: None,
            enabled: false,
        };
        assert_eq!(
            activate(&it),
            Activation {
                action: Action::None,
                close: false
            }
        );
    }

    #[test]
    fn checkbox_toggles_and_closes() {
        let it = Item::Check {
            prop: "show_statusbar",
            label: "Status Bar",
            checked: true,
        };
        assert_eq!(
            activate(&it),
            Activation {
                action: Action::Toggle("show_statusbar"),
                close: true
            }
        );
    }

    #[test]
    fn submenu_opens_and_label_ignored() {
        let it = Item::Sub {
            id: "TOPBAR_MT_file_new",
            label: "New",
            icon: None,
            shortcut: None,
        };
        assert_eq!(
            activate(&it),
            Activation {
                action: Action::OpenSubmenu("TOPBAR_MT_file_new"),
                close: false
            }
        );
        assert_eq!(activate(&Item::Separator).close, false);
        assert_eq!(activate(&Item::Label { label: "x" }).action, Action::None);
    }

    #[test]
    fn other_operator_passes_through() {
        let it = Item::Op {
            idname: "wm.save_mainfile",
            label: "Save",
            icon: None,
            shortcut: None,
            enabled: true,
        };
        assert_eq!(
            activate(&it),
            Activation {
                action: Action::Operator("wm.save_mainfile"),
                close: true
            }
        );
    }
}
