//! The Modifiers panel of the properties editor.
//!
//! Blender's panel (`properties_data_modifier.py`, `MOD_ui_common.cc`) lists the modifiers of the active
//! object, each with its name, move / remove buttons and its settings, and an `Add Modifier` menu. This one is
//! reduced to what the software UI can draw: per modifier a header row (select, move up, move down, remove)
//! and a value row (its summary with `-` / `+` buttons that act like Ctrl + Alt + `[` / `]`), and an
//! `Add Modifier` button that opens a list of every kind. Not modelled: collapsing, per-setting fields,
//! the viewport / render toggles, scrolling (rows that do not fit are left out) and the tabs of the navigation
//! bar (the panel is always shown).
//!
//! Layout and hit-testing are plain functions of the window rect, so they are tested without a window.

use crate::area::{properties_nav_bar_rect, top_header_rect, Rcti};
use crate::font;
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
    Decrease(usize),
    Increase(usize),
    /// The target picker of a Shrinkwrap: the next other mesh object, then none.
    CycleTarget(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Title,
    Button,
    Selected,
    Field,
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

/// What the panel shows: the active mesh object, its modifiers as (name, summary, target row) and the active one.
/// The target row is only there for modifiers that have a target (Shrinkwrap).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelState {
    pub object: Option<String>,
    pub modifiers: Vec<(&'static str, String, Option<String>)>,
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
                        let target = matches!(m, Modifier::Shrinkwrap { .. })
                            .then(|| format!("Target: {}", scene.target_name(i).unwrap_or_else(|| "(none)".into())));
                        (m.label(), m.summary(), target)
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
    for (i, (name, summary, target)) in st.modifiers.iter().enumerate() {
        let style = if i == st.active { Style::Selected } else { Style::Button };
        let (r3, r2, r1) = (x1 - small + 1, x1 - 2 * small - gap + 1, x1 - 3 * small - 2 * gap + 1);
        let head = vec![
            (x0, r1 - gap - 1, name.to_string(), style, Some(Action::Select(i)), false),
            (r1, r1 + small - 1, "^".into(), Style::Button, Some(Action::MoveUp(i)), true),
            (r2, r2 + small - 1, "v".into(), Style::Button, Some(Action::MoveDown(i)), true),
            (r3, x1, "x".into(), Style::Button, Some(Action::Remove(i)), true),
        ];
        let rows = if target.is_some() { 3 } else { 2 };
        if top - rows * (unit + gap) < window.ymin {
            break;
        }
        row(&mut out, &mut top, head);
        let (m2, m1) = (x1 - small + 1, x1 - 2 * small - gap + 1);
        let value = vec![
            (x0, m1 - gap - 1, summary.clone(), Style::Field, Some(Action::Select(i)), false),
            (m1, m1 + small - 1, "-".into(), Style::Button, Some(Action::Decrease(i)), true),
            (m2, x1, "+".into(), Style::Button, Some(Action::Increase(i)), true),
        ];
        row(&mut out, &mut top, value);
        if let Some(label) = target {
            row(&mut out, &mut top, vec![(x0, x1, label.clone(), Style::Button, Some(Action::CycleTarget(i)), false)]);
        }
        top -= gap;
    }
    out
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
        };
        draw_round_box(buf, width, height, user, w.rect, inner, theme::WCOL_TOOL_OUTLINE, round);
        if w.centered {
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
            modifiers: vec![("Mirror", "Mirror X".into(), None), ("Array", "Array x2".into(), None)],
            active: 1,
            add_open: false,
        };
        let ws = widgets(&user, window(), &st);
        /* title, add, then 2 x (4 header + 3 value) widgets */
        assert_eq!(ws.len(), 2 + 2 * 7);
        assert!(ws.iter().all(|w| window().isect(&w.rect) == Some(w.rect)));
        assert!(ws[1].rect.ymax < ws[0].rect.ymin);
        let selected: Vec<_> = ws.iter().filter(|w| w.style == Style::Selected).collect();
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
            modifiers: vec![("Shrinkwrap", "Shrinkwrap 0 m".into(), Some("Target: (none)".into()))],
            ..Default::default()
        };
        let ws = widgets(&user, window(), &st);
        assert_eq!(ws.len(), 2 + 4 + 3 + 1);
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
        assert_eq!(st.modifiers[2].2.as_deref(), Some("Target: (none)"));
        assert!(st.modifiers[0].2.is_none());
        assert!(!apply(&mut s, &mut open, Action::CycleTarget(0)), "an Array has no target");
    }
}
