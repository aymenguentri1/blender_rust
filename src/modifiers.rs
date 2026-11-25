//! The modifier stack of an object (Blender's `Object::modifiers`), evaluated in order on the base mesh
//! before the Subdivision Surface level, which is why a mirrored or arrayed object subdivides as one piece.
//!
//! Defaults follow `DNA_modifier_defaults.h`: Mirror merges at 0.001 m, Array has a count of 2 and a relative
//! offset of (1, 0, 0), Solidify a thickness of 0.01 m and an offset of -1, Bevel a width of 0.1 m.

use crate::mesh::{Mesh, V3};

#[derive(Clone, Debug, PartialEq)]
pub enum Modifier {
    Mirror { axes: [bool; 3], merge: bool, threshold: f32 },
    Array { count: u32, relative: V3, constant: V3 },
    Solidify { thickness: f32, offset: f32 },
    /// One segment, `Offset` width.
    Bevel { width: f32 },
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

    /// The name shown in the modifier panel.
    pub fn label(&self) -> &'static str {
        match self {
            Modifier::Mirror { .. } => "Mirror",
            Modifier::Array { .. } => "Array",
            Modifier::Solidify { .. } => "Solidify",
            Modifier::Bevel { .. } => "Bevel",
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
}
