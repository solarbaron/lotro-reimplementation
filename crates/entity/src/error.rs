// SPDX-License-Identifier: GPL-3.0-or-later
//! Error types.

use std::fmt;

use crate::entity::Entity;
use crate::store::NetId;

/// Result alias of this crate.
pub type Result<T, E = EntityError> = std::result::Result<T, E>;

/// Why a store operation was refused. The store is unchanged when an operation fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EntityError {
    /// Every entity slot is in use (or retired); see [`StoreConfig::max_entities`](crate::StoreConfig::max_entities).
    #[error("entity store is full ({max} entities)")]
    Full {
        /// The configured limit.
        max: u32,
    },
    /// The entity was despawned (its slot may since hold a newer generation), or the ID
    /// never came from this store.
    #[error("entity {0} is not alive")]
    Dead(Entity),
    /// A network ID that already names a live entity.
    #[error("network ID {net} already names entity {entity}")]
    NetIdInUse {
        /// The network ID.
        net: NetId,
        /// The entity holding it.
        entity: Entity,
    },
}

/// A refused operation that carried a payload: the reason plus the payload, handed back
/// so its owner can release it.
///
/// Render and animation payloads are handles to resources owned elsewhere (a GPU
/// instance, a scheduler slot) that dropping does not free. The store therefore never
/// drops one it refuses: [`EntityStore::set_render`](crate::EntityStore::set_render),
/// [`set_anim`](crate::EntityStore::set_anim) and
/// [`spawn_static`](crate::EntityStore::spawn_static) return it in this error.
///
/// ```
/// use tessera_entity::{EntityError, EntityStore, Transform};
///
/// let mut store: EntityStore<u32, ()> = EntityStore::new();
/// let e = store.spawn(Transform::IDENTITY)?;
/// store.despawn(e);
/// let refused = store.set_render(e, 7).unwrap_err();
/// assert_eq!(refused.error, EntityError::Dead(e));
/// assert_eq!(refused.value, 7); // release the instance it names
/// # Ok::<(), EntityError>(())
/// ```
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
#[error("{error}")]
#[non_exhaustive]
pub struct Rejected<T> {
    /// Why the operation was refused.
    pub error: EntityError,
    /// The payload the operation did not store.
    pub value: T,
}

impl<T> Rejected<T> {
    pub(crate) fn new(error: EntityError, value: T) -> Self {
        Self { error, value }
    }

    /// The reason and the payload.
    #[must_use]
    pub fn into_parts(self) -> (EntityError, T) {
        (self.error, self.value)
    }
}

impl<T> fmt::Debug for Rejected<T> {
    /// Shows the reason only, so payload types need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rejected")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
