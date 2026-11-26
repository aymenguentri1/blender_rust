//! The Modifiers panel of the properties editor.
//!
//! Blender's panel (`properties_data_modifier.py`, `MOD_ui_common.cc`) lists the modifiers of the active
//! object, each with its name, move / remove buttons and its settings, and an `Add Modifier` menu. This one is
//! reduced to what the software UI can draw: per modifier a header row (the collapse triangle, select, the Show in Viewport monitor toggle, move up, move down, remove)
//! and a value row (its summary with `-` / `+` buttons that act like Ctrl + Alt + `[` / `]`), and an
//! `Add Modifier` button that opens a list of every kind. Not modelled: per-setting fields,
//! the render toggle, scrolling (rows that do not fit are left out) and the tabs of the navigation
//! bar (the panel is always shown).
//!
//! Layout and hit-testing are plain functions of the window rect, so they are tested without a window.

use crate::area::{properties_nav_bar_rect, top_header_rect, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::modifiers::Modifier;
use crate::scene::Scene;
use crate::theme;
use crate::userdef::UserDef;
use crate::widget_draw::draw_round_box;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    ToggleAddList,
    Add(usize),
    Select(usize),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    /// Bakes the modifier into the mesh (Blender's Apply, Ctrl + A over the modifier).
    Apply(usize),
    /// Inserts a copy right after it (Blender's Duplicate, Shift + D over the modifier).
    Duplicate(usize),
    /// The disclosure triangle in the header.
    ToggleCollapse(usize),
    /// Ctrl + click on the triangle: collapses all the others (or opens all when it is the only open one).
    CollapseOthers(usize),
    /// The monitor toggle in the header (`show_viewport`).
    ToggleViewport(usize),
    Decrease(usize),
    Increase(usize),
    /// The target picker of a Shrinkwrap or Boolean: the next other mesh object, then none.
    CycleTarget(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Title,
    Button,
    Selected,
    Field,
    /// No box, only the icon (the disclosure triangle).
    Flat,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub rect: Rcti,
    pub label: String,
    pub style: Style,
    pub action: Option<Action>,
    /// Small square buttons centre their label.
    pub centered: bool,
}

/// One modifier of the panel. The target row is only there for modifiers that have a target (Shrinkwrap, Boolean).
#[derive(Clone, Debug, PartialEq)]
pub struct ModRow {
    pub name: &'static str,
    pub summary: String,
    pub target: Option<String>,
    /// Shown in the viewport (the highlighted monitor toggle).
    pub shown: bool,
    /// Only the header row is drawn.
    pub collapsed: bool,
}

impl ModRow {
    #[cfg(test)]
    fn new(name: &'static str, summary: &str, target: Option<&str>) -> ModRow {
        ModRow { name, summary: summary.into(), target: target.map(Into::into), shown: true, collapsed: false }
    }
}

/// What the panel shows: the active mesh object, its modifiers and the active one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelState {
    pub object: Option<String>,
    pub modifiers: Vec<ModRow>,
    pub active: usize,
    pub add_open: bool,
}

impl PanelState {
    pub fn of(scene: &Scene, add_open: bool) -> PanelState {
        match scene.active_object().filter(|o| o.mesh().is_some()) {
            None => PanelState { add_open, ..Default::default() },
            Some(o) => PanelState {
                object: Some(o.name.clone()),
                modifiers: o
                    .modifiers
                    .iter()
                    .enumerate()
                    .map(|(i, m)| {
                        let target = matches!(m, Modifier::Shrinkwrap { .. } | Modifier::Boolean { .. })
                            .then(|| format!("Target: {}", scene.target_name(i).unwrap_or_else(|| "(none)".into())));
                        ModRow { name: m.label(), summary: m.summary(), target, shown: o.modifier_shown(i), collapsed: o.modifier_collapsed(i) }
                    })
                    .collect(),
                active: o.active_modifier.min(o.modifiers.len().saturating_sub(1)),
                add_open,
            },
        }
    }
}

/// The window region of the properties editor: right of the navigation bar, below the header.
pub fn window_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    let header = top_header_rect(user, totrct)?;
    let xmin = properties_nav_bar_rect(user, totrct).map_or(totrct.xmin, |n| n.xmax + 1);
    let rect = Rcti { xmin, ymax: header.ymin - 1, ..totrct };
    if rect.is_empty() { None } else { Some(rect) }
}

/// The widgets of the panel in `window` (window coordinates, y up), top to bottom.
pub fn widgets(user: &UserDef, window: Rcti, st: &PanelState) -> Vec<Widget> {
    let unit = user.widget_unit;
    let pad = unit / 2;
    let gap = (unit / 5).max(2);
    let small = unit * 3 / 2;
    let (x0, x1) = (window.xmin + pad, window.xmax - pad);
    let mut out = Vec::new();
    if x1 - x0 < small * 5 {
        return out;
    }
    let mut top = window.ymax - pad + 1;
    /* Adds a row at the current top; false once it no longer fits. */
    let row = |out: &mut Vec<Widget>, top: &mut i32, parts: Vec<(i32, i32, String, Style, Option<Action>, bool)>| -> bool {
        if *top - unit < window.ymin {
            return false;
        }
        for (xa, xb, label, style, action, centered) in parts {
            out.push(Widget { rect: Rcti { xmin: xa, xmax: xb, ymin: *top - unit, ymax: *top - 1 }, label, style, action, centered });
        }
        *top -= unit + gap;
        true
    };
    let title = match &st.object {
        Some(n) => format!("Modifiers  ({n})"),
        None => "Modifiers: no mesh object active".to_string(),
    };
    row(&mut out, &mut top, vec![(x0, x1, title, Style::Title, None, false)]);
    if st.object.is_none() {
        return out;
    }
    let add_style = if st.add_open { Style::Selected } else { Style::Button };
    row(&mut out, &mut top, vec![(x0, x1, "Add Modifier".into(), add_style, Some(Action::ToggleAddList), false)]);
    if st.add_open {
        for (i, m) in Modifier::catalog().iter().enumerate() {
            if !row(&mut out, &mut top, vec![(x0 + pad, x1, m.label().to_string(), Style::Button, Some(Action::Add(i)), false)]) {
                return out;
            }
        }
    }
    top -= gap;
    for (i, ModRow { name, summary, target, shown, collapsed }) in st.modifiers.iter().enumerate() {
        let style = if i == st.active { Style::Selected } else { Style::Button };
        let (r3, r2, r1, r0) = (x1 - small + 1, x1 - 2 * small - gap + 1, x1 - 3 * small - 2 * gap + 1, x1 - 4 * small - 3 * gap + 1);
        let eye_style = if *shown { Style::Selected } else { Style::Button };
        let head = vec![
            (x0, x0 + small - 1, if *collapsed { ">" } else { "v" }.into(), Style::Flat, Some(Action::ToggleCollapse(i)), true),
            (x0 + small + gap, r0 - gap - 1, name.to_string(), style, Some(Action::Select(i)), false),
            (r0, r0 + small - 1, String::new(), eye_style, Some(Action::ToggleViewport(i)), true),
            (r1, r1 + small - 1, "^".into(), Style::Button, Some(Action::MoveUp(i)), true),
            (r2, r2 + small - 1, "v".into(), Style::Button, Some(Action::MoveDown(i)), true),
            (r3, x1, "x".into(), Style::Button, Some(Action::Remove(i)), true),
        ];
        let rows = if *collapsed { 1 } else if target.is_some() { 4 } else { 3 };
        if top - rows * (unit + gap) < window.ymin {
            break;
        }
        row(&mut out, &mut top, head);
        if *collapsed {
            top -= gap;
            continue;
        }
        let (m2, m1) = (x1 - small + 1, x1 - 2 * small - gap + 1);
        let value = vec![
            (x0, m1 - gap - 1, summary.clone(), Style::Field, Some(Action::Select(i)), false),
            (m1, m1 + small - 1, "-".into(), Style::Button, Some(Action::Decrease(i)), true),
            (m2, x1, "+".into(), Style::Button, Some(Action::Increase(i)), true),
        ];
        row(&mut out, &mut top, value);
        let half = (x1 - x0 - gap) / 2;
        row(
            &mut out,
            &mut top,
            vec![
                (x0, x0 + half - 1, "Apply".into(), Style::Button, Some(Action::Apply(i)), false),
                (x0 + half + gap, x1, "Duplicate".into(), Style::Button, Some(Action::Duplicate(i)), false),
            ],
        );
        if let Some(label) = target {
            row(&mut out, &mut top, vec![(x0, x1, label.clone(), Style::Button, Some(Action::CycleTarget(i)), false)]);
        }
        top -= gap;
    }
    out
}

/// The action with Ctrl held: the triangle collapses the other panels instead of itself.
pub fn with_ctrl(action: Action) -> Action {
    match action {
        Action::ToggleCollapse(i) => Action::CollapseOthers(i),
        a => a,
    }
}

/// The action of the widget under `xy`, if any.
pub fn action_at(widgets: &[Widget], xy: [i32; 2]) -> Option<Action> {
    widgets.iter().find(|w| w.rect.isect_pt_v(xy)).and_then(|w| w.action)
}

pub fn draw(buf: &mut [u32], width: usize, height: usize, user: &UserDef, widgets: &[Widget]) {
    for w in widgets {
        let (inner, round) = match w.style {
            Style::Title => {
                font::draw_label_pad(buf, width, height, user, w.rect, &w.label, theme::WCOL_PULLDOWN_TEXT, 0);
                continue;
            }
            Style::Button => (theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_ROUNDNESS),
            Style::Selected => (theme::WCOL_TOGGLE_INNER_SEL, theme::WCOL_TOOL_ROUNDNESS),
            Style::Field => (theme::WCOL_MENU_INNER, theme::WCOL_TOOL_ROUNDNESS),
            Style::Flat => {
                let icon = if w.label == ">" { Icon::RightArrow } else { Icon::DownArrow };
                icons::draw_icon(buf, width, height, user, w.rect, icon, theme::WCOL_PULLDOWN_TEXT);
                continue;
            }
        };
        draw_round_box(buf, width, height, user, w.rect, inner, theme::WCOL_TOOL_OUTLINE, round);
        if w.action.is_some_and(|a| matches!(a, Action::ToggleViewport(_))) {
            icons::draw_icon(buf, width, height, user, w.rect, Icon::Monitor, theme::WCOL_PULLDOWN_TEXT);
        } else if w.centered {
            let pad = ((w.rect.size_x() + 1 - font::string_width(user, &w.label)) / 2).max(0);
            font::draw_label_pad(buf, width, height, user, w.rect, &w.label, theme::WCOL_PULLDOWN_TEXT, pad);
        } else {
            font::draw_label(buf, width, height, user, w.rect, &w.label, theme::WCOL_PULLDOWN_TEXT);
        }
    }
}

/// Runs `action` on the active object. Returns whether the modifier results have to be rebuilt; `add_open` is the
/// state of the Add Modifier list, which the action may toggle or close.
pub fn apply(scene: &mut Scene, add_open: &mut bool, action: Action) -> bool {
    match action {
        Action::ToggleAddList => {
            *add_open = !*add_open;
            false
        }
        Action::Add(i) => {
            *add_open = false;
            Modifier::catalog().get(i).cloned().is_some_and(|m| scene.panel_add_modifier(m))
        }
        Action::Select(i) => {
            scene.panel_select_modifier(i);
            false
        }
        Action::MoveUp(i) => scene.panel_move_modifier(i, -1),
        Action::MoveDown(i) => scene.panel_move_modifier(i, 1),
        Action::Remove(i) => scene.panel_remove_modifier(i),
        Action::Apply(i) => scene.panel_apply_modifier(i),
        Action::Duplicate(i) => scene.panel_duplicate_modifier(i),
        Action::ToggleCollapse(i) => {
            scene.panel_toggle_collapse(i);
            false
        }
        Action::CollapseOthers(i) => {
            scene.panel_collapse_others(i);
            false
        }
        Action::ToggleViewport(i) => scene.panel_toggle_viewport(i),
        Action::Decrease(i) => scene.panel_adjust_modifier(i, -1),
        Action::Increase(i) => scene.panel_adjust_modifier(i, 1),
        Action::CycleTarget(i) => scene.panel_cycle_target(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> Rcti {
        Rcti { xmin: 1608, xmax: 1917, ymin: 23, ymax: 834 }
    }

    #[test]
    fn the_panel_is_right_of_the_nav_bar_and_below_the_header() {
        let user = UserDef::from_window_scale(1.0);
        let totrct = Rcti { xmin: 1579, xmax: 1917, ymin: 23, ymax: 860 };
        let w = window_rect(&user, totrct).unwrap();
        assert_eq!((w.xmin, w.xmax, w.ymin), (1608, 1917, 23));
        assert!(w.ymax < 860);
    }

    #[test]
    fn without_a_mesh_there_is_only_the_title() {
        let user = UserDef::from_window_scale(1.0);
        let w = widgets(&user, window(), &PanelState::default());
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].style, Style::Title);
    }

    #[test]
    fn rows_stack_downwards_and_stay_inside_the_window() {
        let user = UserDef::from_window_scale(1.0);
        let st = PanelState {
            object: Some("Cube".into()),
            modifiers: vec![ModRow::new("Mirror", "Mirror X", None), ModRow::new("Array", "Array x2", None)],
            active: 1,
            add_open: false,
        };
        let ws = widgets(&user, window(), &st);
        /* title, add, then 2 x (6 header + 3 value + 2 apply / duplicate) widgets */
        assert_eq!(ws.len(), 2 + 2 * 11);
        assert!(ws.iter().all(|w| window().isect(&w.rect) == Some(w.rect)));
        assert!(ws[1].rect.ymax < ws[0].rect.ymin);
        /* The active modifier's name is the one highlighted name; the monitor toggles are lit too (shown). */
        let selected: Vec<_> = ws.iter().filter(|w| w.style == Style::Selected && matches!(w.action, Some(Action::Select(_)))).collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].action, Some(Action::Select(1)));
        /* No two buttons overlap. */
        for (i, a) in ws.iter().enumerate() {
            for b in &ws[i + 1..] {
                assert!(a.rect.isect(&b.rect).is_none(), "{a:?} overlaps {b:?}");
            }
        }
        let remove = ws.iter().find(|w| w.action == Some(Action::Remove(0))).unwrap();
        assert_eq!(action_at(&ws, [remove.rect.cent_x(), remove.rect.cent_y()]), Some(Action::Remove(0)));
        assert_eq!(action_at(&ws, [0, 0]), None);
    }

    #[test]
    fn a_shrinkwrap_gets_a_target_row() {
        let user = UserDef::from_window_scale(1.0);
        let st = PanelState {
            object: Some("Cube".into()),
            modifiers: vec![ModRow::new("Shrinkwrap", "Shrinkwrap 0 m", Some("Target: (none)"))],
            ..Default::default()
        };
        let ws = widgets(&user, window(), &st);
        assert_eq!(ws.len(), 2 + 6 + 3 + 2 + 1);
        let t = ws.iter().find(|w| w.action == Some(Action::CycleTarget(0))).unwrap();
        assert_eq!(t.label, "Target: (none)");
        assert_eq!(t.rect.xmin, ws[0].rect.xmin, "full width");
    }

    #[test]
    fn the_add_list_has_one_row_per_kind() {
        let user = UserDef::from_window_scale(1.0);
        let st = PanelState { object: Some("Cube".into()), add_open: true, ..Default::default() };
        let ws = widgets(&user, window(), &st);
        let adds = ws.iter().filter(|w| matches!(w.action, Some(Action::Add(_)))).count();
        assert_eq!(adds, Modifier::catalog().len());
    }

    #[test]
    fn rows_that_do_not_fit_are_left_out() {
        let user = UserDef::from_window_scale(1.0);
        let small = Rcti { ymax: 200, ..window() };
        let st = PanelState { object: Some("Cube".into()), add_open: true, ..Default::default() };
        let ws = widgets(&user, small, &st);
        assert!(ws.iter().all(|w| w.rect.ymin >= small.ymin));
        assert!(ws.iter().filter(|w| matches!(w.action, Some(Action::Add(_)))).count() < Modifier::catalog().len());
    }

    #[test]
    fn actions_edit_the_active_object() {
        let mut s = Scene::startup();
        let mut open = true;
        assert!(apply(&mut s, &mut open, Action::Add(1)));
        assert!(!open, "adding closes the list");
        assert!(apply(&mut s, &mut open, Action::Add(15)));
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        assert!(apply(&mut s, &mut open, Action::Increase(0)));
        assert_eq!(s.active_object().unwrap().modifiers[0], Modifier::array(3));
        assert!(apply(&mut s, &mut open, Action::MoveDown(0)));
        assert_eq!(s.active_object().unwrap().modifiers[1], Modifier::array(3));
        assert!(!apply(&mut s, &mut open, Action::MoveDown(1)), "already last");
        assert!(!apply(&mut s, &mut open, Action::Select(0)), "selecting changes no geometry");
        assert_eq!(s.active_object().unwrap().active_modifier, 0);
        assert!(apply(&mut s, &mut open, Action::Remove(0)));
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        let st = PanelState::of(&s, false);
        assert_eq!(st.modifiers.len(), 2);
        /* Only a Shrinkwrap has a target row. */
        assert!(apply(&mut s, &mut open, Action::Add(Modifier::catalog().len() - 1)));
        let st = PanelState::of(&s, false);
        assert_eq!(st.modifiers[2].target.as_deref(), Some("Target: (none)"));
        assert!(st.modifiers[0].target.is_none());
        assert!(!apply(&mut s, &mut open, Action::CycleTarget(0)), "an Array has no target");
    }

    #[test]
    fn apply_bakes_one_modifier_into_the_mesh() {
        let mut s = Scene::startup();
        let mut open = false;
        let verts = |s: &Scene| s.active_object().unwrap().mesh().unwrap().verts.len();
        assert_eq!(verts(&s), 8);
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        assert!(apply(&mut s, &mut open, Action::Add(15)), "Weld stays on the stack");
        assert!(apply(&mut s, &mut open, Action::Apply(0)));
        assert_eq!(verts(&s), 16, "the Array is part of the base mesh now");
        let o = s.active_object().unwrap();
        assert_eq!(o.modifiers.len(), 1);
        assert_eq!(o.modifiers[0].label(), "Weld");
        assert!(!apply(&mut s, &mut open, Action::Apply(5)), "no such modifier");
        assert!(s.undo());
        assert_eq!(verts(&s), 8);
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        s.panel_select_modifier(1);
        assert!(s.apply_active_modifier(), "the chord applies the active one");
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1);
    }

    #[test]
    fn the_monitor_toggle_hides_a_modifier_in_the_viewport() {
        let mut s = Scene::startup();
        let mut open = false;
        let faces = |s: &Scene| s.active_object().unwrap().evaluated().unwrap().faces.len();
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        s.update_eval();
        assert_eq!(faces(&s), 12);
        assert!(apply(&mut s, &mut open, Action::ToggleViewport(0)));
        s.update_eval();
        assert_eq!(faces(&s), 6, "the Array is skipped");
        let st = PanelState::of(&s, false);
        assert!(!st.modifiers[0].shown);
        let ws = widgets(&UserDef::from_window_scale(1.0), window(), &st);
        let eye = ws.iter().find(|w| w.action == Some(Action::ToggleViewport(0))).unwrap();
        assert_eq!(eye.style, Style::Button, "off: not highlighted");
        assert!(!apply(&mut s, &mut open, Action::Apply(0)), "a hidden modifier is not applied");
        assert!(!apply(&mut s, &mut open, Action::ToggleViewport(7)), "no such modifier");
        assert!(s.undo());
        assert!(PanelState::of(&s, false).modifiers[0].shown, "undo brings it back");
        assert!(apply(&mut s, &mut open, Action::ToggleViewport(0)));
        assert!(apply(&mut s, &mut open, Action::Add(15)), "Weld");
        assert!(apply(&mut s, &mut open, Action::MoveDown(0)));
        let st = PanelState::of(&s, false);
        assert_eq!((st.modifiers[0].name, st.modifiers[0].shown), ("Weld", true));
        assert_eq!((st.modifiers[1].name, st.modifiers[1].shown), ("Array", false), "the flag moved with the modifier");
        assert!(apply(&mut s, &mut open, Action::Remove(0)));
        assert!(!PanelState::of(&s, false).modifiers[0].shown, "removing another one keeps the flag");
    }

    #[test]
    fn a_collapsed_modifier_shows_only_its_header() {
        let user = UserDef::from_window_scale(1.0);
        let mut s = Scene::startup();
        let mut open = false;
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array");
        assert!(apply(&mut s, &mut open, Action::Add(15)), "Weld");
        let full = widgets(&user, window(), &PanelState::of(&s, false));
        assert!(!apply(&mut s, &mut open, Action::ToggleCollapse(0)), "UI state: no geometry change");
        assert!(s.active_object().unwrap().modifier_collapsed(0));
        let st = PanelState::of(&s, false);
        assert!(st.modifiers[0].collapsed && !st.modifiers[1].collapsed);
        let ws = widgets(&user, window(), &st);
        assert_eq!(ws.len(), full.len() - 5, "the value row and Apply / Duplicate are gone");
        let weld = |w: &[Widget]| w.iter().find(|w| w.action == Some(Action::Select(1))).unwrap().rect.ymax;
        assert!(weld(&ws) > weld(&full), "the next modifier moves up");
        let arrow = ws.iter().find(|w| w.action == Some(Action::ToggleCollapse(0))).unwrap();
        assert_eq!((arrow.label.as_str(), arrow.style), (">", Style::Flat));
        assert!(!apply(&mut s, &mut open, Action::ToggleCollapse(9)), "no such modifier");
        /* The state moves with the modifier and survives removing another one; it is not an undo step. */
        assert!(apply(&mut s, &mut open, Action::MoveDown(0)));
        assert!(PanelState::of(&s, false).modifiers[1].collapsed);
        assert!(apply(&mut s, &mut open, Action::Remove(0)));
        assert!(PanelState::of(&s, false).modifiers[0].collapsed);
        assert!(s.undo(), "undoes the remove, not the collapse");
        assert!(PanelState::of(&s, false).modifiers[1].collapsed);
        assert!(!apply(&mut s, &mut open, Action::ToggleCollapse(1)) && !PanelState::of(&s, false).modifiers[1].collapsed);
    }

    #[test]
    fn ctrl_click_on_the_triangle_collapses_the_others() {
        let mut s = Scene::startup();
        let mut open = false;
        for a in [1, 15, 3] {
            apply(&mut s, &mut open, Action::Add(a));
        }
        let collapsed = |s: &Scene| PanelState::of(s, false).modifiers.iter().map(|m| m.collapsed).collect::<Vec<_>>();
        assert_eq!(with_ctrl(Action::ToggleCollapse(1)), Action::CollapseOthers(1));
        assert_eq!(with_ctrl(Action::Remove(1)), Action::Remove(1), "other buttons are unchanged");
        assert!(!apply(&mut s, &mut open, Action::CollapseOthers(1)));
        assert_eq!(collapsed(&s), [true, false, true]);
        apply(&mut s, &mut open, Action::CollapseOthers(1));
        assert_eq!(collapsed(&s), [false; 3], "again: everything opens");
        apply(&mut s, &mut open, Action::ToggleCollapse(0));
        apply(&mut s, &mut open, Action::CollapseOthers(2));
        assert_eq!(collapsed(&s), [true, true, false]);
        assert!(!apply(&mut s, &mut open, Action::CollapseOthers(9)), "no such modifier");
    }

    #[test]
    fn duplicate_inserts_a_copy_after_the_modifier() {
        let mut s = Scene::startup();
        let mut open = false;
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        assert!(apply(&mut s, &mut open, Action::Add(15)), "Weld");
        assert!(apply(&mut s, &mut open, Action::Increase(0)), "Array x3");
        assert!(apply(&mut s, &mut open, Action::Duplicate(0)));
        let o = s.active_object().unwrap();
        let labels: Vec<_> = o.modifiers.iter().map(|m| m.label()).collect();
        assert_eq!(labels, ["Array", "Array", "Weld"]);
        assert_eq!(o.modifiers[1], Modifier::array(3), "the copy keeps the settings");
        assert_eq!(o.active_modifier, 1, "the copy is the active one");
        assert!(!apply(&mut s, &mut open, Action::Duplicate(9)), "no such modifier");
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        assert!(s.duplicate_active_modifier(), "the chord copies the active one");
        assert_eq!(s.active_object().unwrap().modifiers.len(), 3);
    }
}
