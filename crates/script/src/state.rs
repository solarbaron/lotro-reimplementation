// SPDX-License-Identifier: GPL-3.0-or-later
//! Script states: one sandboxed Lua 5.1 state built from a [`Profile`].
//!
//! Spec: `plugin-runtime.md` §5.1 (creation), §9 (memory and collector), §10 (limits),
//! §11 (protected calls and errors).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mlua::chunk::ChunkMode;
use mlua::{
    FromLuaMulti, Function, HookTriggers, IntoLuaMulti, Lua, LuaOptions, MultiValue, StdLib, Table,
    Value, VmState,
};

use crate::budget::Budget;
use crate::compat::{SharedRandom, install_conversions, install_random};
use crate::error::CoreError;
use crate::host::{CoreHost, ErrorKind, ErrorReport};
use crate::limits::Limits;
use crate::native::{SHIM_KEY, SHIM_SOURCE, classify, describe_error};
use crate::sandbox::{self, BYTECODE_SIGNATURE, SandboxSetup, TrampolineParts, load_internal};

/// How often (in VM instructions) the budget hook runs.
const HOOK_GRANULARITY: u32 = 1000;

/// Extra allocation allowed while the host's collector step runs (see
/// [`ScriptState::gc_step`]).
const GC_HEADROOM: usize = 1024 * 1024;

/// Collector work per host step: `lua_gc(LUA_GCSTEP, 20)` (`plugin-runtime.md` §9.2).
pub const GC_STEP_KBYTES: u32 = 20;

/// Which base functions a state has, beyond the ones no state ever has (`io`, `os`,
/// `package`, `debug`, `dofile`, `loadfile`, `require`, `module`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaseFunctions {
    /// Further global names to remove (for example `"newproxy"`, `"print"`, `"pcall"` or
    /// `"coroutine"`). They are removed after the sandbox is built, so hiding a function
    /// the sandbox itself uses (the guard's `pcall`, the patterns' `string`) is allowed and
    /// leaves the global nil.
    pub hidden: Vec<String>,
}

/// Which random generator `math.random` uses.
#[derive(Debug, Clone, Default)]
pub enum RandomSource {
    /// The interpreter's own (the platform C library's `rand`).
    #[default]
    Stock,
    /// The C runtime generator of the original client ([`crate::CrtRandom`]), possibly
    /// shared with other states.
    Shared(SharedRandom),
}

/// Which number and string conversions the library functions use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Conversions {
    /// The platform's (whatever the vendored interpreter does on this system).
    #[default]
    Stock,
    /// The original client's C runtime (`plugin-runtime.md` §4.4): see the `compat`
    /// module documentation for the replaced functions.
    Compatible,
}

/// Everything that decides how a state is built.
#[derive(Debug, Clone)]
pub struct Profile {
    /// State name used in reports and the script log.
    pub name: String,
    /// Resource limits.
    pub limits: Limits,
    /// Base functions.
    pub base: BaseFunctions,
    /// Prefix of the chunk names of host-compiled files: a file `rel` is named
    /// `@{chunk_prefix}{rel}`.
    pub chunk_prefix: String,
    /// The random generator.
    pub random: RandomSource,
    /// Number and string conversions.
    pub conversions: Conversions,
}

impl Profile {
    /// A bare profile: default limits, stock conversions and generator, no extra names
    /// hidden.
    #[must_use]
    pub fn bare(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            limits: Limits::default(),
            base: BaseFunctions::default(),
            chunk_prefix: String::new(),
            random: RandomSource::Stock,
            conversions: Conversions::Stock,
        }
    }
}

/// Registry name of the host-call trampoline (see [`call_protected`]).
const TRAMPOLINE_KEY: &str = "tessera.core.trampoline";

/// Call `f` (any callable value) in protected mode the way host calls are made
/// (`plugin-runtime.md` §11.1): no message handler, and an error value that is not a
/// string or a number becomes the empty message. Does **not** reset the instruction
/// budget (for calls made inside a running host call; use [`ScriptState::call`] to start
/// one). Works on any state built by [`ScriptState::new`].
///
/// # Errors
///
/// The Lua error the call raised.
pub fn call_protected<A: IntoLuaMulti, R: FromLuaMulti>(
    lua: &Lua,
    f: &Value,
    args: A,
) -> mlua::Result<R> {
    let trampoline: Function = lua.named_registry_value(TRAMPOLINE_KEY)?;
    let mut all = args.into_lua_multi(lua)?;
    all.push_front(f.clone());
    trampoline.call(all)
}

/// Compile the source text of a **file** (text only), as the stock 5.1 file loader does:
/// a leading UTF-8 byte-order mark is skipped (a Tessera deviation, `plugin-runtime.md`
/// §7), a first line starting with `#` is skipped up to (not including) its line feed so
/// line numbers stay right, a precompiled chunk is refused, and the text may not exceed
/// `max_bytes`. The chunk is named `chunk_name` as given (`@path` for files); its
/// environment is `env`, or the calling thread's current global table. (`loadstring` and
/// `load` do not skip a `#` line.)
///
/// # Errors
///
/// [`CoreError::BinaryChunk`], [`CoreError::SourceTooLarge`] or the syntax error.
pub fn compile_source(
    lua: &Lua,
    chunk_name: &str,
    bytes: &[u8],
    max_bytes: u64,
    env: Option<Table>,
) -> Result<Function, CoreError> {
    let display = chunk_name.trim_start_matches(['@', '=']);
    if bytes.len() as u64 > max_bytes {
        return Err(CoreError::SourceTooLarge {
            name: display.to_owned(),
            limit: max_bytes,
        });
    }
    let src = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if src.starts_with(BYTECODE_SIGNATURE) {
        return Err(CoreError::BinaryChunk(display.to_owned()));
    }
    let src = if src.first() == Some(&b'#') {
        &src[src.iter().position(|&b| b == b'\n').unwrap_or(src.len())..]
    } else {
        src
    };
    let chunk = lua.load(src).set_name(chunk_name).set_mode(ChunkMode::Text);
    let chunk = match env {
        Some(env) => chunk.set_environment(env),
        None => chunk,
    };
    Ok(chunk.into_function()?)
}

/// One sandboxed Lua 5.1 state.
///
/// Every host-to-Lua call should go through [`ScriptState::call`] (or start with
/// [`ScriptState::enter`]): it resets the instruction budget and turns the error value
/// into a plain message. All access happens on the thread that owns the state; the state
/// is `Send` so it can be moved there.
pub struct ScriptState {
    lua: Lua,
    name: String,
    budget: Arc<Budget>,
    peak: Arc<AtomicUsize>,
    host: Arc<dyn CoreHost>,
    limits: Limits,
    chunk_prefix: String,
    trampoline: Function,
    collect: Function,
    step_word: mlua::LuaString,
}

impl std::fmt::Debug for ScriptState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptState")
            .field("name", &self.name)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl Drop for ScriptState {
    /// Closing the state runs the finalizers of every proxy still alive; they get one fresh
    /// budget between them, so closing always terminates.
    fn drop(&mut self) {
        self.budget.reset();
    }
}

impl ScriptState {
    /// Create a state from `profile` (`plugin-runtime.md` §5.1 steps 1, 2 and 4): the base
    /// library with `coroutine`, `table`, `string` and `math`; the removals of
    /// `lua-plugin-api.md` §3.2; text-only `load`/`loadstring`; the instruction budget and
    /// its guard; the bounded patterns; the metered `newproxy`; the profile's conversions
    /// and generator. Nothing else: no host API is installed.
    ///
    /// # Errors
    ///
    /// [`CoreError::Lua`] if the state cannot be created.
    pub fn new(profile: Profile, host: Arc<dyn CoreHost>) -> Result<Self, CoreError> {
        let Profile {
            name,
            limits,
            base,
            chunk_prefix,
            random,
            conversions,
        } = profile;
        let lua = Lua::new_with(
            StdLib::TABLE | StdLib::STRING | StdLib::MATH,
            LuaOptions::default(),
        )?;
        if limits.memory_bytes > 0 {
            lua.set_memory_limit(limits.memory_bytes)?;
        }
        let budget = Arc::new(Budget::new(limits.instructions_per_call));
        let peak = Arc::new(AtomicUsize::new(0));
        if limits.instructions_per_call.is_some() {
            let b = Arc::clone(&budget);
            let pk = Arc::clone(&peak);
            lua.set_global_hook(
                HookTriggers::new().every_nth_instruction(HOOK_GRANULARITY),
                move |lua, _| {
                    // The memory high-water mark, sampled while code runs (§9.4).
                    pk.fetch_max(lua.used_memory(), Ordering::Relaxed);
                    b.charge(u64::from(HOOK_GRANULARITY))
                        .map(|()| VmState::Continue)
                },
            )?;
        }
        let shim: Function = load_internal(&lua, SHIM_SOURCE)?.call(())?;
        lua.set_named_registry_value(SHIM_KEY, shim)?;

        let g = lua.globals();
        let stock_pcall: Function = g.raw_get("pcall")?;
        let stock_type: Function = g.raw_get("type")?;
        let stock_error: Function = g.raw_get("error")?;
        let collect: Function = g.raw_get("collectgarbage")?;
        let tostring = match conversions {
            Conversions::Compatible => install_conversions(&lua, &g)?,
            Conversions::Stock => g.raw_get("tostring")?,
        };
        if let RandomSource::Shared(rng) = &random {
            install_random(&lua, &g, rng)?;
        }
        sandbox::install(
            &lua,
            SandboxSetup {
                name: &name,
                host: &host,
                budget: &budget,
                max_source: limits.max_source_bytes,
                max_output: limits.memory_bytes,
                hidden: &base.hidden,
                tostring: tostring.clone(),
            },
        )?;
        let step_word = lua.create_string("step")?;
        let trampoline = sandbox::trampoline(
            &lua,
            TrampolineParts {
                pcall: stock_pcall,
                type_fn: stock_type,
                error: stock_error,
                tostring,
            },
        )?;
        lua.set_named_registry_value(TRAMPOLINE_KEY, trampoline.clone())?;
        Ok(Self {
            lua,
            name,
            budget,
            peak,
            host,
            limits,
            chunk_prefix,
            trampoline,
            collect,
            step_word,
        })
    }

    /// The Lua state.
    #[must_use]
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// The state's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The limits the state was built with.
    #[must_use]
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// The per-call instruction budget.
    #[must_use]
    pub fn budget(&self) -> &Arc<Budget> {
        &self.budget
    }

    /// The host.
    #[must_use]
    pub fn host(&self) -> &Arc<dyn CoreHost> {
        &self.host
    }

    /// Start a host call: reset the instruction budget.
    pub fn enter(&self) {
        self.budget.reset();
    }

    /// Bytes the state has requested from its allocator.
    #[must_use]
    pub fn used_memory(&self) -> usize {
        self.lua.used_memory()
    }

    /// The highest [`Self::used_memory`] seen since the state was created: sampled by the
    /// instruction hook while code runs (every 1000 VM instructions) and after every host
    /// call and collector step, so it can miss a short spike between two samples
    /// (`plugin-runtime.md` §9.4 asks for the peak per apartment).
    #[must_use]
    pub fn peak_memory(&self) -> usize {
        self.sample_memory();
        self.peak.load(Ordering::Relaxed)
    }

    fn sample_memory(&self) {
        self.peak
            .fetch_max(self.lua.used_memory(), Ordering::Relaxed);
    }

    /// Classify `e`, raised by a call of this state, by what failed (see
    /// [`crate::classify`]).
    #[must_use]
    pub fn classify(&self, e: &mlua::Error) -> ErrorKind {
        classify(e, &self.budget)
    }

    /// Call `f` (any callable value) as one host call: reset the budget, call it in
    /// protected mode without a message handler, and turn a non-string, non-number error
    /// value into the empty message (`plugin-runtime.md` §11.1).
    ///
    /// # Errors
    ///
    /// The Lua error the call raised.
    pub fn call<A: IntoLuaMulti, R: FromLuaMulti>(&self, f: &Value, args: A) -> mlua::Result<R> {
        self.enter();
        let mut all = args.into_lua_multi(&self.lua)?;
        all.push_front(f.clone());
        let r = self.trampoline.call(all);
        self.sample_memory();
        r
    }

    /// [`Self::call`] for a function.
    ///
    /// # Errors
    ///
    /// The Lua error the call raised.
    pub fn call_function<A: IntoLuaMulti, R: FromLuaMulti>(
        &self,
        f: &Function,
        args: A,
    ) -> mlua::Result<R> {
        self.call(&Value::Function(f.clone()), args)
    }

    /// The same as [`Self::call`] without resetting the budget (for calls nested inside a
    /// host call that already has one).
    ///
    /// # Errors
    ///
    /// The Lua error the call raised.
    pub fn call_nested<A: IntoLuaMulti, R: FromLuaMulti>(
        &self,
        f: &Value,
        args: A,
    ) -> mlua::Result<R> {
        let mut all = args.into_lua_multi(&self.lua)?;
        all.push_front(f.clone());
        let r = self.trampoline.call(all);
        self.sample_memory();
        r
    }

    /// Compile host-supplied source text (`plugin-runtime.md` §7, text only): a leading
    /// UTF-8 byte-order mark is skipped, a precompiled chunk is refused, and the text may
    /// not exceed [`Limits::max_source_bytes`]. The chunk is named `@{prefix}{rel_name}`;
    /// its environment is `env`, or the current global table.
    ///
    /// # Errors
    ///
    /// [`CoreError::BinaryChunk`], [`CoreError::SourceTooLarge`] or the syntax error.
    pub fn compile_file(
        &self,
        rel_name: &str,
        bytes: &[u8],
        env: Option<Table>,
    ) -> Result<Function, CoreError> {
        compile_source(
            &self.lua,
            &format!("@{}{rel_name}", self.chunk_prefix),
            bytes,
            self.limits.max_source_bytes,
            env,
        )
    }

    /// The host's per-tick collector step, `lua_gc(LUA_GCSTEP, 20)`
    /// (`plugin-runtime.md` §9.2), with a fresh budget for the finalizers it runs. Returns
    /// whether the step finished a collection cycle. A finalizer error is reported to the
    /// host and does not stop the step from being retried next tick.
    pub fn gc_step(&self) -> bool {
        self.enter();
        // The step itself must not be refused by the cap it is there to relieve: a state
        // at its cap gets a little headroom for the call (finalizers that run in the step
        // can use at most that much more).
        let cap = self.limits.memory_bytes;
        if cap > 0 {
            let _ = self.lua.set_memory_limit(cap.saturating_add(GC_HEADROOM));
        }
        let result = self
            .collect
            .call::<Value>((self.step_word.clone(), f64::from(GC_STEP_KBYTES)));
        self.sample_memory();
        if cap > 0 {
            let _ = self.lua.set_memory_limit(cap);
        }
        match result {
            Ok(v) => matches!(v, Value::Boolean(true)),
            Err(e) => {
                self.report(None, "running a finalizer", &e);
                false
            }
        }
    }

    /// Build the report of `e` for this state.
    #[must_use]
    pub fn error_report(
        &self,
        source: Option<&str>,
        context: &str,
        e: &mlua::Error,
    ) -> ErrorReport {
        let (message, traceback) = describe_error(e);
        let kind = self.classify(e);
        ErrorReport {
            source: source.map(str::to_owned),
            state: self.name.clone(),
            context: context.to_owned(),
            message,
            traceback,
            kind,
        }
    }

    /// Report `e` to the core host.
    pub fn report(&self, source: Option<&str>, context: &str, e: &mlua::Error) {
        self.host
            .report_error(&self.error_report(source, context, e));
    }

    /// Evaluate `src` (text) as one host call with the current global table; for tests
    /// and tools.
    ///
    /// # Errors
    ///
    /// Compile or run-time errors.
    pub fn eval<R: FromLuaMulti>(&self, src: &str) -> mlua::Result<R> {
        let f = self
            .lua
            .load(src)
            .set_name("=eval")
            .set_mode(ChunkMode::Text)
            .into_function()?;
        self.call_function(&f, MultiValue::new())
    }
}
