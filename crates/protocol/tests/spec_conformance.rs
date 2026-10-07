// SPDX-License-Identifier: GPL-3.0-or-later
//! Independent conformance check of `tessera-protocol` against
//! `docs/specs/tessera-protocol.md` (M7 exit criterion 2).
//!
//! The [`spec`] module is a second, deliberately small codec for the frame layer (§5.4), the
//! datagram layer (§5.5), every message's plaintext layout (§5.1–§6.5) and the tagged
//! property values (§7.2). It was written by the protocol reviewer from the spec text alone,
//! before reading `crates/protocol/src`, so it shares no code or structure with the crate.
//! The tests check that:
//!
//! * the independent codec reproduces every §16 test vector byte for byte, and decodes each
//!   one back to the stated values;
//! * `tessera_protocol`'s public encoder produces the same bytes for the same values, and its
//!   decoder reads the vectors back to the same values;
//! * both codecs reject every input of a hand-written list of malformed messages that the
//!   spec makes a protocol violation (§3, §5, §6, §7.2, §13.1, §16.7);
//! * the §16.1 handshake vectors (SubjectPublicKeyInfo, SHA-256 fingerprint text, ALPN,
//!   connect string) and the §16.5 time-sync worked example hold.
//!
//! Only rules the spec states unambiguously are in the reject list. Rules whose
//! fatal-or-not status the spec leaves open are listed as `> **OPEN:** (review)` notes in the
//! spec instead.

use sha2::{Digest, Sha256};
use tessera_protocol::codes::{
    AppErrorCode, CharacterCreateFailedCode, ChatRejectedCode, EnterWorldFailedCode,
    LoginFailedCode, TeleportRefusedCode,
};
use tessera_protocol::connect::ConnectString;
use tessera_protocol::fingerprint::Fingerprint;
use tessera_protocol::frame::{
    FrameDecoder, StreamKind, decode_datagram, encode_datagram, frame_to_vec,
};
use tessera_protocol::message as pm;
use tessera_protocol::time::TimeSample;
use tessera_protocol::types as pt;
use tessera_protocol::value as pv;
use tessera_protocol::{Channel, Message, Side};

/// A second codec, written from `docs/specs/tessera-protocol.md` only.
mod spec {
    /// Why a decode failed. Every variant is a protocol violation on a complete input.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Bad {
        /// The body ended before its last field (§3).
        Short,
        /// Bytes were left after the last field (§3).
        Trailing,
        /// Any other fatal rule, named.
        Violation(&'static str),
    }

    pub type R<T> = Result<T, Bad>;

    fn v<T>(why: &'static str) -> R<T> {
        Err(Bad::Violation(why))
    }

    /// The side that sent a message.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Sender {
        Client,
        Server,
    }

    /// The channel a message arrived on (§2.3).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Chan {
        Control,
        World,
        Chat,
        Datagram,
    }

    /// §6.1: type, channel, sender (`None` = both directions).
    const TABLE: &[(u16, Chan, Option<Sender>)] = &[
        (0x0001, Chan::Control, Some(Sender::Client)),
        (0x0002, Chan::Control, Some(Sender::Server)),
        (0x0003, Chan::Control, Some(Sender::Client)),
        (0x0004, Chan::Control, Some(Sender::Client)),
        (0x0005, Chan::Control, Some(Sender::Server)),
        (0x0006, Chan::Control, Some(Sender::Server)),
        (0x0007, Chan::Control, Some(Sender::Client)),
        (0x0008, Chan::Control, Some(Sender::Server)),
        (0x0009, Chan::Control, Some(Sender::Server)),
        (0x000A, Chan::Control, Some(Sender::Client)),
        (0x000B, Chan::Control, Some(Sender::Server)),
        (0x000C, Chan::Control, Some(Sender::Client)),
        (0x000D, Chan::Control, Some(Sender::Client)),
        (0x000E, Chan::Control, Some(Sender::Client)),
        (0x000F, Chan::Control, Some(Sender::Client)),
        (0x0010, Chan::Control, Some(Sender::Client)),
        (0x0011, Chan::Control, Some(Sender::Client)),
        (0x0012, Chan::Control, None),
        (0x0013, Chan::Control, None),
        (0x0014, Chan::Control, None),
        (0x0101, Chan::World, Some(Sender::Server)),
        (0x0102, Chan::World, Some(Sender::Server)),
        (0x0103, Chan::World, Some(Sender::Server)),
        (0x0104, Chan::World, Some(Sender::Server)),
        (0x0105, Chan::World, Some(Sender::Server)),
        (0x0106, Chan::World, Some(Sender::Server)),
        (0x0107, Chan::World, Some(Sender::Server)),
        (0x0108, Chan::World, Some(Sender::Server)),
        (0x0109, Chan::World, Some(Sender::Server)),
        (0x0201, Chan::Chat, Some(Sender::Server)),
        (0x0202, Chan::Chat, Some(Sender::Server)),
        (0x0203, Chan::Chat, Some(Sender::Server)),
        (0x0301, Chan::Datagram, Some(Sender::Client)),
        (0x0302, Chan::Datagram, Some(Sender::Server)),
        (0x0303, Chan::Datagram, Some(Sender::Client)),
        (0x0304, Chan::Datagram, Some(Sender::Server)),
    ];

    /// §5.3 Location.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Loc {
        pub region: u16,
        pub x: f64,
        pub y: f64,
        pub z: f64,
    }

    /// §5.3 Anchor.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Anc {
        pub bx: u8,
        pub by: u8,
        pub cell: u16,
    }

    /// §5.3 Pose.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Pose {
        pub loc: Loc,
        pub heading: f32,
        pub anchor: Option<Anc>,
    }

    pub type V3 = [f32; 3];

    /// §5.3 Body.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Body {
        Character(u32, u32),
        Object(u32),
    }

    /// §5.3 Equipped.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Item {
        pub item: u32,
        pub dye: Option<f32>,
        pub dye2: Option<f32>,
        pub stowed: bool,
    }

    /// §5.3 Appearance.
    #[derive(Debug, Clone, PartialEq)]
    pub struct App {
        pub body: Body,
        pub scale: f32,
        pub idle: Option<u32>,
        pub items: Vec<Item>,
    }

    /// §5.3 Destination.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Dest {
        Location(Loc, f32),
        Ground(Loc, f32),
        Anchored(Loc, f32, Anc),
        Named(String),
    }

    /// §7.2 tagged value.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Val {
        None,
        Bool(bool),
        I32(i32),
        I64(i64),
        F32(f32),
        Enum(u32),
        Did(u32),
        Str(String),
        V3(V3),
        B32(u32),
        B64(u64),
        Ent(u64),
        Time(u64),
        Rgba([u8; 4]),
        Arr(Vec<Val>),
        Struct(Vec<(u16, Val)>),
    }

    pub type Props = Vec<(u16, Val)>;

    /// §6.5.2 entry.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Entry {
        pub id: u64,
        pub pos: V3,
        pub heading: f32,
        pub vel: V3,
        pub motion: u8,
    }

    /// Every v1.0 message (§6).
    #[derive(Debug, Clone, PartialEq)]
    pub enum Msg {
        Hello {
            minor_max: u16,
            caps: u32,
            max_entities: u16,
            client_name: String,
        },
        HelloAck {
            minor: u16,
            caps: u32,
            tick: u8,
            snapshot: u8,
            radius: u8,
            name: String,
            motd: String,
        },
        Login(String, String),
        Register(String, String),
        LoginOk(String, Vec<(String, Body)>),
        LoginFailed(u16, u16, String),
        CharacterCreate(String, App),
        CharacterList(Vec<(String, Body)>),
        CharacterCreateFailed(u16, String),
        EnterWorld(String),
        EnterWorldFailed(u16, String),
        TeleportRequest(u32, Dest),
        QueryLocation(u32),
        ChatSend {
            channel: u8,
            peer: Option<String>,
            text: String,
        },
        EmoteSet(u32, bool),
        StanceSet(u8),
        AppearanceSet(App),
        Ping(u64),
        Pong(u64),
        Disconnect(u16, String),
        WorldEnter {
            player: u64,
            name: String,
            instance: u32,
            epoch: u32,
            time: u64,
            pose: Pose,
            app: App,
        },
        LocationReport {
            cause: u8,
            request: u32,
            epoch: u32,
            time: u64,
            pose: Pose,
        },
        TeleportRefused(u32, u16, String),
        EntityCreate {
            id: u64,
            kind: u8,
            name: String,
            class: Option<u32>,
            time: u64,
            pose: Pose,
            vel: V3,
            motion: u8,
            app: App,
            props: Props,
        },
        EntityUpdate(u64, Props),
        EntityAppearance(u64, App),
        EntityDestroy(u64, u8),
        InterestReset,
        WorldClock(u64, f64, f32),
        ChatMessage {
            time: u64,
            channel: u8,
            flags: u8,
            sender_entity: u64,
            sender: String,
            peer: Option<String>,
            text: String,
        },
        ServerNotice(u8, u16, String),
        ChatRejected(u16, String),
        MoveInput {
            seq: u32,
            epoch: u32,
            loc: Loc,
            heading: f32,
            vel: V3,
            motion: u8,
        },
        MotionBatch(u64, Vec<Entry>),
        TimeRequest(u64),
        TimeReply(u64, u64, u64, u64),
    }

    impl Msg {
        /// The §6.1 type code.
        pub fn ty(&self) -> u16 {
            match self {
                Msg::Hello { .. } => 0x0001,
                Msg::HelloAck { .. } => 0x0002,
                Msg::Login(..) => 0x0003,
                Msg::Register(..) => 0x0004,
                Msg::LoginOk(..) => 0x0005,
                Msg::LoginFailed(..) => 0x0006,
                Msg::CharacterCreate(..) => 0x0007,
                Msg::CharacterList(..) => 0x0008,
                Msg::CharacterCreateFailed(..) => 0x0009,
                Msg::EnterWorld(..) => 0x000A,
                Msg::EnterWorldFailed(..) => 0x000B,
                Msg::TeleportRequest(..) => 0x000C,
                Msg::QueryLocation(..) => 0x000D,
                Msg::ChatSend { .. } => 0x000E,
                Msg::EmoteSet(..) => 0x000F,
                Msg::StanceSet(..) => 0x0010,
                Msg::AppearanceSet(..) => 0x0011,
                Msg::Ping(..) => 0x0012,
                Msg::Pong(..) => 0x0013,
                Msg::Disconnect(..) => 0x0014,
                Msg::WorldEnter { .. } => 0x0101,
                Msg::LocationReport { .. } => 0x0102,
                Msg::TeleportRefused(..) => 0x0103,
                Msg::EntityCreate { .. } => 0x0104,
                Msg::EntityUpdate(..) => 0x0105,
                Msg::EntityAppearance(..) => 0x0106,
                Msg::EntityDestroy(..) => 0x0107,
                Msg::InterestReset => 0x0108,
                Msg::WorldClock(..) => 0x0109,
                Msg::ChatMessage { .. } => 0x0201,
                Msg::ServerNotice(..) => 0x0202,
                Msg::ChatRejected(..) => 0x0203,
                Msg::MoveInput { .. } => 0x0301,
                Msg::MotionBatch(..) => 0x0302,
                Msg::TimeRequest(..) => 0x0303,
                Msg::TimeReply(..) => 0x0304,
            }
        }
    }

    // ------------------------------------------------------------------ encoder (§5.1)
    //
    // The encoder writes whatever it is given, valid or not, so the tests can build
    // malformed messages with it.

    struct W(Vec<u8>);

    impl W {
        fn u8(&mut self, x: u8) {
            self.0.push(x);
        }
        fn u16(&mut self, x: u16) {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        fn u32(&mut self, x: u32) {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        fn u64(&mut self, x: u64) {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        fn f32(&mut self, x: f32) {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        fn f64(&mut self, x: f64) {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        fn str(&mut self, s: &str) {
            self.u16(u16::try_from(s.len()).expect("test strings fit a u16"));
            self.0.extend_from_slice(s.as_bytes());
        }
        fn loc(&mut self, l: &Loc) {
            self.u16(l.region);
            self.f64(l.x);
            self.f64(l.y);
            self.f64(l.z);
        }
        fn anc(&mut self, a: &Anc) {
            self.u8(a.bx);
            self.u8(a.by);
            self.u16(a.cell);
        }
        fn pose(&mut self, p: &Pose) {
            self.loc(&p.loc);
            self.f32(p.heading);
            match &p.anchor {
                None => self.u8(0),
                Some(a) => {
                    self.u8(1);
                    self.anc(a);
                }
            }
        }
        fn v3(&mut self, v: &V3) {
            for c in v {
                self.f32(*c);
            }
        }
        fn body(&mut self, b: &Body) {
            match *b {
                Body::Character(species, sex) => {
                    self.u8(1);
                    self.u32(species);
                    self.u32(sex);
                }
                Body::Object(did) => {
                    self.u8(2);
                    self.u32(did);
                }
            }
        }
        fn app(&mut self, a: &App) {
            self.body(&a.body);
            self.f32(a.scale);
            match a.idle {
                None => self.u8(0),
                Some(i) => {
                    self.u8(1);
                    self.u32(i);
                }
            }
            self.u8(u8::try_from(a.items.len()).expect("test lists fit a u8"));
            for it in &a.items {
                self.u32(it.item);
                let flags = u8::from(it.dye.is_some())
                    | (u8::from(it.dye2.is_some()) << 1)
                    | (u8::from(it.stowed) << 2);
                self.u8(flags);
                if let Some(d) = it.dye {
                    self.f32(d);
                }
                if let Some(d) = it.dye2 {
                    self.f32(d);
                }
            }
        }
        fn chars(&mut self, list: &[(String, Body)]) {
            self.u8(u8::try_from(list.len()).expect("test lists fit a u8"));
            for (n, b) in list {
                self.str(n);
                self.body(b);
            }
        }
        fn props(&mut self, p: &Props) {
            self.u16(u16::try_from(p.len()).expect("test lists fit a u16"));
            for (pid, val) in p {
                self.u16(*pid);
                self.val(val);
            }
        }
        fn val(&mut self, x: &Val) {
            match x {
                Val::None => self.u8(0x00),
                Val::Bool(b) => {
                    self.u8(0x01);
                    self.u8(u8::from(*b));
                }
                Val::I32(i) => {
                    self.u8(0x02);
                    self.0.extend_from_slice(&i.to_le_bytes());
                }
                Val::I64(i) => {
                    self.u8(0x03);
                    self.0.extend_from_slice(&i.to_le_bytes());
                }
                Val::F32(f) => {
                    self.u8(0x04);
                    self.f32(*f);
                }
                Val::Enum(k) => {
                    self.u8(0x05);
                    self.u32(*k);
                }
                Val::Did(d) => {
                    self.u8(0x06);
                    self.u32(*d);
                }
                Val::Str(s) => {
                    self.u8(0x07);
                    self.str(s);
                }
                Val::V3(v3) => {
                    self.u8(0x08);
                    self.v3(v3);
                }
                Val::B32(b) => {
                    self.u8(0x09);
                    self.u32(*b);
                }
                Val::B64(b) => {
                    self.u8(0x0A);
                    self.u64(*b);
                }
                Val::Ent(e) => {
                    self.u8(0x0B);
                    self.u64(*e);
                }
                Val::Time(t) => {
                    self.u8(0x0C);
                    self.u64(*t);
                }
                Val::Rgba(c) => {
                    self.u8(0x0D);
                    self.0.extend_from_slice(c);
                }
                Val::Arr(items) => {
                    self.u8(0x0E);
                    self.u16(u16::try_from(items.len()).expect("test lists fit a u16"));
                    for i in items {
                        self.val(i);
                    }
                }
                Val::Struct(fields) => {
                    self.u8(0x0F);
                    self.u16(u16::try_from(fields.len()).expect("test lists fit a u16"));
                    for (f, x) in fields {
                        self.u16(*f);
                        self.val(x);
                    }
                }
            }
        }
        fn dest(&mut self, d: &Dest) {
            match d {
                Dest::Location(l, h) => {
                    self.u8(1);
                    self.loc(l);
                    self.f32(*h);
                }
                Dest::Ground(l, h) => {
                    self.u8(2);
                    self.loc(l);
                    self.f32(*h);
                }
                Dest::Anchored(l, h, a) => {
                    self.u8(3);
                    self.loc(l);
                    self.f32(*h);
                    self.anc(a);
                }
                Dest::Named(n) => {
                    self.u8(4);
                    self.str(n);
                }
            }
        }
    }

    /// Encodes a tagged value (§7.2).
    pub fn value_bytes(x: &Val) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.val(x);
        w.0
    }

    /// The message body, without type or length.
    pub fn body(m: &Msg) -> Vec<u8> {
        let mut w = W(Vec::new());
        match m {
            Msg::Hello {
                minor_max,
                caps,
                max_entities,
                client_name,
            } => {
                w.u16(*minor_max);
                w.u32(*caps);
                w.u16(*max_entities);
                w.str(client_name);
            }
            Msg::HelloAck {
                minor,
                caps,
                tick,
                snapshot,
                radius,
                name,
                motd,
            } => {
                w.u16(*minor);
                w.u32(*caps);
                w.u8(*tick);
                w.u8(*snapshot);
                w.u8(*radius);
                w.str(name);
                w.str(motd);
            }
            Msg::Login(a, p) | Msg::Register(a, p) => {
                w.str(a);
                w.str(p);
            }
            Msg::LoginOk(a, list) => {
                w.str(a);
                w.chars(list);
            }
            Msg::LoginFailed(code, retry, text) => {
                w.u16(*code);
                w.u16(*retry);
                w.str(text);
            }
            Msg::CharacterCreate(n, a) => {
                w.str(n);
                w.app(a);
            }
            Msg::CharacterList(list) => w.chars(list),
            Msg::CharacterCreateFailed(c, t)
            | Msg::EnterWorldFailed(c, t)
            | Msg::ChatRejected(c, t)
            | Msg::Disconnect(c, t) => {
                w.u16(*c);
                w.str(t);
            }
            Msg::EnterWorld(n) => w.str(n),
            Msg::TeleportRequest(id, d) => {
                w.u32(*id);
                w.dest(d);
            }
            Msg::QueryLocation(id) => w.u32(*id),
            Msg::ChatSend {
                channel,
                peer,
                text,
            } => {
                w.u8(*channel);
                if let Some(p) = peer {
                    w.str(p);
                }
                w.str(text);
            }
            Msg::EmoteSet(anim, looping) => {
                w.u32(*anim);
                w.u8(u8::from(*looping));
            }
            Msg::StanceSet(s) => w.u8(*s),
            Msg::AppearanceSet(a) => w.app(a),
            Msg::Ping(n) | Msg::Pong(n) => w.u64(*n),
            Msg::WorldEnter {
                player,
                name,
                instance,
                epoch,
                time,
                pose,
                app,
            } => {
                w.u64(*player);
                w.str(name);
                w.u32(*instance);
                w.u32(*epoch);
                w.u64(*time);
                w.pose(pose);
                w.app(app);
            }
            Msg::LocationReport {
                cause,
                request,
                epoch,
                time,
                pose,
            } => {
                w.u8(*cause);
                w.u32(*request);
                w.u32(*epoch);
                w.u64(*time);
                w.pose(pose);
            }
            Msg::TeleportRefused(id, code, text) => {
                w.u32(*id);
                w.u16(*code);
                w.str(text);
            }
            Msg::EntityCreate {
                id,
                kind,
                name,
                class,
                time,
                pose,
                vel,
                motion,
                app,
                props,
            } => {
                w.u64(*id);
                w.u8(*kind);
                w.str(name);
                match class {
                    None => w.u8(0),
                    Some(c) => {
                        w.u8(1);
                        w.u32(*c);
                    }
                }
                w.u64(*time);
                w.pose(pose);
                w.v3(vel);
                w.u8(*motion);
                w.app(app);
                w.props(props);
            }
            Msg::EntityUpdate(id, props) => {
                w.u64(*id);
                w.props(props);
            }
            Msg::EntityAppearance(id, a) => {
                w.u64(*id);
                w.app(a);
            }
            Msg::EntityDestroy(id, reason) => {
                w.u64(*id);
                w.u8(*reason);
            }
            Msg::InterestReset => {}
            Msg::WorldClock(t, g, r) => {
                w.u64(*t);
                w.f64(*g);
                w.f32(*r);
            }
            Msg::ChatMessage {
                time,
                channel,
                flags,
                sender_entity,
                sender,
                peer,
                text,
            } => {
                w.u64(*time);
                w.u8(*channel);
                w.u8(*flags);
                w.u64(*sender_entity);
                w.str(sender);
                if let Some(p) = peer {
                    w.str(p);
                }
                w.str(text);
            }
            Msg::ServerNotice(level, seconds, text) => {
                w.u8(*level);
                w.u16(*seconds);
                w.str(text);
            }
            Msg::MoveInput {
                seq,
                epoch,
                loc,
                heading,
                vel,
                motion,
            } => {
                w.u32(*seq);
                w.u32(*epoch);
                w.loc(loc);
                w.f32(*heading);
                w.v3(vel);
                w.u8(*motion);
            }
            Msg::MotionBatch(t, entries) => {
                w.u64(*t);
                w.u8(u8::try_from(entries.len()).expect("test lists fit a u8"));
                for e in entries {
                    w.u64(e.id);
                    w.v3(&e.pos);
                    w.f32(e.heading);
                    w.v3(&e.vel);
                    w.u8(e.motion);
                }
            }
            Msg::TimeRequest(t0) => w.u64(*t0),
            Msg::TimeReply(t0, t1, t2, tick) => {
                w.u64(*t0);
                w.u64(*t1);
                w.u64(*t2);
                w.u64(*tick);
            }
        }
        w.0
    }

    /// A stream frame (§5.4): `u32` length = 2 + body, `u16` type, body.
    pub fn frame(m: &Msg) -> Vec<u8> {
        frame_raw(m.ty(), &body(m))
    }

    /// A stream frame around arbitrary body bytes.
    pub fn frame_raw(ty: u16, body: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + body.len());
        out.extend_from_slice(&u32::try_from(body.len() + 2).expect("fits").to_le_bytes());
        out.extend_from_slice(&ty.to_le_bytes());
        out.extend_from_slice(body);
        out
    }

    /// A datagram message (§5.5): `u16` type, body, no length.
    pub fn datagram(m: &Msg) -> Vec<u8> {
        let mut out = m.ty().to_le_bytes().to_vec();
        out.extend_from_slice(&body(m));
        out
    }

    // ------------------------------------------------------------------ decoder

    /// §5.2 text class, as the receiver applies it for this sender.
    #[derive(Clone, Copy)]
    struct Class {
        /// Lengths below this are fatal (0 when the class minimum is not fatal).
        min: usize,
        /// Lengths above this are fatal, detected before the bytes are read.
        max: usize,
        /// Control characters become U+0020 (server text, §5.2).
        sanitise: bool,
    }

    const fn srv(min: usize, max: usize) -> Class {
        Class {
            min,
            max,
            sanitise: true,
        }
    }

    /// Client text: only the maximum is fatal; content and emptiness give the request's own
    /// failure code, which is not a decoder matter (§5.2).
    const fn cli(max: usize) -> Class {
        Class {
            min: 0,
            max,
            sanitise: false,
        }
    }

    const S_NAME: Class = srv(1, 64);
    const S_NAME0: Class = srv(0, 64);
    const S_TEXT: Class = srv(1, 1024);
    const S_ACCOUNT: Class = Class {
        min: 1,
        max: 32,
        sanitise: false,
    };
    const fn s_text0(n: usize) -> Class {
        srv(0, n)
    }
    const C_NAME: Class = cli(64);
    const C_TEXT: Class = cli(1024);
    const C_ACCOUNT: Class = cli(32);
    const C_PASSWORD: Class = cli(256);
    const C_PLACE: Class = cli(256);
    const C_TEXT0_64: Class = cli(64);
    const C_TEXT0_1024: Class = cli(1024);

    /// Unicode general category Cc (§1 provenance row "Text").
    fn is_cc(c: char) -> bool {
        matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}')
    }

    struct Rd<'a> {
        b: &'a [u8],
        p: usize,
    }

    impl<'a> Rd<'a> {
        fn new(b: &'a [u8]) -> Self {
            Self { b, p: 0 }
        }
        fn left(&self) -> usize {
            self.b.len() - self.p
        }
        fn take(&mut self, n: usize) -> R<&'a [u8]> {
            if self.left() < n {
                return Err(Bad::Short);
            }
            let s = &self.b[self.p..self.p + n];
            self.p += n;
            Ok(s)
        }
        fn arr<const N: usize>(&mut self) -> R<[u8; N]> {
            let mut a = [0u8; N];
            a.copy_from_slice(self.take(N)?);
            Ok(a)
        }
        fn u8(&mut self) -> R<u8> {
            Ok(self.take(1)?[0])
        }
        fn peek(&self) -> R<u8> {
            self.b.get(self.p).copied().ok_or(Bad::Short)
        }
        fn u16(&mut self) -> R<u16> {
            Ok(u16::from_le_bytes(self.arr()?))
        }
        fn u32(&mut self) -> R<u32> {
            Ok(u32::from_le_bytes(self.arr()?))
        }
        fn u64(&mut self) -> R<u64> {
            Ok(u64::from_le_bytes(self.arr()?))
        }
        fn f32(&mut self) -> R<f32> {
            let x = f32::from_le_bytes(self.arr()?);
            if x.is_finite() {
                Ok(x)
            } else {
                v("non-finite f32")
            }
        }
        fn f64(&mut self) -> R<f64> {
            let x = f64::from_le_bytes(self.arr()?);
            if x.is_finite() {
                Ok(x)
            } else {
                v("non-finite f64")
            }
        }
        fn boolean(&mut self) -> R<bool> {
            match self.u8()? {
                0 => Ok(false),
                1 => Ok(true),
                _ => v("bool not 0 or 1"),
            }
        }
        fn present(&mut self) -> R<bool> {
            match self.u8()? {
                0 => Ok(false),
                1 => Ok(true),
                _ => v("opt tag not 0 or 1"),
            }
        }
        fn text(&mut self, c: Class) -> R<String> {
            let n = usize::from(self.u16()?);
            // §5.2: above the class maximum is fatal before the bytes are read.
            if n > c.max {
                return v("string above class maximum");
            }
            if n < c.min {
                return v("string below class minimum");
            }
            let raw = self.take(n)?;
            let s = std::str::from_utf8(raw).map_err(|_| Bad::Violation("invalid UTF-8"))?;
            Ok(if c.sanitise {
                s.chars()
                    .map(|ch| if is_cc(ch) { ' ' } else { ch })
                    .collect()
            } else {
                s.to_owned()
            })
        }
        fn end(&self) -> R<()> {
            if self.left() == 0 {
                Ok(())
            } else {
                Err(Bad::Trailing)
            }
        }

        fn loc(&mut self) -> R<Loc> {
            let region = self.u16()?;
            let x = self.f64()?;
            let y = self.f64()?;
            let z = self.f64()?;
            if [x, y, z].iter().any(|c| c.abs() > 1.0e7) {
                return v("coordinate above 10^7");
            }
            Ok(Loc { region, x, y, z })
        }
        fn anc(&mut self) -> R<Anc> {
            let bx = self.u8()?;
            let by = self.u8()?;
            let cell = self.u16()?;
            if bx > 254 || by > 254 {
                return v("landblock index above 254");
            }
            Ok(Anc { bx, by, cell })
        }
        fn pose(&mut self) -> R<Pose> {
            let loc = self.loc()?;
            let heading = self.f32()?;
            let anchor = if self.present()? {
                let a = self.anc()?;
                if !owns(&a, &loc) {
                    return v("anchor does not own the location");
                }
                Some(a)
            } else {
                None
            };
            Ok(Pose {
                loc,
                heading,
                anchor,
            })
        }
        fn v3(&mut self) -> R<V3> {
            Ok([self.f32()?, self.f32()?, self.f32()?])
        }
        fn motion(&mut self) -> R<u8> {
            let m = self.u8()?;
            if m & 0x07 > 3 {
                return v("gait 4-7");
            }
            if m & 0xF0 != 0 {
                return v("motion reserved bits");
            }
            Ok(m)
        }
        fn body(&mut self) -> R<Body> {
            match self.u8()? {
                1 => Ok(Body::Character(self.u32()?, self.u32()?)),
                2 => {
                    let did = self.u32()?;
                    if did == 0 {
                        return v("object DID 0");
                    }
                    Ok(Body::Object(did))
                }
                _ => v("body tag"),
            }
        }
        fn dye(&mut self) -> R<f32> {
            let d = self.f32()?;
            if (0.0..=1.0).contains(&d) {
                Ok(d)
            } else {
                v("dye outside [0, 1]")
            }
        }
        fn app(&mut self) -> R<App> {
            let body = self.body()?;
            let scale = self.f32()?;
            if !(scale > 0.0 && scale <= 100.0) {
                return v("scale outside (0, 100]");
            }
            let idle = if self.present()? {
                Some(self.u32()?)
            } else {
                None
            };
            let n = self.u8()?;
            if n > 32 {
                return v("more than 32 items");
            }
            let mut items = Vec::with_capacity(usize::from(n));
            for _ in 0..n {
                let item = self.u32()?;
                if item == 0 {
                    return v("item DID 0");
                }
                let flags = self.u8()?;
                if flags & 0xF8 != 0 {
                    return v("item flag bits 3-7");
                }
                let dye = if flags & 1 != 0 {
                    Some(self.dye()?)
                } else {
                    None
                };
                let dye2 = if flags & 2 != 0 {
                    Some(self.dye()?)
                } else {
                    None
                };
                items.push(Item {
                    item,
                    dye,
                    dye2,
                    stowed: flags & 4 != 0,
                });
            }
            Ok(App {
                body,
                scale,
                idle,
                items,
            })
        }
        fn chars(&mut self) -> R<Vec<(String, Body)>> {
            let n = self.u8()?;
            if n > 64 {
                return v("more than 64 characters");
            }
            let mut out = Vec::with_capacity(usize::from(n));
            for _ in 0..n {
                let name = self.text(S_NAME)?;
                out.push((name, self.body()?));
            }
            Ok(out)
        }
        fn dest(&mut self) -> R<Dest> {
            Ok(match self.u8()? {
                1 => Dest::Location(self.loc()?, self.f32()?),
                2 => Dest::Ground(self.loc()?, self.f32()?),
                3 => {
                    let l = self.loc()?;
                    let h = self.f32()?;
                    let a = self.anc()?;
                    if !owns(&a, &l) {
                        return v("anchored destination not owned");
                    }
                    Dest::Anchored(l, h, a)
                }
                4 => Dest::Named(self.text(C_PLACE)?),
                _ => return v("destination tag"),
            })
        }

        /// §7.2: depth of a top-level value is 1; each Array or Struct level adds one.
        fn val(&mut self, depth: usize, allow_none: bool) -> R<Val> {
            if depth > 4 {
                return v("value deeper than 4");
            }
            Ok(match self.u8()? {
                0x00 if allow_none => Val::None,
                0x00 => return v("None not allowed here"),
                0x01 => Val::Bool(self.boolean()?),
                0x02 => Val::I32(i32::from_le_bytes(self.arr()?)),
                0x03 => Val::I64(i64::from_le_bytes(self.arr()?)),
                0x04 => Val::F32(self.f32()?),
                0x05 => Val::Enum(self.u32()?),
                0x06 => Val::Did(self.u32()?),
                0x07 => Val::Str(self.text(srv(0, 1024))?),
                0x08 => Val::V3(self.v3()?),
                0x09 => Val::B32(self.u32()?),
                0x0A => Val::B64(self.u64()?),
                0x0B => Val::Ent(self.u64()?),
                0x0C => Val::Time(self.u64()?),
                0x0D => Val::Rgba(self.arr()?),
                0x0E => {
                    let n = self.u16()?;
                    if n > 256 {
                        return v("array longer than 256");
                    }
                    let mut tag = None;
                    let mut items = Vec::new();
                    for _ in 0..n {
                        let t = self.peek()?;
                        if *tag.get_or_insert(t) != t {
                            return v("array element tags differ");
                        }
                        items.push(self.val(depth + 1, false)?);
                    }
                    Val::Arr(items)
                }
                0x0F => {
                    let n = self.u16()?;
                    if n > 64 {
                        return v("struct with more than 64 fields");
                    }
                    let mut last = 0u16;
                    let mut fields = Vec::new();
                    for _ in 0..n {
                        let f = self.u16()?;
                        if f <= last {
                            return v("struct field 0 or not ascending");
                        }
                        last = f;
                        fields.push((f, self.val(depth + 1, false)?));
                    }
                    Val::Struct(fields)
                }
                _ => return v("value tag 0x10-0xFF"),
            })
        }
        fn top_val(&mut self, allow_none: bool) -> R<Val> {
            let start = self.p;
            let x = self.val(1, allow_none)?;
            if self.p - start > 8192 {
                return v("value larger than 8192 bytes");
            }
            Ok(x)
        }
        fn props(&mut self, allow_none: bool, min: usize) -> R<Props> {
            let n = usize::from(self.u16()?);
            if n > 256 || n < min {
                return v("property count");
            }
            let mut last = 0u16;
            let mut out = Vec::new();
            for _ in 0..n {
                let pid = self.u16()?;
                if pid <= last {
                    return v("pid 0 or not strictly ascending");
                }
                last = pid;
                let x = self.top_val(allow_none)?;
                // §7.2: wrong kind for a defined property (§7.3) is fatal; None removes.
                let kind_ok = matches!(
                    (pid, &x),
                    (_, Val::None)
                        | (1, Val::Enum(_))
                        | (2, Val::Struct(_))
                        | (3 | 4, Val::F32(_))
                        | (5.., _)
                );
                if !kind_ok {
                    return v("wrong kind for a defined property");
                }
                out.push((pid, x));
            }
            Ok(out)
        }
    }

    /// §5.3 `Anchor::owns`.
    pub fn owns(a: &Anc, l: &Loc) -> bool {
        (l.x - 160.0 * f64::from(a.bx)).abs() <= 1280.0
            && (l.y - 160.0 * f64::from(a.by)).abs() <= 1280.0
    }

    fn check_type(ty: u16, chan: Chan, from: Sender) -> R<()> {
        let Some(&(_, c, dir)) = TABLE.iter().find(|e| e.0 == ty) else {
            return v("unknown message type");
        };
        if c != chan {
            return v("wrong channel");
        }
        if dir.is_some_and(|d| d != from) {
            return v("wrong direction");
        }
        Ok(())
    }

    /// Decodes one message body of a known, allowed type.
    fn decode_body(ty: u16, b: &[u8]) -> R<Msg> {
        let mut r = Rd::new(b);
        let m = match ty {
            0x0001 => {
                let minor_max = r.u16()?;
                let caps = r.u32()?;
                let max_entities = r.u16()?;
                if max_entities < 64 {
                    return v("max_entities below 64");
                }
                Msg::Hello {
                    minor_max,
                    caps,
                    max_entities,
                    client_name: r.text(C_TEXT0_64)?,
                }
            }
            0x0002 => Msg::HelloAck {
                minor: r.u16()?,
                caps: r.u32()?,
                tick: r.u8()?,
                snapshot: r.u8()?,
                radius: r.u8()?,
                name: r.text(s_text0(64))?,
                motd: r.text(s_text0(1024))?,
            },
            0x0003 | 0x0004 => {
                let a = r.text(C_ACCOUNT)?;
                let p = r.text(C_PASSWORD)?;
                if ty == 3 {
                    Msg::Login(a, p)
                } else {
                    Msg::Register(a, p)
                }
            }
            0x0005 => Msg::LoginOk(r.text(S_ACCOUNT)?, r.chars()?),
            0x0006 => Msg::LoginFailed(r.u16()?, r.u16()?, r.text(s_text0(256))?),
            0x0007 => Msg::CharacterCreate(r.text(C_NAME)?, r.app()?),
            0x0008 => Msg::CharacterList(r.chars()?),
            0x0009 => Msg::CharacterCreateFailed(r.u16()?, r.text(s_text0(256))?),
            0x000A => Msg::EnterWorld(r.text(C_NAME)?),
            0x000B => Msg::EnterWorldFailed(r.u16()?, r.text(s_text0(256))?),
            0x000C => Msg::TeleportRequest(r.u32()?, r.dest()?),
            0x000D => Msg::QueryLocation(r.u32()?),
            0x000E => {
                let channel = r.u8()?;
                if !(1..=4).contains(&channel) {
                    return v("ChatSend channel");
                }
                let peer = if channel == 4 {
                    Some(r.text(C_NAME)?)
                } else {
                    None
                };
                Msg::ChatSend {
                    channel,
                    peer,
                    text: r.text(C_TEXT)?,
                }
            }
            0x000F => Msg::EmoteSet(r.u32()?, r.boolean()?),
            0x0010 => {
                let s = r.u8()?;
                if s > 2 {
                    return v("stance");
                }
                Msg::StanceSet(s)
            }
            0x0011 => Msg::AppearanceSet(r.app()?),
            0x0012 => Msg::Ping(r.u64()?),
            0x0013 => Msg::Pong(r.u64()?),
            // Disconnect travels both ways; TEXT0(1,024) either way. Sanitising is the
            // server-to-client rule; the vectors carry no control characters.
            0x0014 => Msg::Disconnect(r.u16()?, r.text(C_TEXT0_1024)?),
            0x0101 => Msg::WorldEnter {
                player: r.u64()?,
                name: r.text(S_NAME)?,
                instance: r.u32()?,
                epoch: r.u32()?,
                time: r.u64()?,
                pose: r.pose()?,
                app: r.app()?,
            },
            0x0102 => {
                let cause = r.u8()?;
                if !(1..=3).contains(&cause) {
                    return v("LocationReport cause");
                }
                Msg::LocationReport {
                    cause,
                    request: r.u32()?,
                    epoch: r.u32()?,
                    time: r.u64()?,
                    pose: r.pose()?,
                }
            }
            0x0103 => Msg::TeleportRefused(r.u32()?, r.u16()?, r.text(s_text0(256))?),
            0x0104 => {
                let id = r.u64()?;
                let kind = r.u8()?;
                if kind > 3 {
                    return v("entity kind");
                }
                let name = r.text(S_NAME0)?;
                let class = if r.present()? { Some(r.u32()?) } else { None };
                Msg::EntityCreate {
                    id,
                    kind,
                    name,
                    class,
                    time: r.u64()?,
                    pose: r.pose()?,
                    vel: r.v3()?,
                    motion: r.motion()?,
                    app: r.app()?,
                    props: r.props(false, 0)?,
                }
            }
            0x0105 => Msg::EntityUpdate(r.u64()?, r.props(true, 1)?),
            0x0106 => Msg::EntityAppearance(r.u64()?, r.app()?),
            0x0107 => {
                let id = r.u64()?;
                let reason = r.u8()?;
                if reason > 1 {
                    return v("destroy reason");
                }
                Msg::EntityDestroy(id, reason)
            }
            0x0108 => Msg::InterestReset,
            0x0109 => Msg::WorldClock(r.u64()?, r.f64()?, r.f32()?),
            0x0201 => {
                let time = r.u64()?;
                let channel = r.u8()?;
                if !(1..=5).contains(&channel) {
                    return v("ChatMessage channel");
                }
                let flags = r.u8()?;
                let sender_entity = r.u64()?;
                let sender = r.text(S_NAME0)?;
                let peer = if channel == 4 {
                    Some(r.text(S_NAME)?)
                } else {
                    None
                };
                Msg::ChatMessage {
                    time,
                    channel,
                    flags,
                    sender_entity,
                    sender,
                    peer,
                    text: r.text(S_TEXT)?,
                }
            }
            0x0202 => {
                let level = r.u8()?;
                if level > 2 {
                    return v("notice level");
                }
                Msg::ServerNotice(level, r.u16()?, r.text(S_TEXT)?)
            }
            0x0203 => Msg::ChatRejected(r.u16()?, r.text(s_text0(256))?),
            0x0301 => Msg::MoveInput {
                seq: r.u32()?,
                epoch: r.u32()?,
                loc: r.loc()?,
                heading: r.f32()?,
                vel: r.v3()?,
                motion: r.motion()?,
            },
            0x0302 => {
                let t = r.u64()?;
                let n = usize::from(r.u8()?);
                if !(1..=29).contains(&n) {
                    return v("MotionBatch count");
                }
                if r.left() != 37 * n {
                    return v("MotionBatch length is not 11 + 37 n");
                }
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    entries.push(Entry {
                        id: r.u64()?,
                        pos: r.v3()?,
                        heading: r.f32()?,
                        vel: r.v3()?,
                        motion: r.motion()?,
                    });
                }
                Msg::MotionBatch(t, entries)
            }
            0x0303 => Msg::TimeRequest(r.u64()?),
            0x0304 => Msg::TimeReply(r.u64()?, r.u64()?, r.u64()?, r.u64()?),
            _ => return v("unknown message type"),
        };
        r.end()?;
        Ok(m)
    }

    /// Splits one frame off a stream buffer. `Ok(None)` = more bytes needed. The length is
    /// checked before any body byte is looked at (§5.4).
    pub fn split(buf: &[u8]) -> R<Option<(u16, &[u8], usize)>> {
        let Some(h) = buf.get(..4) else {
            return Ok(None);
        };
        let len = u32::from_le_bytes([h[0], h[1], h[2], h[3]]);
        if !(2..=65_536).contains(&len) {
            return v("frame length outside 2..=65536");
        }
        let total = 4 + len as usize;
        let Some(f) = buf.get(4..total) else {
            return Ok(None);
        };
        Ok(Some((u16::from_le_bytes([f[0], f[1]]), &f[2..], total)))
    }

    /// Decodes every complete frame of `buf` as received on `chan` from `from`.
    /// Returns the messages and whether an incomplete frame was left over.
    pub fn decode_stream(chan: Chan, from: Sender, mut buf: &[u8]) -> R<(Vec<Msg>, bool)> {
        let mut out = Vec::new();
        while !buf.is_empty() {
            let Some((ty, body, used)) = split(buf)? else {
                return Ok((out, true));
            };
            check_type(ty, chan, from)?;
            out.push(decode_body(ty, body)?);
            buf = &buf[used..];
        }
        Ok((out, false))
    }

    /// Decodes one datagram (§5.5).
    pub fn decode_datagram(from: Sender, d: &[u8]) -> R<Msg> {
        if d.len() > 1200 {
            return v("datagram above 1200 bytes");
        }
        if d.len() < 2 {
            return Err(Bad::Short);
        }
        let ty = u16::from_le_bytes([d[0], d[1]]);
        check_type(ty, Chan::Datagram, from)?;
        decode_body(ty, &d[2..])
    }

    /// The kind byte of a server-initiated unidirectional stream (§2.3 rule 1).
    pub fn stream_kind(b: u8) -> R<Chan> {
        match b {
            0x57 => Ok(Chan::World),
            0x43 => Ok(Chan::Chat),
            _ => v("unknown stream kind"),
        }
    }

    /// Decodes one top-level tagged value; returns it and the bytes used.
    pub fn decode_value(b: &[u8], allow_none: bool) -> R<(Val, usize)> {
        let mut r = Rd::new(b);
        let x = r.top_val(allow_none)?;
        Ok((x, r.p))
    }

    // ------------------------------------------------------------------ §4.2, §4.4

    /// §4.2: SubjectPublicKeyInfo of an Ed25519 key.
    pub fn ed25519_spki(key: &[u8; 32]) -> Vec<u8> {
        let mut out = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        out.extend_from_slice(key);
        out
    }

    /// §4.2 parsing: exact lowercase prefix, then 64 hex digits of either case.
    pub fn parse_fingerprint(s: &str) -> Option<[u8; 32]> {
        let hex = s.strip_prefix("sha256:")?;
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut out = [0u8; 32];
        for (i, o) in out.iter_mut().enumerate() {
            *o = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(out)
    }

    /// §11.2 offset and delay (exact; the vector's values divide evenly).
    pub fn ntp(t0: u64, t1: u64, t2: u64, t3: u64) -> (i128, i128) {
        let (t0, t1, t2, t3) = (
            i128::from(t0),
            i128::from(t1),
            i128::from(t2),
            i128::from(t3),
        );
        (((t1 - t0) + (t2 - t3)) / 2, (t3 - t0) - (t2 - t1))
    }
}

use spec::{Anc, App, Body, Chan, Dest, Entry, Item, Loc, Msg, Pose, Sender, Val};

// ---------------------------------------------------------------------- helpers

fn hex(s: &str) -> Vec<u8> {
    let digits: Vec<u8> = s.bytes().filter(u8::is_ascii_hexdigit).collect();
    assert_eq!(digits.len() % 2, 0, "odd hex digit count in {s:?}");
    digits
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).expect("ascii"), 16).expect("hex"))
        .collect()
}

/// §16 value L.
fn l() -> Loc {
    Loc {
        region: 1,
        x: 20_250.5,
        y: 22_500.25,
        z: 401.0,
    }
}

/// §16 anchor (126, 140, cell 257).
fn anc() -> Anc {
    Anc {
        bx: 126,
        by: 140,
        cell: 257,
    }
}

/// §16 appearance A.
fn a() -> App {
    App {
        body: Body::Character(23, 1),
        scale: 1.0,
        idle: None,
        items: vec![
            Item {
                item: 0x7000_1234,
                dye: Some(0.25),
                dye2: None,
                stowed: false,
            },
            Item {
                item: 0x7000_5678,
                dye: None,
                dye2: None,
                stowed: true,
            },
        ],
    }
}

fn plain(body: Body) -> App {
    App {
        body,
        scale: 1.0,
        idle: None,
        items: Vec::new(),
    }
}

fn s(x: &str) -> String {
    x.to_owned()
}

fn chan_of(c: Chan) -> Channel {
    match c {
        Chan::Control => Channel::Control,
        Chan::World => Channel::World,
        Chan::Chat => Channel::Chat,
        Chan::Datagram => Channel::Datagram,
    }
}

fn side_of(f: Sender) -> Side {
    match f {
        Sender::Client => Side::Client,
        Sender::Server => Side::Server,
    }
}

// ---------------------------------------------------------------------- spec -> crate values

fn c_loc(x: &Loc) -> pt::Location {
    pt::Location::new(x.region, x.x, x.y, x.z)
}

fn c_anc(x: &Anc) -> pt::Anchor {
    pt::Anchor::new(x.bx, x.by, x.cell)
}

fn c_pose(p: &Pose) -> pt::Pose {
    pt::Pose {
        location: c_loc(&p.loc),
        heading: p.heading,
        anchor: p.anchor.as_ref().map(c_anc),
    }
}

fn c_v3(x: &[f32; 3]) -> pt::Vec3 {
    pt::Vec3::new(x[0], x[1], x[2])
}

fn c_body(b: &Body) -> pt::Body {
    match *b {
        Body::Character(species, sex) => pt::Body::Character { species, sex },
        Body::Object(did) => pt::Body::Object { did },
    }
}

fn c_app(x: &App) -> pt::Appearance {
    pt::Appearance {
        body: c_body(&x.body),
        scale: x.scale,
        idle: x.idle,
        equipment: x
            .items
            .iter()
            .map(|i| pt::Equipped {
                item: i.item,
                dye: i.dye,
                secondary_dye: i.dye2,
                stowed: i.stowed,
            })
            .collect(),
    }
}

fn c_chars(list: &[(String, Body)]) -> Vec<pt::CharacterSummary> {
    list.iter()
        .map(|(n, b)| pt::CharacterSummary {
            name: n.clone(),
            body: c_body(b),
        })
        .collect()
}

fn c_motion(b: u8) -> pt::Motion {
    pt::Motion::from_byte(b).expect("valid motion byte in a positive vector")
}

fn c_val(x: &Val) -> pv::Value {
    match x {
        Val::None => pv::Value::None,
        Val::Bool(b) => pv::Value::Bool(*b),
        Val::I32(i) => pv::Value::Int32(*i),
        Val::I64(i) => pv::Value::Int64(*i),
        Val::F32(f) => pv::Value::Float32(*f),
        Val::Enum(k) => pv::Value::Enum(*k),
        Val::Did(d) => pv::Value::DataId(*d),
        Val::Str(t) => pv::Value::String(t.clone()),
        Val::V3(v3) => pv::Value::Vector3(c_v3(v3)),
        Val::B32(b) => pv::Value::Bitfield32(*b),
        Val::B64(b) => pv::Value::Bitfield64(*b),
        Val::Ent(e) => pv::Value::EntityRef(*e),
        Val::Time(t) => pv::Value::Time(*t),
        Val::Rgba(c) => pv::Value::Colour(*c),
        Val::Arr(items) => pv::Value::Array(items.iter().map(c_val).collect()),
        Val::Struct(f) => pv::Value::Struct(f.iter().map(|(id, x)| (*id, c_val(x))).collect()),
    }
}

fn c_props(p: &[(u16, Val)]) -> Vec<pv::Property> {
    p.iter()
        .map(|(id, x)| pv::Property::new(*id, c_val(x)))
        .collect()
}

fn c_chat(channel: u8, peer: Option<&String>) -> pm::ChatChannel {
    match channel {
        1 => pm::ChatChannel::Say,
        2 => pm::ChatChannel::Yell,
        3 => pm::ChatChannel::Emote,
        4 => pm::ChatChannel::Tell {
            peer: peer.expect("Tell has a peer").clone(),
        },
        5 => pm::ChatChannel::System,
        other => panic!("channel {other} has no crate value"),
    }
}

/// The same message as a `tessera_protocol` value (positive vectors only).
fn to_crate(m: &Msg) -> Message {
    match m {
        Msg::Hello {
            minor_max,
            caps,
            max_entities,
            client_name,
        } => Message::Hello(pm::Hello {
            minor_max: *minor_max,
            client_caps: *caps,
            max_entities: *max_entities,
            client_name: client_name.clone(),
        }),
        Msg::HelloAck {
            minor,
            caps,
            tick,
            snapshot,
            radius,
            name,
            motd,
        } => Message::HelloAck(pm::HelloAck {
            minor: *minor,
            server_caps: pm::ServerCaps(*caps),
            tick_rate: *tick,
            snapshot_rate: *snapshot,
            interest_radius: *radius,
            server_name: name.clone(),
            motd: motd.clone(),
        }),
        Msg::Login(acc, pw) => Message::Login(pm::Credentials {
            account: acc.clone(),
            password: pw.clone(),
        }),
        Msg::Register(acc, pw) => Message::Register(pm::Credentials {
            account: acc.clone(),
            password: pw.clone(),
        }),
        Msg::LoginOk(acc, list) => Message::LoginOk(pm::LoginOk {
            account: acc.clone(),
            characters: c_chars(list),
        }),
        Msg::LoginFailed(code, retry, text) => Message::LoginFailed(pm::LoginFailed {
            code: LoginFailedCode::from_u16(*code),
            retry_after: *retry,
            text: text.clone(),
        }),
        Msg::CharacterCreate(n, x) => Message::CharacterCreate(pm::CharacterCreate {
            name: n.clone(),
            appearance: c_app(x),
        }),
        Msg::CharacterList(list) => Message::CharacterList(pm::CharacterList {
            characters: c_chars(list),
        }),
        Msg::CharacterCreateFailed(code, text) => Message::CharacterCreateFailed(pm::Failure {
            code: CharacterCreateFailedCode::from_u16(*code),
            text: text.clone(),
        }),
        Msg::EnterWorld(n) => Message::EnterWorld(pm::EnterWorld { name: n.clone() }),
        Msg::EnterWorldFailed(code, text) => Message::EnterWorldFailed(pm::Failure {
            code: EnterWorldFailedCode::from_u16(*code),
            text: text.clone(),
        }),
        Msg::TeleportRequest(id, d) => Message::TeleportRequest(pm::TeleportRequest {
            request_id: *id,
            destination: match d {
                Dest::Location(x, h) => pt::Destination::Location {
                    location: c_loc(x),
                    heading: *h,
                },
                Dest::Ground(x, h) => pt::Destination::Ground {
                    location: c_loc(x),
                    heading: *h,
                },
                Dest::Anchored(x, h, an) => pt::Destination::Anchored {
                    location: c_loc(x),
                    heading: *h,
                    anchor: c_anc(an),
                },
                Dest::Named(n) => pt::Destination::Named { name: n.clone() },
            },
        }),
        Msg::QueryLocation(id) => Message::QueryLocation { request_id: *id },
        Msg::ChatSend {
            channel,
            peer,
            text,
        } => Message::ChatSend(pm::ChatSend {
            channel: c_chat(*channel, peer.as_ref()),
            text: text.clone(),
        }),
        Msg::EmoteSet(anim, looping) => Message::EmoteSet(pm::EmoteSet {
            anim: *anim,
            looping: *looping,
        }),
        Msg::StanceSet(st) => Message::StanceSet(pv::Stance::from_u8(*st).expect("valid stance")),
        Msg::AppearanceSet(x) => Message::AppearanceSet(c_app(x)),
        Msg::Ping(n) => Message::Ping { nonce: *n },
        Msg::Pong(n) => Message::Pong { nonce: *n },
        Msg::Disconnect(code, text) => Message::Disconnect(pm::Disconnect {
            code: AppErrorCode::from_u16(*code),
            text: text.clone(),
        }),
        Msg::WorldEnter {
            player,
            name,
            instance,
            epoch,
            time,
            pose,
            app,
        } => Message::WorldEnter(pm::WorldEnter {
            player: *player,
            name: name.clone(),
            instance: *instance,
            epoch: *epoch,
            server_time: *time,
            pose: c_pose(pose),
            appearance: c_app(app),
        }),
        Msg::LocationReport {
            cause,
            request,
            epoch,
            time,
            pose,
        } => Message::LocationReport(pm::LocationReport {
            cause: match cause {
                1 => pm::LocationCause::Teleported,
                2 => pm::LocationCause::Requested,
                _ => pm::LocationCause::Corrected,
            },
            request_id: *request,
            epoch: *epoch,
            server_time: *time,
            pose: c_pose(pose),
        }),
        Msg::TeleportRefused(id, code, text) => Message::TeleportRefused(pm::TeleportRefused {
            request_id: *id,
            code: TeleportRefusedCode::from_u16(*code),
            text: text.clone(),
        }),
        Msg::EntityCreate {
            id,
            kind,
            name,
            class,
            time,
            pose,
            vel,
            motion,
            app,
            props,
        } => Message::EntityCreate(pm::EntityCreate {
            id: *id,
            kind: match kind {
                0 => pm::EntityKind::Player,
                1 => pm::EntityKind::NonPlayer,
                2 => pm::EntityKind::Creature,
                _ => pm::EntityKind::Object,
            },
            name: name.clone(),
            class: *class,
            server_time: *time,
            pose: c_pose(pose),
            velocity: c_v3(vel),
            motion: c_motion(*motion),
            appearance: c_app(app),
            properties: c_props(props),
        }),
        Msg::EntityUpdate(id, props) => Message::EntityUpdate(pm::EntityUpdate {
            id: *id,
            changes: c_props(props),
        }),
        Msg::EntityAppearance(id, x) => Message::EntityAppearance(pm::EntityAppearance {
            id: *id,
            appearance: c_app(x),
        }),
        Msg::EntityDestroy(id, reason) => Message::EntityDestroy(pm::EntityDestroy {
            id: *id,
            reason: if *reason == 0 {
                pm::DestroyReason::LeftInterest
            } else {
                pm::DestroyReason::Removed
            },
        }),
        Msg::InterestReset => Message::InterestReset,
        Msg::WorldClock(t, g, r) => Message::WorldClock(pm::WorldClock {
            server_time: *t,
            game_clock: *g,
            rate: *r,
        }),
        Msg::ChatMessage {
            time,
            channel,
            flags,
            sender_entity,
            sender,
            peer,
            text,
        } => Message::ChatMessage(pm::ChatMessage {
            server_time: *time,
            channel: c_chat(*channel, peer.as_ref()),
            echo: flags & 1 != 0,
            sender_entity: *sender_entity,
            sender: sender.clone(),
            text: text.clone(),
        }),
        Msg::ServerNotice(level, seconds, text) => Message::ServerNotice(pm::ServerNotice {
            level: match level {
                0 => pm::NoticeLevel::Info,
                1 => pm::NoticeLevel::Warning,
                _ => pm::NoticeLevel::Shutdown,
            },
            seconds: *seconds,
            text: text.clone(),
        }),
        Msg::ChatRejected(code, text) => Message::ChatRejected(pm::Failure {
            code: ChatRejectedCode::from_u16(*code),
            text: text.clone(),
        }),
        Msg::MoveInput {
            seq,
            epoch,
            loc,
            heading,
            vel,
            motion,
        } => Message::MoveInput(pm::MoveInput {
            seq: *seq,
            epoch: *epoch,
            location: c_loc(loc),
            heading: *heading,
            velocity: c_v3(vel),
            motion: c_motion(*motion),
        }),
        Msg::MotionBatch(t, entries) => Message::MotionBatch(pm::MotionBatch {
            server_time: *t,
            entries: entries
                .iter()
                .map(|e| pm::MotionEntry {
                    id: e.id,
                    position: c_v3(&e.pos),
                    heading: e.heading,
                    velocity: c_v3(&e.vel),
                    motion: c_motion(e.motion),
                })
                .collect(),
        }),
        Msg::TimeRequest(t0) => Message::TimeRequest { client_time: *t0 },
        Msg::TimeReply(t0, t1, t2, tick) => Message::TimeReply(pm::TimeReply {
            client_time: *t0,
            server_receive: *t1,
            server_send: *t2,
            tick: *tick,
        }),
    }
}

// ---------------------------------------------------------------------- §16 vectors

/// One §16.2–§16.5 vector: channel, sender, value, bytes, size stated in the spec.
struct Vector {
    name: &'static str,
    chan: Chan,
    from: Sender,
    msg: Msg,
    bytes: &'static str,
    size: usize,
}

#[allow(clippy::too_many_lines)]
fn vectors() -> Vec<Vector> {
    use Chan::{Chat, Control, Datagram, World};
    use Sender::{Client, Server};
    let ground = Loc {
        region: 1,
        x: 20_000.0,
        y: 22_000.0,
        z: 0.0,
    };
    let bram_pose = Pose {
        loc: Loc {
            region: 1,
            x: 20_260.0,
            y: 22_500.0,
            z: 400.5,
        },
        heading: 3.0,
        anchor: None,
    };
    vec![
        // §16.2
        Vector {
            name: "Hello",
            chan: Control,
            from: Client,
            msg: Msg::Hello {
                minor_max: 0,
                caps: 0,
                max_entities: 1024,
                client_name: s("tessera-client 0.7.0"),
            },
            bytes: "20 00 00 00 01 00 00 00 00 00 00 00 00 04 14 00 74 65 73 73 65 72 61 2d 63 6c 69 65
                    6e 74 20 30 2e 37 2e 30",
            size: 36,
        },
        Vector {
            name: "HelloAck",
            chan: Control,
            from: Server,
            msg: Msg::HelloAck {
                minor: 0,
                caps: 3,
                tick: 30,
                snapshot: 15,
                radius: 2,
                name: s("Test Shard"),
                motd: s(""),
            },
            bytes: "19 00 00 00 02 00 00 00 03 00 00 00 1e 0f 02 0a 00 54 65 73 74 20 53 68 61 72 64 00
                    00",
            size: 29,
        },
        Vector {
            name: "Login",
            chan: Control,
            from: Client,
            msg: Msg::Login(s("alice"), s("correct horse")),
            bytes: "18 00 00 00 03 00 05 00 61 6c 69 63 65 0d 00 63 6f 72 72 65 63 74 20 68 6f 72 73 65",
            size: 28,
        },
        Vector {
            name: "Register",
            chan: Control,
            from: Client,
            msg: Msg::Register(s("bob"), s("battery staple")),
            bytes: "17 00 00 00 04 00 03 00 62 6f 62 0e 00 62 61 74 74 65 72 79 20 73 74 61 70 6c 65",
            size: 27,
        },
        Vector {
            name: "LoginOk",
            chan: Control,
            from: Server,
            msg: Msg::LoginOk(s("alice"), vec![(s("Aldith"), Body::Character(23, 1))]),
            bytes: "1b 00 00 00 05 00 05 00 61 6c 69 63 65 01 06 00 41 6c 64 69 74 68 01 17 00 00 00 01
                    00 00 00",
            size: 31,
        },
        Vector {
            name: "LoginFailed",
            chan: Control,
            from: Server,
            msg: Msg::LoginFailed(1, 0, s("")),
            bytes: "08 00 00 00 06 00 01 00 00 00 00 00",
            size: 12,
        },
        Vector {
            name: "CharacterCreate",
            chan: Control,
            from: Client,
            msg: Msg::CharacterCreate(s("Aldith"), plain(Body::Character(23, 1))),
            bytes: "19 00 00 00 07 00 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 00 00 80 3f 00
                    00",
            size: 29,
        },
        Vector {
            name: "CharacterList",
            chan: Control,
            from: Server,
            msg: Msg::CharacterList(vec![
                (s("Aldith"), Body::Character(23, 1)),
                (s("Bram"), Body::Character(81, 2)),
            ]),
            bytes: "23 00 00 00 08 00 02 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 04 00 42 72
                    61 6d 01 51 00 00 00 02 00 00 00",
            size: 39,
        },
        Vector {
            name: "CharacterCreateFailed",
            chan: Control,
            from: Server,
            msg: Msg::CharacterCreateFailed(1, s("")),
            bytes: "06 00 00 00 09 00 01 00 00 00",
            size: 10,
        },
        Vector {
            name: "EnterWorld",
            chan: Control,
            from: Client,
            msg: Msg::EnterWorld(s("Aldith")),
            bytes: "0a 00 00 00 0a 00 06 00 41 6c 64 69 74 68",
            size: 14,
        },
        Vector {
            name: "EnterWorldFailed",
            chan: Control,
            from: Server,
            msg: Msg::EnterWorldFailed(3, s("")),
            bytes: "06 00 00 00 0b 00 03 00 00 00",
            size: 10,
        },
        Vector {
            name: "TeleportRequest Ground",
            chan: Control,
            from: Client,
            msg: Msg::TeleportRequest(7, Dest::Ground(ground, 0.0)),
            bytes: "25 00 00 00 0c 00 07 00 00 00 02 01 00 00 00 00 00 00 88 d3 40 00 00 00 00 00 7c d5
                    40 00 00 00 00 00 00 00 00 00 00 00 00",
            size: 41,
        },
        Vector {
            name: "TeleportRequest Anchored",
            chan: Control,
            from: Client,
            msg: Msg::TeleportRequest(8, Dest::Anchored(l(), 1.5, anc())),
            bytes: "29 00 00 00 0c 00 08 00 00 00 03 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5
                    40 00 00 00 00 00 10 79 40 00 00 c0 3f 7e 8c 01 01",
            size: 45,
        },
        Vector {
            name: "TeleportRequest Named",
            chan: Control,
            from: Client,
            msg: Msg::TeleportRequest(9, Dest::Named(s("origin"))),
            bytes: "0f 00 00 00 0c 00 09 00 00 00 04 06 00 6f 72 69 67 69 6e",
            size: 19,
        },
        Vector {
            name: "QueryLocation",
            chan: Control,
            from: Client,
            msg: Msg::QueryLocation(10),
            bytes: "06 00 00 00 0d 00 0a 00 00 00",
            size: 10,
        },
        Vector {
            name: "ChatSend Say",
            chan: Control,
            from: Client,
            msg: Msg::ChatSend {
                channel: 1,
                peer: None,
                text: s("hello"),
            },
            bytes: "0a 00 00 00 0e 00 01 05 00 68 65 6c 6c 6f",
            size: 14,
        },
        Vector {
            name: "ChatSend Tell",
            chan: Control,
            from: Client,
            msg: Msg::ChatSend {
                channel: 4,
                peer: Some(s("Bram")),
                text: s("hi"),
            },
            bytes: "0d 00 00 00 0e 00 04 04 00 42 72 61 6d 02 00 68 69",
            size: 17,
        },
        Vector {
            name: "EmoteSet",
            chan: Control,
            from: Client,
            msg: Msg::EmoteSet(0x1000_0123, false),
            bytes: "07 00 00 00 0f 00 23 01 00 10 00",
            size: 11,
        },
        Vector {
            name: "StanceSet",
            chan: Control,
            from: Client,
            msg: Msg::StanceSet(1),
            bytes: "03 00 00 00 10 00 01",
            size: 7,
        },
        Vector {
            name: "AppearanceSet",
            chan: Control,
            from: Client,
            msg: Msg::AppearanceSet(a()),
            bytes: "1f 00 00 00 11 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02 34 12 00 70 01 00 00
                    80 3e 78 56 00 70 04",
            size: 35,
        },
        Vector {
            name: "Ping",
            chan: Control,
            from: Client,
            msg: Msg::Ping(0x0123_4567_89ab_cdef),
            bytes: "0a 00 00 00 12 00 ef cd ab 89 67 45 23 01",
            size: 14,
        },
        Vector {
            name: "Pong",
            chan: Control,
            from: Server,
            msg: Msg::Pong(0x0123_4567_89ab_cdef),
            bytes: "0a 00 00 00 13 00 ef cd ab 89 67 45 23 01",
            size: 14,
        },
        Vector {
            name: "Disconnect",
            chan: Control,
            from: Client,
            msg: Msg::Disconnect(0, s("")),
            bytes: "06 00 00 00 14 00 00 00 00 00",
            size: 10,
        },
        // §16.3
        Vector {
            name: "WorldEnter",
            chan: World,
            from: Server,
            msg: Msg::WorldEnter {
                player: 42,
                name: s("Aldith"),
                instance: 1,
                epoch: 1,
                time: 5_000_000,
                pose: Pose {
                    loc: l(),
                    heading: 0.0,
                    anchor: None,
                },
                app: a(),
            },
            bytes: "5e 00 00 00 01 01 2a 00 00 00 00 00 00 00 06 00 41 6c 64 69 74 68 01 00 00 00 01 00
                    00 00 40 4b 4c 00 00 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
                    00 00 00 00 00 10 79 40 00 00 00 00 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02
                    34 12 00 70 01 00 00 80 3e 78 56 00 70 04",
            size: 98,
        },
        Vector {
            name: "LocationReport",
            chan: World,
            from: Server,
            msg: Msg::LocationReport {
                cause: 1,
                request: 7,
                epoch: 2,
                time: 6_000_000,
                pose: Pose {
                    loc: l(),
                    heading: 1.5,
                    anchor: Some(anc()),
                },
            },
            bytes: "36 00 00 00 02 01 01 07 00 00 00 02 00 00 00 80 8d 5b 00 00 00 00 00 01 00 00 00 00
                    00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40 00 00 00 00 00 10 79 40 00 00 c0 3f 01 7e 8c
                    01 01",
            size: 58,
        },
        Vector {
            name: "TeleportRefused",
            chan: World,
            from: Server,
            msg: Msg::TeleportRefused(9, 1, s("")),
            bytes: "0a 00 00 00 03 01 09 00 00 00 01 00 00 00",
            size: 14,
        },
        Vector {
            name: "EntityCreate",
            chan: World,
            from: Server,
            msg: Msg::EntityCreate {
                id: 43,
                kind: 0,
                name: s("Bram"),
                class: None,
                time: 6_000_000,
                pose: bram_pose,
                vel: [2.0, 0.0, 0.0],
                motion: 1,
                app: plain(Body::Character(81, 2)),
                props: vec![(1, Val::Enum(0)), (4, Val::F32(6.0))],
            },
            bytes: "65 00 00 00 04 01 2b 00 00 00 00 00 00 00 00 04 00 42 72 61 6d 00 80 8d 5b 00 00 00
                    00 00 01 00 00 00 00 00 00 c9 d3 40 00 00 00 00 00 f9 d5 40 00 00 00 00 00 08 79 40
                    00 00 40 40 00 00 00 00 40 00 00 00 00 00 00 00 00 01 01 51 00 00 00 02 00 00 00 00
                    00 80 3f 00 00 02 00 01 00 05 00 00 00 00 04 00 04 00 00 c0 40",
            size: 105,
        },
        Vector {
            name: "EntityUpdate",
            chan: World,
            from: Server,
            msg: Msg::EntityUpdate(
                43,
                vec![
                    (1, Val::Enum(2)),
                    (
                        2,
                        Val::Struct(vec![
                            (1, Val::Enum(0x1000_0123)),
                            (2, Val::Time(6_500_000)),
                            (3, Val::Bool(false)),
                        ]),
                    ),
                ],
            ),
            bytes: "2e 00 00 00 05 01 2b 00 00 00 00 00 00 00 02 00 01 00 05 02 00 00 00 02 00 0f 03 00
                    01 00 05 23 01 00 10 02 00 0c a0 2e 63 00 00 00 00 00 03 00 01 00",
            size: 50,
        },
        Vector {
            name: "EntityUpdate removal",
            chan: World,
            from: Server,
            msg: Msg::EntityUpdate(43, vec![(2, Val::None)]),
            bytes: "0f 00 00 00 05 01 2b 00 00 00 00 00 00 00 01 00 02 00 00",
            size: 19,
        },
        Vector {
            name: "EntityAppearance",
            chan: World,
            from: Server,
            msg: Msg::EntityAppearance(
                43,
                App {
                    body: Body::Object(0x7900_0001),
                    scale: 2.5,
                    idle: Some(0x1000_0001),
                    items: Vec::new(),
                },
            ),
            bytes: "19 00 00 00 06 01 2b 00 00 00 00 00 00 00 02 01 00 00 79 00 00 20 40 01 01 00 00 10
                    00",
            size: 29,
        },
        Vector {
            name: "EntityDestroy",
            chan: World,
            from: Server,
            msg: Msg::EntityDestroy(43, 0),
            bytes: "0b 00 00 00 07 01 2b 00 00 00 00 00 00 00 00",
            size: 15,
        },
        Vector {
            name: "InterestReset",
            chan: World,
            from: Server,
            msg: Msg::InterestReset,
            bytes: "02 00 00 00 08 01",
            size: 6,
        },
        Vector {
            name: "WorldClock",
            chan: World,
            from: Server,
            msg: Msg::WorldClock(5_000_000, 43_200.0, 1.0),
            bytes: "16 00 00 00 09 01 40 4b 4c 00 00 00 00 00 00 00 00 00 00 18 e5 40 00 00 80 3f",
            size: 26,
        },
        // §16.4
        Vector {
            name: "ChatMessage Say",
            chan: Chat,
            from: Server,
            msg: Msg::ChatMessage {
                time: 7_000_000,
                channel: 1,
                flags: 0,
                sender_entity: 43,
                sender: s("Bram"),
                peer: None,
                text: s("hello"),
            },
            bytes: "21 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 01 00 2b 00 00 00 00 00 00 00 04 00 42 72
                    61 6d 05 00 68 65 6c 6c 6f",
            size: 37,
        },
        Vector {
            name: "ChatMessage Tell echo",
            chan: Chat,
            from: Server,
            msg: Msg::ChatMessage {
                time: 7_000_000,
                channel: 4,
                flags: 1,
                sender_entity: 42,
                sender: s("Aldith"),
                peer: Some(s("Bram")),
                text: s("hi"),
            },
            bytes: "26 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 04 01 2a 00 00 00 00 00 00 00 06 00 41 6c
                    64 69 74 68 04 00 42 72 61 6d 02 00 68 69",
            size: 42,
        },
        Vector {
            name: "ChatMessage System",
            chan: Chat,
            from: Server,
            msg: Msg::ChatMessage {
                time: 7_000_000,
                channel: 5,
                flags: 0,
                sender_entity: 0,
                sender: s(""),
                peer: None,
                text: s("Welcome"),
            },
            bytes: "1f 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 05 00 00 00 00 00 00 00 00 00 00 00 07 00
                    57 65 6c 63 6f 6d 65",
            size: 35,
        },
        Vector {
            name: "ServerNotice",
            chan: Chat,
            from: Server,
            msg: Msg::ServerNotice(2, 60, s("restart")),
            bytes: "0e 00 00 00 02 02 02 3c 00 07 00 72 65 73 74 61 72 74",
            size: 18,
        },
        Vector {
            name: "ChatRejected",
            chan: Chat,
            from: Server,
            msg: Msg::ChatRejected(1, s("")),
            bytes: "06 00 00 00 03 02 01 00 00 00",
            size: 10,
        },
        // §16.5
        Vector {
            name: "MoveInput",
            chan: Datagram,
            from: Client,
            msg: Msg::MoveInput {
                seq: 100,
                epoch: 2,
                loc: l(),
                heading: 1.5,
                vel: [0.0, 6.0, 0.0],
                motion: 2,
            },
            bytes: "01 03 64 00 00 00 02 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
                    00 00 00 00 00 10 79 40 00 00 c0 3f 00 00 00 00 00 00 c0 40 00 00 00 00 02",
            size: 53,
        },
        Vector {
            name: "MotionBatch",
            chan: Datagram,
            from: Server,
            msg: Msg::MotionBatch(
                6_066_666,
                vec![Entry {
                    id: 43,
                    pos: [20_260.0, 22_500.0, 400.5],
                    heading: 3.0,
                    vel: [-2.0, 0.0, 0.0],
                    motion: 1,
                }],
            ),
            bytes: "02 03 ea 91 5c 00 00 00 00 00 01 2b 00 00 00 00 00 00 00 00 48 9e 46 00 c8 af 46 00
                    40 c8 43 00 00 40 40 00 00 00 c0 00 00 00 00 00 00 00 00 01",
            size: 48,
        },
        Vector {
            name: "TimeRequest",
            chan: Datagram,
            from: Client,
            msg: Msg::TimeRequest(1_000_000),
            bytes: "03 03 40 42 0f 00 00 00 00 00",
            size: 10,
        },
        Vector {
            name: "TimeReply",
            chan: Datagram,
            from: Server,
            msg: Msg::TimeReply(1_000_000, 5_020_000, 5_020_100, 150),
            bytes: "04 03 40 42 0f 00 00 00 00 00 60 99 4c 00 00 00 00 00 c4 99 4c 00 00 00 00 00 96 00
                    00 00 00 00 00 00",
            size: 34,
        },
    ]
}

/// Every message type of §6.1 has at least one vector.
#[test]
fn vectors_cover_every_message_type() {
    let mut types: Vec<u16> = vectors().iter().map(|v| v.msg.ty()).collect();
    types.sort_unstable();
    types.dedup();
    assert_eq!(types.len(), 36, "§6.1 lists 36 message types");
}

/// The independent codec reproduces §16.2–§16.5 byte for byte and reads them back.
#[test]
fn independent_codec_matches_spec_vectors() {
    for vec in vectors() {
        let want = hex(vec.bytes);
        assert_eq!(want.len(), vec.size, "{}: stated size", vec.name);
        let got = if vec.chan == Chan::Datagram {
            spec::datagram(&vec.msg)
        } else {
            spec::frame(&vec.msg)
        };
        assert_eq!(got, want, "{}: independent encoder", vec.name);
        let decoded = if vec.chan == Chan::Datagram {
            spec::decode_datagram(vec.from, &want).expect(vec.name)
        } else {
            let (mut msgs, partial) =
                spec::decode_stream(vec.chan, vec.from, &want).expect(vec.name);
            assert!(!partial && msgs.len() == 1, "{}: one whole frame", vec.name);
            msgs.remove(0)
        };
        assert_eq!(decoded, vec.msg, "{}: independent decoder", vec.name);
    }
}

/// `tessera_protocol` encodes the same values to the same bytes and decodes the vectors
/// back to the same values (frames through the incremental `FrameDecoder`, one byte at a
/// time, so reassembly is exercised too).
#[test]
fn crate_codec_matches_spec_vectors() {
    for vec in vectors() {
        let want = hex(vec.bytes);
        let value = to_crate(&vec.msg);
        if vec.chan == Chan::Datagram {
            let got = encode_datagram(&value).expect(vec.name);
            assert_eq!(got, want, "{}: crate datagram encoder", vec.name);
            let back = decode_datagram(side_of(vec.from), &want).expect(vec.name);
            assert_eq!(back, value, "{}: crate datagram decoder", vec.name);
        } else {
            let got = frame_to_vec(&value).expect(vec.name);
            assert_eq!(got, want, "{}: crate frame encoder", vec.name);
            let mut dec = FrameDecoder::new(chan_of(vec.chan), side_of(vec.from));
            let mut out = Vec::new();
            for byte in &want {
                let mut one: &[u8] = std::slice::from_ref(byte);
                while !one.is_empty() {
                    if let Some(m) = dec.push(&mut one).expect(vec.name) {
                        out.push(m);
                    }
                }
            }
            assert_eq!(out, vec![value], "{}: crate frame decoder", vec.name);
        }
    }
}

/// Several frames back to back in one buffer decode the same way in both codecs.
#[test]
fn concatenated_frames_agree() {
    let world_vectors: Vec<Vector> = vectors()
        .into_iter()
        .filter(|v| v.chan == Chan::World)
        .collect();
    let mut stream = Vec::new();
    for v in &world_vectors {
        stream.extend_from_slice(&hex(v.bytes));
    }
    let (mine, partial) = spec::decode_stream(Chan::World, Sender::Server, &stream).expect("ok");
    assert!(!partial);
    let mut dec = FrameDecoder::new(Channel::World, Side::Server);
    let mut input: &[u8] = &stream;
    let mut theirs = Vec::new();
    while !input.is_empty() {
        if let Some(m) = dec.push(&mut input).expect("ok") {
            theirs.push(m);
        }
    }
    assert_eq!(mine.len(), theirs.len());
    for (m, t) in mine.iter().zip(&theirs) {
        assert_eq!(&to_crate(m), t);
    }
}

/// §16.6 tagged values.
#[test]
fn tagged_values_match() {
    let cases: Vec<(Val, &str)> = vec![
        (Val::Bool(true), "01 01"),
        (Val::I32(-2), "02 fe ff ff ff"),
        (Val::I64(-2), "03 fe ff ff ff ff ff ff ff"),
        (Val::F32(6.0), "04 00 00 c0 40"),
        (Val::Enum(2), "05 02 00 00 00"),
        (Val::Did(0x7000_1234), "06 34 12 00 70"),
        (Val::Str(s("ab")), "07 02 00 61 62"),
        (
            Val::V3([1.0, 2.0, 3.0]),
            "08 00 00 80 3f 00 00 00 40 00 00 40 40",
        ),
        (Val::B32(0x8000_0001), "09 01 00 00 80"),
        (Val::B64(1 << 40), "0a 00 00 00 00 00 01 00 00"),
        (Val::Ent(43), "0b 2b 00 00 00 00 00 00 00"),
        (Val::Time(6_500_000), "0c a0 2e 63 00 00 00 00 00"),
        (Val::Rgba([255, 128, 0, 255]), "0d ff 80 00 ff"),
        (
            Val::Arr(vec![Val::I32(1), Val::I32(2)]),
            "0e 02 00 02 01 00 00 00 02 02 00 00 00",
        ),
        (
            Val::Struct(vec![(1, Val::Bool(false))]),
            "0f 01 00 01 00 01 00",
        ),
        (Val::None, "00"),
    ];
    for (x, h) in cases {
        let want = hex(h);
        let allow_none = x == Val::None;
        assert_eq!(spec::value_bytes(&x), want, "{x:?}: independent encoder");
        assert_eq!(
            spec::decode_value(&want, allow_none),
            Ok((x.clone(), want.len())),
            "{x:?}: independent decoder"
        );
        let theirs = c_val(&x);
        let mut got = Vec::new();
        theirs.encode(&mut got, allow_none).expect("encode");
        assert_eq!(got, want, "{x:?}: crate encoder");
        let (back, used) = pv::Value::decode(&want, allow_none).expect("decode");
        assert_eq!((back, used), (theirs, want.len()), "{x:?}: crate decoder");
        if allow_none {
            assert!(spec::decode_value(&want, false).is_err());
            assert!(pv::Value::decode(&want, false).is_err());
        }
    }
}

// ---------------------------------------------------------------------- §16.1, §11.2

/// RFC 8032 §7.1 TEST 1 public key, as used in §16.1.
const TEST1_KEY: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

const TEST1_FINGERPRINT: &str =
    "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";

#[test]
fn handshake_vectors() {
    // SubjectPublicKeyInfo DER, 44 bytes.
    let spki = spec::ed25519_spki(&TEST1_KEY);
    assert_eq!(
        spki,
        hex(
            "30 2a 30 05 06 03 2b 65 70 03 21 00 d7 5a 98 01 82 b1 0a b7 d5 4b fe d3 c9 64 07 3a
             0e e1 72 f3 da a6 23 25 af 02 1a 68 f7 07 51 1a"
        )
    );
    assert_eq!(
        spki.as_slice(),
        tessera_protocol::fingerprint::ed25519_spki(&TEST1_KEY).as_slice()
    );

    // Fingerprint text: "sha256:" + 64 lowercase hex digits of SHA-256(SPKI).
    let digest = Sha256::digest(&spki);
    let mut text = String::from("sha256:");
    for b in digest {
        text.push_str(&format!("{b:02x}"));
    }
    assert_eq!(text, TEST1_FINGERPRINT);
    assert_eq!(text.len(), 71);
    assert_eq!(Fingerprint::of_spki(&spki).to_string(), TEST1_FINGERPRINT);
    assert_eq!(
        Fingerprint::of_ed25519_key(&TEST1_KEY).to_string(),
        TEST1_FINGERPRINT
    );
    assert_eq!(
        Fingerprint::parse(TEST1_FINGERPRINT).expect("parse").0,
        <[u8; 32]>::from(digest)
    );

    // ALPN.
    assert_eq!(hex("74 65 73 73 65 72 61 2f 31"), b"tessera/1");
    assert_eq!(tessera_protocol::limits::ALPN_V1, b"tessera/1");
    assert_eq!(tessera_protocol::version::alpn_for_major(1), b"tessera/1");

    // Connect string with pin.
    let cs = format!("tessera://192.0.2.10:47000?pin={TEST1_FINGERPRINT}");
    let parsed = ConnectString::parse(&cs).expect("connect string");
    assert_eq!(parsed.server.port, 47000);
    assert_eq!(parsed.server.to_string(), "192.0.2.10:47000");
    assert_eq!(
        parsed.pin.map(|p| p.0),
        spec::parse_fingerprint(TEST1_FINGERPRINT)
    );
    assert_eq!(parsed.to_string(), cs);
}

/// §4.2 parsing rules, checked against both parsers.
#[test]
fn fingerprint_text_rules_agree() {
    let upper = format!("sha256:{}", TEST1_FINGERPRINT[7..].to_ascii_uppercase());
    let accept = [TEST1_FINGERPRINT.to_owned(), upper];
    for t in &accept {
        assert!(spec::parse_fingerprint(t).is_some(), "{t}");
        assert!(Fingerprint::parse(t).is_ok(), "{t}");
    }
    let grouped: String = TEST1_FINGERPRINT.as_bytes()[7..]
        .chunks(2)
        .map(|c| std::str::from_utf8(c).expect("ascii"))
        .collect::<Vec<_>>()
        .join(":");
    let reject = [
        format!("SHA256:{}", &TEST1_FINGERPRINT[7..]),
        format!("sha-256:{}", &TEST1_FINGERPRINT[7..]),
        TEST1_FINGERPRINT[..70].to_owned(),
        format!("{TEST1_FINGERPRINT}0"),
        format!(" {TEST1_FINGERPRINT}"),
        format!("{TEST1_FINGERPRINT} "),
        format!("sha256:{grouped}"),
        format!("sha256:{}g", &TEST1_FINGERPRINT[7..70]),
        TEST1_FINGERPRINT[7..].to_owned(),
        String::new(),
    ];
    for t in &reject {
        assert!(spec::parse_fingerprint(t).is_none(), "{t:?}");
        assert!(Fingerprint::parse(t).is_err(), "{t:?}");
    }
}

/// §4.4 connect strings the spec makes invalid, and valid forms.
#[test]
fn connect_string_rules() {
    let pin = TEST1_FINGERPRINT;
    let valid = [
        "tessera://192.0.2.10:47000".to_owned(),
        "tessera://play.example.net:1".to_owned(),
        "tessera://[2001:db8::1]:65535".to_owned(),
        format!("tessera://play.example.net:47000?pin={pin}"),
    ];
    for c in &valid {
        assert!(ConnectString::parse(c).is_ok(), "{c}");
    }
    assert_eq!(
        ConnectString::parse("tessera://PLAY.Example.NET:47000")
            .expect("ok")
            .server
            .to_string(),
        "play.example.net:47000",
        "§4.4: DNS names compare case-insensitively and are stored lowercase"
    );
    assert_eq!(
        ConnectString::parse("tessera://[2001:db8::1]:47000")
            .expect("ok")
            .server
            .to_string(),
        "[2001:db8::1]:47000",
        "§4.4: IPv6 keeps its brackets in the server key"
    );
    let invalid = [
        "tessera://192.0.2.10".to_owned(),                // port required
        "tessera://192.0.2.10:0".to_owned(),              // port 1-65535
        "tessera://192.0.2.10:65536".to_owned(),          // port 1-65535
        "TESSERA://192.0.2.10:47000".to_owned(),          // scheme lowercase
        "http://192.0.2.10:47000".to_owned(),             // scheme
        "tessera://192.0.2.10:47000/".to_owned(),         // path other than empty
        "tessera://192.0.2.10:47000/x".to_owned(),        // path
        "tessera://user@192.0.2.10:47000".to_owned(),     // user information
        "tessera://192.0.2.10:47000#frag".to_owned(),     // fragment
        "tessera://192.0.2.10:47000?foo=bar".to_owned(),  // unknown parameter
        format!("tessera://192.0.2.10:47000?pin= {pin}"), // whitespace in the pin
        format!("tessera://192.0.2.10:47000?pin={pin} "), // trailing whitespace
        format!("tessera://192.0.2.10:47000?pin={}", &pin[7..]), // missing prefix
        "tessera://2001:db8::1:47000".to_owned(),         // IPv6 without brackets
        "tessera://:47000".to_owned(),                    // no host
    ];
    for c in &invalid {
        assert!(ConnectString::parse(c).is_err(), "{c:?} must be invalid");
    }
}

/// §16.5 worked time-sync example.
#[test]
fn time_sync_worked_example() {
    let (t0, t1, t2, t3) = (1_000_000, 5_020_000, 5_020_100, 1_040_100);
    assert_eq!(spec::ntp(t0, t1, t2, t3), (4_000_000, 40_000));
    let sample = TimeSample::compute(t0, t1, t2, t3);
    assert_eq!((sample.offset, sample.delay), (4_000_000, 40_000));
    // §11.1: tick k begins at floor(k * 10^6 / tick_rate); 150 ticks at 30 Hz = 5 s.
    assert_eq!(tessera_protocol::time::tick_start(150, 30), 5_000_000);
    assert_eq!(tessera_protocol::time::tick_at(5_020_100, 30), 150);
}

// ---------------------------------------------------------------------- malformed inputs

/// Where a malformed input arrives.
#[derive(Clone, Copy)]
enum Ctx {
    /// Bytes of a stream after its kind byte.
    Stream(Chan, Sender),
    /// One datagram.
    Dgram(Sender),
    /// The kind byte of a server-initiated unidirectional stream.
    Kind,
}

fn mine_rejects(ctx: Ctx, bytes: &[u8]) -> bool {
    match ctx {
        Ctx::Stream(c, f) => spec::decode_stream(c, f, bytes).is_err(),
        Ctx::Dgram(f) => spec::decode_datagram(f, bytes).is_err(),
        Ctx::Kind => spec::stream_kind(bytes[0]).is_err(),
    }
}

fn crate_rejects(ctx: Ctx, bytes: &[u8]) -> bool {
    match ctx {
        Ctx::Stream(c, f) => {
            let mut dec = FrameDecoder::new(chan_of(c), side_of(f));
            let mut input = bytes;
            while !input.is_empty() {
                match dec.push(&mut input) {
                    Err(_) => return true,
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        if input.is_empty() {
                            break;
                        }
                    }
                }
            }
            false
        }
        Ctx::Dgram(f) => decode_datagram(side_of(f), bytes).is_err(),
        Ctx::Kind => StreamKind::from_byte(bytes[0]).is_err(),
    }
}

fn ctl_c() -> Ctx {
    Ctx::Stream(Chan::Control, Sender::Client)
}
fn ctl_s() -> Ctx {
    Ctx::Stream(Chan::Control, Sender::Server)
}
fn world() -> Ctx {
    Ctx::Stream(Chan::World, Sender::Server)
}
fn chat() -> Ctx {
    Ctx::Stream(Chan::Chat, Sender::Server)
}

fn hello(max_entities: u16) -> Msg {
    Msg::Hello {
        minor_max: 0,
        caps: 0,
        max_entities,
        client_name: s("x"),
    }
}

fn move_input(loc: Loc, heading: f32, motion: u8) -> Msg {
    Msg::MoveInput {
        seq: 1,
        epoch: 1,
        loc,
        heading,
        vel: [0.0; 3],
        motion,
    }
}

fn update(props: Vec<(u16, Val)>) -> Vec<u8> {
    spec::frame(&Msg::EntityUpdate(43, props))
}

fn create_with(app: App, props: Vec<(u16, Val)>) -> Vec<u8> {
    spec::frame(&Msg::EntityCreate {
        id: 43,
        kind: 0,
        name: s("Bram"),
        class: None,
        time: 1,
        pose: Pose {
            loc: l(),
            heading: 0.0,
            anchor: None,
        },
        vel: [0.0; 3],
        motion: 0,
        app,
        props,
    })
}

fn nested_arrays(levels: usize) -> Val {
    let mut x = Val::Bool(true);
    for _ in 0..levels {
        x = Val::Arr(vec![x]);
    }
    x
}

/// `n` motion entries with distinct IDs (§6.5.2: each entity at most once per batch).
fn entries(e: Entry, n: u64) -> Vec<Entry> {
    (0..n).map(|i| Entry { id: e.id + i, ..e }).collect()
}

/// Replaces `len` bytes at `at` of an encoding.
fn patch(mut b: Vec<u8>, at: usize, with: &[u8]) -> Vec<u8> {
    b[at..at + with.len()].copy_from_slice(with);
    b
}

/// A frame header followed by `n` bytes of a `u16` string length field claiming `len`.
fn with_str(ty: u16, prefix: &[u8], len: u16, tail: &[u8]) -> Vec<u8> {
    let mut body = prefix.to_vec();
    body.extend_from_slice(&len.to_le_bytes());
    body.extend_from_slice(tail);
    spec::frame_raw(ty, &body)
}

#[allow(clippy::too_many_lines)]
fn malformed() -> Vec<(&'static str, Ctx, Vec<u8>)> {
    let app = plain(Body::Character(1, 1));
    let good_move = spec::datagram(&move_input(l(), 0.0, 0));
    let entry = Entry {
        id: 43,
        pos: [1.0, 2.0, 3.0],
        heading: 0.0,
        vel: [0.0; 3],
        motion: 0,
    };
    let hello_frame = spec::frame(&hello(1024));
    let mut hello_trailing = spec::frame_raw(0x0001, &spec::body(&hello(1024)));
    hello_trailing.push(0);
    hello_trailing[0] += 1;
    let mut hello_short = spec::frame_raw(0x0001, &spec::body(&hello(1024)));
    hello_short.pop();
    hello_short[0] -= 1;
    let mut chat_tell_long = vec![4u8];
    chat_tell_long.extend_from_slice(&65u16.to_le_bytes());
    chat_tell_long.extend_from_slice(&[b'a'; 65]);
    chat_tell_long.extend_from_slice(&[2, 0, b'h', b'i']);
    let big_string_array = Val::Arr(vec![Val::Str("x".repeat(1000)); 9]);
    let teleport_far = Msg::TeleportRequest(
        1,
        Dest::Anchored(
            l(),
            0.0,
            Anc {
                bx: 0,
                by: 0,
                cell: 0,
            },
        ),
    );
    let report = |anchor: Anc| Msg::LocationReport {
        cause: 3,
        request: 0,
        epoch: 2,
        time: 1,
        pose: Pose {
            loc: l(),
            heading: 0.0,
            anchor: Some(anchor),
        },
    };
    let far_pose = report(Anc {
        bx: 0,
        by: 140,
        cell: 0,
    });
    let good_report = report(anc());

    vec![
        // §5.4 frame layer, §16.7.
        ("frame length 1 (§16.7)", ctl_c(), hex("01 00 00 00 01")),
        ("frame length 0", ctl_c(), hex("00 00 00 00")),
        (
            "frame length 65,537, header only (§16.7)",
            ctl_c(),
            hex("01 00 01 00"),
        ),
        (
            "frame length u32::MAX, header only",
            world(),
            hex("ff ff ff ff"),
        ),
        ("unknown type 0x0000", ctl_c(), spec::frame_raw(0x0000, &[])),
        ("unknown type 0x0015", ctl_c(), spec::frame_raw(0x0015, &[])),
        ("unknown type 0x010A", world(), spec::frame_raw(0x010A, &[])),
        (
            "unknown type 0x0204",
            chat(),
            spec::frame_raw(0x0204, &[0, 0, 0, 0]),
        ),
        (
            "WorldEnter on control",
            ctl_s(),
            spec::frame_raw(0x0101, &[]),
        ),
        (
            "ChatMessage on world",
            world(),
            spec::frame_raw(0x0201, &[]),
        ),
        (
            "MoveInput as a frame",
            ctl_c(),
            spec::frame_raw(0x0301, &good_move[2..]),
        ),
        ("Hello sent by the server", ctl_s(), hello_frame.clone()),
        (
            "LoginOk sent by the client",
            ctl_c(),
            spec::frame(&Msg::LoginOk(s("a"), Vec::new())),
        ),
        (
            "Hello with a trailing byte (§16.7)",
            ctl_c(),
            hello_trailing,
        ),
        ("Hello one byte short", ctl_c(), hello_short),
        (
            "InterestReset with a body byte",
            world(),
            spec::frame_raw(0x0108, &[0]),
        ),
        // §2.3 rule 1, §16.7.
        ("stream kind 0x58 (§16.7)", Ctx::Kind, vec![0x58]),
        ("stream kind 0x00", Ctx::Kind, vec![0x00]),
        ("stream kind 0x77 ('w')", Ctx::Kind, vec![0x77]),
        // §5.1 primitives.
        (
            "bool 2 (EmoteSet looping)",
            ctl_c(),
            spec::frame_raw(0x000F, &[1, 0, 0, 0, 2]),
        ),
        ("opt tag 2 (Appearance idle)", ctl_c(), {
            let b = spec::frame(&Msg::AppearanceSet(app.clone()));
            patch(b, 6 + 9 + 4, &[2])
        }),
        (
            "invalid UTF-8 in Login account",
            ctl_c(),
            with_str(0x0003, &[], 2, &[0xc3, 0x28, 1, 0, b'p']),
        ),
        (
            "WorldClock game_clock NaN",
            world(),
            spec::frame(&Msg::WorldClock(1, f64::NAN, 1.0)),
        ),
        (
            "WorldClock rate +inf",
            world(),
            spec::frame(&Msg::WorldClock(1, 1.0, f32::INFINITY)),
        ),
        // §5.2 text class maxima (detected before reading the bytes).
        (
            "Login account 33 bytes",
            ctl_c(),
            with_str(0x0003, &[], 33, &[b'a'; 33]),
        ),
        (
            "Login password 257 bytes",
            ctl_c(),
            with_str(0x0003, &[1, 0, b'a'], 257, &[b'p'; 257]),
        ),
        (
            "Hello client_name 65 bytes",
            ctl_c(),
            with_str(0x0001, &[0, 0, 0, 0, 0, 0, 0, 4], 65, &[b'c'; 65]),
        ),
        (
            "ChatSend text 1,025 bytes",
            ctl_c(),
            with_str(0x000E, &[1], 1025, &[b't'; 1025]),
        ),
        (
            "ChatSend Tell peer 65 bytes",
            ctl_c(),
            spec::frame_raw(0x000E, &chat_tell_long),
        ),
        (
            "TeleportRequest place 257 bytes",
            ctl_c(),
            with_str(0x000C, &[1, 0, 0, 0, 4], 257, &[b'p'; 257]),
        ),
        (
            "EnterWorld name 65 bytes",
            ctl_c(),
            with_str(0x000A, &[], 65, &[b'n'; 65]),
        ),
        (
            "Disconnect text 1,025 bytes",
            ctl_s(),
            with_str(0x0014, &[0, 0], 1025, &[b'd'; 1025]),
        ),
        (
            "LoginOk account empty",
            ctl_s(),
            spec::frame(&Msg::LoginOk(s(""), Vec::new())),
        ),
        (
            "LoginOk account 33 bytes",
            ctl_s(),
            spec::frame(&Msg::LoginOk("a".repeat(33), Vec::new())),
        ),
        (
            "LoginOk character name empty",
            ctl_s(),
            spec::frame(&Msg::LoginOk(s("a"), vec![(s(""), Body::Character(1, 1))])),
        ),
        (
            "HelloAck motd 1,025 bytes",
            ctl_s(),
            spec::frame(&Msg::HelloAck {
                minor: 0,
                caps: 0,
                tick: 30,
                snapshot: 15,
                radius: 2,
                name: s(""),
                motd: "m".repeat(1025),
            }),
        ),
        (
            "HelloAck server_name 65 bytes",
            ctl_s(),
            spec::frame(&Msg::HelloAck {
                minor: 0,
                caps: 0,
                tick: 30,
                snapshot: 15,
                radius: 2,
                name: "n".repeat(65),
                motd: s(""),
            }),
        ),
        (
            "LoginFailed text 257 bytes",
            ctl_s(),
            spec::frame(&Msg::LoginFailed(1, 0, "t".repeat(257))),
        ),
        (
            "WorldEnter name empty",
            world(),
            spec::frame(&Msg::WorldEnter {
                player: 42,
                name: s(""),
                instance: 1,
                epoch: 1,
                time: 1,
                pose: Pose {
                    loc: l(),
                    heading: 0.0,
                    anchor: None,
                },
                app: app.clone(),
            }),
        ),
        (
            "EntityCreate name 65 bytes",
            world(),
            spec::frame(&Msg::EntityCreate {
                id: 43,
                kind: 0,
                name: "n".repeat(65),
                class: None,
                time: 1,
                pose: Pose {
                    loc: l(),
                    heading: 0.0,
                    anchor: None,
                },
                vel: [0.0; 3],
                motion: 0,
                app: app.clone(),
                props: Vec::new(),
            }),
        ),
        (
            "ChatMessage text empty",
            chat(),
            spec::frame(&Msg::ChatMessage {
                time: 1,
                channel: 1,
                flags: 0,
                sender_entity: 0,
                sender: s("Bram"),
                peer: None,
                text: s(""),
            }),
        ),
        (
            "ChatMessage Tell peer empty",
            chat(),
            spec::frame(&Msg::ChatMessage {
                time: 1,
                channel: 4,
                flags: 0,
                sender_entity: 0,
                sender: s("Bram"),
                peer: Some(s("")),
                text: s("hi"),
            }),
        ),
        (
            "ServerNotice text empty",
            chat(),
            spec::frame(&Msg::ServerNotice(0, 0, s(""))),
        ),
        (
            "ChatRejected text 257 bytes",
            chat(),
            spec::frame(&Msg::ChatRejected(1, "t".repeat(257))),
        ),
        // §5.3 structures, §13.1.
        ("Hello max_entities 63", ctl_c(), spec::frame(&hello(63))),
        (
            "Location x = NaN (§16.7)",
            Ctx::Dgram(Sender::Client),
            patch(good_move.clone(), 12, &hex("00 00 00 00 00 00 f8 7f")),
        ),
        (
            "Location y above 10^7",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(
                Loc {
                    y: 1.0e7 + 1.0,
                    ..l()
                },
                0.0,
                0,
            )),
        ),
        (
            "Location z below -10^7",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(Loc { z: -2.0e7, ..l() }, 0.0, 0)),
        ),
        (
            "MoveInput heading +inf",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(l(), f32::INFINITY, 0)),
        ),
        (
            "anchor block 255",
            ctl_c(),
            spec::frame(&Msg::TeleportRequest(
                1,
                Dest::Anchored(
                    Loc { x: 40_700.0, ..l() },
                    0.0,
                    Anc {
                        bx: 255,
                        by: 140,
                        cell: 0,
                    },
                ),
            )),
        ),
        (
            "Anchored destination not owned",
            ctl_c(),
            spec::frame(&teleport_far),
        ),
        ("Pose anchor not owned", world(), spec::frame(&far_pose)),
        (
            "Pose opt tag 2",
            world(),
            patch(spec::frame(&good_report), 6 + 17 + 26 + 4, &[2]),
        ),
        (
            "Destination tag 0",
            ctl_c(),
            spec::frame_raw(0x000C, &[1, 0, 0, 0, 0]),
        ),
        (
            "Destination tag 5",
            ctl_c(),
            spec::frame_raw(0x000C, &[1, 0, 0, 0, 5, 1, 0, b'x']),
        ),
        (
            "Body tag 3",
            ctl_c(),
            patch(spec::frame(&Msg::AppearanceSet(app.clone())), 6, &[3]),
        ),
        (
            "Body tag 0",
            ctl_c(),
            patch(spec::frame(&Msg::AppearanceSet(app.clone())), 6, &[0]),
        ),
        (
            "Object DID 0",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(plain(Body::Object(0)))),
        ),
        (
            "scale 0",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                scale: 0.0,
                ..app.clone()
            })),
        ),
        (
            "scale -1",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                scale: -1.0,
                ..app.clone()
            })),
        ),
        (
            "scale 100.5",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                scale: 100.5,
                ..app.clone()
            })),
        ),
        (
            "scale NaN",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                scale: f32::NAN,
                ..app.clone()
            })),
        ),
        (
            "33 equipped items",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                items: vec![
                    Item {
                        item: 1,
                        dye: None,
                        dye2: None,
                        stowed: false
                    };
                    33
                ],
                ..app.clone()
            })),
        ),
        (
            "item DID 0",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                items: vec![Item {
                    item: 0,
                    dye: None,
                    dye2: None,
                    stowed: false,
                }],
                ..app.clone()
            })),
        ),
        (
            "item dye 1.5",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                items: vec![Item {
                    item: 1,
                    dye: Some(1.5),
                    dye2: None,
                    stowed: false,
                }],
                ..app.clone()
            })),
        ),
        (
            "item secondary dye -0.5",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                items: vec![Item {
                    item: 1,
                    dye: None,
                    dye2: Some(-0.5),
                    stowed: false,
                }],
                ..app.clone()
            })),
        ),
        ("item flag bit 3", ctl_c(), {
            let b = spec::frame(&Msg::AppearanceSet(App {
                items: vec![Item {
                    item: 1,
                    dye: None,
                    dye2: None,
                    stowed: false,
                }],
                ..app.clone()
            }));
            let at = b.len() - 1;
            patch(b, at, &[0x08])
        }),
        (
            "65 characters in a list",
            ctl_s(),
            spec::frame(&Msg::CharacterList(
                (0..65)
                    .map(|i| (format!("c{i}"), Body::Character(1, 1)))
                    .collect(),
            )),
        ),
        // §6 enumerated fields.
        (
            "StanceSet 3 (§16.7)",
            ctl_c(),
            spec::frame(&Msg::StanceSet(3)),
        ),
        (
            "ChatSend channel 5 (§16.7)",
            ctl_c(),
            spec::frame(&Msg::ChatSend {
                channel: 5,
                peer: None,
                text: s("x"),
            }),
        ),
        (
            "ChatSend channel 0",
            ctl_c(),
            spec::frame(&Msg::ChatSend {
                channel: 0,
                peer: None,
                text: s("x"),
            }),
        ),
        (
            "ChatSend Say with a peer field",
            ctl_c(),
            spec::frame(&Msg::ChatSend {
                channel: 1,
                peer: Some(s("Bram")),
                text: s("x"),
            }),
        ),
        (
            "LocationReport cause 0",
            world(),
            patch(spec::frame(&good_report), 6, &[0]),
        ),
        (
            "LocationReport cause 4",
            world(),
            patch(spec::frame(&good_report), 6, &[4]),
        ),
        (
            "EntityCreate kind 4",
            world(),
            patch(create_with(app.clone(), Vec::new()), 14, &[4]),
        ),
        (
            "EntityDestroy reason 2",
            world(),
            spec::frame(&Msg::EntityDestroy(43, 2)),
        ),
        (
            "ChatMessage channel 6",
            chat(),
            spec::frame(&Msg::ChatMessage {
                time: 1,
                channel: 6,
                flags: 0,
                sender_entity: 0,
                sender: s("a"),
                peer: None,
                text: s("x"),
            }),
        ),
        (
            "ChatMessage channel 0",
            chat(),
            spec::frame(&Msg::ChatMessage {
                time: 1,
                channel: 0,
                flags: 0,
                sender_entity: 0,
                sender: s("a"),
                peer: None,
                text: s("x"),
            }),
        ),
        (
            "ServerNotice level 3",
            chat(),
            spec::frame(&Msg::ServerNotice(3, 0, s("x"))),
        ),
        (
            "motion gait 4",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(l(), 0.0, 4)),
        ),
        (
            "motion gait 7",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(l(), 0.0, 7)),
        ),
        (
            "motion reserved bit 4",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(l(), 0.0, 0x10)),
        ),
        ("EntityCreate motion reserved bit 7", world(), {
            let b = create_with(app.clone(), Vec::new());
            // id 8, kind 1, name 6, class 1, time 8, pose 31, velocity 12 -> motion at 6 + 67.
            patch(b, 6 + 67, &[0x80])
        }),
        // §7.2 property lists and values.
        (
            "PIDs 2 then 1 (§16.7)",
            world(),
            update(vec![(2, Val::None), (1, Val::Enum(0))]),
        ),
        ("PID 0", world(), update(vec![(0, Val::Bool(true))])),
        (
            "duplicate PID",
            world(),
            update(vec![(1, Val::Enum(0)), (1, Val::Enum(1))]),
        ),
        (
            "257 properties",
            world(),
            update((1..=257).map(|i| (0x8000 + i, Val::Bool(true))).collect()),
        ),
        (
            "value tag 0x10",
            world(),
            patch(
                update(vec![(0x8000, Val::Bool(true))]),
                6 + 8 + 2 + 2,
                &[0x10],
            ),
        ),
        (
            "value tag 0xFF",
            world(),
            patch(
                update(vec![(0x8000, Val::Bool(true))]),
                6 + 8 + 2 + 2,
                &[0xFF],
            ),
        ),
        (
            "None in EntityCreate",
            world(),
            create_with(app.clone(), vec![(0x8000, Val::None)]),
        ),
        (
            "None inside an array",
            world(),
            update(vec![(0x8000, Val::Arr(vec![Val::None]))]),
        ),
        (
            "None inside a struct",
            world(),
            update(vec![(0x8000, Val::Struct(vec![(1, Val::None)]))]),
        ),
        (
            "array with mixed tags",
            world(),
            update(vec![(0x8000, Val::Arr(vec![Val::I32(1), Val::Bool(true)]))]),
        ),
        (
            "array of 257",
            world(),
            update(vec![(0x8000, Val::Arr(vec![Val::Bool(true); 257]))]),
        ),
        (
            "struct of 65 fields",
            world(),
            update(vec![(
                0x8000,
                Val::Struct((1..=65).map(|f| (f, Val::Bool(true))).collect()),
            )]),
        ),
        (
            "struct field 0",
            world(),
            update(vec![(0x8000, Val::Struct(vec![(0, Val::Bool(true))]))]),
        ),
        (
            "struct fields 2 then 1",
            world(),
            update(vec![(
                0x8000,
                Val::Struct(vec![(2, Val::Bool(true)), (1, Val::Bool(true))]),
            )]),
        ),
        (
            "value depth 5",
            world(),
            update(vec![(0x8000, nested_arrays(4))]),
        ),
        (
            "value above 8,192 bytes",
            world(),
            update(vec![(0x8000, big_string_array)]),
        ),
        (
            "String value 1,025 bytes",
            world(),
            update(vec![(0x8000, Val::Str("s".repeat(1025)))]),
        ),
        (
            "Float32 value NaN",
            world(),
            update(vec![(0x8000, Val::F32(f32::NAN))]),
        ),
        (
            "Vector3 value -inf",
            world(),
            update(vec![(0x8000, Val::V3([0.0, f32::NEG_INFINITY, 0.0]))]),
        ),
        (
            "Bool value 2",
            world(),
            patch(
                update(vec![(0x8000, Val::Bool(true))]),
                6 + 8 + 2 + 2 + 1,
                &[2],
            ),
        ),
        (
            "STANCE as String",
            world(),
            update(vec![(1, Val::Str(s("x")))]),
        ),
        ("EMOTE as Enum", world(), update(vec![(2, Val::Enum(1))])),
        (
            "WALK_SPEED as Int32",
            world(),
            update(vec![(3, Val::I32(2))]),
        ),
        (
            "RUN_SPEED as Bool in EntityCreate",
            world(),
            create_with(app.clone(), vec![(4, Val::Bool(true))]),
        ),
        // §5.5 and §6.5 datagrams.
        ("datagram of 1,201 bytes", Ctx::Dgram(Sender::Server), {
            let mut d = vec![0x02, 0x03];
            d.resize(1201, 0);
            d
        }),
        ("datagram of 1 byte", Ctx::Dgram(Sender::Client), vec![0x01]),
        ("empty datagram", Ctx::Dgram(Sender::Client), Vec::new()),
        (
            "unknown datagram type 0x0305",
            Ctx::Dgram(Sender::Client),
            vec![0x05, 0x03, 0, 0],
        ),
        (
            "Ping as a datagram",
            Ctx::Dgram(Sender::Client),
            hex("12 00 ef cd ab 89 67 45 23 01"),
        ),
        (
            "MoveInput sent by the server",
            Ctx::Dgram(Sender::Server),
            good_move.clone(),
        ),
        (
            "MotionBatch sent by the client",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&Msg::MotionBatch(1, vec![entry])),
        ),
        ("MoveInput 54 bytes", Ctx::Dgram(Sender::Client), {
            let mut d = good_move.clone();
            d.push(0);
            d
        }),
        (
            "MoveInput 52 bytes",
            Ctx::Dgram(Sender::Client),
            good_move[..52].to_vec(),
        ),
        (
            "MotionBatch count 0 (§16.7)",
            Ctx::Dgram(Sender::Server),
            hex("02 03 00 00 00 00 00 00 00 00 00"),
        ),
        (
            "MotionBatch count 30",
            Ctx::Dgram(Sender::Server),
            spec::datagram(&Msg::MotionBatch(1, entries(entry, 30))),
        ),
        (
            "MotionBatch count 2, one entry",
            Ctx::Dgram(Sender::Server),
            patch(spec::datagram(&Msg::MotionBatch(1, vec![entry])), 10, &[2]),
        ),
        (
            "MotionBatch one entry plus a byte",
            Ctx::Dgram(Sender::Server),
            {
                let mut d = spec::datagram(&Msg::MotionBatch(1, vec![entry]));
                d.push(0);
                d
            },
        ),
        (
            "MotionBatch entry position NaN",
            Ctx::Dgram(Sender::Server),
            spec::datagram(&Msg::MotionBatch(
                1,
                vec![Entry {
                    pos: [f32::NAN, 0.0, 0.0],
                    ..entry
                }],
            )),
        ),
        (
            "MotionBatch entry gait 5",
            Ctx::Dgram(Sender::Server),
            spec::datagram(&Msg::MotionBatch(1, vec![Entry { motion: 5, ..entry }])),
        ),
        (
            "TimeRequest 9 bytes",
            Ctx::Dgram(Sender::Client),
            hex("03 03 40 42 0f 00 00 00 00"),
        ),
        (
            "TimeReply 33 bytes",
            Ctx::Dgram(Sender::Server),
            spec::datagram(&Msg::TimeReply(1, 2, 3, 4))[..33].to_vec(),
        ),
    ]
}

/// Both codecs reject every malformed input of the hand-written list.
#[test]
fn malformed_inputs_rejected_by_both() {
    let mut failures = Vec::new();
    for (name, ctx, bytes) in malformed() {
        let mine = mine_rejects(ctx, &bytes);
        let theirs = crate_rejects(ctx, &bytes);
        if !mine || !theirs {
            failures.push(format!(
                "{name}: independent rejects = {mine}, tessera_protocol rejects = {theirs}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Boundary values the spec allows are accepted by both codecs.
#[test]
fn boundary_values_accepted_by_both() {
    let app = plain(Body::Character(1, 1));
    let max_pos = Loc {
        region: 0xFFFF,
        x: 1.0e7,
        y: -1.0e7,
        z: -0.0,
    };
    let entry = Entry {
        id: 43,
        pos: [1.0, 2.0, 3.0],
        heading: 0.0,
        vel: [0.0; 3],
        motion: 0x0B, // Airborne + Backward
    };
    let cases: Vec<(&str, Ctx, Vec<u8>)> = vec![
        ("Hello max_entities 64", ctl_c(), spec::frame(&hello(64))),
        (
            "Hello max_entities 65,535",
            ctl_c(),
            spec::frame(&hello(65_535)),
        ),
        (
            "Hello unknown caps",
            ctl_c(),
            spec::frame(&Msg::Hello {
                minor_max: 0,
                caps: 0xFFFF_FFFF,
                max_entities: 64,
                client_name: s(""),
            }),
        ),
        (
            "coordinates at ±10^7",
            Ctx::Dgram(Sender::Client),
            spec::datagram(&move_input(max_pos, -1.0e30, 0x0B)),
        ),
        (
            "anchor at the ownership edge",
            world(),
            spec::frame(&Msg::LocationReport {
                cause: 2,
                request: 1,
                epoch: 1,
                time: 1,
                pose: Pose {
                    loc: Loc {
                        region: 1,
                        x: 1280.0,
                        y: 40_640.0 + 1280.0,
                        z: 0.0,
                    },
                    heading: 0.0,
                    anchor: Some(Anc {
                        bx: 0,
                        by: 254,
                        cell: 0xFFFF,
                    }),
                },
            }),
        ),
        (
            "scale 100",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                scale: 100.0,
                ..app.clone()
            })),
        ),
        (
            "32 items, dyes 0 and 1",
            ctl_c(),
            spec::frame(&Msg::AppearanceSet(App {
                items: vec![
                    Item {
                        item: 1,
                        dye: Some(0.0),
                        dye2: Some(1.0),
                        stowed: true
                    };
                    32
                ],
                ..app.clone()
            })),
        ),
        (
            "64 characters",
            ctl_s(),
            spec::frame(&Msg::CharacterList(
                (0..64)
                    .map(|i| (format!("c{i}"), Body::Object(7)))
                    .collect(),
            )),
        ),
        (
            "LoginFailed unknown code",
            ctl_s(),
            spec::frame(&Msg::LoginFailed(999, 0, s(""))),
        ),
        (
            "Disconnect unknown code",
            ctl_s(),
            spec::frame(&Msg::Disconnect(0xBEEF, s("bye"))),
        ),
        (
            "ChatSend text 1,024 bytes",
            ctl_c(),
            spec::frame(&Msg::ChatSend {
                channel: 2,
                peer: None,
                text: "t".repeat(1024),
            }),
        ),
        (
            "256 properties, unknown IDs",
            world(),
            update((1..=256).map(|i| (0x4000 + i, Val::Bool(true))).collect()),
        ),
        (
            "value depth 4",
            world(),
            update(vec![(0x8000, nested_arrays(3))]),
        ),
        (
            "empty array and struct",
            world(),
            update(vec![
                (0x8000, Val::Arr(Vec::new())),
                (0x8001, Val::Struct(Vec::new())),
            ]),
        ),
        (
            "array of 256",
            world(),
            update(vec![(0x8000, Val::Arr(vec![Val::Enum(1); 256]))]),
        ),
        (
            "struct of 64",
            world(),
            update(vec![(
                0x8000,
                Val::Struct((1..=64).map(|f| (f, Val::Time(1))).collect()),
            )]),
        ),
        (
            "STANCE unknown value",
            world(),
            update(vec![(1, Val::Enum(77))]),
        ),
        (
            "server-private property",
            world(),
            create_with(app.clone(), vec![(0xFFFF, Val::Rgba([1, 2, 3, 4]))]),
        ),
        (
            "MotionBatch 29 entries",
            Ctx::Dgram(Sender::Server),
            spec::datagram(&Msg::MotionBatch(1, entries(entry, 29))),
        ),
        (
            "datagram of 1,084 bytes is the 29-entry batch",
            Ctx::Dgram(Sender::Server),
            {
                let d = spec::datagram(&Msg::MotionBatch(1, entries(entry, 29)));
                assert_eq!(d.len(), 1084);
                d
            },
        ),
        (
            "server text with a control character",
            chat(),
            spec::frame(&Msg::ServerNotice(1, 0, s("a\u{7}b"))),
        ),
        (
            "client text with a control character",
            ctl_c(),
            spec::frame(&Msg::ChatSend {
                channel: 1,
                peer: None,
                text: s("a\u{1b}b"),
            }),
        ),
        (
            "client empty name and text",
            ctl_c(),
            spec::frame(&Msg::ChatSend {
                channel: 4,
                peer: Some(s("")),
                text: s(""),
            }),
        ),
    ];
    let mut failures = Vec::new();
    for (name, ctx, bytes) in cases {
        let mine = mine_rejects(ctx, &bytes);
        let theirs = crate_rejects(ctx, &bytes);
        if mine || theirs {
            failures.push(format!(
                "{name}: independent rejects = {mine}, tessera_protocol rejects = {theirs}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// §5.2: control characters in server text become U+0020 in both decoders.
#[test]
fn server_text_is_sanitised_by_both() {
    let frame = spec::frame(&Msg::ServerNotice(0, 0, s("a\u{7}b\u{85}c")));
    let (mine, _) = spec::decode_stream(Chan::Chat, Sender::Server, &frame).expect("ok");
    assert_eq!(mine, vec![Msg::ServerNotice(0, 0, s("a b c"))]);
    let mut dec = FrameDecoder::new(Channel::Chat, Side::Server);
    let mut input: &[u8] = &frame;
    let theirs = dec.push(&mut input).expect("ok").expect("one message");
    assert_eq!(theirs, to_crate(&Msg::ServerNotice(0, 0, s("a b c"))));
}

/// §5.4: a frame is not complete until `length` bytes follow, however it is split.
#[test]
fn split_frames_need_all_bytes() {
    let frame = spec::frame(&Msg::Ping(1));
    for cut in 0..frame.len() {
        assert_eq!(spec::split(&frame[..cut]), Ok(None), "cut at {cut}");
        let mut dec = FrameDecoder::new(Channel::Control, Side::Client);
        let mut input = &frame[..cut];
        assert_eq!(dec.push(&mut input).expect("partial"), None, "cut at {cut}");
    }
}

// ---------------------------------------------------------------------------------------
// Second review round (2026-10-07): premises of the later `> **OPEN:** (review)` notes.
// These pin the spec *as written*. When the spec writer settles one of those notes, the
// matching test here is expected to change with it.

/// §7.2 Array: "all with the **same** tag" compares the element tags only. Two inner arrays
/// with different contents share tag 0x0E, so the literal rule accepts them, and both codecs
/// do. OPEN (review) at §7.2 asks whether element shape is constrained further.
#[test]
fn open_review_nested_array_shapes_follow_the_literal_rule() {
    let mixed = Val::Arr(vec![
        Val::Arr(vec![Val::I32(1)]),
        Val::Arr(vec![Val::Bool(true)]),
    ]);
    let frame = update(vec![(0x8000, mixed.clone())]);
    assert!(!mine_rejects(world(), &frame), "independent codec");
    assert!(!crate_rejects(world(), &frame), "tessera_protocol");
    let value = c_val(&mixed);
    let mut bytes = Vec::new();
    value.encode(&mut bytes, false).expect("encodable");
    let (back, used) = pv::Value::decode(&bytes, false).expect("decodes");
    assert_eq!((back, used), (value, bytes.len()));
    // The outer tag rule itself still holds: an Array mixing Int32 and Bool is rejected.
    let flat = update(vec![(0x8000, Val::Arr(vec![Val::I32(1), Val::Bool(true)]))]);
    assert!(mine_rejects(world(), &flat), "independent codec");
    assert!(crate_rejects(world(), &flat), "tessera_protocol");
}

/// §8.3 step 6, §8.7 and §9.1: an anchor is dropped only when it no longer owns the accepted
/// position, and ownership reaches ±1,280 m. A player who leaves an anchored pose on foot
/// keeps the anchor block as its interest block for up to 8 blocks. OPEN (review) at §8.7.
#[test]
fn open_review_anchor_outlives_the_interior() {
    use tessera_protocol::movement::{
        MoveVerdict, MovementLimits, MovementValidator, block_distance, interest_block,
    };
    let anchor = pt::Anchor::new(126, 140, 257);
    let start = pt::Pose {
        location: pt::Location::new(1, 20_250.5, 22_500.25, 0.0),
        heading: 0.0,
        anchor: Some(anchor),
    };
    let mut v = MovementValidator::new(MovementLimits::default(), 1, start, 0);
    // East at 6 m/s (below the 8.5 m/s V_h), one MoveInput per 15 Hz send.
    let step_us = 66_667_u64;
    let mut x = 20_250.5_f64;
    let mut now = 0_u64;
    for seq in 1..=2_250_u32 {
        now += step_us;
        x += 6.0 * step_us as f64 / 1e6;
        let input = pm::MoveInput {
            seq,
            epoch: 1,
            location: pt::Location::new(1, x, 22_500.25, 0.0),
            heading: 0.0,
            velocity: pt::Vec3::new(6.0, 0.0, 0.0),
            motion: pt::Motion::new(pt::Gait::Running, false),
        };
        assert_eq!(v.check(&input, now), MoveVerdict::Accepted, "seq {seq}");
    }
    let pose = *v.pose();
    assert!(pose.location.x > 21_100.0, "walked about 900 m");
    assert_eq!(
        pose.anchor,
        Some(anchor),
        "the anchor still owns the position"
    );
    let by_position = interest_block(&pose.location, None);
    let by_rule = interest_block(&pose.location, pose.anchor.as_ref());
    assert_eq!(by_position, (132, 140));
    assert_eq!(by_rule, (126, 140));
    // Beyond R + 1 = 3 at the default radius: observers at the real position never see it.
    assert!(block_distance(by_position, by_rule) > 3);
}
