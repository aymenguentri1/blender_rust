//! The Transform panel of the properties editor.
//!
//! Blender's Object tab (`properties_object.py`, `OBJECT_PT_transform`) has Location, Rotation and Scale fields
//! for the active object. This one is reduced to what the software UI can draw: a title row, then for each of
//! Location (m), Rotation (degrees, XYZ Euler), Scale and, for a mesh, Dimensions (m) a caption row and a row of
//! three fields (X, Y, Z). Typing a dimension sets the scale to match (the evaluated bounds, modifiers included).
//! A click on a field edits it by typing: the first key replaces the value, Backspace deletes, Enter applies (a
//! click elsewhere applies too), Tab / Shift + Tab apply and move to the next / previous field, and Escape cancels. Every applied edit is one undo step. Not modelled: dragging
//! a field to change it, expressions, the lock toggles, the rotation modes and the
//! delta transforms.
//!
//! The layout and the hit-testing are plain functions of the window rect, so they are tested without a window.
//! The Modifiers panel is placed below the rows returned by `layout()`.

use crate::area::Rcti;
use crate::modifier_panel::{Style, Widget};
use crate::scene::{Scene, TransformProp};
use crate::userdef::UserDef;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub prop: TransformProp,
    pub axis: usize,
}

/// A field being edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub field: Field,
    pub text: String,
    /// Nothing was typed yet: the next key replaces the whole text.
    pub fresh: bool,
}

impl Edit {
    pub fn start(scene: &Scene, field: Field) -> Option<Edit> {
        let v = value(scene, field)?;
        Some(Edit { field, text: number(v), fresh: true })
    }

    /// Typed characters: digits, a sign, a decimal point and an exponent.
    pub fn push(&mut self, c: char) {
        if !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')) {
            return;
        }
        if self.fresh {
            self.text.clear();
            self.fresh = false;
        }
        if self.text.len() < 16 {
            self.text.push(c);
        }
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
        self.text.trim().parse::<f32>().ok().filter(|v| v.is_finite())
    }

    /// Applies the text to the active object. Returns whether the object changed.
    pub fn commit(&self, scene: &mut Scene) -> bool {
        self.parsed().is_some_and(|v| scene.set_transform_value(self.field.prop, self.field.axis, v))
    }
}

impl Field {
    /// The field `delta` places on in reading order (Location X, Y, Z, then Rotation, then Scale), wrapping round:
    /// where Tab (1) and Shift + Tab (-1) go.
    pub fn step(self, delta: i32) -> Field {
        self.step_n(delta, 3)
    }

    /// `step()` over the first `props` properties (4 when Dimensions is shown too).
    pub fn step_n(self, delta: i32, props: usize) -> Field {
        let all = [TransformProp::Location, TransformProp::Rotation, TransformProp::Scale, TransformProp::Dimensions];
        let props = props.clamp(1, 4) as i32;
        let index = all.iter().position(|&p| p == self.prop).unwrap_or(0) as i32 * 3 + self.axis as i32;
        let n = (index + delta).rem_euclid(props * 3);
        Field { prop: all[(n / 3) as usize], axis: (n % 3) as usize }
    }

    /// `step_n()` for the properties the active object of `scene` shows (Dimensions needs a mesh).
    pub fn step_for(self, scene: &Scene, delta: i32) -> Field {
        self.step_n(delta, if scene.active_object().is_some_and(|o| o.dimensions().is_some()) { 4 } else { 3 })
    }
}

/// The value of `field` for the active object (rotation in degrees).
pub fn value(scene: &Scene, field: Field) -> Option<f32> {
    let o = scene.active_object().filter(|o| !o.hidden)?;
    Some(match field.prop {
        TransformProp::Location => o.loc[field.axis],
        TransformProp::Rotation => o.rot[field.axis].to_degrees(),
        TransformProp::Scale => o.scale[field.axis],
        TransformProp::Dimensions => o.dimensions()?[field.axis],
    })
}

/// `1.5`, `-2`, `0.3333`: at most four decimals, no trailing zeros, no `-0`.
pub fn number(v: f32) -> String {
    let s = format!("{v:.4}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

fn unit(prop: TransformProp) -> &'static str {
    match prop {
        TransformProp::Location | TransformProp::Dimensions => " m",
        TransformProp::Rotation => "\u{b0}",
        TransformProp::Scale => "",
    }
}

pub struct Layout {
    pub widgets: Vec<Widget>,
    /// The editable fields and their rects.
    pub fields: Vec<(Rcti, Field)>,
    /// The highest y the next panel may use (the Modifiers panel is given the window below it).
    pub bottom: i32,
}

/// The panel in `window` (window coordinates, y up) for the active object of `scene`; `edit` is the field being
/// typed into, if any.
pub fn layout(user: &UserDef, window: Rcti, scene: &Scene, edit: Option<&Edit>) -> Layout {
    let unit_h = user.widget_unit;
    let pad = unit_h / 2;
    let gap = (unit_h / 5).max(2);
    let (x0, x1) = (window.xmin + pad, window.xmax - pad);
    let mut out = Layout { widgets: Vec::new(), fields: Vec::new(), bottom: window.ymax };
    let mut top = window.ymax - pad + 1;
    let Some(o) = scene.active_object().filter(|o| !o.hidden) else {
        if x1 > x0 && top - unit_h >= window.ymin {
            out.widgets.push(Widget {
                rect: Rcti { xmin: x0, xmax: x1, ymin: top - unit_h, ymax: top - 1 },
                label: "Transform: no object active".into(),
                style: Style::Title,
                action: None,
                centered: false,
            });
            top -= unit_h + gap;
        }
        out.bottom = top;
        return out;
    };
    let needed = 7 * (unit_h + gap) + gap;
    if x1 - x0 < 9 * unit_h || top - needed < window.ymin {
        out.bottom = top;
        return out;
    }
    /* Dimensions are one more caption and row of fields, left out when they do not fit. */
    let with_dims = o.dimensions().is_some() && top - (needed + 2 * (unit_h + gap)) >= window.ymin;
    let row = |widgets: &mut Vec<Widget>, top: &mut i32, rect_x: (i32, i32), label: String, style: Style| -> Rcti {
        let rect = Rcti { xmin: rect_x.0, xmax: rect_x.1, ymin: *top - unit_h, ymax: *top - 1 };
        widgets.push(Widget { rect, label, style, action: None, centered: false });
        rect
    };
    row(&mut out.widgets, &mut top, (x0, x1), format!("Transform  ({})", o.name), Style::Title);
    top -= unit_h + gap;
    let mut props = vec![(TransformProp::Location, "Location"), (TransformProp::Rotation, "Rotation"), (TransformProp::Scale, "Scale")];
    if with_dims {
        props.push((TransformProp::Dimensions, "Dimensions"));
    }
    for (prop, caption) in props {
        row(&mut out.widgets, &mut top, (x0, x1), caption.to_string(), Style::Title);
        top -= unit_h + gap;
        let col = (x1 - x0 + 1 - 2 * gap) / 3;
        for axis in 0..3 {
            let xa = x0 + axis as i32 * (col + gap);
            let xb = if axis == 2 { x1 } else { xa + col - 1 };
            let field = Field { prop, axis };
            let editing = edit.filter(|e| e.field == field);
            let label = match editing {
                Some(e) => format!("{}: {}|", ["X", "Y", "Z"][axis], e.text),
                None => format!("{}: {}{}", ["X", "Y", "Z"][axis], number(value(scene, field).unwrap_or(0.0)), unit(prop)),
            };
            let style = if editing.is_some() { Style::Selected } else { Style::Field };
            let rect = row(&mut out.widgets, &mut top, (xa, xb), label, style);
            out.widgets.last_mut().unwrap().action = None;
            out.fields.push((rect, field));
        }
        top -= unit_h + gap;
    }
    out.bottom = top;
    out
}

/// The field under `xy`.
pub fn field_at(layout: &Layout, xy: [i32; 2]) -> Option<Field> {
    layout.fields.iter().find(|(r, _)| r.isect_pt_v(xy)).map(|&(_, f)| f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> Rcti {
        Rcti { xmin: 1608, xmax: 1917, ymin: 23, ymax: 834 }
    }

    fn lay(s: &Scene, e: Option<&Edit>) -> Layout {
        layout(&UserDef::from_window_scale(1.0), window(), s, e)
    }

    #[test]
    fn numbers_are_trimmed() {
        assert_eq!(number(1.0), "1");
        assert_eq!(number(-2.5), "-2.5");
        assert_eq!(number(1.0 / 3.0), "0.3333");
        assert_eq!(number(-0.00001), "0");
        assert_eq!(number(100.0), "100");
    }

    #[test]
    fn the_panel_has_twelve_fields_that_do_not_overlap_and_leaves_room_below() {
        let s = Scene::startup();
        let l = lay(&s, None);
        assert_eq!(l.fields.len(), 12);
        assert_eq!(l.widgets.len(), 1 + 4 + 12);
        assert!(l.widgets.iter().all(|w| window().isect(&w.rect) == Some(w.rect)));
        for (i, (a, _)) in l.fields.iter().enumerate() {
            for (b, _) in &l.fields[i + 1..] {
                assert!(a.isect(b).is_none());
            }
        }
        assert!(l.bottom < l.fields.iter().map(|(r, _)| r.ymin).min().unwrap());
        let f = l.fields[4].0;
        assert_eq!(field_at(&l, [f.cent_x(), f.cent_y()]), Some(Field { prop: TransformProp::Rotation, axis: 1 }));
        assert_eq!(field_at(&l, [0, 0]), None);
    }

    #[test]
    fn a_camera_has_no_dimensions() {
        let mut s = Scene::startup();
        s.active = s.objects.iter().find(|o| o.name == "Camera").map(|o| o.id);
        let l = lay(&s, None);
        assert_eq!(l.fields.len(), 9);
        assert!(l.fields.iter().all(|(_, f)| f.prop != TransformProp::Dimensions));
        assert_eq!(Field { prop: TransformProp::Scale, axis: 2 }.step_for(&s, 1), Field { prop: TransformProp::Location, axis: 0 });
    }

    #[test]
    fn dimensions_show_the_size_and_typing_one_sets_the_scale() {
        let mut s = Scene::startup();
        let dx = Field { prop: TransformProp::Dimensions, axis: 0 };
        assert_eq!(value(&s, dx), Some(2.0));
        let labels: Vec<String> = lay(&s, None).widgets.iter().map(|w| w.label.clone()).collect();
        assert!(labels.contains(&"X: 2 m".to_string()), "{labels:?}");
        let mut e = Edit::start(&s, dx).unwrap();
        e.push('5');
        assert!(e.commit(&mut s));
        let o = s.active_object().unwrap();
        assert_eq!(o.scale, [2.5, 1.0, 1.0]);
        assert_eq!(value(&s, dx), Some(5.0));
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().scale, [1.0; 3]);
    }

    #[test]
    fn dimensions_keep_a_mirrored_sign_and_refuse_zero_and_nonmeshes() {
        let mut s = Scene::startup();
        let i = s.objects.iter().position(|o| o.name == "Cube").unwrap();
        s.objects[i].scale = [-1.0, 1.0, 1.0];
        assert!(s.set_transform_value(TransformProp::Dimensions, 0, 4.0));
        assert_eq!(s.objects[i].scale[0], -2.0);
        assert!(!s.set_transform_value(TransformProp::Dimensions, 0, 0.0));
        assert!(!s.set_transform_value(TransformProp::Dimensions, 0, -3.0));
        s.active = s.objects.iter().find(|o| o.name == "Light").map(|o| o.id);
        assert!(!s.set_transform_value(TransformProp::Dimensions, 0, 3.0));
    }

    #[test]
    fn dimensions_include_the_modifiers() {
        let mut s = Scene::startup();
        s.set_array(2);
        s.update_eval();
        assert_eq!(value(&s, Field { prop: TransformProp::Dimensions, axis: 0 }), Some(4.0));
    }

    #[test]
    fn without_an_active_object_there_is_only_the_title() {
        let mut s = Scene::startup();
        s.active = None;
        let l = lay(&s, None);
        assert_eq!((l.widgets.len(), l.fields.len()), (1, 0));
    }

    #[test]
    fn a_panel_that_does_not_fit_is_left_out() {
        let s = Scene::startup();
        let user = UserDef::from_window_scale(1.0);
        let l = layout(&user, Rcti { ymax: 120, ..window() }, &s, None);
        assert!(l.fields.is_empty() && l.widgets.is_empty());
    }

    #[test]
    fn typing_replaces_then_edits_and_commits_one_undo_step() {
        let mut s = Scene::startup();
        let f = Field { prop: TransformProp::Location, axis: 0 };
        let mut e = Edit::start(&s, f).unwrap();
        assert_eq!(e.text, "0");
        for c in "x-1.5".chars() {
            e.push(c);
        }
        assert_eq!(e.text, "-1.5", "the first key replaces the value, letters are ignored");
        e.backspace();
        e.push('2');
        assert_eq!(e.parsed(), Some(-1.2));
        assert!(e.commit(&mut s));
        assert_eq!(s.active_object().unwrap().loc[0], -1.2);
        assert!(!e.commit(&mut s), "unchanged");
        assert!(s.undo());
        assert_eq!(s.active_object().unwrap().loc[0], 0.0);
        let mut bad = Edit::start(&s, f).unwrap();
        bad.push('-');
        assert_eq!(bad.parsed(), None);
        assert!(!bad.commit(&mut s));
    }

    #[test]
    fn tab_steps_through_the_nine_fields_and_wraps() {
        let first = Field { prop: TransformProp::Location, axis: 0 };
        assert_eq!(first.step(1), Field { prop: TransformProp::Location, axis: 1 });
        assert_eq!(Field { prop: TransformProp::Location, axis: 2 }.step(1), Field { prop: TransformProp::Rotation, axis: 0 });
        assert_eq!(Field { prop: TransformProp::Scale, axis: 2 }.step(1), first, "wraps forwards");
        assert_eq!(first.step(-1), Field { prop: TransformProp::Scale, axis: 2 }, "and backwards");
        let mut f = first;
        for _ in 0..9 {
            f = f.step(1);
        }
        assert_eq!(f, first);
    }

    #[test]
    fn rotation_is_in_degrees_and_the_field_shows_the_edit() {
        let mut s = Scene::startup();
        assert!(s.set_transform_value(TransformProp::Rotation, 2, 90.0));
        assert!((s.active_object().unwrap().rot[2] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(s.set_transform_value(TransformProp::Scale, 1, 2.5));
        assert!(!s.set_transform_value(TransformProp::Scale, 3, 1.0));
        assert!(!s.set_transform_value(TransformProp::Scale, 0, f32::NAN));
        let l = lay(&s, None);
        let labels: Vec<&str> = l.widgets.iter().map(|w| w.label.as_str()).collect();
        assert!(labels.contains(&"Z: 90\u{b0}") && labels.contains(&"Y: 2.5"), "{labels:?}");
        let e = Edit { field: Field { prop: TransformProp::Scale, axis: 1 }, text: "7".into(), fresh: false };
        let l = lay(&s, Some(&e));
        let w = l.widgets.iter().find(|w| w.label == "Y: 7|").unwrap();
        assert_eq!(w.style, Style::Selected);
    }
}
