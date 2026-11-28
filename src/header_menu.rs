//! State of the Sculpt, Mask and Face Sets pull-downs of the viewport header: which one is open, which submenu row is
//! open, and what a click does.
//!
//! Behaviour of `ui_handle_menu_event` (interface_handlers.cc): hovering a submenu row opens its submenu next to
//! it, hovering another row of the parent closes it, a click on an operator row runs it and closes every menu, a
//! click on a submenu row keeps it open, a click outside closes everything. Rows come from `sculpt_menu`,
//! `mask_menu`, `face_sets_menu` and `sculpt_submenus`.

use crate::area::Rcti;
use crate::row_menu::{self, Layout};
use crate::sculpt_menu::Row;
use crate::userdef::UserDef;
use crate::{face_sets_menu, mask_menu, sculpt_menu, sculpt_submenus};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Sculpt,
    Mask,
    FaceSets,
    View,
    Select,
    Object,
}

/// Rows of a header pull-down; `dyntopo` is the state of the Dynamic Topology checkbox.
pub fn rows_of(which: Which, dyntopo: bool) -> Vec<Row> {
    match which {
        Which::Sculpt => sculpt_menu::rows(dyntopo),
        Which::Mask => mask_menu::ROWS.to_vec(),
        Which::FaceSets => face_sets_menu::FACE_SETS.to_vec(),
        Which::View => crate::view_menu::rows(&crate::view_menu::ViewState::default()),
        Which::Select => crate::select_menu::SELECT_OBJECT.to_vec(),
        Which::Object => crate::object_menu::rows(None),
    }
}

/// Rows of the submenu `idname`; `None` for a menu this port does not have.
pub fn submenu_rows(idname: &str) -> Option<Vec<Row>> {
    let rows: &[Row] = match idname {
        "VIEW3D_MT_sculpt_transform" => sculpt_submenus::TRANSFORM,
        "VIEW3D_MT_sculpt_showhide" => sculpt_submenus::SHOWHIDE,
        "VIEW3D_MT_sculpt_trim" => sculpt_submenus::TRIM,
        "VIEW3D_MT_sculpt_set_pivot" => sculpt_submenus::SET_PIVOT,
        "VIEW3D_MT_face_sets_init" => face_sets_menu::FACE_SETS_INIT,
        "VIEW3D_MT_random_mask" => crate::mask_menu::RANDOM_MASK,
        "VIEW3D_MT_select_object_more_less" => crate::select_menu::MORE_LESS,
        "VIEW3D_MT_object_clear" => crate::object_menu::CLEAR,
        "VIEW3D_MT_object_apply" => crate::object_menu::APPLY,
        "VIEW3D_MT_object_showhide" => crate::object_menu::SHOWHIDE,
        "VIEW3D_MT_object_parent" => crate::object_menu::PARENT,
        "VIEW3D_MT_view_local" => crate::view_menu::LOCAL,
        "VIEW3D_MT_view_cameras" => crate::view_menu::CAMERAS,
        "VIEW3D_MT_view_viewpoint" => crate::view_menu::VIEWPOINT,
        "VIEW3D_MT_view_navigation" => crate::view_menu::NAVIGATION,
        "VIEW3D_MT_view_align" => crate::view_menu::ALIGN,
        "VIEW3D_MT_view_align_selected" => crate::view_menu::ALIGN_SELECTED,
        "VIEW3D_MT_view_regions" => crate::view_menu::REGIONS,
        _ => return None,
    };
    Some(rows.to_vec())
}

/// Whether a row can be used: a submenu row when the submenu exists, an operator row when `menu_dispatch` has a
/// command for it. The others are drawn dim and a click on them only keeps the menus as they are.
pub fn enabled(row: &Row) -> bool {
    match row {
        Row::Menu { idname, .. } => submenu_rows(idname).is_some(),
        r => crate::menu_dispatch::command_of_row(r).is_some() || view_command_of_row(r).is_some(),
    }
}

/// The View menu command of an operator row, if the port can run it.
pub fn view_command_of_row(row: &Row) -> Option<crate::view_menu_dispatch::ViewCommand> {
    match row {
        Row::Operator { idname, props, .. } => crate::view_menu_dispatch::command_of(idname, props),
        _ => None,
    }
}

/// What a click did.
#[derive(Clone, Debug, PartialEq)]
pub enum Click {
    /// Outside every menu: all menus closed.
    Closed,
    /// On a separator, a submenu row or the panel border: nothing happens, the menus stay open.
    Kept,
    /// On an operator row: the menus are closed and the row is to be run.
    Run(Row),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeaderMenu {
    pub which: Which,
    /// Window position of the top left corner of the pull-down (below its header button).
    pub anchor: [i32; 2],
    /// Index of the row of the pull-down whose submenu is open.
    pub sub: Option<usize>,
    /// What `VIEW3D_MT_view.draw` reads (only used by `Which::View`).
    pub view: crate::view_menu::ViewState,
}

pub struct Panels {
    pub root_rows: Vec<Row>,
    pub root: Layout,
    /// Rows and layout of the open submenu.
    pub sub: Option<(Vec<Row>, Layout)>,
}

impl HeaderMenu {
    pub fn open(which: Which, anchor: [i32; 2]) -> Self {
        Self {
            which,
            anchor,
            sub: None,
            view: Default::default(),
        }
    }

    /// The pull-down's rows: the View menu draws from `view`.
    fn rows(&self, dyntopo: bool) -> Vec<Row> {
        match self.which {
            Which::View => crate::view_menu::rows(&self.view),
            w => rows_of(w, dyntopo),
        }
    }

    /// Layout of the pull-down and of its open submenu in a `win` = `[w, h]` window.
    pub fn panels(&self, user: &UserDef, win: [i32; 2], dyntopo: bool) -> Panels {
        let root_rows = self.rows(dyntopo);
        let root = row_menu::layout(user, &root_rows, self.anchor, win[0], win[1]);
        let sub = self.sub.and_then(|i| {
            let Row::Menu { idname, .. } = root_rows.get(i)? else { return None };
            let rows = submenu_rows(idname)?;
            let rect = root.items.iter().find(|&&(_, k)| k == i)?.0;
            /* The submenu opens to the right of its row, its first row level with the parent row. */
            let l = row_menu::layout(user, &rows, [root.panel.xmax, rect.ymax], win[0], win[1]);
            Some((rows, l))
        });
        Panels {
            root_rows,
            root,
            sub,
        }
    }

    /// Mouse move: opens or closes the submenu according to the row under `cursor`.
    pub fn mouse_move(&mut self, user: &UserDef, win: [i32; 2], dyntopo: bool, cursor: [i32; 2]) {
        let p = self.panels(user, win, dyntopo);
        if let Some((_, l)) = &p.sub {
            if l.panel.isect_pt_v(cursor) {
                return;
            }
        }
        if let Some(i) = row_menu::item_at(&p.root, cursor) {
            self.sub = matches!(p.root_rows[i], Row::Menu { .. }).then_some(i);
        }
    }

    /// Left click at `cursor`.
    pub fn click(
        &mut self,
        user: &UserDef,
        win: [i32; 2],
        dyntopo: bool,
        cursor: [i32; 2],
    ) -> Click {
        let p = self.panels(user, win, dyntopo);
        if let Some((rows, l)) = &p.sub {
            if l.panel.isect_pt_v(cursor) {
                return match row_menu::item_at(l, cursor).map(|i| rows[i].clone()) {
                    Some(r @ Row::Operator { .. }) if enabled(&r) => Click::Run(r),
                    _ => Click::Kept,
                };
            }
        }
        if p.root.panel.isect_pt_v(cursor) {
            return match row_menu::item_at(&p.root, cursor) {
                Some(i) => match &p.root_rows[i] {
                    r @ Row::Operator { .. } if enabled(r) => Click::Run(r.clone()),
                    Row::Operator { .. } => Click::Kept,
                    Row::Menu { .. } => {
                        self.sub = Some(i);
                        Click::Kept
                    }
                    Row::Separator => Click::Kept,
                },
                None => Click::Kept,
            };
        }
        Click::Closed
    }

    /// The rect of the pull-down panel (for the header button switching logic).
    pub fn panel_rect(&self, user: &UserDef, win: [i32; 2], dyntopo: bool) -> Rcti {
        self.panels(user, win, dyntopo).root.panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: [i32; 2] = [1400, 900];

    fn user() -> UserDef {
        UserDef::from_window_scale(1.0)
    }

    fn center(r: Rcti) -> [i32; 2] {
        [(r.xmin + r.xmax) / 2, (r.ymin + r.ymax) / 2]
    }

    fn row_pos(m: &HeaderMenu, label: &str) -> [i32; 2] {
        let p = m.panels(&user(), WIN, false);
        let (r, _) = *p.root.items.iter().find(|&&(_, i)| matches!(&p.root_rows[i], Row::Menu { label: l, .. } | Row::Operator { label: l, .. } if *l == label)).unwrap();
        center(r)
    }

    #[test]
    fn hover_opens_and_closes_the_submenu() {
        let u = user();
        let mut m = HeaderMenu::open(Which::Sculpt, [100, 800]);
        m.mouse_move(&u, WIN, false, row_pos(&m, "Trim/Add"));
        assert!(m.sub.is_some());
        let p = m.panels(&u, WIN, false);
        let (rows, l) = p.sub.unwrap();
        assert_eq!(rows.len(), sculpt_submenus::TRIM.len());
        assert!(l.panel.xmin >= p.root.panel.xmax);
        m.mouse_move(&u, WIN, false, row_pos(&m, "Show All"));
        assert_eq!(m.sub, None);
    }

    #[test]
    fn moving_into_the_submenu_keeps_it_open() {
        let u = user();
        let mut m = HeaderMenu::open(Which::Sculpt, [100, 800]);
        m.mouse_move(&u, WIN, false, row_pos(&m, "Show/Hide"));
        let p = m.panels(&u, WIN, false);
        let (_, l) = p.sub.unwrap();
        m.mouse_move(&u, WIN, false, center(l.items[3].0));
        assert!(m.sub.is_some());
    }

    #[test]
    fn click_on_an_operator_runs_it() {
        let u = user();
        let mut m = HeaderMenu::open(Which::Mask, [100, 800]);
        let pos = row_pos(&m, "Invert Mask");
        match m.click(&u, WIN, false, pos) {
            Click::Run(Row::Operator { idname, props, .. }) => {
                assert_eq!(idname, "paint.mask_flood_fill");
                assert_eq!(props, &[("mode", "INVERT")]);
            }
            c => panic!("{c:?}"),
        }
    }

    #[test]
    fn click_on_a_submenu_row_keeps_it_open_an_enabled_operator_runs_and_a_disabled_one_keeps_it() {
        let u = user();
        let mut m = HeaderMenu::open(Which::FaceSets, [100, 800]);
        let pos = row_pos(&m, "Initialize Face Sets");
        assert_eq!(m.click(&u, WIN, false, pos), Click::Kept);
        assert!(m.sub.is_some());
        let p = m.panels(&u, WIN, false);
        let (_, l) = p.sub.unwrap();
        /* Wired: an enabled operator row runs. */
        assert!(matches!(m.click(&u, WIN, false, center(l.items[2].0)), Click::Run(_)));
        /* Not wired yet: a disabled operator row keeps the menus open. */
        let mut m = HeaderMenu::open(Which::FaceSets, [100, 800]);
        let pos = row_pos(&m, "Extract Face Set");
        assert_eq!(m.click(&u, WIN, false, pos), Click::Kept);
    }

    #[test]
    fn click_outside_closes_and_separator_keeps() {
        let u = user();
        let mut m = HeaderMenu::open(Which::Sculpt, [100, 800]);
        assert_eq!(m.click(&u, WIN, false, [1300, 100]), Click::Closed);
        let p = m.panels(&u, WIN, false);
        assert_eq!(
            m.click(&u, WIN, false, center(p.root.separators[0])),
            Click::Kept
        );
    }

    #[test]
    fn hovering_random_mask_opens_its_submenu_panel() {
        let u = user();
        let mut m = HeaderMenu::open(Which::Mask, [100, 800]);
        m.mouse_move(&u, WIN, false, row_pos(&m, "Random Mask"));
        assert!(m.panels(&u, WIN, false).sub.is_some());
        assert!(submenu_rows("VIEW3D_MT_nonexistent").is_none());
    }

    #[test]
    fn submenu_rows_and_wired_operators_are_enabled() {
        let rows = rows_of(Which::Sculpt, false);
        assert!(rows
            .iter()
            .filter(|r| matches!(r, Row::Menu { .. }))
            .all(enabled));
        assert!(enabled(&Row::Menu {
            idname: "VIEW3D_MT_random_mask",
            label: "Random Mask"
        }));
        let op = |label: &str| rows.iter().find(|r| matches!(r, Row::Operator { label: l, .. } if *l == label)).unwrap();
        assert!(enabled(op("Show All")) && enabled(op("Hide Masked")) && enabled(op("Invert Visible")));
        assert!(!enabled(op("Toggle Visibility")), "face sets are not available yet");
    }

    #[test]
    fn view_menu_opens_its_viewpoint_submenu_on_hover() {
        let u = user();
        let mut m = HeaderMenu::open(Which::View, [100, 800]);
        m.mouse_move(&u, WIN, false, row_pos(&m, "Viewpoint"));
        let (rows, l) = m.panels(&u, WIN, false).sub.unwrap();
        assert_eq!(rows.len(), crate::view_menu::VIEWPOINT.len());
        assert!(l.panel.xmin >= m.panel_rect(&u, WIN, false).xmax);
        m.mouse_move(&u, WIN, false, row_pos(&m, "Frame All"));
        assert_eq!(m.sub, None);
    }

    #[test]
    fn view_menu_rows_run_only_when_the_port_has_them_and_outside_click_closes() {
        let u = user();
        let mut m = HeaderMenu::open(Which::View, [100, 800]);
        assert!(enabled(&Row::Menu { idname: "VIEW3D_MT_view_navigation", label: "Navigation" }));
        assert!(!enabled(&Row::Menu { idname: "INFO_MT_area", label: "Area" }));
        assert_eq!(m.click(&u, WIN, false, [1390, 10]), Click::Closed);
        m.view.stroke_mode = true;
        assert!(m.panels(&u, WIN, false).root_rows.iter().any(|r| matches!(r, Row::Operator { label: "Center Last Stroke", .. })));
    }

    #[test]
    fn select_menu_opens_more_less_and_runs_all_none_invert() {
        let u = user();
        let mut m = HeaderMenu::open(Which::Select, [100, 800]);
        m.mouse_move(&u, WIN, false, row_pos(&m, "More/Less"));
        assert_eq!(m.panels(&u, WIN, false).sub.unwrap().0.len(), crate::select_menu::MORE_LESS.len());
        let p = row_pos(&m, "Invert");
        m.mouse_move(&u, WIN, false, p);
        match m.click(&u, WIN, false, p) {
            Click::Run(r) => assert!(crate::menu_dispatch::command_of_row(&r).is_some()),
            c => panic!("{c:?}"),
        }
        assert!(!enabled(&Row::Operator { idname: "object.select_pattern", label: "Select Pattern...", props: &[], checkbox: None }));
    }
}
