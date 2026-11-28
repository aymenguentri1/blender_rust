//! Body of `OBJECT_PT_visibility` (properties_object.py:413), Workbench path (no ray visibility groups): Selectable and
//! Surface Picking | Show In Viewports and Renders | Mask Holdout. `invert_checkbox=True` rows show the flag negated.
//! Deviation: `hide_viewport` is read from `Object::hidden`, `hide_surface_pick` and `is_holdout` are not modelled (off).

use crate::object_tab_relations::{Kind, Row};
use crate::scene::Object;

fn check(heading: &'static str, caption: &'static str, on: bool) -> Row {
    Row { label: heading, value: if on { "\u{2713}".into() } else { String::new() }, kind: Kind::Toggle, active: true, caption }
}

pub fn groups(o: &Object) -> Vec<Vec<Row>> {
    vec![
        vec![check("", "Selectable", !o.hide_select), check("", "Surface Picking", true)],
        vec![check("Show In", "Viewports", !o.hidden), check("", "Renders", !o.hide_render)],
        vec![check("Mask", "Holdout", false)],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    #[test]
    fn flags_are_shown_inverted() {
        let s = Scene::startup();
        let mut o = s.objects[0].clone();
        let on = |g: &Vec<Vec<Row>>, i: usize, j: usize| !g[i][j].value.is_empty();
        let g = groups(&o);
        assert!(on(&g, 0, 0) && on(&g, 0, 1) && on(&g, 1, 0) && on(&g, 1, 1) && !on(&g, 2, 0));
        o.hide_select = true;
        o.hide_render = true;
        let g = groups(&o);
        assert!(!on(&g, 0, 0) && !on(&g, 1, 1));
        assert_eq!((g[1][0].label, g[2][0].label, g[2][0].caption), ("Show In", "Mask", "Holdout"));
    }
}
