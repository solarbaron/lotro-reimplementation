// SPDX-License-Identifier: GPL-3.0-or-later
//! The hook through which built-in packages (the clean-room class helper, decision 0016)
//! take part in `import`.
//!
//! In **built-in mode** (no user copy of the sample package, `plugin-class-helper.md`
//! §5.1) the import resolver treats each name a [`BuiltinPackages`] provider serves as
//! having a disk part supplied by the provider (§5.3); the user's installed copy, when
//! present, takes precedence for the whole life of the apartment (user-copy mode, §5.2).
//!
//! Spec: `plugin-class-helper.md` §5.1–§5.4; `lua-plugin-api.md` §7.2.

use mlua::{Lua, Table};

/// The shape a built-in name has in the resolver (`lua-plugin-api.md` §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageShape {
    /// A folder package: its namespace is the whole name.
    Folder,
    /// A file package: its namespace is the name without its last component.
    File,
}

/// What a provider's install sees.
pub struct PackageContext<'a> {
    pub(crate) lua: &'a Lua,
    pub(crate) namespace: Table,
    pub(crate) globals: Table,
    pub(crate) import: &'a dyn Fn(&str) -> mlua::Result<()>,
}

impl std::fmt::Debug for PackageContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageContext").finish_non_exhaustive()
    }
}

impl PackageContext<'_> {
    /// The apartment's Lua state.
    #[must_use]
    pub fn lua(&self) -> &Lua {
        self.lua
    }

    /// The package's namespace table `T` (the thread's global table during the install).
    #[must_use]
    pub fn namespace(&self) -> &Table {
        &self.namespace
    }

    /// The apartment's real global table.
    #[must_use]
    pub fn globals(&self) -> &Table {
        &self.globals
    }

    /// `import(name)` with the normal run-once rule (the "imports first" of
    /// `plugin-class-helper.md` §5.3).
    ///
    /// # Errors
    ///
    /// The error the import raises.
    pub fn import(&self, name: &str) -> mlua::Result<()> {
        (self.import)(name)
    }

    /// Compile `source` (text only) with the namespace as environment and run it.
    ///
    /// # Errors
    ///
    /// Compile or run-time errors.
    pub fn run_source(&self, chunk_name: &str, source: &[u8]) -> mlua::Result<()> {
        let f = tessera_script::compile_source(
            self.lua,
            chunk_name,
            source,
            u64::MAX,
            Some(self.namespace.clone()),
        )
        .map_err(|e| match e {
            tessera_script::CoreError::Lua(e) => e,
            other => tessera_script::host_error(other.to_string()),
        })?;
        tessera_script::call_protected(self.lua, &mlua::Value::Function(f), ())
    }
}

/// A provider of built-in packages.
///
/// Called on the thread that runs plugin code, while the import runs inside a host call
/// (its instruction budget applies).
pub trait BuiltinPackages: Send + Sync + 'static {
    /// The shape of the built-in disk part of `name`, matched ASCII case-insensitively, if
    /// this provider serves it.
    fn shape(&self, name: &str) -> Option<PackageShape>;

    /// Install `name` (as passed to `import`) into `ctx.namespace()`, in place of loading
    /// and running a disk chunk (`plugin-class-helper.md` §5.3 "Namespace steps").
    ///
    /// # Errors
    ///
    /// A Lua error makes the import fail (and un-records the name).
    fn install(&self, ctx: &PackageContext<'_>, name: &str) -> mlua::Result<()>;
}
