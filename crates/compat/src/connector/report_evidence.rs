// SPDX-License-Identifier: GPL-3.0-or-later
//! The evidence fields of decision 0046 item 13's resource-set report `0x13D`
//! (`official-session.md` §9.3.12 "Resource-set report source boundary"; remediation §31 fix 8): what
//! the driver decided, the private sequence of its first transmission, what the transport did with
//! it, and where it went relative to the first `0x6F`. Only the opt-in evidence target carries them
//! (`docs/DEVELOPMENT.md` "Reading a live official run"); none of them is a key, mask, label or DID.
//!
//! | Field | Values |
//! |---|---|
//! | `report_decision` | `sent`, `omitted` (with `report_omission`, a [`ReportOmission`] label), `undecided` |
//! | `report_sequence` | the private packet sequence of its first transmission, `0` when not sent |
//! | `report_ack_action` | `covered_by_cumulative_ack`, `nak_resend` (with `report_nak_resends`), `out_of_order_ack`, `none_yet` |
//! | `report_position` | `earlier_datagram`, `same_datagram`, `later_datagram`, `no_6f_yet`, `not_sent` |
//!
//! `same_datagram` and `later_datagram` must never happen (§9.3.12 "Transmit the 13D before a
//! user-selected 6F", remediation §31 fix 3): each blob goes in datagrams of its own, in queue order,
//! and the report is queued before `Connected`. They are named so a live log would show a breach.
//!
//! [`ReportOmission`]: crate::resource_report::ReportOmission

use super::pending_world::ReportStatus;
use crate::session::{KeptBlobAction, KeptBlobFate};

/// Where the report's first datagram went relative to the first `0x6F`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportPosition {
    /// In an earlier datagram: the rule.
    EarlierDatagram,
    /// In the same datagram: must never happen.
    SameDatagram,
    /// In a later datagram: must never happen.
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

    /// The position of a report whose first datagram has sequence `report` against the datagrams
    /// `selection` of the first `0x6F` (the driver's own sequence space; in-flight spans are far
    /// below 2³¹, so the signed difference orders them).
    #[must_use]
    pub fn of(report: u32, selection: &[u32]) -> Option<Self> {
        let first = *selection.first()?;
        Some(if selection.contains(&report) {
            Self::SameDatagram
        } else if (report.wrapping_sub(first) as i32) < 0 {
            Self::EarlierDatagram
        } else {
            Self::LaterDatagram
        })
    }
}

/// The driver's record of the report's evidence.
#[derive(Debug, Default)]
pub struct ReportEvidence {
    /// The sequence of the report's first transmission, once queued.
    sequence: Option<u32>,
    /// Its position against the first `0x6F`, once one went.
    position: Option<ReportPosition>,
    /// The transport fate last logged.
    logged: Option<KeptBlobFate>,
    /// The one summary line was written (at the referral or the close).
    summarised: bool,
}

impl ReportEvidence {
    /// The report was queued with its first datagram at `sequence`.
    pub fn sent(&mut self, sequence: u32) {
        self.sequence = Some(sequence);
    }

    /// The private sequence of the report's first transmission.
    #[must_use]
    pub const fn sequence(&self) -> Option<u32> {
        self.sequence
    }

    /// A `0x6F` went in the datagrams `selection`: the position against the first one, `Some` only
    /// for the first `0x6F` after a sent report.
    pub fn selection_sent(&mut self, selection: &[u32]) -> Option<ReportPosition> {
        if self.position.is_some() {
            return None;
        }
        let position = ReportPosition::of(self.sequence?, selection)?;
        self.position = Some(position);
        Some(position)
    }

    /// The position word for a line now.
    #[must_use]
    pub fn position_label(&self, status: ReportStatus) -> &'static str {
        match (status, self.position) {
            (ReportStatus::Sent, Some(p)) => p.label(),
            (ReportStatus::Sent, None) => "no_6f_yet",
            _ => "not_sent",
        }
    }

    /// `fate` when it changed since the last call that returned one: the moment to log it.
    pub fn fate_changed(&mut self, fate: Option<KeptBlobFate>) -> Option<KeptBlobFate> {
        let fate = fate?;
        if self.logged == Some(fate) {
            return None;
        }
        self.logged = Some(fate);
        Some(fate)
    }

    /// The fate last seen by [`Self::fate_changed`] (the connection may be gone by the close).
    #[must_use]
    pub const fn last_fate(&self) -> Option<KeptBlobFate> {
        self.logged
    }

    /// Whether the summary line is still to be written; `true` once.
    pub fn take_summary(&mut self) -> bool {
        !std::mem::replace(&mut self.summarised, true)
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

/// The `report_ack_action` word of `fate`: its latest peer action, or `none_yet`.
#[must_use]
pub fn ack_action_label(fate: Option<KeptBlobFate>) -> &'static str {
    fate.and_then(|f| f.last)
        .map_or("none_yet", KeptBlobAction::label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_report::ReportOmission;

    #[test]
    fn the_position_orders_the_first_datagrams() {
        assert_eq!(
            ReportPosition::of(5, &[6]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(
            ReportPosition::of(6, &[6, 7]),
            Some(ReportPosition::SameDatagram)
        );
        assert_eq!(
            ReportPosition::of(7, &[6]),
            Some(ReportPosition::LaterDatagram)
        );
        // Across the wrap.
        assert_eq!(
            ReportPosition::of(u32::MAX, &[2]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(ReportPosition::of(5, &[]), None);
    }

    #[test]
    fn the_position_is_the_first_selection_only() {
        let mut e = ReportEvidence::default();
        assert_eq!(e.position_label(ReportStatus::Sent), "no_6f_yet");
        assert_eq!(e.selection_sent(&[9]), None, "no report yet");
        e.sent(4);
        assert_eq!(
            e.selection_sent(&[9]),
            Some(ReportPosition::EarlierDatagram)
        );
        assert_eq!(e.selection_sent(&[3]), None, "only the first 0x6F");
        assert_eq!(e.position_label(ReportStatus::Sent), "earlier_datagram");
        assert_eq!(
            e.position_label(ReportStatus::Omitted(ReportOmission::NotRead)),
            "not_sent"
        );
    }

    #[test]
    fn a_fate_is_logged_when_it_changes() {
        let mut e = ReportEvidence::default();
        assert_eq!(e.fate_changed(None), None);
        let fate = KeptBlobFate {
            first_sequence: 4,
            datagrams: 1,
            ..KeptBlobFate::default()
        };
        assert_eq!(e.fate_changed(Some(fate)), Some(fate));
        assert_eq!(e.fate_changed(Some(fate)), None);
        let resent = KeptBlobFate {
            nak_resends: 1,
            last: Some(KeptBlobAction::NakResend),
            ..fate
        };
        assert_eq!(e.fate_changed(Some(resent)), Some(resent));
        assert_eq!(ack_action_label(Some(fate)), "none_yet");
        assert_eq!(ack_action_label(Some(resent)), "nak_resend");
        assert_eq!(ack_action_label(None), "none_yet");
    }

    #[test]
    fn the_summary_is_written_once() {
        let mut e = ReportEvidence::default();
        assert!(e.take_summary());
        assert!(!e.take_summary());
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
    }
}
