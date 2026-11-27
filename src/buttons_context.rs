//! The tab strip of the properties editor: `ED_buttons_tabs_list()` (space_buttons.cc:211) over the paths of
//! `buttons_context_path()` / `buttons_context_compute()` (buttons_context.cc:632, :802), `eSpaceButtons_Context`
//! (DNA_space_enums.h:95). Not modelled yet (their paths need data this port lacks): Particles, Bone, Bone
//! Constraints, Texture, Compositor, Strip, Strip Modifier, Shader Effects.

use crate::icons::Icon;
use crate::scene::{ObjectData, Scene};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Render = 0,
    Scene = 1,
    World = 2,
    Object = 3,
    Data = 4,
    Material = 5,
    Physics = 8,
    Modifier = 10,
    Constraint = 11,
    ViewLayer = 13,
    Tool = 14,
    Output = 16,
    Collection = 17,
}

/// One entry of the strip: a tab or `BCONTEXT_SEPARATOR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Tab(Context),
    Separator,
}

impl Context {
    const fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// `sbuts->pathflag`: the contexts for which a path exists. Scene-rooted contexts always do; the object ones need
/// an active object (`buttons_context_path_object`); Modifiers need a type with modifiers (`buttons_context_path_modifier`,
/// empties and meshes here); Material needs a type with material support (meshes here); Data needs object data.
pub fn path_flags(scene: &Scene) -> u32 {
    use Context::*;
    let mut f = Tool.bit() | Render.bit() | Output.bit() | Scene.bit() | ViewLayer.bit() | World.bit();
    /* `buttons_context_path_collection`: the active layer collection, unless it is the master collection. Here it is
     * always `Collection`. */
    f |= Collection.bit();
    if let Some(o) = scene.active_object() {
        f |= Object.bit() | Physics.bit() | Constraint.bit() | Data.bit();
        if matches!(o.data, ObjectData::Mesh(_) | ObjectData::Empty(_)) {
            f |= Modifier.bit();
        }
        if matches!(o.data, ObjectData::Mesh(_)) {
            f |= Material.bit();
        }
    }
    f
}

/// `ED_buttons_tabs_list(sbuts, apply_filter = false)`: the tab order with a separator between groups.
pub fn tabs_list(pathflag: u32) -> Vec<Tab> {
    use Context::*;
    let mut tabs: Vec<Tab> = Vec::new();
    let spacer = |t: &mut Vec<Tab>| {
        if !t.is_empty() && t.last() != Some(&Tab::Separator) {
            t.push(Tab::Separator);
        }
    };
    let add = |t: &mut Vec<Tab>, c: Context| {
        if pathflag & c.bit() != 0 {
            t.push(Tab::Tab(c));
        }
    };
    add(&mut tabs, Tool);
    spacer(&mut tabs);
    for c in [Render, Output, Scene, ViewLayer, World] {
        add(&mut tabs, c);
    }
    spacer(&mut tabs);
    add(&mut tabs, Collection);
    spacer(&mut tabs);
    for c in [Object, Modifier, Physics, Constraint, Data, Material] {
        add(&mut tabs, c);
    }
    /* `add_spacer()` after Material and Texture: the unmodelled group adds nothing, the dedupe keeps one. */
    spacer(&mut tabs);
    tabs
}

/// The icon of a tab: `buttons_context_items` (rna_space.cc:603). The Data tab takes `sbuts->dataicon`
/// (buttons_context.cc:826): lights `ICON_OUTLINER_DATA_LIGHT`, otherwise the icon of the data type's RNA struct
/// (`ICON_MESH_DATA`, `ICON_CAMERA_DATA`), an empty `ICON_EMPTY_DATA`.
pub fn tab_icon(c: Context, active: Option<&ObjectData>) -> Icon {
    match c {
        Context::Tool => Icon::ToolSettings,
        Context::Collection => Icon::Group,
        Context::Render => Icon::TabScene,
        Context::Output => Icon::Output,
        Context::Scene => Icon::SceneData,
        Context::ViewLayer => Icon::RenderResult,
        Context::World => Icon::World,
        Context::Object => Icon::ObjectData,
        Context::Constraint => Icon::Constraint,
        Context::Modifier => Icon::ModWrench,
        Context::Physics => Icon::Physics,
        Context::Material => Icon::ShadingMaterial,
        Context::Data => match active {
            Some(ObjectData::Mesh(_)) => Icon::MeshData,
            Some(ObjectData::Camera) => Icon::CameraData,
            Some(ObjectData::Light(_)) => Icon::LightData,
            _ => Icon::EmptyData,
        },
    }
}

/// The tab shown, given the one the user picked (`mainbuser`): it stays while its path exists; otherwise Object,
/// else the first available context. As in `buttons_context_compute`, the test is `flag & BCONTEXT_OBJECT`, a
/// bitmask with the value 3 (Render or Scene), which is true whenever those exist, so Object is chosen even
/// without an active object; the final check then falls back to Object, else Scene.
pub fn effective_tab(pathflag: u32, user: Context) -> Context {
    use Context::*;
    let mut mainb = user;
    if pathflag & mainb.bit() == 0 {
        mainb = if pathflag & 3 != 0 {
            Object
        } else {
            [Render, Scene, World, Object, Data, Material, Physics, Modifier, Constraint, ViewLayer, Tool, Output, Collection]
                .into_iter()
                .find(|c| pathflag & c.bit() != 0)
                .unwrap_or(user)
        };
    }
    if pathflag & mainb.bit() == 0 {
        mainb = if pathflag & Object.bit() != 0 { Object } else { Scene };
    }
    mainb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(t: &[Tab]) -> Vec<i32> {
        t.iter().map(|x| match x { Tab::Tab(c) => *c as i32, Tab::Separator => -1 }).collect()
    }

    #[test]
    fn startup_cube_has_the_default_strip() {
        let s = Scene::startup();
        let f = path_flags(&s);
        /* Tool | Render Output Scene ViewLayer World | Collection | Object Modifier Physics Constraint Data Material */
        assert_eq!(names(&tabs_list(f)), vec![14, -1, 0, 16, 1, 13, 2, -1, 17, -1, 3, 10, 8, 11, 4, 5, -1]);
    }

    #[test]
    fn no_active_object_drops_the_object_group() {
        let mut s = Scene::startup();
        s.active = None;
        let t = tabs_list(path_flags(&s));
        /* The spacers after the scene and object groups run even when nothing follows, so one trailing separator stays (after Collection here). */
        assert_eq!(names(&t), vec![14, -1, 0, 16, 1, 13, 2, -1, 17, -1]);
    }

    #[test]
    fn user_tab_stays_while_valid_else_object() {
        let s = Scene::startup();
        let f = path_flags(&s);
        assert_eq!(effective_tab(f, Context::Material), Context::Material);
        let mut none = Scene::startup();
        none.active = None;
        let g = path_flags(&none);
        /* The first fallback picks Object (`flag & BCONTEXT_OBJECT` is a bitmask), the final check finds no path. */
        assert_eq!(effective_tab(g, Context::Material), Context::Scene);
        assert_eq!(effective_tab(g, Context::Scene), Context::Scene);
    }

    #[test]
    fn data_tab_icon_follows_the_object_data() {
        let s = Scene::startup();
        let o = s.active_object().map(|o| &o.data);
        assert_eq!(tab_icon(Context::Data, o), Icon::MeshData);
        assert_eq!(tab_icon(Context::Data, Some(&ObjectData::Camera)), Icon::CameraData);
        assert_eq!(tab_icon(Context::Data, None), Icon::EmptyData);
        assert_eq!(tab_icon(Context::ViewLayer, None), Icon::RenderResult);
    }

    #[test]
    fn nothing_available_falls_back_to_scene() {
        assert_eq!(effective_tab(0, Context::Object), Context::Scene);
    }
}
