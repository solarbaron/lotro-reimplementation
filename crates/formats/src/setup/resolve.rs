// SPDX-License-Identifier: GPL-3.0-or-later
//! Following an object's chain to its setup (`setup.md` §3.1).

use super::{
    ENTITY_PREFIX, EntityHeader, SETUP_PREFIX, Setup, TEMPLATE_PREFIX, TemplateLink, VisualTemplate,
};
use crate::did::Did;
use crate::error::FormatError;
use crate::record::Record;

/// Most links the resolvers follow before giving up (the spec's suggested bound).
///
/// Links are counted as [`SetupChain::hops`] counts them: every step from one record to the
/// next, including entity → template and template → setup. Real chains use at most six
/// (up to four entity → entity, one entity → template, at most one template → template,
/// one template → setup; SI6).
pub const MAX_CHAIN_HOPS: usize = 16;

/// Why a chain could not be resolved: the caller's fetch function failed, or the data is
/// broken.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError<E> {
    /// The fetch function failed for `did` (I/O, decompression, archive corruption). Kept
    /// apart from [`FormatError::DanglingReference`], which means the record does not
    /// exist.
    #[error("reading record {did:#010x} failed: {source}")]
    Fetch {
        /// The record that could not be read.
        did: u32,
        /// The fetch function's error.
        #[source]
        source: E,
    },
    /// The chain or one of its records is broken (see [`resolve_setup`]).
    #[error(transparent)]
    Format(#[from] FormatError),
}

impl<E> ResolveError<E> {
    /// The data error, or `None` for a fetch failure.
    #[must_use]
    pub fn format_error(&self) -> Option<&FormatError> {
        match self {
            Self::Format(e) => Some(e),
            Self::Fetch { .. } => None,
        }
    }
}

/// A resolved chain: every record visited, and the decoded setup at its end.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupChain {
    /// The DIDs visited, starting DID first and the setup's DID last.
    pub path: Vec<Did>,
    /// The setup the chain ends at.
    pub setup: Setup,
}

impl SetupChain {
    /// The DID resolution started from.
    #[must_use]
    pub fn start(&self) -> Did {
        self.path.first().copied().unwrap_or(self.setup.did)
    }

    /// Number of links followed (0 when the start was the setup itself).
    #[must_use]
    pub fn hops(&self) -> usize {
        self.path.len().saturating_sub(1)
    }

    /// The part meshes (0x06 DIDs) to draw, in order, in the setup's own frame (§7).
    #[must_use]
    pub fn parts(&self) -> &[Did] {
        &self.setup.parts
    }
}

/// Follows `start` — a 0x47 entity descriptor, a 0x1F visual template or a 0x04 setup — to
/// its setup's DID (§3.1) **without reading the setup**, for callers that cache setups by
/// DID. Returns the DIDs visited, `start` first and the 0x04 DID last; when `start` is a
/// setup the path is just `[start]` and nothing is fetched. Whether the setup exists is not
/// checked.
///
/// `fetch` returns a record's complete, decompressed payload, `Ok(None)` when the record
/// does not exist, or `Err` when reading it failed. Entity descriptors are read only up to
/// their template link ([`EntityHeader`]), so no property dictionary is needed. Every
/// record's own DID is checked against the DID that led to it.
///
/// Rules: an entity's `template` may name a 0x1F template or a parent 0x47 entity; a
/// template's setup section must name a 0x04 setup, and a template without one follows its
/// `parent`, which must be a 0x1F template.
///
/// # Errors
///
/// - [`ResolveError::Fetch`] when `fetch` fails;
/// - [`FormatError::DanglingReference`] when a named record does not exist (`from` is 0
///   when `start` itself is missing);
/// - [`FormatError::ReferenceChain`] for a link to an unexpected prefix, a template with
///   neither a setup nor a parent, a cycle, or more than [`MAX_CHAIN_HOPS`] links;
/// - [`FormatError::InRecord`] wrapping the decode error of a record on the chain.
pub fn resolve_setup_did<B: AsRef<[u8]>, E>(
    start: Did,
    mut fetch: impl FnMut(Did) -> Result<Option<B>, E>,
) -> Result<Vec<Did>, ResolveError<E>> {
    let mut path: Vec<Did> = Vec::new();
    let mut current = start;
    loop {
        if path.contains(&current) {
            return Err(chain_error(current, "cycle").into());
        }
        if path.len() > MAX_CHAIN_HOPS {
            return Err(chain_error(current, format!("more than {MAX_CHAIN_HOPS} links")).into());
        }
        if !matches!(
            current.prefix(),
            SETUP_PREFIX | ENTITY_PREFIX | TEMPLATE_PREFIX
        ) {
            return Err(chain_error(
                current,
                "not an entity descriptor, visual template or setup",
            )
            .into());
        }
        path.push(current);
        if current.prefix() == SETUP_PREFIX {
            return Ok(path);
        }
        let bytes = fetch_existing(&mut fetch, &path)?;
        let bytes = bytes.as_ref();
        let in_record = |e: FormatError| FormatError::InRecord {
            did: current.0,
            source: Box::new(e),
        };
        current = if current.prefix() == ENTITY_PREFIX {
            let h = EntityHeader::parse_expecting(bytes, current).map_err(in_record)?;
            match h.template_link() {
                TemplateLink::Template(d) | TemplateLink::Entity(d) => d,
                TemplateLink::Other(d) => {
                    return Err(chain_error(
                        current,
                        format!("template link to {d} is neither 0x1F nor 0x47"),
                    )
                    .into());
                }
            }
        } else {
            let t = VisualTemplate::parse_expecting(bytes, current).map_err(in_record)?;
            match (t.setup(), t.parent) {
                (Some(s), _) if s.prefix() == SETUP_PREFIX => s,
                (Some(s), _) => {
                    return Err(chain_error(
                        current,
                        format!("setup section names {s}, not a 0x04 setup"),
                    )
                    .into());
                }
                (None, Some(p)) if p.prefix() == TEMPLATE_PREFIX => p,
                (None, Some(p)) => {
                    return Err(
                        chain_error(current, format!("parent {p} is not a 0x1F template")).into(),
                    );
                }
                (None, None) => {
                    return Err(
                        chain_error(current, "template has neither setup nor parent").into(),
                    );
                }
            }
        };
    }
}

/// [`resolve_setup_did`], then fetches and decodes the setup the chain ends at.
///
/// # Errors
///
/// Those of [`resolve_setup_did`]; a missing setup is a
/// [`FormatError::DanglingReference`] and a setup that fails to decode a
/// [`FormatError::InRecord`].
pub fn resolve_setup<B: AsRef<[u8]>, E>(
    start: Did,
    mut fetch: impl FnMut(Did) -> Result<Option<B>, E>,
) -> Result<SetupChain, ResolveError<E>> {
    let path = resolve_setup_did(start, &mut fetch)?;
    let bytes = fetch_existing(&mut fetch, &path)?;
    let did = *path.last().unwrap_or(&start);
    let setup = Setup::parse_expecting(bytes.as_ref(), did).map_err(|e| FormatError::InRecord {
        did: did.0,
        source: Box::new(e),
    })?;
    Ok(SetupChain { path, setup })
}

/// Fetches the last DID of `path`, mapping absence to a dangling reference from the DID
/// before it (0 when there is none).
fn fetch_existing<B, E>(
    fetch: &mut impl FnMut(Did) -> Result<Option<B>, E>,
    path: &[Did],
) -> Result<B, ResolveError<E>> {
    let (to, from) = match path {
        [.., from, to] => (*to, from.0),
        [to] => (*to, 0),
        [] => (Did(0), 0),
    };
    match fetch(to) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(FormatError::DanglingReference { from, to: to.0 }.into()),
        Err(source) => Err(ResolveError::Fetch { did: to.0, source }),
    }
}

fn chain_error(did: Did, reason: impl Into<std::borrow::Cow<'static, str>>) -> FormatError {
    FormatError::ReferenceChain {
        did: did.0,
        reason: reason.into(),
    }
}
