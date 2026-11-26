//! Bevel modifier settings and main-panel layout (`BevelModifierData`, DNA_modifier_types.h:506-620; RNA
//! `rna_def_modifier_bevel`, rna_modifier.cc:5112; `panel_draw` and `is_disabled`, MOD_bevel.cc:306-355).
//!
//! Audit: `Modifier::Bevel` (modifiers.rs) stores only width and segments. Differences found:
//! 1. Width is clamped to >= 0.001 in the Rust UI; the RNA range of `width` is 0..FLT_MAX (0 disables the modifier,
//!    `is_disabled`: `value == 0`). Segments range is 1..1000 (`segments` -> `res`).
//! 2. Blender has Affect (Vertices/Edges, default Edges), Width Type (Offset/Width/Depth/Percent/Absolute, default
//!    Offset), a Limit Method (None/Angle/Weight/Vertex Group) defaulting to Angle at 30 degrees; Rust bevels everything.
//! 3. Missing: profile (type, shape 0.5), miter, intersection type, clamp overlap, loop slide, material, face strength,
//!    harden normals. Only the main panel is modelled here; the Profile, Geometry and Shading subpanels follow.

pub const WIDTH_MAX: f32 = f32::MAX;
pub const SEGMENTS_RANGE: (i32, i32) = (1, 1000);
/// `angle_limit` range: 0..180 degrees, in radians.
pub const ANGLE_MAX: f32 = std::f32::consts::PI;

/// `BevelModifierAffectType` (`prop_affect_items`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affect {
    Vertices,
    Edges,
}

impl Affect {
    pub const ALL: [Affect; 2] = [Affect::Vertices, Affect::Edges];
    pub fn label(self) -> &'static str {
        match self {
            Affect::Vertices => "Vertices",
            Affect::Edges => "Edges",
        }
    }
}

/// `BevelModifierValFlag` (`prop_val_type_items`, RNA `offset_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidthType {
    Offset,
    Width,
    Depth,
    Percent,
    Absolute,
}

impl WidthType {
    pub const ALL: [WidthType; 5] =
        [WidthType::Offset, WidthType::Width, WidthType::Depth, WidthType::Percent, WidthType::Absolute];
    pub fn label(self) -> &'static str {
        match self {
            WidthType::Offset => "Offset",
            WidthType::Width => "Width",
            WidthType::Depth => "Depth",
            WidthType::Percent => "Percent",
            WidthType::Absolute => "Absolute",
        }
    }
}

/// `prop_limit_method_items` (`lim_flags`: 0, `MOD_BEVEL_ANGLE`, `MOD_BEVEL_WEIGHT`, `MOD_BEVEL_VGROUP`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitMethod {
    None,
    Angle,
    Weight,
    VertexGroup,
}

impl LimitMethod {
    pub const ALL: [LimitMethod; 4] = [LimitMethod::None, LimitMethod::Angle, LimitMethod::Weight, LimitMethod::VertexGroup];
    pub fn label(self) -> &'static str {
        match self {
            LimitMethod::None => "None",
            LimitMethod::Angle => "Angle",
            LimitMethod::Weight => "Weight",
            LimitMethod::VertexGroup => "Vertex Group",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BevelSettings {
    pub affect: Affect,
    pub width_type: WidthType,
    /// `value`: shared by `width` and `width_pct`.
    pub width: f32,
    /// `res`.
    pub segments: i32,
    pub limit_method: LimitMethod,
    /// `bevel_angle`, radians.
    pub angle_limit: f32,
}

impl Default for BevelSettings {
    /// In-class initialisers of `BevelModifierData`.
    fn default() -> Self {
        BevelSettings {
            affect: Affect::Edges,
            width_type: WidthType::Offset,
            width: 0.1,
            segments: 1,
            limit_method: LimitMethod::Angle,
            angle_limit: 30.0_f32.to_radians(),
        }
    }
}

impl BevelSettings {
    pub fn set_width(&mut self, v: f32) {
        self.width = v.clamp(0.0, WIDTH_MAX);
    }

    pub fn set_segments(&mut self, v: i32) {
        self.segments = v.clamp(SEGMENTS_RANGE.0, SEGMENTS_RANGE.1);
    }

    pub fn set_angle_limit(&mut self, v: f32) {
        self.angle_limit = v.clamp(0.0, ANGLE_MAX);
    }

    /// `is_disabled()`: a zero width turns the modifier off.
    pub fn is_disabled(&self) -> bool {
        self.width == 0.0
    }

    /// `edge_bevel` in `panel_draw`.
    pub fn edge_bevel(&self) -> bool {
        self.affect != Affect::Vertices
    }

    /// The rows of `panel_draw` in order, as (label, active).
    pub fn panel_rows(&self) -> Vec<(&'static str, bool)> {
        let mut rows = vec![
            ("Affect", true),
            ("Width Type", true),
            (if self.width_type == WidthType::Percent { "Width Percent" } else { "Amount" }, true),
            ("Segments", true),
            ("Limit Method", true),
        ];
        match self.limit_method {
            LimitMethod::Angle => rows.push(("Angle", self.edge_bevel())),
            LimitMethod::Weight => rows.push((if self.edge_bevel() { "Edge Weight" } else { "Vertex Weight" }, true)),
            LimitMethod::VertexGroup => rows.push(("Vertex Group", true)),
            LimitMethod::None => {}
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_dna() {
        let b = BevelSettings::default();
        assert_eq!((b.affect, b.width_type, b.width, b.segments), (Affect::Edges, WidthType::Offset, 0.1, 1));
        assert_eq!(b.limit_method, LimitMethod::Angle);
        assert!((b.angle_limit - 0.523_598_8).abs() < 1e-6);
        assert!(!b.is_disabled());
    }

    #[test]
    fn ranges_follow_rna() {
        let mut b = BevelSettings::default();
        b.set_width(-1.0);
        assert_eq!(b.width, 0.0);
        assert!(b.is_disabled());
        b.set_segments(0);
        assert_eq!(b.segments, 1);
        b.set_segments(5000);
        assert_eq!(b.segments, 1000);
        b.set_angle_limit(10.0);
        assert_eq!(b.angle_limit, ANGLE_MAX);
    }

    #[test]
    fn panel_rows_follow_panel_draw() {
        let mut b = BevelSettings::default();
        let labels = |b: &BevelSettings| b.panel_rows().iter().map(|r| r.0).collect::<Vec<_>>();
        assert_eq!(labels(&b), ["Affect", "Width Type", "Amount", "Segments", "Limit Method", "Angle"]);
        b.width_type = WidthType::Percent;
        assert_eq!(labels(&b)[2], "Width Percent");
        b.affect = Affect::Vertices;
        assert_eq!(b.panel_rows().last(), Some(&("Angle", false)));
        b.limit_method = LimitMethod::Weight;
        assert_eq!(b.panel_rows().last(), Some(&("Vertex Weight", true)));
        b.affect = Affect::Edges;
        assert_eq!(b.panel_rows().last(), Some(&("Edge Weight", true)));
        b.limit_method = LimitMethod::VertexGroup;
        assert_eq!(b.panel_rows().last(), Some(&("Vertex Group", true)));
        b.limit_method = LimitMethod::None;
        assert_eq!(b.panel_rows().len(), 5);
    }

    #[test]
    fn enum_items_match_rna() {
        let l = |v: Vec<&'static str>| v;
        assert_eq!(l(Affect::ALL.iter().map(|x| x.label()).collect()), ["Vertices", "Edges"]);
        assert_eq!(
            l(WidthType::ALL.iter().map(|x| x.label()).collect()),
            ["Offset", "Width", "Depth", "Percent", "Absolute"]
        );
        assert_eq!(
            l(LimitMethod::ALL.iter().map(|x| x.label()).collect()),
            ["None", "Angle", "Weight", "Vertex Group"]
        );
    }
}
