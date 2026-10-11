// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` combat widgets (`combat-ui.md`) in their fallback looks, driven by an
//! arbitrary sequence of feed calls (frames, casts with hostile durations, effects,
//! floating texts with hostile anchors, combat state, defeat, quickslot cooldowns) and the
//! procedural-image path parser. Nothing may panic, loop without end or allocate without
//! bound; the §15 invariants CUI2–CUI5 and CUI7 hold after every step.
//!
//! Input layout: a sequence of records, each one opcode byte followed by eight bytes read
//! as an `f64` (and reused as integers); the remainder as a (lossy) UTF-8 procedural path.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_ui::game::NoShortcuts;
use tessera_ui::game::combat::look::procedural_image;
use tessera_ui::game::combat::*;
use tessera_ui::game::quickslot::{SkillSlots, SlotKind, SlotView};
use tessera_ui::{MemoryEnv, Ui, Vocab};

fuzz_target!(|data: &[u8]| {
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let mut c = CombatUi::new(
        &mut ui,
        CombatVocab::default(),
        CombatLayouts::default(),
        CombatRoots::default(),
        FloatySettings::default(),
    );
    let mut slots = SkillSlots::new(&c.vocab);
    if let Some(h) = c.hud.root(&mut ui) {
        slots.fallback_bar(&mut ui, h, &NoShortcuts);
    }
    let mut now = 0.0f64;
    let (records, rest) = data.as_chunks::<9>();
    for ch in records.iter().take(512) {
        let op = ch[0];
        let mut b = [0u8; 8];
        b.copy_from_slice(&ch[1..]);
        let x = f64::from_le_bytes(b);
        let n = u64::from_le_bytes(b);
        let small = n % 16;
        let v = Vital::new(x, (n % 1000) as f64);
        match op % 16 {
            0 => {
                let f = FrameView {
                    entity: small,
                    name: "n".repeat((n % 300) as usize),
                    has_vitals: n % 2 == 0,
                    health: Some(v),
                    power: Some(v),
                    resource: Some(ClassResource::Wrath(v)),
                    level: LevelShown::Level(n as i32),
                    name_colour: ConRank::from_index((n % 12) as usize).map(NameColour::Con),
                    ..FrameView::default()
                };
                c.set_player(&mut ui, &f);
                c.set_target(
                    &mut ui,
                    (n % 3 != 0).then_some(&f),
                    Some(SelectionView::default()),
                );
                c.set_selection_target(&mut ui, Some(&f));
            }
            1 => {
                let owner = if n % 2 == 0 {
                    CastOwner::Player
                } else {
                    CastOwner::Target
                };
                let view = CastView {
                    label: "c".into(),
                    interruptible: n % 3 == 0,
                    cancellable: n % 5 == 0,
                    duration: x,
                };
                c.cast_started(&mut ui, owner, &view, now);
            }
            2 => c.cast_delayed(CastOwner::Player, x),
            3 => {
                let o = [
                    CastOutcome::Completed,
                    CastOutcome::Interrupted,
                    CastOutcome::Cancelled,
                ][(n % 3) as usize];
                c.cast_ended(&mut ui, CastOwner::Player, o, now);
            }
            4 => {
                let e = EffectView {
                    instance: small,
                    effect_id: (n >> 8) as u32 % 4,
                    label: String::from_utf8_lossy(&ch[1..4]).into_owned(),
                    harmful: n % 2 == 1,
                    start: now,
                    duration: (n % 4 != 0).then_some(x),
                    combat_only: n % 7 == 0,
                    ..EffectView::default()
                };
                let owner = if n % 3 == 0 {
                    EffectOwner::Target
                } else {
                    EffectOwner::Player
                };
                c.effect_applied(owner, e);
            }
            5 => {
                c.effect_removed(EffectOwner::Player, small);
            }
            6 => {
                let t = FloatyText {
                    text: format!("{x}"),
                    colour: n as u32,
                    critical: n % 2 == 0,
                    life: x as f32,
                    scale: (x * 3.0) as f32,
                };
                c.floating_text(small % 4, t);
            }
            7 => c.set_in_combat(&mut ui, n % 2 == 0),
            8 => {
                c.player_defeated(&mut ui);
            }
            9 => c.player_revived(&mut ui),
            10 => {
                let view = (n % 3 != 0).then(|| SlotView {
                    kind: SlotKind::Skill(small % 3),
                    label: "ab".into(),
                    icon: None,
                    tooltip: None,
                });
                slots.set_slot(&mut ui, (n % 40) as i32 - 5, view, now);
            }
            11 => {
                // Mostly the slots' skills, sometimes any key (the remembered set is capped).
                let key = if n % 5 == 0 { n } else { small % 3 };
                slots.cooldown_started(&mut ui, key, x, now);
            }
            12 => slots.cooldown_ended(&mut ui, small % 3, now),
            13 => slots.set_casting(&mut ui, (n % 2 == 0).then_some(small % 3), now),
            14 => {
                c.entity_despawned(&mut ui, small % 4);
                // Hostile placement inputs never panic and give finite results.
                let f = x as f32;
                let s = floaty::distance_scale(f, f, f * 2.0, f, -f);
                assert!(s.is_finite());
                let _ = selection::clamp_to_screen((f, f, f, -f), (800, 600));
            }
            _ => {
                if x.is_finite() {
                    now += x.abs().min(10.0);
                }
            }
        }
        let anchor = move |e: EntityId| {
            e.is_multiple_of(2).then_some(ScreenAnchor {
                x: x as f32,
                y: (x * 0.5) as f32,
                distance: (n % 50) as f32,
                on_screen: n % 4 != 0,
                drawn: true,
            })
        };
        c.update(&mut ui, now, &anchor);
        slots.update(&mut ui, now);
        if let Some(f) = &c.floaty {
            for e in 0..4 {
                assert!(f.counts(e).1 <= 51);
                assert!(f.slots_taken(e) <= 6);
            }
            for s in f.shown() {
                assert!((0.0..=1.0).contains(&s.opacity));
            }
        }
        for b in [&c.player_effects, &c.target_effects].into_iter().flatten() {
            assert!(b.rows().satisfies_cui7());
        }
        for b in [&c.player_cast, &c.target_cast].into_iter().flatten() {
            if let Some(p) = b.position(now) {
                assert!((0.0..=1.0).contains(&p));
            }
        }
        assert!(slots.remembered_cooldowns() <= tessera_ui::game::quickslot::MAX_COOLDOWNS);
        for i in -5..40 {
            if let Some(f) = slots.frame(i) {
                assert!((0..=59).contains(&f));
            }
        }
    }
    let path = format!("tessera:look/{}", String::from_utf8_lossy(rest));
    if let Some(img) = procedural_image(&path) {
        assert!(img.width() <= 256 && img.height() <= 256);
    }
});
