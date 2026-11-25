//! The modifier stack of an object (Blender's `Object::modifiers`), evaluated in order on the base mesh
//! before the Subdivision Surface level, which is why a mirrored or arrayed object subdivides as one piece.
//!
//! Defaults follow `DNA_modifier_defaults.h`: Mirror merges at 0.001 m, Array has a count of 2 and a relative
//! offset of (1, 0, 0), Solidify a thickness of 0.01 m and an offset of -1, Bevel a width of 0.1 m.

use crate::mesh::{Mesh, V3};
use crate::mesh_deform::{CastShape, DeformMode};

#[derive(Clone, Debug, PartialEq)]
pub enum Modifier {
    Mirror { axes: [bool; 3], merge: bool, threshold: f32 },
    Array { count: u32, relative: V3, constant: V3 },
    Solidify { thickness: f32, offset: f32 },
    /// One segment, `Offset` width.
    Bevel { width: f32 },
    /// Neighbour averaging: `factor` per round, `repeat` rounds.
    Smooth { factor: f32, repeat: u32 },
    /// Twist (radians) / Taper / Stretch (factor) about the Z axis.
    SimpleDeform { mode: DeformMode, amount: f32 },
    /// Noise displacement along the normals.
    Displace { strength: f32, midlevel: f32, scale: f32 },
    Triangulate,
    /// Blend towards a sphere or cylinder; radius 0 is the average distance.
    Cast { shape: CastShape, factor: f32, radius: f32 },
    /// Frames of the given thickness instead of faces.
    Wireframe { thickness: f32 },
    /// Merge by distance.
    Weld { distance: f32 },
}

impl Modifier {
    pub fn mirror(axis: usize) -> Modifier {
        let mut axes = [false; 3];
        axes[axis.min(2)] = true;
        Modifier::Mirror { axes, merge: true, threshold: 0.001 }
    }

    pub fn array(count: u32) -> Modifier {
        Modifier::Array { count: count.max(1), relative: [1.0, 0.0, 0.0], constant: [0.0; 3] }
    }

    pub fn solidify() -> Modifier {
        Modifier::Solidify { thickness: 0.01, offset: -1.0 }
    }

    pub fn bevel() -> Modifier {
        Modifier::Bevel { width: 0.1 }
    }

    pub fn smooth() -> Modifier {
        Modifier::Smooth { factor: 0.5, repeat: 1 }
    }

    /// Defaults of `DNA_modifier_defaults.h`: a 45 degree twist, a taper / stretch factor of 0.5.
    pub fn simple_deform(mode: DeformMode) -> Modifier {
        let amount = match mode {
            DeformMode::Twist => std::f32::consts::FRAC_PI_4,
            DeformMode::Bend => std::f32::consts::FRAC_PI_4,
            DeformMode::Taper | DeformMode::Stretch => 0.5,
        };
        Modifier::SimpleDeform { mode, amount }
    }

    /// Midlevel 0.5 as in Blender; the strength is 0.2 m (Blender's 1 m is huge next to the 2 m default cube).
    pub fn displace() -> Modifier {
        Modifier::Displace { strength: 0.2, midlevel: 0.5, scale: 1.0 }
    }

    /// Factor 0.5 as in Blender.
    pub fn cast(shape: CastShape) -> Modifier {
        Modifier::Cast { shape, factor: 0.5, radius: 0.0 }
    }

    pub fn wireframe() -> Modifier {
        Modifier::Wireframe { thickness: 0.02 }
    }

    pub fn weld() -> Modifier {
        Modifier::Weld { distance: 0.001 }
    }

    /// The name shown in the modifier panel.
    pub fn label(&self) -> &'static str {
        match self {
            Modifier::Mirror { .. } => "Mirror",
            Modifier::Array { .. } => "Array",
            Modifier::Solidify { .. } => "Solidify",
            Modifier::Bevel { .. } => "Bevel",
            Modifier::Smooth { .. } => "Smooth",
            Modifier::SimpleDeform { .. } => "Simple Deform",
            Modifier::Displace { .. } => "Displace",
            Modifier::Triangulate => "Triangulate",
            Modifier::Cast { .. } => "Cast",
            Modifier::Wireframe { .. } => "Wireframe",
            Modifier::Weld { .. } => "Weld",
        }
    }

    pub fn apply(&self, mesh: &Mesh) -> Mesh {
        match self {
            Modifier::Mirror { axes, merge, threshold } => {
                let mut m = mesh.clone();
                for (axis, on) in axes.iter().enumerate() {
                    if *on {
                        m = m.mirrored(axis, *merge, *threshold);
                    }
                }
                m
            }
            Modifier::Array { count, relative, constant } => mesh.arrayed(*count, *relative, *constant),
            Modifier::Solidify { thickness, offset } => mesh.solidified(*thickness, *offset),
            Modifier::Bevel { width } => mesh.beveled(*width),
            Modifier::Smooth { factor, repeat } => mesh.smoothed(*factor, *repeat),
            Modifier::SimpleDeform { mode, amount } => mesh.deformed(*mode, *amount),
            Modifier::Displace { strength, midlevel, scale } => mesh.displaced(*strength, *midlevel, *scale),
            Modifier::Triangulate => mesh.triangulated(),
            Modifier::Cast { shape, factor, radius } => mesh.casted(*shape, *factor, *radius),
            Modifier::Wireframe { thickness } => mesh.wireframed(*thickness),
            Modifier::Weld { distance } => mesh.merged_by_distance(*distance).0,
        }
    }
}

/// The result of the whole stack.
pub fn evaluate(mesh: &Mesh, stack: &[Modifier]) -> Mesh {
    stack.iter().fold(mesh.clone(), |m, md| md.apply(&m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stack_runs_in_order() {
        /* Array then mirror: 2 cubes, then mirrored in X (the array starts at the origin so the cubes overlap the plane). */
        let cube = Mesh::cube(2.0);
        let a = evaluate(&cube, &[Modifier::array(2)]);
        assert_eq!(a.faces.len(), 12);
        let b = evaluate(&cube, &[Modifier::array(2), Modifier::mirror(2)]);
        assert_eq!(b.faces.len(), 24);
        assert_eq!(evaluate(&cube, &[]), cube);
    }

    #[test]
    fn mirror_axes_combine() {
        let cube = Mesh::cube(2.0).mapped(|v| [v[0] + 3.0, v[1] + 3.0, v[2]], false);
        let m = Modifier::Mirror { axes: [true, true, false], merge: true, threshold: 0.001 };
        assert_eq!(m.apply(&cube).faces.len(), 24);
    }
    #[test]
    fn bevel_in_the_stack() {
        let cube = Mesh::cube(2.0);
        assert_eq!(evaluate(&cube, &[Modifier::bevel()]).faces.len(), 26);
        /* Bevelling a mirrored half cube chamfers the seam too only when it is welded: 12 + ... faces, still closed. */
        let r = evaluate(&cube, &[Modifier::array(2), Modifier::bevel()]);
        assert_eq!(r.faces.len(), 2 * 26);
    }

    #[test]
    fn smooth_deform_and_weld_in_the_stack() {
        let cube = Mesh::cube(2.0);
        let r = evaluate(&cube, &[Modifier::simple_deform(DeformMode::Twist), Modifier::smooth()]);
        assert_eq!(r.faces, cube.faces);
        assert_ne!(r.verts, cube.verts);
        /* Two touching cubes weld into one closed piece of 12 vertices. */
        let two = evaluate(&cube, &[Modifier::array(2), Modifier::Weld { distance: 0.001 }]);
        assert_eq!(two.verts.len(), 12);
    }

    #[test]
    fn displace_and_triangulate_in_the_stack() {
        let cube = Mesh::cube(2.0);
        let r = evaluate(&cube, &[Modifier::displace(), Modifier::Triangulate]);
        assert_eq!(r.faces.len(), 12);
        assert_ne!(r.verts, cube.verts);
        let b = evaluate(&cube, &[Modifier::simple_deform(DeformMode::Bend)]);
        assert_ne!(b.verts, cube.verts);
    }

    #[test]
    fn cast_and_wireframe_in_the_stack() {
        let cube = Mesh::cube(2.0);
        /* A cube's corners are all equally far out, so the default (average) radius leaves it alone. */
        assert_eq!(evaluate(&cube, &[Modifier::cast(CastShape::Sphere)]).verts, cube.verts);
        let c = evaluate(&cube, &[Modifier::Cast { shape: CastShape::Sphere, factor: 0.5, radius: 2.0 }]);
        assert_eq!(c.faces, cube.faces);
        assert_ne!(c.verts, cube.verts);
        let w = evaluate(&cube, &[Modifier::wireframe()]);
        assert_eq!(w.faces.len(), 96);
        /* Wireframe after Triangulate frames the triangles: 12 * 3 * 4. */
        let t = evaluate(&cube, &[Modifier::Triangulate, Modifier::wireframe()]);
        assert_eq!(t.faces.len(), 12 * 3 * 4);
    }
}
