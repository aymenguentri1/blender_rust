//! The Modifiers panel of the properties editor.
//!
//! Blender's panel (`properties_data_modifier.py`, `MOD_ui_common.cc`) lists the modifiers of the active
//! object, each with its name, move / remove buttons and its settings, and an `Add Modifier` menu. This one is
//! reduced to what the software UI can draw: per modifier a header row (the collapse triangle, the type icon, select, the Show in Edit Mode, Show in Viewport and Show in Render toggles, the dropdown menu, move up, move down, remove)
//! and a value row (its summary with `-` / `+` buttons that act like Ctrl + Alt + `[` / `]`), and an
//! `Add Modifier` button that opens a list of every kind. Not modelled: per-setting fields,
//! scrolling (rows that do not fit are left out) and the tabs of the navigation
//! bar (the panel is always shown).
//!
//! Layout and hit-testing are plain functions of the window rect, so they are tested without a window.

use crate::area::{properties_nav_bar_rect, top_header_rect, Rcti};
use crate::font;
use crate::icons::{self, Icon};
use crate::modifiers::Modifier;
use crate::scene::Scene;
use crate::theme::{self, Rgba};
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
    /// A click on the value field: starts typing a number (`ValueEdit`).
    EditValue(usize),
    /// The dropdown arrow in the header: opens the modifier menu (`modifier_menu`).
    OpenMenu(usize),
    /// The monitor toggle in the header (`show_viewport`).
    ToggleViewport(usize),
    /// The edit-mode toggle in the header (`show_in_editmode`).
    ToggleEditMode(usize),
    /// One of the X / Y / Z buttons of a Mirror modifier (`use_axis`): the modifier and the axis 0..2.
    ToggleAxis(usize, usize),
    /// A press on the drag handle at the right of the header: the window starts a drag (`mod_drag`); releasing over
    /// another modifier's row moves it there (`Scene::panel_move_modifier_index`). Does nothing by itself.
    DragStart(usize),
    /// The camera toggle in the header (`show_render`).
    ToggleRender(usize),
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
    /// The number field of a value row (`Count: 2`, text centred between the arrows): the box covers the whole row,
    /// the arrow widgets at its ends are drawn on top of it.
    NumField,
    /// The `<` / `>` ends of a number field: only the triangle, the box belongs to the `NumField`.
    NumArrowL,
    NumArrowR,
    /// The drag handle of a modifier header: six dots, no box.
    Handle,
    /// The type icon of a modifier header: no box, the label is the kind's name.
    ModIcon,
    /// A value field that is being typed into (blue outline).
    Editing,
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
    pub name: String,
    pub summary: String,
    pub target: Option<String>,
    /// Shown in the viewport (the highlighted monitor toggle).
    pub shown: bool,
    /// Enabled in edit mode (the highlighted edit-mode toggle).
    pub editmode: bool,
    /// Enabled for rendering (the highlighted camera toggle).
    pub render: bool,
    /// The modifier kind's name (`Modifier::label`), which picks the type icon of the header.
    pub kind: &'static str,
    /// Only the header row is drawn.
    pub collapsed: bool,
    /// The X / Y / Z axis buttons of a Mirror modifier (`use_axis`); `None` for the other kinds.
    pub axes: Option<[bool; 3]>,
    /// The editable value as `Label: 0.1 m`; `None` for modifiers without one (the summary is shown instead).
    pub field: Option<String>,
}

impl ModRow {
    #[cfg(test)]
    fn new(name: &'static str, summary: &str, target: Option<&str>) -> ModRow {
        ModRow { name: name.into(), kind: "Array", summary: summary.into(), target: target.map(Into::into), shown: true, editmode: true, render: true, collapsed: false, axes: None, field: None }
    }
}

/// What the panel shows: the active mesh object, its modifiers and the active one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelState {
    pub object: Option<String>,
    pub modifiers: Vec<ModRow>,
    pub active: usize,
    pub add_open: bool,
    /// The value field being typed into: the modifier and its text.
    pub editing: Option<(usize, String)>,
}

impl PanelState {
    pub fn with_edit(mut self, edit: Option<&ValueEdit>) -> PanelState {
        self.editing = edit.map(|e| (e.index, e.text.clone()));
        self
    }

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
                        ModRow {
                            name: o.modifier_name(i).unwrap_or_default(), kind: m.label(), summary: m.summary(), target, shown: o.modifier_shown(i), editmode: o.modifier_editmode(i), render: o.modifier_render(i), collapsed: o.modifier_collapsed(i),
                            axes: if let Modifier::Mirror { axes, .. } = m { Some(*axes) } else { None },
                            field: m.main_value().map(|(l, v, u)| format!("{l}: {}{u}", number(v))),
                        }
                    })
                    .collect(),
                active: o.active_modifier.min(o.modifiers.len().saturating_sub(1)),
                add_open,
                editing: None,
            },
        }
    }
}

/// A number as typed into a field: at most 4 decimals, no trailing zeros.
pub fn number(v: f32) -> String {
    let s = format!("{:.4}", v);
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" || t == "-0" { "0".into() } else { t.into() }
}

/// The text of a value field being typed into. Like Blender's text buttons the whole text is selected when the
/// field opens, so the first character typed replaces it (`fresh`); Backspace edits it, Enter applies.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueEdit {
    pub index: usize,
    pub text: String,
    fresh: bool,
}

impl ValueEdit {
    /// Opens the field of modifier `i` of the active object with its current value.
    pub fn start(scene: &Scene, i: usize) -> Option<ValueEdit> {
        let (_, v, _) = scene.active_object()?.modifiers.get(i)?.main_value()?;
        Some(ValueEdit { index: i, text: number(v), fresh: true })
    }

    /// Types the number characters of `s` (digits, `.` or `,`, a leading `-` or `+`).
    pub fn push(&mut self, s: &str) {
        for c in s.chars() {
            let c = if c == ',' { '.' } else { c };
            let ok = c.is_ascii_digit() || (c == '.' && !self.keeps_point()) || ((c == '-' || c == '+') && (self.fresh || self.text.is_empty()));
            if !ok || self.text.len() >= 12 {
                continue;
            }
            if self.fresh {
                self.text.clear();
                self.fresh = false;
            }
            self.text.push(c);
        }
    }

    fn keeps_point(&self) -> bool {
        !self.fresh && self.text.contains('.')
    }

    pub fn backspace(&mut self) {
        if self.fresh {
            self.text.clear();
            self.fresh = false;
        } else {
            self.text.pop();
        }
    }

    /// The typed number, if the text is one.
    pub fn parsed(&self) -> Option<f32> {
        self.text.parse::<f32>().ok().filter(|v| v.is_finite())
    }

    /// Applies the text to the modifier; nothing happens when it is not a number. Returns whether the modifier changed.
    pub fn commit(&self, scene: &mut Scene) -> bool {
        self.parsed().is_some_and(|v| scene.panel_set_modifier_value(self.index, v))
    }
}

/// The window region of the properties editor: right of the navigation bar, below the header.
pub fn window_rect(user: &UserDef, totrct: Rcti) -> Option<Rcti> {
    let header = top_header_rect(user, totrct)?;
    let xmin = properties_nav_bar_rect(user, totrct).map_or(totrct.xmin, |n| n.xmax + 1);
    let rect = Rcti { xmin, ymax: header.ymin - 1, ..totrct };
    if rect.is_empty() { None } else { Some(rect) }
}

impl Action {
    /// Runs an `Add(index)` (an entry of the Add Modifier menu) on the active object; true when the modifier results
    /// have to be rebuilt. Other actions do nothing here.
    pub fn run_add(self, scene: &mut Scene) -> bool {
        match self {
            Action::Add(i) => Modifier::catalog().get(i).cloned().is_some_and(|m| scene.panel_add_modifier(m)),
            _ => false,
        }
    }
}

/// The light blue of Blender's modifier icons.
const MODIFIER_ICON: Rgba = Rgba::hex(0x8ab4ebff);

/// The header icon of a modifier kind (`ICON_MOD_*`); kinds without a drawn glyph get the generic wrench.
pub fn type_icon(kind: &str) -> Icon {
    match kind {
        "Mirror" => Icon::ModMirror,
        "Array" => Icon::ModArray,
        "Solidify" => Icon::ModSolidify,
        "Bevel" => Icon::ModBevel,
        "Smooth" | "Laplacian Smooth" | "Corrective Smooth" | "Wave" => Icon::ModSmooth,
        "Simple Deform" | "Cast" => Icon::ModDeform,
        "Triangulate" => Icon::ModTriangulate,
        "Boolean" => Icon::ModBoolean,
        "Wireframe" => Icon::ModWireframe,
        "Decimate" => Icon::ModDecimate,
        "Remesh" => Icon::ModRemesh,
        _ => Icon::ModWrench,
    }
}

/// The widgets of the panel in `window` (window coordinates, y up), top to bottom.
pub fn widgets(user: &UserDef, window: Rcti, st: &PanelState) -> Vec<Widget> {
    let unit = user.widget_unit;
    let pad = unit / 2;
    let gap = (unit / 5).max(2);
    let small = unit * 3 / 2;
    let (x0, x1) = (window.xmin + pad, window.xmax - pad);
    let mut out = Vec::new();
    if x1 - x0 < unit * 12 {
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
    for (i, ModRow { name, kind, summary, target, shown, editmode, render, collapsed, axes, field }) in st.modifiers.iter().enumerate() {
        let style = if i == st.active { Style::Selected } else { Style::Button };
        /* Right to left: remove, drag handle, dropdown, camera, monitor, edit mode; the name takes the rest. Blender's
        header buttons are one unit wide, which leaves the name field room in the narrow properties editor. */
        let hb = unit;
        let slot = |k: i32| x1 - (k + 1) * hb - k * gap + 1;
        let eye_style = if *shown { Style::Selected } else { Style::Button };
        let edit_style = if *editmode { Style::Selected } else { Style::Button };
        let cam_style = if *render { Style::Selected } else { Style::Button };
        let head = vec![
            (x0, x0 + hb - 1, if *collapsed { ">" } else { "v" }.into(), Style::Flat, Some(Action::ToggleCollapse(i)), true),
            (x0 + hb + gap, x0 + 2 * hb + gap - 1, kind.to_string(), Style::ModIcon, Some(Action::Select(i)), true),
            (x0 + 2 * hb + 2 * gap, slot(5) - gap - 1, name.clone(), style, Some(Action::Select(i)), false),
            (slot(5), slot(5) + hb - 1, String::new(), edit_style, Some(Action::ToggleEditMode(i)), true),
            (slot(4), slot(4) + hb - 1, String::new(), eye_style, Some(Action::ToggleViewport(i)), true),
            (slot(3), slot(3) + hb - 1, String::new(), cam_style, Some(Action::ToggleRender(i)), true),
            (slot(2), slot(2) + hb - 1, String::new(), Style::Button, Some(Action::OpenMenu(i)), true),
            (slot(1), slot(1) + hb - 1, String::new(), Style::Handle, Some(Action::DragStart(i)), true),
            (slot(0), x1, "x".into(), Style::Button, Some(Action::Remove(i)), true),
        ];
        row(&mut out, &mut top, head);
        if *collapsed {
            top -= gap;
            continue;
        }
        let (m2, m1) = (x1 - small + 1, x1 - 2 * small - gap + 1);
        let (text, field_style, field_action) = match (&st.editing, field) {
            (Some((e, t)), _) if *e == i => (format!("{t}|"), Style::Editing, Action::EditValue(i)),
            (_, Some(f)) => (f.clone(), Style::Field, Action::EditValue(i)),
            _ => (summary.clone(), Style::Field, Action::Select(i)),
        };
        let value = if let Some(axes) = axes {
            /* Mirror: one toggle per axis across the row, lit while the axis is on. */
            let w = (x1 - x0 + 1 - 2 * gap) / 3;
            (0..3)
                .map(|k| {
                    let xa = x0 + k as i32 * (w + gap);
                    let xb = if k == 2 { x1 } else { xa + w - 1 };
                    let style = if axes[k] { Style::Selected } else { Style::Button };
                    (xa, xb, ["X", "Y", "Z"][k].to_string(), style, Some(Action::ToggleAxis(i, k)), true)
                })
                .collect()
        } else if field.is_some() && field_style == Style::Field {
            /* Blender's number field: one box with `<` and `>` at its ends and the text centred between them. While
             * typing, the arrows go and the whole row is the text field. */
            let ab = unit;
            vec![
                (x0 + ab, x1 - ab, text, Style::NumField, Some(field_action), true),
                (x0, x0 + ab - 1, "<".into(), Style::NumArrowL, Some(Action::Decrease(i)), true),
                (x1 - ab + 1, x1, ">".into(), Style::NumArrowR, Some(Action::Increase(i)), true),
            ]
        } else if field.is_some() {
            vec![(x0, x1, text, field_style, Some(field_action), false)]
        } else {
            vec![
                (x0, m1 - gap - 1, text, field_style, Some(field_action), false),
                (m1, m1 + small - 1, "-".into(), Style::Button, Some(Action::Decrease(i)), true),
                (m2, x1, "+".into(), Style::Button, Some(Action::Increase(i)), true),
            ]
        };
        row(&mut out, &mut top, value);
        if let Some(label) = target {
            row(&mut out, &mut top, vec![(x0, x1, label.clone(), Style::Button, Some(Action::CycleTarget(i)), false)]);
        }
        top -= gap;
    }
    out
}

/// The top edge of each visible modifier's header, top to bottom (`(index, ymax)`), for `drop_index`.
pub fn header_tops(widgets: &[Widget]) -> Vec<(usize, i32)> {
    widgets.iter().filter_map(|w| if let Some(Action::ToggleCollapse(i)) = w.action { Some((i, w.rect.ymax)) } else { None }).collect()
}

/// The modifier a drag released at height `y` lands on: the last one whose header starts at or above `y`, the first
/// one when `y` is above them all. `None` when there are no rows.
pub fn drop_index(tops: &[(usize, i32)], y: i32) -> Option<usize> {
    tops.iter().rev().find(|(_, top)| *top >= y).or(tops.first()).map(|(i, _)| *i)
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
            Style::Editing => {
                draw_round_box(buf, width, height, user, w.rect, theme::WCOL_MENU_INNER, Rgba::hex(0x4772b3ff), theme::WCOL_TOOL_ROUNDNESS);
                font::draw_label(buf, width, height, user, w.rect, &w.label, theme::WCOL_PULLDOWN_TEXT);
                continue;
            }
            Style::NumField => {
                /* The box spans the whole row, including the arrow zones that are drawn after it. */
                let full = Rcti { xmin: w.rect.xmin - user.widget_unit, xmax: w.rect.xmax + user.widget_unit, ..w.rect };
                draw_round_box(buf, width, height, user, full, theme::WCOL_TOOL_INNER, theme::WCOL_TOOL_OUTLINE, theme::WCOL_TOOL_ROUNDNESS);
                let pad = ((w.rect.size_x() + 1 - font::string_width(user, &w.label)) / 2).max(0);
                font::draw_label_pad(buf, width, height, user, w.rect, &w.label, theme::WCOL_PULLDOWN_TEXT, pad);
                continue;
            }
            Style::NumArrowL | Style::NumArrowR => {
                let icon = if w.style == Style::NumArrowL { Icon::LeftArrow } else { Icon::RightArrow };
                icons::draw_icon(buf, width, height, user, w.rect, icon, theme::WCOL_PULLDOWN_TEXT);
                continue;
            }
            Style::Handle => {
                icons::draw_icon(buf, width, height, user, w.rect, Icon::Grip, theme::WCOL_PULLDOWN_TEXT);
                continue;
            }
            Style::ModIcon => {
                icons::draw_icon(buf, width, height, user, w.rect, type_icon(&w.label), MODIFIER_ICON);
                continue;
            }
            Style::Flat => {
                let icon = if w.label == ">" { Icon::RightArrow } else { Icon::DownArrow };
                icons::draw_icon(buf, width, height, user, w.rect, icon, theme::WCOL_PULLDOWN_TEXT);
                continue;
            }
        };
        draw_round_box(buf, width, height, user, w.rect, inner, theme::WCOL_TOOL_OUTLINE, round);
        if w.action.is_some_and(|a| matches!(a, Action::OpenMenu(_))) {
            icons::draw_icon(buf, width, height, user, w.rect, Icon::DownArrow, theme::WCOL_PULLDOWN_TEXT);
        } else if w.action.is_some_and(|a| matches!(a, Action::ToggleViewport(_))) {
            icons::draw_icon(buf, width, height, user, w.rect, Icon::Monitor, theme::WCOL_PULLDOWN_TEXT);
        } else if w.action.is_some_and(|a| matches!(a, Action::ToggleEditMode(_))) {
            icons::draw_icon(buf, width, height, user, w.rect, Icon::EditMode, theme::WCOL_PULLDOWN_TEXT);
        } else if w.action.is_some_and(|a| matches!(a, Action::ToggleRender(_))) {
            icons::draw_icon(buf, width, height, user, w.rect, Icon::Camera, theme::WCOL_PULLDOWN_TEXT);
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
        /* The window runs the drag, it has to know where the button was released. */
        Action::DragStart(_) => false,
        Action::MoveUp(i) => scene.panel_move_modifier(i, -1),
        Action::MoveDown(i) => scene.panel_move_modifier(i, 1),
        Action::Remove(i) => scene.panel_remove_modifier(i),
        Action::Apply(i) => scene.panel_apply_modifier(i),
        Action::Duplicate(i) => scene.panel_duplicate_modifier(i),
        /* The menu is opened by the window, it has to know where the click was. */
        Action::OpenMenu(_) | Action::EditValue(_) => false,
        Action::ToggleCollapse(i) => {
            scene.panel_toggle_collapse(i);
            false
        }
        Action::CollapseOthers(i) => {
            scene.panel_collapse_others(i);
            false
        }
        Action::ToggleViewport(i) => scene.panel_toggle_viewport(i),
        Action::ToggleAxis(i, axis) => scene.panel_toggle_mirror_axis(i, axis),
        Action::ToggleEditMode(i) => scene.panel_toggle_editmode(i),
        Action::ToggleRender(i) => scene.panel_toggle_render(i),
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
            editing: None,
        };
        let ws = widgets(&user, window(), &st);
        /* title, add, then 2 x (9 header + 3 value) widgets */
        assert_eq!(ws.len(), 2 + 2 * 12);
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
        assert_eq!(ws.len(), 2 + 9 + 3 + 1);
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
    fn the_edit_mode_toggle_is_panel_state_left_of_the_monitor() {
        let mut s = Scene::startup();
        let mut open = false;
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        s.update_eval();
        let faces = |s: &Scene| s.active_object().unwrap().evaluated().unwrap().faces.len();
        assert!(PanelState::of(&s, false).modifiers[0].editmode, "on by default");
        assert!(apply(&mut s, &mut open, Action::ToggleEditMode(0)));
        s.update_eval();
        assert_eq!(faces(&s), 12, "object mode still evaluates it");
        let st = PanelState::of(&s, false);
        assert!(!st.modifiers[0].editmode && st.modifiers[0].shown && st.modifiers[0].render);
        let ws = widgets(&UserDef::from_window_scale(1.0), window(), &st);
        let find = |a| ws.iter().find(|w| w.action == Some(a)).unwrap();
        let (edit, eye) = (find(Action::ToggleEditMode(0)), find(Action::ToggleViewport(0)));
        assert_eq!(edit.style, Style::Button, "off: not highlighted");
        assert!(edit.rect.xmax < eye.rect.xmin, "edit mode, monitor, camera: Blender's order");
        let name = ws.iter().find(|w| w.action == Some(Action::Select(0)) && w.style != Style::ModIcon).unwrap();
        assert!(name.rect.xmax < edit.rect.xmin && name.rect.size_x() > 20, "the name field keeps some width");
        assert!(!apply(&mut s, &mut open, Action::ToggleEditMode(7)), "no such modifier");
        assert!(apply(&mut s, &mut open, Action::Add(15)), "Weld");
        assert!(apply(&mut s, &mut open, Action::MoveDown(0)));
        assert!(!PanelState::of(&s, false).modifiers[1].editmode, "the flag moved with the modifier");
        assert!(s.undo() && s.undo() && s.undo());
        assert!(PanelState::of(&s, false).modifiers[0].editmode, "undo restores it");
    }

    #[test]
    fn dragging_the_handle_moves_a_modifier_to_the_row_it_is_dropped_on() {
        let mut s = Scene::startup();
        let mut open = false;
        for k in [1, 15, 2] {
            assert!(apply(&mut s, &mut open, Action::Add(k)));
        }
        let names = |s: &Scene| PanelState::of(s, false).modifiers.iter().map(|m| m.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&s), ["Array", "Weld", "Solidify"]);
        let st = PanelState::of(&s, false);
        let ws = widgets(&UserDef::from_window_scale(1.0), window(), &st);
        let handle = ws.iter().find(|w| w.action == Some(Action::DragStart(0))).unwrap();
        let x = handle.rect.xmax + 1;
        let remove = ws.iter().find(|w| w.action == Some(Action::Remove(0))).unwrap();
        assert!(handle.rect.xmax < remove.rect.xmin && handle.style == Style::Handle, "handle sits left of the X");
        assert!(!apply(&mut s, &mut open, Action::DragStart(0)), "pressing the handle changes nothing");
        assert!(x > 0);
        let tops = header_tops(&ws);
        assert_eq!(tops.iter().map(|t| t.0).collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(drop_index(&tops, tops[0].1 + 50), Some(0), "above everything: the first");
        assert_eq!(drop_index(&tops, tops[1].1 - 2), Some(1));
        assert_eq!(drop_index(&tops, tops[2].1 - 200), Some(2), "below everything: the last");
        assert_eq!(drop_index(&[], 5), None);
        assert!(s.panel_move_modifier_index(0, 2));
        assert_eq!(names(&s), ["Weld", "Solidify", "Array"]);
        assert_eq!(PanelState::of(&s, false).active, 2, "the moved one is active");
        assert!(!s.panel_move_modifier_index(2, 2) && !s.panel_move_modifier_index(9, 0), "no move, no step");
        assert!(s.undo());
        assert_eq!(names(&s), ["Array", "Weld", "Solidify"], "one undo step");
    }

    #[test]
    fn a_value_row_is_one_number_field_with_arrows_at_its_ends() {
        let mut s = Scene::startup();
        let mut open = false;
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        assert!(apply(&mut s, &mut open, Action::Add(11)), "Triangulate has no number");
        let user = UserDef::from_window_scale(1.0);
        let ws = widgets(&user, window(), &PanelState::of(&s, false));
        let find = |a| ws.iter().find(|w| w.action == Some(a)).unwrap();
        let (field, dec, inc) = (find(Action::EditValue(0)), find(Action::Decrease(0)), find(Action::Increase(0)));
        assert_eq!((field.style, dec.style, inc.style), (Style::NumField, Style::NumArrowL, Style::NumArrowR));
        assert!(field.centered && field.label == "Count: 2");
        assert_eq!((dec.rect.xmax + 1, inc.rect.xmin), (field.rect.xmin, field.rect.xmax + 1), "the three pieces tile the row");
        assert_eq!((dec.rect.ymin, dec.rect.ymax), (field.rect.ymin, field.rect.ymax));
        assert!(inc.rect.size_x() <= user.widget_unit && dec.rect.size_x() <= user.widget_unit, "arrow zones are one unit wide");
        assert_eq!(action_at(&ws, [dec.rect.cent_x(), dec.rect.cent_y()]), Some(Action::Decrease(0)));
        assert_eq!(action_at(&ws, [field.rect.cent_x(), field.rect.cent_y()]), Some(Action::EditValue(0)));
        /* A modifier without a number keeps the summary field and its - / + buttons. */
        let mirror = ws.iter().find(|w| w.action == Some(Action::Decrease(1))).unwrap();
        assert_eq!((mirror.label.as_str(), mirror.style), ("-", Style::Button));
        /* While typing the whole row is the text field. */
        let st = PanelState::of(&s, false).with_edit(Some(&ValueEdit { index: 0, text: "3".into(), fresh: true }));
        let ws = widgets(&user, window(), &st);
        let f = ws.iter().find(|w| w.action == Some(Action::EditValue(0))).unwrap();
        assert_eq!(f.style, Style::Editing);
        assert!(ws.iter().all(|w| w.action != Some(Action::Decrease(0))), "no arrows while typing");
        assert!(f.rect.size_x() > field.rect.size_x());
    }

    #[test]
    fn a_mirror_row_has_one_toggle_per_axis() {
        let mut s = Scene::startup();
        let mut open = false;
        assert!(apply(&mut s, &mut open, Action::Add(0)), "Mirror X");
        s.update_eval();
        let faces = |s: &Scene| s.active_object().unwrap().evaluated().unwrap().faces.len();
        assert_eq!(faces(&s), 12);
        let user = UserDef::from_window_scale(1.0);
        let ws = widgets(&user, window(), &PanelState::of(&s, false));
        let btn = |k| ws.iter().find(|w| w.action == Some(Action::ToggleAxis(0, k))).unwrap();
        assert_eq!([btn(0).label.as_str(), btn(1).label.as_str(), btn(2).label.as_str()], ["X", "Y", "Z"]);
        assert_eq!((btn(0).style, btn(1).style, btn(2).style), (Style::Selected, Style::Button, Style::Button));
        assert!(btn(0).rect.xmax < btn(1).rect.xmin && btn(1).rect.xmax < btn(2).rect.xmin);
        assert_eq!(btn(2).rect.xmax, window().xmax - user.widget_unit / 2, "the row ends at the panel margin");
        assert!(apply(&mut s, &mut open, Action::ToggleAxis(0, 1)), "Y on");
        s.update_eval();
        assert_eq!(faces(&s), 24, "X and Y");
        assert!(apply(&mut s, &mut open, Action::ToggleAxis(0, 1)) && apply(&mut s, &mut open, Action::ToggleAxis(0, 0)), "all off");
        s.update_eval();
        assert_eq!(s.active_object().unwrap().modifiers.len(), 1, "the modifier stays, unlike the chord");
        assert_eq!(faces(&s), 6, "and changes nothing");
        assert!(!apply(&mut s, &mut open, Action::ToggleAxis(0, 3)) && !apply(&mut s, &mut open, Action::ToggleAxis(4, 0)));
        assert!(s.undo());
        assert_eq!(PanelState::of(&s, false).modifiers[0].axes, Some([true, false, false]));
    }

    #[test]
    fn the_camera_toggle_marks_a_modifier_for_render_only() {
        let mut s = Scene::startup();
        let mut open = false;
        let faces = |s: &Scene| s.active_object().unwrap().evaluated().unwrap().faces.len();
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        s.update_eval();
        assert_eq!(faces(&s), 12);
        assert!(apply(&mut s, &mut open, Action::ToggleRender(0)));
        s.update_eval();
        assert_eq!(faces(&s), 12, "the viewport still evaluates it");
        let st = PanelState::of(&s, false);
        assert!(st.modifiers[0].shown && !st.modifiers[0].render);
        let ws = widgets(&UserDef::from_window_scale(1.0), window(), &st);
        let cam = ws.iter().find(|w| w.action == Some(Action::ToggleRender(0))).unwrap();
        let eye = ws.iter().find(|w| w.action == Some(Action::ToggleViewport(0))).unwrap();
        assert_eq!(cam.style, Style::Button, "off: not highlighted");
        assert_eq!(eye.style, Style::Selected);
        assert!(eye.rect.xmax < cam.rect.xmin, "the monitor sits left of the camera, as in Blender");
        let name = ws.iter().find(|w| w.action == Some(Action::Select(0)) && w.style != Style::ModIcon).unwrap();
        assert!(name.rect.xmax < eye.rect.xmin, "the name field does not overlap the toggles");
        assert!(crate::overlay_text::ObjectInfo::of(&s).modifiers[0].ends_with("(no render)"));
        assert!(!apply(&mut s, &mut open, Action::ToggleRender(7)), "no such modifier");
        assert!(apply(&mut s, &mut open, Action::Add(15)), "Weld");
        assert!(apply(&mut s, &mut open, Action::MoveDown(0)));
        let st = PanelState::of(&s, false);
        assert!(st.modifiers[0].render && !st.modifiers[1].render, "the flag moved with the modifier");
        assert!(apply(&mut s, &mut open, Action::Duplicate(1)));
        let st = PanelState::of(&s, false);
        assert!(!st.modifiers[2].render, "Duplicate copies the render flag");
        assert!(s.undo());
        assert!(s.undo());
        assert!(s.undo());
        assert!(!PanelState::of(&s, false).modifiers[0].render, "undo steps back through the edits");
        assert!(s.undo());
        assert!(PanelState::of(&s, false).modifiers[0].render, "and finally the toggle itself");
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
        assert_eq!((st.modifiers[0].name.as_str(), st.modifiers[0].shown), ("Weld", true));
        assert_eq!((st.modifiers[1].name.as_str(), st.modifiers[1].shown), ("Array", false), "the flag moved with the modifier");
        assert!(apply(&mut s, &mut open, Action::Remove(0)));
        assert!(!PanelState::of(&s, false).modifiers[0].shown, "removing another one keeps the flag");
    }

    #[test]
    fn typing_a_number_into_the_value_field() {
        let user = UserDef::from_window_scale(1.0);
        let mut s = Scene::startup();
        let mut open = false;
        assert!(apply(&mut s, &mut open, Action::Add(1)), "Array x2");
        assert!(apply(&mut s, &mut open, Action::Add(0)), "Mirror");
        let st = PanelState::of(&s, false);
        assert_eq!(st.modifiers[0].field.as_deref(), Some("Count: 2"));
        assert_eq!(st.modifiers[1].field, None, "a Mirror has no number");
        let ws = widgets(&user, window(), &st);
        assert!(ws.iter().any(|w| w.action == Some(Action::EditValue(0)) && w.label == "Count: 2"));
        assert!(ws.iter().any(|w| w.action == Some(Action::ToggleAxis(1, 0)) && w.label == "X"), "Mirror: axis buttons");
        assert!(ValueEdit::start(&s, 1).is_none() && ValueEdit::start(&s, 9).is_none());
        /* The first character replaces the selected text. */
        let mut e = ValueEdit::start(&s, 0).unwrap();
        assert_eq!(e.text, "2");
        e.push("5");
        assert_eq!(e.text, "5");
        e.push("x.y.");
        assert_eq!(e.text, "5.", "letters and a second point are ignored");
        e.backspace();
        e.backspace();
        e.push("-");
        e.push("7");
        assert_eq!(e.parsed(), Some(-7.0));
        assert!(e.commit(&mut s), "clamped to one copy");
        assert_eq!(s.active_object().unwrap().modifiers[0], Modifier::array(1));
        assert_eq!(s.active_object().unwrap().active_modifier, 0);
        let mut bad = ValueEdit::start(&s, 0).unwrap();
        bad.backspace();
        assert_eq!(bad.parsed(), None);
        assert!(!bad.commit(&mut s), "an empty field changes nothing");
        let mut c = ValueEdit::start(&s, 0).unwrap();
        c.push("3");
        assert!(c.commit(&mut s) && s.undo(), "one undo step");
        assert_eq!(s.active_object().unwrap().modifiers[0], Modifier::array(1));
        /* While typing, the field shows the text with a caret and the editing style. */
        let st = PanelState::of(&s, false).with_edit(Some(&c));
        let ws = widgets(&user, window(), &st);
        let f = ws.iter().find(|w| w.action == Some(Action::EditValue(0))).unwrap();
        assert_eq!((f.label.as_str(), f.style), ("3|", Style::Editing));
        assert_eq!((number(0.1), number(2.0), number(-0.00001), number(1.23456)), ("0.1".into(), "2".into(), "0".into(), "1.2346".into()));
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
        assert_eq!(ws.len(), full.len() - 3, "the value row is gone");
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
    fn a_modifier_can_be_renamed() {
        let mut s = Scene::startup();
        let mut open = false;
        for a in [1, 1, 15] {
            apply(&mut s, &mut open, Action::Add(a));
        }
        let names = |s: &Scene| PanelState::of(s, false).modifiers.into_iter().map(|m| m.name).collect::<Vec<_>>();
        assert_eq!(names(&s), ["Array", "Array", "Weld"]);
        assert!(s.panel_rename_modifier(0, "  Row  "));
        assert_eq!(names(&s), ["Row", "Array", "Weld"], "trimmed");
        assert!(!s.panel_rename_modifier(0, "Row"), "unchanged");
        assert!(!s.panel_rename_modifier(0, "   "), "empty names are refused");
        assert!(!s.panel_rename_modifier(9, "X"), "no such modifier");
        assert!(s.panel_rename_modifier(2, "Row"));
        assert_eq!(names(&s)[2], "Row.001", "unique among the modifiers");
        assert!(s.panel_rename_modifier(2, "Row.001") == false);
        assert!(s.panel_rename_modifier(1, "Weld"), "another modifier's kind name is free");
        assert!(s.panel_rename_modifier(1, "Array"), "back to the kind's name");
        assert!(s.active_object().unwrap().mod_flags[1].name.is_none(), "which clears the custom name");
        /* The name moves with the modifier, is undoable and does not change the geometry. */
        assert!(apply(&mut s, &mut open, Action::MoveDown(0)));
        assert_eq!(names(&s), ["Array", "Row", "Row.001"]);
        assert!(s.undo());
        assert_eq!(names(&s), ["Row", "Array", "Row.001"]);
        assert!(s.undo());
        assert_eq!(names(&s), ["Row", "Weld", "Row.001"], "the rename back to Array was a step of its own");
        assert_eq!(s.active_object().unwrap().modifiers.len(), 3);
        assert_eq!(crate::rename_popup::Rename::open_modifier([0, 0], "Row", 2).modifier, Some(2));
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
        assert_eq!(PanelState::of(&s, false).modifiers[1].name, "Array.001", "the copy gets a unique name");
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().modifiers.len(), 2);
        assert!(s.duplicate_active_modifier(), "the chord copies the active one");
        assert_eq!(s.active_object().unwrap().modifiers.len(), 3);
    }
}
