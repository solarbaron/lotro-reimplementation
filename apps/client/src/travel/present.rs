// SPDX-License-Identifier: GPL-3.0-or-later
//! The travel and instance-finder windows' views from the session state
//! (`travel-windows.md` §4.10, §4.11, §5.2, §6.8): rows and entries with the install's
//! names, territory and area texts, mapper display texts and coin splits (through
//! `tessera-gamedata`'s [`TravelPresenter`] and [`FinderPresenter`]), or the feed's literal
//! refs without an install. Every value the original computes in game code (cost, level,
//! flags, eligibility) is the feed's (TI5).
//!
//! The labels game code sets come from [`InstallTravelTexts`]: the `Travel`, `UI` and
//! `UI_FellowshipMaker` tokens the spec names, rendered from the install, else Tessera's
//! own short texts (the widgets' defaults).

use tessera_formats::Did;
use tessera_gamedata::instance::{FinderEntryIn, FinderPresenter, RowSubject};
use tessera_gamedata::travel::{
    LevelText, RouteMode as GRoute, TravelEntryIn, TravelFlags as GFlags, TravelPresenter, tokens,
};
use tessera_net::domain::{InstanceEntry, Ref, RouteMode, TravelEntry};
use tessera_ui::game::finder::{FinderEntry, FinderLabel, FinderTexts, FinderView, kind_of};
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::npc::{CoinKind, NpcLabel, NpcTexts};
use tessera_ui::game::travel::{TravelLabel, TravelRow, TravelTexts, TravelView};

use super::state::{InstanceFeed, Journey, TravelFeed};
use crate::npc::NpcData;
use crate::npc::present::{DEFAULT_COIN_VALUES, record_ref};
use crate::npc::texts::InstallTexts;

/// The `UI_FellowshipMaker` tokens of the finder labels (`travel-windows.md` §6.2, §6.4).
const FM_TOKENS: [(FinderLabel, u32); 7] = [
    (FinderLabel::JoinNone, 0x07F0_8470),
    (FinderLabel::Join, 0x028A_C480),
    (FinderLabel::JoinUnavailable, 0x0420_40B0),
    (FinderLabel::Featured, 0x06C3_0B6C),
    (FinderLabel::Heading, 0x026A_8B0C),
    (FinderLabel::Size, 0x049C_28EC),
    (FinderLabel::Tier, 0x088C_340C),
];

/// The `FellowshipMakerUI_AlertStateStringArray` tokens of `UI_FellowshipMaker`, by queue
/// key 1–7 (`travel-windows.md` §7.2).
const QUEUE_TOKENS: [u32; 7] = [
    0x004B_9FED,
    0x01AB_DA45,
    0x09E2_3B7D,
    0x0431_010E,
    0x0A43_BB45,
    0x07F0_DB25,
    0x089E_4D15,
];

/// The `UI_FellowshipMaker` string table's label.
const FM_TABLE: &str = "UI_FellowshipMaker";
/// The `UI` string table's label.
const UI_TABLE: &str = "UI";

/// The coin values of `data` (Tessera's default split without an install).
fn coin_values(data: Option<&NpcData>) -> Vec<u64> {
    data.map(NpcData::coin_values)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_COIN_VALUES.to_vec())
}

fn g_route(r: RouteMode) -> GRoute {
    match r {
        RouteMode::Swift => GRoute::Swift,
        RouteMode::Slow => GRoute::Slow,
    }
}

/// The literal text of a ref (empty for a record).
fn literal(r: &Ref) -> String {
    match r {
        Ref::Literal(s) => s.clone(),
        Ref::Record(_) => String::new(),
    }
}

/// A feed travel entry as `tessera-gamedata`'s input.
#[must_use]
pub fn travel_entry_in(e: &TravelEntry) -> TravelEntryIn {
    TravelEntryIn {
        entry: e.entry,
        destination: record_ref(&e.destination),
        coin: e.coin,
        min_level: e.min_level,
        flags: GFlags(e.flags.0 & 0b111),
        route: e.route.map(g_route),
        area: e.area.as_ref().map(record_ref),
    }
}

/// The travel window's view of `feed` (§4.10): one row per entry in entry order; names,
/// swift flags, territory and area texts from the install when `data` is given (literal
/// refs otherwise), cost, level and flags from the feed. `source_name` is the stable
/// master's name (title, rule 1).
#[must_use]
pub fn travel_view(
    feed: &TravelFeed,
    data: Option<&NpcData>,
    player_level: u32,
    source_name: Option<&str>,
) -> TravelView {
    let values = coin_values(data);
    let tp = data.map(|d| TravelPresenter::new(d.data()));
    let rows = feed
        .entries
        .iter()
        .map(|e| {
            let flags = GFlags(e.flags.0 & 0b111);
            let (name, swift, min_level, territory, area) = match &tp {
                Some(tp) => {
                    let r = tp.row(&travel_entry_in(e), player_level);
                    (
                        r.name.unwrap_or_default(),
                        r.swift,
                        u16::try_from(r.min_level).unwrap_or(u16::MAX),
                        r.territory.unwrap_or_default(),
                        r.area.unwrap_or_default(),
                    )
                }
                None => (
                    literal(&e.destination),
                    e.route == Some(RouteMode::Swift),
                    e.min_level.unwrap_or(0),
                    String::new(),
                    e.area.as_ref().map(literal).unwrap_or_default(),
                ),
            };
            TravelRow {
                entry: e.entry,
                name,
                territory,
                area,
                coin: e.coin,
                coins: coin_counts(e.coin, &values),
                min_level,
                swift,
                discovered: flags.discovered(),
                met: flags.met(),
                usable: flags.usable(),
            }
        })
        .collect();
    let title = tp
        .as_ref()
        .and_then(|tp| tp.title(source_name))
        .or_else(|| source_name.map(str::to_owned))
        .unwrap_or_default();
    TravelView {
        window: feed.window.0,
        title,
        player_level,
        rows,
    }
}

/// Whether a started travel shows the loading screen (§5.2 rule 1): the feed's route when
/// scripted, else the destination's (or the route's destination's) swift flag; a literal
/// destination without a route is slow.
#[must_use]
pub fn journey_swift(j: &Journey, data: Option<&NpcData>) -> bool {
    if let Some(r) = j.route {
        return r == RouteMode::Swift;
    }
    let Some(d) = data else {
        return false;
    };
    let e = TravelEntry {
        entry: 1,
        destination: j.destination.clone(),
        coin: 0,
        min_level: None,
        flags: tessera_net::domain::TravelFlags::NONE,
        route: None,
        area: None,
    };
    TravelPresenter::new(d.data())
        .row(&travel_entry_in(&e), 0)
        .swift
}

/// A feed instance entry as `tessera-gamedata`'s input.
#[must_use]
pub fn finder_entry_in(e: &InstanceEntry) -> FinderEntryIn {
    FinderEntryIn {
        entry: e.entry,
        instance: record_ref(&e.instance),
        level: e.level,
        size: e.size,
        tier: e.tier,
        locked: e.locked,
    }
}

/// The finder's view of `feed` (§6.8): one finder entry per feed entry in entry order,
/// with kind and category headers, group rows, size and tier menus and the level range
/// from the install when `data` is given (literal rows otherwise).
#[must_use]
pub fn finder_view(feed: &InstanceFeed, data: Option<&NpcData>, player_level: u16) -> FinderView {
    let featured = |entries: &[InstanceEntry]| {
        feed.featured
            .as_ref()
            .and_then(|f| entries.iter().find(|e| &e.instance == f).map(|e| e.entry))
    };
    let Some(d) = data else {
        let entries = feed
            .entries
            .iter()
            .map(|e| FinderEntry {
                entry: e.entry,
                template: 0,
                name: literal(&e.instance),
                kind: kind_of(0),
                kind_label: String::new(),
                category: 0,
                category_label: String::new(),
                group: None,
                sizes: e.size.map(|s| vec![(s, s.to_string())]).unwrap_or_default(),
                tiers: e.tier.map(|t| vec![(t, t.to_string())]).unwrap_or_default(),
                level_range: None,
                min_level: 0,
                level: e.level,
                size: e.size,
                tier: e.tier,
                locked: e.locked,
            })
            .collect();
        return FinderView {
            window: feed.window.0,
            entries,
            featured: featured(&feed.entries),
            player_level,
        };
    };
    let fp = FinderPresenter::new(d.data());
    let ins: Vec<FinderEntryIn> = feed.entries.iter().map(finder_entry_in).collect();
    let fr = feed.featured.as_ref().map(record_ref);
    let view = fp.view(&ins, fr.as_ref(), u32::from(player_level));
    let mut entries: Vec<FinderEntry> = Vec::with_capacity(feed.entries.len());
    for e in &feed.entries {
        let row = view.rows.iter().find(|r| r.entries.contains(&e.entry));
        let did = match &e.instance {
            Ref::Record(d) => Some(Did(d.0)),
            Ref::Literal(_) => None,
        };
        let template = did.and_then(|d| fp.template(d));
        let name = match (&template, did) {
            (Some(_), Some(d)) => fp.template_name(d).unwrap_or_default(),
            _ => literal(&e.instance),
        };
        let (kind, category) = row.map_or((kind_of(0), 0), |r| (r.kind, r.category.unwrap_or(0)));
        let kind_label = view
            .headers
            .iter()
            .find(|h| h.kind == kind)
            .and_then(|h| h.text.clone())
            .unwrap_or_default();
        let category_label = view
            .headers
            .iter()
            .filter(|h| h.kind == kind)
            .flat_map(|h| h.categories.iter())
            .find(|c| c.category.unwrap_or(0) == category)
            .and_then(|c| c.text.clone())
            .unwrap_or_default();
        let group = row.and_then(|r| match &r.subject {
            RowSubject::Group { label, .. } => Some((*label, r.text.clone().unwrap_or_default())),
            _ => None,
        });
        let menu = |items: &[tessera_gamedata::instance::MenuItem]| -> Vec<(u32, String)> {
            items
                .iter()
                .map(|m| (m.key, m.text.clone().unwrap_or_else(|| m.key.to_string())))
                .collect()
        };
        let level_range = template.as_ref().and_then(|t| {
            let lo = u16::try_from(t.min_level_scale?).ok()?;
            let hi = u16::try_from(t.max_level_scale?).ok()?;
            Some((lo, hi))
        });
        let min_level = did
            .and_then(|d| fp.permission_min_level(d))
            .and_then(|l| u16::try_from(l).ok())
            .unwrap_or(0);
        entries.push(FinderEntry {
            entry: e.entry,
            template: did.map_or(0, |d| d.0),
            name,
            kind,
            kind_label,
            category,
            category_label,
            group,
            sizes: row.map(|r| menu(&r.sizes)).unwrap_or_default(),
            tiers: row.map(|r| menu(&r.tiers)).unwrap_or_default(),
            level_range,
            min_level,
            level: e.level,
            size: e.size,
            tier: e.tier,
            locked: e.locked,
        });
    }
    FinderView {
        window: feed.window.0,
        entries,
        featured: featured(&feed.entries),
        player_level,
    }
}

/// The instance-travel window of `feed` (§4.11 Tessera rule): shown when the window has a
/// source entity and exactly one entry; its title (the template's name, or the literal)
/// and intro text (`PrivateEncounterTemplate_Description`). `holds_travel_array` tells
/// whether the source's body holds `PrivateEncounter_TravelArray` (`None`: unknown, as
/// without an install: a single-entry window from a source is shown as the
/// instance-travel window).
#[must_use]
pub fn instance_travel(
    feed: &InstanceFeed,
    data: Option<&NpcData>,
    holds_travel_array: Option<bool>,
) -> Option<(String, String)> {
    feed.source?;
    if feed.entries.len() != 1 || holds_travel_array == Some(false) {
        return None;
    }
    let e = feed.entries.first()?;
    match (&e.instance, data) {
        (Ref::Record(d), Some(data)) => {
            let w = FinderPresenter::new(data.data()).instance_travel(Did(d.0));
            Some((w.title.unwrap_or_default(), w.intro.unwrap_or_default()))
        }
        (r, _) => Some((literal(r), String::new())),
    }
}

/// The labels game code sets in the P5 windows, from the install's string tables
/// (module docs); every other label keeps the layout's own text.
#[derive(Debug, Clone, Copy)]
pub struct InstallTravelTexts<'a> {
    data: Option<&'a NpcData>,
    npc: InstallTexts<'a>,
}

impl<'a> InstallTravelTexts<'a> {
    /// The texts of `data` (Tessera's defaults without it).
    #[must_use]
    pub fn new(data: Option<&'a NpcData>) -> Self {
        Self {
            data,
            npc: InstallTexts::new(data),
        }
    }

    fn token(&self, table: &str, token: u32, values: &tessera_text::Values) -> Option<String> {
        use tessera_formats::StringInfo;
        use tessera_formats::string_info::StringSource;
        use tessera_gamedata::tooltip::TextRef;
        let d = self.data?;
        let table = d.data().table(table)?;
        let info = StringInfo {
            source: StringSource::Table { token, table },
            args: None,
        };
        d.data()
            .render(TextRef::Info(&info), values)
            .ok()
            .flatten()
            .map(|t| t.plain_text())
            .filter(|t| !t.trim().is_empty())
    }

    fn travel_token(&self, token: u32) -> Option<String> {
        self.data
            .and_then(|d| TravelPresenter::new(d.data()).travel_text(token))
            .filter(|t| !t.trim().is_empty())
    }
}

impl NpcTexts for InstallTravelTexts<'_> {
    fn label(&self, label: NpcLabel) -> Option<String> {
        self.npc.label(label)
    }

    fn coin_count(&self, count: u64) -> String {
        self.npc.coin_count(count)
    }

    fn coin_tooltip(&self, kind: CoinKind) -> Option<String> {
        self.npc.coin_tooltip(kind)
    }
}

impl TravelTexts for InstallTravelTexts<'_> {
    fn travel_label(&self, label: TravelLabel) -> Option<String> {
        match label {
            TravelLabel::GoNoSelection => self.travel_token(tokens::GO_NO_SELECTION),
            TravelLabel::GoUndiscovered => self.travel_token(tokens::GO_UNDISCOVERED),
            TravelLabel::GoUsable => self.travel_token(tokens::GO_USABLE),
            TravelLabel::Free => self.travel_token(tokens::FREE),
            TravelLabel::NoRouteSelected => self.travel_token(tokens::ROUTE_STATUS[0]),
            TravelLabel::Discovered => self.travel_token(tokens::ROUTE_STATUS[1]),
            TravelLabel::Undiscovered => self.travel_token(tokens::ROUTE_STATUS[2]),
            TravelLabel::EndTravelText => self.token(UI_TABLE, 0x00D2_87E4, &Default::default()),
            TravelLabel::EndTravelButton => self.token(UI_TABLE, 0x0B96_1B54, &Default::default()),
            _ => None,
        }
    }

    fn min_level(&self, level: u16) -> String {
        let text = self.data.and_then(|d| {
            let lt = if level < 2 {
                LevelText::BelowTwo
            } else {
                LevelText::Level(u32::from(level))
            };
            TravelPresenter::new(d.data()).level_text(&lt)
        });
        text.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| {
            if level < 2 {
                "Any level".to_string()
            } else {
                format!("Level {level}")
            }
        })
    }
}

impl FinderTexts for InstallTravelTexts<'_> {
    fn finder_label(&self, label: FinderLabel) -> Option<String> {
        let (_, token) = FM_TOKENS.iter().find(|(l, _)| *l == label)?;
        self.token(FM_TABLE, *token, &Default::default())
    }

    fn queue_state(&self, state: u8) -> String {
        let fallback = || tessera_ui::game::finder::DefaultFinderTexts.queue_state(state);
        let i = usize::from(state);
        if !(1..=7).contains(&i) {
            return fallback();
        }
        QUEUE_TOKENS
            .get(i - 1)
            .and_then(|t| self.token(FM_TABLE, *t, &Default::default()))
            .unwrap_or_else(fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_net::EntityId;
    use tessera_net::domain::{TravelFlags, WindowId};

    fn lit(s: &str) -> Ref {
        Ref::Literal(s.into())
    }

    #[test]
    fn literal_travel_rows_without_an_install() {
        let feed = TravelFeed {
            window: WindowId(4),
            source: Some(EntityId(9)),
            entries: vec![
                TravelEntry {
                    entry: 1,
                    destination: lit("Bree"),
                    coin: 0,
                    min_level: Some(1),
                    flags: TravelFlags::NONE,
                    route: Some(RouteMode::Swift),
                    area: Some(lit("Bree-land")),
                },
                TravelEntry {
                    entry: 2,
                    destination: lit("Michel Delving"),
                    coin: 2_500,
                    min_level: Some(40),
                    flags: TravelFlags::UNDISCOVERED | TravelFlags::UNUSABLE,
                    route: None,
                    area: None,
                },
            ],
            shown: 1,
        };
        let v = travel_view(&feed, None, 35, Some("Stable-master"));
        assert_eq!(v.window, 4);
        assert_eq!(v.title, "Stable-master");
        assert_eq!(v.rows.len(), 2);
        assert!(v.rows[0].swift && v.rows[0].discovered && v.rows[0].usable);
        assert_eq!(v.rows[0].area, "Bree-land");
        assert_eq!(v.rows[1].coins.silver, 25);
        assert!(!v.rows[1].discovered && v.rows[1].met && !v.rows[1].usable);
        assert_eq!(v.rows[1].min_level, 40);
    }

    #[test]
    fn literal_finder_entries_and_instance_travel() {
        let feed = InstanceFeed {
            window: WindowId(5),
            source: Some(EntityId(3)),
            featured: Some(lit("Delving")),
            entries: vec![InstanceEntry {
                entry: 1,
                instance: lit("Delving"),
                level: Some(20),
                size: Some(3),
                tier: None,
                locked: false,
            }],
            shown: 1,
        };
        let v = finder_view(&feed, None, 20);
        assert_eq!(v.featured, Some(1));
        assert_eq!(v.entries[0].name, "Delving");
        assert_eq!(v.entries[0].sizes, vec![(3, "3".to_string())]);
        assert_eq!(
            instance_travel(&feed, None, None),
            Some(("Delving".to_string(), String::new()))
        );
        assert_eq!(instance_travel(&feed, None, Some(false)), None);
    }

    #[test]
    fn journeys_follow_the_feed_route_first() {
        let mut j = Journey {
            destination: lit("Bree"),
            duration_ms: 1000,
            route: None,
            started: 1,
            ended: false,
        };
        assert!(!journey_swift(&j, None));
        j.route = Some(RouteMode::Swift);
        assert!(journey_swift(&j, None));
    }

    #[test]
    fn default_texts_without_an_install() {
        let t = InstallTravelTexts::new(None);
        assert_eq!(t.travel_label(TravelLabel::Free), None);
        assert_eq!(t.min_level(1), "Any level");
        assert_eq!(t.min_level(40), "Level 40");
        assert!(!t.queue_state(2).is_empty());
    }
}
