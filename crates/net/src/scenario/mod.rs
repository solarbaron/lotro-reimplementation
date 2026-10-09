// SPDX-License-Identifier: GPL-3.0-or-later
//! The scenario feed: scripted domain events for the sandbox backend
//! (`docs/specs/scenario-feed.md`, decision 0024 §4).
//!
//! A scenario is a project-defined text file that scripts what a world source would tell
//! the client (actors, items, vitals, casts, effects, dialogue pages, quest states, vendor
//! lists, loot, markers, chat, fellowship and trade, progression values) as timed steps and
//! as scripted answers to client commands. The pipeline:
//!
//! 1. **Parse** with [`Scenario::parse`]: a hostile-input-safe parser with every limit of
//!    the spec; errors are [`ScenarioError`]s with line and column.
//! 2. **Resolve** with [`Scenario::resolve`] and a [`SelectorResolver`]: the client
//!    evaluates selectors against its own install (or uses [`EmptyResolver`]); this crate
//!    never decodes game data. The result is a [`ResolvedScenario`] holding record
//!    identifiers ([`DataId`](crate::DataId)s), scripted values and literal text only.
//! 3. **Play** it with the sandbox backend:
//!    [`SandboxConfig::scenario`](crate::sandbox::SandboxConfig::scenario). The feed runs
//!    on a virtual clock in ticks; [`Session`](crate::Session) works unchanged.
//! 4. **Log** with [`HeadlessDriver`] (headless runs) or [`format_event`] /
//!    [`format_command`] / [`LogWriter`] (the canonical log of §10).
//!
//! ```
//! use std::sync::Arc;
//! use tessera_net::scenario::{EmptyResolver, HeadlessDriver, Scenario};
//! use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
//! use tessera_net::Session;
//!
//! let text = "tessera-scenario 1\nplayer \"Testa\"\nat 0s system \"Hello\"\n";
//! let scenario = Scenario::parse(text.as_bytes())?;
//! let resolved = Arc::new(scenario.resolve(&mut EmptyResolver)?);
//! let config = SandboxConfig { scenario: Some(Arc::clone(&resolved)), ..SandboxConfig::default() };
//! let session = Session::new(SandboxBackend::new(config)?);
//! let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
//! let outcome = driver.run();
//! assert_eq!(outcome.exit_code(), 0);
//! let log = String::from_utf8(driver.into_log().unwrap()).unwrap();
//! assert!(log.contains("0.000 ChatMessage channel=system sender=none text=\"Hello\""));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod ast;
mod driver;
mod error;
pub(crate) mod feed;
mod forms;
mod lex;
mod log;
mod parse;
mod resolve;
mod select;
mod verbs;

use std::sync::Arc;

pub use driver::{DriverFrame, DriverOutcome, HeadlessDriver};
pub use error::{
    Limit, ResolveError, RuntimeErrorKind, ScenarioError, ScenarioErrorKind, ScenarioRuntimeError,
};
pub use feed::{FinishReason, ScenarioCounters, ScenarioStatus};
pub use lex::MAX_FILE_BYTES;
pub use log::{
    ACTOR_ID_BASE, EndCounters, EndReason, LogContext, LogTime, LogWriter, format_command,
    format_end, format_event, quote,
};
pub use resolve::{MAX_CANDIDATES, ResolveNote, ResolvedScenario, ShortfallNote};
pub use select::{
    Alt, Attribute, Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, Family, Filter,
    FilterValue, LabelContext, ResolvedRecord, SelectorQuery, SelectorResolver, SplitMix64,
};

/// A parsed and statically checked scenario (`scenario-feed.md` §4–§7, §11): selectors
/// still unresolved.
#[derive(Debug, Clone)]
pub struct Scenario {
    prog: Arc<ast::Program>,
}

impl Scenario {
    /// Parses a scenario file (at most [`MAX_FILE_BYTES`]) and runs the static checks.
    ///
    /// # Errors
    ///
    /// The first [`ScenarioError`] in file order (§4.5 error order).
    pub fn parse(bytes: &[u8]) -> Result<Self, ScenarioError> {
        Ok(Self {
            prog: Arc::new(parse::parse(bytes)?),
        })
    }

    /// Resolves the selectors with `resolver`, in declaration order (§8.6).
    ///
    /// # Errors
    ///
    /// [`ResolveError`]: the install could not be read, the scenario requires an install
    /// and there is none, or the resolved records exceed 1,048,576.
    pub fn resolve(
        &self,
        resolver: &mut dyn SelectorResolver,
    ) -> Result<ResolvedScenario, ResolveError> {
        resolve::resolve(&self.prog, resolver)
    }

    /// The scenario's title.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.prog.title.as_deref()
    }

    /// The tick length in milliseconds (10 … 1,000; default 50).
    #[must_use]
    pub fn tick_ms(&self) -> u64 {
        self.prog.tick_ms
    }

    /// Whether the scenario says `requires install`.
    #[must_use]
    pub fn requires_install(&self) -> bool {
        self.prog.requires_install
    }

    /// The selector declarations in order: binding name, line, family and filters.
    #[must_use]
    pub fn selectors(&self) -> Vec<(&str, u32, Family, &[Filter])> {
        self.prog
            .selectors
            .iter()
            .map(|s| (s.name.as_str(), s.line, s.family, s.filters.as_slice()))
            .collect()
    }

    /// The labels written outside selectors (`class=`, `species=`, `sex=`, `slot=`,
    /// `type=`), as (line, option key, label) in line order. Lets a content check refuse
    /// numeric enum values in committed scenarios (§13 rule 2) together with
    /// [`numeric_enum_values`](Self::numeric_enum_values).
    #[must_use]
    pub fn labels(&self) -> Vec<(u32, &str, &str)> {
        self.prog
            .labels
            .iter()
            .map(|l| (l.line, l.attribute, l.label.as_str()))
            .collect()
    }

    /// Whether any selector uses the `did` family (forbidden in committed scenarios,
    /// §13 rule 2).
    #[must_use]
    pub fn uses_did_family(&self) -> bool {
        self.prog.selectors.iter().any(|s| s.family == Family::Did)
    }

    /// Whether any enum position holds a number instead of a label (§13 rule 2): selector
    /// value filters, and the `class=`, `species=`, `sex=`, `slot=` (of `equip`) and
    /// `type=` options.
    #[must_use]
    pub fn numeric_enum_values(&self) -> bool {
        use ast::{BodyArg, EnumArg, InputCmd, Verb};
        let num = |e: &Option<EnumArg>| matches!(e, Some(EnumArg::Value(_)));
        let p = &self.prog;
        p.selectors.iter().any(|s| {
            s.filters.iter().any(|f| match &f.value {
                FilterValue::Alternatives(a) => a.iter().any(|x| matches!(x, Alt::Value(_))),
                _ => false,
            })
        }) || p
            .characters
            .iter()
            .any(|c| num(&c.class) || num(&c.species) || num(&c.sex))
            || p.actors.iter().any(|a| {
                matches!(
                    a.body,
                    BodyArg::Character {
                        species: EnumArg::Value(_),
                        ..
                    } | BodyArg::Character {
                        sex: EnumArg::Value(_),
                        ..
                    }
                )
            })
            || p.steps.iter().any(|s| match &s.verb {
                Verb::Equip { slot, .. } => matches!(slot, EnumArg::Value(_)),
                Verb::Delta { damage_type, .. } => num(damage_type),
                Verb::AcceptCharacter(c) => num(&c.class) || num(&c.species) || num(&c.sex),
                _ => false,
            })
            || p.input.iter().any(|l| match &l.cmd {
                InputCmd::Equip { slot, .. } => matches!(slot, EnumArg::Value(_)),
                InputCmd::CreateCharacter { class, body, .. } => {
                    num(class)
                        || body.is_some_and(|(s, x)| {
                            matches!(s, EnumArg::Value(_)) || matches!(x, EnumArg::Value(_))
                        })
                }
                _ => false,
            })
    }
}
