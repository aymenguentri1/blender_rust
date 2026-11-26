//! A file dropped on the window: which ones are `.blend` files and how they are loaded.
//! The extensions are those of `BKE_blendfile_extension_check()` (blendfile.cc).

use std::path::Path;

use crate::blend_scene::{load_scene, BlendSceneObject};

/// `BKE_blendfile_extension_check()`: case-insensitive extension match.
pub fn is_blend_path(path: &Path) -> bool {
    let Some(name) = path.file_name() else { return false };
    let name = name.to_string_lossy().to_ascii_lowercase();
    [".blend", ".ble", ".blend.gz"].iter().any(|e| name.ends_with(e))
}

/// Read and parse a dropped `.blend`; the error is the text shown to the user.
pub fn open(path: &Path) -> Result<Vec<BlendSceneObject>, String> {
    let fail = |e: &dyn std::fmt::Display| format!("Cannot read file \"{}\": {e}", path.display());
    let bytes = std::fs::read(path).map_err(|e| fail(&e))?;
    load_scene(&bytes).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension() {
        for p in ["a.blend", "/x/My File.BLEND", "b.Ble", "c.blend.gz"] {
            assert!(is_blend_path(Path::new(p)), "{p}");
        }
        for p in ["a.blend1", "a.obj", "blend", "a.blend/readme.txt", ""] {
            assert!(!is_blend_path(Path::new(p)), "{p}");
        }
    }

    #[test]
    fn missing_file_is_a_readable_error() {
        let e = open(Path::new("/nonexistent/x.blend")).unwrap_err();
        assert!(e.starts_with("Cannot read file \"/nonexistent/x.blend\": "), "{e}");
    }
}
