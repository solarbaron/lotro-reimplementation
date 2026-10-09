// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors and non-fatal issues of the runtime.

use tessera_formats::Did;
use tessera_formats::script_table::NodeKind;

use crate::sink::EntityId;

/// Errors of the public API. Data problems are never errors: they are reported as
/// [`RuntimeIssue`]s and the affected node completes at once.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RuntimeError {
    /// [`crate::Runtime::spawn_entity`] was called with an ID that is already in use.
    #[error("entity {0:?} already exists")]
    EntityExists(EntityId),
    /// The entity is not known (never spawned, or despawned).
    #[error("entity {0:?} does not exist")]
    NoSuchEntity(EntityId),
    /// The runtime holds [`crate::RuntimeLimits::max_entities`] entities already.
    #[error("entity limit reached ({0})")]
    TooManyEntities(usize),
}

/// A non-fatal problem met while running graphs.
///
/// Each distinct issue is recorded once (per record, key, kind or limit), however often it
/// occurs, so the list stays small; [`crate::Runtime::issues`] returns them and
/// [`crate::RuntimeStats`] counts every occurrence of the limit kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RuntimeIssue {
    /// A table or scriptlet named by the data does not exist or does not decode.
    MissingRecord(Did),
    /// A script key that the client asked to start (for example a start-up name) is defined
    /// in no table of the entity's closure (`script-table.md` §8.1: such keys exist).
    ScriptNotDefined {
        /// The script key.
        key: u32,
    },
    /// A guard of [`crate::RuntimeLimits`] stopped a start or an event; the value names it.
    Limit(&'static str),
    /// A nested change of one property was dropped by the re-entry guard
    /// (`script-runtime.md` §21).
    PropertyReentry {
        /// The property.
        pid: u32,
    },
    /// A node kind the runtime does not implement completed at once
    /// (`script-runtime.md` §18).
    Unimplemented(NodeKind),
    /// A node target type the runtime cannot resolve; the node did nothing for it.
    UnsupportedTarget {
        /// The node kind.
        kind: NodeKind,
        /// The target type (enum 0x2300019B).
        target_type: u8,
    },
    /// A Goto found no Label of its name in its record (`script-runtime.md` §15 step 1).
    LabelNotFound(String),
    /// A GameCallback completed at once: how the original evaluates it offline is open
    /// (`script-runtime.md` §18 OPEN-11), and the runtime does not fire its ports.
    GameCallbackOpaque,
}
