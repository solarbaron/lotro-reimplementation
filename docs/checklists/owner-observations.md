<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Observations only the project owner can make

| | |
|---|---|
| **Status** | Open list; started 2026-10-04; last updated 2026-10-16 (p6: decision 0024 P6 items O160–O175 added: experience bar, level-up, trait panel, skill training, crafting panel, crafting trainers, gathering, plugin progression and crafting views, panel keys); 2026-10-13 (merge of p5: decision 0024 P5 items O140–O154 added: maps, travel, instance finder, mounts, plugin mount views; O155, server-added map markers, added at the P5 exit verification 2026-10-12); P4 spec writer: items O130–O137 added, decision 0035; `social-panels.md` revision 1: O130, O132, O133 gain questions; decision 0024 P3 items O110–O125 added: NPC windows, quest journal, wallet; P2 exit verification: O128 and O129 added; P2 documentation: O93–O109, O126 and O127 added); 2026-10-10 (spec writer: O85 and O86 gain the expectations the client gives for the creation screens, `character-screens.md` revision CS19–CS27); 2026-10-10 (spec writer: O24 gains the W14 question on the puddles' colour and opacity); 2026-10-10 (P1 exit verification: O88–O90 added); 2026-10-10 (spec writer: O26 gains the question on the sky line of row W9; new items O91, the far sea of row W2, and O92, the generated fallen soldiers of 3, 0xD7E7); 2026-10-10 (O24 extended to the water rows W10–W17 of `m4-5-visual.md` §4.8, with the W13 and W17 questions); 2026-10-09 (decision 0024 P1 items O80–O86 added; merge of main: the M5 items O39–O49 joined the summary next to the 2026-10-08 statuses); 2026-10-08 (every pending item checked against the data and the offline client: O5, O14, O70 answered; O6, O18, O28 narrowed; the rest marked as needing the original) |
| **Author role** | Spec writer (dirty room) |
| **Who fills it in** | The project owner, in the original client, on their own account and install |
| **Method** | Black-box observation of the unmodified official client: its own UI, `/loc`, and frame captures (RenderDoc or apitrace under Wine/DXVK). Allowed by `docs/CLEAN_ROOM.md` §4 (method M3) and [ROADMAP §2.1](../ROADMAP.md#method-allowed). |

Some facts cannot be read from the data files: they are decided by the original client's
code at run time. This file lists every such question that a spec currently leaves OPEN,
with what to do and what to record. When an item is answered, the spec writer moves the
fact into the spec (with this file as its source, tagged "owner observation, date") and
marks the item **done** here.

<a id="rules"></a>
## Rules for recording

- **No game text in the repository.** Do not write area, item, NPC or server names, quest
  text or chat output here. Describe things in your own words or by DID. Keep anything
  else in a private note outside the repository.
- **No captures or screenshots in the repository.** Frame captures and screenshots stay in
  the private dirty-room scratch area; only the facts read from them are written here
  (numbers, states, yes/no).
- Record for every observation: the date, the client build (as the launcher or the client's
  own about box shows it, numbers only), graphics settings that matter (detail levels,
  DirectX version), and whether the client ran under Wine/Proton + DXVK.
- Where an item has a Tessera command to show our current output, run it first so you know
  what to compare.

<a id="summary"></a>
## Summary

| ID | Question | Spec | Needs | Status |
|---|---|---|---|---|
| [O1](#o1) | `/loc`, map coordinates and area at 20 reference locations | `docs/checklists/m3-reference-locations.md` | In-game `/loc` | partly answered (`coordinates.md` §4, §8–§9, §14: formula, rounding, landcells and block edges from the client); left for the owner (2026-10-08: not in the data or the client): `oz` on flat ground at two or three rows (expected terrain + 0.00 or + 0.05 m), the area-name column, and whether map coordinates show indoors |
| [O2](#o2) | Compass bearing of a placed object versus `/loc` heading | `entity-descriptor.md` §4.4 | In-game `/loc` | answered from the client (`coordinates.md` §11.3–§11.4: h = (360° − yaw) mod 360, bearing of model +Y); optional confirmation |
| [O3](#o3) | Which content layer the world shows by default | `entity-descriptor.md` §4.5.1 | In-game look | answered from the client (`entity-descriptor.md` §4.5.1, §9.3: layers 1 and 2 are both shown, for every kind of placed object including drawn Physics objects; only named layers depend on story progress); optional confirmation spot: region 1, block 0xA15B |
| [O4](#o4) | Are one-triangle placeholder parts drawn? | `setup.md` §8.5 | Look, or frame capture | answered from the client (`setup.md`: never drawn in objects with more than one part); no observation needed |
| [O5](#o5) | Exact dye blend | `material.md` §12.3 | — | **Answered from the client (2026-10-08)**: smooth blend on stored values, no threshold, dye alpha unused, no sRGB (`material.md` §12.3, MA24, MA33); no observation needed |
| [O6](#o6) | Other material and render-state questions | `material.md` §17 and §19 | Look, toggling options | narrowed (2026-10-08): O6.1–O6.9 answered from the client (`material.md` §17, §19, §20.1); left for the owner, without a frame capture: which graphics option turns on the soft edges of cut-out foliage (`material.md` §8.5 OPEN) |
| [O7](#o7) | Command-line option leniency | `install-layout.md` §10 | Launch test | answered from the client (`install-layout.md` §10.1); no observation needed |
| [O8](#o8) | Standing heights per species and sex (whole-body scale) | `appearance.md` §6.1, §17, `skeleton-anim.md` §2.2 | Look, side-by-side screenshots | partly answered (`appearance.md` §6.1, §17); left for the owner: one screenshot of a High-elf female beside a Human female; unchanged 2026-10-08 (whether the template scale enters the render scale was not traced; the one screenshot pair settles it) |
| [O9](#o9) | Height slider and facial bone sliders in character creation | `appearance.md` §6.2, §6.3 | Character-creation screen | partly answered (`appearance.md` §6.2–§6.5, §17: slider rules); left for the owner, as confirmation only: heights at the ends of the height controls, River-hobbit feet on the ground; unchanged 2026-10-08 (confirmation by look only) |
| [O10](#o10) | Worn-item layering and equal-priority ties | `appearance.md` §4.6, §4.7 | Look (wardrobe preview) | partly answered (`appearance.md` §4.7, §17: the client's tie rule); left for the owner: items 1–8, mainly item 5 and whether the order of equipping matters; unchanged 2026-10-08 (item 5 depends on the order the server sends; not in the client) |
| [O11](#o11) | Updated-avatar option and default style files | `appearance.md` §3.3 | Options screen, look | answered from the client (`appearance.md` §3.3, §3.5, §17); optional glance at the options screen to confirm the default |
| [O12](#o12) | Look of random-appearance NPC figures | `appearance.md` §7.4 | Look at two spots, two visits | partly answered (`appearance.md` §7.4, §17: where outfits come from); left for the owner: two visits to one figure (same look or not); narrowed 2026-10-08: the picks come from the client's clock-seeded global generator with no per-object seed, so a different look per visit is expected (`script-runtime.md` §8, R40); the two visits confirm it |
| [O13](#o13) | Where weapons, instruments and class items rest, drawn and stowed | `appearance.md` §5.3–§5.4, `skeleton-anim.md` §8.5 | Look | partly answered (`appearance.md` §5.6, §17: weapons, shields, ranged items); left for the owner: in-hand direction of long weapons (two side screenshots) and whether class items are ever visible; unchanged 2026-10-08 (look) |
| [O14](#o14) | Animation playback: blending, layering, turn direction, speeds, scale inheritance, cloth | `skeleton-anim.md` §5.4, §6.5, §6.6, §7.3, §7.4, §13 | — | **Answered from the client (2026-10-08)**: all rows (`skeleton-anim.md` §15.1; O14.4 walking speeds 2.5 / 2.25 / 2.57 m/s from `collision.md` §11.8); `/loc` timing and visual checks optional |
| [O15](#o15) | Skin, hair and eye colour tinting; face layers | `appearance.md` §3.3, §3.5 | Frame capture | partly answered (`appearance.md` §3.5, §17: one composited face texture, tints through the dye colour; colour byte order B, G, R, A, `property-system.md` §4.7); left for the owner: one face frame capture for the texture slot (low priority); unchanged 2026-10-08 (frame capture, low priority); 2026-10-10: slot answered from the client (3000), capture no longer needed; open: the eyebrow comparison and a face comparison with face compositing off, an Elf or High-elf face included (low priority) |
| [O16](#o16) | Which idle, walk and run clips play; idle variants and fidgets | `script-table.md` §8, §15.1, §16 | Look, explorer side by side | partly answered (`script-table.md` §8.6, §15.1: avatars fidget, timing predicted; mood idles answered); left for the owner: the side-by-side clip comparison and about 5 minutes standing still; unchanged 2026-10-08 (side-by-side look) |
| [O17](#o17) | Static NPCs: position, facing and presence at the N spots | `m4-visual.md` §N, `appearance.md` §7, `entity-descriptor.md` §9.4 | In-game `/loc`, look | partly answered (`entity-descriptor.md` §9.4: presence rule, position and facing from the client); left for the owner: whether N4 and N6 are there for your character, and each figure's look; unchanged 2026-10-08 (server state and look) |
| [O18](#o18)–[O27](#o27) | World fidelity: movement, scripted walks, visual rows, clock, draw distances, shadows and lights, water, frills, sky and fog, terrain bands | see [M4.5 items](#m4-5-items) | `/loc`, look, short videos | O18 mostly answered (2026-10-08; only the jump measurement left); O21, O22, O26 partly answered (2026-10-07); O19, O20, O23–O25, O27 need the original (visual) |
| [O28](#o28) | Audio and player music: area-music cross-fade, gaps, falloff, day/night and indoor beds, combat fade, ABC checks, live play, instance limit | `sound.md` §16, `script-runtime.md` §26, `abc-dialect.md` §11 and §17, `instruments.md` §16 | Listening, `/loc`, a recording of the client's output, small original ABC files | narrowed (2026-10-08): O28.3, O28.4, O28.7, O28.9 answered from the data and the client; O28.1–O28.2 reduced to where the music changes; the others need listening (M6) |
| [O39](#o39) | Screenshots of at least 10 native panels in the original client | `ui-layout.md` §17, `m5-visual.md` | Screenshots (private) | open (M5 criterion 2) |
| [O40](#o40) | The 14 plugins of the usage census installed and run in Tessera | `plugin-api-usage.md` §9, `lua-plugin-api.md` | Plugin install, look | open (M5 criterion 4) |
| [O41](#o41) | A published user skin in the original client versus Tessera | `skin-definition.md` | Look (optional) | open (M5 criterion 3, optional) |
| [O42](#o42) | Second right or left press during mouse-look | decision 0014 (Known difference 14) | Look | open (M5) |
| [O43](#o43) | Wheel over a plain plugin window: does the camera zoom? | decision 0014 (D5 OPEN) | Look | open (M5) |
| [O44](#o44) | Right press during an item drag | decision 0014 (Known difference 20) | Look | open (M5) |
| [O45](#o45) | Escape with panels open: what closes, in what order | `game-panels.md` §16 | Look | open (M5) |
| [O46](#o46) | Chat window at UI scales 1.0, 1.5, 0.5; auto-fade look; semi-transparent edge blend | decision 0015 (observations 1–3) | Look, frame capture | open (M5) |
| [O47](#o47) | Owner-written nested-surface test plugin | decision 0015 (observation 4) | Test plugin, screenshot | open (M5) |
| [O48](#o48) | Cursor during and after mouse-look; OS cursor size at scale 1.333; minimap marker edges | decision 0015 (observations 5–7) | Look | open (M5) |
| [O49](#o49) | U1–U5 scripted UI measurement on the idle reference laptop | decision 0015 §6, decision 0010 | Measurement run | open (M5 criterion 7) |
| [O70](#o70) | Player movement speeds and turn rate (test-server movement limits) | `test-server.md` §4.5, §8.3; `tessera-protocol.md` §8.3 | — | **Answered from the client (2026-10-08)**: run 6.5 / 3.25 / 5.0 m/s, walk 2.5 / 2.25 / 2.57 m/s (forward / backward / strafe), keyboard turn 150°/s (`[Input] TurnKeySpeed`), no species difference (`collision.md` §11.8–§11.9) |
| [O71](#o71) | How other players' characters move when watched (interpolation feel) | `tessera-protocol.md` §8.6 | Look, optional private video | open (M7); not answerable from the client (a felt comparison; remote updates belong to the excluded network layer) |
| [O72](#o72) | Distance at which other player characters appear and disappear | `tessera-protocol.md` §9, `test-server.md` §8.2 | Second account or a friend, `/loc` | open (M7); not answerable from the client (the server decides) |
| [O73](#o73) | Range of local "say" chat (optional) | `test-server.md` §4.6, §9 | Two characters, `/loc` | open (M7, optional); not answerable from the client (the server decides) |
| [O80](#o80) | Item tooltips of ten items: rows, order, colours, worth coins | `item-tooltips.md` §4–§7, §15 | Look, screenshots (private) | open (P1) |
| [O81](#o81) | Comparison tooltips and pinned tooltips | `item-tooltips.md` §8–§10 | Look | open (P1) |
| [O82](#o82) | Character panel: opening command, paperdoll rotation, header | `character-panels.md` §2–§3 | Look | open (P1) |
| [O83](#o83) | Bag windows: which slots each bag shows; all-bags command; item-slot overlays | `character-panels.md` §4.1, §4.3, §5 | Look | open (P1) |
| [O84](#o84) | Equipping, unequipping, using, splitting and destroying items | `character-panels.md` §6 | Look | open (P1) |
| [O85](#o85) | Character selection and race/class screens: order of screens, lists, defaults | `character-screens.md` §2–§4, §6 | Look | open (P1) |
| [O86](#o86) | Appearance and name screen: controls, randomise, nationality colours | `character-screens.md` §5 | Look | open (P1) |
| [O88](#o88) | Worn look after five equips: chest, head, a weapon in each hand, shield or off-hand item; which hand a held item goes to | `character-panels.md` §7, `appearance.md` §8 | Look, screenshots (private) | open (P1) |
| [O89](#o89) | Classes and nationalities each race and gender offers in the creation screen; which are locked or hidden | `character-screens.md` §4–§5, §9 | Look | open (P1) |
| [O90](#o90) | Customised appearance (styles, a colour, height, a slider) compared with the same choices in Tessera; does the selection figure keep them | `character-screens.md` §5.2, §7.1 | Look, screenshots (private) | open (P1) |
| [O91](#o91) | Far sea at row W2: as dark as Tessera's, or reflecting the sky? | `water.md` §8.4, §8.6.4; `m4-5-visual.md` W2 | Look | open (M4.5) |
| [O92](#o92) | Fallen soldiers generated in landblock 3, 0xD7E7: pose and cloth colours | `appearance.md` §7.4; `m4-5-visual.md` §4.6 (L5 note) | Look | open (M4.5) |
| [O93](#o93) | Targeting keys: what Tab, Backspace, F9, F10, Delete, Backslash and F select; range, order, wrap | `combat-ui.md` §3.3 | Look, `/loc` | open (P2) |
| [O94](#o94) | Escape with a selection and a cast: order of cancelling, clearing and the main menu; craft cancel | `combat-ui.md` §3.4 | Look | open (P2) |
| [O95](#o95) | World selection effect: look, distance bands, per kind | `combat-ui.md` §3.5 | Look | open (P2) |
| [O96](#o96) | When the vital numbers show on the player and target frames, and their form | `combat-ui.md` §4.3 | Look | open (P2) |
| [O97](#o97) | Target health-bar colours for hostile, neutral, tapped, NPC, player and defeated targets | `combat-ui.md` §5.3 | Look | open (P2) |
| [O98](#o98) | Name colours at level differences −10 … +10 | `combat-ui.md` §5.4; `game-controls.md` §7.5 | Look | open (P2) |
| [O99](#o99) | Selection's-target window: shown by default, place, content | `combat-ui.md` §5.7 | Look, options panel | open (P2) |
| [O100](#o100) | Floating combat text: sign, wording, colours, motion, criticals | `combat-ui.md` §6.3–§6.6 | Look, private video | open (P2) |
| [O101](#o101) | Channel bars fill or drain; craft bar and Escape | `combat-ui.md` §7.3, §7.4 | Look | open (P2) |
| [O102](#o102) | Effect rows: which row holds what, icon order, timers or stacks, pulse, overflow | `combat-ui.md` §8.2 | Look | open (P2) |
| [O103](#o103) | Defeat panel: buttons, ghosting, awaken timer, what OK does | `combat-ui.md` §10.2 | Look | open (P2) |
| [O104](#o104) | Skill tooltip with damage, effects and a cost: rows, order, colours | `skill-tooltips.md` §4, §7, §9 | Look, screenshots (private) | open (P2) |
| [O105](#o105) | Effect tooltip from the effect bar: lines, resistance, live countdown and its format | `skill-tooltips.md` §9, §10.2, §12.2 | Look | open (P2) |
| [O106](#o106) | A stat line with a percentage and a decrease | `skill-tooltips.md` §9.4 | Look | open (P2) |
| [O107](#o107) | Skill-granting item: the attached skill tooltip | `skill-tooltips.md` §12.4; `item-tooltips.md` §8.3 | Look | open (P2) |
| [O108](#o108) | Stack count in an effect tooltip | `skill-tooltips.md` §12.2; `combat-ui.md` TR31 | Look | open (P2) |
| [O109](#o109) | Casting and recovery times with tenths of a second | `skill-tooltips.md` §7.6, §7.15, §10.1, §16.3 | Look | open (P2) |
| [O110](#o110) | Using an NPC: which inputs use it, the no-selection and out-of-range lines, the distance at which use stops | `npc-windows.md` §3.1 | Look, `/loc` | open (P3) |
| [O111](#o111) | Dialogue windows: which window shows which kind of NPC talk, the option list's layout, closing when walking away | `npc-windows.md` §5.1, §5.3, §5.7 | Look, `/loc` | open (P3) |
| [O112](#o112) | Quest-offer window: objective shown, reward rows and amounts, choosing a reward before Accept | `npc-windows.md` §6.2, §6.3, §6.5 | Look | open (P3) |
| [O113](#o113) | Vendor window: buy modes and quantities, quantity dialog, unaffordable prices, sell list, drop to sell, Sell All, buy-back | `npc-windows.md` §7.2–§7.6, §7.8 | Look | open (P3) |
| [O114](#o114) | Barter window: cost slots, coin costs, the cost text and its colour, pages | `npc-windows.md` §7.7 | Look | open (P3) |
| [O115](#o115) | Loot window: coins, inputs that take one entry, Loot All and closing | `npc-windows.md` §8.2, §8.3 | Look | open (P3) |
| [O116](#o116) | Wallet window: opening, category and entry order, capped currencies, search | `npc-windows.md` §9.1, §9.4 | Look | open (P3) |
| [O117](#o117) | Coin display: which coins show for an amount, zero coins | `npc-windows.md` §9.2, §9.3 | Look | open (P3) |
| [O118](#o118) | Quest log: category and quest order, row format, filters at first open, expanded state, counts | `quest-journal.md` §4.2–§4.6 | Look | open (P3) |
| [O119](#o119) | Quest details pane: section order, counters of two counted parts, objectives without a counter | `quest-journal.md` §4.7, §5.2–§5.4 | Look | open (P3) |
| [O120](#o120) | Quest tracker: auto-tracking, entry order, cutting long entries, completed quests, menu | `quest-journal.md` §6.2–§6.6 | Look | open (P3) |
| [O121](#o121) | Quest messages: chat lines for new, updated, completed and abandoned quests; text on screen | `quest-journal.md` §7.1–§7.3 | Look | open (P3) |
| [O122](#o122) | Quest rings over NPCs: available, off-level, talk-to and underway-elsewhere cases | `quest-journal.md` §8.1–§8.2 | Look | open (P3) |
| [O123](#o123) | Quest markers on the minimap and the world map: icon, order with the player marker, tooltip, edge | `quest-journal.md` §8.3; `game-panels.md` §8.8, §9.3 | Look | open (P3) |
| [O124](#o124) | Abandoning a quest: confirmation and what the log and tracker show after | `quest-journal.md` §4.8 | Look | open (P3) |
| [O125](#o125) | Plugin wallet views: `GetWallet()` order, `GetMaxQuantity()` of a few items, `MoneyChanged` count | `lua-plugin-api.md` §15.3 | Own test plugin | open (P3) |
| [O126](#o126) | Casts, hits, projectiles and combat stance; a weapon change mid-swing | `combat-visuals.md` §3, §4, §6, §11 rule 6, Q-CV7 | Look, private video | open (P2) |
| [O127](#o127) | Effect visuals to their end; death pose, body fade and revive | `combat-visuals.md` §5, §7 | Look | open (P2) |
| [O128](#o128) | Player and target frames as a whole: layout, name colour, level, morale and power, class resource, the target's effect icons | `combat-ui.md` §4, §5, §8 | Look, screenshots (private) | open (P2) |
| [O129](#o129) | Quickslot bar and cast bars: cooldown sweep and timer, greyed and unavailable slots; cast, induction and channel bars with delay, interrupt and cancel | `combat-ui.md` §7, §9 | Look, private video | open (P2) |
| [O130](#o130) | Fellowship display and vitals: leader marker, member out of view, removal and disband lines | `social-panels.md` §4, §9 TR2–TR4; `scenario-feed.md` §6.7 | Look, a group with other players | open (P4) |
| [O131](#o131) | Fellowship invitation prompt: window, buttons, expiry | `social-panels.md` §5 | Look, an invitation from another player | open (P4) |
| [O132](#o132) | Trade window: both sides, coin, ready states, completion | `social-panels.md` §6 | Look, a trade with another player | open (P4) |
| [O133](#o133) | Social panel: friend and ignore lists | `social-panels.md` §3 | Look | open (P4) |
| [O134](#o134) | Tells and the instant-message tab | `chat-window.md` §15.2–§15.5 | Look, tells with another player | open (P4) |
| [O135](#o135) | Channel join and leave lines, channel slots and tabs | `chat-window.md` §15.6; `tessera-protocol.md` §10.6, §17 | Look | open (P4) |
| [O136](#o136) | How aligned a synchronised fellowship start sounds | `instruments.md` §8.8–§8.9, Q10–Q13 | Listening, stopwatch, a group with other players | open (P4) |
| [O137](#o137) | Owner question: may Tessera's own server relay a synchronised `/play` start as chat? | decision 0024 open item 2; decision 0035 D2 | Owner decision | open (P4) |
| [O140](#o140) | World map: presentation at first opening, full-screen size and position, controls per kind of map | `map-windows.md` §3.1, §3.4 | Look | open (P5) |
| [O141](#o141) | World map overlays and filters: filter menu items, order and defaults, persistence, note order | `map-windows.md` §6.2, §6.3, §6.9 | Look | open (P5) |
| [O142](#o142) | Map navigation: right click, the map menu's order, child-map buttons without a node | `map-windows.md` §4.3–§4.5 | Look | open (P5) |
| [O143](#o143) | Dungeon maps: stretch, fog, tooltips under fog | `map-windows.md` §4.7; `game-panels.md` §8.5.3 | Look | open (P5) |
| [O144](#o144) | Minimap notes and blips: rim icons and their turn, tooltip, the minimap's filter menu | `map-windows.md` §6.8 | Look | open (P5) |
| [O145](#o145) | Place lookup: no search in the original; coordinates under the pointer against `/loc` | `map-windows.md` §3.4, §7.1 | Look, `/loc` | open (P5) |
| [O146](#o146) | Travel window: headers, row order, search and case | `travel-windows.md` §4.4–§4.6 | Look | open (P5) |
| [O147](#o147) | Travel window: cost display, marks, go button states, closing | `travel-windows.md` §4.7–§4.9 | Look | open (P5) |
| [O148](#o148) | Travel presentation: end-travel box, loading screen, durations, chat lines | `travel-windows.md` §5 | Look, timing | open (P5) |
| [O149](#o149) | Instance finder: window and page, header levels, featured field, size, tier and level controls | `travel-windows.md` §6.1–§6.4 | Look | open (P5) |
| [O150](#o150) | Instance queue, invitation window and leaving an instance | `travel-windows.md` §6.5–§6.6, §7.2 | Look | open (P5) |
| [O151](#o151) | Mounted gaits of a normal mount: speeds, strafing, camera height, River-hobbit mount scale | `mounts.md` §5.1, §14.6, §14.8, §14.9; `walker.md` §16.12 | Look, `/loc` | open (P5) |
| [O152](#o152) | Mounted gaits of a war-steed: gait steps and timing, speed on release, braking | `mounts.md` §5.3–§5.7, §14.6; `walker.md` §16.12 | Look, timing, video (private) | open (P5) |
| [O153](#o153) | Mount and dismount sequence: summon timing, cancels, bow and wait, water, forbidden areas | `mounts.md` §14.7, §14.12; `walker.md` §16.12 | Look, timing | open (P5) |
| [O154](#o154) | Plugin mount views: `MountChanged` count, class, values | `lua-plugin-api.md` §15.5 | Own test plugin | open (P5) |
| [O155](#o155) | Map markers a server adds outside quests: whether any exist, icon, label, tooltip on the map and the minimap | `map-windows.md` §6.7, §6.8 | Look | open (P5) |
| [O160](#o160) | Experience bar: place, segments, fill colours, tooltip, gain animation and floating text | `progression-panels.md`; `game-panels.md` §11 | Look | open (P6) |
| [O161](#o161) | Level-up presentation: effect, sound, chat lines, banner, bar reset, announcements | `progression-panels.md`; `scenario-feed.md` §6.6 | Look, timing | open (P6) |
| [O162](#o162) | Trait panel: tabs, trees and branches per class, cell rows and columns, branch choice | `progression-panels.md`; `combat-data.md` §8.5 | Look, explorer | open (P6) |
| [O163](#o163) | Trait tree cells: rank display, locked cells and dependencies, points, spend and refund | `progression-panels.md`; `combat-data.md` §8.3–§8.5 | Look | open (P6) |
| [O164](#o164) | Trait slots and trait tooltips: slot counts, locked slots, tooltip rows, slotting | `progression-panels.md`; `combat-data.md` §8.1–§8.3 | Look | open (P6) |
| [O165](#o165) | Skill-training window: layout, rows shown, columns, ghosting, confirm, after training | `progression-panels.md`; `npc-windows.md` §5.3 | Look | open (P6) |
| [O166](#o166) | Crafting panel: profession tab order, extra tabs, tier-row icons and meters | `crafting-panels.md` §4.3, §4.7 | Look | open (P6) |
| [O167](#o167) | Recipe list: initial expansion, order of tiers, categories and recipes, counts, green states, search | `crafting-panels.md` §4.4–§4.6 | Look | open (P6) |
| [O168](#o168) | Recipe page: multiple outputs, tool and facility lines, optional ingredients and critical chance | `crafting-panels.md` §5.2–§5.5 | Look | open (P6) |
| [O169](#o169) | Make controls: quantity typing and arrows, Make during a batch, hide when crafting, interruption | `crafting-panels.md` §5.6–§5.8 | Look | open (P6) |
| [O170](#o170) | Crafting bar and result lines: label, length, chat lines for success, critical and interruption | `crafting-panels.md` §6; `combat-ui.md` §7 | Look, timing | open (P6) |
| [O171](#o171) | Recipe-scroll tooltips: one-use and cooldown rows, attached result tooltip, requirement rows | `crafting-panels.md` §7; `item-tooltips.md` §6.20 | Look | open (P6) |
| [O172](#o172) | Crafting trainers: choice rows, trainer window texts, profession selector, vocation confirmation | `crafting-panels.md` §8.1, §8.3 | Look (do not confirm) | open (P6) |
| [O173](#o173) | Gathering nodes: highlight, cursors, gathering bar, loot or bags, node removal, refusals | `crafting-panels.md` §9.1; `interaction.md` §3.3–§3.8 | Look, timing | open (P6) |
| [O174](#o174) | Plugin progression and crafting views: profession levels and titles, events, `RecipeAdded`, untrained list, vocation | `lua-plugin-api.md` §15.6 | Own test plugin | open (P6) |
| [O175](#o175) | Panel toggle keys: crafting, trait and skills panels, profession toggles, close behaviour | `crafting-panels.md` §12; `progression-panels.md`; `keymap.md` §8.2 | Key-binding options | open (P6) |

M4 exit criteria that wait on these items ([ROADMAP M4 status](../ROADMAP.md#m4-status)):
criterion 2 (bodies play idle, walk and run): O8, O9, O16 (O14 answered from the client on 2026-10-08); criterion 3 (worn and held
items): O10, O13; the sandbox's static NPCs: O12, O17. Criterion 4 (performance) passed
on the reference machine and needs no observation.

M4.5 exit criteria that wait on these items ([ROADMAP M4.5 status](../ROADMAP.md#m4-5-status)):
criterion 4 (walking on collision): O18 (only the jump measurement left since 2026-10-08), O19; criterion 6 (visual checklist): O20 (terrain,
scenery and frill rows), O23 (lighting rows), O24 (water rows), O25 (frills), O26 (sky and
fog rows, with the W9 sky line), O27, O91 (W2 far sea), O92 (figures seen from L5); the default-settings profile behind criteria 5 and 6: O21, O22.

M5 exit criteria that wait on these items ([ROADMAP M5 status](../ROADMAP.md#m5-status)):
criterion 2 (native panels): O39; criterion 3 (user skin, optional cross-check): O41;
criterion 4 (third-party plugins): O40; criterion 7 (UI budgets): O49. O42–O48 settle OPEN
behaviour of decisions 0014 and 0015 and do not block the criteria.

M6 exit criteria that wait on these items ([ROADMAP M6 status](../ROADMAP.md#m6-status)):
criterion 2 (area music cross-fades at area borders): O28.1 (row O1 of
[`m6-audio.md`](m6-audio.md)). The other O28 rows check facts the M6 specs leave OPEN or
mark as owner observations; criteria 1, 3 and 4 need no observation in the original client
(criterion 4 still needs an idle re-measurement on the reference machine, decision 0010).

M7 items O70–O73 tune defaults of Tessera's own protocol and test server so that movement
feels like the original (O70 answered from the client on 2026-10-08; O71–O73 depend on the
server or on feel and stay with the owner); no M7 exit criterion waits on them ([ROADMAP M7
status](../ROADMAP.md#m7-status)). They are numbered from O70 because other milestone
branches added items at the same time (M4.5 uses O18–O27, M6 uses O28); they keep their
numbers.

P3 items O110–O125 ([ROADMAP P3](../ROADMAP.md#p3)) compare the NPC windows, the quest
journal and the wallet with the original; no P3 exit criterion waits on them, and they block
nothing until P8 (decision 0024 §2.6).

P2 items O93–O109 and O126–O129 ([below](#p2-items)) compare Tessera's combat presentation
with the original: O93–O103 are the owner checks OC1–OC11 of `combat-ui.md` §18, O104–O109 the
checks OS1–OS6 of `skill-tooltips.md` §17, O126–O127 looks at combat visuals
(`combat-visuals.md`), O128–O129 whole-panel looks at the frames, the quickslot bar and the
cast bars (P2 exit verification). O126–O129 are numbered after P3's O110–O125 (the P3 branch holds those). No P2
exit criterion waits on them ([ROADMAP P2](../ROADMAP.md#p2)); they settle OPENs and Tessera
rules of those specs and block nothing until P8 (decision 0024 §2.6).

P4 items O130–O137 compare Tessera's social panels and chat with the original
([ROADMAP P4](../ROADMAP.md#p4)); they block nothing until P8 (decision 0024 §2.6). They are
numbered from O130 because other P milestones add items at the same time (P2 from O93, P3
from O110). O137 is a decision, not an observation.

P5 items O140–O155 ([ROADMAP P5](../ROADMAP.md#p5)) compare the maps, the travel and
instance-finder windows, the mounted walker and the plugin mount views with the original; no P5
exit criterion waits on them, and they block nothing until P8. They are numbered from O140
because P2 and P4 add items on their own branches at the same time.

P6 items O160–O175 ([below](#p6-items)) compare the experience bar and level-up, the trait
panel, skill training, the crafting panel, crafting trainers, gathering and the plugin
progression and crafting views with the original; no P6 exit criterion waits on them
([decision 0037](../decisions/0037-p6-trait-tree-and-recipe-census.md)), and they block nothing
until P8. They number in one sequence the rows the P6 specs proposed (`crafting-panels.md`
O180–O191, `lua-plugin-api.md` O192–O195); the mapping is at the head of the section.

---

<a id="o1"></a>
## O1. Twenty reference locations (M3 exit criteria 4 and 5)

**Spec:** [`m3-reference-locations.md`](m3-reference-locations.md) §2.2 (table with empty
columns) and §4.2 (procedure).

**What to do:** for each row you can reach, stand on open ground just outside the listed
corner of the anchor object, type `/loc`, and fill in the three empty columns as §4.2 says:
the numeric `/loc` fields and the map-coordinate string under the minimap; `= walker` /
`≠ walker` / `seen` for the area name (never the name itself); pass, fail or `n/a`.

**Why:** these readings check the height scale against `oz`, the `/loc` landcell and
offset convention, the map-coordinate formula and its rounding, the formula's use outside
region 1, and the block-map area assignment (that checklist's §3).

**2026-10-07 (from the client):** `coordinates.md` §4, §8–§9 and §14 now take from the
client the map-coordinate formula and its per-region constants, the rounding (exact ties
round to even), the letters, the `/loc` landcell and offset rules, the meaning of `r`, and
which block a point exactly on a block edge belongs to. Those columns are now only a
confirmation. Still for the owner:

- **Avatar height above the ground:** the `oz` column of the rows you can reach, on flat open
  ground. The client code that keeps the avatar on the terrain was not traced, so only these
  readings show whether `oz` sits exactly on the terrain or at a fixed distance above it.
- **Area names:** the area-name column (`= walker` / `≠ walker` / `seen`) for every row you
  can reach.
- **Indoors (new):** stand inside one building or dungeon and note whether any panel shows
  your map coordinates there, where it shows them, and what `/loc` prints (numbers and
  layout only, no names). `coordinates.md` §8.3 OPEN.

**2026-10-08 (check against the data and the client):** none of the three columns can be read
from the install or the offline client. The area name and the indoor display are what the
original shows on screen, and the avatar's height above the ground depends on where the
original places the player's capsule, which was not traced (`collision.md` §11.7 OPEN, §16
Q18). To keep the `oz` part short:

- **`oz` (reduced):** at two or three rows on flat open ground, record `oz`. The spec writer
  compares it with the walker's terrain height there; the client's rules allow only about
  0.00 m (capsule bottom at the feet) or about 0.05 m (the terrain's 0.05 m convex radius)
  above the terrain (`collision.md` §11.7 table). The other rows' `oz` are not needed.
- The area-name column and the indoors check stay as above.

<a id="o2"></a>
## O2. Bearing of placed objects versus the `/loc` heading

**Spec:** `entity-descriptor.md` §4.4: an object's local +Y axis is derived to point at
compass bearing (360° − ψ) mod 360°, measured clockwise from north. Not yet checked against
the original.

**What to do:** at three long buildings from the O1 table, walk along the long wall and
stop with the character facing exactly along it (back or shoulder against the wall helps).
Type `/loc` and record `h`. Do it once in each direction if convenient.

| Row | Long wall runs along the object's | ψ (°) | Expected `h` if §4.4 is right | `h` if the turn direction were reversed | Observed `h` |
|---|---|---|---|---|---|
| M3-06 | local *y* (38.4 m side) | 307.3 | 52.7 or 232.7 | 127.3 or 307.3 | |
| M3-10 | local *y* (30.6 m side) | 75.0 | 285.0 or 105.0 | 255.0 or 75.0 | |
| M3-14 | local *y* (27.6 m side) | 225.0 | 135.0 or 315.0 | 45.0 or 225.0 | |

**Record:** the `h` values (±3° is enough). `/loc` omits `h` within about 1.3° of north
(research note); write "omitted" then.

<a id="o3"></a>
## O3. Default content layer

**Spec:** `entity-descriptor.md` §4.5.1. Placed objects can carry content-layer lists;
layers 1 and 2 are the two common alternatives. Which one the open world shows (for a
character with no special progress) is unknown. The walker draws objects without layers
plus those listing layer 1, with a debug toggle.

**What to do (once the entity-descriptor spec writer has added a spot here):** go to the
spot, note in your own words which of the two object sets is present (the walker's toggle
shows both sets), and whether it differs between characters at different stages of the
story.

**Record:** "layer 1 set present" / "layer 2 set present", and the character's level band.

**2026-10-07 (from the client):** answered for every kind of placed object, now including
the drawn Physics objects (`entity-descriptor.md` §4.5.1, §9.3): objects whose lists name
layer 1 or layer 2 are both shown in the shared world, and objects whose lists name only
named layers are hidden until the server makes such a layer active. No observation is
needed; the spot (region 1, block 0xA15B) stays available as an optional confirmation.

<a id="o4"></a>
## O4. One-triangle placeholder parts

**Spec:** `setup.md` §8.5 (OPEN); found by `m2-visual.md` row C6. 259 mesh records draw a
single triangle; most use the plain white surface 0x310000E5. Tessera's viewer draws them,
so creature 0x79018D38 (a small humanoid creature; setup 0x04000110, part 2 = mesh
0x060068B9) shows a white triangle at its feet.

**First, see ours:** `tessera-explorer --select 0x79018D38 --view mesh --mesh-angle 20,10
--mesh-select 2,0` highlights the triangle.

**What to do:**

1. By eye: find a creature of the same kind in the original, approach it at the highest
   model detail and look at the ground near its feet from several angles. Record
   "triangle visible" or "not visible".
2. Frame capture (settles it): capture a frame with that creature in view. Look for a draw
   of 3 indices whose vertex positions match a single small triangle near the creature's
   feet, with a plain white texture. Record whether such a draw exists; if it does, its
   blend state, depth write, colour write mask and the alpha of the material colour it
   receives.

<a id="o5"></a>
## O5. Dye blend

**Spec:** `material.md` §12.3 (OPEN, frame capture): hard threshold on the mask
(alpha < 128 ⇒ multiply) or a smooth blend; linear or gamma space; the role of the dye
colour's alpha (1.0 in most entries, 0.45 in 49); any brightness boost of the grey base.
Tessera uses the smooth form.

**First, see ours:** `tessera-explorer --select 0x79002B7C --view mesh --mesh-choice 1
--dye d02020` (chest armour, red) and `tessera-explorer --select 0x790698CC --view mesh
--mesh-choice 1 --dye 2040d0` (cloak, blue), as in `m2-visual.md` rows A2 and A10.

**What to do:**

1. By eye: dye the same pieces in the original if you have them (otherwise any dyeable
   armour piece) with one strong dye, using the dye preview. Zoom in on the boundary
   between dyed and undyed areas. Record: hard edge or soft gradient; whether mid-tone
   areas of the mask look partly dyed; whether the dyed colour looks brighter or darker
   than ours with the same dye colour.
2. Frame capture (settles it): capture the dyed piece. In the pixel shader's constants
   find the dye colour and record its four values; record whether the shader compares
   the mask alpha against a constant (threshold) or interpolates with it; and whether the
   render target and texture views are sRGB (gamma-space or linear-space multiply).

**Answered from the client (2026-10-08).** No observation is needed. `material.md` §12.3
(MA24 from the compiled shader programs, BIN-S; MA33 from the graphics back ends, BIN-T (g))
already holds every answer: the blend is smooth, dyed colour = base × (1 − *w*) + base × dye
× *w* with *w* = 1 − mask alpha, applied to the sampled texture values before lighting; there
is no threshold and no brightness boost; the dye colour's alpha is not used; no back end uses
sRGB textures or frame buffers, so the multiply works on the stored values. Steps 1 and 2
remain an optional confirmation.

<a id="o6"></a>
## O6. Other material and render-state questions (frame captures)

**Spec:** `material.md` §17 lists what only a capture can settle; §18 and §19 add the
items raised by the implementation. One capture session can answer several at once. For
each, record the per-draw pipeline state (blend enable and factors, depth test and write,
cull mode, alpha-to-coverage) and, where noted, shader constants.

| # | Question | Spec | What to capture | Record |
|---|---|---|---|---|
| O6.1 | Render-state bytes 4–6; does a lit base pass blend; is any lit surface translucent | §7.6, §11.3 | One lit opaque object, one additive effect (glow, fire), one alpha-blended effect (smoke) | Blend enable, factors, depth write for each draw |
| O6.2 | Is flag 4007 the two-sided switch | §11.4 | A leaf card or cloth edge seen from behind | Cull mode of that draw; whether the back is drawn |
| O6.3 | Alpha test with no opacity texture; alpha-to-coverage or soft edges | §11.2 | Foliage and a grille at close range, with anti-aliasing on | Whether a test happens, on which channel, at what threshold; coverage on/off |
| O6.4 | Opacity of unlit blended effects without opacity in alpha | §18 | One unlit blended effect card | Which texture channel drives its alpha |
| O6.5 | Lighting model: gloss, specular colour 2001, power 1000; environment map; specular power 0 | §10.2, §11.5, §19 | A shiny metal object near a light | Specular term constants; whether power 0 gives no highlight |
| O6.6 | Normal-map green channel (Y up or down) | §10.2 | Any normal-mapped wall lit from the side | Whether the shader flips green |
| O6.7 | Texture matrix: U/V order, composition order, coordinate set per slot | §8.4 | A surface with a rotated or offset detail texture | Texture-coordinate transform constants |
| O6.8 | Animated float curves (modes 2–10) | §5.3 | Scrolling water or a pulsing glow, as a short video or several captures over time | Value over time of the animated constant |
| O6.9 | Vertex colours and diffuse colour 2000 | §19 | Foliage cards and a snow-covered surface | Whether vertex colour or a material tint constant enters the colour |

**2026-10-08 (from the client, narrowed).** Rows O6.1–O6.9 are answered in `material.md` from
the compiled shader programs and the client (§17 items 1–8, §19, §20.1; MA22–MA49): blend
and depth state per pass (§7.6, §11.3), two-sided = flag 4003 (§11.4), no alpha test without
an opacity source and discard at or below the threshold (§11.2), opacity of unlit blended
effects (§20.1), lighting and specular (`lighting.md` §5; §8.3, §11.5), normal-map axes
(§10.2), texture matrices (§8.4), float curves (§5.3.1), vertex colours and colour 2000
(§19). The frame captures above are no longer needed. Left for the owner, by look only:

- **Soft foliage edges** (`material.md` §8.5 OPEN): with a tree close in view, switch each
  graphics option that could plausibly affect edges (anti-aliasing, alpha-to-coverage,
  material or texture detail) off and on once, and record which one turns the leaf edges from
  hard to soft.
- **Optional** (`material.md` §11.2 OPEN): whether cut-out foliage on an object that is
  fading in at the draw distance loses its hard edges during the fade.

<a id="o7"></a>
## O7. Command-line option leniency (low priority)

**Spec:** `install-layout.md` §10 (OPEN): whether the original client accepts
`--opt=value` and treats long option names case-insensitively. Tessera's parser accepts
both.

**What to do:** only if a launcher lets you add client arguments easily: start the client
once with a harmless option written as `--option=value`, once with its name in a different
case. Never touch or record login tickets or account options.

**Record:** accepted / ignored / error, for each form.

**2026-10-07 (from the client):** still answered by `install-layout.md` §10.1, which now also
settles which of repeated options wins (the last) and how `--debug` reads its number. No
launch test is needed for O7.

<a id="o8"></a>
## O8. Standing heights per species and sex (M4)

**Spec:** `appearance.md` §6.1 (OPEN), `skeleton-anim.md` §2.2 (OPEN). The data gives two
whole-body scales: the visual template's uniform `vec3` and the avatar entity's
`Render_AdjustableScale`. The River-hobbit entities carry 0.7 (their setups are human-sized),
Beorning entities 1.03, Elf and High-elf female entities 1.02, the Elf male template 1.03,
the High-elf templates 1.05 (male) and 1.03 (female). Tessera multiplies the two. Expected
heights at default sliders: Hobbit about 1.16/1.14 m, Human 1.81/1.77, Elf 1.85/1.80, Dwarf
1.36, Beorning 1.86/1.82, High-elf 1.90/1.84 (female 1.82 if the entity value replaces the
template value), River-hobbit 1.27/1.24 (male/female).

**What to do:** with characters of different species at the default height slider (new
characters are fine, or the character-creation screen), stand two at a time side by side on
flat ground, facing the camera, and take a screenshot each: Human with Elf, Human with
High-elf (both sexes), Human with Beorning, Human with River-hobbit, River-hobbit with
Hobbit. Measure the top of the head of each in pixels.

**Record:** for each pair, the height ratio (to two decimals), and whether a High-elf
female stands about 3 % or about 5 % taller than a Human female.

**2026-10-05 (informal):** the owner looked at characters in Tessera on the laptop and found
their heights fine by eye. No ratios were measured, so the record above is still wanted.

**2026-10-07 (from the client, partly):** `appearance.md` §6.1 and §17 now state that template
`vec3` values multiply along the template chain and that the render scale is a per-axis
product of the entity's `Render_AdjustableScale` and other object factors. Whether the
template `vec3` is one of those factors was not traced. Only that one question is left, and
one screenshot pair settles it:

- **What to do (reduced):** a High-elf female beside a Human female, default height slider,
  side by side on flat ground, facing the camera. Measure the head tops in pixels.
- **Expected:** Human female about 1.77 m. High-elf female about **1.84 m** (ratio about
  1.04) if the scales multiply, about **1.82 m** (ratio about 1.03) if the template value
  does not enter. If a High-elf male and a Human male are at hand, that pair shows the same
  thing more clearly: about 1.90 m against 1.81 m (ratio about 1.05) if they multiply, about
  the same height (ratio about 1.00) if not.
- **Record:** the ratio (to two decimals) for the pair or pairs taken. The other pairs above
  are welcome as a confirmation but no longer needed.

<a id="o9"></a>
## O9. Height slider and facial bone sliders (M4)

**Spec:** `appearance.md` §6.2 and §6.3 (OPEN). The height control drives
`AC_BoneScale_Test` from 0 to 1 (default 0.5); River-hobbits have a second height control.
The 35 facial sliders store one byte each (0x80 default) between stored limits; which byte
gives the limit, and whether the effect is in the bone's own frame, is not in the data.

**What to do** (character-creation screen, no character needs to be saved):

1. Human male: move the height slider fully left, centre, fully right; screenshot each from
   the side at the same zoom. Repeat for a River-hobbit (both height controls if shown).
2. Human male: for the nose-bridge scale slider (the strongest: 0.7 to 1.3) and the eye
   separation slider, screenshot the face from the front at the left end, the centre and the
   right end.
3. Note whether the centre position of a slider looks exactly like the default face.

**Record:** for 1, the head-top height ratio left : centre : right; for 2, whether the left
end is the smaller (narrower) one, and whether eye separation moves both eyes symmetrically;
for 3, yes/no.

4. If time allows, repeat step 1 (and the body sliders the creation screen shows, such as
   build or weight) for one body of every other playable species and sex, and note the
   numbers the UI shows at each end, if any.

**Also record (M4 River-hobbit rows):** whether a River-hobbit's feet rest on the ground at
the default height slider, in idle, walk and run. Tessera's figures float about 0.09 m when
the two hidden leg-scale sliders are applied as `appearance.md` §6.3 decides (rows B37–B42
of [`m4-visual.md`](m4-visual.md), note 1); this observation and the slider frame settle it.

**2026-10-07 (from the client, partly):** the rules are answered (`appearance.md` §6.2–§6.5,
§17). The left end of a slider gives the stored minimum (the smaller value, for example the
narrower nose bridge); eye separation moves both eyes symmetrically; the default value
(0x80) lies a hair off the slider's centre value (0.502 of the range instead of 0.5);
sliders deform the mesh once about each bone's own joint, not the animated pose. The slider
frame question above is settled, and steps 2 and 3 need no observation (they remain
welcome as a confirmation). Still for the owner, as confirmation only:

- **Step 1:** the head-top heights at the left end, centre and right end of the height
  control for a Human male and a River-hobbit (both controls if shown), compared with
  `appearance.md` §6.5.
- **River-hobbit feet:** whether they rest on the ground at the default height slider in
  idle, walk and run (Tessera computes them within 1 cm of the ground in idle; walking is
  the open part).
- Step 4 stays optional.

<a id="o10"></a>
## O10. Worn-item layering and ties (M4)

**Spec:** `appearance.md` §4.4 (normative rule: per body slot the highest part priority
wins), §4.6 (cases) and §4.7 (ties, OPEN). Check items are rows W and L of
[`m4-visual.md`](m4-visual.md); use similar items you own if not those exact ones (the
wardrobe preview or the dressing-room view is enough).

**What to do:** check these situations on a Man or Elf character:

1. A floor-length robe over leggings: are the leggings visible anywhere?
2. A long robe that shows its own shoes, with tall boots, then with low shoes: which
   footwear shows?
3. A long-sleeved chest piece with gloves: are the gloves visible?
4. A hooded cloak with a helmet: hood or helmet? With a very large full helm?
5. Battle gauntlets (or any gloves) with a chest piece that hides gloves: which hands show?
6. A hat that shows "hair under the hat": is it the character's own hairstyle or a generic
   one (compare two characters with different hairstyles)?

7. A helmet with the client's "hide helm" option off, then on: is the hair (and the beard)
   the character's own style when the helmet is hidden?
8. If you have the exact items of rows W1–W22 and L1–L8 of [`m4-visual.md`](m4-visual.md)
   (or close look-alikes), compare each with the explorer's figure
   (`tessera-explorer --character human,m --wear <DID> --clip idle`).

**Record:** one short answer per item (what shows). For 5, which one wins when both are
equipped, and whether the order of equipping matters. For 7, own style / generic / bald. For
8, `same` / `different` per row (what differs, in your own words).

**2026-10-07 (from the client, partly):** `appearance.md` §4.7 and §17 settle the client's
rule: on equal priority the part applied later wins, in a fixed order of records and keys,
and the cases of §4.6 follow from the priorities. The order between **worn items** comes to
the original client from its server and is not in the client, so the owner is still
needed:

- **Item 5** (battle gauntlets or gloves with a chest piece that hides gloves) matters most:
  which hands show, and whether the order of equipping changes it.
- Items 1–4 and 6–8 remain useful as a confirmation of the priority rule.

<a id="o11"></a>
## O11. Updated-avatar option (M4)

**Spec:** `appearance.md` §3.3 (OPEN). Head, hair and beard lists name a plain style record
and a "toggle" record, switched by a boolean property that the avatar data sets to true. The
toggle record holds additional, later styles.

**What to do:** in the client's options, look for a setting that switches between original
and updated character models or appearances (its exact wording does not matter; do not
record it). Note its default state. If convenient, toggle it and see whether existing
characters' heads or hair change.

**Record:** setting present yes/no; default on/off; whether toggling changes faces or hair.

**2026-10-07 (from the client):** answered (`appearance.md` §3.3, §3.5, §17). The setting is a
graphics option, on by default. With it on, the client uses the toggle (newer) style
records and the newer face compositor; turning it off rebuilds the figures with the older
records. Character creation offers the check box only for Hobbit and Dwarf. Nothing is
required; a glance at the options screen to confirm the default (on) is welcome.

<a id="o12"></a>
## O12. Random-appearance NPC figures (M4)

**Spec:** `appearance.md` §7.4 (OPEN). About 43,000 placed NPC figures and most server NPCs
name only a "random appearance type"; no table mapping types to clothes was found in the
data. Tessera draws such figures with their rig's default body. Spot: row N5 of
`m4-visual.md` (landblock 0x80214F79; the figure holds a tool in its right hand).

**First, see ours:** `tessera-client --spawn 1,79,121,151.07,145.72 --heading 0 --pitch -10`
(the sandbox backend, standing 5 m south of the N5 figure and facing it; the walker places
it at the data's position, row N5 of [`m4-visual.md`](m4-visual.md)).

**What to do:** visit the spot (the landblock is in region 1; ask the spec writer for the map
area if needed) and look at the figure; log out and back in (or come back another day) and
look again. Do the same at one more figure of the same kind nearby.

**Record:** "same look both visits" or "different look"; whether the two nearby figures of
the same kind look alike; whether the figure wears more than a plain default outfit (for
example a hat, a cloak, a distinct shirt and trousers).

**2026-10-07 (from the data and the client, partly):** where the outfits come from is answered
(`appearance.md` §7.4, §17): the figures' start-up scripts pick one random piece per group,
and 316 of the 318 placed random-appearance types have outfits. So these figures wear more
than a default outfit. Whether the original picks per object (same look on every visit) or
anew each time was not traced; Tessera keeps one look per figure (seeded from the instance).
Still for the owner:

- **What to do (reduced):** visit one such figure (the N5 figure is fine) twice, logging out
  and back in or on another day.
- **Record:** "same look both visits" or "different look". The second nearby figure and the
  outfit description are now optional.

**2026-10-08 (from the client, narrowed):** the client draws these random picks from its single
global generator, which it seeds once from the wall-clock time at start-up; nothing in the
pick seeds it per figure (`script-runtime.md` §8, R40; `appearance.md` §7.4). So a **different
look on each visit is expected**. A game-logic reseed before the figure's start-up script was
not ruled out, so the two visits above remain the confirmation; nothing else is needed.

<a id="o13"></a>
## O13. Where held items rest (M4)

**Spec:** `appearance.md` §5.3–§5.4 and `skeleton-anim.md` §8.5 (OPEN). The data names
in-hand and stowing locations (hips, back) on every player body but nothing selects between
them; Tessera's interim rule is in `appearance.md` §5.4.

**What to do:** on one character, equip in turn a one-handed sword (and a second one-handed
weapon in the off hand), a two-handed sword, a two-handed axe or hammer, a staff, a spear, a
bow, a crossbow, a shield, a musical instrument, and a class item if your class has one.
For each, note where it rests out of combat and where it is in combat stance.

Also note whether a shield or other off-hand item sits on the forearm or in the hand, and
whether a long weapon held in one hand points forward (along the line of sight) or along
the forearm (Tessera follows `skeleton-anim.md` §8.3: the item's +Y forward; the checklist
asks which is meant, [`m4-visual.md`](m4-visual.md) note 3). Rows H1–H25 of
[`m4-visual.md`](m4-visual.md) list the items and Tessera's locations
(`tessera-explorer --character human,m --wear <DID>[@sheathed] --clip idle`).

**Record:** a two-column table per item: resting place (right hip, left hip, back diagonal,
back straight, in hand, hidden) and combat place (right hand, left hand, forearm, both
hands); and per H row `same` / `different` from Tessera, wielded and stowed.

**2026-10-07 (from the data and the client, partly):** answered for weapons, shields and
ranged items (`appearance.md` §5.6, §17): stowed and wielded locations, offsets, rotations
and per-species scales come from placement scripts by equipment category. Class items and
instruments have no placement script (§5.3); instruments appear while they are played.
Still for the owner:

- **In-hand direction:** one side-view screenshot of a stowed staff and one of a drawn sword
  (does the long weapon point along the line of sight or along the forearm, and does the
  stowed staff lie as Tessera shows it).
- **Class items:** whether a class item is ever visible on the figure (yes/no, and when).
- The per-item table and the H rows above remain useful as a confirmation.

<a id="o14"></a>
## O14. Animation playback (M4)

**Spec:** `skeleton-anim.md` §15 lists six owner observations. One session covers them:

| # | Question | Spec | What to do | Record |
|---|---|---|---|---|
| O14.1 | Blend method and cross-fade time between clips | §6.5; `script-table.md` §8 (0.2 s walk/run cross-fade in the data) | Short video (60 fps) of a character going from standing to walking, walking to running, running to standing | Approximate cross-fade time in frames for idle→walk, walk→run and run→idle (instant, under about 0.3 s, longer); any visible pop |
| O14.2 | Upper-body clips layered over locomotion | §6.6 | Attack or use a skill while running | Do the legs keep running during the attack? |
| O14.3 | Turn direction of turn-in-place clips | §7.3 | Watch a four-legged creature turn on the spot (or an NPC that turns in place) | Which way it ends up facing (left/right of start) |
| O14.4 | Walk, run and backward speeds; foot sliding | §7.4; `script-table.md` §15 (ground speed) | On flat ground, type `/loc`, move straight for a timed 10 s (forward walk, forward run, backward), type `/loc` again; repeat per species if they seem to differ | Metres per second per gait (and per species if different); feet slide yes/no |
| O14.5 | Scale inheritance from parent bones | §5.4 | Watch a creature attack that visibly inflates or shrinks a body part | Do the children (for example a hand on a growing arm) grow too? |
| O14.6 | Runtime cloth on capes and skirts | §13 | Stand still in wind, then turn quickly on the spot | Does the cape swing on its own, or only follow the animation? |
| O14.7 | Turning in place | `script-table.md` §8 (turn branches) | Turn a standing character slowly with the keyboard, then quickly | Does a turn animation play (feet step), or does the body rotate rigidly? |

**2026-10-07 (from the client, partly):** `skeleton-anim.md` §15.1 gives the status per row.

- **Answered, no observation needed:** O14.3 (the turn comes from the movement system, not
  from the clips), O14.5 (children grow too, but their joints stay where the unscaled parent
  puts them), O14.6 (there is no cloth: capes and skirts move only with their clips). The
  clip crop is answered too (`havok-data.md` §2.3).
- **Answered, a visual check is welcome:** O14.1 (linear blending; walk ↔ run is a 0.2 s
  cross-fade; idle ↔ walk and idle ↔ run follow the player's movement ramp, about 0.13 s
  running and 0.25 s walking, so about 8 or 15 frames at 60 fps; no pop), O14.2 (legs keep
  running during an attack while moving; a standing attack moves the whole body), O14.7 (a
  standing turn plays stepping turn clips while the body rotates).
- **Still for the owner, O14.4:** the running speeds are now known from the data (forward
  6.5 m/s, backward 3.25 m/s, strafe 5.0 m/s), but the walking speed depends on an input
  value that was not traced. Do the 10 s `/loc` timing for **walk forward** (walk toggle on),
  **walk backward** and **walk strafe**; run forward, backward and strafe only as a
  confirmation. Note whether the feet slide (the client does not match the clip rate to the
  speed, so some sliding is expected). This is the same measurement as O18 step 1; do it once
  for both.

**Answered from the client (2026-10-08).** O14.4 is answered too, so every row of O14 is
answered and nothing is required (`collision.md` §11.8, BIN; `skeleton-anim.md` §7.4,
§15.1). Walk mode halves the key values, which selects the Walking layer: walking moves at
**2.5 m/s** forward, **2.25 m/s** backward and **2.57 m/s** sideways; running at 6.5, 3.25 and
5.0 m/s (no speed effects active). The walk clips cover about 2 m/s, so the feet slide a
little when walking. The `/loc` timing and the visual checks of O14.1, O14.2 and O14.7 are
optional confirmations.

<a id="o15"></a>
## O15. Skin, hair and eye colours; face layers (M4, frame capture, low priority)

**Spec:** `appearance.md` §3.3 (OPEN, out of scope for M4). Characters store skin, hair, eye,
lip and war-paint colours as RGBA properties, and face details (complexion, eyebrows, mouth,
scars, wrinkles, stubble) as texture-layer indices composited by 0x33 records.

**What to do:** capture one frame of a character's face close up. In the head draw call,
find the shader constants that hold colours, and the textures bound.

**Record:** whether the skin colour is a constant multiplied into the texture or baked into a
generated texture; whether the face texture is one composited image (generated at run time)
or several layered textures; the same for hair (constant tint on a grey texture, or baked).

**2026-10-07 (from the data and the client, partly):** the mechanism is answered
(`appearance.md` §3.5, §17). The face is one texture composited at run time from the face
layers and the character's colours; hair, beard and the skin surfaces are tinted through
their dye colour, set from the hair and skin colours; a second graphics option, on by
default, turns face compositing off. Colours are stored B, G, R, A (`property-system.md`
§4.7). Still for the owner (low priority):

- **What to do (reduced):** the same one frame capture of a face close up.
- **Record:** which texture slot of the face-skin draw holds the composed face image. The
  other questions above are now only a confirmation.

**2026-10-10 (Tessera's composed faces, one comparison added):** on a Dwarf M figure with
the default colours, Tessera shows saturated orange eyebrows (the compositor's eyebrow colour
multiplied by `AC_Color_Hair`) next to dark brown hair and beard (the same colour as their
dye colour 2003).

- **What to do:** look at a Dwarf M character's face close up in the original client, next
  to the same character in Tessera (`tessera-explorer --character dwarf,m`).
- **Record:** whether the eyebrows have the same hue and brightness as the hair and beard,
  or are visibly more orange or brighter, as in Tessera. No code change unless the original
  differs.

**2026-10-10 (texture slot answered; one comparison added):** the client code gives the slot
(`appearance.md` §3.5: the composed face is the surface's base-colour texture, parameter 3000),
so the frame capture for the slot is no longer needed. New, low priority:

- **What to do:** in the original client, turn the face-compositing graphics option off, then
  look closely at the faces of a Human M (or a River-hobbit) character and of a Hobbit M
  character (log out and in again if the faces do not change at once).
- **Record:** for each, whether the face still shows the character's chosen face details
  (eyebrows, scars, war paint, eye colour) or a plain face. From the data alone the Human M and
  River-hobbit faces should stay detailed and the Hobbit M face should turn plain
  (`appearance.md` §3.5, table "When face compositing is off"); a plain Human M face would
  mean the server sends a value the install does not hold.

**2026-10-10 (one more face, same session as above):** whether an elf figure's start-up
scripts run before or after it is first placed in the world could not be read from the client
(`appearance.md` §3.5, narrow OPEN). The same comparison settles it.

- **What to do:** with the face-compositing option off, log in (or switch) to an Elf or
  High-elf character and look closely at its face.
- **Record:** whether the face keeps the chosen details (eyebrows, scars, war paint, eye
  colour) or turns plain. Detailed means the figure was placed before its start-up ran (the
  option is not honoured for the elf bodies); plain means the start-up ran first, as Tessera
  assumes.

<a id="o16"></a>
## O16. Which idle, walk and run clips play (M4)

**Spec:** `script-table.md` §8 (how the scripts choose clips; §8.3 lists the clips Tessera
resolves per body) and §16 (OPEN: the resolver also reaches random idle fidgets that §8.6
says avatars do not have). [ROADMAP](../ROADMAP.md#m4) M4 criterion 2.

**First, see ours:** `tessera-explorer --character <species>,<m|f> --clip idle` (then
`walk`, `run`); the inspector lists the chosen clip and the other candidates.

**What to do:** for one character of each playable species and sex you can reach (new
characters at the starting area are fine), stand still for about 5 minutes (was "about a
minute"; see the note of 2026-10-07 below), then walk and run in a straight line, and
compare with the explorer's figure of the same species and sex.

**Record:** per species and sex: whether idle shows variants or fidgets (and roughly how
often, for example "every 10–20 s"); for idle, walk and run, `same` / `different` from
Tessera's clip (describe a difference in your own words, never by a name from the game).

**2026-10-07 (from the data and the client, partly):** `script-table.md` §15.1.

- **Fidgets:** avatars do fidget (§8.6). The predicted timing: the first pick comes after
  about 3.6 minutes of standing still (idle, on the ground, out of combat, not emoting); it
  is a fidget or a short pause; further picks follow about every 10 s while the character
  stays idle. One minute is too short to see any, so **stand still for about 5 minutes** and
  note when the first fidget comes and how often they follow. Still open in the spec:
  whether something restarts the long first wait.
- **Mood idles:** answered from the data; no observation needed.
- **Which idle, walk and run clips play:** still for the owner. The data path is complete;
  only the side-by-side look can say `same` or `different`. One character per species and
  sex you can reach, as above.

<a id="o17"></a>
## O17. Static NPCs at the N spots (M4)

**Spec:** rows N1–N7 of [`m4-visual.md`](m4-visual.md) (positions and headings from the
data) and `appearance.md` §7. The walker places these figures exactly where the data
places them (`apps/client/tests/npc_spots_real_data.rs`).

**First, see ours:** `tessera-client --place "<the spot's /loc line>"` (for N4 and N6 add
`--content-layers 1,0x10000019` or `--content-layers 1,0x10000536`).

**What to do:** at each N spot you can reach (ask the spec writer for the map area if
needed), stand next to the figure, type `/loc`, and note the compass direction it faces.
Note whether it is there at all for a character with no special progress (N4 and N6 are on
content layers; see also [O3](#o3)).

**Record:** per row: present yes/no; the `/loc` numbers; the facing as a compass bearing
(±10° is enough); `same` / `different` from the walker's figure (position within about a
metre, facing, pose such as sitting or standing, held item).

**2026-10-05 (informal):** the owner looked at characters in Tessera on the laptop and found
their positions fine by eye. No `/loc` readings or facings were taken, so the record above
is still wanted.

**2026-10-07 (from the client, partly):** presence, position and facing are answered
(`entity-descriptor.md` §9.4). The client places these figures like any other object, at
their stored position and facing, without ground snapping; the expected facings of N1–N7
are in that section's table. N1, N2, N3 and N5 are present in the shared world; N7 is
present but is a hidden marker, so no figure is expected there; N4 and N6 sit on named
content layers and appear only while the server makes their layer active. Still for the
owner:

- **N4 and N6:** whether each figure is there for your character (yes/no, and the
  character's level band). This is server state, not client code.
- **Look:** at each spot you reach, `same` / `different` from the walker's figure for pose,
  dress and held item (governed by `appearance.md` and the animation specs).
- The `/loc` numbers and the facing are now only a confirmation.

---

<a id="m4-5-summary"></a>
<a id="m4-5-items"></a>
## M4.5 items (world fidelity, added 2026-10-06)

The spots, walks and rows these items refer to are in
[`m4-5-visual.md`](m4-5-visual.md). Every item compares the original client at its default
graphics settings (record any setting you changed) with the expected facts listed there.

| ID | Question | Spec | Needs | Status |
|---|---|---|---|---|
| [O18](#o18) | Movement: jump, speeds, highest step, steepest slope, wading and swimming depth | `collision.md` §10–§12 (Q9, Q11), `water.md` §7.4 | Stopwatch or short video | mostly answered from the client (2026-10-08: speeds, highest step, steepest slope, swimming depth, `collision.md` §10.5, §11.3, §11.5–§11.6, §11.8); left for the owner: the jump's height and airtime (predicted 1.2 m, 0.8 s, §11.10) |
| [O19](#o19) | Outcome of the 32 scripted walks in the original | `m4-5-visual.md` §2 | In-game `/loc`, look | open; not answerable from the client (outcome in the original; needs `/loc` and look) |
| [O20](#o20) | Side-by-side look at the terrain, scenery and frill rows | `m4-5-visual.md` §4.1–§4.3 | Look, private screenshots | open; not answerable from the client (visual comparison) |
| [O21](#o21) | Game clock: time of day, day length, epoch | `environment.md` §6, §10.1, §11.1 | Clock in the UI or `/loc`, stopwatch | partly answered (`environment.md` §10.1, §11.1, §11.5: rate, date, night flag, day-type rule); left for the owner: two (in-game time, UTC) readings for the epoch, and the weather on several days; unchanged 2026-10-08 (the epoch is not in the client) |
| [O22](#o22) | Default draw distances: landscape extent, object pop-in, fog clipping | `walker.md` §11.3–§11.4, `mesh.md` §8, `environment.md` §11.7 | Look, `/loc` | partly answered (`environment.md` §10.1, §11.7: fog-distance limits; the distance rules were already from the client); left for the owner: steps 1–6 as a visual check; unchanged 2026-10-08 (visual check) |
| [O23](#o23) | Sun shadows and point lights | `lighting.md` §8–§9, `m4-5-visual.md` §4.6 | Look at a set time, frame capture optional | open; not answered from the client in this pass (shadow colour, cascades and fade stay `lighting.md` §9 OPEN; the rest is visual) |
| [O24](#o24) | Water look: surface, lava, placed water, distant water, puddles cut by the ground (W13), puddle colour against the mud (W14), river bed and ripples (W17) | `water.md` §8–§9, `m4-5-visual.md` §4.4, §4.8 | Look, frame capture optional | open; not answerable from the client (visual comparison; reflection, flow and foam stay `water.md` §8 OPEN) |
| [O25](#o25) | Frills: radius, fade, density, sway, terrain tint | `scenery.md` §8.3–§8.4 | Look, short video | open; not answerable from the client (visual; radius, fade and density are already the client's, `scenery.md` §8.3–§8.4) |
| [O26](#o26) | Sky and fog at four times of day; the day type; interior fog | `environment.md` §10.1, §11.2–§12, `lighting.md` §10 | Look at set times | partly answered (`environment.md` §10.1: day-type rule, interior fog rules, sun and moon path); left for the owner: the Y1–Y4 look at set times (after O21), sun and moon size, fog on the sky; unchanged 2026-10-08 (look at set times) |
| [O27](#o27) | Terrain near and far bands, and the seam between them | `terrain-texturing.md` §12 | Look, short video | open; not answerable from the client (visual comparison) |

<a id="o18"></a>
## O18. Movement measurements (M4.5)

**Spec:** `collision.md` §10.3 (movement records), §11 (layering, walkable 45°), §12.3
(Tessera's controller, built where the original's integration is OPEN, Q9 and Q11);
`water.md` §7.4 (Tessera's wade/swim rule, OPEN in the original). Tessera's values for a
Human male: run 6.5 m/s, walk 5 m/s, backward 3.25 m/s, strafe 5 m/s, swim 3.2 m/s; step-up
0.5 m; jump about 0.9 m high and 0.7 s in the air (a Tessera Decision); wading until the
water surface is 1.42 m above the feet, swimming beyond.

**First, see ours:** `tessera-client --walk-path` with walks ST1, SL1 and WA1 of
`m4-5-visual.md` §2.3, and `--spawn 1,161,151,127.81,89.59` (walk WL1's start) for open
ground.

**What to do** (a Human male character, no mount, no speed buffs; flat open ground such as
the start of walk WL1):

1. **Speeds.** Type `/loc`, hold one movement key for a timed 10 s, type `/loc` again; once
   each for run forward, walk forward (walk toggle on), backward, strafe left, and swimming
   forward in deep water (WA1, after the swim point). If a character of another species is
   at hand, repeat run and walk with it and note whether the speeds differ.
2. **Jump.** Jump on flat ground next to a wall with known height (any SED of the walk
   table) and film it from the side at 60 fps; or jump onto ever higher steps until it
   fails. Count the frames from take-off to landing for the airtime.
3. **Highest step.** At ST1–ST3 walk up the platform steps; then find a ledge you cannot
   walk onto without jumping and one you can, and type `/loc` on the ground before each and
   on top (the difference in `oz` is the step height).
4. **Steepest slope.** At SL1–SL3 run straight uphill: note whether the character climbs,
   stops or slides back, and type `/loc` at the highest point reached.
5. **Wading and swimming.** At WA1–WA4 walk slowly into the water and type `/loc` at the
   moment the swimming animation starts.

**Record:** metres per second per gait (one decimal); jump height (to 0.1 m) and airtime
(to 0.1 s); the `oz` values of step 3; for each SL walk the highest `oz` and whether the
character slid; for each WA walk the `/loc` where swimming started (compare with the
`swim=` point of the row; the depth there is the original's swim depth).

**2026-10-07 (note):** step 1 overlaps O14.4. `skeleton-anim.md` §7.4 now takes the running
speeds from the data (they match Tessera's values above); the walking speed depends on an
input value that was not traced, so the walking timings are the ones that matter. Steps
2–5 are unchanged and still open.

**2026-10-08 (from the client, mostly answered).** Steps 1, 3, 4 and 5 are answered from the
client's code and the data; the measurements above are optional confirmations
(`collision.md` §10.5, §11.3, §11.5–§11.6, §11.8):

- **Step 1, speeds:** running 6.5 m/s forward, 3.25 m/s backward, 5.0 m/s sideways; walking
  2.5, 2.25 and 2.57 m/s; swimming 3.2 m/s forward. The same for every playable species
  (only server-set speed effects change them). Tessera's walk of 5 m/s above is not the
  original's.
- **Step 3, highest step:** the client lifts the character by a fixed 0.55 m (0.5502 m) when
  a ledge blocks it and keeps the step only when it lands on walkable ground, so ledges up to
  about 0.55 m are climbed without jumping.
- **Step 4, steepest slope:** players can walk on ground up to 45°; on steeper ground they
  cannot climb and slide down (no control; the Slope layer's gravity of 10 m/s²).
- **Step 5, swimming depth:** the character swims once the water at its feet is deeper than
  its float height: Human, Elf, Beorning, High-elf 1.42 m (male) and 1.40 m (female),
  Hobbit 0.92 m, Dwarf 1.05 m, Stout-axe dwarf 1.12 m, River-hobbit 0.985 m (male) and 0.97 m (female); shallower
  water is wading.

**Still for the owner, step 2 only (reduced):** the jump's push is now known from the client
and the data (6.0 m/s upwards on land, `collision.md` §11.10), which predicts a jump about
**1.2 m** high and about **0.8 s** in the air; the take-off itself was not traced end to end.
Jump once on flat ground beside something of known height and film it from the side (or count
frames); record the height (to 0.1 m) and the airtime (to 0.1 s).

<a id="o19"></a>
## O19. Scripted walks in the original (M4.5)

**Spec:** `m4-5-visual.md` §2.3 (32 walks with expected outcomes) and §2.2 (pass
conditions).

**First, see ours:** `tessera-client --walk-path <id>` for any row (the table form of the
checklist is accepted directly).

**What to do:** for every walk you can reach (`n/a` otherwise; interiors IN1–IN4 lie in
instanced spaces), go to the start (`loc` column), face the heading, and do what the
`inputs` column says by hand (hold forward, or walk to the listed end point). Type `/loc`
at the end.

**Record:** per walk: the final `/loc` numbers (no server name, no time stamp); `blocked`
/ `reached` / `slid back` / `wading then swimming` in your own words; for WL, UB and GN
whether the character stopped at the object (and roughly how far from it, that is how
deep the body seems to sink into the wall or how wide a gap it keeps); for UB whether
anything is visible there; for GN whether the tree or rock is there at all (it is
generated from the data, so its absence would mean a generation difference).

<a id="o20"></a>
## O20. Terrain, scenery and frill rows side by side (M4.5)

**Spec:** `m4-5-visual.md` §4.1 (terrain layers, T1–T16), §4.2 (generated scenery,
S1–S11), §4.3 (frills, F1–F6).

**First, see ours:** `tessera-client --screenshot ours.png --spawn R,X,Y,x,y --heading H
--pitch P --fly` with each row's eye; the explorer's terrain and scenery views list the
layer stack and the generated objects.

**What to do:** stand at each row's eye position in the original (time of day around
noon), look in the row's direction, and compare with Tessera's screenshot of the same
row. For S rows count the trees and rocks in view; for T rows look at the ground texture
under the target (roads, snow, paving, normal-mapped relief, highlights); for F rows look
at the grass and ground cover near the target.

**Record:** per row `same` / `different` (pass / fail); a difference in your own words (for
example "road edge softer", "extra rock at the left", "grass sparser"). Screenshots stay
private. The other rows of `m4-5-visual.md` are compared the same way under O24 (water,
W1–W9), O26 (sky and fog, Y1–Y5) and O23 (lighting, L1–L7), so that together these items
give a pass or fail for every row of criterion 6.

<a id="o21"></a>
## O21. Game clock and epoch (M4.5)

**Spec:** `environment.md` §6 (day length 11,160 s, 360 days per year, zero offset
3,600 s, zero year 34) and §11.1 (the clock); the epoch of the clock is OPEN, so Tessera's
clock has a settable start. The `/loc` reply names the game time (game text; record only
numbers).

**What to do:**

1. Note the in-game time the client shows (clock or the `/loc` sentence, or the sun's
   position if nothing else shows it) and the real date and time (UTC), twice, at least an
   hour apart.
2. Note the in-game date if the client shows one (day of the month, month number, year
   number, as numbers only).
3. If possible, note the real time at which dusk visibly starts (the light turns orange)
   and at which night starts.
4. Day type: at one fixed spot (row Y1), note the weather (clear, overcast, rain, fog) on
   several in-game days, together with the in-game date of each (see also O26).

**Record:** the pairs (in-game time, UTC) with the date; the in-game date numbers. From
two pairs the spec writer derives the rate (expected: one game day per 11,160 s, 3.1 h) and
the epoch.

**2026-10-07 (from the client, partly):** `environment.md` §10.1, §11.1 and §11.5.

- **Answered:** the rate (one game day per 11,160 real seconds at the default rate), the
  360-day year of 12 thirty-day months and the date formulas. There is no single dusk event:
  light and fog follow the time-of-day keyframes, and the night flag switches on at time of
  day 0.8 and off at 0.25. The day type is computed by the client at each game-day change
  by a fixed rule (§11.5).
- **Not in the client:** the epoch. The client's clock has no wall-clock start of its own;
  it is set from outside the game-time code (probably the connection layer, which this
  project does not analyse). So the owner is still needed.
- **What to do (reduced):** step 1, two (in-game time, UTC) readings at least an hour apart,
  with the real date. Step 2 (the in-game date numbers) helps check the year and month.
  Step 3 is now optional: once the epoch is known, the night times follow from it. Step 4
  (the weather at row Y1 on several in-game days, with each in-game date) checks the
  day-type rule against the live game, which also depends on the server; do it if
  convenient, together with O26.

<a id="o22"></a>
## O22. Default draw distances (M4.5)

**Spec:** `walker.md` §11.3 (landscape extent 2,080 m at the built-in High setting; the
global limit *G* is the fog far distance when that is shorter), §11.4 (object distances by
LOD class, 1.5 s fades), §11.8 (default profile).

**What to do** (default graphics settings; record them):

1. At perf spot P5 (`m4-5-visual.md` §3), look along heading 190: is the sea visible out to
   the horizon, and where does the landscape end?
2. Walk slowly toward a lone building from 400 m away (any WL row's building) and note the
   distance (from `/loc`) at which it appears, and whether it fades in or pops.
3. Do the same with a small object (a barrel or crate) and with a tree.
4. At night in the town of row L2 (fog far 300 m in the data), look along a long street:
   do buildings beyond about 300 m disappear?
5. Do the same as in 2 with another character (an NPC or another player): the distance at
   which it appears and finishes fading in.
6. How far grass and other frills reach and how dense they look (details under O25).

**Record:** the graphics preset in use; distances in metres (to 10 m); fade or pop; for 4,
yes/no and the distance; the farthest visible landscape (from 1).

**2026-10-07 (from the client, partly):** the distance rules were already from the client
(`walker.md` §11.3–§11.4, `mesh.md` §8.5–§8.7); `environment.md` §10.1 and §11.7 now add
that the outdoor fog far distance, which can shorten every draw distance, never exceeds
10,000 and never falls below 20 (in the same distance units as the 300 m of step 4). So steps 1–6 now check Tessera's picture against
predicted values rather than discover them: do them as described, as a visual check.
Step 4 (buildings vanishing at the fog distance at night) is the one this revision
touches.

<a id="o23"></a>
## O23. Sun shadows and point lights (M4.5)

**Spec:** `lighting.md` §8.1 (sun from the day description), §8.3 (point lights and their
attenuation), §9 (sun shadows; colour, cascades and fade OPEN); `m4-5-visual.md` §4.6.

**What to do:**

1. Row L1 around noon: note the direction the barn's shadow points (compass) and roughly
   how far it reaches from the wall; the shadow's colour (black, blue-grey, tinted);
   whether the shadow edge is sharp or soft; at what distance from the camera shadows stop
   being drawn, and whether they fade out or end at a line.
2. Row L2 at night: are the lamps lit; do they light the walls and the ground around them;
   roughly how far does the light reach (the data says range 10 m); is the colour orange?
3. Rows L3 and L4: do the fire and the crystals glow (bright regardless of the darkness,
   with a halo or bloom)?
4. Optional frame capture at L1: the shadow-map size and cascade count, and the shadow
   colour constants (`lighting.md` §9 OPEN).

**Record:** compass bearing (±10°) and length (±5 m) of the shadow; sharp/soft; shadow
distance; per lamp question yes/no and the reach; glow yes/no and halo yes/no; capture
values if taken.

<a id="o24"></a>
## O24. Water look (M4.5)

**Spec:** `water.md` §8 (drawing water; reflection, flow, foam and depth OPEN), §9
(distant water), §7.3 (placed water); `m4-5-visual.md` §4.4 rows W1–W9.

**What to do:** at each W row (daytime), look at the water and note: colour and
transparency (can you see the bed?), reflections (sky, shore), waves or flow direction,
foam at the shore, and any edge where two water heights meet (W6). At W7 note whether lava
glows and moves. At W8 note whether the pool surface is see-through. At W9 note whether
distant water looks like the near water. Optionally dive under at W4 and note the
under-water look (fog colour, distance).

**Record:** per row a few words per aspect; `n/a` where not reachable.

**Added 2026-10-10 (spec writer; `m4-5-visual.md` §4.8 rows W10–W17, `water.md` §8.5–§8.7):**
at each row's `/loc`, note what the rows' "Expected" column asks (waterfall streaming and soft
edges, fountain water, flow speeds). Two answers settle open points:

- **W13 (puddle):** is the puddle cut by the ground (its outline following the terrain where
  the ground rises above the water patch, as Tessera draws it), or does it show a rounded soft
  outline all round, as if drawn on top of the ground? The data predict the first
  (`water.md` §8.5.4); a different look would mean the original raises, biases or projects
  puddles in a way the data do not show. Note the graphics settings (reflections on or off).
- **W14 (puddles in the mine yard; added 2026-10-10):** what colour are the puddles against
  the mud: a reflection of the sky and the yard (darker or lighter than the mud?), and does
  each patch grow more opaque toward its far side? Tessera, without a reflection pass, tints
  them with the fog colour, so they read blue-grey and lighter than the mud
  (`m4-5-visual.md` row W14); the original's colour is the reflected scene × the diffuse
  (`water.md` §8.5.4).
- **W17 (river):** do moving ripples show (`water.md` §8.6.3 OPEN), and is the river bed
  visible through the water, or is the water dark at a few tens of metres? Note whether
  water reflections are enabled and which DirectX version the client runs.

<a id="o25"></a>
## O25. Frills (M4.5)

**Spec:** `scenery.md` §8.3–§8.4 (frill radius 97 m at the default level 2, fade from
67 m, reduction from 40 m, fade-in 1.5 s, terrain tint; shader formulas OPEN);
`m4-5-visual.md` §4.3 and walks FR1–FR3.

**What to do:** at rows F2 and F4 (dense grass), stand still and look along the ground:
estimate where grass ends and whether it fades out or shrinks; walk forward and watch new
grass appear (fade or pop); watch it in wind (does it sway, how much). Compare the colour
of grass with the ground under it (tinted toward the ground?). At walk FR1 note whether
the character walks through the ground cover without stopping.

**Record:** end distance (to 10 m), fade/shrink/pop, sway yes/no and strength in your own
words, tint yes/no; FR walks pass/fail.

<a id="o26"></a>
## O26. Sky and fog; day type; interior fog (M4.5)

**Spec:** `environment.md` §11.2 (the day type *D* is OPEN; Tessera starts with 2, which
selects only the local default table, `m4-5-visual.md` §1.1 item 3), §11.3 (blending),
§12 (sky); `lighting.md` §10 (fog); rows Y1–Y5.

**What to do:** at rows Y1–Y4 (or any spot with the same sky key) at the four times of day
of the rows (night 0.0, dawn 0.25, noon 0.5, dusk 0.78; use O21 to know when), look at the
sky, the sun or moon (its path and apparent size), clouds and stars, the fog colour and the
fog distance (where distant hills fade out). Do this under at least three sky keys
(regions). On two different real days note whether the weather at Y1 differs (rain,
overcast). At Y5 (IN1's dungeon) and in one cave, note the fog colour, its density and
the distance at which walls fade.

**Record:** per row: sky colour and fog colour in your own words, fog distance (to 50 m),
sun/moon position (compass and height above the horizon); weather per day; interior fog
distance and colour.

**2026-10-07 (from the client, partly):** `environment.md` §10.1.

- **Day type:** answered as a rule (§11.5): the client computes it at each game-day change.
  The "Tessera starts with 2" above was the default sky key, not a day type (§11.2). The
  weather on a given real day still depends on the epoch and on the server, so the weather
  at Y1 on several days (shared with O21 step 4) remains a useful check.
- **Interior fog (Y5 and a cave):** answered as rules (§7.2, §11.7); the colours and
  distances are data. Optional confirmation only.
- **Sun and moon path:** the rotation is answered (§11.4); their **apparent size** is still
  open, so note it (for example compared with a nearby object or the screen width).
- **Fog on the sky:** in dense fog the sky objects are not drawn at all; how lighter fog
  tints the sky is open. Note whether the sky shows through heavy fog and whether it looks
  tinted in light fog.
- **Still for the owner:** the Y1–Y4 look at the four times of day, as described above.
  Do O21 step 1 first: the real time at which to stand there follows from the epoch.

**2026-10-10 (new question, row W9 of `m4-5-visual.md` §4.4; `environment.md` §12):** from W9's
camera (high above a sea that ends at the landscape extent), Tessera shows the sky dome below
the horizon and, about 0.66° below the horizon, a thin darker line: the dome's equator, where
its texture wraps (the data's address mode; no sky-specific rule is known in the client). In
the original client, from the same camera at noon (*t* 0.5): what fills the view just below
the horizon beyond the sea (sky, fog colour, distant water or land)? Is a thin line visible
there? Record yes / no and, if there is a line, its colour compared with the sky above it.

<a id="o27"></a>
## O27. Terrain near and far bands (M4.5)

**Spec:** `terrain-texturing.md` §12 (distance bands and the near/far seam OPEN; Reported:
full detail to about 14 blocks, medium to about 24, then distant imposters).

**What to do:** at perf spot P5 and at row T3 (a high cliff top), look over a wide
landscape: note the distance at which ground textures lose detail or change colour, whether
a line or band is visible where they change, and whether it moves with the camera. A short
video while walking forward helps. At rows T1–T16 also note how sharp road edges are,
whether wet or rocky ground shows a specular sheen, and whether the landscape itself casts
shadows onto the terrain (hills shading valleys).

**Record:** distances (to 100 m) of each visible change; visible seam yes/no; moves with the
camera yes/no; road edge sharp/soft; sheen yes/no per row; landscape shadow yes/no.

---

<a id="m6-items"></a>
<a id="o28"></a>
## O28. Audio and player music (M6)

**Specs:** `sound.md` §16 (S-1 to S-5), `script-runtime.md` §26 (RT-1 to RT-5),
`abc-dialect.md` §11 (OPEN on the commands) and §17 (OC1 to OC10), `instruments.md` §16
(Q1, Q2, Q4, Q5, Q6). [ROADMAP](../ROADMAP.md#m6) M6 criterion 2 waits on O28.1.

**First, hear ours:** device output is opt-in (decision 0019):
`cargo run --release -p tessera-client --features audio-device -- --audio device --spawn
<region>,<block x>,<block y>,<x>,<y>` (the walker's HUD shows the audio line). Player music:
put the test file in the settings folder's `Music` folder and add
`--music-command "/play <name>"` (repeatable; `/playstart`, `/playstop`, `/playlist`).

**What to do:** in the original client, at places you choose (record them by `/loc` only,
never by name), make the observations below. For ABC checks write each small test file
yourself (original tunes only, as in the table of `abc-dialect.md` §17) and put it in the
original client's `Music` folder. **Do not try digit runs of 128 characters or more** in an
ABC file (`abc-dialect.md` §4.1: undefined behaviour in the original).

| # | Question | Spec | What to do | Record |
|---|---|---|---|---|
| O28.1 | Area-border music change (S-1, RT-1; M6 criterion 2) | `sound.md` §10.5, `script-runtime.md` §6.6, §24.1 | Walk across an area border with music playing | Does the old track fade out over roughly 1–4 s while the new area's music starts at once (or a silence begins)? Are both audible for a moment? The border by `/loc` and the approximate fade length |
| O28.2 | Crossing back and forth (RT-2) | `script-runtime.md` §6.6 | Cross a border and walk straight back within one track | Does the music restart (the track beginning again) each time? |
| O28.3 | Silence between tracks (S-2) | `sound.md` §10.4 | Stand in one area for several tracks with a stopwatch | Gap lengths in seconds (expected 5–30 s) |
| O28.4 | Positional falloff (S-3) | `sound.md` §6.2, §11 | Walk away from a positional emitter (waterfall, fire, torch), typing `/loc` at the emitter and where it falls silent | The distance in metres; whether loudness falls smoothly or steps |
| O28.5 | Day/night and indoor variants (S-4) | `sound.md` §10.3.5, §10.7 | At one place compare the ambient bed and the reverb at day and at night, and outdoors versus inside a building or cave | Does each change? When: on entering, or at a time of day? |
| O28.6 | Ogg played range (S-5) | `sound.md` §3.3.2 | Record the client's output while a short one-shot plays (ask the maintainer to name the sound info) | Its length in seconds, to compare with the spec's *M* and *N* |
| O28.7 | Combat music fade (RT-3) | `sound.md` §10.6, `script-runtime.md` §4.4 | Enter and leave combat in an area with music; a recording of the client's output helps | Does area music fade out over about 3 s while combat music plays, and fade back in over about 8 s afterwards? Loudness at the end of the fade (about −52 dB below the track expected) |
| O28.8 | Quest-dependent music (RT-4) | `script-runtime.md` §18 (OPEN-11) | In an area whose music depends on a quest, compare with the quest done and not done (the spec writer can name a place by `/loc`) | Same or different music |
| O28.9 | Pacing of rapid ambient loops (RT-5) | `script-runtime.md` §10.4, §11.2 | Listen close to a dense emitter such as a fire | Any burst faster than about ten per second? |
| O28.10 | ABC dialect checks OC1–OC10 | `abc-dialect.md` §17 | Put each OC file in `Music` and `/play` it | Per row: plays / refused (with the message, in your own words) / truncated, and the approximate timing |
| O28.11 | ABC commands | `abc-dialect.md` §11 (OPEN) | `/play` a file with several parts and no part number; try `/playlist` with a filter and with songs in a sub-folder; time `/playstart` after a synced `/play` and a solo `/play` | Which part plays; the filter syntax and whether sub-folders are listed; the delay to the first note in both cases (about 0.7 s expected) |
| O28.12 | Re-attack with a different volume | `abc-dialect.md` §9.3, `instruments.md` §15 (OC6) | Play a file that strikes the same pitch again with a changed `+dynamic+` mark | Does the re-struck note play or go silent? |
| O28.13 | Live play and release (Q1) | `instruments.md` §5.1, §9.2 | Hold a sustained instrument's note key for 2 s and for 10 s, then release; compare live loudness with `+mf+` and `+ffff+` song notes; play live with the quantize option on and off against a metronome | A short (about 0.2 s) release fade yes/no; live loudness relative to the two song marks; whether quantize moves notes onto a grid, and its step |
| O28.14 | Fast passages and the instance limit (Q2) | `instruments.md` §8.6 | Play a lute run of more than 10 notes within one second | Any dropped notes? |
| O28.15 | Quantize and from-song flags (Q4) | `instruments.md` §5.8 | `/play` a horn part that uses the top C with the quantize option on | Is the top C missing? Do loudness marks have no effect? |
| O28.16 | Several performers (Q6) | `instruments.md` §9.3, `sound.md` §6.6 | With several characters playing at once (a group, or several of your own clients) | Dropped notes once many voices sound? |
| O28.17 | Handknells octave (Q5, optional) | `instruments.md` §7.4 | Compare one Handknells note against a reference tone (a tuner app) | The octave heard |

**2026-10-08 (from the data and the client, narrowed).** Several rows are predicted exactly by
the original's scripts (data) and its script runtime (BIN); for those, listening is only a
confirmation:

- **O28.3, answered from the data:** the silences between tracks are one-shot timers of the
  music programs, mostly 5–30 s (15 s commonest, then 10, 20 and 30 s; `sound.md` §10.4).
- **O28.4, answered from the client:** loudness falls linearly from the sound's minimum to its
  maximum distance and is zero beyond; the distances are stored per sound (`sound.md` §6.2).
- **O28.7, answered from the data and the client:** entering combat runs a 3 s fade-out to
  0.05 of the music level (applied twice, so about −52 dB, `script-runtime.md` §4.4) and the
  combat music; leaving combat fades the area music back in over 8 s (`sound.md` §10.6).
  When combat starts is server state.
- **O28.9, answered from the client:** a looping sequence runs at most ten passes per second
  and a looping timer's cycle is at least 0.1 s (`script-runtime.md` §10.4, §11.2), so no
  burst is faster than ten per second.
- **O28.1 and O28.2, narrowed:** when the area music changes, the old track fades out
  linearly over its own ramp-out time (1–4 s, mostly 4 s) while the new area's programme
  starts at once; crossing back starts the first area's programme again from its first step
  (`script-runtime.md` §6.6, `sound.md` §10.5). Not in the data or the client code read so
  far: **where** the music changes (landblock or finer, and whether dungeon, landmark or area
  music wins; `sound.md` §10.3.1 OPEN). Still for the owner: cross one area border with music
  playing and record the `/loc` where the music changes and whether the old track fades for
  a few seconds; O28.2 is then optional.
- **Unchanged (listening needed):** O28.5 (which beds change at night and indoors; the night
  switch itself is at time of day 0.8 and 0.25, `environment.md` §6, §10.1), O28.6, O28.8
  (quest-dependent music: the quest state comes from the server and the game callback that
  reads it was not decoded, `script-runtime.md` §18 OPEN-11), O28.10–O28.17 (ABC and instrument
  checks: OC1–OC8 confirm `abc-dialect.md`'s predictions, OC9–OC10 settle its §4.1 OPEN).

---

<a id="m5-items"></a>
<a id="o39"></a>
## O39. Native panels in the original client (M5 criterion 2)

**Spec:** `ui-layout.md` §17 (panels P1–P15, their G, A, T and S rows) and the method of
[`m5-visual.md`](m5-visual.md). [ROADMAP](../ROADMAP.md#m5) M5 criterion 2.

**First, see ours:** the commands in the "Tessera-side self-check" section of
[`m5-visual.md`](m5-visual.md) (one run per panel with `--open-panel P<n>`).

**What to do:** at a known screen size (record it), interface scale 1.0, no user skin and
the default panel positions, take a screenshot of at least 10 of the panels P1–P15, plus the
button states listed in each panel's S row.

**Record:** the screenshots stay private (dirty-room scratch area). In `m5-visual.md` record
only pass / fail / n/a per row, the screen size and the date.

<a id="o40"></a>
## O40. The 14 census plugins in Tessera (M5 criterion 4)

**Spec:** `plugin-api-usage.md` §9 (the 14 plugins), `lua-plugin-api.md`. [ROADMAP](../ROADMAP.md#m5)
M5 criterion 4. Tessera's synthetic plugin suite passes; the real plugins have never been
run because the owner has no `Plugins` folder yet.

**What to do:** install the 14 plugins into a `Plugins` folder (optionally also the
publisher's 2010 sample package, to check that a user copy takes precedence over Tessera's
built-in class helper). Run `cargo test --release -p tessera-plugins --test
compat_real_plugins` with that folder in place (it prints a table per plugin and writes
nothing into the install), then start Tessera with the folder and try each plugin's windows.
For reference, also note in the original client what each plugin's windows look like and do.

**Record:** per plugin: loads yes/no, creates its windows yes/no, responds to input yes/no,
and any difference in your own words. The incompatibility list goes into the ROADMAP M5
status.

<a id="o41"></a>
## O41. A user skin in both clients (M5 criterion 3, optional)

**Spec:** `skin-definition.md`. Tessera's side passes with a generated skin
(`apps/client/tests/ui_skin_real_data.rs`).

**What to do:** if you use a published user skin, compare how the original client shows the
panels it maps (art and geometry) with Tessera started with `--skin <name> --skins-root
<folder>` on the same skin folder.

**Record:** per mapped panel: same / different, in your own words.

<a id="o42"></a>
## O42. Second press during mouse-look (decision 0014, Known difference 14)

**What to do:** in the original client, during mouse-look over the world, make a quick
second right press; then, while right is held, make a quick second left press.

**Record:** does the camera keep turning; does a double-click or tap fire; does the run chord
start.

<a id="o43"></a>
## O43. Wheel over a plain plugin window (decision 0014, D5 OPEN)

**What to do:** with a plugin window open that has no scrollable content, turn the mouse
wheel with the pointer over it.

**Record:** does the camera zoom, yes/no.

<a id="o44"></a>
## O44. Right press during an item drag (decision 0014, Known difference 20)

**What to do:** start dragging an item from a bag or a quickslot, then press the right mouse
button; repeat while trying the run chord.

**Record:** does the camera turn; is the drag cancelled; where does the item land; does the
run chord work during the drag.

<a id="o45"></a>
## O45. Escape with panels open (`game-panels.md` §16)

**What to do:** open several panels (for example the social, options and world-map panels),
then press Escape repeatedly.

**Record:** which panel closes first and in what order, and when the main menu opens.

<a id="o46"></a>
## O46. UI scale, auto-fade and edge blending (decision 0015, observations 1–3)

**What to do:** look at the chat window at interface scale 1.0, 1.5 and 0.5; watch a panel
fade below its top level; take a frame capture of a semi-transparent panel edge.

**Record:** text crispness and size at each scale; the look of the fade; the blend of the
edge pixels (numbers from the capture).

<a id="o47"></a>
## O47. Nested-surface test plugin (decision 0015, observation 4)

**What to do:** write a small test plugin for the original client: a window at opacity 0.5
holding a stretch-mode child, an overlapping sibling, and a child window with a stretch
control, at z = 30. Take a screenshot (kept private).

**Record:** how opacity, overlap and clipping combine, in your own words.

<a id="o48"></a>
## O48. Cursor and minimap marker (decision 0015, observations 5–7)

**What to do:** watch the cursor during mouse-look and when it ends; look at the OS cursor
size at compositor scale factor 1.333; look at the minimap marker pointing north, east and
at 45 degrees.

**Record:** cursor visible during mouse-look yes/no and where it reappears; cursor size; the
look of the marker's edges at 45 degrees.

<a id="o49"></a>
## O49. UI budgets on the idle reference laptop (M5 criterion 7)

**Spec:** decision 0015 §6 (U1–U5), decision 0010, [`docs/perf/m5-ui-budgets.md`](../perf/m5-ui-budgets.md).
The values recorded during M5 are indicative only (the machine was loaded).

**What to do:** on the idle reference laptop, run the scripted UI measurement of
`m5-ui-budgets.md` at 3840 × 2160 and at the window size the compositor gives.

**Record:** the two reports, added to `m5-ui-budgets.md`.

---

<a id="m7-items"></a>
<a id="o70"></a>
## O70. Player movement speeds and turn rate (M7)

**Spec:** `test-server.md` §4.5 and §8.3 (movement validation and its tolerances),
`tessera-protocol.md` §8.3, and the load tool's bot speeds (`test-server.md` §13.1). The specs
currently use the walker's 2 m/s walk and 6 m/s run (`walker.md`). Overlaps with
[O14](#o14) item O14.4 and [O18](#o18) step 1 (M4.5); one set of readings answers all three.

**What to do:** on flat open ground, type `/loc`, move straight for a timed 10 s, type `/loc`
again. Do this for walking forward, running forward, walking backward, and strafing if the
client has a strafe move. Then turn in place with the keyboard for one full turn (360°) and
time it.

**Record:** metres per second per gait (forward walk, forward run, backward, strafe or "no
strafe"); degrees per second for the keyboard turn; whether the speeds seem to differ by
species (if so, per species).

**Answered from the client (2026-10-08).** No measurement is needed (`collision.md`
§11.8–§11.9, BIN; values from the data):

- **Speeds** (no speed effects active): running 6.5 m/s forward, 3.25 m/s backward, 5.0 m/s
  strafing (the client has strafe); walking (walk mode) 2.5 m/s forward, 2.25 m/s backward,
  2.57 m/s strafing. The 2 m/s walk and 6 m/s run that the M7 specs use are therefore not the
  original's.
- **Keyboard turn on foot:** the `[Input] TurnKeySpeed` preference, 150 degrees per second by
  default (a full turn in 2.4 s); walk mode does not slow it.
- **Species:** no difference; only speed effects set by the server change the speeds.

The stopwatch runs above are an optional confirmation.

<a id="o71"></a>
## O71. How other players' characters move when watched (M7)

**Spec:** `tessera-protocol.md` §8.6 (snapshot interpolation and its recommended delay, now
about 167 ms at the default rates) and the client's remote-character placement
(`walker.md` §10.8).

**What to do:** watch another player's character for a few minutes in a busy place: while it
walks, runs, stops, turns sharply and turns on the spot. An informal look is enough; a short
video may be kept privately, never in the repository.

**Record:** yes/no answers: does it trail its true position smoothly, or snap after stopping
or turning; does it turn on the spot with a stepping animation or rotate rigidly; does it
slide or stutter at a normal latency. Rough delays in milliseconds where you can judge them.

**2026-10-08 (check):** not answerable from the data or the offline client. It is a felt
comparison, and the path that applies other players' updates belongs to the network layer,
which decision 0011 excludes. Unchanged.

<a id="o72"></a>
## O72. Distance at which other player characters appear and disappear (M7)

**Spec:** `tessera-protocol.md` §9 (interest management by landblock: radius 2 blocks with a
one-block margin before an entity leaves) and `test-server.md` §8.2.

**What to do:** in open terrain, with a second account or a friend, stand still and let the
other character walk straight away from you until it vanishes, then walk back until it
appears again. Take `/loc` readings of both characters at the moment it vanishes and at the
moment it appears.

**Record:** the distance in metres (or landblocks, from the two `/loc` readings) for
disappearing and for appearing again; whether the two differ (hysteresis).

**2026-10-08 (check):** not answerable from the data or the offline client: the server
decides which characters a client is told about. Unchanged.

<a id="o73"></a>
## O73. Range of local "say" chat (M7, optional)

**Spec:** `test-server.md` §4.6 (`[chat] say_range_m`) and §9 (chat relay).

**What to do:** with two characters, one speaks on the local "say" channel while the other
moves away in steps; note where the lines stop arriving. Repeat for an emote if emote text
seems to have its own range.

**Record:** metres only (from `/loc`), for say and, if different, for emote text. **Do not
record any chat text.**

**2026-10-08 (check):** not answerable from the data or the offline client: the server
decides who receives a chat line. Unchanged (optional).

<a id="p1-items"></a>
## Decision 0024 P1 items (characters, inventory and equipment)

These rows compare Tessera's P1 screens with the original client. They block nothing until
P8 (decision 0024 §2.6). Describe what you see in your own words; never copy item names,
descriptions or other game text into this file; use DIDs (the explorer shows them) or "the
item in bag 1, slot 3".

<a id="o80"></a>
## O80. Item tooltips of ten items (P1)

**Spec:** `item-tooltips.md` §4–§7 (rows, order, colours), §6.11 (worth coins), §6.12
(damage and DPS), §6.14 (allowed slots), §6.17 (set lines), §15.

**What to do:** hover over ten items of different kinds: a piece of armour, a jewellery item,
a one-handed weapon, a two-handed or ranged weapon, a stackable consumable, a quest item, a
recipe, a set item while wearing another member, an item with a worth whose silver part is
zero (for example a value of exactly one gold and some copper), and a damaged item. Take a
private screenshot of each tooltip.

**Record:** per item, its DID (from the explorer) and, for each row from top to bottom: the
row kind (name, binding, item level, armour, damage, DPS, stat line, durability, requirement,
set member, set bonus, description, worth, other), its colour in words, and whether a blank
gap separates it from the next. For the worth row: which coins show (is a zero silver coin
shown?). For damage: whether the two numbers are integers and what the third part names. For
DPS: how many decimals. For the stat lines and requirements: the wording pattern without the
item's text (for example "plus, number, stat name").

<a id="o81"></a>
## O81. Comparison and pinned tooltips (P1)

**Spec:** `item-tooltips.md` §8 (when comparisons show and where), §10 (pinning).

**What to do:** with the comparison option of the options panel on and then off, hover over a
backpack ring while wearing two rings, and over a one-handed weapon while holding weapons in
both hands. Then try to pin a tooltip: press the key bound to the tooltip-detach command (or
look in the key-binding panel for it) while a tooltip shows; pin the same item twice; pin
many items.

**Record:** the option's default state; how many comparison tooltips appear and on which side
of the main tooltip, top-aligned or not; whether the item already equipped shows a comparison
when hovered on the paperdoll; which input pins; whether pinning the same item twice closes
it; how many can be pinned (the spec says 20) and what happens at the limit.

<a id="o82"></a>
## O82. Character panel (P1)

**Spec:** `character-panels.md` §2–§3.

**What to do:** open the character panel with its key; turn the figure with the arrow buttons
(hold one) and by dragging in the picture; look at the header and the stat list.

**Record:** the command name of the key that opened it (key-binding panel); degrees turned per
second while an arrow is held, and roughly per centimetre of drag; the figure's pose (idle or
still); which stat rows show values in green or red and when.

<a id="o83"></a>
## O83. Bag windows and item-slot overlays (P1)

**Spec:** `character-panels.md` §4.1 (all-bags command), §4.3 (backpack slots per bag), §5
(overlays).

**What to do:** open all bags; note how many slots each shows and in which order items fill
them; drag an item from the first slot of bag 1 to the last slot of bag 2 and back. Press the
key bound to "toggle all bags" (not a single bag) once with all bags closed, once with only
bag 1 open, once with all open. Look at a
stack, a damaged and a broken item, an item your character cannot use (level too low), an
item on cooldown.

**Record:** slots per bag; whether bag 2 starts where bag 1 ends; which bags are open after
each press of the all-bags key; for each overlay: when it
shows (stack of 1: quantity shown or not?), its look in words.

<a id="o84"></a>
## O84. Equipping and item handling (P1)

**Spec:** `character-panels.md` §6.

**What to do:** equip an item by dragging it to its paperdoll slot, by right-clicking it, and
by double-clicking it; unequip by dragging back and by right-clicking the paperdoll slot; split
a stack (try Shift with a drag); destroy an item (drag it out of the window).

**Record:** which inputs equip, unequip, use, split and destroy; any confirmation dialogs (no
text, only that one appears and its buttons).

<a id="o85"></a>
## O85. Selection and race/class screens (P1)

**Spec:** `character-screens.md` §2–§4, §6, §7.2.

**What to do:** log in to the character selection screen; start a new character; go through
the race and class screen, forward to the appearance and name screen and back.

**Record:** the order of screens and what the back button does on each; the order of
characters in the list and which is selected first; the order of races (by species, in your
own words) and of classes for two races; which racial traits the race panel lists (all, or
only some); whether changing race keeps the selected class; on each screen, which of the
back, continue and exit buttons, the turntable buttons and the avatar-update check box are
shown (`character-screens.md` §2.3), when exit appears; whether the delete button is offered
and what happens after a deletion is confirmed. Expectations from the client
(`character-screens.md` §2.1–§2.3, §4.1, revised 2026-10-10), to confirm by look: on every
screen the figure stands on its scene's spot facing the camera when the screen opens, whole
from head to feet, with the race's scenery, lights and effects around it; a turn button
moves the view around the figure (the scenery behind it turns too), about 20° per press and
smoothly while held, and dragging over the picture does the same; zoom eases in and out; the
gender buttons show the chosen one lit, and for a race with one gender the gender buttons are
not shown at all.

<a id="o86"></a>
## O86. Appearance and name screen (P1)

**Spec:** `character-screens.md` §5; `player-character.md` §6.2 Q14.

**What to do:** on the appearance and name screen, press randomise five times; switch
nationality and look at the hair, skin and eye colour choices; try the random-name button.

**Record:** what randomise changes (all controls or some; the name too?); whether the colour
choices change with the nationality (fewer swatches, other swatches, or only the default);
the kind of widget each control kind uses (arrows, swatches, slider). Expectations from the
client (`character-screens.md` §5.2.1–§5.2.2, revised 2026-10-10), to confirm by look: each
control is labelled with its own name (each colour control a different one), the style and
face controls show "current of total" beside the arrows, groups (where a race has them) have
their own headers in a fixed order and start open; a fresh character shows the first style of
every style control (head and hair present on every race and sex).

## O87. Water, waterfalls and puddles after the water fix (2026-10-10)

**Spec:** `water.md` §8.5–§8.7, `material.md` §10.3 and §11.6; rows W7–W17 of
`docs/checklists/m4-5-visual.md` §4.4 and §4.8.

**Background:** the owner reported on 2026-10-09 that waterfalls, textured (non-shader) water
and puddles drew as a white texture, and that the shader water might move too fast. Since the
merge of branch `water-fix`, those surfaces have their own water, mirror, liquid-distortion
and lava drawings. Two-layer and animated surfaces (waterfalls, foam, spouts) scroll with both
layers, and flow-map water no longer uses the plain-water scroll. That scroll was the cause
of drifts up to 22.6 m/s; the spec gives 0.15–0.9 m/s.

**What to do:** walk to the places of rows W10–W17 in the original client and in
`tessera-client`, and compare:
1. Waterfalls (W10, W11): do the sheets and foam move in the same direction and at about the
   same speed?
2. River and lake water (W15–W17): does the water drift about as slowly as in the original,
   and does a river show its bed near the bank?
3. Puddles (W13, W14): soft rounded shape or hard-edged polygon? Tessera still shows a
   hard edge where the terrain cuts the flat puddle quad (`water.md` §8.5.4 OPEN). Does the
   original's puddle lie on the ground everywhere, even on slopes?
4. Ripples and lava (W7, W17): do they move at all in the original? (`water.md` §8.6.3 OPEN)

**Record:** for each row, same / different, with a few words on what differs. Speeds as
"slower / about the same / faster". No screenshots of the original in the repository.

<a id="o88"></a>
## O88. Worn look after equipping (P1)

**Spec:** `character-panels.md` §7 (worn look), `appearance.md` §8 (held items and their
hands).

**Tessera's side:** `apps/client/tests/worn_look_real_data.rs` equips an item from a scenario
and checks that the walker's figure changes; a scenario with `equip` steps run with
`tessera-client --scenario FILE` shows the same equips in the walker (USER_GUIDE §3.11, §3.13).

**What to do:** on one character, equip about five items one after another: a chest piece, a
head piece, a one-handed weapon in each hand, and a shield or other off-hand item. After each
equip take a private screenshot of the figure from the front and the side. Then run the same
equips (same DIDs, read from the explorer) in a Tessera scenario and look at the walker.

**Record:** per equip, the item's DID and its paperdoll slot; same / different for the
figure's look (in words: missing part, other part, wrong place); for each held item, which
hand holds it, and where an off-hand weapon or shield sits (hand, back, hip).

<a id="o89"></a>
## O89. Classes and nationalities per race and gender (P1)

**Spec:** `character-screens.md` §4–§5 (lists and their order), §9 (data checks).

**Tessera's side:** from this install Tessera offers 8 races, 15 (race, gender) entries and 69
(race, class) entries (`crates/gamedata/tests/chargen_real_data.rs`,
`apps/client/tests/character_screens_real_data.rs`); `--character-select`, then New, shows
the lists.

**What to do:** in the original creation screen, go through each race and gender.

**Record:** per race (by species, in your own words) and gender: the classes offered, in their
order, and which of them are greyed out (locked) or missing; the nationalities offered and
which are locked; whether a gender is missing for a race. Compare with Tessera's lists and
note every difference.

<a id="o90"></a>
## O90. Customised appearance (P1)

**Spec:** `character-screens.md` §5.2 (controls and the look they write), §7.1 (what the
session keeps).

**Tessera's side:** `--character-select`, New, then the appearance and name screen. Tessera
draws heights, styles and body sliders of a look; colours and face layers are not drawn yet
(`character-screens.md` §6 and §7.1 OPENs), and the creation viewport's figure is not drawn
yet, so compare on the figure in the world after entering.

**What to do:** in the original, make a character and change several style controls (hair,
beard or ears where offered), one colour, the height and one body slider. Take private
screenshots of the figure in the creation screen and, after creating it, in the character
selection screen. Make the same choices in Tessera.

**Record:** per changed control: its kind (style, colour, height, slider), same / different
on the figure; whether the selection screen's figure keeps every choice (or which it drops).

<a id="o91"></a>
## O91. Far sea at row W2 (M4.5, added 2026-10-10)

**Spec:** `water.md` §8.4 (the reflection pass is OPEN) and §8.6.4 (Tessera rule without a
reflection pass); row W2 of `docs/checklists/m4-5-visual.md` §4.4 and its camera in §4.7.1.

**Background:** at W2's camera (12 m out over the sea of region 3, heading 172, looking 10°
down, noon), Tessera draws the far sea very dark. Tessera has no reflection pass: where the
original mixes in the reflected sky toward grazing angles, Tessera puts the refracted view
through the water. So the far sea can be darker in Tessera than in the original.

**What to do:** stand at W2's camera in the original client at noon (*t* 0.5, O21) and look
at the sea between the near water and the far shore.

**Record:** is the far sea near black, a dark blue-green, or a bright band that reflects the
sky? Where does it change from seeing the bed to reflecting (near, middle, far)? A few words;
no screenshots in the repository.

<a id="o92"></a>
## O92. Fallen soldiers generated in landblock 3, 0xD7E7 (M4.5, added 2026-10-10)

**Spec:** `appearance.md` §7.4 ("Random dye colours", A58); the L5 note of
`docs/checklists/m4-5-visual.md` §4.6; `walker.md` §10.2 (generated rigs as figures).

**Background:** the scenery generator places 54 figures in this block: nine objects of
fallen soldiers (three types, three objects each), lying in dead idles. Tessera dresses them
from the start-up scripts of their class table. The data gives their cloth a random colour
from short lists: dark greens, olives and grey browns for the hooded type, near-black upper
bodies for the other two, and brown, grey or black hair. A first drawing showed three of them
in a magenta tunic. That is a placeholder colour which the data always replaces, so it is not
expected.

**What to do:** from L5's camera (block 3, 0xD7E8, looking south over the boundary into
0xD7E7), or by walking into 0xD7E7, look at the figures lying on the ground in the original
client. Visit twice, on two different days or after a relog, to see whether colours and
pieces change.

**Record:** are there fallen figures here at all (if not, the generated rigs may not be drawn
in the original)? Do they lie on the ground, and in which poses (on the back, on the side,
face down)? Colours of the hooded figures' cloth and of the others' upper bodies, in your own
words. Same or different on the second visit?


<a id="p2-items"></a>
## Decision 0024 P2 items (targeting, skills and combat presentation)

These rows compare Tessera's P2 combat presentation with the original client. They are the
owner checks OC1–OC11 of `combat-ui.md` §18 (O93–O103) and OS1–OS6 of `skill-tooltips.md` §17
(O104–O109), plus two looks at combat visuals (O126, O127) and two whole-panel looks (O128,
O129; O110–O125 are P3's, O130–O137 P4's). They block
nothing until P8 (decision 0024 §2.6). Fighting is needed for most of them: a low-level
creature in a starting area and a few skills of any class are enough. Describe what you see in your own words; never
copy skill, effect, creature or item names, tooltip lines or the defeat text into this file.
Name a skill or effect by its kind ("a melee skill with one hit", "a heal over time") or by its
DID: search the name in the explorer (`tessera-explorer --search`) and write only the DID.
Where a row names Tessera's side, the command lines and expected looks are in
[`p2-visual.md`](p2-visual.md) (C1 is the reference scenario, C4 the combat-visuals scenario).

<a id="o93"></a>
## O93. Targeting keys (P2, OC1)

**Spec:** `combat-ui.md` §3.3 (bindings and the OPEN on the search rules), TR4 (Tessera's rule:
50 m, nearest first, ties by ID, wrap).

**Tessera's side:** `p2-visual.md` row T3 (C4).

**What to do:** stand among several creatures at different distances (some beyond 50 m, one
defeated) with another player near; press Tab repeatedly, then Shift+Tab, Backspace, F10,
Shift+F10, F9, Delete near an object, Backslash and F (with a selection that targets someone).

**Record:** per key: what it selects (creature, player, object, yourself), whether defeated
creatures, neutral creatures and NPCs are included, roughly the farthest distance it reaches
(`/loc` difference), the order of the Tab cycle (nearest first, or another order) and whether
it wraps; what F does when your selection has no target.

<a id="o94"></a>
## O94. Escape with a selection and a cast (P2, OC2)

**Spec:** `combat-ui.md` §3.4 TR5; `game-panels.md` §16 (O45 covers Escape with panels open).

**Tessera's side:** `p2-visual.md` row T5.

**What to do:** select a creature and start a skill with an induction (a cast bar that fills);
press Escape once during the bar; press it again; press it once more. Repeat with a panel open.

**Record:** what each press did, in order (cancel the cast, clear the selection, open the main
menu, close the panel); whether a crafting bar can be cancelled with Escape.

<a id="o95"></a>
## O95. World selection effect (P2, OC3)

**Spec:** `combat-ui.md` §3.5 (E4: the selection script with four distance bands, 0–2, 2–15,
15–35 and 35 m upward), TR6.

**Tessera's side:** `p2-visual.md` row V0 (C4).

**What to do:** select a creature and walk away from it slowly from close by to beyond 40 m;
select an NPC and a player; clear the selection.

**Record:** whether a marker appears on the ground or around the selected figure as soon as it
is selected; its look in a few words (ring, glow, colour); at which distances it changes
(compare with the four bands); whether it differs between creatures, NPCs and players; whether
it goes away at once when the selection clears.

<a id="o96"></a>
## O96. Vital texts on the player and target frames (P2, OC4)

**Spec:** `combat-ui.md` §4.3 TR10 (Tessera shows "current/maximum" while the pointer is over
the vital field).

**Tessera's side:** C1 of `p2-visual.md`, pointer over the player frame's health meter.

**What to do:** look at your own frame and a target's frame with the pointer away, then over
the health and power meters; look in the options panel for a setting about vital numbers.

**Record:** when the numbers show (always, on hover, by an option); their form (current and
maximum, a percentage, both) and separator; whether large numbers are grouped or shortened.

<a id="o97"></a>
## O97. Target health-bar colours (P2, OC5)

**Spec:** `combat-ui.md` §5.3 TR14 (Tessera: green for players and NPCs, red for creatures,
grey for defeated; yellow unused).

**Tessera's side:** C1 (creature target), C8 (players and wanderers).

**What to do:** select a hostile creature, a neutral creature (one that does not attack until
attacked), a creature another player is fighting (tapped), an NPC, another player and a
defeated creature.

**Record:** the health bar's colour for each (green, yellow, red, grey, other).

<a id="o98"></a>
## O98. Name colours by level difference (P2, OC6)

**Spec:** `combat-ui.md` §5.4 TR15 (Tessera's reading of `game-controls.md` §7.5: the
consideration state by the level difference, the player's level picking the list).

**Tessera's side:** C1: a level-20 player and a level-7 creature.

**What to do:** at one character level, select creatures from well below to well above your
level (as many differences between −10 and +10 as you can find, and the extremes).

**Record:** your level; per creature: its level and the colour of its name in the target frame
(in words: grey, green, cyan, blue, white, yellow, orange, red, purple); whether the colour of
the same difference changes at another character level.

<a id="o99"></a>
## O99. Selection's-target window (P2, OC7)

**Spec:** `combat-ui.md` §5.7 TR18.

**Tessera's side:** C1 from 2 s: the small window shows the creature's target.

**What to do:** on a character with default UI options, select a creature that is fighting you
or someone else; then look in the options panel for a setting about the target's target.

**Record:** whether the window shows by default; where it sits; whether it shows a name only or
a name and health; the option's name in your own words and its default.

<a id="o100"></a>
## O100. Floating combat text (P2, OC8)

**Spec:** `combat-ui.md` §6.3–§6.6, TR19 (Tessera: "−" and the number for damage, "+" for heals,
the short avoidance word, no text for 0), TR20 (colours: damage on others pale yellow, on you
red, heals green, avoidance white; life 2 s).

**Tessera's side:** C1, 3–7 s (rows E13a–E13d of `p2-visual.md`).

**What to do:** in a fight, watch the numbers over the creature and over yourself: your normal
hits, a critical, a miss or parry by either side, damage you take, a heal on yourself. A short
private video helps.

**Record:** per case: the sign (none, minus, plus), the number's form, whether a word stands
with it and which kind (short "Parry" style, past tense, a sentence) without copying it, the
colour in words, the direction it moves (up, sideways), roughly how long it stays, and how a
critical differs (larger, a different colour, a word).

<a id="o101"></a>
## O101. Channel bars and crafting bars (P2, OC9)

**Spec:** `combat-ui.md` §7.3 TR26 (Tessera: every bar fills from empty to full), §7.4 TR28.

**Tessera's side:** C4, the channel at 27–31 s (row V5 of `p2-visual.md`).

**What to do:** use a channelled skill (one that keeps running while you hold still); craft an
item that takes a few seconds and press Escape during its bar.

**Record:** whether the channel's bar fills or empties; its colour against a normal cast bar;
whether the craft stops on Escape.

<a id="o102"></a>
## O102. Effect rows (P2, OC10)

**Spec:** `combat-ui.md` §8.2 (rows, order, pulse at 15 s), TR30 (Tessera: beneficial effects in
row 1, harmful in row 2), TR31 (no stacks or timer on the icon).

**Tessera's side:** C1, 3–8 s (rows E8–E10 of `p2-visual.md`).

**What to do:** gain two beneficial effects and suffer two harmful ones at the same time; look
at your own effect rows and at a target's; let one run out; get one effect that stacks.

**Record:** which row holds beneficial and which harmful effects (or one mixed row); the order
of icons (newest first or last); whether icons show a timer or a stack number; when an icon
starts to pulse (seconds before its end); how many icons a row shows before it wraps or hides.

<a id="o103"></a>
## O103. Defeat panel (P2, OC11)

**Spec:** `combat-ui.md` §10.2 (the OPEN on the statement, the buttons and the awaken timer),
TR38 (Tessera: its own statement, OK closes, revive ghosted, no timer).

**Tessera's side:** C1 at 8 s (row E18 of `p2-visual.md`).

**What to do:** let your character be defeated by a creature (a low-level character in a
starting area is enough). Wait without pressing anything for a minute, then press OK.

**Record:** which buttons the panel shows and which are ghosted; whether a timer appears, where,
and what it counts; what OK does (you return to a rally point, stay where you are, other); what
the revive button needs before it is usable. Do not copy the panel's text.

<a id="o104"></a>
## O104. Skill tooltip with damage, effects and a cost (P2, OS1)

**Spec:** `skill-tooltips.md` §4 (structure and row order), §7 (skill rows; §7.9 damage, §7.10 costs), §9 (effect lines).

**Tessera's side:** C1, hover over quickslot 3 (row TT1 of `p2-visual.md`); or the record's
card, C7.

**What to do:** hover over a damaging skill on your quickslot bar that also applies an effect
and costs power; then the same skill in the skills panel. Find its DID in the explorer.

**Record:** the DID; for each row from top to bottom: its kind (name, rank, type, range,
casting time, cooldown, cost, damage, effect line, description, requirement, other), its colour
in words, and whether a blank gap follows; whether the two tooltips (bar and panel) differ.

<a id="o105"></a>
## O105. Effect tooltip from the effect bar (P2, OS2)

**Spec:** `skill-tooltips.md` §8 (effect rows; §8.2 resistance), §9 (lines), §10.2 and §12.2
(the remaining time counting down), §16.4 (resistance text).

**Tessera's side:** C1, hover over the player's 30 s effect icon (row TT2).

**What to do:** hover over one of your timed effects in the effect bar and keep the pointer
still for ten seconds; then over a target's harmful effect that can be resisted.

**Record:** the effect's DID; the rows in order (kind and colour); whether the remaining time
counts down while the tooltip stays open and its format (minutes and seconds, a unit word,
tenths); whether a resistance line shows and its pattern without the text.

<a id="o106"></a>
## O106. A stat line with a percentage and a decrease (P2, OS3)

**Spec:** `skill-tooltips.md` §9.4 (modifier values as numbers), §16.2 (worked examples).

**Tessera's side:** C7 on the effect's DID.

**What to do:** find an effect (or a skill applying one) that raises one stat by a percentage
and lowers another; hover over it.

**Record:** the DID; for each stat line, the wording pattern without the stat's name (for
example "plus, number, percent sign, stat name"), the number of decimals, how a decrease is
written (minus sign, a word) and its colour.

<a id="o107"></a>
## O107. Skill-granting item (P2, OS4)

**Spec:** `skill-tooltips.md` §12.3 (skill items), §12.4 (skill- and trait-granting items),
`item-tooltips.md` §6.20 and §8.3 (`SkillGrantingItem`).

**Tessera's side:** row TT5 of `p2-visual.md`.

**What to do:** hover over an item in your bags that teaches or grants a skill when used.

**Record:** the item's DID; whether a second tooltip with the skill appears next to the item's
tooltip (which side, top-aligned or not), or the skill's rows are inside the item's tooltip;
whether it appears for an item you have already used.

<a id="o108"></a>
## O108. Stack count in an effect tooltip (P2, OS5)

**Spec:** `skill-tooltips.md` §12.2 Q10, `combat-ui.md` TR31.

**What to do:** get an effect that stacks (the same effect applied several times) and hover
over its icon at two and at three stacks.

**Record:** whether the tooltip shows the stack count, where (name line, own row) and in what
form; whether the icon itself shows it.

<a id="o109"></a>
## O109. Casting and recovery times with tenths (P2, OS6)

**Spec:** `skill-tooltips.md` §7.6 (casting time), §7.15 (recovery time), §10.1 (the duration
formatter), §16.3 (a stored 1.55 s that Tessera shows from its single-precision value).

**Tessera's side:** C7 on the same skills.

**What to do:** find two skills whose casting time or cooldown is not a whole number of seconds
(for example about 1.5 s or 2.5 s) and hover over them; find their DIDs.

**Record:** per DID: the casting-time and recovery-time texts' form (decimals, unit word
pattern) and the shown value; whether a value like 1.55 shows as 1.5 or 1.6.

<a id="p3-items"></a>
## Decision 0024 P3 items (NPC windows, quest journal and wallet)

These rows compare Tessera's P3 windows with the original client. They block nothing until
P8 (decision 0024 §2.6); the P3 exit criteria need no observation in the original. They
settle the owner checks of [`npc-windows.md` §14](../specs/npc-windows.md#s14) (O110–O117),
[`quest-journal.md` §13](../specs/quest-journal.md#s13) (O118–O124) and the wallet OPENs of
[`lua-plugin-api.md` §15.3](../specs/lua-plugin-api.md#s15-3) (O125). Describe what you see
in your own words; never copy NPC, quest, item or currency names, dialogue, objective text
or chat lines into this file; use DIDs (the explorer shows them), counts and positions ("the
second option", "the vendor's third entry").

**Tessera's side** for every row: the committed scenarios `dialogue`, `vendor`, `loot` and
`quest-log` in `apps/client/tests/scenarios/`, run windowed with your install
(`tessera-client --ui --scenario-input --scenario FILE`, USER_GUIDE §3.14), and
`--open-panel NAME` for each window opened empty.

The P3 plan listed these observations under other numbers; this file keeps the specs'
numbers. Plan's list → rows here: NPC use → O110; dialogue window → O111; quest-offer window
→ O112; quest log → O118 (with O124 for abandoning); quest tracker and objective counters →
O119, O120; quest messages → O121; quest markers → O122, O123; vendor window → O113, O114;
loot window → O115; wallet and coin display → O116, O117; plugin wallet views → O125.

<a id="o110"></a>
## O110. Using an NPC (P3)

**Spec:** `npc-windows.md` §3.1 (inputs, messages; Q1), `interaction.md` §3.4.1 (use range).

**Tessera's side:** U uses the nearest entity in range that is not a player (an NPC, a
creature or an object such as a chest; Tessera's stand-in until targeting, P2); out of range or with nobody near, a short line of Tessera's own text
appears in chat.

**What to do:** near a vendor or quest giver, use it in each of these ways: select it and
press U; double-click it; right-click it; Shift and right-click a defeated creature you may
loot. Press U with nothing selected. Select an NPC, walk away with `/loc` readings at each
step, and press U until it stops working.

**Record:** which inputs open its window (U, double-click, right click, Shift + right
click); whether a right click on an NPC that is not the current selection uses it; with
nothing selected and when too far away: whether a line appears, in which chat tab and
colour (no text); the distance (from `/loc`) at which using stops working, for two NPCs of
different size if you can; the mouse cursor shown over an NPC you can use, in and out of
range (its shape in your own words).

<a id="o111"></a>
## O111. Dialogue windows and closing (P3)

**Spec:** `npc-windows.md` §5.1 (which window; Q2), §5.3 (the choice list; Q4), §5.7
(closing; Q3).

**Tessera's side:** `dialogue.scenario` shows a choice page, a conversation page, an NPC
comment and a quest offer; Tessera picks the window by the page's content (§5.1 Tessera
rule) and closes a window when you walk out of the NPC's use range.

**What to do:** talk to three kinds of NPC: one that only says a short remark, one with a
single page of text, and a vendor or stable-master with several options. On the one with
options, note the list's layout. Then open a conversation and walk away slowly; open
another and let the NPC walk away or disappear if one does.

**Record:** per kind, which window shows it (a small comment box, a large text window with
one button, a list of options) and its position on screen; for the option list: whether it
has section headers, the order of kinds of option (quests, services, goodbye), and whether
greyed or hidden options appear; whether walking away closes the window, and at roughly what
distance (`/loc`); what happens when the NPC disappears.

<a id="o112"></a>
## O112. Quest-offer window (P3)

**Spec:** `npc-windows.md` §6.2 (objective block; Q5), §6.3 (reward block; Q5), §6.5
(selectable rewards; Q6).

**Tessera's side:** `dialogue.scenario` with an install offers one quest record with coin
and item-experience amounts (`rewards` step); the objective block shows the lowest
objective's description.

**What to do:** read three quest offers: one with a fixed item reward, one with a choice of
rewards, and one that rewards a title, an emote or a virtue if you meet one. On the one with
a choice, press Accept without choosing, then choose and accept.

**Record:** which objective the offer shows (the first, the current, all); the order of the
reward rows by kind (coins, experience, reputation, items, choices, titles, virtues); how
amounts are written (thousands separators, signs, the coin images); whether Accept is greyed
or refused until a reward is chosen, and what tells you so (no text); where the accept and
decline buttons sit and how they look; whether a long description scrolls or wraps.

<a id="o113"></a>
## O113. Vendor window: buying, selling, buy-back and quantities (P3)

**Spec:** `npc-windows.md` §7.2–§7.6 (entries, prices, buy modes, quantity dialog,
selling; Q7, Q13), §7.8 (buy-back and the controls).

**Tessera's side:** `vendor.scenario` shows a buy list with prices and stock, the sell list
from the backpack, a buy-back page, buying three, a sale and a buy-back; Alt, Shift and Tab
held while buying select the quantity dialog, a full stack and filling partial stacks.

**What to do:** at a vendor that sells a stackable item: buy one with a right click; with
Alt held (the quantity dialog: try the full, half and lowest presets, the arrows, and typing
a number above the maximum, then OK); with Shift held; with Tab held while you carry a
partial stack of that item. Look at an entry you cannot afford. On the sell page, sell one
item with a right click, drag an item from a bag onto the window, and try Sell All with a
junk item and a locked item in the bags. Buy one item back from the buy-back page.

**Record:** the quantity each mode buys (Tab: the quantity bought against your partial
stack's quantity and the stack size); the dialog's starting quantity and what OK does with a
value above the maximum or an empty entry; how an unaffordable price looks (colour); which
items the sell page lists (all bag items, sellable ones only, locked ones?); whether
dropping an item on the window sells it, and whether a confirmation appears; whether the
sell page shows a price per item and whether it matches the item tooltip's worth; Sell All's
scope; whether the buy-back tab is always shown or only after a sale; the tabs or pages
the window has (buy, sell, buy-back, repair) and whether a barter vendor uses the same
window; how a limited stock shows on an entry.

<a id="o114"></a>
## O114. Barter window (P3)

**Spec:** `npc-windows.md` §7.7 (Q8).

**Tessera's side:** the barter part of `vendor.scenario`: entries that cost a token, up to
three cost slots, a coin cost drawn as coins in a slot, the required quantity in red when
you hold fewer, 20 entries per page.

**What to do:** open a barter vendor (one that trades items for tokens or other items), one
that also asks for coins if you find one, and turn its pages.

**Record:** per entry, how many cost slots show and in which order (items, then coins?);
how a coin cost appears (in a slot, as a line, not at all); the format of the small text on
a cost slot (a single number, "held / required", other) and when it turns red; whether a
vendor mixes plain entries and barter entries in one window; how the page control looks
(arrows, page number).

<a id="o115"></a>
## O115. Loot window (P3)

**Spec:** `npc-windows.md` §8.2 (rows and coins; Q9), §8.3 (taking loot).

**Tessera's side:** `loot.scenario`: coins in the first row, stacked entries, one entry
taken with a right click, then Loot All; a chest from which the scenario takes one entry
while the window stays open.

**What to do:** loot a creature that drops coins and items; take one item with a right
click, one with a double click, one by dragging it into a bag, then press Loot All. Loot
another and press Enter. Loot a container that leaves items behind when your bags are full.

**Record:** whether coins appear as a row in the window (where, and with coin images or
text) or go straight to the wallet; which inputs take a single entry; whether Loot All
closes the window at once, after everything is taken, or not when something is left; the
button's label change after the last entry (only that it changes).

<a id="o116"></a>
## O116. Wallet window (P3)

**Spec:** `npc-windows.md` §9.4 (Q10), §9.1 (where money is shown).

**Tessera's side:** the wallet window opens from the character panel's wallet button or with
`--open-panel wallet`: the money row first, then one row per category with a held currency
(ascending category key), its currencies sorted by name, and a search field.

**What to do:** open the wallet with its key and with the character panel's button; expand
and collapse categories; search for part of a currency's name; look at a currency at its
cap if you have one.

**Record:** the key that opens it (key-binding panel, command name only); the order of the
categories (by their position, in your own words, not their names if they are game text)
and of the entries within one; whether categories start expanded; whether a total row or
the coins come first; how a capped currency looks (colour, an icon, "x / max"); whether the
search matches the start of a name or any part; how barter tokens you hold are listed (in a
category of their own or mixed with other currencies).

<a id="o117"></a>
## O117. Coin display (P3)

**Spec:** `npc-windows.md` §9.2 (which currency fills which coin; Q11), §9.3 (splitting an
amount).

**Tessera's side:** coins split by the install's coin values; a zero coin is hidden except
where the display shows all coins, and a zero is shown only after a larger non-zero coin
where leading zeroes are suppressed.

**What to do:** look at amounts in four places: your money (wallet or bags), a vendor's
price below one silver, a price of exactly one gold with no silver, and a quest reward in
coins.

**Record:** per place, which of the gold, silver and copper images show for each amount,
including zero coins (shown or hidden, before or after a non-zero coin); whether any place
shows all three coins always.

<a id="o118"></a>
## O118. Quest log (P3)

**Spec:** `quest-journal.md` §4.2–§4.6 (which quests, grouping, row, filters, counts).

**Tessera's side:** L opens the log; `quest-log.scenario` makes literal and install quests
active, completed and failed.

**What to do:** with several quests underway in different categories and some completed,
open the quest log; switch to completed quests and back; press each filter button; collapse
and expand everything; close and open the log again.

**Record:** the order of the categories and of quests within one (level, name, time
accepted); the format of a quest row (level in brackets, a suffix, colours by difficulty, in
your own words); which filter buttons are pressed at first open and what each hides; whether
categories start expanded and whether the log remembers that; the count texts in the header
(the pattern, for example "number of number", without the game's words).

<a id="o119"></a>
## O119. Quest details pane and objective counters (P3)

**Spec:** `quest-journal.md` §4.7 (details pane), §5.2–§5.4 (progress text and counters;
Q1, Q2).

**Tessera's side:** the details pane of `quest-log.scenario`'s quests shows the objective
text and progress lines with counters (for example 2/4).

**What to do:** select a quest with one counted objective (kill or collect a number of
things), one with two counted parts in one objective, and one with an objective of a kind
that has no counter (talk to, go to).

**Record:** the order of the sections (progress lines before or after the objective text,
rewards, the quest-giver line); how the numbers of an objective with two counted parts show
(two lines, one line); the shape of the line for an objective without a counter (a bullet
and a sentence, the word for done, nothing); whether reward amounts show in the details pane;
whether a long background text wraps over several lines or is cut (Tessera cuts it at the
pane width, `quest-journal.md` §4.7 OPEN).

<a id="o120"></a>
## O120. Quest tracker (P3)

**Spec:** `quest-journal.md` §6.2–§6.6 (which quests, entry content, order, controls; Q3,
Q4).

**Tessera's side:** at most 10 tracked quests; the tracker shows the current objective and
counters; a click opens the log at the quest; the right-click menu's items appear in chat
(no popup yet).

**What to do:** accept a new quest and see whether it appears in the tracker; track five or
more quests with long objectives; complete an objective of one and complete one quest;
right-click a tracker entry; click one.

**Record:** whether accepting tracks a quest (and which option controls it, by its position
in the options panel); the order of entries (newest first, oldest first, by level); how a
long entry is cut (an ellipsis, a line limit) when several are tracked; what an entry shows
after its quest is completed and for how long; the right-click menu's items (count and
order, no text needed when they are the "show in log", "remove" and "remove all" kinds);
the tracker's position on screen and how many quests it holds at most; the counter format
(for example "2/4") and how a completed objective is drawn (colour, a mark).

<a id="o121"></a>
## O121. Quest messages in chat and on screen (P3)

**Spec:** `quest-journal.md` §7.1–§7.3 (state, objective-update and server lines; Q5).

**Tessera's side:** a line in the Quest chat type on a new, completed or failed quest, and on
an objective update when the quest's record has an update text; nothing on abandon.

**What to do:** accept a quest, advance a counted objective twice, complete the quest, and
abandon another.

**Record:** for each event: whether a chat line appears, in which tab and colour; whether
text also appears in the middle of the screen (and where, for how long); whether two lines
appear for one update. **Do not record the lines' text.**

<a id="o122"></a>
## O122. Quest rings over NPCs (P3)

**Spec:** `quest-journal.md` §8.1–§8.2 (ring kinds and the feed rule; Q6).

**Tessera's side:** an NPC whose scenario names a quest with `@Quest_CurrentBestowRingDID`
shows a small sign over its head (available, continue, in progress) in place of the ring;
`--quest-icons off` hides it.

**What to do:** look at a quest giver with a quest available to you, one with a quest of a
level well above or below yours, one you must talk to for a quest you are on, and the giver
of a quest you are on whose next step is with another NPC.

**Record:** for each, whether a ring or icon shows over or around the NPC, its shape and
colour in your own words; whether an option in the options panel hides them (its position).

<a id="o123"></a>
## O123. Quest markers on the minimap and the world map (P3)

**Spec:** `quest-journal.md` §8.3; `game-panels.md` §8.8, §9.3.

**Tessera's side:** the quest markers of `quest-log.scenario` on the minimap (within its
radius) and on the world map, the label as tooltip, a highlight on mouse-over, drawn below
the player marker; markers of another region are hidden.

**What to do:** with a quest underway whose objective is nearby, look at the minimap and the
world map; hover over the objective's icon on each; look for the quest giver's icon of an
available quest.

**Record:** on each map: the icon's shape and colour in words; whether it is drawn above or
below the player's arrow; the tooltip's pattern (quest name, objective, both; no text);
whether objective areas are shaded; whether an icon at the minimap's edge stays pinned to
the edge or disappears.

<a id="o124"></a>
## O124. Abandoning a quest (P3)

**Spec:** `quest-journal.md` §4.8 (the abandon button and its confirmation).

**Tessera's side:** the first click on Abandon posts the confirmation text in chat; a second
click for the same quest sends the abandon (no dialog box yet).

**What to do:** abandon a quest you can abandon from the log; try with a quest whose abandon
button is greyed, if any.

**Record:** the confirmation's shape (a dialog box with which buttons, no text); what the
log shows afterwards (the quest gone, moved, greyed); whether a chat line appears; whether
the tracker drops the quest.

<a id="o125"></a>
## O125. Plugin wallet views (P3)

**Spec:** `lua-plugin-api.md` §15.3 (the OPENs on `GetMaxQuantity()`, the order of
`GetWallet()`'s items, `MoneyChanged`).

**Tessera's side:** `GetWallet()` lists the currencies in the order they reached a non-zero
amount; `GetMaxQuantity()` follows the Tessera rule of §15.3; one `MoneyChanged` per change of
the coin amount; `crates/plugins/tests/wallet_views.rs`.

**What to do:** in the original client, run a small plugin of your own that prints, for each
item of `LocalPlayer:GetWallet()`, its index, `GetQuantity()` and `GetMaxQuantity()`, and
that counts `MoneyChanged` and `Wallet.ItemAdded` events. Gain and spend some coins; gain a
currency you did not hold before.

**Record:** for three or four wallet items, by DID (the explorer, not the name): the index
and the maximum the plugin printed; whether the list's order follows the wallet window's,
the order of acquisition or neither; how many `MoneyChanged` events one purchase and one loot
gave; whether `ItemAdded` fired for the new currency and at which index; whether
`GetMoney()` and `GetMoneyComponents()` match the coins the wallet window shows. **Do not
record item names.**

<a id="p2-items-visuals"></a>
## Decision 0024 P2 items, continued (combat visuals and whole panels)

O126–O129 belong to the P2 group above ([P2 items](#p2-items)); they stand here, after P3's O110–O125, to keep the numbers in order.

<a id="o126"></a>
## O126. Casts, hits and stance (P2, combat visuals)

**Spec:** `combat-visuals.md` §3 (behaviours), §4 (strikes, hooks, hit reactions,
projectiles), §6 and §11 rule 6 (stance by the main-hand item), Q-CV7 (a stance change while a
behaviour plays).

**Tessera's side:** C4 of `p2-visual.md`, rows V1–V3, V6, V8.

**What to do:** with a one-handed weapon, then a two-handed weapon, then empty hands, attack a
creature with a melee skill, once standing and once while walking; use a ranged skill; let the
creature hit you. Draw or sheathe your weapon (or change weapons) in the middle of a swing. A
short private video helps.

**Record:** whether the attack plays on the whole body when standing and on the upper body
only when walking; when the creature's hit reaction plays against the swing (at the moment the
weapon lands, or at the end of the animation); whether a critical hit looks different; for the
ranged skill, whether something travels to the target and the hit waits for it; the idle stance
in combat for each weapon kind (weapon raised, held low, fists up); whether changing weapons
mid-swing restarts or changes the swing.

<a id="o127"></a>
## O127. Effects, defeat and revive (P2, combat visuals)

**Spec:** `combat-visuals.md` §5 (effect visuals: attach, loop, stop, linger), §7 (death clip,
hold, revive), §9.5.

**Tessera's side:** C4 of `p2-visual.md`, rows V7, V9, V10.

**What to do:** apply a timed effect with a visible glow or particles to yourself and to a
creature and watch it to its end; defeat a creature and watch its body; be defeated and revive
(or be revived by another player).

**Record:** whether the effect's visuals last exactly as long as the icon, and whether they cut
off or fade at the end; the defeated creature's pose (falls once and stays down, or loops) and
how long until the body fades; your own death pose and what plays when you revive (a get-up
animation, a fade-in, nothing).

<a id="o128"></a>
## O128. Player and target frames as a whole (P2, exit verification)

**Spec:** `combat-ui.md` §4 (player frame, class resource), §5 (target frame: field choice,
decoration, vitals, name colour, level), §8 (the target's effect row).

**Tessera's side:** C1 of `p2-visual.md`, rows E5, E6, E8–E10, E20.

**What to do:** select a creature of your level, one 3–5 levels above and one far below; keep
one of them selected while it carries one of your harmful effects and you carry a beneficial
one. Use a class with a class resource (pips or a bar). A private screenshot of each helps.

**Record:** the frames' layout (portrait, name, level, bars, resource) and how it differs per
target kind; the name colour per level difference; how the level shows for a far higher
target; the order and size of the target's effect icons under its frame; whether the class
resource shows as pips or a bar, and where.

<a id="o129"></a>
## O129. Quickslot bar and cast bars (P2, exit verification)

**Spec:** `combat-ui.md` §7 (cast, induction and channel bars: fill, delay, end states,
interrupt, cancel), §9.3–§9.4 (cooldown display, slot states).

**Tessera's side:** C1 of `p2-visual.md`, rows E11, E12, E15, E16.

**What to do:** use a skill with a cooldown of several seconds, a skill with an induction, and
a channelled skill; let a creature interrupt an induction; cancel one by moving or with Escape;
try a skill that is not usable yet (out of range or without power). A short private video helps.

**Record:** the cooldown sweep (direction, darkness, whether a number counts down); how a slot
looks when the skill cannot be used and when it becomes available again; the cast bar's fill
direction for an induction and for a channel, how a delay shows, and what the bar shows on
success, interrupt and cancel, and for how long.

<a id="p4-items"></a>
## Decision 0024 P4 items (social panels)

These rows compare Tessera's P4 panels and chat with the original client. They block nothing
until P8 (decision 0024 §2.6). Most need other players (a friend, a kin member or a public
group). Describe what you see in your own words; never copy player names, chat text,
channel names or other game text into this file: write "member 2", "the leader", "the
channel I made", and name notices by their role ("someone left").

<a id="o130"></a>
## O130. Fellowship display and vitals (P4)

**Spec:** `social-panels.md` §4.1–§4.6 (panels, rows, leader marker, member out of view,
leader change, removal, disbanding) and §9 TR2–TR4 (Tessera's rules); `scenario-feed.md` §6.7
(vitals of members without an entity).

**Tessera's side:** a committed P4 fellowship scenario run with `tessera-client --scenario
FILE` shows the rows, the leader marker, a member out of view with vitals, a leader change,
a removal and a disband, with the lines Tessera derives (TR4).

**What to do:** in a fellowship of three or more, watch the fellowship display while (a) the
leader passes leadership, (b) a member leaves or is dismissed, (c) a member walks out of
view (another area, far away), (d) the fellowship is disbanded.

**Record:** where the leader marker sits (on the leader's row; on your own portrait when you
lead); whether you get a row yourself; row order (join order or another); for (c): does the
row stay, do its morale and power keep changing, what does a left click on it do; for (b)
and (d): which chat lines appear (by role, for example "X left", "the fellowship was
disbanded"), in which order relative to the rows changing, and whether both "you dismissed"
and "was dismissed" appear when you dismiss someone (Q14); whether the display hides after
the disband. Also (added 2026-10-11, `social-panels.md` revision 1): with voice chat off,
whether any voice icon shows on a member row or on your own portrait (Tessera hides them,
TR14); what F1 and F2 select (Q6); what the two disband names and the two promote names do in
a fellowship (Q5).

<a id="o131"></a>
## O131. Fellowship invitation prompt (P4)

**Spec:** `social-panels.md` §5 (window, behaviour, keys), Q13.

**Tessera's side:** a scenario `invite` step shows the prompt (TR7); Accept and Decline send
`AnswerInvite`.

**What to do:** have another player invite you twice: accept once, decline once, and once
let it wait without answering.

**Record:** where the window opens; what it lists (inviter only, or the members); the
buttons; whether Escape or the close button counts as decline; after how many seconds an
unanswered invitation disappears, and which line appears then (by role).

<a id="o132"></a>
## O132. Trade window (P4)

**Spec:** `social-panels.md` §6 (layout, item entries, coin, accept states and resets), Q8,
Q9, Q11.

**Tessera's side:** a committed P4 trade scenario shows both sides, coin and the ready
states (TR9, TR10).

**What to do:** open a trade with another player; put in two items and some coin; let the
other side do the same; press the trade button on one side, then change an item, then press
it on both sides until the trade completes. Try to take an offered item back.

**Record:** how each side looks (own items left or right, coin fields); how the ready state
shows on each side; what happens to both ready states when either side changes its offer;
how many items a side can hold; whether and how an offered item can be removed; what happens
at completion (window closes, lines by role, items moved to which bag slots). Also (added
2026-10-11, `social-panels.md` revision 1): how your money and the partner's coin are shown
(coin images with counts; which coins show at 0), and the colour of the item names.

<a id="o133"></a>
## O133. Social panel: friend and ignore lists (P4)

**Spec:** `social-panels.md` §3.1–§3.8 (pages, rows, sorting, buttons and dialogs, ignore
page, notices), Q1, Q2, Q7.

**Tessera's side:** a scenario's `friends` and `ignore` steps fill the pages (TR5, TR6);
`/friend` and `/ignore` with `add`, `remove`, `list` work in the chat line (TR11).

**What to do:** open the Social panel on the friends page and on the ignore page; add and
remove a friend and an ignored name; watch a friend log in and out; sort by each column.

**Record:** columns and the default sort column and direction for each page (Q7); what
pressing O and a page command does while the panel is open at another page (Q1); colours of
online and offline friends; what the add and remove dialogs ask; which lines a friend's login
and logout give (by role); what `/friend list` and `/ignore list` print (by role, no names).

<a id="o134"></a>
## O134. Tells and the instant-message tab (P4)

**Spec:** `chat-window.md` §15.2–§15.5 (sending tells, tell lines, the instant-message tab and
its sessions, refusals) and §15.10 CT1–CT4.

**Tessera's side:** on a test server (`--server`) tells work between two Tessera clients;
`tell`, `reply` and `retell` run in the chat window's dispatcher.

**What to do:** exchange tells with another player; reply with the reply key and with the
reply command; send a tell to a name that is offline and to one that does not exist; open
and close the instant-message tab.

**Record:** how your own tell and an incoming tell look (prefix form, colour, by role); when
the instant-message tab appears (first incoming tell, first outgoing, never by default) and
what its title shows; what Shift or Ctrl with the line-up key does in the entry; which line
an offline or unknown recipient gives (by role); whether typing the tell command word and a
space changes the entry's channel label (the spec says it does not, while the reply word and a
space does, `chat-window.md` CW44); whether the instant-message tab's session list ever
reorders its rows (the spec says it does not, §15.4 step 4).

<a id="o135"></a>
## O135. Channel join and leave lines, channel slots and tabs (P4)

**Spec:** `chat-window.md` §15.6 (user chat channels) and §15.10 CT5; `tessera-protocol.md`
§10.6 and the §17 OPEN on membership notices; `test-server.md` §9.1 (name rule).

**Tessera's side:** on a test server at protocol 1.2 `join` and `part` create and leave named
channels (decision 0035 D1); lines reach only members.

**What to do:** join a channel of your own invention, write a line, have another player join
and leave it, leave it yourself; try a long name (over 30 characters), a name with digits and
one with punctuation; join more channels than the client allows.

**Record:** the lines shown on join and leave (by role), and whether you see others join or
leave; which slot number the channel takes and how a line in it is prefixed; whether a
channel gets its own tab or filter; the longest name accepted and which characters are
refused; the per-character limit and its refusal line (by role).

<a id="o136"></a>
## O136. Synchronised fellowship start (P4)

**Spec:** `instruments.md` §8.8 (waiting part, start, notices, withdrawal), §8.9 (start
tolerance) and Q10–Q13; `scenario-feed.md` §6.7.

**Tessera's side:** a committed P4 scenario makes two members' parts wait and starts them
together (`perform … sync`, `perform-start`); with your own `/play x sync` and `/playstart`
your part joins (P4 exit criterion 3).

**What to do:** in a fellowship of two or more players with instruments, each runs
`/play <song> <part> sync` with parts of one song; then one member runs `/playstart`.
Repeat with the starter having no part of their own, and once with one member leaving music
mode before the start.

**Record:** time from pressing Enter on `/playstart` to the first note (stopwatch, a few
tries; compare with a solo `/play`); how aligned the parts sound to you (together, a slight
flam, an audible offset; estimate in milliseconds if you can); whether the starter needs a
part; whether a member who left music mode still starts (Q11, Q13); which lines appear (by
role) for "ready" and "start".

<a id="o137"></a>
## O137. Owner question: synchronised-start relay on Tessera's own server (P4)

**Spec:** [decision 0024](../decisions/0024-parity-track.md) open item 2;
[decision 0035](../decisions/0035-p4-social-on-test-server.md) D2, D3; `instruments.md` Q14;
LEGAL.md §11.1 rule 5.

**Question:** does relaying a synchronised `/play` start (a member's "ready" with a tune name
and a common start time, `instruments.md` §8.8) through Tessera's own server count as chat
under rule 5? Until answered, synchronised starts with other players exist only on the
scenario feed; protocol v1.2 carries no message for them.

**Record:** yes or no, with any condition (for example "only between members of one named
channel", or "only the start time, no tune name"), and the date. A yes leads to a later
protocol minor; a no closes the question.

<a id="p5-items"></a>
## Decision 0024 P5 items (maps, travel, instance finder and mounts)

These rows compare Tessera's P5 windows and mounted walker with the original client. They
block nothing until P8 (decision 0024 §2.6); the P5 exit criteria need no observation in the
original ([ROADMAP P5](../ROADMAP.md#p5)). They settle the owner checks of
[`map-windows.md` §13](../specs/map-windows.md#s13), [`travel-windows.md`
§13](../specs/travel-windows.md#s13), [`walker.md` §16.12](../specs/walker.md#s16-12), the
OPENs of [`mounts.md` §12](../specs/mounts.md#s12) that the original can show, and those of
[`lua-plugin-api.md` §15.5](../specs/lua-plugin-api.md#s15-5). Describe what you see in your own
words; never copy map, area, place, dungeon, destination, instance or mount names, note texts
or chat lines into this file; use DIDs (the explorer shows them), `/loc` readings, counts and
positions ("the third item of the menu", "the second destination under the first header").

**Tessera's side** for every row: the committed scenarios `map`, `travel`, `instances` and
`mount` in `apps/client/tests/scenarios/`, run windowed with your install
(`tessera-client --ui --scenario-input --scenario FILE`, USER_GUIDE §3.17); `--open-panel map`;
and, for the mount rows, the offline walker with `--mount SPEC` or `/ride` (USER_GUIDE §3.17).

The two P5 specs proposed their own rows, both numbered from O140; this file numbers them in
one sequence. Spec rows → rows here: `map-windows.md` O140 → O140 and O141, O141 → O142,
O142 → O143, O143 → O144, O144 → O142, O145 → O142, O146 → O143; `travel-windows.md` O140 →
O146, O141 → O147, O142 → O146, O143 → O148, O144 → O148, O145 → O149, O146 → O150, O147 →
O150; `walker.md` §16.12 (a) → O153, (b) → O152, (c) → O151, (d) → O151, (e) → O153. The
P5 plan's topics map as: world map overlays and filters → O140, O141 (with O142, O143 for
navigation and dungeon maps); minimap notes and blips → O144; place lookup → O145; markers a
server adds outside quests → O155; travel
window and travel presentation → O146–O148; instance finder → O149, O150; mounted gaits of a
normal mount and a war-steed → O151, O152; mount and dismount sequence → O153; plugin mount
views → O154.

<a id="o140"></a>
## O140. World map: presentation and controls (P5)

**Spec:** `map-windows.md` §3.1 (two presentations; OPEN on the full-screen placement and
scale), §3.4 (controls), §13 row O140 (first part).

**Tessera's side:** the map opens in the presentation of the install's option default (full
screen here), centred and scaled by the largest factor that fits the screen (Tessera rule); the
corner button switches and the choice is kept in Tessera's settings; the stable-master button
and the quest-guide option are hidden.

**What to do:** on a new character, or after resetting the options to their defaults, open the
map with its key. Note the presentation, then switch with the corner button and back. Do this
at two screen resolutions or UI scales if you can. Look at the world map, a territory map and an
area map.

**Record:** full screen or windowed at the first opening; in full screen, the map's size and
position on screen (pixels or fractions of the screen) at each resolution; whether the choice is
kept after closing and reopening, and after a relog; which buttons of the controls row are
visible on each of the three kinds of map (by position and icon, no text); whether the
coordinates under the pointer are shown, and where.

<a id="o141"></a>
## O141. World map overlays and filters (P5)

**Spec:** `map-windows.md` §6.2 (drawing order), §6.3 (whether a note's z adds its relative z
level, Probable), §6.6 (the player marker), §6.9 (filter menu, defaults, label collation), §13
row O140 (second part).

**Tessera's side:** table notes are drawn in read order, feed markers in front of them and the
player marker in front of all; the filter menu lists *all* and *none*, then one item per kind of
note sorted by Tessera's collation rule; the menu stays open while you click; the filter values
are kept per kind of map (near and distant), not per character.

**What to do:** on an area map, open the filter menu without changing anything; then untick one
kind, close the menu, open another area map and then a distant map; relog. Find two notes that
overlap and hover them.

**Record:** the number of items and how many are ticked at first; whether the items are
alphabetical in your client's language (describe the first five by their icons, no text);
whether the menu stays open after a click; whether an unticked kind stays unticked on another
area map, on a distant map and after a relog; for two overlapping notes, which kind is drawn on
top; whether the player's arrow is drawn above every note.

<a id="o142"></a>
## O142. Map navigation and the map menu (P5)

**Spec:** `map-windows.md` §4.3 (child maps, the three targets that are no node), §4.4 (going
up; OPEN on the call at the top and on `UI_Map_PreventZoomOut`), §4.5 (the map menu; Tessera
order), §13 rows O141, O144, O145.

**Tessera's side:** a click on a marked area opens that map; a right click goes to the parent
map, does nothing on the world map and does nothing on the one node that carries
`UI_Map_PreventZoomOut` (§4.4); the map menu has one submenu per region in the region list's
order, with the maps sorted by their labels; the three child-map buttons of §4.3 that name no
node show their map without notes.

**What to do:** right-click on an area map, on its territory map and on the world map; right-
click on the map of the one node with `UI_Map_PreventZoomOut` (§4.4 names it). Open the map menu and one of its submenus. Click the three child-map buttons that
§4.3 lists.

**Record:** what each right click does (parent map, the map of your position, nothing); the
order of the submenus (by region number, the region table's order, or another); whether the
maps inside one submenu are alphabetical; for each of the three buttons, what appears (a map
without notes, nothing, something else); whether a sound plays when the map changes.

<a id="o143"></a>
## O143. Dungeon maps (P5)

**Spec:** `map-windows.md` §4.7 (the dungeon-map view; stretch into its frame), §6.3, §13 rows
O142 and O146; `game-panels.md` §8.5.3 (fog of war, OPEN).

**Tessera's side:** a dungeon map's image is stretched into the 800 × 600 field inside the
frame, whatever its proportions; the unexplored-area shading is not drawn; notes and markers
are placed as on outdoor maps.

**What to do:** inside a dungeon whose map image is not 4 : 3 (a tall cave map), open the map.
Hover a note in a part you have not explored yet. Right-click the dungeon map.

**Record:** the map's `/loc` reading or DID; whether the image is squeezed into the frame,
letterboxed or cropped; whether unexplored parts are shaded, and whether a note there shows a
tooltip; what the right click does.

<a id="o144"></a>
## O144. Minimap notes and blips (P5)

**Spec:** `map-windows.md` §6.8 (radius, off-screen images and their turn, Probable; tooltip;
the radar's filter menu, not traced), §12 item 7, §13 row O143.

**Tessera's side:** notes inside the radar's circle use their on-screen icons; outside, the
kinds that have an off-screen icon are drawn on the rim in the direction of the note, turned to
point that way; the tooltip shows the note's text and the horizontal distance; scenario markers
show only inside the circle; every icon is drawn behind the player's arrow.

**What to do:** stand so that a vendor's note is just outside the minimap's circle, then walk
until it comes inside. Hover the note at the rim and inside. Open the minimap's menu for the
kinds of notes, if you find one.

**Record:** whether the note is drawn at the rim, and whether its icon turns towards the
vendor; the tooltip's pattern (text, distance, its unit; no text copied); which control opens
the minimap's filter menu and how many items it has; whether a quest objective's icon stays on
the rim (see also O123).

<a id="o145"></a>
## O145. Place lookup and coordinates on the map (P5)

**Spec:** `map-windows.md` §7.1 (the original has no place search), §7.3 (Tessera's
`findplace`), §3.4 (coordinates under the pointer), §6.3 (note tooltips).

**Tessera's side:** `/findplace TEXT` is Tessera's own command (coordinates, a `/loc` line, a
place key or a name); the map shows the coordinates under the pointer when the install's option
is on, and each note's tooltip shows its coordinates.

**What to do:** look for any way in the original to search for a place by name (the map window,
its menus, the command list). With the coordinates display on, point at three spots on an area
map and compare the text with `/loc` taken when standing there. Hover two notes.

**Record:** whether any place search exists (yes or no, and where); the coordinates' format
(digits, decimal places, the letters for the directions, the order); whether the pointer's
coordinates agree with `/loc` at the three spots, and by how much; whether note tooltips show
coordinates.

<a id="o146"></a>
## O146. Travel window: list, headers and search (P5)

**Spec:** `travel-windows.md` §4.4–§4.6 (rows, grouping and order; search, case OPEN), §13
rows O140 and O142.

**Tessera's side:** `travel.scenario`: rows in the scenario's order under territory headers,
headers in the order of their first row, rows without a territory at the top; the search keeps
rows whose search text contains the typed text, ignoring case (Tessera rule).

**What to do:** talk to a stable master with many destinations. Expand one header. Type part of
an area name into the search field, then the same in another letter case.

**Record:** the stable master's DID or `/loc`; whether the headers are territories; whether the
rows under one header are alphabetical; whether rows of other territories with that area text
stay visible; whether case matters; whether empty headers disappear.

<a id="o147"></a>
## O147. Travel window: selection, cost and go button (P5)

**Spec:** `travel-windows.md` §4.7 (selection display), §4.8 (go controls), §4.9 (closing),
§13 row O141.

**Tessera's side:** a free row shows the free text instead of the coins; marks for rows that
are undiscovered, out of reach or unusable; the go button is greyed unless the row is
discovered and usable, with the undiscovered label or the plain go label (Tessera uses no
per-case reason); rows whose level or requirements are not met are marked but can be chosen;
paper-item and buy-now buttons are hidden; the window waits for the session's answer before it
closes.

**What to do:** select a free row, a paid row and a row you have not discovered; try one you
cannot use yet (level or other requirement). Press go on a usable row.

**Record:** what the cost area shows for each (coins, a free label, nothing); the marks on the
undiscovered and unusable rows (icon shapes in words); the go button's state and whether its
label changes per case (no text); whether a paper-item or buy-now button appears; whether the
window closes at once on go or after a delay.

<a id="o148"></a>
## O148. Travel presentation: slow and swift routes (P5)

**Spec:** `travel-windows.md` §5.1 (started travel and arrival; OPEN on when the loading screen
appears), §5.2 (Tessera presentation), §13 rows O143 and O144.

**Tessera's side:** a swift route shows the loading screen with a bar filling over the
scenario's duration; a slow route shows the end-travel box for its duration; its button asks
to stop and the box stays until the session answers; no chat line is posted.

**What to do:** ride a slow route, timing it from go to arrival; press the end-travel button on
another slow route; take a swift route and time the loading screen.

**Record:** where the end-travel box appears (position on screen); whether a chat line appears
at start or arrival (where, no text); the ride's duration and the destination's DID; what the
end-travel button does (stops at once, after a delay, puts you where); for the swift route,
whether the loading screen shows and for how long, and whether its bar fills smoothly.

<a id="o149"></a>
## O149. Instance finder window (P5)

**Spec:** `travel-windows.md` §6.1 (which window), §6.3 (the entry tree; featured field), §6.4
(size, tier and level; OPEN on how the menus are filled), §13 row O145.

**Tessera's side:** `instances.scenario`: the fellowship maker's finder page with kind and
category headers, group rows, size and tier menus filled from the template's arrays (first item
chosen), the level field clamped into the template's range, the featured field when the
scenario names one, and the listings board shown unavailable.

**What to do:** open the instance finder with its default key. Expand one kind and one category;
select a group row; open the size and tier menus; change the level if the field allows it.

**Record:** which window and page open; the tree's two header levels (kinds, then categories?)
and the order of instances inside a category; the featured field (filled or empty, what it
shows by icon); for the group row, the items of the size and tier menus (counts) and which is
chosen first; whether the level field is editable and its range for two instances (by DID).

<a id="o150"></a>
## O150. Instance queue, invitation and leaving (P5)

**Spec:** `travel-windows.md` §6.5 (queue, confirmation and invitation; OPEN on when the
invitation window opens), §6.6 (inside an instance), §7.2 (alert fields), §13 rows O146 and
O147.

**Tessera's side:** the scenario's queue states show the waiting page with its timer counting
up, the confirmation with its count-down and the queue alert; the leave-queue button sends the
command; inside an instance the join button is greyed and `/leaveinstance` asks to leave (no
button, no timer).

**What to do:** queue for an instance through the finder and wait; leave the queue once; queue
again until the instance is offered; enter, then leave it the way the client offers.

**Record:** the waiting page (timer shape, counting up or down), the alert icon's position and
its tooltip's pattern (no text); whether an invitation window appears and with which buttons;
how the client offers leaving (a dialog, a command, a menu item, a button) and whether a timer
is shown inside.

<a id="o151"></a>
## O151. Mounted gaits of a normal mount (P5)

**Spec:** `mounts.md` §5.1 (speed; MQ6), §14.2–§14.3 (rider and mount clips), §14.6 (normal
mounts), §14.8 (seat and scale), §14.9 (camera; MQ14); `walker.md` §16.6, §16.12 (c) and (d).

**Tessera's side:** `--mount auto` (the first mount of your body's group) or `/ride`: the Mounted
layer's speeds with the effect's speed factor; walk and run clips by the walking style (Shift held); the
camera aimed 0.75 m higher and 0.2 m forward of its on-foot target; a River-hobbit's horse drawn
at 0.7 scale; the HUD's mount line and the frame log's mount columns.

**What to do:** on flat ground, ride a horse for 10 s at a run and 10 s at a walk, with `/loc` at
start and end of each; walk backwards and strafe. Note the camera's height compared with on foot.
With a River-hobbit and a Hobbit if you have them, mount the same kind of horse beside a Human's.

**Record:** the mount's skill or effect DID; the distances covered (from `/loc`) for each run;
whether strafing moves sideways; the camera's height and distance compared with on foot (in your
own words, or two screenshots kept private); whether the River-hobbit's horse looks smaller than
the Human's, and about how much.

<a id="o152"></a>
## O152. Mounted gaits of a war-steed (P5)

**Spec:** `mounts.md` §5.3–§5.7 (the war-steed controller), §14.4 (gaits), §14.6 (Tessera's
gear and gait rule; MQ9); `walker.md` §16.6, §16.12 (b).

**Tessera's side:** `--mount steed:medium` or `/ride steed:medium`: holding forward starts at
Walk and shifts up one gait every 0.5 s (gear-change rate 2.0) to Gallop; releasing the keys
keeps the speed; holding back brakes, then backs off; the gait drives the rider's and the
steed's clips; `@lock=` holds a gear.

**What to do:** on flat ground on your war-steed, hold forward from a stop for 5 s, then release
for 3 s, then hold back. Repeat with a video if you can (kept private). Try walking style.

**Record:** whether the gait rises in steps and about how often; how long from a stop to the
fastest gait; whether the speed holds when you release the keys, or drops; what holding back
does (brake, reverse, both); whether walking style caps the gait; whether the steed ever shows a
gait between standing and walking.

<a id="o153"></a>
## O153. Mount and dismount sequence (P5)

**Spec:** `mounts.md` §14.7 (summoning, mounting, dismounting; MQ4), §14.12 (water, areas;
MQ13, MQ15); `walker.md` §16.5, §16.12 (a) and (e).

**Tessera's side:** `/ride` or Ctrl+M: summoning for the skill's time (2.0 s, 3.0 s for a
war-steed), interrupted by movement, a jump, a fall or water; the mounting clip; the mount
appears, fading in, 0.4 s after summoning ends; dismounting at rest by the player plays the mount's bow and waits
1.5 s, moving or by other dismounts at once; riding into water dismounts; an area entered while
mounted does not dismount.

**What to do:** summon a normal mount standing still, timing from the key to the mount's
appearance; summon again and move during the summoning; dismount standing still and again while
moving; ride into deep water; ride into an area where mounts are not allowed (a town where your
mount is refused, if you know one).

**Record:** the time from the key to the end of the summoning bar and from there to the mount
being visible; whether moving cancels the summoning; at rest, whether the mount bows and how
long until you stand; while moving, whether you dismount at once; what deep water does (swim
mounted, dismount, nothing) and at what depth; what entering the forbidden area does
(dismount, nothing, a message: no text).

<a id="o154"></a>
## O154. Plugin mount views (P5)

**Spec:** `lua-plugin-api.md` §15.5 (`GetMount`, `MountChanged`, `BasicMount`, `CombatMount`;
OPENs on the extra `MountChanged` events, the second `BasicMount` event and the war-steed
getters).

**Tessera's side:** `GetMount()` is nil on foot and a `BasicMount` or `CombatMount` while
mounted; one `MountChanged` per change of the mount model (mounted or not, which mount);
the mount-view tests of `tessera-plugins` and the scenario checks 100–107 of `lua-plugin-api.md` §18.

**What to do:** in the original client, run a small plugin of your own that counts
`LocalPlayer.MountChanged` events and prints whether `GetMount()` is nil in each handler, its
class (whether `GetFury` exists), and `GetMorale()`, `GetMaxMorale()`; for a war-steed also
`GetPower()`, `GetFury()`, `GetStrength()` and `GetTurnRate()`. Mount and dismount a normal
mount and a war-steed; take a hit while on the war-steed if you can.

**Record:** how many `MountChanged` events one mount and one dismount gave, and in which of them
`GetMount()` was nil; the class per mount; the numbers printed (no names); which events the
mount object fired when its morale changed; whether the war-steed's strength and turn rate are
0 or real values. **Do not record mount names.**

<a id="o155"></a>
## O155. Map markers a server adds outside quests (P5)

**Spec:** `map-windows.md` §6.7 (feed markers: Tessera rules, because the spec found no
counterpart in the client for a non-quest marker a server adds), §6.8 (markers on the minimap).

**Tessera's side:** a scenario marker without a quest is drawn with the icon table's entry for
the waypoint note type (or its own icon), in front of the table notes and behind the player
marker; its tooltip is its label and its map coordinates on the world map, its label and the
distance on the minimap, where it shows only inside the circle. The committed `map` scenario
places such markers (`tessera-client --ui --scenario-input --scenario map.scenario`).

**What to do:** watch the world map and the minimap for markers that are neither table notes nor
quest objectives: during a festival or world event, a fellowship member's position, a marker
the game sets after a command or a dialogue. Hover each one you find.

**Record:** whether such markers exist at all (and the situation in which you saw one); for each,
its icon (by DID if the explorer shows it, else its shape and colour), whether it has a label
or tooltip and what the tooltip consists of (label, coordinates, distance; no text copied),
whether it is drawn on the minimap and at the rim, and whether it is drawn above or below the
table notes.

<a id="p6-items"></a>
## Decision 0024 P6 items (progression and crafting panels)

These rows compare Tessera's P6 panels with the original client: the experience bar and
level-up, the trait panel, skill training, the crafting panel, crafting trainers, gathering and
the plugin progression and crafting views. They block nothing until P8 (decision 0024 §2.6); the
P6 exit criteria need no observation in the original
([decision 0037](../decisions/0037-p6-trait-tree-and-recipe-census.md)). They settle the owner
checks of `progression-panels.md`, [`crafting-panels.md` §17](../specs/crafting-panels.md#s17)
and the proposed rows of [`lua-plugin-api.md` §15.6](../specs/lua-plugin-api.md#s15-6), and the
Tessera rules those specs mark "owner check". Describe what you see in your own words; never copy
skill, trait, recipe, item, profession, trainer or NPC names, tooltip texts or chat lines into
this file; use DIDs (the explorer shows them), counts, numbers and positions ("the third tab",
"the second recipe under the first category").

**Tessera's side** for every row: the committed P6 scenarios in `apps/client/tests/scenarios/`
(the progression scenario with its level-up, train and craft legs), run windowed with your
install (`tessera-client --ui --scenario-input --scenario FILE`, USER_GUIDE §3.11); the `--open-panel`
names `crafting`, `crafting-recipe`, `vocation-trainer`, `profession-trainer` and
`craftguild-trainer` (`crafting-panels.md` §3) and those of `progression-panels.md`.

The P6 specs proposed their own rows: `progression-panels.md` from O160, `crafting-panels.md`
O180–O191 and `lua-plugin-api.md` §15.6 O192–O195. This file numbers them in one sequence:
`crafting-panels.md` O180 → O166, O181 → O167, O182 → O167, O183 → O169, O184 → O168, O185 →
O170, O186 → O167, O187 → O171, O188 → O172, O189 → O172, O190 → O173, O191 → O168, its §12
keymap OPEN → O175; `lua-plugin-api.md` O192–O195 → O174. `progression-panels.md` is not yet
in the tree (decision 0037, Open): the "Tessera's side" sentences of O160–O165 and O175 that
cite it state the rules it is to give and are provisional until it is committed and they are
checked against it and given its section numbers. The rows of `progression-panels.md`
map to O160–O165 and O175 by topic (experience bar, level-up, trait panel and trees, trait
slots and tooltips, skill training, keys); the spec writer reconciles its row numbers with these
at the merge. The topics of the P6 plan map as: experience bar and level-up → O160, O161; trait
panel and trees per class → O162, O163; trait slots and tooltips → O164; skill-training window →
O165; crafting panel professions, tiers, meters and recipe list → O166, O167; recipe detail and
make controls → O168, O169 (with O171 for recipe-scroll tooltips); crafting bar and result lines
→ O170; crafting trainers → O172; gathering hotspot look, cursor, use and gathering bar → O173;
plugin progression and crafting views → O174; panel toggle keys → O175.

<a id="o160"></a>
## O160. Experience bar (P6)

**Spec:** `progression-panels.md` (the experience bar: element, fill, text and tooltip);
`game-panels.md` §11 (`LevelProgressMeter`, panel P3);
`scenario-feed.md` §6.6 (`xp`, `level`).

**Tessera's side:** the bar's fill is the feed's current experience divided by the level's cap
(`xp A/B`), clamped to 0 … 1 (provisional until `progression-panels.md` is committed); the text and tooltip show the scripted numbers in the forms the
progression spec names. Tessera derives no experience and no cap from the install's level
tables (decision 0024 §2 principle 4).

**What to do:** on a character below the level cap, look at the experience bar at rest; hover
it; earn some experience (a kill or a quest) and watch the bar; if the client has an option to
show the experience as text, turn it on and off.

**Record:** where the bar is drawn (which panel, top or bottom, its length relative to the
screen); whether it is one bar or segmented (how many segments); the fill colour and whether a
second colour shows rested or bonus experience; what the tooltip consists of (current, cap,
remaining, percentage, rested amount; describe, no text copied); whether the fill animates or
jumps when experience is earned; whether a floating "+N" text appears and where.

<a id="o161"></a>
## O161. Level-up presentation (P6)

**Spec:** `progression-panels.md` (level-up presentation); `scenario-feed.md` §6.6 ("Meaning
for the client half": a not-quiet `LevelChanged` that raises the level is a level-up);
`combat-ui.md` §4 (the player frame's `PlayerLevel`, property `Advancement_Level`) and TR16
(the target frame's `OpponentLevel`).

**Tessera's side:** a `LevelChanged` for the player that raises the level and is not `quiet`
runs the level-up presentation of `progression-panels.md`; the first level of a session and
quiet reports set the level silently; the player frame's level changes with it.

**What to do:** level up a character (or watch one level up). Note everything that happens in
the first ten seconds: on the character, on screen, in chat, in the panels.

**Record:** whether a visual effect plays on the character (and roughly how long); whether a
sound plays; which chat lines appear (how many, in which chat tab; no text copied); whether a
banner or window opens and whether it closes by itself (after how long); whether the experience
bar empties at once or animates; whether the level number in the player frame changes at once;
whether new skills or trait points are announced and how (a flashing button, a window, a line);
whether the presentation also plays when two levels are gained at once.

<a id="o162"></a>
## O162. Trait panel: tabs and trees per class (P6)

**Spec:** `progression-panels.md` (trait panel, tabs, tree pages); `combat-data.md` §8.5 (trait
trees: 17 records, branches, cells, dependencies; OPEN on rows and columns of the cell keys)
and §8.6 (the trait control object).

**Tessera's side:** the trait panel opens with the tabs `progression-panels.md` names; the tree
page shows the tree the feed reports active (`trait-tree`) with its branch (`branch=`), laid out
from the cell keys by the progression spec's rule; points come from `trait-points … tree=`.

**What to do:** open the trait panel on a character of two different classes. Go through every
tab. On the class-tree tab, look at each branch's page and at the summary page if there is one.
Note one known trait's cell on screen (count rows and columns from the top left) and its DID in
the explorer.

**Record:** the tabs in order (by position and icon, no text); for each class, how many branches
the tree has and whether they are side by side or on separate pages; how many rows and columns a
branch page has; for the noted trait, its row and column on screen and its DID (this settles the
row/column OPEN of `combat-data.md` §8.5); whether a branch must be chosen before others open,
and how the choice is shown.

<a id="o163"></a>
## O163. Trait tree cells: ranks, prerequisites and points (P6)

**Spec:** `progression-panels.md` (cells, ranks, prerequisites, point display, spend and refund
controls); `combat-data.md` §8.3 (ranks), §8.4 (prerequisites), §8.5 (dependencies, required
ranks per row); `scenario-feed.md` §6.6 (`trait-rank`, `trait-points`, `SetTraitRank`,
`ChooseTraitBranch`).

**Tessera's side:** a cell shows the rank the feed reported (`trait-rank`) out of the trait's
maximum rank; a cell whose dependencies or row requirement are not met is drawn as the progression
spec says; a click sends `SetTraitRank` and the cell changes only on the answering event (the
client spends nothing itself).

**What to do:** on a class tree, look at a cell with no ranks, one part-ranked and one at its
maximum; at a cell under an unmet dependency; at a row that needs more points in the branch.
Spend one point and, if possible without cost, take it back (a reset or refund control).

**Record:** how ranks are shown (a number, pips, an overlay icon; "N/M" or only N); how locked
cells and unmet dependencies are drawn (greyed, dimmed, lines between cells, arrows); where the
available and spent points appear and in what form; whether spending is immediate or needs a
confirm or apply button; what a refund costs and how it is shown (no amounts copied if they are
names; numbers are fine).

<a id="o164"></a>
## O164. Trait slots and trait tooltips (P6)

**Spec:** `progression-panels.md` (slotted traits, trait tooltips); `combat-data.md` §8.1–§8.3
(trait classes, common properties, ranks); `scenario-feed.md` §6.6 (`trait N REF rank=N`,
`TraitSlotted`).

**Tessera's side:** slotted traits (virtues, racial and other slotted kinds) come from the
feed's `trait` step, with their rank; tooltips are built from the trait record (name, nature,
rank, description, effects) in the row order of `progression-panels.md`.

**What to do:** open the slotted-trait views (virtues, racial traits, others the panel has).
Hover a slotted trait, an empty slot and a trait in a tree cell at rank 0, part-ranked and at its
maximum. Drag a trait into a slot if the panel allows it.

**Record:** how many slots each kind has at your level and how locked slots are drawn; the rows
of each tooltip in order (by role: name, rank, nature, description, effect lines, requirement,
"next rank" part; no text copied); whether a tree trait's tooltip shows the next rank's values;
whether slotting is by drag, click or a list, and whether it needs a trainer or a confirm.

<a id="o165"></a>
## O165. Skill-training window (P6)

**Spec:** `progression-panels.md` (skill training); `npc-windows.md` §5.3 (trainers reached
from the choice window); `scenario-feed.md` §6.6 (`trainer … offers=…`, `rank=`, `trained=`,
`unmet=`; `untrained`); `lua-plugin-api.md` §15.6 (untrained skills).

**Tessera's side:** a trainer window opens only on the feed's `TrainerShown`; its rows are the
feed's entries with price, level and rank, ghosted when the feed marks them trained or unmet; a
click on train sends `TrainSkill` and the row changes only on the answering events.

**What to do:** visit your class trainer at a level with something to train and at one with
nothing to train. Look at the list, select a row, hover a row, train one skill.

**Record:** the window's layout (a list, a tree by level, tabs); which rows are shown (only
trainable now, also later levels, also trained); how a row above your level or rank is drawn;
the row's columns (icon, level, price, rank; by role); whether training asks for a confirm;
what changes after training (the row disappears, ghosts, the price line updates); whether a
"train all" control exists.

<a id="o166"></a>
## O166. Crafting panel: profession tabs and tier rows (P6)

**Spec:** `crafting-panels.md` §4.3 (profession tabs; Tessera rule from the feed), §4.7 (tier
rows, proficiency and mastery meters and icons), §17 row O180.

**Tessera's side:** one tab per profession the feed reported, in the order of first report,
then a "train more" tab when the feed's slot count is larger; each tier row shows the
proficiency icon and the mastery icon or meter by the rule of §4.7 (worked example §15.2).

**What to do:** press T: the crafting panel opens. Take a screenshot (private) with each
profession tab selected. On one page, look at the tier rows of completed tiers, of the tier
being earned and of a later tier; hover a meter and an icon.

**Record:** the tab order against the vocation's profession order (first, second, third);
whether a "train more" or store tab is present; for the tier rows, which show an icon, which a
meter, which nothing; the meter's text form ("current/cap", a percentage); whether proficiency
and mastery meters sit side by side.

<a id="o167"></a>
## O167. Crafting panel: the recipe list (P6)

**Spec:** `crafting-panels.md` §4.4 (tree, texts, order; Tessera rules on sorting and
expansion), §4.5 (templates, makeable and pack counts, states), §4.6 (search), §17 rows O181,
O182, O186.

**Tessera's side:** tiers lowest first, categories sorted by name ignoring case, recipes in
the feed's report order; every tier and category is expanded when a page is built; a craftable
recipe's row shows "name (n)" and its row, category and tier texts turn green
(`RecipeGroup_Active`); search keeps rows whose text holds every typed word.

**What to do:** open a profession page fresh after login. Note which rows are expanded. Read the
order of the tier rows, of the categories under one tier and of three neighbouring recipes.
Note one recipe whose row shows a count and one without, and the ingredients you hold for
both. Type two words in the search field.

**Record:** expanded rows at first opening; tiers lowest or highest first; whether categories
are alphabetical; whether the three recipes keep their order after a relog; whether the count
equals what your held ingredients allow (§15.1); whether tier and category rows turn green with
a craftable recipe below; whether search matches part words and the count text.

<a id="o168"></a>
## O168. Recipe page: result, outputs, ingredients and requirements (P6)

**Spec:** `crafting-panels.md` §5.2 (name, result, experience line, cooldown, outputs; Tessera
rule on output choices), §5.3 (tool and facility lines), §5.4 (ingredients, packs), §5.5
(optional ingredients, critical result, critical chance), §17 rows O184, O191.

**Tessera's side:** the page shows the result with its quantity, the experience line, the
cooldown line, six ingredient slots with "held/needed name", the tool and facility lines (red
in `requirement_notmet` when the feed's tool bits or effects do not pass), the optional slots
one per page behind the master overlay until the tier is mastered, and the critical chance as
a whole percentage; a multiple-output recipe has one page more than its output list, page 1
being the recipe's own result.

**What to do:** select a plain recipe, a recipe with a pack, one with optional ingredients and a
critical result, a multiple-output recipe and a cooldown recipe. For a recipe that needs a
facility, look at the page once next to the facility (a forge, an oven) and once away from it.
Check and uncheck an optional ingredient and watch the critical chance.

**Record:** for the multiple-output recipe, how many pages the selector shows and whether the
first is the recipe's own result; the facility line's colour near and away from the facility,
and the tool line's colour without the tool; how the optional slots are shown (one per page, all
at once) and whether the chance changes by the slot's bonus; how the not-held ingredient is
drawn; whether the experience line names the tier.

<a id="o169"></a>
## O169. Make controls: quantity, Make, Make All and hiding (P6)

**Spec:** `crafting-panels.md` §5.6 (quantity rules, the request, busy state; Tessera rule on
when busy ends), §5.7 (when Make is enabled), §5.8 (closing), §17 row O183.

**Tessera's side:** the quantity entry takes at most 3 digits and is clamped to 1 … the
makeable count; Make All sets min(count, 999); while a request is busy both buttons are ghosted;
with "hide when crafting" checked the panel fades out and returns when the last result of the
request (or an interrupted craft bar) has been reported.

**What to do:** with ingredients for at least four, type 9 and then 007 in the quantity field;
use the arrows past both ends; check "hide when crafting" and craft 3; during the batch, try to
press Make; craft 3 again and move to interrupt it.

**Record:** what the field shows after each typing; whether the panel reappears after each
craft or after the last; whether Make can be pressed during the batch; what an interruption
does to the remaining crafts and to the panel.

<a id="o170"></a>
## O170. Crafting bar and result lines (P6)

**Spec:** `crafting-panels.md` §6.1 (the induction bar in the original), §6.2 (Tessera bar,
result lines; "Critical success" is Tessera wording), §17 row O185; `combat-ui.md` §7 (cast
bar) and TR25 (labels).

**Tessera's side:** a craft shows the cast bar of kind `Craft` labelled with the recipe's
induction title; each `CraftResult` posts one system line (success, failure or Tessera's own
"Critical success") with the item's name and quantity; the proficiency meter moves only on the
feed's `ProfessionChanged`.

**What to do:** craft one item that succeeds, one that crits (with an optional ingredient), and
interrupt one craft by moving. Watch the bar and the chat.

**Record:** the bar's position, length and label (whether it is the recipe's name or a generic
craft title; no text copied); how long it runs for a common recipe; the chat lines after a
success, a critical success and an interruption (how many lines, which chat tab, whether the item
name is a link, whether the quantity is shown); whether a floating text or sound accompanies a
critical success; whether the experience gain is announced.

<a id="o171"></a>
## O171. Recipe-scroll tooltips (P6)

**Spec:** `crafting-panels.md` §7 (rows of `Recipe_ExamineLayout`; Tessera rule for the one-use
and cooldown rows), §17 row O187; `item-tooltips.md` §6.20, §8.3 (compare type 4).

**Tessera's side:** a recipe scroll's tooltip has the one-use row for a one-use recipe and the
cooldown row with the cooldown's duration for a cooldown recipe, otherwise neither; the
requirement rows show profession, tier and (some scrolls) mastery; the result item's tooltip is
attached beside it.

**What to do:** hover a one-use recipe scroll, a cooldown recipe scroll and a plain one, in your
bags or at a vendor.

**Record:** the rows of each tooltip in order (by role); where the one-use and cooldown rows
appear and how the duration is written (units); whether the result item's tooltip is attached
and on which side; how the requirement rows look when you do not meet them.

<a id="o172"></a>
## O172. Crafting trainers (P6)

**Spec:** `crafting-panels.md` §8.1 (choice-window rows; Tessera rule), §8.3 (trainer windows,
selector, confirmation; Tessera rules), §17 rows O188, O189; `npc-windows.md` §5.3.

**Tessera's side:** the choice window lists the trainer's professions, vocations or guilds with
their icons and names (vocations with their three professions on a second line); the trainer
window shows the NPC's name, the learn or change text, and for a profession trainer with full
slots the "lose profession" selector; a vocation change asks for confirmation first.

**What to do:** talk to a profession trainer, a vocation trainer and a craft-guild trainer.
Screenshot (private) the choice rows and each trainer window. At a vocation trainer, with a
vocation already chosen, press Train and look at the confirmation; **do not confirm**.

**Record:** the number of choice rows and their order; whether vocation rows have a second line
and what it lists (by role); which text the trainer window shows (learn or change, by role);
whether the selector appears and what it lists; the confirmation's buttons and whether it names
the old vocation.

<a id="o173"></a>
## O173. Gathering nodes: look, cursor, use and gathering bar (P6)

**Spec:** `crafting-panels.md` §9.1 (gathering nodes; what the client shows; Tessera rule for
gathering from the feed), §17 row O190; `interaction.md` §3.3–§3.4 (cursors, range, mouse-over)
and §3.8 (inductions).

**Tessera's side:** a node is drawn from its own object at its scale; hovering it shows the
container cursor, and the out-of-range cursor beyond 3.5 m; using it sends a use request and the
scenario answers with a 7-second bar labelled with the induction's title, a loot window and a
profession report; the node then disappears.

**What to do:** find an ore, wood, crop or ruin node. Hover it from far away and from close by,
with and without the right tool and profession. Gather it once (and once while mounted, if the
node allows it).

**Record:** whether the node glows or is highlighted at a distance; the cursor in range, out of
range and without the tool or profession; the bar's label (generic or per kind) and length; what
opens afterwards (a loot window or straight to the bags); whether the node disappears at once or
fades; whether a refusal shows as a chat line, a screen message or both.

<a id="o174"></a>
## O174. Plugin progression and crafting views (P6)

**Spec:** `lua-plugin-api.md` §15.6 (progression and crafting views: `GetProfessionInfo`,
`ProfessionInfo`, `Recipe`, `GetUntrainedSkills`, `GetVocation`, `VocationChanged`; its OPENs
and proposed rows O192–O195).

**Tessera's side:** the views read the feed's progression model; proficiency and mastery
levels, recipe lists and the untrained list follow the Tessera rules of §15.6; events are queued
and delivered in frame-loop step 7.

**What to do:** with your own test plugin (as for O154), print for each profession of a character
`GetProficiencyLevel()`, `GetMasteryLevel()` and both titles next to the crafting panel's tier
rows; craft until a proficiency tier completes and log the `ProfessionInfo` events; learn a
recipe and print `RecipeAdded`'s argument table (`Index`, the type of `Recipe`); print
`GetUntrainedSkills()` (price, level, rank; no names) before and after training at a trainer and
after a level-up; print `GetVocation()` before and after a vocation change.

**Record:** whether the levels are the tier being earned or the number of completed tiers; the
events and their order at a tier completion; the `RecipeAdded` fields and types; the untrained
list's length and numbers at the three moments; whether `VocationChanged` fired and how often.
**Do not record names.**

<a id="o175"></a>
## O175. Panel toggle keys (P6)

**Spec:** `crafting-panels.md` §12 (`ToggleCraftingPanel` on T; the three profession toggles,
unbound; OPEN for `keymap.md`); `progression-panels.md` (the trait panel and skill-training
toggles); `keymap.md` §8.2 (defaults that take walker keys).

**Tessera's side:** T toggles the crafting panel and the profession toggles open it at that tab
(Tessera rule); the trait-panel toggle follows `progression-panels.md`; where a default clashes
with a walker key, `keymap.md` records the choice.

**What to do:** with default key bindings (or after a reset), press the keys for the crafting
panel, the trait panel and the skills panel; open the key-binding options and look up the
crafting profession entries and any trait or training entries.

**Record:** which key opens which panel; whether a second press closes it and whether Escape
does; whether the profession entries have a default key and what they do when pressed (open the
panel at that tab or something else); whether opening one panel closes another.
