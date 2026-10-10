// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the character-screen tests (`character_screens.rs`,
//! `character_screens_real_data.rs`): driving [`CharacterScreens`] over a session frame by
//! frame, and a synthetic install model (two races, invented keys) for tests without data.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tessera_client::screens::controls::{
    AvatarDetail, ControlChoices, ControlDetail, ControlValue, ScreenPids,
};
use tessera_client::screens::{Action, CharacterScreens, ScreenData};
use tessera_formats::Did;
use tessera_gamedata::chargen::{
    AppearanceControl, AvatarControls, ChargenData, ClassChoice, ControlKind, GenderEntry,
    NationalityRecord, RaceChoice, RaceRecord,
};
use tessera_gamedata::{GENDER_FEMALE, GENDER_MALE};
use tessera_net::{Event, PropertyKind, Session};

/// One client frame.
pub const FRAME: Duration = Duration::from_millis(16);

/// A session with its screens and a clock.
#[derive(Debug)]
pub struct Driver {
    pub session: Session,
    pub screens: CharacterScreens,
    pub now: Instant,
    pub events: Vec<Event>,
}

impl Driver {
    pub fn new(session: Session, screens: CharacterScreens) -> Self {
        Self {
            session,
            screens,
            now: Instant::now(),
            events: Vec::new(),
        }
    }

    /// One frame.
    pub fn frame(&mut self) {
        self.screens
            .pump(&mut self.session, self.now, &mut self.events);
        self.now += FRAME;
    }

    /// Frames until `done` holds (at most `max` frames); panics otherwise.
    pub fn until(&mut self, what: &str, max: usize, mut done: impl FnMut(&Self) -> bool) {
        for _ in 0..max {
            self.frame();
            if done(self) {
                return;
            }
        }
        panic!(
            "timed out waiting for {what}: status {:?}, stage {:?}, errors {:?}",
            self.screens.status(),
            self.screens.model().stage(),
            self.screens.model().error_texts()
        );
    }

    /// Frames until `done`, sleeping between frames (a networked session).
    pub fn until_real(&mut self, what: &str, limit: Duration, mut done: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + limit;
        loop {
            self.now = Instant::now();
            self.screens
                .pump(&mut self.session, self.now, &mut self.events);
            if done(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}: status {:?}, stage {:?}, errors {:?}, session {:?}",
                self.screens.status(),
                self.screens.model().stage(),
                self.screens.model().error_texts(),
                self.session.state()
            );
            std::thread::sleep(FRAME);
        }
    }

    /// The player's action now, then one frame.
    pub fn act(&mut self, a: Action) {
        self.screens.act(a, self.now);
        self.frame();
    }
}

/// Invented keys of the synthetic install.
pub const SPECIES_A: u32 = 0x0A01;
pub const SPECIES_B: u32 = 0x0B02;
pub const CLASS_1: u32 = 101;
pub const CLASS_2: u32 = 102;
pub const CLASS_3: u32 = 103;
pub const NAT_A: u32 = 11;
pub const NAT_B: u32 = 12;
pub const AVATAR_AM: Did = Did(0x7000_0A01);
pub const AVATAR_AF: Did = Did(0x7000_0A02);
pub const AVATAR_BM: Did = Did(0x7000_0B01);
/// The written properties of the synthetic controls.
pub const PROP_HEAD: u32 = 0x1000_0100;
pub const PROP_HAIR_COLOUR: u32 = 0x1000_0101;
pub const PROP_SCALE: u32 = 0x1000_0102;

fn race(
    did: u32,
    species: u32,
    genders: &[(u32, Did)],
    classes: &[u32],
    nats: &[u32],
) -> RaceChoice {
    let record = RaceRecord {
        race: species,
        genders: genders
            .iter()
            .map(|&(gender, avatar)| GenderEntry {
                gender,
                player_avatar: Some(avatar),
                race_select_entity: Some(Did(0x4700_0000 | gender)),
                ..GenderEntry::default()
            })
            .collect(),
        classes: classes.to_vec(),
        nationalities: nats.to_vec(),
        ui_scene: Some(Did(0x2100_0000 | species)),
        chargen_camera: Some(3),
        select_camera: Some(4),
        ..RaceRecord::default()
    };
    let mut r = RaceChoice::from_record(Did(did), record);
    for g in &mut r.genders {
        g.chargen_zoom_camera = Some(5);
        g.select_zoom_camera = Some(6);
    }
    r
}

fn control(index: usize, kind: ControlKind) -> AppearanceControl {
    AppearanceControl {
        index,
        availability: 1,
        group: Some(1),
        kind,
    }
}

/// A synthetic avatar: a head style (int property), a hair colour, a body scale.
pub fn avatar_detail() -> AvatarDetail {
    AvatarDetail {
        entity: Some(Did(0x4700_1000)),
        controls: vec![
            ControlDetail {
                control: control(
                    0,
                    ControlKind::Style {
                        file: None,
                        key: Some(0x1000_0009),
                        script_property: Some(PROP_HEAD),
                    },
                ),
                writes: Some((PROP_HEAD, PropertyKind::Int)),
                choices: ControlChoices::Styles {
                    key: Some(0x1000_0009),
                    variants: vec![0.0, 0.01, 0.02, 0.3, 0.31],
                },
                default: Some(ControlValue::Style(0)),
            },
            ControlDetail {
                control: control(
                    1,
                    ControlKind::Colour {
                        property: Some(PROP_HAIR_COLOUR),
                        palette: None,
                    },
                ),
                writes: Some((PROP_HAIR_COLOUR, PropertyKind::Colour)),
                choices: ControlChoices::Palette(None),
                default: None,
            },
            ControlDetail {
                control: control(
                    2,
                    ControlKind::BodyScale {
                        min: Some(0.0),
                        max: Some(1.0),
                    },
                ),
                writes: Some((PROP_SCALE, PropertyKind::Float)),
                choices: ControlChoices::Range { min: 0.0, max: 1.0 },
                default: Some(ControlValue::Float(0.5)),
            },
        ],
        notes: Vec::new(),
    }
}

/// Two races: A (male and female, classes 1–3 with priorities 3, 1, 2, nationalities A
/// and B), B (male only, class 2, nationality B).
pub fn synthetic_data() -> Arc<ScreenData> {
    let mut chargen = ChargenData {
        races: vec![
            race(
                0x7000_00A0,
                SPECIES_A,
                &[(GENDER_MALE, AVATAR_AM), (GENDER_FEMALE, AVATAR_AF)],
                &[CLASS_1, CLASS_2, CLASS_3],
                &[NAT_A, NAT_B],
            ),
            race(
                0x7000_00B0,
                SPECIES_B,
                &[(GENDER_MALE, AVATAR_BM)],
                &[CLASS_2],
                &[NAT_B],
            ),
        ],
        ..ChargenData::default()
    };
    chargen.priority.entries = vec![(CLASS_1, 3), (CLASS_2, 1), (CLASS_3, 2)];
    for c in [CLASS_1, CLASS_2, CLASS_3] {
        chargen.classes.insert(
            c,
            ClassChoice {
                class: c,
                table: Some(Did(0x7000_1000 + c)),
                ..ClassChoice::default()
            },
        );
    }
    for n in [NAT_A, NAT_B] {
        chargen.nationalities.insert(
            n,
            (
                Did(0x7000_2000 + n),
                NationalityRecord {
                    nationality: n,
                    ..NationalityRecord::default()
                },
            ),
        );
    }
    let mut avatars = BTreeMap::new();
    for a in [AVATAR_AM, AVATAR_AF, AVATAR_BM] {
        let d = avatar_detail();
        chargen.controls.insert(
            a,
            Ok(AvatarControls {
                entity: d.entity.unwrap(),
                all: d.controls.iter().map(|c| c.control).collect(),
                creation: d.controls.iter().map(|c| c.control).collect(),
            }),
        );
        avatars.insert(a, d);
    }
    Arc::new(ScreenData {
        chargen,
        names: None,
        avatars,
        pids: ScreenPids {
            bone_scale: Some(PROP_SCALE),
            ..ScreenPids::default()
        },
        stage_keys: Default::default(),
    })
}
