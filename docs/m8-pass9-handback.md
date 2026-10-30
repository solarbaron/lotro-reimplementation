<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 pass 9 hand-back index

Branch: `m8-spec-pass-9`, originally created from `origin/main`. The branch and commits
of `m8-spec-pass-8` are neither amended nor deleted; its writer text is imported and
corrected on this branch. `73c0571` is retained, followed by a separate §17 writer commit.
The writer leaves all five specs in Review. Acceptance requires a separate coordinator
that wrote none of the pass to start new independent reviewer sessions.

## Audit §16

| Finding | Fix / answer |
|---|---|
| Blocking 1: unsupported neutral wire summary | Decision 0041 “Pass 9 writer remediation”; unsupported additions not imported; canonical spec references replace them. Source limitation recorded in research only. |
| Blocking 2: writer pre-recorded acceptance / reviewers | Five headers pending; decision 0041 pass-9 session table contains writer entries only. Archived pass-8 writer/research/reviewer identities are listed in Exposure without review outcomes. |
| Blocking 3: trailers | New writer commit uses sign-off and one final trailer paragraph with Assisted-by and Clean-Room-Role. Reviewer must do likewise. |
| Blocking 4: Case B arithmetic | Session §3.2; independently recomputed decimal and hexadecimal examples. |
| Blocking 5: decision 0044 | Login §2, §3.3, §6.2; explicit-port policy settled by decision 0044; official-launcher HTTP steps OPEN in §3. |
| Stale decoded chat ticket | Play §6.1.1, session §6.2.1. |
| Heading rounding | World §2.5.2 retains the E8/BIN rounding rule; world §5.5 and play §3.1/§4.2 reference it consistently. |
| Queue references and contradictory choices | Session §5.6.2 and §9.3; play §4.2, §4.5, §6.6.3. Non-wire group removed. |
| Client-internal wording / format string | Session §3.1–§3.2, §4.3, §6.2.1, §6.4, §8.2; world §4.3.1, §5.4; play §4.3.2, §4.5. The pass-8 reviewer row containing a format string was not imported. |
| Coordinates citation | World §2.5.2 cites coordinates §11.3–§11.4. |
| RPC signatures / decision 0042 item 10 | Play §5.1; documented layouts may be implemented after acceptance; undocumented signature extraction OPEN. |
| Dictionary-map DID mentions | Login §2.4, §2.4.3, §4.2, invariant I16, V7; root-key lookup, with observed/synthetic DIDs illustrative only. |
| Property OPEN count | Property header: §2.3 and §4.3; §10 has none. |
| State names | Session §8.2 matches §6.1 diagram/table. |
| Edited pass-7 review row | Session §13 restored exactly from origin/main; all prior change-log blocks preserved verbatim. |
| Provenance rows | Login §1.1–§1.3: tool version not recorded rather than guessed; launcher and in-game transport evidence separated, uncertainties OPEN. Session §2 P9-SYN; world provenance P9; play §2 P9. |

## First-datagram questions (remediation §15, L2)

| Question | Answer location |
|---|---|
| Iteration and table; sequence number | Session §4.2, §5.1 and §10.2 Vector 6. |
| Checksum buffer boundaries | Session §4.4.1–§4.4.2 and §10.2 Vector 6. |
| Network version from install | Login §2.4.1, §4.2; session §6.2 and Vector 6. |
| Exact versus padded authentication length | Session §6.2 and Vector 6. |
| Ticket text versus decoded bytes | Session §6.2.1; login §4.2; play §6.1.1. |
| Header time origin and initial time | Session §4.2 and Vector 6; clientTimestamp separately in §6.2. |
| Queue prerequisite with empty queue URLs | Login §3.5 and §5.1. |
| Bind interface, computed port and send buffer | Session §3.1–§3.2. |
| Authentication mode / account form | Session §6.2–§6.2.1; login §3.4 and §4.2. |
| ConnectAccept layout and bind / Case B | Session §6.3–§6.4, §3.2; Vector 5. |

## S1 blockers

| S1 item | Answer location |
|---|---|
| 1: appearance bit zero | World §2.8. |
| 2: localized argument key | World §2.6; property §10.4. |
| 3: checksums | Session §4.4.2. |
| 4: return to character selection | Play §7.2 and §8. |
| 5: movement mapping | Play §4.2 and §8; world §5.5. |
| 6: portal wait trigger | Play §4.5; world §4.3.3. |
| 7: Yell / Fellowship categories | Play §6.7. |
| 8: RPC signatures | Play §5.1 and §6.3–§7.1; decision 0042 item 10. |

Vectors 5 and 6 were independently recomputed by the writer, including all three complete
datagrams, header/payload sums, length fields and the challenge/accept byte reversal.
This synthetic verification does not claim a successful official-server session.

## Additional writer checks

Session §3.2 now distinguishes the confirmed Case B candidate progression from OPEN
exhaustion behavior (OPEN-SESSION-7 in §12); the finite attempt cap is explicitly Tessera's
resource policy. Session §11 and login §6.4 state research exclusions without claiming
that an excluded mechanism is absent. Login §4.3, I9 and I10 use timer samples consistent
with the strict trigger. Play §6.7 delegates enum-mapper layout to records §7.1.

## Origin/main remediation §17

| Finding | Correction |
|---|---|
| 1: heading observation/provenance | World §2.5.2 restores the prior E8/BIN rounding rule; provenance P9 and world §5.5/play §3.1/§4.2 match it. No truncation observation claimed. |
| 2: incomplete private waiting flag | Play §4.5 consistently uses the current stamp reaching/passing the expected stamp under modulo-256 ordering. |
| 3: queue citations | Session §5.6.2 cites BIN-05/BIN-08 for the queue and sequence-group numbers; removed non-supporting section citations. |
| 4: BIN-Q5 certainty | Login §1.2 BIN-Q5 says scheme/agent/proxy details are reported and OPEN, matching §3. |
| 5: pass-8 attribution/exposure | Decision 0041 Exposure lists the archived writer, research sessions and historical reviewer identities without outcomes; remediation text credits Gemini/Antigravity imported authorship. |
| Split provenance table | Session §2 joins P9-SYN, P9-BIND and P9-17 to the existing table. |
| Function-call/internal wording | Play §8; session §4.2, §4.4.2 and §6.4 now describe values, argument order and arithmetic in prose. |
| Ordinary post-handshake time | Session §4.2/§12 OPEN-SESSION-8 explicitly records the unsupported case; 6 session OPENs total. |
| HTTP answer provenance | Login §2 includes decision 0044 item 2. |
| Extra chat-auth alternative | Play §6.6.2 and §9.2 defer to session §6.2.1; no extra receiver alternative. |
| Property change-log columns | Property §18 appends an attribution/column erratum with sections in the proper column, preserving the earlier row as history. |
| Ambiguous pass-8 preservation | Opening paragraph distinguishes the untouched pass-8 branch/history from corrected imported spec text. |

The continuation writer's session ID remains not recorded because the host does not expose it.

Additional consistency checks in the same §17 writer commit distinguish 0xCC authoritative
correction (no teleport stamp) from 0xCE stamped placement in play §4.5 and world §4.3.3;
remove the remaining wait-flag/Tell/NAK call notation in play §8 and session §5.2; join
login P14–P16 to their table; and align login §5.1–§5.2 timer summaries and runtime-URL
descriptions with §4.3 and the §2 address policy. No new binary observation is claimed.

World §5.1's batch-target summary now matches the canonical distinction between explicit-ID
0xCE and implicit-target 0xCC; play's owner scenario makes the same stamp distinction as
§4.5. World provenance BIN/C9 identifies 0x19E consistently with canonical world §3.4.
