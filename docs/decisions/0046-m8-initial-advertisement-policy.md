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
   and to continue. On 2026-11-04 the owner extended this to the labelled evidence paragraphs of §9.3.9–§9.3.11. It is not treated as a contamination incident under `docs/CLEAN_ROOM.md` §10.
   Implementers still take no rule from a description the spec itself says is not a rule; the rules
   they need are stated as wire or observable facts.
5. **Omissions.** Not sending 0x13D, 0x19E and the first world RPC is accepted for any user past the
   warning and consent gate, as a labelled interim policy and a known difference from the observed
   client. Neither capture omits 0x13D or 0x19E, so the server's treatment of this is untested.
6. **Availability.** The conformance build merges without waiting for the character list to work,
   and the official login stays available. An account with characters may therefore use a login
   ticket and send the real advertisement, and then stop at an "unsupported" result until
   OPEN-WORLD-10 is answered. The result text must say this is a Tessera limit, not an account fault.
7. **Character create and delete** on the official service are left to the spec writer to specify
   or withhold. Nothing sends them until the spec gives a rule.
8. **Selection.** Any user past the warning and consent gate may send the character selection on the
   official service, even while the client's final ready message cannot be sent. A selection may
   therefore take a real character into the world on the server's side and end at a deadline, until
   OPEN-WORLD-9 and OPEN-SESSION-18 are answered. The server's reaction is unknown. The result text
   must say this is a Tessera limit, not a service outage.
9. **Further known differences.** Tessera opens no chat connection at login, though both captures
   show one, and it may repeatedly ask for a resend of a datagram it dropped for an undefined flag
   bit, for up to 30 seconds. Both are accepted and listed where a refusal is analysed.
10. **Later client messages before the final ready message.** Selection goes on while later world
    0x30, 0x30000035, 0x24 and 0x19A are not sent, for any user past the warning and consent gate, until
    a source for their contents exists. They are labelled known differences from the observed client,
    and the server's reaction is untested. Nothing is invented to fill them.
11. **Live-run logs.** The owner may give a live-run log to the builder session as well as to the spec
    writer. Logs describe official traffic, so this is a recorded exception to the clean-room boundary, not
    a contamination incident under `docs/CLEAN_ROOM.md` §10: a session that reads one is exposed, is listed
    under decision 0041, and code it writes afterwards is not strictly clean-room for M8. Logs stay out of the
    repository and carry no address, port, account, ticket, seed, cookie, character name or ID, install path,
    identifier or iteration value.
12. **Counts in the checks.** The per-stream replay counts of the owner's two private sessions, and the older
    figures of that kind in `docs/m8-remediation.md` and the session spec, may stay as published.
13. **Resource-set report (0x13D), owner 2026-11-06.** Any user past the warning and consent gate may
    send 0x13D after the character list and before the selection, with the install's default resource
    set as the only set in both the available and the loaded field. This is the candidate the spec writer
    found equal to the official client's report in both of the owner's captures
    (`official-session.md` "Resource-set report source boundary", OPEN-SESSION-20). It is a labelled,
    revocable exception to item 2: the set's key comes from the user's own install data, never from a
    capture, but the complete rule for when the official client counts a set as available or loaded is
    not yet sourced, so the report may be wrong for an install with other sets mounted or missing. When
    the default set cannot be resolved from the install by the spec's rules, or the install configures
    more than one set, 0x13D stays omitted under item 5. Nothing else is filled in, and a rejected attempt
    is not retried with other sets. The exception ends when the writer settles OPEN-SESSION-20, or earlier
    if the owner withdraws it.

## Consequences

- The interim header word and flow-control policies are known to differ from, or are not confirmed
  against, the official client (OPEN-SESSION-8, OPEN-SESSION-13). A live attempt under this decision
  may be rejected for those reasons, and the outcome does not identify which rule caused it. The
  account risk of that is the one the ban-risk warning describes.
- The clean-room deviation in item 4 is recorded here, so a later reviewer can see it was an owner
  decision and not an oversight. Sessions that read the text are listed under decision 0041.
- Spec pass 10 and the conformance build C8 follow this decision.
