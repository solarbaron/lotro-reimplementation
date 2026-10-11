// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's built-in actions, contexts and default bindings
//! (`docs/specs/keymap.md` §8), plus the glue from action states to
//! controller intents.
//!
//! The layout is Tessera's own choice for a third-person world walker
//! (WASD-style movement with A/D turning, Q/E strafing, both mouse buttons
//! to run forward); it is not copied from any other program's key map.

use crate::action::{ActionDef, ActionId, ContextDef, ContextId, ModifierPolicy, Registry};
use crate::controller::MoveIntent;
use crate::device::AnalogAxis;
use crate::keymap::{BindingSpec, Keymap};
use crate::mapper::InputMapper;

/// Context names and priorities (`keymap.md` §8).
pub mod contexts {
    /// Always-on commands (menu, screenshot, mode toggles). Game band, 0.
    pub const GLOBAL: &str = "global";
    /// Camera look and zoom. Game band, 10.
    pub const CAMERA: &str = "camera";
    /// Walking. Game band, 20.
    pub const MOVEMENT: &str = "movement";
    /// Free flight; active instead of or above movement. Game band, 30.
    pub const FLY: &str = "fly";
    /// The pointer actions (clicks, taps, double clicks). Game band, 40,
    /// pass-through; activated by the client when it attaches a UI.
    pub const POINTER: &str = "pointer";
    /// Text entry (`EditControls`). UI band, priority 100 for `activate`;
    /// `opaque_keys`, receives text.
    pub const TEXT_ENTRY: &str = "text_entry";
    /// Dialog navigation (`DialogBoxes`). UI band.
    pub const UI_DIALOG: &str = "ui.dialog";
    /// Clipboard keys in text fields (`CopyAndPasteControls`). UI band.
    pub const UI_CLIPBOARD: &str = "ui.clipboard";
    /// Wheel over scrollables (`ScrollableControls_Mouse`). UI band.
    pub const UI_SCROLL_MOUSE: &str = "ui.scroll_mouse";
    /// Scrollable widgets (`ScrollableControls`). UI band; binds nothing.
    pub const UI_SCROLL: &str = "ui.scroll";
    /// Keyboard eater (`EatKeyboardInput`). UI band, `opaque_keys`; binds
    /// nothing.
    pub const UI_EAT_KEYBOARD: &str = "ui.eat_keyboard";
    /// The chat log's paging keys (Tessera context). UI band.
    pub const UI_CHAT_LOG: &str = "ui.chat_log";
}

/// The built-in actions, in registration order (§7.2.1 orders events of one
/// context by it).
fn action_defs() -> Vec<ActionDef> {
    let sub = ModifierPolicy::Subset;
    let b = ActionDef::button;
    let mut v = vec![
        // global
        b("menu"),
        b("screenshot"),
        b("toggle_fly"),
        b("toggle_debug"),
        b("open_chat"),
        // P1 panels (`character-panels.md` §2, §4.1; `item-tooltips.md` §10): Tessera
        // actions for the commands `ToggleJournalPanel`, `QUICKSLOT_BACKPACK`,
        // `QUICKSLOT_BACKPACK1` … `6` and `TOOLTIP_DETACH` (`keymap.md` §12.14.2 OPEN).
        b("toggle_character"),
        b("toggle_bags"),
        b("toggle_bag_1"),
        b("toggle_bag_2"),
        b("toggle_bag_3"),
        b("toggle_bag_4"),
        b("toggle_bag_5"),
        b("toggle_bag_6"),
        b("pin_tooltip"),
        // P4 social panels (`social-panels.md` §7.2, §9 TR12): Tessera actions for the
        // commands `ToggleSocialPanel`, the five page commands, `SelectFellowOne` … `Six`
        // and `AssistFellowTwo` … `Six`.
        b("toggle_social"),
        b("toggle_social_grouping"),
        b("toggle_social_friends"),
        b("toggle_social_kinship"),
        b("toggle_social_raid"),
        b("toggle_social_raid_lock"),
        b("select_fellow_1"),
        b("select_fellow_2"),
        b("select_fellow_3"),
        b("select_fellow_4"),
        b("select_fellow_5"),
        b("select_fellow_6"),
        b("assist_fellow_2"),
        b("assist_fellow_3"),
        b("assist_fellow_4"),
        b("assist_fellow_5"),
        b("assist_fellow_6"),
    ];
    // P2 targeting and quickslot actions (`keymap.md` §8.2): exact policy, no repeat.
    v.extend(crate::combat::TARGET_ACTIONS.iter().map(|r| b(r.1)));
    v.extend(crate::combat::QUICKSLOT_ACTIONS.iter().map(|n| b(n)));
    v.extend(vec![
        // camera
        b("mouselook").with_modifiers(sub),
        ActionDef::axis("look_x"),
        ActionDef::axis("look_y"),
        ActionDef::axis("zoom"),
        b("reset_camera"),
        // movement
        ActionDef::axis("move_forward"),
        ActionDef::axis("strafe"),
        ActionDef::axis("turn"),
        b("jump").with_modifiers(sub),
        b("walk").with_modifiers(sub),
        b("autorun").with_modifiers(sub),
        // fly
        ActionDef::axis("fly_vertical"),
        b("fly_boost").with_modifiers(sub),
        b("fly_slow").with_modifiers(sub),
        ActionDef::axis("fly_speed"),
        // pointer (decision 0014 D2)
        b("select_left").with_modifiers(sub),
        b("select_double_left").with_modifiers(sub),
        b("tap_left").with_modifiers(sub),
        b("select_right").with_modifiers(sub),
        b("select_double_right").with_modifiers(sub),
        b("select_middle").with_modifiers(sub),
        b("select_double_middle").with_modifiers(sub),
        b("tap_right").with_modifiers(sub),
        b("tap_middle").with_modifiers(sub),
        // ui.scroll_mouse
        b("scroll_up").with_modifiers(sub),
        b("scroll_down").with_modifiers(sub),
        // ui.dialog
        b("focus_next").with_repeat(true),
        b("focus_prev").with_repeat(true),
        b("focus_left").with_repeat(true),
        b("focus_right").with_repeat(true),
        b("focus_up").with_repeat(true),
        b("focus_down").with_repeat(true),
        // text_entry and ui.chat_log
        b("text_char_left").with_modifiers(sub).with_repeat(true),
        b("text_char_right").with_modifiers(sub).with_repeat(true),
        b("text_word_left").with_modifiers(sub).with_repeat(true),
        b("text_word_right").with_modifiers(sub).with_repeat(true),
        b("text_line_up").with_modifiers(sub).with_repeat(true),
        b("text_line_down").with_modifiers(sub).with_repeat(true),
        b("text_page_up").with_modifiers(sub).with_repeat(true),
        b("text_page_down").with_modifiers(sub).with_repeat(true),
        b("text_line_start").with_modifiers(sub),
        b("text_line_end").with_modifiers(sub),
        b("text_doc_start").with_modifiers(sub),
        b("text_doc_end").with_modifiers(sub),
        b("text_backspace").with_modifiers(sub).with_repeat(true),
        b("text_delete").with_modifiers(sub).with_repeat(true),
        // ui.clipboard
        b("text_copy"),
        b("text_cut"),
        b("text_paste"),
    ]);
    v
}

/// The built-in contexts (§8).
fn context_defs() -> Vec<ContextDef> {
    use contexts::{
        CAMERA, FLY, GLOBAL, MOVEMENT, POINTER, TEXT_ENTRY, UI_CHAT_LOG, UI_CLIPBOARD, UI_DIALOG,
        UI_EAT_KEYBOARD, UI_SCROLL, UI_SCROLL_MOUSE,
    };
    vec![
        ContextDef::new(GLOBAL, 0),
        ContextDef::new(CAMERA, 10),
        ContextDef::new(MOVEMENT, 20),
        ContextDef::new(FLY, 30),
        ContextDef::new(POINTER, 40).pass_through(),
        ContextDef::new(TEXT_ENTRY, 100)
            .ui()
            .with_opaque_keys()
            .with_text(),
        ContextDef::new(UI_DIALOG, 0).ui(),
        ContextDef::new(UI_CLIPBOARD, 0).ui(),
        ContextDef::new(UI_SCROLL_MOUSE, 0).ui(),
        ContextDef::new(UI_SCROLL, 0).ui(),
        ContextDef::new(UI_EAT_KEYBOARD, 0).ui().with_opaque_keys(),
        ContextDef::new(UI_CHAT_LOG, 0).ui(),
    ]
}

/// A registry with every built-in context and action.
#[must_use]
pub fn default_registry() -> Registry {
    let mut r = Registry::new();
    for c in context_defs() {
        // Built-in names are valid and unique; a failure would be a bug
        // caught by the tests, never by users.
        if let Err(e) = r.add_context(c) {
            tracing::error!(error = %e, "built-in context rejected");
        }
    }
    for a in action_defs() {
        if let Err(e) = r.add_action(a) {
            tracing::error!(error = %e, "built-in action rejected");
        }
    }
    r
}

/// Built-in default bindings, as `(context, action, bindings)`.
#[allow(clippy::type_complexity)]
fn default_table() -> Vec<(&'static str, &'static str, Vec<Entry>)> {
    use crate::chord::Activation::{DoubleNearby, Tap};
    use Entry::{A, D, O, V};
    use contexts::{
        CAMERA, FLY, GLOBAL, MOVEMENT, POINTER, TEXT_ENTRY, UI_CHAT_LOG, UI_CLIPBOARD, UI_DIALOG,
        UI_SCROLL_MOUSE,
    };
    let mut t = vec![
        (GLOBAL, "menu", vec![D("Escape")]),
        (GLOBAL, "screenshot", vec![D("PrintScreen"), D("F12")]),
        (GLOBAL, "toggle_fly", vec![D("Ctrl+F")]),
        (GLOBAL, "toggle_debug", vec![D("F3")]),
        (GLOBAL, "open_chat", vec![D("Enter"), D("NumpadEnter")]),
        // P1 panels: Tessera's own default keys (`keymap.md` §12.14.2 OPEN).
        (GLOBAL, "toggle_character", vec![D("C")]),
        (GLOBAL, "toggle_bags", vec![D("B")]),
        (GLOBAL, "toggle_bag_1", vec![D("I")]),
        (GLOBAL, "toggle_bag_2", vec![]),
        (GLOBAL, "toggle_bag_3", vec![]),
        (GLOBAL, "toggle_bag_4", vec![]),
        (GLOBAL, "toggle_bag_5", vec![]),
        (GLOBAL, "toggle_bag_6", vec![]),
        (GLOBAL, "pin_tooltip", vec![D("Ctrl+T")]),
        // P4 social panels: O opens the Social panel (the install's default, SP20). The
        // page commands are unbound there too. The install binds `SelectFellowOne` … `Six`
        // to F1 … F6 and `AssistFellowTwo` … `Six` to Shift+F2 … F6 (SP20); Tessera's own
        // map leaves them unbound because F3 is its debug key, and the key-map import
        // (`keymap.md` §12.14) brings the install's keys.
        (GLOBAL, "toggle_social", vec![D("O")]),
        (GLOBAL, "toggle_social_grouping", vec![]),
        (GLOBAL, "toggle_social_friends", vec![]),
        (GLOBAL, "toggle_social_kinship", vec![]),
        (GLOBAL, "toggle_social_raid", vec![]),
        (GLOBAL, "toggle_social_raid_lock", vec![]),
        (GLOBAL, "select_fellow_1", vec![]),
        (GLOBAL, "select_fellow_2", vec![]),
        (GLOBAL, "select_fellow_3", vec![]),
        (GLOBAL, "select_fellow_4", vec![]),
        (GLOBAL, "select_fellow_5", vec![]),
        (GLOBAL, "select_fellow_6", vec![]),
        (GLOBAL, "assist_fellow_2", vec![]),
        (GLOBAL, "assist_fellow_3", vec![]),
        (GLOBAL, "assist_fellow_4", vec![]),
        (GLOBAL, "assist_fellow_5", vec![]),
        (GLOBAL, "assist_fellow_6", vec![]),
        (CAMERA, "mouselook", vec![D("MouseRight")]),
        (
            CAMERA,
            "look_x",
            vec![A("MouseX", 1.0, 0.0), A("PadRightX", 1.0, 0.15)],
        ),
        (
            CAMERA,
            "look_y",
            vec![A("MouseY", 1.0, 0.0), A("PadRightY", -1.0, 0.15)],
        ),
        (
            CAMERA,
            "zoom",
            vec![A("WheelY", 1.0, 0.0), V("PageUp", 1.0), V("PageDown", -1.0)],
        ),
        (CAMERA, "reset_camera", vec![D("Home")]),
        (
            MOVEMENT,
            "move_forward",
            vec![
                D("W"),
                D("Up"),
                D("MouseLeft+MouseRight"),
                V("S", -1.0),
                V("Down", -1.0),
                A("PadLeftY", 1.0, 0.15),
            ],
        ),
        (
            MOVEMENT,
            "strafe",
            vec![V("E", 1.0), V("Q", -1.0), A("PadLeftX", 1.0, 0.15)],
        ),
        (
            MOVEMENT,
            "turn",
            vec![V("A", 1.0), V("Left", 1.0), V("D", -1.0), V("Right", -1.0)],
        ),
        // Mouse look is bound here as well as in camera: a press of the
        // right button that completes the `MouseLeft+MouseRight` run chord
        // is consumed by this context, so camera's binding would never see
        // it. Bound here, the single-button binding fires together with the
        // chord (same context, §7.2), and its hold outlives the chord when
        // the left button is let go, whatever the press order.
        (MOVEMENT, "mouselook", vec![D("MouseRight")]),
        (MOVEMENT, "jump", vec![D("Space"), D("PadSouth")]),
        (MOVEMENT, "walk", vec![D("Shift")]),
        (MOVEMENT, "autorun", vec![D("NumLock"), D("R"), D("PadL3")]),
        (
            FLY,
            "fly_vertical",
            vec![
                V("Space", 1.0),
                V("C", -1.0),
                V("PadR1", 1.0),
                V("PadL1", -1.0),
            ],
        ),
        (FLY, "fly_boost", vec![D("Shift")]),
        (FLY, "fly_slow", vec![D("Alt")]),
        (FLY, "fly_speed", vec![A("WheelY", 1.0, 0.0)]),
        // Pointer and UI actions (§8, decision 0014 D2).
        (POINTER, "select_left", vec![D("MouseLeft")]),
        (
            POINTER,
            "select_double_left",
            vec![O("MouseLeft", DoubleNearby)],
        ),
        (POINTER, "tap_left", vec![O("MouseLeft", Tap)]),
        (POINTER, "select_right", vec![O("MouseRight", Tap)]),
        (
            POINTER,
            "select_double_right",
            vec![O("MouseRight", DoubleNearby)],
        ),
        (POINTER, "select_middle", vec![]),
        (POINTER, "select_double_middle", vec![]),
        (POINTER, "tap_right", vec![]),
        (POINTER, "tap_middle", vec![]),
        (UI_SCROLL_MOUSE, "scroll_up", vec![D("WheelUp")]),
        (UI_SCROLL_MOUSE, "scroll_down", vec![D("WheelDown")]),
        (UI_DIALOG, "focus_next", vec![D("Tab")]),
        (UI_DIALOG, "focus_prev", vec![D("Shift+Tab")]),
        (UI_DIALOG, "focus_left", vec![D("Left")]),
        (UI_DIALOG, "focus_right", vec![D("Right")]),
        (UI_DIALOG, "focus_up", vec![D("Up")]),
        (UI_DIALOG, "focus_down", vec![D("Down")]),
        (UI_DIALOG, "menu", vec![D("Escape")]),
        (TEXT_ENTRY, "menu", vec![D("Escape")]),
        (TEXT_ENTRY, "open_chat", vec![D("Enter"), D("NumpadEnter")]),
        (TEXT_ENTRY, "text_char_left", vec![D("Left")]),
        (TEXT_ENTRY, "text_char_right", vec![D("Right")]),
        (TEXT_ENTRY, "text_line_up", vec![D("Up")]),
        (TEXT_ENTRY, "text_line_down", vec![D("Down")]),
        (TEXT_ENTRY, "text_line_start", vec![D("Home")]),
        (TEXT_ENTRY, "text_line_end", vec![D("End")]),
        (TEXT_ENTRY, "text_backspace", vec![D("Backspace")]),
        (TEXT_ENTRY, "text_delete", vec![D("Delete")]),
        (
            UI_CLIPBOARD,
            "text_copy",
            vec![D("Ctrl+C"), D("Ctrl+Insert")],
        ),
        (
            UI_CLIPBOARD,
            "text_cut",
            vec![D("Ctrl+X"), D("Shift+Delete")],
        ),
        (
            UI_CLIPBOARD,
            "text_paste",
            vec![D("Ctrl+V"), D("Shift+Insert")],
        ),
    ];
    // P2 targeting and quickslot actions: the original's defaults (`keymap.md` §8.2, K47).
    for (_, action, _, keys) in crate::combat::TARGET_ACTIONS {
        t.push((GLOBAL, action, keys.iter().map(|k| D(k)).collect()));
    }
    for (i, action) in crate::combat::QUICKSLOT_ACTIONS.iter().enumerate() {
        let chords = crate::combat::quickslot_default_chords(i)
            .into_iter()
            .map(Entry::S)
            .collect();
        t.push((GLOBAL, *action, chords));
    }
    // Shared by text_entry and ui.chat_log.
    for ctx in [TEXT_ENTRY, UI_CHAT_LOG] {
        t.push((ctx, "text_page_up", vec![D("PageUp")]));
        t.push((ctx, "text_page_down", vec![D("PageDown")]));
        t.push((
            ctx,
            "text_doc_start",
            vec![D("Ctrl+Home"), D("Ctrl+Shift+Home")],
        ));
        t.push((
            ctx,
            "text_doc_end",
            vec![D("Ctrl+End"), D("Ctrl+Shift+End")],
        ));
    }
    t.push((
        TEXT_ENTRY,
        "text_word_left",
        vec![D("Ctrl+Left"), D("Ctrl+Shift+Left")],
    ));
    t.push((
        TEXT_ENTRY,
        "text_word_right",
        vec![D("Ctrl+Right"), D("Ctrl+Shift+Right")],
    ));
    t
}

enum Entry {
    /// Digital, value 1.
    D(&'static str),
    /// Digital, value 1, from a built chord text.
    S(String),
    /// Digital with a value.
    V(&'static str, f32),
    /// Digital with an activation filter (`on`).
    O(&'static str, crate::chord::Activation),
    /// Analog `(axis, scale, deadzone)`.
    A(&'static str, f32, f32),
}

/// The built-in default keymap (every built-in action listed; the unbound
/// pointer actions of §8 with empty lists).
#[must_use]
pub fn default_keymap() -> Keymap {
    let mut k = Keymap::new();
    for (ctx, action, entries) in default_table() {
        let list = entries
            .into_iter()
            .filter_map(|e| match e {
                Entry::D(t) => BindingSpec::parse(t).ok(),
                Entry::S(t) => BindingSpec::parse(&t).ok(),
                Entry::V(t, v) => crate::Chord::parse(t)
                    .ok()
                    .map(|c| BindingSpec::digital(c, v)),
                Entry::O(t, on) => crate::Chord::parse(t)
                    .and_then(|c| c.with_activation(on))
                    .ok()
                    .map(BindingSpec::chord),
                Entry::A(a, s, d) => {
                    AnalogAxis::from_name(a).map(|axis| BindingSpec::analog(axis, s, d))
                }
            })
            .collect();
        if let Err(e) = k.set(ctx, action, list) {
            tracing::error!(error = %e, "built-in binding rejected");
        }
    }
    k
}

/// Ids of the built-in actions and contexts, looked up once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct StandardActions {
    pub global: ContextId,
    pub camera: ContextId,
    pub movement: ContextId,
    pub fly: ContextId,
    pub text_entry: ContextId,
    pub menu: ActionId,
    pub screenshot: ActionId,
    pub toggle_fly: ActionId,
    pub toggle_debug: ActionId,
    pub open_chat: ActionId,
    pub mouselook: ActionId,
    pub look_x: ActionId,
    pub look_y: ActionId,
    pub zoom: ActionId,
    pub reset_camera: ActionId,
    pub move_forward: ActionId,
    pub strafe: ActionId,
    pub turn: ActionId,
    pub jump: ActionId,
    pub walk: ActionId,
    pub autorun: ActionId,
    pub fly_vertical: ActionId,
    pub fly_boost: ActionId,
    pub fly_slow: ActionId,
    pub fly_speed: ActionId,
    /// The pointer context (§8); not touched by the mode helpers.
    pub pointer: ContextId,
    pub text_backspace: ActionId,
}

/// Ids of the built-in UI contexts and pointer and UI actions (§8), looked
/// up once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct UiActions {
    pub pointer: ContextId,
    pub text_entry: ContextId,
    pub ui_dialog: ContextId,
    pub ui_clipboard: ContextId,
    pub ui_scroll_mouse: ContextId,
    pub ui_scroll: ContextId,
    pub ui_eat_keyboard: ContextId,
    pub ui_chat_log: ContextId,
    pub select_left: ActionId,
    pub select_double_left: ActionId,
    pub tap_left: ActionId,
    pub select_right: ActionId,
    pub select_double_right: ActionId,
    pub select_middle: ActionId,
    pub select_double_middle: ActionId,
    pub tap_right: ActionId,
    pub tap_middle: ActionId,
    pub scroll_up: ActionId,
    pub scroll_down: ActionId,
    pub focus_next: ActionId,
    pub focus_prev: ActionId,
    pub focus_left: ActionId,
    pub focus_right: ActionId,
    pub focus_up: ActionId,
    pub focus_down: ActionId,
    pub text_char_left: ActionId,
    pub text_char_right: ActionId,
    pub text_word_left: ActionId,
    pub text_word_right: ActionId,
    pub text_line_up: ActionId,
    pub text_line_down: ActionId,
    pub text_page_up: ActionId,
    pub text_page_down: ActionId,
    pub text_line_start: ActionId,
    pub text_line_end: ActionId,
    pub text_doc_start: ActionId,
    pub text_doc_end: ActionId,
    pub text_backspace: ActionId,
    pub text_delete: ActionId,
    pub text_copy: ActionId,
    pub text_cut: ActionId,
    pub text_paste: ActionId,
}

impl UiActions {
    /// Look up every built-in UI id in `registry`; `None` if one is missing.
    #[must_use]
    pub fn new(registry: &Registry) -> Option<Self> {
        let c = |n| registry.context(n);
        let a = |n| registry.action(n);
        Some(Self {
            pointer: c(contexts::POINTER)?,
            text_entry: c(contexts::TEXT_ENTRY)?,
            ui_dialog: c(contexts::UI_DIALOG)?,
            ui_clipboard: c(contexts::UI_CLIPBOARD)?,
            ui_scroll_mouse: c(contexts::UI_SCROLL_MOUSE)?,
            ui_scroll: c(contexts::UI_SCROLL)?,
            ui_eat_keyboard: c(contexts::UI_EAT_KEYBOARD)?,
            ui_chat_log: c(contexts::UI_CHAT_LOG)?,
            select_left: a("select_left")?,
            select_double_left: a("select_double_left")?,
            tap_left: a("tap_left")?,
            select_right: a("select_right")?,
            select_double_right: a("select_double_right")?,
            select_middle: a("select_middle")?,
            select_double_middle: a("select_double_middle")?,
            tap_right: a("tap_right")?,
            tap_middle: a("tap_middle")?,
            scroll_up: a("scroll_up")?,
            scroll_down: a("scroll_down")?,
            focus_next: a("focus_next")?,
            focus_prev: a("focus_prev")?,
            focus_left: a("focus_left")?,
            focus_right: a("focus_right")?,
            focus_up: a("focus_up")?,
            focus_down: a("focus_down")?,
            text_char_left: a("text_char_left")?,
            text_char_right: a("text_char_right")?,
            text_word_left: a("text_word_left")?,
            text_word_right: a("text_word_right")?,
            text_line_up: a("text_line_up")?,
            text_line_down: a("text_line_down")?,
            text_page_up: a("text_page_up")?,
            text_page_down: a("text_page_down")?,
            text_line_start: a("text_line_start")?,
            text_line_end: a("text_line_end")?,
            text_doc_start: a("text_doc_start")?,
            text_doc_end: a("text_doc_end")?,
            text_backspace: a("text_backspace")?,
            text_delete: a("text_delete")?,
            text_copy: a("text_copy")?,
            text_cut: a("text_cut")?,
            text_paste: a("text_paste")?,
        })
    }
}

/// Look-input tuning for [`StandardActions::look`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LookSettings {
    /// Radians per mouse device unit.
    pub mouse_sensitivity: f32,
    /// Radians per second at full stick deflection.
    pub stick_rate: f32,
    /// Invert vertical look.
    pub invert_y: bool,
}

impl Default for LookSettings {
    /// Tessera's own defaults: 0.0025 rad per device unit, 2.5 rad/s stick.
    fn default() -> Self {
        Self {
            mouse_sensitivity: 0.0025,
            stick_rate: 2.5,
            invert_y: false,
        }
    }
}

impl StandardActions {
    /// Look up every built-in id in `registry`; `None` if one is missing
    /// (the registry was not built from [`default_registry`]).
    #[must_use]
    pub fn new(registry: &Registry) -> Option<Self> {
        let c = |n| registry.context(n);
        let a = |n| registry.action(n);
        Some(Self {
            global: c(contexts::GLOBAL)?,
            camera: c(contexts::CAMERA)?,
            movement: c(contexts::MOVEMENT)?,
            fly: c(contexts::FLY)?,
            text_entry: c(contexts::TEXT_ENTRY)?,
            menu: a("menu")?,
            screenshot: a("screenshot")?,
            toggle_fly: a("toggle_fly")?,
            toggle_debug: a("toggle_debug")?,
            open_chat: a("open_chat")?,
            mouselook: a("mouselook")?,
            look_x: a("look_x")?,
            look_y: a("look_y")?,
            zoom: a("zoom")?,
            reset_camera: a("reset_camera")?,
            move_forward: a("move_forward")?,
            strafe: a("strafe")?,
            turn: a("turn")?,
            jump: a("jump")?,
            walk: a("walk")?,
            autorun: a("autorun")?,
            fly_vertical: a("fly_vertical")?,
            fly_boost: a("fly_boost")?,
            fly_slow: a("fly_slow")?,
            fly_speed: a("fly_speed")?,
            pointer: c(contexts::POINTER)?,
            text_backspace: a("text_backspace")?,
        })
    }

    /// Movement intent for this frame. `autorun` adds full forward input
    /// unless the player is already pulling backwards. Mouse look while
    /// `mouselook` is held turns the body as well (the caller decides
    /// whether to also feed [`StandardActions::look`] into the camera).
    #[must_use]
    pub fn intent(&self, m: &InputMapper, autorun: bool) -> MoveIntent {
        let mut forward = m.value(self.move_forward);
        if autorun && forward >= 0.0 {
            forward = 1.0;
        }
        MoveIntent {
            forward,
            strafe: m.value(self.strafe),
            turn: m.value(self.turn),
            vertical: m.value(self.fly_vertical),
            jump: m.just_pressed(self.jump),
            walk: m.pressed(self.walk),
            boost: m.pressed(self.fly_boost),
            slow: m.pressed(self.fly_slow),
        }
        .sanitized()
    }

    /// Look change `(yaw, pitch)` in radians for a frame of `dt` seconds:
    /// mouse deltas only while `mouselook` is held or `captured` is true
    /// (cursor captured by the caller), plus stick deflection at
    /// `stick_rate`. Positive yaw turns left (counter-clockwise seen from
    /// above, `+Z` up); positive pitch looks up.
    ///
    /// `look_x`/`look_y` follow the mouse convention (`+` = right / down),
    /// so both the mouse and the stick term are subtracted: the default
    /// `PadRightY` binding has scale −1 (stick pushed away = `look_y` −1 =
    /// look up, like moving the mouse away).
    #[must_use]
    pub fn look(
        &self,
        m: &InputMapper,
        settings: &LookSettings,
        dt: f32,
        captured: bool,
    ) -> (f32, f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let mouse = captured || m.pressed(self.mouselook);
        let (mx, my) = if mouse {
            (m.delta(self.look_x), m.delta(self.look_y))
        } else {
            (0.0, 0.0)
        };
        let sens = settings.mouse_sensitivity;
        let sy = if settings.invert_y { -1.0 } else { 1.0 };
        // Mouse +x is right (turn right = negative yaw); +y is down.
        let yaw = -mx * sens - m.value(self.look_x) * settings.stick_rate * dt;
        let pitch = sy * (-my * sens - m.value(self.look_y) * settings.stick_rate * dt);
        let f = |v: f32| {
            if v.is_finite() {
                v.clamp(-10.0, 10.0)
            } else {
                0.0
            }
        };
        (f(yaw), f(pitch))
    }

    /// Activate the contexts for walking (global, camera, movement).
    pub fn enter_walk_mode(&self, m: &mut InputMapper) {
        m.deactivate(self.fly);
        m.activate(self.global);
        m.activate(self.camera);
        m.activate(self.movement);
    }

    /// Activate the contexts for free flight (fly above movement).
    pub fn enter_fly_mode(&self, m: &mut InputMapper) {
        m.activate(self.global);
        m.activate(self.camera);
        m.activate(self.movement);
        m.activate(self.fly);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::device::{KeyCode, MouseButton};
    use crate::mapper::RawEvent;

    #[test]
    fn defaults_compile_cleanly() {
        let reg = default_registry();
        assert_eq!(reg.action_count(), action_defs().len());
        let km = default_keymap();
        // Every built-in action is listed and every entry parsed.
        for (c, a, entries) in default_table() {
            assert_eq!(km.get(c, a).unwrap().len(), entries.len(), "{c}.{a}");
        }
        for (_, def) in reg.actions() {
            assert!(
                km.iter().any(|(_, a, _)| a == def.name),
                "{} not listed",
                def.name
            );
        }
        let (_, w) = InputMapper::new(reg.clone(), &km);
        assert!(w.is_empty(), "{w:?}");
        assert!(StandardActions::new(&reg).is_some());
        assert!(UiActions::new(&reg).is_some());
        assert!(StandardActions::new(&Registry::new()).is_none());
        for retired in [
            "text_submit",
            "text_cancel",
            "text_history_prev",
            "text_history_next",
        ] {
            assert!(reg.action(retired).is_none(), "{retired}");
        }
    }

    #[test]
    fn p2_targeting_and_quickslot_defaults() {
        let (ids, mut m) = walk_mapper();
        let t = Duration::ZERO;
        let k = |key, pressed| RawEvent::Key {
            key,
            pressed,
            repeat: false,
        };
        let a = |m: &InputMapper, n: &str| m.registry().action(n).expect(n);
        let press = |m: &mut InputMapper, mods: &[KeyCode], key: KeyCode| {
            for &mk in mods {
                m.handle(&k(mk, true), t);
            }
            m.handle(&k(key, true), t);
            m.update(t);
        };
        let release = |m: &mut InputMapper, mods: &[KeyCode], key: KeyCode| {
            m.handle(&k(key, false), t);
            for &mk in mods {
                m.handle(&k(mk, false), t);
            }
            m.update(t);
        };
        use KeyCode::{AltLeft, Backslash, ControlLeft, Digit1, Equal, F, ShiftLeft, Tab};
        for (mods, key, action) in [
            (&[][..], Digit1, "quickslot_1"),
            (&[ControlLeft][..], Digit1, "quickslot_13"),
            (&[AltLeft][..], Equal, "quickslot_36"),
            (&[ShiftLeft][..], Equal, "quickslot_48"),
            (&[][..], Tab, "target_next_enemy"),
            (&[ShiftLeft][..], Tab, "target_previous_enemy"),
            (&[ControlLeft][..], Tab, "target_nearest_player"),
            (&[][..], Backslash, "target_self"),
            (&[][..], F, "target_assist"),
        ] {
            press(&mut m, mods, key);
            assert!(m.just_pressed(a(&m, action)), "{mods:?} {key:?} {action}");
            release(&mut m, mods, key);
        }
        // Exact policy: Ctrl+F stays `toggle_fly`, not `target_assist` (§8.2).
        press(&mut m, &[ControlLeft], F);
        assert!(m.just_pressed(ids.toggle_fly));
        assert!(!m.just_pressed(a(&m, "target_assist")));
        release(&mut m, &[ControlLeft], F);
        // Shift+1 also holds `walk` (§8.2 interplay).
        press(&mut m, &[ShiftLeft], Digit1);
        assert!(m.just_pressed(a(&m, "quickslot_37")));
        assert!(m.pressed(ids.walk));
        assert!(!m.just_pressed(a(&m, "quickslot_1")));
        release(&mut m, &[ShiftLeft], Digit1);
    }

    fn walk_mapper() -> (StandardActions, InputMapper) {
        let reg = default_registry();
        let ids = StandardActions::new(&reg).unwrap();
        let (mut m, _) = InputMapper::new(reg, &default_keymap());
        ids.enter_walk_mode(&mut m);
        (ids, m)
    }

    fn mouse(m: &mut InputMapper, button: MouseButton, pressed: bool) {
        m.handle(&RawEvent::MouseButton { button, pressed }, Duration::ZERO);
    }

    #[test]
    fn stick_and_mouse_pitch_agree() {
        let (ids, mut m) = walk_mapper();
        let t = Duration::ZERO;
        let s = LookSettings::default();
        // Mouse moved away from the player (up) with mouse look held.
        mouse(&mut m, MouseButton::Right, true);
        m.handle(
            &RawEvent::MouseMotion {
                dx: 0.0,
                dy: -100.0,
            },
            t,
        );
        m.update(t);
        let (_, mouse_pitch) = ids.look(&m, &s, 0.1, false);
        mouse(&mut m, MouseButton::Right, false);
        // Right stick pushed away from the player (§4.3: +Y).
        m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadRightY,
                value: 1.0,
            },
            t,
        );
        m.update(t);
        let (_, stick_pitch) = ids.look(&m, &s, 0.1, false);
        assert!(
            mouse_pitch > 0.0 && stick_pitch > 0.0,
            "{mouse_pitch} {stick_pitch}"
        );
        // Inverting flips both.
        let inv = LookSettings {
            invert_y: true,
            ..s
        };
        assert!(ids.look(&m, &inv, 0.1, false).1 < 0.0);
        // Stick right turns right (negative yaw), like the mouse.
        m.handle(
            &RawEvent::PadAxis {
                axis: AnalogAxis::PadRightX,
                value: 1.0,
            },
            t,
        );
        m.update(t);
        assert!(ids.look(&m, &s, 0.1, false).0 < 0.0);
    }

    #[test]
    fn mouselook_survives_releasing_left_in_either_order() {
        for order in [
            [MouseButton::Left, MouseButton::Right],
            [MouseButton::Right, MouseButton::Left],
        ] {
            let (ids, mut m) = walk_mapper();
            for b in order {
                mouse(&mut m, b, true);
            }
            m.update(Duration::ZERO);
            assert!(m.pressed(ids.mouselook), "{order:?}");
            assert_eq!(m.value(ids.move_forward), 1.0, "{order:?}");
            mouse(&mut m, MouseButton::Left, false);
            m.update(Duration::ZERO);
            assert!(m.pressed(ids.mouselook), "{order:?}: right still held");
            assert_eq!(m.value(ids.move_forward), 0.0, "{order:?}");
            mouse(&mut m, MouseButton::Right, false);
            m.update(Duration::ZERO);
            assert!(!m.pressed(ids.mouselook), "{order:?}");
        }
    }

    #[test]
    fn walk_and_fly_mode_bindings() {
        let reg = default_registry();
        let ids = StandardActions::new(&reg).unwrap();
        let (mut m, _) = InputMapper::new(reg, &default_keymap());
        ids.enter_walk_mode(&mut m);
        let t = Duration::ZERO;
        let k = |key, pressed| RawEvent::Key {
            key,
            pressed,
            repeat: false,
        };
        m.handle(&k(KeyCode::Space, true), t);
        m.handle(&k(KeyCode::ShiftLeft, true), t);
        m.handle(&k(KeyCode::W, true), t);
        m.update(t);
        let i = ids.intent(&m, false);
        assert!(i.jump && i.walk && i.forward == 1.0 && i.vertical == 0.0);
        m.handle(&k(KeyCode::Space, false), t);
        ids.enter_fly_mode(&mut m);
        m.handle(&k(KeyCode::Space, true), t);
        m.update(t);
        let i = ids.intent(&m, false);
        assert!(!i.jump && i.vertical == 1.0 && i.boost);
        // Mouse look only while the right button is held.
        m.handle(&RawEvent::MouseMotion { dx: 100.0, dy: 0.0 }, t);
        m.update(t);
        assert_eq!(
            ids.look(&m, &LookSettings::default(), 0.016, false),
            (0.0, 0.0)
        );
        m.handle(
            &RawEvent::MouseButton {
                button: MouseButton::Right,
                pressed: true,
            },
            t,
        );
        m.handle(
            &RawEvent::MouseMotion {
                dx: 100.0,
                dy: -40.0,
            },
            t,
        );
        m.update(t);
        let (yaw, pitch) = ids.look(&m, &LookSettings::default(), 0.016, false);
        assert!(
            (yaw + 0.25).abs() < 1e-6 && (pitch - 0.1).abs() < 1e-6,
            "{yaw} {pitch}"
        );
        // Both buttons: run forward and keep looking, in either order.
        m.handle(
            &RawEvent::MouseButton {
                button: MouseButton::Right,
                pressed: false,
            },
            t,
        );
        m.handle(&k(KeyCode::W, false), t);
        for b in [MouseButton::Left, MouseButton::Right] {
            m.handle(
                &RawEvent::MouseButton {
                    button: b,
                    pressed: true,
                },
                t,
            );
        }
        m.handle(&RawEvent::MouseMotion { dx: 10.0, dy: 0.0 }, t);
        m.update(t);
        assert_eq!(ids.intent(&m, false).forward, 1.0);
        assert!(ids.look(&m, &LookSettings::default(), 0.016, false).0 < 0.0);
        for b in [MouseButton::Left, MouseButton::Right] {
            m.handle(
                &RawEvent::MouseButton {
                    button: b,
                    pressed: false,
                },
                t,
            );
        }
        // Autorun.
        m.handle(&k(KeyCode::W, false), t);
        m.update(t);
        assert_eq!(ids.intent(&m, true).forward, 1.0);
    }
}
