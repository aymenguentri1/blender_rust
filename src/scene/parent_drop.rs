//! Outliner drag and drop parenting, ported from
//! source/blender/editors/space_outliner/outliner_dragdrop.cc (`parent_drop_allowed`, `parent_drop_set_parents`,
//! `parent_drop_invoke`) and source/blender/editors/object/object_relations.cc (`parent_set_with_depsgraph`, PAR_OBJECT).
//! Not ported: non-object parent types (bone, curve, vertex), linked/override checks, view layers.

use super::Scene;

impl Scene {
    /// `parent_drop_allowed`: the drop target must be another object, not already the parent of `child`, and not
    /// one of its descendants (`BKE_object_is_child_recursive`).
    pub fn parent_drop_allowed(&self, child: u32, parent: u32) -> bool {
        let (Some(ci), Some(pi)) = (self.index_of(child), self.index_of(parent)) else { return false };
        child != parent && self.objects[ci].parent != Some(parent) && !self.is_descendant(pi, child)
    }

    /// `OUTLINER_OT_parent_drop`: parents object `child` to `parent` (`PAR_OBJECT`) as one undo step. With
    /// `keep_transform` (no Alt held) the previous parenting is first applied to loc/rot/scale; the new parent
    /// inverse is always the inverse of the parent's world matrix. Returns false when the drop is refused.
    pub fn parent_drop(&mut self, child: u32, parent: u32, keep_transform: bool) -> bool {
        if !self.parent_drop_allowed(child, parent) {
            return false;
        }
        let (ci, pi) = (self.index_of(child).unwrap(), self.index_of(parent).unwrap());
        self.push_undo();
        self.refresh_parents();
        if keep_transform {
            self.bake_world_and_unparent(ci);
        }
        let inv = crate::view3d::invert_m4_m4(&self.full_world(pi, 0));
        let o = &mut self.objects[ci];
        o.parent = Some(parent);
        o.parent_inv = inv;
        o.eval = None;
        self.refresh_parents();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::super::{mat4_mul_point, Scene};
    use crate::mesh::Primitive;

    fn pair() -> (Scene, u32, u32) {
        let mut s = Scene::startup();
        let p = s.objects[0].id;
        let c = s.add_primitive(Primitive::Cube, [4.0, 0.0, 0.0]);
        let pi = s.index_of(p).unwrap();
        s.objects[pi].loc = [0.0, 1.0, 0.0];
        s.objects[pi].rot = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        s.objects[pi].scale = [2.0; 3];
        s.refresh_parents();
        (s, p, c)
    }

    fn world_origin(s: &Scene, id: u32) -> [f32; 3] {
        mat4_mul_point(&s.objects[s.index_of(id).unwrap()].world_matrix(), [0.0; 3])
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    #[test]
    fn drop_keeps_the_world_transform_and_follows_the_parent() {
        let (mut s, p, c) = pair();
        let before = world_origin(&s, c);
        assert!(s.parent_drop(c, p, true));
        assert_eq!(s.objects[s.index_of(c).unwrap()].parent, Some(p));
        assert!(close(world_origin(&s, c), before));
        let pi = s.index_of(p).unwrap();
        s.objects[pi].loc[0] += 2.0;
        s.refresh_parents();
        assert!(close(world_origin(&s, c), [before[0] + 2.0, before[1], before[2]]));
    }

    #[test]
    fn drop_refuses_self_same_parent_and_cycles() {
        let (mut s, p, c) = pair();
        assert!(!s.parent_drop(p, p, true));
        assert!(s.parent_drop(c, p, true));
        assert!(!s.parent_drop(c, p, true), "already the parent");
        assert!(!s.parent_drop(p, c, true), "would make a loop");
        assert!(!s.parent_drop(c, 9999, true));
    }

    #[test]
    fn drop_is_one_undo_step() {
        let (mut s, p, c) = pair();
        let before = world_origin(&s, c);
        assert!(s.parent_drop(c, p, true));
        assert!(s.undo());
        let ci = s.index_of(c).unwrap();
        assert_eq!(s.objects[ci].parent, None);
        assert!(close(world_origin(&s, c), before));
    }

    #[test]
    fn drop_reparents_from_another_parent_in_place() {
        let (mut s, p, c) = pair();
        let q = s.add_primitive(Primitive::Cube, [-3.0, 2.0, 1.0]);
        assert!(s.parent_drop(c, q, true));
        let before = world_origin(&s, c);
        assert!(s.parent_drop(c, p, true));
        assert!(close(world_origin(&s, c), before));
    }
}
