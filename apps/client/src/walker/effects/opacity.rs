// SPDX-License-Identifier: GPL-3.0-or-later
//! Run-time writes of a static owner's `Render_Opacity` (`particle-runtime.md` §12 rule 15,
//! its spec-r9 answer item 3 and the spec-r10 answer under the anchor `s12-r15-writes`).
//!
//! The scripts of a static effect owner run in the effects layer's script runtime, where a
//! SetProperty node (or any other write) gives the owner a run-time value that masks its
//! stored value. Rule 15 asks the object layer to follow such a value, both in the opacity
//! it draws the object with and in the rule-15 factor of the owner's systems. Once per frame,
//! after every runtime call of the effects layer's frame (the timers, the beam ports), the
//! layer watches the value the runtime reads for each static owner
//! ([`OwnerOpacity::observe`]) and keeps the object layer's run-time value in step with it
//! ([`follow`], `ObjectLayer::set_runtime_opacity`), so several writes in one frame end as
//! the last one (spec-r10 answer item 2). The walker draws the objects after the effects
//! layer's frame, so the object is drawn with the value in that frame; the factor follows one
//! frame later, as rule 15 allows. Writes made through the layer's own calls after its frame
//! (a script started from the walker, `EffectLayer::set_entity_property`) reach the object
//! one frame later, which item 2 allows too.
//!
//! **How a run-time value is told apart** (Tessera engineering choice): the runtime's public
//! interface gives only the value an entity reads (its run-time value, else its stored
//! value, else the dictionary default). The stored value does not change while the owner is
//! spawned, so a value that differs from it ([`OwnerOpacity::new`]'s baseline) is a run-time
//! value, and one equal to it is taken as none. So that this never changes what is drawn,
//! the owner's stored `Render_Opacity` in the runtime is made to give the same factor as
//! the value the object layer reads from the owner's property sets (the per-instance set,
//! then the 0x47 chain; rule 15, spec-r9 answer item 1) wherever the two differ
//! ([`stored_value`]): the script side alone reads only the chain, because the walker's
//! bundles do not keep the landblock's instance sets. A write of exactly the stored value
//! then gives the object the factor it already has, and is not counted as a write.
//!
//! **Values** (spec-r9 answer item 4, spec-r10 answer item 3): a float is clamped to 0–1, a
//! float that is not finite and a value of any other type count as 1 ([`written_factor`]).
//!
//! **Lifetime** (spec-r10 answer item 1): the value lasts for as long as the entity stays
//! placed, that is while the object layer holds its landblock's objects. The runtime owns
//! it while the owner is spawned; the object layer keeps it after the owner despawns (out
//! of its effects' reach), and a respawned owner takes it back as its run-time value right
//! after its start-up scripts ran, unless they wrote one ([`OwnerOpacity::holding`],
//! [`OwnerOpacity::restore`]; listeners of the property react to it then, while the
//! start-up scripts themselves still read the stored value: Tessera's reach-based despawn
//! has no counterpart in the client, so this is an engineering choice). While the owner
//! stays spawned and the landblock's objects are built again, the object layer is handed
//! the runtime's value again. Objects of a landblock placed again after it was unloaded
//! start without one (the owner despawned with the landblock). Removing the value in the
//! runtime removes it from the object layer.
//!
//! **Counts** (spec-r10 answer item 4): every write seen is counted ([`WriteCounts`]) and
//! ends as applied (it reached the object layer), replaced (a later write or a removal came
//! before it reached the object layer), lost (its owner despawned before that) or waiting
//! (the object layer does not hold the owner's object yet).

use tessera_formats::PropertyValue;
use tessera_scriptgraph::EntityId;

use super::data::PlacedObject;
use crate::walker::objects::ObjectLayer;

/// The opacity factor a run-time `Render_Opacity` value gives: a float clamped to 0–1;
/// a float that is not finite, or a value of another type, counts as the dictionary
/// default 1.
#[must_use]
pub fn written_factor(value: &PropertyValue) -> f32 {
    match value {
        PropertyValue::Float(f) if f.is_finite() => f.clamp(0.0, 1.0),
        _ => 1.0,
    }
}

/// Whether two values the runtime reads are the same (floats compared bit for bit, so a
/// stored value that is not a number still equals itself).
fn same(a: Option<&PropertyValue>, b: Option<&PropertyValue>) -> bool {
    match (a, b) {
        (Some(PropertyValue::Float(x)), Some(PropertyValue::Float(y))) => {
            x.to_bits() == y.to_bits()
        }
        (a, b) => a == b,
    }
}

/// Whether two factors are the same, bit for bit (`None`: no value).
fn same_factor(a: Option<f32>, b: Option<f32>) -> bool {
    a.map(f32::to_bits) == b.map(f32::to_bits)
}

/// The run-time value of an owner whose runtime reads `current` and whose stored or
/// default value is `baseline` (module docs): `None` when they agree (no run-time value),
/// else the factor `current` gives ([`written_factor`]).
#[must_use]
pub fn runtime_factor(
    current: Option<&PropertyValue>,
    baseline: Option<&PropertyValue>,
) -> Option<f32> {
    if same(current, baseline) {
        return None;
    }
    current.map(written_factor)
}

/// The stored `Render_Opacity` a static owner's runtime entity takes in place of its
/// scripts' (module docs): `None` when `script_side` (the value its scripts' property sets
/// give, else the dictionary default) gives the same factor as `drawn`, the factor the
/// object layer reads from its property sets (`None`: none holds a usable value, factor 1);
/// else that factor as a float.
#[must_use]
pub fn stored_value(
    script_side: Option<&PropertyValue>,
    drawn: Option<f32>,
) -> Option<PropertyValue> {
    let drawn = match drawn {
        Some(v) if v.is_finite() => v.clamp(0.0, 1.0),
        _ => 1.0,
    };
    let script = script_side.map_or(1.0, written_factor);
    (script.to_bits() != drawn.to_bits()).then_some(PropertyValue::Float(drawn))
}

/// The watch on one static owner's `Render_Opacity` (module docs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OwnerOpacity {
    /// The value the owner reads without a run-time value: its stored value, else the
    /// dictionary default.
    baseline: Option<PropertyValue>,
    /// The run-time factor seen at the last observation (`None`: none).
    seen: Option<f32>,
    /// `seen` is a write that has not reached the object layer yet.
    pending: bool,
}

/// What [`OwnerOpacity::observe`] saw change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpacityChange {
    /// No change since the last observation.
    None,
    /// A run-time value was written (a new value, or one where there was none).
    Written(f32),
    /// The run-time value was removed.
    Removed,
}

/// One [`OwnerOpacity::observe`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observation {
    /// What changed.
    pub change: OpacityChange,
    /// The change replaced a write that had not reached the object layer.
    pub replaced: bool,
}

impl OwnerOpacity {
    /// A watch for an owner whose stored or default value is `baseline`.
    #[must_use]
    pub fn new(baseline: Option<PropertyValue>) -> Self {
        Self::holding(baseline, None)
    }

    /// A watch for an owner (re)spawned while the object layer holds `held`, a run-time
    /// value an earlier spawn of it wrote (module docs, "Lifetime"); `None` as [`Self::new`].
    #[must_use]
    pub fn holding(baseline: Option<PropertyValue>, held: Option<f32>) -> Self {
        Self {
            baseline,
            seen: held,
            pending: false,
        }
    }

    /// The run-time value to give the freshly spawned owner, whose runtime now reads
    /// `current`: the value the object layer held at spawn ([`Self::holding`]), unless the
    /// start-up scripts already wrote one (or there is none).
    #[must_use]
    pub fn restore(&self, current: Option<&PropertyValue>) -> Option<PropertyValue> {
        let held = self.seen?;
        runtime_factor(current, self.baseline.as_ref())
            .is_none()
            .then_some(PropertyValue::Float(held))
    }

    /// Observes the value the runtime reads now; a written value becomes pending for the
    /// object layer ([`Self::write_waiting`]).
    pub fn observe(&mut self, current: Option<&PropertyValue>) -> Observation {
        let now = runtime_factor(current, self.baseline.as_ref());
        if same_factor(self.seen, now) {
            return Observation {
                change: OpacityChange::None,
                replaced: false,
            };
        }
        let replaced = self.write_waiting();
        self.seen = now;
        self.pending = now.is_some();
        Observation {
            change: now.map_or(OpacityChange::Removed, OpacityChange::Written),
            replaced,
        }
    }

    /// The run-time factor the object layer should hold for the owner (`None`: none).
    #[must_use]
    pub fn wanted(&self) -> Option<f32> {
        self.seen
    }

    /// Whether a written value has not reached the object layer yet.
    #[must_use]
    pub fn write_waiting(&self) -> bool {
        self.pending && self.seen.is_some()
    }

    /// The object layer holds [`Self::wanted`].
    fn reached(&mut self) {
        self.pending = false;
    }
}

/// The run-time `Render_Opacity` writes of static owners (module docs, "Counts"):
/// `writes` = `applied` + `replaced` + `lost` + `waiting` at the end of every frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteCounts {
    /// Writes seen (a value that differs from the last one the owner read).
    pub writes: u64,
    /// Of them, those that reached the object layer.
    pub applied: u64,
    /// Those a later write or a removal replaced before they reached the object layer.
    pub replaced: u64,
    /// Those whose owner despawned before they reached the object layer.
    pub lost: u64,
    /// Those still waiting for the object layer after the last frame (one per owner at
    /// most, so also the owners waiting).
    pub waiting: u64,
    /// Static entities of the scanned landblocks whose start-up scripts reach a SetProperty
    /// of `Render_Opacity` but no Particle node: they run no scripts in the walker, so
    /// their writes are never made and not followed (`EffectData::scan_block`).
    pub unfollowed_writers: usize,
}

impl WriteCounts {
    /// The writes that have not (or not yet) reached the object layer.
    #[must_use]
    pub fn not_applied(&self) -> u64 {
        self.writes.saturating_sub(self.applied)
    }

    /// An owner with `watch` despawned.
    pub fn despawned(&mut self, watch: &OwnerOpacity) {
        if watch.write_waiting() {
            self.lost = self.lost.saturating_add(1);
        }
    }
}

/// The object layer's side of the run-time values ([`follow`]).
pub trait RuntimeOpacityLayer {
    /// The run-time value the layer holds for `placed`'s object: `None` when it holds no
    /// object of it, `Some(None)` when the object has no run-time value.
    fn runtime_opacity(&self, placed: &PlacedObject) -> Option<Option<f32>>;
    /// Sets (`Some`) or removes (`None`) the run-time value of `placed`'s object; `false`
    /// when the layer holds no object of it.
    fn set_runtime_opacity(&mut self, placed: &PlacedObject, value: Option<f32>) -> bool;
}

impl RuntimeOpacityLayer for ObjectLayer {
    fn runtime_opacity(&self, placed: &PlacedObject) -> Option<Option<f32>> {
        ObjectLayer::runtime_opacity(self, placed.key, placed.index, &placed.entity)
    }

    fn set_runtime_opacity(&mut self, placed: &PlacedObject, value: Option<f32>) -> bool {
        ObjectLayer::set_runtime_opacity(self, placed.key, placed.index, &placed.entity, value)
    }
}

/// One frame of the watch (module docs): for every static owner the runtime has (`read`
/// gives what it reads, `None` for an owner the runtime does not have), observes its value
/// and, where `layer` holds the owner's object with another run-time value, hands it the
/// owner's (a write, a removal, or a value the layer lost when the landblock's objects were
/// built again while the owner stayed spawned). Counts into `counts`; `waiting` is set anew.
pub fn follow<'a>(
    owners: impl IntoIterator<Item = (EntityId, &'a mut OwnerOpacity, &'a PlacedObject)>,
    mut read: impl FnMut(EntityId) -> Option<Option<PropertyValue>>,
    layer: &mut impl RuntimeOpacityLayer,
    counts: &mut WriteCounts,
) {
    let mut waiting = 0u64;
    for (id, watch, placed) in owners {
        let Some(current) = read(id) else {
            continue;
        };
        let seen = watch.observe(current.as_ref());
        if let OpacityChange::Written(v) = seen.change {
            counts.writes = counts.writes.saturating_add(1);
            tracing::trace!(entity = id.0, value = v, "effects: Render_Opacity written");
        }
        if seen.replaced {
            counts.replaced = counts.replaced.saturating_add(1);
        }
        let wanted = watch.wanted();
        let held = match layer.runtime_opacity(placed) {
            Some(held) => held,
            None => {
                if watch.write_waiting() {
                    waiting += 1;
                }
                continue;
            }
        };
        if !same_factor(held, wanted) && !layer.set_runtime_opacity(placed, wanted) {
            if watch.write_waiting() {
                waiting += 1;
            }
            continue;
        }
        if watch.write_waiting() {
            counts.applied = counts.applied.saturating_add(1);
        }
        watch.reached();
    }
    counts.waiting = waiting;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::objects::test_blocks::TestBlocks;
    use tessera_scriptgraph::{
        EntityInit, MemorySource, NullSink, Runtime, RuntimeVocabulary, SyntheticCatalog,
    };
    use tessera_world::LandblockKey;

    /// Spec-r10 answer item 3 (spec-r9 item 4): floats are clamped, non-finite floats and
    /// other types give 1.
    #[test]
    fn written_values_are_clamped_and_other_types_count_one() {
        assert_eq!(written_factor(&PropertyValue::Float(0.25)), 0.25);
        assert_eq!(written_factor(&PropertyValue::Float(0.0)), 0.0);
        assert_eq!(written_factor(&PropertyValue::Float(1.5)), 1.0);
        assert_eq!(written_factor(&PropertyValue::Float(-3.0)), 0.0);
        assert_eq!(written_factor(&PropertyValue::Float(f32::NAN)), 1.0);
        assert_eq!(
            written_factor(&PropertyValue::Float(f32::NEG_INFINITY)),
            1.0
        );
        assert_eq!(written_factor(&PropertyValue::Int(0)), 1.0);
        assert_eq!(written_factor(&PropertyValue::Enum(0)), 1.0);
    }

    /// A value equal to the stored or default one is no run-time value; any other is.
    #[test]
    fn a_runtime_value_differs_from_the_baseline() {
        let half = PropertyValue::Float(0.5);
        let one = PropertyValue::Float(1.0);
        assert_eq!(runtime_factor(Some(&one), Some(&one)), None);
        assert_eq!(runtime_factor(Some(&half), Some(&one)), Some(0.5));
        assert_eq!(runtime_factor(Some(&half), None), Some(0.5));
        assert_eq!(runtime_factor(None, None), None);
        assert_eq!(runtime_factor(None, Some(&one)), None);
        let nan = PropertyValue::Float(f32::NAN);
        assert_eq!(runtime_factor(Some(&nan), Some(&nan)), None, "bit for bit");
        assert_eq!(
            runtime_factor(Some(&PropertyValue::Int(1)), Some(&one)),
            Some(1.0)
        );
    }

    /// The runtime's stored value is replaced only where its factor differs from the one
    /// the object layer reads from the property sets (rule 15, spec-r9 answer item 1).
    #[test]
    fn the_stored_value_follows_the_object_layers_property_sets() {
        let one = PropertyValue::Float(1.0);
        // The per-instance set hides the object (0); the chain holds nothing (default 1).
        assert_eq!(
            stored_value(Some(&one), Some(0.0)),
            Some(PropertyValue::Float(0.0))
        );
        assert_eq!(
            stored_value(None, Some(0.0)),
            Some(PropertyValue::Float(0.0))
        );
        // Same factor: the script side keeps its own value.
        assert_eq!(stored_value(Some(&one), None), None);
        assert_eq!(
            stored_value(Some(&PropertyValue::Float(0.4)), Some(0.4)),
            None
        );
        assert_eq!(
            stored_value(Some(&PropertyValue::Float(7.0)), Some(1.0)),
            None
        );
        assert_eq!(stored_value(Some(&PropertyValue::Int(3)), None), None);
        assert_eq!(stored_value(None, None), None);
        // The chain gives 0.3 but the per-instance set holds no usable value (factor 1).
        assert_eq!(
            stored_value(Some(&PropertyValue::Float(0.3)), None),
            Some(PropertyValue::Float(1.0))
        );
    }

    /// Spec-r10 answer items 1–3: a write becomes pending until it reaches the layer;
    /// several writes before the next observation end as the last; a removal replaces a
    /// waiting write; an unchanged value changes nothing.
    #[test]
    fn writes_and_removals_are_observed() {
        let stored = PropertyValue::Float(0.8);
        let mut w = OwnerOpacity::new(Some(stored.clone()));
        let none = Observation {
            change: OpacityChange::None,
            replaced: false,
        };
        assert_eq!(w.observe(Some(&stored)), none);
        assert_eq!(w.wanted(), None);
        assert!(!w.write_waiting());
        let o = w.observe(Some(&PropertyValue::Float(0.3)));
        assert_eq!(o.change, OpacityChange::Written(0.3));
        assert!(!o.replaced);
        assert!(w.write_waiting());
        // Not applied yet, written again: the last value is the one wanted.
        let o = w.observe(Some(&PropertyValue::Float(0.1)));
        assert_eq!(o.change, OpacityChange::Written(0.1));
        assert!(o.replaced, "the 0.3 never reached the layer");
        assert_eq!(w.wanted(), Some(0.1));
        w.reached();
        assert_eq!(w.observe(Some(&PropertyValue::Float(0.1))), none);
        // Removed: the stored value reads again.
        let o = w.observe(Some(&stored));
        assert_eq!(o.change, OpacityChange::Removed);
        assert!(!o.replaced);
        assert_eq!(w.wanted(), None);
        assert!(!w.write_waiting());
        // A waiting write the script removes again is replaced.
        w.observe(Some(&PropertyValue::Float(0.2)));
        let o = w.observe(Some(&stored));
        assert_eq!((o.change, o.replaced), (OpacityChange::Removed, true));
        // Clamped writes and a non-float one; a clamped write of the same factor is none.
        assert_eq!(
            w.observe(Some(&PropertyValue::Float(7.0))).change,
            OpacityChange::Written(1.0)
        );
        assert_eq!(w.observe(Some(&PropertyValue::Float(9.0))), {
            Observation {
                change: OpacityChange::None,
                replaced: false,
            }
        });
        assert_eq!(
            w.observe(Some(&PropertyValue::Float(-1.0))).change,
            OpacityChange::Written(0.0)
        );
        assert_eq!(
            w.observe(Some(&PropertyValue::String("x".into()))).change,
            OpacityChange::Written(1.0)
        );
        let mut d = OwnerOpacity::default();
        assert_eq!(d.observe(None), none);
        assert_eq!(
            d.observe(Some(&PropertyValue::Float(0.5))).change,
            OpacityChange::Written(0.5)
        );
    }

    /// A respawned owner takes back the value the object layer held, unless its start-up
    /// scripts wrote one.
    #[test]
    fn a_respawned_owner_takes_the_held_value_back() {
        let one = PropertyValue::Float(1.0);
        let w = OwnerOpacity::holding(Some(one.clone()), Some(0.0));
        assert_eq!(w.restore(Some(&one)), Some(PropertyValue::Float(0.0)));
        assert_eq!(w.restore(Some(&PropertyValue::Float(0.6))), None);
        assert_eq!(
            OwnerOpacity::new(Some(one.clone())).restore(Some(&one)),
            None
        );
    }

    // --- The watch on a real runtime and the object layer's blocks ---

    impl RuntimeOpacityLayer for TestBlocks {
        fn runtime_opacity(&self, placed: &PlacedObject) -> Option<Option<f32>> {
            TestBlocks::runtime_opacity(self, placed.key, placed.index, &placed.entity)
        }

        fn set_runtime_opacity(&mut self, placed: &PlacedObject, value: Option<f32>) -> bool {
            TestBlocks::set_runtime_opacity(self, placed.key, placed.index, &placed.entity, value)
        }
    }

    const PID: u32 = 0x1000_0010;
    const OWNER: EntityId = EntityId(42);

    fn runtime() -> Runtime<'static> {
        let mut catalog = SyntheticCatalog::default();
        catalog.defaults.insert(PID, PropertyValue::Float(1.0));
        Runtime::new(MemorySource::new(), catalog, RuntimeVocabulary::default())
    }

    fn key() -> LandblockKey {
        LandblockKey::new(1, 10, 20).expect("key")
    }

    /// The placed object of entity index 3 of [`key`].
    fn placed() -> PlacedObject {
        PlacedObject {
            key: key(),
            index: Some(3),
            entity: crate::walker::objects::PlacedObjectRef {
                position: [1.0, 2.0, 3.0],
                parts: vec![5].into(),
                rank: 0,
            },
            property_opacity: None,
        }
    }

    /// Spawns [`OWNER`] as `EffectLayer::spawn_owner` does: the stored value made to
    /// agree with `drawn` (the object layer's property-set factor), the watch seeded with
    /// what `layer` holds, the held value restored after the start-up.
    fn spawn(
        g: &mut Runtime<'static>,
        layer: &TestBlocks,
        script_side: Option<PropertyValue>,
        drawn: Option<f32>,
    ) -> OwnerOpacity {
        let p = placed();
        let mut init = EntityInit::default();
        if let Some(v) = &script_side {
            init.properties.set(PID, v.clone());
        }
        let default = Some(PropertyValue::Float(1.0));
        let script = script_side.or(default);
        let stored = stored_value(script.as_ref(), drawn);
        if let Some(v) = &stored {
            init.properties.set(PID, v.clone());
        }
        let held = RuntimeOpacityLayer::runtime_opacity(layer, &p).flatten();
        let watch = OwnerOpacity::holding(stored.or(script), held);
        g.spawn_entity(&mut NullSink, OWNER, init).expect("spawn");
        if let Some(v) = watch.restore(g.property(OWNER, PID).as_ref()) {
            g.set_property(&mut NullSink, OWNER, PID, v)
                .expect("restore");
        }
        watch
    }

    fn frame(
        g: &Runtime<'static>,
        watch: &mut OwnerOpacity,
        layer: &mut TestBlocks,
        counts: &mut WriteCounts,
    ) {
        let p = placed();
        follow(
            [(OWNER, watch, &p)],
            |id| g.has_entity(id).then(|| g.property(id, PID)),
            layer,
            counts,
        );
        assert_eq!(
            counts.writes,
            counts.applied + counts.replaced + counts.lost + counts.waiting,
            "{counts:?}"
        );
    }

    /// Rule 15 end to end (spec-r10 answer items 1–4): a write made before the landblock's
    /// objects are in the layer waits and is applied when they arrive; a write reaches the
    /// drawn factor; a removal brings the property sets' value back once; writes replaced
    /// while waiting and writes of an owner despawned while waiting are counted; an owner
    /// the runtime does not have is skipped.
    #[test]
    fn writes_reach_the_object_layer_and_are_counted() {
        let mut g = runtime();
        let mut layer = TestBlocks::default();
        let mut counts = WriteCounts::default();
        let mut w = spawn(&mut g, &layer, None, Some(0.5));
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(counts, WriteCounts::default(), "no write yet");
        // Written before the block is in the layer: waiting.
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.3))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!((counts.writes, counts.waiting, counts.applied), (1, 1, 0));
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!((counts.writes, counts.waiting), (1, 1), "counted once");
        // Replaced while waiting: the last write counts.
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.2))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!((counts.writes, counts.replaced, counts.waiting), (2, 1, 1));
        // The block arrives: the waiting write is retried and applied.
        layer.insert(key(), 3, 0.5);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.5));
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!((counts.applied, counts.waiting), (1, 0));
        assert_eq!(layer.owner_factor(key(), 3), Some(0.2));
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(counts.applied, 1, "applied once");
        // Several writes in one frame: the last counts.
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.9))
            .expect("write");
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.7))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.7));
        assert_eq!((counts.writes, counts.applied), (3, 2));
        // Removed: the property sets' value again.
        g.clear_property(&mut NullSink, OWNER, PID).expect("clear");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.5));
        assert_eq!(layer.runtime_value(key(), 3), Some(None));
        // A write of the stored value: no write, and the object keeps its factor.
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.5))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(counts.writes, 3);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.5));
        // The block's objects are built again while the owner stays spawned: the value
        // the runtime holds is handed over again, not counted as a new write.
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.1))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        layer.insert(key(), 3, 0.5);
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.1));
        assert_eq!((counts.writes, counts.applied), (4, 3));
        // An owner the runtime does not have is skipped.
        g.despawn(&mut NullSink, OWNER).expect("despawn");
        let before = counts;
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(counts, before);
        // The object layer keeps the value after the despawn (the entity stays placed).
        assert_eq!(layer.owner_factor(key(), 3), Some(0.1));
    }

    /// The per-instance layer hides the object (factor 0) and the chain holds nothing: a
    /// start-up write of 1 reveals it (the finding of the stored-value baseline).
    #[test]
    fn a_write_of_the_chains_value_reveals_an_instance_hidden_object() {
        let mut g = runtime();
        let mut layer = TestBlocks::default();
        layer.insert(key(), 3, 0.0);
        let mut counts = WriteCounts::default();
        let mut w = spawn(&mut g, &layer, None, Some(0.0));
        assert_eq!(g.property(OWNER, PID), Some(PropertyValue::Float(0.0)));
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.0));
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(1.0))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(1.0));
        assert_eq!((counts.writes, counts.applied), (1, 1));
    }

    /// A write waiting when its owner despawns is lost; a respawned owner takes back the
    /// value the object layer held, so a later write of the default or a removal still
    /// reaches the object (the finding of the respawn lifetime).
    #[test]
    fn despawn_and_respawn_keep_one_value() {
        let mut g = runtime();
        let mut layer = TestBlocks::default();
        let mut counts = WriteCounts::default();
        let mut w = spawn(&mut g, &layer, None, None);
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.4))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(counts.waiting, 1);
        g.despawn(&mut NullSink, OWNER).expect("despawn");
        counts.despawned(&w);
        counts.waiting = 0;
        assert_eq!((counts.writes, counts.lost), (1, 1));
        // Respawned with the block in the layer; a write of 0 is applied.
        layer.insert(key(), 3, 1.0);
        let mut w = spawn(&mut g, &layer, None, None);
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.0))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.0));
        // Despawned: the object stays at 0. Respawned: the runtime reads 0 again.
        g.despawn(&mut NullSink, OWNER).expect("despawn");
        counts.despawned(&w);
        assert_eq!(counts.lost, 1, "the 0 had been applied");
        let mut w = spawn(&mut g, &layer, None, None);
        assert_eq!(g.property(OWNER, PID), Some(PropertyValue::Float(0.0)));
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.0));
        // A later write of the default 1 reveals the object.
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(1.0))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(1.0));
        assert_eq!(
            layer.runtime_value(key(), 3),
            Some(None),
            "read as a removal"
        );
        // Respawned again with a stored value of its own: the held 0.25 is taken back, and a
        // later write replaces it (a start-up write is covered by `restore`).
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.25))
            .expect("write");
        frame(&g, &mut w, &mut layer, &mut counts);
        g.despawn(&mut NullSink, OWNER).expect("despawn");
        let mut w = spawn(&mut g, &layer, Some(PropertyValue::Float(1.0)), None);
        assert_eq!(g.property(OWNER, PID), Some(PropertyValue::Float(0.25)));
        g.set_property(&mut NullSink, OWNER, PID, PropertyValue::Float(0.6))
            .expect("later write");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(0.6));
        // A removal by the respawned owner brings the property sets' value back.
        g.clear_property(&mut NullSink, OWNER, PID).expect("clear");
        frame(&g, &mut w, &mut layer, &mut counts);
        assert_eq!(layer.owner_factor(key(), 3), Some(1.0));
        assert_eq!(layer.runtime_value(key(), 3), Some(None));
    }

    /// The HUD figures add up.
    #[test]
    fn not_applied_is_writes_less_applied() {
        let c = WriteCounts {
            writes: 5,
            applied: 2,
            replaced: 1,
            lost: 1,
            waiting: 1,
            unfollowed_writers: 0,
        };
        assert_eq!(c.not_applied(), 3);
    }
}
