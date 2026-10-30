<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Pass 9 writer reconciliation (dirty room only)

Date: 2026-10-30. Codex / GPT-6, one writer session, host session ID not
recorded (not exposed); task path /root. No subagents started by the writer.

Starting material: writer diff e26848b..2bc806a, the five Stage 1 specs,
decisions 0034/0041/0042/0044, and remediation §1–§6, §15–§16. The held
incident-0041 text was not opened. Existing research notes were searched for
handshake and launcher evidence; that search did not produce corroborating
notes for the contradictory pass-8 decision summary. Existing pass8-w2-session,
pass8-w3-world and pass8-w4-play scratch directories contain no files at the
inspected top level. No claim of a new binary derivation or live capture is made.

## Unsupported decision summary

The pass-8 neutral summary claims a 12-byte little-endian datagram header,
checksum seed 0x00B704CE, challenge bytes +0x06..+0x0D, V1A–V6 and a 6-second
flush. The writer has no observation supporting those claims and cannot establish
where Gemini got them. They are not imported. Specs instead retain the supported
20-byte big-endian header, checksum placeholder 0xBADD70DD, challenge bytes
+0x08..+0x0F, Vectors 1–6 and 5-second application-exit flush. See session
§4.2, §4.4.2, §6.4, §8.2 and §10.2. Remaining OPENs are 1, 2, 4 and 6.

## Synthetic checks performed

A scratch Python probe independently decoded the complete hexadecimal datagrams
in session §10.2 Vectors 5 and 6, using the checksum algorithm in §4.4.1.
It asserted the payload length, sums and on-wire checksum for all three packets:

| Packet | Total bytes | Header sum | Payload sum | Total sum |
|---|---|---|---|---|
| Vector 5 challenge | 66 | 0x0970E1BA | 0x103DC96D | 0x19AEAB27 |
| Vector 5 accept | 50 | 0xFA70E9BD | 0x13233747 | 0x0D942104 |
| Vector 6 first datagram | 139 | 0x5370DEBA | 0xB4C43647 | 0x08351501 |

The probe also checked the challenge time (1000.0), byte-reversed checkValue,
63-byte version string, exact 49-byte auth block, primary authType, sequence 1,
time 0 and unchanged synthetic ticket text. Vector 6's ticket is deliberately
opaque transport text; the transport does not Base64-decode it.

Case B: 3221225994 remainder 64007 is 9712 (0x25F0). Add 256: 9968
(0x26F0), then 10224 (0x27F0). Wrap example: 65520 + 256 modulo 65536
is 240, clamped to 1024. These computations do not establish socket behavior;
the underlying Case B rule retains pass-8 BIN-01 provenance.

## Source limits and corrections

Heading encoding in world §2.5.2 is aligned to truncation already stated by world
§5.5 and play §3.1/§4.2, rather than inventing a second rounding rule. This pass
makes no claim of a new binary measurement of heading conversion.

Decision 0044 itself settles the explicit port: do not carry it to the HTTPS
attempt; preserve it on the original HTTP retry. Launcher per-step HTTP use and
User-Agent/proxy behavior are OPEN because strings/imports alone cannot establish
requests or service acceptance. The login research log did not establish which
Ghidra version was used, so pass 9 records it as not recorded instead of guessing.

Undocumented RPC signature extraction remains OPEN. The removed bytecode
call-sequence description is not a functional algorithm suitable for the interface.
Documented layouts remain directly usable after acceptance under decision 0042
item 10. No script method names or source text are copied here.

Earlier change-log blocks were restored verbatim from origin/main; only the
pass-8 writer row and a pass-9 writer row were appended. No reviewer is recorded
for this pass by the writer.

## Further writer checks

The Case B recurrence does not necessarily return to its seed after clamping. A synthetic
simulation with seed 9712 reaches 1024 at increment 219 and again at 471, without revisiting
9712. The original claim of official termination within 256 attempts is withdrawn.
OPEN-SESSION-7 records official exhaustion behavior as unconfirmed. Tessera's own finite
256-attempt limit is an engineering resource bound, not a discovered official-client fact.

The strict refresh comparison is retained. Failed refreshes sampled at ages 10800.5,
16200.5 and 18900.5 update next offsets to 16199.75, 18899.5 and 20249.25. Exact equality
does not fire. A success sampled at 16200.5 resets the origin; age 10800 is reached at
27000.5 and still does not fire. These are synthetic arithmetic examples, not observations.

Play §6.7's header description had put a TSize hint before the base DID; it now delegates
to records §7.1, preserving canonical data DID, base DID, then names-table TSize order.
Research-exclusion clauses state policy and do not assert absence of excluded mechanisms.

## Origin/main §17 continuation (2026-10-30)

Same Codex/GPT-6 writer conversation; host session ID not recorded. No new binary
observation is claimed. The earlier truncation reconciliation was unsupported by E8:
world §2.5.2 now restores the E8/BIN add-one-half-and-floor rule from origin/main,
and world §5.5/play §3.1/§4.2 defer to it. This supersedes this note's earlier
truncation explanation. Synthetic heading checks include all 256 decode/encode
round trips and a half-step boundary; these check the stated arithmetic only.

The header time field for ordinary post-handshake packets with neither timing
header was not established by the recorded observation. It is OPEN-SESSION-8 rather
than an inferred zero. Existing initial-handshake and timing-header rules remain.
The queue/group assignments retain BIN-05/BIN-08 provenance, with no new observation.
BIN-Q5 certainty is reduced to match the OPENs about per-request launcher behavior.
The copied pass-8 text is credited to its Antigravity/Gemini writer, and all archived
writer/research/reviewer identities are restored as exposure only, with no outcomes.
Property §18 appends an erratum instead of rewriting the earlier malformed-column row.

Further consistency pass: canonical world §4.3.1 lists no stamp in 0xCC, so the
play §4.5 claim that this event supplies a newer teleport stamp is removed. The
stamp sources remain 0x5E and 0xCE; world §4.3.3 distinguishes stamped placement
from authoritative correction. This correction follows existing field tables,
not a newly claimed binary observation. Remaining Tell/NAK call notation and
private wait-flag wording are replaced with wire argument order and stamp conditions.
Login's provenance-table break and exact-time summary shorthand are also corrected.

Canonical layout cross-check: world §5.4 0xCE explicitly has an entity_id; world
§4.3.1 0xCC does not. The batch-target summary now applies implicit routing to
0xCC rather than listing it as an explicit-ID exception. Play's owner scenario
likewise distinguishes 0xCE stamps from 0xCC correction. World provenance labels
for 0x19E now follow the canonical §3.4 client-to-server platform report.
