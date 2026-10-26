// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target for the live connector's UDP session driver
//! (`tessera_compat::connector::session_phase`; `official-session.md` §6, §9.3,
//! `official-messages-world.md` §3–§4, `official-messages-play.md` §8): arbitrary datagrams
//! from the login and world servers, and arbitrary messages wrapped in valid sequenced
//! datagrams after a synthetic handshake, interleaved with neutral commands. Nothing opens
//! a socket: the network is an in-memory stand-in. No panic, bounded state.
#![no_main]

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex};

use libfuzzer_sys::fuzz_target;
use tessera_compat::connector::data::{InstallData, SchemaView};
use tessera_compat::connector::net::{ConnectorNet, SendHttpClient};
use tessera_compat::connector::session_phase::{SessionDriver, SessionStart};
use tessera_compat::login::{
    HttpClient, HttpRequest, HttpResponse, LoginError, SessionHandoff, TicketRefresher, VersionInfo,
};
use tessera_compat::play::PlayError;
use tessera_compat::session::{
    ConnectChallengeHeader, DatagramHeader, IsaacCipher, OptionalHeaders, SessionError,
    UdpDatagramTransport, encode_datagram, split_blob,
};
use tessera_compat::world::PropertyType;
use tessera_net::{ChatChannel, Command, Location, Vec3};

const LOCAL: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 1), 5000);
const LOGIN: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 10), 9001);
const WORLD: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(192, 0, 2, 20), 9100);
const SEED: u32 = 0xBEEF_0001;
const MAX_STEPS: usize = 256;

#[derive(Debug)]
struct NoData;

impl InstallData for NoData {
    fn record(&self, _did: u32) -> Option<Vec<u8>> {
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
struct Net(Arc<Mutex<Queues>>);

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

    fn resolve(&mut self, host: &str, _port: u16) -> Result<SocketAddrV4, SessionError> {
        match host {
            "login.example.invalid" => Ok(LOGIN),
            _ => Err(SessionError::Io("unknown".to_owned())),
        }
    }

    fn local_ipv4_toward(&mut self, _remote: SocketAddrV4) -> Option<Ipv4Addr> {
        Some(*LOCAL.ip())
    }

    fn bind_udp(&mut self, _ip: Ipv4Addr, _port: u16, _unique: bool) -> Result<Udp, SessionError> {
        Ok(Udp(Arc::clone(&self.0)))
    }

    fn chat_binder(&self) -> Option<Box<dyn FnMut() -> Result<Udp, PlayError> + Send>> {
        None
    }
}

fn challenge(rec_id: u16) -> Option<Vec<u8>> {
    let opt = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            server_time: 1.0,
            assigned_client_rec_id: rec_id,
            server_rec_id: 0x0222,
            outgoing_seed: 0xCAFE_1234,
            incoming_seed: SEED,
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

struct Server {
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
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
}

fn start(queues: &Arc<Mutex<Queues>>) -> Option<SessionDriver<Net>> {
    let version = VersionInfo::new("9999", "00000000-0000-0000-0000-000000000000", 1).ok()?;
    let handoff = SessionHandoff::new(
        "login.example.invalid:9001",
        0,
        "SUB0000000000000000000000",
        "AAECAwQFBgcICQ==",
        version.clone(),
        "English",
        "",
        "https://auth.example.invalid/",
        0,
    )
    .ok()?;
    let refresher = TicketRefresher::new("", "", "", 0, 0.0);
    let start = SessionStart {
        handoff,
        refresher,
        version,
        codecs: Default::default(),
        codec_uuids: Vec::new(),
        schema: SchemaView(Arc::new(NoData)),
    };
    SessionDriver::start(&mut Net(Arc::clone(queues)), start, 0.0).ok()
}

fuzz_target!(|data: &[u8]| {
    let queues = Arc::new(Mutex::new(Queues::default()));
    let Some(mut driver) = start(&queues) else {
        return;
    };
    let deliver = |d: Vec<u8>, src: SocketAddrV4| {
        if let Ok(mut q) = queues.lock() {
            q.inbound.push_back((d, src));
        }
    };
    let mut server = Server {
        isaac: IsaacCipher::from_seed(SEED),
        seq: 1,
    };
    let mut now = 0.0f64;
    let mut rest = data;
    if rest.first().is_some_and(|b| b & 1 == 1) {
        // A synthetic handshake first, so the messages below reach the upper layers.
        if let Some(c) = challenge(0x0111) {
            deliver(c, LOGIN);
        }
        driver.step(now);
    }
    rest = rest.get(1..).unwrap_or_default();
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
        match kind % 6 {
            0 => deliver(chunk.to_vec(), LOGIN),
            1 => deliver(chunk.to_vec(), WORLD),
            2 | 3 => {
                // A message: opcode (one byte, widened) and its envelope.
                let opcode = match chunk.first() {
                    Some(&b) if kind % 6 == 3 => 0x0000_0100 | u32::from(b),
                    Some(&b) => u32::from(b),
                    None => 0x6C,
                };
                if let Some(d) = server.blob(opcode, chunk.get(1..).unwrap_or_default()) {
                    deliver(d, LOGIN);
                }
            }
            4 => {
                let text = String::from_utf8_lossy(chunk).into_owned();
                let cmd = match chunk.first().copied().unwrap_or(0) % 3 {
                    0 => Command::EnterWorld { character: text },
                    1 => Command::Chat {
                        channel: ChatChannel::Say,
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
            _ => now += f64::from(chunk.first().copied().unwrap_or(1)) * 0.25,
        }
        driver.step(now);
        let _ = driver.take_events();
    }
    driver.begin_shutdown(now);
    let _ = driver.shutdown_step(now + 1.0);
});
