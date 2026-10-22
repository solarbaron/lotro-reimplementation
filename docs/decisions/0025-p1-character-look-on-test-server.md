<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# 0025 — P1 on M7's server: scenario overlay, character looks, held-item slots, creation and the new feed steps

**Status:** Accepted (independent review 2026-10-10, passed; see [Review](#review)). The three
items under [Open for the owner](#open-for-the-owner) stay open: this acceptance decides none
of them. `scenario-feed.md` revision 4, `tessera-protocol.md` v1.1 and `test-server.md` are
Accepted; `server-content.md` passed the same review and stays Draft for the owner. Before:
Proposed (spec writer, design, 2026-10-10).

Tessera-own design. No client fact, research note, research clone or decompiler output was
needed or used; the inputs are `docs/` only (decision 0024, `server-content.md`, the specs
named below).

## Context

Decision 0024 P1 has two exit criteria that involve M7's test server:

1. "A made character enters the sandbox, and on M7's server other clients see its look."
   M7's `Appearance` carries a body (species and sex), a scale, worn items and an idle, but
   none of the creation choices (styles, face layers, colours, height, sliders). Observers
   therefore see the race's default look (`character-screens.md` §7.1 OPEN, Q7).
3. "Equipping an item from a scenario changes the worn look in the walker; observers on M7's
   server see it through `AppearanceChanged`." But `scenario-feed.md` §12.1 forbids
   `--scenario` together with `--server`, and the scenario feed is a feature of the sandbox
   backend only.

Further gaps the P1 specs raised as OPENs:

- `Equipped` carries no slot, so an item that fits either hand cannot say which hand holds it
  (`character-panels.md` §7 OPEN).
- `scenario-feed.md` §9.5 and `tessera-protocol.md` Appendix A do not yet point to the
  normative creation rule of `character-screens.md` §7.1.1 (Q11).
- The plain sandbox ignores `CreateCharacter` (`character-screens.md` §8 OPEN); the feed has
  no step that removes a character (§7.2 OPEN there), none that scripts a stat value
  (`character-panels.md` §9 OPEN), and none that gives an item instance properties
  (`item-tooltips.md` §11 OPEN, Q16).
- `CreateCharacter` carries only an appearance and a class (`scenario-feed.md` §16 OPEN 1).

Binding frame: decision 0024 §2 (client half only; the server stays data-free and feed-free;
values scripted, never derived), `server-content.md` §4 and §7, and the server dependency
guard (`scenario-feed.md` §3.1, `tools/check-server-deps.sh`).

## Decision

### D1. Scenario overlay on the test-server backend (exit criterion 3)

`tessera-client --scenario FILE --server CONNECT` becomes valid and runs an **overlay run**
(`scenario-feed.md` §12.5): the session's backend is the test-server (`remote`) backend
wrapped by a new `tessera-net` backend, the **overlay**, which runs the scenario feed for the
local **Character and Inventory groups only**, while everything else (login, character list,
world entry, movement, other players, chat) stays with the server.

- **What the overlay may run.** A fixed subset of the file format: settings `title`, `tick`,
  `end-at`, `defaults`, `requires install`; declarations `select`, `item`, `look`; steps
  `bag`, `give`, `remove`, `quantity`, `equip`, `unequip`, `condition`, `wallet`, and
  `level`, `stat` for the player only, and `item-property`; `repeat`; `on` triggers on the
  inventory patterns; an `input` block with inventory forms and no `at pre` lines. Anything
  else is the static error `NotAllowedInOverlay` (exit 3) before anything connects
  (`scenario-feed.md` §12.5.1).
- **Routing.** Inventory-group commands go to the feed; every other command goes to the
  `remote` backend, which refuses the other new groups as before. The feed's events follow
  the remote events of the same poll.
- **Clock.** The feed's virtual clock starts at the poll that delivers the server's
  `WorldEntered`; overlay logs are not normative (they depend on the server's timing).
- **Worn look.** The player's base appearance is the server's (`WorldEnter`, then every
  `EntityAppearance` for the player). When the feed's equipment list (`scenario-feed.md`
  §5.5) changes and the server advertised `APPEARANCE_CHANGE`, the overlay emits no
  `AppearanceChanged` of its own; it sends `AppearanceSet` with the base appearance and the
  feed's equipment list. The server answers every `AppearanceSet` with `EntityAppearance`
  for the player (`tessera-protocol.md` §6.2.17), which the player sees as
  `AppearanceChanged`, and relays the same appearance to observers, who see it as
  `AppearanceChanged` of that entity. Without `APPEARANCE_CHANGE` the overlay emits the
  `AppearanceChanged` locally (own view only) and logs once that observers will not see it.
- **Bounds.** At most one `AppearanceSet` in flight and at most one per 250 ms (4 per
  second, far below the protocol's 50 control messages per second); changes in between are
  coalesced into the latest list; an answer that does not come within 10 s clears the
  in-flight mark with a warning. The feed's own limits (`scenario-feed.md` §11.1) apply
  unchanged.
- **Server side: nothing changes.** `tessera-server` does not link `tessera-net`, the feed or
  any data crate; the overlay lives in the client's process. `check-server-deps` is unchanged
  and still passes.

**Alternatives rejected.** (a) A scenario run by the server: breaks decision 0024 §2.3 and
`server-content.md` §7 (the server would hold scripted inventory). (b) A second, local
sandbox session beside the remote one, merged in the client: two sessions, two state
machines and an event merge in the client half, for no gain over one composite backend.
(c) Letting the walker send `AppearanceSet` itself on every `AppearanceChanged`: puts
protocol traffic in the client half, which must not know its backend (`scenario-feed.md` §0
item 3), and loses the coalescing bound.

**Server content scope.** The server receives nothing it does not receive today: an
`AppearanceSet` with a body, a scale, an idle and up to 32 opaque item DIDs, which
`server-content.md` §4 allows to be stored and relayed as opaque references. Bags,
items, quantities, durability, wallet, levels and stats exist only in the client and are
scripted (decision 0024 §4); the server never learns them, never checks that a worn item is
"owned", and runs no rule. Items as gameplay (`server-content.md` §7) stay out of the
server.

### D2. Character looks: creation choices in `Appearance` (exit criterion 1, Q7)

- **Model.** `tessera-net`'s `Appearance` gains an optional **look**: a list of 1–48
  **look entries** (data property ID, scripted or chosen value), strictly ascending by
  property ID. Value kinds: bool, int32, int64, float32 (finite), enum key, bitfield32,
  bitfield64, data ID, colour (RGBA) and string (0–128 bytes of printable ASCII,
  U+0021–U+007E). The encoded look is at most 768 bytes. The client's creation screen fills
  it with the values its controls write (`character-screens.md` §5.2, column "Writes").
- **Wire: protocol 1.1** (minor version 1, negotiated in `Hello`/`HelloAck`; the minimum
  supported minor stays 0). The look is appended as a last field to `CharacterCreate`,
  `WorldEnter`, `EntityCreate` and `EntityAppearance`, and as a list parallel to
  `characters` to `LoginOk` and `CharacterList`; `AppearanceSet` carries none (a look is
  fixed at creation in 1.1; a later minor may add a change path). Appending at the end of a
  message is the compatible change §3 already names; the `Hello` and `HelloAck` bodies stay
  frozen for major 1. A 1.0 peer never sees a look; a 1.1 client on a 1.0 server keeps the
  `character-screens.md` §7.1 rule (choices in its own settings, applied to its own figure).
- **Receiver rule (hostile peers).** A client applies a look entry only when the property is
  one that a creation control of the body's player avatar writes in **its own** install and
  the value kind matches that property's dictionary type; other entries are ignored with a
  rate-limited debug log. Look strings are never shown as text.
- **Test server.** Stores the look with the character (`characters/<account>.toml`), sends it
  wherever the appearance goes, never judges it (`server-content.md` §4.1 R2), never logs it
  in bulk (R4). A character file is at most 1 MiB.
- **Feed.** `CreateCharacter` gains `nationality: Option<u32>`; its `appearance` carries the
  look. The scenario format gains a `look` declaration, `look=` on `player`, `character`
  and `accept-character` (`look=asked` copies the command's look), and `nationality=` and
  `look=` on the input form `create-character` (answers `scenario-feed.md` §16 OPEN 1).
  `CharacterInfo` gains `look`. Class and nationality never go on the wire.

**Server content scope.** Property IDs are keys of the player's own install, the same kind of
opaque number as the DIDs and enum keys `server-content.md` §4 names; the values are the
player's own choices on Tessera's creation screen, meaningless without the install. The server
stores them per character (R3, now bounded: one body, 32 items, one look of at most 48 entries
and 768 bytes), relays them, and never interprets, completes, collects or pairs them with text
(R1, R2, R4, R6). Class and nationality are dropped because they have meaning only for the
game's rules (`server-content.md` §7 "classes as gameplay").

### D3. Held-item slot in `Equipped` (character-panels §7)

A slot field is added, not the Tessera rule confirmed: the rule cannot place a one-handed
item held only in the secondary hand. `Equipped` gains an optional `slot`, the data's
equipment slot key (1–64, `property-system.md` §7.3), opaque to the server. On the wire (1.1)
it is announced by flag bit 3 of `Equipped.flags` and follows the dye fields as a `u8`; a
1.0 peer never receives bit 3. The feed fills it from the equipment slot of each equipped
item (`scenario-feed.md` §5.5). The walker uses it when present and falls back to the
Tessera rule of `character-panels.md` §7 when absent (1.0 servers, other sources).
**Server content scope:** a slot key is an enum key (`server-content.md` §4), bounded by the
32 items.

### D4. Creation on the test-server backend (Q11)

`scenario-feed.md` §9.5 and `tessera-protocol.md` Appendix A adopt `character-screens.md`
§7.1.1 as the normative rule: `CreateCharacter` → `CharacterCreate` (class and nationality
dropped; the look appended under 1.1), `CharacterList` → `CharacterInfo` or
`CharacterRefused`, `CharacterCreateFailed` → `CharacterRefused`. In addition, after
`Connected` the backend emits one `CharacterInfo { name, body, class: None, level: None,
look }` per `LoginOk` entry, so the selection screen can show each character's figure.
`DeleteCharacter` and every other new command stay refused on that backend. No server-scope
change: these are the account and character messages M7 already has.

### D5. Plain-sandbox creation and the new feed steps (decision 0024 §4: scripted, never derived)

- **Plain sandbox** (no scenario): `CreateCharacter` with a valid, new name and fewer than 64
  characters adds the name and emits `CharacterInfo` with the command's body, class and look;
  `EnterWorld` with that name uses the command's appearance as the player's base appearance.
  A taken name or a full list gives `CharacterRefused` with Tessera's own text. In memory
  only, for the sandbox's lifetime. `DeleteCharacter` stays accepted and ignored.
- **`remove-character NAME`** (feed, in `on create-character` and `on delete-character`
  bodies): removes the name from the list and emits `CharacterRemoved { name }` (Character
  group); an unknown name is a skip.
- **`stat ENTITY PROPERTY=VALUE`**: emits `PropertyChanged { entity, property, value }`
  (Character group); `VALUE` `none` clears it.
- **`item-property $item PROPERTY=VALUE`**: emits `ItemProperty { item, property, value }`
  (Inventory group).
- `PROPERTY` is a dictionary label (`@Name`, resolved by the client's resolver, which also
  reports the property's value kind) or `pid:N:KIND` (number and kind written out; checked
  against the dictionary when an install is present). Values are literal: the feed computes
  nothing, and the dictionary only tells the client which kind a label has.
- **Bounds.** Strings 256 bytes (128 in looks), floats finite with |v| ≤ 10^9, integer
  ranges of their kind, at most 48 entries per look and 64 look bindings; the client keeps at
  most 1,024 properties per entity and per item and drops further ones with a warning
  (`scenario-feed.md` §9.5).
- **Server content scope:** all of it runs only in the client's sandbox or overlay; the server
  gains no message, state or rule.

### Hostile-input limits (summary; the specs are normative)

| Item | Limit | Exceeded |
|---|---|---|
| Look entries | 1–48 per look, property IDs non-zero and strictly ascending | Wire: protocol violation. Feed: `LimitExceeded(look)`; duplicates and order resolved with notes |
| Look size | 768 bytes encoded, count byte included | Wire: protocol violation. Feed: entries cut from the highest property ID, with a note |
| Look values | Kinds of D2 only; strings 0–128 bytes of U+0021–U+007E; floats finite | Wire: protocol violation. Feed: `BadValue` (static) or a `mismatch` note |
| `looks` in `LoginOk` / `CharacterList` | Count equal to `characters` | Protocol violation |
| `Equipped.slot` | 1–64, only with flag bit 3, bit 3 only at minor ≥ 1 | Protocol violation |
| Overlay `AppearanceSet` | One in flight, ≥ 250 ms apart, 10 s answer timeout | Coalesced; timeout clears the mark with a warning |
| Character file | 1 MiB | Refused when read, write refused |
| Feed property values | Strings 256 bytes; floats finite, \|v\| ≤ 10^9; integers within their kind | `OutOfRange`, `BadValue` or a note |
| Client-side property state | 1,024 properties per entity and per item | Further properties dropped with a warning |

### Conformance vectors

`tessera-protocol.md` §16.8 (1.1 frames and negative vectors), `scenario-feed.md` §14.5
(new hostile rows), §14.7 (Vector F updated for the new log fields), §14.8 (Vector G:
looks, character removal, stats, item properties), §14.9 (overlay cases) and §14.10
(plain-sandbox cases).

## Consequences

- **Specs revised (status Review; independent review required):** `scenario-feed.md`
  revision 4, `tessera-protocol.md` v1.1, `test-server.md` (character file look and slot,
  version negotiation, file size), `server-content.md` (§4.1 look and slot; stays Draft
  pending the owner). OPEN notes pointed to the new sections in `character-screens.md` §7.1,
  §7.1.1, §7.2, §8; `character-panels.md` §7, §9; `item-tooltips.md` §11. Facts of those three
  specs are unchanged.
- **Implementers:** `crates/protocol` (minor 1: look, look entries, `Equipped` bit 3, the
  appended fields, the new negative vectors), `crates/net` (model `Look`, `LookEntry`,
  `PropertyValue`, `Equipped.slot`, `Appearance.look`; new events and command field; the
  overlay backend; plain-sandbox creation; remote backend 1.1 and §7.1.1; feed steps,
  resolver methods, log forms, Vectors F and G), `crates/gamedata` (resolver: property labels
  and kinds, property IDs, enum labels through a property's mapper, nationality labels),
  `apps/client` (`--scenario` with `--server`; the creation screen fills the look and
  nationality; figures apply received looks under the receiver rule; the walker uses
  `Equipped.slot`), `apps/server` (negotiation up to minor 1, look storage, file limit),
  `apps/loadtest` (hostile 1.1 cases), `fuzz/` (protocol and scenario targets cover the new
  forms).
- **ARCHITECTURE §19.1** must list `CharacterRemoved`, `PropertyChanged`, `ItemProperty` and
  the overlay backend (owner of that document).
- The P1 real-data test of exit criterion 3 runs the test server in process with two
  clients: one in an overlay run equips from a scenario, the other observes
  `AppearanceChanged` for that entity with the item's DID and slot.
- The server's dependency graph does not change; `check-server-deps` stays as it is.

## Open for the owner

1. **Numeric enum keys in committed scenarios** (`scenario-feed.md` §16, owner policy, not
   decided here). The same question now covers property IDs written as `pid:N:KIND` and
   numeric property values: until the owner decides, committed scenarios use labels only, so
   the no-install CI replay cannot show `stat`, `item-property` or look entries with
   values (Vector G, which is spec text, uses property IDs outside the dictionary's two ID
   families, `property-system.md` MP4).
2. **Relayed look values.** `server-content.md` §4 covers relayed look values (property IDs
   with player-chosen values) as well as DIDs and enum keys.
3. **Deletion on M7's server** (`character-screens.md` §7.2, Q9) is not part of this
   decision; the delete button stays ghosted on that backend.

## Review

**2026-10-10**, spec reviewer (Clean-Room-Role: review): a Claude review session that wrote no
part of this decision, of the P1 revisions of `scenario-feed.md`, `tessera-protocol.md`,
`test-server.md` and `server-content.md`, or of the code. Checked against `docs/CLEAN_ROOM.md`
§5.3 and `server-content.md` §7: consistency of D1–D5 with the specs, bounds and hostile-input
limits of every new field, the conformance vectors, version negotiation, and the
implementation on branch `p1` (read to compare only). Result: **passed**.

- **Server content scope.** The server gains no message, state or rule. Looks and slots are
  stored and relayed unjudged, class and nationality never travel, and the overlay sends only
  `AppearanceSet`s, at most one in flight and 4 per second. Bags, items, wallet, levels, stats
  and item properties stay in the client. `tools/check-server-deps.sh` passed on `p1`.
- **Bounds.** Every row of the hostile-input table above is in the specs with the same value
  and outcome (`tessera-protocol.md` §5.3, §13.1; `test-server.md` §3.5, §10.1;
  `scenario-feed.md` §9.5, §11.1, §12.5.4).
- **Vectors and negotiation.** `tessera-protocol.md` §16.8 re-encoded independently and equal
  byte for byte; Vector G and the 15 new hostile rows re-derived; `Hello` and `HelloAck`
  frozen, minors 0–1, byte identity at minor 0 (invariant 10). Tests of `tessera-protocol`,
  `tessera-net` and `tessera-server` on `p1` passed.
- **Erratum (D1).** D1 lists `look` among the declarations an overlay run allows;
  `scenario-feed.md` §12.5.1, which is normative, excludes it, and the implementation follows
  §12.5.1. Read D1 without `look`: every place that binds a look (`player`, `character`,
  `accept-character`, the `create-character` input form) is outside the overlay subset anyway.
- Non-blocking notes on the specs are in their review records: `tessera-protocol.md`
  (a)–(c), `test-server.md` (a)–(c), `scenario-feed.md` §18.3 (a)–(f), and the
  `server-content.md` Reviewer row (looks hold player-chosen values as well as record
  numbers, owner item 2).
