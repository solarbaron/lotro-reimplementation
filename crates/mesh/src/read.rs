// SPDX-License-Identifier: GPL-3.0-or-later
//! A bounds-checked little-endian cursor over a packed payload (`records.md` §1).
//!
//! Nothing in a mesh record is aligned (`mesh.md` §3), so every read is a byte copy.

use crate::error::{MeshError, Result};

#[derive(Debug, Clone)]
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// The unread bytes, without consuming them.
    pub(crate) fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn truncated(&self, what: &'static str, needed: u64) -> MeshError {
        MeshError::Truncated {
            what,
            offset: self.pos,
            needed,
            available: self.remaining(),
        }
    }

    /// Fails unless `needed` bytes remain.
    pub(crate) fn need(&self, what: &'static str, needed: u64) -> Result<()> {
        if needed > self.remaining() as u64 {
            Err(self.truncated(what, needed))
        } else {
            Ok(())
        }
    }

    /// Takes `n` bytes.
    pub(crate) fn take(&mut self, what: &'static str, n: u64) -> Result<&'a [u8]> {
        self.need(what, n)?;
        // `n` fits in usize: it is at most `remaining()`.
        let n = n as usize;
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N]> {
        let s = self.take(what, N as u64)?;
        let mut out = [0u8; N];
        out.copy_from_slice(s);
        Ok(out)
    }

    pub(crate) fn u8(&mut self, what: &'static str) -> Result<u8> {
        Ok(self.array::<1>(what)?[0])
    }

    pub(crate) fn u32(&mut self, what: &'static str) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn f32(&mut self, what: &'static str) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array(what)?))
    }

    pub(crate) fn vec3(&mut self, what: &'static str) -> Result<[f32; 3]> {
        Ok([self.f32(what)?, self.f32(what)?, self.f32(what)?])
    }

    /// Reads a count whose elements take `elem` bytes each, and checks that that many
    /// elements fit in the rest of the payload (the hostile-input bound of `mesh.md`
    /// §13). Returns the count; the caller then reads the elements.
    pub(crate) fn count(&mut self, what: &'static str, elem: u64) -> Result<u32> {
        let n = self.u32(what)?;
        self.need(what, u64::from(n) * elem)?;
        Ok(n)
    }
}

/// Little-endian helpers on a slice whose length the caller has already checked.
#[inline]
pub(crate) fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[inline]
pub(crate) fn vec2_at(b: &[u8], at: usize) -> [f32; 2] {
    [f32_at(b, at), f32_at(b, at + 4)]
}

#[inline]
pub(crate) fn vec3_at(b: &[u8], at: usize) -> [f32; 3] {
    [f32_at(b, at), f32_at(b, at + 4), f32_at(b, at + 8)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_bounds() {
        let data = [1u8, 0, 0, 0, 0, 0, 128, 63, 7];
        let mut c = Cursor::new(&data);
        assert_eq!(c.u32("a").unwrap(), 1);
        assert_eq!(c.f32("b").unwrap(), 1.0);
        assert_eq!(c.remaining(), 1);
        assert!(matches!(
            c.u32("c"),
            Err(MeshError::Truncated {
                needed: 4,
                available: 1,
                offset: 8,
                ..
            })
        ));
        assert_eq!(c.u8("d").unwrap(), 7);
        assert!(c.take("e", u64::MAX).is_err());
    }

    #[test]
    fn count_is_bounded_by_remaining() {
        let mut data = 1000u32.to_le_bytes().to_vec();
        data.extend_from_slice(&[0; 10]);
        assert!(Cursor::new(&data).count("x", 4).is_err());
        assert_eq!(Cursor::new(&data).count("x", 0).unwrap(), 1000);
    }
}
