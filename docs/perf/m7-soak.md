<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M7 soak and load results (`tessera-loadtest`)

| | |
|---|---|
| **Serves** | [ROADMAP](../ROADMAP.md) M7 exit criterion 1 (one-hour soak without desync or leak) and criterion 3 (hostile packets cannot crash the server); `docs/specs/test-server.md` §13 |
| **Tool** | `apps/loadtest` (`tessera-loadtest`), release build |
| **Status** | Three series of runs on 2026-10-07: the **first** with the tool as first written, the **second** after a review of the tool found checks that could not fail (see "What changed"), the **third** in the M7 exit check after the bot-side memory growth was traced and fixed in `tessera-net` (see "Third series"). Every timing and performance number is **INDICATIVE** (see "Conditions"). **Exit criterion 1 is met on one machine** (loopback and a container network); a run between two machines on a real LAN is still open (see the end) |

## Conditions

- **Machine:** the reference laptop of [decision 0010](../decisions/0010-reference-machine-laptop.md)
  (Intel Core i9-10885H, 16 threads, 31 GiB usable), Gentoo, kernel 7.2.5, rustc 1.99.0
  (pinned), release builds.
- **Not quiet:** builds and tests of the parallel M4.5, M5 and M6 work ran at the same time.
  The 1-minute load average reached 16–34 during the first series and 10.4 during the second
  (half the core count is 8), and several runs ran at the same time as each other. By
  decision 0010 and `test-server.md` §13.4 every timing, latency, CPU and traffic number is
  therefore **indicative**. The desync and leak verdicts do not depend on that mark.
- **Code:** branch `m7` at `9f5a0f7` plus uncommitted M7 work (protocol, server, test-server
  backend, load tool), so **no result here can be reproduced from a commit yet**. The second
  series ran binaries built at 07:17 CEST (`tessera-loadtest` sha256 `67cf5e9ace4b94b5…`,
  `tessera-server` `7fd30eafc694ef4e…`); the digest of their sources (sha256 of `git diff HEAD`
  plus the untracked files under `apps/loadtest`, `apps/server`, `crates/net` and
  `crates/protocol`) was `51d6711ec89f8dda…`, and no source file changed between the build and
  the runs. The first series ran an earlier, unrecorded state of the tool, which changed
  between its runs.
- **Network:** loopback only (in-process server on `127.0.0.1`, or a separate
  `tessera-server` process on `127.0.0.1`) in the first two series. The third series adds a
  run across a container network: the server on the host's Docker bridge address
  `172.17.0.1`, the bots in a container with their own network stack (`172.17.0.2`), so every
  packet crosses a virtual Ethernet link and the bridge. **No run between two machines yet.**
- **Bots:** `tessera_net::Session` over `RemoteBackend`, the client's own network path. They
  walk at 2 m/s and run at 6 m/s (the server's `WALK_SPEED`/`RUN_SPEED`) on random legs of up
  to 250 m inside a square of landblocks of region 0, stop, turn, teleport (half of the
  teleports "far"), change appearance and stance, chat (Say, Yell, Tell) and reconnect. The
  soak mix is the `test-server.md` §13.1 default: 70 % walking, one chat line per bot per
  30 s, 10 % of the bots reconnecting every 5 minutes, one teleport per bot per 10 minutes,
  one appearance (and, second series, stance) change per bot per 5 minutes.
- **Checks:** `test-server.md` §13.2 (desync, zero allowed) and §13.3 (bounded counts at
  every sample, return to the baseline within 10 s, throttle tables empty, memory trend after
  a warm-up of the larger of 10 % of the run and 5 minutes). The tool's readings where the
  spec is silent are the implementer notes of `test-server.md` §15.2.

## What changed in the tool between the series

A review of the tool (2026-10-07) found that several checks of the first series could not
fail. The second series uses the corrected tool:

| First series | Second series |
|---|---|
| 4 × 4-block area: with *R* = 2 no two points are more than *R* + 1 = 3 blocks apart, so no pair ever left interest range. "Far" teleports could not be far; the Ghost-on-distance check, the destroy on distance and the `InterestReset` of a far teleport were **never exercised** | 8 × 8 blocks (2*R* + 4) by default; far teleports go more than *R* + 1 blocks; the report counts pairs that left range and warns when none did |
| `cheat` could not fail: every correction counted, none judged; an accepted cheat held 200 ms was below the 1 s Position hold | Each cheat must be corrected back to within 1.5 m of the place before it (or dropped, in-process counter); an observer showing the refused target is a **Cheat** desync at once |
| Desync sampled at about 7.5 Hz (100 ms after each 33 ms tick) | Fixed 10 Hz schedule; the achieved rate is reported; below 9 Hz the run is invalid |
| Appearance only | Appearance and `STANCE`; new **Motion** kind (heading and gait while moving) |
| Missing ignored the §9.4 create budget | Missing allows for the budget (nearest first) and pauses while the in-process server is behind |
| Any system line during a teleport counted as a refusal | Only `teleport refused:` lines; chat rejections are counted and their lines not judged |
| A dense n² pair matrix (168 MiB at 1,024 bots inside the measured process) | Pair state only while a check runs, plus three bit sets (128 KiB each at 1,024 bots) |
| No self-measurement | Loop overruns and sampling rate reported; timing indicative above 5 % overruns |
| JSON without configuration, server version, per-sample load or episodes; seed as a float | All of these; the seed as a string |

## Third series (exit check, final build)

### What was wrong and what changed

The second series left the in-process hours failing the memory criterion while a separate
server process stayed flat. Short experiments (16 bots, in-process, release, same seed) found
the cause in `tessera-net`, not in any state the code kept:

| Experiment | Build | Memory after warm-up |
|---|---|---|
| All 16 bots reconnect every 20 s, 4 min | thread and runtime per session (second series) | +30.9 MiB; 22–38 threads, threads came and went with sessions |
| Same, `MALLOC_ARENA_MAX=1` | same | no trend, but swings of 60 MiB |
| Same | one network thread for every session | +0.11 MiB; 7 threads all the time |
| 2 bots reconnect every 15 s (the soak's pattern, fast-forwarded), 5 min | one network thread | **+28.2 MiB** in steps of 2.9 MiB per reconnection |
| Same, `MALLOC_MMAP_THRESHOLD_=131072` | same | +0.34 MiB, flat at 14.6 MiB |
| Same, appearance and stance changes every 15 s instead of reconnections | same | +0.07 MiB |
| Same as the 2-bot row | one network thread **and** one shared quinn endpoint | **+0.44 MiB** |

Each session had started its own thread, tokio runtime and quinn client endpoint. quinn gives
an endpoint a receive buffer of about 2.9 MiB (1,472-byte datagrams × 64 segments × a batch
of 32). Once glibc's dynamic mmap threshold has risen above that size, such buffers come from
the heap, and a freed one stays resident behind later small allocations, so every
reconnection left about 2.9 MiB until a high-water mark (the second series' "levels off" and
"keeps growing" readings were both this, at different reconnection rates). **Fixed in
`tessera-net`:** every session is a task on one network thread per process, and the sessions
share one client endpoint per address family (ARCH §19.4; regression test
`sessions_share_one_network_thread`). Also fixed during the exit check: the tool provisioned
accounts 8 at a time, which a server on a non-loopback address refuses (at most 4 handshakes
in progress per address, `test-server.md` §15.1), and the server's tick deadlines drifted
10 µs a second from `tessera-protocol.md` §11.1 (now `tick_start(k)`).

### Runs

Binaries built from the `m7` working tree at the exit check (uncommitted; `tessera-loadtest`
sha256 `7111f16e43b67565…`, `tessera-server` `a7b7279c5d7cc606…`), copied to the clean scratch
directory. S60c and L60c ran an intermediate build (one network thread, endpoint per
session). The laptop suspended for about six hours in the middle of S60c and L60c; the tool,
the server and QUIC use monotonic clocks only, which stopped too, so the hour counts as
measured. During S60d, L60d and H5d a full debug build and test run of the workspace loaded
the machine (load average 26–73).

| Run | Command |
|---|---|
| S60d | `tessera-loadtest --scenario soak --bots 16 --duration 1h --csv … --json …` |
| L60d | `tessera-server run` on `172.17.0.1:47130` (registration on, Argon2id at the floor, 10,000 logins per address per minute, `lockout_max_s` 30, `max_connections_per_address` 64, otherwise defaults and the built-in world), then in a container on the Docker bridge: `tessera-loadtest --server tessera://172.17.0.1:47130 --fingerprint <fp> --password-file <file> --register --server-pid <pid> --say-range 40 --scenario soak --bots 16 --duration 1h` |
| H5d | `tessera-loadtest --scenario soak --bots 16 --duration 5m --hostile --hostile-clients 8 --chat-interval 10s --teleport-interval 60s --reconnect-interval 60s --appearance-interval 60s --warmup-min 60s` (the H5-16/H5b mix and seed) |
| S60c, L60c | as S60d and L60d, intermediate build |

| Run | Sessions (reconnects) | Unexpected disconnects | Teleports (far) | Position comparisons (pairs left range) | Desync episodes | Chat certain deliveries ok / judged | Memory | Verdict |
|---|---|---|---|---|---|---|---|---|
| **S60d** (in-process) | 38 (22) | 0 | 93 (52) | 5,185,495 (240 / 240) | **0** | 13,649 / 13,649 | warm-up max 68.3 MiB, max after 68.6 MiB, no growth; counts back to baseline, throttle tables empty | **PASS** |
| **L60d** (container network) | 38 (22) | 0 | 94 (47) | 4,272,246 (236 / 240) | **0** | 13,434 / 13,434 | server process max 47.6 MiB, no growth; bot process max 10.7 MiB, no growth | **PASS** |
| H5d (in-process, hostile) | 24 (8) | 0 | 80 (38) | 342,050 (240 / 240) | **0** | 3,376 / 3,376 | +1.49 MiB (max 72.6 MiB); counts back to baseline | **PASS** |
| S60c (intermediate) | 38 (22) | 0 | 95 (51) | 5,007,121 (240 / 240) | 0 | 13,689 / 13,689 | +28.4 MiB in 2.9–5.8 MiB steps at each reconnection round, flat after 2,100 s | FAIL (memory) |
| L60c (intermediate, container network) | 38 (22) | 0 | 110 (46) | 4,394,319 (240 / 240) | 0 | 13,437 / 13,437 | server process −1.47 MiB (flat); bot process +25.6 MiB (reported, not judged) | PASS |

Every desync kind (position, rest, missing, ghost, state, motion, cheat, chat, correction) had
zero episodes in every run; no teleport was refused or fell short, no chat line was rejected,
late, lost, duplicated or out of order, 0 panics. Position error p50 / p99: S60d 0.084 /
0.507 m, L60d 0.087 / 0.492 m; desync sampled at 10.00 Hz. Interpolation delay 167 ms. L60d's
server log has no warning or error other than tick overruns.

H5d: 1,924 attacks (bad-login 201, control-flood 185, datagram-flood 203, handshake-hold 199,
out-of-order 184, oversized-frame 187, stream-garbage 185, truncated-frame 185, udp-garbage
195 with 12,480 raw datagrams, uni-stream 200); the server closed 1,249 offenders with
`PROTOCOL_VIOLATION` and 121 with `AUTH_TIMEOUT`; honest bots unaffected. The same mix failed
the memory check in the second series (H5b, +18.6 MiB).

INDICATIVE (machine loaded, see above): login p50 233 ms (both hours), world entry p50 33 ms,
ping RTT p50 0.40 ms (S60d) and 0.44 ms (L60d), chat delivery p50 34 / 33 ms, per bot about
1.36 kB/s up and 4.1–4.6 kB/s down; S60d server 164 overruns in 108,000 ticks (longest
133 ms) under load average up to 73, and Missing was not judged in 2,537 sampling rounds
while the server was behind (`test-server.md` §15.2); process CPU 6 % of one core (S60d).

### Two clients on screen

A `tessera-server` on loopback with two accounts and one character each (a Human male and a
Hobbit female body, character files written by hand), and two release `tessera-client --server`
sessions on the owner's install: one headless for 120 s, the other taking `--screenshot`
images at the default spawn (region 1, block (126, 140)). The screenshots (clean scratch
directory, not committed) show the other player's character and the default world's three
wanderers (which copy the first player's appearance) drawn and animated around the avatar,
the network HUD line (`InWorld`, RTT, clock offset, `interp 167 ms`, `4 remote`) and the
server's message of the day in the chat overlay. Chat between the two clients is covered by
`apps/client/tests/net_server.rs` (both clients, overlay and plugin path).

## Second series (corrected tool)

### Command lines

All from copies of the release binaries in the clean scratch directory.

| Run | Command |
|---|---|
| S60b | `tessera-loadtest --scenario soak --bots 16 --duration 1h --csv … --json …` |
| A60b | `MALLOC_ARENA_MAX=2 tessera-loadtest --scenario soak --bots 16 --duration 1h …` (other account and name prefixes) |
| R60b | a separate `tessera-server run` on `127.0.0.1:47125` (registration on, Argon2id at the floor, 10,000 logins per address per minute, `lockout_max_s` 30, otherwise defaults: one instance of 64, the built-in world with its 3 wanderers), then `tessera-loadtest --server tessera://127.0.0.1:47125 --fingerprint <fp> --password-file <file> --register --server-pid <pid> --say-range 40 --scenario soak --bots 16 --duration 1h` |
| H5b | `tessera-loadtest --scenario soak --bots 16 --duration 5m --hostile --hostile-clients 8 --chat-interval 10s --teleport-interval 60s --reconnect-interval 60s --appearance-interval 60s --warmup-min 60s` (the H5-16 command and seed) |
| HR15 | the R60b server again, then `tessera-loadtest --server … --register --server-pid <pid> --say-range 40 --scenario soak --bots 16 --duration 15m --hostile --hostile-clients 8 --chat-interval 10s --teleport-interval 60s --reconnect-interval 60s --appearance-interval 60s --warmup-min 5m` (the H15-16 mix against a separate server) |
| W128, W256, W512 | `tessera-loadtest --scenario walk --bots N --duration 60s` |

S60b, A60b and R60b ran at the same time (05:22–06:22 UTC); H5b and the short checks ran
during them; W128–W512 ran just before them; HR15 ran after them (06:25–06:40 UTC).

### Desync, chat and sessions

| Run | Measured | World entries | Planned reconnects | Unexpected disconnects | Teleports (far) | Appearance / stance changes | Position comparisons (pairs; pairs that left range) | Desync sampling | Desync episodes | Chat certain deliveries ok / judged |
|---|---|---|---|---|---|---|---|---|---|---|
| S60b | 3,600 s | 38 | 22 | 0 | 95 (56) | 189 / 189 | 4,849,107 (240; 240) | 10.00 Hz | **0** | 13,605 / 13,605 |
| A60b | 3,600 s | 38 | 22 | 0 | 94 (49) | 189 / 189 | 5,220,857 (240; 238) | 10.00 Hz | **0** | 13,665 / 13,665 |
| R60b | 3,600 s | 38 | 22 | 0 | 110 (42) | 188 / 188 | 4,936,612 (240; 238) | 10.00 Hz | **0** | 13,448 / 13,448 |
| H5b | 300 s | 24 | 8 | 0 | 80 (38) | 78 / 78 | 342,560 (240; 240) | 10.00 Hz | **0** | 3,355 / 3,355 |
| HR15 | 900 s | 44 | 28 | 0 | 236 (121) | 238 / 238 | 803,609 (240; 240) | 10.00 Hz | **0** | 9,683 / 9,683 |

Zero episodes of every kind (position, rest, missing, ghost, state, motion, cheat, chat,
correction); no far teleport fell short, no teleport was refused or unanswered, no chat line
was rejected, late, lost, duplicated or out of order, no session failed to close, no panic.
Position error p50 / p99: S60b 0.089 / 0.512 m, A60b 0.087 / 0.497 m, R60b 0.075 / 0.459 m
(the maxima of about 1.2 km are single samples at a far-teleport snap). Interpolation delay
167 ms (`tessera-protocol.md` §17.3 OPEN note). R60b's only coverage note: the bots' world
instance is unknown to the tool (one instance here, so it does not matter).

### Latency, traffic and server (INDICATIVE)

| Run | Login p50 / p99 | World entry p50 / p99 | Ping RTT p50 / p99 | Chat delivery p50 / p99 | Per bot up / down (UDP) | Server ticks, overruns, longest tick |
|---|---|---|---|---|---|---|
| S60b | 66 / 66 ms | 33 / 33 ms | 0.14 / 0.24 ms | 33 / 66 ms | 1,345 / 4,480 B/s | 108,001, 0, 3.2 ms |
| A60b | 67 / 99 ms | 33 / 33 ms | 0.14 / 0.25 ms | 33 / 66 ms | 1,360 / 4,613 B/s | 108,001, 0, 2.9 ms |
| R60b | 66 / 66 ms | 33 / 33 ms | 0.15 / 0.27 ms | 33 / 66 ms | 1,358 / 4,478 B/s | 0 overruns, longest 2.2 ms (server stats line); no warning or error logged |

The ping RTT is now recorded once per measurement (the backend pings every 2 s); the first
series recorded the latest value every second, so each ping counted about twice. Process
CPU: S60b 183 s over 3,600 s (5 % of one core, server and bots), R60b bots alone 119 s (3 %).
Peak accounted server memory 39 MiB (38 MiB of it the Argon2id reservation).

### Leaks

| Run | Bound violations | Back to baseline (now including cached character records) | Throttle tables | Memory |
|---|---|---|---|---|
| S60b (in-process) | 0 | yes | empty | **GROWING**: warm-up max 74.9 MiB, then 78 → 114.5 MiB by 2,880 s, flat to the end (fitted growth 47.4 MiB > 16 MiB) |
| A60b (in-process, `MALLOC_ARENA_MAX=2`) | 0 | yes | empty | **GROWING**: warm-up max 45.1 MiB, 51 → 82.2 MiB, still rising at the end (38.7 MiB) |
| R60b, server process | 0 (bot sessions) | yes (server stats line: 0 connections at the end) | not checked (remote) | **ok**: warm-up max 47.8 MiB, growth 0.09 MiB (0.002 MiB/min), max 48.0 MiB |
| R60b, bot process | — | — | — | grew 8 → 51 MiB (fitted 37.9 MiB; reported, not judged) |
| H5b (in-process, hostile, reconnects every 60 s) | 0 | yes | empty | **GROWING**: warm-up max 73.9 MiB, growth 18.6 MiB > 16 MiB, max 92.0 MiB |
| HR15, server process (hostile) | 0 (bot sessions) | not checked (remote) | not checked (remote) | **ok**: warm-up max 48.9 MiB, growth 0.09 MiB (0.009 MiB/min), max 49.0 MiB |
| HR15, bot process | — | — | — | grew 33.1 → 63.9 MiB (fitted 29.4 MiB; reported, not judged) |

**Diagnosis (indicative):** the server's own process stays flat for an hour (R60b), and every
server count stays bounded and returns to its baseline. The process that holds only the bots
grows by about 0.7 MiB/min in steps (R60b), so the in-process runs, which measure server and
bots together, fail §13.3 criterion 3. The first series' reading, that the growth "levels off
after about nine reconnection rounds" and goes away with `MALLOC_ARENA_MAX=2`, **did not
hold**: A60b grew with that setting too, and R60b's bot process was still growing at the
end. The tool's own per-bot state is bounded (trajectories, histories, chat ledger, pair
state), so the likely place is `tessera-net` (per-session network threads and runtimes, or
per-entity state now that entities are created and destroyed far more often), but this has
not been profiled. It is a finding for the `tessera-net` owner (`test-server.md` §15.2).

### Hostile mode (exit criterion 3)

H5b ran the same seed as H5-16 below: 1,943 attacks (bad-login 207, control-flood 187,
datagram-flood 204, handshake-hold 199, out-of-order 184, oversized-frame 188,
stream-garbage 188, truncated-frame 190, udp-garbage 195 with 12,480 raw datagrams,
uni-stream 201); the server closed 1,265 offenders with `PROTOCOL_VIOLATION` and 121 with
`AUTH_TIMEOUT`; 0 panics, 0 server overruns, honest bots unaffected (0 desync, 0 unexpected
disconnects, 3,355 / 3,355 chat deliveries), every count back to its baseline, throttle
tables empty. Its in-process memory failed criterion 3 as above.

HR15 ran the H15-16 mix against the separate server: 6,467 attacks (bad-login 691,
control-flood 689, datagram-flood 640, handshake-hold 603, out-of-order 618, oversized-frame
627, stream-garbage 637, truncated-frame 638, udp-garbage 674 with 43,136 raw datagrams,
uni-stream 650); the server closed 4,240 offenders with `PROTOCOL_VIOLATION` and 317 with
`AUTH_TIMEOUT`; 0 panics, no warning or error in the server's log, honest bots unaffected
(0 desync, 0 unexpected disconnects, 9,683 / 9,683 chat deliveries), and the **server
process's memory flat** (0.09 MiB growth). Verdict PASS.

### Other scenarios (short checks, 25 s, 8 bots unless noted)

| Scenario | Result |
|---|---|
| `cheat` (2 of 8 bots send 40 m jumps every 5 s) | 16 cheats, 16 corrected back to the place before them, 0 unanswered, 0 Cheat episodes, 0 desync. The integration test `tests/cheat.rs` (6 bots in one block, so every observer watches every cheat) passes the same way; a server that accepted the jumps would now fail on the missing corrections and on observers showing the target |
| `churn` | 0 desync, 74 / 74 chat deliveries |
| `connect-storm`, 100 bots at once | 100 world entries, 0 admission refusals, login p50 689 ms / p99 1.3 s, 0 desync |

Short runs in a walking-only scenario usually end before any pair walks out of range; the
report says so as a coverage note.

### The tool's own limit (INDICATIVE)

The tool runs every bot and the desync checks on one thread (`walk`, 60 s, in-process,
load average 3.5–10):

| Bots | Loop iterations over the 33 ms tick | Longest iteration | Desync round p99 | Achieved sampling | Verdict |
|---|---|---|---|---|---|
| 128 | 0 % | 27 ms | 20 ms | 10.00 Hz | PASS |
| 256 | 37 % | 95 ms | 62 ms | 10.00 Hz | PASS, timing indicative (bots polled at about 23 Hz instead of 30) |
| 512 | 45 % | 492 ms | 242 ms | 5.91 Hz (87 rounds skipped) | **invalid** (the server also fell behind: 650 overruns, 691 skipped ticks; Missing not judged in 239 rounds) |

So on this machine the tool is sound up to about 128 bots, usable with indicative timing up
to 256, and reports itself invalid above that. The first series' 512-bot attempt reported
1,515–4,652 "missing" episodes instead; they came from the tool's saturated loop and the
server's create budget, not from interest management.

## First series (tool as first written; kept for the record)

Runs S10-2, S10-32, S60-16 (in-process soaks of 10 min and 1 h), H5-16 and H15-16 (hostile),
XA, XB, XC (10-minute experiments on reconnects, hostile mode and `MALLOC_ARENA_MAX=2`),
R60-16 (separate server) and A60-16 (in-process with `MALLOC_ARENA_MAX=2`), on a 4 × 4-block
area. Every one reported 0 desync and 0 unexpected disconnects. **Corrections to what this
section first said:**

- **Interest leave was never tested.** The bots did not teleport "out of interest range":
  in a 4 × 4 area no pair was ever more than 3 blocks apart. Every run compared all pairs
  all the time (S10-32: 992 = 32 × 31 pairs; S60-16: 240 = 16 × 15).
- **The `cheat` result proved nothing.** "16 cheats, 16 corrections" was what the tool
  printed, but it did not relate corrections to cheats, and "no observer ever saw a refused
  position" was not something the tool measured.
- **Desync was sampled at about 7.7 Hz**, not 10 Hz (S60-16: 6,682,278 comparisons / 240
  pairs / 3,600 s).
- **H15-16 failed the memory criterion** (warm-up max 93.0 MiB, max 109.7 MiB: GROWING), as
  did S60-16 (fitted growth 33.5 MiB) and XA. H5-16 passed it narrowly (15.0 MiB; the same
  command and seed with the corrected tool, H5b, does not). Memory stayed bounded only in XB
  (hostile, no reconnects: 74.9 → 75.1 MiB) and XC (`MALLOC_ARENA_MAX=2`).
- **R60-16 was reported FAIL by the tool at run time** (it judged its own, bot-only
  process). It was re-judged in this document after the rule changed, not re-run. R60b
  above is the re-run with the current rule; the server's memory passes there on the tool's
  own verdict.
- **A60-16's pass did not reproduce** with the corrected tool (A60b fails).

Their other numbers (indicative): login p50 66–169 ms; ping RTT p50 0.14–0.15 ms; chat
p50 33 ms; per-bot traffic 1.4 kB/s up, 6.4–13.3 kB/s down; 0 to 3 server overruns per run;
in the hostile runs 1,940 (H5-16) and 6,348 (H15-16) attacks with 0 panics. Their raw
reports are in the scratch directory of that session, not in the repository.

## Exit criterion status

ROADMAP M7 criterion 1: clients on a LAN see each other move, animate and chat for a
one-hour soak without desync or leak.

- **Desync: met.** Zero episodes of every kind in every one-hour run of the second and third
  series, with interest leave exercised for 236–240 of 240 pairs and 10 Hz sampling.
- **Leak: met.** S60d (server and bots in one process) and L60d (server process and bot
  process measured separately) pass `test-server.md` §13.3 with the final build: no memory
  growth after warm-up, every count bounded and back to its baseline. The second series'
  failures were the bot-side endpoint and thread churn fixed above.
- **Move, animate, chat:** moves and chat are judged by the tool (0 episodes, every line
  delivered); heading, gait and stance are compared (Motion and State kinds, 0 episodes), but
  `test-server.md` §13.2 does not define an animation check yet (§15.2 OPEN). On screen: the
  two-client screenshots above.
- **LAN: met on one machine only.** L60d crossed a container network (separate network
  stacks on a virtual Ethernet bridge), not a physical LAN between two machines. That run is
  left for the owner: start `tessera-server run` with `--listen <LAN address>:<port>` and
  registration on one machine, and `tessera-loadtest --server …` (or two
  `tessera-client --server …`) on another.
- Runs are from an uncommitted working tree; re-run S60d and L60d from the merge commit to make
  them reproducible.

ROADMAP M7 criterion 3 (hostile packets cannot crash the server): **met.** No crash and
honest bots unaffected in every hostile run (H5-16, H15-16, XB, XC, H5b, HR15, H5d: 0 panics,
0 desync, 0 unexpected disconnects, counts back to baseline); with the final build the
in-process hostile run also passes the memory check (H5d).
