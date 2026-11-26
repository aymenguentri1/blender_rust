//! Keymap data model and event matching.
//!
//! Items mirror the `(idname, {type, value, shift, ctrl, alt, ...}, {"properties": [...]})` tuples of
//! `blender_default.py` (including `op_menu`, which is `wm.call_menu` with a `name` property). Matching is
//! `wm_eventmatch` (`wm_event_system.cc`) and the first-match loop of `WM_event_match_keymap_item` (the operator poll
//! is the caller's concern, see [`Keymap::match_item_with`]):
//!  - a repeat event only matches items with `"repeat": True` (the default is `KMI_REPEAT_IGNORE`);
//!  - type and value must be equal unless the item uses `ANY` (`value: 'ANY'`);
//!  - every modifier is `Off`, `Held` or `Any` (`"any": True` sets all of them to `Any`); a modifier mismatch is
//!    forgiven when the event itself is that modifier key (e.g. pressing LEFT_SHIFT with a shift item);
//!  - an item with a `key_modifier` only matches while that key is held (`winevent->keymodifier`).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mod {
    Off,
    Held,
    Any,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

/// `wmKeyMapItem`.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyMapItem {
    pub idname: &'static str,
    pub kind: &'static str,
    pub value: &'static str,
    pub shift: Mod,
    pub ctrl: Mod,
    pub alt: Mod,
    pub oskey: Mod,
    pub repeat: bool,
    pub key_modifier: Option<&'static str>,
    pub properties: Vec<(&'static str, Prop)>,
}

impl KeyMapItem {
    /// `("idname", {"type": kind, "value": value}, None)`.
    pub fn new(idname: &'static str, kind: &'static str, value: &'static str) -> Self {
        KeyMapItem { idname, kind, value, shift: Mod::Off, ctrl: Mod::Off, alt: Mod::Off, oskey: Mod::Off, repeat: false, key_modifier: None, properties: Vec::new() }
    }
    pub fn shift(mut self) -> Self {
        self.shift = Mod::Held;
        self
    }
    pub fn ctrl(mut self) -> Self {
        self.ctrl = Mod::Held;
        self
    }
    pub fn alt(mut self) -> Self {
        self.alt = Mod::Held;
        self
    }
    pub fn oskey(mut self) -> Self {
        self.oskey = Mod::Held;
        self
    }
    /// `"any": True`.
    pub fn any_modifier(mut self) -> Self {
        (self.shift, self.ctrl, self.alt, self.oskey) = (Mod::Any, Mod::Any, Mod::Any, Mod::Any);
        self
    }
    /// `"repeat": True`.
    pub fn repeat(mut self) -> Self {
        self.repeat = true;
        self
    }
    pub fn key_modifier(mut self, key: &'static str) -> Self {
        self.key_modifier = Some(key);
        self
    }
    /// `{"properties": [(name, value)]}`.
    pub fn prop(mut self, name: &'static str, value: Prop) -> Self {
        self.properties.push((name, value));
        self
    }
    /// `op_menu(menu, kmi_args)`.
    pub fn op_menu(menu: &'static str, kind: &'static str, value: &'static str) -> Self {
        Self::new("wm.call_menu", kind, value).prop("name", Prop::Str(menu))
    }

    pub fn property(&self, name: &str) -> Option<&Prop> {
        self.properties.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }
}

/// `wmEvent` fields used by the matching.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Event {
    pub kind: &'static str,
    pub value: &'static str,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub oskey: bool,
    pub is_repeat: bool,
    pub key_modifier: Option<&'static str>,
}

impl Event {
    pub fn new(kind: &'static str, value: &'static str) -> Self {
        Event { kind, value, ..Default::default() }
    }
}

fn mod_ok(want: Mod, held: bool, event_kind: &str, own_keys: &[&str]) -> bool {
    match want {
        Mod::Any => true,
        Mod::Held => held || own_keys.contains(&event_kind),
        Mod::Off => !held || own_keys.contains(&event_kind),
    }
}

/// `wm_eventmatch`.
pub fn event_match(event: &Event, kmi: &KeyMapItem) -> bool {
    if event.is_repeat && !kmi.repeat {
        return false;
    }
    if kmi.kind != "ANY" && event.kind != kmi.kind {
        return false;
    }
    if kmi.value != "ANY" && event.value != kmi.value {
        return false;
    }
    if !mod_ok(kmi.shift, event.shift, event.kind, &["LEFT_SHIFT", "RIGHT_SHIFT"])
        || !mod_ok(kmi.ctrl, event.ctrl, event.kind, &["LEFT_CTRL", "RIGHT_CTRL"])
        || !mod_ok(kmi.alt, event.alt, event.kind, &["LEFT_ALT", "RIGHT_ALT"])
        || !mod_ok(kmi.oskey, event.oskey, event.kind, &["OSKEY"])
    {
        return false;
    }
    match kmi.key_modifier {
        Some(k) => event.key_modifier == Some(k),
        None => true,
    }
}

/// `wmKeyMap`: a name and its ordered items.
#[derive(Clone, Debug, PartialEq)]
pub struct Keymap {
    pub name: &'static str,
    pub items: Vec<KeyMapItem>,
}

impl Keymap {
    pub fn new(name: &'static str, items: Vec<KeyMapItem>) -> Self {
        Keymap { name, items }
    }

    /// `WM_event_match_keymap_item`: the first item that matches the event and passes `poll` (the operator poll).
    pub fn match_item_with(&self, event: &Event, poll: impl Fn(&KeyMapItem) -> bool) -> Option<&KeyMapItem> {
        self.items.iter().find(|kmi| event_match(event, kmi) && poll(kmi))
    }

    pub fn match_item(&self, event: &Event) -> Option<&KeyMapItem> {
        self.match_item_with(event, |_| true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_matching_item_wins_and_poll_skips() {
        let km = Keymap::new("T", vec![KeyMapItem::new("a.first", "A", "PRESS"), KeyMapItem::new("b.second", "A", "PRESS")]);
        let e = Event::new("A", "PRESS");
        assert_eq!(km.match_item(&e).unwrap().idname, "a.first");
        assert_eq!(km.match_item_with(&e, |k| k.idname != "a.first").unwrap().idname, "b.second");
        assert!(km.match_item(&Event::new("B", "PRESS")).is_none());
    }

    #[test]
    fn modifiers_must_match_exactly() {
        let km = Keymap::new("T", vec![KeyMapItem::new("save", "S", "PRESS").ctrl(), KeyMapItem::new("plain", "S", "PRESS")]);
        let mut e = Event::new("S", "PRESS");
        assert_eq!(km.match_item(&e).unwrap().idname, "plain");
        e.ctrl = true;
        assert_eq!(km.match_item(&e).unwrap().idname, "save");
        e.shift = true; // ctrl + shift: neither item has shift
        assert!(km.match_item(&e).is_none());
    }

    #[test]
    fn value_press_release_and_any() {
        let item = KeyMapItem::new("x", "A", "PRESS");
        assert!(event_match(&Event::new("A", "PRESS"), &item));
        assert!(!event_match(&Event::new("A", "RELEASE"), &item));
        let any = KeyMapItem::new("t", "TIMER0", "ANY").any_modifier();
        let mut e = Event::new("TIMER0", "PRESS");
        e.shift = true;
        e.alt = true;
        assert!(event_match(&e, &any));
    }

    #[test]
    fn repeat_only_when_allowed() {
        let (plain, rep) = (KeyMapItem::new("p", "LEFT_ARROW", "PRESS"), KeyMapItem::new("r", "LEFT_ARROW", "PRESS").repeat());
        let mut e = Event::new("LEFT_ARROW", "PRESS");
        e.is_repeat = true;
        assert!(!event_match(&e, &plain) && event_match(&e, &rep));
        e.is_repeat = false;
        assert!(event_match(&e, &plain) && event_match(&e, &rep));
    }

    #[test]
    fn modifier_key_as_event_type_and_key_modifier() {
        // pressing LEFT_SHIFT itself matches a shift item even though the shift flag state differs
        let item = KeyMapItem::new("s", "LEFT_SHIFT", "PRESS").shift();
        assert!(event_match(&Event::new("LEFT_SHIFT", "PRESS"), &item));
        let km_item = KeyMapItem::new("d", "LEFTMOUSE", "PRESS").key_modifier("D");
        let mut e = Event::new("LEFTMOUSE", "PRESS");
        assert!(!event_match(&e, &km_item));
        e.key_modifier = Some("D");
        assert!(event_match(&e, &km_item));
    }

    #[test]
    fn properties_and_op_menu() {
        let m = KeyMapItem::op_menu("TOPBAR_MT_file_new", "N", "PRESS");
        assert_eq!((m.idname, m.property("name")), ("wm.call_menu", Some(&Prop::Str("TOPBAR_MT_file_new"))));
        let f = KeyMapItem::new("screen.frame_offset", "LEFT_ARROW", "PRESS").repeat().prop("delta", Prop::Int(-1));
        assert_eq!(f.property("delta"), Some(&Prop::Int(-1)));
        assert_eq!(f.property("missing"), None);
    }
}
