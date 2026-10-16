// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui` P6 crafting and progression widgets (`crafting-panels.md` §4–§8, §11;
//! decision 0037): the quantity-entry reading and the critical chance run on fuzzed text and
//! floats; the fallback crafting panel, a crafting-trainer window, the experience bar, the
//! level-up banner, the trait panel and the skill-training window are built and shown with
//! fuzzed views (hostile counts, ranks, cells, texts, coin and float values) and take fuzzed
//! clicks on any of their elements and fuzzed typing into their entries. Nothing may panic,
//! loop without end or allocate without bound.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_ui::events::{Message, MsgArg};
use tessera_ui::game::crafting::{
    CategoryRowView, CraftPanel, CraftPanelView, CraftTemplates, CraftTrainerView,
    CraftTrainerWindow, DefaultCraftTexts, IngredientView, MasterView, MeterView, OptionalView,
    P6Art, PackView, ProfessionPageView, RecipePageView, RecipeRowView, RequirementView,
    ResultView, RowKind, TierMarkView, TierRowView, TrainerKind, fallback as craft_fallback, rules,
};
use tessera_ui::game::npc::coins::coin_counts;
use tessera_ui::game::progression::{
    BranchView, DefaultProgressionTexts, ExperienceBar, ExperienceView, LevelUpBanner,
    SkillRowView, SkillTrainerView, SkillTrainerWindow, TraitCellView, TraitPanel, TraitPanelView,
    TraitSlotView, fallback,
};
use tessera_ui::game::{GameVocab, NpcVocab, P6Vocab, find_below, subtree};
use tessera_ui::{Command, ElementKey, MemoryEnv, Ui, UiEvent, Vocab};

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        let b = self.data.get(self.pos).copied().unwrap_or(0);
        self.pos += 1;
        b
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }

    fn u64(&mut self) -> u64 {
        u64::from(self.u32()) << 32 | u64::from(self.u32())
    }

    fn f32(&mut self) -> f32 {
        f32::from_bits(self.u32())
    }

    fn bool(&mut self) -> bool {
        self.u8() & 1 == 1
    }

    fn text(&mut self) -> String {
        let n = usize::from(self.u8() % 12);
        (0..n)
            .map(|_| match self.u8() % 40 {
                0 => ' ',
                1 => '0',
                2..=11 => char::from(b'0' + self.u8() % 10),
                12 => 'Ä',
                _ => char::from(b'a' + self.u8() % 26),
            })
            .collect()
    }

    fn n(&mut self, m: u8) -> usize {
        usize::from(self.u8() % m)
    }
}

fn meter(b: &mut Bytes<'_>) -> MeterView {
    MeterView {
        current: b.u64(),
        cap: b.u64(),
        text: b.text(),
        tooltip: b.text(),
    }
}

fn panel_view(b: &mut Bytes<'_>) -> CraftPanelView {
    let profs = (0..b.n(4))
        .map(|_| ProfessionPageView {
            name: b.text(),
            tiers: (0..b.n(5))
                .map(|_| TierRowView {
                    tier: b.u16(),
                    text: b.text(),
                    marks: (0..b.n(7))
                        .map(|_| match b.u8() % 4 {
                            0 => TierMarkView::ProficiencyIcon(b.text()),
                            1 => TierMarkView::MasteryIcon(b.text()),
                            2 => TierMarkView::ProficiencyMeter(meter(b)),
                            _ => TierMarkView::MasteryMeter(meter(b)),
                        })
                        .collect(),
                    categories: (0..b.n(4))
                        .map(|_| CategoryRowView {
                            text: b.text(),
                            recipes: (0..b.n(6))
                                .map(|_| RecipeRowView {
                                    recipe: u64::from(b.u8() % 8),
                                    tier: u16::from(b.u8() % 4),
                                    kind: match b.u8() % 4 {
                                        0 => RowKind::Plain,
                                        1 => RowKind::OneUse,
                                        2 => RowKind::MultiOutput,
                                        _ => RowKind::Cooldown,
                                    },
                                    text: b.text(),
                                    packs: b.bool().then(|| b.text()),
                                    craftable: b.bool(),
                                    cooldown: b.bool().then(|| b.text()),
                                })
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    CraftPanelView {
        title: b.text(),
        professions: profs,
        train_more: b.bool().then(|| b.text()),
    }
}

fn ingredient(b: &mut Bytes<'_>) -> IngredientView {
    IngredientView {
        icon: None,
        needed: b.u64() % 1000,
        held: b.u64(),
        info: b.text(),
    }
}

fn result(b: &mut Bytes<'_>) -> ResultView {
    if b.bool() {
        ResultView::Item {
            icon: None,
            quantity: b.u64(),
            name: b.text(),
        }
    } else {
        ResultView::Image {
            image: None,
            name: b.text(),
        }
    }
}

fn page_view(b: &mut Bytes<'_>) -> RecipePageView {
    RecipePageView {
        recipe: u64::from(b.u8() % 8),
        tier: u16::from(b.u8() % 4),
        output: b.u16(),
        name: b.text(),
        name_colour: b.bool().then(|| b.u32()),
        result: result(b),
        outputs: b.bool().then(|| b.u16()),
        one_use: b.bool(),
        experience: b.bool().then(|| b.text()),
        cooldown_line: b.bool().then(|| b.text()),
        cooldown_timer: b.bool().then(|| b.text()),
        tool: RequirementView {
            text: b.text(),
            met: b.bool(),
        },
        facility: RequirementView {
            text: b.text(),
            met: b.bool(),
        },
        ingredients: (0..b.n(9)).map(|_| ingredient(b)).collect(),
        pack: b.bool().then(|| PackView {
            slot: ingredient(b),
            checkbox_label: b.text(),
            packs_label: b.text(),
        }),
        master: b.bool().then(|| MasterView {
            mastered: b.bool(),
            optional: (0..b.n(8))
                .map(|_| OptionalView {
                    slot: ingredient(b),
                    bonus: b.f32(),
                    bonus_text: b.text(),
                })
                .collect(),
            critical: result(b),
            base_chance: b.f32(),
        }),
        makeable: b.u64(),
        pack_count: b.u64(),
        channel_blocked: b.bool(),
    }
}

/// A click, double-click, right click or character input on a random element below `root`.
fn event(b: &mut Bytes<'_>, ui: &mut Ui, root: ElementKey) -> UiEvent {
    let all = subtree(ui, root);
    let k = all[usize::from(b.u16()) % all.len()];
    let m = ui.vocab().msgs;
    let (msg, cmd) = match b.u8() % 5 {
        0 => (m.button_click, None),
        1 => (m.mouse_click, Some("SelectLeft")),
        2 => (m.mouse_click, Some("SelectRight")),
        3 => (m.mouse_dbl_click, Some("SelectDblLeft")),
        _ => {
            let t = b.text();
            ui.set_text(k, Some(t));
            (m.character_input, None)
        }
    };
    UiEvent::Game {
        message: Message {
            seq: 0,
            source: k,
            source_id: ui.element_id(k).unwrap_or(0),
            msg,
            arg: MsgArg {
                command: cmd.map(Command::named),
                ..MsgArg::default()
            },
        },
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    // The rules on hostile text and floats.
    let t = b.text();
    let m = b.u64();
    let v = rules::read(&t, m);
    let _ = rules::clamp(v, m);
    let _ = rules::shown(&t, m);
    let _ = rules::increase(v, m);
    let _ = rules::decrease(v, m);
    let _ = rules::to_send(v, m);
    let bonuses: Vec<f32> = (0..b.n(10)).map(|_| b.f32()).collect();
    let c = rules::critical_percent(b.f32(), &bonuses, b.u8());
    assert!(c <= 100);
    let _ = rules::passes(&b.text(), &rules::search_words(&b.text()));

    let (nv, p6, gv) = (
        NpcVocab::default(),
        P6Vocab::default(),
        GameVocab::default(),
    );
    let art = P6Art::default();
    let mut ui = Ui::new(
        Box::new(MemoryEnv::default()),
        Vocab::default(),
        (1024, 768),
    );
    let ct = DefaultCraftTexts;
    let pt = DefaultProgressionTexts;
    // The crafting panel.
    if let Ok(root) = craft_fallback::panel(&mut ui, &nv, &p6, &art, &ct) {
        // An incomplete tree (an install layout without some part): each of these parts is
        // removed when its fuzzed bit is set.
        let cut = b.u8();
        let ids = p6.ids;
        for (bit, id) in [
            ids.craft_dynamic_panel,
            ids.dynamic_tabs,
            ids.dynamic_pages,
            ids.craft_recipe_page,
            ids.quantity_entry,
            ids.packs_field,
        ]
        .into_iter()
        .enumerate()
        {
            if cut & (1 << bit) != 0
                && let Some(k) = find_below(&ui, root, id)
            {
                ui.destroy(k);
            }
        }
        let mut p = CraftPanel::attach(&mut ui, &nv, &p6, root, CraftTemplates::default(), &art);
        for _ in 0..b.n(10) {
            match b.u8() % 7 {
                0 => {
                    let v = panel_view(&mut b);
                    p.show(&mut ui, &v, &ct);
                }
                1 => {
                    let v = b.bool().then(|| page_view(&mut b));
                    p.show_recipe(&mut ui, v.as_ref(), &ct);
                }
                2 => {
                    let r = u64::from(b.u8() % 8);
                    let batch = b.bool().then(|| (b.u16(), b.u16()));
                    p.craft_result(&mut ui, r, batch, &ct);
                }
                3 => p.craft_interrupted(&mut ui, &ct),
                4 => {
                    let f = b.text();
                    p.set_filter(&mut ui, &f, &ct);
                }
                _ => {
                    let e = event(&mut b, &mut ui, root);
                    let _ = p.handle_event(&mut ui, &e, &ct);
                }
            }
            let _ = p.make_enabled();
            let _ = p.critical_percent();
        }
        p.closed(&mut ui, &ct);
        p.detach(&mut ui);
    }
    // A crafting trainer.
    let kind = match b.u8() % 3 {
        0 => TrainerKind::Profession,
        1 => TrainerKind::Vocation,
        _ => TrainerKind::Guild,
    };
    if let Ok(root) = craft_fallback::trainer(&mut ui, &nv, &p6, &art, kind, &ct) {
        let mut w = CraftTrainerWindow::attach(&mut ui, &nv, &p6, root, kind, &art, &ct);
        let v = CraftTrainerView {
            window: b.u32(),
            kind,
            name: b.text(),
            text: b.text(),
            replace: b.bool().then(|| (0..b.n(5)).map(|_| b.text()).collect()),
            confirm: b.bool().then(|| b.text()),
        };
        w.show(&mut ui, &v, &ct);
        for _ in 0..b.n(8) {
            let e = event(&mut b, &mut ui, root);
            let _ = w.handle_event(&mut ui, &e);
        }
        w.detach(&mut ui);
    }
    // The experience bar and the banner.
    if let Ok(root) = fallback::experience_bar(&mut ui, &nv, &p6, &art) {
        let mut bar = ExperienceBar::attach(&ui, &gv, root);
        bar.show(
            &mut ui,
            &ExperienceView {
                level: b.u32(),
                level_text: b.text(),
                current: b.u64(),
                needed: b.u64(),
                text: b.text(),
                tooltip: b.text(),
            },
        );
    }
    if let Ok(root) = fallback::levelup_banner(&mut ui, &nv, &p6, &art, &pt) {
        let mut banner = LevelUpBanner::attach(&ui, root);
        let t0 = f64::from(b.f32());
        banner.start(&mut ui, t0, b.u32(), &b.text());
        banner.update(&mut ui, f64::from(b.f32()));
    }
    // The trait panel.
    if let Ok(root) = fallback::trait_panel(&mut ui, &nv, &p6, &art, &pt) {
        let mut tp = TraitPanel::attach(&mut ui, &nv, &p6, root, &art);
        for _ in 0..b.n(6) {
            if b.bool() {
                let v = TraitPanelView {
                    title: b.text(),
                    trees: (0..b.n(40)).map(|_| b.text()).collect(),
                    current: usize::from(b.u8()),
                    tree: b.u64(),
                    branches: (0..b.n(20))
                        .map(|_| BranchView {
                            key: b.u32(),
                            text: b.text(),
                            chosen: b.bool(),
                        })
                        .collect(),
                    cells: (0..b.n(64))
                        .map(|_| TraitCellView {
                            id: b.u64(),
                            row: b.u16(),
                            col: b.u16(),
                            icon: None,
                            name: b.text(),
                            rank: b.u16(),
                            max_rank: b.u16(),
                            rank_text: b.text(),
                            prerequisites: b.bool().then(|| b.bool()),
                            available: b.bool(),
                            tooltip: b.text(),
                        })
                        .collect(),
                    points_text: b.text(),
                    slots: (0..b.n(70))
                        .map(|_| TraitSlotView {
                            icon: None,
                            text: b.text(),
                        })
                        .collect(),
                };
                tp.show(&mut ui, &v);
            } else {
                let e = event(&mut b, &mut ui, root);
                let _ = tp.handle_event(&mut ui, &e);
            }
        }
        tp.detach(&mut ui);
    }
    // The skill-training window.
    if let Ok(root) = fallback::skill_trainer(&mut ui, &nv, &p6, &art, &pt) {
        let mut w = SkillTrainerWindow::attach(&mut ui, &nv, &p6, root, &art);
        for _ in 0..b.n(6) {
            if b.bool() {
                let v = SkillTrainerView {
                    window: b.u32() % 3,
                    title: b.text(),
                    rows: (0..b.n(24))
                        .map(|i| {
                            let coin = b.u64();
                            SkillRowView {
                                entry: i as u16,
                                icon: None,
                                name: b.text(),
                                level_text: b.text(),
                                coin,
                                coins: coin_counts(coin, &[100_000, 100, 1]),
                                trained: b.bool(),
                                unmet: b.bool(),
                            }
                        })
                        .collect(),
                    player_coin: b.bool().then(|| b.u64()),
                    empty_text: b.text(),
                };
                w.show(&mut ui, &v, &pt);
            } else {
                let e = event(&mut b, &mut ui, root);
                let _ = w.handle_event(&mut ui, &e);
            }
        }
        w.detach(&mut ui);
    }
});
