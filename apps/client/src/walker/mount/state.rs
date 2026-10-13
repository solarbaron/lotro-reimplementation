// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's mount states (`walker.md` §16.5; timeline `mounts.md` §14.7 Decision): on
//! foot, summoning, mounting, mounted and dismounting, on the walker's step clock.
//!
//! [`MountState`] is a pure state machine: the caller feeds it requests and, once per
//! controller step, what the rider does ([`StepFacts`]); it answers what the controller and
//! the figures must show ([`MountState::is_mounted`], [`MountState::mount_alpha`], …). The
//! durations come from the install ([`Timeline`]); nothing here reads data.

/// Opacity change of the mount's fade when the data gives none, seconds (never used with
/// this install's data: every mount node stores 0.25 s, `mounts.md` §4.4).
pub const FALLBACK_FADE: f32 = 0.0;
/// A horizontal speed above this cancels summoning, m/s (`walker.md` §16.5).
pub const SUMMON_CANCEL_SPEED: f32 = 0.05;
/// Most seconds any one phase may last (bounds a hostile duration).
pub const MAX_PHASE_SECONDS: f32 = 60.0;

/// The walker's mount state (`walker.md` §16.5), with its log number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MountPhase {
    /// 0: on foot.
    #[default]
    OnFoot,
    /// 1: the skill's induction runs on the rider.
    Summoning,
    /// 2: the start script runs; the mount is not drawn yet.
    Mounting,
    /// 3: riding.
    Mounted,
    /// 4: the dismount script runs.
    Dismounting,
}

impl MountPhase {
    /// The log number 0–4.
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            Self::OnFoot => 0,
            Self::Summoning => 1,
            Self::Mounting => 2,
            Self::Mounted => 3,
            Self::Dismounting => 4,
        }
    }

    /// The HUD word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::OnFoot => "on foot",
            Self::Summoning => "summoning",
            Self::Mounting => "mounting",
            Self::Mounted => "mounted",
            Self::Dismounting => "dismounting",
        }
    }
}

/// The durations of one mount (`mounts.md` §14.7, §14.8), seconds, read from the install.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    /// `Induction_Duration` of the skill's induction record (`None`: no summoning phase).
    pub summon: Option<f32>,
    /// How long the start script's graph runs before the walker starts the mesh and the
    /// loop (`mounts.md` §14.7).
    pub mounting: f32,
    /// The wait of the dismount script by the player at rest before the loop stops.
    pub dismount_wait: f32,
    /// The mount's `FadeInTime`.
    pub fade_in: f32,
    /// The mount's `FadeOutTime`.
    pub fade_out: f32,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            summon: None,
            mounting: 0.0,
            dismount_wait: 0.0,
            fade_in: FALLBACK_FADE,
            fade_out: FALLBACK_FADE,
        }
    }
}

fn seconds(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, MAX_PHASE_SECONDS)
    } else {
        0.0
    }
}

impl Timeline {
    /// The timeline with every duration finite and within 0 … [`MAX_PHASE_SECONDS`].
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            summon: self.summon.map(seconds),
            mounting: seconds(self.mounting),
            dismount_wait: seconds(self.dismount_wait),
            fade_in: seconds(self.fade_in),
            fade_out: seconds(self.fade_out),
        }
    }
}

/// What the rider did in one step, for the cancel and dismount rules of `walker.md` §16.5.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepFacts {
    /// Forward, back or strafe input, or a jump.
    pub movement_input: bool,
    /// Horizontal speed of the feet, m/s.
    pub speed: f32,
    /// `Entity_Element` 4 (in the air).
    pub airborne: bool,
    /// `Entity_Element` 1, 2 or 3 (in water).
    pub in_water: bool,
}

/// Why a phase ended other than by its own end (for the HUD and the log notes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MountEvent {
    /// Summoning met movement, a fall, water, fly mode or another request.
    SummonInterrupted,
    /// A mounted rider entered water (dismounted by other).
    WaterDismount,
}

impl MountEvent {
    /// The HUD text (`walker.md` §16.5, §16.9).
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::SummonInterrupted => "Summoning interrupted.",
            Self::WaterDismount => "Dismounted in water.",
        }
    }
}

/// The state machine (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MountState {
    phase: MountPhase,
    timeline: Timeline,
    /// Seconds spent in the phase.
    time: f32,
    /// The dismount is the player's (status 7) rather than another cause (6).
    by_player: bool,
    /// The dismount waits (the player's, at rest).
    wait: bool,
    /// The mount's opacity.
    alpha: f32,
    /// The mount is drawn (appearing, shown or fading out).
    drawn: bool,
    /// Seconds in the whole ride (for the state summary).
    ridden: f32,
}

impl MountState {
    /// The current phase.
    #[must_use]
    pub fn phase(&self) -> MountPhase {
        self.phase
    }

    /// Seconds spent in the phase.
    #[must_use]
    pub fn time(&self) -> f32 {
        self.time
    }

    /// The timeline in use.
    #[must_use]
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// Seconds of summoning left (0 outside summoning).
    #[must_use]
    pub fn summon_left(&self) -> f32 {
        match (self.phase, self.timeline.summon) {
            (MountPhase::Summoning, Some(d)) => (d - self.time).max(0.0),
            _ => 0.0,
        }
    }

    /// `Mount_Is_Mounted` (`mounts.md` §4.6): from the loop's start to its stop (mounted,
    /// and dismounting until its wait ends).
    #[must_use]
    pub fn is_mounted(&self) -> bool {
        matches!(self.phase, MountPhase::Mounted | MountPhase::Dismounting)
    }

    /// Whether the Decision of §4.1 applies to the rider (the rider fields are set): from
    /// mounting to the end of dismounting.
    #[must_use]
    pub fn rider_fields_set(&self) -> bool {
        matches!(
            self.phase,
            MountPhase::Mounting | MountPhase::Mounted | MountPhase::Dismounting
        )
    }

    /// The `Mount_CurrentMountStatus` of the rider: 5 while mounted, 7 or 6 while
    /// dismounting, else 0 (`mounts.md` §4.1, §4.3).
    #[must_use]
    pub fn status(&self) -> u32 {
        match self.phase {
            MountPhase::Mounted => 5,
            MountPhase::Dismounting => {
                if self.by_player {
                    7
                } else {
                    6
                }
            }
            _ => 0,
        }
    }

    /// The mount's opacity, 0–1 (`mounts.md` §14.8 item 3).
    #[must_use]
    pub fn mount_alpha(&self) -> f32 {
        if self.drawn { self.alpha } else { 0.0 }
    }

    /// Whether the mount is drawn (also during its fade-out after the dismount).
    #[must_use]
    pub fn mount_drawn(&self) -> bool {
        self.drawn
    }

    /// Whether anything is going on (not on foot, or the mount still fading out).
    #[must_use]
    pub fn active(&self) -> bool {
        self.phase != MountPhase::OnFoot || self.drawn
    }

    /// Seconds ridden since the last mount began.
    #[must_use]
    pub fn ridden(&self) -> f32 {
        self.ridden
    }

    /// Starts a mount with `timeline`: summoning when `summon` and the timeline has an
    /// induction, else mounted at once with the mount at full opacity (`instant`). Returns
    /// false (nothing changes) unless on foot.
    pub fn start(&mut self, timeline: Timeline, summon: bool) -> bool {
        if self.phase != MountPhase::OnFoot {
            return false;
        }
        let timeline = timeline.sanitized();
        *self = Self {
            timeline,
            ..Self::default()
        };
        if summon && timeline.summon.is_some() {
            self.phase = MountPhase::Summoning;
        } else if summon {
            self.enter_mounting();
        } else {
            self.phase = MountPhase::Mounted;
            self.drawn = true;
            self.alpha = 1.0;
        }
        true
    }

    fn enter_mounting(&mut self) {
        self.phase = MountPhase::Mounting;
        self.time = 0.0;
    }

    fn enter_mounted(&mut self) {
        self.phase = MountPhase::Mounted;
        self.time = 0.0;
        self.drawn = true;
        if self.timeline.fade_in <= 0.0 {
            self.alpha = 1.0;
        }
    }

    /// A dismount request (`by_player`: status 7, else 6). Summoning is cancelled instead
    /// (returns [`MountEvent::SummonInterrupted`]); mounting and mounted dismount; the wait
    /// applies only to the player's dismount at rest (`moving` false). Returns the event,
    /// if any, and does nothing on foot or while already dismounting.
    pub fn dismount(&mut self, by_player: bool, moving: bool) -> Option<MountEvent> {
        match self.phase {
            MountPhase::OnFoot | MountPhase::Dismounting => None,
            MountPhase::Summoning => {
                self.cancel();
                Some(MountEvent::SummonInterrupted)
            }
            MountPhase::Mounting | MountPhase::Mounted => {
                self.by_player = by_player;
                self.wait = by_player && !moving && self.phase == MountPhase::Mounted;
                self.phase = MountPhase::Dismounting;
                self.time = 0.0;
                if !self.wait {
                    self.finish_dismount();
                }
                None
            }
        }
    }

    fn finish_dismount(&mut self) {
        self.phase = MountPhase::OnFoot;
        self.time = 0.0;
        self.wait = false;
        if self.timeline.fade_out <= 0.0 {
            self.drawn = false;
            self.alpha = 0.0;
        }
    }

    fn cancel(&mut self) {
        self.phase = MountPhase::OnFoot;
        self.time = 0.0;
        self.drawn = false;
        self.alpha = 0.0;
    }

    /// Cancels summoning or dismounts by other at once (fly mode, a teleport during
    /// summoning, another request): the event says which.
    pub fn interrupt(&mut self) -> Option<MountEvent> {
        match self.phase {
            MountPhase::Summoning => {
                self.cancel();
                Some(MountEvent::SummonInterrupted)
            }
            MountPhase::Mounting | MountPhase::Mounted | MountPhase::Dismounting => {
                self.by_player = false;
                self.finish_dismount();
                None
            }
            MountPhase::OnFoot => None,
        }
    }

    /// One step of `dt` seconds with what the rider did; returns the event of a cancel or
    /// a water dismount.
    pub fn step(&mut self, dt: f32, facts: &StepFacts) -> Option<MountEvent> {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let mut event = None;
        // The fade runs in every phase (also after the dismount, on foot).
        if self.drawn {
            let appearing = self.phase != MountPhase::OnFoot;
            if appearing {
                self.alpha = if self.timeline.fade_in > 0.0 {
                    (self.alpha + dt / self.timeline.fade_in).min(1.0)
                } else {
                    1.0
                };
            } else {
                self.alpha = if self.timeline.fade_out > 0.0 {
                    (self.alpha - dt / self.timeline.fade_out).max(0.0)
                } else {
                    0.0
                };
                if self.alpha <= 0.0 {
                    self.drawn = false;
                }
            }
        }
        if self.phase != MountPhase::OnFoot {
            self.ridden += dt;
        }
        self.time += dt;
        match self.phase {
            MountPhase::OnFoot => {}
            MountPhase::Summoning => {
                let moved = facts.movement_input
                    || (facts.speed.is_finite() && facts.speed > SUMMON_CANCEL_SPEED)
                    || facts.airborne
                    || facts.in_water;
                if moved {
                    self.cancel();
                    event = Some(MountEvent::SummonInterrupted);
                } else if self.timeline.summon.is_none_or(|d| self.time >= d - 1e-4) {
                    self.enter_mounting();
                }
            }
            MountPhase::Mounting => {
                if self.time >= self.timeline.mounting - 1e-4 {
                    self.enter_mounted();
                }
            }
            MountPhase::Mounted => {
                if facts.in_water {
                    self.by_player = false;
                    self.phase = MountPhase::Dismounting;
                    self.finish_dismount();
                    event = Some(MountEvent::WaterDismount);
                }
            }
            MountPhase::Dismounting => {
                if !self.wait || self.time >= self.timeline.dismount_wait - 1e-4 {
                    self.finish_dismount();
                }
            }
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn timeline() -> Timeline {
        Timeline {
            summon: Some(2.0),
            mounting: 0.4,
            dismount_wait: 1.5,
            fade_in: 0.25,
            fade_out: 0.25,
        }
    }

    fn run(s: &mut MountState, seconds: f32, facts: StepFacts) -> Vec<(f32, MountPhase)> {
        let mut out = Vec::new();
        let n = (seconds / DT).round() as usize;
        for i in 0..n {
            let before = s.phase();
            s.step(DT, &facts);
            if s.phase() != before {
                out.push(((i + 1) as f32 * DT, s.phase()));
            }
        }
        out
    }

    #[test]
    fn summon_mount_and_dismount_follow_the_timeline() {
        // walker.md §16.11 check 4: 1 → 2 → 3 at the induction duration and the start
        // script's length (±1 frame).
        let mut s = MountState::default();
        assert!(s.start(timeline(), true));
        assert_eq!(s.phase(), MountPhase::Summoning);
        assert!(!s.is_mounted() && !s.mount_drawn());
        let changes = run(&mut s, 3.0, StepFacts::default());
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!((changes[0].0 - 2.0).abs() <= DT + 1e-4);
        assert_eq!(changes[0].1, MountPhase::Mounting);
        assert!((changes[1].0 - 2.4).abs() <= DT + 1e-4);
        assert_eq!(changes[1].1, MountPhase::Mounted);
        assert!(s.is_mounted() && s.status() == 5);
        assert!(
            (s.mount_alpha() - 1.0).abs() < 1e-6,
            "faded in after 0.25 s"
        );
        // The player's dismount at rest waits 1.5 s, then fades out over 0.25 s.
        assert!(!s.start(timeline(), false), "only on foot");
        assert_eq!(s.dismount(true, false), None);
        assert_eq!(s.phase(), MountPhase::Dismounting);
        assert_eq!(s.status(), 7);
        assert!(s.is_mounted());
        let changes = run(&mut s, 1.6, StepFacts::default());
        assert_eq!(changes.len(), 1);
        assert!((changes[0].0 - 1.5).abs() <= DT + 1e-4);
        assert!(!s.is_mounted() && s.mount_drawn());
        run(&mut s, 0.3, StepFacts::default());
        assert!(!s.mount_drawn() && !s.active());
    }

    #[test]
    fn movement_cancels_summoning_and_water_dismounts() {
        let mut s = MountState::default();
        s.start(timeline(), true);
        run(&mut s, 0.5, StepFacts::default());
        let ev = s.step(
            DT,
            &StepFacts {
                movement_input: true,
                ..StepFacts::default()
            },
        );
        assert_eq!(ev, Some(MountEvent::SummonInterrupted));
        assert_eq!(s.phase(), MountPhase::OnFoot);
        for facts in [
            StepFacts {
                speed: 0.06,
                ..StepFacts::default()
            },
            StepFacts {
                airborne: true,
                ..StepFacts::default()
            },
            StepFacts {
                in_water: true,
                ..StepFacts::default()
            },
        ] {
            let mut s = MountState::default();
            s.start(timeline(), true);
            assert_eq!(s.step(DT, &facts), Some(MountEvent::SummonInterrupted));
        }
        // Instant: mounted at once at full opacity; water dismounts by other, no wait.
        let mut s = MountState::default();
        s.start(timeline(), false);
        assert_eq!(s.phase(), MountPhase::Mounted);
        assert_eq!(s.mount_alpha(), 1.0);
        let ev = s.step(
            DT,
            &StepFacts {
                in_water: true,
                ..StepFacts::default()
            },
        );
        assert_eq!(ev, Some(MountEvent::WaterDismount));
        assert_eq!(s.phase(), MountPhase::OnFoot);
        assert!(s.mount_drawn(), "fades out");
        // A dismount while moving does not wait.
        let mut s = MountState::default();
        s.start(timeline(), false);
        s.dismount(true, true);
        assert_eq!(s.phase(), MountPhase::OnFoot);
        // Fly mode during summoning cancels; while mounted it dismounts by other.
        let mut s = MountState::default();
        s.start(timeline(), true);
        assert_eq!(s.interrupt(), Some(MountEvent::SummonInterrupted));
        s.start(timeline(), false);
        assert_eq!(s.interrupt(), None);
        assert_eq!(s.phase(), MountPhase::OnFoot);
    }

    #[test]
    fn summon_without_induction_and_hostile_timelines() {
        let mut s = MountState::default();
        s.start(
            Timeline {
                summon: None,
                ..timeline()
            },
            true,
        );
        assert_eq!(s.phase(), MountPhase::Mounting);
        let mut s = MountState::default();
        s.start(
            Timeline {
                summon: Some(f32::NAN),
                mounting: f32::INFINITY,
                dismount_wait: -3.0,
                fade_in: f32::NAN,
                fade_out: 1e30,
            },
            true,
        );
        assert_eq!(s.timeline().summon, Some(0.0));
        assert_eq!(s.timeline().mounting, 0.0);
        assert_eq!(s.timeline().fade_out, MAX_PHASE_SECONDS);
        s.step(f32::NAN, &StepFacts::default());
        s.step(DT, &StepFacts::default());
        s.step(DT, &StepFacts::default());
        assert_eq!(s.phase(), MountPhase::Mounted);
    }
}
