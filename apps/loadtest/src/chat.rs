// SPDX-License-Identifier: GPL-3.0-or-later
//! Chat delivery bookkeeping (`test-server.md` §13.2 "Chat"): every line a bot sends
//! carries a tag naming its speaker and sequence number. The ledger records who must
//! receive it (from the rules of `tessera-protocol.md` §10.1 at the moment it was sent),
//! who may (near a range edge, or while moving between sessions or places), and checks every
//! receipt: a line must reach each certain recipient within 2 s, exactly once, and in order
//! with the speaker's other lines. A line on a named channel (protocol minor 2,
//! `tessera-protocol.md` §10.6) must reach exactly the channel's members (`test-server.md`
//! §14 invariant 15).

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::metrics::Histogram;

/// A line must arrive within this time.
pub const DELIVERY_LIMIT: Duration = Duration::from_secs(2);
/// A line is judged this long after it was sent (late arrivals still count as late).
pub const JUDGE_AFTER: Duration = Duration::from_secs(10);
/// Most lines waiting for judgement; beyond it the oldest are judged early.
pub const MAX_OPEN_LINES: usize = 100_000;

/// The channel of a line, as the ledger needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Range-limited (Say).
    Say,
    /// Instance-wide (Yell).
    Yell,
    /// To one character (Tell).
    Tell,
    /// To the members of the run's shared channel with this number (`Named`, minor 2).
    Channel(u8),
}

impl Kind {
    /// Whether delivery depends on the speaker's position (Say).
    #[must_use]
    pub fn positional(self) -> bool {
        self == Self::Say
    }
}

/// Counts of chat delivery problems and successes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatCounts {
    /// Lines sent.
    pub lines: u64,
    /// Certain deliveries judged.
    pub expected: u64,
    /// Certain deliveries that arrived once, in time.
    pub delivered: u64,
    /// Arrived after [`DELIVERY_LIMIT`].
    pub late: u64,
    /// Never arrived.
    pub lost: u64,
    /// Arrived more than once (counted once per recipient and line).
    pub duplicated: u64,
    /// Arrived before an earlier line of the same speaker.
    pub out_of_order: u64,
    /// Arrived at a bot the rules exclude.
    pub unexpected: u64,
    /// Optional receipts (near a range edge or a session change), not judged.
    pub optional_received: u64,
    /// Lines with a tag the ledger does not know (another run, or garbled).
    pub unknown: u64,
    /// Lines the server rejected (`ChatRejected`); their recipients are not judged.
    pub rejected: u64,
    /// Lines sent on named channels (included in `lines`).
    pub channel_lines: u64,
}

/// One chat problem, for the report's episode list.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatProblem {
    /// What went wrong: `late`, `lost`, `duplicated`, `out-of-order`, `unexpected`.
    pub what: &'static str,
    /// The speaker bot.
    pub speaker: usize,
    /// The recipient bot.
    pub recipient: usize,
    /// The line's sequence number.
    pub seq: u32,
    /// Seconds since the run started, at send time.
    pub sent_s: f64,
    /// The delay when it arrived, seconds.
    pub delay_s: Option<f64>,
}

#[derive(Debug, Clone)]
struct Line {
    kind: Kind,
    sent: Instant,
    certain: Vec<usize>,
    optional: Vec<usize>,
    receipts: Vec<(usize, Instant)>,
    rejected: bool,
}

/// The ledger.
#[derive(Debug)]
pub struct ChatLedger {
    run: u32,
    start: Instant,
    lines: HashMap<(usize, u32), Line>,
    order: VecDeque<(usize, u32)>,
    last_seen: HashMap<(usize, usize), u32>,
    /// Totals.
    pub counts: ChatCounts,
    /// Delivery latency of judged deliveries, ms.
    pub latency_ms: Histogram,
    /// The first problems (at most 1,000).
    pub problems: Vec<ChatProblem>,
}

/// Most problems listed individually.
const MAX_PROBLEMS: usize = 1_000;

impl ChatLedger {
    /// A ledger for a run started at `start`; `run` makes tags unique to this run.
    #[must_use]
    pub fn new(run: u32, start: Instant) -> Self {
        Self {
            run,
            start,
            lines: HashMap::new(),
            order: VecDeque::new(),
            last_seen: HashMap::new(),
            counts: ChatCounts::default(),
            latency_ms: Histogram::new(),
            problems: Vec::new(),
        }
    }

    /// The text of line `seq` of `speaker`.
    #[must_use]
    pub fn text(&self, speaker: usize, seq: u32, kind: Kind) -> String {
        let k = match kind {
            Kind::Say => "say",
            Kind::Yell => "yell",
            Kind::Tell => "tell",
            Kind::Channel(_) => "channel",
        };
        format!("lt{:08x} {speaker} {seq} {k} load test line", self.run)
    }

    /// The name of shared channel `k` of this run: letters, digits and single spaces, so
    /// it meets the server's channel-name rule (`test-server.md` §9.1), and unique to the
    /// run, so runs against one server never share a channel.
    #[must_use]
    pub fn channel_name(&self, k: u8) -> String {
        format!("lt{:08x} ch{k}", self.run)
    }

    /// Parses a tag made by [`text`](Self::text) for this run.
    #[must_use]
    pub fn parse(&self, text: &str) -> Option<(usize, u32)> {
        let mut it = text.split(' ');
        let tag = it.next()?.strip_prefix("lt")?;
        if u32::from_str_radix(tag, 16).ok()? != self.run {
            return None;
        }
        let speaker = it.next()?.parse().ok()?;
        let seq = it.next()?.parse().ok()?;
        Some((speaker, seq))
    }

    /// Records a line sent by `speaker`: `certain` must receive it (the speaker's echo
    /// included), `optional` may.
    pub fn sent(
        &mut self,
        speaker: usize,
        seq: u32,
        kind: Kind,
        now: Instant,
        certain: Vec<usize>,
        optional: Vec<usize>,
    ) {
        self.counts.lines += 1;
        if matches!(kind, Kind::Channel(_)) {
            self.counts.channel_lines += 1;
        }
        self.lines.insert(
            (speaker, seq),
            Line {
                kind,
                sent: now,
                certain,
                optional,
                receipts: Vec::new(),
                rejected: false,
            },
        );
        self.order.push_back((speaker, seq));
        while self.order.len() > MAX_OPEN_LINES {
            if let Some(k) = self.order.pop_front() {
                self.judge(k, &|_, _, _, _| true);
            }
        }
    }

    /// Records that `recipient` received `text` at `now`. Returns false when the text is
    /// not one of this run's lines.
    pub fn received(&mut self, recipient: usize, text: &str, now: Instant) -> bool {
        let Some((speaker, seq)) = self.parse(text) else {
            return false;
        };
        let Some(line) = self.lines.get_mut(&(speaker, seq)) else {
            self.counts.unknown += 1;
            return true;
        };
        if line.receipts.len() < 4_096 {
            line.receipts.push((recipient, now));
        }
        let sent = line.sent;
        let last = self.last_seen.entry((recipient, speaker)).or_insert(seq);
        if seq < *last {
            self.counts.out_of_order += 1;
            self.problem("out-of-order", speaker, recipient, seq, sent, Some(now));
        } else {
            *last = seq;
        }
        true
    }

    fn problem(
        &mut self,
        what: &'static str,
        speaker: usize,
        recipient: usize,
        seq: u32,
        sent: Instant,
        at: Option<Instant>,
    ) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(ChatProblem {
                what,
                speaker,
                recipient,
                seq,
                sent_s: sent.saturating_duration_since(self.start).as_secs_f64(),
                delay_s: at.map(|a| a.saturating_duration_since(sent).as_secs_f64()),
            });
        }
    }

    /// The server rejected a line of `speaker` (`ChatRejected`, which names no line): the
    /// newest open line of that speaker whose echo has not arrived is marked rejected, and
    /// none of its recipients is judged. Returns false when no such line is open.
    pub fn rejected(&mut self, speaker: usize) -> bool {
        for k in self.order.iter().rev() {
            if k.0 != speaker {
                continue;
            }
            if let Some(line) = self.lines.get_mut(k)
                && !line.rejected
                && !line.receipts.iter().any(|(r, _)| *r == speaker)
            {
                line.rejected = true;
                self.counts.rejected += 1;
                return true;
            }
        }
        false
    }

    /// Judges every line sent at least [`JUDGE_AFTER`] ago. `stable(bot, from, to, kind)`
    /// says whether a recipient's situation held still from `from` to `to` for a line of
    /// `kind` (in the world the whole time; for Say also no teleport; for a channel no
    /// join or leave of it in flight or answered), which decides whether a certain
    /// recipient stays certain and whether an unlisted one may not receive the line.
    pub fn judge_due(
        &mut self,
        now: Instant,
        stable: &dyn Fn(usize, Instant, Instant, Kind) -> bool,
    ) {
        while let Some(&k) = self.order.front() {
            let due = self
                .lines
                .get(&k)
                .is_none_or(|l| now.saturating_duration_since(l.sent) >= JUDGE_AFTER);
            if !due {
                break;
            }
            self.order.pop_front();
            self.judge(k, stable);
        }
    }

    /// Judges every open line now (end of run).
    pub fn judge_all(&mut self, stable: &dyn Fn(usize, Instant, Instant, Kind) -> bool) {
        while let Some(k) = self.order.pop_front() {
            self.judge(k, stable);
        }
    }

    fn judge(&mut self, key: (usize, u32), stable: &dyn Fn(usize, Instant, Instant, Kind) -> bool) {
        let Some(line) = self.lines.remove(&key) else {
            return;
        };
        let (speaker, seq) = key;
        if line.rejected {
            self.counts.optional_received += line.receipts.len() as u64;
            return;
        }
        let kind = line.kind;
        let window_end = line.sent + DELIVERY_LIMIT;
        let window_start = line
            .sent
            .checked_sub(Duration::from_secs(1))
            .unwrap_or(line.sent);
        let mut by: HashMap<usize, Vec<Instant>> = HashMap::new();
        for (r, t) in &line.receipts {
            by.entry(*r).or_default().push(*t);
        }
        for &r in &line.certain {
            let times = by.remove(&r).unwrap_or_default();
            if !stable(r, window_start, window_end, kind) {
                if !times.is_empty() {
                    self.counts.optional_received += 1;
                }
                continue;
            }
            self.counts.expected += 1;
            match times.first() {
                None => {
                    self.counts.lost += 1;
                    self.problem("lost", speaker, r, seq, line.sent, None);
                }
                Some(&t) => {
                    let delay = t.saturating_duration_since(line.sent);
                    self.latency_ms.record(delay.as_secs_f64() * 1e3);
                    let mut ok = true;
                    if delay > DELIVERY_LIMIT {
                        self.counts.late += 1;
                        self.problem("late", speaker, r, seq, line.sent, Some(t));
                        ok = false;
                    }
                    if times.len() > 1 {
                        self.counts.duplicated += 1;
                        self.problem("duplicated", speaker, r, seq, line.sent, Some(t));
                        ok = false;
                    }
                    if ok {
                        self.counts.delivered += 1;
                    }
                }
            }
        }
        for &r in &line.optional {
            if let Some(times) = by.remove(&r) {
                self.counts.optional_received += 1;
                if times.len() > 1 && stable(r, window_start, window_end, kind) {
                    self.counts.duplicated += 1;
                    self.problem(
                        "duplicated",
                        speaker,
                        r,
                        seq,
                        line.sent,
                        times.first().copied(),
                    );
                }
            }
        }
        // Whoever is left received a line the rules exclude, unless its situation changed.
        let mut rest: Vec<_> = by.into_iter().collect();
        rest.sort_by_key(|(r, _)| *r);
        for (r, times) in rest {
            if stable(r, window_start, window_end, kind) {
                self.counts.unexpected += 1;
                self.problem(
                    "unexpected",
                    speaker,
                    r,
                    seq,
                    line.sent,
                    times.first().copied(),
                );
            } else {
                self.counts.optional_received += 1;
            }
        }
    }

    /// Lines not yet judged.
    #[must_use]
    pub fn open_lines(&self) -> usize {
        self.lines.len()
    }

    /// Whether any chat problem was seen.
    #[must_use]
    pub fn failed(&self) -> bool {
        let c = &self.counts;
        c.late + c.lost + c.duplicated + c.out_of_order + c.unexpected > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always(_: usize, _: Instant, _: Instant, _: Kind) -> bool {
        true
    }

    #[test]
    fn tags_round_trip() {
        let l = ChatLedger::new(0xabc, Instant::now());
        let t = l.text(3, 17, Kind::Yell);
        assert_eq!(l.parse(&t), Some((3, 17)));
        let other = ChatLedger::new(0xabd, Instant::now());
        assert_eq!(other.parse(&t), None);
        assert_eq!(l.parse("hello"), None);
        assert_eq!(l.parse("ltzz 1 2"), None);
        assert_eq!(l.parse("lt00000abc x 2"), None);
    }

    #[test]
    fn judges_every_case() {
        let t0 = Instant::now();
        let mut l = ChatLedger::new(1, t0);
        let ms = Duration::from_millis;
        // Line 0: 0 and 1 certain, 2 optional; 1 gets it twice, 2 gets it, 3 gets it unasked.
        l.sent(0, 0, Kind::Say, t0, vec![0, 1], vec![2]);
        let text = l.text(0, 0, Kind::Say);
        assert!(l.received(0, &text, t0 + ms(5)));
        assert!(l.received(1, &text, t0 + ms(6)));
        assert!(l.received(1, &text, t0 + ms(7)));
        assert!(l.received(2, &text, t0 + ms(8)));
        assert!(l.received(3, &text, t0 + ms(9)));
        // Line 1: 1 certain, never arrives; line 2: arrives late.
        l.sent(0, 1, Kind::Yell, t0, vec![1], vec![]);
        l.sent(0, 2, Kind::Tell, t0, vec![2], vec![]);
        let late = l.text(0, 2, Kind::Tell);
        l.received(2, &late, t0 + ms(2_500));
        // Out of order: line 0 again at 2 after line 2.
        l.received(2, &text, t0 + ms(2_600));
        assert!(!l.received(2, "not ours", t0));
        l.judge_due(t0 + ms(5_000), &always);
        assert_eq!(l.open_lines(), 3);
        l.judge_due(t0 + JUDGE_AFTER, &always);
        assert_eq!(l.open_lines(), 0);
        let c = &l.counts;
        assert_eq!(c.lines, 3);
        assert_eq!(c.expected, 4);
        assert_eq!(c.delivered, 1);
        assert_eq!(c.duplicated, 2, "{c:?}");
        assert_eq!(c.unexpected, 1);
        assert_eq!(c.lost, 1);
        assert_eq!(c.late, 1);
        assert_eq!(c.out_of_order, 1);
        assert!(l.failed());
        assert!(!l.problems.is_empty());
    }

    #[test]
    fn unstable_recipients_are_not_judged() {
        let t0 = Instant::now();
        let mut l = ChatLedger::new(2, t0);
        l.sent(1, 0, Kind::Yell, t0, vec![1, 2], vec![]);
        let text = l.text(1, 0, Kind::Yell);
        l.received(1, &text, t0);
        l.received(5, &text, t0);
        l.judge_all(&|bot, _, _, _| bot == 1);
        assert_eq!(l.counts.expected, 1);
        assert_eq!(l.counts.delivered, 1);
        assert_eq!(l.counts.lost, 0);
        assert_eq!(l.counts.unexpected, 0);
        assert_eq!(l.counts.optional_received, 1);
        assert!(!l.failed());
    }

    /// A channel line: members must receive it, and a bot whose membership held still
    /// outside the channel must not; one with a request in flight is not judged.
    #[test]
    fn channel_lines_reach_exactly_the_members() {
        let t0 = Instant::now();
        let mut l = ChatLedger::new(4, t0);
        let name = l.channel_name(2);
        assert_eq!(name, "lt00000004 ch2");
        l.sent(0, 0, Kind::Channel(2), t0, vec![0, 1], vec![2]);
        let text = l.text(0, 0, Kind::Channel(2));
        assert!(text.contains(" channel "));
        for r in [0, 1, 2, 3, 4] {
            l.received(r, &text, t0);
        }
        // Bot 3 held still outside the channel; bot 4 had a request in flight.
        l.judge_all(&|bot, _, _, kind| {
            assert_eq!(kind, Kind::Channel(2));
            bot != 4
        });
        let c = &l.counts;
        assert_eq!((c.lines, c.channel_lines), (1, 1));
        assert_eq!((c.expected, c.delivered), (2, 2));
        assert_eq!(c.unexpected, 1, "{c:?}");
        assert_eq!(c.optional_received, 2, "{c:?}");
        assert!(l.failed());
        assert!(Kind::Say.positional() && !Kind::Channel(0).positional());
    }

    #[test]
    fn rejected_lines_are_not_judged() {
        let t0 = Instant::now();
        let mut l = ChatLedger::new(3, t0);
        l.sent(0, 0, Kind::Tell, t0, vec![0, 1], vec![]);
        l.sent(0, 1, Kind::Tell, t0, vec![0, 2], vec![]);
        let first = l.text(0, 0, Kind::Tell);
        l.received(0, &first, t0);
        l.received(1, &first, t0);
        // The rejection names no line: the newest one without its echo is meant.
        assert!(l.rejected(0));
        assert!(
            !l.rejected(0),
            "line 0 was answered, line 1 is already marked"
        );
        assert!(!l.rejected(7));
        l.judge_all(&always);
        assert_eq!(l.counts.rejected, 1);
        assert_eq!(l.counts.lost, 0);
        assert_eq!(l.counts.delivered, 2);
        assert!(!l.failed());
    }
}
