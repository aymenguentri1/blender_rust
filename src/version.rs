//! Port of `source/blender/blenkernel/BKE_blender_version.h` (values)
//! and `blenkernel/intern/blender.cc` `blender_version_init()` (string format).

pub const BLENDER_VERSION: i32 = 504;
pub const BLENDER_VERSION_PATCH: i32 = 0;
/// BLENDER_VERSION_CYCLE is `alpha` in this source tree -> " Alpha".
pub const VERSION_CYCLE: &str = " Alpha";
/// BKE_blender_version_is_lts() is false for this tree (no " LTS" suffix).
pub const VERSION_SUFFIX: &str = "";

/// `version_cycle_compact` of `blender_version_init()`: " a" for alpha.
pub const VERSION_CYCLE_COMPACT: &str = " a";

/// `BKE_blender_version_string_compact()`: "%d.%01d.%d%s" with the compact cycle, as shown in the
/// status bar by `ED_info_statusbar_string_ex()` (`STATUSBAR_SHOW_VERSION`).
pub fn version_string_compact() -> String {
    format!(
        "{}.{}.{}{}",
        BLENDER_VERSION / 100,
        BLENDER_VERSION % 100,
        BLENDER_VERSION_PATCH,
        VERSION_CYCLE_COMPACT
    )
}

/// `BKE_blender_version_string()`: "%d.%01d.%d%s%s"
pub fn version_string() -> String {
    format!(
        "{}.{}.{}{}{}",
        BLENDER_VERSION / 100,
        BLENDER_VERSION % 100,
        BLENDER_VERSION_PATCH,
        VERSION_SUFFIX,
        VERSION_CYCLE
    )
}
