// SPDX-License-Identifier: GPL-3.0-or-later
//! Error type of the crate.

use std::sync::Arc;

use tessera_assets::AssetError;
use tessera_formats::FormatError;
use tessera_physics::PhysicsError;

use crate::environment::EnvironmentError;
use crate::scenery::SceneryError;
use crate::water::WaterError;

/// Result alias of this crate.
pub type Result<T, E = WorldError> = std::result::Result<T, E>;

/// Longest piece of user input quoted back in a [`WorldError::Parse`] message.
pub(crate) const MAX_QUOTED_INPUT: usize = 64;

/// Everything that can go wrong in `tessera-world`.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum WorldError {
    /// A landblock coordinate outside 0–254 or a region outside 1–15
    /// (`coordinates.md` §4).
    #[error("invalid landblock: region {region}, x {x}, y {y}")]
    InvalidLandblock {
        /// Region number.
        region: u32,
        /// Landblock X.
        x: u32,
        /// Landblock Y.
        y: u32,
    },
    /// A position that is not finite or lies outside the region's landblock grid.
    #[error("position ({x}, {y}) is outside the landblock grid")]
    OutOfRange {
        /// Region-global x.
        x: f64,
        /// Region-global y.
        y: f64,
    },
    /// Text input (map coordinates, `/loc` output, spawn strings) did not parse.
    #[error("cannot parse {what} from {input:?}: {reason}")]
    Parse {
        /// What was being parsed.
        what: &'static str,
        /// The input, truncated to a few dozen characters.
        input: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// A configuration value was rejected.
    #[error("invalid configuration: {0}")]
    Config(&'static str),
    /// A record decoder failed.
    #[error("decoding {what} {did:#010x}: {source}")]
    Format {
        /// The record.
        did: u32,
        /// What kind of record.
        what: &'static str,
        /// The decoder's error.
        #[source]
        source: FormatError,
    },
    /// A record that must exist is missing.
    #[error("{what} {did:#010x} is missing")]
    Missing {
        /// The record.
        did: u32,
        /// What kind of record.
        what: &'static str,
    },
    /// A DID used as a landblock-bundle key is not a landblock-data DID `0x800RXXYY` of a
    /// valid landblock (nothing is missing; the key itself is wrong).
    #[error("{did:#010x} is not a landblock-data DID (bundle key)")]
    NotABundleKey {
        /// The DID given.
        did: u32,
    },
    /// The asset pipeline or record source failed.
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// A landblock-bundle frame (the transport between the bundle source and its decoder)
    /// is malformed.
    #[error("malformed bundle frame at byte {offset}: {reason}")]
    Frame {
        /// Byte offset of the problem.
        offset: usize,
        /// What is wrong.
        reason: &'static str,
    },
    /// The job was cancelled while decoding.
    #[error("cancelled")]
    Cancelled,
    /// Loading the inputs of scenery generation failed (`scenery.md` §4–§5, §9.2).
    #[error("scenery: {0}")]
    Scenery(#[source] Arc<SceneryError>),
    /// Loading water inputs failed (`water.md` §4–§7).
    #[error("water: {0}")]
    Water(#[source] Arc<WaterError>),
    /// Loading the environment (sky, day descriptions, game time) failed
    /// (`environment.md` §5–§6, §11).
    #[error("environment: {0}")]
    Environment(#[source] Arc<EnvironmentError>),
    /// Collision geometry was refused (`collision.md` §12.1; limits or hostile input).
    #[error("collision: {0}")]
    Physics(#[from] PhysicsError),
}

impl From<SceneryError> for WorldError {
    fn from(e: SceneryError) -> Self {
        Self::Scenery(Arc::new(e))
    }
}

impl From<WaterError> for WorldError {
    fn from(e: WaterError) -> Self {
        Self::Water(Arc::new(e))
    }
}

impl From<EnvironmentError> for WorldError {
    fn from(e: EnvironmentError) -> Self {
        Self::Environment(Arc::new(e))
    }
}

impl WorldError {
    /// Whether the failure depends on the data and so recurs on every attempt (a missing or
    /// broken record, a refused configuration), as opposed to the asset pipeline's state
    /// (full queue, shutdown, cancellation).
    #[must_use]
    pub fn is_data_error(&self) -> bool {
        match self {
            Self::Asset(e) => e.is_data_error(),
            Self::Cancelled => false,
            Self::Scenery(e) => e.is_data_error(),
            Self::Water(e) => e.is_data_error(),
            Self::Environment(e) => match &**e {
                EnvironmentError::World(w) => w.is_data_error(),
                _ => true,
            },
            _ => true,
        }
    }

    /// A parse error quoting at most [`MAX_QUOTED_INPUT`] characters of `input`.
    pub(crate) fn parse(what: &'static str, input: &str, reason: &'static str) -> Self {
        Self::Parse {
            what,
            input: input.chars().take(MAX_QUOTED_INPUT).collect(),
            reason,
        }
    }

    /// A decoder error for record `did`.
    pub(crate) fn format(did: impl Into<u32>, what: &'static str, source: FormatError) -> Self {
        Self::Format {
            did: did.into(),
            what,
            source,
        }
    }
}
