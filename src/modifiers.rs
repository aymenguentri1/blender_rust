//! The modifier stack of an object (Blender's `Object::modifiers`), evaluated in order on the base mesh
//! before the Subdivision Surface level, which is why a mirrored or arrayed object subdivides as one piece.
//!
//! Defaults follow `DNA_modifier_defaults.h`: Mirror merges at 0.001 m, Array has a count of 2 and a relative
//! offset of (1, 0, 0), Solidify a thickness of 0.01 m and an offset of -1, Bevel a width of 0.1 m.

use crate::mesh::{Mesh, V3};
use crate::mesh_boolean::BoolOp;
use crate::mesh_deform::{CastShape, DeformMode, DisplaceDir};

#[derive(Clone, Debug, PartialEq)]
pub enum Modifier {
    Mirror { axes: [bool; 3], merge: bool, threshold: f32 },
    /// `merge` is the merge distance in metres (0 is off): vertices of neighbouring copies that touch are joined.
    ///
    /// `fit` is Blender's Fit Length in metres (0 is Fixed Count): the number of copies is then the length divided by
    /// the length of the offset, rounded down, and `count` is not used.
    Array { count: u32, relative: V3, constant: V3, merge: f32, fit: f32 },
    /// `even` is Even Thickness: the shell is stretched at corners so the wall keeps its thickness there.
    Solidify { thickness: f32, offset: f32, even: bool },
    /// `Offset` width, `segments` segments (1 is a chamfer).
    Bevel { width: f32, segments: u32 },
    /// Neighbour averaging: `factor` per round, `repeat` rounds.
    Smooth { factor: f32, repeat: u32 },
    /// Twist (radians) / Taper / Stretch (factor) about the Z axis.
    SimpleDeform { mode: DeformMode, amount: f32 },
    /// Noise displacement along the normals.
    Displace { strength: f32, midlevel: f32, scale: f32, dir: DisplaceDir },
    Triangulate,
    /// Blend towards a sphere or cylinder; radius 0 is the average distance.
    Cast { shape: CastShape, factor: f32, radius: f32 },
    /// Collapse decimation keeping `ratio` of the triangles.
    Decimate { ratio: f32 },
    /// Catmull-Clark subdivision (`MOD_subsurf.cc`): `levels` is Levels Viewport.
    Subsurf { levels: u32 },
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
    /// Reveals the faces over `length` frames from frame `start` (taken apart again with `reverse`), in random
    /// order for the `seed` when `randomize`.
    Build { start: f32, length: f32, reverse: bool, randomize: bool, seed: u32 },
    /// Difference / Union / Intersect with the mesh object with id `target` (Fast solver, closed meshes). Without
    /// a target the mesh is left alone.
    Boolean { op: BoolOp, target: Option<u32> },
    /// Simple smoothing that puts the lost detail of the stack's starting mesh back (`factor` per round).
    CorrectiveSmooth { factor: f32, repeat: u32, scale: f32 },
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
#[derive(Clone, Debug)]
pub struct EvalCtx {
    pub targets: std::collections::HashMap<u32, Mesh>,
    /// `Scene::r.cfra`, for the modifiers that depend on time (Build).
    pub frame: f32,
}

impl Default for EvalCtx {
    /// No targets, at the startup frame 1.
    fn default() -> EvalCtx {
        EvalCtx { targets: Default::default(), frame: 1.0 }
    }
}

impl Modifier {
    pub fn mirror(axis: usize) -> Modifier {
        let mut axes = [false; 3];
        axes[axis.min(2)] = true;
        Modifier::Mirror { axes, merge: true, threshold: 0.001 }
    }

    pub fn array(count: u32) -> Modifier {
        Modifier::Array { count: count.max(1), relative: [1.0, 0.0, 0.0], constant: [0.0; 3], merge: 0.0, fit: 0.0 }
    }

    pub fn solidify() -> Modifier {
        Modifier::Solidify { thickness: 0.01, offset: -1.0, even: false }
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
        Modifier::Displace { strength: 0.2, midlevel: 0.5, scale: 1.0, dir: DisplaceDir::Normal }
    }

    /// Factor 0.5 as in Blender.
    pub fn cast(shape: CastShape) -> Modifier {
        Modifier::Cast { shape, factor: 0.5, radius: 0.0 }
    }

    pub fn wireframe() -> Modifier {
        Modifier::Wireframe { thickness: 0.02 }
    }

    /// Levels Viewport 1 as in `DNA_modifier_defaults.h` (`_DNA_DEFAULT_SubsurfModifierData`).
    pub fn subsurf() -> Modifier {
        Modifier::Subsurf { levels: 1 }
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

    /// Defaults of `DNA_modifier_defaults.h`: starts at frame 1 and takes 100 frames, in mesh order, seed 0.
    pub fn build() -> Modifier {
        Modifier::Build { start: 1.0, length: 100.0, reverse: false, randomize: false, seed: 0 }
    }

    /// Defaults of `DNA_modifier_defaults.h`: factor 0.5, 5 repeats, scale 1.
    pub fn corrective_smooth() -> Modifier {
        Modifier::CorrectiveSmooth { factor: 0.5, repeat: 5, scale: 1.0 }
    }

    pub fn shrinkwrap() -> Modifier {
        Modifier::Shrinkwrap { target: None, offset: 0.0 }
    }

    /// Blender's default operation is Difference.
    pub fn boolean() -> Modifier {
        Modifier::Boolean { op: BoolOp::Difference, target: None }
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
            Modifier::corrective_smooth(),
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
            Modifier::build(),
            Modifier::shrinkwrap(),
            Modifier::boolean(),
            Modifier::subsurf(),
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
            Modifier::Subsurf { .. } => "Subdivision Surface",
            Modifier::EdgeSplit { .. } => "Edge Split",
            Modifier::Wireframe { .. } => "Wireframe",
            Modifier::Weld { .. } => "Weld",
            Modifier::Screw { .. } => "Screw",
            Modifier::Wave { .. } => "Wave",
            Modifier::LaplacianSmooth { .. } => "Laplacian Smooth",
            Modifier::Remesh { .. } => "Remesh",
            Modifier::Shrinkwrap { .. } => "Shrinkwrap",
            Modifier::Boolean { .. } => "Boolean",
            Modifier::Build { .. } => "Build",
            Modifier::CorrectiveSmooth { .. } => "Corrective Smooth",
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
            Modifier::Array { count, relative, constant, merge, fit } => {
                let mut d = if *fit > 0.0 { format!(" fit {} m", num(*fit)) } else { format!(" x{count}") };
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
            Modifier::Solidify { thickness, even, .. } => format!(" {} m{}", num(*thickness), if *even { " even" } else { "" }),
            Modifier::Bevel { width, segments } if *segments > 1 => format!(" {} m x{segments}", num(*width)),
            Modifier::Bevel { width, .. } => format!(" {} m", num(*width)),
            Modifier::Smooth { factor, repeat } => format!(" {} x{repeat}", num(*factor)),
            Modifier::SimpleDeform { mode, amount } => match mode {
                DeformMode::Twist | DeformMode::Bend => format!(": {} {}\u{b0}", mode.label(), num(amount.to_degrees())),
                DeformMode::Taper | DeformMode::Stretch => format!(": {} {}", mode.label(), num(*amount)),
            },
            Modifier::Displace { strength, dir: DisplaceDir::Normal, .. } => format!(" {} m", num(*strength)),
            Modifier::Displace { strength, dir, .. } => format!(" {} m {}", num(*strength), dir.label()),
            Modifier::Triangulate => String::new(),
            Modifier::Cast { shape, factor, .. } => format!(": {} {}", shape.label(), num(*factor)),
            Modifier::Decimate { ratio } => format!(" {}", num(*ratio)),
            Modifier::Subsurf { levels } => format!(" level {levels}"),
            Modifier::EdgeSplit { angle } => format!(" {}\u{b0}", num(*angle)),
            Modifier::Wireframe { thickness } => format!(" {} m", num(*thickness)),
            Modifier::Weld { distance } => format!(" {} m", num(*distance)),
            Modifier::Screw { angle, steps, .. } => format!(" {}\u{b0} x{steps}", num(angle.to_degrees())),
            Modifier::Wave { height, .. } => format!(" {} m", num(*height)),
            Modifier::LaplacianSmooth { factor, repeat } => format!(" {} x{repeat}", num(*factor)),
            Modifier::Remesh { voxel } => format!(": Blocks {} m", num(*voxel)),
            Modifier::Shrinkwrap { offset, .. } => format!(" {} m", num(*offset)),
            Modifier::Boolean { op, .. } => format!(": {}", op.label()),
            Modifier::CorrectiveSmooth { factor, repeat, .. } => format!(" {} x{repeat}", num(*factor)),
            Modifier::Build { start, length, reverse, randomize, .. } => {
                let mut d = format!(" {}-{}", num(*start), num(*start + *length));
                if *reverse {
                    d.push_str(" reversed");
                }
                if *randomize {
                    d.push_str(" random");
                }
                d
            }
        };
        format!("{}{}", self.label(), detail)
    }

    /// Raises (`steps` > 0) or lowers the main value of the modifier by `steps` notches and returns whether it
    /// changed. The notch and the range per modifier: Array count 1, Solidify thickness 0.01 m, Bevel width
    /// 0.01 m, Smooth factor 0.1 (0 to 2), Twist / Bend 5 degrees, Taper / Stretch 0.1, Displace strength
    /// 0.05 m, Cast factor 0.1 (0 to 1), Decimate ratio 0.05 (0.01 to 1), Edge Split angle 5 degrees
    /// (0 to 180), Wireframe thickness 0.005 m, Weld distance 0.001 m, Screw angle 15 degrees (1 to 360),
    /// Wave height 0.05 m, Laplacian Smooth factor 0.1 (0 to 2), Remesh voxel size 0.01 m (min 0.01),
    /// Shrinkwrap offset 0.01 m, Build duration 10 frames (min 1), Corrective Smooth factor 0.1 (0 to 1). Boolean steps through its operation (Difference, Union, Intersect). Mirror and Triangulate have no value.
    pub fn adjust(&mut self, steps: i32) -> bool {
        let n = steps as f32;
        let round = |x: f32| (x * 10000.0).round() / 10000.0;
        let before = self.clone();
        match self {
            Modifier::Mirror { .. } | Modifier::Triangulate => {}
            Modifier::Boolean { op, .. } => {
                for _ in 0..steps.unsigned_abs() % 3 {
                    *op = if steps > 0 { op.next() } else { op.prev() };
                }
            }
            Modifier::Array { fit, .. } if *fit > 0.0 => *fit = round((*fit + 0.5 * n).max(0.1)),
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
            Modifier::Subsurf { levels } => *levels = (*levels as f32 + n).round().clamp(0.0, 6.0) as u32,
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
            Modifier::CorrectiveSmooth { factor, .. } => *factor = round((*factor + 0.1 * n).clamp(0.0, 1.0)),
            Modifier::Build { length, .. } => *length = round((*length + 10.0 * n).clamp(1.0, 1_000_000.0)),
        }
        *self != before
    }

    /// The value the panel field edits (the one `adjust` steps): its label, the value in display units (degrees for
    /// angles) and the unit suffix. `None` for Mirror, Triangulate and Boolean.
    pub fn main_value(&self) -> Option<(&'static str, f32, &'static str)> {
        Some(match self {
            Modifier::Mirror { .. } | Modifier::Triangulate | Modifier::Boolean { .. } => return None,
            Modifier::Array { fit, .. } if *fit > 0.0 => ("Length", *fit, " m"),
            Modifier::Array { count, .. } => ("Count", *count as f32, ""),
            Modifier::Solidify { thickness, .. } => ("Thickness", *thickness, " m"),
            Modifier::Bevel { width, .. } => ("Width", *width, " m"),
            Modifier::Smooth { factor, .. } => ("Factor", *factor, ""),
            Modifier::SimpleDeform { mode, amount } => match mode {
                DeformMode::Twist | DeformMode::Bend => ("Angle", amount.to_degrees(), " deg"),
                DeformMode::Taper | DeformMode::Stretch => ("Factor", *amount, ""),
            },
            Modifier::Displace { strength, .. } => ("Strength", *strength, " m"),
            Modifier::Cast { factor, .. } => ("Factor", *factor, ""),
            Modifier::Decimate { ratio } => ("Ratio", *ratio, ""),
            Modifier::Subsurf { levels } => ("Levels Viewport", *levels as f32, ""),
            Modifier::EdgeSplit { angle } => ("Angle", *angle, " deg"),
            Modifier::Wireframe { thickness } => ("Thickness", *thickness, " m"),
            Modifier::Weld { distance } => ("Distance", *distance, " m"),
            Modifier::Screw { angle, .. } => ("Angle", angle.to_degrees(), " deg"),
            Modifier::Wave { height, .. } => ("Height", *height, " m"),
            Modifier::LaplacianSmooth { factor, .. } => ("Factor", *factor, ""),
            Modifier::Remesh { voxel } => ("Voxel Size", *voxel, " m"),
            Modifier::Shrinkwrap { offset, .. } => ("Offset", *offset, " m"),
            Modifier::CorrectiveSmooth { factor, .. } => ("Factor", *factor, ""),
            Modifier::Build { length, .. } => ("Length", *length, " fr"),
        })
    }

    /// Sets the value `main_value` reports (display units) with the same limits as `adjust`, like typing into the
    /// panel field. Returns whether the modifier changed; non-finite numbers and modifiers without a value change nothing.
    pub fn set_main_value(&mut self, v: f32) -> bool {
        if !v.is_finite() {
            return false;
        }
        let before = self.clone();
        match self {
            Modifier::Mirror { .. } | Modifier::Triangulate | Modifier::Boolean { .. } => {}
            Modifier::Array { fit, .. } if *fit > 0.0 => *fit = v.max(0.1),
            Modifier::Array { count, .. } => *count = v.round().clamp(1.0, 1000.0) as u32,
            Modifier::Solidify { thickness, .. } => *thickness = v.max(0.001),
            Modifier::Bevel { width, .. } => *width = v.max(0.001),
            Modifier::Smooth { factor, .. } => *factor = v.clamp(0.0, 2.0),
            Modifier::SimpleDeform { mode, amount } => match mode {
                DeformMode::Twist | DeformMode::Bend => *amount = v.to_radians(),
                DeformMode::Taper | DeformMode::Stretch => *amount = v,
            },
            Modifier::Displace { strength, .. } => *strength = v,
            Modifier::Cast { factor, .. } => *factor = v.clamp(0.0, 1.0),
            Modifier::Decimate { ratio } => *ratio = v.clamp(0.01, 1.0),
            Modifier::Subsurf { levels } => *levels = v.round().clamp(0.0, 6.0) as u32,
            Modifier::EdgeSplit { angle } => *angle = v.clamp(0.0, 180.0),
            Modifier::Wireframe { thickness } => *thickness = v.max(0.001),
            Modifier::Weld { distance } => *distance = v.max(0.0001),
            Modifier::Screw { angle, .. } => *angle = if v >= 360.0 { std::f32::consts::TAU } else { v.clamp(1.0, 360.0).to_radians() },
            Modifier::Wave { height, .. } => *height = v,
            Modifier::LaplacianSmooth { factor, .. } => *factor = v.clamp(0.0, 2.0),
            Modifier::Remesh { voxel } => *voxel = v.max(0.01),
            Modifier::Shrinkwrap { offset, .. } => *offset = v.clamp(-10.0, 10.0),
            Modifier::CorrectiveSmooth { factor, .. } => *factor = v.clamp(0.0, 1.0),
            Modifier::Build { length, .. } => *length = v.clamp(1.0, 1_000_000.0),
        }
        *self != before
    }

    #[cfg(test)]
    pub fn apply(&self, mesh: &Mesh) -> Mesh {
        self.apply_with(mesh, &EvalCtx::default())
    }

    /// `apply()` with the scene context; a Shrinkwrap without a (found) target leaves the mesh alone.
    pub fn apply_with(&self, mesh: &Mesh, ctx: &EvalCtx) -> Mesh {
        self.apply_rest(mesh, ctx, &mesh.verts)
    }

    /// `apply_with()` where `rest` are the vertices of the mesh at the start of the stack (Corrective Smooth).
    fn apply_rest(&self, mesh: &Mesh, ctx: &EvalCtx, rest: &[V3]) -> Mesh {
        match self {
            Modifier::CorrectiveSmooth { factor, repeat, scale } => mesh.corrective_smoothed(rest, *factor, *repeat, *scale),
            Modifier::Shrinkwrap { target, offset } => match target.and_then(|t| ctx.targets.get(&t)) {
                Some(t) => mesh.shrinkwrapped(t, *offset),
                None => mesh.clone(),
            },
            Modifier::Boolean { op, target } => match target.and_then(|t| ctx.targets.get(&t)) {
                Some(t) => mesh.boolean(t, *op),
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
            Modifier::Array { count, relative, constant, merge, fit } => {
                let count = mesh.array_fit_count(*relative, *constant, *fit).unwrap_or(*count);
                let m = mesh.arrayed(count, *relative, *constant);
                if *merge > 0.0 {
                    m.merged_by_distance(*merge).0
                } else {
                    m
                }
            }
            Modifier::Solidify { thickness, offset, even } => mesh.solidified(*thickness, *offset, *even),
            Modifier::Bevel { width, segments } => mesh.beveled_segments(*width, *segments),
            Modifier::Smooth { factor, repeat } => mesh.smoothed(*factor, *repeat),
            Modifier::SimpleDeform { mode, amount } => mesh.deformed(*mode, *amount),
            Modifier::Displace { strength, midlevel, scale, dir } => mesh.displaced_along(*strength, *midlevel, *scale, *dir),
            Modifier::Triangulate => mesh.triangulated(),
            Modifier::Cast { shape, factor, radius } => mesh.casted(*shape, *factor, *radius),
            Modifier::Decimate { ratio } => mesh.decimated(*ratio),
            Modifier::Subsurf { levels } => mesh.subdivided(*levels),
            Modifier::EdgeSplit { angle } => mesh.edge_split(*angle),
            Modifier::Wireframe { thickness } => mesh.wireframed(*thickness),
            Modifier::Weld { distance } => mesh.merged_by_distance(*distance).0,
            Modifier::Screw { angle, steps, offset } => mesh.screwed(*angle, *steps, *offset),
            Modifier::Wave { height, width, speed, time } => mesh.waved(*height, *width, *speed, *time),
            Modifier::LaplacianSmooth { factor, repeat } => mesh.laplacian_smoothed(*factor, *repeat),
            Modifier::Remesh { voxel } => mesh.remeshed_blocks(*voxel),
            Modifier::Build { start, length, reverse, randomize, seed } => {
                mesh.built(ctx.frame, *start, *length, *reverse, *randomize, *seed)
            }
        }
    }
}

/// The result of the whole stack.
#[cfg(test)]
pub fn evaluate(mesh: &Mesh, stack: &[Modifier]) -> Mesh {
    evaluate_with(mesh, stack, &EvalCtx::default())
}

pub fn evaluate_with(mesh: &Mesh, stack: &[Modifier], ctx: &EvalCtx) -> Mesh {
    evaluate_flagged(mesh, stack, &[], ctx)
}

/// Per-modifier panel state that is not part of the modifier itself (`ModifierData::mode` / `ui_expand_flag`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModFlags {
    /// `eModifierMode_Realtime` cleared: the modifier is skipped in the viewport (Blender's monitor toggle).
    pub hide_viewport: bool,
    /// `eModifierMode_Editmode` cleared (Blender's edit-mode toggle). This port has no edit mode, so it is panel state only.
    pub hide_editmode: bool,
    /// `eModifierMode_Render` cleared: the modifier is skipped in renders (Blender's camera toggle). Nothing in this
    /// port renders, so it is panel state only; the viewport ignores it.
    pub hide_render: bool,
    /// `ui_expand_flag` cleared: the panel shows only its header.
    pub collapsed: bool,
    /// The name typed into the panel header (`ModifierData::name`); `None` shows the kind's name.
    pub name: Option<String>,
}

/// Like `evaluate_with`, skipping the modifiers whose flags hide them in the viewport. `flags` may be shorter than
/// the stack (the missing ones are the defaults).
pub fn evaluate_flagged(mesh: &Mesh, stack: &[Modifier], flags: &[ModFlags], ctx: &EvalCtx) -> Mesh {
    let shown = |i: usize| !flags.get(i).is_some_and(|f| f.hide_viewport);
    stack.iter().enumerate().filter(|(i, _)| shown(*i)).fold(mesh.clone(), |m, (_, md)| md.apply_rest(&m, ctx, &mesh.verts))
}

/// Keeps `flags` lined up with the stack after `after` was made from `before` by adding one modifier, removing
/// one, or swapping two (every other edit leaves the structure alone).
pub fn resync_flags(before: &[Modifier], after: &[Modifier], flags: &mut Vec<ModFlags>) {
    flags.resize(before.len(), ModFlags::default());
    let first_diff = |a: &[Modifier], b: &[Modifier]| (0..a.len().max(b.len())).find(|&j| a.get(j) != b.get(j)).unwrap_or(0);
    if after.len() == before.len() + 1 {
        flags.insert(first_diff(before, after).min(before.len()), ModFlags::default());
    } else if after.len() + 1 == before.len() {
        flags.remove(first_diff(before, after).min(before.len() - 1));
    } else if after.len() == before.len() {
        let diff: Vec<usize> = (0..after.len()).filter(|&j| before[j] != after[j]).collect();
        if let [a, b] = diff[..] {
            if before[a] == after[b] && before[b] == after[a] {
                flags.swap(a, b);
            }
        }
    }
    flags.resize(after.len(), ModFlags::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_value_can_be_read_and_typed() {
        for m in Modifier::catalog() {
            let mut m = m.clone();
            let Some((label, v, _)) = m.main_value() else {
                assert!(!m.set_main_value(1.0), "{} has no value", m.label());
                continue;
            };
            assert!(!label.is_empty());
            assert!(!m.set_main_value(v), "{}: setting the current value changes nothing", m.label());
            assert!(!m.set_main_value(f32::NAN) && !m.set_main_value(f32::INFINITY));
            /* The value that adjust(1) produces can be typed in and gives the same modifier. */
            let mut stepped = m.clone();
            if stepped.adjust(1) {
                let (_, sv, _) = stepped.main_value().unwrap();
                let mut typed = m.clone();
                assert!(typed.set_main_value(sv), "{}", m.label());
                let (_, tv, _) = typed.main_value().unwrap();
                assert!((tv - sv).abs() < 1e-3, "{}: {tv} vs {sv}", m.label());
            }
        }
        let mut a = Modifier::array(2);
        assert!(a.set_main_value(5.4));
        assert_eq!(a.main_value(), Some(("Count", 5.0, "")), "counts are whole numbers");
        assert!(a.set_main_value(5000.0));
        assert_eq!(a.main_value().unwrap().1, 1000.0);
        assert!(a.set_main_value(-3.0));
        assert_eq!(a.main_value().unwrap().1, 1.0);
        let mut t = Modifier::simple_deform(DeformMode::Twist);
        assert!(t.set_main_value(90.0));
        assert!((t.main_value().unwrap().1 - 90.0).abs() < 1e-3);
        assert_eq!(t.main_value().unwrap().2, " deg");
        let mut sc = Modifier::screw();
        assert!(sc.set_main_value(90.0));
        assert!(sc.set_main_value(720.0));
        assert_eq!(sc.main_value().unwrap().1, 360.0, "a full turn at most");
        let mut d = Modifier::decimate(0.5);
        assert!(d.set_main_value(0.0));
        assert_eq!(d.main_value().unwrap().1, 0.01);
    }

    #[test]
    fn a_hidden_modifier_is_skipped() {
        let cube = Mesh::cube(2.0);
        let stack = [Modifier::array(2), Modifier::mirror(2)];
        let hide = |i: usize| {
            let mut f = vec![ModFlags::default(); 2];
            f[i].hide_viewport = true;
            f
        };
        let ctx = EvalCtx::default();
        assert_eq!(evaluate_flagged(&cube, &stack, &[], &ctx).faces.len(), 24);
        assert_eq!(evaluate_flagged(&cube, &stack, &hide(1), &ctx).faces.len(), 12);
        assert_eq!(evaluate_flagged(&cube, &stack, &hide(0), &ctx).faces.len(), 12);
        let all = vec![ModFlags { hide_viewport: true, ..Default::default() }; 2];
        assert_eq!(evaluate_flagged(&cube, &stack, &all, &ctx), cube);
    }

    #[test]
    fn flags_follow_adds_removals_and_swaps() {
        let (a, b, c) = (Modifier::array(2), Modifier::mirror(0), Modifier::bevel());
        let off = ModFlags { hide_viewport: true, ..Default::default() };
        let mut f = vec![off.clone(), ModFlags::default()];
        resync_flags(&[a.clone(), b.clone()], &[b.clone(), a.clone()], &mut f);
        assert_eq!(f, [ModFlags::default(), off.clone()], "a swap moves the flags");
        resync_flags(&[b.clone(), a.clone()], &[b.clone(), a.clone(), c.clone()], &mut f);
        assert_eq!(f, [ModFlags::default(), off.clone(), ModFlags::default()], "an add at the end gets the defaults");
        resync_flags(&[b.clone(), a.clone(), c.clone()], &[b.clone(), c.clone()], &mut f);
        assert_eq!(f, vec![ModFlags::default(); 2], "removing the hidden one drops its flag");
        let mut short = Vec::new();
        resync_flags(std::slice::from_ref(&b), &[b.clone(), c.clone()], &mut short);
        assert_eq!(short.len(), 2, "a short list is padded");
    }

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
    fn subsurf_modifier_levels() {
        let cube = Mesh::cube(2.0);
        let r = evaluate(&cube, &[Modifier::subsurf()]);
        assert_eq!((r.verts.len(), r.faces.len()), (26, 24));
        assert_eq!(evaluate(&cube, &[Modifier::Subsurf { levels: 0 }]).faces.len(), 6);
        assert_eq!(evaluate(&cube, &[Modifier::Subsurf { levels: 2 }]).faces.len(), 96);
        assert_eq!(Modifier::subsurf().label(), "Subdivision Surface");
        let mut m = Modifier::subsurf();
        m.set_main_value(9.0);
        assert_eq!(m, Modifier::Subsurf { levels: 6 });
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
        assert_eq!(evaluate_with(&cube, std::slice::from_ref(&m), &ctx), cube, "no target found: unchanged");
        ctx.targets.insert(7, Mesh::uv_sphere(16, 8, 2.0));
        let r = evaluate_with(&cube, &[m], &ctx);
        assert_ne!(r.verts, cube.verts);
        assert_eq!(Modifier::shrinkwrap().summary(), "Shrinkwrap 0 m");
    }

    #[test]
    fn build_follows_the_scene_frame() {
        let cube = Mesh::cube(2.0);
        let mut ctx = EvalCtx::default();
        assert_eq!(ctx.frame, 1.0);
        let b = [Modifier::build()];
        assert!(evaluate_with(&cube, &b, &ctx).faces.is_empty(), "frame 1 is the start: nothing built yet");
        ctx.frame = 51.0;
        assert_eq!(evaluate_with(&cube, &b, &ctx).faces.len(), 3);
        ctx.frame = 101.0;
        assert_eq!(evaluate_with(&cube, &b, &ctx), cube);
        assert_eq!(Modifier::build().summary(), "Build 1-101");
        let r = Modifier::Build { start: 1.0, length: 50.0, reverse: true, randomize: true, seed: 2 };
        assert_eq!(r.summary(), "Build 1-51 reversed random");
        let mut m = Modifier::build();
        assert!(m.adjust(1) && m.summary() == "Build 1-111");
        assert!(m.adjust(-100) && m == Modifier::Build { start: 1.0, length: 1.0, reverse: false, randomize: false, seed: 0 });
        assert!(!m.adjust(-1));
    }

    #[test]
    fn corrective_smooth_uses_the_start_of_the_stack_as_rest() {
        let sphere = Mesh::uv_sphere(16, 8, 1.0);
        let cs = Modifier::corrective_smooth();
        assert_eq!(cs.summary(), "Corrective Smooth 0.5 x5");
        /* First in the stack the rest is the input itself: no change. */
        let same = evaluate(&sphere, std::slice::from_ref(&cs));
        assert!(same.verts.iter().zip(&sphere.verts).all(|(a, b)| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3)));
        /* After a deformer it differs from plain smoothing. */
        let tw = Modifier::simple_deform(DeformMode::Taper);
        let a = evaluate(&sphere, &[tw.clone(), cs]);
        let b = evaluate(&sphere, &[tw, Modifier::Smooth { factor: 0.5, repeat: 5 }]);
        assert_ne!(a.verts, b.verts);
        let mut m = Modifier::corrective_smooth();
        assert!(m.adjust(-100) && m.summary() == "Corrective Smooth 0 x5");
    }

    #[test]
    fn boolean_uses_its_target_from_the_context() {
        let cube = Mesh::cube(2.0);
        let cutter = cube.mapped(|v| [v[0] + 1.0, v[1], v[2]], false);
        let mut ctx = EvalCtx::default();
        ctx.targets.insert(3, cutter);
        let m = Modifier::Boolean { op: crate::mesh_boolean::BoolOp::Union, target: Some(3) };
        assert_ne!(m.apply_with(&cube, &ctx), cube);
        /* No target, or one that is not in the context: unchanged. */
        assert_eq!(Modifier::boolean().apply_with(&cube, &ctx), cube);
        assert_eq!(Modifier::Boolean { op: crate::mesh_boolean::BoolOp::Union, target: Some(9) }.apply_with(&cube, &ctx), cube);
        assert_eq!(Modifier::boolean().summary(), "Boolean: Difference");
        let mut m = Modifier::boolean();
        assert!(m.adjust(1) && m.summary() == "Boolean: Union");
        assert!(m.adjust(-1) && m.summary() == "Boolean: Difference");
        assert!(!m.adjust(3), "a full turn is no change");
    }

    #[test]
    fn solidify_even_thickness_shows_in_the_summary_and_the_mesh() {
        let cube = Mesh::cube(2.0);
        let plain = Modifier::Solidify { thickness: 0.2, offset: -1.0, even: false };
        let even = Modifier::Solidify { thickness: 0.2, offset: -1.0, even: true };
        assert_eq!(plain.summary(), "Solidify 0.2 m");
        assert_eq!(even.summary(), "Solidify 0.2 m even");
        assert_eq!(plain.apply(&cube).faces, even.apply(&cube).faces);
        assert_ne!(plain.apply(&cube).verts, even.apply(&cube).verts);
    }

    #[test]
    fn displace_direction_shows_in_the_summary() {
        let mut m = Modifier::displace();
        assert_eq!(m.summary(), "Displace 0.2 m");
        let Modifier::Displace { dir, .. } = &mut m else { unreachable!() };
        *dir = DisplaceDir::Y;
        assert_eq!(m.summary(), "Displace 0.2 m Y");
        let cube = Mesh::cube(2.0);
        assert_ne!(m.apply(&cube), Modifier::displace().apply(&cube));
    }

    #[test]
    fn array_fit_length_derives_the_count() {
        let cube = Mesh::cube(2.0);
        let mut m = Modifier::array(2);
        let Modifier::Array { fit, .. } = &mut m else { unreachable!() };
        *fit = 7.0;
        /* 2 m cubes with a relative offset of 1: floor(7 / 2) = 3 copies. */
        assert_eq!(m.apply(&cube).faces.len(), 18);
        assert_eq!(m.summary(), "Array fit 7 m");
        assert!(m.adjust(1) && m.summary() == "Array fit 7.5 m");
        assert!(m.adjust(-100) && m.summary() == "Array fit 0.1 m");
        /* Shorter than one step: one copy, never zero. */
        assert_eq!(m.apply(&cube).faces.len(), 6);
        /* No offset length (zero offset): the fixed count is used. */
        let mut z = Modifier::Array { count: 4, relative: [0.0; 3], constant: [0.0; 3], merge: 0.0, fit: 5.0 };
        assert_eq!(z.apply(&cube).faces.len(), 24);
        assert!(!z.adjust(0));
    }

    #[test]
    fn the_catalog_has_every_kind_once() {
        let c = Modifier::catalog();
        assert_eq!(c.len(), 22);
        let mut names: Vec<&str> = c.iter().map(|m| m.label()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 22);
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
        let m = Modifier::Array { count: 2, relative: [0.0; 3], constant: [0.0, 1.0, 0.0], merge: 0.001, fit: 0.0 };
        assert_eq!(m.summary(), "Array x2 +1 m Y merge 0.001 m");
        let m = Modifier::Array { count: 2, relative: [0.5, 0.0, 0.0], constant: [0.0; 3], merge: 0.0, fit: 0.0 };
        assert_eq!(m.summary(), "Array x2 custom offset");
    }

    #[test]
    fn array_merge_joins_touching_copies() {
        /* Two unit cubes side by side along X share a face: 16 vertices become 12 once merged. */
        let cube = Mesh::cube(1.0);
        let plain = Modifier::Array { count: 2, relative: [1.0, 0.0, 0.0], constant: [0.0; 3], merge: 0.0, fit: 0.0 };
        let merged = Modifier::Array { count: 2, relative: [1.0, 0.0, 0.0], constant: [0.0; 3], merge: 0.001, fit: 0.0 };
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
