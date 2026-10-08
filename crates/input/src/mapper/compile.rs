// SPDX-License-Identifier: GPL-3.0-or-later
//! Compiling a keymap's names against a registry (`keymap.md` §2.2, §3.1,
//! §5.2, §5.3).

use std::sync::Arc;

use crate::action::{ActionId, ActionKind, ContextId, ModifierPolicy, Registry};
use crate::chord::{Activation, Chord};
use crate::commands::command_for_entry;
use crate::device::AnalogAxis;
use crate::error::KeymapWarning;
use crate::keymap::{BindingSpec, Keymap, push_warning};

/// One enabled digital binding of a context.
#[derive(Debug, Clone)]
pub(super) struct DigitalBinding {
    pub(super) action: ActionId,
    pub(super) chord: Chord,
    /// Entry value, clamped to `-1..=1` (1 for plain entries).
    pub(super) value: f32,
    pub(super) policy: ModifierPolicy,
    /// The command name its events carry (§2.2), if any.
    pub(super) command: Option<Arc<str>>,
}

impl DigitalBinding {
    pub(super) fn on(&self) -> Activation {
        self.chord.activation()
    }
}

/// One analog binding of a context.
#[derive(Debug, Clone)]
pub(super) struct AnalogBinding {
    pub(super) action: ActionId,
    pub(super) axis: AnalogAxis,
    pub(super) scale: f32,
    pub(super) deadzone: f32,
}

/// The bindings of one context. Digital bindings are ordered by action
/// registration order, then entry order (§7.2.1).
#[derive(Debug, Clone, Default)]
pub(super) struct CompiledContext {
    pub(super) digital: Vec<DigitalBinding>,
    pub(super) analog: Vec<AnalogBinding>,
}

/// Resolve a keymap's names against a registry. Unknown names, entries a
/// context cannot use and prefix conflicts are reported as warnings (capped
/// like the reader's, §6.3).
pub(super) fn compile(
    registry: &Registry,
    keymap: &Keymap,
) -> (Vec<CompiledContext>, Vec<KeymapWarning>) {
    let mut out = vec![CompiledContext::default(); registry.context_count()];
    // (action, entry index, binding) per context, sorted before storing.
    let mut staged: Vec<Vec<(ActionId, usize, DigitalBinding)>> =
        vec![Vec::new(); registry.context_count()];
    let mut warnings = Vec::new();
    let mut unknown_ctx: Vec<String> = Vec::new();
    for (c, a, list) in keymap.iter() {
        let Some(ctx) = registry.context(c) else {
            if !unknown_ctx.iter().any(|u| u == c) {
                unknown_ctx.push(c.into());
                push_warning(&mut warnings, || KeymapWarning::UnknownContext(c.into()));
            }
            continue;
        };
        let Some(action) = registry.action(a) else {
            push_warning(&mut warnings, || KeymapWarning::UnknownAction {
                context: c.into(),
                action: a.into(),
            });
            continue;
        };
        let (Some(def), Some(cdef)) = (registry.action_def(action), registry.context_def(ctx))
        else {
            continue;
        };
        let (Some(cc), Some(stage)) = (out.get_mut(ctx.index()), staged.get_mut(ctx.index()))
        else {
            continue;
        };
        for (index, spec) in list.iter().enumerate() {
            match spec {
                BindingSpec::Digital { chord, value } => {
                    let disabled = if chord.activation() != Activation::Press
                        && def.kind == ActionKind::Axis
                    {
                        Some("a non-`press` `on` on an axis action adds nothing to its value")
                    } else if cdef.pass_through && chord.strokes().len() > 1 {
                        Some("a pass-through context holds no multi-stroke binding")
                    } else {
                        None
                    };
                    if let Some(reason) = disabled {
                        push_warning(&mut warnings, || KeymapWarning::Disabled {
                            context: c.into(),
                            action: a.into(),
                            binding: binding_text(chord),
                            reason,
                        });
                        continue;
                    }
                    let command = keymap
                        .entry_command(c, a, index)
                        .map(str::to_owned)
                        .or_else(|| command_for_entry(c, a, def.kind, spec).map(str::to_owned))
                        .or_else(|| def.command.clone())
                        .map(Arc::<str>::from);
                    stage.push((
                        action,
                        index,
                        DigitalBinding {
                            action,
                            chord: chord.clone(),
                            // §2.2, §5.3: a button action ignores `value`;
                            // its offered value is 1.0.
                            value: if def.kind == ActionKind::Button {
                                1.0
                            } else if value.is_finite() {
                                value.clamp(-1.0, 1.0)
                            } else {
                                0.0
                            },
                            policy: def.modifiers,
                            command,
                        },
                    ));
                }
                BindingSpec::Analog {
                    axis,
                    scale,
                    deadzone,
                } => cc.analog.push(AnalogBinding {
                    action,
                    axis: *axis,
                    scale: if scale.is_finite() { *scale } else { 0.0 },
                    deadzone: if deadzone.is_finite() {
                        deadzone.clamp(0.0, 0.99)
                    } else {
                        0.0
                    },
                }),
            }
        }
    }
    for (cc, mut stage) in out.iter_mut().zip(staged) {
        stage.sort_by_key(|(a, i, _)| (*a, *i));
        cc.digital = stage.into_iter().map(|(_, _, d)| d).collect();
    }
    // A binding whose strokes start with all strokes of a shorter `press`
    // binding of the same context would never be reached (the shorter one
    // always fires first, §5.2): disable the longer one.
    for (ci, cc) in out.iter_mut().enumerate() {
        let shorter: Vec<Chord> = cc
            .digital
            .iter()
            .filter(|d| d.on() == Activation::Press)
            .map(|d| d.chord.clone())
            .collect();
        let ctx_name = registry
            .context_def(ContextId(u16::try_from(ci).unwrap_or(u16::MAX)))
            .map(|d| d.name.clone())
            .unwrap_or_default();
        cc.digital.retain(|d| {
            let s = d.chord.strokes();
            let shadow = shorter.iter().find(|other| {
                let o = other.strokes();
                o.len() < s.len() && s[..o.len()] == *o
            });
            if let Some(shadow) = shadow {
                push_warning(&mut warnings, || KeymapWarning::PrefixConflict {
                    context: ctx_name.clone(),
                    sequence: d.chord.to_string(),
                    stroke: shadow.to_string(),
                });
                false
            } else {
                true
            }
        });
    }
    (out, warnings)
}

/// A binding as written in the keymap: its strokes, plus the `on` value when
/// it is not `press`.
fn binding_text(chord: &Chord) -> String {
    match chord.activation() {
        Activation::Press => chord.to_string(),
        on => format!("{chord} (on = {on})"),
    }
}
