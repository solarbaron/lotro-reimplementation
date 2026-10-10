// SPDX-License-Identifier: GPL-3.0-or-later
//! The figures of resident landblocks as the sandbox serves them (`walker.md` §10): each
//! [`tessera_world::PlacedFigure`] of a bundle becomes a sandbox
//! [`StaticEntity`] carrying a [`PlacedFigure`] (served as `EntityCreated`), tagged so the
//! client can find the figure's instance property set again when the event arrives.
//!
//! - **Tag**: the landblock-data DID in the high 32 bits, the figure's index in the
//!   bundle in the low 32 bits ([`figure_tag`], [`tag_parts`]).
//! - **Kind**: NPC-like Physics entities are [`EntityKind::NonPlayer`]; weenie placements
//!   by their role ([`WeenieRole`]): `Monster` → [`EntityKind::Creature`], `RealNPC` →
//!   [`EntityKind::NonPlayer`], doors and markers → [`EntityKind::Object`].
//! - **Appearance**: the placed object (0x47 entity, or the weenie's 0x70 game object) as
//!   [`Body::Object`], the instance set's starting idle, and scale 1 (the instance set's
//!   `Render_AdjustableScale` is part of the assembled model scale, `appearance.md` §6.1)
//!   or, without an instance set, the entity's own scale.
//! - Generated objects on a skinned rig are figures too (`FigureKind::Rig`, a
//!   [`tessera_world::EntityRef::Generated`] entity; `scenery.md` §7.6): served like a
//!   placed rig, without an instance set or IID.
//! - **Class**: the weenie's game object (none for Physics figures).
//! - Interior figures are left out unless interiors are drawn.

use tessera_net::sandbox::world::{PlacedFigure, StaticEntity};
use tessera_net::{Appearance, Body, DataId, EntityKind, Placement, RegionId, Rotation};
use tessera_world::{FigureKind, LandblockBundle, LandblockKey};

use super::characters::catalog::{CharacterCatalog, WeenieRole};

/// The tag of figure `index` of `key`'s bundle.
#[must_use]
pub fn figure_tag(key: LandblockKey, index: u32) -> u64 {
    (u64::from(key.data_did().0) << 32) | u64::from(index)
}

/// The landblock and figure index of a [`figure_tag`].
#[must_use]
pub fn tag_parts(tag: u64) -> Option<(LandblockKey, u32)> {
    let did = u32::try_from(tag >> 32).ok()?;
    let key = LandblockKey::from_did(tessera_formats::Did(did))?;
    (key.data_did().0 == did).then_some((key, (tag & 0xFFFF_FFFF) as u32))
}

/// The role of a figure: `None` for an NPC-like Physics figure, the weenie role otherwise.
#[must_use]
pub fn figure_role(
    kind: FigureKind,
    weenie_type: Option<u32>,
    catalog: Option<&CharacterCatalog>,
) -> Option<WeenieRole> {
    match kind {
        FigureKind::Npc | FigureKind::Rig => None,
        FigureKind::Weenie => {
            Some(catalog.map_or(WeenieRole::Marker, |c| c.weenie_role(weenie_type)))
        }
    }
}

/// The sandbox entries of `bundle`'s figures (module docs). At most `limit` entries.
#[must_use]
pub fn figure_entries(
    bundle: &LandblockBundle,
    catalog: Option<&CharacterCatalog>,
    interiors: bool,
    limit: usize,
) -> Vec<StaticEntity> {
    let key = bundle.key;
    let mut out = Vec::new();
    for (i, f) in bundle.figures.iter().enumerate() {
        if out.len() >= limit {
            break;
        }
        let Some(e) = bundle.figure_entity(f) else {
            continue;
        };
        if e.interior && !interiors {
            continue;
        }
        let Some(object) = e.object else {
            continue;
        };
        let Ok(index) = u32::try_from(i) else {
            break;
        };
        let role = figure_role(f.kind, f.weenie_type, catalog);
        let kind = match role {
            None | Some(WeenieRole::Npc) => EntityKind::NonPlayer,
            Some(WeenieRole::Creature) => EntityKind::Creature,
            Some(WeenieRole::Door | WeenieRole::Marker) => EntityKind::Object,
        };
        let p = e.placement.position;
        let block =
            tessera_net::Landblock::new(RegionId(u16::from(key.region())), key.x(), key.y());
        let Some(block) = block else { continue };
        let location = block.location([f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]);
        let q = e.placement.rotation;
        let rotation = Rotation::new(q.w, q.x, q.y, q.z)
            .normalized()
            .unwrap_or(Rotation::IDENTITY);
        let scale = if f.instance.is_some() {
            1.0
        } else {
            let s = e.scale[0].max(e.scale[1]).max(e.scale[2]);
            if s.is_finite() && s > 0.0 && s <= tessera_net::model::MAX_APPEARANCE_SCALE {
                s
            } else {
                1.0
            }
        };
        let appearance = Appearance {
            scale,
            idle: f.idle,
            ..Appearance::new(Body::Object(DataId(object.0)))
        };
        out.push(
            StaticEntity::new(
                DataId(object.0),
                Placement::new(location, rotation, 1.0),
                figure_tag(key, index),
            )
            .with_figure(PlacedFigure {
                kind,
                name: String::new(),
                class: (f.kind == FigureKind::Weenie).then_some(DataId(object.0)),
                appearance,
            }),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use tessera_net::sandbox::{SandboxBackend, SandboxConfig, SpawnPoint, StaticConfig};
    use tessera_net::{Command, Event, Session};
    use tessera_world::testkit::{SyntheticWorld, obj};
    use tessera_world::{BundleOptions, BundleSource, WorldContext, build_bundle};

    use super::*;
    use crate::walker::source::{FigureBlock, SourceShared, WalkerSource, lock};

    fn bundle(x: u8, y: u8) -> Arc<LandblockBundle> {
        let mut w = SyntheticWorld::rectangle(1, x..=x, y..=y);
        w.figures = true;
        let ctx = WorldContext::load(Arc::new(w.build()), BundleOptions::default()).unwrap();
        let key = LandblockKey::new(1, x, y).unwrap();
        let src = BundleSource::new(Arc::clone(ctx.source()));
        let frame = tessera_assets::RecordSource::read_record(&src, key.data_did().0).unwrap();
        Arc::new(build_bundle(&ctx, key.data_did(), &frame, &|| false).unwrap())
    }

    #[test]
    fn bundle_figures_become_sandbox_entries() {
        let b = bundle(3, 4);
        let entries = figure_entries(&b, None, false, 100);
        assert_eq!(entries.len(), 2);
        // The NPC-like tree (serial 1, at block-local (0, 0, 100)).
        let npc = &entries[0];
        let f = npc.figure.as_ref().unwrap();
        assert_eq!(f.kind, EntityKind::NonPlayer);
        assert_eq!(f.class, None);
        assert_eq!(f.appearance.body, Body::Object(DataId(obj::TREE)));
        assert_eq!(f.appearance.idle, Some(SyntheticWorld::FIGURE_IDLE));
        assert_eq!(
            f.appearance.scale, 1.0,
            "the instance set carries the scale"
        );
        let l = npc.placement.location;
        assert_eq!((l.x, l.y, l.z), (3.0 * 160.0, 4.0 * 160.0, 100.0));
        assert_eq!(tag_parts(npc.tag), Some((b.key, 0)));
        // Unrotated: model +Y faces north.
        let h = npc.placement.rotation.heading().unwrap();
        assert!((h - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        // The weenie placement: its game object as class, a marker without a catalog.
        let w = &entries[1];
        let f = w.figure.as_ref().unwrap();
        assert_eq!(f.kind, EntityKind::Object);
        assert_eq!(f.class, Some(DataId(obj::WEENIE)));
        assert_eq!(f.appearance.body, Body::Object(DataId(obj::WEENIE)));
        let l = w.placement.location;
        assert_eq!(
            (l.x, l.y, l.z),
            (3.0 * 160.0 + 80.0, 4.0 * 160.0 + 80.0, 1.0)
        );
        assert_eq!(figure_entries(&b, None, false, 1).len(), 1, "limit");
    }

    #[test]
    fn the_sandbox_creates_the_figures_of_the_walkers_bundles() {
        let b = bundle(3, 4);
        let shared = Arc::new(Mutex::new(SourceShared {
            default_region: 1,
            ready: true,
            ..SourceShared::default()
        }));
        let config = SandboxConfig {
            spawn_point: Some(SpawnPoint::parse("1,3,4,80,80,100").unwrap()),
            statics: StaticConfig {
                radius: 1,
                ..StaticConfig::default()
            },
            wanderers: 0,
            ..SandboxConfig::default()
        };
        let backend =
            SandboxBackend::with_world(config, WalkerSource::new(Arc::clone(&shared))).unwrap();
        let mut session = Session::new(backend);
        let mut now = Instant::now();
        session.send(now, Command::Connect).unwrap();
        let _ = session.poll(now);
        session
            .send(
                now,
                Command::EnterWorld {
                    character: "Wanderer".into(),
                },
            )
            .unwrap();
        let mut events = Vec::new();
        for _ in 0..3 {
            events.extend(session.poll(now));
            now += Duration::from_millis(16);
        }
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::EntityCreated { .. })),
            "nothing until the walker has the bundle"
        );
        {
            let mut s = lock(&shared);
            s.figures.insert(b.key, FigureBlock::Ready(Arc::clone(&b)));
            // The neighbours have no data.
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(n) = b.key.offset(dx, dy)
                        && n != b.key
                    {
                        s.figures.insert(n, FigureBlock::Missing);
                    }
                }
            }
        }
        for _ in 0..5 {
            events.extend(session.poll(now));
            now += Duration::from_millis(16);
        }
        let created: Vec<(u64, f64, f32)> = events
            .iter()
            .filter_map(|e| match e {
                Event::EntityCreated {
                    tag: Some(t),
                    location,
                    heading,
                    ..
                } => Some((*t, location.x, *heading)),
                _ => None,
            })
            .collect();
        assert_eq!(created.len(), 2, "{created:?}");
        assert_eq!(tag_parts(created[0].0), Some((b.key, 0)));
        assert_eq!(created[0].1, 480.0);
        let info = lock(&shared).figure_info(created[0].0).expect("resident");
        assert!(info.instance.is_some());
        assert_eq!(info.kind, FigureKind::Npc);
        assert_eq!(info.role, None);
        let info = lock(&shared).figure_info(created[1].0).expect("resident");
        assert_eq!(info.kind, FigureKind::Weenie);
        assert_eq!(info.role, Some(WeenieRole::Marker));
    }

    /// A generated object on a skinned rig is served like a placed rig (`walker.md` §10.2
    /// Rig row, `scenery.md` §7.6): the character layer animates it, and its static
    /// instance (bind pose) is left out while the character is drawn.
    #[test]
    fn generated_rigs_become_sandbox_entries() {
        let schema = tessera_world::testkit::schema();
        let (template, setup) = (0x1F00_0030u32, 0x0400_0030u32);
        let mut src = SyntheticWorld {
            scenery: true,
            water_level: Some(12),
            ..SyntheticWorld::rectangle(1, 10..=11, 20..=20)
        }
        .build();
        src.insert(
            obj::ON_WATER,
            tessera_world::testkit::entity_descriptor(obj::ON_WATER, template, &[], &schema),
            None,
        );
        src.insert(
            template,
            tessera_world::testkit::visual_template(template, setup),
            None,
        );
        src.insert(
            setup,
            tessera_world::testkit::setup_with_skeleton(setup, &[obj::TREE_PARTS[0]], 36),
            None,
        );
        let ctx = WorldContext::load(Arc::new(src), BundleOptions::default()).unwrap();
        let key = LandblockKey::new(1, 10, 20).unwrap();
        let bsrc = BundleSource::new(Arc::clone(ctx.source()));
        let frame = tessera_assets::RecordSource::read_record(&bsrc, key.data_did().0).unwrap();
        let b = Arc::new(build_bundle(&ctx, key.data_did(), &frame, &|| false).unwrap());
        let (index, f) = b
            .figures
            .iter()
            .enumerate()
            .find(|(_, f)| matches!(f.entity, tessera_world::EntityRef::Generated(_)))
            .expect("a generated rig");
        assert_eq!(f.kind, FigureKind::Rig);
        let g = b.figure_entity(f).unwrap();
        let entries = figure_entries(&b, None, false, 100);
        let e = entries
            .iter()
            .find(|e| tag_parts(e.tag) == Some((key, u32::try_from(index).unwrap())))
            .expect("served");
        let fig = e.figure.as_ref().unwrap();
        assert_eq!(fig.kind, EntityKind::NonPlayer);
        assert_eq!(fig.class, None);
        assert_eq!(fig.appearance.body, Body::Object(DataId(obj::ON_WATER)));
        assert_eq!(fig.appearance.idle, None);
        let p = g.placement.position;
        let l = e.placement.location;
        assert_eq!(
            (l.x, l.y, l.z),
            (
                10.0 * 160.0 + f64::from(p[0]),
                20.0 * 160.0 + f64::from(p[1]),
                f64::from(p[2])
            )
        );
        // The figure's tag finds it again, without an IID (generated objects have none).
        let mut shared = SourceShared {
            default_region: 1,
            ready: true,
            ..SourceShared::default()
        };
        shared
            .figures
            .insert(key, FigureBlock::Ready(Arc::clone(&b)));
        let info = shared.figure_info(e.tag).expect("resident");
        assert_eq!(info.kind, FigureKind::Rig);
        assert_eq!(info.iid, None);
        assert!(info.instance.is_none());
    }

    #[test]
    fn tags_round_trip() {
        let k = LandblockKey::new(1, 27, 143).unwrap();
        let t = figure_tag(k, 5);
        assert_eq!(tag_parts(t), Some((k, 5)));
        assert_eq!(tag_parts(5), None);
        assert_eq!(tag_parts(u64::MAX), None);
    }
}
