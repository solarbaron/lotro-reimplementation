// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded allocation helpers shared by the decoders.

use tessera_formats::Reader;

use crate::error::{FormatContext, Result, WStateError};

/// Returns an empty vector with room for `count` elements of type `T`, after checking that
/// `count` elements of at least `min_size` stored bytes each fit in the remaining input and
/// charging the vector's heap size to the reader's memory budget.
///
/// Because the count is checked against the input and the memory against the budget (a
/// fixed multiple of the input length), a hostile count can never reserve more memory than
/// the input justifies.
pub(crate) fn bounded_vec<T>(
    r: &mut Reader<'_>,
    count: u32,
    min_size: usize,
    what: &'static str,
) -> Result<Vec<T>> {
    r.ensure_elements(count, min_size).ctx(what)?;
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    r.charge(count.saturating_mul(std::mem::size_of::<T>()))
        .ctx(what)?;
    let mut v = Vec::new();
    v.try_reserve_exact(count).map_err(|_| {
        WStateError::malformed(what, r.position(), "allocation failed for element count")
    })?;
    Ok(v)
}

/// Charges `bytes` of retained heap to the reader's budget.
pub(crate) fn charge(r: &mut Reader<'_>, bytes: usize, what: &'static str) -> Result<()> {
    r.charge(bytes).ctx(what)
}

/// Reads a `u32` count followed by `count` elements read by `f`.
pub(crate) fn counted<T>(
    r: &mut Reader<'_>,
    min_size: usize,
    what: &'static str,
    mut f: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<Vec<T>> {
    let count = r.u32().ctx(what)?;
    let mut v = bounded_vec(r, count, min_size, what)?;
    for _ in 0..count {
        v.push(f(r)?);
    }
    Ok(v)
}
