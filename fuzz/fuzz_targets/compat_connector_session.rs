// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for the live connector's UDP session driver
//! (`tessera_compat::connector::session_phase`; `official-session.md` §3.4, §6, §9.3,
//! `official-messages-world.md` §3–§4, `official-messages-play.md` §6.6, §8). Nothing opens
//! a socket: the network is an in-memory stand-in.
//!
//! The first byte selects how far a synthetic, valid exchange goes before the fuzzed part:
//! bit 0 the login handshake, bit 1 a valid character list (`0x6C`), bit 2 `EnterWorld`, a
//! referral, the world handshake and `0x90` (the session is then in the world), bit 3 the
//! wire dictionaries of a synthetic install, bit 4 a chat server with an in-memory socket
//! binder, bit 5 a ticket lifetime (so the refresh runs, against a client that always
//! fails), bit 6 a cancelled attempt (the driver's stop predicate and the refresh thread's
//! cancel flag hold), bit 7 the end without sending (a revoked right, `abandon`) instead of
//! the graceful shutdown. Then arbitrary
//! datagrams from the login, world and chat servers, arbitrary messages wrapped in valid
//! sequenced datagrams of the login or world connection, neutral commands and clock steps
//! follow. No panic, bounded state.
#![no_main]

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use libfuzzer_sys::fuzz_target;
use tessera_compat::chat_session::ChatCodecs;
use tessera_compat::connector::data::{InstallData, SchemaView};
use tessera_compat::connector::net::{ConnectorNet, SendHttpClient};
use tessera_compat::connector::session_phase::{SessionDriver, SessionStart};
use tessera_compat::login::{
    HttpClient, HttpRequest, HttpResponse, LoginError, SessionHandoff, TicketRefresher, VersionInfo,
};
use tessera_compat::play::PlayError;
use tessera_compat::session::{
    BitTreeTable, ConnectChallengeHeader, DatagramHeader, IsaacCipher, OptionalHeaders,
    SessionError, UdpDatagramTransport, WorldReferralHeader, encode_datagram, split_blob,
};
use tessera_compat::world::{
    CharacterListResponse, CharacterSummaryEntry, EncodedPropertyTable, EntityAppearanceDesc,
    PlayerSessionReady, PropertyType, encode_event_envelope,
};
use tessera_net::{ChatChannel, Command, Location, SocialCommand, Vec3};

const LOCAL: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 5000);
const LOGIN: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9001);
const WORLD: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 20), 9100);
const CHAT: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 30), 9200);
const LOGIN_SEED: u32 = 0xBEEF_0001;
const WORLD_SEED: u32 = 0xBEEF_0002;
const MAX_STEPS: usize = 256;

#[derive(Debug)]
struct NoData;

impl InstallData for NoData {
    fn record(&self, _did: u32, _max: usize) -> Option<Vec<u8>> {
        None
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        PropertyType::from_code(pid % 23).ok()
    }
}

#[derive(Debug, Default)]
struct Queues {
    inbound: VecDeque<(Vec<u8>, SocketAddrV4)>,
    sent: usize,
}

#[derive(Debug)]
struct Udp(Arc<Mutex<Queues>>);

impl UdpDatagramTransport for Udp {
    fn local_addr(&self) -> Result<SocketAddrV4, SessionError> {
        Ok(LOCAL)
    }

    fn send_to(&mut self, buf: &[u8], _target: SocketAddrV4) -> Result<usize, SessionError> {
        if let Ok(mut q) = self.0.lock() {
            q.sent = q.sent.saturating_add(1);
        }
        Ok(buf.len())
    }

    fn recv_from(&mut self, buf: &mut [u8]) -> Result<Option<(usize, SocketAddrV4)>, SessionError> {
        let next = self.0.lock().ok().and_then(|mut q| q.inbound.pop_front());
        Ok(next.and_then(|(d, src)| {
            let n = d.len().min(buf.len());
            buf.get_mut(..n)?.copy_from_slice(d.get(..n)?);
            Some((n, src))
        }))
    }
}

#[derive(Debug)]
struct NoHttp;

impl HttpClient for NoHttp {
    fn execute(&mut self, _request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        Err(LoginError::ServiceUnavailable {
            reason: "no network in the fuzz target".to_owned(),
        })
    }
}

#[derive(Debug)]
struct Net {
    session: Arc<Mutex<Queues>>,
    chat: Option<Arc<Mutex<Queues>>>,
}

impl ConnectorNet for Net {
    type Udp = Udp;

    fn http(&mut self, _request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        Err(LoginError::ServiceUnavailable {
            reason: "no network in the fuzz target".to_owned(),
        })
    }

    fn background_http(&self) -> SendHttpClient {
        Box::new(NoHttp)
    }

    fn is_live(&self) -> bool {
        true
    }

    fn resolve(&mut self, host: &str, _port: u16) -> Result<SocketAddrV4, SessionError> {
        match host {
            "login.example.invalid" => Ok(LOGIN),
            "chat.example.invalid" => Ok(CHAT),
            _ => Err(SessionError::Io("unknown".to_owned())),
        }
    }

    fn local_ipv4_toward(&mut self, _remote: SocketAddrV4) -> Option<Ipv4Addr> {
        Some(*LOCAL.ip())
    }

    fn bind_udp(&mut self, _ip: Ipv4Addr, _port: u16, _unique: bool) -> Result<Udp, SessionError> {
        Ok(Udp(Arc::clone(&self.session)))
    }

    fn chat_binder(&self) -> Option<Box<dyn FnMut() -> Result<Udp, PlayError> + Send>> {
        let chat = Arc::clone(self.chat.as_ref()?);
        Some(Box::new(move || Ok(Udp(Arc::clone(&chat)))))
    }
}

fn challenge(rec_id: u16, seed: u32) -> Option<Vec<u8>> {
    let opt = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            server_time: 1.0,
            assigned_client_rec_id: rec_id,
            server_rec_id: 0x0222,
            outgoing_seed: 0xCAFE_1234,
            incoming_seed: seed,
            reserved_dword_14: 0,
            client_conn_idx: 4,
            reserved_word_1a: 0,
            codec_uuid: [0u8; 16],
        }),
        ..OptionalHeaders::default()
    };
    let hdr = DatagramHeader {
        rec_id,
        ..DatagramHeader::default()
    };
    encode_datagram(hdr, &opt, &[], None, None).ok()
}

/// One synthetic server connection.
struct Server {
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
    fn new(seed: u32) -> Self {
        Self {
            isaac: IsaacCipher::from_seed(seed),
            seq: 1,
        }
    }

    fn control(&self, opt: OptionalHeaders) -> Option<Vec<u8>> {
        let hdr = DatagramHeader {
            rec_id: 0x0111,
            seq_num: self.seq,
            ..DatagramHeader::default()
        };
        encode_datagram(hdr, &opt, &[], None, None).ok()
    }

    fn blob(&mut self, opcode: u32, envelope: &[u8]) -> Option<Vec<u8>> {
        self.seq = self.seq.checked_add(1)?;
        let frags = split_blob(2, (self.seq & 0xFFFF) as u16, 1, 0, None, opcode, envelope).ok()?;
        let mask = self.isaac.next_mask();
        let hdr = DatagramHeader {
            rec_id: 0x0111,
            seq_num: self.seq,
            ..DatagramHeader::default()
        };
        encode_datagram(hdr, &OptionalHeaders::default(), &frags, Some(mask), None).ok()
    }

    fn event(&mut self, opcode: u32, inner: &[u8]) -> Option<Vec<u8>> {
        self.blob(opcode, &encode_event_envelope(inner))
    }
}

fn character_list() -> Option<Vec<u8>> {
    CharacterListResponse {
        characters: vec![CharacterSummaryEntry {
            character_id: 0x0200_0000_0000_0042,
            name: "Aldo".to_owned(),
            status_or_world: 0,
            public_props: EncodedPropertyTable::new(),
            private_props: EncodedPropertyTable::new(),
            metadata_a: 0,
            visual_desc: EntityAppearanceDesc::default(),
            metadata_b: 0,
            metadata_c: u32::MAX,
            metadata_d: 0,
            metadata_e: u32::MAX,
            timestamp_or_meta: 0,
        }],
        account_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .ok()
}

fn session_ready() -> Option<Vec<u8>> {
    PlayerSessionReady {
        player_id: 0x0100_0000_0000_0007,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .ok()
}

fn codecs(on: bool) -> (ChatCodecs, Vec<[u8; 16]>) {
    if !on {
        return (ChatCodecs::default(), Vec::new());
    }
    let out = Arc::new(BitTreeTable::synthetic_dictionary([1; 16]));
    let inc = Arc::new(BitTreeTable::synthetic_dictionary([2; 16]));
    (
        ChatCodecs {
            incoming: Some(inc),
            outgoing: Some(out),
        },
        vec![[1; 16], [2; 16]],
    )
}

fn start(net: &mut Net, flags: u8) -> Option<SessionDriver<Net>> {
    let version = VersionInfo::new("9999", "00000000-0000-0000-0000-000000000000", 1).ok()?;
    let chat = if flags & 0x10 != 0 {
        "chat.example.invalid:9200"
    } else {
        ""
    };
    let lifetime = if flags & 0x20 != 0 { 60 } else { 0 };
    let handoff = SessionHandoff::new(
        "login.example.invalid:9001",
        0,
        "SUB0000000000000000000000",
        "AAECAwQFBgcICQ==",
        version.clone(),
        "English",
        chat,
        "https://auth.example.invalid/",
        lifetime,
    )
    .ok()?;
    let refresher = TicketRefresher::new(
        "https://auth.example.invalid/",
        "SUB0000000000000000000000",
        "AAECAwQFBgcICQ==",
        lifetime,
        0.0,
    );
    let (codecs, codec_uuids) = codecs(flags & 0x08 != 0);
    let start = SessionStart {
        handoff,
        login_peer: LOGIN,
        refresh_secure: true,
        refresher,
        chat_endpoint: (flags & 0x10 != 0).then_some(CHAT),
        version,
        codecs,
        codec_uuids,
        schema: SchemaView(Arc::new(NoData)),
        cancel: Arc::new(AtomicBool::new(flags & 0x40 != 0)),
    };
    SessionDriver::start(net, start, 0.0, &|| false).ok()
}

fn ack() -> OptionalHeaders {
    OptionalHeaders {
        cumulative_ack: Some(0),
        ..OptionalHeaders::default()
    }
}

fuzz_target!(|data: &[u8]| {
    let session = Arc::new(Mutex::new(Queues::default()));
    let flags = data.first().copied().unwrap_or(0);
    let chat = (flags & 0x10 != 0).then(|| Arc::new(Mutex::new(Queues::default())));
    let mut net = Net {
        session: Arc::clone(&session),
        chat: chat.clone(),
    };
    let Some(mut driver) = start(&mut net, flags) else {
        return;
    };
    let deliver = |q: &Arc<Mutex<Queues>>, d: Option<Vec<u8>>, src: SocketAddrV4| {
        if let (Some(d), Ok(mut q)) = (d, q.lock()) {
            q.inbound.push_back((d, src));
        }
    };
    let mut login = Server::new(LOGIN_SEED);
    let mut world = Server::new(WORLD_SEED);
    let mut in_world = false;
    let mut now = 0.0f64;
    let step = |driver: &mut SessionDriver<Net>, now: f64| {
        // Bit 6 of the first byte: the attempt is cancelled (no referral, no refresh).
        driver.step(now, true, &|| flags & 0x40 != 0);
        if let Some((player, name)) = driver.wants_chat(now) {
            match net.chat_binder() {
                Some(b) => driver.attach_chat(b, player, &name, now),
                None => driver.chat_failed(now),
            }
        }
        let _ = driver.take_events();
    };
    if flags & 0x01 != 0 {
        deliver(&session, challenge(0x0111, LOGIN_SEED), LOGIN);
        deliver(&session, login.control(ack()), LOGIN);
        step(&mut driver, now);
        if flags & 0x02 != 0 {
            deliver(
                &session,
                character_list().and_then(|l| login.event(0x6C, &l)),
                LOGIN,
            );
            step(&mut driver, now);
            if flags & 0x04 != 0 {
                let _ = driver.command(
                    &Command::EnterWorld {
                        character: "Aldo".to_owned(),
                    },
                    now,
                );
                deliver(
                    &session,
                    login.control(OptionalHeaders {
                        world_referral: Some(WorldReferralHeader {
                            referral_cookie: 0x1122_3344_5566_7788,
                            sin_family: 2,
                            target_addr: WORLD,
                            reserved_word_10: 0,
                        }),
                        ..OptionalHeaders::default()
                    }),
                    LOGIN,
                );
                step(&mut driver, now);
                deliver(&session, challenge(0x0111, WORLD_SEED), WORLD);
                deliver(&session, world.control(ack()), WORLD);
                deliver(
                    &session,
                    session_ready().and_then(|r| world.event(0x90, &r)),
                    WORLD,
                );
                step(&mut driver, now);
                in_world = driver.in_world();
            }
        }
    }
    let mut rest = data.get(1..).unwrap_or_default();
    let mut steps = 0;
    while let Some((&kind, tail)) = rest.split_first() {
        steps += 1;
        if steps > MAX_STEPS || driver.ended() {
            break;
        }
        let len = usize::from(tail.first().copied().unwrap_or(0));
        let chunk = tail
            .get(1..1 + len)
            .unwrap_or_else(|| tail.get(1..).unwrap_or_default());
        rest = tail.get(1 + chunk.len()..).unwrap_or_default();
        match kind % 8 {
            0 => deliver(&session, Some(chunk.to_vec()), LOGIN),
            1 => deliver(&session, Some(chunk.to_vec()), WORLD),
            2 | 3 => {
                // A message: opcode (one byte, widened) and its envelope, on the world
                // connection once in the world, else on the login connection.
                let opcode = match chunk.first() {
                    Some(&b) if kind % 8 == 3 => 0x0000_0100 | u32::from(b),
                    Some(&b) => u32::from(b),
                    None => 0x6C,
                };
                let body = chunk.get(1..).unwrap_or_default();
                if in_world {
                    deliver(&session, world.blob(opcode, body), WORLD);
                } else {
                    deliver(&session, login.blob(opcode, body), LOGIN);
                }
            }
            4 => {
                let text = String::from_utf8_lossy(chunk).into_owned();
                let cmd = match chunk.first().copied().unwrap_or(0) % 5 {
                    0 => Command::EnterWorld { character: text },
                    1 => Command::Chat {
                        channel: ChatChannel::Say,
                        text,
                    },
                    2 => Command::Social(SocialCommand::JoinChannel { name: text }),
                    3 => Command::Chat {
                        channel: ChatChannel::Fellowship,
                        text,
                    },
                    _ => Command::Move {
                        location: Location::default(),
                        heading: f32::from(chunk.len() as u8),
                        velocity: Vec3::new(1.0, 0.5, 0.0),
                        on_ground: true,
                    },
                };
                let _ = driver.command(&cmd, now);
            }
            5 => {
                if let Some(q) = &chat {
                    deliver(q, Some(chunk.to_vec()), CHAT);
                }
            }
            6 => {
                // A datagram from an address no connection has.
                deliver(
                    &session,
                    Some(chunk.to_vec()),
                    SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 99), 1),
                );
            }
            _ => now += f64::from(chunk.first().copied().unwrap_or(1)) * 0.25,
        }
        step(&mut driver, now);
    }
    if flags & 0x80 != 0 {
        driver.abandon();
    } else {
        driver.begin_shutdown(now);
        let _ = driver.shutdown_step(now + 1.0);
        let _ = driver.shutdown_step(now + 3.0);
    }
});
