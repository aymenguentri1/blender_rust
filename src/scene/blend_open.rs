//! Replace the scene objects with the mesh objects read from a `.blend`.

use super::{ObjectData, Scene};
use crate::blend_scene::BlendSceneObject;

impl Scene {
    /// One undo step: the previous objects come back with undo.
    pub fn open_blend_objects(&mut self, objects: Vec<BlendSceneObject>) {
        self.push_undo();
        self.objects.clear();
        self.local_view = None;
        for o in objects {
            let ob = self.new_object(&o.name, ObjectData::Mesh(o.mesh));
            ob.loc = o.loc;
            ob.rot = o.rot;
            ob.scale = o.scale;
        }
        self.active = self.objects.first().map(|o| o.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;

    #[test]
    fn replaces_objects_and_can_be_undone() {
        let mut s = Scene::startup();
        let item = |name: &str, x: f32| BlendSceneObject {
            name: name.to_string(),
            mesh: Mesh::cube(1.0),
            loc: [x, 0.0, 0.0],
            rot: [0.0; 3],
            scale: [2.0; 3],
        };
        s.open_blend_objects(vec![item("A", 1.0), item("B", 3.0)]);
        assert_eq!(s.objects.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!((s.objects[1].loc, s.objects[1].scale), ([3.0, 0.0, 0.0], [2.0; 3]));
        assert_eq!(s.active, Some(s.objects[0].id));
        assert!(s.undo());
        assert!(s.objects.iter().any(|o| o.name == "Cube"));
    }
}
