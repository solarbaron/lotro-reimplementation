<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0046 — M8: the initial install advertisement under interim policy P10-10E

**Status:** Accepted (owner, 2026-11-03).

Applies to `official-session.md` §9.3.2 and §9.3.4 on spec pass 10 (`docs/m8-remediation.md`
§23.4 and §24). Works with [decision 0034](0034-official-server-compatibility.md), whose limits
are unchanged, and with [decision 0041](0041-incident-m8-spec-contamination.md).

## Context

The client's first application message on the login connection advertises the install's archive
metadata and the selected language. The official rules for which archives are listed, and in what
order, are not fully settled. Spec pass 10 proposes an interim policy, P10-10E, that lets Tessera
send this message from the user's real install. The spec text called the policy both "offline only"
and "defensible for an owner-authorized attempt", and no record settled who may use it. An auditor
also reported that new prose in §9.3.4–§9.3.6 describes how the official client works internally,
not only wire or observable facts.

## Decision

1. **Live use.** Any user who has passed the ban-risk warning and the consent gate
   (`official-entry.md`) may send the message P10-10E permits to the official service. It is not
   limited to offline runs or to the owner's runs.
2. **Truthful data only.** The message is built from the user's real mounted archives and the
   language actually selected. Nothing is invented, altered or substituted, and a rejected attempt
   is not retried with varied data. When the policy cannot build a truthful message, the client
   reports the step as unsupported.
3. **Order.** Entries are sent in ascending remainder of the identifier divided by 23, with a
   labelled Tessera tie-break. This is an interim relation that fits the owner's two captures, not
   the official algorithm. The spec names the exact integer and the tie-break.
4. **The flagged prose stays.** The owner, as maintainer, decided to keep the wording of §9.3.4–§9.3.6
   and to continue. It is not treated as a contamination incident under `docs/CLEAN_ROOM.md` §10.
   Implementers still take no rule from a description the spec itself says is not a rule; the rules
   they need are stated as wire or observable facts.

## Consequences

- The interim header word and flow-control policies are known to differ from, or are not confirmed
  against, the official client (OPEN-SESSION-8, OPEN-SESSION-13). A live attempt under this decision
  may be rejected for those reasons, and the outcome does not identify which rule caused it. The
  account risk of that is the one the ban-risk warning describes.
- The clean-room deviation in item 4 is recorded here, so a later reviewer can see it was an owner
  decision and not an oversight. Sessions that read the text are listed under decision 0041.
- Spec pass 10 and the conformance build C8 follow this decision.
