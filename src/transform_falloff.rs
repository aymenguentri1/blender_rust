//! Port of the proportional-editing falloff in
//! `source/blender/editors/transform/transform_generics.cc` (`calculatePropRatio`):
//! the `td->factor` computation and the `proptext` labels.

/// `PROP_*` falloff types (`DNA_scene_types.h`), the ones handled by `calculatePropRatio`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropMode {
    Smooth,
    Sphere,
    Root,
    InvSquare,
    Sharp,
    Lin,
    Const,
    Random,
}

impl PropMode {
    /// `pet_id` shown in the header text (`t->proptext`).
    pub fn label(self) -> &'static str {
        match self {
            PropMode::Sharp => "(Sharp)",
            PropMode::Smooth => "(Smooth)",
            PropMode::Root => "(Root)",
            PropMode::Lin => "(Linear)",
            PropMode::Const => "(Constant)",
            PropMode::Sphere => "(Sphere)",
            PropMode::Random => "(Random)",
            PropMode::InvSquare => "(InvSquare)",
        }
    }
}

/// The `switch (t->prop_mode)` on the normalized distance `dist` (already `(size - d) / size`,
/// clamped to >= 0). `rand` is the `BLI_rng_get_float()` sample used by `PROP_RANDOM` only.
pub fn falloff(mode: PropMode, dist: f32, rand: f32) -> f32 {
    match mode {
        PropMode::Sharp => dist * dist,
        // `std::min(1.0f, ...)`: float error near dist == 1 can exceed 1 (#147530).
        PropMode::Smooth => (3.0 * dist * dist - 2.0 * dist * dist * dist).min(1.0),
        PropMode::Root => dist.sqrt(),
        PropMode::Lin => dist,
        PropMode::Const => 1.0,
        PropMode::Sphere => (2.0 * dist - dist * dist).sqrt(),
        PropMode::Random => rand * dist,
        PropMode::InvSquare => dist * (2.0 - dist),
    }
}

/// `td->factor` of one element: selected elements get 1, elements beyond `prop_size` get 0
/// (`d` is `td->dist` when connected, `td->rdist` otherwise), the rest use [`falloff`].
pub fn prop_factor(mode: PropMode, selected: bool, d: f32, prop_size: f32, rand: f32) -> f32 {
    if selected {
        return 1.0;
    }
    if d > prop_size {
        return 0.0;
    }
    let dist = ((prop_size - d) / prop_size).max(0.0);
    falloff(mode, dist, rand)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [PropMode; 8] = [
        PropMode::Smooth, PropMode::Sphere, PropMode::Root, PropMode::InvSquare,
        PropMode::Sharp, PropMode::Lin, PropMode::Const, PropMode::Random,
    ];

    #[test]
    fn formulas_at_half() {
        assert_eq!(falloff(PropMode::Sharp, 0.5, 0.0), 0.25);
        assert_eq!(falloff(PropMode::Smooth, 0.5, 0.0), 0.5);
        assert!((falloff(PropMode::Root, 0.25, 0.0) - 0.5).abs() < 1e-6);
        assert_eq!(falloff(PropMode::Lin, 0.5, 0.0), 0.5);
        assert_eq!(falloff(PropMode::Const, 0.5, 0.0), 1.0);
        assert!((falloff(PropMode::Sphere, 0.5, 0.0) - 0.75f32.sqrt()).abs() < 1e-6);
        assert_eq!(falloff(PropMode::InvSquare, 0.5, 0.0), 0.75);
        assert_eq!(falloff(PropMode::Random, 0.5, 0.5), 0.25);
    }

    #[test]
    fn endpoints() {
        for m in ALL {
            if m != PropMode::Const {
                assert_eq!(falloff(m, 0.0, 0.7), 0.0, "{m:?}");
            }
            if m != PropMode::Random {
                assert!((falloff(m, 1.0, 0.7) - 1.0).abs() < 1e-6, "{m:?}");
            }
        }
    }

    #[test]
    fn smooth_never_exceeds_one() {
        let d = 1.0 - f32::EPSILON;
        assert!(falloff(PropMode::Smooth, d, 0.0) <= 1.0);
        assert!(falloff(PropMode::Smooth, 1.0000001, 0.0) <= 1.0);
    }

    #[test]
    fn prop_factor_selected_outside_and_clamp() {
        assert_eq!(prop_factor(PropMode::Lin, true, 99.0, 2.0, 0.0), 1.0);
        assert_eq!(prop_factor(PropMode::Lin, false, 2.5, 2.0, 0.0), 0.0);
        assert_eq!(prop_factor(PropMode::Lin, false, 0.5, 2.0, 0.0), 0.75);
        assert_eq!(prop_factor(PropMode::Lin, false, 2.0, 2.0, 0.0), 0.0);
        // negative distance would give dist > 1 only if d < 0; positive overshoot is clamped to 0
        assert_eq!(prop_factor(PropMode::Sharp, false, 2.0, 2.0, 0.0), 0.0);
    }

    #[test]
    fn labels() {
        assert_eq!(PropMode::InvSquare.label(), "(InvSquare)");
        assert_eq!(PropMode::Lin.label(), "(Linear)");
    }
}
