//! Scene unit settings and the Units panel of the Scene tab (`SCENE_PT_unit`, properties_scene.py:64-90).
//!
//! Data: `UnitSettings` (DNA_scene_types.h:2511, defaults in `BKE_scene_init` scene.cc:222-228, RNA in
//! rna_scene.cc:5031 `rna_def_unit_settings`). The per-type unit lists (`rna_UnitSettings_*_unit_itemf`) need the unit
//! tables of unit.cc and are not ported: units are stored as indices into those tables, `USER_UNIT_ADAPTIVE` (0xFF)
//! meaning "adaptive".

/// `USER_UNIT_ADAPTIVE`.
pub const UNIT_ADAPTIVE: u8 = 0xFF;
/// `RNA_def_property_range` of `scale_length`.
pub const SCALE_MIN: f32 = 1e-9;
pub const SCALE_MAX: f32 = 1e9;

/// `eUnit_System` (rna `unit_systems`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitSystem {
    None,
    Metric,
    Imperial,
}

impl UnitSystem {
    pub const ALL: [UnitSystem; 3] = [UnitSystem::None, UnitSystem::Metric, UnitSystem::Imperial];

    /// The RNA item name shown in the dropdown.
    pub fn label(self) -> &'static str {
        match self {
            UnitSystem::None => "None",
            UnitSystem::Metric => "Metric",
            UnitSystem::Imperial => "Imperial",
        }
    }

    /// `BKE_unit_base_of_type_get(system, B_UNIT_LENGTH)` (unit.cc: metric `base_unit` 3, imperial 4).
    fn base_length(self) -> u8 {
        if self == UnitSystem::Imperial { 4 } else { 3 }
    }

    /// `BKE_unit_base_of_type_get(system, B_UNIT_MASS)` (metric 2, imperial 3).
    fn base_mass(self) -> u8 {
        if self == UnitSystem::Imperial { 3 } else { 2 }
    }

    /// `B_UNIT_TIME` is the natural collection in every system (`base_unit` 3).
    fn base_time(self) -> u8 {
        3
    }

    /// `B_UNIT_TEMPERATURE` (metric `base_unit` 0, imperial 1).
    fn base_temperature(self) -> u8 {
        if self == UnitSystem::Imperial { 1 } else { 0 }
    }
}

/// `rotation_units` (rna `system_rotation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationUnit {
    Degrees,
    Radians,
}

impl RotationUnit {
    pub const ALL: [RotationUnit; 2] = [RotationUnit::Degrees, RotationUnit::Radians];

    pub fn label(self) -> &'static str {
        match self {
            RotationUnit::Degrees => "Degrees",
            RotationUnit::Radians => "Radians",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnitSettings {
    pub system: UnitSystem,
    pub system_rotation: RotationUnit,
    pub scale_length: f32,
    /// `USER_UNIT_OPT_SPLIT`: "Separate Units", display units in pairs (e.g. 1m 0cm).
    pub use_separate: bool,
    pub length_unit: u8,
    pub mass_unit: u8,
    pub time_unit: u8,
    pub temperature_unit: u8,
}

impl Default for UnitSettings {
    /// `BKE_scene_init()`: metric, scale 1, the base unit of metric for each type.
    fn default() -> Self {
        let m = UnitSystem::Metric;
        UnitSettings {
            system: m,
            system_rotation: RotationUnit::Degrees,
            scale_length: 1.0,
            use_separate: false,
            length_unit: m.base_length(),
            mass_unit: m.base_mass(),
            time_unit: m.base_time(),
            temperature_unit: m.base_temperature(),
        }
    }
}

impl UnitSettings {
    /// Setting `scale_length` clamps to the RNA range.
    pub fn set_scale_length(&mut self, v: f32) {
        self.scale_length = v.clamp(SCALE_MIN, SCALE_MAX);
    }

    /// Setting `system` then running `rna_UnitSettings_system_update()` (rna_scene.cc:3135): None makes length and mass
    /// adaptive, any other system resets them to its base units. Time and temperature are left alone.
    pub fn set_system(&mut self, system: UnitSystem) {
        self.system = system;
        if system == UnitSystem::None {
            self.length_unit = UNIT_ADAPTIVE;
            self.mass_unit = UNIT_ADAPTIVE;
        } else {
            self.length_unit = system.base_length();
            self.mass_unit = system.base_mass();
        }
    }
}

/// One row of the Units panel. `enabled` is `col.enabled` / `subcol.enabled` in `SCENE_PT_unit.draw()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: &'static str,
    pub enabled: bool,
}

/// The rows of `SCENE_PT_unit.draw()` in order: Unit System; Unit Scale and Separate Units (disabled with system
/// None); Rotation (always enabled); Length, Mass, Time and Temperature (disabled with system None).
pub fn panel_rows(unit: &UnitSettings) -> [Row; 8] {
    let on = unit.system != UnitSystem::None;
    [
        Row { label: "Unit System", enabled: true },
        Row { label: "Unit Scale", enabled: on },
        Row { label: "Separate Units", enabled: on },
        Row { label: "Rotation", enabled: true },
        Row { label: "Length", enabled: on },
        Row { label: "Mass", enabled: on },
        Row { label: "Time", enabled: on },
        Row { label: "Temperature", enabled: on },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_metric_base_units() {
        let u = UnitSettings::default();
        assert_eq!(u.system, UnitSystem::Metric);
        assert_eq!(u.scale_length, 1.0);
        assert!(!u.use_separate);
        assert_eq!((u.length_unit, u.mass_unit, u.time_unit, u.temperature_unit), (3, 2, 3, 0));
        assert_eq!(u.system_rotation, RotationUnit::Degrees);
    }

    #[test]
    fn scale_is_clamped_to_the_rna_range() {
        let mut u = UnitSettings::default();
        u.set_scale_length(0.0);
        assert_eq!(u.scale_length, 1e-9);
        u.set_scale_length(1e12);
        assert_eq!(u.scale_length, 1e9);
        u.set_scale_length(0.01);
        assert_eq!(u.scale_length, 0.01);
    }

    #[test]
    fn system_update_resets_length_and_mass() {
        let mut u = UnitSettings::default();
        u.set_system(UnitSystem::None);
        assert_eq!((u.length_unit, u.mass_unit), (UNIT_ADAPTIVE, UNIT_ADAPTIVE));
        assert_eq!((u.time_unit, u.temperature_unit), (3, 0));
        u.set_system(UnitSystem::Imperial);
        assert_eq!((u.length_unit, u.mass_unit), (4, 3));
        u.set_system(UnitSystem::Metric);
        assert_eq!((u.length_unit, u.mass_unit), (3, 2));
    }

    #[test]
    fn panel_rows_follow_the_python_layout() {
        let mut u = UnitSettings::default();
        let labels: Vec<_> = panel_rows(&u).iter().map(|r| r.label).collect();
        assert_eq!(labels, ["Unit System", "Unit Scale", "Separate Units", "Rotation", "Length", "Mass", "Time", "Temperature"]);
        assert!(panel_rows(&u).iter().all(|r| r.enabled));
        u.set_system(UnitSystem::None);
        let on: Vec<_> = panel_rows(&u).iter().map(|r| r.enabled).collect();
        assert_eq!(on, [true, false, false, true, false, false, false, false]);
    }

    #[test]
    fn enum_items_match_rna() {
        let s: Vec<_> = UnitSystem::ALL.iter().map(|x| x.label()).collect();
        assert_eq!(s, ["None", "Metric", "Imperial"]);
        let r: Vec<_> = RotationUnit::ALL.iter().map(|x| x.label()).collect();
        assert_eq!(r, ["Degrees", "Radians"]);
    }
}
