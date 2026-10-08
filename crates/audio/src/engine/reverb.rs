// SPDX-License-Identifier: GPL-3.0-or-later
//! Reverb environments (`docs/specs/sound.md` §8): the registry that picks the active
//! environment (main thread) and the reverb processor (audio thread).
//!
//! The spec names the 26 presets and says that values 0–25 select the standard environment
//! preset of the same name, but it does not hold the presets' parameters. Until it does
//! (OPEN in `sound.md` §8), every preset maps to Tessera's own coarse placeholder values
//! (decay time, high-frequency damping, wet level) chosen by ear, not taken from any preset
//! table. The processor is a generic Schroeder–Moorer network (eight damped comb filters in
//! parallel, four all-pass filters in series, per output side), whose delay lengths are the
//! public-domain "Freeverb" tunings.

use std::fmt;

/// A reverb environment value of the "SoundEnv" mapper (`sound.md` §8). Values above 25
/// select [`ReverbPreset::GENERIC`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ReverbPreset(u8);

impl ReverbPreset {
    /// 0, GENERIC: the preset with no environment registered.
    pub const GENERIC: Self = Self(0);
    /// Number of distinct presets (0–25).
    pub const COUNT: usize = 26;

    /// The preset for an enum value; 26 ("Undef") and anything larger select GENERIC.
    #[must_use]
    pub fn from_value(value: u32) -> Self {
        if value < Self::COUNT as u32 {
            Self(value as u8)
        } else {
            Self::GENERIC
        }
    }

    /// The enum value 0–25.
    #[must_use]
    pub fn value(self) -> u32 {
        u32::from(self.0)
    }

    /// The preset name of `sound.md` §8.
    #[must_use]
    pub fn name(self) -> &'static str {
        PRESETS[usize::from(self.0)].0
    }

    fn params(self) -> PresetParams {
        let (_, decay, hf, wet) = PRESETS[usize::from(self.0)];
        PresetParams { decay, hf, wet }
    }
}

impl fmt::Display for ReverbPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.0, self.name())
    }
}

/// Name, decay time (s), high-frequency ratio (0–1, lower damps more), wet level.
/// Placeholder values (Tessera's, see the module documentation).
const PRESETS: [(&str, f32, f32, f32); ReverbPreset::COUNT] = [
    ("GENERIC", 1.4, 0.8, 0.25),
    ("PADDEDCELL", 0.2, 0.2, 0.15),
    ("ROOM", 0.45, 0.8, 0.25),
    ("BATHROOM", 1.5, 0.6, 0.45),
    ("LIVINGROOM", 0.5, 0.3, 0.2),
    ("STONEROOM", 2.2, 0.7, 0.4),
    ("AUDITORIUM", 4.0, 0.6, 0.35),
    ("CONCERTHALL", 3.5, 0.7, 0.35),
    ("CAVE", 3.0, 1.0, 0.5),
    ("ARENA", 6.5, 0.4, 0.3),
    ("HANGAR", 9.0, 0.3, 0.3),
    ("CARPETEDHALLWAY", 0.3, 0.1, 0.2),
    ("HALLWAY", 1.4, 0.6, 0.3),
    ("STONECORRIDOR", 2.5, 0.8, 0.4),
    ("ALLEY", 1.5, 0.9, 0.25),
    ("FOREST", 1.5, 0.5, 0.15),
    ("CITY", 1.5, 0.7, 0.15),
    ("MOUNTAINS", 1.5, 0.6, 0.12),
    ("QUARRY", 1.5, 0.8, 0.2),
    ("PLAIN", 1.5, 0.6, 0.1),
    ("PARKINGLOT", 1.6, 1.0, 0.2),
    ("SEWERPIPE", 2.8, 0.2, 0.5),
    ("UNDERWATER", 1.5, 0.1, 0.5),
    ("DRUGGED", 8.0, 1.0, 0.5),
    ("DIZZY", 15.0, 0.6, 0.4),
    ("PSYCHOTIC", 7.5, 0.9, 0.5),
];

#[derive(Debug, Clone, Copy, PartialEq)]
struct PresetParams {
    decay: f32,
    hf: f32,
    wet: f32,
}

/// A registered environment (`sound.md` §8, §9.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReverbEnvironment {
    /// `SoundEnvironmentScript_Name`.
    pub preset: ReverbPreset,
    /// `SoundEnvironmentScript_Priority`.
    pub priority: i32,
    /// The node's intensity; scales the reverb send (`sound.md` §8 OPEN, Decision). A change
    /// (or a change of the active environment) is spread over 20 ms and applies to the
    /// sound that enters the reverb from then on; the tail already in it decays as before.
    pub intensity: f32,
}

/// Names one registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvironmentId(u64);

/// The registry of running environments (main thread).
#[derive(Debug, Default)]
pub(crate) struct EnvironmentRegistry {
    next: u64,
    entries: Vec<(EnvironmentId, ReverbEnvironment)>,
}

impl EnvironmentRegistry {
    /// Upper bound on registrations (allocation bound; 21 nodes exist in the data).
    pub const MAX: usize = 4096;

    pub fn register(&mut self, env: ReverbEnvironment) -> Option<EnvironmentId> {
        if self.entries.len() >= Self::MAX {
            return None;
        }
        self.next += 1;
        let id = EnvironmentId(self.next);
        self.entries.push((id, env));
        Some(id)
    }

    pub fn unregister(&mut self, id: EnvironmentId) -> bool {
        let before = self.entries.len();
        self.entries.retain(|(i, _)| *i != id);
        before != self.entries.len()
    }

    pub fn set_intensity(&mut self, id: EnvironmentId, intensity: f32) -> bool {
        match self.entries.iter_mut().find(|(i, _)| *i == id) {
            Some((_, e)) => {
                e.intensity = intensity;
                true
            }
            None => false,
        }
    }

    /// The active environment: the highest priority, ties to the most recently
    /// registered; GENERIC at intensity 0 with none registered (`sound.md` §8).
    pub fn active(&self) -> ReverbEnvironment {
        let mut best: Option<&ReverbEnvironment> = None;
        for (_, e) in &self.entries {
            if best.is_none_or(|b| e.priority >= b.priority) {
                best = Some(e);
            }
        }
        best.copied().unwrap_or(ReverbEnvironment {
            preset: ReverbPreset::GENERIC,
            priority: 0,
            intensity: 0.0,
        })
    }
}

/// What the audio thread needs to run the reverb.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ReverbParams {
    pub enabled: bool,
    pub preset: ReverbPreset,
    pub intensity: f32,
}

const COMB_TUNING: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNING: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;
const INPUT_GAIN: f32 = 0.015;
const ALLPASS_FEEDBACK: f32 = 0.5;
/// Time over which a change of the send level is spread.
const LEVEL_SMOOTH_SECS: f32 = 0.02;
/// Output below this (about −120 dB) counts as silence when deciding to go idle.
const SILENCE: f32 = 1e-6;

#[derive(Debug)]
struct Comb {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
    damp: f32,
    store: f32,
}

impl Comb {
    fn process(&mut self, x: f32) -> f32 {
        let y = self.buf[self.idx];
        self.store = y * (1.0 - self.damp) + self.store * self.damp;
        self.buf[self.idx] = x + self.store * self.feedback;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
        y
    }
}

#[derive(Debug)]
struct Allpass {
    buf: Vec<f32>,
    idx: usize,
}

impl Allpass {
    fn process(&mut self, x: f32) -> f32 {
        let b = self.buf[self.idx];
        self.buf[self.idx] = x + b * ALLPASS_FEEDBACK;
        self.idx += 1;
        if self.idx == self.buf.len() {
            self.idx = 0;
        }
        b - x
    }
}

/// The reverb processor (audio thread). All buffers are allocated by [`Reverb::new`].
#[derive(Debug)]
pub(crate) struct Reverb {
    sample_rate: u32,
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<Allpass>; 2],
    params: ReverbParams,
    /// Current input level (INPUT_GAIN × preset wet level × intensity), moving linearly
    /// towards `target` by `step` per sample.
    level: f32,
    target: f32,
    step: f32,
    /// Consecutive output frames below [`SILENCE`] while the input level is 0.
    quiet: usize,
    /// Frames of silence after which the delay lines are known to be empty enough to idle:
    /// the longest comb delay plus every all-pass delay of one side.
    quiet_needed: usize,
    /// Whether the delay lines may hold sound (skip work when silent and disabled).
    active: bool,
}

impl Reverb {
    pub fn new(sample_rate: u32) -> Self {
        let scale = f64::from(sample_rate.clamp(8_000, 384_000)) / 44_100.0;
        let len = |n: usize| ((n as f64 * scale).round() as usize).max(1);
        let mk = |spread: usize| -> (Vec<Comb>, Vec<Allpass>) {
            (
                COMB_TUNING
                    .iter()
                    .map(|&n| Comb {
                        buf: vec![0.0; len(n + spread)],
                        idx: 0,
                        feedback: 0.0,
                        damp: 0.0,
                        store: 0.0,
                    })
                    .collect(),
                ALLPASS_TUNING
                    .iter()
                    .map(|&n| Allpass {
                        buf: vec![0.0; len(n + spread)],
                        idx: 0,
                    })
                    .collect(),
            )
        };
        let (cl, al) = mk(0);
        let (cr, ar) = mk(STEREO_SPREAD);
        let quiet_needed = cr.iter().map(|c| c.buf.len()).max().unwrap_or(0)
            + ar.iter().map(|a| a.buf.len()).sum::<usize>();
        let mut r = Self {
            sample_rate,
            combs: [cl, cr],
            allpasses: [al, ar],
            params: ReverbParams {
                enabled: false,
                preset: ReverbPreset::GENERIC,
                intensity: 0.0,
            },
            level: 0.0,
            target: 0.0,
            step: 0.0,
            quiet: 0,
            quiet_needed,
            active: false,
        };
        r.set_params(r.params);
        r
    }

    pub fn set_params(&mut self, params: ReverbParams) {
        self.params = params;
        let p = params.preset.params();
        let fs = self.sample_rate.max(1) as f32;
        for side in &mut self.combs {
            for c in side.iter_mut() {
                // Feedback for a 60 dB decay over `decay` seconds through this comb's delay.
                let delay = c.buf.len() as f32 / fs;
                c.feedback = 10f32
                    .powf(-3.0 * delay / p.decay.max(0.05))
                    .clamp(0.0, 0.98);
                c.damp = (1.0 - p.hf).clamp(0.0, 0.9) * 0.6;
            }
        }
        let intensity = if params.intensity.is_finite() {
            params.intensity.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.target = if params.enabled {
            INPUT_GAIN * p.wet * intensity
        } else {
            0.0
        };
        let frames = (LEVEL_SMOOTH_SECS * fs).max(1.0);
        self.step = (self.target - self.level).abs() / frames;
    }

    /// Whether the processor is running (its delay lines may hold sound).
    #[cfg(test)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Adds the reverb of `send` to `l` and `r`.
    ///
    /// The send level (preset wet level × intensity) scales the input, so that a change of
    /// environment, intensity or the enable setting never steps the output: the tail already
    /// in the delay lines decays on its own. Once the input level is 0 and the output has
    /// stayed below −120 dB for longer than the delay lines, the lines are cleared and the
    /// processor idles.
    pub fn process(&mut self, send: &[f32], l: &mut [f32], r: &mut [f32]) {
        if !self.active && self.level <= 0.0 && self.target <= 0.0 {
            return;
        }
        self.active = true;
        let n = send.len().min(l.len()).min(r.len());
        for i in 0..n {
            if self.level != self.target {
                self.level = if self.level < self.target {
                    (self.level + self.step).min(self.target)
                } else {
                    (self.level - self.step).max(self.target)
                };
            }
            let x = send[i] * self.level;
            let x = if x.is_finite() { x } else { 0.0 };
            let mut loud = false;
            for (side, out) in [(0usize, &mut l[i]), (1, &mut r[i])] {
                let mut acc = 0.0;
                for c in self.combs[side].iter_mut() {
                    acc += c.process(x);
                }
                for a in self.allpasses[side].iter_mut() {
                    acc = a.process(acc);
                }
                loud |= acc.abs() >= SILENCE;
                *out += acc;
            }
            if loud || self.level > 0.0 {
                self.quiet = 0;
            } else {
                self.quiet += 1;
            }
        }
        if self.level <= 0.0 && self.target <= 0.0 && self.quiet >= self.quiet_needed {
            for side in &mut self.combs {
                for c in side.iter_mut() {
                    c.buf.fill(0.0);
                    c.store = 0.0;
                }
            }
            for side in &mut self.allpasses {
                for a in side.iter_mut() {
                    a.buf.fill(0.0);
                }
            }
            self.quiet = 0;
            self.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_values() {
        assert_eq!(ReverbPreset::from_value(8).name(), "CAVE");
        assert_eq!(ReverbPreset::from_value(25).name(), "PSYCHOTIC");
        assert_eq!(ReverbPreset::from_value(26), ReverbPreset::GENERIC);
        assert_eq!(ReverbPreset::from_value(u32::MAX), ReverbPreset::GENERIC);
    }

    #[test]
    fn active_environment_rule() {
        let mut reg = EnvironmentRegistry::default();
        let env = |v, p| ReverbEnvironment {
            preset: ReverbPreset::from_value(v),
            priority: p,
            intensity: 1.0,
        };
        assert_eq!(reg.active().intensity, 0.0);
        let a = reg.register(env(18, 1)).unwrap();
        let b = reg.register(env(8, 5)).unwrap();
        let _c = reg.register(env(2, 5)).unwrap();
        // Tie at priority 5: the newest wins.
        assert_eq!(reg.active().preset.value(), 2);
        assert!(reg.unregister(_c));
        assert_eq!(reg.active().preset.value(), 8);
        assert!(reg.unregister(b));
        assert_eq!(reg.active().preset.value(), 18);
        assert!(reg.set_intensity(a, 0.5));
        assert_eq!(reg.active().intensity, 0.5);
        assert!(reg.unregister(a));
        assert!(!reg.unregister(a));
        assert_eq!(reg.active().preset, ReverbPreset::GENERIC);
    }

    #[test]
    fn reverb_produces_a_tail_and_stays_finite() {
        let mut r = Reverb::new(48_000);
        r.set_params(ReverbParams {
            enabled: true,
            preset: ReverbPreset::from_value(8),
            intensity: 1.0,
        });
        let mut send = vec![0.0; 4800];
        // After the 20 ms level ramp.
        send[1000] = 1.0;
        send[1001] = f32::NAN;
        let (mut l, mut rr) = (vec![0.0; 4800], vec![0.0; 4800]);
        r.process(&send, &mut l, &mut rr);
        assert!(l.iter().chain(&rr).all(|x| x.is_finite()));
        assert!(l[3000..].iter().any(|x| x.abs() > 1e-6));
    }

    fn cave(intensity: f32) -> ReverbParams {
        ReverbParams {
            enabled: true,
            preset: ReverbPreset::from_value(8),
            intensity,
        }
    }

    /// Feeds an impulse at full level (after the 20 ms level ramp) and returns the reverb.
    fn excited(intensity: f32) -> Reverb {
        let mut r = Reverb::new(48_000);
        r.set_params(cave(intensity));
        let mut send = vec![0.0; 4800];
        send[2000] = 1.0;
        let (mut l, mut rr) = (vec![0.0; 4800], vec![0.0; 4800]);
        r.process(&send, &mut l, &mut rr);
        r
    }

    #[test]
    fn disabling_lets_the_tail_decay_then_idles() {
        // Regression: switching the reverb off (or losing the active environment) zeroed the
        // tail within one sample, which clicks.
        let mut r = excited(1.0);
        let (mut l, mut rr) = (vec![0.0; 480], vec![0.0; 480]);
        r.process(&[0.0; 480], &mut l, &mut rr);
        let before: f32 = l.iter().map(|x| x * x).sum();
        r.set_params(ReverbParams {
            enabled: false,
            ..cave(1.0)
        });
        let (mut l, mut rr) = (vec![0.0; 480], vec![0.0; 480]);
        r.process(&[0.0; 480], &mut l, &mut rr);
        let after: f32 = l.iter().map(|x| x * x).sum();
        assert!(after > 0.25 * before, "tail cut: {before} -> {after}");
        assert!(r.is_active());
        // New sound no longer enters once the level has ramped down; the tail dies out
        // and the processor idles (CAVE decays over 3 s).
        let mut send = vec![0.0; 48_000];
        send[24_000] = 1.0;
        let (mut l, mut rr) = (vec![0.0; 48_000], vec![0.0; 48_000]);
        for _ in 0..10 {
            l.fill(0.0);
            rr.fill(0.0);
            r.process(&send, &mut l, &mut rr);
        }
        assert!(!r.is_active());
        assert!(l.iter().chain(&rr).all(|x| *x == 0.0));
    }

    #[test]
    fn intensity_scales_new_input_not_the_tail() {
        let mut a = excited(1.0);
        let mut b = excited(1.0);
        b.set_params(cave(0.25));
        let (mut la, mut ra) = (vec![0.0; 2000], vec![0.0; 2000]);
        let (mut lb, mut rb) = (vec![0.0; 2000], vec![0.0; 2000]);
        a.process(&[0.0; 2000], &mut la, &mut ra);
        b.process(&[0.0; 2000], &mut lb, &mut rb);
        assert_eq!(la, lb);
        assert_eq!(ra, rb);
    }

    #[test]
    fn level_changes_are_ramped() {
        let mut r = Reverb::new(48_000);
        r.set_params(cave(1.0));
        let (mut l, mut rr) = (vec![0.0; 10], vec![0.0; 10]);
        r.process(&[0.0; 10], &mut l, &mut rr);
        assert!(r.level > 0.0 && r.level < r.target);
        let (mut l, mut rr) = (vec![0.0; 960], vec![0.0; 960]);
        r.process(&[0.0; 960], &mut l, &mut rr);
        assert_eq!(r.level, r.target);
    }
}
