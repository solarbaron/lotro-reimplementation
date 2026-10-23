<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Official play: entry points, ban-risk consent, account form, world list and queue

| | |
|---|---|
| **Status** | **Accepted** (2026-10-23, independent review of revision 4, see the review record). Before: Revised (revision 4, 2026-10-23, spec writer: answers six implementer and reviewer OPENs as Tessera rules, see the change log). Before: Accepted (2026-10-23, independent review of revision 3, see the review record). Before: Revised (revision 3, 2026-10-23, spec writer: answers the ten implementer OPENs of the clean implementation and its review and audit as Tessera rules, see the change log). Before: Accepted (2026-10-23, independent re-review of revision 2, see the review record). Before: Review (revision 2, 2026-10-23, spec writer: answers review 1). Before: Review (revision 1, 2026-10-23, first complete version). Before: Draft (2026-10-23). |
| **Last updated** | 2026-10-23 |
| **Author role** | Spec writer. A Tessera **design** spec, like `tessera-protocol.md` and `ui-scaling.md`: no observation of the original client, its launcher, the official service or the install went into it, and no dirty-room material was opened (§1) |
| **Reviewer** | spec-review:spec-m8-entry:1 (independent session; wrote no part of this spec), 2026-10-23 |
| **Review record** | **Review 4, 2026-10-23** (spec-review:spec-m8-entry:1 (independent session), review of revision 4, the uncommitted diff against main; no dirty material, no M8 protocol spec, no network, no build): **passed**; status Revised → Accepted. Checked `CLEAN_ROOM.md` §5.3 on every changed part (§4.1 acceptance line, §4.6 "Closing the window" table, §5.6 key step 1, Withdraw and the 128-byte `/etc/machine-id` bound, §6.3 "Release builds", OE-T8(b), OE-T11, OE-T18, OE-T22–OE-T24): each answer is a Design rule with its reason; no code or pseudo-code; no official-protocol fact, hostname, endpoint, key or account data; the bounds and expected results are concrete and testable; the Cargo feature-unification reason for the release rule is general build-tool behaviour, consistent with the workspace's resolver 3. Decision 0034 §1 item 4 still holds as testable rules: a window close never accepts (Decline on the warning, one step back elsewhere; OE-T23); an unrepaired key only shortens an acceptance to its run and never skips the warning (OE-T24, invariant 11); the release rule keeps the record-building helpers out of shipped binaries, so no externally built record path exists in them (OE-T8(e), OE-T22); the other rules are unchanged from review 3. Links and anchors resolve. Note (not blocking): OE-T5 needs a FIFO and OE-T8(b) now bars process spawning in test code too, while the standard library cannot make a FIFO; the implementer needs a dev-dependency or safe wrapper for it, or skips that case where none is available. **Review 3, 2026-10-23** (spec-review:spec-m8-entry:1 (independent session), review of revision 3, the uncommitted diff against main; no dirty material, no M8 protocol spec, no network): **passed**; status Revised → Accepted. Checked `CLEAN_ROOM.md` §5.3 on every changed part (§4.1, §4.2, §4.5, new §4.6, §5.2–§5.6, §5.8, §6.1, §6.3, §7.1–§7.3, §8.3, §8.6, §8.7, OE-T5, OE-T6, OE-T8, OE-T10, OE-T11, OE-T15–OE-T21, invariants 9–11): each answered OPEN is a Design rule with its reason; no code or pseudo-code; no official-protocol fact, hostname, endpoint, key or account data (`/etc/machine-id` is an OS file, not a service fact); the canonical record form is complete (key order, line format, integer and string forms, time format) and testable byte for byte; tests and invariants concrete. Decision 0034 §1 item 4 still holds as testable rules: first use and text change (§5.1, §5.3; OE-T1, OE-T3, OE-T4); only explicit confirmation on Tessera's own self-painted screen (§4.6, §5.4; Decline at any time only refuses, Continue still needs the box and the delay; OE-T7, OE-T11); no flag, environment, external file, plugin, script or server can accept (§4.2 wider conflicts, §5.6 canonical form and key never overwritten, Windows fails closed, §6.3 scan over `crates/compat/src` too, test-support feature kept out of release builds; OE-T5, OE-T6, OE-T8(e), OE-T15–OE-T17, OE-T21); no connection before acceptance (§6.1 now also bars `tessera-net` transport/remote and process spawning; OE-T8). Links and anchors resolve. Pure formatting fix by the reviewer: in §8.7 the sentence "Leaving the character screens back to the start ends the session …" had run into the placeholder string table's row; it is again its own paragraph (no wording changed). Notes (not blocking): OE-T11 still says "no acceptance from the first six", but it now lists nine cases, and only the keyboard path should accept; §8.7 lets closing the window on the placeholder return to the account form, while elsewhere closing the window ends the client or declines; say which is meant. **Review 2, 2026-10-23** (spec-review:spec-m8-entry:1 (independent session), re-review of revision 2; no dirty material, no M8 protocol spec, no network): **passed**; status Review → Accepted. Every point of review 1 is answered: blocking (1) §4.5, §4.2 (`--load-plugin` conflicts), §5.7 Plugins row, OE-T6(a), OE-T7(d), invariant 9; (2) §5.4 rules 2, 5, 6 (no shortcut accepts; keyboard confirmation of the focused Continue counts), OE-T11; (3) §5.6, §5.7 configuration row and §5.8 now state the host-name fallback, cloned images and the Windows default-permission limit without overclaiming. Non-blocking (a)–(h) answered in §6.1, OE-T8(b), OE4, OE5, §4.4, §8.2, §5.2/§5.3, §5.6, §5.4 rule 1 and §2, OE-T6(a), §6.1/§7.3. Re-ran `CLEAN_ROOM.md` §5.3 on §2, §4.2, §4.4, §4.5, §5.2–§5.8, §6.1, §7.3, §8.2, §10 and §11: Design/Project facts only, provenance OE5 re-checked against `apps/client/src/plugins.rs` and `crates/compat`; no code or pseudo-code; no official-protocol fact, hostname, endpoint, key or account data; digest input fully defined; tests and invariants concrete; links and anchors resolve. Decision 0034 §1 item 4: each rule met by a testable rule (§5.1/§5.3 with OE-T1, OE-T3, OE-T4; §5.4/§5.7 with OE-T6, OE-T7, OE-T11; §6 with OE-T8, OE-T9). Note (not blocking): OE-T11 "no acceptance from the first six" leaves out the seventh case, window close, which declines anyway. **Review 1, 2026-10-23** (spec-review:spec-m8-entry:1 (independent session), wrote no part of it; the uncommitted worktree diff, opened no dirty material and no M8 protocol spec): **changes requested**; status stays Review. Checked: `CLEAN_ROOM.md` §5.3 every item (functional facts and Design rules only; no code or pseudo-code; no copied prose; no game text, keys, hostnames, endpoints or account data; provenance rows OE1–OE11 marked Design/Project and spot-checked against decision 0034/0041, `crates/net`, `crates/compat`, `crates/settings`, `apps/client`; digest and record byte strings complete; invariants testable; login unknowns are OPENs that state no official fact; nothing outside decision 0034 §2); decision 0034 §1 item 4 rule by rule; the Account group against `Backend`, `Session::send`/`accept`, `SessionState`, `DisconnectReason`, `sanitize_text` and `name_tests`; credentials handling; tests; §5.8; links and anchors (all resolve). **Blocking:** (1) §4.2, §5.4, §5.7 "Plugins": the claim that no plugin apartment exists while the start, warning, form and login screens show is false for the windowed client today: `--load-plugin` queues plugin loads at runtime start-up (`apps/client/src/plugins.rs` `Plugins::start`), independent of any login; add `--load-plugin` (and decide on `--plugins`) to the §4.2 conflict list, or require the official flow not to start the plugin host before §8.7, and cover it in OE-T6(a)/OE-T7; §5.4's "cannot cover" rests on it. (2) §5.4 rule 5 "Keys may only refuse" contradicts rule 3 (Tab to Continue, then Enter or Space accepts); reword rule 5 to what is meant (no shortcut key accepts; Escape and window close decline). (3) §5.8 first guarantee and §5.7 "Configuration files" row overclaim: "no ready-made file can be handed around" fails when the binding falls back to the host name (no machine identifier, §5.6) or on cloned images that share machine identifier, user ID and path, where a key-and-record pair made elsewhere validates; and "other OS users cannot write the record" holds on Windows only by default (no check is made there). Qualify both statements. **Non-blocking:** (a) `crates/compat/src/session.rs` already binds `UdpSocket` with no proof; §6.1 should say that transport moves behind the network-access type, and OE-T8(b) should list the scanned type names (address types such as `SocketAddr` are not sockets). (b) OE5 and §4.4 quote "protocol not built"; `CompatError::ProtocolNotBuilt` displays "official-server session connector is not wired into the client yet" (§4.4's text is then a change; say so); OE4's quotation of `Command::Connect` is a paraphrase. (c) §8.2: `Session` logs dropped invalid events with their `Debug` form at `warn`, which for Account events puts world names and details at `warn` against §7.3, and it has no rate bound today; state the logging rule for the group. A name "of more than 64 bytes after `sanitize_text`" cannot occur. (d) §5.3: say how the `<br><br>` paragraph breaks enter the digest input. (e) §5.6: "a lock" is undefined (no locking is specified). (f) §5.4/§6.3: name how device input is told from synthetic input (an origin on input events) so OE-T2, OE-T7(c) and OE-T11 are implementable. (g) OE-T6(a) "every option combination": each option paired with `--official`. (h) `CompatConfig` derives `Clone` today; carrying the proof and credentials (§4.3, §7.3) removes that. |
| **Implements into** | `crates/compat` (`tessera-compat`: the `consent` module of §5.6 and §6, the availability answer of §4.4, the consent proof on every network entry point, §6), `crates/net` (`tessera-net`: the Account group of §8.1 and its session rules, §8.2), `apps/client` (`tessera-client`: the start screen and `--official` of §4, the warning screen of §5, the account form of §7, the login screens of §8, the flow of §9), `fuzz/` (§10.3) |
| **Companion documents** | [Decision 0034](../decisions/0034-official-server-compatibility.md) §1 item 4 (binding; §1 item 2 and §3 items 1–3), [decision 0041](../decisions/0041-incident-m8-spec-contamination.md) (why this spec uses no M8 protocol spec), [`ARCHITECTURE.md` §19](../ARCHITECTURE.md#net-backend) (the `Backend` trait, sessions, backends), [`character-screens.md`](character-screens.md) (the screens after the login, §3 and §6), [`scenario-feed.md` §9](scenario-feed.md#s9) (the event and command groups this spec adds one to), [`ui-widgets.md` §18](ui-widgets.md#s18) (engine dialogs, which this spec does **not** use), [decision 0017](../decisions/0017-extended-scripting.md) §5.3 (the prompt-integrity rules this spec borrows), [decision 0022](../decisions/0022-bundled-ui-font.md) (the bundled UI font) |

---

<a id="s0"></a>
## 0. Purpose

Decision 0034 puts official-server play in scope. Its §1 item 4 is binding: before any
connection to an official server is possible, the client shows a warning about the account
risk of using a third-party client; the warning is shown on first use and again whenever its
text changes; only the user's explicit confirmation in the client's own interface accepts
it, and no command-line flag, environment variable, configuration file written by anything
else, plugin, script or server can accept or skip it; until it is accepted the client makes
no official connection.

The official protocol specs are under incident 0041 and are not Accepted, so the
connection itself cannot be built yet. This spec designs everything **around** it, so that it
can be built and tested now, without a network and without any fact of the official login:

- how the user chooses official play: a start-screen choice and a command-line option that
  only **selects** (§4);
- the ban-risk warning: its text as Tessera's own string-table entries, when it is shown, how
  a change of its text is detected, the screen's input rules, accepting, declining and
  withdrawing, and the acceptance record and what it can and cannot guarantee (§5);
- the gate: the one code path that makes an official connection impossible before
  acceptance, and how it is tested (§6);
- the account form: fields, masking, memory lifetime, and the rule that the credentials go
  to the compatibility backend and nowhere else, never to disk or logs (§7);
- the world list, the queue display, progress, errors and retries, as abstract events and
  commands of a new `tessera_net` group, **Account** (§8);
- the flow from the start screen to the character screens (§9), the tests (§10) and
  invariants (§11).

Every point that needs a fact of the official login flow is an `OPEN` that names the fact
needed without stating it (§12). The compatibility backend's own mapping from the official
wire to the Account group belongs to the M8 protocol specs once they are Accepted again.

**Out of scope:** the official wire protocol, login sequence, endpoints and keys (the M8
protocol specs, decision 0034 §2); what happens after the character list arrives
(`character-screens.md`, the gameplay specs); Tessera's own test server's login
(`tessera-protocol.md` §4, unchanged); extended scripts on official servers (decision 0034 §4,
M9).

---

<a id="s1"></a>
## 1. Provenance

Legend: **Design** = decided by this spec (normative, no observation behind it).
**Project** = a rule or fact of a Tessera decision, spec or of the code on main, restated here;
the source stays normative where it is cited.

| # | Facts | Source | Mark |
|---|---|---|---|
| — | Everything not listed below: the start screen and `--official`, the warning's text and string IDs, its digest and revision, the screen's input rules, the acceptance record (location, format, binding, authentication code, failure handling), the consent proof and the network-entry rule, the account form, the Account group and its session rules, the presentation of worlds, queue, progress and errors, retry rules, the flow, tests and invariants | This spec, 2026-10-23, for decision 0034 §1 item 4 | **Design** |
| OE1 | The four rules of the ban-risk consent (before any connection; first use and on every text change; only an explicit confirmation in the client's own interface, nothing else can accept or skip it; no official connection until accepted) | Decision 0034 §1 item 4 (owner, 2026-10-22) | **Project** |
| OE2 | Only the user's own credentials, typed by the user; no bypass of any authentication, subscription or access check; M8 stops where the service needs an excluded mechanism; no endpoint, hostname, key or account data in the repository; endpoints and key material read at run time from the user's install; CI never connects to an official server | Decision 0034 §1 item 2, §3 items 1–3 | **Project** |
| OE3 | The M8 protocol specs are not Accepted; building from them is paused | Decision 0041, "Actions taken" | **Project** |
| OE4 | `Session` and `Backend` (non-blocking `submit` and `poll`), `SessionState` (`Idle`, `Connecting`, `Connected`, `EnteringWorld`, `InWorld`, `Closing`, `Closed`), `Command::Connect` (its documentation: "Credentials, if any, are part of the backend's own construction, never of this neutral model."), `Event::Connected` carrying the character list, `Event::Disconnected` with `DisconnectReason` (`ClientRequested`, `ServerClosed`, `TimedOut`, `TransportFailed`), the event and command groups and the name-uniqueness test `session::name_tests`, `sanitize_text` | `ARCHITECTURE.md` §19.1; `crates/net/src/session.rs`, `model.rs` (read 2026-10-23) | **Project** |
| OE5 | `tessera-compat` has a `CompatBackend` over a `Connector`; the default `PlaceholderConnector` always fails with the error `CompatError::ProtocolNotBuilt` (its text today: "official-server session connector is not wired into the client yet") and opens no socket; `Connect` on it gives one `Disconnected` with `TransportFailed` carrying that text; `CompatConfig` derives `Clone` and `Default`; the crate's session module opens a standard-library UDP socket itself; the client does not link `tessera-compat` yet; the client starts its plugin host at start-up (`Plugins::start`) and queues the `--load-plugin` loads there | `crates/compat/src/backend.rs`, `lib.rs`, `session.rs` (read or searched 2026-10-23); `apps/client/src/plugins.rs`, `runtime.rs` (read 2026-10-23); `apps/client` (no reference, 2026-10-23) | **Project** |
| OE6 | Precedent for credentials: the test-server password type prints redacted and is overwritten (best effort) when dropped; the backend takes it, keeps no copy and wipes unused credentials; one login attempt per session | `ARCHITECTURE.md` §19.2 (configuration); `tessera-protocol.md` Appendix A (implementer note on `LoginFailed`); `crates/net/src/remote/config.rs` | **Project** |
| OE7 | Precedent for a prompt that cannot be spoofed: Tessera's own strings, an input delay, a press that must start after the delay, keys that may only refuse, no module that synthesises UI input | Decision 0017 §5.3 | **Project** (borrowed; decision 0017 binds only extended scripts) |
| OE8 | Tessera's settings file is TOML in the platform config directory (`…/tessera/settings.toml`), or where `TESSERA_SETTINGS_FILE` points; unknown keys are kept; writes are atomic | `crates/settings` rustdoc; `ARCHITECTURE.md` §17 | **Project** |
| OE9 | The character screens follow `Connected`; the pre-world flow is opened by `--character-select` today; plugins autoload when the session reports the login | `character-screens.md` §3, §6 (implementer notes); `plugin-runtime.md` §8.2 | **Project** |
| OE10 | Engine dialog boxes are built from descriptions that game code hands the engine | `ui-widgets.md` §18 | **Project** |
| OE11 | Tessera bundles its own UI font | Decision 0022 | **Project** |

No fact about the official client, its launcher, the official service or its login flow is
stated anywhere in this spec.

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning |
|---|---|
| **Official play** | A session whose backend is the compatibility backend (`tessera-compat`) |
| **Warning** | The ban-risk warning of §5, Tessera's own text |
| **Warning digest** | The SHA-256 digest of the warning's shown text (§5.3) |
| **Acceptance record** | The two files of §5.6 that keep an acceptance across runs |
| **Consent proof** | The in-memory value of §6.1 that only a valid acceptance yields, and without which no official network operation exists |
| **Start screen** | Tessera's own first screen of §4.1 |
| **Account form** | The screen of §7 where the user types the account name and password |
| **Login screens** | The progress, world-list and queue screens of §8 |
| **Device input** | Key, pointer and text events that the windowing system delivers to the client's window, as they arrive in the client's window-event handler (§5.4 rule 1) |
| **Synthetic input** | Any input the client generates itself: the windowed input script of `--scenario-input`, `--command` lines, `--ui-script`, the command dispatcher, plugin calls, test drivers |

---

<a id="s3"></a>
## 3. Requirements and where they are met

| Rule (decision 0034 §1 item 4) | Met by |
|---|---|
| A warning about the account risk of a third-party client is shown before any official connection is possible | §5.2 (the text), §6 (no network operation without a consent proof), §9 (the flow) |
| Shown on first use | §5.1 (no valid record → shown) |
| Shown again whenever its text changes | §5.3 (digest of the shown text; any change gives a new digest; the record's digest must equal it) |
| Only the user's explicit confirmation in the client's own interface accepts it | §5.4 (Tessera-drawn screen, device input only, delay, check box and button), §5.5 |
| No command-line flag, environment variable, configuration file written by anything else, plugin, script or server can accept or skip it | §5.7 (each source, one by one), §5.6 (the record's authentication code and binding), §5.8 (what a local file can and cannot guarantee) |
| Until accepted, no official connection | §6 (the consent proof on every network entry point of `tessera-compat`; the client's single construction site), §10 tests OE-T1, OE-T8, OE-T9 |

Decision 0034 §1 item 2 and §3 items 1–3 are met by §7 (own credentials, typed by the user,
handed only to the backend) and §8.6 (no workaround when the service needs a step Tessera does
not support).

---

<a id="s4"></a>
## 4. Choosing official play

<a id="s4-1"></a>
### 4.1 The start screen

A new Tessera screen, painted by Tessera's own code with the bundled font of decision 0022 and
Tessera's neutral art; it is not built from `tessera-ui` elements and uses no install layout
(§4.6). It holds, top to bottom:

| Control | Label (Tessera string, English) | Action |
|---|---|---|
| Button | "Explore offline" | Starts the offline world walker as the client does today without a mode option (sandbox backend). There is no way back from the walker to the start screen: quitting the walker ends the client (§4.5) |
| Button | "Official servers" | Starts the official flow (§9, step 1) |
| Line under it, shown only while a valid acceptance exists (a record that passes §5.6, or the run's in-memory acceptance, §5.5) | With a stored record: "You accepted the official-server warning on ⟨date⟩."; with an acceptance for this run only: the `official.consent.saved_failed` text (Unix-like systems, the write failed, §5.6) or the `official.consent.not_stored` text (Windows); in every case a link-styled button "Withdraw acceptance" | §5.5 "Withdrawing" |
| Button | "Quit" | Ends the client (exit code 0) |

Escape on the start screen is Quit. Tessera's test server stays a command-line choice
(`--server`) in this revision; the start screen does not offer it.

The start screen opens when the client starts windowed with `--start-screen`, or with
`--official` (§4.2).

> **OPEN (owner):** whether the start screen becomes the default for a windowed start without
> a mode option (today that start goes straight to the offline walker), and whether it should
> offer Tessera's test server with a connect-string field.

<a id="s4-2"></a>
### 4.2 The command-line option `--official`

`--official` **only selects**: the client opens the start screen with "Official servers"
chosen, which runs §9 from step 1 at once. It never accepts or skips the warning, never fills
the account form, and takes no value.

- **Conflicts** (a usage error, exit code 2, before any window exists): `--official` cannot be
  combined with `--server`, `--scenario`, `--scenario-input`, `--headless`, `--smoke`,
  `--showcase`, `--walk-path`, `--fly-path`, `--ui-script`, `--command`, `--load-plugin`,
  `--no-data`, `--account`, `--character`, `--screenshot`, `--start-screen`, or any other option
  that makes the client run a headless task (every option of the client's headless-task group,
  whatever it is named). Official play needs a window (the warning and the
  form exist only there), the user's install, no synthetic input and no plugin loaded before the
  login (§4.5, §5.7). `--plugins` (which only names the plugin folder) is allowed.
- **No official credential or consent option exists.** No option, and no environment
  variable, names an account, a password, a world, a consent state or the location of the
  acceptance record. `TESSERA_PASSWORD` (the test server's password variable) is never read in
  official play.
- `--start-screen` opens the start screen with nothing chosen. It conflicts (usage error,
  exit code 2) with `--official`, `--server`, `--scenario`, `--headless`, `--smoke`,
  `--screenshot`, `--walk-path` and `--ui-script`.

> **OPEN (implementer, 2026-10-23):** which options conflict with `--start-screen`, and whether `--official`
> must also refuse `--screenshot` and the other options that dispatch a headless run.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule, as the two lists above say: `--start-screen`
> conflicts with `--official`, `--server`, `--scenario`, `--headless`, `--smoke`, `--screenshot`,
> `--walk-path` and `--ui-script`; `--official` also conflicts with `--screenshot` and with every
> option that dispatches a headless run. A start-screen run is always an interactive windowed
> run.

<a id="s4-3"></a>
### 4.3 Mapping to the backend

Official play maps to `tessera_compat::CompatBackend` behind the `Backend` trait, wrapped in a
`Session` like every other backend (`ARCHITECTURE.md` §19.1). The client constructs it in one
place only: the official-entry module of `apps/client` (§6.2), after §9 step 4. Its
configuration carries the consent proof (§6.1) and the credentials (§7.3); `Command::Connect`
starts the login (§8). Nothing above `tessera-net` sees which backend runs, except the
official-entry module, which also draws the login screens of §8 from the Account group.

<a id="s4-4"></a>
### 4.4 While the backend is the placeholder

`tessera-compat` answers one question without a network and without a consent proof:
**availability**, one of `Available` (a live connector is built into this binary) or
`NotBuilt` (only the placeholder connector exists). While `PlaceholderConnector` is the only
connector, the answer is `NotBuilt`.

- The official flow asks for availability at §9 step 4, **after** the warning (so the gate is
  exercised and visible now, and wiring a live connector later cannot reorder it). With
  `NotBuilt` the client shows Tessera's message "Playing on the official servers is not
  available in this version of Tessera yet." with an OK button that returns to the start
  screen. The account form is not shown and no backend is constructed.
- If a placeholder backend is nevertheless constructed (tests) and receives `Connect`, it
  answers (a change: today it sends only the `Disconnected`) with `LoginRefused { reason: NotAvailable }` and then one `Disconnected`
  (`TransportFailed`, carrying the text of `CompatError::ProtocolNotBuilt`) in the same poll,
  and opens no socket. The client shows the same message, not the error text.

<a id="s4-5"></a>
### 4.5 The plugin host in a start-screen run

Today the client starts its plugin host at start-up (`Plugins::start`: plugin discovery, the
command dispatcher's plugin registry, the `--load-plugin` queue). In a run that opens the start
screen (`--start-screen` or `--official`) this changes:

- The plugin host is **not started** while the start screen, the warning, the account form,
  the login screens or the official character placeholder (§8.7) show. It is started only when
  a session reaches the point where plugins belong: for official play when real character
  screens take the session at §8.7 (not while the placeholder shows), for "Explore offline" when
  the walker starts.
- So no plugin is discovered, loaded or run, and no plugin command is registered, before §8.7.
  `--load-plugin` conflicts with `--official` (§4.2). In a `--start-screen` run its names are
  queued only when the host starts for "Explore offline"; if the user chooses official play
  they are dropped, with one warning line naming how many were dropped.
- Going back to the start screen (from the character screens, §8.7) unloads every plugin
  apartment and stops the plugin host **before** the start screen is shown. There is no way
  back from the offline walker: quitting the walker ends the client, as it does today.

Runs without the start screen keep today's behaviour.

> **OPEN (implementer, 2026-10-23):** §4.1 and §4.5 imply a way back from the offline walker to the start
> screen, which the walker does not have.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: there is no way back from the offline walker to
> the start screen; quitting the walker ends the client. §4.1 and §4.5 now say so. A way back
> needs a revision of this spec (it would also have to stop the plugin host, §4.5).

<a id="s4-6"></a>
### 4.6 How the official screens are drawn

**Closing the window.** On the start screen, closing the window ends the client (exit code 0),
like Quit. On every other official screen it acts as that screen's way back, one step towards
the start screen, and never ends the client:

| Screen | Closing the window |
|---|---|
| Warning (§5.4) | Decline: back to the start screen |
| Not-available message (§4.4) | OK: back to the start screen |
| Account form (§7) | Back: both fields zeroed, back to the start screen |
| Progress, world list, queue (§8.3, §8.4) | Cancel: `Disconnect`, back to the account form |
| Official character placeholder (§8.7) | Back: `Disconnect`, back to the account form, both fields empty |

The start screen, the warning screen, the not-available message, the account form, the login
screens, their message panels and the official character placeholder (§8.7) are **painted by
Tessera's own code** with the bundled font (decision 0022) and Tessera's neutral art. They are
not built from `tessera-ui` elements and do not pass through the UI host, so the warning's input
stays away from the UI host's queues (§5.4 rule 1) and no element tree that plugins or game code
can reach holds them.

> **OPEN (implementer, 2026-10-23):** §4.1 and §5.4 say "code-made `tessera-ui` elements", which would route
> the warning's input through the UI host.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: these screens are painted by Tessera's own code
> with the bundled font, not built from `tessera-ui` elements (this section; §4.1, §5.4, §7.1,
> §7.2 follow it).

> **OPEN (reviewer, 2026-10-23):** §8.7 lets closing the window on the character placeholder return to the
> account form (built that way), while elsewhere closing the window ends the client or declines.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule, the build's behaviour kept: closing the window on
> the placeholder returns to the account form ("Closing the window" above). Reason: inside the
> official flow closing the window is always the safe way back of the screen it is on, never an
> exit: on the warning it is Decline, which also does not end the client, and on the placeholder
> it ends the official session cleanly with `Disconnect` instead of tearing it down with the
> process. Only the start screen, which holds no session, ends the client on a close, so at most
> a few closes in a row reach it. Implementers re-check the form and login screens against the
> table.

---

<a id="s5"></a>
## 5. The ban-risk warning

<a id="s5-1"></a>
### 5.1 When it is shown

At §9 step 2, every time the user chooses official play, the client checks the acceptance
record (§5.6) against the current warning digest (§5.3). The warning screen is shown unless
the check gives **accepted**. So it is shown:

- on first use (no record);
- whenever the warning's text changes (the record's digest differs);
- whenever the record is missing, unreadable, damaged, too large, not a regular file, writable
  by other OS users, bound to another OS user, machine or directory, or fails its
  authentication code (§5.6);
- after the user withdrew the acceptance (§5.5).

Every doubt shows the warning; nothing makes the client skip it except a record that passes
every check of §5.6. The check never repairs, rewrites or deletes a record it rejects.

<a id="s5-2"></a>
### 5.2 The text (Tessera's own string-table entries)

The warning is Tessera's own text, English, in Tessera's built-in string table **compiled into
the client binary** (not read from the install, from a file next to the binary, from a UI skin,
or from any file a user or program could replace). Its entries:

| String ID | Text |
|---|---|
| `official.consent.title` | Before you play on the official servers |
| `official.consent.body` | Tessera is an independent, third-party program. It is not made, approved or supported by the operator of the official game service, and it is not the official game client.<br><br>Using a third-party client may break the official service's terms of use. The operator may notice that you are not using its own client and may suspend or permanently close your account because of it, with every character, item and purchase on it. Nobody who works on Tessera can prevent that, undo it or restore an account.<br><br>Tessera may also behave differently from the official client in ways you cannot see, and a fault in Tessera could make the service treat your account as misbehaving.<br><br>Continue only if you accept this risk for the account you are going to use. If you are not sure, choose Decline: you can still use Tessera offline.<br><br>Tessera sends your account name and password only to the official service's login and never stores them. It shows this warning again whenever its text changes. |
| `official.consent.checkbox` | I have read this warning and I accept the risk to my account. |
| `official.consent.accept` | Continue to the official servers |
| `official.consent.decline` | Decline |
| `official.consent.changed` | This warning has changed since you last accepted it. Please read it again. |
| `official.consent.saved_failed` | Your acceptance could not be saved. It counts for this run only, and the warning will be shown again next time. |
| `official.consent.withdraw` | Withdraw acceptance |
| `official.consent.withdrawn` | Your acceptance was withdrawn. The warning will be shown again before you play on the official servers. |
| `official.consent.not_stored` | On this system Tessera does not save your acceptance. It counts for this run only, and the warning is shown every time you start Tessera. |

`<br><br>` in the table marks a paragraph break, which in the string itself is two line-feed
characters (U+000A U+000A), with no other whitespace around it and none at the start or end of
the body; the body is shown as plain text in four or five
paragraphs, never with markup, links or text from any other source inserted. The **warning
revision** is 1 for this text. Tessera's other strings for official play (start screen, form,
login screens, errors) are the English texts quoted in §4, §7 and §8, kept in the same table.

> **OPEN (owner):** translations. This revision ships English only. When translations are
> added, each is a further built-in entry set with its own digest (§5.3), and choosing another
> UI language shows the warning again (its shown text changed). Please confirm that a
> language change should re-prompt.

<a id="s5-3"></a>
### 5.3 Digest and revision: how a text change is detected

The **warning digest** is the SHA-256 digest of one byte string built from the shown text:

1. the ASCII text `tessera-official-consent-v1`;
2. the language tag of the shown entry set (`en` in this revision);
3. the warning revision as a decimal number;
4. the shown texts of `official.consent.title`, `.body`, `.checkbox`, `.accept` and `.decline`,
   in that order, each in Unicode normalisation form C, as stored in the string table (so the
   body's paragraph breaks enter as their two U+000A characters, §5.2; no markup, wrapping or
   layout enters).

Each of the items is encoded as UTF-8 and preceded by its length in bytes as a 4-byte
little-endian unsigned number, so no two different texts give the same byte string.

- Any change to any of the five texts, the language or the revision gives a different digest,
  so the record no longer matches and the warning is shown again (§5.1). A typo fix counts as a
  change.
- The revision exists for people, not for the check: it is raised with every intended change,
  shown in the record (§5.6) and in logs, and lets the screen show `official.consent.changed`
  when a record with a **different** digest exists. Validity is digest equality alone.
- A unit test pins the digest of the shipped text (OE-T4): changing the text fails that test
  until the test's expected digest is updated and the revision raised in the same change, so a
  text change is always deliberate and visible in review.
- The texts `.changed`, `.saved_failed`, `.withdraw`, `.withdrawn` and `.not_stored` are not
  part of the digest.

<a id="s5-4"></a>
### 5.4 The screen

The warning screen is a full-window Tessera screen painted by Tessera's own code with Tessera's
neutral art and the bundled font (decision 0022), not built from `tessera-ui` elements (§4.6).
It is **not** an engine dialog of
`ui-widgets.md` §18 and uses no install layout: no game-code description, plugin, script or
server message can create, fill, cover or answer it.

Layout: the title; when a record with another digest exists, the `.changed` line in a
highlighted box; the body in a scrollable text area; the check box with the `.checkbox` label;
two buttons, `.decline` (left) and `.accept` (right). The window title and every label are
Tessera's own strings (§5.2). Nothing else is drawn on the screen.

**Input rules** (Design, after decision 0017 §5.3):

1. The screen takes **device input only**. The client's window-event handler (the windowing
   library's per-window event callback in `apps/client`, today `app.rs` `window_event`) is the
   only place that creates a **device-input** value from a key, pointer or text event, and the
   warning screen reads only device-input values, handed to it directly by that handler, never
   through the UI host's queues. Synthetic sources (the windowed input script, `--ui-script`,
   `--command` lines, the command dispatcher, the plugin host, scenario drivers) feed the UI
   through other entry points and cannot make a device-input value. A source-scan test checks
   that the device-input value is created only in that handler and in `#[cfg(test)]` code
   (OE-T7), and tests build device-input values with a constructor that exists only in test
   builds of the client (OE-T2, OE-T11). Input that another program injects at the operating
   system level reaches the handler like real input and cannot be told apart (§5.8).
2. **Delay.** For its first 1,000 ms on screen, every input except Decline is ignored, Tab
   included. Decline acts at any time, without delay: Escape, closing the window, a click on the
   Decline button and Enter or Space while Decline has focus (it has focus from the start, rule
   3), because declining is always safe. A pointer press counts only if it starts after the delay, on the
   control; a click is a press and release both on the same control. A key counts only if it is
   pressed after the delay; a key that was held when the screen appeared is ignored until it is
   released.
3. **Focus starts on Decline.** Tab and Shift+Tab move focus among the check box, Decline and
   Continue; Space toggles the focused check box; Enter or Space activates the focused button.
   Nothing gives Continue focus but the user's own Tab presses or a pointer press on it.
4. **Continue is disabled until the check box is ticked** (drawn ghosted, takes no input or
   focus). Unticking it disables Continue again.
5. **No shortcut accepts.** No key accepts the warning by itself: there is no default button,
   no accelerator and no key bound to Continue; Enter and Space act only on the control that has
   focus. Escape is Decline, at any time, without delay, whatever has focus; closing the window
   is Decline.
6. **Keyboard confirmation counts.** Activating the focused Continue button with Enter or Space,
   after the user ticked the box and moved focus there with their own key presses (all after the
   delay), is an explicit confirmation, exactly like a click: it needs the same two deliberate
   actions, and keeps the screen usable without a pointer.
7. The screen shows only when its text has been laid out in full with the bundled font. If
   the font or the text cannot be drawn, Continue stays disabled (it fails closed) and the
   screen offers only Decline.

Accepting therefore needs two deliberate actions by the user after reading time: tick the box,
then activate Continue.

> **OPEN (implementer, 2026-10-23):** rules 2 and 5 let Escape and closing the window decline at once, but
> not a click on Decline or Enter on the focused Decline button during the delay.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: Decline acts at any time, with no delay, by
> every route (Escape, closing the window, a click on Decline, Enter or Space on the focused
> Decline), since declining is always safe; only the check box, Continue and focus moves wait for
> the delay (rule 2).

<a id="s5-5"></a>
### 5.5 Accepting, declining, withdrawing

- **Accepting.** Activating Continue (with the box ticked) calls the acceptance writer with a
  user-confirmation value that only this handler creates (§6.3). The writer writes the record
  (§5.6) on a worker thread; the flow goes on to §9 step 4 at once. If the write fails, the
  acceptance counts for this run only (an in-memory acceptance with the same digest), the
  client shows `.saved_failed`, and the next run shows the warning again. On Windows nothing
  is written (§5.6 "Windows"): the acceptance is in memory for the run and the client shows
  `.not_stored` instead.
- **Declining** (Decline, Escape or closing the window) writes nothing, changes no existing
  record and returns to the start screen. No backend is constructed, nothing of official play
  is read from the install, and no network operation happens.
- **Withdrawing.** "Withdraw acceptance" on the start screen deletes both files of the record
  (§5.6) and any in-memory acceptance of the run, then shows `.withdrawn`. It is offered only
  on the start screen, so never while an official session exists.

<a id="s5-6"></a>
### 5.6 The acceptance record

**Location.** Two files in Tessera's own config directory, the platform's per-user
configuration directory followed by `tessera/` (on Linux `$XDG_CONFIG_HOME/tessera/`, usually
`~/.config/tessera/`), the same directory family as the settings file:

| File | Content |
|---|---|
| `official-consent.toml` | The record document (below), at most 4,096 bytes |
| `official-consent.key` | Exactly 32 bytes from the operating system's random source, made at the first acceptance; never shown, logged or copied elsewhere |

The location is derived from the platform's directory conventions only. It is **never** taken
from `TESSERA_SETTINGS_FILE`, `--data-dir`, `--known-servers`, any other option, any other
environment variable named by Tessera, the settings file or the install. Consent is never kept
in `settings.toml`: a consent-like key there (in any section) is an unknown key, kept as the
settings crate keeps unknown keys, and never read.

**Record document** (TOML, the keys below only; any other key, a missing key or a value out of
range makes the record invalid):

| Key | Type | Meaning |
|---|---|---|
| `schema` | integer | 1 |
| `revision` | integer, 1–1,000,000 | The warning revision accepted (§5.3) |
| `language` | string, 2–16 ASCII letters, digits or `-` | The language tag of the accepted text |
| `text_sha256` | string, 64 lowercase hexadecimal digits | The warning digest accepted |
| `accepted_at` | string, RFC 3339 date-time in UTC, ending in an upper-case `Z` | When the user activated Continue |
| `mac` | string, 64 lowercase hexadecimal digits | The authentication code below |

**Canonical form.** The writer writes the document in one canonical form, and the check
accepts no other: the six keys in the order of the table, one per line as the key, a space, `=`,
a space and the value, each line ended by one line feed (U+000A), and no other byte (no
comment, blank line, table header, byte-order mark or extra whitespace). Integers are written in
canonical decimal: digits only, no sign, no leading zero, no underscore. Strings are TOML basic
strings in double quotes with no escape sequence (every allowed value is printable ASCII without
`"` or `\`). `accepted_at` is written as `YYYY-MM-DDThh:mm:ssZ` in UTC. The check parses the
document strictly, renders the parsed values again in the canonical form and requires the result
to equal the file byte for byte; so `0x1`, `+1`, `01`, `1_000`, single-quoted or multi-line
strings, escapes, reordered keys and an `accepted_at` ending in `z` or an offset such as
`+00:00` give `invalid`.

**Authentication code.** `mac` is HMAC-SHA-256 with the key file's 32 bytes as key, over one
byte string built like §5.3 (each item length-prefixed): the ASCII text
`tessera-official-consent-record-v1`; the five values `schema`, `revision`, `language`,
`text_sha256` and `accepted_at` as written in the document (which, by the canonical-form rule,
is their canonical form); and the **binding**:

- the OS account: the numeric user ID (Unix-like systems; Windows stores no record, below);
- the machine: on Linux the content of `/etc/machine-id` and of no other file (one line feed at
  its end removed), read up to 128 bytes plus one; on other systems, or when that file cannot be
  read, is empty or is longer than 128 bytes, the host name;
- the absolute, canonical path of the directory that holds the record.

The binding values are read from the operating system at check time and never written into
the document. A record copied to another OS user, another machine or another directory fails
its code.

**Check** (on a worker thread, bounded): both files are opened without following a final
symbolic link, must be regular files, and are read up to their limit plus one byte. On
Unix-like systems a file that group or others can write, or that another user owns, is
rejected. The document must be in the canonical form (above), the code must match (compared
in constant time), and `text_sha256` must equal the current warning digest. A missing key file,
or one that is not exactly 32 bytes, gives `wrong_binding_or_code` (not `missing` or
`invalid`). Then the check gives **accepted**;
any other outcome gives **not accepted** with a reason for the log (`missing`, `unreadable`,
`invalid`, `wrong_binding_or_code`, `text_changed`), and `text_changed` makes the screen show
`.changed`. A read that takes longer than 1 s gives **not accepted**. No file lock is taken:
writes are atomic renames (below), so a reader sees either the old or the new whole document,
and two Tessera processes accepting at once leave one complete record.

**Write** (on a worker thread):

1. **Key.** If no key file exists, the writer makes one: 32 random bytes written to a temporary
   file in the same directory (owner-only permissions, mode 0600), flushed, then renamed to
   `official-consent.key` with a rename that never replaces an existing file. If that rename
   finds a key file already there (another Tessera process won the race), the writer uses that
   file if it passes the checks of the next step, else fails. If a key file exists, it is used
   only if it is a regular file of exactly 32 bytes that passes the permission and owner checks
   above; a key file that exists with another size or with wrong permissions or owner is
   **never overwritten or deleted** by the writer: the write fails (closed), the acceptance
   counts for the run (§5.5, `.saved_failed`), the warning shows again next run, and the log
   names the file and the reason (never its content). The key file is never repaired: on that
   machine every later acceptance lasts for its run only, and the start screen shows the
   `.saved_failed` line (§4.1), until the user removes the file by hand or withdraws (§5.5);
   Withdraw deletes both files, whatever their state (the directory entries themselves, never the target of a link), so the next acceptance writes a fresh key.
   If Withdraw cannot delete a file (another owner, a read-only directory), the log names the
   file and the line stays.
2. **Document.** The document is written in the canonical form to a temporary file in the same
   directory, flushed and renamed over the old one.

The writer never writes a record for a digest other than the current one.

**Windows.** On Windows Tessera stores no record in this revision: the check always gives **not
accepted** (log reason `not_stored`) without opening any file, the writer writes neither file,
and an acceptance lasts for the run only (§5.5, `.not_stored`), so the warning is shown on every
run. Withdraw clears the run's acceptance. This is lifted by a revision of this spec that binds
the record to the Windows user's security identifier and checks the files' owner and access
rules, through a dependency or reviewed unsafe code the owner accepts for that purpose.

> **OPEN (implementer, 2026-10-23):** binding the record to the Windows user's security identifier needs
> unsafe code or a new dependency.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule (orchestrator, delegated by the owner): Windows
> fails closed for now: no record is stored, the acceptance lasts for the run and the warning
> shows on every run ("Windows" above). What lifts it is stated there.

> **OPEN (implementer, 2026-10-23):** the record details: integer forms, what the code covers, the time
> zone mark, the outcome for a missing or wrong-size key, whether a bad key file may be
> overwritten, how the key is written, and where the Linux machine identifier comes from.
>
> **Answered (spec writer, 2026-10-23):** Tessera rules, written into this section: integers in
> canonical decimal and the document accepted only in its canonical form, re-rendered and
> compared byte for byte (so `0x1`, `+1`, quote variants and escapes are refused); the code
> covers the values as written, which equal their canonical form; `accepted_at` ends in `Z`; a
> missing key file or one of the wrong size gives `wrong_binding_or_code`; an existing key file
> with the wrong size or permissions is never overwritten (fail closed, the warning shows again);
> the key is written through a temporary file and a rename that never replaces; on Linux the
> machine identifier comes from `/etc/machine-id` only.

> **OPEN (implementer, 2026-10-23):** an existing invalid key file is never repaired, so every later acceptance
> on that machine lasts for its run only until the user withdraws; is that the rule, and what does
> the start screen show meanwhile?
>
> **Answered (spec writer, 2026-10-23):** Tessera rule, confirmed: an invalid key file is never repaired
> or replaced by the writer (fail closed); every later acceptance lasts for its run only until the
> user withdraws, which deletes both files, or removes the file by hand. Meanwhile the start
> screen's acceptance line shows the `.saved_failed` text with "Withdraw acceptance" ("Key" step 1
> above, §4.1).

> **OPEN (implementer, 2026-10-23):** the size bound for `/etc/machine-id` (the implementation reads at most
> 128 bytes).
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: `/etc/machine-id` is read up to 128 bytes; a
> longer file, an empty one or one that cannot be read gives the host-name fallback (the binding
> list above). 128 bytes is a Design bound, well above the identifier's usual length.

<a id="s5-7"></a>
### 5.7 What cannot accept or skip the warning

| Source | Why it cannot accept or skip |
|---|---|
| **Command-line options** | No option names consent, an account, a password or the record's location (§4.2). `--official` only selects; `--official` conflicts with every option that brings synthetic input (§4.2) |
| **Environment variables** | No code of the consent module or the official-entry module reads an environment variable directly. The platform directory functions may consult the platform's own variables (such as `XDG_CONFIG_HOME`); a record in a directory they point to still needs a valid code bound to this OS user, this machine and that directory (§5.6, §5.8) |
| **Configuration files written by anything else** | The record is valid only with an authentication code over a per-machine random key and the binding (§5.6); a file edited by hand or written into `settings.toml` is rejected, and so is a file copied from another OS user, machine or directory, as far as the binding tells them apart (not when the machine falls back to its host name and the copy comes from a machine with the same host name, nor between clones of one system image that share machine identifier, user ID and path; §5.8). `PluginOptions.xml`, `UserPreferences.ini`, the keymap file and every file of the install are never read for consent |
| **Plugins** | The plugin host is not started while the start, warning, form and login screens show (§4.5), and `--load-plugin` conflicts with `--official` (§4.2), so no plugin is discovered, loaded or run and no plugin window exists that could cover or click the warning; autoload comes only after the login (`plugin-runtime.md` §8.2). The plugin API has no binding that reaches the consent module, the record or these screens, and the plugin-data writer cannot write outside its own data directory, which never holds the record |
| **Scripts** | The windowed input script (`--scenario-input`), `--ui-script` and `--command` conflict with `--official` (§4.2); the command dispatcher (decision 0020) has no built-in or registered command that reaches consent; extended scripts (M9) do not exist before the warning and get no such binding (decision 0034 §4). Game-code dialogs (`ui-widgets.md` §18) cannot produce the screen (§5.4) |
| **Servers** | No backend event changes consent. The compatibility backend cannot be constructed without a consent proof (§6.1), so no server is ever reached before acceptance; no text from any server is drawn on the warning screen |
| **Synthetic input** | The warning screen takes device input only (§5.4 rule 1) |

<a id="s5-8"></a>
### 5.8 What a local file can and cannot guarantee

A record on the user's own disk **cannot** stop a program that runs with the user's own rights
from forging it: such a program can read the key file, read the binding values and compute a
valid code, or simply be a modified build of Tessera without the gate (Tessera is free
software under the GPL; anyone may change and rebuild it). Nor can the client tell input that
another program injects at the operating-system level from the user's own (§5.4 rule 1).
Tessera does not claim otherwise and does not try to hide the key from the user.

What the design **does** guarantee:

- Tessera's own code treats as accepted only a record that Tessera wrote after a confirmation
  on its own screen (§5.5, §6.3), a record that a purpose-built program computed for this
  binding, or a copy of a record from a place with the same binding (below).
- A ready-made file handed around (a guide's "drop this file in", an installer, a mod pack, a
  settings sync from another machine) fails wherever its binding differs from the one it was
  made for, and the warning is shown again. The binding cannot tell apart:
  - machines that have no readable machine identifier and share a host name (the fallback of
    §5.6), when the OS user ID and the path also match;
  - clones of one system image (virtual machines, cloned disks, shared container images) that
    share the machine identifier, the user ID and the path; a record accepted on the image is
    valid on every clone.

  In both cases a copy of both files carries the acceptance. Tessera accepts this: the record
  was made by an explicit confirmation on a system with that identity.
- Nothing Tessera reads by design (options, environment variables, the settings file, the
  install, plugins, scripts, servers) can produce an acceptance (§5.7).
- Every failure is closed: a doubtful record shows the warning (§5.1).
- On Unix-like systems, a record that other OS users could write is rejected (§5.6). On
  Windows no record is stored or read in this revision (§5.6 "Windows"), so nothing on disk can
  accept the warning there; it is shown on every run.

So circumventing the warning needs a deliberate act equivalent to modifying Tessera, which no
local mechanism can prevent and which the rule of decision 0034 does not require Tessera to
prevent. Keeping the key in the platform's credential store was considered; it does not change
this limit for a same-user program, so it is not required.

---

<a id="s6"></a>
## 6. The gate: no official connection before acceptance

<a id="s6-1"></a>
### 6.1 The consent proof

`tessera-compat` gets a module `consent`, independent of the crate's protocol modules and with
no network code. It owns the warning's string entries (§5.2), the digest (§5.3), the record
check and writer (§5.6) and the **consent proof** type:

- A consent proof is produced only by the record check giving **accepted**, or by the writer
  after an acceptance (including a write that failed, for that run, §5.5).
- It carries the warning digest it was made for. It cannot be cloned, copied, defaulted,
  serialised, deserialised or built from parts outside the module (its fields are private and
  it has no public constructor).
- Every constructor of `CompatBackend` takes a consent proof by value; the backend checks that
  the proof's digest equals the current warning digest and otherwise refuses to be built.
- Inside `tessera-compat`, every operation that can send or receive on a network (sockets,
  name resolution, a request of any protocol, whatever its purpose) is reachable only through one internal network-access type whose
  constructor takes a reference to the consent proof. The UDP transport that the crate's
  session module builds today on a standard-library `UdpSocket` moves behind that type (it is
  made from the network-access value, never from a bare address). The crate also may not
  reach a network through `tessera-net`'s `transport` or `remote` modules, nor by starting
  another process. A source-scan test (OE-T8) enforces the rule over the names listed there.
- `CompatConfig` stops deriving `Clone` and `Default` (it carries the proof and the credentials,
  §7.3), and its `Debug` form prints neither.
- The availability answer (§4.4) and the consent module itself need no proof and do no network
  operation.

Unit tests of `tessera-compat` itself may obtain a proof from a record written in a temporary
directory through the writer; no proof constructor exists outside the module, test builds
included.

The helpers that build record parts (rendering a document, computing a code from given binding
values) are private to the crate. They are exposed only behind a test-support Cargo feature of
`tessera-compat` for the fuzz target and tests; only `fuzz/` and dev-dependencies enable it,
release builds never do, and a check of the workspace's manifests fails if any normal dependency
on `tessera-compat` enables it (OE-T8).

<a id="s6-2"></a>
### 6.2 The client's one construction site

In `apps/client`, `CompatBackend` is constructed in exactly one function, in the official-entry
module, which is reached only from §9 step 5 with the proof from step 2 or 3. A source-scan test
fails if any other file of `apps/` or `crates/` (outside `crates/compat`) names `CompatBackend`
or a `Connector` implementation (OE-T8). `tessera-server` never links `tessera-compat`
(`tools/check-server-deps.sh`, unchanged).

<a id="s6-3"></a>
### 6.3 The user-confirmation value

The acceptance writer takes a **user-confirmation** value. It is created only in the warning
screen's Continue handler, from the device-input event that activated Continue with the box
ticked. A source-scan test fails if the writer is called, or the confirmation value created,
anywhere else (OE-T6). The scan covers `apps/` and every crate's `src/`, `crates/compat/src`
included; it exempts only the definitions inside the consent module and `#[cfg(test)]` code
(and the test directories).

> **OPEN (implementer, 2026-10-23):** the scan scope: whether the acceptance-writer scan covers
> `crates/compat/src`, how the record-building helpers are kept out of release builds, and whether
> the network scan also covers `tessera-net`'s transport and remote modules and process spawning.
>
> **Answered (spec writer, 2026-10-23):** Tessera rules: the acceptance-writer scan covers
> `crates/compat/src` too, exempting only the definitions in the consent module and `#[cfg(test)]`
> code (this section); the record-building helpers are crate-private, exposed only behind a
> test-support feature that release builds never enable (§6.1); the network scan also covers the
> use of `tessera-net`'s transport and remote modules and process spawning from `tessera-compat`
> (§6.1, OE-T8(b)). Rust visibility
cannot restrict a public function to one caller crate, so this rule is enforced by the test,
like `session::name_tests` enforces event names.

**Release builds.** Release artefacts are built with a plain `cargo build --release -p
tessera-client`, never with `--all-targets`, `--tests`, `--benches`, `--examples`, `--workspace`,
`--all-features` or a `--features` list: with `--all-targets` (and the like) cargo unifies the
features of dev-dependencies into the binary, which would turn on `tessera-compat`'s test-support
feature (§6.1) in it. The check `tools/check-release-features.sh` resolves the feature tree of
`tessera-client` over normal dependency edges only (no dev or build edges, the release command's
feature set) and fails if the test-support feature of `tessera-compat` is enabled in it; it runs
in CI and in the local gate (OE-T22). Release instructions name the exact command.

> **OPEN (reviewer, 2026-10-23):** release builds: with `--all-targets` cargo enables the dev-dependency
> test-support feature in the binary; how are release artefacts built and checked?
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: release artefacts come from a plain
> `cargo build --release -p tessera-client`, and `tools/check-release-features.sh` checks the
> normal-edge feature tree ("Release builds" above, OE-T22).

<a id="s6-4"></a>
### 6.4 Why this is testable without a network

The gate is a type rule (no proof → no backend → no network-access value) plus source scans; the
flow of §9 is a pure state machine in the client with the record directory injected. Tests drive
it with a temporary record directory, a recording test connector and a test-double backend; no
test opens a socket or reaches any service (decision 0034 §3 item 1).

---

<a id="s7"></a>
## 7. The account form

<a id="s7-1"></a>
### 7.1 Fields and controls

A Tessera screen painted like §4.1 (Tessera's own code, bundled font, neutral art, no
`tessera-ui` elements, §4.6):

| Control | Label | Rules |
|---|---|---|
| Text entry | "Account name" | 1–128 bytes of UTF-8 as typed, no control characters; starts **empty** every time |
| Masked entry | "Password" | 1–256 bytes of UTF-8 as typed; shows one bullet (U+2022) per character; starts empty every time |
| Button | "Log in" | Enabled when both fields are non-empty and no retry wait runs (§8.6); Enter in either field activates it |
| Button | "Back" | Clears both fields, returns to the start screen; Escape does the same |
| Line | "Tessera sends your account name and password only to the official service's login and never stores them." | Always shown |

> **OPEN:** the account-name and password rules of the official service (lengths, allowed
> characters, case, any normalisation Tessera must or must not apply). Until answered Tessera
> sends both exactly as typed and checks only the bounds above.

> **OPEN (owner):** whether to offer remembering the account name (stored in Tessera's
> settings, off by default). This revision stores nothing.

<a id="s7-2"></a>
### 7.2 Masking

- The masked field's drawn text, its layout, glyph cache and every draw see only bullets; the
  characters live only in the form's secret buffer (§7.3).
- Copy, cut and paste are not offered in either field in this revision (Tessera takes no
  clipboard dependency). Password managers that type the password as keystrokes work. There is
  no "show password" control.
- **Keystroke text.** The text of a key event is carried from the window-event handler to the
  form in a fixed-size value held inline (no heap string), copied into the secret buffer and then
  overwritten with zeros. Copies the windowing library or the operating system makes before the
  handler are outside Tessera's control (§7.3).
- Input-method composition is disabled for the masked entry, so no composition window shows
  the characters.
- The entry keeps no undo history.

> **OPEN (implementer, 2026-10-23):** paste needs a clipboard dependency, and text events would otherwise
> pass through heap strings.
>
> **Answered (spec writer, 2026-10-23):** Tessera rules: paste is not offered for now (no clipboard
> dependency); password managers that type work; keystroke text is carried inline and zeroed,
> never in heap strings (this section). Offering paste needs a revision of this spec.

<a id="s7-3"></a>
### 7.3 Memory lifetime and where the credentials go

- **Secret buffer.** The password is held in a buffer of fixed capacity (256 bytes) allocated
  when the form opens, so typing never reallocates and leaves copies behind. The account name
  is held in a second buffer of the same kind (128 bytes). Both are overwritten with zeros
  (best effort, as the test-server password type does) when the form is left by Back, by
  Escape, by closing the window, and right after Log in has moved their contents on.
- **Account-name copy for `BadCredentials`.** At Log in the official-entry module keeps one
  further copy of the account name only, in a buffer of the same kind that is overwritten with
  zeros when dropped. It is used only to refill the account-name field after `BadCredentials`
  (§8.6) and is dropped as soon as the attempt's outcome is known: after refilling on
  `BadCredentials`, at once on every other outcome, and when the flow leaves for the start
  screen. Every other row of §8.6 that returns to the form reopens it with both fields empty.
- **Hand-over.** Log in moves both values into the compatibility backend's configuration as a
  **credentials** value: no `Clone`, a `Debug` form that prints only `<redacted>`, no
  serialisation, overwritten on drop; the configuration that holds it (`CompatConfig`) has no
  `Clone` or `Default` either (§6.1). The configuration and the backend are made at that moment
  and handed to a new `Session`; the credentials never travel in a `Command` or `Event`.
- **One use.** The backend takes the credentials on their first use and drops (overwrites)
  them as soon as the login reports success or failure, or the session ends. A later step that
  would need them again ends the session instead (§8.6); the user types them again.
- **Where they are sent.** The client hands the credentials to the compatibility backend and
  to nothing else: no Tessera server, plugin, script, log, settings file, record, crash report,
  clipboard or telemetry (Tessera has none). Where the backend sends them is the M8 login
  spec's matter, bounded by decision 0034 §3 item 3: only to the official service's login, as
  the user.
- **Never on disk or in logs.** No log line, `tracing` field, panic message, error text,
  window title or file contains the account name or the password. Errors name a category
  only (§8.6). The official-entry module logs state changes and categories; world names and
  service texts only at `debug` and sanitised (§8.3).

Tessera does not claim protection against a program with the user's rights reading its memory,
nor against swap or core files written by the operating system, nor against copies the windowing
library or the operating system makes of typed text before it reaches the client's handler.

> **OPEN (implementer, 2026-10-23):** §7.3 zeroes both fields right after Log in, but §8.6 keeps the account
> name after `BadCredentials`.
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: a separate copy of the account name, zeroed when
> dropped, kept only for the `BadCredentials` refill ("Account-name copy" above); every other row
> reopens the form empty (§8.6).

> **OPEN:** whether the official login needs anything beyond the account name and password
> (a second factor, a challenge, a choice made on the login, an agreement the service asks
> the user to accept), and at which point. Until answered, a backend that meets such a step
> answers `LoginRefused { reason: UnsupportedStep }` (§8.6); no workaround is built (decision
> 0034 §3 item 3). If the service presents terms that the user must accept, Tessera shows them
> to the user as received and never accepts them on the user's behalf; that needs a revision
> of this spec.

---

<a id="s8"></a>
## 8. Login over the abstract backend

<a id="s8-1"></a>
### 8.1 The Account group

A new group extends `Event` and `Command` like the groups of `ARCHITECTURE.md` §19.1:
`Event::Account(AccountEvent)` and `Command::Account(AccountCommand)`, both
`#[non_exhaustive]`. Names are Tessera's own and unique among all events and commands
(`session::name_tests` covers the group). `tessera-net` interprets no value; it only checks the
bounds of §8.2.

| Variant | Kind | Fields | Meaning |
|---|---|---|---|
| `LoginProgress` | event | `stage`: `Authenticating`, `ListingWorlds` or `JoiningWorld` | The login reached a stage (for the progress screen) |
| `WorldList` | event | `worlds`: 1–256 entries, each `id` (a number the backend assigns, unique in this list), `name` (1–64 bytes), `status` (`Open`, `Busy`, `Full`, `Closed`, `Restricted`, `Unknown`), optional `load` (`Low`, `Medium`, `High`), optional `note` (at most 128 bytes) | The worlds the user may choose from; a later `WorldList` replaces the earlier |
| `LoginQueueChanged` | event | optional `position` (1 = next), optional `length`, optional `wait_seconds` (at most 86,400) | The user waits for a place on the chosen world |
| `WorldRefused` | event | `world` (an `id` of the latest list), `reason` (`Full`, `Closed`, `NotAllowed`, `Other`), optional `detail` (at most 256 bytes) | The chosen world refused the user; the session stays in `Connecting` and a new `WorldList` may follow |
| `LoginRefused` | event | `reason` (§8.6), optional `retry_after_seconds` (at most 3,600), optional `detail` (at most 256 bytes) | The login failed; one `Disconnected` follows in the same poll |
| `ChooseWorld` | command | `world` (an `id` of the latest `WorldList`) | Join that world |
| `LeaveLoginQueue` | command | — | Leave the queue; the backend sends a new `WorldList` if it can, else ends the session with `Disconnected { ClientRequested }` |

`Event::Connected { characters }` (existing) ends the login: the backend sends it when the
chosen world accepts the user, and the character screens follow (§8.7). World `id`s are
numbers so that no address, key or service token can reach the client through them; the
backend keeps its own mapping.

> **OPEN:** the facts the backend needs to fill a `WorldList` entry (which attributes of a
> world the official service reports, and how they map to `status`, `load` and `note`).

> **OPEN:** whether the official service reports a queue position, a queue length and an
> estimated wait, and how often.

> **OPEN:** whether, after a world refusal or after leaving a queue, the login can return to
> the world list without authenticating again; and whether anything after the first
> authentication needs the password again. Until answered, a backend that cannot keep the
> login ends the session (§8.6).

<a id="s8-2"></a>
### 8.2 Session rules

- Account events are surfaced only while the session is `Connecting`. In `Closing` (entered from
  `Connecting`) they are stale and dropped quietly; in any other state they are a backend bug
  and dropped.
- **Logging of dropped Account events.** `Session` today logs a dropped invalid event in full
  (its `Debug` form) at `warn`. For Account events this changes: the line is at `debug`, names
  only the variant and the reason (never a field: no world name, note or detail), and is
  rate-limited to one line per variant per 10 s.
- Account commands are allowed only in `Connecting` (`NetError::InvalidState` otherwise).
  `ChooseWorld` with an `id` that is not in the latest `WorldList` is `NetError::InvalidCommand`.
  The session keeps the `id`s of the latest list (at most 256).
- An Account event out of its bounds is dropped and logged as above. The bounds are checked on
  the values as the backend sent them, before any cleaning: an empty list or one of more than
  256 entries, a repeated `id`, a `name` empty or longer than 64 bytes, a `note` longer than 128
  or a `detail` longer than 256 bytes, a `wait_seconds` or `retry_after_seconds` over its limit.
  Texts within their bounds then have control characters replaced by `sanitize_text`; a name
  that is only control characters or spaces after that is also out of bounds.
- After `LoginRefused` the session accepts only `Disconnected`.
- The sandbox, test-server and overlay backends never emit Account events and refuse Account
  commands with `NetError::InvalidCommand`. The scenario feed has no Account steps in this
  revision, and the canonical log is unchanged for every existing scenario.
- `Command::Disconnect` stays valid in `Connecting` and is the way to cancel a login.

<a id="s8-3"></a>
### 8.3 Progress and the world list

- **Progress screen.** From `Connect` until the first `WorldList` (and again after
  `ChooseWorld` until `Connected`, a queue or a refusal), a panel painted by Tessera's own code
  (§4.6) shows the stage text
  ("Logging in…", "Getting the list of worlds…", "Joining ⟨world⟩…") and a Cancel button
  (sends `Disconnect`; Escape does the same).
- **Watchdog.** If no event arrives for 60 s while the progress screen shows (not while
  queued), the client sends `Disconnect` and shows "No answer from the official service."
  (§8.6).
- **World list screen.** A list with the columns Name, Status, Load and Note, in the backend's
  order. Status texts: "Open", "Busy", "Full (queue)", "Closed", "Not available to this
  account", "Unknown". `Closed` and `Restricted` rows are ghosted and cannot be chosen. The
  row whose name equals the last world chosen (§8.5) is selected first, else the first row
  that can be chosen. A double click on a row or the Join button sends `ChooseWorld`; Back
  sends `Disconnect` and returns to the account form. A new `WorldList` replaces the rows and
  keeps the selection by name.
- **Service text.** World names and notes are shown as plain text after `sanitize_text`
  (control characters removed, cut at their bounds), never as markup or links.

<a id="s8-4"></a>
### 8.4 The queue

On `LoginQueueChanged` the client shows a queue panel: "Waiting for a place on ⟨world⟩."; with a
position "You are number ⟨position⟩ in the queue." (and " of ⟨length⟩" with a length); with a
wait "Estimated wait: about ⟨minutes⟩ min." (rounded up to whole minutes, "less than a minute"
under 60 s); without any of them "Your place in the queue is not known.". The panel updates on
every event, at most 4 times a second. Buttons: "Leave queue" (sends `LeaveLoginQueue`; Escape
does the same) and "Cancel" (sends `Disconnect`). The watchdog of §8.3 does not run while
queued.

<a id="s8-5"></a>
### 8.5 What is remembered

Only the display name of the last world chosen, in Tessera's settings file (`[Official]`
section, key `last_world`, at most 64 bytes, sanitised), written by the existing settings
writer. No account name, password, world `id`, endpoint, service text or consent state is
written there.

<a id="s8-6"></a>
### 8.6 Errors and retries

`LoginRefused.reason` takes one of Tessera's categories below; the backend maps the service's
answers onto them.

| Reason | Tessera's text | Then |
|---|---|---|
| `BadCredentials` | "The account name or password was not accepted." | Account form; account name kept, password empty and focused |
| `AccountNotAllowed` | "The official service did not let this account log in." | Account form, both fields empty |
| `UnsupportedStep` | "This login needs a step that Tessera does not support. Tessera cannot log in to this account." | Start screen (decision 0034 §3 item 3: no workaround) |
| `ClientNotAccepted` | "The official service did not accept this client." | Start screen |
| `RateLimited` | "Too many attempts. Please wait ⟨seconds⟩ s." | Account form; Log in disabled for `retry_after_seconds` (60 s without one) with a countdown |
| `ServiceUnavailable` | "The official login service is not available right now." | Account form |
| `NotAvailable` | "Playing on the official servers is not available in this version of Tessera yet." | Start screen (§4.4) |
| `Other` | "The login failed." | Account form |

| Without `LoginRefused` | Tessera's text | Then |
|---|---|---|
| `WorldRefused` (`Full`, `Closed`, `NotAllowed`, `Other`) | "⟨world⟩ is full.", "⟨world⟩ is closed.", "This account cannot join ⟨world⟩.", "⟨world⟩ could not be joined." | World list (the session stays) |
| `Disconnected { ClientRequested }` after Cancel, Back or Leave queue | none | The screen the user asked for |
| `Disconnected { TimedOut }`, or the watchdog | "No answer from the official service." | Account form |
| `Disconnected { TransportFailed }` | "The connection to the official service failed." | Account form |
| `Disconnected { ServerClosed }` | "The official service closed the connection." | Account form |

- **Fields.** Only the `BadCredentials` row refills the account name (from the copy of §7.3);
  every other row whose target is the account form reopens it with both fields empty.
- A `detail` (or a `ServerClosed` text) is shown under the line as "The service said: ⟨text⟩",
  plain text after `sanitize_text`, for every row except `BadCredentials` and `NotAvailable`.
- **No automatic retry of a login.** Every new attempt is a new `Session` that the user starts
  with Log in, with the password typed again (§7.3). The backend never resends credentials on
  its own. After any failure, Log in stays disabled for 2 s (and for the `RateLimited` wait).
- The messages are message panels painted by Tessera's own code on the current screen (§4.6),
  with an OK button; they are not engine dialogs (§5.4).

> **OPEN:** how the official service's refusals and failures map onto the categories above
> (the facts the backend needs to choose a reason and a retry time).

<a id="s8-7"></a>
### 8.7 Hand-over to the character screens

On `Connected { characters }` the official-entry module hands the session to the pre-world flow
of `character-screens.md` (as `--character-select` does today, `character-screens.md` §6). What
those screens offer on an official session (creation, deletion, entering) is for the gameplay
and protocol specs.

**Placeholder (current rule).** Until a session can feed the pre-world flow, the official
session gets a placeholder instead of the character screens: a screen painted like §4.6 with the
text below and a Back button. Back (and Escape, and closing the window) sends `Disconnect` and
returns to the account form with both fields empty. The plugin host is not started while it
shows (§4.5).

| String ID | Text |
|---|---|
| `official.characters.unavailable` | Choosing a character on the official servers is not available in this version of Tessera yet. |

Leaving the character screens back to the start ends the session
(`Disconnect`) and returns to the account form; the warning is not shown again in that run
unless it was withdrawn.

> **OPEN (implementer, 2026-10-23):** the client cannot feed an official session into the pre-world flow
> yet; what does §8.7 show meanwhile?
>
> **Answered (spec writer, 2026-10-23):** Tessera rule: a placeholder with the text
> `official.characters.unavailable` and Back ("Placeholder" above) until a session can feed the
> pre-world flow. The string is listed with its ID above; §5.2's table holds the warning's
> strings only, and the other official strings are quoted where they are used.

---

<a id="s9"></a>
## 9. Flow

| Step | Screen or work | Next |
|---|---|---|
| 1 | The user chooses "Official servers" (or `--official` chose it) | 2 |
| 2 | Check the record (§5.6) on a worker; "Checking…" shows meanwhile. Nothing else of official play is read, built or contacted yet | accepted: 4 with the proof; not accepted: 3 |
| 3 | The warning screen (§5.4) | Continue: write (§5.5), 4 with the proof; Decline: start screen |
| 4 | Availability (§4.4) | `NotBuilt`: the not-available message, then start screen; `Available`: 5 |
| 5 | The account form (§7) | Log in: construct the backend with proof and credentials (§6.2), new `Session`, `Connect`, 6; Back: start screen |
| 6 | Login screens (§8.3, §8.4) driven by the Account group | `Connected`: 7; a refusal or end: the row's target in §8.6 |
| 7 | Character screens (`character-screens.md`) | — |

The proof made at step 2 or 3 is held by the official-entry module for the run and moved into
each backend constructed at step 5 (a new proof is made from the in-memory acceptance for each
new attempt, without reading the record again). Withdrawing (§5.5) drops it.

---

<a id="s10"></a>
## 10. Tests the implementer provides

<a id="s10-1"></a>
### 10.1 Unit and integration tests

All run without a network, without an install and without the user's real config directory
(the record directory is a temporary directory handed to the flow).

| ID | What | Passes when |
|---|---|---|
| OE-T1 | **Consent gating.** Drive the §9 state machine with an empty record directory | It stops at the warning screen; the account form and every later step are unreachable; no backend is constructed (construction counter 0) |
| OE-T2 | **Accept.** Tick, wait 1,000 ms, activate Continue with device events | The record is written with the current digest, the check then gives accepted, the flow reaches step 4 |
| OE-T3 | **Text-change re-prompt.** Accept, then check against a different test text set (another body, then another revision, then another language) | Each gives `text_changed`, the screen shows `.changed`, accepting again rewrites the record |
| OE-T4 | **Digest pinned.** Compute the digest of the shipped §5.2 texts | It equals the expected value written in the test; any change to the texts fails |
| OE-T5 | **Rejected records.** Wrong code; key missing and key of 31 or 33 bytes (each `wrong_binding_or_code`); non-canonical documents (`0x1`, `+1`, `01`, `1_000`, single-quoted and multi-line strings, an escape, reordered keys, a comment, a blank line, CRLF line ends, a missing final line feed, `accepted_at` ending in `z` or `+00:00`); record bound to another user ID, machine ID or directory (binding values injected); record copied with its key to another directory; unknown, missing or out-of-range keys; 4,097 bytes; not UTF-8; a symbolic link as final component; a directory, FIFO or device in its place; group- or world-writable file (Unix) | Each gives not accepted, never panics, and leaves the files byte for byte unchanged |
| OE-T6 | **Flags, environment, configuration cannot accept.** (a) Parse each client option paired with `--official`: none reaches step 4 with an empty record directory; each option of the §4.2 conflict list, `--load-plugin` included, is a usage error. (b) Source scan: the consent and official-entry modules read no environment variable; no CLI option refers to them. (c) A `settings.toml` with consent-like keys in any section, and `TESSERA_SETTINGS_FILE` pointing at a directory that holds a record valid for another binding: the check still gives not accepted. (d) Source scan over `apps/` and every crate's `src/`, `crates/compat/src` included: the acceptance writer and the user-confirmation value occur only in the Continue handler, exempting only their definitions in the consent module and `#[cfg(test)]` code (§6.3) | All four hold |
| OE-T7 | **Plugins, scripts, servers cannot accept.** (a) Source scan: no plugin-API binding, dispatcher command or `tessera-script` module names the consent module, the record files or the official screens. (b) A test-double backend that emits every Event variant, every Account event and out-of-bound values while the warning shows or after a decline: the record directory is unchanged and the flow does not pass step 3. (c) Synthetic key and pointer events (the input-script path, `--ui-script`, dispatcher lines, plugin-host input) delivered while the warning shows do not tick the box or activate Continue; source scan: the device-input value (§5.4 rule 1) is created only in the window-event handler and in `#[cfg(test)]` code. (d) Plugin host: in a `--start-screen` and an `--official` run driven through steps 1–6 (decline, accept, not-available, form, login screens), the plugin host is started 0 times and 0 apartments exist; after a hand-over (§8.7) and a return to the start screen, 0 apartments exist and the host is stopped before the start screen shows | All four hold |
| OE-T8 | **No connection attempt before acceptance.** (a) A recording `Connector` and network-access counter: through declines, Escape, window close, withdraw and rejected records, `connect` is called 0 times and no network-access value exists. (b) Source scan of `crates/compat`: the names `UdpSocket`, `TcpStream`, `TcpListener`, `ToSocketAddrs`, `to_socket_addrs`, `lookup_host`, `tokio::net`, `quinn`, `socket2`, `mio`, `reqwest`, `ureq` and `hyper` occur only in the network-access type's file; the names `tessera_net::transport`, `tessera_net::remote`, `std::process` and `tokio::process` occur nowhere in the crate, test code (`#[cfg(test)]` modules and `crates/compat/tests/`) included: test code is not exempt from this rule (a test that needs a FIFO makes it without starting a process); the crate's `Cargo.toml` lists no other network-I/O dependency; address types (`SocketAddr`, `SocketAddrV4`, `IpAddr`, `Ipv4Addr`, `Ipv6Addr`) are not scanned, since they open nothing; every `CompatBackend` constructor takes a proof. (c) Source scan of the rest of the workspace: `CompatBackend` and `Connector` implementations are named only in the client's one construction site (§6.2). (d) A compile-fail test: building a `CompatBackend` without a proof does not compile. (e) Manifest check: `tessera-compat`'s test-support feature (§6.1) is enabled only by `fuzz/` and dev-dependencies, never by a normal dependency | All five hold |
| OE-T9 | **Placeholder.** Availability is `NotBuilt` with only the placeholder; the flow shows the not-available message after the warning and never shows the form. A placeholder backend built with a proof from an acceptance written in a temporary directory answers `Connect` with `LoginRefused { NotAvailable }` then one `Disconnected` in the same poll, without a socket | All hold |
| OE-T10 | **Credentials never persisted or logged.** Run the form, Log in (test-double backend), each refusal of §8.6, Back and window close with a canary account name and password, under a captured `tracing` subscriber at `trace` level and with a forced panic in the backend | The canaries occur in no log line, panic message, `Debug` output of any flow, form, config, backend or session value, settings file, record file or other file in the test's directories; the secret buffers and the keystroke-text values read back as zeros after hand-over and after Back; neither field offers copy, cut or paste; the masked entry's element text holds only bullets |
| OE-T11 | **Input rules** (device-input values from the test constructor). Press before 1,000 ms then release after; key held from before the screen; Tab during the delay; Enter with focus on Decline during the delay (declines at once); a click on Decline at 0 ms (declines); Continue before the box is ticked; Escape at 0 ms with focus on Continue; window close; then the keyboard path: Tab to the box, Space, Tab to Continue, Enter, all after the delay | Only the documented results of §5.4 occur: none of the first eight cases accepts (the Enter on Decline, the click on Decline, the Escape and the window close decline at once); only the ninth, the keyboard path after the box is ticked and the delay has passed, accepts |
| OE-T12 | **Decline and withdraw.** Decline with an older record present; withdraw with a valid record | Decline changes no file; withdraw removes both files and the next check gives `missing` |
| OE-T13 | **Session rules.** Account events in each session state; `ChooseWorld` with an id from an older list; out-of-bound lists; events after `LoginRefused`; Account commands on the sandbox and test-server backends | As §8.2; `session::name_tests` passes with the new group |
| OE-T14 | **Login presentation.** A test-double backend scripts progress, lists (ghosted rows, selection by last world, replacement), queue panels with and without numbers, every row of §8.6, the watchdog at 60 s on a virtual clock, retry waits | Screens, texts, targets and the remembered `last_world` follow §8.3–§8.6 |
| OE-T15 | **Failed write.** The record directory is read-only; a key file of 31 bytes; a key file that group or others can write (Unix) | Acceptance counts for the run, `.saved_failed` is shown, the next run's check gives not accepted; the bad key file is byte for byte unchanged |
| OE-T16 | **Key race.** A key file appears between the writer's check and its no-replace rename (injected) | The rename does not replace it; the writer uses it if valid, else fails closed |
| OE-T17 | **Windows** (`cfg(windows)`). Check and write with a temporary directory | The check gives not accepted without opening a file, the writer writes no file, an acceptance lasts for the run and `.not_stored` is shown |
| OE-T18 | **Linux machine identifier.** Binding values read with `/etc/machine-id` present, absent, empty, of 128 bytes and of 129 bytes (injected root) | The identifier comes from that file only (128 bytes accepted), else the host name (absent, empty, 129 bytes) |
| OE-T19 | **Account-name copy.** `BadCredentials`, then each other refusal row | After `BadCredentials` the name is refilled and the copy reads back as zeros; after every other row both fields are empty and the copy is zeros |
| OE-T20 | **Official character placeholder.** `Connected` from a test-double backend | The placeholder shows `official.characters.unavailable`; Back sends `Disconnect` and reopens the empty form; the plugin host is started 0 times |
| OE-T21 | **Start-screen conflicts.** `--start-screen` paired with each option of §4.2's list for it, and `--official` with `--screenshot` and each headless-task option | Each is a usage error, exit code 2 |
| OE-T22 | **Release features.** `tools/check-release-features.sh` on the workspace, and on a fixture manifest in which a normal dependency enables `tessera-compat`'s test-support feature | It passes on the workspace and fails on the fixture |
| OE-T23 | **Window close.** Close the window on each official screen of §4.6 | Each gives the table's result; only the start screen ends the client |
| OE-T24 | **Unrepaired key.** An invalid key file, then two acceptances in two runs, then Withdraw, then an acceptance | Both acceptances last for their run, the start screen shows the `.saved_failed` line; Withdraw deletes both files; the last acceptance writes a fresh key and record |

<a id="s10-2"></a>
### 10.2 Owner check

The owner looks at the start screen, the warning screen (fresh and changed) and the
not-available message in a windowed run and confirms the text and the input rules (a row for
`docs/checklists/owner-observations.md`).

<a id="s10-3"></a>
### 10.3 Fuzz targets

- `compat_consent_record`: arbitrary bytes as the record document and key file, with arbitrary
  binding values: no panic, bounded allocation, never accepted unless the code matches.
- `client_official_entry`: arbitrary sequences of Account events, `Connected` and `Disconnected`
  through the session into the official-entry presentation: no panic, bounded state.

---

<a id="s11"></a>
## 11. Invariants

1. No `CompatBackend` exists, and no network operation of `tessera-compat` happens, in a run in
   which no consent proof was made.
2. A consent proof exists only after a check that gave accepted for the current digest, or
   after an acceptance on the warning screen in this run.
3. A record whose `text_sha256` differs from the current warning digest is never accepted.
4. No file that Tessera writes contains an account name or a password; no log line does.
5. Declining, Escape and closing the window during the warning change no file.
6. With `--official`, every option of §4.2's conflict list is a usage error.
7. Account events are surfaced only in `Connecting`; after `LoginRefused` only `Disconnected`
   follows.
8. Every login attempt is started by the user's Log in; no backend resends credentials.
9. In a start-screen run, no plugin host runs while the start, warning, form or login screens
   or the official character placeholder show.
10. On Windows no consent file is written or read.
11. The writer never overwrites or deletes an existing key file.

---

<a id="s12"></a>
## 12. Open questions (summary)

| Where | Question | Who |
|---|---|---|
| §4.1 | Start screen as the default windowed start; test server on the start screen | Owner |
| §5.2 | Translations of the warning; re-prompt on a language change | Owner |
| §7.1 | The service's account-name and password rules | M8 login spec (once Accepted) |
| §7.1 | Remembering the account name | Owner |
| §7.3 | Login steps beyond name and password (second factor, challenge, agreement), and when | M8 login spec |
| §8.1 | World attributes the service reports and their mapping | M8 login spec |
| §8.1 | Queue position, length, wait and their rate | M8 login spec |
| §8.1 | Returning to the world list without a new authentication; later needs for the password | M8 login spec |
| §8.6 | Mapping of the service's refusals onto Tessera's categories, retry times | M8 login spec |

Implementation note, not a question: if decision 0041 open item 4 leads to a fresh
re-implementation of `tessera-compat`, the `consent` module and the network-access rule of §6.1
move into it unchanged.

---

## Change log

| Date | Change |
|---|---|
| 2026-10-23 | New spec (spec writer, design only; no dirty-room material, no M8 protocol spec, no observation). Ban-risk consent per decision 0034 §1 item 4: warning text as Tessera's own string-table entries, digest and revision, screen input rules, accept, decline, withdraw, acceptance record with an authentication code bound to OS user, machine and directory, honest limits of a local file. Gate: consent proof on every network entry point of `tessera-compat`, one construction site in the client. Start screen and `--official` (select only), availability and the placeholder. Account form with masking, memory lifetime and no persistence. Account group (`LoginProgress`, `WorldList`, `LoginQueueChanged`, `WorldRefused`, `LoginRefused`, `ChooseWorld`, `LeaveLoginQueue`) with session rules, world list, queue, errors and retries. Tests OE-T1–OE-T15, fuzz targets, invariants, nine OPENs. Status Draft → Review. |
| 2026-10-23 | **Revision 2** (spec writer, design only; answers review 1; status stays Review, awaiting an independent re-review). **Blocking:** (1) new §4.5: in a start-screen run the plugin host is not started before §8.7 (or the offline walker) and is stopped before the start screen returns; `--load-plugin` conflicts with `--official` (§4.2); §5.7 Plugins row rewritten; OE-T6(a), new OE-T7(d), invariant 9. (2) §5.4 rules 2–6: Tab is ignored during the delay; rule 5 "No shortcut accepts" (no default button or accelerator, Escape and window close decline); new rule 6: Enter or Space on the focused Continue, after ticking the box, is an explicit confirmation; old rule 6 is now rule 7; OE-T11 extended. (3) §5.7 configuration row and §5.8 qualified: host-name fallback and cloned system images can share a binding; Windows has no permission check in this revision (§5.6 too). **Non-blocking:** §6.1 the session module's UDP transport moves behind the network-access type, `CompatConfig` loses `Clone`/`Default` (§7.3); OE-T8(b) lists the scanned names and excludes address types; OE5 and §4.4 cite the placeholder error by its type and exact current text; §8.2 dropped Account events logged at `debug`, fields omitted, rate-limited, and bounds checked on the values as sent; §5.2/§5.3 paragraph breaks are two U+000A and enter the digest as such; §5.6 no file lock (atomic renames); §5.4 rule 1 and §2 say how device input is told apart (window-event handler only, test-only constructor); OE5 records the plugin-host and `CompatConfig` facts of the code; §4.4 marks the placeholder's `LoginRefused` as a change to today's behaviour; OE4 quotes `Command::Connect`'s documentation exactly; §5.8 states that input injected at the operating-system level cannot be told apart. |
| 2026-10-23 | Header only: status Review → Accepted after the independent re-review of revision 2 (spec-review:spec-m8-entry:1); Reviewer and Review record rows. |
| 2026-10-23 | **Revision 3** (spec writer, design only; no dirty material, no M8 protocol spec, no observation; status Accepted → Revised, awaiting an independent review). Answers the ten implementer OPENs of the clean implementation and its review and audit as Tessera rules, each as an OPEN (implementer) callout with an Answered block: (1) §5.6 "Windows": no record is stored on Windows, the acceptance lasts for the run, the warning shows every run, new string `official.consent.not_stored` (§5.2, §5.5), §5.8 bullet, what lifts it; the binding's OS account is the numeric user ID only. (2) §5.6: canonical form of the document (key order, line format, canonical decimal integers, basic strings without escapes, `accepted_at` as `YYYY-MM-DDThh:mm:ssZ`), re-rendered and compared byte for byte; the code covers the values as written; a missing or wrong-size key gives `wrong_binding_or_code`; an existing bad key file is never overwritten (fail closed); key written through a temporary file and a no-replace rename, race rule; Linux machine identifier from `/etc/machine-id` only. (3) §7.3, §8.6: a separate zeroed-on-drop copy of the account name only for the `BadCredentials` refill; every other row reopens the form empty. (4) §7.2: no paste (no clipboard dependency), typing password managers work, keystroke text inline and zeroed. (5) §8.7: official character placeholder with `official.characters.unavailable` and Back; the plugin host stays stopped (§4.5). (6) §4.1, §4.5: no way back from the offline walker; quitting it ends the client. (7) §4.2: `--start-screen` conflicts listed; `--official` also conflicts with `--screenshot`, `--start-screen` and every headless-task option. (8) new §4.6: the official screens are painted by Tessera's own code with the bundled font, not built from `tessera-ui` elements (§4.1, §5.4, §7.1, §7.2, §8.3, §8.6 follow). (9) §5.4 rule 2: Decline acts at any time by every route. (10) §6.1, §6.3: the acceptance-writer scan covers `crates/compat/src` (exempting the consent module's definitions and `#[cfg(test)]` code); record-building helpers crate-private behind a test-support feature that release builds never enable; the network scan covers `tessera-net`'s transport and remote modules and process spawning. §4.1 Withdraw line also shows for the run's in-memory acceptance. Tests: OE-T5, OE-T6(d), OE-T8(b)(e), OE-T10, OE-T11, OE-T15 extended; new OE-T16–OE-T21; invariants 10 and 11. No section number or anchor changed; new anchor `s4-6`. |
| 2026-10-23 | Header only: status Revised → Accepted after the independent review of revision 3 (spec-review:spec-m8-entry:1); Review record row; one table-formatting slip in §8.7 fixed (no wording changed). |
| 2026-10-23 | **Revision 4** (spec writer, design only; no dirty material, no M8 protocol spec, no observation; status Accepted → Revised, awaiting an independent review). Six OPENs answered as Tessera rules, each an OPEN (implementer/reviewer) callout with an Answered block: (1) OE-T11's result names the nine cases correctly: none of the first eight accepts, only the keyboard path does. (2) §4.6 "Closing the window": a close is the screen's way back inside the official flow, ends the client only on the start screen; the placeholder returns to the account form (build kept, reason given). (3) §5.6, §4.1: an invalid key file is never repaired; acceptances last for their run until Withdraw (which deletes both files) or a removal by hand; the start screen shows the `.saved_failed` line meanwhile (or `.not_stored` on Windows); OE-T24. (4) §5.6: `/etc/machine-id` read up to 128 bytes, longer falls back to the host name; OE-T18. (5) OE-T8(b): test code is not exempt from the `std::process`/`tokio::process` rule. (6) §6.3 "Release builds": plain `cargo build --release -p tessera-client`, checked by `tools/check-release-features.sh` over normal edges; OE-T22. New OE-T23 (window close). No section number or anchor changed. |
| 2026-10-23 | Header only: status Revised → Accepted after the independent review of revision 4 (spec-review:spec-m8-entry:1); Review record row. |
