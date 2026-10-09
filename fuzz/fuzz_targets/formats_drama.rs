// SPDX-License-Identifier: GPL-3.0-or-later
//! NPC drama views (`tessera_formats::gameplay::drama`, `docs/specs/npc-drama.md`) on
//! arbitrary bytes.
//!
//! The input after byte 0 is an embedded property set decoded with a total synthetic schema
//! (type = PID mod 24). Every label the module reads gets its own PID whose type is the one
//! §3.1 gives it (lists and sequences arrays, dramas and actions structs, names and patrol
//! names strings, …); when bit 0 of byte 0 is set, about one label in eight takes a wrong
//! type instead. The list, every drama and action, the facets, the plan of Decision T1, the
//! scripted-fight and starting-idle views and the label helpers are exercised.
//!
//! Nothing may panic. A plan never holds anything after `UntilStopped`, applies the facets
//! of an action in T1 order, and an instant drama plans at most one step.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_drama -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::drama::{
    DramaKind, DramaLabels, DramaList, DramaStep, Facet, KungFu, StartingIdle, actor_number,
    actors_of_mask, dialog_presentation, perform_decision, pick_line, plan_drama, script_link_port,
    transition_fires,
};
use tessera_formats::{PropertySet, PropertyType};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

/// The type §3.1, §7 and §8 give `label`.
fn spec_type(label: &str) -> u32 {
    const STRUCT: u32 = 11;
    const ARRAY: u32 = 12;
    const STRING: u32 = 1;
    const FLOAT: u32 = 9;
    const BOOL: u32 = 21;
    const ENUM: u32 = 8;
    const DID: u32 = 20;
    const SI: u32 = 13;
    const PID: u32 = 10;
    const BITS: u32 = 18;
    const BITS_ANY: u32 = 22;
    match label {
        l if l.ends_with("DramaList")
            || l.contains("ActionSequence")
            || l.ends_with("TextList")
            || l.ends_with("TransitionList") =>
        {
            ARRAY
        }
        "Drama_NamedDrama"
        | "Drama_NamedDrama_Instant"
        | "Drama_NamedDrama_Idle"
        | "Drama_Action"
        | "Drama_Action_Instant"
        | "Drama_Action_Idle"
        | "Drama_Dialog"
        | "Drama_Sound"
        | "Drama_PatrolChangeSpeed"
        | "Drama_BroadcastRadius"
        | "Drama_DirectActors"
        | "Drama_DirectActors_Dialog"
        | "Drama_AddEffect"
        | "Drama_AIHints"
        | "AI_KungFu_DramaTransition"
        | "AI_KungFu_QuestTransition" => STRUCT,
        l if l.ends_with("Name") || l.contains("Patrol") && !l.ends_with("Speed") => STRING,
        "Drama_StartingIdle"
        | "Drama_SetIdleDrama"
        | "Drama_Debug_Shout"
        | "Drama_DirectActors_PerformDrama"
        | "AI_KungFu_Transition_Drama"
        | "Drama_Comment"
        | "Drama_ActionComment" => STRING,
        l if l.contains("Wait_") && !l.ends_with("Emote") || l.ends_with("Distance") => FLOAT,
        l if l.ends_with("Speed") || l.ends_with("Range") || l.ends_with("Spellcraft") => FLOAT,
        l if l.ends_with("HealthRatio") || l.ends_with("Offset") || l.ends_with("WaitForDrama") => {
            FLOAT
        }
        l if l.ends_with("Text") || l.ends_with("Billboard") => SI,
        l if l.ends_with("SoundID") || l.ends_with("EffectID") || l.ends_with("Scriptlet") => DID,
        "Drama_BroadcastRadius_PropertyToMatch" => PID,
        "Drama_DirectActors_Actors" => BITS,
        "Drama_SetHints" | "Drama_HintMask" => BITS_ANY,
        l if l.ends_with("Type")
            || l.ends_with("Channel")
            || l.ends_with("Emote")
            || l.ends_with("Emote_Stop")
            || l.ends_with("ScriptLink")
            || l.contains("RemoteDetector")
            || l.ends_with("Animation") =>
        {
            ENUM
        }
        _ => BOOL,
    }
}

fn labels_for(wrong: bool) -> DramaLabels {
    let map: HashMap<String, u32> = DramaLabels::labels()
        .enumerate()
        .map(|(i, l)| {
            let i = u32::try_from(i).unwrap_or(0);
            let mut ty = spec_type(l);
            if wrong && i.wrapping_mul(0x9E37_79B9) >> 29 == 0 {
                ty = (ty + 7) % 23 + 1;
            }
            (l.to_owned(), (i + 1) * 24 + ty)
        })
        .collect();
    DramaLabels::resolve(&map)
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let Ok(set) = PropertySet::parse(rest, &total) else {
        return;
    };
    let labels = labels_for(mode & 1 != 0);
    let d = &labels.drama;
    let _ = (
        KungFu::read(&labels, &set),
        StartingIdle::read(&labels, &set),
    );
    if let Some(list) = DramaList::read(d, &set) {
        let _ = list.has_duplicate_names();
        for drama in &list.dramas {
            let _ = (drama.name(), drama.interruptible(), drama.wait_for_drama());
            let _ = drama.do_not_end_combat_effective(&HashMap::<u32, _>::new());
            if let Some(n) = drama.name() {
                assert!(list.find(n).is_some() || matches!(drama.kind, DramaKind::Other(_)));
            }
            for a in drama.actions() {
                let f = a.facets();
                let present = f.present();
                assert!(present.windows(2).all(|w| w[0] < w[1]));
                if let Some(da) = &f.direct_actors {
                    let _ = actors_of_mask(da.actors.unwrap_or(0), |k| {
                        ["", "Actor05", "Actor06", "Actor01"]
                            .get(k as usize)
                            .copied()
                    });
                }
                if let Some(dialog) = &f.dialog {
                    let _ = pick_line(dialog.lines.len(), |n| usize::from(*mode) % n.max(1));
                }
            }
            let mut draws = 0u8;
            let plan = plan_drama(drama, |v| {
                draws = draws.wrapping_add(1);
                v * f32::from(draws % 3) / 2.0
            });
            if let Some(pos) = plan.iter().position(|s| *s == DramaStep::UntilStopped) {
                assert_eq!(pos + 1, plan.len());
            }
            if drama.kind == DramaKind::Instant {
                assert!(plan.len() <= 1);
            }
            for s in &plan {
                if let DramaStep::Wait(w) = s {
                    assert!(w.is_finite() && *w > 0.0);
                }
            }
        }
    }
    let text = String::from_utf8_lossy(rest);
    let _ = (
        script_link_port(&text),
        actor_number(&text),
        dialog_presentation(Some(&text)),
    );
    let _ = perform_decision(mode & 2 != 0, Some(Some(mode & 4 != 0)), usize::from(*mode));
    let _ = transition_fires(f32::from(*mode) / 255.0, 0.5, 1.0, mode & 8 != 0);
    let _ = Facet::T1_ORDER.iter().filter(|f| f.runs_offline()).count();
});
