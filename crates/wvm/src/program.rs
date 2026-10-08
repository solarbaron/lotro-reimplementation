// SPDX-License-Identifier: GPL-3.0-or-later
//! A loaded program: class library, validated code block and messages, plus the
//! resolution of function IDs and virtual slots (`wlib-bytecode.md` §6).

use std::sync::Arc;

use tessera_wstate::ClassLibrary;

use crate::check::{LibraryReport, check_library};
use crate::code::{Body, CodeBlock};
use crate::container::{CodeChunks, Messages, code_block};
use crate::error::{Result, WvmError};

/// A function ID: declaring class × 65,536 + function number (`wlib.md` §6.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(pub u32);

impl FunctionId {
    /// The declaring class, masked to 15 bits as the client does (`wlib.md` §6.4).
    #[must_use]
    pub fn class(self) -> u32 {
        (self.0 >> 16) & 0x7FFF
    }

    /// The function number.
    #[must_use]
    pub fn function(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }
}

/// Why a function could not be resolved: the "missing function" error of §6.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Missing {
    /// The class is undefined.
    Class(u32),
    /// The declaring class has no function of that number, or its index is −1.
    Function,
    /// The slot is outside the class's table.
    Slot(u32),
    /// The slot holds 0xFFFFFFFF.
    NoBody(u32),
}

/// The class library with its code and messages.
#[derive(Debug, Clone)]
pub struct Program {
    /// The class library (`wlib.md`).
    pub library: Arc<ClassLibrary>,
    /// The validated code block.
    pub code: CodeBlock,
    /// The messages (empty if the record has no chunk type 2).
    pub messages: Messages,
}

impl Program {
    /// Decodes a class-library record payload (DID 0x56000000) including its code.
    pub fn parse(record: &[u8]) -> Result<Self> {
        let library = Arc::new(ClassLibrary::parse(record)?);
        Self::with_library(library, record)
    }

    /// Decodes the code and messages of `record`, whose class table `library` already
    /// holds (for example from `tessera_wstate::ObjectStore::library`).
    pub fn with_library(library: Arc<ClassLibrary>, record: &[u8]) -> Result<Self> {
        let chunks = CodeChunks::find(record)?;
        let code = code_block(chunks.bytecode.ok_or(WvmError::NoCode)?)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(code.len())
            .map_err(|_| WvmError::Alloc { what: "code block" })?;
        owned.extend_from_slice(code);
        let starts = library
            .classes()
            .flat_map(|c| c.method_offsets.iter().copied());
        let code = CodeBlock::decode(owned, starts)?;
        let messages = match chunks.messages {
            Some(p) => Messages::parse(p)?,
            None => Messages::default(),
        };
        Ok(Self {
            library,
            code,
            messages,
        })
    }

    /// Checks V8–V13 and the sharing rule (see [`check_library`]).
    #[must_use]
    pub fn check(&self) -> LibraryReport {
        check_library(&self.code, &self.library, self.messages.len())
    }

    /// The body in slot `slot` of class `class` (virtual by slot, §6.4; also 0x14, §6.2).
    pub fn resolve_slot(&self, class: u32, slot: u32) -> Result<&Body, Missing> {
        let c = self.library.class(class).ok_or(Missing::Class(class))?;
        let off = *c
            .method_offsets
            .get(usize::try_from(slot).map_err(|_| Missing::Slot(slot))?)
            .ok_or(Missing::Slot(slot))?;
        if off == u32::MAX {
            return Err(Missing::NoBody(slot));
        }
        // Every method-table offset of a loaded program is a body start.
        self.code.body_at(off).ok_or(Missing::NoBody(slot))
    }

    /// The slot of function `id` in its declaring class: `method_indices[f]` (§6.4).
    pub fn function_slot(&self, id: FunctionId) -> Result<u32, Missing> {
        let p = self
            .library
            .class(id.class())
            .ok_or(Missing::Class(id.class()))?;
        let v = *p
            .method_indices
            .get(usize::from(id.function()))
            .ok_or(Missing::Function)?;
        if v == 0xFFFF {
            return Err(Missing::Function);
        }
        Ok(u32::from(v))
    }

    /// Static call by function ID (§6.4): the body in the declaring class.
    pub fn resolve_static(&self, id: FunctionId) -> Result<&Body, Missing> {
        let v = self.function_slot(id)?;
        self.resolve_slot(id.class(), v)
    }

    /// Virtual call by function ID on a receiver of class `receiver` (§6.4, §14.5).
    pub fn resolve_virtual(&self, receiver: u32, id: FunctionId) -> Result<&Body, Missing> {
        let v = self.function_slot(id)?;
        self.resolve_slot(receiver, v)
    }
}
