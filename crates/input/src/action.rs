// SPDX-License-Identifier: GPL-3.0-or-later
//! Actions and contexts: the names the keymap binds to, defined by code
//! (`docs/specs/keymap.md` §2–§3).

use std::collections::HashMap;

use crate::error::{InputError, Result};

/// Most actions one registry holds.
pub const MAX_ACTIONS: usize = 1024;
/// Most contexts one registry holds.
pub const MAX_CONTEXTS: usize = 64;
/// Longest action or context name.
pub const MAX_NAME_LEN: usize = 64;

/// Handle of a registered action (index into its [`Registry`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActionId(pub(crate) u16);

impl ActionId {
    /// Dense index, `0..registry.action_count()`.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// Handle of a registered context (index into its [`Registry`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContextId(pub(crate) u16);

impl ContextId {
    /// Dense index, `0..registry.context_count()`.
    #[must_use]
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

/// What an action reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionKind {
    /// Pressed/released with edge counts; analog sources press it past
    /// [`ANALOG_PRESS_THRESHOLD`](crate::ANALOG_PRESS_THRESHOLD).
    #[default]
    Button,
    /// A value in `-1..=1` (held digital bindings and absolute axes) plus a
    /// per-frame `delta` (relative axes such as mouse motion).
    Axis,
}

/// How a binding's modifiers are compared with the held modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModifierPolicy {
    /// The held modifiers must equal the binding's (`K` does not fire while
    /// Ctrl is held). Right for commands.
    #[default]
    Exact,
    /// The binding's modifiers must be held; extra ones are ignored, but an
    /// exact match elsewhere in the same context wins. Right for movement
    /// (`W` keeps moving while Shift is held).
    Subset,
}

/// Definition of one action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDef {
    /// Unique name used in the keymap file.
    pub name: String,
    /// What it reports.
    pub kind: ActionKind,
    /// Modifier matching for its bindings.
    pub modifiers: ModifierPolicy,
    /// Whether its offered holds repeat on OS auto-repeat **when no
    /// action-map data is present** (`keymap.md` §2.1, §7.8.6). With
    /// action-map data the command of each hold decides.
    pub repeat: bool,
    /// The InputActions command name used for its events when neither the
    /// binding entry nor the (context, action) table gives one (§2.2 rule
    /// 3). No built-in action sets it.
    pub command: Option<String>,
}

impl ActionDef {
    /// A button action with exact modifier matching.
    #[must_use]
    pub fn button(name: &str) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Button,
            modifiers: ModifierPolicy::Exact,
            repeat: false,
            command: None,
        }
    }

    /// An axis action with subset modifier matching.
    #[must_use]
    pub fn axis(name: &str) -> Self {
        Self {
            name: name.into(),
            kind: ActionKind::Axis,
            modifiers: ModifierPolicy::Subset,
            repeat: false,
            command: None,
        }
    }

    /// Same definition with another modifier policy.
    #[must_use]
    pub fn with_modifiers(mut self, policy: ModifierPolicy) -> Self {
        self.modifiers = policy;
        self
    }

    /// Same definition with the `repeat` flag set as given (§2.1).
    #[must_use]
    pub fn with_repeat(mut self, repeat: bool) -> Self {
        self.repeat = repeat;
        self
    }

    /// Same definition with its own command name (§2.1, §2.2 rule 3).
    #[must_use]
    pub fn with_command(mut self, command: &str) -> Self {
        self.command = Some(command.to_owned());
        self
    }
}

/// The two bands of the context stack (`keymap.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Band {
    /// Game band: activated by code, ordered by priority then recency.
    #[default]
    Game,
    /// UI band: entries pushed and popped by owner (§3.2), consulted first.
    Ui,
}

/// Definition of one input context (`keymap.md` §3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextDef {
    /// Unique name used in the keymap file.
    pub name: String,
    /// Game band: higher priorities are consulted first; equal priorities
    /// by most recent activation. UI band: the priority `activate` pushes
    /// with (§3.4).
    pub priority: i32,
    /// An opaque context swallows every input it does not bind, so contexts
    /// consulted after it see nothing while it is active.
    pub opaque: bool,
    /// Whether this context can receive typed text (§7.6).
    pub text: bool,
    /// The band, fixed at registration (§3.1, §3.4).
    pub band: Band,
    /// UI band only: keys and pad buttons that no UI-band context takes do
    /// not reach the game band (§3.3).
    pub opaque_keys: bool,
    /// Its matches fire and are offered, and the input continues to lower
    /// contexts unless the UI consumed one of their Starts (§3.1, §7.2).
    pub pass_through: bool,
}

impl ContextDef {
    /// A transparent game-band context without text input.
    #[must_use]
    pub fn new(name: &str, priority: i32) -> Self {
        Self {
            name: name.into(),
            priority,
            opaque: false,
            text: false,
            band: Band::Game,
            opaque_keys: false,
            pass_through: false,
        }
    }

    /// Same definition, in the UI band.
    #[must_use]
    pub fn ui(mut self) -> Self {
        self.band = Band::Ui;
        self
    }

    /// Same definition, with `opaque_keys` (UI band only; registering a
    /// game-band context with it is an error).
    #[must_use]
    pub fn with_opaque_keys(mut self) -> Self {
        self.opaque_keys = true;
        self
    }

    /// Same definition, pass-through.
    #[must_use]
    pub fn pass_through(mut self) -> Self {
        self.pass_through = true;
        self
    }

    /// Same definition, opaque.
    #[must_use]
    pub fn opaque(mut self) -> Self {
        self.opaque = true;
        self
    }

    /// Same definition, receiving text.
    #[must_use]
    pub fn with_text(mut self) -> Self {
        self.text = true;
        self
    }
}

/// The set of actions and contexts a program understands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    actions: Vec<ActionDef>,
    contexts: Vec<ContextDef>,
    action_names: HashMap<String, ActionId>,
    context_names: HashMap<String, ContextId>,
}

/// Whether `name` is a valid action or context name: 1–64 bytes of
/// `a-z`, `0-9`, `_`, `.`.
#[must_use]
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
}

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an action.
    ///
    /// # Errors
    ///
    /// [`InputError::InvalidName`], [`InputError::Duplicate`] or
    /// [`InputError::TooMany`].
    pub fn add_action(&mut self, def: ActionDef) -> Result<ActionId> {
        if !is_valid_name(&def.name) {
            return Err(InputError::InvalidName(def.name));
        }
        if self.action_names.contains_key(&def.name) {
            return Err(InputError::Duplicate {
                kind: "action",
                name: def.name,
            });
        }
        if self.actions.len() >= MAX_ACTIONS {
            return Err(InputError::TooMany {
                kind: "actions",
                limit: MAX_ACTIONS,
            });
        }
        let id = ActionId(u16::try_from(self.actions.len()).unwrap_or(u16::MAX));
        self.action_names.insert(def.name.clone(), id);
        self.actions.push(def);
        Ok(id)
    }

    /// Register a context.
    ///
    /// # Errors
    ///
    /// [`InputError::InvalidName`], [`InputError::Duplicate`],
    /// [`InputError::TooMany`], or [`InputError::InvalidContext`] for a
    /// game-band context with `opaque_keys`.
    pub fn add_context(&mut self, def: ContextDef) -> Result<ContextId> {
        if !is_valid_name(&def.name) {
            return Err(InputError::InvalidName(def.name));
        }
        if def.opaque_keys && def.band == Band::Game {
            return Err(InputError::InvalidContext {
                name: def.name,
                reason: "`opaque_keys` is defined only for UI-band contexts",
            });
        }
        if self.context_names.contains_key(&def.name) {
            return Err(InputError::Duplicate {
                kind: "context",
                name: def.name,
            });
        }
        if self.contexts.len() >= MAX_CONTEXTS {
            return Err(InputError::TooMany {
                kind: "contexts",
                limit: MAX_CONTEXTS,
            });
        }
        let id = ContextId(u16::try_from(self.contexts.len()).unwrap_or(u16::MAX));
        self.context_names.insert(def.name.clone(), id);
        self.contexts.push(def);
        Ok(id)
    }

    /// Find an action by name.
    #[must_use]
    pub fn action(&self, name: &str) -> Option<ActionId> {
        self.action_names.get(name).copied()
    }

    /// Find a context by name.
    #[must_use]
    pub fn context(&self, name: &str) -> Option<ContextId> {
        self.context_names.get(name).copied()
    }

    /// An action's definition (`None` for a foreign id).
    #[must_use]
    pub fn action_def(&self, id: ActionId) -> Option<&ActionDef> {
        self.actions.get(id.index())
    }

    /// A context's definition (`None` for a foreign id).
    #[must_use]
    pub fn context_def(&self, id: ContextId) -> Option<&ContextDef> {
        self.contexts.get(id.index())
    }

    /// Number of actions.
    #[must_use]
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Number of contexts.
    #[must_use]
    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    /// All actions with their ids.
    pub fn actions(&self) -> impl Iterator<Item = (ActionId, &ActionDef)> {
        self.actions
            .iter()
            .enumerate()
            .map(|(i, d)| (ActionId(i as u16), d))
    }

    /// All contexts with their ids.
    pub fn contexts(&self) -> impl Iterator<Item = (ContextId, &ContextDef)> {
        self.contexts
            .iter()
            .enumerate()
            .map(|(i, d)| (ContextId(i as u16), d))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_duplicates() {
        let mut r = Registry::new();
        let a = r.add_action(ActionDef::button("jump")).unwrap();
        assert_eq!(r.action("jump"), Some(a));
        assert!(matches!(
            r.add_action(ActionDef::axis("jump")),
            Err(InputError::Duplicate { .. })
        ));
        for bad in ["", "Jump", "a b", "x-y", &"a".repeat(65)] {
            assert!(r.add_action(ActionDef::button(bad)).is_err(), "{bad:?}");
        }
        let c = r.add_context(ContextDef::new("ui.dialog", 5)).unwrap();
        assert_eq!(r.context_def(c).unwrap().priority, 5);
        assert!(r.context_def(ContextId(99)).is_none());
        assert!(matches!(
            r.add_context(ContextDef::new("keys", 0).with_opaque_keys()),
            Err(InputError::InvalidContext { .. })
        ));
        let k = r
            .add_context(ContextDef::new("keys", 0).ui().with_opaque_keys())
            .unwrap();
        assert_eq!(r.context_def(k).unwrap().band, Band::Ui);
    }

    #[test]
    fn limits() {
        let mut r = Registry::new();
        for i in 0..MAX_CONTEXTS {
            r.add_context(ContextDef::new(&format!("c{i}"), 0)).unwrap();
        }
        assert!(matches!(
            r.add_context(ContextDef::new("over", 0)),
            Err(InputError::TooMany { .. })
        ));
    }
}
