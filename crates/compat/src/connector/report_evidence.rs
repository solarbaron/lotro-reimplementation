// SPDX-License-Identifier: GPL-3.0-or-later
//! The evidence fields of decision 0046 item 13's resource-set report `0x13D`
//! (`official-session.md` §9.3.11 "Item-13 report" row, §9.3.12 "Resource-set report source
//! boundary"): what the driver decided, the private sequence of each report-bearing datagram, every
//! transition of what the transport did with each of them and whether it is still retained, where
//! the report went relative to the first `0x6F`, whether a selection was held for it, and where the
//! first `0x149` after it arrived. Only the opt-in evidence target carries them
//! (`docs/DEVELOPMENT.md` "Reading a live official run"); none of them is a key, mask, label or DID.
//!
//! | Field | Values |
//! |---|---|
//! | `report_decision` | `sent`, `omitted` (with `report_omission`, a [`ReportOmission`] label), `undecided` |
//! | `report_reason` | `none` (undecided), `eligible`, `withdrawn`, `missing_route`, `malformed_mapper`, `cycle_or_depth`, `ambiguous_name`, `invalid_configuration`, `dictionary_mismatch`, `mount_mismatch`, `resource_bound`, `encoding_or_send_failure` |
//! | `report_sequence` | the private sequence of one report-bearing datagram: one record per datagram |
//! | `report_ack_action` | `cumulative_covered`, `nak_resend`, `out_of_order_ack`, `none` (with `direction`, `report_ack_from` and `report_retained`) |
//! | `report_position` | `earlier_datagram`, `same_datagram`, `later_datagram`, `no_6F`, `omitted`, `undecided` |
//! | `selection_held` | `true` when a selection arrived while the report was undecided |
//! | `first_149_after_report` | `none`, `after_report_before_6f`, `after_6f` |
//!
//! `report_ack_action` uses the spec's words. Its `direction` is `recv` for `cumulative_covered`
//! (the peer's Ack) and `send` for `nak_resend` (Tessera resent the saved bytes) and
//! `out_of_order_ack`. `out_of_order_ack` means only that Tessera sent an OutOfOrderAck because the
//! NAKed report sequence was no longer retained (after a covering cumulative Ack, for example). An
//! inbound OutOfOrderAck names the server's sequence space and is never a report action.
//!
//! `report_position` compares the datagram of the report's LAST fragment with the first `0x6F`'s
//! datagrams (§9.3.12: ALL fragments first in earlier datagrams). `earlier_datagram`, `no_6F` and
//! `omitted` are the spec's words. Engineering choices (spec silent): `same_datagram` stands for
//! both of the spec's `same_datagram_before_6F` and `same_datagram_after_6F`, because the driver
//! sees datagram sequences, not the order of siblings inside one datagram (an alias the remediation
//! §32 audit accepted); `later_datagram` is the value remediation §32 fix 6 asks the spec to add;
//! `undecided` is written at a close that came before the report's decision. `same_datagram` and
//! `later_datagram` must never happen: each blob goes in datagrams of its own, in queue order, and
//! the report is queued before any `0x6F`. They are named so a live log would show a breach.
//!
//! Engineering choice (spec silent): `first_149_after_report` counts a login `0x149` only when it was read in a later driver pass
//! than the flush that put the report on the wire, and calls it `after_6f` only when the first
//! `0x6F` was flushed in an earlier pass too: a `0x149` read in the same pass as the `0x6C` that
//! decided the report left the server before the report could reach it.
//!
//! Engineering choices (spec silent), beyond the §9.3.11 row: `report_reason="none"` for a report
//! not yet decided; the extra fields `report_ack_from`, `report_retained`, `report_datagram(s)` and
//! `login_retiring`; and up to three summary lines (`at=referral`, `at=login_close`, `at=close`), so
//! the report's fate stays visible after the referral and after the login's own close.
//!
//! [`ReportOmission`]: crate::resource_report::ReportOmission

use super::pending_world::ReportStatus;
use crate::resource_report::ReportOmission;
use crate::session::{KeptBlobAction, MAX_WATCHED_SEQUENCES};

/// Where the report's last datagram went relative to the first `0x6F`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportPosition {
    /// Every report datagram is earlier than the first `0x6F`'s: the rule.
    EarlierDatagram,
    /// A report fragment shares a datagram with the `0x6F`: must never happen.
    SameDatagram,
    /// A report fragment went in a later datagram than the first `0x6F`'s: must never happen.
    LaterDatagram,
}

impl ReportPosition {
    /// The evidence word.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EarlierDatagram => "earlier_datagram",
            Self::SameDatagram => "same_datagram",
            Self::LaterDatagram => "later_datagram",
        }
    }

    /// The position of a report whose datagrams are `report` against the datagrams `selection` of
    /// the first `0x6F` (the driver's own sequence space; in-flight spans are far below 2³¹, so the
    /// signed difference orders them). The report's LAST datagram decides.
    #[must_use]
    pub fn of(report: &[u32], selection: &[u32]) -> Option<Self> {
        let last = *report.last()?;
        let first = *selection.first()?;
        Some(if report.iter().any(|seq| selection.contains(seq)) {
            Self::SameDatagram
        } else if (last.wrapping_sub(first) as i32) < 0 {
            Self::EarlierDatagram
        } else {
            Self::LaterDatagram
        })
    }
}

/// Where the first `0x149` of the login connection arrived after the report was sent: a bounded
/// presence and order field (§9.3.12 "expected FIRST observable").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum First149 {
    /// None arrived after the report (or no report was sent).
    #[default]
    None,
    /// After the report and before the first `0x6F` was sent: the expected first observable.
    AfterReportBefore6F,
    /// After the first `0x6F` was sent.
    After6F,
}

impl First149 {
    /// The evidence word.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AfterReportBefore6F => "after_report_before_6f",
            Self::After6F => "after_6f",
        }
    }
}

/// The driver's record of the report's evidence.
#[derive(Debug, Default)]
pub struct ReportEvidence {
    /// The sequences of the report's datagrams, once queued (at most [`MAX_WATCHED_SEQUENCES`]).
    sequences: Vec<u32>,
    /// Its position against the first `0x6F`, once one went.
    position: Option<ReportPosition>,
    /// A first `0x6F` went (after the report or not).
    selection_sent: bool,
    /// A selection arrived while the report was undecided, and waited for its decision.
    selection_held: bool,
    /// The report's datagrams were flushed to the socket by an earlier driver pass.
    report_on_wire: bool,
    /// The first `0x6F`'s datagrams were flushed to the socket by an earlier driver pass.
    selection_on_wire: bool,
    /// The first `0x149` after the report.
    first_149: First149,
    /// The summary points already written ([`SummaryPoint`] bits).
    summarised: u8,
}

/// Where a summary line is written: each at most once, so at most three lines per login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryPoint {
    /// The valid referral, before the login retires.
    Referral,
    /// The retiring login connection's own close, which releases what it still kept.
    LoginClose,
    /// The session's end, which releases what is still kept.
    Close,
}

impl SummaryPoint {
    /// The evidence word (`at`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Referral => "referral",
            Self::LoginClose => "login_close",
            Self::Close => "close",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Referral => 1,
            Self::LoginClose => 2,
            Self::Close => 4,
        }
    }
}

impl ReportEvidence {
    /// The report was queued in the datagrams `sequences`.
    pub fn sent(&mut self, sequences: &[u32]) {
        self.sequences = sequences
            .iter()
            .copied()
            .take(MAX_WATCHED_SEQUENCES)
            .collect();
    }

    /// The private sequence of the report's first datagram.
    #[must_use]
    pub fn sequence(&self) -> Option<u32> {
        self.sequences.first().copied()
    }

    /// The private sequences of every report-bearing datagram.
    #[must_use]
    pub fn sequences(&self) -> &[u32] {
        &self.sequences
    }

    /// A selection arrived while the report was undecided: it was held for the decision.
    pub fn hold_selection(&mut self) {
        self.selection_held = true;
    }

    /// Whether a selection was held for the report.
    #[must_use]
    pub const fn selection_held(&self) -> bool {
        self.selection_held
    }

    /// A `0x6F` went in the datagrams `selection`: the position against the first one, `Some` only
    /// for the first `0x6F` after a sent report.
    pub fn selection_sent(&mut self, selection: &[u32]) -> Option<ReportPosition> {
        let first = !std::mem::replace(&mut self.selection_sent, true);
        if !first || self.position.is_some() {
            return None;
        }
        let position = ReportPosition::of(&self.sequences, selection)?;
        self.position = Some(position);
        Some(position)
    }

    /// The driver pass flushed what was queued to the socket: a report and a first `0x6F` queued
    /// before it are now on the wire, for the `0x149`s read in later passes.
    pub fn flushed(&mut self) {
        self.report_on_wire |= !self.sequences.is_empty();
        self.selection_on_wire |= self.selection_sent;
    }

    /// A `0x149` arrived on the login connection: `Some` with the new value the first time one
    /// arrives after the report was on the wire ([`Self::flushed`]), for its one evidence line. One
    /// read in the pass that queued the report is not after it and is not counted.
    pub fn note_149(&mut self) -> Option<First149> {
        if !self.report_on_wire || self.first_149 != First149::None {
            return None;
        }
        self.first_149 = if self.selection_on_wire {
            First149::After6F
        } else {
            First149::AfterReportBefore6F
        };
        Some(self.first_149)
    }

    /// Where the first `0x149` after the report arrived.
    #[must_use]
    pub const fn first_149(&self) -> First149 {
        self.first_149
    }

    /// The position word for a line now.
    #[must_use]
    pub fn position_label(&self, status: ReportStatus) -> &'static str {
        match (status, self.position) {
            (ReportStatus::Sent, Some(p)) => p.label(),
            (ReportStatus::Sent, None) => "no_6F",
            (ReportStatus::Omitted(_), _) => "omitted",
            (ReportStatus::Undecided, _) => "undecided",
        }
    }

    /// Whether the summary line of `at` is still to be written; `true` once per point.
    pub fn take_summary(&mut self, at: SummaryPoint) -> bool {
        let first = self.summarised & at.bit() == 0;
        self.summarised |= at.bit();
        first
    }
}

/// The `report_decision` word of `status`.
#[must_use]
pub const fn decision_label(status: ReportStatus) -> &'static str {
    match status {
        ReportStatus::Sent => "sent",
        ReportStatus::Omitted(_) => "omitted",
        ReportStatus::Undecided => "undecided",
    }
}

/// The `report_omission` word of `status`: the kind, or `none`.
#[must_use]
pub const fn omission_label(status: ReportStatus) -> &'static str {
    match status {
        ReportStatus::Omitted(kind) => kind.label(),
        ReportStatus::Sent | ReportStatus::Undecided => "none",
    }
}

/// The `report_reason` category of `status` (`official-session.md` §9.3.11 item-13 row). `none`
/// for an undecided report is an engineering choice (spec silent). The grouping of Tessera's omission kinds into the spec's categories is an engineering choice; the
/// finer kind stays in `report_omission`.
#[must_use]
pub const fn reason_label(status: ReportStatus) -> &'static str {
    match status {
        ReportStatus::Sent => "eligible",
        ReportStatus::Undecided => "none",
        ReportStatus::Omitted(kind) => match kind {
            ReportOmission::Withdrawn => "withdrawn",
            ReportOmission::NotRead | ReportOmission::RouteMissing => "missing_route",
            ReportOmission::WrongFamily
            | ReportOmission::DecodeError
            | ReportOmission::DuplicateKey
            | ReportOmission::BaseDefaultNotUnique
            | ReportOmission::ZeroKey
            | ReportOmission::LabelChanged
            | ReportOmission::NonAsciiLabel => "malformed_mapper",
            ReportOmission::CycleOrDepth => "cycle_or_depth",
            ReportOmission::AmbiguousLabel => "ambiguous_name",
            ReportOmission::ConfigEmpty
            | ReportOmission::ConfigNonAscii
            | ReportOmission::ConfigUnmatched
            | ReportOmission::ConfigMultiple
            | ReportOmission::ConfigOtherSet
            | ReportOmission::ConfigSpelling
            | ReportOmission::ConfigPlaceholder => "invalid_configuration",
            ReportOmission::DictionaryMissing
            | ReportOmission::DictionaryType
            | ReportOmission::DictionaryMismatch => "dictionary_mismatch",
            ReportOmission::NoMountedBase | ReportOmission::VariantMismatch => "mount_mismatch",
            ReportOmission::KeyOutOfRange => "resource_bound",
            ReportOmission::Encoding | ReportOmission::Enqueue => "encoding_or_send_failure",
        },
    }
}

/// The `report_ack_action` word of an action, `none` before the peer did anything.
#[must_use]
pub fn ack_action_label(action: Option<KeptBlobAction>) -> &'static str {
    action.map_or("none", KeptBlobAction::label)
}

/// The `direction` of a `report_ack_action` line (§9.3.11): the peer's cumulative Ack was received;
/// a NAK resend and an OutOfOrderAck are what Tessera sent.
#[must_use]
pub const fn ack_action_direction(action: KeptBlobAction) -> &'static str {
    match action {
        KeptBlobAction::CoveredByCumulativeAck => "recv",
        KeptBlobAction::NakResend | KeptBlobAction::OutOfOrderAck => "send",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_position_follows_the_last_report_datagram() {
        assert_eq!(
            ReportPosition::of(&[5], &[6]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(
            ReportPosition::of(&[4, 5], &[6]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(
            ReportPosition::of(&[6], &[6, 7]),
            Some(ReportPosition::SameDatagram)
        );
        assert_eq!(
            ReportPosition::of(&[5, 6], &[6, 7]),
            Some(ReportPosition::SameDatagram),
            "a later fragment shares the 0x6F's datagram"
        );
        assert_eq!(
            ReportPosition::of(&[5, 8], &[6, 7]),
            Some(ReportPosition::LaterDatagram),
            "the first fragment earlier is not enough"
        );
        assert_eq!(
            ReportPosition::of(&[7], &[6]),
            Some(ReportPosition::LaterDatagram)
        );
        // Across the wrap.
        assert_eq!(
            ReportPosition::of(&[u32::MAX - 1, u32::MAX], &[2]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(ReportPosition::of(&[5], &[]), None);
        assert_eq!(ReportPosition::of(&[], &[6]), None);
    }

    #[test]
    fn the_position_is_the_first_selection_only() {
        let mut e = ReportEvidence::default();
        assert_eq!(e.position_label(ReportStatus::Sent), "no_6F");
        assert_eq!(e.position_label(ReportStatus::Undecided), "undecided");
        e.sent(&[4]);
        assert_eq!(
            e.selection_sent(&[9]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(e.selection_sent(&[3]), None, "only the first 0x6F");
        assert_eq!(e.position_label(ReportStatus::Sent), "earlier_datagram");
        assert_eq!(
            e.position_label(ReportStatus::Omitted(ReportOmission::NotRead)),
            "omitted"
        );
        // A first 0x6F before any report has no position, and a later report gets none either.
        let mut f = ReportEvidence::default();
        assert_eq!(f.selection_sent(&[9]), None);
        f.sent(&[10]);
        assert_eq!(f.selection_sent(&[11]), None);
    }

    /// §9.3.12 "expected FIRST observable": the first `0x149` after the report is placed once, before
    /// or after the first `0x6F`; one before the report, or read before its flush, is not counted.
    #[test]
    fn the_first_149_after_the_report_is_placed_once() {
        let mut e = ReportEvidence::default();
        assert_eq!(e.note_149(), None, "before the report");
        e.sent(&[4, 5]);
        assert_eq!(e.note_149(), None, "queued, not yet on the wire");
        e.flushed();
        assert_eq!(e.note_149(), Some(First149::AfterReportBefore6F));
        assert_eq!(e.note_149(), None, "only the first");
        e.selection_sent(&[6]);
        e.flushed();
        assert_eq!(e.note_149(), None);
        assert_eq!(e.first_149().label(), "after_report_before_6f");

        let mut late = ReportEvidence::default();
        late.sent(&[4]);
        late.flushed();
        late.selection_sent(&[6]);
        assert_eq!(
            late.note_149(),
            Some(First149::AfterReportBefore6F),
            "the 0x6F queued in this pass is not on the wire yet"
        );
        let mut late = ReportEvidence::default();
        late.sent(&[4]);
        late.selection_sent(&[6]);
        late.flushed();
        assert_eq!(late.note_149(), Some(First149::After6F));
        assert_eq!(late.first_149().label(), "after_6f");
        assert_eq!(ReportEvidence::default().first_149().label(), "none");
    }

    #[test]
    fn a_held_selection_is_recorded() {
        let mut e = ReportEvidence::default();
        assert!(!e.selection_held());
        e.hold_selection();
        assert!(e.selection_held());
    }

    #[test]
    fn each_summary_point_is_written_once() {
        let mut e = ReportEvidence::default();
        assert!(e.take_summary(SummaryPoint::Referral));
        assert!(!e.take_summary(SummaryPoint::Referral));
        assert!(e.take_summary(SummaryPoint::LoginClose));
        assert!(!e.take_summary(SummaryPoint::LoginClose));
        assert!(e.take_summary(SummaryPoint::Close));
        assert!(!e.take_summary(SummaryPoint::Close));
        assert_eq!(SummaryPoint::LoginClose.label(), "login_close");
    }

    #[test]
    fn the_words_name_no_value() {
        assert_eq!(decision_label(ReportStatus::Sent), "sent");
        let omitted = ReportStatus::Omitted(ReportOmission::ConfigMultiple);
        assert_eq!(decision_label(omitted), "omitted");
        assert_eq!(
            omission_label(omitted),
            ReportOmission::ConfigMultiple.label()
        );
        assert_eq!(omission_label(ReportStatus::Sent), "none");
        assert_eq!(reason_label(ReportStatus::Sent), "eligible");
        assert_eq!(reason_label(omitted), "invalid_configuration");
        assert_eq!(
            reason_label(ReportStatus::Omitted(ReportOmission::Enqueue)),
            "encoding_or_send_failure"
        );
        assert_eq!(
            reason_label(ReportStatus::Omitted(ReportOmission::VariantMismatch)),
            "mount_mismatch"
        );
        assert_eq!(ack_action_label(None), "none");
        assert_eq!(
            ack_action_direction(KeptBlobAction::CoveredByCumulativeAck),
            "recv"
        );
        assert_eq!(ack_action_direction(KeptBlobAction::NakResend), "send");
        assert_eq!(ack_action_direction(KeptBlobAction::OutOfOrderAck), "send");
        assert_eq!(
            ack_action_label(Some(KeptBlobAction::CoveredByCumulativeAck)),
            "cumulative_covered"
        );
        assert_eq!(
            ack_action_label(Some(KeptBlobAction::OutOfOrderAck)),
            "out_of_order_ack"
        );
    }
}
