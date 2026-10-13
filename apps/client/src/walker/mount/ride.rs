// SPDX-License-Identifier: GPL-3.0-or-later
//! Mounted movement inside the walker's physics steps (`walker.md` §16.6, §16.8;
//! `mounts.md` §5, §14.6).
//!
//! A [`MountMover`] rides with the [`crate::walker::movement::PhysicsWalker`]: before each
//! controller step it sets the controller's conditions (`Mount_Is_Mounted`,
//! `Mount_Movement_Use_RampUp_Controller` from the controller flags of `mounts.md` §4.7),
//! the normal mount's speed factor (§5.1 Decision) or the war-steed drive with the gear
//! rule (§14.6); after the step it derives the gait, the published sliders and advances the
//! mount states ([`MountState`]).
//!
//! | Kind | Controller | Gait (`MountMovement_CurrentSpeed`) |
//! |---|---|---|
//! | normal | Generic, Mounted / MountedWalking layer, *m* = speed factor | unset (logged 2) |
//! | Clydesdale | as normal | 4 while walking, 6 while `Slider_Propel` ≠ 0, else 2 (§14.6 Decision) |
//! | war-steed | war-steed controller, Warsteed layer | from the speed, gear rule (§14.6) |
//! | hobby-horse | on foot | – |

use std::sync::Arc;

use tessera_formats::gameplay::mounts::{MountType, controller_flags};
use tessera_physics::controller::gait::{GAIT_CANTER, GAIT_WALK, GearBox, gait_from_speed_step};
use tessera_physics::controller::warsteed::{
    GAIT_NEUTRAL, GaitMultipliers, INPUT_THRESHOLD, WarSteedDrive,
};
use tessera_physics::controller::{
    Controller, JumpTable, LayerConditions, MovementState, TURN_KEY_SPEED_DEGREES,
};

use super::eval::{ClipKey, ElementClass, SLIDER_DEAD_BAND, Stage, TURN_DEAD_BAND, slider_sign};
use super::profile::{MountKind, MountProfile};
use super::state::{MountEvent, MountPhase, MountState, StepFacts, Timeline};

/// Most events kept for the walker between frames.
pub const MAX_EVENTS: usize = 16;

/// What one step's input was, for the mounted rules.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RideInput {
    /// The controller's input pair (*x* strafe, *y* forward), `collision.md` §11.8.
    pub x: f32,
    /// See [`RideInput::x`].
    pub y: f32,
    /// Walking style.
    pub walking: bool,
    /// Turn right − turn left (−1 … 1).
    pub turn: f32,
    /// The jump command.
    pub jump: bool,
}

/// The mounted rider's runtime (module docs).
#[derive(Debug, Clone)]
pub struct MountMover {
    profile: Arc<MountProfile>,
    state: MountState,
    gear: GearBox,
    gait: u32,
    walking: bool,
    sliders: [f32; 3],
    control_speed: f32,
    turn_degrees: f32,
    events: Vec<MountEvent>,
    jumping: bool,
    element: u32,
    lock: Option<u32>,
    /// The mounted branch's jump table (`mounts.md` §14.12), when the data gives one.
    mounted_jump: Option<JumpTable>,
    /// The controller's own (on-foot) table while the mounted one is swapped in.
    foot_jump: Option<Option<JumpTable>>,
}

impl MountMover {
    /// A mover for `profile` with `timeline`: summoning when `summon` (and the timeline has
    /// an induction), else mounted at once; `lock` fixes a war-steed's gear (a gait key 4–7).
    #[must_use]
    pub fn new(
        profile: Arc<MountProfile>,
        timeline: Timeline,
        summon: bool,
        lock: Option<u32>,
    ) -> Self {
        let mut state = MountState::default();
        state.start(timeline, summon);
        Self {
            profile,
            state,
            gear: GearBox::new(lock),
            gait: 0,
            walking: false,
            sliders: [0.0; 3],
            control_speed: 0.0,
            turn_degrees: 0.0,
            events: Vec::new(),
            jumping: false,
            element: 0,
            lock,
            mounted_jump: None,
            foot_jump: None,
        }
    }

    /// The mover with the mounted branch's jump table (`walker.md` §16.6: the jump of
    /// `collision.md` §11.10 with the mounted branch). While the rider is mounted the
    /// controller uses it instead of its on-foot table. `None` or an empty table means the
    /// mounted branch gives no jump node (the case on this install, `walker.md` §16.12): a
    /// mounted rider then cannot jump — the on-foot table is **not** used while mounted.
    #[must_use]
    pub fn with_jump(mut self, table: Option<JumpTable>) -> Self {
        self.mounted_jump = table.filter(|t| !t.is_empty());
        self
    }

    /// The mounted branch's jump table.
    #[must_use]
    pub fn mounted_jump(&self) -> Option<&JumpTable> {
        self.mounted_jump.as_ref()
    }

    /// A teleport (`walker.md` §16.5): summoning is cancelled, the mount state is kept and
    /// the rider stands still (gear, gait, sliders and speed reset).
    pub fn teleported(&mut self) {
        if self.state.phase() == MountPhase::Summoning
            && let Some(ev) = self.state.interrupt()
            && self.events.len() < MAX_EVENTS
        {
            self.events.push(ev);
        }
        self.gear = GearBox::new(self.lock);
        self.gait = 0;
        self.sliders = [0.0; 3];
        self.control_speed = 0.0;
        self.jumping = false;
    }

    /// The mount.
    #[must_use]
    pub fn profile(&self) -> &Arc<MountProfile> {
        &self.profile
    }

    /// The mount states.
    #[must_use]
    pub fn state(&self) -> &MountState {
        &self.state
    }

    /// The mount states, for requests (dismount, interrupt).
    pub fn state_mut(&mut self) -> &mut MountState {
        &mut self.state
    }

    /// `MountMovement_CurrentSpeed` of the rider: 0 on foot, 2 when unset while mounted.
    #[must_use]
    pub fn gait(&self) -> u32 {
        if !self.state.is_mounted() {
            0
        } else if self.gait == 0 {
            GAIT_NEUTRAL
        } else {
            self.gait
        }
    }

    /// The selected gear (4–7) or 0.
    #[must_use]
    pub fn locked_gear(&self) -> u32 {
        if self.profile.is_war_steed() && self.state.is_mounted() {
            self.gear.selected().unwrap_or(0)
        } else {
            0
        }
    }

    /// The fixed gear of `lock=`.
    #[must_use]
    pub fn lock(&self) -> Option<u32> {
        self.lock
    }

    /// Walking style.
    #[must_use]
    pub fn walking(&self) -> bool {
        self.walking
    }

    /// The published sliders (`Slider_Propel`, `Slider_Strafe`, `Slider_Turn`).
    #[must_use]
    pub fn sliders(&self) -> [f32; 3] {
        self.sliders
    }

    /// The forward control velocity after the last step, m/s (signed).
    #[must_use]
    pub fn control_speed(&self) -> f32 {
        self.control_speed
    }

    /// The war-steed's turn rate, degrees per second (`mounts.md` §5.6), while it applies.
    #[must_use]
    pub fn war_steed_turn_degrees(&self) -> Option<f32> {
        (self.profile.is_war_steed() && self.state.is_mounted()).then_some(self.turn_degrees)
    }

    /// The factor that turns the on-foot keyboard turn rate (`collision.md` §11.9,
    /// [`TURN_KEY_SPEED_DEGREES`]) into the war-steed's (`mounts.md` §5.6); 1 otherwise.
    #[must_use]
    pub fn turn_scale(&self) -> f32 {
        match self.war_steed_turn_degrees() {
            Some(d) if d.is_finite() && d >= 0.0 => d / TURN_KEY_SPEED_DEGREES,
            _ => 1.0,
        }
    }

    /// Takes the events since the last call.
    pub fn take_events(&mut self) -> Vec<MountEvent> {
        std::mem::take(&mut self.events)
    }

    /// The clip state key of the last step (`mounts.md` §14.5 item 3).
    #[must_use]
    pub fn clip_key(&self) -> ClipKey {
        let stage = match self.state.phase() {
            MountPhase::Summoning => Stage::Summoning,
            MountPhase::Mounting => Stage::Mounting,
            MountPhase::Dismounting => Stage::Dismounting,
            _ => Stage::Riding,
        };
        ClipKey {
            stage,
            walking: self.walking,
            propel: slider_sign(self.sliders[0], SLIDER_DEAD_BAND),
            strafe: slider_sign(self.sliders[1], SLIDER_DEAD_BAND),
            turn: slider_sign(self.sliders[2], TURN_DEAD_BAND),
            element: ElementClass::of(self.element),
            gait: if self.state.is_mounted() {
                self.gait
            } else {
                0
            },
            jump: self.jumping && self.element == MovementState::EarthNoSurface.key(),
        }
    }

    /// The conditions and drive of the controller before a step (module docs).
    pub fn before_step(&mut self, ctl: &mut Controller, input: &RideInput, dt: f32) {
        self.walking = input.walking;
        let mounted = self.state.is_mounted() && self.profile.rides();
        // The jump's mounted branch while the channelling loop runs (`mounts.md` §14.12;
        // `walker.md` §16.6, §16.12 resolution, implementer 2026-10-13). The static scan of
        // the rider's jump scripts reaches no node of the mounted branch on this install, so
        // [`MountMover::mounted_jump`] is `None`: while mounted the controller is given an
        // empty table, a mounted rider cannot jump, and the on-foot table does **not** carry
        // over. Should a later install expose the mounted branch, `with_jump` carries it and
        // it is swapped in instead.
        match (mounted, self.foot_jump) {
            (true, None) => {
                self.foot_jump = Some(ctl.config().jump);
                ctl.set_jump_table(Some(self.mounted_jump.unwrap_or_default()));
            }
            (false, Some(saved)) => {
                ctl.set_jump_table(saved);
                self.foot_jump = None;
            }
            _ => {}
        }
        let (ramp_up, _) = controller_flags(MountType(self.profile.mount_type));
        let mut c: LayerConditions = *ctl.conditions();
        c.mounted = mounted;
        c.ramp_up_controller = mounted && ramp_up;
        ctl.set_conditions(c);
        match (self.profile.kind, mounted) {
            (MountKind::WarSteed, true) => {
                let Some(setup) = self.profile.war_steed else {
                    ctl.set_war_steed(None);
                    return;
                };
                let v = ctl.war_steed().map_or(0.0, |d| d.velocity[1]);
                let g = ctl
                    .war_steed()
                    .map(|d| d.params(&ctl.params()).gear_change_rate)
                    .unwrap_or(tessera_physics::controller::warsteed::DEFAULT_GEAR_CHANGE_RATE);
                self.gear.shift(input.y, input.walking, v, g, dt);
                let gaits = &self.profile.gaits;
                let gait = self.gait();
                let slot = match gait {
                    5 => 1,
                    6 => 2,
                    7 => 3,
                    _ => 0,
                };
                let add = setup.turn_add_mods.get(slot).copied().unwrap_or(0.0);
                let multipliers = GaitMultipliers {
                    gallop_speed: gaits.speed_multiplier(7),
                    locked_gear_speed: self.gear.selected().map(|s| gaits.speed_multiplier(s)),
                    turn: gaits.turn_multiplier(gait) + add,
                };
                let mut actor = setup.actor;
                actor.gait = Some(gait);
                actor.locked_gear = self.gear.selected();
                match ctl.war_steed_mut() {
                    Some(d) => {
                        d.actor = actor;
                        d.gaits = multipliers;
                    }
                    None => ctl.set_war_steed(Some(WarSteedDrive::new(
                        actor,
                        multipliers,
                        setup.world_limit,
                    ))),
                }
                ctl.set_speed_multiplier(1.0);
                let params = ctl.war_steed().map(|d| d.params(&ctl.params()));
                self.turn_degrees = params.map_or(0.0, |p| multipliers.turn * p.turn_rate);
            }
            (MountKind::Normal | MountKind::Clydesdale, true) => {
                ctl.set_war_steed(None);
                ctl.set_speed_multiplier(self.profile.speed_factor);
            }
            _ => {
                ctl.set_war_steed(None);
                ctl.set_speed_multiplier(1.0);
            }
        }
    }

    /// The gait, sliders and states after a step of `dt` seconds (module docs). `jumping`
    /// is the controller's `Movement_IsJumping`.
    pub fn after_step(&mut self, ctl: &Controller, input: &RideInput, dt: f32) {
        let s = ctl.state();
        let element = s.element.key();
        self.element = element;
        self.jumping = ctl.is_jumping();
        let (sin, cos) = s.heading.sin_cos();
        let forward_v = s.velocity[0] * cos + s.velocity[1] * sin;
        let mounted = self.state.is_mounted() && self.profile.rides();
        match (self.profile.kind, mounted, ctl.war_steed()) {
            (MountKind::WarSteed, true, Some(d)) => {
                let v = d.velocity[1];
                let f = d.params(&ctl.params()).forward;
                let gaits = &self.profile.gaits;
                // §14.6 step 4 with the §16.11 check-3 / §16.12 crossing rule: the step whose
                // forward control velocity crosses from forward to backward reads Neutral, so
                // braking falls 4 → 2 → 1 (`self.control_speed` still holds the step before).
                let selected = self.gear.selected();
                self.gait = gait_from_speed_step(self.control_speed, v, f, selected, |g| {
                    gaits.speed_multiplier(g)
                });
                let (strafe, propel) = d
                    .last
                    .map_or((0.0, 0.0), |l| (l.strafe_slider, l.propel_slider));
                self.sliders = [propel, strafe, input.turn];
                self.control_speed = v;
            }
            (MountKind::Clydesdale, true, _) => {
                self.sliders = [input.y, input.x, input.turn];
                self.gait = if input.walking {
                    GAIT_WALK
                } else if input.y.abs() > INPUT_THRESHOLD {
                    GAIT_CANTER
                } else {
                    GAIT_NEUTRAL
                };
                self.control_speed = forward_v;
            }
            _ => {
                self.sliders = [input.y, input.x, input.turn];
                self.gait = 0;
                self.control_speed = forward_v;
            }
        }
        let facts = StepFacts {
            movement_input: input.x.abs() > INPUT_THRESHOLD
                || input.y.abs() > INPUT_THRESHOLD
                || input.jump,
            speed: s.velocity[0].hypot(s.velocity[1]),
            airborne: element == MovementState::EarthNoSurface.key(),
            in_water: matches!(element, 1..=3),
        };
        if let Some(ev) = self.state.step(dt, &facts)
            && self.events.len() < MAX_EVENTS
        {
            self.events.push(ev);
        }
        if !self.state.is_mounted() {
            self.gear = GearBox::new(self.lock);
            self.gait = 0;
        }
    }

    /// Clears what the mover set on the controller (on foot again), the on-foot jump table
    /// included.
    pub fn release(&mut self, ctl: &mut Controller) {
        if let Some(saved) = self.foot_jump.take() {
            ctl.set_jump_table(saved);
        }
        let mut c = *ctl.conditions();
        c.mounted = false;
        c.ramp_up_controller = false;
        ctl.set_conditions(c);
        ctl.set_war_steed(None);
        ctl.set_speed_multiplier(1.0);
    }
}

/// The controller's top movement layer name for the logs (`walker.md` §16.10 `move_layer`).
#[must_use]
pub fn top_layer_name(ctl: &Controller) -> &'static str {
    let c = ctl.conditions();
    let element = ctl.state().element;
    let layers = c.layers(element);
    layers.iter().last().map_or("BasePlayer", |k| k.name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::mount::eval::Stage;
    use crate::walker::mount::profile::WarSteedSetup;
    use crate::walker::movement::PhysicsWalker;
    use crate::walker::movement::run::StaticWorld;
    use crate::walker::movement::setup::MoverSetup;
    use tessera_formats::landblock::HeightGrid;
    use tessera_formats::{Did, LandblockData, LandblockId};
    use tessera_gamedata::mount::{GaitTable, MountCategory, MountClass, MountInfo};
    use tessera_physics::controller::MovementCommands;
    use tessera_physics::controller::warsteed::WarSteedActor;
    use tessera_physics::geometry::{CollisionLimits, CollisionSetBuilder, GridMesh};
    use tessera_world::LandblockKey;

    fn world() -> (StaticWorld, [f64; 3]) {
        let raw = (10.0_f32 * 65_535.0 / 2_000.0).round() as u16;
        let mut w = StaticWorld::default();
        let c = LandblockKey::new(1, 100, 100).unwrap();
        for dy in -2..=2 {
            for dx in -2..=2 {
                let k = c.offset(dx, dy).unwrap();
                let data =
                    LandblockData::from_heights(k.data_did(), HeightGrid::from_fn(|_, _| raw));
                let mut b = CollisionSetBuilder::new(CollisionLimits::default());
                b.set_landblock(LandblockId::from_did(k.data_did()).unwrap());
                b.add_grid(Arc::new(GridMesh::terrain(&data))).unwrap();
                w.sets.insert(k, Arc::new(b.build()));
            }
        }
        let o = c.origin();
        (
            w,
            [
                o[0] + 96.0,
                o[1] + 96.0,
                f64::from(raw) * 2_000.0 / 65_535.0 + 0.01,
            ],
        )
    }

    fn profile(kind: MountKind, mount_type: u32) -> MountProfile {
        let category = match kind {
            MountKind::Normal => MountCategory::Normal,
            MountKind::Clydesdale => MountCategory::Clydesdale,
            MountKind::WarSteed => MountCategory::WarSteed,
            MountKind::HobbyHorse => MountCategory::HobbyHorse,
        };
        MountProfile {
            effect: Did(0x7000_0001),
            skill: None,
            info: MountInfo {
                effect: Did(0x7000_0001),
                skill: None,
                name: None,
                icon: None,
                mount_type: Some(mount_type),
                type_labels: Vec::new(),
                war_steed: kind == MountKind::WarSteed,
                class: MountClass::Basic,
                category,
                entity: Some(Did(0x4700_0001)),
                scale: None,
            },
            mount_type,
            kind,
            in_group: true,
            entries: Vec::new(),
            // mounts.md §11.5: 1.0 + 0.62.
            speed_factor: 1.62,
            // mounts.md §11.2's actor values.
            war_steed: (kind == MountKind::WarSteed).then_some(WarSteedSetup {
                actor: WarSteedActor {
                    max_speed_cached: Some(8.3),
                    acceleration_cached: Some(16.0),
                    turn_rate_cached: Some(120.0),
                    ..WarSteedActor::default()
                },
                turn_add_mods: [0.0; 4],
                world_limit: 16.0,
            }),
            // mounts.md §14.13's multipliers.
            gaits: GaitTable {
                speed: [Some(0.5), Some(0.7), Some(0.8), Some(1.0)],
                turn: [Some(1.0), Some(0.8), Some(0.6), Some(0.47)],
            },
            induction: None,
            start_script: None,
            dismount_by_player: None,
            camera: Some([0.2, 0.75]),
            notes: Vec::new(),
        }
    }

    /// A walker on flat ground mounted (instant) on `p`, settled.
    fn mounted(p: MountProfile) -> (PhysicsWalker, StaticWorld) {
        let (w, feet) = world();
        let setup = MoverSetup::builtin();
        let mut walker = PhysicsWalker::new(&setup, 1, feet, 0.0);
        walker.check_placement();
        let still = MovementCommands::default();
        for _ in 0..30 {
            walker.step_commands(&still, false, None, false, &w);
        }
        let timeline = Timeline {
            fade_in: 0.25,
            fade_out: 0.25,
            ..Timeline::default()
        };
        walker.set_mount(Some(MountMover::new(Arc::new(p), timeline, false, None)));
        (walker, w)
    }

    fn steps(walker: &mut PhysicsWalker, w: &StaticWorld, forward: f32, walking: bool, n: usize) {
        let c = MovementCommands::from_axes(forward, 0.0, 0.0);
        for _ in 0..n {
            assert!(walker.step_commands(&c, walking, None, false, w).is_some());
        }
    }

    /// `mounts.md` §5.1 Decision (example §11.5), §4.7 controller flags: a normal mount
    /// rides the Generic controller on the Mounted layer (MountedWalking while walking)
    /// with the speed factor; the gait stays unset.
    #[test]
    fn normal_mounts_take_the_generic_controller_with_the_speed_factor() {
        let (mut walker, w) = mounted(profile(MountKind::Normal, 0x1));
        steps(&mut walker, &w, 1.0, false, 120);
        let c = walker.controller();
        assert!(c.conditions().mounted && !c.conditions().ramp_up_controller);
        assert!(c.war_steed().is_none());
        assert!((c.speed_multiplier() - 1.62).abs() < 1e-6);
        assert_eq!(top_layer_name(c), "Mounted");
        let target = walker.last().map(|r| r.target_speed).unwrap();
        let forward = c.params().forward_speed;
        assert!((target - forward * 1.62).abs() < 1e-3, "{target}");
        let m = walker.mount().unwrap();
        assert_eq!(m.gait(), GAIT_NEUTRAL);
        assert_eq!(m.locked_gear(), 0);
        assert_eq!(m.clip_key().propel, 1);
        assert_eq!(m.clip_key().stage, Stage::Riding);
        steps(&mut walker, &w, 1.0, true, 30);
        assert_eq!(top_layer_name(walker.controller()), "MountedWalking");
        assert!(walker.mount().unwrap().clip_key().walking);
    }

    /// `mounts.md` §5.2–§5.5, §14.6 (examples §11.2, §14.13): a war-steed type selects the
    /// Warsteed layer and the war-steed controller; holding forward shifts Walk → Gallop one
    /// gear per 0.5 s, the gait follows the speed, and the speed never exceeds the gear's
    /// limit; the turn rate is the gait's multiplier × *R*.
    #[test]
    fn war_steeds_take_the_war_steed_controller_and_shift_gears() {
        let (mut walker, w) = mounted(profile(MountKind::WarSteed, 0x20));
        let mut gaits: Vec<u32> = Vec::new();
        let c = MovementCommands::from_axes(1.0, 0.0, 0.0);
        for i in 0..150 {
            walker.step_commands(&c, false, None, false, &w).unwrap();
            let m = walker.mount().unwrap();
            if gaits.last() != Some(&m.gait()) {
                gaits.push(m.gait());
            }
            let gear = m.locked_gear();
            let limit = [0.5, 0.7, 0.8, 1.0][usize::try_from(gear - 4).unwrap()] * 8.3;
            assert!(m.control_speed() <= limit + 1e-4, "step {i}");
        }
        let ctl = walker.controller();
        assert!(ctl.conditions().mounted && ctl.conditions().ramp_up_controller);
        assert!(ctl.war_steed().is_some());
        assert_eq!(top_layer_name(ctl), "Warsteed");
        assert_eq!(gaits, vec![4, 5, 6, 7]);
        let m = walker.mount().unwrap();
        assert!((m.control_speed() - 8.3).abs() < 1e-3);
        // Gallop turn multiplier 0.47 × R 120 = 56.4 °/s (§11.3).
        assert!((m.war_steed_turn_degrees().unwrap() - 56.4).abs() < 1e-3);
        assert!((m.turn_scale() - 56.4 / 150.0).abs() < 1e-5);
        // Released: momentum keeps the speed and the gait.
        steps(&mut walker, &w, 0.0, false, 60);
        let m = walker.mount().unwrap();
        assert!((m.control_speed() - 8.3).abs() < 1e-3);
        assert_eq!(m.gait(), 7);
        // Braking: the gaits fall through Neutral (2) to Backwards (1) (`walker.md` §16.11
        // check 3, §16.12: the step whose forward velocity crosses zero reads Neutral).
        let mut back: Vec<u32> = Vec::new();
        let c = MovementCommands::from_axes(-1.0, 0.0, 0.0);
        for _ in 0..120 {
            walker.step_commands(&c, false, None, false, &w).unwrap();
            let g = walker.mount().unwrap().gait();
            if back.last() != Some(&g) {
                back.push(g);
            }
        }
        assert_eq!(back.last(), Some(&1), "{back:?}");
        assert!(back.contains(&2), "Neutral at the crossing: {back:?}");
    }

    /// `mounts.md` §14.6 Decision for the Clydesdale; §14.11 the hobby-horse stays on foot.
    #[test]
    fn clydesdale_gait_rule_and_hobby_horse_on_foot() {
        let (mut walker, w) = mounted(profile(MountKind::Clydesdale, 0x8000));
        steps(&mut walker, &w, 0.0, false, 5);
        assert_eq!(walker.mount().unwrap().gait(), 2);
        steps(&mut walker, &w, 1.0, false, 5);
        assert_eq!(walker.mount().unwrap().gait(), 6);
        steps(&mut walker, &w, 1.0, true, 5);
        assert_eq!(walker.mount().unwrap().gait(), 4);
        assert_eq!(top_layer_name(walker.controller()), "MountedWalking");

        let (mut walker, w) = mounted(profile(MountKind::HobbyHorse, 0x400));
        steps(&mut walker, &w, 1.0, false, 30);
        let c = walker.controller();
        assert!(
            !c.conditions().mounted,
            "the hobby-horse rider stays on foot"
        );
        assert_eq!(top_layer_name(c), "BasePlayer");
        assert_eq!(walker.mount().unwrap().state().phase(), MountPhase::Mounted);
    }

    /// `walker.md` §16.5: a dismount on the move ends at once, the mount fades out, the
    /// controller is on foot again and the mover is removed once faded.
    #[test]
    fn dismount_releases_the_controller() {
        let (mut walker, w) = mounted(profile(MountKind::WarSteed, 0x20));
        steps(&mut walker, &w, 1.0, false, 30);
        let moving = true;
        assert!(
            walker
                .mount_mut()
                .unwrap()
                .state_mut()
                .dismount(true, moving)
                .is_none()
        );
        steps(&mut walker, &w, 0.0, false, 1);
        let c = walker.controller();
        assert!(!c.conditions().mounted && c.war_steed().is_none());
        assert!((c.speed_multiplier() - 1.0).abs() < 1e-6);
        steps(&mut walker, &w, 0.0, false, 30);
        assert!(walker.mount().is_none(), "removed after the fade");
        assert!(walker.take_mount_events().is_empty());
    }

    /// `walker.md` §16.6, `mounts.md` §14.12: while mounted the controller jumps with the
    /// mounted branch's table; the on-foot table returns with the dismount.
    #[test]
    fn mounted_jump_table_is_swapped_in_and_out() {
        let (w, feet) = world();
        let setup = MoverSetup::builtin();
        let mut walker = PhysicsWalker::new(&setup, 1, feet, 0.0);
        walker.check_placement();
        steps(&mut walker, &w, 0.0, false, 10);
        let on_foot = walker.controller().config().jump;
        let mut mounted = JumpTable::new();
        for k in 1..=6 {
            let st = MovementState::from_key(k).unwrap();
            mounted.set(st, tessera_physics::controller::JumpNode::new(0.285, 15.0));
        }
        let timeline = Timeline {
            fade_out: 0.25,
            ..Timeline::default()
        };
        walker.set_mount(Some(
            MountMover::new(
                Arc::new(profile(MountKind::Normal, 0x1)),
                timeline,
                false,
                None,
            )
            .with_jump(Some(mounted)),
        ));
        steps(&mut walker, &w, 0.0, false, 1);
        assert_eq!(walker.controller().config().jump, Some(mounted));
        walker.mount_mut().unwrap().state_mut().dismount(true, true);
        steps(&mut walker, &w, 0.0, false, 1);
        assert_eq!(walker.controller().config().jump, on_foot, "on foot again");
        // Taking the mover away at once restores it too.
        walker.set_mount(Some(
            MountMover::new(
                Arc::new(profile(MountKind::Normal, 0x1)),
                timeline,
                false,
                None,
            )
            .with_jump(Some(mounted)),
        ));
        steps(&mut walker, &w, 0.0, false, 1);
        walker.set_mount(None);
        assert_eq!(walker.controller().config().jump, on_foot);
        // No mounted branch (this install, `walker.md` §16.12): while mounted the controller
        // gets an empty jump table, so a mounted rider cannot jump and the on-foot table does
        // not carry over; it returns on dismount.
        walker.set_mount(Some(MountMover::new(
            Arc::new(profile(MountKind::Normal, 0x1)),
            timeline,
            false,
            None,
        )));
        steps(&mut walker, &w, 0.0, false, 1);
        assert_eq!(
            walker.controller().config().jump,
            Some(JumpTable::new()),
            "no jump while mounted without a mounted branch"
        );
        walker.set_mount(None);
        assert_eq!(walker.controller().config().jump, on_foot);
    }

    /// `walker.md` §16.5: a teleport keeps the mount but stops the steed (stored velocity,
    /// gear and gait); summoning is cancelled.
    #[test]
    fn teleports_stop_the_steed_and_cancel_summoning() {
        let (mut walker, w) = mounted(profile(MountKind::WarSteed, 0x20));
        steps(&mut walker, &w, 1.0, false, 120);
        assert!(walker.mount().unwrap().gait() >= 6);
        let p = walker.state().position;
        assert!(walker.teleport(1, [p[0] - 20.0, p[1], p[2]], 0.0));
        let d = walker.controller().war_steed().unwrap();
        assert_eq!(d.velocity, [0.0; 2]);
        let m = walker.mount().unwrap();
        assert_eq!(m.state().phase(), MountPhase::Mounted, "the state is kept");
        assert_eq!((m.gait(), m.locked_gear()), (GAIT_NEUTRAL, 0));
        steps(&mut walker, &w, 0.0, false, 30);
        assert!(
            walker.mount().unwrap().control_speed().abs() < 1e-3,
            "no coasting"
        );

        let (w, feet) = world();
        let mut walker = PhysicsWalker::new(&MoverSetup::builtin(), 1, feet, 0.0);
        walker.check_placement();
        steps(&mut walker, &w, 0.0, false, 10);
        let timeline = Timeline {
            summon: Some(2.0),
            ..Timeline::default()
        };
        walker.set_mount(Some(MountMover::new(
            Arc::new(profile(MountKind::Normal, 0x1)),
            timeline,
            true,
            None,
        )));
        assert_eq!(
            walker.mount().unwrap().state().phase(),
            MountPhase::Summoning
        );
        let p = walker.state().position;
        assert!(walker.teleport(1, p, 0.0));
        assert_eq!(
            walker.mount().map(|m| m.state().phase()),
            Some(MountPhase::OnFoot)
        );
        assert_eq!(
            walker.take_mount_events(),
            vec![MountEvent::SummonInterrupted]
        );
    }

    /// A ridden war-steed held at an absent landblock (`walker.md` §13) stops: its stored
    /// control velocity is zeroed with the mover's, so it does not leave at full speed.
    #[test]
    fn the_boundary_hold_stops_a_war_steed() {
        let raw = (10.0_f32 * 65_535.0 / 2_000.0).round() as u16;
        let mut w = StaticWorld::default();
        let k = LandblockKey::new(1, 100, 100).unwrap();
        let data = LandblockData::from_heights(k.data_did(), HeightGrid::from_fn(|_, _| raw));
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        b.set_landblock(LandblockId::from_did(k.data_did()).unwrap());
        b.add_grid(Arc::new(GridMesh::terrain(&data))).unwrap();
        w.sets.insert(k, Arc::new(b.build()));
        let o = k.origin();
        let feet = [
            o[0] + tessera_world::LANDBLOCK_SIZE - 25.0,
            o[1] + tessera_world::LANDBLOCK_SIZE / 2.0,
            f64::from(raw) * 2_000.0 / 65_535.0 + 0.01,
        ];
        // Heading 0 is east (+x).
        let mut walker = PhysicsWalker::new(&MoverSetup::builtin(), 1, feet, 0.0);
        walker.check_placement();
        steps(&mut walker, &w, 0.0, false, 10);
        walker.set_mount(Some(MountMover::new(
            Arc::new(profile(MountKind::WarSteed, 0x20)),
            Timeline::default(),
            false,
            None,
        )));
        let mut held = false;
        for _ in 0..240 {
            let c = MovementCommands::from_axes(1.0, 0.0, 0.0);
            if let Some(r) = walker.step_commands(&c, false, None, false, &w)
                && r.at_boundary
            {
                held = true;
                break;
            }
        }
        assert!(
            held,
            "reached the absent landblock: {:?} from {feet:?}",
            walker.state().position
        );
        assert_eq!(walker.controller().war_steed().unwrap().velocity, [0.0; 2]);
        // Released at the boundary: no coasting into it.
        steps(&mut walker, &w, 0.0, false, 1);
        assert!(walker.mount().unwrap().control_speed().abs() < 1e-3);
    }
}
