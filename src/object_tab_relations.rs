//! Body of `OBJECT_PT_relations` (properties_object.py:133): Parent, Parent Type, Camera Parent Lock | Tracking Axis,
//! Up Axis | Pass Index, three separated groups with `use_property_split`. Values: `Object.parent`, `parent_type`
//! (always Object: bone/vertex parents are not modelled), `use_camera_lock_parent` (off), `trackflag` +Y and `upflag` Z
//! (`DNA_object_types.h:567`, labels `rna_enum_object_axis_items`, rna_object.cc:269), `pass_index` 0.
//! Deviation: row pitch and group gap are estimates, not measured; the toggle is drawn as an empty box.

use crate::area::Rcti;
use crate::modifier_panel::{Style, Widget};
use crate::scene::{Object, Scene};
use crate::userdef::UserDef;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Id,
    Enum,
    Toggle,
    Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
    pub kind: Kind,
    /// `sub.active = (parent is not None)` greys the Camera Parent Lock row.
    pub active: bool,
    /// Text beside a checkbox (`text=` of a toggle whose heading sits in the label column).
    pub caption: &'static str,
}

/// Rows per group (`flow.column()` x 3).
pub fn groups(scene: &Scene, o: &Object) -> [Vec<Row>; 3] {
    let parent = o.parent.and_then(|id| scene.index_of(id)).map(|i| scene.objects[i].name.clone());
    let row = |label, value: &str, kind, active| Row { label, value: value.into(), kind, active, caption: "" };
    [
        vec![
            row("Parent", parent.as_deref().unwrap_or(""), Kind::Id, true),
            row("Parent Type", "Object", Kind::Enum, true),
            row("Camera Parent Lock", "", Kind::Toggle, parent.is_some()),
        ],
        vec![row("Tracking Axis", "+Y", Kind::Enum, true), row("Up Axis", "Z", Kind::Enum, true)],
        vec![row("Pass Index", "0", Kind::Number, true)],
    ]
}

/// Heights at scale 1: top padding, row pitch, gap of `col.separator()`.
const PAD: f32 = 7.5;
const PITCH: f32 = 22.0;
const SEP: f32 = 8.0;

fn sc(user: &UserDef, v: f32) -> i32 {
    (v * user.scale_factor as f32).round() as i32
}

pub fn body_height(user: &UserDef, g: &[Vec<Row>]) -> i32 {
    let rows: usize = g.iter().map(Vec::len).sum();
    sc(user, PAD * 2.0 + rows as f32 * PITCH + (g.len() - 1) as f32 * SEP - 2.0)
}

/// Widgets of the rows inside a body whose top edge is `body_top`, between `ix0` and `ix1`.
pub fn widgets(user: &UserDef, g: &[Vec<Row>], ix0: i32, ix1: i32, body_top: i32) -> Vec<Widget> {
    let cell = user.widget_unit;
    let fx0 = ix0 + ((ix1 - ix0) as f32 * 0.4).round() as i32;
    let mut out = Vec::new();
    let mut y = body_top - sc(user, PAD);
    for (gi, rows) in g.iter().enumerate() {
        if gi > 0 {
            y -= sc(user, SEP);
        }
        for r in rows {
            let (ymax, ymin) = (y, y - cell + 1);
            let w = |xmin, xmax, label: &str, style| Widget { rect: Rcti { xmin, xmax, ymin, ymax }, label: label.into(), style, action: None, centered: false };
            out.push(w(ix0, fx0 - 1, r.label, Style::PropLabel));
            let style = match r.kind {
                Kind::Enum => Style::DropBox,
                Kind::Number => Style::PropBox { corners: 0 },
                Kind::Id | Kind::Toggle => Style::Field,
            };
            let xmax = if r.kind == Kind::Toggle { fx0 + cell - 1 } else { ix1 };
            out.push(w(fx0, xmax, &r.value, style));
            if !r.caption.is_empty() {
                out.push(w(xmax + 1 + cell / 4, ix1, r.caption, Style::Title));
            }
            y -= sc(user, PITCH);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_cube_rows_have_blender_defaults() {
        let s = Scene::startup();
        let cube = s.objects.iter().find(|o| o.name == "Cube").unwrap();
        let g = groups(&s, cube);
        assert_eq!(g.iter().map(Vec::len).collect::<Vec<_>>(), [3, 2, 1]);
        assert_eq!(g[0][0].value, "");
        assert_eq!(g[0][1].value, "Object");
        assert!(!g[0][2].active, "Camera Parent Lock is greyed without a parent");
        assert_eq!((g[1][0].value.as_str(), g[1][1].value.as_str(), g[2][0].value.as_str()), ("+Y", "Z", "0"));
    }

    #[test]
    fn a_parent_shows_its_name_and_enables_the_lock() {
        let mut s = Scene::startup();
        let ids: Vec<u32> = s.objects.iter().map(|o| o.id).collect();
        let (child, parent) = (ids[0], ids[1]);
        let i = s.index_of(child).unwrap();
        s.objects[i].parent = Some(parent);
        let name = s.objects[s.index_of(parent).unwrap()].name.clone();
        let g = groups(&s, &s.objects[i]);
        assert_eq!(g[0][0].value, name);
        assert!(g[0][2].active);
    }

    #[test]
    fn widgets_stack_downwards_inside_the_body() {
        let user = UserDef::from_window_scale(1.0);
        let s = Scene::startup();
        let g = groups(&s, &s.objects[0]);
        let w = widgets(&user, &g, 40, 300, 700);
        assert_eq!(w.len(), 12);
        let labels: Vec<_> = w.iter().filter(|w| w.style == Style::PropLabel).collect();
        assert!(labels.windows(2).all(|p| p[1].rect.ymax < p[0].rect.ymin));
        assert!(labels.last().unwrap().rect.ymin > 700 - body_height(&user, &g));
    }
}
