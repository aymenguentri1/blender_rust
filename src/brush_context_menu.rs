//! `VIEW3D_MT_brush_context_menu` ("Brush Specials", scripts/startup/bl_ui/space_view3d_toolbar.py) as pure rows.
//! Icons are Blender icon identifiers. Pure data; the menu draws it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// `layout.label(text=.., icon=..)`.
    Label { text: &'static str, icon: &'static str },
    /// `layout.operator(idname, text=.., icon=..)`.
    Operator { idname: &'static str, text: &'static str, icon: Option<&'static str> },
    Separator,
}

fn op(idname: &'static str, text: &'static str, icon: Option<&'static str>) -> Row {
    Row::Operator { idname, text, icon }
}

/// `has_brush` is `brush is not None`; `library_editable` is `brush.library and brush.library.is_editable`.
pub fn layout(has_brush: bool, library_editable: bool) -> Vec<Row> {
    if !has_brush {
        return vec![Row::Label { text: "No brush selected", icon: "STATUS_INFO" }];
    }
    let mut r = if library_editable {
        vec![
            op("brush.asset_save_as", "Duplicate Asset...", Some("DUPLICATE")),
            op("brush.asset_delete", "Delete Asset", None),
        ]
    } else {
        vec![
            op("brush.asset_save_as", "Save As Asset...", Some("FILE_TICK")),
            op("brush.asset_delete", "Delete", None),
        ]
    };
    r.push(Row::Separator);
    r.push(op("brush.asset_edit_metadata", "Edit Metadata...", None));
    r.push(op("brush.asset_load_preview", "Edit Preview Image...", None));
    r.push(op("brush.asset_save", "Save Changes to Asset", None));
    r.push(op("brush.asset_revert", "Revert to Asset", None));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_brush_is_one_label() {
        assert_eq!(layout(false, true), vec![Row::Label { text: "No brush selected", icon: "STATUS_INFO" }]);
    }

    #[test]
    fn editable_library_duplicates() {
        let r = layout(true, true);
        assert_eq!(r.len(), 7);
        assert_eq!(r[0], op("brush.asset_save_as", "Duplicate Asset...", Some("DUPLICATE")));
        assert_eq!(r[1], op("brush.asset_delete", "Delete Asset", None));
        assert_eq!(r[2], Row::Separator);
    }

    #[test]
    fn non_editable_saves_as() {
        let r = layout(true, false);
        assert_eq!(r[0], op("brush.asset_save_as", "Save As Asset...", Some("FILE_TICK")));
        assert_eq!(r[1], op("brush.asset_delete", "Delete", None));
        let tail: Vec<_> = r[3..].iter().map(|x| if let Row::Operator { idname, .. } = x { *idname } else { "" }).collect();
        assert_eq!(tail, ["brush.asset_edit_metadata", "brush.asset_load_preview", "brush.asset_save", "brush.asset_revert"]);
    }
}
