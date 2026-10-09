// SPDX-License-Identifier: GPL-3.0-or-later
//! The interactive parts of the gameplay cards, evaluated on the UI thread from data the
//! worker prepared ([`LiveData`]):
//!
//! | Part | What the user sets | Spec |
//! |---|---|---|
//! | decal previews | which Decal SED to show in the mesh view | `decals.md` §9.3 |
//! | day and night lights | the day fraction and the indoor or outdoor state | `instance-scripts.md` §6, §9.4, §9.6 item 5 |
//! | simulated journal | quests and their status; every phasing box is evaluated with §4.3–§4.4 | `world-logic.md` §13 items 1–2 |
//! | drama player | the drama to play in the character viewer | `npc-drama.md` §11 item 4 |
//! | mount viewer | the mount effect to ride in the character viewer | `mounts.md` §10 |

use eframe::egui::{self, Color32, RichText, Ui};
use tessera_formats::gameplay::quests::{EntryGates, MapJournal, QuestStatus, StatusKind};
use tessera_formats::gameplay::world_logic::{LocalPlayer, PhasingBox};
use tessera_formats::{Did, PropertyValue};
use tessera_world::lighting::glow::{
    GlowPids, GlowTableLights, GlowThresholds, TimeOverride, Variant, glow_light, running_variants,
};

use crate::drama_play::DramaPlayArg;
use crate::views::Action;

/// Most entries of each live list.
pub const MAX_LIVE: usize = 400;

/// The interactive data of one card.
#[derive(Debug, Clone, Default)]
pub struct LiveData {
    /// Decal choices of the mesh view (index *i* is choice *i* + 1).
    pub decals: Vec<String>,
    /// The light-glow objects of the landblock.
    pub glow: Option<GlowLive>,
    /// The phasing boxes and the quests they check.
    pub journal: Option<JournalLive>,
    /// Dramas that can be played.
    pub dramas: Vec<DramaPlayArg>,
    /// A mount effect that can be ridden.
    pub ride: Option<u32>,
}

impl LiveData {
    /// Whether there is nothing interactive.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.decals.is_empty()
            && self.glow.is_none()
            && self.journal.is_none()
            && self.dramas.is_empty()
            && self.ride.is_none()
    }
}

/// The light-glow objects of a landblock with what §9.4 needs.
#[derive(Debug, Clone)]
pub struct GlowLive {
    /// The member and engine property IDs.
    pub pids: GlowPids,
    /// The thresholds read from the light-glow scriptlet, or why not.
    pub thresholds: Result<GlowThresholds, String>,
    /// Labels of `m_eTimeOverride` keys.
    pub time_labels: Vec<(u32, String)>,
    /// The dictionary default of `m_eTimeOverride`.
    pub time_default: Option<u32>,
    /// The dictionary default of `m_bIgnoreInsideOutside`.
    pub ignore_default: bool,
    /// The dictionary defaults of `Day.` and `Night.m_bLightActivated`.
    pub activated_defaults: (bool, bool),
    /// The objects.
    pub entities: Vec<GlowEntity>,
}

/// One light-glow object.
#[derive(Debug, Clone)]
pub struct GlowEntity {
    /// Its IID, as text.
    pub iid: String,
    /// The placed object (0x47).
    pub object: Did,
    /// Its class table (a light-glow table).
    pub table: Did,
    /// The values the entity's lookup finds for the members and the two time-of-day
    /// properties (per-instance set, then the object chain), by PID.
    pub values: Vec<(u32, PropertyValue)>,
    /// The table's stored Light-node fields.
    pub stored: GlowTableLights,
}

impl GlowEntity {
    fn get(&self, pid: u32) -> Option<&PropertyValue> {
        self.values.iter().find(|(p, _)| *p == pid).map(|(_, v)| v)
    }
}

/// The phasing boxes of a landblock with the quests they check.
#[derive(Debug, Clone, Default)]
pub struct JournalLive {
    /// Quests named by the boxes, with their names.
    pub quests: Vec<(Did, String)>,
    /// The boxes.
    pub boxes: Vec<BoxLive>,
    /// Status labels of the status mapper (§8.3).
    pub status_labels: Vec<(u32, String)>,
}

/// One phasing box.
#[derive(Debug, Clone)]
pub struct BoxLive {
    /// IID text.
    pub iid: String,
    /// "SED" or "spawn-only (D2)".
    pub kind: &'static str,
    /// The fields.
    pub fields: PhasingBox,
    /// Its checks as text ("quest name operator status").
    pub checks: Vec<String>,
    /// Its targets (end 1 of its phasing links).
    pub targets: Vec<String>,
}

/// One quest of the simulated journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalEntry {
    /// Underway at an objective (0 = any).
    Underway(u32),
    /// Completed.
    Completed,
    /// Failed.
    Failed,
}

/// The UI state of the live parts.
#[derive(Debug, Clone)]
pub struct LiveState {
    /// The day fraction (`World_PresentTimeOfDay`, 0–1).
    pub day_fraction: f32,
    /// `Volume_IsOutside`: 0 inside, 1 outside, 255 unset.
    pub outside: u8,
    /// The simulated journal.
    pub journal: Vec<(Did, JournalEntry)>,
    /// Text of the "add quest" box.
    pub add_quest: String,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            day_fraction: 0.5,
            outside: 1,
            journal: Vec::new(),
            add_quest: String::new(),
        }
    }
}

/// The status of a quest check, decoded (`quests.md` §8.3): the mapper's label when it
/// names the code, else the raw status and objective.
#[must_use]
pub fn status_text(code: u32, labels: &[(u32, String)]) -> String {
    if let Some((_, l)) = labels.iter().find(|(k, _)| *k == code) {
        return format!("{l} ({code:#x})");
    }
    let s = QuestStatus(code);
    match s.kind() {
        StatusKind::Underway if s.objective() == 0 => "underway (any objective)".into(),
        StatusKind::Underway => format!("underway at objective {}", s.objective()),
        StatusKind::Failed => "failed".into(),
        StatusKind::Completed => "completed".into(),
        StatusKind::Abandoned => "abandoned".into(),
        StatusKind::Tapped => "tapped".into(),
        StatusKind::Other(_) => format!("{code:#010x}"),
    }
}

/// The journal of `entries` as the box test reads it.
#[must_use]
pub fn map_journal(entries: &[(Did, JournalEntry)]) -> MapJournal {
    let mut j = MapJournal::default();
    for (q, e) in entries {
        match e {
            JournalEntry::Underway(o) => {
                j.underway.insert(*q, *o);
            }
            JournalEntry::Completed => {
                j.completed.insert(*q);
            }
            JournalEntry::Failed => {
                j.failed.insert(*q);
            }
        }
    }
    j
}

/// Draws the live parts of a card.
pub(crate) fn live_panel(
    ui: &mut Ui,
    live: &LiveData,
    st: &mut LiveState,
    actions: &mut Vec<Action>,
) {
    if let Some(effect) = live.ride {
        ui.horizontal(|ui| {
            if ui
                .button("Ride in the character viewer")
                .on_hover_text("The mount viewer of mounts.md §10: the figure rides this effect")
                .clicked()
            {
                actions.push(Action::Ride(effect));
            }
        });
    }
    if !live.decals.is_empty() {
        egui::CollapsingHeader::new(
            RichText::new(format!(
                "Decal previews ({}) — decals.md §9.3",
                live.decals.len()
            ))
            .strong(),
        )
        .id_salt("live-decals")
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "The mesh view draws the box as a wireframe with the local axes and a \
                     preview mesh with the decal's material and colour.",
                )
                .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                for (i, c) in live.decals.iter().enumerate().take(MAX_LIVE) {
                    if ui.small_button(c).clicked()
                        && let Ok(n) = u8::try_from(i + 1)
                    {
                        actions.push(Action::MeshChoice(n));
                    }
                }
            });
        });
    }
    if let Some(g) = &live.glow {
        egui::CollapsingHeader::new(
            RichText::new(format!(
                "Day and night lights ({}) — instance-scripts.md §6, §9.4",
                g.entities.len()
            ))
            .strong(),
        )
        .id_salt("live-glow")
        .default_open(true)
        .show(ui, |ui| glow_panel(ui, g, st, actions));
    }
    if let Some(j) = &live.journal {
        egui::CollapsingHeader::new(
            RichText::new(format!(
                "Simulated journal and phasing boxes ({}) — world-logic.md §13",
                j.boxes.len()
            ))
            .strong(),
        )
        .id_salt("live-journal")
        .default_open(true)
        .show(ui, |ui| journal_panel(ui, j, st, actions));
    }
    if !live.dramas.is_empty() {
        egui::CollapsingHeader::new(
            RichText::new(format!(
                "Play a drama ({}) — npc-drama.md §11",
                live.dramas.len()
            ))
            .strong(),
        )
        .id_salt("live-dramas")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Runs the drama by Decision T1 under T2/T5 in the character viewer.")
                    .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                for d in live.dramas.iter().take(MAX_LIVE) {
                    if ui.small_button(format!("▶ {}", d.caption)).clicked() {
                        actions.push(Action::PlayDrama(Box::new(d.clone())));
                    }
                    // §11 item 4.1 (Decision T7): without a figure of its own (an instance
                    // set of a server-placed object names no template) the drama plays on
                    // the figure the character view shows, through that figure's tables.
                    if d.figure.is_none() {
                        ui.label(
                            RichText::new("(on the shown figure; emotes use its script tables)")
                                .weak(),
                        );
                    }
                }
            });
        });
    }
}

fn glow_panel(ui: &mut Ui, g: &GlowLive, st: &mut LiveState, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label("day fraction");
        ui.add(egui::Slider::new(&mut st.day_fraction, 0.0..=1.0).fixed_decimals(3));
        ui.separator();
        for (v, name) in [(1u8, "outdoors"), (0, "indoors"), (255, "unset")] {
            ui.radio_value(&mut st.outside, v, name);
        }
    });
    let th = match &g.thresholds {
        Ok(t) => *t,
        Err(e) => {
            ui.colored_label(Color32::LIGHT_RED, e);
            return;
        }
    };
    ui.label(
        RichText::new(format!(
            "thresholds from the scriptlet: {:?} (Volume_IsOutside = {})",
            th.0, st.outside
        ))
        .weak(),
    );
    let p = &g.pids;
    egui::Grid::new("live-glow-grid")
        .striped(true)
        .num_columns(6)
        .show(ui, |ui| {
            for c in ["IID", "object", "time override", "runs", "day light", "night light"] {
                ui.label(RichText::new(c).weak());
            }
            ui.end_row();
            for e in g.entities.iter().take(MAX_LIVE) {
                let key = p
                    .time_override
                    .and_then(|pid| e.get(pid))
                    .and_then(PropertyValue::as_bits)
                    .and_then(|k| u32::try_from(k).ok())
                    .or(g.time_default);
                let label = key.and_then(|k| {
                    g.time_labels
                        .iter()
                        .find(|(x, _)| *x == k)
                        .map(|(_, l)| l.clone())
                });
                let case = label.as_deref().and_then(TimeOverride::from_label);
                let ignore = p
                    .ignore_inside_outside
                    .and_then(|pid| e.get(pid))
                    .and_then(|v| match v {
                        PropertyValue::Bool(b) => Some(*b),
                        _ => None,
                    })
                    .unwrap_or(g.ignore_default);
                ui.label(&e.iid);
                if ui.link(RichText::new(e.object.to_string()).monospace()).clicked() {
                    actions.push(Action::Open(e.object.0));
                }
                ui.label(format!(
                    "{}{}",
                    label.as_deref().unwrap_or("—"),
                    if ignore { ", ignores inside/outside" } else { "" }
                ));
                let Some(case) = case else {
                    ui.label("none (no case)");
                    ui.label("—");
                    ui.label("—");
                    ui.end_row();
                    continue;
                };
                let (day, night) = running_variants(case, ignore, st.outside, st.day_fraction, &th);
                ui.label(match (day, night) {
                    (true, true) => "day and night",
                    (true, false) => "day",
                    (false, true) => "night",
                    (false, false) => "none",
                });
                for (on, variant, default) in [
                    (day, Variant::Day, g.activated_defaults.0),
                    (night, Variant::Night, g.activated_defaults.1),
                ] {
                    let text = if !on {
                        "not running".to_owned()
                    } else if let Some(stored) = e.stored.stored(variant) {
                        match glow_light(p, variant, |pid| e.get(pid), default, stored) {
                            Some(l) => format!(
                                "range {:.2} falloff {:.2} rgb ({:.2}, {:.2}, {:.2}) key {} shadow {}",
                                l.params.range,
                                l.params.falloff,
                                l.params.diffuse[0],
                                l.params.diffuse[1],
                                l.params.diffuse[2],
                                l.params.key_light,
                                l.shadow_caster
                            ),
                            None => "off (not activated)".to_owned(),
                        }
                    } else {
                        "no Light node in the table".to_owned()
                    };
                    ui.label(RichText::new(text).monospace().small());
                }
                ui.end_row();
            }
        });
}

fn journal_panel(ui: &mut Ui, j: &JournalLive, st: &mut LiveState, actions: &mut Vec<Action>) {
    ui.label(
        RichText::new(
            "Set the status of the quests the boxes check; every box is evaluated with \
             world-logic.md §4.3–§4.4 (the empty journal of D1 when nothing is set).",
        )
        .weak(),
    );
    egui::Grid::new("live-journal-quests")
        .striped(true)
        .num_columns(3)
        .show(ui, |ui| {
            for (q, name) in j.quests.iter().take(MAX_LIVE) {
                if ui.link(RichText::new(q.to_string()).monospace()).clicked() {
                    actions.push(Action::Open(q.0));
                }
                ui.label(name);
                let at = st.journal.iter().position(|(x, _)| x == q);
                let current = at.and_then(|i| st.journal.get(i)).map(|(_, e)| *e);
                let text = match current {
                    None => "not in the journal".to_owned(),
                    Some(JournalEntry::Underway(o)) => format!("underway at objective {o}"),
                    Some(JournalEntry::Completed) => "completed".to_owned(),
                    Some(JournalEntry::Failed) => "failed".to_owned(),
                };
                let mut chosen = current;
                egui::ComboBox::from_id_salt(("journal", q.0))
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen, None, "not in the journal");
                        for o in 0..=8 {
                            ui.selectable_value(
                                &mut chosen,
                                Some(JournalEntry::Underway(o)),
                                if o == 0 {
                                    "underway (objective 0)".to_owned()
                                } else {
                                    format!("underway at objective {o}")
                                },
                            );
                        }
                        ui.selectable_value(
                            &mut chosen,
                            Some(JournalEntry::Completed),
                            "completed",
                        );
                        ui.selectable_value(&mut chosen, Some(JournalEntry::Failed), "failed");
                    });
                if chosen != current {
                    st.journal.retain(|(x, _)| x != q);
                    if let Some(e) = chosen {
                        st.journal.push((*q, e));
                    }
                }
                ui.end_row();
            }
        });
    if ui.button("Clear the journal (D1)").clicked() {
        st.journal.clear();
    }
    let journal = map_journal(&st.journal);
    let gates = EntryGates::default();
    let prereq = |_: Did| false;
    let player = LocalPlayer {
        journal: &journal,
        gates: &gates,
        prerequisites_pass: &prereq,
    };
    egui::Grid::new("live-journal-boxes")
        .striped(true)
        .num_columns(4)
        .show(ui, |ui| {
            for c in ["box", "checks", "test", "targets"] {
                ui.label(RichText::new(c).weak());
            }
            ui.end_row();
            for b in j.boxes.iter().take(MAX_LIVE) {
                ui.label(format!("{} ({})", b.iid, b.kind));
                ui.label(RichText::new(b.checks.join("; ")).small());
                let shown = b.fields.test(true, Some(&player));
                ui.label(if shown {
                    RichText::new("targets shown").color(Color32::from_rgb(140, 220, 140))
                } else {
                    RichText::new("targets phased out").color(Color32::from_rgb(230, 150, 120))
                });
                ui.label(RichText::new(b.targets.join(", ")).small().monospace());
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_are_decoded() {
        let labels = vec![(0x3000_0000, "Completed".to_owned())];
        assert_eq!(status_text(0x3000_0000, &labels), "Completed (0x30000000)");
        assert_eq!(status_text(0x1000_0003, &[]), "underway at objective 3");
        assert_eq!(status_text(0x1000_0000, &[]), "underway (any objective)");
        assert_eq!(status_text(0x2000_0000, &[]), "failed");
        assert_eq!(status_text(0x7000_0000, &[]), "0x70000000");
        let j = map_journal(&[
            (Did(0x7000_0001), JournalEntry::Completed),
            (Did(0x7000_0002), JournalEntry::Underway(4)),
            (Did(0x7000_0003), JournalEntry::Failed),
        ]);
        assert!(j.completed.contains(&Did(0x7000_0001)));
        assert_eq!(j.underway.get(&Did(0x7000_0002)), Some(&4));
        assert!(j.failed.contains(&Did(0x7000_0003)));
    }
}
