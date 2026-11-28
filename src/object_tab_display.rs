//! Body of `OBJECT_PT_display` (properties_object.py:219), rows by object type: Show Name, Axes, (Wireframe, All Edges,
//! Texture Space, Shadow for geometry; All Edges for meshes), In Front, Color (geometry), then Display As (inactive on
//! cameras) and the Bounds toggle with its type (geometry only). Empties count as geometry (`is_geometry`).
//! Defaults: `show_shadows` on, the rest off, `display_type` Textured, `display_bounds_type` Box.
//! Deviation: none of these properties is modelled yet, so the values are the defaults; the bounds type is its own
//! row (Blender puts it beside the toggle).

use crate::object_tab_relations::{Kind, Row};
use crate::scene::{Object, ObjectData};

fn check(heading: &'static str, caption: &'static str, on: bool) -> Row {
    Row { label: heading, value: if on { "\u{2713}".into() } else { String::new() }, kind: Kind::Toggle, active: true, caption }
}

pub fn groups(o: &Object) -> Vec<Vec<Row>> {
    let mesh = matches!(o.data, ObjectData::Mesh(_));
    let geometry = mesh || matches!(o.data, ObjectData::Empty(_));
    let mut show = vec![check("Show", "Name", false), check("", "Axes", false)];
    if geometry {
        show.push(check("", "Wireframe", false));
    }
    if mesh {
        show.push(check("", "All Edges", false));
    }
    if geometry {
        show.push(check("", "Texture Space", false));
        show.push(check("", "Shadow", true));
    }
    show.push(check("", "In Front", false));
    if geometry {
        show.push(Row { label: "Color", value: String::new(), kind: Kind::Id, active: true, caption: "" });
    }
    let active = !matches!(o.data, ObjectData::Camera);
    let mut g = vec![show, vec![Row { label: "Display As", value: "Textured".into(), kind: Kind::Enum, active, caption: "" }]];
    if geometry {
        g.push(vec![
            check("Bounds", "", false),
            Row { label: "", value: "Box".into(), kind: Kind::Enum, active: false, caption: "" },
        ]);
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    fn find<'a>(s: &'a Scene, n: &str) -> &'a Object {
        s.objects.iter().find(|o| o.name == n).unwrap()
    }

    #[test]
    fn mesh_shows_every_row() {
        let s = Scene::startup();
        let g = groups(find(&s, "Cube"));
        let caps: Vec<_> = g[0].iter().map(|r| if r.caption.is_empty() { r.label } else { r.caption }).collect();
        assert_eq!(caps, ["Name", "Axes", "Wireframe", "All Edges", "Texture Space", "Shadow", "In Front", "Color"]);
        assert_eq!(g.len(), 3);
        assert!(!g[0][5].value.is_empty(), "show_shadows is on by default");
    }

    #[test]
    fn camera_has_few_rows_and_greys_display_as() {
        let s = Scene::startup();
        let g = groups(find(&s, "Camera"));
        assert_eq!(g[0].iter().map(|r| r.caption).collect::<Vec<_>>(), ["Name", "Axes", "In Front"]);
        assert!(!g[1][0].active);
        assert_eq!(g.len(), 2, "no Bounds");
    }
}
