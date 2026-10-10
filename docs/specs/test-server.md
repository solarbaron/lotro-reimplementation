<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Test server (`tessera-server`) and load tools

| | |
|---|---|
| **Status** | **Revised, changes requested** (independent review 2026-10-10 of the fourth revision of 2026-10-10, commit a5c56b1: one blocking item, the §11.1 "code and reason" field names the `CONNECTION_CLOSE` that ended the connection, but under `tessera-protocol.md` §12.3 a server-initiated close normally ends with the peer's `NO_ERROR` close, so the listed server causes would not appear and the §15.3 gap misdescribes the common case; review notes (a)–(c) of the previous review are answered; see the review record). Before: Revised (2026-10-10, fourth revision of the day, spec writer: answers the blocking item of the review of the third revision; §11.1 keeps the closed line's "code and reason" field as defined and states which end sent the close, §15.3 lists the server's missing code and reason for closes it initiates as an open gap, the provenance row and the third revision's change-log row are narrowed to the fields the server writes; review notes (a)–(c) folded in: the closed line's duration is defined from the arrival of the connection attempt to the writing of the line, invariant 10 is phrased on the field table's terms, the connection's slots are released when the line is written; no wire rule, limit or default changes; awaiting review, see the change log). Before: **Revised, changes requested** (independent review 2026-10-10 of the third revision of 2026-10-10, commit c19c0cd: one blocking item, §15.3 and the provenance row claim the server already writes the closed line's code and reason as §11.1 states, which it does not for closes the server initiates; see the review record). Before: Revised (2026-10-10, third revision of the day, spec writer: §11.1 lists the "connection closed" line's fields normatively and adds the bound of 1 s on waiting for the connection's stream writers before that line; §14 invariant 10; §15.3 marks the closed line's byte counts closed (commit a744d7e); no wire rule, limit or default changes; awaiting review, see the change log). Before: **Accepted** (independent review 2026-10-10 of the second revision of 2026-10-10, commit 0afa1db: CLEAN_ROOM.md §5.3 passed, every item; §11.1 and §15.3 agree with `tessera-server` as stated; see the review record). Before: Revised (2026-10-10, spec writer: §15.3 gap list updated, the statistics line's resident memory is no longer a gap and the "connection closed" line's byte counts still are; §11.1 states that the closed line's bytes are per-connection totals counted like the statistics line's, and folds in the review notes (a) (interval boundaries are the start times of the ticks that write the lines) and (b) (the kind byte of a unidirectional stream is not counted); no rule of the server changes; awaiting review, see the change log). Before: **Accepted** (independent review 2026-10-10 of the revision of 2026-10-10: CLEAN_ROOM.md §5.3 passed, every item; §11.1 statistics line and §14 invariant 9 agree with `tessera-server` since commit 9ce64c8; §4.1 and §8.3 agree with `tessera-protocol.md` §8.3 as revised and accepted 2026-10-10; see the review record). Before: Revised (2026-10-10, spec writer: §11.1 statistics line defined exactly (interval, longest tick reset per line, byte rates), §3.1 content refusal status, §4.1 and §8.3 parameters fixed for a run and the protocol's new rules for changes and out-of-bound values, §8.3 run-out OPEN restated with its consequence and test, §14 invariant 9, §15.1 rows brought up to date, reviewer row filled in; ready for review, awaiting re-review; see the change log). Before: **Accepted** (2026-10-09: independent whole-spec re-review of the revision of 2026-10-09 passed CLEAN_ROOM.md §5.3, every item; first whole-spec acceptance; see the review record). Before: Revised (2026-10-09: the blocking item of the whole-spec review of 2026-10-09 is fixed, §4.6 `log_chat` row moved into the `[chat]` table; non-blocking (a), (b), (d), (e) also addressed, (c) noted; awaiting re-review, see the change log). Before: Review, **changes requested** (independent whole-spec review 2026-10-09, one blocking item, see the review record). Before that: Review (2026-10-09: prepared for its first whole-spec review; every implementer note of §15.1 and §15.2 folded into the text or kept as a stated OPEN or implementation gap; provenance and cross-references checked against `tessera-protocol.md` v1.0 as revised 2026-10-09) |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (dirty room; the content is design, plus one terrain height observed for the default spawn, §1) |
| **Reviewer** | Claude review session in the spec-reviewer role (did not write this spec, its revisions or its code), whole-spec reviews 2026-10-09 (changes requested, then passed on re-review); partial review 2026-10-08; see the record. The revision of 2026-10-10 was reviewed 2026-10-10 by another Claude review session in the same role (passed) |
| **Review record** | Review 2026-10-10 of the fourth revision of 2026-10-10 (commit a5c56b1; `git show a5c56b1` of this file: header, two provenance rows, §11.1 closed-line fields "duration" and "code and reason" and the drain paragraph, §14 invariant 10, §15.3, two §16 rows) by a Claude review session in the spec-reviewer role that wrote neither this spec, its revisions nor the code, against CLEAN_ROOM.md §5.3: **changes requested**, one blocking item. Checklist: functional facts only (field definitions, release point of the limits, invariant wording, a gap statement); no code or pseudo-code; no copied prose; no game text, keys, hostnames or traffic; the new Design row covers the new facts and the third revision's row is narrowed; invariant 10 testable on its stated condition; no closed gate (private and LAN test server, decision 0004). All pass except the accuracy of the field rule and gap below. Checked against the clean `tessera-server` (read only to compare): the duration starts when the connection task takes up the incoming attempt, before the handshake, and ends after the writer wait (note (a) answered); the `max_connections` and per-address places and the statistics connection count are released after the wait, immediately before the line (note (c) answered); a closed line is written only past a completed handshake (invariant 10); the line's cause text carries the application code and reason only for a peer's close and a generic local-close text otherwise; `tessera-protocol.md` §12.3 and §14.1 (codes `FLOODING`, `SLOW_CONSUMER`, `SERVER_FULL` exist). **Blocking:** (1) §11.1 now defines "code and reason" as those of the `CONNECTION_CLOSE` that ended the connection and gives a kick, `SERVER_FULL`, `SLOW_CONSUMER`, `FLOODING` and `SERVER_SHUTDOWN` as examples of the server's own code appearing. Under `tessera-protocol.md` §12.3 the server ends these with `Disconnect(code)` on the control stream (step 1) and a conformant peer answers by closing with `NO_ERROR` (step 2); the server sends its own `CONNECTION_CLOSE` with the code only after the 2 s linger (step 3) or when it skips step 1 (step 4). So for the normal kick or refusal the literal rule logs the peer's `NO_ERROR`, and the examples contradict the rule. The same applies to §15.3: the current server, for a kick acknowledged by the peer, logs the peer's `NO_ERROR` close (code and reason present, cause lost), and only the linger and immediate-close paths give the generic local-close text, which the new gap entry does not distinguish. State which cause the field carries when a `Disconnect` was sent or received before the close (for example the precedence `tessera-protocol.md` §12.3 gives the client: the `Disconnect` code and text first, then the `CONNECTION_CLOSE` code and reason, then the idle timeout, then the local error), and restate the §15.3 gap for both paths. Earlier: Review 2026-10-10 of the third revision of 2026-10-10 (commit c19c0cd; `git show c19c0cd` of this file: header, one provenance row, §11.1 "connection closed" line and drain bound, §14 invariant 10, §15.3, §16 row) by a Claude review session in the spec-reviewer role that wrote neither this spec, its revisions nor the code, against CLEAN_ROOM.md §5.3: **changes requested**, one blocking item. Checklist: functional facts only (the closed line's fields and units, the 1 s bound, what counts after it); no code or pseudo-code (field table and prose); no copied prose; no game text, keys, hostnames or traffic; the new Design row covers the new facts; invariant 10 testable; no closed gate (private and LAN test server, decision 0004). All pass except the provenance and gap statement below. Checked against the clean `tessera-server` (read only to compare) and its QUIC library's error texts: one closed line per connection past the handshake; per-connection byte totals counted at the same points as the server totals (control-stream reads and datagrams in, frames and datagrams handed to QUIC out, kind byte excluded); the writers of the connection awaited at most 1 s in that connection's own task, then the line written with the totals so far; `tessera-protocol.md` §14.1 exists as cited. **Blocking:** (1) §11.1 field "code and reason" requires the close code and reason whenever a `CONNECTION_CLOSE` with an application code was *sent* or received, and the provenance row, the §16 row ("adopts the behaviour of `tessera-server`") and §15.3 ("no gap of `apps/server` on logging remains") say the server does so. It does only for closes the peer sends: when the server itself closes (kick, `SERVER_FULL`, `SLOW_CONSUMER`, protocol error, shutdown) its line carries the transport library's text for a local close, without code or reason. Either keep the field as stated and list this as an open gap in §15.3 (and narrow the provenance and change-log wording to the fields the server does write), or change the field. Non-blocking, for the same or a later revision: (a) "duration" is defined from the connection's acceptance to its end; the server measures from the arrival of the connection attempt (before the QUIC handshake, while the "connection accepted" line follows the handshake) to after the writer wait. Define "acceptance" and whether the wait counts, or list the difference. (b) Invariant 10 equates the line's totals with what the peer "sent and received"; the field table defines them as bytes read from and handed to QUIC, which differ when the peer is lost or the server stops reading. Phrase the invariant on the table's terms or name the condition (both sides read everything before a peer-initiated close), as the server's test does. (c) The server frees the connection's slots (`max_connections`, `max_connections_per_address`) and its statistics-line connection count only after the writer wait, so in the bound case a new connection from the same address can meet `SERVER_FULL` for up to 1 s; say whether the slot is freed before or after the wait, since "MUST NOT hold up … any other connection" reads as before. Earlier: Review 2026-10-10 of the second revision of 2026-10-10 (commit 0afa1db; `git show 0afa1db` of this file: header, one provenance row, §11.1 closed line and statistics line, §15.3, §16 row) by a Claude review session in the spec-reviewer role that wrote neither this spec, its revisions, `tessera-protocol.md` nor the code, against CLEAN_ROOM.md §5.3: **passed**, every item. Checklist: functional facts only (what the closed line's bytes count, the server time of a statistics line, the exclusion of the kind byte, the gap list); no code or pseudo-code (prose and the existing field table); no copied prose; no game text, keys, hostnames or traffic; the new Design row covers the three new facts; invariant 9 unchanged and still testable with a synthetic clock, the closed line's totals testable per connection; no unknown guessed; no closed gate (private and LAN test server, decision 0004). Checked: `tessera-protocol.md` §2.3 rule 1 (the kind byte precedes the frames of a server-initiated unidirectional stream, 0x57 world, 0x43 chat; not a frame of §5.4); the clean `tessera-server` (read only to compare) writes the kind byte before it starts counting the stream's frames into the outgoing byte total, takes the line's time and the interval boundary from the start time of the tick that writes it, reports resident memory where the platform reports it (since 51aa8be, as §15.3 says), and its "connection closed" line carries the duration and the end reason but no byte counts (so the remaining §15.3 gap is real). Review notes (a) and (b) of the previous review are answered. No non-blocking notes. Earlier: Review 2026-10-10 of the revision of 2026-10-10 (commit ef0fdfc; `git show ef0fdfc` of this file: header, §1 two rows, §3.1, §4.1, §8.3, §11.1, §13.4, §14 invariant 9, §15, §15.1, §16 row) by a Claude review session in the spec-reviewer role that wrote neither this spec, its revisions, `tessera-protocol.md` nor the code, against CLEAN_ROOM.md §5.3: **passed**, every item. Checklist: functional facts only (the statistics line's interval, fields and units, exit statuses, fixed parameters); no code or pseudo-code (the fields are a table, the interval rule is prose); no copied prose; no game text, keys, hostnames or traffic; provenance: the two new rows (Design for §11.1, Project for §4.1/§8.3) cover the new facts; invariant 9 testable with a synthetic clock; the run-out OPEN is kept and now states its consequence and a concrete test; no closed gate (private and LAN test server, decision 0004). Checked against the clean `tessera-server` (read only to compare): the interval starts when the simulation starts and then at each line; the line is written at the first tick whose start is at least `stats_interval_s` after the interval start; the interval's longest tick is taken and reset when the line is written, so the writing tick counts toward the next interval (invariant 9); byte rates are the interval's bytes × 10⁶ / its length in µs, rounded down, 0 for a zero length; bytes are counted on stream frames and datagram messages as handed to or read from QUIC; overruns are a since-start count; the since-start longest tick stays for the reports (§13.4). `[movement]` ranges `settle_grace_s` 0–10 and `slide_reserve` 0–100 are refused, not clamped, outside their range (§4.1). §3.1 status 3 for a content file agrees with §2.6 `INSTALL-PATH`, §2.8 status 3 and `server-content.md` E2; status 4 for a data directory with §2.8. Earlier non-blocking items (ii) and (iii) are done; (i) re-checked: `tessera-protocol.md` §8.3 item 1 names a `Named` teleport and its 2026-10-10 rules change no value used here. Non-blocking, for a later revision: (a) §11.1 could say that the interval boundaries are the start times of the ticks that write the lines (the "server time of that line"), which fixes the divisor of the byte rates. (b) The kind byte that opens each server uni stream is not a frame and is not counted; saying so would make "bytes in, bytes out" exact. Earlier: Whole-spec re-review 2026-10-09 of the revision of 2026-10-09 (commit 0f2e39f; Claude review session, spec-reviewer role, did not write this spec, its revision or its code), the whole text read against CLEAN_ROOM.md §5.3: **passed**, every item. Functional facts only (commands, files, formats, defaults, limits, policies, measurements); no code or pseudo-code (the §8.3 update rules are stated as conditions and assignments in prose, matching `tessera-protocol.md` §8.3 "Server relaxations"); no copied prose; no game text, keys, hostnames or traffic (Tessera's own texts only; one DID and one terrain height in §1 as the Confirmed observation; property and layer names only as interoperability names via `collision.md`); every fact under a provenance row with a mark; invariants of §14 testable; unknowns marked OPEN (§3.3, §8.3, §15); no closed gate (private and LAN test server, decision 0004 respected). Earlier blocking item (1) fixed: `log_chat` is now a row of the §4.6 table, before the paragraph. Non-blocking (a), (b), (d), (e) fixed as the change log says; checked: `facing` → heading = (90° − `facing`) in radians reduced to (−π, π] agrees with the Pose convention of `tessera-protocol.md` §5.3 and the four listed values; the tick start ⌊*k* · 10⁶ / `tick_rate`⌋ µs agrees with `tessera-protocol.md` §11.1; 83 MiB = 19,456 KiB × 1 + 64 MiB, and the largest constrained value (8 × 1 GiB + 64 MiB) lies inside 65,536. Re-checked: `tessera-protocol.md` §8.3 "Server relaxations" (window ≤ 10 s skipping only the up and down tests, *S* ≤ 100 m, the same *C* update) agrees with §8.3 here; `collision.md` BasePlayer row (6.5, 3.25, 5, damper 0.2, terminal 50, step-up 0.5) and §12.3 step 3 (length limited to the terminal velocity and 120 m/s) agree with §4.5 and §8.3; its pending revision of 2026-10-09 (§12.5 water-states row) touches none of the cited limits. Arithmetic re-checked: 57-character alphabet, 66.5 m, 18.5 m/s, 42.7 m/s, 166.7 ms. Non-blocking, for a later revision: (i) carry-over (c): `tessera-protocol.md` is itself Revised (its §8.3 relaxations await review); if that review changes the relaxations, re-check §4.5, §8.3 and §14 invariant 8 here; the protocol's settling-window examples still do not name a `Named` teleport to a `ground` spawn. (ii) §3.1 says the install guard refuses a data directory with status 4 and "refuses a content file" without a status; §2.8 and §5.6 step 5 imply status 3 for the content case; say so in §3.1. (iii) The **Reviewer** row above still reads "none yet (whole spec)"; this record names the reviewer (this session) and should be copied there in the next revision. (iv) "Revised" was used before the first acceptance, where CLEAN_ROOM.md §5.2 reserves it for Accepted specs; moot now. Earlier: whole-spec review 2026-10-09 (Claude review session, spec-reviewer role, did not write this spec or its code), against CLEAN_ROOM.md §5.3: **changes requested**. Checklist: functional facts only, no code or pseudo-code, no copied prose, no game text, keys, hostnames or traffic, provenance per fact with the legend of `tessera-protocol.md` §1, invariants testable, unknowns marked OPEN, no closed gate: all pass except the item below. Spot-checks (read-only): the Confirmed row of §1 re-read in place, sample (16, 4) of height record 0x80017E8C gives 438.7 m and the block spans 432.8–452.2 m (agrees); `tessera-protocol.md` §8.3 "Server relaxations" (10 s, 100 m, update rules identical to §8.3 here), the extent 42,080 m, §10.2 chat limits (5, 2 s, 20 in 60 s), §9.4 backlog values, §4.2 71-character fingerprint, §12.2 Accepting 10 s, §8.6 delay bound, §6.3.9 rate range, §5.2 `NAME`/`ACCOUNT` limits; `collision.md` §10.3 BasePlayer and layer values; arithmetic of the 57-character alphabet, *S* = 58 m, the 66.5 m bound, the credit-fill bound and 166.7 ms (all agree). **Blocking:** (1) §4.6: the `log_chat` row (bool, default false, §11.2) stands after the prose paragraph, outside the `[chat]` table, so it renders as paragraph text and the key is missing from the normative table; since §4.1 makes unknown keys an error, a reader who builds the key set from the table rejects valid configurations. Move the row into the table, before the paragraph. Non-blocking: (a) §5.3 `facing` defines the conversion only by naming a function in `tessera_net`; state it in words (protocol heading = 90° − facing, in radians, reduced to (−π, π], from the Pose convention of `tessera-protocol.md` §5.3). (b) §8.1 cites `tick_start(k)` of `tessera-protocol.md` §11.1, which gives the start as ⌊*k* · 10⁶ / `tick_rate`⌋ µs but has no such name; give the formula inline. (c) §8.3 and §4.5 rest on `tessera-protocol.md` §8.3 and `collision.md` revisions of 2026-10-09 that are themselves awaiting review; re-check if either changes. The protocol's settling-window parenthesis names world entry and `Ground` destinations; the grace here also follows a `Named` teleport to a `ground` spawn, covered by the protocol's general clause and its closing sentence, but the protocol could say so explicitly. (d) Author role reads "Spec writer (design; …)" rather than CLEAN_ROOM.md §5.1's "Spec writer (dirty room)" (same convention as `tessera-protocol.md`). (e) §4.7 `memory_budget_mib` lower bound 64 can never meet its own constraint (at least 83 MiB at the Argon2id floor); harmless, the constraint governs. Earlier: Review 2026-10-08 of the 2026-10-08 revision only (§4.7, §10.1, §13.1, §13.3, §15.1, §15.2; Claude review session, spec-reviewer role, fresh session): **passed**, no blocking finding; Tessera design facts only, no dirty-room material, cross-references to `tessera-protocol.md` §12.2 (Accepting 10 s, closing linger) and ARCHITECTURE §19.4 checked. The spec as a whole has not been reviewed, so the status stays Draft. Non-blocking: the §15.1 implementer note on the per-address login budget still said only the connection limit exempts loopback; the §10.1 handshake cap does too (answered 2026-10-09: §6.2 step 1 now lists both exemptions and says the login budget has none). |
| **Implements into** | `apps/server` (`tessera-server` library and binary), `apps/loadtest` (`tessera-loadtest`), `fuzz/` (content-file and accounts-file parsers) |
| **Companion documents** | `tessera-protocol.md` (the wire format; this spec never restates it), `server-content.md` (what content a server may hold), [decision 0030](../decisions/0030-tessera-protocol-transport.md) (QUIC, threads), `walker.md` §10.5 and §10.7, `collision.md` §10.3, §11.3, §12.3–§12.4, `coordinates.md` §4 and §7, `docs/ARCHITECTURE.md` §19–§20, [LEGAL.md](../LEGAL.md) §11.1 |

This spec describes everything about Tessera's own test server that is **not** on the wire:
its command line, files, configuration, content format, world instances, simulation
parameters, limits, logging, shutdown and persistence, and what the load and soak tools
measure. The wire format, message names, codes and the rules both peers follow belong to
`tessera-protocol.md`, cited below by section. Where that spec says "server policy" or
"`test-server.md`", this spec gives the policy.

---

<a id="s0"></a>
## 0. Scope

- `tessera-server` is a headless program for **private and LAN testing** (`server-content.md`
  §8). It holds no game data and never reads a game installation (`server-content.md` §3).
- It serves one content file (the built-in default world unless configured otherwise),
  keeps accounts local to its data directory, and runs one or more world instances.
- It never connects to, imitates or exchanges data with any official service (decision 0004).
- Keywords **MUST**, **MUST NOT**, **SHOULD**, **MAY** as in RFC 2119 and RFC 8174.

---

<a id="s1"></a>
## 1. Provenance

Legend as in `tessera-protocol.md` §1: **Design** (decided here), **Standard**, **Project**
(another Tessera spec or crate), plus **Confirmed** for the one observation below.

| Facts | Source | Mark |
|---|---|---|
| Everything not listed below: commands, files, formats, defaults, limits, policies, the desync and leak definitions | Original design, 2026-10-06 (spec writer, design role). No other server's behaviour was consulted | Design |
| Wire behaviour referred to (states, codes, budgets, validation, interest, chat, time) | `tessera-protocol.md` v1.0 (Accepted 2026-10-07; §8.3 server relaxations and invariant 8 revised 2026-10-09), checked section by section on 2026-10-09 | Project |
| `WALK_SPEED` 2 m/s and `RUN_SPEED` 6 m/s (animation and gait only) | `walker.md` §10.5, §10.7; `tessera-protocol.md` §1 | Project |
| Movement limits of §4.5 and §8.3 (`collision.md` re-checked 2026-10-09: its §11.3.2 revision of that date changes only the water states, not these limits): fastest target speed 6.5 m/s, walkable limit 45°, steep faces 45°–85°, jump push 5.2 m/s, ground braking ≥ 38.5 m/s² and damper 0.2, Slope gravity 10 m/s², Falling gravity 15 m/s² and control acceleration 10 m/s² toward at most 5 m/s with damper 1, Slide and Ice braking ≤ 6.4 m/s², terminal velocity 50 m/s (default) and 100 m/s (Slide layer), speed cap 120 m/s, step-up 0.5 m, landings keep the horizontal length of the clipped velocity | `collision.md` §10.3, §11.1, §11.3, §12.3, §12.4 (a)–(c), (i), (p), (v) (Accepted 2026-10-07); `tessera_physics::controller::params` (`MovementParams::BUILTIN`, `MAX_SPEED`), read 2026-10-08 | Project |
| Run-out lengths behind *S* = 58 m (§8.3) | Tessera's own stepping of the `collision.md` §12.3 controller with the default records at 1/60 s, 2026-10-08 (spec writer, design), and the implementer's independent stepped model of 2026-10-08 (ground braking without the damper; §15.1 note answered 2026-10-09) | Design |
| 30 Hz tick; network thread with a single-threaded async runtime; bounded channels | ARCH §19.4, §20.3; decision 0030 item 4 | Project |
| Landblock 160 m, indices 0–254, ±1,280 m anchor ownership, region 0 = no data | `coordinates.md` §4, §7; `tessera_net::model` | Project |
| Terrain heights lie in 0–2,000 m (stored u16 × 2,000 / 65,535) | `landblock.md` §3.4 | Project |
| Sandbox precedents: wanderers walking circles around the spawn, at most 1,024; landblock spawn grammar `R,X,Y[,x,y[,z]]` | `tessera_net::sandbox` (`MAX_WANDERERS`, `SpawnPoint`), read 2026-10-06 | Project |
| Terrain at region 1, landblock (126, 140), block-local (80, 20) is 438.7 m; the block's samples span 432.8–452.2 m (used for the default spawn height, §5.7) | Height record 0x80017E8C of the owner's install, read in place with `tessera-dattool extract` into the dirty scratch directory and decoded per `landblock.md` §3.2–§3.4, 2026-10-06; the scratch file was deleted | Confirmed |
| Argon2id; recommended parameter set t = 3, p = 4, m = 64 MiB; salt 128 bits; tag 256 bits | RFC 9106 §4 (second recommended option), §3.1 | Standard |
| Minimum Argon2id setting m = 19 MiB, t = 2, p = 1 | OWASP Password Storage Cheat Sheet (background; used only as the floor of §4.3) | Reported |
| PHC string format for password hashes (`$argon2id$v=19$m=…,t=…,p=…$salt$hash`, unpadded base64) | Password Hashing Competition string format, as used by the `argon2` crate | Standard |
| Ed25519 private key file: PKCS#8 (RFC 5958 OneAsymmetricKey) with the RFC 8410 algorithm identifier, PEM label `PRIVATE KEY` (RFC 7468) | RFC 5958, RFC 8410, RFC 7468 | Standard |
| Private address ranges | RFC 1918, RFC 6598, RFC 4193, RFC 3927, RFC 4291 | Standard |
| Configuration and data syntax: TOML 1.0; times as RFC 3339 offset date-times in UTC | toml.io v1.0.0; RFC 3339 | Standard |
| Statistics line of §11.1: interval boundaries, the longest tick reset at every line, bytes per second from the bytes of the interval, overruns counted since start (2026-10-10) | Design decision of the spec writer, 2026-10-10, adopting the behaviour of the clean-room `tessera-server` (commit 9ce64c8, which logged since-start figures before); no dirty-room input | Design |
| Movement parameters fixed for a run; out-of-range values refused at load; the protocol's rules for changing them and for out-of-bound relaxations (§4.1, §8.3, 2026-10-10) | `tessera-protocol.md` §8.3 "Bounds of the values in force" and "Changing the parameters during an epoch" (revised 2026-10-10, awaiting review) | Project |
| §11.1 byte counting made exact (2026-10-10, second revision of that date): the "connection closed" line's bytes in and out are per-connection totals counted like the statistics line's; the kind byte of a server-initiated unidirectional stream is not counted; the server time of a statistics line is the start time of the tick that writes it | Design decision of the spec writer, 2026-10-10, taking up the review notes (a) and (b) of 2026-10-10; matches the counting and the interval boundaries of `tessera-server` (commits 9ce64c8, 51aa8be), which the review record checked; the kind byte rule is `tessera-protocol.md` §2.3 rule 1 | Design |
| "Connection closed" line (§11.1, third revision of 2026-10-10): its fields (connection number, source address, duration in ms, bytes in, bytes out, code and reason); the wait for the connection's stream writers before the line, bounded at 1 s, with the line written regardless when the bound passes; §14 invariant 10 | Design decision of the spec writer, 2026-10-10, adopting the behaviour of the clean-room `tessera-server` (commit a744d7e, which added the byte counts and the bounded wait) for the connection number, source address, duration, bytes and the wait; the code and reason as defined are a decision of the spec writer that the server meets only for closes the peer sends (§15.3); the 1 s bound is an engineering choice, not a measured value; no dirty-room input | Design |
| Closed line, fourth revision of 2026-10-10: the code and reason name the close of whichever end sent it, the line SHOULD say which end closed; duration measured from the arrival of the connection attempt to the writing of the line; the connection's slots and count released when the line is written; invariant 10 on the field table's terms | Design decision of the spec writer, 2026-10-10, taking up the blocking item and review notes (a)–(c) of the review of the third revision; duration, release point and byte counting match `tessera-server` as the review record checked; the code and reason for server-initiated closes do not yet (§15.3); no dirty-room input | Design |
| Readings adopted from the implementer notes of 2026-10-07 and 2026-10-08 (§2.2, §2.3, §3.4, §3.5, §5.3, §6.2, §6.4, §8.1, §8.3, §8.4, §10.1, §10.2, §11.1, §12, §13) | The clean-room implementations of `apps/server` and `apps/loadtest`, their notes reviewed and adopted by the spec writer (design), 2026-10-09 | Design |

---

<a id="s2"></a>
## 2. Command line

<a id="s2-1"></a>
### 2.1 Global options and environment

| Option | Environment | Default | Meaning |
|---|---|---|---|
| `--data-dir DIR` | `TESSERA_SERVER_DIR` | The platform data directory plus `tessera-server` (§3.1) | The server's data directory |
| `--config FILE` | — | `<data-dir>/server.toml` | Configuration file (§4). A missing default file means "all defaults"; a missing file named explicitly is an error |
| `-v`, `-q` | `RUST_LOG` | info | More or less log output (§11). `RUST_LOG` overrides |

The binary's `--help` text MUST state, in Tessera's own words, the operator rules of
`server-content.md` §8 (private or LAN testing only, no public hosting, no money, each player
needs their own legally obtained installation).

<a id="s2-2"></a>
### 2.2 `run`

`tessera-server run [--listen ADDR:PORT]... [--content FILE] [--log-format text|json]`

Starts the server in the foreground. Steps, in order; any failure exits with the status of
§2.8 before a socket is opened:

1. Resolve the data directory, apply the install guard (§3.1), create the directory if
   missing (mode 0700 on Unix), and take the server lock (§3.7).
2. Read and validate the configuration (§4). `--listen` (repeatable) replaces
   `[server] listen`; `--content` replaces `[server] content`. A relative `--content` path is
   resolved against the working directory, as `--config` and `check-content` arguments are
   (a relative `content` key in the configuration against the data directory, §4.2).
3. Load and check the content file (§5.6, the checks `run` applies).
4. Apply the address guard (§4.2).
5. Load or generate the server key (§3.3). Log the fingerprint (`tessera-protocol.md` §4.2) at
   info, and for each listen address a connect-string hint (`tessera-protocol.md` §4.4):
   the address itself, or `<host>` for an unspecified address.
6. Load accounts (§3.4), characters on demand (§3.5), and the world state (§3.6).
7. Start the threads (§8.1), open the QUIC endpoint on every listen address, and log
   "ready".

It runs until a shutdown signal (§12.1).

<a id="s2-3"></a>
### 2.3 `keygen` and `fingerprint`

- `tessera-server keygen [--force]` creates the server key (§3.3) if none exists and prints its
  fingerprint. If a key exists it refuses (status 2) unless `--force` is given. With
  `--force` it renames the old file to `server.key.old-<UTC time as YYYYMMDDTHHMMSSZ>`
  (same mode; when that name exists it appends `.1`, `.2`, … so that no earlier backup is ever
  replaced), writes a new key, prints the old and new fingerprints, and warns that every
  client that pinned the old fingerprint will fail with `KEY_MISMATCH`
  (`tessera-protocol.md` §4.9). It needs the server lock (§3.7): it refuses while a server
  runs on the directory.
- `tessera-server fingerprint [--connect HOST]` prints the fingerprint as one 71-character line.
  With `--connect HOST` it prints instead one connect string per configured listen port, with
  HOST as the host (`tessera://HOST:PORT?pin=…`, IPv6 in brackets). It never creates
  anything; with no key it fails (status 1).

Neither command ever prints private key material.

<a id="s2-4"></a>
### 2.4 `account`

| Command | Effect | While a server runs |
|---|---|---|
| `account add NAME [--password-stdin \| --generate] [--disabled]` | Creates an account. NAME must be an `ACCOUNT` string (`tessera-protocol.md` §5.2) and is stored canonical (§6.1). Fails if the canonical name exists. `--generate` creates a random password of 20 characters from the 57-character alphabet without look-alikes (`A–Z a–z 2–9` minus `I O l`) and prints it once | Allowed (§3.7) |
| `account list` | One line per account: canonical name, creation time, `disabled` and `muted` flags, number of characters. Never hashes | Allowed |
| `account passwd NAME [--password-stdin \| --generate]` | Replaces the password hash. Does not end a running session | Allowed |
| `account disable NAME`, `account enable NAME` | Sets or clears `disabled`. A running server ends the account's session with `Disconnect(KICKED)` on reload (§3.7) | Allowed |
| `account mute NAME`, `account unmute NAME` | Sets or clears `muted` (§9) | Allowed |
| `account remove NAME [--yes]` | Removes the account and its character file. Asks for confirmation on a terminal unless `--yes` | Refused (status 4): stop the server or use `account disable` |

<a id="s2-5"></a>
### 2.5 `character`

| Command | Effect | While a server runs |
|---|---|---|
| `character list [ACCOUNT]` | Name, account, instance, region and landblock of the last position | Allowed (read only) |
| `character remove NAME [--yes]` | Removes one character | Refused (status 4) |
| `character move NAME --spawn SPAWN` | Resets the stored position to a content spawn point (for a character stuck somewhere) | Refused (status 4) |

<a id="s2-6"></a>
### 2.6 `check-content`

`tessera-server check-content [FILE]...`

Checks content files against the format (§5) and the automatic items of
`server-content.md` §9.3. With no FILE it checks the content `run` would use (the built-in
default world if none is configured) **and** the configuration file. It reads only the named
files and the configuration. Output: one line per finding,
`<file>:<line>: <RULE>: <message>`, sorted by file and line, then a summary line. Rules:

| Rule | Finding |
|---|---|
| `SYNTAX` | Not valid TOML, or not UTF-8 |
| `FORMAT` | Missing or unknown `format`, unknown key, missing required key, wrong type |
| `RANGE` | A value outside its range (§5.6) |
| `SPDX` | No `# SPDX-License-Identifier: …` comment before the first key, or a licence other than `GPL-3.0-or-later` or `CC0-1.0` |
| `POSITIONS` | More than 16 positions (`server-content.md` §5.2 P4) |
| `REFERENCES` | More than 8 opaque references (`server-content.md` §9.2) |
| `TEXT` | A name or text that breaks its text class |
| `WORD` | A word or phrase from LEGAL.md §10 rule 1 in any text or name (§5.6) |
| `INSTALL-PATH` | The file lies in a game installation (§3.1) |
| `CONFIG` | A configuration finding (§4.1), when the configuration is checked |

Exit status 0 when there are no findings, 3 otherwise. `run` applies every rule except
`SPDX` (an operator's private file need not carry a licence header) and refuses to start on
any finding.

<a id="s2-7"></a>
### 2.7 Password entry

- A password is **never** taken from the command line or an environment variable (both leak
  into process listings and shell history).
- On a terminal, `account add` and `account passwd` prompt twice without echo and fail if the
  two entries differ.
- `--password-stdin` reads the first line of standard input and removes one trailing LF or
  CRLF. Nothing else is trimmed.
- Without a terminal and without `--password-stdin` or `--generate`, the command fails
  (status 2).
- The password policy of §6.5 applies.

<a id="s2-8"></a>
### 2.8 Exit status

| Status | Meaning |
|---|---|
| 0 | Success, or a clean shutdown |
| 1 | Runtime failure (I/O, key unreadable, save failed during shutdown) |
| 2 | Usage or configuration error (bad option, invalid configuration, refused without `--force`) |
| 3 | `check-content` found violations, or `run` refused its content |
| 4 | The data directory is locked by a running server, or the install guard (§3.1) refused it |

---

<a id="s3"></a>
## 3. Data directory

<a id="s3-1"></a>
### 3.1 Location and install guard

- Default: the platform's per-user data directory plus `tessera-server`: `$XDG_DATA_HOME/tessera-server`
  (usually `~/.local/share/tessera-server`) on Linux and other Unix systems,
  `%APPDATA%\tessera-server` on Windows, `~/Library/Application Support/tessera-server` on
  macOS. `--data-dir` or `TESSERA_SERVER_DIR` override it.
- `run`, `keygen` and `account add` create it if missing (mode 0700 on Unix). The other
  commands never create it.
- **Install guard** (`server-content.md` §10 E2). The server refuses a data directory (exit
  status 4) and a content file (exit status 3, rule `INSTALL-PATH` of §2.6) when the path
  itself or any of its ancestor directories contains a file whose name matches
  `client_*.dat` or ends in `.datx`, or when the data directory contains any file ending in
  `.dat` or `.datx`. Paths are checked after
  resolving symbolic links. The server never opens such files; it only lists directory
  names.

<a id="s3-2"></a>
### 3.2 Layout

| Path (in the data directory) | Written by | Mode (Unix) | Content |
|---|---|---|---|
| `server.toml` | The operator | any | Configuration (§4). Optional |
| `server.key` | `run` (first start), `keygen` | 0600 | Ed25519 private key (§3.3) |
| `server.lock` | `run`, exclusive commands | 0600 | Advisory lock; holds the PID of the running server (§3.7) |
| `accounts.toml` | `account …`, `run` (registration, rehash) | 0600 | Accounts (§3.4) |
| `accounts.lock` | as above | 0600 | Advisory lock for read-modify-write of `accounts.toml` |
| `characters/` | `run`, `character …`, `account remove` | 0700 | One file per account that has characters: `<canonical account>.toml` (§3.5) |
| `world-state.toml` | `run` | 0600 | Game clock per instance (§3.6) |

Nothing else is written there. Logs go to standard error unless `[log] file` names a file.
No file in the data directory is ever committed, published or sent to clients
(`server-content.md` §4.1 R5).

<a id="s3-3"></a>
### 3.3 Server key

- One Ed25519 key per data directory, generated from the operating system's CSPRNG
  (`tessera-protocol.md` §4.1). File format: PKCS#8 (RFC 5958, version 1, no public key
  attribute) with the RFC 8410 identifier, PEM-encoded with the label `PRIVATE KEY`
  (RFC 7468). One key per file, nothing else in it.
- Generation writes a new file created exclusively with mode 0600, then renames it into
  place (§3.7).
- At every start (Unix): the server refuses to start (status 1) if `server.key` is not a
  regular file owned by the effective user, or if any group or other permission bit is set,
  and the message names the fix (`chmod 600`). On Windows no permission check is made in v1.
- A key file that exists but cannot be read or parsed stops the server (status 1). The server
  **never** replaces an existing key on its own, because that changes the fingerprint.
- The certificate is built from the key at each start (`tessera-protocol.md` §4.1) and is not
  stored.
- Key bytes are never logged, printed or sent anywhere.

> **OPEN:** a Windows permission check (owner-only ACL) for `server.key` and the accounts
> file.

<a id="s3-4"></a>
### 3.4 Accounts file

TOML (§1). Top level: `format = 1`, then one `[[account]]` table per account, in canonical
name order:

| Key | Type | Rule |
|---|---|---|
| `name` | string | Canonical account name (§6.1), unique |
| `hash` | string | PHC string of an Argon2id (version 19) hash: `$argon2id$v=19$m=<KiB>,t=<iterations>,p=<lanes>$<salt>$<tag>`, salt and tag in unpadded standard base64 |
| `created` | offset date-time | UTC |
| `disabled` | bool | Optional, default false |
| `muted` | bool | Optional, default false |

- **Hash parameters** come from `[accounts]` (§4.3). Defaults: memory 65,536 KiB (64 MiB),
  3 iterations, 4 lanes (RFC 9106's second recommended option); salt 16 bytes from the
  CSPRNG, fresh for every hash; tag 32 bytes; no secret key and no associated data.
- Each hash carries its own parameters, so changing the configuration never breaks existing
  hashes. After a successful login whose stored parameters are below the configured ones in
  any of memory, iterations or lanes, the server re-hashes the password it still holds and
  replaces the entry (under the accounts lock).
- **Size.** The file is at most 4 MiB; every reader of the server and the CLI refuses a larger
  one. A change that would make it larger is refused: `Register` with
  `LoginFailed(REGISTRATION_CLOSED)`, `account add` with status 1. Every write checks the
  size, so the server can always read the file it wrote. There is no separate account-count
  limit.
- Unknown keys, a duplicate name, a non-canonical name or a hash that is not a valid
  Argon2id PHC string make the file invalid. `run` then refuses to start (status 2), and a
  running server keeps its last good copy and logs an error.

<a id="s3-5"></a>
### 3.5 Character files

`characters/<canonical account>.toml`: `format = 1`, `account = "<canonical name>"`, then one
`[[character]]` per character:

| Key | Type | Rule |
|---|---|---|
| `name` | string | The display name as created (NFC, §6.4) |
| `created` | offset date-time | UTC |
| `instance` | integer | The last world instance, 0 = never entered |
| `position` | table | `region` (0–65,535), `x`, `y`, `z` (floats, metres, `tessera-protocol.md` §5.3 limits), `heading` (radians, protocol convention), optional `anchor` = [block X, block Y, cell] that owns the position |
| `appearance` | table | `body` = `{ species = N, sex = N }` or `{ object = N }`; `scale`; optional `idle`; optional `[[character.appearance.items]]` with `did`, optional `dye`, optional `secondary_dye`, `stowed` (bool). Same limits as the protocol's Appearance (`tessera-protocol.md` §5.3) |

- Character names are unique across the server by their key (§6.4); the server keeps an index
  in memory, built at start from all character files.
- A character whose stored name is not a protocol `NAME` (`tessera-protocol.md` §5.2) makes
  the file invalid. A stored anchor whose indices do not fit the protocol's anchor (block
  index above 255, cell above 65,535) fails the limits like any other position.
- A character whose stored position or appearance fails the protocol's limits is loaded with
  the default spawn (§5.2) or, for the appearance, refused at `EnterWorld` with
  `SERVER_ERROR` and an error log line naming the file.
- A file holds only its own account's characters (`server-content.md` §11 invariant 6).

<a id="s3-6"></a>
### 3.6 World-state file

`world-state.toml`: `format = 1`, then `[[instance]]` tables with `number` (1–16) and
`game_clock` (seconds, finite, ≥ 0). Read at start: an instance with a saved clock resumes
from it, others start from the content's `[clock] start`. Written at every save round
(§12.2) and at shutdown.

<a id="s3-7"></a>
### 3.7 Locks and atomic writes

- **Server lock.** `run` takes an exclusive advisory lock on `server.lock` (`flock` on Unix,
  `LockFileEx` on Windows) and holds it until exit, and writes its PID into the file. If the
  lock is held, `run` exits with status 4 and names the PID. `keygen`, `account remove`,
  `character remove` and `character move` take the same lock for their duration, so they
  cannot run beside a server and a server cannot start during them.
- **Accounts lock.** Every read-modify-write of `accounts.toml` (CLI or server) holds an
  exclusive lock on `accounts.lock`: read the current file, apply the change, write it
  atomically, release.
- **Reload.** A running server re-reads `accounts.toml` when its modification time or size
  differs from the copy it loaded. It checks before each login verification and every 10 s.
  After a reload, every session whose account was removed or disabled ends with
  `Disconnect(KICKED)`, and `muted` takes effect at once.
- **Atomic write.** Every file is written as a new file in the same directory (name starting
  with `.` and ending in `.tmp`), flushed to disk, renamed over the target, and on Unix the
  directory is synced. A crash leaves either the old or the new file. Stale `.tmp` files are
  removed at start.

---

<a id="s4"></a>
## 4. Configuration file

<a id="s4-1"></a>
### 4.1 Syntax and validation

`server.toml` is TOML with the tables below. Every key is optional unless marked. Unknown
tables or keys, wrong types and values outside their range are errors that name the key
(`CONFIG` findings, §2.6). `run` refuses to start on any error (status 2); a value outside
its range is never clamped. Values are read once at start; changing them needs a restart. In
particular the `[movement]` parameters (§4.5) stay the same for the whole run, so the rules of
`tessera-protocol.md` §8.3 for changing the parameters during an epoch never act in
`tessera-server` v1 (the accounts-file reload of §3.7 is not configuration and changes no
limit).

<a id="s4-2"></a>
### 4.2 `[server]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `listen` | array of strings | **required** (no default) | 1–8 socket addresses `host:port`, IPv6 in brackets; port 1–65,535 | UDP addresses to listen on. There is no default port (decision 0030 item 6) |
| `name` | string | `Tessera test server` | `TEXT0`(64); no LEGAL.md §10 rule-1 word | `server_name` in `HelloAck` |
| `motd` | string | (the content's) | `TEXT0`(1,024); no rule-1 word | Replaces the content's message of the day |
| `content` | path | (built-in default world) | Relative paths are resolved against the data directory | Content file (§5) |
| `registration` | bool | false | — | Advertise `REGISTRATION` and accept `Register` |
| `character_creation` | bool | true | — | Advertise `CHARACTER_CREATION` |
| `appearance_change` | bool | true | — | Advertise `APPEARANCE_CHANGE` |
| `duplicate_login` | string | `replace` | `replace`, `refuse` | `tessera-protocol.md` §4.7: `replace` ends the old session with `LOGGED_IN_ELSEWHERE`; `refuse` answers `ALREADY_ONLINE` |
| `shutdown_grace_s` | integer | 10 | 0–600 | §12.1 |

**Address guard** (`server-content.md` §8 O2, O4, §10 E5). A listen address is **private** if
it is loopback (127.0.0.0/8, ::1), RFC 1918 (10/8, 172.16/12, 192.168/16), RFC 6598 shared
address space (100.64/10), link-local (169.254/16, fe80::/10) or an IPv6 unique local
address (fc00::/7). Every other address, the unspecified addresses 0.0.0.0 and :: included,
is **not private**.

- With `registration = true`, every listen address MUST be private. Otherwise `run` refuses
  to start (status 2) and suggests binding the LAN address explicitly.
- With any non-private listen address, `run` logs a warning at start that restates the
  operator rules (private use, no public hosting).

The server cannot see port forwarding or NAT. The guard catches mistakes; it does not replace
the operator rules.

<a id="s4-3"></a>
### 4.3 `[accounts]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `argon2_memory_kib` | integer | 65,536 | 19,456–1,048,576 | Argon2id memory for new hashes |
| `argon2_iterations` | integer | 3 | 2–10 | Argon2id passes |
| `argon2_lanes` | integer | 4 | 1–8 | Argon2id parallelism |
| `hash_workers` | integer | 2 | 1–8 | Hash computations that may run at once (§6.2) |
| `max_characters` | integer | 8 | 1–64 | Characters per account (`TOO_MANY`) |
| `password_min_bytes` | integer | 8 | 1–256 | §6.5 |
| `lockout_after` | integer | 5 | 1–100 | Consecutive failures before an account lockout (§6.2) |
| `lockout_base_s` | integer | 30 | 1–3,600 | First lockout |
| `lockout_max_s` | integer | 900 | `lockout_base_s`–86,400 | Longest lockout |
| `attempts_per_address_per_min` | integer | 20 | 1–10,000 | Per-address login budget (§6.2); loopback addresses are not exempt |
| `allow_object_bodies` | bool | false | — | Accept `Object` bodies in player appearances (§6.4) |

<a id="s4-4"></a>
### 4.4 `[world]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `instances` | integer | 1 | 1–16 | World instances (§7) |
| `instance_capacity` | integer | 64 | 1–1,024 | Characters per instance |
| `snapshot_rate` | integer | 15 | 1, 2, 3, 5, 6, 10, 15 or 30 | `MotionBatch` rate (`tessera-protocol.md` §8.5); a divisor of the 30 Hz tick |
| `interest_radius` | integer | 2 | 0–8 | *R* (`tessera-protocol.md` §9.2) |
| `max_entities_cap` | integer | 1,024 | 64–4,096 | Server cap on entities created per connection; the effective limit is the smaller of this and the client's `max_entities` (`tessera-protocol.md` §9.4) |
| `creates_per_tick` | integer | 16 | 1–256 | New `EntityCreate`s per connection per tick (§9.4 there) |
| `world_bytes_per_tick` | integer | 65,536 | 4,096–1,048,576 | World-stream bytes per connection per tick |
| `idle_timeout_min` | integer | 30 | 0–1,440; 0 = never | In-world inactivity limit (§8.1) |
| `teleport` | string | `any` | `any`, `named`, `off` | §8.4 |
| `teleport_to_players` | bool | true | — | §8.4 `@name` destinations |
| `teleport_interval_ms` | integer | 500 | 0–10,000 | Least time between two executed teleports of one player |
| `save_interval_s` | integer | 60 | 10–3,600 | §12.2 |

The tick rate is fixed at 30 Hz in v1 (`tessera-protocol.md` §6.2.2), and the minimum
supported minor version is 0 (`tessera-protocol.md` §3); neither is configurable. The world-stream
backlog limits (1 MiB, 256 KiB, 4 MiB, 30 s) are the protocol's §9.4 defaults and are not
configurable.

<a id="s4-5"></a>
### 4.5 `[movement]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `walk_speed` | float | 2.0 | (0, 100], ≤ `run_speed` | `WALK_SPEED` sent to each player (`tessera-protocol.md` §7.3); animation and gait only |
| `run_speed` | float | 6.0 | (0, 100] | `RUN_SPEED`; animation and gait only |
| `max_horizontal_speed` | float | 8.5 | 6.5–120 | *V*ₕ |
| `max_up_speed` | float | 10.0 | 6.5–120 | *V*ᵤ |
| `max_down_speed` | float | 120.0 | 50–200 | *V*_d |
| `slide_reserve` | float | 58.0 | 0–100 | *S*, the most descent credit kept for a slide's run-out, in metres (§8.3); 0 turns the credit off |
| `window_s` | float | 1.0 | 0.25–5.0 | *W*, the allowance window |
| `settle_grace_s` | float | 3.0 | 0–10 | §8.3 settling grace |

The defaults of *V*ₕ, *V*ᵤ, *V*_d and *W* are the protocol's §8.3 defaults; `slide_reserve` and
`settle_grace_s` use the two relaxations that §8.3 there permits (descent credit *S* ≤ 100 m,
settling window ≤ 10 s). The limits follow the character controller of
`collision.md` §12 (Tessera's client, `crates/physics`), whose speeds come from the player
movement record (`collision.md` §10.3), not from `WALK_SPEED`/`RUN_SPEED`; the walk and run
speeds stay those of `walker.md` §10.5 and no longer set a limit. Reasoning:

- ***V*ₕ = 8.5 m/s** (6.5 m/s + 30 %). No controlled motion is faster than the BasePlayer
  forward speed of 6.5 m/s (steady 6.33 m/s after the damper, `collision.md` §12.4 (b)):
  the input pair is scaled to length 1 (`collision.md` §12.3 step 1), strafe (5) and back
  (3.25; Troll 6) are slower, and the air layers brake toward 5 m/s or less. The margin
  covers send-time jitter, push-outs (≤ 0.5 m per step, `collision.md` §12.4 (g)) and
  step-up raised moves (§12.4 (d)). Faster horizontal motion comes only from sliding,
  covered by the descent credit of §8.3 here.
- ***V*ᵤ = 10 m/s** (6.5 m/s + about 50 %). The fastest sustained rise is walking up the
  steepest walkable slope (45°, `collision.md` §11.3) at 6.5 m/s; a jump starts at 5.2 m/s
  (`collision.md` §12.4 (c), (i)). The margin and the burst *V*ᵤ · *W* = 10 m cover
  step-ups (up to 0.5 m in one 1/60 s step, `collision.md` §12.3 step 5) and buoyancy
  lifting a swimmer to the float line (§12.4 (p)).
- ***V*_d = 120 m/s** (the Slide layer's 100 m/s + 20 %). Falls end at the terminal
  velocity of 50 m/s (BasePlayer; Falling lists none, `collision.md` §10.3), slides at 100
  m/s (Slide layer), and every speed is capped at 120 m/s (`collision.md` §12.3 step 3), so
  no legitimate descent exceeds it.
- **Cheats still rejected.** On level ground a speed or teleport hack gains at most
  *V*ₕ · *W* + *S* = 66.5 m beyond *V*ₕ · *t* and is then corrected; the credit grows only by
  descending and is cleared by every correction (§8.3). Over any stretch of one epoch the
  accepted horizontal distance is at most *V*ₕ · (*t* + *W*) + *S* + the descent counted in it
  (§14 invariant 8), so a client whose height comes back to where it was averages at most
  *V*ₕ + *V*ᵤ = 18.5 m/s. A rise beyond *V*ᵤ or a descent beyond *V*_d is corrected outside the
  settling grace. What a server without terrain cannot reject: horizontal motion paid metre
  for metre by descent, the signature of a slide, looks the same as a speed hack down a slope
  or a client sinking through the ground, and is accepted at up to *V*ₕ + *V*_d horizontally
  while it lasts. Terrain checks (not in v1, §8.3) would close that.

<a id="s4-6"></a>
### 4.6 `[chat]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `say_range_m` | float | 40 | 1–1,000 | Say range (`tessera-protocol.md` §10.1) |
| `emote_range_m` | float | 40 | 1–1,000 | Emote range |
| `yell_scope` | string | `instance` | `instance`, `region` | Yell reaches the whole instance (protocol default) or only the speaker's region in it |
| `bucket_capacity` | integer | 5 | 1–5 | Chat token bucket (`tessera-protocol.md` §10.2) |
| `refill_s` | float | 2.0 | 2.0–60 | Seconds per token |
| `flood_rejections` | integer | 20 | 5–20 | Rejections within `flood_window_s` that end the session with `FLOODING` |
| `flood_window_s` | integer | 60 | 60–3,600 | |
| `log_chat` | bool | false | — | §11.2 |

The protocol fixes the chat rate limit and flood rule (`tessera-protocol.md` §10.2: capacity 5,
one token per 2 s, 20 rejections within 60 s), so these keys can only make them **stricter**;
their ranges stop at the protocol's values and the defaults equal them. The protocol session
applies its own limits as well; with the ranges above the configured ones are never looser, so
the configured values are the ones that act.

<a id="s4-7"></a>
### 4.7 `[limits]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `max_connections` | integer | 64 | 1–4,096 | Connections past the QUIC handshake, in any state (§10) |
| `max_connections_per_address` | integer | 8 | 1–4,096 | Per source IP address; loopback addresses are exempt |
| `max_handshakes` | integer | 32 | 1–1,024 | QUIC handshakes in progress. The per-address cap (4, loopback exempt) is fixed, not a key (§10.1) |
| `control_queue` | integer | 64 | 16–1,024 | Inbound control messages queued per connection between the network and simulation threads |
| `memory_budget_mib` | integer | 1,024 | 83–65,536 | Accounted memory budget (§10.2). Must be at least `argon2_memory_kib` × `hash_workers` + 64 MiB |

<a id="s4-8"></a>
### 4.8 `[log]`

| Key | Type | Default | Range | Meaning |
|---|---|---|---|---|
| `level` | string | `info` | `error`, `warn`, `info`, `debug`, `trace` | Default filter; `RUST_LOG` overrides |
| `format` | string | `text` | `text`, `json` | One line per event |
| `file` | path | (standard error) | | Append to this file instead (mode 0600 on Unix) |
| `stats_interval_s` | integer | 60 | 0–3,600; 0 = off | Periodic statistics line (§11.1) |

---

<a id="s5"></a>
## 5. Content file

A content file defines a world (`server-content.md` §2). It holds only original content
(`server-content.md` §6, §9).

<a id="s5-1"></a>
### 5.1 Syntax

TOML, UTF-8. Committed content files start with a comment line
`# SPDX-License-Identifier: GPL-3.0-or-later` (or `CC0-1.0`) before the first key. Top level:
`format = 1` (required), then the tables below. Unknown keys are errors.

<a id="s5-2"></a>
### 5.2 `[world]` (required)

| Key | Type | Rule |
|---|---|---|
| `id` | string | 1–32 characters of `a–z`, `0–9`, `-`. Logged; identifies the content |
| `title` | string | `TEXT0`(64). Logged at start |
| `motd` | string | `TEXT0`(1,024). The message of the day in `HelloAck`, unless `[server] motd` replaces it |
| `default_spawn` | string | The name of a `[[spawn]]`. Where new characters start and where invalid stored positions are reset to |

<a id="s5-3"></a>
### 5.3 `[[spawn]]` (1–16)

| Key | Type | Rule |
|---|---|---|
| `name` | string | 1–32 characters of `a–z`, `0–9`, `-`, unique in the file; project-made (`server-content.md` §5.2 P2). `Named` teleports match it (§8.4) |
| `region` | integer | 0–65,535. 0 is the world without data |
| `block` | [X, Y] | Landblock indices 0–254. Exactly one of `block` + `offset` or `location` is given |
| `offset` | [x, y, z] | Block-local metres. x and y within ±1,280; z absolute height, finite, within ±10⁷ |
| `location` | [x, y, z] | Region-global metres (`tessera-protocol.md` §5.3 limits) |
| `facing` | float | Compass degrees: 0 = north, 90 = east, clockwise; [0, 360). Converted to the protocol's heading (`tessera-protocol.md` §5.3 Pose: radians, 0 = +X east, counter-clockwise; +Y is north, `coordinates.md` §3) as heading = (90° − `facing`) in radians, reduced to (−π, π]; so 0 → π/2, 90 → 0, 180 → −π/2, 270 → π. Default 0 |
| `ground` | bool | Default false. True: the height is approximate and the player settles onto their own terrain (§8.3 settling grace). Not allowed with an out-of-square offset |

A `block` + `offset` spawn whose x or y lies outside [0, 160] is an **interior** position: it is
anchored to that block (cell 0, unknown; `coordinates.md` §7) and is never treated as ground.
Every other spawn is outdoors and carries **no anchor**: its interest block follows the position
(`tessera-protocol.md` §8.7, §9.1). (An anchor on an outdoor spawn would keep deciding the
interest block while it owns the position, ±1,280 m, so a player walking away would keep the
spawn's block for up to eight blocks.) Each spawn counts as one
position (`server-content.md` §5.2 P4).

<a id="s5-4"></a>
### 5.4 `[[wanderers]]` (0–16 groups)

Simulated, project-made actors (`server-content.md` §7 "In scope").

| Key | Type | Rule |
|---|---|---|
| `spawn` | string | The spawn point they walk around |
| `count` | integer | 1–256. At most 1,024 wanderers per instance over all groups |
| `radius` | float | 0.5–100 m: radius of each wanderer's circle |
| `speed` | float | (0, 7.5] m/s along the circle |
| `name` | string | Template; `{n}` is replaced by the wanderer's 1-based number within the instance. The result must be a `NAME` and contain no rule-1 word. Default `Wanderer {n}` |
| `appearance` | string or table | `"first-player"`, or an explicit appearance table with the keys of §3.5 `appearance` |

**Behaviour** (computed from server time, no state beyond the start time):

- Wanderer *i* of a group of *n* walks a circle of `radius` around its own centre. The centres
  lie on a ring of radius 2 × `radius` around the spawn point, at angle 2π *i* / *n* from east,
  counter-clockwise. Each wanderer starts at angle 2π *i* / *n* on its own circle, moves
  counter-clockwise at `speed`, faces along its velocity, and keeps the spawn's height and
  region. It has the spawn's anchor, that is none for an outdoor spawn (§5.3).
- Its entity kind is NonPlayer, it has no class, no properties and no name collisions with
  characters (character names cannot contain digits, §6.4). Its motion byte has gait Walking
  when `speed` is at most the midpoint of the configured walk and run speeds, otherwise
  Running (`tessera-protocol.md` §8.1).
- **`first-player`:** the group does not exist until the first character enters the
  instance. At that moment it is created with that character's appearance at entry. Later
  appearance changes of that character do not propagate. The group then stays until the
  server stops, whether or not players remain. This lets the default world show animated
  characters without holding any opaque reference.
- With an explicit appearance, the group exists from the moment the instance starts. Its
  references count towards the limit of `server-content.md` §9.2 (one per body, one for
  `idle`, one per item).

<a id="s5-5"></a>
### 5.5 `[clock]` (optional)

| Key | Type | Default | Rule |
|---|---|---|---|
| `start` | float | 0 | Game clock *G* in seconds for an instance with no saved state; finite, ≥ 0 |
| `rate` | float | 1 | Game seconds per real second, 0–10,000 (`tessera-protocol.md` §6.3.9) |

<a id="s5-6"></a>
### 5.6 Validation

Applied by `check-content` and by `run` (§2.6):

1. Syntax, `format = 1`, required keys, types, unknown keys.
2. Every range and text class above. Spawn names unique; `default_spawn` and every
   `wanderers.spawn` name an existing spawn.
3. At most 16 positions (one per spawn) and at most 8 opaque references.
4. **Word check:** every text and name (title, message of the day, spawn names, wanderer
   name templates, and also `[server] name` and `motd` of the configuration) is checked,
   ignoring case, against the words and phrases of LEGAL.md §10 rule 1. A word matches only
   at word boundaries (a boundary is the start, the end, or a character that is not a letter
   or digit), so ordinary words that contain a listed word as a part do not match. The list is
   project policy compiled into the server, not game data. It cannot catch every
   Tolkien-invented word; the human items of `server-content.md` §9.3 cover the rest.
5. Install guard (§3.1) on the file's path.
6. SPDX header (`check-content` only).

<a id="s5-7"></a>
### 5.7 The default world

`apps/server/content/default-world.toml`, compiled into the binary (`server-content.md` §6). Its
values:

| Item | Value |
|---|---|
| `id`, `title` | `default`, "Tessera test world" |
| `motd` | Tessera's own text: a test server for Tessera; not affiliated with any game publisher; each player needs their own legally obtained installation to see the world |
| `default_spawn` | `start` |
| Spawn `start` | Region 1, `block` [126, 140], `offset` [80, 20, 440], `facing` 0, `ground` true. The height is 1.3 m above the observed terrain (438.7 m, §1), so a client falls onto its own terrain without a correction |
| Spawn `void` | Region 0, `block` [127, 127], `offset` [80, 80, 0], `ground` false: the middle of the grid of a world without data, for clients without an install and for the load tester |
| Wanderers | One group: `spawn` `start`, `count` 3, `radius` 6, `speed` 1.5, `name` `Wanderer {n}`, `appearance` `first-player` |
| Clock | `start` 0, `rate` 1 |

---

<a id="s6"></a>
## 6. Accounts and characters

<a id="s6-1"></a>
### 6.1 Account names

An account name is an `ACCOUNT` string (`tessera-protocol.md` §5.2). Its **canonical** form
is its ASCII lowercase. Accounts are stored, looked up, compared and reported (`LoginOk`)
by the canonical form only.

<a id="s6-2"></a>
### 6.2 Login verification

For each `Login` (`tessera-protocol.md` §4.7):

1. **Address budget.** A token bucket per source IP address, capacity
   `attempts_per_address_per_min`, refilled at that many tokens per minute. Empty: answer
   `LoginFailed(RATE_LIMITED)` with `retry_after` = seconds until the next token, rounded
   up, at least 1. No hash is computed. Loopback addresses are **not** exempt from this budget
   (only the connection and handshake caps of §4.7 and §10.1 exempt them), so a load test that
   runs many bots on loopback raises `attempts_per_address_per_min` or uses several loopback
   addresses (§13.1).
2. **Name lockout.** If the canonical account **name** is locked out, answer `RATE_LIMITED`
   with the remaining seconds. No hash is computed. Lockouts are kept per canonical name
   whether or not an account of that name exists (step 5).
3. **Queue.** The verification goes to the hash workers (§8.1): at most `hash_workers` run at
   once and at most 64 wait. With the queue full: `RATE_LIMITED`, `retry_after` 5.
4. **Verify.** Known account: verify the password against its hash. Unknown account: verify
   it against a dummy hash made at start with the configured parameters, so both cases take
   similar time. The result for an unknown account is always a failure.
5. **Answer.**
   - Wrong password or unknown account: `BAD_CREDENTIALS`. The consecutive failure count of
     the canonical name increases, **whether or not the account exists**, so that a known and
     an unknown name answer alike on the wire (`tessera-protocol.md` §4.7); reaching
     `lockout_after` locks the name for `lockout_base_s`, and each further failure while the
     count stays above the limit doubles the lockout, up to `lockout_max_s`. The table of
     counted names holds at most 65,536 entries; when it is full, no new name is counted,
     known or not.
   - Right password, account disabled: `ACCOUNT_DISABLED`. (A wrong password never reveals
     that an account is disabled.)
   - Right password: reset the failure count; apply the duplicate-login rule (§6.3); re-hash
     if needed (§3.4); `LoginOk`.
6. The password buffer is overwritten once the verification (and any re-hash) is done.

Lockout and address-budget entries live in memory only. A lockout entry is removed when it
has been idle for `lockout_max_s`; an address-budget entry is removed only once its bucket has
refilled completely (it then equals a new entry), so removing it never hands out tokens early. A per-account lockout lets a stranger lock a known account out for
a while; on a private server (`server-content.md` §8 O1) that is accepted.

> **Answered (2026-10-09, spec writer; was OPEN, implementer, review of 2026-10-07):** both
> readings of the code are adopted: failures and lockouts per canonical name whether the account
> exists or not (step 5, table cap 65,536), and an address-budget entry removed only once its
> bucket has refilled (above). The log keeps "unknown account" for unknown names (§11.2).

<a id="s6-3"></a>
### 6.3 Duplicate login

`[server] duplicate_login` chooses the protocol's behaviour (`tessera-protocol.md` §4.7).
With `replace`, the old session's character leaves the world first (it is saved, §12.2, and
destroyed for observers with reason `REMOVED`), then the new session gets `LoginOk`.

<a id="s6-4"></a>
### 6.4 Characters

- **Name rule** (server policy for `NAME_INVALID`). The name is normalised to Unicode NFC.
  It is 2–24 Unicode scalar values, starts with a letter, and contains only letters (general
  category L), single spaces, apostrophes (U+0027, U+2019) and hyphens, with no two
  non-letters next to each other and no non-letter at the end. Digits are not allowed. The
  NFC form must also be a protocol `NAME` (`tessera-protocol.md` §5.2, at most 64 bytes of
  UTF-8; a few letters grow under NFC, for example U+FA6C becomes the four-byte U+242EE). The
  names `System` and `Server` are reserved.
- **Name key.** The full Unicode lowercase mapping of the NFC name. Names are unique across
  the server by key (`NAME_TAKEN`); `Tell` recipients and `@name` teleports (§8.4) are matched
  by key.
- **Creation** (`CharacterCreate`): checks in order: creation enabled (`DISABLED`), name rule,
  name taken, `max_characters` (`TOO_MANY`), appearance policy. The new character starts at
  the default spawn with instance 0 (never entered) and is written to its account's file
  before `CharacterList` is sent.
- **Appearance policy** (`APPEARANCE_REFUSED` on creation; on `AppearanceSet`, the old
  appearance comes back with a system line): an `Object` body is refused unless
  `allow_object_bodies` is true. Nothing else about an appearance is judged, because the
  server cannot know what a reference means (`server-content.md` §4.1 R2). The protocol's own
  limits are enforced by the decoder.
- **Starting properties** on world entry: `STANCE` absent (Relaxed), `WALK_SPEED` and
  `RUN_SPEED` from `[movement]` (`tessera-protocol.md` §7.3).

<a id="s6-5"></a>
### 6.5 Passwords and registration

- **Policy** for new passwords (`account add`, `account passwd`, `Register`): valid UTF-8 of
  `password_min_bytes`–256 bytes, and not equal to the account name ignoring ASCII case. No
  composition rules. Failing it gives `PASSWORD_REJECTED` for `Register`.
- **Registration** (`[server] registration`, off by default, private addresses only, §4.2):
  `Register` creates the account under the accounts lock and logs in. `ACCOUNT_EXISTS` if the
  canonical name is taken. Registrations count against the address budget of §6.2 step 1 like
  logins.

---

<a id="s7"></a>
## 7. World instances

- A server runs `instances` world instances, numbered from 1. Each has its own entity set,
  wanderers, game clock (`tessera-protocol.md` §11.3), Yell scope and interest management.
  Instances share accounts, character names and `Tell`.
- **Assignment** at `EnterWorld`: (1) the character's stored instance, if it exists in the
  current configuration and has room; (2) otherwise the lowest-numbered instance with room;
  (3) otherwise `EnterWorldFailed(NO_INSTANCE)`. The instance number goes into `WorldEnter`
  and is stored with the character.
- **Capacity** is `instance_capacity` characters in the world. Wanderers do not count.
- v1 has no way to change instance during a session (`tessera-protocol.md` §9.5 treats a
  change like a region change; no command uses it yet).

---

<a id="s8"></a>
## 8. Simulation

<a id="s8-1"></a>
### 8.1 Threads and tick

| Thread | Count | Does |
|---|---|---|
| Network | 1 | tokio `current_thread` runtime with the QUIC endpoint and one task per connection: framing, decoding, encoding, flow control (decision 0030) |
| Simulation | 1 | Owns all world state. Runs the 30 Hz tick |
| Hash workers | `hash_workers` | Argon2id computations (§6.2), and the read-modify-write of `accounts.toml` that goes with a registration or a re-hash, under the accounts lock (§3.7) |
| Writer | 1 | Every other file write, atomically (§3.7, §12.2) |

The threads talk over bounded channels only. Neither the network nor the simulation thread
ever computes a hash or touches a file.

**Tick** (every 1/30 s of server time, `tessera-protocol.md` §11.1). Tick *k* is due at
server time ⌊*k* · 10⁶ / `tick_rate`⌋ µs (§11.1 there), computed from *k*; a deadline is never found by adding a rounded
period to the previous one (which drifts and can repeat tick numbers). In order:

1. Drain inbound queues: at most 256 control messages over all connections per tick, the
   rest wait (oldest connection first, round robin); all queued movement.
2. Apply logins, world entries, teleports, chat, emotes, stances, appearances, movement
   (§8.3). For a connection that closed after its events were queued, its queued `ChatSend`,
   `EmoteSet`, `StanceSet` and `AppearanceSet` and its newest `MoveInput` are still applied,
   then its character leaves the world and is saved (so a last chat line reaches its
   recipients and the saved position is the last accepted one); its queued `Login`,
   `Register`, `CharacterCreate`, `EnterWorld`, `TeleportRequest` and `QueryLocation` are
   dropped, since no answer can arrive.
3. Advance wanderers (§5.4).
4. Interest: re-evaluate the pairs whose interest block changed; also check 1/30 of the
   observers in full, rotating, so that every observer is fully checked once per second
   (`tessera-protocol.md` §9.2).
5. Emit property and appearance changes (coalesced per tick, `tessera-protocol.md` §7.2),
   creates and destroys within the budgets (`[world]`, §9.4 there).
6. On every (30 / `snapshot_rate`)-th tick: build `MotionBatch`es (`tessera-protocol.md` §8.5).
7. Hand outbound work to the network thread.

**Overrun.** If a tick ends after the next tick's start time, the next one starts at once. If
the simulation is more than 5 ticks behind, it skips to the current tick (tick numbers keep
following server time) and counts the skipped ticks. Overruns are counted and logged at warn
at most once per 10 s.

**In-world inactivity.** A player is active when the server accepts a `MoveInput` that moves
it at least 0.1 m from its last active position, or receives any control message other than
`Ping`. With `idle_timeout_min` > 0, a player inactive for that long gets
`Disconnect(IDLE)`.

<a id="s8-2"></a>
### 8.2 Interest management

The protocol's rules (`tessera-protocol.md` §9) with these parameters: radius
`interest_radius` (default 2 blocks, a 5 × 5 square), leave at *R* + 1 (hysteresis), created
entities per connection at most min(`max_entities` from `Hello`, `max_entities_cap`),
`creates_per_tick` and `world_bytes_per_tick` per connection, `MotionBatch` budget 16
datagrams per observer per snapshot. Candidates are the players and wanderers of the same
instance and region.

<a id="s8-3"></a>
### 8.3 Movement validation

The protocol's procedure (`tessera-protocol.md` §8.3) with the `[movement]` parameters, plus
two server rules: a settling grace and a descent credit. Both are relaxations that §8.3 there
permits within stated bounds ("Server relaxations", revised 2026-10-09): a settling window of
at most 10 s that skips only the up and down tests, and a descent credit of at most 100 m. Besides the protocol's state the
server keeps, per player, the **descent credit** *C* in metres, 0 ≤ *C* ≤ *S*. *C* is 0 at
world entry and at the start of every epoch (the client sets its velocity to 0 on every
teleport and correction, `tessera-protocol.md` §8.4) and never grows with time. *A*ₕ, *A*ᵤ
and *A*_d refill and are capped exactly as there (at *V* · *W*; *A*ₕ never holds more).

Steps 5 and 6 there become, with *d*ₕ, *d*ᵤ and *d*_d as defined there:

5. The **counted descent** is *d*_c = min(*d*_d, *A*_d), the part of the descent that the down
   allowance covers (outside the settling grace that is all of *d*_d, since a larger descent is
   rejected). Reject (step 7) when *d*ₕ > *A*ₕ + *d*_c + *C*; outside the settling grace, also
   when *d*ᵤ > *A*ᵤ or *d*_d > *A*_d.
6. On acceptance, with *x* = max(0, *d*ₕ − *A*ₕ), the horizontal distance that *A*ₕ does not
   cover: *A*ₕ ← max(0, *A*ₕ − *d*ₕ), *A*ᵤ ← max(0, *A*ᵤ − *d*ᵤ), *A*_d ← *A*_d − *d*_c and
   *C* ← min(*S*, *C* + *d*_c − *x*). The rest of step 6 there is unchanged.

So the horizontal allowance pays first, the move's counted descent next and the credit last,
and descent the move does not need is kept in *C*. With *S* = 0 the credit is off: *d*_c and
*C* drop out of the horizontal test, which is the protocol's again.

- **Settling grace.** The server has no terrain (`server-content.md` §3.2), so a spawn or
  teleport height may differ from the ground a client sees. For `settle_grace_s` of server
  time after an epoch starts at a `ground` spawn (world entry or a `Named` teleport to one)
  or at a `Ground` destination, the up and down tests of step 5 are skipped (at world entry
  whenever the character is placed at a `ground` spawn, whatever instance it was stored in): only the
  horizontal test (with its counted descent and credit) and the extent check apply. Step 6
  still lowers *A*ᵤ and *A*_d, so a move counts at most *A*_d ≤ *V*_d · *W* = 120 m of descent
  toward the credit however far it claims to drop, and *C* stays capped at *S*. A slide that
  starts right after a ground spawn or a `Ground` teleport therefore gets its credit as it
  would anywhere else, and the grace adds nothing to what a cheat can bank. The vertical
  velocity clamp still applies to what is relayed. The grace is measured from the start of the
  ground epoch: a correction inside it (horizontal or extent) starts a new epoch at the last
  accepted *P* and clears *C* as every new epoch does, but neither ends nor restarts the grace,
  which runs until `settle_grace_s` after the ground epoch began.
- **Descent credit (slides).** Horizontal speed above *V*ₕ arises only from gravity on
  steep faces (normals 45°–85° from up, `collision.md` §11.3, §12.4 (q), (t); there the Slope
  layer applies, gravity 10 m/s², no control acceleration) and is then carried into the air
  and onto walkable ground. Along a face of 45° or steeper the horizontal speed is at most
  the vertical one, so every move of a slide counts at least as much descent as it covers
  horizontally, and in the air after leaving a face gravity only adds descent. What follows
  the descent, the **run-out**, is paid from *C*. *S* = 58 m covers the longest run-out that
  the default records allow (`collision.md` §10.3), found by stepping the controller of
  `collision.md` §12.3 at 1/60 s (Tessera's own calculation, 2026-10-08):
  - **Fastest horizontal speed.** Every speed is limited to the terminal velocity, 50 m/s
    (Slope and Falling list none, so BasePlayer's applies). On a face the horizontal part is at
    most 50 × cos 45° ≈ 35.4 m/s. Landing from a face or a fall on a walkable plane keeps the
    horizontal length of the clipped velocity (`collision.md` §12.4 (v)), at most
    50 × (1 + cos 45°) / 2 ≈ 42.7 m/s (a 45° face onto a 22.5° plane).
  - **Run-out from 42.7 m/s**, horizontal distance beyond *V*ₕ · *t* less the descent it
    counts: about 7 m when it brakes on walkable ground (at least 38.5 m/s² plus the damper of
    0.2 per second, `collision.md` §12.4 (a), (b)); about 26 m when it first runs off a ledge
    (the Falling layer brakes at only 10 m/s² per axis, toward at most 5 m/s, with no damper);
    about 18 m into water; about 45.5 m when the walker jumps at every landing (5.2 m/s,
    `collision.md` §12.4 (c); each hop is about 0.7 s in the air and one step on the ground,
    and the air brakes only at 10 m/s²). From 35.4 m/s: about 4, 16 and 28 m. The
    implementer's independent model (ground braking at 38.5 m/s² **without** the damper, air
    10 m/s² toward 5 m/s, jump 5.2 m/s, Falling gravity 15 m/s²) gives about 49 m for the
    jumping run-out and about 15 m for level braking; leaving the damper out lengthens both, so
    49 m is the conservative figure and is the one used below.
  - **Late moves.** During a run-out *A*ₕ is near 0, so a move that arrives late pays from *C*
    what *A*ₕ would have refilled meanwhile; *S* adds one window, *V*ₕ · *W* = 8.5 m, for that:
    49 + 8.5 = 57.5, rounded up to 58 m (revised 2026-10-09 from 56 m, which took the 45.5 m
    figure).
  - **The credit fills in time.** On a face *d*_c ≥ *d*ₕ, so *A*ₕ + *C* grows by at least *V*ₕ
    per second of sliding (the allowance pays first, so none of its refill is lost), and by
    all the descent while the mover is slower than *V*ₕ. Reaching a horizontal speed *v* on a
    45° face takes at least *v* / 5 s (10 × sin 45° m/s² along the face), which leaves
    *A*ₕ + *C* ≥ min(8.5 + 1.7 *v*, 66.5) m at its foot, more than the run-out from *v* needs at
    every speed: 42.7 m/s needs 50 m/s along the face, at least 7 s of sliding. Steeper faces
    fill it faster. No face just steeper than 45° leaves too little credit.
  - **Not covered.** The Slide and Ice layers (terminal velocity up to 100 m/s, braking at
    6.4 m/s² or less) apply only while `Physics_OnSlide` or `Physics_OnIce` is set
    (`collision.md` §11.1 steps 5–6), properties that no Tessera server sends; and movement
    records other than the observed ones of `collision.md` §10.3 (it accepts speeds up to
    120 m/s), which a client reads from its own install.

> **Answered (2026-10-09, spec writer, design; was OPEN, addressed to `tessera-protocol.md`):**
> `tessera-protocol.md` §8.3 now permits both relaxations within bounds ("Server relaxations":
> a settling window of at most 10 s after an epoch that starts at a position whose height the
> server does not know, skipping only the up and down tests; a descent credit pool of at most
> 100 m used as here), and its invariant 8 counts them. The defaults here (3 s, 58 m) lie within
> those bounds, so `tessera-server` conforms with its default configuration. No bytes change.

> **OPEN:** (spec writer, 2026-10-08; restated 2026-10-10) run-outs that **jump** up a rising
> walkable slope have not been stepped. A run-out onto a rising walkable slope is lifted onto
> it (`collision.md` §12.4 (v)) and climbs at up to its horizontal speed (45°). Braking on the
> ground it stays within *V*ᵤ: about 8 m of rise from 35.4 m/s against *V*ᵤ · (*t* + *W*)
> ≈ 13.8 m. With a jump at every landing the rise was not computed. What is at stake: if such
> a run-out rises faster than *V*ᵤ outside the settling grace, the up test of step 5 corrects
> a legitimate player (back to the last accepted position); no cheat is accepted, because the
> up test only refuses. To close it: step the `collision.md` §12.3 controller at 1/60 s with
> the default records for a run-out that jumps at every landing, from 35.4 m/s and 42.7 m/s,
> onto walkable slopes from 0° to 45°, and compare the rise over every stretch of *t* seconds
> with *V*ᵤ · (*t* + *W*); or count corrections that follow slides in soak runs (§13). Until
> then *V*ᵤ = 10 m/s stays the default.

The server adds no terrain or collision checks of its own in v1.

**Parameters and the protocol's bounds.** The `[movement]` values are fixed for a run (§4.1),
so `tessera-server` never changes *V*ₕ, *V*ᵤ, *V*_d, *W*, *S* or the grace while players are in
the world. Their ranges (§4.5) keep the relaxations inside the bounds of
`tessera-protocol.md` §8.3 ("Bounds of the values in force": window at most 10 s, *S* at most
100 m); a configuration outside them is refused at load (§4.1), so no value is ever clamped.
`settle_grace_s` = 0 opens no window, and `slide_reserve` = 0 keeps no credit. Should a later
version change the parameters at run time, it follows `tessera-protocol.md` §8.3 "Changing the
parameters during an epoch": the grace keeps the start of its ground epoch and takes the new
length, a length of 0 or an end already passed closes it for good, and the allowances and *C*
are capped by the new values at once.

<a id="s8-4"></a>
### 8.4 Teleports

- **Policy** (`[world] teleport`): `off` refuses every `TeleportRequest` with `NOT_ALLOWED`;
  `named` accepts only `Named` destinations; `any` accepts every destination kind
  (`tessera-protocol.md` §5.3), in any region number the protocol allows.
- **`Named` places.** The name is trimmed of surrounding whitespace, then:
  - if it starts with `@`, the rest is a character name, matched by key (§6.4). The target
    must be online and in the same instance, and `teleport_to_players` must be true.
    Destination: the target's last accepted position, its anchor and heading, exactly
    (no settling grace). Unknown or offline: `UNKNOWN_PLACE`. Other instance or disabled:
    `NOT_ALLOWED`;
  - otherwise it is compared, ignoring ASCII case, with the content's spawn names. A match
    gives the spawn's position, facing and anchor (with the settling grace when the spawn is
    `ground`). No match: `UNKNOWN_PLACE`.
  The server knows no other place names: it has no game data (`server-content.md` §3). A
  client that understands the data's place names or `/loc` text resolves them itself and
  sends a numeric destination.
- **`Ground` destinations** keep the given height (the server has no terrain,
  `tessera-protocol.md` §8.4) and start the settling grace.
- **Extent.** A `Location`, `Ground` or `Anchored` destination whose x or y lies outside the
  movement extent of `tessera-protocol.md` §8.3 (each in [−1,280, 42,080]) is refused with
  `TeleportRefused(INVALID_DESTINATION)`: every later `MoveInput` there would be corrected.
- **Rate.** At most one executed teleport per player per `teleport_interval_ms`. A request
  that arrives earlier waits until the interval has passed; a newer request supersedes it
  (`CANCELLED`, `tessera-protocol.md` §6.2.12). Every request is answered within 10 s.
- Region changes and far teleports send `InterestReset` as `tessera-protocol.md` §9.5 requires.

<a id="s8-5"></a>
### 8.5 Location descriptions

The optional system line answering `QueryLocation` (`tessera-protocol.md` §6.2.13) is the
server's own text, numbers only: instance, region, landblock, block-local x, y, z (one decimal),
the compass facing in whole degrees, and "interior" when anchored outside its square. When a
content spawn lies within 160 m in the same region, it adds that spawn's name. It never names
game places (the server knows none).

---

<a id="s9"></a>
## 9. Chat relay

The protocol's channels, delivery and limits (`tessera-protocol.md` §10) with the `[chat]`
parameters:

- Say and Emote reach characters of the same instance and region within `say_range_m` and
  `emote_range_m` (horizontal distance). Yell reaches the instance, or with `yell_scope =
  "region"` the speaker's region in it. Tell reaches the named online character in any
  instance.
- The token bucket and the flood rule use `bucket_capacity`, `refill_s`, `flood_rejections`
  and `flood_window_s`.
- A muted account (§3.4) gets `ChatRejected(MUTED)` for every channel.
- The server never changes chat text beyond the protocol's sanitising, and never stores it.
  It is logged only with `log_chat` (§11.2).

---

<a id="s10"></a>
## 10. Resource limits

<a id="s10-1"></a>
### 10.1 Limits and what happens when they are exceeded

| Resource | Limit | When exceeded |
|---|---|---|
| QUIC handshakes in progress | `max_handshakes` | New connection attempts are refused at the QUIC level (CONNECTION_REFUSED) until one completes. Clients see a connection failure |
| QUIC handshakes in progress from one source IP address | **4**, fixed (not configurable); loopback addresses are exempt | A fifth attempt from that address is refused at the QUIC level, as above, until one of its four completes or fails |
| Connections alive past the handshake, admitted or not | `max_connections` + `max_handshakes` | New attempts are refused at the QUIC level. (A connection refused with `SERVER_FULL` stays alive until its `Hello` is answered and the closing linger ends, `tessera-protocol.md` §12.2; each counts its 64 KiB towards §10.2) |
| Connections past the handshake | `max_connections` | The server answers `Hello` with `Disconnect(SERVER_FULL)` |
| Connections per source address | `max_connections_per_address` (loopback exempt) | As above, `SERVER_FULL` |
| Characters in an instance | `instance_capacity` | Assignment tries the next instance; none: `EnterWorldFailed(NO_INSTANCE)` |
| Inbound control messages per connection | `control_queue` | Counted **per message**, not per read: the network thread feeds received control bytes to the protocol session one frame at a time and stops decoding once `control_queue` events wait for the simulation; the rest stays buffered unread, and QUIC flow control pushes back. Full for 5 s: `Disconnect(SLOW_CONSUMER)` (`tessera-protocol.md` §13.3) |
| Inbound movement per connection | The newest `MoveInput` only (a queue of 1, newest wins), plus the rate limit of `tessera-protocol.md` §13.1 | Older unprocessed movement is dropped and counted |
| Outbound world stream per connection | The protocol's §9.4 backlog rules | Creates and updates pause; persistent: `Disconnect(SLOW_CONSUMER)` |
| Outbound chat stream per connection | 256 KiB unsent | Above it for 30 s, or above 1 MiB: `Disconnect(SLOW_CONSUMER)` |
| Login verifications | `hash_workers` running, 64 waiting | `LoginFailed(RATE_LIMITED)` (§6.2) |
| Writer queue | One pending job per file (a newer snapshot replaces an older one); at most 4,096 files | Above it, the simulation skips the periodic save round, the save after teleports and the save at world entry, and logs a warning. The save when a character leaves the world and the shutdown saves are never skipped: they wait for room. A shutdown save that cannot be queued counts as a failed save (§12.1 step 6) |
| Accounted memory | `memory_budget_mib` | §10.2 |

**Admission of a new QUIC connection attempt.** The network thread checks, in this order,
and the first check that fails ends the attempt:

1. The server is accepting (not shutting down, §12.1); otherwise refused.
2. Fewer than `max_handshakes` handshakes are in progress; otherwise refused.
3. Fewer than `max_connections` + `max_handshakes` connections are alive past the
   handshake; otherwise refused.
4. Address validation under pressure: once at least half of `max_handshakes` are in
   progress, a source whose address QUIC has not yet validated is answered with a QUIC Retry
   instead of a handshake (refused where the library cannot send one), so spoofed source
   addresses cannot hold handshake slots. The retried attempt comes back validated and
   passes this check.
5. Fewer than 4 handshakes are in progress from the source IP address (not checked for
   loopback addresses); otherwise refused. The attempt then counts towards this cap and
   `max_handshakes` until its handshake completes, fails or times out
   (`tessera-protocol.md` §12.2, Accepting, 10 s).

The per-address handshake cap bounds how fast one address may open connections, not how
many it holds (that is `max_connections_per_address`). It is a fixed rule rather than a
`[limits]` key so that clients can rely on it: a client that opens several connections from
one address at once, such as `tessera-loadtest` provisioning accounts (§13.1), keeps at most
4 handshakes in progress. Loopback is exempt for the same reason as in
`max_connections_per_address`: in-process load tests and a local client share one address.

<a id="s10-2"></a>
### 10.2 Memory budget

The server **accounts** for the memory it controls: every connection's unsent world and chat
bytes, its queued inbound control events at 1 KiB each (the network thread queues decoded
events, not frames; a frame is at most 64 KiB and typical control messages are far smaller), a
fixed 64 KiB per connection for its state, and `argon2_memory_kib` × `hash_workers` for
hashing. (The QUIC library's own buffers are bounded
by the flow-control windows of `tessera-protocol.md` §2.1 and are not counted separately.)

- At 80 % of the budget, new connections get `Disconnect(SERVER_FULL)` after `Hello`.
- At 100 %, the server disconnects the connection with the largest unsent backlog with
  `SLOW_CONSUMER`, and repeats each tick until the accounted total is below 90 %. A connection
  that is already ending (its `Disconnect` is queued) is not chosen again.
- The accounted total is reported in the statistics line (§11.1). The soak tools compare it
  with the process's resident memory (§13.3).

---

<a id="s11"></a>
## 11. Logging

<a id="s11-1"></a>
### 11.1 Events

`tracing` events, one line each, in `text` or `json` format (§4.8). At info level:

- start: version, protocol version, listen addresses, fingerprint, content `id`, instances;
- connection accepted (connection number, source address) and the **"connection closed"
  line** (below);
- login success (canonical account, source address), registration, world entry (account,
  character, instance), leaving the world;
- kicks, `SLOW_CONSUMER`, `FLOODING`, `SERVER_FULL`, refused teleports at debug;
- every `stats_interval_s`, the **statistics line** (below).

At warn or error: configuration problems, failed saves, overruns, reload failures.

**"Connection closed" line** (revised 2026-10-10). Every accepted connection gets exactly one
such info line, written once the connection has ended, whatever the cause (a `Disconnect`, a
kick, a protocol error, an idle timeout, a lost peer or shutdown). Fields:

| Field | Meaning | Unit |
|---|---|---|
| connection number | The number given in this connection's "connection accepted" line | — |
| source address | The peer's address and port, as in the accepted line | — |
| duration | Time from the moment the server takes up the connection attempt (its arrival, before the QUIC handshake) to the moment this line is written (after the drain wait below), rounded down. It therefore includes the handshake, which precedes the "connection accepted" line, and the drain wait | ms |
| bytes in | Bytes received on this connection from its acceptance to its end, counted as the statistics line counts bytes in (below): the protocol's stream frames and datagram messages as read from QUIC | bytes |
| bytes out | Bytes sent on this connection from its acceptance to its end, counted as the statistics line counts bytes out: the stream frames and datagram messages handed to QUIC, without the kind byte that opens each server-initiated unidirectional stream | bytes |
| code and reason | How the connection ended: the application close code and reason of the `CONNECTION_CLOSE` that ended it (`tessera-protocol.md` §14.1), whichever end sent it: the peer's code and reason when the peer closed, the code and reason the server sent when the server closed (a kick, `SERVER_FULL`, `SLOW_CONSUMER`, `FLOODING`, a protocol error, `SERVER_SHUTDOWN`). The line SHOULD also say which end closed. Only when no application close ended the connection (an idle timeout, a reset, a transport error) does the field give the transport's description of the end instead. The parts MAY be written as one field. See §15.3 for the current server | — |

Bytes in and out are this connection's own totals, never shares of the server's figures and
never rates; summed over every connection they equal the bytes the statistics lines count
over the same span, apart from the case of the drain bound below.

**Drain bound.** Outgoing data of a connection is handed to QUIC by its stream writers (one
for the control stream, one per open server-initiated unidirectional stream). Before writing
the closed line the server MUST wait for every writer of that connection to finish, so that
the line's bytes out include every byte they handed to QUIC, and it MUST wait **at most 1 s**
in total (measured from the end of the connection). When the bound passes with a writer still
running, the line is written at once with the bytes counted up to that moment; bytes that
writer hands to QUIC afterwards still count in the statistics line but not in this
connection's closed line. The wait runs in that connection's own task: it MUST NOT block the
simulation thread or the handling of any other connection. The connection keeps its places
in `max_connections`, in `max_connections_per_address` and in the statistics line's
connection count until the wait ends; they are released when the closed line is written. So
in the bound case a new connection from the same address can still be refused with
`SERVER_FULL` for up to 1 s after the old one ended. Writers normally end at once, because
the connection's queues are closed when it ends; the bound only guards against one that does
not.

**Statistics line** (revised 2026-10-10). With `stats_interval_s` > 0 the simulation thread
writes one info line per **interval**. The first interval starts when the simulation starts;
the line is written at the first tick whose start time is at least `stats_interval_s` seconds
of server time after the start of the current interval, and the next interval starts at the
server time of that line. The **server time of a line** is the server time at which the tick
that writes it starts (read when the tick begins, which may be later than its due time,
§8.1); so the interval boundaries are the start times of the ticks that write the lines, and
an interval's length is the difference between two such start times (the first interval's
start is the server time at which the simulation starts). With `stats_interval_s` = 0 no line
is written. Fields:

| Field | Meaning | Unit |
|---|---|---|
| connections | Three counts at the time of the line: connections the simulation knows; of them, those past login (`LoginOk` sent); of those, the ones in the world (`tessera-protocol.md` §12.2 states) | count |
| characters per instance | Characters in the world, one figure per instance in instance order, at the time of the line | count |
| entities | Entities in all instances (characters and wanderers), at the time of the line | count |
| interest pairs | (observer, entity) pairs in interest, at the time of the line | count |
| last tick | Duration of the most recently completed tick | µs |
| longest tick | The largest duration among the ticks completed during this interval; 0 if none completed. It is reset when the line is written, so each line reports only its own interval (the longest tick since start is a figure of the load and soak reports, §13.4, not of this line) | µs |
| overruns | Ticks that ended after the next tick's start time (§8.1), counted since the server started | count |
| bytes in, bytes out per second | The bytes received (in) and sent (out) during this interval, multiplied by 10⁶ and divided by the interval's length in µs, rounded down; 0 if the length is 0. Bytes are those of the protocol's stream frames and datagram messages (`tessera-protocol.md` §5.4, §5.5) over all connections, as read from and handed to QUIC; QUIC, TLS, UDP and IP overhead is not counted, nor is the kind byte that opens each server-initiated unidirectional stream (`tessera-protocol.md` §2.3 rule 1; it is not a frame) | bytes per second |
| accounted memory | The §10.2 accounted total at the time of the line | bytes |
| resident memory | Only where the platform reports it | bytes |

Percentiles of the tick duration (p50, p99 over the interval) MAY be added as further fields.
No field of the line is a total since start except overruns.

<a id="s11-2"></a>
### 11.2 Never logged

- passwords, at any level, including failed attempts and the bodies of `Login` and
  `Register` (debug-level message traces leave those bodies out entirely);
- password hashes and salts, and the server's private key;
- the account name of a **failed** login, unless it names an existing account (a mistyped
  password in the name field must not reach the log). A failure is logged with the source
  address and "unknown account" or the canonical name;
- chat text, unless `log_chat` is true; then each line is logged at info under its own
  target with channel, sender and (for Tell) recipient;
- appearances in bulk (`server-content.md` §4.1 R4). A debug trace of one message may show
  it.

---

<a id="s12"></a>
## 12. Shutdown and persistence

<a id="s12-1"></a>
### 12.1 Graceful shutdown

Triggered by SIGINT or SIGTERM on Unix, and by Ctrl+C on Windows (Ctrl+Break and console close
SHOULD trigger it too).

1. Stop accepting connections.
2. Send `ServerNotice` with level Shutdown and `seconds` = remaining grace to every logged-in
   connection (`tessera-protocol.md` §6.4.2), at once and again when 60, 30, 10 and 5 seconds
   remain (only marks inside the grace).
3. Play continues until `shutdown_grace_s` has passed, or until a second signal.
4. Stop the simulation, save every character in the world and the world state, and wait for
   the writer to finish.
5. Send `Disconnect(SERVER_SHUTDOWN)` to every connection and follow the closing procedure
   (`tessera-protocol.md` §12.3), waiting at most 2 s.
6. Close the endpoint, flush logs, release the server lock, exit 0 (1 if a save failed).

A third signal exits at once with status 1 after logging what was not saved.

<a id="s12-2"></a>
### 12.2 Persistence of characters

A character's file is written (through the writer, §8.1; which writes may be skipped when the
writer queue is full is §10.1):

- when the character leaves the world (disconnect, kick, idle, replaced login), never skipped;
- 5 s after a teleport (one write for a burst of teleports);
- every `save_interval_s`, for each character in the world that has moved at least 1 m or
  changed appearance since its last save, together with `world-state.toml`;
- on creation, appearance changes and removals;
- at world entry (the instance it was placed in);
- at shutdown (§12.1 step 4), never skipped.

The position saved is the last accepted one, with its anchor and heading, and the instance.
After a crash, at most `save_interval_s` of movement is lost. Nothing else about a session
(chat, emotes, stance, epochs) is persisted.

---

<a id="s13"></a>
## 13. Load and soak tools (`tessera-loadtest`)

<a id="s13-1"></a>
### 13.1 What it runs

`tessera-loadtest` drives many headless **bots** through the same `tessera-net` test-server
backend the client uses, against a remote server (`--server tessera://…?pin=…`) or an
**in-process** server on loopback (`--in-process`, with a temporary data directory,
generated accounts `bot-0001`… and Argon2id at the configuration floor, §4.3). Bots have no
game data: they use the content's `void` or `start` spawn and invented appearances made only
of numbers chosen by the tool, never resolved.

| Scenario | Bots do |
|---|---|
| `connect-storm` | Connect, log in and enter the world as fast as the limits allow; measures admission |
| `walk` | Walk or run on random paths inside a square of K × K landblocks around the spawn, at 2 or 6 m/s, following the client send rules (`tessera-protocol.md` §8.2), with stops and turns |
| `chat` | Say, Yell and Tell at a configured rate within the chat limits |
| `churn` | Disconnect and reconnect at random intervals with `Disconnect`; abrupt loss (no `Disconnect`) is exercised by the hostile mode's attackers, because the client backend always sends one |
| `cheat` | Send moves above the limits; every one must be handled (below). Stale-epoch moves are exercised by the hostile mode's pre-login `MoveInput` floods, because the client backend stamps the current epoch |
| `soak` | A mix for a long duration: default 1 hour, 16 bots, 70 % walking, one chat line per bot per 30 s, 10 % of the bots reconnecting every 5 minutes, a teleport per bot per 10 minutes |

Bots in every scenario except `cheat` stay within the limits, so any correction of such a
bot is a desync (§13.2).

The tool provisions accounts (registration, character creation) at most 4 at a time, so that
it never has more handshakes in progress from one address than the server's per-address cap
allows (§10.1).

- **Hostile mode on loopback.** The login budget applies to loopback (§6.2 step 1), so the
  hostile clients bind a second loopback address (for example `127.0.0.2`) against a loopback
  server to keep their failed logins out of the honest bots' budget; where no second address
  can be bound they share `127.0.0.1` and the in-process server's
  `attempts_per_address_per_min` is raised to 10,000.
- **`cheat` verdict.** An over-limit move is **handled** when a `CORRECTED` report puts the bot
  back within 1.5 m of where it was, within 2 s after the move (the bot holds the move for
  200 ms so that the newest-move slot sends it), or, in-process, when the server's
  dropped-move counter covers the unanswered moves. A correction elsewhere fails the run, and
  any observer sample within 5 m of the refused target during the following 3.7 s is a desync
  of kind **Cheat** (§13.2). Remote, a dropped move can be told from an accepted one only
  through the observers.
- **Walking area.** Interest leave needs pairs more than *R* + 1 blocks apart, so the `walk`
  square is at least *R* + 3 blocks wide, and 2*R* + 4 (8 for *R* = 2) for every far teleport
  to leave range from anywhere. The report adds a coverage note when the area is too small,
  when far teleports fell short, or when no observer–entity pair left range. Region and
  instance changes are not exercised (one region).
- **Scale.** The tool checks every observer–entity pair; it measures its own loop and its
  achieved sampling rate, marks timing indicative when the loop overruns its tick in more than
  5 % of iterations, and declares a run **invalid** (a failure) when sampling falls below 9 Hz.
  The tool MAY state the largest bot count it supports on a machine instead of scaling
  further.

<a id="s13-2"></a>
### 13.2 Desync

For every observer bot *O* and every moving entity *E* that *O* should see, the tool compares
what *O* shows with the truth. The truth for a bot is its own simulated trajectory (it sends
exactly what it simulates); for a wanderer it is the formula of §5.4. Times are mapped
through each bot's clock-offset estimate (`tessera-protocol.md` §11.2); in-process runs share
one monotonic clock. Samples are taken at 10 Hz per observer, at the observer's render time
*t* = estimated server time − *D*, where *D* is the observer's actual interpolation delay
(`tessera-protocol.md` §8.6; at least 2 / `snapshot_rate` + 1 / `tick_rate`, 166.7 ms at
15 Hz and 30 Hz), reported in the summary.

A client cannot learn a wanderer group's start time, so the phase of its circle is unknown:
wanderers are **not** judged for the Position, Rest and Motion kinds; they are judged for
Missing, Ghost and State like other entities.

A **desync** is any of these, each episode counted once:

| Kind | Condition |
|---|---|
| Position | The horizontal distance between *O*'s interpolated position of *E* at *t* and *E*'s true position at *t* exceeds 1.0 m + 0.1 s × *E*'s true speed for 1 s or longer |
| Rest | *E* has been at rest for 1 s, and *O*'s position of *E* is more than 0.05 m off for a further 1 s |
| Missing | *E* is within *R* blocks of *O* (same instance and region) for 2 s, and *O* has not created it, while *O*'s created count is below its entity limit. The 2 s hold of the *k*-th nearest candidate is extended by ⌈*k* / `creates_per_tick`⌉ ticks (the protocol creates at most that many per tick, nearest first, `tessera-protocol.md` §9.4); in-process, Missing is not judged for 2 s after the server counted an overrun or a skipped tick |
| Ghost | *O* still has *E* created 2 s after *E* left *R* + 1 blocks, left the instance or region, or left the world |
| State | *O*'s view of *E*'s appearance or `STANCE` differs from the server's for 2 s after a change (`EMOTE` is not compared: it is an opaque animation key whose start time the client estimates) |
| Motion | While *E*'s true speed is at least 0.5 m/s, *O*'s heading of *E* is more than 0.35 rad off, or *O*'s gait of *E* differs from the gait that `tessera-protocol.md` §8.1 derives from the true speed and the configured `WALK_SPEED` and `RUN_SPEED`, for 1 s (this is what ROADMAP M7 criterion 1 calls "animate") |
| Chat | A line that the rules say *O* must receive arrives later than 2 s, never, twice, or out of order with another line of the same speaker. *O* counts as a certain recipient only when it was in the world from 1 s before to 2 s after the line was sent and, for Say, did not teleport in that time; Say recipients within ±6 m of the Say range (one second at run speed) are not judged, and those beyond it must not receive the line |
| Cheat | An observer sample within 5 m of a refused `cheat` target during the 3.7 s after the move (§13.1) |
| Correction | A bot that stayed within the limits receives a `CORRECTED` location report |

Missing, Ghost and Yell need the observers' instance. A remote run that cannot learn it
(the client backend does not report the instance of `WorldEnter`) judges them as if all bots
were in one instance and adds a coverage note; the in-process server sizes
`instance_capacity` to the bot count. Remote runs take the walk and run speeds from the
player's `WALK_SPEED` and `RUN_SPEED` properties and need `--say-range` for Say to be judged
beyond the speaker's echo (the protocol does not announce the range).

A run passes the desync criterion with **zero** desync events. The report lists every
episode with the bots, entity, kind, time and size of the error.

<a id="s13-3"></a>
### 13.3 Leaks

The tool samples every 10 s: resident memory (the processes named below), the server's
object counts from its statistics snapshot
(connections by state, characters in the world, entities per instance, interest pairs,
created-entity sets, inbound queue lengths, unsent bytes, pending hash jobs, pending writer
jobs, open network tasks, lockout and address-budget entries, cached character records),
and the same kind of counts for the bots' own `tessera-net` sessions.

A run is **leak-free** when all of these hold:

1. **Bounded counts.** Every count stays within the bound the scenario implies at each sample
   (for example, entities ≤ wanderers + bots; interest pairs ≤ bots × (bots + wanderers);
   network tasks ≤ connections + a constant).
2. **Return to baseline.** Within 10 s after the last bot has disconnected, every count
   except the throttle tables equals its value before the first bot connected. The throttle
   tables are empty `lockout_max_s` + 60 s later.
3. **Bounded memory.** After a warm-up of the larger of 10 % of the run and `--warmup-min`
   (default 5 minutes), capped at half the run (so that runs shorter than 10 minutes still get
   a verdict, marked indicative), a
   least-squares line through the resident-memory samples grows by less than the larger of
   16 MiB and 5 % of the warm-up maximum over the rest of the run, and no sample exceeds the
   warm-up maximum by more than that amount.

Criteria 1 and 2 judge the server's counts only where the tool can read them, that is
in-process; a remote run judges its own sessions' counts and says that the server's were not
judged. The throttle-table check of criterion 2 runs by default for runs of 5 minutes or more,
with the in-process server's `lockout_max_s` set to 30 s so that the wait is at most 90 s.

Resident memory alone is not trusted to fall after disconnections (allocators keep freed
memory), which is why criterion 2 uses counts and criterion 3 uses the trend.

**Which process criterion 3 judges.** Criterion 3 is about the server, so it judges the
process that holds the server whenever the tool can sample it:

| Run | Criterion 3 judges | Also sampled, reported, not judged |
|---|---|---|
| `--in-process` | The tool's own process, which holds the server **and** the bots; growth on either side fails the run | — |
| `--server` with `--server-pid` (a separate `tessera-server` on the same machine) | That server process | The tool's own process (the bots) |
| `--server` without `--server-pid` (the server's memory cannot be sampled) | The tool's own process (the bots' `tessera-net` sessions), as the only memory in view; the report says that the server's memory was not judged | — |

The in-process row holds the bots to the same bound as the server. That is intended: the
bots run the client's own network path (`tessera-net`, ARCH §19.4: one network thread per
process and one shared client endpoint per address family), and a client that grows with
every reconnection is a defect to fix there, not to exempt here. A growth that levels off
counts as bounded only if it stays within the allowance of criterion 3 after the warm-up;
there is no separate level-off rule. Verdicts are taken with the allocator's default
settings: tuning it through the environment (for example glibc's `MALLOC_ARENA_MAX` or
`MALLOC_MMAP_THRESHOLD_`) may help a diagnosis but does not turn a failed run into a pass.

<a id="s13-4"></a>
### 13.4 Other measurements and the report

- Login and world-entry latency; round-trip time (`Ping`); bytes per second per bot in each
  direction (QUIC UDP bytes); corrections, drops and their causes; the server's longest tick
  duration since its start (in-process runs, where the tool reads the server's counters; the
  §11.1 line reports only the longest of each interval) and overruns; CPU time of the server process. Where the client backend
  reports them, also `MotionBatch` inter-arrival times and loss.
- The report is one JSON file plus a short text summary. It records the tool and server
  versions, configuration, scenario, duration, the machine (CPU model, cores, memory) and the
  system load average at every sample.
- **Indicative results.** Performance numbers count as reference results only on the
  reference machine ([decision 0010](../decisions/0010-reference-machine-laptop.md)) with no
  other heavy load. The report marks a run **indicative** when it ran elsewhere, or when the
  1-minute load average exceeded half the core count at any sample (for example while other
  builds share the CPU). Desync and leak verdicts do not depend on this mark, but a desync
  that appears only under external load is noted as such.
- Exit status: 0 when the run passed (no desync, leak-free, no bot failed for a reason other
  than an expected one), 1 when it failed, 2 on usage errors.

---

<a id="s14"></a>
## 14. Invariants

1. The server never computes a hash or writes a file on the network or simulation thread.
2. No password, hash, salt or private key appears in any log line, at any level.
3. A server started with `registration = true` listens only on private addresses.
4. A shutdown that completes saves every character that was in the world; their next
   `WorldEnter` places them at their last accepted position, anchor and heading.
5. `run` never starts with content that `check-content` (minus `SPDX`) would reject.
6. Every count of §13.3 returns to its baseline after all clients leave.
7. The server never opens a file inside a game installation (§3.1).
8. Within one epoch, over any stretch of *t* seconds of server time, the horizontal distance a
   player's accepted `MoveInput`s cover is at most *V*ₕ · (*t* + *W*) + *S* + the descent they
   count (§8.3 *d*_c); with *S* = 0, at most *V*ₕ · (*t* + *W*).
9. Each statistics line (§11.1) reports its own interval: its longest tick is the largest
   duration among the ticks completed since the previous line (or the start), and its byte
   rates equal the bytes of that interval × 10⁶ / the interval's length in µs, rounded down.
   A tick in which a line is written completes after it and counts toward the next interval.
10. Every accepted connection (one whose QUIC handshake completed) produces exactly one
    "connection closed" line (§11.1), written at most 1 s after the connection ends. When its
    writers finished within that bound, the line's bytes in equal the bytes the server read
    from QUIC on that connection and its bytes out the bytes it handed to QUIC, as protocol
    stream frames and datagram messages (the stream kind byte excluded). These equal the bytes
    the peer sent and received only when both sides read everything before the close, for
    example in a test where the peer closes after the server has read all it sent and the
    peer has read all the server sent; a lost peer or a server that stopped reading makes
    them differ.

---

<a id="s15"></a>
## 15. Open items

> **OPEN:** Windows permission checks for the key and accounts files (§3.3). v1 makes none.

> **OPEN:** an operator console or admin commands while running (kick, broadcast, instance
> moves). v1 has only signals, the accounts-file reload and the CLI.

> **OPEN:** changing instance during a session (§7).

> **OPEN:** the defaults of §4 (instance capacity 64, tick budgets, chat ranges, settling
> grace 3 s, slide reserve 58 m) are starting points. The third M7 soak series passed with
> them (`docs/perf/m7-soak.md`); a soak between two machines on a LAN is still to run, as
> `tessera-protocol.md` §17 says for the protocol's defaults.

> **OPEN:** run-outs that jump up a rising walkable slope (§8.3, spec writer, 2026-10-08) were
> not stepped; §8.3 states the consequence (a correction of a legitimate player, never an
> accepted cheat) and the stepped calculation that would close it.

The former OPEN "settling grace versus `tessera-protocol.md` §8.3" is answered (§8.3).

<a id="s15-1"></a>
### 15.1 Implementer notes (`tessera-server`, 2026-10-07 and 2026-10-08), answered 2026-10-09

Each note's reading was reviewed by the spec writer (design) and written into the text; the
notes themselves are not repeated here.

| Note | Answer |
|---|---|
| §5.3 anchor of outdoor spawns | Adopted: outdoor spawns carry no anchor, interior spawns keep theirs; wanderers inherit (§5.3, §5.4) |
| §8.4 destinations outside the movement extent | Adopted: refused with `INVALID_DESTINATION` (§8.4) |
| §4.6 chat bucket and flood rule in the protocol session | Resolved without an API change: the `[chat]` keys may only tighten the protocol's fixed limits, so the configured values always act (§4.6) |
| §8.1 who writes `accounts.toml` for a registration or re-hash | Adopted: the hash worker, under the accounts lock; the writer does every other write (§8.1) |
| §10.2 "queued inbound frames" | Adopted: 1 KiB per queued control event; an ending connection is not chosen again (§10.2) |
| §11.1 statistics line, §12.1 Windows signals | §11.1 now requires the last and the longest tick (p50 and p99 optional); §12.1 requires Ctrl+C (Ctrl+Break and console close SHOULD). The rest is §15.3. Revised 2026-10-10: §11.1 defines the interval, the longest tick of the interval (reset at every line), the byte rates and the other fields exactly (§14 invariant 9), as `tessera-server` reports them since commit 9ce64c8 |
| §6.2 step 1 loopback and the login budget | Adopted: the login budget has no loopback exemption (§6.2 step 1, §4.3). Loopback is exempt from exactly two limits: `max_connections_per_address` (§4.7) and the per-address handshake cap of 4 (§10.1); the note's "only the connection limit" predates that cap |
| §10.1 `control_queue` per message | Adopted (§10.1) |
| §4.3/§6.5 no cap on the number of accounts | A 4 MiB size limit on `accounts.toml`, checked by every write (§3.4); no separate count limit |
| §10.1/§12.2 which saves may be skipped | Adopted: only the periodic round, the save after teleports and the save at world entry (§10.1, §12.2) |
| §8.1 events queued by a connection that closes | Adopted (§8.1 tick step 2) |
| Smaller readings (§6.4 NFC `NAME`, §3.5 stored names and anchors, §8.3 grace at any ground spawn, §2.2 relative `--content`, §2.3 keygen backups) | Adopted (§6.4, §3.5, §8.3, §2.2, §2.3) |
| M7 exit check: tick deadlines | §8.1: tick *k* is due at ⌊*k* · 10⁶ / `tick_rate`⌋ µs; the wording question about `server_time` stays at `tessera-protocol.md` §11.1 |
| §8.3 settling grace across a correction | Adopted: the grace runs from the start of the ground epoch; a correction clears *C* but neither ends nor restarts it (§8.3) |
| §8.3 API shape and the 56 m margin | The API is the implementation's. The margin is revised: *S* = 58 m, from the conservative 49 m run-out of the implementer's model (§8.3, §4.5) |

<a id="s15-2"></a>
### 15.2 Implementer notes (`tessera-loadtest`, 2026-10-07), answered 2026-10-09

| Note | Answer |
|---|---|
| Wanderer phase unknown to clients | Wanderers are not judged for Position, Rest and Motion (§13.2) |
| *D* = 150 ms against `tessera-protocol.md` §8.6 | *D* is the observer's actual interpolation delay (§13.2) |
| Chat certainty, range edge, Yell instance | Adopted as the Chat rule and the instance paragraph of §13.2 |
| `churn` without `Disconnect` | Abrupt loss is exercised by the hostile mode (§13.1) |
| Remote runs and server memory and counts | §13.3 (which process criterion 3 judges; criteria 1–2 on server counts in-process only) |
| Warm-up and the throttle check | Adopted (§13.3) |
| Measurements not taken | §13.4 makes `MotionBatch` timing conditional on the backend; tick figures as §11.1; the rest is §15.3 |
| Hostile clients and the loopback login budget | Adopted (§13.1) |
| `cheat` verdict and stale epochs | Adopted with the stated numbers; new desync kind Cheat (§13.1, §13.2) |
| State and "animate" | State compares appearance and `STANCE`; new kind Motion (§13.2) |
| Missing and the create budget | Adopted (§13.2) |
| Instance limitation of Missing, Ghost and Yell | Adopted as a coverage note (§13.2); a backend change is §15.3 |
| Typed refusal events, ping counter | §15.3 |
| Size of the walking area | Adopted (§13.1) |
| Single-threaded checking and its limit | The tool may state its limit; below 9 Hz a run is invalid (§13.1) |

The resident-memory notes were resolved on 2026-10-08 (§13.3, change log).

<a id="s15-3"></a>
### 15.3 Known implementation gaps (2026-10-09, updated 2026-10-10; not questions)

For the owners of `apps/server`, `apps/loadtest` and `crates/net`; none of them changes this
spec's rules.

- (Closed 2026-10-10: the "connection closed" line carries the connection's own bytes in
  and out, after a wait of at most 1 s for its stream writers, since commit a744d7e (§11.1).
  Closed 2026-10-10: the statistics line reports resident memory where the platform reports
  it, since commit 51aa8be.)
- The "connection closed" line's code and reason (§11.1) are written only when the peer
  closed the connection with an application code. When the server itself closes it (a kick,
  `SERVER_FULL`, `SLOW_CONSUMER`, `FLOODING`, a protocol error, `SERVER_SHUTDOWN`), the line
  carries only the transport library's text for a local close, without the code and reason
  the server sent, and it does not say which end closed (found by the review of 2026-10-10).
- On Windows only Ctrl+C triggers a graceful shutdown (§12.1 SHOULD items).
- `tessera-net`: no way to drop a connection without `Disconnect`; moves carry the backend's
  current epoch only; `Event::WorldEntered` does not carry the instance of `WorldEnter`;
  `TeleportRefused`, `ChatRejected` and `ServerNotice` all arrive as system chat lines, which the
  tool tells apart by text prefixes that an operator notice could imitate; the latest ping RTT
  has no sequence counter (two equal measurements count as one); `RemoteStats` has counts only
  (no `MotionBatch` inter-arrival times or loss), and the bots poll at 30 Hz, which quantises
  chat latency to about 33 ms.

---

<a id="s16"></a>
## 16. Change log

| Date | Change |
|---|---|
| 2026-10-06 | First draft (M7, spec writer, design). Companion to `tessera-protocol.md` v1.0 and `server-content.md` |
| 2026-10-08 | §1, §4.5, §8.3: movement limits follow the `collision.md` §12 controller instead of the walk 2 / run 6 m/s controller: *V*ₕ 8.5 m/s (`max_horizontal_speed` replaces `tolerance`; no longer derived from `run_speed`), *V*ᵤ 10 m/s (reasoned anew), *V*_d 120 m/s (was 60); new descent credit with `slide_reserve` *S* = 16 m. Spec writer (design) |
| 2026-10-08 | §1, §4.5, §8.3, §14, §15.1: answers the three implementer notes of 2026-10-08. Descent credit is a pool *C* of its own (0 to *S*, cleared at every epoch); *A*ₕ pays first, then the move's counted descent *d*_c, then *C*; unused descent is kept in *C*. *S* (`slide_reserve`) 16 → 56 m, derived from stepped run-outs with a jump at every landing; *S* = 0 turns the credit off. Settling grace skips only the up and down tests; *A*ᵤ and *A*_d stay tracked and bound the counted descent. Cheat bounds restated (§4.5), new invariant 8, new OPEN on run-outs up slopes. Spec writer (design) |
| 2026-10-08 | §4.7, §10.1, §13.1, §13.3, §15.1, §15.2: §10.1 gains the per-address handshake cap (4 in progress per source IP address, loopback exempt; a fixed rule, not a `[limits]` key), the cap of `max_connections` + `max_handshakes` live connections, the QUIC Retry under handshake pressure and the order of the admission checks; §13.1: the load tool provisions at most 4 accounts at a time. §13.3 says which process criterion 3 judges (in-process: the whole process, bots included; `--server-pid`: the server process; otherwise the tool's own, with the server not judged), no separate level-off rule, verdicts with default allocator settings. The four §15.2 notes on bot-side memory growth (fixed in `tessera-net`: one network thread and a shared client endpoint per process) and the §15.1 handshake note are marked Resolved. Spec writer (design) |
| 2026-10-09 | Prepared for review (spec writer, design); status Draft → Review. Provenance: `tessera-protocol.md` v1.0 cited as Accepted and revised 2026-10-09; a row for the adopted implementer readings; the *S* derivation row names both stepped models. Every implementer note of §15.1 and §15.2 answered and folded into the text: §2.2 relative `--content`, §2.3 keygen backup names, §3.4 4 MiB accounts-file limit, §3.5 stored names and anchors, §4.3 and §6.2 login budget without a loopback exemption, §4.6 chat keys may only tighten the protocol's limits, §5.3 outdoor spawns carry no anchor, §6.2 lockouts per canonical name and the address-budget removal rule, §6.4 NFC `NAME`, §8.1 hash workers write `accounts.toml`, tick deadlines from `tick_start(k)`, events of a closed connection, §8.3 grace across corrections and at any ground spawn, *S* 56 → 58 m (cheat bound 66.5 m), the protocol OPEN answered by `tessera-protocol.md` §8.3 "Server relaxations", §8.4 destinations outside the extent, §10.1 `control_queue` per message and the writer-queue skips, §10.2 accounting per event, §11.1 tick figures, §12.1 Windows signals, §12.2 saves; §13.1 churn, cheat verdict, hostile mode on loopback, walking area, scale; §13.2 *D*, wanderers, Chat certainty, Missing create budget, State, new kinds Motion and Cheat, instance coverage; §13.3 warm-up, remote counts; §13.4 measurements. §15 restructured: open items, answered-note tables §15.1–§15.2, new §15.3 implementation gaps. |
| 2026-10-09 | Revision after the whole-spec review of 2026-10-09 (spec writer); status Review (changes requested) → Revised. Blocking (1): §4.6 `log_chat` (bool, default false, §11.2) moved into the `[chat]` table before the paragraph, so the table lists every `[chat]` key (§4.1). Non-blocking: (a) §5.3 `facing` conversion stated in words (heading = 90° − `facing` in radians, reduced to (−π, π]) instead of naming a crate function; (b) §8.1 and §15.1 give the tick start ⌊*k* · 10⁶ / `tick_rate`⌋ µs inline instead of the name `tick_start(k)`; (d) author role reads "Spec writer (dirty room; …)"; (e) §4.7 `memory_budget_mib` lower bound 64 → 83 (the smallest value the constraint can meet: 19 MiB Argon2id floor × 1 worker + 64 MiB), no change in accepted configurations. (c) left as a re-check item: §4.5 and §8.3 must be re-read if the 2026-10-09 revisions of `tessera-protocol.md` §8.3 or `collision.md` change in their reviews. No new facts; provenance unchanged. |
| 2026-10-10 | Revision (spec writer); status Accepted → Revised, ready for review. §11.1: the statistics line is defined field by field: interval boundaries (from the start of the simulation, then from each line), the longest tick of the interval reset at every line, bytes per second from the bytes of the interval (protocol frames and datagram messages, rounded down), overruns counted since start, three connection counts; new §14 invariant 9; §13.4 names the since-start longest tick as a report figure (adopted from commit 9ce64c8). §4.1: values outside their range are refused, never clamped; `[movement]` fixed for the run. §8.3: new paragraph on the parameters and `tessera-protocol.md` §8.3's new rules (bounds of the values in force, changing the parameters during an epoch, revised 2026-10-10); the run-out OPEN restated with its consequence (a correction of a legitimate player, never an accepted cheat) and the stepped calculation that would close it; §15 entry points to it. §3.1: a content file in an installation is refused with status 3 (`INSTALL-PATH`), a data directory with status 4 (re-review note (ii)). Header: **Reviewer** row names the 2026-10-09 reviewer (note (iii)). §15.1: the loopback row says both exemptions (connection limit and handshake cap); the statistics row points to the new §11.1. The 2026-10-09 OPENs on the settling grace and credit (grace credit bounded by *A*_d, faces just steeper than 45°) were already answered in §8.3; checked, no change. Two provenance rows (Design for §11.1, Project for §4.1/§8.3). Re-review note (i) checked: `tessera-protocol.md` §8.3 item 1 now names a `Named` teleport; its 2026-10-10 revision adds rules only and changes none of the values §4.5, §8.3 and invariant 8 rely on |
| 2026-10-10 | Second revision of the day (spec writer); status Accepted → Revised, awaiting review. §15.3: the gap "no resident memory in the statistics line" is closed (reported where the platform does, since commit 51aa8be); the "connection closed" line still lacks the connection's bytes in and out and stays listed. §11.1: the closed line's bytes in and out are defined as the connection's own totals from acceptance to end, counted as for the statistics line; review notes (a) and (b) of 2026-10-10 folded in (the server time of a statistics line is the start time of the tick that writes it, which fixes the interval length that divides the byte counts; the kind byte that opens each server-initiated unidirectional stream, `tessera-protocol.md` §2.3 rule 1, is not counted). Provenance row for these three facts (Design). No limit, default, status or wire rule changes; `tessera-server` already counts and bounds as stated, except the closed line's byte counts (§15.3) |
| 2026-10-10 | Third revision of the day (spec writer); status Accepted → Revised, awaiting review. §11.1: the "connection closed" line has its own paragraph and field table (connection number, source address, duration in ms, bytes in, bytes out, code and reason), one line per accepted connection whatever the cause of the end; new normative drain bound: the server waits for the connection's stream writers before the line, at most 1 s, then writes it regardless, with later bytes counted only in the statistics line; the wait holds up neither the simulation nor other connections. §14: new invariant 10. §15.3: the closed line's byte counts are marked closed (commit a744d7e); no gap of `apps/server` on logging remains (corrected by the fourth revision: the code and reason of server-initiated closes are a gap). One provenance row (Design). Adopts the behaviour of `tessera-server` since commit a744d7e for the byte counts and the wait (not for the code and reason of closes the server initiates, see the fourth revision); no wire rule, limit, default or status changes |
| 2026-10-10 | Fourth revision of the day (spec writer); status Revised, changes requested → Revised, awaiting review. Blocking item of the review of the third revision: §11.1 keeps the closed line's "code and reason" field and states it for both ends (the peer's code and reason when the peer closed, the server's own when the server closed; the line SHOULD say which end closed; the transport's description only when no application close ended the connection); §15.3 lists as an open gap that `tessera-server` writes the code and reason only for closes the peer sends; the provenance row of the third revision and that revision's change-log row are narrowed to the fields the server writes. Review notes: (a) the duration runs from the arrival of the connection attempt (before the handshake) to the writing of the line (after the drain wait), as the server measures it; (b) invariant 10 is phrased on bytes read from and handed to QUIC and names the condition under which they equal the peer's; (c) the connection's places in `max_connections`, `max_connections_per_address` and the statistics count are released when the line is written, so `SERVER_FULL` can meet a new connection for up to 1 s in the bound case. One provenance row (Design). No wire rule, limit, default or status code changes |
