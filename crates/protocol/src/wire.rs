// SPDX-License-Identifier: GPL-3.0-or-later
//! Primitive types of §5.1: a bounds-checked little-endian reader and a writer.

use crate::error::{ProtocolError, Result};
use crate::text;

/// How a received `str` field is checked (§5.2).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextRule {
    /// Field name for errors.
    pub field: &'static str,
    /// Smallest length the decoder accepts.
    pub min: usize,
    /// Largest length; above it is always a violation.
    pub max: usize,
    /// Replace control characters by spaces on receipt (server → client classes). The
    /// encoder refuses control characters in such fields.
    pub sanitise: bool,
}

impl TextRule {
    /// A server-to-client class: length `min..=max`, control characters sanitised.
    pub const fn sanitised(field: &'static str, min: usize, max: usize) -> Self {
        Self {
            field,
            min,
            max,
            sanitise: true,
        }
    }

    /// A client-to-server class whose content the server checks itself: only the maximum is
    /// a wire rule; the text is kept as sent.
    pub const fn raw(field: &'static str, max: usize) -> Self {
        Self {
            field,
            min: 0,
            max,
            sanitise: false,
        }
    }

    /// A field with a length range and no content rule.
    pub const fn plain(field: &'static str, min: usize, max: usize) -> Self {
        Self {
            field,
            min,
            max,
            sanitise: false,
        }
    }
}

/// A cursor over hostile bytes. Every read checks the remaining length first.
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len()
    }

    pub fn rest(&self) -> &'a [u8] {
        self.buf
    }

    pub fn finish(&self) -> Result<()> {
        if self.buf.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::TrailingBytes(self.buf.len()))
        }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if n > self.buf.len() {
            return Err(ProtocolError::Truncated);
        }
        let (head, tail) = self.buf.split_at(n);
        self.buf = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let b = self.bytes(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(b);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self, field: &'static str) -> Result<f32> {
        let v = f32::from_le_bytes(self.array()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(ProtocolError::NonFinite(field))
        }
    }

    pub fn f64(&mut self, field: &'static str) -> Result<f64> {
        let v = f64::from_le_bytes(self.array()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(ProtocolError::NonFinite(field))
        }
    }

    pub fn bool(&mut self, field: &'static str) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ProtocolError::InvalidBool { field, value }),
        }
    }

    /// Reads an `opt` tag: true when a value follows.
    pub fn opt(&mut self, field: &'static str) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ProtocolError::InvalidOption { field, value }),
        }
    }

    /// Reads a `str` under `rule`. The length is checked against the maximum before the
    /// bytes are read (§5.2).
    pub fn str(&mut self, rule: TextRule) -> Result<String> {
        let len = usize::from(self.u16()?);
        if len > rule.max || len < rule.min {
            return Err(ProtocolError::StringLength {
                field: rule.field,
                len,
                min: rule.min,
                max: rule.max,
            });
        }
        let raw = self.bytes(len)?;
        let s = std::str::from_utf8(raw).map_err(|_| ProtocolError::InvalidUtf8(rule.field))?;
        if rule.sanitise {
            Ok(text::sanitise(s).into_owned())
        } else {
            Ok(s.to_owned())
        }
    }
}

/// Appends little-endian primitives to a buffer.
#[derive(Debug)]
pub(crate) struct Writer<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    pub fn new(out: &'a mut Vec<u8>) -> Self {
        Self { out }
    }

    pub fn len(&self) -> usize {
        self.out.len()
    }

    pub fn u8(&mut self, v: u8) {
        self.out.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn u64(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn i64(&mut self, v: i64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    pub fn f32(&mut self, field: &'static str, v: f32) -> Result<()> {
        if !v.is_finite() {
            return Err(ProtocolError::NonFinite(field));
        }
        self.out.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    pub fn f64(&mut self, field: &'static str, v: f64) -> Result<()> {
        if !v.is_finite() {
            return Err(ProtocolError::NonFinite(field));
        }
        self.out.extend_from_slice(&v.to_le_bytes());
        Ok(())
    }

    pub fn bool(&mut self, v: bool) {
        self.out.push(u8::from(v));
    }

    pub fn str(&mut self, rule: TextRule, s: &str) -> Result<()> {
        let len = s.len();
        if len > rule.max || len < rule.min {
            return Err(ProtocolError::StringLength {
                field: rule.field,
                len,
                min: rule.min,
                max: rule.max,
            });
        }
        if rule.sanitise && text::has_control(s) {
            return Err(ProtocolError::ControlCharacter(rule.field));
        }
        // len <= max <= 1,024 here, so it fits a u16.
        let len16 = u16::try_from(len).map_err(|_| ProtocolError::StringLength {
            field: rule.field,
            len,
            min: rule.min,
            max: rule.max,
        })?;
        self.u16(len16);
        self.out.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
