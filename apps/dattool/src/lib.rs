// SPDX-License-Identifier: GPL-3.0-or-later
//! Library part of `tessera-dattool`: the snapshot file format.
//!
//! The command-line tool lives in `main.rs`. This library holds only the parts that parse
//! untrusted input on their own, so that `fuzz/` can drive them: [`snapfile::decode`] and
//! [`snapfile::decode_from`], and the argument readers [`hexarg::parse_did`] and
//! [`hexarg::parse_prefix`].

pub mod hexarg;
pub mod snapfile;
