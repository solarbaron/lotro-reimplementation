<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M8 plan: official-server compatibility

The step-by-step plan from today's state to M8's exit criteria ([ROADMAP M8](ROADMAP.md#m8)).
The binding conditions are in [ROADMAP M8 conditions](ROADMAP.md#m8-gate), [decision
0034](decisions/0034-official-server-compatibility.md) and [decision
0042](decisions/0042-m8-remediation-decisions.md); this plan does not repeat them. Written
2026-10-26; update the status column as steps finish.

## Roles

| Role | Who | Does |
|---|---|---|
| Builder | Claude Code sessions (clean room) | Tessera code in `crates/`, `apps/`, `fuzz/`, `tools/`; reviews and audits; merges and gates |
| Spec writer | The owner's other AI (dirty room) | The official-protocol specs; answers to spec questions; never Tessera code |
| Owner | The owner | Decisions, and every contact with an official server (exit criterion 4) |

Builders never do official-protocol research, never read `research/` and never connect to an
official server. Every spec change goes through a writer commit, a header-only reviewer commit
by new sessions, a default-mode audit and a squashed merge ([`m8-remediation.md`](m8-remediation.md)
§12–§14).

## Where it stands (2026-10-26)

| Part | State |
|---|---|
| Stage-1 specs: `official-login.md`, `official-session.md`, `official-messages-world.md`, `official-messages-play.md`, `property-system.md` §10 | Accepted (2026-10-25); about 50 open questions from the build in [`m8-remediation.md`](m8-remediation.md) §15 |
| Stage-2 spec: `official-messages-gameplay.md` | Held (incident 0041); to be re-derived |
| `official-entry.md` (start screen, consent gate, account form, world list, queue, errors) | Accepted (rev 6), built |
| `crates/compat` `login`, `session`, `world` | Conformed to the specs (`m8-conform`), on main |
| `crates/compat` `play`, `chat_session` | Built (`m8-play`), on main |
| Fuzz coverage of `crates/compat` | All parsers covered (`m8-conform-notes`), on main |
| Live connector (`crates/compat/src/connector/`, HTTPS on ureq/rustls) | Merged 2026-10-26 after four fix rounds (C1) |
| Hand-over of an official session to the character screens | Not built; the client shows a placeholder |
| `availability()` | `NotBuilt`: `--official` stops before any network use |
| Live verification (exit criterion 4) | Not started |

## Step list

IDs are stable; "after" lists hard dependencies.

### C — Core build (builder)

| ID | Step | After | Done when |
|---|---|---|---|
| C1 | Merge `m8-connector`: full gate, push, CI green | — | on main, CI 17/17 |
| C2 | `m8-connector-support`: `SessionMultiplexer::next_deadline` (the worker waits for the next timer); a neutral "world loading done" command in `tessera-net`, so 0xF4 follows `official-messages-play.md` §4.5 step 4 instead of the interim rule; no install paths in `crates/install` logs | C1 | reviewed, audited, merged |
| C3 | Character-screen hand-over (`official-entry.md` §8.7): the client runtime, `net.rs`, `pre_world.rs` and `app.rs` take a session from the connector; the character list from 0x6C fills the selection screen; select and enter world (0x6F → 0x90); an entry failure returns to the list. Includes the return from the world to character select, which `tessera-net` cannot express today (a `Connected` event outside `Connecting`): a small domain change, or the spec answer of S1 | C1 | a synthetic-server test walks start screen → login → list → world |
| C4 | In-world feed from the official session: entity create and despawn (0x98/0x40, world §5.2.3), remote motion into the walker, chat lines into the chat window, 0xCC state schemas from the install (0x07000029 scriptlets), chat categories from the install's enums, string-table text | C3; parts wait for S1 | the walker shows server entities and chat from a synthetic server |
| C5 | Turn on `availability()` (one line plus the OE-T9 test) | C3 (C4 for a useful world) | `--official` reaches the login form in a release build |
| C6 | Optional, owner decides: a diagnostic mode that logs in, lists worlds, passes the queue and completes the UDP handshake to the character list, then prints only world names and a character count; same consent gate | C1 | lets L1–L2 run before C3 |
| C7 | Conformance package after each spec pass (S1, S2): code follows the changed rules | each S pass | merged |

### S — Spec work (spec writer)

| ID | Step | After | Done when |
|---|---|---|---|
| S1 | Answer the questions in [`m8-remediation.md`](m8-remediation.md) §15 that block the build or the first live run (list below) | — | Accepted after review and audit, merged |
| S2 | Answer the rest of §15 (timeouts, queue numbers, ID rules, wording, citations) | S1 | as S1 |
| S3 | Re-derive `official-messages-gameplay.md` by writer sessions that never read the held text; audit on its own (decision 0042 item 8) | — (parallel to S1) | Accepted, audited, merged; incident 0041 closed for stage 2 |

Blocking questions for S1:

1. `EntityAppearanceDesc` bit 0 (`model_b`): no character-list entry with a base model can be
   decoded (world §2.8).
2. `LocalizedTokenText` wire argument key: no text with arguments can be decoded (world §2.7).
3. Send vs receive checksum rule (session §4.4.2): the code follows the sender rule; the
   server may check the other.
4. Return from the world to character select (play §8, the `Connected` mapping).
5. Neutral move command to 0x24's `move_state` and input axes (play §4.2, §8).
6. What starts a portal-space wait (play §4.5 step 2).
7. Chat category numbers for Yell and Fellowship (play §6.7).
8. Play §5.1 vs §6.3–§7.1 (RPC signatures; decision 0042 item 10).

### L — Live verification (owner; exit criterion 4)

Each step is one run with the owner's own account in a release build, after the ban-risk
warning. The owner records the result in `docs/checklists/owner-observations.md`. Logs stay
local; captures are never committed. A failure goes back to the spec writer as a question;
builders cannot analyse official traffic.

| ID | Step | Needs | First things that may fail |
|---|---|---|---|
| L1 | Login and world list over HTTPS | C6 or C5 | the install's endpoint chain, the SOAP exchange |
| L2 | Queue and UDP handshake to the character list | L1 | `checkValue` (session §6.4), the checksum rule (S1 item 3), the codec |
| L3 | Enter the world | L2, C3 | the referral, 0x90, portal space and 0xF4 |
| L4 | Move | L3, C4 | the 0x24 cadence and mapping, 0xCC corrections |
| L5 | Chat (say, tell, a channel) | L3, C4 | the chat-server session, categories |
| L6 | Log out | L3 | the 20 teardown opcodes |
| L7 | Server-driven P-track windows | G2 | stage-2 messages |

### G — Gameplay messages (stage 2)

| ID | Step | After | Done when |
|---|---|---|---|
| G1 | Re-check the prepared packages `m8-gameplay-a` and `m8-gameplay-b` against the new spec | S3 | task files current |
| G2 | Build both in parallel (P1–P6 groups mapped to `tessera_net::domain`); review, fix, audit | G1 | merged, all parsers fuzzed |
| G3 | Live run of the P-track windows (L7) | G2, L3 | owner records the result |

### E — Close-out

| ID | Step | After |
|---|---|---|
| E1 | Update the M8 exit-criteria table in the roadmap with evidence (1: specs Accepted; 2: content clean; 3: fuzz coverage; 4: owner's record) | L1–L7 |
| E2 | Close incident 0041 completely; update `CLAUDE.md` | S3, E1 |
| E3 | M8 done; M9 becomes eligible once P8's owner items are closed too | E1, E2 |

## Order

```
C1 ──┬─ C2
     ├─ C6 (optional) ── L1 ── L2
     └─ C3 ── C4 ── C5 ── L3 ── L4, L5, L6
S1 ──┬─ C7 ── (feeds C4 and L2)
     └─ S2 ── C7
S3 ── G1 ── G2 ── L7 (G3) ── E1 ── E2 ── E3
```

The longest chain is C1 → C3 → C4 → C5 → L3–L6, with S1 alongside: it feeds C4 and decides
whether L2 passes. S3 and G can run in parallel to all of it.

## Risks

- **The account.** A third-party client may break the service's terms. The ban-risk warning
  and the consent record stay in front of every network use (decision 0034 §1 item 4).
- **Unconfirmed rules.** `checkValue`, the checksum rule, delivery queues and several
  timeouts are engineering readings until L2–L6 confirm them.
- **Stop condition.** If the service can only be used by getting around anti-cheat,
  anti-tamper or another access control, M8 stops there and records why.
- **Spec turnaround.** Every answer needs a writer pass, a review and an audit; questions are
  batched (S1 before S2) so the build is not held up by low-value ones.
