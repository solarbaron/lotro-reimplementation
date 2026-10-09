// SPDX-License-Identifier: GPL-3.0-or-later
//! Selector resolution and the resolved scenario (`scenario-feed.md` §8.6).

use std::fmt;
use std::sync::Arc;

use super::ast::{ActorName, Element, EnumArg, Program};
use super::error::ResolveError;
use super::select::{
    self, Alt, Candidate, CandidateStatus, Family, FilterValue, ResolvedRecord, SelectorQuery,
    SelectorResolver,
};
use crate::model::{MAX_NAME_LEN, sanitize_text};

/// Most candidates per selector, and most resolved records over all selectors.
pub const MAX_CANDIDATES: usize = 1_048_576;

/// Why a selector resolved short (the `note=` of a `resolve` log line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ShortfallNote {
    /// No install was available.
    NoInstall,
    /// The resolver does not support the family.
    Unsupported,
    /// More candidates existed than the limit.
    Truncated,
    /// Fewer records than requested.
    Short,
}

impl ShortfallNote {
    fn name(self) -> &'static str {
        match self {
            Self::NoInstall => "no-install",
            Self::Unsupported => "unsupported",
            Self::Truncated => "truncated",
            Self::Short => "short",
        }
    }
}

/// A resolution note (`scenario-feed.md` §8.6, §10.2); its `Display` is the log line.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolveNote {
    /// One per selector, in declaration order.
    Resolve {
        /// The binding, with its `$`.
        name: String,
        /// The `select` line.
        line: u32,
        /// The family.
        family: Family,
        /// `n=`, or `None` for `all`.
        requested: Option<u32>,
        /// Candidates after sorting, deduplication and the cap.
        candidates: usize,
        /// Records stored.
        got: usize,
        /// The first shortfall that applies.
        note: Option<ShortfallNote>,
    },
    /// A label that matched nothing (with an install only).
    Label {
        /// Its line.
        line: u32,
        /// The filter attribute or option key.
        attribute: String,
        /// The label as written.
        label: String,
    },
    /// A fan-out step over a binding longer than 65,536.
    Fanout {
        /// The step's line.
        line: u32,
        /// Elements left out.
        cut: usize,
    },
    /// A window step over a binding longer than its entry limit.
    Window {
        /// The step's line.
        line: u32,
        /// Elements left out.
        cut: usize,
    },
}

impl fmt::Display for ResolveNote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolve {
                name,
                line,
                family,
                requested,
                candidates,
                got,
                note,
            } => {
                write!(
                    f,
                    "resolve {name} line={line} family={} requested=",
                    family.keyword()
                )?;
                match requested {
                    Some(n) => write!(f, "{n}")?,
                    None => f.write_str("all")?,
                }
                write!(f, " candidates={candidates} got={got}")?;
                if let Some(n) = note {
                    write!(f, " note={}", n.name())?;
                }
                Ok(())
            }
            Self::Label {
                line,
                attribute,
                label,
            } => write!(f, "label line={line} attribute={attribute} label={label}"),
            Self::Fanout { line, cut } => write!(f, "fanout line={line} cut={cut}"),
            Self::Window { line, cut } => write!(f, "window line={line} cut={cut}"),
        }
    }
}

/// A scenario whose selectors are resolved: what the sandbox backend plays
/// (`scenario-feed.md` §3.3). It holds record identifiers, rows, locations, scripted
/// values, literal text, resolved enum values and actor names only.
#[derive(Debug, Clone)]
pub struct ResolvedScenario {
    pub(crate) prog: Arc<Program>,
    pub(crate) bindings: Vec<Vec<ResolvedRecord>>,
    pub(crate) labels: Vec<Option<u32>>,
    pub(crate) actor_names: Vec<String>,
    pub(crate) notes: Vec<ResolveNote>,
    pub(crate) install: bool,
    pub(crate) play_input: bool,
}

impl ResolvedScenario {
    /// The resolution notes, in log order.
    #[must_use]
    pub fn notes(&self) -> &[ResolveNote] {
        &self.notes
    }

    /// Whether the resolver had an install.
    #[must_use]
    pub fn has_install(&self) -> bool {
        self.install
    }

    /// The scenario's title.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.prog.title.as_deref()
    }

    /// The tick length in milliseconds.
    #[must_use]
    pub fn tick_ms(&self) -> u64 {
        self.prog.tick_ms
    }

    /// The `end-at` time in milliseconds.
    #[must_use]
    pub fn end_at_ms(&self) -> Option<u64> {
        self.prog.end_at
    }

    /// Whether the run plays the input script (on by default; a windowed run without
    /// `--scenario-input` turns it off). It decides when a run without `end-at` ends
    /// (§7.3).
    #[must_use]
    pub fn plays_input(&self) -> bool {
        self.play_input
    }

    /// Sets [`plays_input`](Self::plays_input).
    pub fn set_plays_input(&mut self, on: bool) {
        self.play_input = on;
    }

    /// The declared character list, the player first (empty without declarations).
    #[must_use]
    pub fn characters(&self) -> Vec<String> {
        self.prog
            .characters
            .iter()
            .map(|c| c.name.clone())
            .collect()
    }

    /// The character the headless driver enters with when the scenario names one: the
    /// `enter` setting, else the `player`, else the first `character` (§4.3). `None` means
    /// the first name of the `Connected` list.
    #[must_use]
    pub fn enter_character(&self) -> Option<&str> {
        self.prog
            .enter
            .as_deref()
            .or_else(|| self.prog.characters.first().map(|c| c.name.as_str()))
    }

    /// The resolved records of the selector binding `name` (with its `$`).
    #[must_use]
    pub fn binding(&self, name: &str) -> Option<&[ResolvedRecord]> {
        let i = self.prog.selectors.iter().position(|s| s.name == name)?;
        self.bindings.get(i).map(Vec::as_slice)
    }

    /// The header and note lines of the canonical log (§10.2 items 1–5), each without
    /// its line end.
    #[must_use]
    pub fn log_header(&self) -> Vec<String> {
        let mut out = vec![
            "tessera-scenario-log 1".to_owned(),
            format!("install {}", if self.install { "present" } else { "none" }),
            format!("tick {}", self.prog.tick_ms),
        ];
        if let Some(t) = &self.prog.title {
            out.push(format!("title {}", super::log::quote(t)));
        }
        out.extend(self.notes.iter().map(ToString::to_string));
        out
    }

    pub(crate) fn element(&self, el: Element) -> Option<&ResolvedRecord> {
        self.bindings
            .get(usize::try_from(el.sel).ok()?)?
            .get(usize::try_from(el.index).ok()?)
    }

    pub(crate) fn enum_value(&self, e: EnumArg) -> Option<u32> {
        match e {
            EnumArg::Value(v) => Some(v),
            EnumArg::Label(i) => self.labels.get(usize::try_from(i).ok()?).copied().flatten(),
        }
    }

    /// The elements of a binding a step fans out over, cut to `cap`.
    pub(crate) fn fan(&self, sel: u32, cap: usize) -> &[ResolvedRecord] {
        let list = self
            .bindings
            .get(usize::try_from(sel).unwrap_or(usize::MAX))
            .map_or(&[][..], Vec::as_slice);
        &list[..list.len().min(cap)]
    }
}

fn is_table_family(f: Family) -> bool {
    matches!(f, Family::Place | Family::MapNote)
}

/// Untrusted resolver output: drop invalid locations and anchors that do not own them.
fn clean_candidate(mut c: Candidate) -> Candidate {
    if c.location.is_some_and(|l| !l.is_valid()) {
        c.location = None;
    }
    match (c.anchor, c.location) {
        (Some(a), Some(l)) if !a.owns(&l) => c.anchor = None,
        (Some(_), None) => c.anchor = None,
        _ => {}
    }
    c
}

pub(crate) fn resolve(
    prog: &Arc<Program>,
    resolver: &mut dyn SelectorResolver,
) -> Result<ResolvedScenario, ResolveError> {
    let install = resolver.has_install();
    if prog.requires_install && !install {
        return Err(ResolveError::InstallRequired);
    }
    let mut bindings: Vec<Vec<ResolvedRecord>> = Vec::with_capacity(prog.selectors.len());
    let mut notes = Vec::new();
    // (line, col, order, note) for label/fanout/window notes.
    let mut late: Vec<(u32, u32, usize, ResolveNote)> = Vec::new();
    let mut total = 0usize;
    for sel in &prog.selectors {
        let mut out = Vec::new();
        let outcome = {
            let query = SelectorQuery {
                line: sel.line,
                family: sel.family,
                dids: &sel.dids,
                filters: &sel.filters,
                resolved: &bindings,
            };
            resolver.candidates(&query, MAX_CANDIDATES, &mut out)
        };
        let mut note = None;
        let mut truncated = false;
        match &outcome.status {
            CandidateStatus::Failed(reason) => {
                return Err(ResolveError::InstallUnreadable {
                    line: sel.line,
                    reason: sanitize_text(reason, crate::model::MAX_CHAT_LEN).into_owned(),
                });
            }
            CandidateStatus::NoInstall => {
                out.clear();
                note = Some(ShortfallNote::NoInstall);
            }
            CandidateStatus::Unsupported => {
                out.clear();
                note = Some(ShortfallNote::Unsupported);
            }
            CandidateStatus::Truncated => truncated = true,
            CandidateStatus::Ready => {}
        }
        let mut list: Vec<Candidate> = out.into_iter().map(clean_candidate).collect();
        select::sort_dedup(&mut list);
        if list.len() > MAX_CANDIDATES {
            list.truncate(MAX_CANDIDATES);
            truncated = true;
        }
        let candidates = list.len();
        let stored = match (sel.count, sel.seed) {
            (None, _) => list,
            (Some(n), Some(seed)) => {
                select::sample(&list, usize::try_from(n).unwrap_or(usize::MAX), seed)
            }
            (Some(n), None) => {
                list.truncate(usize::try_from(n).unwrap_or(usize::MAX));
                list
            }
        };
        if note.is_none() && truncated {
            note = Some(ShortfallNote::Truncated);
        }
        if note.is_none()
            && let Some(n) = sel.count
            && stored.len() < usize::try_from(n).unwrap_or(usize::MAX)
        {
            note = Some(ShortfallNote::Short);
        }
        total = total.saturating_add(stored.len());
        if total > MAX_CANDIDATES {
            return Err(ResolveError::ResolveLimit { line: sel.line });
        }
        notes.push(ResolveNote::Resolve {
            name: sel.name.clone(),
            line: sel.line,
            family: sel.family,
            requested: sel.count,
            candidates,
            got: stored.len(),
            note,
        });
        if install {
            // One note per unknown label (§8.2, §8.6), bounded by the labels the select
            // line holds, so a resolver cannot inflate the notes.
            let written: usize = sel
                .filters
                .iter()
                .map(|f| match &f.value {
                    FilterValue::Alternatives(a) => {
                        a.iter().filter(|x| matches!(x, Alt::Label(_))).count()
                    }
                    _ => 0,
                })
                .sum();
            for (i, (attr, label)) in outcome.unmatched_labels.iter().take(written).enumerate() {
                late.push((
                    sel.line,
                    0,
                    i,
                    ResolveNote::Label {
                        line: sel.line,
                        attribute: attr.name().to_owned(),
                        label: sanitize_text(label, 128).into_owned(),
                    },
                ));
            }
        }
        bindings.push(stored);
    }
    // Labels outside selectors.
    let mut labels = Vec::with_capacity(prog.labels.len());
    for site in &prog.labels {
        let v = if install {
            resolver.enum_value(site.context, &site.label)
        } else {
            None
        };
        if install && v.is_none() {
            late.push((
                site.line,
                site.col,
                0,
                ResolveNote::Label {
                    line: site.line,
                    attribute: site.attribute.to_owned(),
                    label: site.label.clone(),
                },
            ));
        }
        labels.push(v);
    }
    // Cut notes.
    for cut in &prog.cuts {
        let len = bindings
            .get(usize::try_from(cut.sel).unwrap_or(usize::MAX))
            .map_or(0, Vec::len);
        if len > cut.cap {
            let n = len - cut.cap;
            let note = if cut.window {
                ResolveNote::Window {
                    line: cut.line,
                    cut: n,
                }
            } else {
                ResolveNote::Fanout {
                    line: cut.line,
                    cut: n,
                }
            };
            late.push((cut.line, cut.col, 0, note));
        }
    }
    late.sort_by_key(|(line, col, order, _)| (*line, *col, *order));
    notes.extend(late.into_iter().map(|(_, _, _, n)| n));
    // Actor names.
    let mut actor_names = Vec::with_capacity(prog.actors.len());
    for a in &prog.actors {
        let name = match &a.label {
            ActorName::None => String::new(),
            ActorName::Literal(s) => s.clone(),
            ActorName::Element(el) => {
                let rec = bindings
                    .get(usize::try_from(el.sel).unwrap_or(usize::MAX))
                    .and_then(|b| b.get(usize::try_from(el.index).unwrap_or(usize::MAX)));
                match rec {
                    Some(r) => {
                        let fam = prog
                            .selectors
                            .get(usize::try_from(el.sel).unwrap_or(usize::MAX))
                            .map_or(Family::Item, |s| s.family);
                        let row = is_table_family(fam).then_some(r.row);
                        resolver
                            .display_name(r.did, row)
                            .map(|n| sanitize_text(&n, MAX_NAME_LEN).into_owned())
                            .unwrap_or_default()
                    }
                    None => String::new(),
                }
            }
        };
        actor_names.push(name);
    }
    Ok(ResolvedScenario {
        prog: Arc::clone(prog),
        bindings,
        labels,
        actor_names,
        notes,
        install,
        play_input: true,
    })
}
