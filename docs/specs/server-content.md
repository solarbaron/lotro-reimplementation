<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Server content rules

| | |
|---|---|
| **Status** | Draft. Needs the project owner's confirmation before any public test build ([LEGAL.md §11.1](../LEGAL.md#111-servers)) |
| **Last updated** | 2026-10-06 |
| **Author role** | Spec writer (policy and design; no data observed, no dirty-room input) |
| **Reviewer** | none yet |
| **Implements into** | `apps/server` (`tessera-server`: content loader, `check-content`, data-directory guard, default world), `apps/loadtest` (bots use only what this spec allows), `tools/check-content.sh` (repository guard, unchanged) |
| **Companion documents** | [LEGAL.md](../LEGAL.md) §4.1, §7, §8, §10, §11, §11.1, §12, §16. [Decision 0003](../decisions/0003-game-data-in-place.md), [0004](../decisions/0004-server-strategy.md), [0012](../decisions/0012-numeric-constants.md), [0030](../decisions/0030-tessera-protocol-transport.md). `tessera-protocol.md` (wire format, §0, §7.4, §9.1, §15 invariant 9). `test-server.md` (server behaviour and the content-file format). `coordinates.md` §4 and §7 |

> **NOT LEGAL ADVICE.** Like [LEGAL.md](../LEGAL.md), this is engineering policy written by
> non-lawyers. It turns LEGAL.md's rules into checkable rules for one program. Where it
> decides a question LEGAL.md leaves open, it says so and marks the point for the owner.

---

<a id="s0"></a>
## 0. Purpose and scope

This spec says what content a Tessera server may hold, use and send, and how a server may be
operated. It applies to `tessera-server` and to **any other server build** that speaks the
Tessera protocol, whoever runs it. It lets:

- the server implementer build the content loader, the `check-content` command and the
  data-directory guard (§10);
- contributors know what they may commit as server content, and reviewers check it (§9);
- operators know how they may run a server (§8).

The rules here are the project's position at tier **T2** of [LEGAL.md §8](../LEGAL.md#8-risk-tier-ladder):
a project-defined protocol, a project-owned server, original or test content. Nothing here
concerns the official servers or their protocol. That work is gated
([decision 0004](../decisions/0004-server-strategy.md), LEGAL.md §9.3).

Keywords **MUST**, **MUST NOT**, **SHOULD**, **MAY** as in RFC 2119 and RFC 8174.

---

<a id="s1"></a>
## 1. Provenance

Legend: **Policy** = restates a binding rule of LEGAL.md or a decision record. **Design** =
decided by this spec (normative for Tessera servers, subject to the owner's confirmation).
**Project** = an existing fact of another Tessera spec or crate.

| # | Fact or rule | Source | Mark |
|---|---|---|---|
| C1 | No game content in the repository, CI artefacts or releases: archives, extracted assets, strings, quest or dialogue text, maps, item/quest/deed databases, Lua UI files | LEGAL.md §11; decision 0003 | Policy |
| C2 | No bulk data tables from the client; numeric algorithm constants allowed when a spec records them | LEGAL.md §7 item 4, §11; decision 0012 | Policy |
| C3 | T2 = project-defined protocol, project-owned test server, only original or test content; lower risk "if the protocol is our own and the server holds no game content" | LEGAL.md §8 | Policy |
| C4 | T3 (quests, NPC scripts, combat or world reproduced by a server, or **any publicly hosted shard**, free or not) is not planned without written permission | LEGAL.md §8; ROADMAP §3 | Policy |
| C5 | Free of charge, no paid tiers, perks, ads or sales; **no hosted public game servers** | LEGAL.md §12, §8 permanently-forbidden list | Policy |
| C6 | Naming: no marks or Tolkien-invented words as product, binary or feature names; the game named only in plain text to state compatibility | LEGAL.md §10 | Policy |
| C7 | Distributing files that *describe* protected audiovisual content can make a derivative work (*Micro Star v. FormGen*); a tool that alters display without incorporating the work is not one (*Galoob*) | LEGAL.md §4.1 (summaries) | Policy (as LEGAL.md reads them) |
| C8 | Extracting or re-using a substantial part of a database can infringe the EU sui generis right | LEGAL.md §5 (Database Directive 96/9/EC) | Policy |
| C9 | The protocol carries the data's records only as opaque numbers that each client resolves against its own install; the server never sends static objects or game data | `tessera-protocol.md` §0, §7.4, §9.1, §15 invariant 9 | Project |
| C10 | A region number is a key of the data's region map; a landblock is a 160 m square, indices 0–254; region 0 means "no region" | `coordinates.md` §4; `tessera_net::model` | Project |
| C11 | The repository already contains bare numeric positions without game text: the walker's default spawn `1,126,140` (`walker.md` §1 option table, `coordinates.md` §8.4 example B) and twenty reference positions named only by DIDs (`docs/checklists/m3-reference-locations.md`) | Read 2026-10-06 | Project |
| C12 | The engine must run with original placeholder content so it has uses that do not depend on game data | LEGAL.md §11 last paragraph | Policy |
| C13 | Everything else in this spec: definitions, the coordinate rule, the relay rules, the operator rules, the commit rules and the checklist | This spec, 2026-10-06 | Design |

No game data was read for this spec.

---

<a id="s2"></a>
## 2. Terms

| Term | Meaning here |
|---|---|
| **Game data** | Anything that comes from the game's installation or its publisher: the archives and any part of them, records decoded from them, extracted textures, meshes, animations, sounds, music, fonts, UI art, strings and any text (names of places, people, items, quests; dialogue; descriptions), quest and NPC logic, item, quest, deed, NPC, loot or map databases, Lua UI files, client binaries, keys and hostnames |
| **Opaque reference** | A number that identifies a record or an enum value **in a client's own install**: a DID (32-bit data ID), a species key or sex value, an animation key, a starting-idle key, a dye code. On its own it carries no expression and no data. Only a program with a copy of the data can turn it into anything |
| **Position** | A region number plus coordinates (or a landblock index plus a block-local offset), as in `coordinates.md` §4 and §7 |
| **Server content** | Everything a server holds or sends that is not produced by its players: the world definition (spawn points, simulated actors, message of the day, server name), and any data files it ships with |
| **Content file** | The file that defines a server's world (`test-server.md` §5) |
| **Player-supplied value** | A value a player's client sends during play: account and character names, chat text, appearance, emote and stance keys, movement, teleport destinations |
| **Operator** | Whoever runs a server process |
| **Private server** | A server whose accounts the operator creates for people they know, which is not listed, advertised or open to registration by strangers (§8) |

---

<a id="s3"></a>
## 3. The server holds and serves no game data

<a id="s3-1"></a>
### 3.1 Rule

A Tessera server **MUST NOT** hold, read, cache, derive or send game data (§2). In
particular it MUST NOT:

1. open, read, copy, mirror or index any archive (`.dat`, `.datx`) or any other file of a
   game installation, the operator's own included;
2. contain or load extracted assets, strings, quest text, dialogue, NPC scripts, item, quest,
   deed, NPC, loot or map databases, or tables built from any of them;
3. send static world objects, landblock contents, terrain or any record content. Each client
   renders the static world from its own install (`tessera-protocol.md` §9.1);
4. resolve an opaque reference to anything (a name, a mesh, a description, a category). The
   server treats every reference as an uninterpreted number (§4);
5. hard-code or load hostnames, keys or tables from the client binary (C2). The server's own
   key is generated per installation (`tessera-protocol.md` §4.1);
6. ship with, download, or point users to game files, patches or clients.

<a id="s3-2"></a>
### 3.2 Why the server does not read the operator's install either

ARCHITECTURE §19.2 allows, in principle, "the server operator's own data limited to what
LEGAL.md allows". This spec decides that **v1 servers use none**:

- A server that loads game data to simulate the world (terrain, collision, placed figures)
  is the first step towards reproducing the game's world server-side, which is T3 (C4).
- Data on a server reaches players: whatever the server derives from it shapes what other
  people see. Keeping the server free of data keeps the line simple: every piece of game
  data a person sees comes from **their own** install, on their own machine (decision 0003).
- The protocol needs none of it. Movement is validated by speed and extent
  (`tessera-protocol.md` §9.3), not by terrain.

Consequences: the server has no terrain heights (a `Ground` destination keeps its given
height, `tessera-protocol.md` §8.4), no collision, no area names, and cannot check what a
reference means.

> **OPEN (owner):** whether a later version may let an operator point the server at **their
> own** install, read-only and in place, for terrain heights and collision checks only (no
> placed figures, no text, nothing sent to clients). It would need a decision record and
> this section revised. Until then the answer is no.

---

<a id="s4"></a>
## 4. Opaque references from players

<a id="s4-1"></a>
### 4.1 What is relayed

The protocol carries opaque references in appearances (body species key and sex value, or
an object DID; scale; starting-idle key; up to 32 equipped item DIDs with dye codes), in the
`EMOTE` property (an animation key) and in an entity's optional class DID
(`tessera-protocol.md` §5.3, §7.3–§7.5). For players, these are **chosen by each player's
client from that player's own install**: the client picks an outfit or an emote it can
resolve, and sends the numbers.

A server **MAY** store such references as part of a character (its appearance) and **MAY**
relay them to other clients, under these conditions:

| # | Condition |
|---|---|
| R1 | **Player origin.** The references the server stores and relays come from players' clients, as the protocol carries them. The server never invents, completes or "corrects" them from any list |
| R2 | **Uninterpreted.** The server checks only the protocol's limits (counts, ranges, non-zero) and its own policy switches that do not depend on meaning (for example: object bodies allowed or not, `test-server.md` §6.4). It never looks a reference up in anything |
| R3 | **Bounded per character.** At most what the protocol allows per appearance (one body, 32 items). Nothing else per reference is stored: no names, no counts, no "seen" lists |
| R4 | **No collection.** The server MUST NOT aggregate references across players or over time into a list, catalogue, index, statistics table or export (for example "every item DID players wore"). Logs MUST NOT dump appearances in bulk; a debug log line about one message is fine |
| R5 | **No publication.** The server's data directory (character files) is private operator state. Its references are never published, committed or offered for download |
| R6 | **No game text alongside.** A reference is never paired with a name, label or description from the game, in the server, its files or its logs |

<a id="s4-2"></a>
### 4.2 Why relaying a reference is not distributing content

- A DID or key is a small integer that names a record. It contains no part of the record:
  no pixels, vertices, samples or text. It is closer to a page number than to a page. A
  person without the data can do nothing with it.
- Each receiving client resolves the number **in its own, legally obtained install**, on
  its own machine. What it then draws is its own copy of data its user is entitled to use
  (with the contract caveats of LEGAL.md §3, which apply to every tier and are not changed by
  this rule). If the client cannot resolve the number, it skips it
  (`tessera-protocol.md` §7.4).
- The server performs no copying of the work, does not incorporate it, and adds nothing to
  it (C7, *Galoob*). It forwards a choice one player made to the other players, as a chat
  server forwards a sentence.
- The risk C7 names (*Micro Star*: files that *describe* protected audiovisual content, such
  as level files that arrange a game's art into new levels) arises when **someone authors and
  distributes** an arrangement of references that recreates or extends the protected work.
  That is why server **content** may not arrange the game's records into scenes, NPC
  placements or quests (§5.3, §6.3), and why committed content files may hold only a handful
  of references (§9.2). A player's outfit, chosen live and relayed live, is not such a file.
- Collection (R4) would turn relaying into building a database out of the data's
  identifiers, which edges towards C8. The no-collection rule keeps the server a relay.

> **OPEN (counsel):** LEGAL.md §16 lists no question about references relayed by a server.
> This reasoning should be added to the questions for counsel before any public test build
> (LEGAL.md §11.1).

---

<a id="s5"></a>
## 5. Numeric world positions

<a id="s5-1"></a>
### 5.1 Decision

**A small number of bare numeric positions MAY appear in server content, including content
committed to the repository, under the conditions of §5.2.** A bulk list of positions, any
position paired with game text, or positions that reproduce the game's placements are
**forbidden** (§5.3).

<a id="s5-2"></a>
### 5.2 Conditions for a position in server content

| # | Condition |
|---|---|
| P1 | **Numbers only.** Region number, coordinates or landblock index and offset, heading. Nothing from the game's text: no place, zone, region, town, building or NPC name, no map label, no `/loc` output pasted with its text |
| P2 | **Own names.** A position that needs a name (for example a spawn point that `Named` teleports understand) gets a name the contributor made up that does not refer to the game's world (`start`, `meadow-test`, `spawn-3`), and that obeys LEGAL.md §10 (no marks, no Tolkien-invented words) |
| P3 | **Own observation.** The numbers were read by the contributor from Tessera itself (the walker's position readout, its spawn grammar) or from the contributor's own install, never copied from a third-party map, wiki, database, guide or tool output |
| P4 | **Few.** At most **16** positions per content file, and the purpose of each is testing or a place to start (spawn points, teleport targets). A content file is not a gazetteer |
| P5 | **Not a reproduction.** No position marks where the game puts a quest, NPC, creature, item, resource node or other gameplay element, and no set of positions traces the game's routes, borders or layout |
| P6 | **Explained neutrally.** Comments next to positions describe them in the contributor's own words and only as needed ("flat open ground for movement tests"), never by quoting or naming the game's content |

<a id="s5-3"></a>
### 5.3 Reasoning

- A coordinate is a **fact**: the number a client uses to say "here". Facts are not
  protected expression (17 U.S.C. §102(b); *SAS Institute* on functionality, LEGAL.md §5),
  and a single number reveals nothing of the game's world to someone without the data.
- A position becomes content when it carries the game's **expression** (a place name, a
  label: that is game text, C1, and may be a mark, C6), or when many positions together
  **describe** the world (a map or location database: C1 forbids "maps" and databases; C8,
  the database right, protects substantial extraction; C7, an arrangement that describes
  the work). P1, P4 and P5 block those three paths. P3 blocks copying someone else's
  compiled database or map.
- The repository already holds such numbers (C11): the walker's default spawn and twenty
  reference positions named only by DIDs. A spawn point in a content file is the same kind
  of fact used the same way. Forbidding it would make a server's default world unusable with
  the walker's own default, without reducing risk.
- Players' teleport destinations are player-supplied values (§2). A client may compute them
  from its own install (`/loc` text, map coordinates, area names it resolves locally). The
  server receives only numbers (or a `Named` destination that only matches the content's own
  names, `test-server.md` §8.4) and keeps only each character's last position (§4.1 R3 applies
  by analogy: no collection of visited places).

> **OPEN (owner):** confirm the coordinate rule (§5.1–§5.2), including the limit of 16 per
> file, before any public test build. If the owner rules that no numeric positions of the
> game's regions may be committed, the default world (§6) moves its spawn to region 0 and
> nothing else in this spec changes.

---

<a id="s6"></a>
## 6. The default world

<a id="s6-1"></a>
### 6.1 Rule

The default world is the content a server uses when the operator configures none. It is
built into `tessera-server` and committed as `apps/server/content/default-world.toml`
(format: `test-server.md` §5). It MUST be:

1. **Original.** Written by Tessera contributors for the project, licensed
   GPL-3.0-or-later (the repository's licence, covered by `REUSE.toml`) or CC0-1.0 with its
   own REUSE annotation and licence text (LEGAL.md §11 "Allowed").
2. **Free of game text.** No place names, character names, item names, lore or quotations
   from the game or the books, and no marks in the world's title, server name, message of
   the day or spawn names (C6).
3. **Free of game data** (§3) and within the position rule (§5) and the reference rule
   (§9.2).
4. **Usable without data.** A client with no install (no-data mode, ARCH §20.1) and the load
   tester must be able to join it, move and chat (C12).

<a id="s6-2"></a>
### 6.2 Contents

| Item | Value |
|---|---|
| World title | A neutral project phrase, for example "Tessera test world" |
| Message of the day | Tessera's own words: what the server is for (testing), that it is not affiliated with any game publisher (the disclaimer's first sentence, LEGAL.md §10), and that each player needs their own legally obtained installation to see the world |
| Spawn points | `start` (default): region 1, landblock (126, 140), block-local (80, 20), height 440 m, on the ground: the walker's default block, from `coordinates.md` §8.4 example B (C11); the height is one terrain sample observed for `test-server.md` §5.7. `void`: region 0, the middle of the grid, for clients without data and for load tests. Both names are project-made (§5.2 P2) |
| Simulated wanderers | A few project-made actors walking circles around `start`, named "Wanderer 1", "Wanderer 2", ...; their appearance is copied from the first player who enters the instance (`test-server.md` §5.4 `first-player`), so the file holds **no** opaque references |
| Game clock | Starts at 0 and runs at rate 1; time of day is computed by each client from its own data (`tessera-protocol.md` §11.3) |
| Opaque references | None |

<a id="s6-3"></a>
### 6.3 What the default world never contains

Placed figures or objects chosen from the data, recreated scenes, NPCs with the game's names
or roles, quests, dialogue, vendors, combat, loot, or any position from §5.3's forbidden
list.

---

<a id="s7"></a>
## 7. Gameplay reproduction is out of scope (T3)

A Tessera server MUST NOT reproduce the game's gameplay or world logic. Out of scope, with
or without game data:

- combat, skills, effects, health and death, experience and levels, classes as gameplay;
- quests, deeds, dialogue, conversations, instances or encounters from the game;
- NPC, creature or monster behaviour, spawning or placement taken from the data or from
  knowledge of the game;
- items as gameplay (inventory, loot, vendors, crafting, trade, currency, auctions,
  housing);
- any server-side use of the data's scripts, tables or properties;
- server-side copies of the static world (§3.1 item 3).

**In scope** (T2) for a Tessera server: accounts local to the server; characters as a name,
an appearance of opaque references and a position; presence and movement; emotes and
stances as opaque keys chosen by clients; chat; the world clock as a number; teleports to
numeric or content-named positions; simple **project-made test actors** whose behaviour is
generic and defined by Tessera's own content (walking a circle, standing), not drawn from the
data or from knowledge of the game.

A feature request that crosses this line is answered by pointing at LEGAL.md §8 (T3 needs
written permission from Daybreak and Middle-earth Enterprises) and ROADMAP §3.

---

<a id="s8"></a>
## 8. Operator rules

These rules bind anyone who runs a Tessera server build (C4, C5). `tessera-server` states
them in its `--help` text and enforces what it can (`test-server.md` §2.1, §4.2).

| # | Rule |
|---|---|
| O1 | **Private or LAN use for testing and development.** A server is run for the operator and people they know, on a LAN, on loopback, or reachable by those people only |
| O2 | **No public hosting.** No server may be listed, advertised, or opened to registration by strangers. "Public" means anyone can find it and create an account, whatever the price (C4: "any publicly hosted shard, free or not"; C5). The project itself hosts no game servers |
| O3 | **No money.** No fees, subscriptions, donations tied to access, paid perks, ads or sales connected with a server (C5) |
| O4 | **Accounts are created by the operator.** Open registration (`REGISTRATION`, `tessera-protocol.md` §6.2.4) is off by default and is for loopback and LAN tests only (`test-server.md` §4.2) |
| O5 | **Each player brings their own legally obtained installation.** The operator never hands out game files, clients, patches or links to unofficial copies (LEGAL.md §7 item 6) |
| O6 | **Neutral names.** The server name and message of the day obey LEGAL.md §10: no marks or Tolkien-invented words, no implied endorsement |
| O7 | **No game data on the server** (§3), and only content that passes §9.3 |
| O8 | **No bridges.** A server never connects to, proxies, imitates or exchanges data with any official service, launcher or login (decision 0004) |
| O9 | **Players' data stays private.** Account files, character files and logs stay on the operator's machine. Chat is not logged unless the operator turns it on for debugging (`test-server.md` §11.2) |
| O10 | **Incidents.** If a rights holder objects to a server, the operator stops it; LEGAL.md §14 applies to the project |

> **OPEN (owner):** LEGAL.md's banner asks for a lawyer's review "before any networked feature
> beyond the local sandbox". This spec reads that as: contributors may build and run the
> server privately (O1) for development now, and any **public test build** (binaries or
> instructions offered to people outside the project for running servers) waits for that
> review and for the owner's confirmation of LEGAL.md §11.1. The owner should confirm this
> reading.

---

<a id="s9"></a>
## 9. What a contributor may commit as server content

<a id="s9-1"></a>
### 9.1 Allowed

- Content files in `apps/server/content/` (the default world and small test worlds) and
  synthetic content generated by tests at run time.
- Test fixtures for `check-content` that **violate** these rules on purpose, if they are
  made only of invented values (a made-up place name, an invented DID) and marked as negative
  fixtures. They MUST NOT contain real game text; the only exception is single words of the
  LEGAL.md §10 rule 1 list, needed to test the word check.
- Load-test scenarios (`apps/loadtest`) that use the default world or generated worlds.

Every committed content file starts with an SPDX header comment and is original.

<a id="s9-2"></a>
### 9.2 Opaque references in committed content

Committed content SHOULD hold no opaque references (the default world holds none, §6.2). A
file MAY hold **at most 8** when a test needs a specific body or item, if each one:

- was read by the contributor from their own install with Tessera's tools (P3 applies);
- is written as a bare number with a neutral comment in the contributor's words ("a body
  key", "a worn item"), never with the record's name or text (R6);
- is not part of an arrangement that recreates the game's figures, scenes or outfits
  (§4.2, §6.3).

Specs and tests outside server content follow the existing rules (C2, decision 0012).

<a id="s9-3"></a>
### 9.3 Review checklist for a content file

A reviewer who did not write the file checks every item. `tessera-server check-content`
(`test-server.md` §2.6) checks the items marked **[auto]**; the rest need a person.

- [ ] **[auto]** The file parses, has a known `format` version, and passes every range check
      of `test-server.md` §5.
- [ ] **[auto]** It starts with an SPDX licence header, and the licence is GPL-3.0-or-later
      or CC0-1.0 (CC0 also needs a REUSE annotation).
- [ ] **[auto]** At most 16 positions (§5.2 P4) and at most 8 opaque references (§9.2).
- [ ] **[auto]** Names and texts (title, message of the day, spawn names, actor names) are
      valid text classes, and none contains a word from the LEGAL.md §10 rule 1 list
      (case-insensitive match against that list, which is project policy, not game data).
- [ ] **[auto]** The file is not inside, and does not name a path inside, a game
      installation, and no `.dat`/`.datx` file sits next to it (§10).
- [ ] The content is original: written for Tessera, not copied from a game, book, wiki, map,
      guide, database or another project.
- [ ] No game text anywhere: names, labels, comments and the message of the day are the
      contributor's own words and do not name the game's places, people, items or events.
- [ ] Positions satisfy §5.2 P1–P6. The commit message or comments say how each was obtained
      (P3).
- [ ] Opaque references (if any) satisfy §9.2, and the file does not recreate a scene,
      figure set or placement from the game.
- [ ] Simulated actors behave generically (§7 "In scope"). Nothing reproduces gameplay.
- [ ] The server name, title and message of the day imply no affiliation and obey LEGAL.md
      §10.
- [ ] The commit carries the trailers CLAUDE.md requires (`Clean-Room-Role`, sign-off).

---

<a id="s10"></a>
## 10. Enforcement in the server

| # | Mechanism | Where |
|---|---|---|
| E1 | The server has no code that reads game archives. `tessera-server` MUST NOT depend on `tessera-dat`, `tessera-formats` or any crate that decodes game data. A dependency check in CI SHOULD assert this | `apps/server/Cargo.toml`; CI |
| E2 | At start-up and in `check-content`, the server refuses a data directory or content file that lies inside a directory containing `client_*.dat`/`.datx` files, or a directory that contains any `.dat`/`.datx` file | `test-server.md` §3.1 |
| E3 | `check-content` implements the **[auto]** items of §9.3. `run` performs the same checks (except the SPDX header, which only committed files need) on the content it loads and refuses to start on a violation | `test-server.md` §2.6 |
| E4 | Opaque references from players are validated only against protocol limits and meaning-free switches (R2); they are stored per character only (R3) and never logged in bulk (R4) | `test-server.md` §6.4, §11.2 |
| E5 | Registration is off by default; enabling it on a non-private address refuses to start (O2, O4) | `test-server.md` §4.2 |
| E6 | `tools/check-content.sh` keeps guarding the repository (extensions, magic bytes, marks in identifiers) | unchanged |

---

<a id="s11"></a>
## 11. Invariants

1. A `tessera-server` build links no crate that decodes game archives (E1).
2. Every byte a server sends is either protocol structure, a player-supplied value relayed
   under §4, or server content that passed §9.3. No byte comes from game data.
3. The default world contains no opaque references, no game text, and at most the two
   positions of §6.2.
4. A content file with more than 16 positions, more than 8 references, or a LEGAL.md §10
   rule-1 word in any text is rejected by `check-content` and by `run`. A missing SPDX header
   is rejected by `check-content`.
5. A server started with registration on and a non-private listen address does not start.
6. Nothing in a server's data directory lists references or positions across characters:
   each character file holds that character's own appearance and last position only.

---

<a id="s12"></a>
## 12. Open items

> **OPEN (owner):** confirm the coordinate rule and its limits (§5.3).

> **OPEN (owner):** confirm the reading of LEGAL.md's banner for private development
> servers versus public test builds (§8).

> **OPEN (owner):** a later, read-only use of the operator's own install by the server for
> terrain and collision (§3.2).

> **OPEN (counsel):** add relayed opaque references (§4.2) and numeric positions in content
> (§5.3) to LEGAL.md §16 before any public test build.

> **OPEN:** (M7 exit check, 2026-10-07, from the protocol review R.7) two text slips for the
> spec writer: §3.2 cites `tessera-protocol.md` §9.3 for movement validation, which is §8.3
> there; §6.2 calls the 440 m spawn height a terrain sample, while `test-server.md` §5.7
> gives the observed sample as 438.7 m and 440 m as 1.3 m above it.

---

<a id="s13"></a>
## 13. Change log

| Date | Change |
|---|---|
| 2026-10-06 | First draft (M7). Derived from LEGAL.md §4, §5, §7, §8, §10–§12; confirmed for Tessera servers in LEGAL.md §11.1 pending the owner |
