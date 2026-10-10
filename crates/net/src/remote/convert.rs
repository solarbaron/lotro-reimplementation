// SPDX-License-Identifier: GPL-3.0-or-later
//! Conversions between `tessera-protocol` wire types and the neutral model of
//! [`crate::model`] (`tessera-protocol.md` Appendix A). Every inbound value has already
//! been checked by the protocol decoder (§5, §13); these functions only change
//! representation, and never panic.

use tessera_protocol::message as wire;
use tessera_protocol::types as wt;

use crate::model::{
    Anchor, Appearance, Body, ChatChannel, DataId, Destination, EntityKind, Equipped, Landblock,
    Location, Look, LookEntry, PropertyValue, RegionId, Transform, Vec3,
};

pub(crate) fn location(l: &wt::Location) -> Location {
    Location::new(RegionId(l.region), l.x, l.y, l.z)
}

pub(crate) fn wire_location(l: &Location) -> wt::Location {
    wt::Location {
        region: l.region.0,
        x: l.x,
        y: l.y,
        z: l.z,
    }
}

/// The model anchor of a wire anchor in `region`. `None` only for block indices above
/// 254, which the decoder already refuses.
pub(crate) fn anchor(region: u16, a: &wt::Anchor) -> Option<Anchor> {
    Landblock::new(RegionId(region), a.block_x, a.block_y).map(|b| Anchor::new(b, a.cell))
}

pub(crate) fn wire_anchor(a: &Anchor) -> wt::Anchor {
    wt::Anchor {
        block_x: a.block.x,
        block_y: a.block.y,
        cell: a.cell,
    }
}

/// The pose's location, reduced heading and anchor.
pub(crate) fn pose(p: &wt::Pose) -> (Location, f32, Option<Anchor>) {
    let region = p.location.region;
    (
        location(&p.location),
        wt::reduce_heading(p.heading),
        p.anchor.as_ref().and_then(|a| anchor(region, a)),
    )
}

pub(crate) fn vec3(v: &wt::Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

pub(crate) fn wire_vec3(v: Vec3) -> wt::Vec3 {
    wt::Vec3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

pub(crate) fn transform(location: &Location, heading: f32) -> Transform {
    Transform::new(location.position(), heading)
}

pub(crate) fn body(b: &wt::Body) -> Body {
    match *b {
        wt::Body::Character { species, sex } => Body::Character { species, sex },
        wt::Body::Object { did } => Body::Object(DataId(did)),
    }
}

/// The model appearance of a wire appearance (no look).
#[cfg(test)]
pub(crate) fn appearance(a: &wt::Appearance) -> Appearance {
    appearance_with_look(a, &wt::Look::none())
}

/// The model appearance of a wire appearance and the look that travels beside it (minor
/// ≥ 1; an empty wire look is none).
pub(crate) fn appearance_with_look(a: &wt::Appearance, l: &wt::Look) -> Appearance {
    Appearance {
        body: body(&a.body),
        scale: a.scale,
        equipment: a
            .equipment
            .iter()
            .map(|e| Equipped {
                item: DataId(e.item),
                dye: e.dye,
                secondary_dye: e.secondary_dye,
                stowed: e.stowed,
                slot: e.slot,
            })
            .collect(),
        idle: a.idle,
        look: look(l).map(Box::new),
    }
}

/// The model look of a wire look; `None` when it is empty. The decoder has checked every
/// rule of §5.3 already.
pub(crate) fn look(l: &wt::Look) -> Option<Look> {
    if l.entries.is_empty() {
        return None;
    }
    let entries = l
        .entries
        .iter()
        .map(|e| LookEntry {
            property: e.property,
            value: match &e.value {
                wt::LookValue::Bool(b) => PropertyValue::Bool(*b),
                wt::LookValue::Int32(v) => PropertyValue::Int(*v),
                wt::LookValue::Int64(v) => PropertyValue::Int64(*v),
                wt::LookValue::Float32(v) => PropertyValue::Float(*v),
                wt::LookValue::Enum(v) => PropertyValue::Enum(*v),
                wt::LookValue::DataId(v) => PropertyValue::Did(DataId(*v)),
                wt::LookValue::String(s) => PropertyValue::String(s.clone()),
                wt::LookValue::Bitfield32(v) => PropertyValue::Bitfield32(*v),
                wt::LookValue::Bitfield64(v) => PropertyValue::Bitfield64(*v),
                wt::LookValue::Colour(c) => PropertyValue::Colour(*c),
            },
        })
        .collect();
    Some(Look { entries })
}

/// The wire look of a model look (empty for none). Entries the wire cannot carry (an
/// instance value, which a valid look never holds) are left out.
pub(crate) fn wire_look(l: Option<&Look>) -> wt::Look {
    let Some(l) = l else {
        return wt::Look::none();
    };
    wt::Look {
        entries: l
            .entries
            .iter()
            .filter_map(|e| {
                let value = match &e.value {
                    PropertyValue::Bool(b) => wt::LookValue::Bool(*b),
                    PropertyValue::Int(v) => wt::LookValue::Int32(*v),
                    PropertyValue::Int64(v) => wt::LookValue::Int64(*v),
                    PropertyValue::Float(v) => wt::LookValue::Float32(*v),
                    PropertyValue::Enum(v) => wt::LookValue::Enum(*v),
                    PropertyValue::Did(d) => wt::LookValue::DataId(d.0),
                    PropertyValue::String(s) => wt::LookValue::String(s.clone()),
                    PropertyValue::Bitfield32(v) => wt::LookValue::Bitfield32(*v),
                    PropertyValue::Bitfield64(v) => wt::LookValue::Bitfield64(*v),
                    PropertyValue::Colour(c) => wt::LookValue::Colour(*c),
                    _ => return None,
                };
                Some(wt::LookEntry {
                    property: e.property,
                    value,
                })
            })
            .collect(),
    }
}

/// The wire form of a model appearance, or `None` when the protocol cannot carry it (an
/// object body with DID 0, or a body kind the protocol does not know).
pub(crate) fn wire_appearance(a: &Appearance) -> Option<wt::Appearance> {
    let body = match a.body {
        Body::Character { species, sex } => wt::Body::Character { species, sex },
        Body::Object(DataId(did)) if did != 0 => wt::Body::Object { did },
        _ => return None,
    };
    let mut out = wt::Appearance::new(body);
    out.scale = a.scale;
    out.idle = a.idle;
    out.equipment = a
        .equipment
        .iter()
        .map(|e| wt::Equipped {
            item: e.item.0,
            dye: e.dye,
            secondary_dye: e.secondary_dye,
            stowed: e.stowed,
            // Sent only at minor ≥ 1; the encoder leaves it off at minor 0 (§3.1).
            slot: e.slot,
        })
        .collect();
    Some(out)
}

pub(crate) fn kind(k: wire::EntityKind) -> EntityKind {
    match k {
        wire::EntityKind::Player => EntityKind::Player,
        wire::EntityKind::NonPlayer => EntityKind::NonPlayer,
        wire::EntityKind::Creature => EntityKind::Creature,
        wire::EntityKind::Object => EntityKind::Object,
    }
}

pub(crate) fn chat_channel(c: &wire::ChatChannel) -> ChatChannel {
    match c {
        wire::ChatChannel::Say => ChatChannel::Say,
        wire::ChatChannel::Yell => ChatChannel::Yell,
        wire::ChatChannel::Emote => ChatChannel::Emote,
        wire::ChatChannel::Tell { peer } => ChatChannel::Tell { peer: peer.clone() },
        wire::ChatChannel::System => ChatChannel::System,
    }
}

/// The wire channel for an outgoing line; `None` for System (server only).
pub(crate) fn wire_chat_channel(c: &ChatChannel) -> Option<wire::ChatChannel> {
    Some(match c {
        ChatChannel::Say => wire::ChatChannel::Say,
        ChatChannel::Yell => wire::ChatChannel::Yell,
        ChatChannel::Emote => wire::ChatChannel::Emote,
        ChatChannel::Tell { peer } => wire::ChatChannel::Tell { peer: peer.clone() },
        // Fellowship and named channels have no wire form until a minor version of
        // `tessera-protocol.md` adds them (scenario-feed.md §9.5).
        ChatChannel::System | ChatChannel::Fellowship | ChatChannel::Named { .. } => return None,
    })
}

/// The wire destination of a teleport (tags 1–4, Appendix A).
pub(crate) fn wire_destination(d: &Destination) -> wt::Destination {
    match d {
        Destination::Location { location, heading } => wt::Destination::Location {
            location: wire_location(location),
            heading: *heading,
        },
        Destination::Ground { location, heading } => wt::Destination::Ground {
            location: wire_location(location),
            heading: *heading,
        },
        Destination::Anchored {
            location,
            heading,
            anchor,
        } => wt::Destination::Anchored {
            location: wire_location(location),
            heading: *heading,
            anchor: wire_anchor(anchor),
        },
        Destination::Named(name) => wt::Destination::Named { name: name.clone() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_round_trip() {
        let a = Appearance::new(Body::Character {
            species: 23,
            sex: 1,
        })
        .wearing(Equipped {
            item: DataId(0x7000_1234),
            dye: Some(0.25),
            secondary_dye: None,
            stowed: true,
            slot: Some(17),
        });
        let w = wire_appearance(&a).unwrap();
        assert_eq!(appearance(&w), a);
        let l = Look {
            entries: vec![
                LookEntry {
                    property: 1,
                    value: PropertyValue::Float(0.5),
                },
                LookEntry {
                    property: 2,
                    value: PropertyValue::Colour([1, 2, 3, 4]),
                },
                LookEntry {
                    property: 3,
                    value: PropertyValue::String("80ff".into()),
                },
            ],
        };
        let wl = wire_look(Some(&l));
        assert!(wl.check().is_ok());
        let back = appearance_with_look(&w, &wl);
        assert_eq!(back.look.as_deref(), Some(&l));
        assert_eq!(look(&tessera_protocol::types::Look::none()), None);
        assert!(wire_appearance(&Appearance::new(Body::Object(DataId(0)))).is_none());
    }

    #[test]
    fn destinations_and_channels() {
        let l = Location::new(RegionId(1), 20_250.5, 22_500.25, 401.0);
        let block = Landblock::containing(&l).unwrap();
        let a = Anchor::new(block, 257);
        match wire_destination(&Destination::Anchored {
            location: l,
            heading: 1.5,
            anchor: a,
        }) {
            wt::Destination::Anchored { anchor: w, .. } => {
                assert_eq!(super::anchor(1, &w), Some(a));
            }
            other => panic!("{other:?}"),
        }
        assert!(wire_chat_channel(&ChatChannel::System).is_none());
        assert_eq!(
            chat_channel(&wire::ChatChannel::Tell {
                peer: "Bram".into()
            }),
            ChatChannel::Tell {
                peer: "Bram".into()
            }
        );
        assert_eq!(location(&wire_location(&l)), l);
    }
}
