// SPDX-License-Identifier: GPL-3.0-or-later
//! Common record behaviour: the DID prologue every record starts with (spec §1.6).

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::Reader;

/// A record type decoded from a complete subfile payload.
pub trait Record<'a>: Sized {
    /// Human-readable record type name, used in errors.
    const NAME: &'static str;

    /// Decodes a complete payload. The DID prologue must have the right type prefix.
    fn parse(data: &'a [u8]) -> Result<Self>;

    /// The DID stored in the record prologue.
    fn did(&self) -> Did;

    /// Decodes a payload and checks that its prologue matches the DID that was looked up.
    ///
    /// `expected` accepts a [`Did`] or a raw `u32` (the archive layer's DID type).
    fn parse_expecting(data: &'a [u8], expected: impl Into<Did>) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data)?;
        if rec.did() != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did().0,
            });
        }
        Ok(rec)
    }
}

/// Reads the DID prologue and checks its type prefix.
pub(crate) fn read_prologue(
    r: &mut Reader<'_>,
    expected: &'static str,
    accept: impl Fn(DidKind) -> bool,
) -> Result<Did> {
    let did = r.did()?;
    if !accept(did.kind()) {
        return Err(FormatError::WrongRecordType {
            did: did.0,
            expected,
        });
    }
    Ok(did)
}
