// SPDX-License-Identifier: GPL-3.0-or-later
//! Panic-free little-endian field access.

/// Reads a little-endian `u32` at `off`; returns 0 if the slice is too short.
///
/// Callers check lengths up front; the fallback only exists so that no code path can panic.
#[inline]
pub(crate) fn u32_at(b: &[u8], off: usize) -> u32 {
    b.get(off..off.saturating_add(4))
        .and_then(|s| <[u8; 4]>::try_from(s).ok())
        .map_or(0, u32::from_le_bytes)
}

/// Reads a little-endian `u16` at `off`; returns 0 if the slice is too short.
#[inline]
pub(crate) fn u16_at(b: &[u8], off: usize) -> u16 {
    b.get(off..off.saturating_add(2))
        .and_then(|s| <[u8; 2]>::try_from(s).ok())
        .map_or(0, u16::from_le_bytes)
}

/// Copies `N` bytes at `off`; returns zeros if the slice is too short.
#[inline]
pub(crate) fn array_at<const N: usize>(b: &[u8], off: usize) -> [u8; N] {
    b.get(off..off.saturating_add(N))
        .and_then(|s| <[u8; N]>::try_from(s).ok())
        .unwrap_or([0; N])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_slices_do_not_panic() {
        assert_eq!(u32_at(&[1, 2, 3], 0), 0);
        assert_eq!(u32_at(&[1, 0, 0, 0], 0), 1);
        assert_eq!(u32_at(&[1, 0, 0, 0], usize::MAX), 0);
        assert_eq!(u16_at(&[2, 1], 0), 0x0102);
        assert_eq!(array_at::<2>(&[9], 0), [0, 0]);
    }
}
