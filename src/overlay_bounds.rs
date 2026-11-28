//! Overlay engine "Bounds" (draw/engines/overlay/overlay_bounds.hh): when an object draws its bounds and the
//! local transform (`scale`, `center`) of each shape instance for box, sphere, cylinder, cone and capsule.
//! Deviation: instances carry the 3 axis columns and location of the local matrix; the caller multiplies by
//! the object matrix and fills the draw buffers.

#![allow(dead_code)]

type V3 = [f32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    Box,
    Sphere,
    Cylinder,
    Cone,
    Capsule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjKind {
    Lamp,
    Camera,
    Speaker,
    /// `basis` = `BKE_mball_is_basis`.
    MetaBall { basis: bool },
    /// `with_geometry` = evaluated geometry set is not empty (Blender ORs it in; it does not change the result).
    Empty { with_geometry: bool },
    Other,
}

/// `has_bounds` of `object_sync()`.
pub fn has_bounds(kind: ObjKind) -> bool {
    match kind {
        ObjKind::Lamp | ObjKind::Camera | ObjKind::Speaker => false,
        ObjKind::MetaBall { basis } => basis,
        // Not lamp/camera/speaker/mball, so the first clause holds; `empty_with_geometry` is redundant here.
        ObjKind::Empty { .. } => true,
        ObjKind::Other => true,
    }
}

/// `draw_bounds`: the draw-type "Bounds" ignores `show_extras`; the "Bounds" display option needs it.
pub fn draw_bounds(has_bounds: bool, dt_is_boundbox: bool, dtx_drawboundox: bool, show_extras: bool) -> bool {
    has_bounds && (dt_is_boundbox || (dtx_drawboundox && show_extras))
}

/// `show_extras = !from_dupli && state.show_extras()`.
pub fn show_extras(from_dupli: bool, state_show_extras: bool) -> bool {
    !from_dupli && state_show_extras
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Box,
    Sphere,
    Cylinder,
    Cone,
    CapsuleCap,
    CapsuleBody,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub shape: Shape,
    /// Columns x, y, z of the local matrix.
    pub axes: [V3; 3],
    pub location: V3,
}

fn diag(sx: f32, sy: f32, sz: f32) -> [V3; 3] {
    [[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, sz]]
}

/// `size = (max - min) * 0.5`, `center` as `add_bounds()` computes them (zero center when `around_origin`).
/// Missing bounds fall back to (-1..1).
pub fn size_center(bounds: Option<(V3, V3)>, around_origin: bool) -> (V3, V3) {
    let (min, max) = bounds.unwrap_or(([-1.0; 3], [1.0; 3]));
    let size = std::array::from_fn(|i| (max[i] - min[i]) * 0.5);
    let center = if around_origin { [0.0; 3] } else { std::array::from_fn(|i| (min[i] + max[i]) * 0.5) };
    (size, center)
}

/// `add_bounds_ex()`: the instances drawn for one bound.
pub fn add_bounds_ex(center: V3, size: V3, bound_type: BoundType) -> Vec<Instance> {
    let xy = size[0].max(size[1]);
    match bound_type {
        BoundType::Box => vec![Instance { shape: Shape::Box, axes: diag(size[0], size[1], size[2]), location: center }],
        BoundType::Sphere => {
            let m = size[0].max(size[1]).max(size[2]);
            vec![Instance { shape: Shape::Sphere, axes: diag(m, m, m), location: center }]
        }
        BoundType::Cylinder => vec![Instance { shape: Shape::Cylinder, axes: diag(xy, xy, size[2]), location: center }],
        BoundType::Cone => {
            // The cone batch points to +Y with its base at 0: swap the Y and Z columns, lower by size.z.
            let a = diag(xy, xy, size[2]);
            vec![Instance {
                shape: Shape::Cone,
                axes: [a[0], a[2], a[1]],
                location: [center[0], center[1], center[2] - size[2]],
            }]
        }
        BoundType::Capsule => {
            let off = (size[2] - size[0]).max(0.0);
            let top = Instance { shape: Shape::CapsuleCap, axes: diag(xy, xy, xy), location: [center[0], center[1], center[2] + off] };
            let flipped = [top.axes[0], top.axes[1], [0.0, 0.0, -xy]];
            let bottom = Instance { shape: Shape::CapsuleCap, axes: flipped, location: [center[0], center[1], center[2] - off] };
            let body_z = (size[2] * 2.0 - size[0] * 2.0).max(0.0);
            let body = Instance { shape: Shape::CapsuleBody, axes: [flipped[0], flipped[1], [0.0, 0.0, body_z]], location: bottom.location };
            vec![top, bottom, body]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lamp_camera_speaker_have_no_bounds_and_mball_needs_basis() {
        assert!(!has_bounds(ObjKind::Lamp) && !has_bounds(ObjKind::Camera) && !has_bounds(ObjKind::Speaker));
        assert!(has_bounds(ObjKind::Other));
        assert!(has_bounds(ObjKind::Empty { with_geometry: false }));
        assert!(has_bounds(ObjKind::MetaBall { basis: true }));
        assert!(!has_bounds(ObjKind::MetaBall { basis: false }));
    }

    #[test]
    fn draw_bounds_ignores_extras_for_boundbox_draw_type() {
        assert!(draw_bounds(true, true, false, false));
        assert!(!draw_bounds(true, false, true, false));
        assert!(draw_bounds(true, false, true, true));
        assert!(!draw_bounds(false, true, true, true));
        assert!(!show_extras(true, true) && show_extras(false, true) && !show_extras(false, false));
    }

    #[test]
    fn size_center_and_fallback() {
        let (s, c) = size_center(Some(([0.0, 0.0, 0.0], [2.0, 4.0, 6.0])), false);
        assert_eq!((s, c), ([1.0, 2.0, 3.0], [1.0, 2.0, 3.0]));
        assert_eq!(size_center(Some(([0.0; 3], [2.0, 4.0, 6.0])), true).1, [0.0; 3]);
        assert_eq!(size_center(None, false), ([1.0; 3], [0.0; 3]));
    }

    #[test]
    fn box_sphere_cylinder_scales() {
        let i = add_bounds_ex([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], BoundType::Box)[0];
        assert_eq!((i.axes, i.location), (diag(1.0, 2.0, 3.0), [1.0, 2.0, 3.0]));
        assert_eq!(add_bounds_ex([0.0; 3], [1.0, 2.0, 3.0], BoundType::Sphere)[0].axes, diag(3.0, 3.0, 3.0));
        assert_eq!(add_bounds_ex([0.0; 3], [1.0, 2.0, 3.0], BoundType::Cylinder)[0].axes, diag(2.0, 2.0, 3.0));
    }

    #[test]
    fn cone_swaps_axes_and_drops_to_base() {
        let i = add_bounds_ex([0.0, 0.0, 1.0], [2.0, 1.0, 3.0], BoundType::Cone)[0];
        assert_eq!(i.axes, [[2.0, 0.0, 0.0], [0.0, 0.0, 3.0], [0.0, 2.0, 0.0]]);
        assert_eq!(i.location, [0.0, 0.0, -2.0]);
    }

    #[test]
    fn capsule_has_two_caps_and_a_body() {
        let v = add_bounds_ex([0.0; 3], [1.0, 1.0, 3.0], BoundType::Capsule);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].location, [0.0, 0.0, 2.0]);
        assert_eq!(v[1].location, [0.0, 0.0, -2.0]);
        assert_eq!(v[1].axes[2], [0.0, 0.0, -1.0]); // flipped cap
        assert_eq!(v[2].shape, Shape::CapsuleBody);
        assert_eq!(v[2].axes[2], [0.0, 0.0, 4.0]);
        // short capsule: no cylinder part
        let s = add_bounds_ex([0.0; 3], [2.0, 2.0, 1.0], BoundType::Capsule);
        assert_eq!(s[0].location, [0.0; 3]);
        assert_eq!(s[2].axes[2], [0.0; 3]);
    }
}
