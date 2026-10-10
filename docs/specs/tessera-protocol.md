<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera protocol v1

| | |
|---|---|
| **Status** | **Accepted** (independent review 2026-10-10 of the v1.1 minor revision of 2026-10-10, decision 0025: CLEAN_ROOM.md §5.3 passed, every item; minor-1 layouts, limits, negotiation, vectors and Appendix A agree with `test-server.md`, `scenario-feed.md` revision 4 and the code on branch `p1`; non-blocking notes (a)–(c) in the review record). Before: Review (v1.1 minor revision of 2026-10-10, spec writer, design, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md): §3.1 minor 1, §5.3 `Equipped.slot`, `LookEntry` and `Look`, the look appended to `CharacterCreate`, `LoginOk`, `CharacterList`, `WorldEnter`, `EntityCreate`, `EntityAppearance`, §7.4 receiver rules, §13.1 limits, §15 invariants 10–11, §16.8 vectors, Appendix A creation mapping; v1.0 bytes unchanged; an independent review by a session that wrote no part of v1.1 must follow). Before: **Accepted** (independent review 2026-10-10 of the revision of 2026-10-10: CLEAN_ROOM.md §5.3 passed, every item; §8.3 "Bounds of the values in force" and "Changing the parameters during an epoch" and §15 invariant 8 agree with `test-server.md` §4.1, §4.5 and §8.3 as revised 2026-10-10; no byte changes, minor version 0; see the review record). Before: Revised (2026-10-10, spec writer, design: §8.3 "Server relaxations" gains two normative paragraphs, "Bounds of the values in force" and "Changing the parameters during an epoch", answering the two implementer notes of 2026-10-09 and 2026-10-10 in §17.1; §15 invariant 8 covers a change of the parameters within a stretch; no byte changes, minor version 0; awaiting review, see the change log). Before: **Accepted** (independent re-review 2026-10-09 of the review-fix revision of 2026-10-09: CLEAN_ROOM.md §5.3 passed, every item; both blocking items of the earlier 2026-10-09 review fixed; §8.3 "Server relaxations" and §15 invariant 8 agree with `test-server.md` §4.5, §8.3, §8.4 and §14; no byte changes, minor version 0; implementers re-check `MovementValidator` and `tessera-server` against §8.3; see the review record). Before: Revised (2026-10-09, spec writer, design: the two blocking items of the 2026-10-09 review are fixed in §8.3 "Server relaxations" and §15 invariant 8, with non-blocking notes (a)–(c); no byte changes, minor version 0; awaiting review). Before: Revised, **changes requested** (independent review 2026-10-09 of the 2026-10-09 revision: two blocking items, see the review record). Before that: Revised (2026-10-09, spec writer, design: §8.3 "Server relaxations" answers the round-2 OPEN on the settling grace, and invariant 8 is reworded for them and for unreported rejections; no byte two peers exchange changes, the minor version stays 0; awaiting review). Before that: Accepted (independent review 2026-10-07, no blocking findings; non-blocking `> **OPEN:** (review)` notes remain, see [Review](#review)) |
| **Protocol version** | 1.1 (major 1 in ALPN `tessera/1`; minor 1 in `Hello`, peers may negotiate minor 0 = v1.0, §3.1) |
| **Last updated** | 2026-10-10 |
| **Author role** | Spec writer (design) |
| **Reviewer** | reviewer (did not write the code), 2026-10-07; second round 2026-10-07 ([R.7](#review-r7)). Method and results: [Review](#review). Later revisions: reviews 2026-10-09 (changes requested, then passed) and 2026-10-10 (passed), each by a Claude session in the spec-reviewer role; v1.1 reviewed 2026-10-10 by a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of decision 0025, of the P1 revisions of `scenario-feed.md`, `tessera-protocol.md`, `test-server.md` and `server-content.md`, or of the code (passed; see the review record) |
| **Review record** | Review 2026-10-10 of the v1.1 minor revision (commit 8122d97, decision 0025; `git diff 8122d97^ 8122d97` of this file: header, §3 compatible changes and the frozen `Hello`/`HelloAck`, §3.1, §5.3 `Equipped.slot`, `LookEntry` and `Look`, the appended look fields of §6.2.5, §6.2.7, §6.2.8, §6.3.1, §6.3.4, §6.3.6, §6.2.17, §7.4 slot, look and receiving rule, §13.1 rows, §15 invariants 10–11, §16.8, Appendix A, §18 row) by a Claude review session in the spec-reviewer role (Clean-Room-Role: review) that wrote no part of decision 0025, of the P1 revisions of `scenario-feed.md`, `tessera-protocol.md`, `test-server.md` and `server-content.md`, or of the code, against CLEAN_ROOM.md §5.3 and LEGAL.md §11.1 rule 5: **passed**, every item. Checklist: functional design facts only (layouts, ranges, negotiation and receiver rules); no code or pseudo-code; no copied prose; no game text, keys, hostnames or traffic (all IDs in §16.8 are made up); provenance (the Design row and decision 0025; no dirty-room input); invariants 10–11 testable; unknowns marked OPEN; no closed gate (Tessera's own protocol). Checked: (1) **Vectors.** The seven §16.8 frames and look K were re-encoded from the §5.3 and §6 tables by a throwaway script (scratch, not committed): byte for byte equal, lengths 36, 29, 61, 64, 73, 37, 36 and 32. The twelve negative vectors were checked by hand against §5.3 and §13.1 (811 = 1 + 6 × 135 bytes; count 49; zero and descending IDs; tag 0x0E; byte 0x20; slots 0x00 and 0x41; flag bit 4; bit 3 and 32 trailing bytes at minor 0; `looks` count 0 for one character). (2) **Bounds.** Every new field has a limit and a fatal rule in §13.1: 0–48 entries, ≤ 768 bytes checked while decoding, IDs non-zero and strictly ascending, ten tags, strings 0–128 bytes of 0x21–0x7E, finite floats; `slot` 1–64, only with bit 3, bit 3 only at minor ≥ 1; `looks` count equal to `characters`. The largest `LoginOk` or `CharacterList` (64 characters of 768-byte looks) stays below 54,000 bytes, inside §5.4's 65,536. (3) **Negotiation.** `Hello` and `HelloAck` frozen for major 1 (the round-2 OPEN is answered, the §16.7 vector stays); minors 0–1 with minimum 0 agree with `test-server.md` §4.4; invariant 10 (byte identity at minor 0) and the two minor-0 negative vectors make the downgrade testable. (4) **Rule 5.** The server gains no message, state or rule: looks and slots are stored and relayed unjudged (`server-content.md` R1–R3, R7), `AppearanceSet` carries no look, class and nationality never travel (Appendix A), and the overlay sends only `AppearanceSet`s at ≤ 4 per second. Looks also carry player-chosen values, not only record numbers; decision 0025 routes that to counsel (owner item 2) and `server-content.md` stays Draft, which is the right place for it. (5) **Code on branch `p1`**, read to compare only: `crates/protocol` (`PROTOCOL_MINOR` 1; `Look`, `LookValue`, `Equipped` codecs; minor-gated encode and decode; the `looks` count check; frozen `Hello`/`HelloAck`), `crates/net` (remote mapping of Appendix A, `CharacterInfo` per `LoginOk` entry) and `apps/server` (looks stored per character, left off at minor 0, 1 MiB file limit) match; `cargo test -p tessera-protocol -p tessera-net -p tessera-server` passed with 0 failures and `tools/check-server-deps.sh` passed. Non-blocking, for a later revision: (a) §3's first bullet still reads "This document is **1.0**"; make it 1.1. (b) §6.3.4 says `EntityCreate.look` is "always empty for entities that are not characters", but `tessera-server`'s `first-player` wanderers (kind NonPlayer, Character body) carry the copied look (`test-server.md` §5.4); word it "entities whose body is not a `Character` body", as §7.4 receiving condition (1) does. (c) Look K's IDs 0x10000A01–0x10000A03 lie in the 0x1000xxxx family of `property-system.md` MP4, unlike `scenario-feed.md` Vector G's; harmless on the wire (opaque, made up), but IDs outside both families would avoid naming a real property by chance. Earlier: Review 2026-10-10 of the revision of 2026-10-10 (commit ef0fdfc; `git show ef0fdfc` of this file: header, §1 provenance row, §8.3 "Bounds of the values in force" and "Changing the parameters during an epoch", §15 invariant 8, the two §17.1 Answered callouts, the §18 row) by a Claude review session in the spec-reviewer role that wrote neither this spec, its revisions, `test-server.md` nor the code, against CLEAN_ROOM.md §5.3: **passed**, every item. Checklist: functional design facts only (bounds, update rules at a change, the window rule); no code or pseudo-code (the caps are formulas stated in prose, the window rule is three numbered conditions); no copied prose; no game text, keys, hostnames or traffic; provenance: one new Design row (spec writer, 2026-10-10, no dirty-room input) covers both paragraphs and the invariant; invariants testable; unknowns marked OPEN (none new); no closed gate (Tessera's own protocol; no official-server material). Checked: (1) the rules agree with the clean implementation the two implementer notes describe (`MovementValidator` after commits a692dce and e73f00d, read only to compare): the window is kept as the start of the opening epoch with its end derived from the length in force; a closed or already ended window is not reopened; a zero length closes it; the allowances and *C* are capped at the change and refilled lazily at the next check at the rates then in force. (2) Invariant 8 across a change: the horizontal excess still telescopes (capping only lowers *A*ₕ and *C*), so the bounds with the largest values in force hold; a throwaway simulation (dirty scratch, not committed) of steps 5 and 6 with the credit, random moves and random parameter changes at random times (80 runs of 250 events, every stretch of every run) found no stretch above *V*ₕ · (*t* + *W*) + *S* + Σ*d*_c, *V*ᵤ · (*t* + *W*) or *V*_d · (*t* + *W*) with the largest values in force, and no allowance below 0. (3) No byte changes and no client change; `test-server.md` §4.1 (parameters fixed for a run, refused out of range) and §4.5 ranges (0–10 s, 0–100 m) lie inside the bounds, so `tessera-server` conforms. Non-blocking, for a later revision: (a) "Bounds of the values in force" says what a reserve that is negative or not a number does but not a negative or non-number window length; say it turns the window off or is refused. (b) "an open window never lasts more than 10 s from the start of its epoch": with corrections inside the window the current epoch is a later one; say "of the epoch that opened it". (c) Merge: the branch was cut before main gained the two §17.1 implementer OPENs (2026-10-09 from a692dce, 2026-10-10 from e73f00d) that the two Answered callouts here answer; when merging, keep the Answered callouts and drop those OPENs. (d) Carry-overs of the 2026-10-09 re-review, still open: (i) invariant 8 "outside a settling window" wording, (ii) the §8.6 round-2 OPEN quoting *V*_d 60 and *V*ₕ 7.5, (iii) R.3 and R.7 row 2 marks, (iv) the **Reviewer** row naming only the 2026-10-07 rounds. Earlier: Re-review 2026-10-09 of the review-fix revision of 2026-10-09 (commit c118f62; `git diff main` of this file: §8.3 "Server relaxations", §15 invariant 8, the answered callouts and the §18 rows) by a Claude review session in the spec-reviewer role that wrote neither this spec, its revisions nor `test-server.md`, against CLEAN_ROOM.md §5.3 and the blocking items of the earlier review below: **passed**, every item. Checklist: functional design facts only; no code or pseudo-code (the update rules are formulas stated in prose); no copied prose; no game text, keys, hostnames or traffic; provenance (the relaxations fall under the Design row, the limits under the Project row); invariants testable; unknowns marked OPEN; no closed gate (Tessera's own protocol). Earlier blocking items: (1) fixed, §8.3 item 2 now applies only with *S* > 0, and a server without the credit or with *S* = 0 applies steps 5 and 6 unchanged; invariant 8's parenthesis matches. (2) fixed, item 1 gives *A*_d ← max(0, *A*_d − *d*_d) in the window, and the credit's update *A*_d − min(*d*_d, *A*_d) is the same value, as the text says. Non-blocking (a)–(c) of that review are done; (d) re-checked: `collision.md` on this branch changes only a provenance row, none of the cited speeds. Compatibility checked: the relaxations change no byte and need no client change; with them, the horizontal excess over *A*ₕ summed over a stretch is at most *C* at its start + Σ*d*_c ≤ *S* + Σ*d*_c, *C* stays in [0, *S*] because the rejection test bounds the excess by *d*_c + *C*, and every allowance stays ≥ 0, so invariant 8 holds as worded; outside the window the up and down tests bound rise and descent by *V* · (*t* + *W*) from allowances that are never negative. Against `test-server.md` (Accepted 2026-10-09): §4.5 ranges (`settle_grace_s` 0–10 s, `slide_reserve` 0–100 m) lie within the bounds; §8.3 steps 5 and 6 (*d*_c, the rejection test, *A*ₕ, *A*ᵤ, *A*_d and *C* updates; *C* cleared at every epoch; "with *S* = 0 the credit is off") are the same rules; its grace skips only the up and down tests and runs from the ground epoch across corrections, as item 1's last sentence permits; its §8.4 grace cases (world entry or `Named` teleport to a `ground` spawn, `Ground` destinations; none for `@` player targets) are positions of unknown height, covered by item 1; its §14 invariant 8 is the protocol's, including the *S* = 0 case. Non-blocking, for a later revision: (i) invariant 8's "outside a settling window" could say "over a stretch that lies wholly after the window ends", since a stretch that starts inside it is not bounded vertically. (ii) The §8.6 round-2 OPEN still quotes *V*_d 60 and *V*ₕ 7.5 (before 2026-10-08); with the descent credit a relayed slide run-out (up to about 42.7 m/s, `test-server.md` §8.3) exceeds *V*ₕ · Δt + 5 m per snapshot when `snapshot_rate` is below about 6.8 Hz (34.2 m/s · Δt > 5 m), so the snap rule settled there should account for it. (iii) R.3's movement bullet and R.7 row 2 still read "OPEN at §8.3"; the reviewer of the next round may mark them answered. (iv) The **Reviewer** row names only the 2026-10-07 rounds; copy this review there in the next revision. Earlier: Review 2026-10-09 of the 2026-10-09 revision (§8.3 "Server relaxations", §15 invariant 8, §18 rows and the answered callouts; `git diff main`) by a Claude review session in the spec-reviewer role that wrote neither this spec, the revision nor `test-server.md`, against CLEAN_ROOM.md §5.3 and the round-2 notes of [R.7](#review-r7): **changes requested**. Checklist: functional design facts only, no code or pseudo-code (the update rules are stated as formulas in prose), no copied prose, no game text, keys, hostnames or traffic, provenance (Design, with the §8.3 limits row as Project), invariants testable, no closed gate: pass, except the two items below. Earlier blocking items: none (2026-10-07); the round-2 §8.3 note (R.7 #2), the implementer note on invariant 8 and the §18 note are answered. Checked: the relaxations change no byte and need no client change; the bounds (10 s, 100 m) contain `test-server.md` §4.5 and §8.3 (3 s, 58 m; ranges 0–10 s and 0–100 m); the window skips only the up and down tests and runs from the epoch that opened it across corrections, as `test-server.md` §8.3 says; the credit rules (*d*_c, the rejection test, the updates of *A*ₕ, *A*_d and *C*) are identical in both specs and keep *C* in [0, *S*]; invariant 8 holds for them (the excess over *A*ₕ telescopes to at most *S* + Σ*d*_c) and agrees with `test-server.md` §14 invariant 8 for *S* > 0; the 2026-10-08 limits (*V*ₕ 8.5 = 6.5 + 30 %, *V*ᵤ 10, *V*_d 120 = the speed cap) agree with `collision.md` §10.3, §11.3 and §12.3 as on this branch (that row's "needs a reviewer's confirmation" is hereby given). **Blocking:** (1) §8.3 item 2 says "With *S* = 0 this is step 5 exactly", but with *S* = 0 the stated test is still *d*ₕ > *A*ₕ + *d*_c (only *C* stays 0), so every descending move gains *d*_c of horizontal allowance. `test-server.md` §8.3 ("with *S* = 0 the credit is off: *d*_c and *C* drop out") and its invariant 8 ("with *S* = 0, at most *V*ₕ · (*t* + *W*)") rely on the other reading, so a server built from this text with `slide_reserve` 0 breaks that invariant. State that a server without the credit, or with *S* = 0, applies step 5 unchanged (*d*_c and *C* drop out), and word invariant 8's parenthesis to match ("*S* = 0 and no counted descent without the credit"). (2) §8.3 item 1 gives the up-allowance update in the window (*A*ᵤ ← max(0, *A*ᵤ − *d*ᵤ)) but not the down one; without the credit, step 6 then subtracts the full *d*_d and *A*_d goes negative after a large claimed drop (an allowance below 0 exists nowhere else in §8.3, and the first descent after the window is then rejected), while with the credit item 2 keeps *A*_d ≥ 0 as `test-server.md` §8.3 does. State *A*_d ← max(0, *A*_d − *d*_d) for the window. Non-blocking: (a) the window's parenthesis lists world entry and `Ground` destinations; `tessera-server` also opens it after a `Named` teleport to a `ground` spawn (`test-server.md` §8.4), which the general clause covers; say "for example" or name it. (b) Item 1 lists "the extent check" beside steps 1–4, which already contain it. (c) Invariant 8's "the descent they count" could name *d*_c of §8.3 item 2. (d) The relaxations cite `collision.md` §12, itself Revised and awaiting review on this branch; re-check if its speeds change. |
| **Implements into** | `crates/protocol` (schema, framing, encode/decode, version negotiation, fingerprint text; no I/O), `crates/net` (test-server backend), `apps/server` (`tessera-server`), `apps/loadtest`, `fuzz/` |
| **Companion documents** | [Decision 0030](../decisions/0030-tessera-protocol-transport.md) (transport choice). `test-server.md` (server-local facts: account store and Argon2id parameters, key files, configuration, instance assignment, place names, policies). `server-content.md` (what content a server may serve). `coordinates.md` §4 and §7 (regions, landblocks, interiors). `walker.md` §10.5 (locomotion from velocity). `environment.md` §11.1 (game clock to time of day). `transport-design.md` (TRD v1, not used by this protocol). `docs/ARCHITECTURE.md` §19 and §20.3 |

This is Tessera's **own** multiplayer protocol, spoken between Tessera clients and a Tessera
server (`tessera-server`). It is a design, not a description of anything observed.

---

<a id="s0"></a>
## 0. Scope and non-goals

- **Scope.** The protocol carries the following between one client and one server: transport
  binding, version negotiation, server authentication by a pinned key, account login against
  accounts local to the server, character list, world entry, dynamic entities and their
  properties, movement, teleport and location reports, chat, server notices, time sync, the
  world clock, and disconnect.
- **Not related to any official protocol.** The protocol has no relation to, and no
  compatibility goal with, the network protocol, login, launcher or servers of the game
  whose data files Tessera reads. Nothing here was derived from observing them; that work is
  gated behind M8 ([decision 0004](../decisions/0004-server-strategy.md), `docs/LEGAL.md`
  §9.3). The only links to the user's data are opaque record identifiers (DIDs) and enum
  keys. The client resolves them against its **own** install (§7.4). The server never
  needs, stores or sends game data.
- **Non-goals of v1:** the static world (each client renders it from its own install, §9.1),
  combat, inventory, quests, NPC scripting, server-to-server links, leaving the world back to
  character selection without reconnecting, account management beyond optional
  registration, and key rollover (§4.9 OPEN).

> **OPEN:** (review, non-blocking, round 2) ROADMAP M6 promises player music with a
> "synchronised start between local players (sandbox) and later between networked players
> (M7)". v1.0 has no message, property or capability for it (§7.3 defines only `STANCE`,
> `EMOTE`, `WALK_SPEED`, `RUN_SPEED`), and the non-goals above do not mention it. Spec
> writer: either list "player music and synchronised performance start" as a v1 non-goal
> planned for a later minor (a new message plus a capability bit, or a property), or have
> the ROADMAP M6 line name the minor that will carry it.

- **Server-local facts** (how accounts and keys are stored, configuration, which world
  instance a character joins, the place names that `Named` teleports understand, rate-limit
  overrides) live in `test-server.md`. This spec states only what crosses the wire and the
  rules both sides must follow.

Keywords: **MUST**, **MUST NOT**, **SHOULD**, **MAY** as in RFC 2119 and RFC 8174.
"Protocol violation" means the receiver ends the connection with application error
`PROTOCOL_VIOLATION` (§14.1).

---

<a id="s1"></a>
## 1. Provenance

Legend: **Design** = decided by this project in this spec (normative, no observation
behind it). **Standard** = taken from the cited public standard. **Project** = an existing
Tessera fact from another spec or crate, restated here. **Computed** = produced by running
this spec's rules (test vectors). **Reported** = third-party documentation or source, used as
background only and not normative.

| Facts | Source | Mark |
|---|---|---|
| Everything not listed below: message set, layouts, codes, limits, algorithms | Original design by the Tessera project, 2026-10-06 (spec writer, design role). No dirty-room input. No other game's protocol was consulted | Design |
| Transport: QUIC v1, its streams, flow control, loss recovery and congestion control | RFC 9000, RFC 9001 (TLS for QUIC), RFC 9002 (loss detection and congestion control) | Standard |
| Unreliable datagrams | RFC 9221 (`max_datagram_frame_size` transport parameter, DATAGRAM frames) | Standard |
| TLS 1.3, cipher suites, CertificateVerify, alerts | RFC 8446; ALPN: RFC 7301; failed ALPN → `no_application_protocol` (alert 120, QUIC error 0x0178): RFC 9001 §8.1 | Standard |
| Server key: Ed25519 | RFC 8032 (algorithm; §7.1 TEST 1 public key used in §16.1), RFC 8410 (Ed25519 in X.509 SubjectPublicKeyInfo, OID 1.3.101.112), RFC 5280 (X.509 v3; §4.1.2.5 "no well-defined expiration" date 99991231235959Z) | Standard |
| Fingerprint: SHA-256, lowercase hexadecimal | FIPS 180-4; RFC 4648 §8 (base16) | Standard |
| Clock offset and round-trip delay formulas | RFC 5905 §8 (NTP on-wire protocol) | Standard |
| Password storage (referenced only) | RFC 9106 (Argon2id); parameters in `test-server.md` | Standard |
| Serial-number comparison of sequence numbers | RFC 1982 | Standard |
| Text: UTF-8; control characters = Unicode general category Cc (U+0000–U+001F, U+007F–U+009F) | RFC 3629; Unicode Standard | Standard |
| Floating point: IEEE 754 binary32 and binary64 | IEEE 754-2019 | Standard |
| Keywords | RFC 2119, RFC 8174 | Standard |
| Documentation addresses in examples | RFC 5737 (192.0.2.0/24), RFC 3849 (2001:db8::/32), RFC 2606 (`example.net`) | Standard |
| Regions, landblocks (160 m, indices 0–254), interiors owned by an anchor block, ±1,280 m anchor ownership, Chebyshev block distance | `coordinates.md` §4 and §7; `tessera_net::model` (`LANDBLOCK_SIZE`, `MAX_LANDBLOCK`, `MAX_LOCAL_OFFSET`, `Anchor::owns`, `Landblock::chebyshev`), read 2026-10-06 | Project |
| Limits: chat text 1,024 bytes, names 64 bytes, place names 256 bytes, 32 equipped items, appearance scale (0, 100], dye codes [0, 1], coordinates within ±10⁷ m; text sanitising (control characters → space, cut on a character boundary) | `tessera_net::model` (`MAX_CHAT_LEN`, `MAX_NAME_LEN`, `MAX_PLACE_NAME_LEN`, `MAX_EQUIPMENT`, `MAX_APPEARANCE_SCALE`, `MAX_DYE_CODE`, `MAX_LOCATION_COORDINATE`, `is_valid_name`, `sanitize_text`), read 2026-10-06 | Project |
| Heading convention (radians, 0 = +X east, counter-clockwise from above) | `tessera_net::model` module documentation | Project |
| `WALK_SPEED` 2 m/s and `RUN_SPEED` 6 m/s defaults | `walker.md` §10.5 and §10.7; `tessera_input` walk-controller defaults, read 2026-10-06. Tessera's own constants, not new observations | Project |
| §8.3 limits: fastest target speed 6.5 m/s, walkable limit 45°, jump push 5.2 m/s, terminal velocity 50 m/s (default) and 100 m/s (Slide layer), speed cap 120 m/s; reasoning and margins in `test-server.md` §4.5 | `collision.md` §10.3, §11.3, §12.3–§12.4 (Accepted 2026-10-07); `tessera_physics::controller::params`, read 2026-10-08 | Project |
| 30 Hz simulation tick; network thread with a single-threaded async runtime and bounded channels | `docs/ARCHITECTURE.md` §20.3, §19.4 | Project |
| Property value kinds mirrored in §7.2 | `tessera_props::Value` (21 kinds), read 2026-10-06 | Project |
| quinn 0.11 defaults (CUBIC congestion control, 30 s idle timeout, worst-case 1-RTT datagram overhead) | quinn-proto 0.11.19 source in the local cargo registry, 2026-10-06 | Reported (background only; §2 states the normative values) |
| §8.3 bounds of the values in force and changing the parameters during an epoch; invariant 8 across a change (2026-10-10) | Design decision of the spec writer, 2026-10-10, adopting the readings of the clean-room `MovementValidator` (implementer notes of 2026-10-09 and 2026-10-10, §17.1); no dirty-room input | Design |
| v1.1 (minor 1): character looks, held-item slots, the minor-version table, the creation mapping of Appendix A (2026-10-10) | Design decision of the spec writer, [decision 0025](../decisions/0025-p1-character-look-on-test-server.md); `character-screens.md` §5.2 and §7.1.1, `character-panels.md` §7, `property-system.md` §3 and §7.3 (Accepted, read 2026-10-10); no dirty-room input | Design |
| Test vectors of §16.8 (v1.1) | Computed 2026-10-10 by a throwaway encoder written from §5–§7 (dirty scratch directory, not committed); each frame was checked to have length = 2 + body, and the v1.0 `CharacterCreate` and `AppearanceSet` vectors of §16.2 were reproduced byte for byte by the same encoder first | Computed |
| Test vectors (§16) | Computed 2026-10-06 by a throwaway encoder written from §5–§7 (dirty scratch directory, not committed); each frame was checked to have length = 2 + body | Computed |

---

<a id="s2"></a>
## 2. Transport binding ([decision 0030](../decisions/0030-tessera-protocol-transport.md))

<a id="s2-1"></a>
### 2.1 QUIC profile

| Item | Rule |
|---|---|
| QUIC version | 1 (RFC 9000). Other versions MUST NOT be offered |
| UDP port | Chosen by the server operator. There is **no default port**, and none is associated with any other service. Connect strings always carry the port (§4.4) |
| ALPN | Exactly one protocol ID per major version. v1: the 9 ASCII bytes `tessera/1` (`74 65 73 73 65 72 61 2f 31`). A client MAY offer several majors; the server selects one. If none matches, TLS fails with `no_application_protocol` (QUIC error 0x0178) |
| 0-RTT | Disabled. Clients MUST NOT send 0-RTT data; servers MUST NOT accept it |
| Session resumption | Clients MUST NOT resume TLS sessions. Servers SHOULD NOT issue session tickets |
| Datagrams | Both endpoints MUST send the `max_datagram_frame_size` transport parameter (RFC 9221) with a value of at least 1,200. A peer that does not is closed with `DATAGRAMS_REQUIRED` (§14.1) once the handshake completes |
| Stream limits | The client advertises `initial_max_streams_bidi` = 0 and `initial_max_streams_uni` = 2. The server advertises `initial_max_streams_bidi` = 1 and `initial_max_streams_uni` = 0. Neither side raises them in v1 |
| Flow control | Each endpoint SHOULD allow at least 256 KiB per stream and 1 MiB per connection |
| Idle timeout | `max_idle_timeout` = 30,000 ms on both sides |
| Keep-alive | The client sends a QUIC PING whenever it has sent nothing for 5 s. The server MAY do the same at 10 s |
| Congestion control | Any RFC 9002-conformant controller (NewReno, CUBIC per RFC 9438, ...). Not part of interoperability |
| Path MTU | Initial 1,200 bytes. PMTU discovery is optional and does not change any limit in this spec |
| Connection migration | Allowed (RFC 9000 §9). The server identifies a session by its QUIC connection, not by address |

> **OPEN:** (implementer note) quinn 0.11 does not expose the peer's
> `max_datagram_frame_size` transport parameter, only the datagram payload it can send now
> (about 1,162 bytes at the 1,200-byte initial MTU). `crates/protocol` therefore accepts
> either the parameter (≥ 1,200) or, when only the payload is known, a payload of at least
> 1,100 bytes (§5.5), which every v1 datagram fits. A peer that advertises a parameter
> between about 1,110 and 1,199 is then not detected. It also builds every endpoint with
> QUIC version 1 as its only supported version (quinn's default also accepts drafts 29–34).

<a id="s2-2"></a>
### 2.2 TLS profile

- TLS 1.3 only (QUIC requires it). Every endpoint MUST support `TLS_AES_128_GCM_SHA256`
  and MAY offer `TLS_AES_256_GCM_SHA384` and `TLS_CHACHA20_POLY1305_SHA256`.
- Key exchange: X25519 MUST be supported. secp256r1 MAY be.
- Server signature: **Ed25519** (TLS SignatureScheme `ed25519`, 0x0807). In v1 the server's
  key MUST be Ed25519 and clients MUST support it. The certificate profile and the client's
  check are in §4.1–§4.3.
- No client certificates. A server MUST NOT send CertificateRequest.
- SNI: the client sends the host as `server_name` when the host is a DNS name, and omits SNI
  for IP literals. The server ignores SNI in v1.
- Key logging (`SSLKEYLOGFILE`) for analysing Tessera's own traffic MAY exist in debug
  builds. It MUST be off unless the user explicitly enables it.

<a id="s2-3"></a>
### 2.3 Channels

| Channel | QUIC object | Opened by, when | Kind byte | Delivery | Carries (message type range) |
|---|---|---|---|---|---|
| **Control** | The first client-initiated bidirectional stream (stream ID 0) | Client, right after the handshake | none | Reliable, ordered, both directions | 0x0001–0x00FF: handshake, login, characters, world entry, requests from the client, ping, disconnect (§6.2) |
| **World** | A server-initiated unidirectional stream | Server, when it accepts `EnterWorld` | 0x57 (`W`) | Reliable, ordered, server → client | 0x0101–0x01FF: world entry, location reports, entity create/update/appearance/destroy, interest reset, world clock (§6.3) |
| **Chat** | A server-initiated unidirectional stream | Server, right after it sends `LoginOk` | 0x43 (`C`) | Reliable, ordered, server → client | 0x0201–0x02FF: chat lines, server notices, chat rejections (§6.4) |
| **Datagram** | RFC 9221 DATAGRAM frames | Either side once the state allows (§6.5) | none | Unreliable, unordered, may be lost; QUIC never duplicates them | 0x0301–0x03FF: movement and time sync (§6.5) |

Each message class has exactly one delivery class:

| Class | Delivery | Why |
|---|---|---|
| Control (login, requests, ping, disconnect) | Reliable, ordered | Request/response, small |
| Entity state (create, update, appearance, destroy, location reports, clock) | Reliable, ordered, on its own stream | Create must precede update; bulk state must not block chat or control |
| Movement (client proposals, server snapshots) | Unreliable | Only the newest state matters; a lost snapshot is replaced by the next |
| Chat and notices | Reliable, ordered, on its own stream | Must not wait behind entity bulk |
| Time sync | Unreliable | A retransmitted sample would carry a false delay; samples are filtered (§11.2) |

Rules:

1. The first bytes of a server-initiated unidirectional stream are its **kind byte**. Frames
   (§5.4) follow it. A kind byte other than 0x57 or 0x43, a second stream of the same kind,
   or any further stream is a protocol violation.
2. The control stream has no kind byte. Its first frame in each direction is `Hello` (client)
   and `HelloAck` or `Disconnect` (server).
3. A message type outside the range of the channel it arrived on, or one sent in the wrong
   direction (§6.1), is a protocol violation.
4. Neither side finishes (FIN), resets (RESET_STREAM) or stops (STOP_SENDING) any of these
   streams except while closing (§12.3). Otherwise it is a protocol violation.
5. **Read gating.** The client does not read the chat stream before it has processed
   `LoginOk`, and does not read the world stream before it has sent `EnterWorld`. QUIC flow
   control holds early bytes, so no cross-stream buffering is needed and events are surfaced
   in a sensible order.
6. There is no ordering between channels. Every rule that needs one goes through a single
   channel (create-before-update is on the world stream) or through explicit epochs and
   timestamps (§8.4, §8.6).

---

<a id="s3"></a>
## 3. Versioning and negotiation

- The **protocol version** is *major.minor*. This document is **1.0**.
- **Major** versions are incompatible. They are negotiated by ALPN alone (`tessera/<major>`
  in decimal, no leading zeros). There is no in-band fallback.
- **Minor** versions are negotiated in `Hello`/`HelloAck` (§6.2.1). A peer that supports
  minor *m* supports every minor from its stated minimum up to *m*. The negotiated minor is
  min(client `minor_max`, server maximum). If that is below the server's minimum supported
  minor (from `test-server.md`; 0 for every v1 server today), the server sends
  `Disconnect(UNSUPPORTED_VERSION)`.
- **Compatible changes** (minor bump): a new message type; a new field appended at the end of
  a message body; a new value of an enumerated field; a new bit of a flags field whose
  undefined bits MUST be 0, together with the payload the bit announces inside the same
  structure (as `Equipped` bits 0 and 1 announce their dye fields); a new capability bit; a
  new property value tag. A peer sends any of these only when the negotiated minor includes
  them. The `Hello` and `HelloAck` bodies are frozen for major 1 (no minor appends to them),
  because they are decoded before a minor is negotiated; client extensions use `client_caps`
  bits or messages sent after `HelloAck`.
- **Changes that need no negotiation:** new property IDs (§7.1). Values are self-describing,
  so receivers skip IDs they do not know.
- **Incompatible changes** (major bump): changing the meaning, width or order of an existing
  field; removing a message; changing framing or a channel's role; changing the
  authentication method.
- **Unknown-message and unknown-field handling.** Both sides know exactly which layout the
  negotiated minor allows, so decoding is **strict**:
  - an unknown message type is a protocol violation;
  - a body with bytes left after its last field, or one that ends early, is a protocol
    violation;
  - a value outside the listed range of an enumerated field is a protocol violation, unless
    the field's definition says how to treat unknown values (property values do, §7.3);
  - unknown **capability bits** are ignored, never rejected;
  - unknown **property IDs** are skipped (§7.1). An unknown **value tag** is a protocol
    violation.

> **OPEN:** (review, non-blocking) The spec gives many fields a "Rule" without saying what a
> receiver does when it is broken, outside §13.1's list. `tessera-protocol` closes with
> `PROTOCOL_VIOLATION` on all of these (found by probing its public decoder and reading its
> client state machine): `HelloAck.tick_rate` outside 1–120, `snapshot_rate` outside 1..`tick_rate`,
> `interest_radius` above 8, `minor` above `minor_max`; `WorldEnter.player` 0 or `epoch` 0;
> `WorldClock.game_clock` < 0 or `rate` outside 0–10,000; `EntityCreate.id` 0;
> `EntityUpdate` with 0 changes; `ChatMessage` flag bits 1–7, a System line with a sender name
> or a non-zero `sender_entity`, a non-System line with an empty sender; `TimeReply` with
> *t*₂ < *t*₁; `TeleportRequest` or `QueryLocation` with `request_id` 0;
> `LocationReport(CORRECTED)` with `request_id` ≠ 0; duplicate names in `LoginOk` or
> `CharacterList`; duplicate IDs in one `MotionBatch`; an `EMOTE` field of the wrong kind. It
> accepts: `LoginFailed.retry_after` ≠ 0 without `RATE_LIMITED`, `ServerNotice.seconds` ≠ 0
> for Info or Warning, a System `ChatMessage` with `ECHO`, `WALK_SPEED`/`RUN_SPEED` outside
> (0, 100], unknown `EMOTE` field IDs, `LocationReport(REQUESTED)` with `request_id` 0.
> Conformant peers interoperate either way, but a second implementation cannot tell from the
> text which of these end a session. Spec writer: add one general sentence ("a value that
> breaks its field's Rule is a protocol violation unless the field says otherwise") and mark
> the informational exceptions explicitly.


> **Answered (2026-10-10, spec writer, design, v1.1; was OPEN, review round 2):** the
> `Hello` and `HelloAck` bodies are frozen for major 1 (bullet "Compatible changes" above);
> v1.1 appends nothing to them, and the §16.7 vector stays.
>
> **OPEN (as raised):** (review, non-blocking, round 2) "A new field appended at the end of a message
> body" cannot apply to `Hello`, nor to any message sent before `HelloAck`: no minor is
> negotiated yet, and strict decoding makes the extra bytes fatal. §16.7 even lists "`Hello`
> frame with one extra trailing `00`" as a negative vector, and `tessera-protocol` closes
> such a session with `PROTOCOL_VIOLATION` instead of negotiating down. A 1.1 spec that
> appended a field to `Hello` would therefore break every 1.0 server. Spec writer: say that
> the `Hello` body is frozen for all of major 1 and that client extensions go through
> `client_caps` bits or messages after `HelloAck`; or define that a server whose maximum
> minor is below `Hello.minor_max` ignores bytes after the fields it knows (and then change
> the §16.7 vector).


<a id="s3-1"></a>
### 3.1 Minor versions

| Minor | Adds (all sent only when the negotiated minor is at least this value) | Spec |
|---|---|---|
| 0 | v1.0: everything not listed below | — |
| 1 | `Equipped` flag bit 3 and its `slot` byte (§5.3); the `Look` structure (§5.3); the appended fields `CharacterCreate.look` (§6.2.7), `LoginOk.looks` (§6.2.5), `CharacterList.looks` (§6.2.8), `WorldEnter.look` (§6.3.1), `EntityCreate.look` (§6.3.4), `EntityAppearance.look` (§6.3.6) | [Decision 0025](../decisions/0025-p1-character-look-on-test-server.md) |

- A client that supports v1.1 sends `minor_max` = 1. `tessera-server` supports minors 0 and 1
  (its minimum stays 0, `test-server.md` §4.4), so v1.0 and v1.1 clients both connect.
- At negotiated minor 0 a v1.1 peer encodes and decodes exactly the v1.0 layouts: no flag
  bit 3, no appended field. A v1.1 server keeps the looks and slots it stores and simply does
  not send them on a minor-0 connection; a v1.1 client on a minor-0 connection sends no look
  and no slot (`scenario-feed.md` §9.7).
- No capability bit is added: a minor-1 server stores and relays looks and slots as §7.4
  says. `server_caps` bit 2 `APPEARANCE_CHANGE` keeps its meaning.

---

<a id="s4"></a>
## 4. Handshake and authentication

<a id="s4-1"></a>
### 4.1 Server key and certificate

- On first start the server generates an **Ed25519 key pair** with a CSPRNG and stores it in
  its own data directory (`test-server.md`). The repository, releases and documentation
  contain no server keys. Every server installation has its own key.
- From that key the server builds a **self-signed X.509 v3 certificate** (RFC 5280), signed
  with the same key (RFC 8410). Subject and issuer are free (recommended:
  `CN=tessera-server`). Validity is free (recommended: notBefore = generation time − 1 day,
  notAfter = 99991231235959Z). The certificate MAY be regenerated at each start. Its
  SubjectPublicKeyInfo does not change while the key does not change, so the fingerprint
  stays the same.
- The TLS certificate chain contains exactly this one certificate.
- At start-up the server prints its fingerprint (§4.2) so the operator can pass it to users.

<a id="s4-2"></a>
### 4.2 Key fingerprint (exact text format)

- **Input:** the DER bytes of the `subjectPublicKeyInfo` element, exactly as they occur in
  the end-entity certificate. For Ed25519 this is always 44 bytes: the 12 bytes
  `30 2a 30 05 06 03 2b 65 70 03 21 00` followed by the 32-byte public key.
- **Digest:** SHA-256 of those bytes.
- **Text:** the 7 ASCII characters `sha256:` followed by the 64 **lowercase** hexadecimal
  digits of the digest, most significant byte first. Total length 71 characters. No
  spaces, colons between bytes, or other separators.
- **Parsing:** the prefix MUST be exactly `sha256:` (lowercase). Hex digits MAY be upper or
  lower case and compare case-insensitively. Anything else (wrong length, other characters,
  surrounding whitespace in a connect string) is an invalid pin. In the known-servers file,
  surrounding whitespace is part of the line syntax (§4.5), not of the fingerprint. Programs
  always write the lowercase form.
- Clients MAY show the fingerprint to people in groups for readability, but MUST NOT use a
  grouped form in connect strings or the known-servers file.
- Test vector: §16.1.

<a id="s4-3"></a>
### 4.3 Client verification of the server

During the TLS handshake the client:

1. takes the **first** certificate of the chain and ignores any others;
2. parses it as X.509 v3 DER and extracts the `subjectPublicKeyInfo` bytes. If parsing fails
   or the algorithm is not id-Ed25519 (1.3.101.112), the connection fails as `TLS_FAILURE`;
3. computes the fingerprint (§4.2) and compares it with the **expected fingerprint** (§4.5).
   A mismatch fails the connection as `KEY_MISMATCH`, and no expected fingerprint fails it as
   `KEY_UNKNOWN` (§14.2);
4. verifies the TLS 1.3 CertificateVerify signature with that key, as TLS requires;
5. **ignores** certificate names, validity dates, key usage, other extensions, and whether
   the certificate is self-signed. There is no web PKI and no certificate authority.

The connection proceeds to `Hello` only after steps 1–4 succeed, so every application byte,
the password included, goes only to the holder of the pinned key.

<a id="s4-4"></a>
### 4.4 Connect string

```
tessera://<host>:<port>[?pin=<fingerprint>]
```

| Part | Rule |
|---|---|
| Scheme | `tessera://`, lowercase |
| `<host>` | A DNS name (ASCII letters, digits, `-`, `.`; compared case-insensitively and stored lowercase), an IPv4 address in dotted decimal, or an IPv6 address in square brackets (`[2001:db8::1]`, RFC 5952 form recommended). Internationalised names are entered in their ASCII (`xn--`) form |
| `<port>` | Decimal, 1–65535, **required** |
| `?pin=` | Optional. `<fingerprint>` in the §4.2 form. No other query parameters exist in v1. An unknown parameter, a path other than empty, user information, or a fragment makes the string invalid |

The **server key** of a connect string is `<host>:<port>` with the host lowercased and IPv6
kept in brackets, for example `192.0.2.10:47000`, `[2001:db8::1]:47000` or
`play.example.net:47000`. (These port numbers are examples, not defaults.)

Account name and password are never part of the connect string. The client supplies them
separately (the `tessera-net` backend's constructor, ARCH §19.1).

<a id="s4-5"></a>
### 4.5 Expected fingerprint: pin or trust on first use

The client chooses the expected fingerprint like this:

1. **Pin in the connect string:** that fingerprint is authoritative. The known-servers file
   is neither consulted nor changed.
2. **No pin:** the client looks up the server key (§4.4) in its **known-servers file**.
   - Found: that fingerprint is expected. A mismatch fails as `KEY_MISMATCH`. The client
     MUST NOT offer to override this inside the connection flow. To accept a new key, the
     user edits or removes the line.
   - Not found, **trust on first use**: an interactive client completes the TLS handshake up
     to step 3 of §4.3, shows the server key and the presented fingerprint, and continues
     only if the user accepts. It then appends a line to the file. A non-interactive client
     (load tester, CI) fails as `KEY_UNKNOWN` unless it was explicitly configured to accept
     new keys. Trust on first use cannot detect an attacker present on that first
     connection, so operators SHOULD hand out pinned connect strings.

**Known-servers file** (client-local, in Tessera's settings directory, named
`known_servers`): UTF-8 text. Each line is either empty, a comment starting with `#` (after
optional leading whitespace), or an entry: the server key, one or more spaces or tabs, the
fingerprint, optional trailing whitespace. Lines end with LF or CRLF. If a server key appears
more than once, the first entry wins and the client logs a warning. A line that is not valid
is skipped with a warning. Example:

```
# Tessera known servers
192.0.2.10:47000 sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9
```


> **OPEN:** (review, round 2) The interactive flow above (stop at §4.3 step 3, show the
> fingerprint, continue when the user accepts) cannot finish within the spec's own
> timeouts: the server closes a connection still in Accepting after 10 s (§12.2), the client
> times out Handshaking after 10 s (§12.1), and the QUIC idle timeout is 30 s. A user who
> takes 15 s to compare the fingerprint gets `TIMEOUT`. A prompt inside the TLS callback
> would also stall the single network thread of decision 0030 point 4. `tessera-protocol`'s
> `TofuPolicy` is a synchronous callback (its rustdoc already recommends "refuse, ask,
> reconnect"), and no interactive client exists yet. Spec writer: make the interactive
> flow "fail the first attempt as `KEY_UNKNOWN` while reporting the presented fingerprint,
> ask the user outside the connection, append the line to `known_servers`, reconnect with
> that fingerprint expected", and keep accept-in-handshake for non-interactive clients
> configured to accept new keys.

<a id="s4-6"></a>
### 4.6 Sequence

```
client                                         server
  | QUIC Initial/Handshake (TLS 1.3, ALPN tessera/1)  |
  |<---------- certificate (Ed25519), CertificateVerify|
  | §4.3 pin check                                     |
  |== control stream (bidi 0) =========================|
  | Hello ------------------------------------------->|
  |<------------------------------------------ HelloAck
  | Login (or Register) ----------------------------->|  Argon2id check (test-server.md)
  |<---------------------------- LoginOk | LoginFailed |
  |<== chat stream 'C' opened after LoginOk ==========|
  | [CharacterCreate -> CharacterList | ...Failed]     |
  | EnterWorld -------------------------------------->|
  |<------------------------------- EnterWorldFailed  |  (control), or:
  |<== world stream 'W': WorldEnter, WorldClock, ... =|
  |<=== datagrams: MoveInput / MotionBatch / Time* ==>|
```

<a id="s4-7"></a>
### 4.7 Account authentication: what goes on the wire

- `Login` (§6.2.3) carries the **account name** and the **password** as UTF-8 strings. They
  travel only inside QUIC 1-RTT packets, which TLS 1.3 encrypts and authenticates (AEAD),
  after the client has authenticated the server's key (§4.3). With 0-RTT disabled, no
  replayable early data exists. **Passwords never travel unencrypted.**
- The password is sent as the exact bytes the user typed, with no normalisation in v1.
  > **OPEN:** whether to apply a Unicode normalisation profile (RFC 8265 OpaqueString)
  > before sending and hashing. Changing it later invalidates stored hashes of non-ASCII
  > passwords, so decide before any public server.
- The server verifies the password against its Argon2id hash (RFC 9106; parameters,
  salt handling and account-name canonicalisation in `test-server.md`). It MUST NOT log,
  persist or echo the plain password, and SHOULD discard it from memory after verifying.
  Failures for an unknown account and for a wrong password MUST be indistinguishable on the
  wire (both `BAD_CREDENTIALS`) and SHOULD take similar time.
- `Register` (§6.2.4), when the server advertises `REGISTRATION`, creates an account with the
  same fields and logs in on success.
- **Attempts:** after `LoginFailed` the client MAY send another `Login` or `Register` on the
  same connection, one at a time. After the third failure on a connection, the server sends
  `Disconnect(AUTH_FAILED)`. `RATE_LIMITED` carries `retry_after` in seconds, and the client
  MUST NOT retry earlier. Per-account and per-address throttling are server policy
  (`test-server.md`).
- **One session per account.** If the account is already in a session, the server either
  refuses with `ALREADY_ONLINE` or ends the old session with
  `Disconnect(LOGGED_IN_ELSEWHERE)` and accepts the new one. Which one is configured in
  `test-server.md`. Clients handle both.

<a id="s4-8"></a>
### 4.8 Handshake failure codes

| Where | Code | Meaning |
|---|---|---|
| TLS (client-local) | `TLS_FAILURE`, `KEY_MISMATCH`, `KEY_UNKNOWN`, `NO_COMMON_VERSION` | §14.2 |
| QUIC close | `DATAGRAMS_REQUIRED`, `UNSUPPORTED_VERSION`, `AUTH_TIMEOUT`, `AUTH_FAILED`, `SERVER_FULL` | §14.1 |
| `LoginFailed` | `BAD_CREDENTIALS` … `SERVER_ERROR` | §14.3 |

<a id="s4-9"></a>
### 4.9 Key changes

> **OPEN:** v1 has no key rollover. A server whose key changes gets a new fingerprint, and
> users must re-pin or edit their known-servers file. A later minor version could let the old
> key sign the new one.

---

<a id="s5"></a>
## 5. Encoding and framing

<a id="s5-1"></a>
### 5.1 Primitive types

All multi-byte integers and floats are **little-endian**. There are no varints at the
application layer. QUIC's own varints are internal to QUIC.

| Name | Size | Encoding and validity |
|---|---|---|
| `u8`, `u16`, `u32`, `u64` | 1, 2, 4, 8 | Unsigned |
| `i32`, `i64` | 4, 8 | Two's complement |
| `f32`, `f64` | 4, 8 | IEEE 754 binary32 / binary64. **Every float in v1 MUST be finite** (no NaN, no ±∞). −0.0 and subnormals are allowed. A non-finite float is a protocol violation |
| `bool` | 1 | 0 = false, 1 = true. Any other value is a protocol violation |
| `str` | 2 + n | `u16` byte length *n*, then *n* bytes of UTF-8 (RFC 3629, no BOM). Invalid UTF-8 is a protocol violation. Each field states its length range and text class (§5.2) |
| `opt<T>` | 1 or 1 + size(T) | `u8` 0 = absent, or 1 followed by `T`. Any other value is a protocol violation |
| `list8<T>` / `list16<T>` | 1 / 2 + … | `u8` / `u16` count, then that many `T`. Each field states its maximum count |

<a id="s5-2"></a>
### 5.2 Text classes

| Class | Bytes | Content rule | Client → server, rule broken | Server → client, rule broken |
|---|---|---|---|---|
| `NAME` | 1–64 | No Cc control characters (`tessera_net::model::is_valid_name`) | The request fails with its own code (`NAME_INVALID`, `UNKNOWN_CHARACTER`, `UNKNOWN_RECIPIENT`) | Length outside 1–64: protocol violation. Control characters: replaced by spaces (sanitised), not fatal |
| `NAME0` | 0–64 | As `NAME`, or empty | (not sent by clients) | As `NAME` |
| `TEXT` | 1–1,024 | No Cc control characters | `ChatRejected(INVALID_TEXT)` | Length > 1,024 or 0: protocol violation. Control characters: sanitised |
| `TEXT0`(*n*) | 0–*n* | No Cc control characters | (not sent by clients) | Length > *n*: protocol violation. Control characters: sanitised |
| `ACCOUNT` | 1–32 | ASCII `A`–`Z`, `a`–`z`, `0`–`9`, `.`, `_`, `-` only | `LoginFailed(INVALID_ACCOUNT_NAME)` | Length outside 1–32: protocol violation |
| `PASSWORD` | 1–256 | Any valid UTF-8 | `LoginFailed(BAD_CREDENTIALS)` (empty) | (never sent) |
| `PLACE` | 1–256 | No Cc control characters, not only whitespace (`is_valid_place_name`) | `TeleportRefused(INVALID_DESTINATION)` | (never sent) |

> **OPEN:** (review, non-blocking) `TEXT0` says "(not sent by clients)", but two client
> fields use it: `Hello.client_name` (`TEXT0`(64), see the implementer note in §17.1) and
> `Disconnect.text` (`TEXT0`(1,024)). Give `TEXT0` a client column (length above *n* fatal,
> control characters sanitised by the server, never a failure code); the implementation
> already behaves that way for both fields.

A length field above the class maximum is detected **before** the bytes are read, and is
always a protocol violation, whichever direction it travels. From the client, a length
below the class minimum (an empty string) is treated like a broken content rule and gets
the request's own failure code. "Sanitised" means each control
character becomes U+0020, exactly as `tessera_net::model::sanitize_text` does. Clients never
interpret markup in any received text (§10.5).

<a id="s5-3"></a>
### 5.3 Common structures

**Location** (26 bytes): a precise, region-qualified position (`tessera_net::model::Location`).

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 2 | u16 | `region` | Region number (`coordinates.md` §4). 0 = the abstract region of a world without data |
| 2 | 8 | f64 | `x` | Metres east, region frame. Finite, \|x\| ≤ 10⁷ |
| 10 | 8 | f64 | `y` | Metres north. Same rule |
| 18 | 8 | f64 | `z` | Metres up, absolute height. Same rule |

**Anchor** (4 bytes): the landblock and interior cell that own a position (`coordinates.md`
§7). Its region is the region of the Location it accompanies.

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 1 | u8 | `block_x` | 0–254 |
| 1 | 1 | u8 | `block_y` | 0–254 |
| 2 | 2 | u16 | `cell` | Interior cell, 0 = not known |

An Anchor is valid for a Location only if it **owns** it: |x − 160 · block_x| ≤ 1,280 and
|y − 160 · block_y| ≤ 1,280 (`Anchor::owns`). Otherwise it is a protocol violation.

**Pose** (31 or 35 bytes): `Location`, then `heading` f32 (radians, 0 = +X east,
counter-clockwise seen from above; any finite value, receivers reduce it to (−π, π]), then
`anchor` opt<Anchor>.

**Vec3** (12 bytes): three f32, x, y, z, finite. Velocities are in m/s in the region frame.

**Body** (5 or 9 bytes): `u8` tag, then:

| Tag | Variant | Then | Meaning |
|---|---|---|---|
| 1 | Character | `species` u32, `sex` u32 | A playable body by the data's species key and sex value (opaque; `appearance.md` §2) |
| 2 | Object | `did` u32, non-zero | A data record that describes the figure (opaque DID) |

Any other tag is a protocol violation.

**Equipped** (5, 9 or 13 bytes; at minor ≥ 1 also 6, 10 or 14):

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u32 | `item` | DID of the item record, non-zero (opaque) |
| 2 | u8 | `flags` | Bit 0: `dye` follows. Bit 1: `secondary_dye` follows. Bit 2: stowed. Bit 3 (minor ≥ 1): `slot` follows. Bits 4–7 MUST be 0, and bit 3 MUST be 0 at minor 0 |
| 3 | f32 | `dye` | Only if bit 0. In [0, 1] |
| 4 | f32 | `secondary_dye` | Only if bit 1. In [0, 1] |
| 5 | u8 | `slot` | Only if bit 3. 1–64: the data's equipment slot key that holds the item (opaque; `property-system.md` §7.3 bit ↔ key). Lets an item that fits either hand say which hand holds it (§7.4) |

**Appearance** (`tessera_net::model::Appearance`):

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | Body | `body` | |
| 2 | f32 | `scale` | In (0, 100] |
| 3 | opt<u32> | `idle` | The data's starting-idle enum key (opaque) |
| 4 | list8<Equipped> | `equipment` | At most 32 entries |

**LookEntry** (minor ≥ 1): `property` u32, then a tagged value (§7.2).

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u32 | `property` | A property ID of the client's data (opaque; unrelated to §7.1's IDs). Non-zero |
| 2 | tagged value | `value` | Tag 0x01 Bool, 0x02 Int32, 0x03 Int64, 0x04 Float32 (finite), 0x05 Enum, 0x06 DataId, 0x07 String, 0x09 Bitfield32, 0x0A Bitfield64 or 0x0D Colour. A String here is 0–128 bytes, each byte 0x21–0x7E (printable ASCII without space). Any other tag (None, Vector3, EntityRef, Time, Array, Struct) is a protocol violation |

**Look** (minor ≥ 1): `list8<LookEntry>`, 0–48 entries (0 = no look), `property` strictly
ascending, the whole encoding (count byte included) at most 768 bytes. A character's
creation choices: the values its creation controls wrote (`character-screens.md` §5.2), each
an opaque (property, value) pair (§7.4).

**CharacterSummary**: `name` str (`NAME`), then `body` Body.

**Destination** (for teleports): `u8` tag, then:

| Tag | Variant | Then |
|---|---|---|
| 1 | Location | `location` Location, `heading` f32: an exact outdoor position |
| 2 | Ground | `location` Location, `heading` f32: outdoor. The server replaces `z` with its terrain height if it knows one |
| 3 | Anchored | `location` Location, `heading` f32, `anchor` Anchor (which MUST own the location): an interior, never snapped or re-homed |
| 4 | Named | `name` str (`PLACE`): a place the server knows by name (`test-server.md`) |

Any other tag is a protocol violation.

<a id="s5-4"></a>
### 5.4 Stream frames

On the control, world and chat streams (after the kind byte on uni streams), messages are
sent as **frames**:

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 4 | u32 | `length` | Number of bytes that follow: 2 + body length. **Minimum 2, maximum 65,536** in v1 |
| 4 | 2 | u16 | `type` | Message type (§6.1) |
| 6 | length − 2 | bytes | `body` | The message body, decoded strictly (§3) |

- A `length` outside 2–65,536 is a protocol violation, detected before any body byte is
  buffered. A receiver therefore never buffers more than 65,540 bytes for one frame.
- Frames are not aligned to QUIC STREAM frames or packets. Receivers reassemble.
- One frame carries exactly one message. There is no batching inside a frame.

<a id="s5-5"></a>
### 5.5 Datagram messages

One QUIC DATAGRAM frame carries exactly **one** message: `u16 type`, then the body, with no
length field. The datagram's own length bounds the body, and strict decoding applies (§3).

- **Size:** a datagram message MUST NOT exceed 1,200 bytes, and MUST NOT exceed the
  transport's current maximum datagram payload. Senders SHOULD keep every datagram at
  **1,100 bytes or less**. That always fits a 1,200-byte path MTU, whatever the
  connection-ID length (worst-case 1-RTT overhead: 25 bytes of short header with a 20-byte
  connection ID and 4-byte packet number, a 16-byte AEAD tag, and up to 9 bytes of DATAGRAM
  frame header). A receiver treats a datagram over 1,200 bytes as a protocol violation.
- QUIC authenticates every datagram, so a malformed one cannot come from network corruption.
  A datagram that fails to decode is a protocol violation. A well-formed datagram that
  arrives when the state does not allow it (§6.5) is silently dropped and counted, because
  datagrams race the streams.

---

<a id="s6"></a>
## 6. Message catalogue

<a id="s6-1"></a>
### 6.1 Summary

Direction: C→S client to server, S→C server to client. Channel: Ctl control, W world, Ch
chat, D datagram. "State" is when the **sender** may send it (states in §12).

| Type | Name | Dir | Ch | Sender state | §|
|---|---|---|---|---|---|
| 0x0001 | Hello | C→S | Ctl | First client frame | 6.2.1 |
| 0x0002 | HelloAck | S→C | Ctl | Answer to Hello | 6.2.2 |
| 0x0003 | Login | C→S | Ctl | Authenticating, none outstanding | 6.2.3 |
| 0x0004 | Register | C→S | Ctl | Authenticating, server has `REGISTRATION` | 6.2.4 |
| 0x0005 | LoginOk | S→C | Ctl | Answer to Login/Register | 6.2.5 |
| 0x0006 | LoginFailed | S→C | Ctl | Answer to Login/Register | 6.2.6 |
| 0x0007 | CharacterCreate | C→S | Ctl | CharacterSelect, server has `CHARACTER_CREATION`, none outstanding | 6.2.7 |
| 0x0008 | CharacterList | S→C | Ctl | Answer to CharacterCreate | 6.2.8 |
| 0x0009 | CharacterCreateFailed | S→C | Ctl | Answer to CharacterCreate | 6.2.9 |
| 0x000A | EnterWorld | C→S | Ctl | CharacterSelect, none outstanding | 6.2.10 |
| 0x000B | EnterWorldFailed | S→C | Ctl | Answer to EnterWorld (success opens the world stream instead) | 6.2.11 |
| 0x000C | TeleportRequest | C→S | Ctl | InWorld | 6.2.12 |
| 0x000D | QueryLocation | C→S | Ctl | InWorld | 6.2.13 |
| 0x000E | ChatSend | C→S | Ctl | InWorld | 6.2.14 |
| 0x000F | EmoteSet | C→S | Ctl | InWorld | 6.2.15 |
| 0x0010 | StanceSet | C→S | Ctl | InWorld | 6.2.16 |
| 0x0011 | AppearanceSet | C→S | Ctl | InWorld, server has `APPEARANCE_CHANGE` | 6.2.17 |
| 0x0012 | Ping | both | Ctl | Client: after `HelloAck` received. Server: after `HelloAck` sent | 6.2.18 |
| 0x0013 | Pong | both | Ctl | Answer to Ping | 6.2.18 |
| 0x0014 | Disconnect | both | Ctl | Client: after `Hello`. Server: at any time | 6.2.19 |
| 0x0101 | WorldEnter | S→C | W | First world frame | 6.3.1 |
| 0x0102 | LocationReport | S→C | W | After WorldEnter | 6.3.2 |
| 0x0103 | TeleportRefused | S→C | W | After WorldEnter | 6.3.3 |
| 0x0104 | EntityCreate | S→C | W | After WorldEnter | 6.3.4 |
| 0x0105 | EntityUpdate | S→C | W | Entity exists on this connection | 6.3.5 |
| 0x0106 | EntityAppearance | S→C | W | Entity exists, or the player | 6.3.6 |
| 0x0107 | EntityDestroy | S→C | W | Entity exists | 6.3.7 |
| 0x0108 | InterestReset | S→C | W | After WorldEnter | 6.3.8 |
| 0x0109 | WorldClock | S→C | W | After WorldEnter | 6.3.9 |
| 0x0201 | ChatMessage | S→C | Ch | After LoginOk | 6.4.1 |
| 0x0202 | ServerNotice | S→C | Ch | After LoginOk | 6.4.2 |
| 0x0203 | ChatRejected | S→C | Ch | Answer to ChatSend | 6.4.3 |
| 0x0301 | MoveInput | C→S | D | InWorld | 6.5.1 |
| 0x0302 | MotionBatch | S→C | D | After WorldEnter | 6.5.2 |
| 0x0303 | TimeRequest | C→S | D | After HelloAck | 6.5.3 |
| 0x0304 | TimeReply | S→C | D | Answer to TimeRequest | 6.5.4 |

A receiver that gets a message it knows but whose **sender state** does not allow it (for
example `EnterWorld` before `LoginOk`, or a second `Hello`) treats it as a protocol
violation, except for datagrams (§5.5) and the stale cases listed in §13.2.

<a id="s6-2"></a>
### 6.2 Control messages

<a id="s6-2-1"></a>
#### 6.2.1 Hello (0x0001, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `minor_max` | Highest minor version the client supports (0 for v1.0) |
| 2 | u32 | `client_caps` | Capability bits. None are defined in v1.0: send 0. Unknown bits are ignored |
| 3 | u16 | `max_entities` | The most entities (§9.4) the client accepts at once. 64–65,535. Below 64 is a protocol violation |
| 4 | str | `client_name` | `TEXT0`(64): software name and version, for logs only |

Pre: first frame on the control stream, sent at once after the handshake. Post: the server
answers with `HelloAck` or `Disconnect(UNSUPPORTED_VERSION | SERVER_FULL)`.

> **OPEN:** (review, non-blocking, round 2) This body cannot grow within major 1 under the
> current rules; see the round-2 note at §3.

<a id="s6-2-2"></a>
#### 6.2.2 HelloAck (0x0002, S→C)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `minor` | Negotiated minor (§3), ≤ `minor_max` |
| 2 | u32 | `server_caps` | Bit 0 `REGISTRATION`: `Register` accepted. Bit 1 `CHARACTER_CREATION`: `CharacterCreate` accepted. Bit 2 `APPEARANCE_CHANGE`: `AppearanceSet` accepted. Bits 3–31: 0 in v1.0, ignored by clients |
| 3 | u8 | `tick_rate` | Server simulation rate in Hz, 1–120. v1 servers use 30 |
| 4 | u8 | `snapshot_rate` | `MotionBatch` rate in Hz, 1 to `tick_rate`. Default 15 |
| 5 | u8 | `interest_radius` | *R*, in landblocks, 0–8. Default 2 (§9.2) |
| 6 | str | `server_name` | `TEXT0`(64): the operator's display name for the server |
| 7 | str | `motd` | `TEXT0`(1,024): message of the day, may be empty |

Post: the client may send `Login`/`Register`, `TimeRequest` and `Ping`.

<a id="s6-2-3"></a>
#### 6.2.3 Login (0x0003, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | str | `account` | `ACCOUNT` |
| 2 | str | `password` | `PASSWORD`. Plain text inside the encrypted connection (§4.7) |

Answered by exactly one `LoginOk` or `LoginFailed`.

<a id="s6-2-4"></a>
#### 6.2.4 Register (0x0004, C→S)

Same body as `Login`. Creates the account and logs in. It is a protocol violation unless
`HelloAck` advertised `REGISTRATION`. Answered by exactly one `LoginOk` or `LoginFailed`
(`ACCOUNT_EXISTS`, `INVALID_ACCOUNT_NAME`, `PASSWORD_REJECTED`, `REGISTRATION_CLOSED`, ...).

<a id="s6-2-5"></a>
#### 6.2.5 LoginOk (0x0005, S→C)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | str | `account` | `ACCOUNT`: the account's canonical name as the server stores it |
| 2 | list8<CharacterSummary> | `characters` | 0–64 entries, names unique within the list |
| 3 | list8<Look> | `looks` | Minor ≥ 1 only. Exactly as many entries as `characters`; entry *i* is the look of character *i* (empty: none) |

Post: the server opens the chat stream. The client is in CharacterSelect. (With 64 characters
of 768-byte looks and a 32-byte account the frame stays below 54,000 bytes.)

<a id="s6-2-6"></a>
#### 6.2.6 LoginFailed (0x0006, S→C)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `code` | §14.3 |
| 2 | u16 | `retry_after` | Seconds. Non-zero only with `RATE_LIMITED` |
| 3 | str | `text` | `TEXT0`(256): optional human-readable detail |

After `SERVER_FULL` or `SERVER_ERROR` the server follows with `Disconnect`.

<a id="s6-2-7"></a>
#### 6.2.7 CharacterCreate (0x0007, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | str | `name` | `NAME` |
| 2 | Appearance | `appearance` | §5.3 |
| 3 | Look | `look` | Minor ≥ 1 only. The character's creation choices (empty: none). Stored with the character and never changed afterwards in v1.1 |

Pre: CharacterSelect, `CHARACTER_CREATION`, no `CharacterCreate` or `EnterWorld`
outstanding. Answered by exactly one `CharacterList` or `CharacterCreateFailed`. The server
checks names and appearances against its own policy and `server-content.md`. Starting
position, instance and starting properties are server-local (`test-server.md`).

<a id="s6-2-8"></a>
#### 6.2.8 CharacterList (0x0008, S→C)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | list8<CharacterSummary> | `characters` | The account's full list after the change, 0–64 entries |
| 2 | list8<Look> | `looks` | Minor ≥ 1 only. As `LoginOk.looks` |

<a id="s6-2-9"></a>
#### 6.2.9 CharacterCreateFailed (0x0009, S→C)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `code` | §14.4 |
| 2 | str | `text` | `TEXT0`(256) |

<a id="s6-2-10"></a>
#### 6.2.10 EnterWorld (0x000A, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | str | `name` | `NAME`: one of the account's characters |

Pre: CharacterSelect, nothing outstanding. On success the server places the character in a
world instance (`test-server.md`), opens the world stream and sends `WorldEnter` as its first
frame. There is no success message on the control stream. On failure: `EnterWorldFailed`. In
v1 a connection enters the world at most once.

<a id="s6-2-11"></a>
#### 6.2.11 EnterWorldFailed (0x000B, S→C)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `code` | §14.5 |
| 2 | str | `text` | `TEXT0`(256) |

The client is back in CharacterSelect.

<a id="s6-2-12"></a>
#### 6.2.12 TeleportRequest (0x000C, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u32 | `request_id` | Non-zero, chosen by the client, increasing (serial arithmetic, RFC 1982) |
| 2 | Destination | `destination` | §5.3 |

Answered **exactly once** on the world stream: by a `LocationReport` with cause
`TELEPORTED` and the same `request_id`, or by `TeleportRefused` with the same `request_id`.
A request that arrives while an earlier one is unresolved supersedes it. The earlier one is
answered with `TeleportRefused(CANCELLED)`. A server answers within 10 s. Effects: §8.4 and
§9.5.

<a id="s6-2-13"></a>
#### 6.2.13 QueryLocation (0x000D, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u32 | `request_id` | Non-zero |

Answered by `LocationReport` with cause `REQUESTED` and the same `request_id`. The server MAY
also send a system `ChatMessage` describing the place.

<a id="s6-2-14"></a>
#### 6.2.14 ChatSend (0x000E, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u8 | `channel` | 1 Say, 2 Yell, 3 Emote, 4 Tell. Other values (System = 5 included) are a protocol violation |
| 2 | str | `peer` | Present **only** when `channel` = 4: `NAME`, the recipient |
| 3 | str | `text` | `TEXT` |

Delivery rules: §10. A rejection is answered with `ChatRejected` on the chat stream.
Success has no explicit acknowledgement. The sender receives its own line back with the
`ECHO` flag (§10.3).

<a id="s6-2-15"></a>
#### 6.2.15 EmoteSet (0x000F, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u32 | `anim` | Opaque animation key from the client's data (§7.5). 0 = stop the current emote |
| 2 | bool | `looping` | Play until stopped (true) or once (false) |

Effect: the server sets or removes the player's `EMOTE` property (§7.3). Rate-limited with
the control budget (§13.3).

<a id="s6-2-16"></a>
#### 6.2.16 StanceSet (0x0010, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u8 | `stance` | 0 Relaxed, 1 Ready, 2 Seated. Other values are a protocol violation |

Effect: the server sets the player's `STANCE` property.

<a id="s6-2-17"></a>
#### 6.2.17 AppearanceSet (0x0011, C→S)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | Appearance | `appearance` | §5.3 |

A protocol violation unless `APPEARANCE_CHANGE` was advertised. No look is appended in v1.1:
a character's look is set by `CharacterCreate` and kept by the server, which sends it with
every `EntityAppearance` answer (§6.3.6). Slots (`Equipped` bit 3) are allowed at minor ≥ 1. The server answers
**every** `AppearanceSet` with an `EntityAppearance` for the player carrying the resulting
appearance, which is the old one if it refused. When it refuses, it also sends a system
`ChatMessage` saying why.

<a id="s6-2-18"></a>
#### 6.2.18 Ping (0x0012) and Pong (0x0013), both directions

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `nonce` | Any value. `Pong` echoes the `Ping`'s nonce |

The receiver of `Ping` answers with `Pong` promptly. An endpoint sends at most one `Ping` per
second. This measures application-level round-trip time, including any head-of-line delay
on the control stream. Liveness itself is QUIC's job (§2.1).

> **OPEN:** (review, non-blocking, round 2) The receiver's limit (§13.1: "≤ 1 per second,
> excess ignored") is as tight as the sender's, so jitter alone can make a conformant
> sender's `Ping` arrive 0.99 s after the previous one and go unanswered; both
> `ServerSession` and `ClientSession` then drop it. Separately, §6.1 gives `Pong` the state
> "Answer to Ping", which makes an unsolicited or superseded `Pong` fatal by §13.1, while
> `tessera-protocol` drops it (the safer choice: a peer's outstanding `Ping` can be
> overtaken by its next one). Spec writer: give the receiver slack (for example "more than
> 2 `Ping`s in any 1 s window are ignored") and list "a `Pong` whose nonce is not the newest
> outstanding `Ping`" under §13.2.

<a id="s6-2-19"></a>
#### 6.2.19 Disconnect (0x0014, both directions)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `code` | §14.1 |
| 2 | str | `text` | `TEXT0`(1,024): human-readable reason, may be empty |

The sender then follows the closing procedure (§12.3). A client sends code 0 (`NO_ERROR`) for
a user-requested disconnect.

<a id="s6-3"></a>
### 6.3 World-stream messages (S→C)

<a id="s6-3-1"></a>
#### 6.3.1 WorldEnter (0x0101)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `player` | The player's entity ID, non-zero |
| 2 | str | `name` | `NAME`: the character's name |
| 3 | u32 | `instance` | World-instance number (informational; `test-server.md`) |
| 4 | u32 | `epoch` | Initial position epoch, ≥ 1 (§8.4) |
| 5 | u64 | `server_time` | Server time of this state, µs (§11.1) |
| 6 | Pose | `pose` | Where the player is |
| 7 | Appearance | `appearance` | How the player looks |
| 8 | Look | `look` | Minor ≥ 1 only. The character's look (empty: none) |

Pre: the first frame on the world stream. It is followed at once by `WorldClock`, and then by
the entities around the player (§9).

<a id="s6-3-2"></a>
#### 6.3.2 LocationReport (0x0102)

The player's precise position, for the player only.

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u8 | `cause` | 1 `TELEPORTED`, 2 `REQUESTED`, 3 `CORRECTED`. Other values are a protocol violation |
| 2 | u32 | `request_id` | The answered request's ID for `TELEPORTED` and `REQUESTED`. 0 for `CORRECTED` and for teleports the server initiated |
| 3 | u32 | `epoch` | `TELEPORTED`, `CORRECTED`: previous epoch + 1. `REQUESTED`: the current epoch, unchanged |
| 4 | u64 | `server_time` | µs |
| 5 | Pose | `pose` | |

Client behaviour: `TELEPORTED` and `CORRECTED` move the local avatar to `pose` and adopt
`epoch` (§8.4). `REQUESTED` is informational: it shows the server's last accepted position,
and the client does not snap to it.

<a id="s6-3-3"></a>
#### 6.3.3 TeleportRefused (0x0103)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u32 | `request_id` | The refused request |
| 2 | u16 | `code` | §14.6 |
| 3 | str | `text` | `TEXT0`(256) |

<a id="s6-3-4"></a>
#### 6.3.4 EntityCreate (0x0104)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `id` | Non-zero, ≠ the player's ID, not currently created on this connection |
| 2 | u8 | `kind` | 0 Player, 1 NonPlayer, 2 Creature, 3 Object. Other values are a protocol violation |
| 3 | str | `name` | `NAME0`: display name, may be empty |
| 4 | opt<u32> | `class` | DID of the entity's class record, if any (opaque) |
| 5 | u64 | `server_time` | Server time of the state below, µs |
| 6 | Pose | `pose` | Same region as the player's current region |
| 7 | Vec3 | `velocity` | m/s |
| 8 | u8 | `motion` | §8.1 |
| 9 | Appearance | `appearance` | |
| 10 | PropList | `properties` | Full property state (§7.2). No removals |
| 11 | Look | `look` | Minor ≥ 1 only. The entity's look (empty: none; always empty for entities that are not characters) |

Entity IDs are assigned by the server, unique for the server's lifetime, and never reused for
another entity while the server runs. An entity that leaves and re-enters interest keeps its
ID and is created again.

<a id="s6-3-5"></a>
#### 6.3.5 EntityUpdate (0x0105)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `id` | A created entity, or the player |
| 2 | PropList | `changes` | 1–256 entries. A value replaces the property whole. Tag 0 removes it (§7.2) |

<a id="s6-3-6"></a>
#### 6.3.6 EntityAppearance (0x0106)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `id` | A created entity, or the player |
| 2 | Appearance | `appearance` | Full replacement |
| 3 | Look | `look` | Minor ≥ 1 only. The entity's look, also a full replacement (empty: none) |

<a id="s6-3-7"></a>
#### 6.3.7 EntityDestroy (0x0107)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `id` | A created entity (never the player) |
| 2 | u8 | `reason` | 0 `LEFT_INTEREST`, 1 `REMOVED` (gone from the world: logged out, despawned). Other values are a protocol violation |

<a id="s6-3-8"></a>
#### 6.3.8 InterestReset (0x0108)

Empty body. Every entity created on this connection, except the player, is destroyed as if
each had received `EntityDestroy(LEFT_INTEREST)`. Used on region changes and far teleports
(§9.5).

<a id="s6-3-9"></a>
#### 6.3.9 WorldClock (0x0109)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `server_time` | µs: the moment the values below hold |
| 2 | f64 | `game_clock` | Game clock *G* in seconds (`environment.md` §11.1). Finite, ≥ 0 |
| 3 | f32 | `rate` | *ρ*: game seconds per real second, 0–10,000 (0 = frozen) |

Sent after `WorldEnter`, and again whenever the server sets the clock or changes the rate.
Clients advance it themselves (§11.3).

<a id="s6-4"></a>
### 6.4 Chat-stream messages (S→C)

<a id="s6-4-1"></a>
#### 6.4.1 ChatMessage (0x0201)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u64 | `server_time` | µs, when the server accepted the line |
| 2 | u8 | `channel` | 1 Say, 2 Yell, 3 Emote, 4 Tell, 5 System. Other values are a protocol violation |
| 3 | u8 | `flags` | Bit 0 `ECHO`: this is the receiver's own line coming back. Bits 1–7 MUST be 0 |
| 4 | u64 | `sender_entity` | The speaker's entity ID if it is currently created on this connection or is the player, else 0. Always 0 for System |
| 5 | str | `sender` | `NAME0`: the speaker's name. Empty for System, non-empty otherwise |
| 6 | str | `peer` | Present **only** when `channel` = 4: `NAME`, the other party (the recipient on an echo, otherwise equal to `sender`) |
| 7 | str | `text` | `TEXT` |

<a id="s6-4-2"></a>
#### 6.4.2 ServerNotice (0x0202)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u8 | `level` | 0 Info, 1 Warning, 2 Shutdown. Other values are a protocol violation |
| 2 | u16 | `seconds` | For Shutdown, seconds until the server stops. 0 otherwise |
| 3 | str | `text` | `TEXT` |

Shown as a system line. A client MAY also show it more prominently.

<a id="s6-4-3"></a>
#### 6.4.3 ChatRejected (0x0203)

| Order | Type | Field | Rule |
|---|---|---|---|
| 1 | u16 | `code` | §14.7 |
| 2 | str | `text` | `TEXT0`(256) |

Answers one `ChatSend` (in order, since the control and chat streams each keep their order
and the server handles `ChatSend`s in order). Not fatal.

<a id="s6-5"></a>
### 6.5 Datagram messages

<a id="s6-5-1"></a>
#### 6.5.1 MoveInput (0x0301, C→S), 53 bytes

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 2 | u16 | type | 0x0301 |
| 2 | 4 | u32 | `seq` | +1 per MoveInput on this connection (start at any value, serial arithmetic) |
| 6 | 4 | u32 | `epoch` | The client's current position epoch (§8.4) |
| 10 | 26 | Location | `location` | The player's position, in the current region |
| 36 | 4 | f32 | `heading` | Radians |
| 40 | 12 | Vec3 | `velocity` | m/s |
| 52 | 1 | u8 | `motion` | §8.1 |

The server drops it silently unless it is InWorld for that connection. Validation: §8.3.

<a id="s6-5-2"></a>
#### 6.5.2 MotionBatch (0x0302, S→C), 11 + 37 · *n* bytes

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 2 | u16 | type | 0x0302 |
| 2 | 8 | u64 | `server_time` | µs: the snapshot tick's time |
| 10 | 1 | u8 | `count` *n* | 1–29 (a full datagram is 1,084 bytes) |
| 11 | 37 · *n* | entries | | Below, each entity at most once per batch |

Entry (37 bytes):

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 8 | u64 | `id` | An entity ID (never the receiving player) |
| 8 | 12 | Vec3 | `position` | f32 metres in the frame of the **receiver's current region** (about 4 mm precision at 40 km) |
| 20 | 4 | f32 | `heading` | Radians |
| 24 | 12 | Vec3 | `velocity` | m/s |
| 36 | 1 | u8 | `motion` | §8.1 |

A count of 0 or above 29, or a length other than 11 + 37 · *n*, is a protocol violation.
Entries for unknown IDs are dropped silently (§8.6).

<a id="s6-5-3"></a>
#### 6.5.3 TimeRequest (0x0303, C→S), 10 bytes

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 2 | u16 | type | 0x0303 |
| 2 | 8 | u64 | `client_time` | *t*₀: the client's monotonic clock in µs (any epoch) |

<a id="s6-5-4"></a>
#### 6.5.4 TimeReply (0x0304, S→C), 34 bytes

| Offset | Size | Type | Field | Rule |
|---|---|---|---|---|
| 0 | 2 | u16 | type | 0x0304 |
| 2 | 8 | u64 | `client_time` | *t*₀ echoed exactly |
| 10 | 8 | u64 | `server_receive` | *t*₁: server time when the request arrived, µs |
| 18 | 8 | u64 | `server_send` | *t*₂: server time when this reply was sent, µs. *t*₂ ≥ *t*₁ |
| 26 | 8 | u64 | `tick` | Number of the server tick in progress at *t*₂ (§11.1) |

---

<a id="s7"></a>
## 7. Property replication

<a id="s7-1"></a>
### 7.1 Namespace

Properties are Tessera's own, with 16-bit IDs. They are unrelated to the data files'
property IDs, although the value kinds mirror `tessera-props` (§7.2).

| ID range | Owner | Receiver rule for an unknown ID |
|---|---|---|
| 0x0000 | Invalid | Protocol violation |
| 0x0001–0x3FFF | Tessera protocol (this spec, later minors) | Skip the entry (decode the value to step over it) |
| 0x4000–0x7FFF | Reserved | Skip |
| 0x8000–0xFFFF | Server-private (an operator's extensions) | Skip |

Defined in v1.0 (§7.3): 0x0001 `STANCE`, 0x0002 `EMOTE`, 0x0003 `WALK_SPEED`, 0x0004
`RUN_SPEED`.

<a id="s7-2"></a>
### 7.2 Values and property lists

**PropList**: `u16` count, then *count* entries of `pid` u16 and a **tagged value**. Within
one list, `pid`s are **strictly ascending**: no duplicates, so a list has a single meaning.
Count at most 256.

**Tagged value**: `u8` tag, then the payload:

| Tag | Kind | Payload | `tessera_props::Value` counterpart (non-normative) |
|---|---|---|---|
| 0x00 | None | (nothing). Allowed **only** as a top-level value in `EntityUpdate`: removes the property | (absent) |
| 0x01 | Bool | bool | `Bool` |
| 0x02 | Int32 | i32 | `Int` |
| 0x03 | Int64 | i64 | `Int64` |
| 0x04 | Float32 | f32, finite | `Float` |
| 0x05 | Enum | u32 key | `Enum` |
| 0x06 | DataId | u32 DID (0 = none), opaque | `DidRef` |
| 0x07 | String | str, 0–1,024 bytes, control characters sanitised on receipt | `String` |
| 0x08 | Vector3 | 3 × f32, finite | `Vector3` |
| 0x09 | Bitfield32 | u32 | `Bitfield32` |
| 0x0A | Bitfield64 | u64 | `Bitfield64` |
| 0x0B | EntityRef | u64 entity ID (0 = none) | `InstanceId` |
| 0x0C | Time | u64 server time, µs (§11.1) | `Timestamp` (seconds = µs / 10⁶) |
| 0x0D | Colour | 4 × u8, in the order R, G, B, A | `Colour` |
| 0x0E | Array | `u16` count (0–256), then *count* tagged values, all with the **same** tag, none of them None | `Array` (each element gets the array's PID) |
| 0x0F | Struct | `u16` count (0–64), then *count* × (`field` u16, non-zero, strictly ascending; tagged value, not None) | `Struct` |

- Tags 0x10–0xFF are a protocol violation in v1.0.
- **Depth:** a top-level value has depth 1, and each Array or Struct adds one level. Depth
  above 4 is a protocol violation. **Size:** the encoding of one top-level value (tag
  included) is at most 8,192 bytes. **Message:** the whole frame stays within 65,536 bytes.
  A server whose full state would not fit sends `EntityCreate` with part of the properties
  and the rest in `EntityUpdate`s right after it.

> **OPEN:** (implementer note) "the whole frame stays within 65,536 bytes" conflicts with
> §5.4 and invariant 1, which allow `length` up to 65,536 and so a frame of up to 65,540
> bytes on the wire. `crates/protocol` follows §5.4 (`length` ≤ 65,536). Spec writer: confirm
> the wording "the frame's `length` stays within 65,536 (§5.4)".

> **OPEN:** (review, non-blocking) Confirmed the implementer's reading of the two points
> above: §5.4, invariant 1 and the §16.7 vector (65,537 is the first invalid `length`) all
> say `length` ≤ 65,536, so only the §7.2 sentence needs rewording. For **depth**, the
> independent decoder of `crates/protocol/tests/spec_conformance.rs` was written with the
> same reading as §17.1 (a value nested in *k* containers has depth *k* + 1; above 4 is
> fatal), and both reject `Array`⁴(`Bool`) and accept `Array`³(`Bool`). The only case the
> text leaves open is an **empty** Array or Struct at depth 4 (accepted by both); state it.

> **OPEN:** (review, non-blocking, round 2) "All with the **same** tag" compares element
> tags only. An Array of Arrays (or of Structs) with different contents, such as
> [[Int32 1], [Bool true]], meets the rule literally because every element's tag is 0x0E;
> both codecs accept it (`spec_conformance.rs`,
> `open_review_nested_array_shapes_follow_the_literal_rule`). A reader of "same kind" would
> reject it. Spec writer: state whether element shape is constrained beyond the outer tag.

- Struct field IDs are local to the property that defines the struct. They are not property
  IDs.
- `tessera_props` kinds not carried in v1: string token, waveform, tri-state, property-ID,
  StringInfo, Position, any-width bitfield. Tessera properties have no need for them yet.
- A value of the wrong kind for a **defined** property ID (for example `STANCE` sent as a
  String) is a protocol violation.

**Semantics:**

1. **Full state on create.** `EntityCreate.properties` lists every property the entity has
   that this observer may see (scope, §7.3). A property not listed is absent, which means
   its default.
2. **Deltas afterwards.** `EntityUpdate` lists only changed properties. Each value replaces
   the old one **whole**: arrays and structs are never patched in part. None (0x00) removes
   the property, so it falls back to its default.
3. **Coalescing.** The server sends at most one `EntityUpdate` per entity per observer per
   tick, holding the latest value of each property that changed during the tick. Changes
   that cancel out within a tick MAY be omitted.
4. **Ordering and staleness.** All property traffic is on the world stream, so it is applied
   in order. No timestamps or sequence numbers are needed, and nothing can arrive stale.
   `EntityUpdate` never precedes the entity's `EntityCreate` and never follows its
   `EntityDestroy` (§9.3).
5. **The player's own entity** has no `EntityCreate`. `WorldEnter` creates it on the client.
   The server sends the player's initial owner-scope properties as an `EntityUpdate` for
   the player ID right after `WorldClock`.

<a id="s7-3"></a>
### 7.3 Properties defined in v1.0

| ID | Name | Kind | Scope | Default when absent | Meaning |
|---|---|---|---|---|---|
| 0x0001 | `STANCE` | Enum | All observers | 0 | 0 Relaxed, 1 Ready (held items drawn), 2 Seated. Clients treat unknown values as 0. Set by `StanceSet` |
| 0x0002 | `EMOTE` | Struct | All observers | No emote | Fields: 1 `anim` Enum (opaque animation key from the data, §7.5), 2 `started` Time, 3 `looping` Bool. Set by `EmoteSet` (anim ≠ 0). Removed by `EmoteSet(0)`, when the entity starts moving (gait ≠ Standing, §8.1), or 60 s after `started` if `looping` is false |
| 0x0003 | `WALK_SPEED` | Float32 | Owner only | 2.0 | The player's walking speed in m/s, (0, 100]. Clients use it for gait and locomotion animation (§8.1); the §8.3 limits do not depend on it |
| 0x0004 | `RUN_SPEED` | Float32 | Owner only | 6.0 | Running speed in m/s, (0, 100] |

"Owner only" properties are sent only to the connection that controls the entity (in
`EntityUpdate` for the player). A Struct value with a missing field takes that field's
default (anim 0 = no emote, started = `server_time` of the message, looping false).

> **OPEN:** (implementer note, from review) "started = `server_time` of the message" has no
> referent for `EntityUpdate` (§6.3.5 has only `id` and `changes`). `tessera_net` uses the
> newest server time it has seen in any message when the update is applied (world stream,
> chat stream or a `MotionBatch` datagram, which can run ahead of the world stream), so two
> clients can disagree on an emote's start. Spec writer: either require `started` in
> `EMOTE` values sent in `EntityUpdate`, or define the reference time (for example the
> newest world-stream `server_time`).

> **OPEN:** (review, non-blocking) `tessera-server` always sends all three `EMOTE` fields
> (`Emote::to_property`), so the default for a missing `started` is never exercised between
> Tessera's own programs. Requiring `started` (the first option above) costs nothing. Also
> say whether an `EMOTE` field of the wrong kind is fatal (the implementation treats it so)
> and whether unknown field IDs are skipped (the implementation skips them).

<a id="s7-4"></a>
### 7.4 Appearance

Appearance travels as the dedicated `Appearance` structure (§5.3), not as properties: in
`WorldEnter`, `EntityCreate`, `EntityAppearance` and the client's `CharacterCreate` and
`AppearanceSet`. Each copy is a full replacement.

- **Body**: species key and sex value, or an object DID. These are opaque numbers from the
  client's data.
- **Scale**: a uniform factor on top of the body's own scale.
- **Equipment**: up to 32 items as **opaque DIDs**, with optional dye codes and a stowed
  flag. The client resolves every DID against **its own install** (`appearance.md`,
  `walker.md` §10.4). It draws what it can resolve and skips the rest with a log entry. A
  DID that does not resolve is never an error.
- **Idle**: an optional starting-idle enum key from the data.
- **Slot** (minor ≥ 1): an equipped item may name the equipment slot key that holds it. A
  client places a held item in the hand its slot names; without a slot it applies the Tessera
  rule of `character-panels.md` §7 (default slot, then the secondary hand).
- **Look** (minor ≥ 1): up to 48 opaque (property ID, value) pairs, the character's creation
  choices, carried beside the appearance in the messages of §3.1. The creating client fills
  it from its creation controls (`character-screens.md` §5.2); the server stores it with the
  character at `CharacterCreate` and sends it, unchanged, wherever it sends that character's
  appearance. v1.1 has no message that changes a look after creation.

The server treats all of these as opaque. It checks only the wire limits and its content
policy (`server-content.md`). It never holds or sends game assets.

**Receiving a look** (every client; a peer may be hostile). A client applies a look entry to a
character's figure only when (1) the body is a `Character` body, (2) the entry's property is
one that a creation control of that body's player avatar writes in the receiving client's
**own** install (`character-screens.md` §5.2, column "Writes"; `appearance.md` §3.4), and
(3) the value's tag matches that property's type in the receiver's property dictionary
(`property-system.md` §3: Bool 21, Int32 15, Int64 19, Float32 9, Enum 8, DataId 20,
String 1, Bitfield32 18, Bitfield64 14, Colour 16). An applied entry replaces the avatar's own
value of that property, which is otherwise the default (`appearance.md` §3.4). Every other
entry is ignored and counted, with at most one debug line per message. A look string is a
property value, never shown as text. Two clients with different installs may therefore show
different looks for the same bytes, as for every other reference.

<a id="s7-5"></a>
### 7.5 Motion and animation state

Remote characters animate from these inputs:

| Input | Carried in | Use on the client (`walker.md` §10.5) |
|---|---|---|
| Velocity | `MotionBatch` entries, `EntityCreate` | Locomotion blend (walk/run by ground speed) and playback rate. Locomotion is driven by velocity, never by root motion |
| Gait (`motion` bits 0–2) | `MotionBatch`, `EntityCreate`, `MoveInput` | Picks airborne (jump/fall) clips, and settles the walk/run choice when interpolated speed is ambiguous |
| Backward (`motion` bit 3) | Same | Plays the locomotion clip backwards when the entity moves opposite to its heading |
| `STANCE` | Property | Selects the idle family |
| `EMOTE` | Property | Plays the keyed clip once or looping, from `started`, layered over the idle |
| `appearance.idle` | Appearance | Starting idle |

Animation keys and DIDs are interpreted only by clients, against their own data. Two
clients with different installs MAY show different animations for the same key. That is
acceptable, because the protocol carries intent, not assets.

---

<a id="s8"></a>
## 8. Movement

<a id="s8-1"></a>
### 8.1 Motion byte

| Bits | Field | Values |
|---|---|---|
| 0–2 | Gait | 0 Standing, 1 Walking, 2 Running, 3 Airborne. 4–7 are a protocol violation |
| 3 | Backward | 1 = moving opposite to the heading |
| 4–7 | Reserved | MUST be 0. Otherwise a protocol violation |

Clients derive the gait from their controller: Airborne when not on the ground; otherwise
Standing when horizontal speed < 0.15 m/s, Running when it exceeds the midpoint of
`WALK_SPEED` and `RUN_SPEED` (4 m/s by default), and Walking in between.

<a id="s8-2"></a>
### 8.2 Authority and client send rules

- **Model:** the client simulates its own avatar (input, controller, collision against its
  own loaded geometry) and **proposes** positions. The server **validates** them (§8.3),
  relays accepted ones, and **corrects** the client when it rejects one. The server is
  authoritative for region, teleports, speed limits and epochs.
- **Rates.** While the avatar moves or turns, the client sends `MoveInput` at 15 Hz (every
  second 30 Hz simulation tick). It sends one at once when the gait changes, when it starts
  or stops, and on every teleport or correction it adopts. After stopping it sends the rest
  state in the next 3 ticks (redundancy against loss). At rest it sends a heartbeat at 1 Hz.
  It never sends more than 30 per second.
- `seq` increases by one per `MoveInput` sent. `epoch` is the client's current epoch (§8.4).
- The position is always in the current region. A region changes only by teleport.

<a id="s8-3"></a>
### 8.3 Server validation

The server keeps, per player: the current `epoch`, the last accepted `seq`, the last
accepted Location *P*, the current anchor (or none), and three **allowances**: horizontal
*A*ₕ, up *A*ᵤ and down *A*_d, in metres.

Parameters (defaults here; `test-server.md` may make them configurable):

| Parameter | Default | Note |
|---|---|---|
| *V*ₕ | 8.5 m/s | Fastest controlled speed 6.5 m/s (`collision.md` §10.3) + 30 % |
| *V*ᵤ | 10 m/s | Rise up a 45° walkable slope at 6.5 m/s (`collision.md` §11.3); jump 5.2 m/s; + about 50 % |
| *V*_d | 120 m/s | The controller's speed cap (`collision.md` §12.3 step 3), above the terminal velocities 50 and 100 m/s (§10.3 there) |
| *W* | 1.0 s | Allowance window (burst) |
| Extent | Each of x and y in [−1,280, 40,800 + 1,280] | The grid plus the anchor margin |

Allowances grow at *V* per second of server time and are capped at *V* · *W*. At each new
epoch they start full, and *P* is the pose of the `WorldEnter` or `LocationReport` that
started the epoch.

For each `MoveInput`, in this order:

1. Not InWorld → drop.
2. `epoch` ≠ current epoch → drop silently (stale, sent before the client saw a teleport or
   correction).
3. `seq` not newer than the last accepted `seq` (RFC 1982) → drop (duplicate or reordered).
   The first `MoveInput` of an epoch is accepted whatever its `seq`.
4. `location.region` ≠ current region, or x or y outside the extent → reject (step 7).
5. Horizontal distance *d*ₕ = |*P* − new|ₓᵧ, up *d*ᵤ = max(0, Δz), down *d*_d = max(0, −Δz).
   If *d*ₕ > *A*ₕ, *d*ᵤ > *A*ᵤ or *d*_d > *A*_d → reject (step 7).
6. **Accept:** subtract the distances from the allowances, set *P*, heading, velocity
   (horizontal part clamped to *V*ₕ, vertical part to [−*V*_d, *V*ᵤ]) and gait. Drop the
   anchor if it no longer owns *P* (`Anchor::owns`). The server MAY run further checks of
   its own (terrain, collision; `test-server.md`).
7. **Reject:** send `LocationReport(CORRECTED, request_id 0, epoch + 1, pose = last accepted
   P with its heading and anchor)` and start the new epoch. The server sends at most 4
   corrections per second. Rejections beyond that are dropped without a report.
   (`MoveInput`s of the old epoch are dropped by step 2 in any case.)


**Server relaxations** (revised 2026-10-09). The further checks that step 6 allows may only
tighten this procedure, with two exceptions that a server MAY apply. Neither changes a byte on
the wire, and a client needs no change for either (it sends as §8.2 requires).

1. **Settling window.** For at most 10 s of server time after an epoch whose starting pose has a
   height the server does not know (for example a world entry or a `Named` teleport to a
   position the server's content marks as approximate, or a `Ground` teleport destination,
   §5.3), the server MAY skip the up and down tests of step 5. Steps 1–4, the horizontal test
   of step 5 and step 6 still apply. On acceptance inside the window the up and down
   allowances are reduced but never below 0: *A*ᵤ ← max(0, *A*ᵤ − *d*ᵤ) and
   *A*_d ← max(0, *A*_d − *d*_d) (with the descent credit of item 2 in use, its update of *A*_d
   applies instead and gives the same result). A correction inside the window starts a new
   epoch as step 7 says; the window stays measured from the start of the epoch that opened it.
2. **Descent credit.** The server MAY keep per player a credit *C*, 0 ≤ *C* ≤ *S* with
   *S* ≤ 100 m, set to 0 at every new epoch. With the counted descent *d*_c = min(*d*_d, *A*_d),
   step 5 then rejects when *d*ₕ > *A*ₕ + *d*_c + *C* (instead of *d*ₕ > *A*ₕ), and step 6 sets
   *A*ₕ ← max(0, *A*ₕ − *d*ₕ), *A*_d ← *A*_d − *d*_c and
   *C* ← min(*S*, *C* + *d*_c − max(0, *d*ₕ − *A*ₕ)), the last using *A*ₕ before the update. The
   credit is in use only with *S* > 0: a server that does not keep it, or keeps it with
   *S* = 0, applies steps 5 and 6 unchanged (*d*_c and *C* drop out of the horizontal test, and
   the horizontal allowance gains nothing from a descent). It lets the horizontal run-out of a slide down a steep face,
   which the controller of `collision.md` §12 can make faster than *V*ₕ, be paid by the descent
   that produced it.

**Bounds of the values in force** (added 2026-10-10). The bounds above (window at most 10 s,
*S* at most 100 m) apply to the values a server uses. A server MUST NOT apply a larger value:
where one is configured it either refuses the configuration or applies the bound in its
place. A window length of 0, and a reserve that is 0, negative or not a number, turn the
relaxation off. With no relaxation configured, steps 5 and 6 apply unchanged.

**Changing the parameters during an epoch** (added 2026-10-10). A server MAY change *V*ₕ,
*V*ᵤ, *V*_d, *W*, *S* or the window length while players are in the world. At the moment of
a change, for each player:

1. The allowances are capped at once by the new values: *A*ₕ ← min(*A*ₕ, *V*ₕ · *W*),
   *A*ᵤ ← min(*A*ᵤ, *V*ᵤ · *W*), *A*_d ← min(*A*_d, *V*_d · *W*), and *C* ← min(*C*, *S*)
   (*C* = 0 when the new *S* is 0). The change does not by itself refill them; server time
   that has passed since the last refill is refilled at the next check at the rates then in
   force.
2. An open settling window keeps its start, the start of the epoch that opened it (item 1),
   and from then on ends at that start plus the new length. A new length of 0 closes it. A
   new length that puts the end at or before the moment of the change closes it at once.
3. A window that was already closed at the moment of the change (its end under the old
   length had passed, or it was closed by a teleport epoch or an earlier change) stays
   closed, whatever the new length. A change never opens a window; only the start of an
   epoch with an unknown height does (item 1).

So an open window never lasts more than 10 s from the start of its epoch, and no allowance
or credit ever exceeds the cap of the values in force. Invariant 8 (§15) covers stretches in
which the values change.

`test-server.md` §8.3 gives `tessera-server`'s use of both (settling window 3 s after a `ground`
spawn or a `Ground` destination, *S* = 58 m) and the reasoning. Invariant 8 (§15) counts them.

> **Answered (2026-10-09, spec writer, design; was OPEN, review round 2):** a bounded
> server-defined settling window and a bounded descent credit are allowed ("Server
> relaxations" above), and invariant 8 is reworded to match; `tessera-server`'s defaults lie
> within the bounds. The earlier note of 2026-10-08 from `test-server.md` is superseded.

<a id="s8-4"></a>
### 8.4 Epochs, teleports and corrections

- The **position epoch** (u32) starts at the `WorldEnter` value. The server increments it
  for each teleport and each correction, and states it in that `LocationReport`.
- On `LocationReport` with cause `TELEPORTED` or `CORRECTED` the client moves its avatar to
  the pose, sets its velocity to 0, adopts the epoch and the anchor, and sends a `MoveInput`
  with the new epoch at once.
- `MoveInput`s still in flight from before carry the old epoch and are ignored (§8.3 step 2).
  This is the "moves sent before the report are ignored" rule of `Command::Move`.
- A teleport's order on the world stream: `InterestReset` (if any, §9.5), then
  `LocationReport(TELEPORTED)`, then the new neighbourhood's `EntityCreate`s.
- A `Ground` destination's height comes from the server's terrain if it has one. Otherwise
  the given `z` is used.

<a id="s8-5"></a>
### 8.5 Server relay (`MotionBatch`)

- The server sends snapshots at `snapshot_rate` (default 15 Hz, every second tick of 30 Hz).
  `server_time` is the time of the snapshot tick.
- For each observer, a snapshot includes the entities in its interest set (§9) that:
  (a) changed position, heading, velocity or motion since they were last sent to that
  observer; or (b) came to rest, for the 3 snapshots after their last change (redundancy);
  or (c) were last sent 1 s or more ago (refresh).
- Position and heading are the latest accepted state at the snapshot tick. Player positions
  are relayed as accepted, not extrapolated.
- Entries are split into datagrams of at most 29 entries (§6.5.2). The budget is at most
  16 datagrams per observer per snapshot (§9.4). Over budget, entries are chosen
  nearest-first by Chebyshev block distance, then Euclidean distance, but no entity goes
  more than 1 s without an entry while it is in interest.
- The receiving player never appears in its own batches.

<a id="s8-6"></a>
### 8.6 Client interpolation

- The client keeps, per remote entity, the snapshots received in the last second, and the
  `server_time` of the newest one it applied. It initialises them from `EntityCreate`
  (`server_time`, pose, velocity, motion).
- **Staleness:** an entry whose batch `server_time` is not later than the entity's newest
  applied `server_time` is dropped. So is an entry for an ID that is not currently created.
  Reordered datagrams and datagrams from before a destroy or re-create are discarded this
  way, because a re-create carries a newer `server_time`.
- **Render time** = estimated server time now (§11.2) − *D*. The recommended **interpolation
  delay** is *D* = 150 ms: two snapshot intervals at 15 Hz (133 ms) plus a margin. In
  general, *D* ≥ 2 / `snapshot_rate` + 1 / `tick_rate`.
- Between the two snapshots that bracket render time, interpolate the position linearly and
  the heading along the shorter arc, and use the later snapshot's motion. Velocity for
  animation is interpolated linearly.
- Past the newest snapshot, extrapolate with its velocity for at most 250 ms, then hold.
- If two consecutive snapshots are further apart than *V*ₕ · Δt + 5 m (a teleport or a
  correction of the remote entity), snap instead of interpolating.

> **OPEN:** (review, non-blocking, round 2) "Further apart" does not say horizontal or 3-D,
> and the bound uses only *V*ₕ. `tessera-net` measures 3-D distance, so an entity falling at
> the terminal *V*_d = 60 m/s (§8.3) is treated as teleporting whenever `snapshot_rate`
> ≤ 10 Hz (60 · 0.1 = 6 m > 7.5 · 0.1 + 5 = 5.75 m) and is drawn stair-stepped; a reader who
> measures horizontally interpolates the same fall. Spec writer: "snap when the horizontal
> distance exceeds *V*ₕ · Δt + 5 m or the vertical distance exceeds *V*_d · Δt + 5 m" (or a
> 3-D bound with max(*V*ₕ, *V*_d)); then `tessera-net`'s `SnapshotBuffer` follows.
- The local player is never interpolated. It is simulated locally (§8.2).

> **OPEN:** (review, non-blocking) Confirmed: the recommended *D* = 150 ms is below the
> stated bound at the default rates (2/15 + 1/30 s = 166.7 ms), and `test-server.md` §13.2
> repeats 150 ms. Three implementer notes (§17.2, §17.3) raise it and `tessera-net` uses the
> larger of the two. It has no effect on the wire. Spec writer: state *D* = max(150 ms,
> 2 / `snapshot_rate` + 1 / `tick_rate`), and the history as at least max(1 s, *D* +
> 1 / `snapshot_rate`).

<a id="s8-7"></a>
### 8.7 Anchors

The server keeps each player's anchor from the last `WorldEnter` or `LocationReport` pose
(and from `Anchored` teleports), and drops it when it no longer owns the accepted position:
the same rule as `tessera_net::Session`. The anchor decides the entity's interest block
(§9.1). `MoveInput` carries no anchor in v1.

> **OPEN:** walking from outdoors into an interior that lies outside its block's square
> (`coordinates.md` §7) keeps the outdoor interest block until a teleport or the server's
> own geometry knowledge sets an anchor. A later minor could let the client propose one.

> **OPEN:** (review, round 2) The opposite case, **walking out** of an interior, is not
> covered either. An anchor is dropped only when it no longer owns the accepted position
> (§8.3 step 6), and ownership reaches ±1,280 m, so a player who leaves an anchored pose
> (an `Anchored` teleport, an interior spawn or saved position, an `@name` teleport to an
> anchored player) and walks outdoors keeps the anchor block as its interest block (§9.1)
> for up to 8 blocks. Interest then fails both ways: nearby players are not created, far
> ones are, and Say (by real distance) reaches players whose entity is never created.
> Reproduced through `MovementValidator` from the §16.2 Anchored pose: after 900 m east the
> position block is (132, 140) and the interest block still (126, 140), 6 > *R* + 1 blocks
> (`spec_conformance.rs`, `open_review_anchor_outlives_the_interior`). `test-server.md`
> §15.1 avoids it only for outdoor spawns (no anchor sent). Spec writer: either (a) in a
> later minor, let `MoveInput` carry `opt<Anchor>` or an "outdoors" flag, since only the
> client knows whether it is in a cell; or (b) as a v1 server rule, drop the anchor once
> the landblock containing the accepted position differs from the anchor block by more
> than one block. Then restate §9.1 so the anchor decides the interest block only while
> the client is plausibly inside the interior.

---

<a id="s9"></a>
## 9. Interest management by landblock

<a id="s9-1"></a>
### 9.1 What is replicated

- **Statics are not replicated.** Each client renders the static world (terrain, buildings,
  scenery, figures placed by the landblocks) from its **own install**, around its own
  position, exactly as the offline walker does. The server never sends static objects,
  landblock contents or any game data.
- The server replicates only **dynamic entities** it simulates: other players and any
  server-owned characters or objects (`server-content.md`).
- **Interest block** of an entity: its anchor's block if it has an anchor that owns its
  position. Otherwise the landblock containing its position (`Landblock::containing`), with
  x and y first clamped into [0, 40,800] so that positions off the grid map to the nearest
  edge block.

> **OPEN:** (review, non-blocking) A coordinate of exactly 40,800 m lies on the far edge
> of block 254, and ⌊40,800 / 160⌋ = 255 is not a valid block index (0–254).
> `tessera_protocol::movement::interest_block` clamps the index to 254, which is the evident
> intent. Interest blocks never travel on the wire. Spec writer: say "clamped into
> [0, 40,800], and the index into 0–254".

<a id="s9-2"></a>
### 9.2 Radius and hysteresis

- Observer *O* and entity *E* (*E* ≠ *O*) are candidates only in the same **world instance**
  and the same **region**.
- *E* **enters** *O*'s interest set when Chebyshev(block(*O*), block(*E*)) ≤ *R*, where *R* =
  `interest_radius` (`HelloAck`, default 2: a 5 × 5 block square, 800 m across).
- *E* **leaves** when the distance is > *R* + 1. Between *R* + 1 and *R* it stays where it
  is (hysteresis), so an entity walking along a block edge does not flicker.
- The server re-evaluates an observer–entity pair when either side changes interest block,
  and checks every pair in full at least once per second.

<a id="s9-3"></a>
### 9.3 Ordering

- For each entity on each connection, the world stream carries: `EntityCreate`, then zero or
  more `EntityUpdate`/`EntityAppearance`, then `EntityDestroy` (or `InterestReset`). After
  that, the cycle may repeat with a new `EntityCreate` of the same ID.
- `MotionBatch` entries for an entity are sent only after its `EntityCreate` has been
  written to the world stream. Datagrams can still overtake the stream, which is why §8.6
  drops unknown IDs.
- Properties and appearance that change in the same tick as a create are folded into that
  `EntityCreate`.

<a id="s9-4"></a>
### 9.4 Budgets per client per tick

| Budget | Default | Over budget |
|---|---|---|
| Entities created at once | ≤ `max_entities` from `Hello` (server cap 1,024 unless `test-server.md` raises it) | The farthest candidates are not created until room frees up |
| New `EntityCreate`s per tick | 16 | Nearest first, the rest wait |
| World-stream bytes written per tick | 64 KiB | Remaining updates wait for the next tick (destroys and location reports are never deferred) |
| World-stream unsent backlog | 1 MiB | Stop new creates and property updates (destroys still go) until the backlog drops below 256 KiB |
| Backlog that does not clear | 4 MiB, or above 1 MiB for 30 s | `Disconnect(SLOW_CONSUMER)` |
| `MotionBatch` datagrams per snapshot | 16 (≤ 464 entries) | Priority rule of §8.5 |

<a id="s9-5"></a>
### 9.5 Region change, teleport and world entry

- **World entry:** `WorldEnter`, `WorldClock`, the player's own `EntityUpdate`, then the
  `EntityCreate`s of the initial interest set (within the budgets).
- **Region change** (only by teleport): the server MUST send `InterestReset` before the
  `LocationReport(TELEPORTED)`. Every entity of the old region is gone, and from then on
  every position is in the new region's frame.
- **Teleport within the region:** the server SHOULD send `InterestReset` when the new
  interest block is more than *R* + 1 blocks from the old one. Otherwise it applies the
  normal enter and leave rules after the report.
- Entering a different **world instance** is the same as a region change.

---

<a id="s10"></a>
## 10. Chat

<a id="s10-1"></a>
### 10.1 Channels and ranges

| Wire value | `ChatChannel` | Who receives it (default ranges; `test-server.md` may configure them) |
|---|---|---|
| 1 Say | `Say` | Players in the same world instance and region within 40 m horizontal distance of the speaker (`Location::distance_xy`) |
| 2 Yell | `Yell` | Every player in the same world instance, any region |
| 3 Emote | `Emote` | As Say |
| 4 Tell | `Tell { peer }` | The one online character named `peer`, anywhere on the server |
| 5 System | `System` | Sent by the server only (to one client, an instance or everyone) |

The speaker always receives its own line, with `ECHO` set.

<a id="s10-2"></a>
### 10.2 Validation and rate limits

- The server validates `text` as `TEXT` and `peer` as `NAME`. A failure gives
  `ChatRejected(INVALID_TEXT)` or `ChatRejected(UNKNOWN_RECIPIENT)`.
- **Rate limit:** a token bucket per connection with capacity 5 and a refill of 1 token per
  2 s (sustained 30 lines per minute). Each `ChatSend` costs 1 token. With no token left:
  `ChatRejected(RATE_LIMITED)`. 20 rejections within 60 s lead to `Disconnect(FLOODING)`.
- A muted player (server policy) gets `ChatRejected(MUTED)`.

<a id="s10-3"></a>
### 10.3 Delivery

- Say, Yell, Emote: each recipient gets a `ChatMessage` with the channel, `sender` = the
  speaker's character name, and `sender_entity` (or 0). The speaker gets the same with `ECHO`.
- Tell: the recipient gets channel 4, `sender` = `peer` = the speaker's name. The speaker gets
  an echo with `ECHO`, `sender` = its own name and `peer` = the recipient's name. Recipient
  name matching is by the server's character-name rule (`test-server.md`).
- Lines from one speaker reach each recipient in the order they were sent.
- `ServerNotice`, refusals the server explains in words (§6.2.17), and the optional
  description of a `QueryLocation` are System lines.

<a id="s10-4"></a>
### 10.4 Maximum length

1,024 bytes of UTF-8 (`MAX_CHAT_LEN`) for `text`, and 64 bytes for names (`MAX_NAME_LEN`).

<a id="s10-5"></a>
### 10.5 Display

Clients show received chat as **plain text**. They never interpret markup, escape codes or
links in it, and they sanitise control characters (§5.2).

---

<a id="s11"></a>
## 11. Time

<a id="s11-1"></a>
### 11.1 Server time and ticks

- **Server time** is a u64 count of microseconds since an arbitrary epoch the server picks
  at start (for example process start). It is monotonic and shared by all connections of
  that server process.
- The server simulates **ticks** at `tick_rate` (30 Hz). Tick *k* begins at server time
  ⌊*k* · 10⁶ / `tick_rate`⌋ µs. `TimeReply.tick` is the tick in progress at *t*₂.
- Every `server_time` field (and the Time property kind) is on this clock.

> **OPEN:** (review, non-blocking, round 2) Two readings differ in `tessera-server`. (1) Its
> run loop schedules ticks by adding 33,333 µs (10⁶ / 30, truncated) each time instead of
> starting tick *k* at ⌊*k* · 10⁶ / 30⌋, so the schedule drifts 10 µs per second ahead of
> this section's boundaries (36 ms per hour), and recomputing the tick number from the
> wake time then repeats a tick number once per drift step, which shifts the 15 Hz snapshot
> cadence. (2) `MotionBatch` and `EntityCreate` `server_time` carry the wake time of the
> tick (scheduler latency included), while §6.5.2 and §8.5 say "the snapshot tick's time".
> Neither changes bytes. Spec writer: say whether "the tick's time" means ⌊*k* · 10⁶ /
> `tick_rate`⌋ exactly or the server time at which the snapshot was taken.

<a id="s11-2"></a>
### 11.2 Clock-offset estimation (NTP-style)

The client stamps *t*₀ (its monotonic clock, µs) in `TimeRequest`. The server stamps *t*₁ on
receipt and *t*₂ on sending `TimeReply`. The client stamps *t*₃ on receipt. Following RFC
5905 §8:

- offset θ = ((*t*₁ − *t*₀) + (*t*₂ − *t*₃)) / 2;
- round-trip delay δ = (*t*₃ − *t*₀) − (*t*₂ − *t*₁);
- estimated server time = client clock + θ.

Recommended procedure (non-normative except the rate limit):

1. After `HelloAck`, send 4 requests 250 ms apart, then one every 5 s.
2. Ignore replies whose *t*₀ was not sent in the last 10 s, and replies with δ < 0.
3. Keep the last 8 samples and use θ of the sample with the **smallest δ**.
4. If the new estimate differs from the one in use by more than 100 ms, step to it.
   Otherwise slew by at most 5 ms per second of real time, so render time (§8.6) never runs
   backwards.

The server answers at most 4 `TimeRequest`s per second per connection and drops the rest
silently.

> **OPEN:** (review, non-blocking) `tessera-server` stamps *t*₁ and *t*₂ with the same
> server time, taken when its per-connection session processes the datagram on the network
> thread, not when the datagram arrived. That satisfies *t*₂ ≥ *t*₁ and only moves the
> server's queueing time into δ, which the minimum-δ filter discounts. The §17.1 note on
> rounding θ towards −∞ is fine; the spec should state it, because two implementations that
> round differently disagree by 1 µs.

<a id="s11-3"></a>
### 11.3 World clock and time of day

`WorldClock` gives the game clock *G* at a server time. At estimated server time *s*, the
client computes *G*(*s*) = `game_clock` + `rate` · (*s* − `server_time`) / 10⁶, and from it the
time of day, year and day with the constants of **its own data** (`environment.md` §11.1:
*T* = *G* + `zero_offset`, ...). The server knows nothing about those constants. It runs one
game clock per world instance (`test-server.md`).

---

<a id="s12"></a>
## 12. Connection state machines and timeouts

<a id="s12-1"></a>
### 12.1 Client

| State | Entered when | Leaves to | Timeout (default) |
|---|---|---|---|
| Idle | Created | Handshaking (connect) | — |
| Handshaking | QUIC/TLS started | Greeting (pin verified, handshake done) | 10 s → `TIMEOUT` |
| Greeting | `Hello` sent | Authenticating (`HelloAck`) | 10 s |
| Authenticating | `HelloAck` received | CharacterSelect (`LoginOk`); stays on `LoginFailed` | 30 s per `Login` answer |
| CharacterSelect | `LoginOk` | EnteringWorld (`EnterWorld` sent) | — (the server's idle rule applies) |
| EnteringWorld | `EnterWorld` sent | InWorld (`WorldEnter` read); CharacterSelect (`EnterWorldFailed`) | 30 s |
| InWorld | `WorldEnter` processed | Closing | — |
| Closing | `Disconnect` sent or received, or a fatal error | Closed | 2 s linger (§12.3) |
| Closed | Connection closed | — | — |

Any state goes to Closing on a fatal error, and to Closed when the QUIC idle timeout (30 s)
fires.

<a id="s12-2"></a>
### 12.2 Server, per connection

| State | Entered when | Leaves to | Timeout (default) |
|---|---|---|---|
| Accepting | QUIC handshake started | AwaitHello | 10 s |
| AwaitHello | Handshake complete | AwaitLogin (`HelloAck` sent) | 10 s → `Disconnect(AUTH_TIMEOUT)` |
| AwaitLogin | `HelloAck` sent | CharacterSelect (`LoginOk` sent). At most 3 failures → `Disconnect(AUTH_FAILED)` | 60 s from `HelloAck` → `Disconnect(AUTH_TIMEOUT)` |
| CharacterSelect | `LoginOk` sent | InWorld (`WorldEnter` sent) | 15 min without a message → `Disconnect(IDLE)` |
| InWorld | `WorldEnter` sent | Closing | Inactivity limit is server policy (`test-server.md`), `IDLE` |
| Closing | `Disconnect` sent or received | Closed | 2 s linger |


> **OPEN:** (review, non-blocking, round 2) "15 min without a message" never fires for a
> Tessera client: every control message resets it, `Ping` included, and `tessera-net` sends
> a `Ping` every 2 s in every state after `HelloAck`. A client left at character selection
> holds its connection slot indefinitely. `test-server.md` §8.1 already leaves `Ping` out of
> in-world activity. Spec writer: "15 min without a message other than `Ping` or `Pong`".

<a id="s12-3"></a>
### 12.3 Closing procedure

1. The closing side sends `Disconnect(code, text)` on the control stream, finishes (FIN)
   its control stream, and stops sending other messages.
2. A side that receives `Disconnect` stops sending, and closes the QUIC connection with
   application error 0 (`NO_ERROR`).
3. If the closing side has not seen the connection closed within 2 s, it closes the
   connection itself with application error = `code` and a reason phrase of at most 64 bytes.
4. On a protocol violation or another local fatal condition, an endpoint MAY skip step 1
   and close at once with the code.

The client reports the cause from the first source available: the `Disconnect` message, the
peer's CONNECTION_CLOSE application code and reason, the idle timeout, or the local error.

---

<a id="s13"></a>
## 13. Hostile-input rules

Every byte from the peer is hostile. No input may panic either side or make it allocate
beyond the limits below.

<a id="s13-1"></a>
### 13.1 Limits

| Item | Limit | Exceeded |
|---|---|---|
| Frame `length` | 2–65,536 | Fatal |
| Datagram size | ≤ 1,200 bytes | Fatal |
| Body bytes left over or missing | 0 | Fatal |
| Any `str` length above its class maximum | §5.2 | Fatal |
| Invalid UTF-8 | — | Fatal |
| Non-finite float, `bool` ∉ {0, 1}, `opt` tag ∉ {0, 1} | — | Fatal |
| Location coordinates | finite, \|c\| ≤ 10⁷ | Fatal |
| Landblock index | ≤ 254 | Fatal |
| Anchor ownership | ±1,280 m | Fatal |
| Appearance: scale (0, 100], ≤ 32 items, dye in [0, 1], item ≠ 0, flag bits 4–7 = 0; bit 3 = 0 at minor 0; at minor ≥ 1 a `slot` (bit 3) in 1–64 | — | Fatal |
| Look (minor ≥ 1): ≤ 48 entries, encoding ≤ 768 bytes (checked while decoding, before further allocation), `property` ≠ 0 and strictly ascending, the ten allowed tags only, String 0–128 bytes of 0x21–0x7E, Float32 finite | — | Fatal |
| `LoginOk.looks`, `CharacterList.looks` (minor ≥ 1) | count = `characters` count | Fatal |
| Characters in a list | 64 | Fatal |
| PropList entries | 256, `pid` strictly ascending and ≠ 0 | Fatal |
| Value depth, array length, struct fields, one value's size | 4, 256, 64, 8,192 bytes | Fatal |
| `MotionBatch` count | 1–29 and the exact length | Fatal |
| Enumerated fields | §6 lists | Fatal |
| Unknown message type, wrong channel, wrong direction, wrong state | — | Fatal (datagrams in the wrong state: dropped) |
| Extra or wrongly-kinded streams, FIN/reset outside Closing | — | Fatal |
| Text class content (control characters) from the server | — | Sanitised |
| Text class content from the client | — | The request's own failure code |
| `Ping` | ≤ 1 per second | Excess ignored (no `Pong`) |
| `TimeRequest` | ≤ 4 per second | Excess dropped |
| `MoveInput` | ≤ 30 per second | Excess dropped |
| `ChatSend` | Bucket of §10.2 | `ChatRejected`; repeated → `FLOODING` |
| Control messages from the client (all types together) | 50 per second, averaged over 5 s | `Disconnect(FLOODING)` |
| Outstanding requests | One `Login`/`Register`, one `CharacterCreate`/`EnterWorld` | A second one while one is outstanding is fatal |

<a id="s13-2"></a>
### 13.2 Dropped, not fatal

These cases are normal on a real network or under race conditions. The receiver drops the
message, counts it, and logs it at debug level:

- datagrams that arrive before or after the state that allows them;
- `MoveInput` with a stale epoch or an old `seq`;
- `MotionBatch` entries for unknown IDs, or not newer than the entity's last applied state;
- `TimeReply` with an unknown or too-old *t*₀;
- on the client, a well-formed `EntityUpdate`, `EntityAppearance` or `EntityDestroy` for an
  ID that is not currently created, or an `EntityCreate` for an ID that already is. These
  are server bugs, logged at warning level. They cannot corrupt client state, so they do
  not end the session.


> **OPEN:** (review, non-blocking, round 2) The last bullet asks for a warning-level line
> for every such message with no rate limit, and the cases are never fatal. A hostile
> server can stream 15-byte `EntityDestroy` frames for an unknown ID without end and make
> the client write several times that traffic to its log on the network thread
> (`ClientSession` logs each one with `tracing::warn!`). Spec writer: count every case but
> log at warning level rate-limited (the first few, then one summary per interval with the
> count), or make a sustained rate of these server bugs fatal (`PROTOCOL_VIOLATION`).

<a id="s13-3"></a>
### 13.3 Resource rules

- Decoders check every length and count against the remaining bytes and the limits
  **before** allocating.
- Per connection, the server keeps at most one outstanding request of each kind (§13.1), and
  bounded queues between its network and simulation threads (decision 0030). When a queue
  is full, movement is dropped first. Control and chat are never dropped. A connection whose
  control queue stays full for 5 s is closed with `SLOW_CONSUMER`.
- The client's event channel to the main thread is bounded (ARCH §19.4). When it is full,
  the network thread stops reading the world and chat streams (QUIC flow control then pushes
  back on the server) and drops movement datagrams, and so never loses reliable state.

> **OPEN:** (review, round 2) "Bounded" in items does not bound memory. Decoded values are
> about 15 times larger than their wire form (a 65,464-byte `EntityUpdate` of server-private
> nested Bool arrays, within every §13.1 limit, decodes to about 1 MB of heap), and
> `tessera-net` pauses reading only at 256 waiting items on top of a 1,024-item channel, so
> a hostile server can make a stalled client hold about 1.3 GB. The properties that blow up
> are ones §7.1 says to skip anyway. Spec writer: state the client's bound in decoded or
> wire bytes (for example "pause reading at a few MiB of queued input"), and recommend that
> receivers drop unknown property IDs and unknown `EMOTE` fields at decode time, before
> queueing.

> **OPEN:** (review, non-blocking, round 2) This section bounds the server's control queue
> and §9.4 the world-stream backlog, but nothing bounds the **chat-stream** backlog, and
> "control and chat are never dropped" invites unbounded buffering: a client that never
> reads its chat stream while others Yell to the instance makes a server built from this
> spec alone queue chat without limit (QUIC flow control does not help, because frames are
> queued above QUIC). `test-server.md` §10.1 has a server-local rule (256 KiB unsent for 30 s,
> or 1 MiB, then `SLOW_CONSUMER`). Spec writer: add it here or to §9.4, with those defaults
> cited from `test-server.md`.

---

<a id="s14"></a>
## 14. Codes

<a id="s14-1"></a>
### 14.1 Application error codes (`Disconnect.code` and QUIC CONNECTION_CLOSE application codes)

| Code | Name | Sent by | Meaning |
|---|---|---|---|
| 0x00 | `NO_ERROR` | Both | Normal close (user disconnect, acknowledging a `Disconnect`) |
| 0x01 | `PROTOCOL_VIOLATION` | Both | Any fatal rule of this spec was broken. The reason phrase SHOULD name it |
| 0x02 | `UNSUPPORTED_VERSION` | S | No common minor version |
| 0x03 | `DATAGRAMS_REQUIRED` | Both | The peer did not offer RFC 9221 datagrams |
| 0x04 | `AUTH_FAILED` | S | Too many failed logins on this connection |
| 0x05 | `AUTH_TIMEOUT` | S | No `Hello` or no successful login in time |
| 0x06 | `SERVER_SHUTDOWN` | S | The server is stopping |
| 0x07 | `KICKED` | S | Removed by the operator |
| 0x08 | `LOGGED_IN_ELSEWHERE` | S | The account logged in on another connection |
| 0x09 | `IDLE` | S | Inactive for too long |
| 0x0A | `FLOODING` | S | Rate limits exceeded repeatedly |
| 0x0B | `SLOW_CONSUMER` | S | The client does not read fast enough (§9.4) |
| 0x0C | `SERVER_FULL` | S | No room for another connection |
| 0x0D | `INTERNAL_ERROR` | Both | Unexpected local failure |

Other codes: receivers treat them as `INTERNAL_ERROR`.

<a id="s14-2"></a>
### 14.2 Client-local connection failures (never on the wire)

`TLS_FAILURE` (handshake or certificate parsing failed, or the key is not Ed25519),
`KEY_MISMATCH` (fingerprint differs from the pin or the known-servers entry),
`KEY_UNKNOWN` (no pin and no entry, and trust on first use was declined or not allowed),
`NO_COMMON_VERSION` (ALPN failed, QUIC error 0x0178), `TIMEOUT` (handshake or idle timeout).

<a id="s14-3"></a>
### 14.3 `LoginFailed` codes

| Code | Name | Meaning |
|---|---|---|
| 1 | `BAD_CREDENTIALS` | Unknown account or wrong password (deliberately not distinguished) |
| 2 | `ACCOUNT_DISABLED` | The operator disabled the account |
| 3 | `RATE_LIMITED` | Too many attempts. Wait `retry_after` seconds |
| 4 | `ALREADY_ONLINE` | The account is in another session and the server refuses a second one |
| 5 | `SERVER_FULL` | Followed by `Disconnect(SERVER_FULL)` |
| 6 | `REGISTRATION_CLOSED` | `Register` not allowed now |
| 7 | `ACCOUNT_EXISTS` | `Register` for a name already taken |
| 8 | `INVALID_ACCOUNT_NAME` | Not an `ACCOUNT` string |
| 9 | `PASSWORD_REJECTED` | `Register`: the password breaks the server's policy |
| 10 | `SERVER_ERROR` | Followed by `Disconnect(INTERNAL_ERROR)` |

<a id="s14-4"></a>
### 14.4 `CharacterCreateFailed` codes

1 `NAME_TAKEN`, 2 `NAME_INVALID`, 3 `TOO_MANY` (64 characters, or a lower server limit),
4 `APPEARANCE_REFUSED` (content policy), 5 `DISABLED`, 6 `SERVER_ERROR`.

<a id="s14-5"></a>
### 14.5 `EnterWorldFailed` codes

1 `UNKNOWN_CHARACTER`, 2 `NO_INSTANCE` (no world instance can take the character),
3 `CHARACTER_BUSY` (already in the world on another connection), 4 `SERVER_ERROR`.

<a id="s14-6"></a>
### 14.6 `TeleportRefused` codes

1 `UNKNOWN_PLACE` (a `Named` destination the server does not know), 2 `NOT_ALLOWED` (policy,
or a region the server does not serve), 3 `CANCELLED` (superseded by a newer request),
4 `INVALID_DESTINATION` (for example an invalid place name), 5 `SERVER_ERROR`.

<a id="s14-7"></a>
### 14.7 `ChatRejected` codes

1 `RATE_LIMITED`, 2 `UNKNOWN_RECIPIENT` (no such character online), 3 `INVALID_TEXT`,
4 `MUTED`, 5 `NOT_IN_WORLD`.

Unknown values of any code in §14.3–§14.7: clients show a generic failure. They are not a
protocol violation, because codes are informational.

> **OPEN:** (review, non-blocking, round 2) `NOT_IN_WORLD` has no defined trigger: §6.1
> allows `ChatSend` only InWorld, so outside it the server closes the connection.
> `tessera-server` uses the code for a race the spec does not describe (the session is
> InWorld but the simulation has not yet placed, or has already removed, the player).
> Spec writer: state that case, or mark code 5 reserved.

---

<a id="s15"></a>
## 15. Invariants

1. Every frame's `length` equals 2 + its body length, and no frame exceeds 65,540 bytes on
   the wire.
2. Decode → encode reproduces every valid message byte for byte. There is exactly one
   encoding per value: no optional padding, ascending PIDs, fixed-width integers.
3. Arbitrary bytes fed to any decoder (frame, datagram, value) never panic, and never
   allocate more than the stated limits.
4. No password or account name is sent before the server's key has been verified against
   the expected fingerprint. No 0-RTT data is ever sent.
5. On a connection, each entity follows create (update | appearance)* (destroy | reset)
   on the world stream. A client never surfaces an update for an entity it has not created.
6. Each `TeleportRequest` gets exactly one answer (`LocationReport(TELEPORTED)` or
   `TeleportRefused`) carrying its `request_id`.
7. Positions in `EntityCreate` and `MotionBatch` are in the receiver's current region. A
   region change is always preceded by `InterestReset` on the world stream.
8. Accepted movement never exceeds the allowance of §8.3 together with the relaxations that
   §8.3 permits and the server applies: within one epoch, over any stretch of *t* seconds of
   server time, the horizontal distance of the accepted `MoveInput`s is at most
   *V*ₕ · (*t* + *W*) + *S* + the descent they count (the sum of *d*_c of §8.3 item 2; *S* = 0
   and no counted descent without the descent credit or with *S* = 0), and,
   outside a settling window, the rise at most *V*ᵤ · (*t* + *W*) and the descent at most
   *V*_d · (*t* + *W*). No allowance is ever below 0. Every rejection that is reported advances
   the epoch (rejections beyond the 4 corrections per second of §8.3 step 7 are dropped
   without a report). (Reworded 2026-10-09; parenthesis and allowance floor revised 2026-10-09
   after review.) When the server changes the parameters within the stretch (§8.3 "Changing
   the parameters during an epoch"), the bounds hold with the largest *V*ₕ, *V*ᵤ, *V*_d, *W*
   and *S* in force at any time during the stretch, and no allowance ever exceeds *V* · *W*
   nor *C* exceeds *S* for the values in force at that moment. (Added 2026-10-10.)

> **Answered (2026-10-09, spec writer; was OPEN, implementer note):** invariant 8 now says
> "every rejection that is reported advances the epoch", as the review had confirmed.
9. The server never sends static world objects or game data. Every DID and data key on the
   wire is an opaque number the client resolves against its own install.
10. (v1.1) At negotiated minor 0 every message is byte for byte a v1.0 message: no `Equipped`
    flag bit 3 and no appended look field is ever sent.
11. (v1.1) A server sends a character's look exactly as the client created it, and never
    derives, completes, changes or merges looks; a receiver never applies a look entry that
    fails the rule of §7.4 "Receiving a look".

> **OPEN:** (review, non-blocking) Invariant 2 cannot hold for server text that contains
> control characters: such a message is valid (§5.2 sanitises it rather than failing), the
> decoded value has spaces in their place, and `tessera_protocol`'s encoder refuses control
> characters. Reword as "every valid message whose text fields contain no control
> characters". Invariant 8: confirmed the implementer's reading above ("every rejection
> that is reported advances the epoch"); `MovementValidator` follows §8.3 step 7.

---

<a id="s16"></a>
## 16. Test vectors

All vectors are **plaintext application bytes**. Stream frames include the 4-byte length
and the 2-byte type, and datagrams include the 2-byte type. The QUIC and TLS layers add
encryption with random keys, so their bytes are not reproducible, and these vectors stop at
the application layer. The kind byte (`57` or `43`) that starts each server uni stream is not
repeated here. Values used:

- Location L = region 1, x 20,250.5, y 22,500.25, z 401.0 (landblock (126, 140); anchor
  (126, 140, cell 257) owns it).
- Appearance A = Character (species 23, sex 1), scale 1.0, no idle; items 0x70001234 with
  dye 0.25, and 0x70005678 stowed. (All DIDs and keys here are made-up numbers.)
- Player ID 42 ("Aldith"), another player 43 ("Bram"), server time in µs.

<a id="s16-1"></a>
### 16.1 Key fingerprint and ALPN

```
Ed25519 public key (RFC 8032 §7.1 TEST 1):
  d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
SubjectPublicKeyInfo DER (44 bytes):
  30 2a 30 05 06 03 2b 65 70 03 21 00 d7 5a 98 01 82 b1 0a b7 d5 4b fe d3 c9 64 07 3a
  0e e1 72 f3 da a6 23 25 af 02 1a 68 f7 07 51 1a
Fingerprint text:
  sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9
ALPN "tessera/1":
  74 65 73 73 65 72 61 2f 31
Connect string with pin (example port):
  tessera://192.0.2.10:47000?pin=sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9
```

<a id="s16-2"></a>
### 16.2 Control stream (handshake frames first)

```
Hello  minor_max 0, caps 0, max_entities 1024, client_name "tessera-client 0.7.0"   (36 bytes)
  20 00 00 00 01 00 00 00 00 00 00 00 00 04 14 00 74 65 73 73 65 72 61 2d 63 6c 69 65
  6e 74 20 30 2e 37 2e 30
HelloAck  minor 0, caps REGISTRATION|CHARACTER_CREATION, tick 30, snapshot 15, R 2,
          server_name "Test Shard", motd ""                                           (29 bytes)
  19 00 00 00 02 00 00 00 03 00 00 00 1e 0f 02 0a 00 54 65 73 74 20 53 68 61 72 64 00
  00
Login  account "alice", password "correct horse"                                     (28 bytes)
  18 00 00 00 03 00 05 00 61 6c 69 63 65 0d 00 63 6f 72 72 65 63 74 20 68 6f 72 73 65
Register  account "bob", password "battery staple"                                   (27 bytes)
  17 00 00 00 04 00 03 00 62 6f 62 0e 00 62 61 74 74 65 72 79 20 73 74 61 70 6c 65
LoginOk  account "alice", characters [("Aldith", Character 23/1)]                    (31 bytes)
  1b 00 00 00 05 00 05 00 61 6c 69 63 65 01 06 00 41 6c 64 69 74 68 01 17 00 00 00 01
  00 00 00
LoginFailed  BAD_CREDENTIALS, retry_after 0, text ""                                  (12 bytes)
  08 00 00 00 06 00 01 00 00 00 00 00
CharacterCreate  "Aldith", Character 23/1, scale 1.0, no idle, no items               (29 bytes)
  19 00 00 00 07 00 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 00 00 80 3f 00
  00
CharacterList  [("Aldith", Character 23/1), ("Bram", Character 81/2)]                (39 bytes)
  23 00 00 00 08 00 02 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 04 00 42 72
  61 6d 01 51 00 00 00 02 00 00 00
CharacterCreateFailed  NAME_TAKEN, ""                                                 (10 bytes)
  06 00 00 00 09 00 01 00 00 00
EnterWorld  "Aldith"                                                                  (14 bytes)
  0a 00 00 00 0a 00 06 00 41 6c 64 69 74 68
EnterWorldFailed  CHARACTER_BUSY, ""                                                  (10 bytes)
  06 00 00 00 0b 00 03 00 00 00
TeleportRequest  id 7, Ground (region 1, 20000, 22000, 0), heading 0                  (41 bytes)
  25 00 00 00 0c 00 07 00 00 00 02 01 00 00 00 00 00 00 88 d3 40 00 00 00 00 00 7c d5
  40 00 00 00 00 00 00 00 00 00 00 00 00
TeleportRequest  id 8, Anchored (L, heading 1.5, anchor 126/140 cell 257)            (45 bytes)
  29 00 00 00 0c 00 08 00 00 00 03 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5
  40 00 00 00 00 00 10 79 40 00 00 c0 3f 7e 8c 01 01
TeleportRequest  id 9, Named "origin"                                                 (19 bytes)
  0f 00 00 00 0c 00 09 00 00 00 04 06 00 6f 72 69 67 69 6e
QueryLocation  id 10                                                                  (10 bytes)
  06 00 00 00 0d 00 0a 00 00 00
ChatSend  Say "hello"                                                                 (14 bytes)
  0a 00 00 00 0e 00 01 05 00 68 65 6c 6c 6f
ChatSend  Tell peer "Bram", "hi"                                                      (17 bytes)
  0d 00 00 00 0e 00 04 04 00 42 72 61 6d 02 00 68 69
EmoteSet  anim 0x10000123, looping false                                              (11 bytes)
  07 00 00 00 0f 00 23 01 00 10 00
StanceSet  Ready                                                                       (7 bytes)
  03 00 00 00 10 00 01
AppearanceSet  A                                                                      (35 bytes)
  1f 00 00 00 11 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02 34 12 00 70 01 00 00
  80 3e 78 56 00 70 04
Ping  nonce 0x0123456789abcdef                                                        (14 bytes)
  0a 00 00 00 12 00 ef cd ab 89 67 45 23 01
Pong  nonce 0x0123456789abcdef                                                        (14 bytes)
  0a 00 00 00 13 00 ef cd ab 89 67 45 23 01
Disconnect  NO_ERROR, ""                                                              (10 bytes)
  06 00 00 00 14 00 00 00 00 00
```

<a id="s16-3"></a>
### 16.3 World stream

```
WorldEnter  player 42, "Aldith", instance 1, epoch 1, time 5,000,000, pose (L, 0.0, no
            anchor), A                                                                (98 bytes)
  5e 00 00 00 01 01 2a 00 00 00 00 00 00 00 06 00 41 6c 64 69 74 68 01 00 00 00 01 00
  00 00 40 4b 4c 00 00 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
  00 00 00 00 00 10 79 40 00 00 00 00 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02
  34 12 00 70 01 00 00 80 3e 78 56 00 70 04
LocationReport  TELEPORTED, request 7, epoch 2, time 6,000,000, pose (L, 1.5, anchor
                126/140 cell 257)                                                     (58 bytes)
  36 00 00 00 02 01 01 07 00 00 00 02 00 00 00 80 8d 5b 00 00 00 00 00 01 00 00 00 00
  00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40 00 00 00 00 00 10 79 40 00 00 c0 3f 01 7e 8c
  01 01
TeleportRefused  request 9, UNKNOWN_PLACE, ""                                         (14 bytes)
  0a 00 00 00 03 01 09 00 00 00 01 00 00 00
EntityCreate  id 43, Player, "Bram", no class, time 6,000,000, pose ((1, 20260, 22500,
              400.5), 3.0, no anchor), velocity (2, 0, 0), Walking, Character 81/2 scale 1
              no items, properties {STANCE: Enum 0, RUN_SPEED: Float32 6.0}          (105 bytes)
  65 00 00 00 04 01 2b 00 00 00 00 00 00 00 00 04 00 42 72 61 6d 00 80 8d 5b 00 00 00
  00 00 01 00 00 00 00 00 00 c9 d3 40 00 00 00 00 00 f9 d5 40 00 00 00 00 00 08 79 40
  00 00 40 40 00 00 00 00 40 00 00 00 00 00 00 00 00 01 01 51 00 00 00 02 00 00 00 00
  00 80 3f 00 00 02 00 01 00 05 00 00 00 00 04 00 04 00 00 c0 40
EntityUpdate  id 43, {STANCE: Enum 2, EMOTE: Struct {1: Enum 0x10000123, 2: Time
              6,500,000, 3: Bool false}}                                              (50 bytes)
  2e 00 00 00 05 01 2b 00 00 00 00 00 00 00 02 00 01 00 05 02 00 00 00 02 00 0f 03 00
  01 00 05 23 01 00 10 02 00 0c a0 2e 63 00 00 00 00 00 03 00 01 00
EntityUpdate  id 43, {EMOTE: None (removed)}                                         (19 bytes)
  0f 00 00 00 05 01 2b 00 00 00 00 00 00 00 01 00 02 00 00
EntityAppearance  id 43, Object 0x79000001, scale 2.5, idle 0x10000001, no items     (29 bytes)
  19 00 00 00 06 01 2b 00 00 00 00 00 00 00 02 01 00 00 79 00 00 20 40 01 01 00 00 10
  00
EntityDestroy  id 43, LEFT_INTEREST                                                   (15 bytes)
  0b 00 00 00 07 01 2b 00 00 00 00 00 00 00 00
InterestReset                                                                          (6 bytes)
  02 00 00 00 08 01
WorldClock  time 5,000,000, game_clock 43,200.0 s, rate 1.0                          (26 bytes)
  16 00 00 00 09 01 40 4b 4c 00 00 00 00 00 00 00 00 00 00 18 e5 40 00 00 80 3f
```

> **OPEN:** (review, non-blocking) The `EntityCreate` vector creates entity 43 ("Bram", not
> the receiving player 42) with `RUN_SPEED`, which §7.3 marks "Owner only": "sent only to
> the connection that controls the entity". The bytes decode fine (receivers have no scope
> check), so the vector still tests the layout, but it shows a server doing what §7.3
> forbids. Spec writer: use a property of scope "All observers" or a server-private ID
> (0x8000 and up), recompute the vector, and update `crates/protocol/tests/vectors.rs` and
> `spec_conformance.rs` with it.

<a id="s16-4"></a>
### 16.4 Chat stream

```
ChatMessage  time 7,000,000, Say, no flags, sender 43 "Bram", "hello"                (37 bytes)
  21 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 01 00 2b 00 00 00 00 00 00 00 04 00 42 72
  61 6d 05 00 68 65 6c 6c 6f
ChatMessage  time 7,000,000, Tell, ECHO, sender 42 "Aldith", peer "Bram", "hi"       (42 bytes)
  26 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 04 01 2a 00 00 00 00 00 00 00 06 00 41 6c
  64 69 74 68 04 00 42 72 61 6d 02 00 68 69
ChatMessage  time 7,000,000, System, sender 0 "", "Welcome"                          (35 bytes)
  1f 00 00 00 01 02 c0 cf 6a 00 00 00 00 00 05 00 00 00 00 00 00 00 00 00 00 00 07 00
  57 65 6c 63 6f 6d 65
ServerNotice  Shutdown, 60 s, "restart"                                               (18 bytes)
  0e 00 00 00 02 02 02 3c 00 07 00 72 65 73 74 61 72 74
ChatRejected  RATE_LIMITED, ""                                                        (10 bytes)
  06 00 00 00 03 02 01 00 00 00
```

<a id="s16-5"></a>
### 16.5 Datagrams

```
MoveInput  seq 100, epoch 2, L, heading 1.5, velocity (0, 6, 0), Running              (53 bytes)
  01 03 64 00 00 00 02 00 00 00 01 00 00 00 00 00 a0 c6 d3 40 00 00 00 00 10 f9 d5 40
  00 00 00 00 00 10 79 40 00 00 c0 3f 00 00 00 00 00 00 c0 40 00 00 00 00 02
MotionBatch  time 6,066,666, 1 entry: id 43, (20260, 22500, 400.5), heading 3.0,
             velocity (−2, 0, 0), Walking                                             (48 bytes)
  02 03 ea 91 5c 00 00 00 00 00 01 2b 00 00 00 00 00 00 00 00 48 9e 46 00 c8 af 46 00
  40 c8 43 00 00 40 40 00 00 00 c0 00 00 00 00 00 00 00 00 01
TimeRequest  t0 1,000,000                                                             (10 bytes)
  03 03 40 42 0f 00 00 00 00 00
TimeReply  t0 1,000,000, t1 5,020,000, t2 5,020,100, tick 150                        (34 bytes)
  04 03 40 42 0f 00 00 00 00 00 60 99 4c 00 00 00 00 00 c4 99 4c 00 00 00 00 00 96 00
  00 00 00 00 00 00
```

Worked time-sync example: with the reply above received at *t*₃ = 1,040,100 µs (client
clock), θ = ((5,020,000 − 1,000,000) + (5,020,100 − 1,040,100)) / 2 = 4,000,000 µs, and
δ = 40,100 − 100 = 40,000 µs.

<a id="s16-6"></a>
### 16.6 Tagged values

```
Bool true              01 01
Int32 −2               02 fe ff ff ff
Int64 −2               03 fe ff ff ff ff ff ff ff
Float32 6.0            04 00 00 c0 40
Enum 2                 05 02 00 00 00
DataId 0x70001234      06 34 12 00 70
String "ab"            07 02 00 61 62
Vector3 (1, 2, 3)      08 00 00 80 3f 00 00 00 40 00 00 40 40
Bitfield32 0x80000001  09 01 00 00 80
Bitfield64 1 << 40     0a 00 00 00 00 00 01 00 00
EntityRef 43           0b 2b 00 00 00 00 00 00 00
Time 6.5 s             0c a0 2e 63 00 00 00 00 00
Colour (255,128,0,255) 0d ff 80 00 ff
Array [Int32 1, 2]     0e 02 00 02 01 00 00 00 02 02 00 00 00
Struct {1: Bool false} 0f 01 00 01 00 01 00
None (removal)         00
```

<a id="s16-7"></a>
### 16.7 Negative vectors (each MUST be rejected as a protocol violation)

| Input | Why |
|---|---|
| `01 00 00 00 01` (frame length 1) | `length` < 2 |
| `01 00 01 00` + … (frame length 65,537) | `length` > 65,536 |
| `StanceSet` with body `03` | Stance value 3 undefined |
| `Hello` frame with one extra trailing `00` and `length` + 1 | Bytes left after the body |
| `MotionBatch` with count 0 (`02 03` + 8 time bytes + `00`) | Count outside 1–29 |
| `EntityUpdate` whose PropList has PIDs 2 then 1 | PIDs not ascending |
| `MoveInput` with x = NaN (`00 00 00 00 00 00 f8 7f` at offset 12) | Non-finite float |
| `ChatSend` with channel 5 | System is server-only |
| Uni stream starting with kind byte `58` | Unknown stream kind |

<a id="s16-8"></a>
### 16.8 Minor 1 (v1.1)

Negotiated minor 1 unless a row says otherwise. Look K = [(0x10000A01, Float32 0.5),
(0x10000A02, Colour (200, 150, 100, 255)), (0x10000A03, String "8080ff")], 32 bytes:
`03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8 96 64 ff 03 0a 00 10 07 06 00 38 30 38 30
66 66`. (All IDs, DIDs and keys are made-up numbers.)

```
Hello  minor_max 1, caps 0, max_entities 1024, client_name "tessera-client 0.8.0"   (36 bytes)
  20 00 00 00 01 00 01 00 00 00 00 00 00 04 14 00 74 65 73 73 65 72 61 2d 63 6c 69 65
  6e 74 20 30 2e 38 2e 30
HelloAck  minor 1, caps REGISTRATION|CHARACTER_CREATION|APPEARANCE_CHANGE, tick 30,
          snapshot 15, R 2, server_name "Test Shard", motd ""                         (29 bytes)
  19 00 00 00 02 00 01 00 07 00 00 00 1e 0f 02 0a 00 54 65 73 74 20 53 68 61 72 64 00
  00
CharacterCreate  "Aldith", Character 23/1, scale 1.0, no idle, no items, look K       (61 bytes)
  39 00 00 00 07 00 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 00 00 80 3f 00
  00 03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8 96 64 ff 03 0a 00 10 07 06 00 38
  30 38 30 66 66
LoginOk  account "alice", characters [("Aldith", Character 23/1)], looks [K]          (64 bytes)
  3c 00 00 00 05 00 05 00 61 6c 69 63 65 01 06 00 41 6c 64 69 74 68 01 17 00 00 00 01
  00 00 00 01 03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8 96 64 ff 03 0a 00 10 07
  06 00 38 30 38 30 66 66
CharacterList  [("Aldith", Character 23/1), ("Bram", Character 81/2)], looks [K, none] (73 bytes)
  45 00 00 00 08 00 02 06 00 41 6c 64 69 74 68 01 17 00 00 00 01 00 00 00 04 00 42 72
  61 6d 01 51 00 00 00 02 00 00 00 02 03 01 0a 00 10 04 00 00 00 3f 02 0a 00 10 0d c8
  96 64 ff 03 0a 00 10 07 06 00 38 30 38 30 66 66 00
AppearanceSet  Character 23/1, scale 1.0, no idle; items 0x70001234 dye 0.25 slot 17,
               0x70005678 stowed slot 18                                              (37 bytes)
  21 00 00 00 11 00 01 17 00 00 00 01 00 00 00 00 00 80 3f 00 02 34 12 00 70 09 00 00
  80 3e 11 78 56 00 70 0c 12
EntityAppearance  id 43, Character 81/2, scale 1.0, no idle, item 0x70001234 slot 17,
                  no look                                                             (36 bytes)
  20 00 00 00 06 01 2b 00 00 00 00 00 00 00 01 51 00 00 00 02 00 00 00 00 00 80 3f 00
  01 34 12 00 70 08 11 00
```

Negative vectors (each MUST be rejected as a protocol violation):

| Input | Why |
|---|---|
| The `AppearanceSet` above at negotiated minor 0 | `Equipped` flag bit 3 at minor 0 |
| The `CharacterCreate` above at negotiated minor 0 | 32 bytes left after the body |
| `AppearanceSet` whose first item has flags `08` and slot `00` | `slot` outside 1–64 |
| `AppearanceSet` whose first item has flags `08` and slot `41` | `slot` outside 1–64 |
| `AppearanceSet` whose first item has flags `10` | Flag bit 4 |
| `CharacterCreate` with look `02 02 0a 00 10 01 01 01 0a 00 10 01 00` | Property IDs not ascending |
| `CharacterCreate` with look `01 00 00 00 00 01 01` | Property ID 0 |
| `CharacterCreate` with look `01 01 0a 00 10 0e 00 00` | Array tag in a look |
| `CharacterCreate` with look `01 01 0a 00 10 07 03 00 61 20 62` | Space (0x20) in a look string |
| `CharacterCreate` with a look of count 49 (49 entries `pid` ascending, each Bool) | More than 48 entries |
| `CharacterCreate` with a look of six String entries of 128 bytes each (811 bytes) | Look above 768 bytes |
| `LoginOk` with 1 character and `looks` count 0 | Count differs from `characters` |

---

<a id="appA"></a>
## Appendix A. Mapping to `tessera_net` (non-normative)

How the test-server backend (`tessera-net`) translates between this protocol and the
neutral `Event`/`Command` vocabulary (`crates/net/src/session.rs`).

| `Command` | Wire |
|---|---|
| `Connect` | Open QUIC to the connect string, run §4.3–§4.5, send `Hello`, then `Login` (or `Register`) with the credentials given to the backend's constructor |
| `EnterWorld { character }` | `EnterWorld` |
| `Move { location, heading, velocity }` | `MoveInput`. The backend adds `seq`, `epoch` and the motion byte (§8.1, from velocity and the controller's ground state). It applies the 15 Hz / 1 Hz rate rule of §8.2 |
| `Teleport { destination }` | `TeleportRequest`. The backend assigns `request_id`. `Destination::{Location, Ground, Anchored, Named}` → tags 1–4 |
| `QueryLocation` | `QueryLocation` |
| `Chat { channel, text }` | `ChatSend` (Say 1, Yell 2, Emote 3, `Tell { peer }` 4). `System` is refused by `Session` before it reaches the backend |
| `CreateCharacter { name, appearance, class, nationality }` | `CharacterCreate { name, appearance, look }` under the normative rule of `character-screens.md` §7.1.1 (capability, `Connected` state, appearance required, one outstanding); `class` and `nationality` are dropped (`server-content.md` §7); `look` is the appearance's look at minor ≥ 1 and is not sent at minor 0 (`scenario-feed.md` §9.7). Any other `CreateCharacter` is refused with `NetError::InvalidCommand` and sends nothing |
| Equipment changes of an overlay run (`scenario-feed.md` §12.5) | `AppearanceSet` built by the overlay backend from the server's last appearance of the player and the feed's equipment list (no look; slots at minor ≥ 1), at most one in flight and at most 4 per second |
| `DeleteCharacter` and the other commands of `scenario-feed.md` §9.4 | Refused with `NetError::InvalidCommand` (`scenario-feed.md` §9.5) |
| `Disconnect` | `Disconnect(NO_ERROR)`, then §12.3 |

| Wire | `Event` |
|---|---|
| `LoginOk` | `Connected { characters }` (names), then one `CharacterInfo { name, body, class: None, level: None, look }` per entry, in list order (`look` from `looks` at minor ≥ 1, else none; `scenario-feed.md` §9.7) |
| `LoginFailed` (final) / `Disconnect` / CONNECTION_CLOSE | `Disconnected { reason: ServerClosed(text or code name) }` |
| `CharacterList` (answer to `CharacterCreate`) | `CharacterInfo { name, body, class: None, level: None, look }` for the entry with the outstanding name, else `CharacterRefused { name, reason }` with Tessera's text for `SERVER_ERROR` (`character-screens.md` §7.1.1 step 3); other entries emit nothing |
| `CharacterCreateFailed { code, text }` | `CharacterRefused { name: the outstanding name, reason }` with Tessera's own text per code of §14.4 (unknown codes as 6); `text` logged only (`character-screens.md` §7.1.1 step 4) |
| `EnterWorldFailed` | A System `ChatMessage` with the reason, then `Disconnect(NO_ERROR)` and `Disconnected { reason: ServerClosed(text) }`, because `Session` has no way back from EnteringWorld. See OPEN below |
| `WorldEnter` | `WorldEntered { player, name, transform (f32 of pose) }`, then `AppearanceChanged { player, appearance }` (with `look` and slots at minor ≥ 1), then `LocationReport { cause: Entered, anchor }` |
| `LocationReport(TELEPORTED / REQUESTED / CORRECTED)` | `LocationReport { cause: Teleported / Requested / Corrected }`. For TELEPORTED and CORRECTED, also `EntityMoved` for the player with the same position and zero velocity |
| `TeleportRefused` | `ChatMessage { channel: System, sender: None, text }` |
| `EntityCreate` | `EntityCreated { id, kind, name, class, location, heading, appearance, tag: None }` (`appearance.look` from `look` at minor ≥ 1) |
| `MotionBatch` entry | `EntityMoved { id, transform { position, heading }, velocity }` (after §8.6 staleness; interpolation is the client's job above the backend) |
| `EntityAppearance` | `AppearanceChanged` (`appearance.look` from `look` at minor ≥ 1) |
| `EntityDestroy` | `EntityDespawned { id }` |
| `InterestReset` | `EntityDespawned` for every tracked entity except the player. Not `StaticsCleared`, which is about statics and data-placed entities, which this protocol never sends |
| `ChatMessage` | `ChatMessage { channel, sender: Some(name) / None for System, text }`. `Tell { peer }`: on an echo, `peer` is the recipient |
| `ServerNotice`, `ChatRejected` | System `ChatMessage` |
| QUIC idle timeout | `Disconnected { reason: TimedOut }` |
| Local protocol violation or I/O error | `Disconnected { reason: TransportFailed(detail) }` |

> **Partly answered (2026-10-10, v1.1, decision 0025):** the character list after creation
> (`CharacterList` → `CharacterInfo`, §7.1.1 of `character-screens.md`) and `CharacterCreate`
> are mapped above; the overlay backend sends `AppearanceSet` without a public `Command`.
> The rest stays open:
>
> **OPEN (for `tessera-net`, not the wire):** `Event` has no variant yet for property
> changes (`EntityUpdate`; the feed's `PropertyChanged` of `scenario-feed.md` §9.2 carries
> data property IDs, not §7.1's) and for time sync and the world clock (`TimeReply`,
> `WorldClock`). `Command` has none for `EmoteSet`, `StanceSet`, a public `AppearanceSet` or
> registration, and `Connected` has no failure path before
> world entry. ARCH §19.1 already lists "property changed" and "time sync" events. Adding
> them is a `tessera-net` API change for the implementer, not a protocol change.

---

<a id="s17"></a>
## 17. Open items

> **OPEN:** Password normalisation (§4.7).

> **OPEN:** Key rollover (§4.9).

> **OPEN:** Client-proposed anchors when walking into out-of-square interiors (§8.7).

> **OPEN:** `tessera-net` events and commands for properties, time, emotes, stance and
> appearance changes (Appendix A; the character list and creation are mapped since v1.1).

> **OPEN:** Default ranges (Say 40 m), interest radius (2 blocks), snapshot rate (15 Hz),
> interpolation delay (150 ms) and the movement tolerances of §8.3 are design starting
> points. The M7 soak and LAN tests (ROADMAP M7 exit criterion 1) should confirm or tune them.
> Changing a default is not a protocol change, because servers state rate and radius in
> `HelloAck`.


### 17.1 Implementer notes (`crates/protocol`, 2026-10-06)

Readings the clean-room implementation of `tessera-protocol` had to choose where this spec is
silent or ambiguous. The code follows each note; a reviewer should confirm or correct it.

> **OPEN:** (implementer note) §7.2 depth. Read as: every value has a depth, a top-level
> value 1 and each element or field one more than its Array or Struct; any value deeper than
> 4 is a violation. So `Array(Array(Array(Bool)))` is the deepest legal nesting around a
> scalar, and an Array at depth 4 must be empty.

> **OPEN:** (implementer note) §7.3 `EMOTE`. A wrong kind for fields 1–3 is treated like a
> wrong kind for a defined property (violation). Unknown field IDs are accepted and ignored,
> by analogy with unknown property IDs.

> **OPEN:** (implementer note) §6.2.6 `retry_after` non-zero without `RATE_LIMITED`, and
> §6.4.2 `seconds` non-zero for a level other than Shutdown, are not treated as violations
> (informational fields); the server session always sends 0 there.

> **OPEN:** (implementer note) §6.2.1 `Hello.client_name` is `TEXT0`(64) sent by the client,
> which §5.2 lists as "not sent by clients". Implemented as: length above 64 is fatal, control
> characters are sanitised by the server on receipt. Encoders refuse control characters in
> every field the receiver sanitises, so they never rely on the receiver's sanitising.

> **OPEN:** (implementer note) Field rules the client treats as protocol violations, though
> §13.1 does not list them: `LocationReport(CORRECTED)` with `request_id` ≠ 0;
> `TELEPORTED`/`CORRECTED` whose epoch is not the previous epoch + 1; `REQUESTED` with an epoch
> other than the current one; a region change by `CORRECTED`; a region change by `TELEPORTED`
> while entities are still created or with no `InterestReset` since the previous
> `TELEPORTED` report (a `REQUESTED` or `CORRECTED` report in between does not use it up);
> `EntityCreate` for the player's ID or in another region; `EntityDestroy` for the player;
> `TimeReply` with *t*₂ < *t*₁; duplicate names in a character list (compared after
> sanitising); duplicate IDs in one `MotionBatch`. A `MotionBatch` entry for the receiving
> player is dropped like any unknown ID.

> **OPEN:** (implementer note) §9.4 / §6.2.1: the client treats an `EntityCreate` that would
> exceed its own `Hello.max_entities` as a protocol violation, which keeps its entity table
> bounded.

> **OPEN:** (implementer note) §12.1 does not say which application code a client closes with
> when its own Greeting, Authenticating or EnteringWorld timeout fires. Implemented as an
> immediate close with `NO_ERROR` and reason `TIMEOUT`, reported locally as `TIMEOUT`. The
> server closes a connection still in Accepting after 10 s with `AUTH_TIMEOUT` (no stream
> exists yet for a `Disconnect`). The same applies in AwaitHello when the client has sent
> no byte on the control stream: the server cannot open that stream (the client advertises
> `initial_max_streams_bidi` = 0), so it closes at once with `AUTH_TIMEOUT` instead of
> sending `Disconnect` and lingering.

> **OPEN:** (implementer note) §10.2: a `ChatSend` takes its token before its content is
> checked, and every `ChatRejected` (`INVALID_TEXT`, `UNKNOWN_RECIPIENT`, `MUTED`, ...)
> counts towards the 20 rejections that lead to `FLOODING`, not only `RATE_LIMITED`.

> **OPEN:** (implementer note) §13.1 "50 per second, averaged over 5 s" is implemented as at
> most 250 client control messages in any 5 s window; the 251st ends the session with
> `Disconnect(FLOODING)` (not an immediate close).

> **OPEN:** (implementer note) §4.4 connect strings: a host made only of digits and dots must
> be a valid dotted-decimal IPv4 address; DNS names are limited to 253 bytes; ports with
> leading zeros are refused; IPv6 zone IDs (`%`) are not supported. §4.5: an entry line with
> leading whitespace is invalid (skipped with a warning); a line of only spaces or tabs counts
> as empty.

> **OPEN:** (implementer note) §11.2 θ is an integer number of µs: the halved sum is rounded
> towards negative infinity. On equal δ the newest sample wins.

> **OPEN:** (implementer note) §11.2 step 2: the client remembers the *t*₀ of the requests
> it sent in the last 10 s, at most 40 (4 answers per second over 10 s; the oldest is
> forgotten first), and accepts a `TimeReply` only for one of them, once. Unknown, repeated
> and expired *t*₀ are dropped (§13.2).

> **OPEN:** (implementer note) §12.3 does not say how a receiver treats the peer's QUIC
> CONNECTION_CLOSE reason phrase. The client sanitises it like other server text and cuts
> it to 64 bytes before reporting it.

> **OPEN:** (implementer note) While Closing, the state machines drop malformed or
> unexpected input instead of treating it as a violation, so a queued `Disconnect` is still
> sent; only the peer's `Disconnect` is acted on (close with `NO_ERROR`, §12.3 step 2), also
> when both sides disconnect at once. Once Closed, all input is ignored. FIN, RESET_STREAM
> and STOP_SENDING are fatal outside Closing (§2.3 rule 4) and expected while Closing,
> including a partial frame left at a FIN.

> **OPEN:** (implementer note) §6.2.12: a `TeleportRequest` with an invalid `Named` place is
> answered by the server's session with `INVALID_DESTINATION`, and still supersedes an
> unresolved earlier request, which the server then answers with `CANCELLED`.

> **OPEN:** (implementer note) §4.7: after `LoginFailed(RATE_LIMITED, retry_after)` the
> client session refuses `Login` and `Register` until `retry_after` seconds have passed.

> **Answered (2026-10-10, spec writer, design; was OPEN, implementer note 2026-10-09):** §8.3
> "Server relaxations", out-of-bound values. Confirmed: a validator that clamps a configured
> window above 10 s to 10 s and a reserve above 100 m to 100 m conforms, as does a server that
> refuses such a configuration; §8.3 "Bounds of the values in force" now says so, and that a
> reserve that is 0, negative or not a number turns the credit off. Both relaxations are off
> unless configured. Opening the window per epoch whose starting height the server does not
> know, measuring it from that epoch's start, keeping it across corrections and closing it at
> the next teleport epoch is item 1 as written. `tessera-server` refuses out-of-range values
> at load (`test-server.md` §4.1, §4.5), so its clamping never acts.

> **Answered (2026-10-10, spec writer, design; was OPEN, implementer note 2026-10-10):** §8.3
> settling window and allowances when the limits change during an epoch. The implemented
> choice is adopted as the rule, now normative in §8.3 "Changing the parameters during an
> epoch": the window stays measured from the start of the epoch that opened it, with the new
> length; a length of 0 or an end at or before the change closes it; a closed window never
> reopens; the allowances and *C* are capped by the new *V* · *W* and *S* at the moment of the
> change, and time not yet refilled is refilled at the next check at the new rates. Invariant
> 8 (§15) states the bound across a change. `tessera-server` v1 never changes the limits while
> running (`test-server.md` §4.1), so the rule binds the validator and any server that does.

---

### 17.2 Implementer notes (`tessera-net` test-server backend, 2026-10-06)

Readings the clean-room implementation of `tessera_net::remote` and `tessera_net::interp`
had to choose where this spec is silent or ambiguous. A reviewer should confirm or correct
them.

> **OPEN:** (implementer note) §8.6 vs its own rule: the recommended *D* = 150 ms is below
> the stated bound *D* ≥ 2 / `snapshot_rate` + 1 / `tick_rate` at the default rates
> (2 / 15 + 1 / 30 s = 166.7 ms). `tessera_net` uses the larger of the configured delay
> (default 150 ms) and the bound rounded up to the millisecond, so 167 ms with a default
> server. `test-server.md` §13.2 also fixes *D* = 150 ms for desync sampling. Spec writer:
> pick one (for example "*D* = max(150 ms, 2 / `snapshot_rate` + 1 / `tick_rate`)").

> **OPEN:** (implementer note) Appendix A OPEN on events and commands: no `Event` or
> `Command` variant was added (apps/client is edited in parallel). Property changes are
> applied to a per-entity state read with `RemoteBackend::entity_state` (only the v1
> properties of §7.3; unknown IDs skipped); the world clock with `world_clock`/`game_clock`;
> time sync and transport figures with `stats`; `HelloAck` with `server_info`; emote, stance
> and appearance changes are sent through `RemoteBackend::handle()`. The character list after
> `CharacterCreate` and registration-then-create flows have no API yet (`RemoteConfig::register`
> only switches `Login` to `Register`). ARCH §19.1 "property changed" and "time sync" events
> stay open.

> **OPEN:** (implementer note, from review) §8.2 "while the avatar moves or turns": the
> backend counts the avatar as moving when its 3-D speed (vertical included) is at least
> 0.15 m/s (`STANDING_SPEED`) or its heading changed since the last `MoveInput`, so a jump
> in place or a vertical fall is sent at 15 Hz although its gait stays Standing (see the
> next note on the missing ground state).

> **OPEN:** (implementer note) §8.1 / Appendix A: `Command::Move` carries no ground state, so
> the backend never sends gait Airborne. It derives the gait from the horizontal speed and
> the player's `WALK_SPEED`/`RUN_SPEED`, and sets Backward when the velocity points more
> than 90° away from the heading. A ground flag on `Command::Move` (or a new command) would
> close this.

> **OPEN:** (implementer note) Appendix A, `LoginFailed`: the backend logs in once with the
> credentials it was built with. Any `LoginFailed` ends the session (`Disconnect(NO_ERROR)`,
> then `Disconnected { ServerClosed("login failed: CODE[: text]") }`, details in
> `RemoteBackend::failure`); it never retries, so `RATE_LIMITED`'s `retry_after` is only
> reported.

> **OPEN:** (implementer note) Appendix A, `EntityCreate`: the state carries a velocity and
> `EntityCreated` has none, so the backend follows `EntityCreated` with an `EntityMoved` at
> the same position when the velocity is not zero.

> **OPEN:** (implementer note) Appendix A, `LocationReport(REQUESTED)`: surfaced as
> `LocationReport { cause: Requested }`. The client does not snap to it, but
> `Session::player_location` (unchanged existing behaviour) takes it as the player's
> position until the next move.

> **OPEN:** (implementer note) Entity IDs: the server's IDs below 2⁶³ are used as
> `EntityId`s unchanged (so bots and logs can compare them across connections); statics
> streamed from the client's data and any server ID at or above 2⁶³ get local IDs from 2⁶³
> up (`RemoteBackend::server_entity`/`local_entity` translate).

> **OPEN:** (implementer note) §8.6 details: across a snapshot pair that fails the teleport
> test the client shows the earlier state until the later snapshot's time, then the later
> one (no blending). Past the extrapolation limit the position holds and the newest
> velocity is still reported (a sample says it is held). Snapshots are kept for 1 s behind
> the newest (longer at low snapshot rates, see the next note), at most 64, keeping one
> older than the cut so a render time *D* behind the newest still has its bracket. Before
> the first `TimeReply` the render time is based on the newest server timestamp received,
> without latency correction.

> **OPEN:** (implementer note, from review) §8.6 "keeps the snapshots received in the last
> second" conflicts with *D* ≥ 2 / `snapshot_rate` + 1 / `tick_rate` when `snapshot_rate`
> ≤ 2 (§6.2.2 allows 1): at 1 Hz *D* = 2.034 s, so a one-second history never holds the
> bracket a render time needs and every remote entity freezes. `tessera_net` keeps at least
> *D* + 1 / `snapshot_rate` of history (1 s at the default rates). Spec writer: state the
> history as "at least max(1 s, *D* + 1 / `snapshot_rate`)" or similar.

> **OPEN:** (implementer note) §13.3 back-pressure: the client's network thread stops
> reading every stream (control included) while 256 items wait for the main thread, and,
> while the main thread's queue is full and more than 64 items wait, drops movement items.
> When a read chunk decodes several messages and a later one breaks a rule, the earlier
> messages of that chunk are not surfaced (the session ends anyway).

> **OPEN:** (implementer note) §4.5: the backend refuses to write the known-servers file
> into a directory that holds `*.dat`/`*.datx` files, or anywhere below a directory that
> holds a game install's `client_*.dat`/`client_*.datx` archives. A file larger than 1 MiB,
> or one that exists but cannot be read (permissions, I/O error), fails the connection as
> `KEY_UNKNOWN` before the handshake, whatever the first-use policy: a found entry is
> authoritative, so a file the client could not read completely must not lead to trust on
> first use (and nothing is appended to it). Spec writer: confirm, or name a dedicated
> failure for it in §14.2. DNS and socket set-up failures are reported as transport failures, not
> `TLS_FAILURE`; TLS alerts other than `no_application_protocol` are `TLS_FAILURE`.

### 17.3 Implementer notes (`apps/loadtest`, 2026-10-07)

> **OPEN:** (implementer note) §8.6 recommends *D* = 150 ms and also requires
> *D* ≥ 2 / `snapshot_rate` + 1 / `tick_rate`, which is 166.7 ms at the defaults (15 Hz,
> 30 Hz); `tessera-net` follows the inequality. Spec writer: correct the recommended value
> or the inequality (`test-server.md` §13.2 repeats 150 ms).

> **OPEN:** (implementer note) Appendix A maps `WorldEnter` to `Event::WorldEntered`
> without its `instance`, so a client (and the load tool's Yell expectations, §10.1) cannot
> tell which world instance it joined.


<a id="s18"></a>
## 18. Change log

| Date | Change |
|---|---|
| 2026-10-06 | v1.0 draft (spec writer, design). Transport per decision 0030. Awaiting review by someone who did not write it (ROADMAP M7 exit criterion 2) |
| 2026-10-07 | Independent review (M7 exit criterion 2): accepted; non-blocking `OPEN (review)` notes, see [Review](#review). Second round the same day ([R.7](#review-r7)) |
| 2026-10-08 | §1, §7.3, §8.3: default limits follow the `collision.md` §12 controller: *V*ₕ 8.5 m/s (was 1.25 × `RUN_SPEED` = 7.5), *V*ᵤ 10 m/s (unchanged, reasoned anew), *V*_d 120 m/s (was 60); `WALK_SPEED`/`RUN_SPEED` no longer set a limit. No bytes change. Reasoning in `test-server.md` §4.5. Spec writer (design); needs a reviewer's confirmation |
| 2026-10-09 | §8.3 "Server relaxations" (spec writer, design): a server MAY apply a settling window of at most 10 s after an epoch with an unknown height (skipping only the up and down tests) and a descent credit of at most 100 m; the round-2 OPEN at §8.3 is answered. §15 invariant 8 reworded for the relaxations and for unreported rejections; its implementer OPEN answered. This table gains the 2026-10-07 review row (round-2 OPEN at §18 answered). No byte changes; minor version 0. Status Accepted → Revised, awaiting review |
| 2026-10-09 | Review fixes (spec writer, design), answering the 2026-10-09 review: blocking (1), §8.3 item 2 now says the credit is in use only with *S* > 0, and a server without it or with *S* = 0 applies steps 5 and 6 unchanged (*d*_c and *C* drop out); §15 invariant 8's parenthesis matches. Blocking (2), §8.3 item 1 gives the down-allowance update in the window, *A*_d ← max(0, *A*_d − *d*_d); invariant 8 states that no allowance is below 0. Non-blocking (a) the window's examples say "for example" and name a `Named` teleport; (b) "the extent check" removed from item 1 (step 4 has it); (c) invariant 8 names *d*_c. (d) needs no change now. No byte changes; minor version 0. Status: changes requested → Revised, awaiting review |
| 2026-10-10 | §1, §8.3, §15, §17.1 (spec writer, design). §8.3 "Server relaxations": new paragraph "Bounds of the values in force" (a server never applies a window above 10 s or *S* above 100 m; it refuses or clamps; a window of 0 or a reserve that is 0, negative or not a number turns the relaxation off) and new paragraph "Changing the parameters during an epoch" (allowances and *C* capped at once by the new values, no refill at the change; an open window keeps the start of its epoch and takes the new length, 0 or a passed end closes it; a closed window never reopens). §15 invariant 8: with parameters changing within a stretch, the largest values in force bound it. Both implementer notes of §17.1 (2026-10-09 clamping, 2026-10-10 limit change, from the implementations in commits a692dce and e73f00d) answered: the implemented behaviour is adopted. New provenance row. No byte changes; minor version 0. Status Accepted → Revised, awaiting review |
| 2026-10-10 | v1.1, minor version 1 (spec writer, design; decision 0025): §3 compatible flag bits with payload and the frozen `Hello`/`HelloAck` (round-2 OPEN answered), new §3.1 minor table; §5.3 `Equipped` bit 3 `slot`, `LookEntry`, `Look`; looks appended to `LoginOk`, `CharacterCreate`, `CharacterList`, `WorldEnter`, `EntityCreate`, `EntityAppearance`; `AppearanceSet` carries none; §7.4 slot, look and the receiver rule; §13.1 limits; §15 invariants 10–11; §16.8 vectors; Appendix A maps `CreateCharacter`, `LoginOk`, `CharacterList`, `CharacterCreateFailed` per `character-screens.md` §7.1.1 (Q11) and the overlay's `AppearanceSet`. v1.0 bytes unchanged. Status Accepted → Review |
| 2026-10-10 | Independent review of v1.1 (spec reviewer, Clean-Room-Role: review): passed, every item of CLEAN_ROOM.md §5.3; status Review → Accepted. Non-blocking notes (a)–(c) in the review record |

> **Answered (2026-10-09, spec writer; was OPEN, review, non-blocking, round 2):** the
> 2026-10-07 review row is added and "Last updated" is current.

---

<a id="review"></a>
## Review

Independent review for ROADMAP M7 exit criterion 2, 2026-10-07, by a reviewer who did not
write this spec, `crates/protocol`, `tessera_net::remote` or `apps/server`.

| Item | Result |
|---|---|
| Inputs | This spec, [decision 0030](../decisions/0030-tessera-protocol-transport.md), `server-content.md`, `test-server.md`, `docs/LEGAL.md` §11.1. The implementations were read only after the independent codec (step 2) was written and passing against the vectors |
| Blocking findings | **None.** Status set to Accepted |
| Non-blocking findings | `> **OPEN:** (review, non-blocking)` notes at §3 (consequence of breaking a field's Rule), §5.2 (`TEXT0` sent by clients), §7.2 (65,536 wording, depth of empty containers), §7.3 (`EMOTE` `started`, field kinds), §8.6 (*D* and history), §9.1 (block index at 40,800 m), §11.2 (*t*₁ = *t*₂, rounding of θ), §15 (invariants 2 and 8), §16.3 (`EntityCreate` vector carries an owner-only property). Second round ([R.7](#review-r7)), marked "round 2": §0 (music sync), §3 and §6.2.1 (`Hello` cannot grow), §4.5 (trust on first use against the timeouts), §6.2.18 (`Ping` slack, stray `Pong`), §7.2 (nested array shapes), §8.3 (settling grace), §8.6 (snap distance), §8.7 (walking out of an interior), §11.1 (tick schedule, `server_time`), §12.2 (idle and `Ping`), §13.2 (warning flood), §13.3 (client memory bound, chat backlog), §14.7 (`NOT_IN_WORLD`), §18 (change log) |
| TRD v1 (`transport-design.md`) | Not reviewed here: decision 0030 point 5 keeps TRD v1 out of this protocol (frozen in `tessera_net::transport`, not used), so its Reviewer row stays open |

### R.1 Spec read as a second implementer

Every byte layout of §5–§7 was implemented from the text alone. All layouts, limits, frame
and datagram rules, the handshake vectors and the state tables were complete enough to do
that without guessing. The gaps found are the notes listed above; none of them changes the
bytes two conformant peers exchange. The largest one is §3's: about twenty field rules
whose breach the spec does not classify, which the implementation treats as fatal.

### R.2 Independent codec (`crates/protocol/tests/spec_conformance.rs`)

A second codec of about 1,500 lines (frame layer, datagram layer, the 36 message types,
tagged values, SPKI and fingerprint text, NTP formulas), written from this spec and sharing
no code with `crates/protocol`. Compared through `tessera_protocol`'s public API only.

| Check | Result |
|---|---|
| §16.2–§16.5: the 42 frame and datagram vectors, every message type at least once | Independent encoder byte-identical to the spec; stated sizes match; decoded back to the stated values |
| Same values through `tessera_protocol` (`frame_to_vec`, `encode_datagram`; `FrameDecoder` fed one byte at a time, `decode_datagram`) | Byte-identical; decoded values equal |
| §16.6: the 16 tagged values, None allowed only where stated | Identical both ways in both codecs |
| §16.1: SPKI DER, SHA-256 fingerprint text (computed with `sha2`), ALPN, pinned connect string | Match; `Fingerprint` and `ConnectString` agree |
| §16.5 worked time-sync example; §11.1 tick numbering | θ = 4,000,000 µs, δ = 40,000 µs; tick 150 starts at 5,000,000 µs |
| 124 hand-written malformed inputs (all of §16.7, frame lengths, unknown types, wrong channel and direction, stream kinds, primitives, text-class maxima, structures, enumerated fields, property lists and values, datagram sizes and lengths) | Rejected by both codecs |
| 23 boundary inputs the spec allows (limits exactly at their maximum, unknown codes and capability bits, unknown property IDs, depth 4, control characters in text) | Accepted by both; control characters in server text become spaces in both |
| §4.2 fingerprint text rules (2 valid forms, 10 invalid ones) | Independent parser and `Fingerprint::parse` agree |
| §4.4 connect strings (4 valid, 15 invalid: port, scheme, path, user information, fragment, unknown parameter, whitespace in the pin, IPv6 without brackets, empty host; host lowercased, IPv6 kept in brackets in the server key) | `ConnectString::parse` follows every rule |

### R.3 Implementations checked against the spec

- **Limits** (`crates/protocol/src/limits.rs`): every constant matches §2.1, §5, §6, §8.3,
  §9.4, §10.2, §11.2, §12 and §13.1.
- **Hostile input:** frame length checked from the 4-byte header before buffering; every
  count checked against its limit and the remaining bytes before allocating; value
  recursion bounded by the depth rule; datagrams above 1,200 bytes fatal; input ignored
  while Closing or Closed. Fuzz targets exist for frames, datagrams, messages, the handshake
  and the server's connection handling.
- **Transport** (`crates/protocol/src/transport.rs`, feature `transport`): QUIC version 1
  only, TLS 1.3 only, ALPN `tessera/1`, no 0-RTT, no resumption, no session tickets, the
  §2.1 stream limits, windows, idle timeout, keep-alive and initial MTU; the client verifier
  takes the first certificate, requires Ed25519, compares the fingerprint, and verifies
  CertificateVerify with `ed25519` only. Its 5 loopback tests pass.
- **Server session** (`server.rs`): the §12.2 states and timeouts, one outstanding request
  of each kind, three login failures then `AUTH_FAILED`, capability checks, text-class
  failures answered with the request's own code, control flooding at 250 messages in 5 s,
  Ping 1/s, `TimeRequest` 4/s, `MoveInput` 30/s.
- **Movement:** `MovementValidator` follows §8.3 steps 1–7 in order (`tessera-server`'s settling
  grace relaxes them; see R.7) (allowances by server
  time, extent, region, corrections at most 4/s). `MoveScheduler` follows §8.2 (15 Hz while
  moving, at once on start, stop and gait change, 3 redundant rest sends, 1 Hz heartbeat,
  never two in one tick).
- **Interest management** (`apps/server/src/sim.rs`): candidates share instance and region;
  enter at ≤ *R*, leave at > *R* + 1; destroys first and never deferred; creates nearest
  first, at most 16 per tick and min(`max_entities`, 1,024) at once; 64 KiB per tick for
  updates and creates; backlog pause at 1 MiB, resume below 256 KiB, `SLOW_CONSUMER` at 4 MiB
  or 30 s; `InterestReset` before `LocationReport(TELEPORTED)` on every region change and on
  jumps beyond *R* + 1; properties changed in the creating tick folded into `EntityCreate`;
  owner-only speeds sent only to the owner. Snapshots: changed, 3 at rest, 1 s refresh;
  overdue first, then nearest; at most 16 × 29 entries; never the receiving player.
  Minor: the owner's own `EntityUpdate`s are written outside the per-tick byte budget (they
  are a few bytes).
- **Time sync** (`time.rs`, `tessera_net::remote`): §11.2 formulas in 128-bit arithmetic,
  replies with δ < 0 or unknown, repeated or expired *t*₀ dropped, best of the last 8 by
  δ, step above 100 ms, slew of at most 5 ms per second applied every 30 Hz tick so the
  estimate never runs backwards; the server answers at most 4 per second with the tick in
  progress at *t*₂.
- **Chat:** Say and Emote within 40 m in the same instance and region, Yell to the instance
  (region optional by configuration), Tell by the server's name rule; echo with `ECHO` and
  the speaker's own entity; `sender_entity` 0 when the speaker is not created on the
  receiving connection; `peer` set as §10.3 says; the 5-token, 2 s bucket and 20 rejections
  in 60 s; `ChatRejected` kept in `ChatSend` order (`apps/server/src/conn.rs` holds
  session-made rejections behind earlier sends). The session's bucket and the simulation's
  configurable bucket both apply; that is already an open item of `test-server.md`.
- **Client receive rules** (`client.rs`): epoch arithmetic, `InterestReset` before a region
  change, `max_entities`, `MotionBatch` staleness by entity, unknown IDs dropped, the §13.2
  stale cases dropped and counted.

### R.4 Server content and LEGAL.md §11.1 against what `apps/server` ships

- `apps/server/Cargo.toml` depends on no crate that decodes game data (no `tessera-dat`,
  `tessera-formats`); a comment states the rule. The CI check that `server-content.md` E1
  recommends (SHOULD) does not exist yet.
- No code path opens a game archive. `datadir.rs` only recognises `client_*.dat` and
  `*.datx` names in order to refuse data directories and content files next to them (E2).
- `apps/server/content/default-world.toml` matches `server-content.md` §6.2 item by item:
  neutral title and message of the day with the disclaimer, the two spawns `start` (region
  1, block (126, 140), offset (80, 20, 440), ground) and `void` (region 0, block (127, 127)),
  three "Wanderer {n}" actors copying the first player's appearance, clock 0 at rate 1, no
  opaque references, SPDX header. It is built in with `include_str!` (§6.1).
- LEGAL.md §11.1 rules 1–7 agree with `server-content.md` and with the shipped server.
- A config unit test uses "The Shire" as a server name; it is a negative fixture for the
  word check, which `server-content.md` §9.1 allows.
- Documentation nit for the owner of `server-content.md`: §3.2 cites
  `tessera-protocol.md` §9.3 for movement validation, which is §8.3.

### R.5 Implementer notes of §17

The readings recorded in §17.1–§17.3 were checked against the text. All are conservative
and consistent with the spec; those that need spec wording are taken up in the review notes
above (§3, §5.2, §7.2, §7.3, §8.6, §11.2, §15). The `tessera-net` API notes (missing
`Event`/`Command` variants, the unsurfaced `WorldEnter.instance`, the ground flag for
`Command::Move`) concern the backend's API, not the wire. The §17.2 known-servers note
(unreadable file → `KEY_UNKNOWN`) is a safe choice; §14.2 may name it.

### R.6 Not covered

Interoperability with a second QUIC stack was not run (the comparison is at the
application-byte level, which decision 0030 makes the whole of this spec's surface). The
interactive trust-on-first-use prompt, soak and performance figures (`docs/perf/m7-soak.md`)
were not part of this review.

<a id="review-r7"></a>
### R.7 Second round (2026-10-07)

A second pass by the same reviewer over 18 findings raised against the spec and its
implementations. Each was checked against the text and, where it concerns behaviour, against
the code or through `tessera_protocol`'s public API. Two premises are now pinned by tests in
`crates/protocol/tests/spec_conformance.rs` (they check the spec *as written* and are
expected to change when the note is settled):
`open_review_anchor_outlives_the_interior` and
`open_review_nested_array_shapes_follow_the_literal_rule`. The §3 `Hello` premise is
already pinned by the §16.7 negative vector in `malformed_inputs_rejected_by_both`.

| # | Finding | Verdict | Where it went |
|---|---|---|---|
| 1 | Anchor pins the interest block for up to 8 blocks after walking out of an interior | Confirmed (test) | OPEN at §8.7 |
| 2 | `tessera-server`'s settling grace relaxes §8.3 (invariant 8) | Confirmed in code (`sim.rs` `GRACE_VERTICAL`, default 3 s) | OPEN at §8.3; the server change (default 0, or a spec'd window) is for `apps/server` once decided |
| 3 | Tick schedule drifts 10 µs/s; `server_time` is the wake time | Confirmed in code (`run()` adds 33,333 µs; `tick()` recomputes `tick_at(now)`) | Spec wording: OPEN at §11.1. The schedule itself breaks §11.1 as written and is an `apps/server` fix (derive the deadline from `tick_start(k + 1)`) |
| 4 | CharacterSelect idle never fires because `Ping` resets it | Confirmed in code (`ServerSession::on_control` moves the deadline for every control message) | OPEN at §12.2; `ServerSession` follows once worded |
| 5 | §8.6 snap distance: horizontal or 3-D | Confirmed (text) | OPEN at §8.6; `tessera-net` `SnapshotBuffer` follows |
| 6 | `server-content.md` §6.2 calls 440 m a terrain sample | Confirmed (text): `test-server.md` §1 and §5.7 give 438.7 m observed, 440 m set 1.3 m above | Not this spec's text. Documentation nit for the owner of `server-content.md`, like the R.4 one |
| 7 | Client input bound counts items, not bytes | Confirmed (`EVENT_CHANNEL_CAPACITY` 1,024, `PENDING_PAUSE` 256; a decoded `Value` is 32 bytes against 2 on the wire for a Bool element) | OPEN at §13.3; the byte bound and decode-time skipping are for `tessera-net` and `ClientSession` |
| 8 | Unlimited warning lines for the §13.2 server-bug cases | Confirmed (four `tracing::warn!` in `client.rs`) | OPEN at §13.2; rate limiting is for `ClientSession` |
| 9 | Chat-stream backlog unbounded in this spec | Confirmed (text; the bound exists only in `test-server.md` §10.1) | OPEN at §13.3 |
| 10 | Interactive trust on first use cannot finish within the timeouts | Confirmed (text: §4.5 against §12.1, §12.2) | OPEN at §4.5. This supersedes R.6's "not part of this review" for the flow's wording |
| 11 | `Hello` cannot take an appended field | Confirmed (text, and the §16.7 vector requires the rejection) | OPEN at §3 and §6.2.1 |
| 12 | `tessera-protocol` binds sockets behind feature `transport`, against decision 0030 point 4; point 4 cites §9.5 for slow consumers | Confirmed (`transport.rs` binds `UdpSocket`; 0030 point 4 says no I/O; the rules are in §9.4 and §13.3). The status table's "no I/O" is accurate for the default features only | Not this spec's text: decision 0030 needs an amendment (or the module moves out of the crate). For the owner of decision 0030 |
| 13 | `ServerParams` not validated; codec cannot carry a second minor | Confirmed in code (`negotiate_minor` uses `max_minor` as given; `ServerParams { max_minor: 7 }` advertises 7; bad rates fail per connection with `INTERNAL_ERROR`) | Implementation API, no spec change. For `crates/protocol`: a `ServerParams::validate` at start-up, and minor-aware decoders before the first 1.1 change |
| 14 | Receiver `Ping` limit has no slack; stray `Pong` fatal by §6.1 | Confirmed (text; both sessions use a 1-per-second window for the peer) | OPEN at §6.2.18 |
| 15 | `ChatRejected` `NOT_IN_WORLD` has no trigger | Confirmed (text; used by `sim.rs` for a race) | OPEN at §14.7 |
| 16 | Change log and "Last updated" contradict the status | Confirmed | The reviewer does not own the change log or that row: OPEN at §18 for the spec writer |
| 17 | No message for the M6 networked music start | Confirmed (text) | OPEN at §0 |
| 18 | "Same tag" for nested containers | Confirmed (test: both codecs accept [[Int32 1], [Bool true]]) | OPEN at §7.2; the agreed case goes into the boundary or malformed lists afterwards |

None of the round-2 findings changes a byte two conformant v1.0 peers exchange, so the status
stays Accepted. The ones marked without "non-blocking" (§4.5, §8.3, §8.7, §13.3) should be
settled before a second, independent client or server is written against this spec.

TRD v1 (`transport-design.md`): decision 0030 point 5 still keeps it out of this protocol,
so its Reviewer row stays open, as in the first round.
