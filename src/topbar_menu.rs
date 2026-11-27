//! Item model and contents of the top bar menus, ported from `TOPBAR_MT_file`, `TOPBAR_MT_edit`,
//! `TOPBAR_MT_render`, `TOPBAR_MT_window` and `TOPBAR_MT_help` in `space_topbar.py`.
//! Labels of operators without `text=` are the `ot->name` of the C operator.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `layout.operator(...)`.
    Op(&'static str),
    /// `layout.menu(...)`: opens a sub menu.
    Menu(&'static str),
    /// `layout.prop(...)`: a checkbox row.
    Prop(&'static str),
    /// `layout.separator()`.
    Separator,
}

/// The parts of `context` the menus branch on.
#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
pub struct Context {
    pub is_saved: bool,
    pub show_developer_ui: bool,
    pub windows: bool,
}

use Entry::{Menu, Op, Prop, Separator as Sep};

pub fn file(_ctx: Context) -> Vec<Entry> {
    vec![
        Menu("New"), Op("Open..."), Menu("Open Recent"), Op("Revert"), Menu("Recover"), Sep,
        Op("Save"), Op("Save As..."), Op("Save Copy..."), Op("Save Incremental"), Sep,
        Op("Link..."), Op("Append..."), Menu("Data Previews"), Sep,
        Menu("Project"), Sep,
        Menu("Import"), Menu("Export"), Sep,
        Menu("External Data"), Menu("Clean Up"), Sep,
        Menu("Defaults"), Sep,
        Op("Quit"),
    ]
}

pub fn edit(ctx: Context) -> Vec<Entry> {
    let mut v = vec![Op("Undo"), Op("Redo"), Menu("Undo History"), Sep];
    v.extend([Op("Adjust Last Operation..."), Op("Repeat Last"), Op("Repeat History..."), Sep]);
    v.push(Op("Menu Search..."));
    if ctx.show_developer_ui {
        v.push(Op("Operator Search..."));
    }
    v.extend([Sep, Op("Rename Active Item..."), Op("Batch Rename..."), Sep, Prop("Lock Object Modes"), Sep]);
    v.push(Op("Preferences..."));
    v
}

pub fn render(_ctx: Context) -> Vec<Entry> {
    vec![
        Op("Render Image"), Op("Render Animation"), Sep,
        Op("Render Audio..."), Sep,
        Op("View Render"), Op("View Animation"), Sep,
        Prop("Lock Interface"),
    ]
}

pub fn window(ctx: Context) -> Vec<Entry> {
    let mut v = vec![
        Op("New Window"), Op("New Main Window"), Sep,
        Op("Toggle Window Fullscreen"), Sep,
        Op("Next Workspace"), Op("Previous Workspace"), Sep,
        Prop("Status Bar"), Sep,
        Op("Save Screenshot..."), Op("Save Screenshot (Editor)..."),
    ];
    if ctx.windows {
        v.extend([Sep, Op("Toggle System Console")]);
    }
    v
}

pub fn help(ctx: Context) -> Vec<Entry> {
    let mut v = vec![Op("Manual"), Op("Support"), Op("User Communities"), Op("Get Involved"), Op("Release Notes"), Sep];
    if ctx.show_developer_ui {
        v.extend([Op("Developer Documentation"), Op("Developer Community"), Op("Python API Reference"), Op("Operator Cheat Sheet"), Sep]);
    }
    v.extend([Op("Report a Bug"), Op("Save System Info")]);
    v
}

/// The entries of the menu named by its header label.
pub fn entries(label: &str, ctx: Context) -> Option<Vec<Entry>> {
    Some(match label {
        "File" => file(ctx),
        "Edit" => edit(ctx),
        "Render" => render(ctx),
        "Window" => window(ctx),
        "Help" => help(ctx),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(v: &[Entry]) -> Vec<&'static str> {
        v.iter().filter_map(|e| match e { Op(l) | Menu(l) | Prop(l) => Some(*l), Sep => None }).collect()
    }

    #[test]
    fn file_menu_order() {
        let l = labels(&file(Context::default()));
        assert_eq!(&l[..5], ["New", "Open...", "Open Recent", "Revert", "Recover"]);
        assert_eq!(l.last(), Some(&"Quit"));
        assert_eq!(l.len(), 19);
    }

    #[test]
    fn no_double_or_edge_separators() {
        for m in ["File", "Edit", "Render", "Window", "Help"] {
            for ctx in [Context::default(), Context { is_saved: true, show_developer_ui: true, windows: true }] {
                let v = entries(m, ctx).unwrap();
                assert_ne!(v.first(), Some(&Sep), "{m}");
                assert_ne!(v.last(), Some(&Sep), "{m}");
                assert!(v.windows(2).all(|w| w != [Sep, Sep]), "{m}");
            }
        }
    }

    #[test]
    fn window_console_only_on_windows() {
        assert!(!labels(&window(Context::default())).contains(&"Toggle System Console"));
        assert!(labels(&window(Context { windows: true, ..Default::default() })).contains(&"Toggle System Console"));
    }

    #[test]
    fn developer_entries() {
        assert!(!labels(&help(Context::default())).contains(&"Developer Community"));
        let h = help(Context { show_developer_ui: true, ..Default::default() });
        assert!(labels(&h).contains(&"Developer Community"));
    }
}
