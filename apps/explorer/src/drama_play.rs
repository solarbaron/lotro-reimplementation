// SPDX-License-Identifier: GPL-3.0-or-later
//! The drama player of the character view (`npc-drama.md` §11 item 4, Decision T7): runs
//! one named drama by Decision T1 under the offline rules of T2 and T5.
//!
//! - [`prepare`] (a worker) reads the entity's drama list from its property set (an
//!   object's set or a landblock instance set), plans the drama ([`plan_drama`]), resolves
//!   every line of its dialogs and the clip each emote key starts on the figure through the
//!   figure's script tables (the clip candidates whose run starts at that key), and does
//!   the same for the entity's starting idle drama (T5).
//! - [`DramaRun`] (the UI thread) steps the plan with the view's clock
//!   ([`DramaPlayback`]) and turns each applied facet into an effect: an emote plays its
//!   clip (until it ends; a persistent emote holds the drama for at most 30 s, T1 step
//!   3.4), a dialog line (one picked at random, T2) shows over the figure for 10 s for the
//!   `Color…` types and goes to the chat log for every type but `ColorOnly`, a billboard
//!   shows for 5 s, and the facets that need server state are logged as doing nothing
//!   offline (T5). When the drama ends the figure returns to its idle; the starting idle
//!   drama then repeats with a pause of 5 s between passes (T5).

use std::collections::HashMap;

use tessera_formats::gameplay::common::{Pid, ValueExt};
use tessera_formats::gameplay::drama::{
    BILLBOARD_SECONDS, DramaKind, DramaLabels, DramaList, DramaPlayback, DramaStep, Facet,
    IDLE_PAUSE, NamedDrama, OVERHEAD_SECONDS, PlaybackEvent, StartingIdle, dialog_presentation,
    pick_line, plan_drama,
};
use tessera_formats::landblock::InstanceId;
use tessera_formats::script_table::resolve::{
    ClipResolver, MotionState, ScriptVocabulary, collect_entity_scripts,
};
use tessera_formats::{Did, LandblockInfo, PropertySet, PropertySetRecord};

use crate::data::DataContext;
use crate::error::ExplorerError;
use crate::gameplay::card::Labels;

/// Most lines of the chat log kept.
pub const MAX_CHAT: usize = 64;
/// Most actions of one drama prepared.
pub const MAX_ACTIONS: usize = 256;

/// Where a drama list lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DramaSource {
    /// A property set record (an object's 0x79… set or a 0x78 set).
    Set(u32),
    /// The instance set `iid` of landblock-info record `did`, else the static entity's own
    /// set of that IID.
    Landblock {
        /// The landblock-info record.
        did: u32,
        /// The instance ID.
        iid: u64,
    },
}

/// A request to play a drama in the character view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DramaPlayArg {
    /// The figure to show (a 0x70 object or 0x47 entity); `None` keeps the view's figure.
    pub figure: Option<u32>,
    /// The drama list's set.
    pub source: DramaSource,
    /// The drama's name (matched by T3).
    pub name: String,
    /// A caption for the UI.
    pub caption: String,
}

/// An emote of an action: its key, label and the clip it starts on the figure.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedEmote {
    /// The script key.
    pub key: u32,
    /// Its label.
    pub label: String,
    /// The clip the figure's tables start for it, with its rate and whether it loops.
    pub clip: Option<(Did, f32, bool)>,
}

/// One action, prepared for playback.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreparedAction {
    /// `Drama_Emote` (a non-zero key).
    pub emote: Option<PreparedEmote>,
    /// `Drama_Emote_Stop` (a non-zero key).
    pub emote_stop: Option<u32>,
    /// `Drama_Wait_Emote`.
    pub wait_emote: bool,
    /// The dialog: type label, presentation (overhead, chat), resolved lines.
    pub dialog: Option<(String, bool, bool, Vec<String>)>,
    /// The billboard text.
    pub billboard: Option<String>,
    /// Text of the other facets (what they would do).
    pub other: Vec<(Facet, String)>,
}

/// One named drama prepared for playback.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedDrama {
    /// The drama's name.
    pub name: String,
    /// Its kind.
    pub kind: DramaKind,
    /// `Drama_Interruptible`.
    pub interruptible: bool,
    /// The plan (Decision T1).
    pub steps: Vec<DramaStep>,
    /// The actions by index.
    pub actions: Vec<PreparedAction>,
}

/// What [`prepare`] gives the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedPlay {
    /// The request.
    pub arg: DramaPlayArg,
    /// The figure the clips were resolved for.
    pub figure: Option<Did>,
    /// The drama.
    pub drama: PreparedDrama,
    /// The entity's starting idle drama (T5), when it has one other than `drama`.
    pub idle: Option<PreparedDrama>,
    /// `NPC_ReturnsToStartingIdle`.
    pub returns_to_idle: bool,
    /// The speaker's name for the chat log.
    pub speaker: String,
    /// Problems that do not stop the play.
    pub notes: Vec<String>,
}

/// A small random source for the waits and line picks (xorshift; seeded from the clock).
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A source seeded with `seed` (0 is replaced).
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number in [0, 1).
    pub fn unit(&mut self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let v = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        v
    }

    /// An index in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        usize::try_from(self.next() % (n as u64)).unwrap_or(0)
    }
}

fn read_set(ctx: &DataContext, did: Did) -> Result<PropertySet, ExplorerError> {
    let registry = ctx.game.registry()?;
    let bytes = ctx.game.read(did)?.ok_or(ExplorerError::NotFound(did.0))?;
    Ok(PropertySetRecord::parse_expecting(&bytes, did, registry.as_ref())?.set)
}

/// The set and the static entity's object of a [`DramaSource`].
fn source_set(
    ctx: &DataContext,
    source: DramaSource,
) -> Result<(PropertySet, Option<Did>), ExplorerError> {
    match source {
        DramaSource::Set(d) => Ok((read_set(ctx, Did(d))?, None)),
        DramaSource::Landblock { did, iid } => {
            let raw = crate::decoders::read_raw(ctx, did, None)?;
            let registry = ctx.game.registry()?;
            let info = LandblockInfo::parse_expecting(
                &raw.bytes,
                Did(did),
                Some(raw.entry.stamp),
                registry.dictionary(),
            )?;
            let iid = InstanceId(iid);
            let sed = info.entities.iter().find(|e| e.iid == Some(iid));
            let object = sed.and_then(|e| e.object);
            let own = sed.and_then(|e| e.properties.clone());
            let set = match info.weenies.iter().find(|w| w.iid == iid) {
                Some(w) => w.properties.clone(),
                None => own.ok_or_else(|| {
                    ExplorerError::NotApplicable(format!("no instance set {:#018x}", iid.0))
                })?,
            };
            Ok((set, object))
        }
    }
}

/// The clips the figure's tables start for each emote key (`npc-drama.md` §11 item 4.2,
/// Decision T7): among the candidates whose run starts at the key, the one with the largest
/// blend weight × chance, held clips excluded, ties to the first in table order. A key with
/// no clip is left out (logged by the caller; the drama goes on with its next facet).
fn emote_clips(
    ctx: &DataContext,
    figure: Did,
    instance: Option<&PropertySet>,
    keys: &[u32],
    notes: &mut Vec<String>,
) -> HashMap<u32, (Did, f32, bool)> {
    let mut out = HashMap::new();
    let data = &ctx.game;
    let Ok(registry) = data.registry() else {
        return out;
    };
    let dict = registry.dictionary();
    let vocab = match ScriptVocabulary::from_data(dict, |d| {
        data.enum_mapper(d).ok().flatten().map(|m| (*m).clone())
    }) {
        Ok(v) => v,
        Err(e) => {
            notes.push(format!("script vocabulary: {e}"));
            return out;
        }
    };
    let chain = match collect_entity_scripts(figure, dict, &vocab, |d| data.read(d)) {
        Ok(c) => c,
        Err(e) => {
            notes.push(format!("scripts of {figure}: {e}; emotes play nothing"));
            return out;
        }
    };
    let mut base = chain.scripts;
    if let Some(inst) = instance {
        base.add_property_set(inst, &vocab);
    }
    let mut resolver =
        ClipResolver::new(dict, vocab, |d: Did| data.read(d)).with_dictionary_defaults(dict);
    for &key in keys {
        let mut scripts = base.clone();
        scripts.startup_scripts = vec![key];
        match resolver.resolve(&scripts, MotionState::Idle) {
            Ok(r) => {
                // §11 item 4.2 (Decision T7): the largest blend weight × chance, held clips
                // excluded, ties to the first in table order.
                let mut best: Option<&tessera_formats::script_table::resolve::ClipCandidate> = None;
                for c in r
                    .candidates
                    .iter()
                    .filter(|c| !c.is_held() && c.scripts.first() == Some(&key))
                {
                    if best.is_none_or(|b| c.blend_weight * c.chance > b.blend_weight * b.chance) {
                        best = Some(c);
                    }
                }
                if let Some(c) = best {
                    let rate = Some(c.playback_rate())
                        .filter(|r| r.is_finite() && *r > 0.0)
                        .unwrap_or(1.0);
                    out.insert(key, (c.clip, rate, c.loops()));
                }
            }
            Err(e) => notes.push(format!("emote {key:#x}: {e}")),
        }
    }
    out
}

fn prepare_drama(
    l: &Labels<'_>,
    labels: &DramaLabels,
    d: &NamedDrama<'_>,
    rng: &mut Rng,
    emote_label: &dyn Fn(u32) -> String,
) -> PreparedDrama {
    let steps = plan_drama(d, |v| rng.unit() * v);
    let dialog_type = labels.drama.dialog_type;
    let mut actions = Vec::new();
    for a in d.actions().iter().take(MAX_ACTIONS) {
        let f = a.facets();
        let mut p = PreparedAction {
            emote: f.emote.filter(|k| *k != 0).map(|key| PreparedEmote {
                key,
                label: emote_label(key),
                clip: None,
            }),
            emote_stop: f.emote_stop.filter(|k| *k != 0),
            wait_emote: f.wait_emote == Some(true),
            ..PreparedAction::default()
        };
        if let Some(dialog) = &f.dialog {
            let label = dialog
                .dialog_type
                .map(|t| l.enum_key(dialog_type, t))
                .unwrap_or_default();
            // The raw label (without the key) decides the presentation (T2).
            let raw = label
                .split_once('(')
                .map(|(_, r)| r.trim_end_matches(')').to_owned());
            let pres = dialog_presentation(raw.as_deref());
            let lines: Vec<String> = dialog.lines.iter().map(|si| l.text(si)).collect();
            p.dialog = Some((label, pres.overhead, pres.chat, lines));
        }
        p.billboard = f.billboard.as_ref().map(|si| l.text(si));
        for facet in f.present() {
            let text = match facet {
                Facet::Emote | Facet::EmoteStop | Facet::Dialog | Facet::InstanceBillboard => {
                    continue;
                }
                Facet::Sound => format!(
                    "sound {} (played by the entity's drama sound script; not in the viewer)",
                    f.sound
                        .and_then(|s| s.sound)
                        .map_or_else(|| "—".into(), |d| d.to_string())
                ),
                Facet::Scriptlet if f.replicate == Some(true) => format!(
                    "scriptlet {} (replicated to the client; not run by the viewer)",
                    f.scriptlet.map_or_else(|| "—".into(), |d| d.to_string())
                ),
                Facet::DestroySelf | Facet::DestroySelfFadeOut => {
                    "hides the entity until its landblock reloads (T5)".into()
                }
                other if !other.runs_offline() => {
                    format!("{other:?}: needs server state; does nothing offline (T5)")
                }
                other => format!("{other:?}"),
            };
            p.other.push((facet, text));
        }
        actions.push(p);
    }
    PreparedDrama {
        name: d.name().unwrap_or("").to_owned(),
        kind: d.kind,
        interruptible: d.interruptible() == Some(true),
        steps,
        actions,
    }
}

/// Prepares `arg` on a worker (see the module documentation). `seed` seeds the random
/// waits.
///
/// # Errors
///
/// When the set cannot be read, has no drama list, or the list lacks the name.
pub fn prepare(
    ctx: &DataContext,
    arg: &DramaPlayArg,
    seed: u64,
) -> Result<PreparedPlay, ExplorerError> {
    let registry = ctx.game.registry()?;
    let dict = registry.dictionary();
    let (set, sed_object) = source_set(ctx, arg.source)?;
    let labels = DramaLabels::resolve(dict);
    let list = DramaList::read(&labels.drama, &set)
        .ok_or_else(|| ExplorerError::NotApplicable("the set has no drama list".into()))?;
    let d = list.find(&arg.name).ok_or_else(|| {
        ExplorerError::NotApplicable(format!("no drama named {:?} in the list (T3)", arg.name))
    })?;
    let l = Labels {
        registry: Some(registry),
        game: Some(&ctx.game),
        strings: ctx.strings.as_ref(),
    };
    let emote_pid = labels.drama.emote;
    let emote_label = |k: u32| l.enum_key(emote_pid, k);
    let mut rng = Rng::new(seed);
    let mut drama = prepare_drama(&l, &labels, d, &mut rng, &emote_label);
    let idle_info = StartingIdle::read(&labels, &set);
    let mut idle = idle_info
        .drama
        .as_deref()
        .and_then(|n| list.find(n))
        .filter(|i| i.name() != d.name())
        .map(|i| prepare_drama(&l, &labels, i, &mut rng, &emote_label));
    let mut notes = Vec::new();
    let figure = arg.figure.map(Did).or(sed_object);
    let keys: Vec<u32> = drama
        .actions
        .iter()
        .chain(idle.iter().flat_map(|i| i.actions.iter()))
        .filter_map(|a| a.emote.as_ref().map(|e| e.key))
        .collect();
    match figure {
        Some(f) if !keys.is_empty() => {
            let clips = emote_clips(ctx, f, Some(&set), &keys, &mut notes);
            for a in drama
                .actions
                .iter_mut()
                .chain(idle.iter_mut().flat_map(|i| i.actions.iter_mut()))
            {
                if let Some(e) = &mut a.emote {
                    e.clip = clips.get(&e.key).copied();
                    if e.clip.is_none() {
                        notes.push(format!(
                            "emote {}: the figure's tables start no clip for it",
                            e.label
                        ));
                    }
                }
            }
        }
        None if !keys.is_empty() => {
            notes.push("no figure for the entity: emotes play nothing".into());
        }
        _ => {}
    }
    notes.dedup();
    let speaker = Pid::resolve(dict, "Name")
        .id()
        .and_then(|p| set.get(p))
        .and_then(|v| v.string_info().map(|si| l.text(si)))
        .unwrap_or_else(|| arg.caption.clone());
    Ok(PreparedPlay {
        arg: arg.clone(),
        figure,
        drama,
        idle,
        returns_to_idle: idle_info.returns == Some(true),
        speaker,
        notes,
    })
}

/// What a running drama asks the character view to do.
#[derive(Debug, Clone, PartialEq)]
pub enum RunEffect {
    /// Play this clip (DID, rate, loops) on the figure.
    PlayClip(Did, f32, bool),
    /// Back to the figure's idle (the motion state the view shows).
    Idle,
}

/// Overhead text shown over the figure.
#[derive(Debug, Clone, PartialEq)]
pub struct Overhead {
    /// The text.
    pub text: String,
    /// Run time it disappears.
    pub until: f32,
    /// A centred billboard (else speech over the figure).
    pub billboard: bool,
}

/// The emote an action started (T1 step 3.4).
#[derive(Debug, Clone, Copy, PartialEq)]
struct EmoteState {
    action: usize,
    start: f32,
    /// When it ends; `None` until the view knows the clip's length.
    end: Option<f32>,
    /// Looping or persistent: runs until stopped (the wait for it lasts at most 30 s).
    endless: bool,
}

impl EmoteState {
    fn running(&self, now: f32) -> bool {
        self.endless || self.end.is_none_or(|e| now < e)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// The requested drama.
    Main,
    /// The idle drama (T5).
    Idle,
    /// The pause between two idle passes.
    Pause,
    /// Nothing more.
    Done,
}

/// A drama being played (the UI thread's state).
#[derive(Debug, Clone)]
pub struct DramaRun {
    /// What was prepared.
    pub play: PreparedPlay,
    playback: DramaPlayback,
    phase: Phase,
    /// Run time (s).
    pub clock: f32,
    pause_until: f32,
    /// The running emote.
    emote: Option<EmoteState>,
    /// Actions of the current pass whose emote has ended.
    ended: Vec<usize>,
    /// Texts shown over the figure.
    pub overhead: Vec<Overhead>,
    /// The chat log (T2).
    pub chat: Vec<String>,
    /// What happened, for the inspector.
    pub log: Vec<String>,
    rng: Rng,
}

impl DramaRun {
    /// Starts `play` at time 0.
    #[must_use]
    pub fn new(play: PreparedPlay, seed: u64) -> Self {
        let playback = DramaPlayback::new(play.drama.steps.clone());
        Self {
            play,
            playback,
            phase: Phase::Main,
            clock: 0.0,
            pause_until: 0.0,
            emote: None,
            ended: Vec::new(),
            overhead: Vec::new(),
            chat: Vec::new(),
            log: vec!["Drama_Performing = true".into()],
            rng: Rng::new(seed),
        }
    }

    /// The drama being performed now (the main or the idle drama), `None` once done or
    /// pausing.
    #[must_use]
    pub fn current(&self) -> Option<&PreparedDrama> {
        match self.phase {
            Phase::Main => Some(&self.play.drama),
            Phase::Idle => self.play.idle.as_ref(),
            Phase::Pause | Phase::Done => None,
        }
    }

    /// The step being waited on, as text.
    #[must_use]
    pub fn status(&self) -> String {
        match self.phase {
            Phase::Pause => format!(
                "idle pause, {:.1} s left (T5)",
                (self.pause_until - self.clock).max(0.0)
            ),
            Phase::Done => "ended".into(),
            _ if self.playback.is_held() => "waits until stopped (T1 step 3.5)".into(),
            _ => self
                .playback
                .current()
                .map_or_else(|| "ending".into(), |s| format!("{s:?}")),
        }
    }

    /// Whether nothing more will happen.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }

    fn push_chat(&mut self, line: String) {
        if self.chat.len() >= MAX_CHAT {
            self.chat.remove(0);
        }
        self.chat.push(line);
    }

    fn apply(&mut self, action: usize, facets: &[Facet], out: &mut Vec<RunEffect>) {
        let Some(a) = self.current().and_then(|d| d.actions.get(action)).cloned() else {
            return;
        };
        for f in facets {
            match f {
                Facet::EmoteStop => {
                    if let Some(e) = self.emote.take() {
                        self.ended.push(e.action);
                        out.push(RunEffect::Idle);
                        self.log.push(format!("{:.1} s: emote stopped", self.clock));
                    }
                }
                Facet::Emote => {
                    if let Some(e) = &a.emote {
                        match e.clip {
                            Some((clip, rate, loops)) => {
                                let endless = loops || a.wait_emote;
                                self.ended.retain(|x| *x != action);
                                out.push(RunEffect::PlayClip(clip, rate, endless));
                                // The end is set by `emote_duration` once the clip loads.
                                self.emote = Some(EmoteState {
                                    action,
                                    start: self.clock,
                                    end: None,
                                    endless,
                                });
                                self.log.push(format!(
                                    "{:.1} s: emote {} → clip {clip}",
                                    self.clock, e.label
                                ));
                            }
                            None => self.log.push(format!(
                                "{:.1} s: emote {} (no clip on this figure)",
                                self.clock, e.label
                            )),
                        }
                    }
                }
                Facet::Dialog => {
                    if let Some((label, overhead, chat, lines)) = &a.dialog {
                        let pick = pick_line(lines.len(), |n| self.rng.below(n));
                        if let Some(text) = pick.and_then(|i| lines.get(i)).cloned() {
                            if *overhead {
                                self.overhead.push(Overhead {
                                    text: text.clone(),
                                    until: self.clock + OVERHEAD_SECONDS,
                                    billboard: false,
                                });
                            }
                            if *chat {
                                let line = format!("{}: {text}", self.play.speaker);
                                self.push_chat(line);
                            }
                            self.log
                                .push(format!("{:.1} s: dialog ({label})", self.clock));
                        }
                    }
                }
                Facet::InstanceBillboard => {
                    if let Some(t) = &a.billboard {
                        self.overhead.push(Overhead {
                            text: t.clone(),
                            until: self.clock + BILLBOARD_SECONDS,
                            billboard: true,
                        });
                    }
                }
                other => {
                    if let Some((_, text)) = a.other.iter().find(|(x, _)| x == other) {
                        self.log.push(format!("{:.1} s: {text}", self.clock));
                    }
                }
            }
        }
    }

    /// Tells the run how long the current emote's clip lasts (seconds at its rate), once
    /// the view has loaded it; a looping or persistent emote keeps running until stopped
    /// or the 30 s limit of T1 step 3.4.
    /// A length that is not finite or not positive (a clip that failed to load) ends a
    /// timed emote at once.
    pub fn emote_duration(&mut self, seconds: f32) {
        if let Some(e) = &mut self.emote
            && e.end.is_none()
        {
            let len = if seconds.is_finite() {
                seconds.max(0.0)
            } else {
                0.0
            };
            e.end = Some(e.start + len);
        }
    }

    /// Whether the current emote waits for its clip's length ([`DramaRun::emote_duration`]).
    #[must_use]
    pub fn wants_emote_duration(&self) -> bool {
        self.emote.is_some_and(|e| e.end.is_none() && !e.endless)
    }

    /// Advances by `dt` seconds and returns the effects for the view, in order.
    pub fn advance(&mut self, dt: f32) -> Vec<RunEffect> {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        let mut out = Vec::new();
        self.clock += dt;
        let now = self.clock;
        self.overhead.retain(|o| o.until > now);
        // A timed emote ends when its clip does.
        if let Some(e) = self.emote
            && !e.endless
            && !e.running(now)
        {
            self.emote = None;
            self.ended.push(e.action);
            out.push(RunEffect::Idle);
        }
        match self.phase {
            Phase::Done => return out,
            Phase::Pause => {
                if now < self.pause_until {
                    return out;
                }
                self.start_idle();
            }
            _ => {}
        }
        // A drama that ends starts the idle drama in the same step (at most twice: the
        // main drama, then the idle one).
        let mut step = dt;
        for _ in 0..2 {
            let before = self.phase;
            let emote = self.emote;
            // An emote applied in this same step is not in `emote` yet: an action whose
            // emote has a clip has just started it, so it runs.
            let started: Vec<bool> = self.current().map_or_else(Vec::new, |d| {
                d.actions
                    .iter()
                    .enumerate()
                    .map(|(i, x)| {
                        x.emote.as_ref().is_some_and(|e| e.clip.is_some())
                            && !self.ended.contains(&i)
                    })
                    .collect()
            });
            let events = self.playback.advance(step, |a| match emote {
                Some(e) if e.action == a => e.running(now),
                _ => started.get(a).copied().unwrap_or(false),
            });
            for ev in events {
                match ev {
                    PlaybackEvent::Apply { action, facets, .. } => {
                        self.apply(action, &facets, &mut out);
                    }
                    PlaybackEvent::UntilStopped { .. } => {}
                    PlaybackEvent::Finished { .. } => self.finish(&mut out),
                }
            }
            if !(before == Phase::Main && self.phase == Phase::Idle) {
                break;
            }
            step = 0.0;
        }
        out
    }

    fn start_idle(&mut self) {
        self.ended.clear();
        if let Some(i) = &self.play.idle {
            self.playback = DramaPlayback::new(i.steps.clone());
            self.phase = Phase::Idle;
            self.log
                .push(format!("{:.1} s: idle drama {:?} (T5)", self.clock, i.name));
        } else {
            self.phase = Phase::Done;
        }
    }

    fn finish(&mut self, out: &mut Vec<RunEffect>) {
        if self.emote.take().is_some() {
            out.push(RunEffect::Idle);
        }
        match self.phase {
            Phase::Main => {
                self.log.push(format!(
                    "{:.1} s: Drama_Performing = false{}",
                    self.clock,
                    if self.play.returns_to_idle {
                        "; returns to the starting idle"
                    } else {
                        ""
                    }
                ));
                out.push(RunEffect::Idle);
                if self.play.drama.kind == DramaKind::Idle {
                    // An idle drama played directly repeats too (T5).
                    self.play.idle = Some(self.play.drama.clone());
                }
                if self.play.idle.is_some() {
                    self.start_idle();
                } else {
                    self.phase = Phase::Done;
                }
            }
            Phase::Idle => {
                self.phase = Phase::Pause;
                self.pause_until = self.clock + IDLE_PAUSE;
            }
            Phase::Pause | Phase::Done => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(emote: Option<(u32, f32)>, wait: bool, line: Option<&str>) -> PreparedAction {
        PreparedAction {
            emote: emote.map(|(key, _)| PreparedEmote {
                key,
                label: format!("K{key}"),
                clip: Some((Did(0x0500_0000 | key), 1.0, false)),
            }),
            wait_emote: wait,
            dialog: line.map(|t| ("4 (ColorSay)".into(), true, true, vec![t.to_owned()])),
            ..PreparedAction::default()
        }
    }

    /// `npc-drama.md` §13.1 played: t = 0 emote and line, t = 4 the second emote, the wait
    /// for it, then the idle drama with its 5 s pause.
    #[test]
    fn greeting_plays_by_t1_and_t5() {
        let main = PreparedDrama {
            name: "Signal_EnterWorld".into(),
            kind: DramaKind::Named,
            interruptible: false,
            steps: vec![
                DramaStep::Apply {
                    action: 0,
                    facets: vec![Facet::Emote, Facet::Dialog],
                },
                DramaStep::Wait(3.0),
                DramaStep::Wait(1.0),
                DramaStep::Apply {
                    action: 1,
                    facets: vec![Facet::Emote],
                },
                DramaStep::WaitForEmote { action: 1 },
            ],
            actions: vec![
                action(Some((1, 0.0)), false, Some("hello")),
                action(Some((2, 0.0)), true, None),
            ],
        };
        let idle = PreparedDrama {
            name: "idle_a".into(),
            kind: DramaKind::Idle,
            interruptible: false,
            steps: vec![
                DramaStep::Apply {
                    action: 0,
                    facets: vec![Facet::Emote],
                },
                DramaStep::Wait(6.0),
            ],
            actions: vec![action(Some((3, 0.0)), false, None)],
        };
        let play = PreparedPlay {
            arg: DramaPlayArg {
                figure: None,
                source: DramaSource::Set(0x7900_0001),
                name: "signal_enterworld".into(),
                caption: "E".into(),
            },
            figure: None,
            drama: main,
            idle: Some(idle),
            returns_to_idle: true,
            speaker: "E".into(),
            notes: Vec::new(),
        };
        let mut r = DramaRun::new(play, 7);
        let fx = r.advance(0.0);
        assert_eq!(fx, vec![RunEffect::PlayClip(Did(0x0500_0001), 1.0, false)]);
        r.emote_duration(2.0);
        assert_eq!(r.overhead.len(), 1);
        assert_eq!(r.chat, vec!["E: hello".to_owned()]);
        // The first emote ends at 2 s (back to idle); the second starts at 4 s, persistent.
        assert_eq!(r.advance(2.5), vec![RunEffect::Idle]);
        let fx = r.advance(1.6);
        assert_eq!(fx, vec![RunEffect::PlayClip(Did(0x0500_0002), 1.0, true)]);
        // The drama waits for it: at most 30 s.
        assert!(r.advance(20.0).is_empty());
        assert!(r.current().is_some_and(|d| d.name == "Signal_EnterWorld"));
        let fx = r.advance(10.5);
        assert!(fx.contains(&RunEffect::Idle));
        assert!(
            fx.contains(&RunEffect::PlayClip(Did(0x0500_0003), 1.0, false)),
            "{fx:?}"
        );
        assert!(r.current().is_some_and(|d| d.name == "idle_a"));
        // The overhead line is gone after 10 s.
        assert!(r.overhead.is_empty());
        // The idle pass (6 s), then the 5 s pause, then again.
        r.advance(6.5);
        assert!(r.status().contains("idle pause"));
        let fx = r.advance(5.0);
        assert!(
            fx.contains(&RunEffect::PlayClip(Did(0x0500_0003), 1.0, false)),
            "{fx:?}"
        );
        assert!(!r.is_done());
    }

    #[test]
    fn random_source_stays_in_range() {
        let mut r = Rng::new(0);
        for n in 1..50 {
            assert!(r.below(n) < n);
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
        assert_eq!(r.below(0), 0);
    }
}
