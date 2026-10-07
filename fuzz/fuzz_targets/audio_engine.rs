// SPDX-License-Identifier: GPL-3.0-or-later
//! The audio engine (`docs/specs/sound.md` §6–§8, decision 0019) driven by arbitrary
//! operations through the offline backend: requests with hostile parameters (NaN, infinite
//! and negative values, bad buffer shapes, streams), stops, scheduled starts and stops,
//! volume, listener, position, settings, suspension, reverb environments, payload loads,
//! updates, renders, reopening the backend, positioner offsets and orientations, and the
//! pitch law with hostile node values.
//!
//! Nothing may panic. Every rendered sample is finite and within [−1, 1], and no tracked
//! request completes twice.

#![no_main]

use std::collections::HashSet;

use libfuzzer_sys::fuzz_target;
use tessera_audio::engine::law::pitch_multiplier_from_draw;
use tessera_audio::engine::{
    AudioEngine, AudioEvent, AudioTime, Channel, EngineConfig, Listener, LivePosition,
    MixerSettings, Payload, PositionalClass, Positioner, ReverbEnvironment, ReverbPreset,
    SoundRequest, SoundSource, VoiceHandle, payload_channel,
};
use tessera_audio::pcm::{Pcm, PcmCursor};

struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn u8(&mut self) -> u8 {
        match self.0.split_first() {
            Some((b, rest)) => {
                self.0 = rest;
                *b
            }
            None => 0,
        }
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    /// Either a raw bit pattern (hostile) or a small sane value.
    fn f32(&mut self) -> f32 {
        let tag = self.u8();
        if tag & 0x80 != 0 {
            f32::from_bits(self.u32())
        } else {
            f32::from(tag) / 16.0 - 2.0
        }
    }
    fn pos(&mut self) -> [f32; 3] {
        [self.f32() * 10.0, self.f32() * 10.0, self.f32()]
    }
    fn quat(&mut self) -> [f32; 4] {
        [self.f32(), self.f32(), self.f32(), self.f32()]
    }
    fn empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn buffer(b: &mut Bytes<'_>) -> Pcm {
    let channels = u16::from(b.u8() % 4);
    let rate = [0, 8_000, 22_050, 44_100, 48_000, 1, u32::MAX, 384_001][usize::from(b.u8() % 8)];
    let len = usize::from(b.u8()) * 4 + usize::from(b.u8() % 3);
    let samples: Vec<f32> = (0..len)
        .map(|i| ((i * 7) % 13) as f32 / 13.0 - 0.5)
        .collect();
    Pcm {
        sample_rate: rate,
        channels,
        samples: samples.into(),
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes(data);
    let Ok(mut e) = AudioEngine::new(EngineConfig {
        voice_slots: 16,
        ..EngineConfig::offline(48_000)
    }) else {
        return;
    };
    let live = LivePosition::new([0.0; 3]);
    let mut handles: Vec<VoiceHandle> = Vec::new();
    let mut envs = Vec::new();
    let mut loads = Vec::new();
    let mut completed = HashSet::new();
    let mut out = vec![0.0f32; 2 * 2048];
    let mut ops = 0;
    while !b.empty() && ops < 400 {
        ops += 1;
        match b.u8() % 18 {
            0..=3 => {
                let source = match b.u8() % 4 {
                    0 => SoundSource::Stream(Box::new(PcmCursor::new(buffer(&mut b)))),
                    1 => {
                        let (tx, rx) = payload_channel();
                        loads.push(tx);
                        SoundSource::Loading(rx)
                    }
                    _ => SoundSource::Pcm(buffer(&mut b)),
                };
                let flags = b.u8();
                let mut req = SoundRequest::new(source)
                    .volume(b.f32())
                    .pitch(b.f32())
                    .looping(flags & 1 != 0)
                    .tracked(flags & 2 != 0)
                    .limit_exempt(flags & 4 != 0)
                    .channel(Channel::new_lossy(u32::from(b.u8() % 10)))
                    .positional(
                        PositionalClass::from_sound_info_flags(u32::from(flags)),
                        b.f32(),
                        b.f32() * 20.0,
                    )
                    .priority(b.f32())
                    .ramp_out(b.f32() / 8.0);
                req = match b.u8() % 5 {
                    0 => req.at(Positioner::Fixed(b.pos())),
                    1 => req.at(Positioner::live(live.clone())),
                    2 => req.at(Positioner::Live {
                        anchor: live.clone(),
                        offset: b.pos(),
                    }),
                    3 => req.at(Positioner::Listener {
                        offset: b.pos(),
                        orientation: Some(live.clone()),
                    }),
                    _ => req.at(Positioner::listener()),
                };
                if flags & 8 != 0 {
                    req = req.sound_id(b.u32());
                }
                if flags & 16 != 0 {
                    req = req.start_at(AudioTime(e.clock().0 + u64::from(b.u32() % 100_000)));
                }
                if let Some(h) = e.request(req).handle() {
                    handles.push(h);
                }
            }
            4 => {
                if let Some(h) = handles.get(usize::from(b.u8()) % handles.len().max(1)) {
                    e.stop_with_ramp(*h, b.f32());
                }
            }
            5 => {
                if let Some(h) = handles.get(usize::from(b.u8()) % handles.len().max(1)) {
                    let at = AudioTime(e.clock().0 + u64::from(b.u32() % 50_000));
                    e.stop_at(*h, at, b.f32());
                }
            }
            6 => {
                if let Some(h) = handles.get(usize::from(b.u8()) % handles.len().max(1)) {
                    e.set_volume(*h, b.f32());
                }
            }
            7 => e.set_listener(Listener {
                position: b.pos(),
                right: b.pos(),
                up: b.pos(),
                forward: b.pos(),
            }),
            8 => match b.u8() % 4 {
                0 => live.set(b.pos()),
                1 => live.set_transform(b.pos(), b.quat()),
                2 => live.set_invalid(),
                _ => live.remove(),
            },
            9 => e.set_settings(MixerSettings {
                master_volume: b.f32(),
                channel_volumes: std::array::from_fn(|_| b.f32()),
                max_playing_sounds: u32::from(b.u8() % 40),
                reverb_enabled: b.u8() & 1 != 0,
                sound_disabled: b.u8() % 8 == 0,
                music_disabled: b.u8() & 1 != 0,
            }),
            10 => e.set_suspended(b.u8() & 1 != 0),
            11 => {
                let env = ReverbEnvironment {
                    preset: ReverbPreset::from_value(b.u32() % 40),
                    priority: b.u32() as i32,
                    intensity: b.f32(),
                };
                if let Some(id) = e.register_environment(env) {
                    envs.push(id);
                }
            }
            12 => {
                if !envs.is_empty() {
                    let id = envs.swap_remove(usize::from(b.u8()) % envs.len());
                    e.unregister_environment(id);
                }
            }
            13 => {
                if let Some(tx) = loads.pop() {
                    match b.u8() % 3 {
                        0 => tx.send(Ok(Payload::Pcm(buffer(&mut b)))),
                        1 => tx.send(Ok(Payload::Stream(Box::new(PcmCursor::new(buffer(
                            &mut b,
                        )))))),
                        _ => tx.send(Err("fuzz".into())),
                    }
                }
            }
            14 => e.update(),
            16 => {
                let m = pitch_multiplier_from_draw(b.f32(), b.f32(), b.f32());
                assert!(m.is_finite() && m > 0.0);
            }
            17 => {
                if b.u8() % 8 == 0 {
                    let _ = e.reopen();
                }
            }
            _ => {
                let frames = (usize::from(b.u8()) * 8 + 1).min(2048);
                let buf = &mut out[..frames * 2];
                if e.render(buf).is_ok() {
                    assert!(buf.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
                }
            }
        }
        for ev in e.drain_events() {
            let AudioEvent::Completed { handle, .. } = ev;
            assert!(completed.insert(handle), "{handle} completed twice");
        }
    }
});
