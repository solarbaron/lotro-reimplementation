// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's [`SelectorResolver`]: `tessera-net`'s selector queries converted to
//! `tessera-gamedata`'s query type and evaluated against the opened install
//! (`docs/specs/scenario-feed.md` §3.1 "Adapter", §8.6).
//!
//! `tessera-gamedata` cannot depend on `tessera-net`, so it mirrors the selector language
//! with its own types; this module is the thin mapping between the two. Everything the
//! resolver returns is treated as untrusted by `tessera-net`, which sorts, deduplicates,
//! caps and samples it itself (§8.4–§8.6).
//!
//! | Resolver | When | Behaviour |
//! |---|---|---|
//! | [`EmptyResolver`] (from `tessera-net`) | no install (`--no-data`, none found) | every selector resolves empty (§8.7) |
//! | [`GameDataResolver`] | an install was found and opened | candidates, labels and names from the install |
//! | [`UnreadableInstall`] | an install was found but could not be opened | `has_install` is true and every selector fails with `InstallUnreadable` (§8.6 step 1, exit 4) |

use std::path::Path;

use tessera_gamedata as gd;
use tessera_net::scenario::{
    Alt, Attribute, Candidate, CandidateOutcome, CandidateStatus, EmptyResolver, Family, Filter,
    FilterValue, LabelContext, SelectorQuery, SelectorResolver,
};
use tessera_net::{Anchor, DataId, Landblock, Location, RegionId};
use tessera_props::RecordSource;

/// A [`SelectorResolver`] over a [`gd::GameDataStore`]: the install the client opened.
///
/// Generic over the store's record source so tests can use a synthetic store; the client
/// uses the install source ([`GameDataResolver::open`]).
pub struct GameDataResolver<S: RecordSource = gd::InstallSource> {
    store: gd::GameDataStore<S>,
    /// Reused buffer for the store's candidates (converted into the caller's vector).
    buf: Vec<gd::Candidate>,
}

impl<S: RecordSource> std::fmt::Debug for GameDataResolver<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameDataResolver")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl GameDataResolver<gd::InstallSource> {
    /// Opens the install in `dir` read-only (a directory walk; no record is read until a
    /// selector needs it) with string access for display names when the string tables
    /// open.
    ///
    /// # Errors
    ///
    /// When the install's archives or object directory cannot be opened.
    pub fn open(dir: &Path) -> Result<Self, gd::GameDataError> {
        let mut store = gd::GameDataStore::open(dir)?;
        match tessera_text::Strings::open_install(dir, None) {
            Ok(strings) => store = store.with_strings(strings),
            Err(e) => tracing::warn!("scenario: no string tables for display names: {e}"),
        }
        Ok(Self::new(store))
    }
}

impl<S: RecordSource> GameDataResolver<S> {
    /// A resolver over `store`.
    #[must_use]
    pub fn new(store: gd::GameDataStore<S>) -> Self {
        Self {
            store,
            buf: Vec::new(),
        }
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &gd::GameDataStore<S> {
        &self.store
    }
}

/// The earlier bindings of a `tessera-net` query, as `tessera-gamedata` reads them.
struct NetBindings<'q, 'a>(&'q SelectorQuery<'a>);

impl gd::Bindings for NetBindings<'_, '_> {
    fn for_each_did(&self, index: usize, f: &mut dyn FnMut(gd::Did)) {
        for c in self.0.referenced(index) {
            f(gd::Did(c.did.0));
        }
    }
}

impl<S: RecordSource> SelectorResolver for GameDataResolver<S> {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        query: &SelectorQuery<'_>,
        limit: usize,
        out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        let Some(q) = convert_query(query) else {
            tracing::debug!(
                line = query.line,
                family = query.family.keyword(),
                "scenario: selector not expressible for the data layer"
            );
            return CandidateOutcome::new(CandidateStatus::Unsupported);
        };
        self.buf.clear();
        let outcome = self
            .store
            .candidates_with(&q, &NetBindings(query), limit, &mut self.buf);
        let room = limit.saturating_sub(out.len().min(limit));
        out.extend(self.buf.iter().take(room).map(convert_candidate));
        self.buf.clear();
        convert_outcome(outcome)
    }

    fn enum_value(&mut self, context: LabelContext, label: &str) -> Option<u32> {
        if context == LabelContext::NpcAction {
            return npc_action_key(&self.store, label);
        }
        self.store.enum_value(convert_context(context)?, label)
    }

    fn display_name(&mut self, did: DataId, row: Option<u32>) -> Option<String> {
        self.store.display_name(gd::Did(did.0), row)
    }
}

/// The resolver of an install that was found but could not be opened: `has_install` is
/// true, every selector reports the install unreadable (resolve error `InstallUnreadable`,
/// exit 4, §8.6 step 1), labels and names resolve to nothing.
#[derive(Debug, Clone)]
pub struct UnreadableInstall {
    reason: String,
}

impl UnreadableInstall {
    /// A resolver that reports `reason` for every selector.
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl SelectorResolver for UnreadableInstall {
    fn has_install(&self) -> bool {
        true
    }

    fn candidates(
        &mut self,
        _query: &SelectorQuery<'_>,
        _limit: usize,
        _out: &mut Vec<Candidate>,
    ) -> CandidateOutcome {
        CandidateOutcome::new(CandidateStatus::Failed(self.reason.clone()))
    }

    fn enum_value(&mut self, _context: LabelContext, _label: &str) -> Option<u32> {
        None
    }

    fn display_name(&mut self, _did: DataId, _row: Option<u32>) -> Option<String> {
        None
    }
}

/// The resolver a run uses: none, the install, or an install that failed to open.
#[derive(Debug)]
pub enum ClientResolver {
    /// No install (§8.7).
    NoInstall(EmptyResolver),
    /// The opened install.
    Install(Box<GameDataResolver>),
    /// An install that could not be opened.
    Unreadable(UnreadableInstall),
}

impl ClientResolver {
    /// The resolver for `install`: [`ClientResolver::NoInstall`] without one, else the
    /// install opened with [`GameDataResolver::open`] (or [`ClientResolver::Unreadable`]
    /// when that fails).
    #[must_use]
    pub fn for_install(install: Option<&Path>) -> Self {
        match install {
            None => Self::NoInstall(EmptyResolver),
            Some(dir) => match GameDataResolver::open(dir) {
                Ok(r) => Self::Install(Box::new(r)),
                Err(e) => {
                    tracing::error!(dir = %dir.display(), "scenario: the install cannot be read: {e}");
                    Self::Unreadable(UnreadableInstall::new(e.to_string()))
                }
            },
        }
    }

    /// The resolver as a trait object.
    pub fn as_dyn(&mut self) -> &mut dyn SelectorResolver {
        match self {
            Self::NoInstall(r) => r,
            Self::Install(r) => r.as_mut(),
            Self::Unreadable(r) => r,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Conversions between the two query types
// ---------------------------------------------------------------------------------------------

/// The data layer's family for a scenario family (both spell §8.3's keywords).
#[must_use]
pub fn convert_family(family: Family) -> Option<gd::Family> {
    gd::Family::from_keyword(family.keyword())
}

/// The data layer's attribute for a scenario attribute.
#[must_use]
pub fn convert_attribute(a: Attribute) -> Option<gd::Attribute> {
    use gd::Attribute as G;
    Some(match a {
        Attribute::Kind => G::Kind,
        Attribute::Slot => G::Slot,
        Attribute::Fits => G::Fits,
        Attribute::Equippable => G::Equippable,
        Attribute::Category => G::Category,
        Attribute::Quality => G::Quality,
        Attribute::Level => G::Level,
        Attribute::MinLevel => G::MinLevel,
        Attribute::Class => G::Class,
        Attribute::Stackable => G::Stackable,
        Attribute::HasIcon => G::HasIcon,
        Attribute::ListedBy => G::ListedBy,
        Attribute::Mount => G::Mount,
        Attribute::Type => G::Type,
        Attribute::Harmful => G::Harmful,
        Attribute::Monster => G::Monster,
        Attribute::Induction => G::Induction,
        Attribute::Toggle => G::Toggle,
        Attribute::Role => G::Role,
        Attribute::Nature => G::Nature,
        Attribute::Species => G::Species,
        Attribute::Genus => G::Genus,
        Attribute::Alignment => G::Alignment,
        Attribute::AgentClass => G::AgentClass,
        Attribute::HasAppearance => G::HasAppearance,
        Attribute::Vendor => G::Vendor,
        Attribute::Trainer => G::Trainer,
        Attribute::QuestGiver => G::QuestGiver,
        Attribute::Deed => G::Deed,
        Attribute::Objectives => G::Objectives,
        Attribute::Profession => G::Profession,
        Attribute::Tier => G::Tier,
        Attribute::Of => G::Of,
        Attribute::Region => G::Region,
        _ => return None,
    })
}

/// The scenario attribute for a data-layer attribute (for unmatched-label notes).
#[must_use]
pub fn attribute_from_data(a: gd::Attribute) -> Option<Attribute> {
    use gd::Attribute as G;
    Some(match a {
        G::Kind => Attribute::Kind,
        G::Slot => Attribute::Slot,
        G::Fits => Attribute::Fits,
        G::Equippable => Attribute::Equippable,
        G::Category => Attribute::Category,
        G::Quality => Attribute::Quality,
        G::Level => Attribute::Level,
        G::MinLevel => Attribute::MinLevel,
        G::Class => Attribute::Class,
        G::Stackable => Attribute::Stackable,
        G::HasIcon => Attribute::HasIcon,
        G::ListedBy => Attribute::ListedBy,
        G::Mount => Attribute::Mount,
        G::Type => Attribute::Type,
        G::Harmful => Attribute::Harmful,
        G::Monster => Attribute::Monster,
        G::Induction => Attribute::Induction,
        G::Toggle => Attribute::Toggle,
        G::Role => Attribute::Role,
        G::Nature => Attribute::Nature,
        G::Species => Attribute::Species,
        G::Genus => Attribute::Genus,
        G::Alignment => Attribute::Alignment,
        G::AgentClass => Attribute::AgentClass,
        G::HasAppearance => Attribute::HasAppearance,
        G::Vendor => Attribute::Vendor,
        G::Trainer => Attribute::Trainer,
        G::QuestGiver => Attribute::QuestGiver,
        G::Deed => Attribute::Deed,
        G::Objectives => Attribute::Objectives,
        G::Profession => Attribute::Profession,
        G::Tier => Attribute::Tier,
        G::Of => Attribute::Of,
        G::Region => Attribute::Region,
        _ => return None,
    })
}

/// The data layer's filter value for a scenario filter value.
#[must_use]
pub fn convert_value(v: &FilterValue) -> Option<gd::FilterValue> {
    Some(match v {
        FilterValue::Alternatives(alts) => gd::FilterValue::Alternatives(
            alts.iter()
                .map(|a| match a {
                    Alt::Value(n) => gd::Alt::Value(*n),
                    Alt::Label(l) => gd::Alt::Label(l.clone()),
                })
                .collect(),
        ),
        FilterValue::Keywords(k) => gd::FilterValue::Keywords(k.clone()),
        FilterValue::Range { lo, hi } => gd::FilterValue::Range { lo: *lo, hi: *hi },
        FilterValue::Integers(v) => gd::FilterValue::Integers(v.clone()),
        FilterValue::Bool(b) => gd::FilterValue::Bool(*b),
        FilterValue::Binding(i) => gd::FilterValue::Binding(*i),
        _ => return None,
    })
}

/// The data layer's filter for a scenario filter.
#[must_use]
pub fn convert_filter(f: &Filter) -> Option<gd::Filter> {
    Some(gd::Filter::new(
        convert_attribute(f.attribute)?,
        convert_value(&f.value)?,
    ))
}

/// The data layer's query for a scenario query: family, the `did` list and the filters,
/// count `all` (the count is applied by `tessera-net`, §8.6 step 3). `None` when a part
/// has no counterpart (a newer variant on one side); the resolver then reports the family
/// as unsupported, which resolves empty with a note.
#[must_use]
pub fn convert_query(q: &SelectorQuery<'_>) -> Option<gd::SelectorQuery> {
    let mut out = gd::SelectorQuery::all(convert_family(q.family)?);
    // At most the `did` list limit; `tessera-net` already refuses longer lists (§11.1).
    out.dids = q
        .dids
        .iter()
        .take(gd::MAX_DID_LIST)
        .map(|d| gd::Did(d.0))
        .collect();
    out.filters = q
        .filters
        .iter()
        .map(convert_filter)
        .collect::<Option<Vec<_>>>()?;
    Some(out)
}

/// The scenario candidate for a data-layer candidate. A location outside the protocol's
/// coordinate range is dropped by `tessera-net` (untrusted resolver output); an anchor
/// whose landblock index is out of range is dropped here.
#[must_use]
pub fn convert_candidate(c: &gd::Candidate) -> Candidate {
    Candidate {
        did: DataId(c.did.0),
        row: c.row,
        location: c.location.map(|l| Location {
            region: RegionId(l.region),
            x: l.x,
            y: l.y,
            z: l.z,
        }),
        anchor: c.anchor.and_then(|a| {
            Landblock::new(RegionId(a.region), a.x, a.y).map(|block| Anchor::new(block, a.cell))
        }),
    }
}

/// The scenario outcome for a data-layer outcome.
#[must_use]
pub fn convert_outcome(o: gd::CandidateOutcome) -> CandidateOutcome {
    let status = match o.status {
        gd::CandidateStatus::Ready => CandidateStatus::Ready,
        gd::CandidateStatus::Truncated => CandidateStatus::Truncated,
        gd::CandidateStatus::NoInstall => CandidateStatus::NoInstall,
        gd::CandidateStatus::Failed(reason) => CandidateStatus::Failed(reason),
        _ => CandidateStatus::Unsupported,
    };
    CandidateOutcome {
        status,
        unmatched_labels: o
            .unmatched_labels
            .into_iter()
            .filter_map(|(a, label)| Some((attribute_from_data(a)?, label)))
            .collect(),
    }
}

/// The key of NPC action label `label` (`@Name` or `Name`): the label's key in the enum
/// mapper the property dictionary names for `ActionPickerUI_Action`, or in its base chain
/// (`scenario-feed.md` §8.6 P3 revision, context `NpcAction`; `npc-windows.md` §5.3).
/// `None` for an unknown label or when the dictionary or mapper cannot be read.
pub fn npc_action_key<S: RecordSource>(store: &gd::GameDataStore<S>, label: &str) -> Option<u32> {
    let data = store.object_store().game_data();
    let registry = data.registry().ok()?;
    let pid = registry.pid("ActionPickerUI_Action")?;
    let tessera_props::EnumSource::Mapper(start) = registry.enum_source(pid)? else {
        return None;
    };
    let name = label.strip_prefix('@').unwrap_or(label);
    tessera_formats::EnumMapper::key_in_chain_from(
        start,
        |d| data.enum_mapper(d).ok().flatten(),
        name,
    )
}

/// The data layer's label context for a scenario label context.
#[must_use]
pub fn convert_context(c: LabelContext) -> Option<gd::LabelContext> {
    Some(match c {
        LabelContext::Class => gd::LabelContext::Class,
        LabelContext::Species => gd::LabelContext::Species,
        LabelContext::Sex => gd::LabelContext::Sex,
        LabelContext::EquipSlot => gd::LabelContext::EquipSlot,
        LabelContext::DamageType => gd::LabelContext::DamageType,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ATTRIBUTES: [Attribute; 34] = [
        Attribute::Kind,
        Attribute::Slot,
        Attribute::Fits,
        Attribute::Equippable,
        Attribute::Category,
        Attribute::Quality,
        Attribute::Level,
        Attribute::MinLevel,
        Attribute::Class,
        Attribute::Stackable,
        Attribute::HasIcon,
        Attribute::ListedBy,
        Attribute::Mount,
        Attribute::Type,
        Attribute::Harmful,
        Attribute::Monster,
        Attribute::Induction,
        Attribute::Toggle,
        Attribute::Role,
        Attribute::Nature,
        Attribute::Species,
        Attribute::Genus,
        Attribute::Alignment,
        Attribute::AgentClass,
        Attribute::HasAppearance,
        Attribute::Vendor,
        Attribute::Trainer,
        Attribute::QuestGiver,
        Attribute::Deed,
        Attribute::Objectives,
        Attribute::Profession,
        Attribute::Tier,
        Attribute::Of,
        Attribute::Region,
    ];

    #[test]
    fn every_family_maps_by_keyword() {
        for f in Family::ALL {
            let g = convert_family(f).unwrap_or_else(|| panic!("{f:?} has no data family"));
            assert_eq!(g.keyword(), f.keyword());
        }
        assert_eq!(Family::ALL.len(), gd::Family::ALL.len());
    }

    #[test]
    fn every_attribute_maps_both_ways_with_the_same_name() {
        for a in ATTRIBUTES {
            let g = convert_attribute(a).expect("mapped");
            assert_eq!(g.name(), a.name());
            assert_eq!(attribute_from_data(g), Some(a));
        }
    }

    /// Records the converted form of every query `tessera-net` hands over.
    #[derive(Default)]
    struct Recorder {
        queries: Vec<Option<gd::SelectorQuery>>,
        referenced: Vec<Vec<u32>>,
    }

    impl SelectorResolver for Recorder {
        fn has_install(&self) -> bool {
            true
        }
        fn candidates(
            &mut self,
            q: &SelectorQuery<'_>,
            limit: usize,
            out: &mut Vec<Candidate>,
        ) -> CandidateOutcome {
            self.queries.push(convert_query(q));
            let mut dids = Vec::new();
            gd::Bindings::for_each_did(&NetBindings(q), 0, &mut |d| dids.push(d.0));
            self.referenced.push(dids);
            if q.family == Family::Recipe && limit > 0 {
                out.push(Candidate {
                    did: DataId(0x7000_0010),
                    row: 0,
                    location: None,
                    anchor: None,
                });
            }
            CandidateOutcome::new(CandidateStatus::Ready)
        }
        fn enum_value(&mut self, _: LabelContext, _: &str) -> Option<u32> {
            None
        }
        fn display_name(&mut self, _: DataId, _: Option<u32>) -> Option<String> {
            None
        }
    }

    #[test]
    fn queries_convert_filters_dids_and_bindings() {
        let text = "tessera-scenario 1\n\
            select $r = recipe all\n\
            select $i = item kind=weapon|shield level=5.. quality=3|@Rare_Example stackable=no listed-by=$r n=2 seed=9\n\
            select $p = place region=1|2 all\n\
            select $d = did 0x70000001,0x70000002 all\n";
        let sc = tessera_net::scenario::Scenario::parse(text.as_bytes()).expect("parses");
        let mut rec = Recorder::default();
        let resolved = sc.resolve(&mut rec).expect("resolves");
        assert!(resolved.has_install());
        let q: Vec<gd::SelectorQuery> = rec
            .queries
            .into_iter()
            .map(|q| q.expect("converts"))
            .collect();
        assert_eq!(q.len(), 4);
        assert_eq!(q[0].family, gd::Family::Recipe);
        let item = &q[1];
        assert_eq!(item.family, gd::Family::Item);
        assert_eq!(
            item.count,
            gd::Count::All,
            "the count stays with tessera-net"
        );
        let attrs: Vec<&str> = item.filters.iter().map(|f| f.attribute.name()).collect();
        assert_eq!(
            attrs,
            ["kind", "level", "quality", "stackable", "listed-by"],
            "filters keep their order"
        );
        assert_eq!(
            item.filters[0].value,
            gd::FilterValue::Keywords(vec!["weapon".into(), "shield".into()])
        );
        assert_eq!(
            item.filters[1].value,
            gd::FilterValue::Range {
                lo: Some(5),
                hi: None
            }
        );
        assert_eq!(
            item.filters[2].value,
            gd::FilterValue::Alternatives(vec![
                gd::Alt::Value(3),
                gd::Alt::Label("@Rare_Example".into())
            ])
        );
        assert_eq!(item.filters[3].value, gd::FilterValue::Bool(false));
        assert_eq!(item.filters[4].value, gd::FilterValue::Binding(0));
        assert_eq!(
            rec.referenced[1],
            vec![0x7000_0010],
            "binding filters read the resolved list of the earlier selector"
        );
        assert_eq!(q[2].family, gd::Family::Place);
        assert_eq!(q[2].filters[0].value, gd::FilterValue::Integers(vec![1, 2]));
        assert_eq!(q[3].family, gd::Family::Did);
        assert_eq!(q[3].dids, vec![gd::Did(0x7000_0001), gd::Did(0x7000_0002)]);
    }

    #[test]
    fn an_install_without_records_is_unreadable() {
        // An install with nothing in it: no dictionary, no class library.
        let store = gd::GameDataStore::new(
            tessera_wstate::ObjectStore::new(tessera_props::MemorySource::new()),
            vec![gd::Did(0x7000_0001)],
        );
        let mut r = GameDataResolver::new(store);
        assert!(r.has_install());
        assert_eq!(r.enum_value(LabelContext::Class, "@Anything"), None);
        let sc =
            tessera_net::scenario::Scenario::parse(b"tessera-scenario 1\nselect $s = skill all\n")
                .expect("parses");
        let err = sc.resolve(&mut r).expect_err("unreadable");
        assert_eq!(err.kind_name(), "InstallUnreadable");
        assert_eq!(err.line(), 2);
        let mut u = ClientResolver::Unreadable(UnreadableInstall::new("broken"));
        assert_eq!(
            sc.resolve(u.as_dyn()).expect_err("unreadable").kind_name(),
            "InstallUnreadable"
        );
        // Without selectors there is nothing to read.
        let plain =
            tessera_net::scenario::Scenario::parse(b"tessera-scenario 1\n").expect("parses");
        assert!(plain.resolve(u.as_dyn()).expect("resolves").has_install());
    }

    #[test]
    fn no_install_resolves_every_selector_empty() {
        let sc = tessera_net::scenario::Scenario::parse(
            b"tessera-scenario 1\nselect $s = skill harmful=yes n=2 seed=7\n",
        )
        .expect("parses");
        let mut r = ClientResolver::for_install(None);
        let resolved = sc.resolve(r.as_dyn()).expect("resolves");
        assert!(!resolved.has_install());
        assert_eq!(resolved.binding("$s").map(<[_]>::len), Some(0));
        let req = tessera_net::scenario::Scenario::parse(b"tessera-scenario 1\nrequires install\n")
            .expect("parses");
        assert_eq!(
            req.resolve(r.as_dyn()).expect_err("required").kind_name(),
            "InstallRequired"
        );
    }

    #[test]
    fn candidates_and_outcomes_convert() {
        let c = gd::Candidate {
            did: gd::Did(0x7000_0042),
            row: 7,
            location: Some(gd::Location {
                region: 1,
                x: 1.5,
                y: -2.0,
                z: 3.25,
            }),
            anchor: Some(gd::Anchor {
                region: 1,
                x: 12,
                y: 200,
                cell: 0x105,
            }),
        };
        let n = convert_candidate(&c);
        assert_eq!(n.did, DataId(0x7000_0042));
        assert_eq!(n.row, 7);
        let l = n.location.expect("location");
        assert_eq!((l.region, l.x, l.y, l.z), (RegionId(1), 1.5, -2.0, 3.25));
        let a = n.anchor.expect("anchor");
        assert_eq!((a.block.x, a.block.y, a.cell), (12, 200, 0x105));
        let o = convert_outcome(gd::CandidateOutcome {
            status: gd::CandidateStatus::Truncated,
            unmatched_labels: vec![(gd::Attribute::Class, "@Nobody".into())],
        });
        assert_eq!(o.status, CandidateStatus::Truncated);
        assert_eq!(
            o.unmatched_labels,
            vec![(Attribute::Class, "@Nobody".into())]
        );
        assert_eq!(
            convert_outcome(gd::CandidateOutcome::new(gd::CandidateStatus::Failed(
                "x".into()
            )))
            .status,
            CandidateStatus::Failed("x".into())
        );
    }

    #[test]
    fn label_contexts_map() {
        for (n, g) in [
            (LabelContext::Class, gd::LabelContext::Class),
            (LabelContext::Species, gd::LabelContext::Species),
            (LabelContext::Sex, gd::LabelContext::Sex),
            (LabelContext::EquipSlot, gd::LabelContext::EquipSlot),
            (LabelContext::DamageType, gd::LabelContext::DamageType),
        ] {
            assert_eq!(convert_context(n), Some(g));
        }
    }
}
