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
    /// `merge` is the merge distance in metres (0 is off): vertices of neighbouring copies that touch are joined.
    Array { count: u32, relative: V3, constant: V3, merge: f32 },
    Solidify { thickness: f32, offset: f32 },
    /// `Offset` width, `segments` segments (1 is a chamfer).
    Bevel { width: f32, segments: u32 },
    /// Neighbour averaging: `factor` per round, `repeat` rounds.
    Smooth { factor: f32, repeat: u32 },
    /// Twist (radians) / Taper / Stretch (factor) about the Z axis.
    SimpleDeform { mode: DeformMode, amount: f32 },
    /// Noise displacement along the normals.
    Displace { strength: f32, midlevel: f32, scale: f32 },
    Triangulate,
    /// Blend towards a sphere or cylinder; radius 0 is the average distance.
    Cast { shape: CastShape, factor: f32, radius: f32 },
    /// Collapse decimation keeping `ratio` of the triangles.
    Decimate { ratio: f32 },
    /// Cut edges sharper than the angle (degrees).
    EdgeSplit { angle: f32 },
    /// Frames of the given thickness instead of faces.
    Wireframe { thickness: f32 },
    /// Merge by distance.
    Weld { distance: f32 },
    /// Revolve the profile edges about Z by `angle` radians (default one full turn), `steps` steps, rising
    /// `offset` per turn.
    Screw { angle: f32, steps: u32, offset: f32 },
    /// Circular sine ripple along Z.
    Wave { height: f32, width: f32, speed: f32, time: f32 },
    /// Laplacian smoothing that keeps the volume.
    LaplacianSmooth { factor: f32, repeat: u32 },
    /// Remesh, Blocks mode: voxel size in metres.
    Remesh { voxel: f32 },
    /// Nearest Surface Point wrap onto the object with id `target`, keeping `offset` metres away.
    Shrinkwrap { target: Option<u32>, offset: f32 },
}

/// The offsets the Array key cycles through: a label, the relative offset (a factor of the bounds size) and the
/// constant one (metres). The first is the default.
pub const ARRAY_PRESETS: [(&str, V3, V3); 6] = [
    ("relative X", [1.0, 0.0, 0.0], [0.0; 3]),
    ("relative Y", [0.0, 1.0, 0.0], [0.0; 3]),
    ("relative Z", [0.0, 0.0, 1.0], [0.0; 3]),
    ("+1 m X", [0.0; 3], [1.0, 0.0, 0.0]),
    ("+1 m Y", [0.0; 3], [0.0, 1.0, 0.0]),
    ("+1 m Z", [0.0; 3], [0.0, 0.0, 1.0]),
];

/// Which preset an Array offset is, `None` for a custom one.
pub fn array_preset(relative: V3, constant: V3) -> Option<usize> {
    ARRAY_PRESETS.iter().position(|p| p.1 == relative && p.2 == constant)
}

/// What a modifier needs from the rest of the scene: the target meshes, already in the space of the object
/// that owns the stack (built by `Scene`).
#[derive(Clone, Debug, Default)]
pub struct EvalCtx {
    pub targets: std::collections::HashMap<u32, Mesh>,
}

impl Modifier {
    pub fn mirror(axis: usize) -> Modifier {
        let mut axes = [false; 3];
        axes[axis.min(2)] = true;
        Modifier::Mirror { axes, merge: true, threshold: 0.001 }
    }

    pub fn array(count: u32) -> Modifier {
        Modifier::Array { count: count.max(1), relative: [1.0, 0.0, 0.0], constant: [0.0; 3], merge: 0.0 }
    }

    pub fn solidify() -> Modifier {
        Modifier::Solidify { thickness: 0.01, offset: -1.0 }
    }

    pub fn bevel() -> Modifier {
        Modifier::Bevel { width: 0.1, segments: 1 }
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

    pub fn decimate(ratio: f32) -> Modifier {
        Modifier::Decimate { ratio }
    }

    /// 30 degrees as in Blender.
    pub fn edge_split() -> Modifier {
        Modifier::EdgeSplit { angle: 30.0 }
    }

    pub fn weld() -> Modifier {
        Modifier::Weld { distance: 0.001 }
    }

    /// Defaults of `DNA_modifier_defaults.h`: 360 degrees, 16 steps, no offset.
    pub fn screw() -> Modifier {
        Modifier::Screw { angle: std::f32::consts::TAU, steps: crate::mesh_gen::SCREW_STEPS, offset: 0.0 }
    }

    /// Width 1.5 and speed 0.25 as in Blender; the height is 0.1 m (Blender's 0.5 m is huge next to the 2 m cube).
    pub fn wave() -> Modifier {
        Modifier::Wave { height: 0.1, width: 1.5, speed: 0.25, time: 0.0 }
    }

    pub fn shrinkwrap() -> Modifier {
        Modifier::Shrinkwrap { target: None, offset: 0.0 }
    }

    pub fn laplacian_smooth() -> Modifier {
        Modifier::LaplacianSmooth { factor: 0.5, repeat: 2 }
    }

    /// Blender's Blocks mode starts at octree depth 4 (16 cells across the object); 0.1 m is a sensible voxel here.
    pub fn remesh() -> Modifier {
        Modifier::Remesh { voxel: 0.1 }
    }

    /// One default instance of every modifier kind, in the order of the Add Modifier list of the properties editor.
    pub fn catalog() -> Vec<Modifier> {
        use crate::mesh_deform::{CastShape, DeformMode};
        vec![
            Modifier::mirror(0),
            Modifier::array(2),
            Modifier::solidify(),
            Modifier::bevel(),
            Modifier::smooth(),
            Modifier::laplacian_smooth(),
            Modifier::simple_deform(DeformMode::Twist),
            Modifier::displace(),
            Modifier::wave(),
            Modifier::cast(CastShape::Sphere),
            Modifier::triangulate(),
            Modifier::decimate(0.5),
            Modifier::edge_split(),
            Modifier::wireframe(),
            Modifier::weld(),
            Modifier::screw(),
            Modifier::remesh(),
            Modifier::shrinkwrap(),
        ]
    }

    pub fn triangulate() -> Modifier {
        Modifier::Triangulate
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
            Modifier::Decimate { .. } => "Decimate",
            Modifier::EdgeSplit { .. } => "Edge Split",
            Modifier::Wireframe { .. } => "Wireframe",
            Modifier::Weld { .. } => "Weld",
            Modifier::Screw { .. } => "Screw",
            Modifier::Wave { .. } => "Wave",
            Modifier::LaplacianSmooth { .. } => "Laplacian Smooth",
            Modifier::Remesh { .. } => "Remesh",
            Modifier::Shrinkwrap { .. } => "Shrinkwrap",
        }
    }

    /// One line for the viewport overlay: the name followed by the main settings.
    pub fn summary(&self) -> String {
        fn num(f: f32) -> String {
            let s = format!("{:.3}", f);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        let detail = match self {
            Modifier::Mirror { axes, .. } => {
                let names: String = axes.iter().zip(["X", "Y", "Z"]).filter(|(on, _)| **on).map(|(_, n)| n).collect();
                format!(" {names}")
            }
            Modifier::Array { count, relative, constant, merge } => {
                let mut d = format!(" x{count}");
                if let Some(i) = array_preset(*relative, *constant).filter(|&i| i != 0) {
                    d.push_str(&format!(" {}", ARRAY_PRESETS[i].0));
                } else if array_preset(*relative, *constant).is_none() {
                    d.push_str(" custom offset");
                }
                if *merge > 0.0 {
                    d.push_str(&format!(" merge {} m", num(*merge)));
                }
                d
            }
            Modifier::Solidify { thickness, .. } => format!(" {} m", num(*thickness)),
            Modifier::Bevel { width, segments } if *segments > 1 => format!(" {} m x{segments}", num(*width)),
            Modifier::Bevel { width, .. } => format!(" {} m", num(*width)),
            Modifier::Smooth { factor, repeat } => format!(" {} x{repeat}", num(*factor)),
            Modifier::SimpleDeform { mode, amount } => match mode {
                DeformMode::Twist | DeformMode::Bend => format!(": {} {}\u{b0}", mode.label(), num(amount.to_degrees())),
                DeformMode::Taper | DeformMode::Stretch => format!(": {} {}", mode.label(), num(*amount)),
            },
            Modifier::Displace { strength, .. } => format!(" {} m", num(*strength)),
            Modifier::Triangulate => String::new(),
            Modifier::Cast { shape, factor, .. } => format!(": {} {}", shape.label(), num(*factor)),
            Modifier::Decimate { ratio } => format!(" {}", num(*ratio)),
            Modifier::EdgeSplit { angle } => format!(" {}\u{b0}", num(*angle)),
            Modifier::Wireframe { thickness } => format!(" {} m", num(*thickness)),
            Modifier::Weld { distance } => format!(" {} m", num(*distance)),
            Modifier::Screw { angle, steps, .. } => format!(" {}\u{b0} x{steps}", num(angle.to_degrees())),
            Modifier::Wave { height, .. } => format!(" {} m", num(*height)),
            Modifier::LaplacianSmooth { factor, repeat } => format!(" {} x{repeat}", num(*factor)),
            Modifier::Remesh { voxel } => format!(": Blocks {} m", num(*voxel)),
            Modifier::Shrinkwrap { offset, .. } => format!(" {} m", num(*offset)),
        };
        format!("{}{}", self.label(), detail)
    }

    /// Raises (`steps` > 0) or lowers the main value of the modifier by `steps` notches and returns whether it
    /// changed. The notch and the range per modifier: Array count 1, Solidify thickness 0.01 m, Bevel width
    /// 0.01 m, Smooth factor 0.1 (0 to 2), Twist / Bend 5 degrees, Taper / Stretch 0.1, Displace strength
    /// 0.05 m, Cast factor 0.1 (0 to 1), Decimate ratio 0.05 (0.01 to 1), Edge Split angle 5 degrees
    /// (0 to 180), Wireframe thickness 0.005 m, Weld distance 0.001 m, Screw angle 15 degrees (1 to 360),
    /// Wave height 0.05 m, Laplacian Smooth factor 0.1 (0 to 2), Remesh voxel size 0.01 m (min 0.01),
    /// Shrinkwrap offset 0.01 m. Mirror and Triangulate have no value.
    pub fn adjust(&mut self, steps: i32) -> bool {
        let n = steps as f32;
        let round = |x: f32| (x * 10000.0).round() / 10000.0;
        let before = self.clone();
        match self {
            Modifier::Mirror { .. } | Modifier::Triangulate => {}
            Modifier::Array { count, .. } => *count = (*count as i64 + steps as i64).clamp(1, 1000) as u32,
            Modifier::Solidify { thickness, .. } => *thickness = round((*thickness + 0.01 * n).max(0.001)),
            Modifier::Bevel { width, .. } => *width = round((*width + 0.01 * n).max(0.001)),
            Modifier::Smooth { factor, .. } => *factor = round((*factor + 0.1 * n).clamp(0.0, 2.0)),
            Modifier::SimpleDeform { mode, amount } => match mode {
                DeformMode::Twist | DeformMode::Bend => *amount += 5.0f32.to_radians() * n,
                DeformMode::Taper | DeformMode::Stretch => *amount = round(*amount + 0.1 * n),
            },
            Modifier::Displace { strength, .. } => *strength = round(*strength + 0.05 * n),
            Modifier::Cast { factor, .. } => *factor = round((*factor + 0.1 * n).clamp(0.0, 1.0)),
            Modifier::Decimate { ratio } => *ratio = round((*ratio + 0.05 * n).clamp(0.01, 1.0)),
            Modifier::EdgeSplit { angle } => *angle = round((*angle + 5.0 * n).clamp(0.0, 180.0)),
            Modifier::Wireframe { thickness } => *thickness = round((*thickness + 0.005 * n).max(0.001)),
            Modifier::Weld { distance } => *distance = round((*distance + 0.001 * n).max(0.0001)),
            Modifier::Screw { angle, .. } => {
                let deg = ((angle.to_degrees() + 15.0 * n).clamp(1.0, 360.0) * 100.0).round() / 100.0;
                *angle = if deg >= 360.0 { std::f32::consts::TAU } else { deg.to_radians() };
            }
            Modifier::Wave { height, .. } => *height = round(*height + 0.05 * n),
            Modifier::LaplacianSmooth { factor, .. } => *factor = round((*factor + 0.1 * n).clamp(0.0, 2.0)),
            Modifier::Remesh { voxel } => *voxel = round((*voxel + 0.01 * n).max(0.01)),
            Modifier::Shrinkwrap { offset, .. } => *offset = round((*offset + 0.01 * n).clamp(-10.0, 10.0)),
        }
        *self != before
    }

    pub fn apply(&self, mesh: &Mesh) -> Mesh {
        self.apply_with(mesh, &EvalCtx::default())
    }

    /// `apply()` with the scene context; a Shrinkwrap without a (found) target leaves the mesh alone.
    pub fn apply_with(&self, mesh: &Mesh, ctx: &EvalCtx) -> Mesh {
        match self {
            Modifier::Shrinkwrap { target, offset } => match target.and_then(|t| ctx.targets.get(&t)) {
                Some(t) => mesh.shrinkwrapped(t, *offset),
                None => mesh.clone(),
            },
            Modifier::Mirror { axes, merge, threshold } => {
                let mut m = mesh.clone();
                for (axis, on) in axes.iter().enumerate() {
                    if *on {
                        m = m.mirrored(axis, *merge, *threshold);
                    }
                }
                m
            }
            Modifier::Array { count, relative, constant, merge } => {
                let m = mesh.arrayed(*count, *relative, *constant);
                if *merge > 0.0 {
                    m.merged_by_distance(*merge).0
                } else {
                    m
                }
            }
            Modifier::Solidify { thickness, offset } => mesh.solidified(*thickness, *offset),
            Modifier::Bevel { width, segments } => mesh.beveled_segments(*width, *segments),
            Modifier::Smooth { factor, repeat } => mesh.smoothed(*factor, *repeat),
            Modifier::SimpleDeform { mode, amount } => mesh.deformed(*mode, *amount),
            Modifier::Displace { strength, midlevel, scale } => mesh.displaced(*strength, *midlevel, *scale),
            Modifier::Triangulate => mesh.triangulated(),
            Modifier::Cast { shape, factor, radius } => mesh.casted(*shape, *factor, *radius),
            Modifier::Decimate { ratio } => mesh.decimated(*ratio),
            Modifier::EdgeSplit { angle } => mesh.edge_split(*angle),
            Modifier::Wireframe { thickness } => mesh.wireframed(*thickness),
            Modifier::Weld { distance } => mesh.merged_by_distance(*distance).0,
            Modifier::Screw { angle, steps, offset } => mesh.screwed(*angle, *steps, *offset),
            Modifier::Wave { height, width, speed, time } => mesh.waved(*height, *width, *speed, *time),
            Modifier::LaplacianSmooth { factor, repeat } => mesh.laplacian_smoothed(*factor, *repeat),
            Modifier::Remesh { voxel } => mesh.remeshed_blocks(*voxel),
        }
    }
}

/// The result of the whole stack.
pub fn evaluate(mesh: &Mesh, stack: &[Modifier]) -> Mesh {
    evaluate_with(mesh, stack, &EvalCtx::default())
}

pub fn evaluate_with(mesh: &Mesh, stack: &[Modifier], ctx: &EvalCtx) -> Mesh {
    stack.iter().fold(mesh.clone(), |m, md| md.apply_with(&m, ctx))
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

    #[test]
    fn decimate_and_edge_split_in_the_stack() {
        let sphere = Mesh::uv_sphere(32, 16, 1.0);
        let d = evaluate(&sphere, &[Modifier::decimate(0.5)]);
        assert!(d.faces.len() < sphere.triangulated().faces.len());
        let cube = Mesh::cube(2.0);
        assert_eq!(evaluate(&cube, &[Modifier::edge_split()]).verts.len(), 24);
    }

    #[test]
    fn screw_and_wave_in_the_stack() {
        let plane = Mesh::plane(2.0).mapped(|v| [v[0] + 3.0, v[1], v[2]], false);
        let r = evaluate(&plane, &[Modifier::screw()]);
        assert_eq!(r.faces.len(), 4 * 16);
        let g = Mesh::grid(8, 8, 4.0);
        let w = evaluate(&g, &[Modifier::wave()]);
        assert_eq!(w.faces, g.faces);
        assert_ne!(w.verts, g.verts);
        assert_eq!(Modifier::screw().summary(), "Screw 360\u{b0} x16");
        assert_eq!(Modifier::wave().summary(), "Wave 0.1 m");
        let mut s = Modifier::screw();
        assert!(s.adjust(-1) && s.summary() == "Screw 345\u{b0} x16");
        assert!(s.adjust(1) && s == Modifier::screw());
        assert!(!s.adjust(1));
    }

    #[test]
    fn laplacian_smooth_and_remesh_in_the_stack() {
        let cube = Mesh::cube(2.0);
        let r = evaluate(&cube, &[Modifier::Remesh { voxel: 0.5 }]);
        assert_eq!(r.faces.len(), 96);
        let s = evaluate(&r, &[Modifier::laplacian_smooth()]);
        assert_eq!(s.faces, r.faces);
        assert_eq!(Modifier::laplacian_smooth().summary(), "Laplacian Smooth 0.5 x2");
        assert_eq!(Modifier::remesh().summary(), "Remesh: Blocks 0.1 m");
        let mut m = Modifier::remesh();
        assert!(m.adjust(1) && m.summary() == "Remesh: Blocks 0.11 m");
        assert!(m.adjust(-100) && m == Modifier::Remesh { voxel: 0.01 });
        assert!(!m.adjust(-1));
    }

    #[test]
    fn shrinkwrap_uses_its_target_from_the_context() {
        let cube = Mesh::cube(2.0);
        let mut ctx = EvalCtx::default();
        let m = Modifier::Shrinkwrap { target: Some(7), offset: 0.0 };
        assert_eq!(evaluate_with(&cube, &[m.clone()], &ctx), cube, "no target found: unchanged");
        ctx.targets.insert(7, Mesh::uv_sphere(16, 8, 2.0));
        let r = evaluate_with(&cube, &[m], &ctx);
        assert_ne!(r.verts, cube.verts);
        assert_eq!(Modifier::shrinkwrap().summary(), "Shrinkwrap 0 m");
    }

    #[test]
    fn the_catalog_has_every_kind_once() {
        let c = Modifier::catalog();
        assert_eq!(c.len(), 18);
        let mut names: Vec<&str> = c.iter().map(|m| m.label()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 18);
        /* Every entry evaluates on a cube without panicking. */
        for m in &c {
            let _ = m.apply(&Mesh::cube(2.0));
        }
    }

    #[test]
    fn summaries_show_the_main_settings() {
        assert_eq!(Modifier::Mirror { axes: [true, false, true], merge: true, threshold: 0.001 }.summary(), "Mirror XZ");
        assert_eq!(Modifier::array(3).summary(), "Array x3");
        assert_eq!(Modifier::solidify().summary(), "Solidify 0.01 m");
        assert_eq!(Modifier::bevel().summary(), "Bevel 0.1 m");
        assert_eq!(Modifier::smooth().summary(), "Smooth 0.5 x1");
        assert_eq!(Modifier::simple_deform(DeformMode::Twist).summary(), "Simple Deform: Twist 45\u{b0}");
        assert_eq!(Modifier::simple_deform(DeformMode::Taper).summary(), "Simple Deform: Taper 0.5");
        assert_eq!(Modifier::Triangulate.summary(), "Triangulate");
        assert_eq!(Modifier::cast(CastShape::Cylinder).summary(), "Cast: Cylinder 0.5");
        assert_eq!(Modifier::decimate(0.25).summary(), "Decimate 0.25");
        assert_eq!(Modifier::edge_split().summary(), "Edge Split 30\u{b0}");
    }

    #[test]
    fn adjust_changes_the_main_value_within_its_range() {
        let mut a = Modifier::array(2);
        assert!(a.adjust(1) && a == Modifier::array(3));
        assert!(a.adjust(-5) && a == Modifier::array(1));
        assert!(!a.adjust(-1));
        let mut b = Modifier::bevel();
        assert!(b.adjust(2) && b.summary() == "Bevel 0.12 m");
        let mut d = Modifier::decimate(0.5);
        assert!(d.adjust(100) && d == Modifier::decimate(1.0));
        assert!(!d.adjust(1));
        assert!(d.adjust(-200) && d == Modifier::decimate(0.01));
        let mut t = Modifier::simple_deform(DeformMode::Twist);
        assert!(t.adjust(1) && t.summary() == "Simple Deform: Twist 50\u{b0}");
        let mut e = Modifier::edge_split();
        assert!(e.adjust(-100) && e == Modifier::EdgeSplit { angle: 0.0 });
        assert!(!Modifier::Triangulate.adjust(1));
        assert!(!Modifier::mirror(0).adjust(1));
        /* Every notch of every modifier is reversible. */
        let mut s = Modifier::smooth();
        s.adjust(3);
        s.adjust(-3);
        assert_eq!(s, Modifier::smooth());
    }

    #[test]
    fn array_summary_shows_non_default_offset_and_merge() {
        assert_eq!(Modifier::array(3).summary(), "Array x3");
        let m = Modifier::Array { count: 2, relative: [0.0; 3], constant: [0.0, 1.0, 0.0], merge: 0.001 };
        assert_eq!(m.summary(), "Array x2 +1 m Y merge 0.001 m");
        let m = Modifier::Array { count: 2, relative: [0.5, 0.0, 0.0], constant: [0.0; 3], merge: 0.0 };
        assert_eq!(m.summary(), "Array x2 custom offset");
    }

    #[test]
    fn array_merge_joins_touching_copies() {
        /* Two unit cubes side by side along X share a face: 16 vertices become 12 once merged. */
        let cube = Mesh::cube(1.0);
        let plain = Modifier::Array { count: 2, relative: [1.0, 0.0, 0.0], constant: [0.0; 3], merge: 0.0 };
        let merged = Modifier::Array { count: 2, relative: [1.0, 0.0, 0.0], constant: [0.0; 3], merge: 0.001 };
        assert_eq!(plain.apply(&cube).verts.len(), 16);
        assert_eq!(merged.apply(&cube).verts.len(), 12);
    }

    #[test]
    fn array_presets_are_distinct_and_the_first_is_the_default() {
        assert_eq!(array_preset([1.0, 0.0, 0.0], [0.0; 3]), Some(0));
        assert_eq!(array_preset([0.3, 0.0, 0.0], [0.0; 3]), None);
        for (i, p) in ARRAY_PRESETS.iter().enumerate() {
            assert_eq!(array_preset(p.1, p.2), Some(i));
        }
    }

    #[test]
    fn bevel_segments_show_in_the_summary_and_the_evaluation() {
        assert_eq!(Modifier::bevel().summary(), "Bevel 0.1 m");
        let m = Modifier::Bevel { width: 0.1, segments: 3 };
        assert_eq!(m.summary(), "Bevel 0.1 m x3");
        let cube = Mesh::cube(2.0);
        assert!(m.apply(&cube).verts.len() > Modifier::bevel().apply(&cube).verts.len());
    }
}
