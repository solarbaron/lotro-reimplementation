<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Legal position and project policy

> **NOT LEGAL ADVICE.** This document is engineering research written by
> non-lawyers so that the project can make careful decisions. It is not a legal opinion,
> it may be wrong, and the law changes. Anyone who contributes, distributes or runs this
> software is responsible for their own legal position. **Before any public release, and
> before any networked feature beyond the local sandbox, the project must obtain review by
> a qualified IP lawyer** (see [§9](#9-gates-and-the-official-server-target)). For
> official-server compatibility (T4) the project owner decided on 2026-10-11 to proceed
> with development **without** such a review ([decision 0034](decisions/0034-official-server-compatibility.md));
> no review exists, and §9 lists the risks that decision accepts. The call for a review
> before any public release stands.

Status: adopted 2026-10-03; §17 (plugin API names) added 2026-10-06; §9.5 and §18
(extended scripting, [decision 0017](decisions/0017-extended-scripting.md)) added
2026-10-07; §1, §8, §9, §13, §15, §16 and §18 updated 2026-10-11 for
[decision 0034](decisions/0034-official-server-compatibility.md) (official-server
compatibility in scope by the owner's decision, without a legal review). Sources were fetched
and read on 2026-10-03 unless another date is given. Quoted contract text is from the versions dated below; Daybreak can change
these documents at any time, so re-check them before relying on this page.

Related documents: [CLEAN_ROOM.md](CLEAN_ROOM.md) (how we write code without copying),
[../DISCLAIMER.md](../DISCLAIMER.md), [../CONTRIBUTING.md](../CONTRIBUTING.md).

---

## 1. Summary

1. **Tessera** (working codename) is an independent, from-scratch game client that reads
   data files from a user's own, legally obtained installation of *The Lord of the Rings
   Online*™, **in place and read-only**. It ships no game content.
2. *The Lord of the Rings Online* is a **live, commercial game**. No form of this project is
   risk-free while that is true. In 2025–26 Daybreak sued the operators of an EverQuest
   emulator and won an injunction and a $3.5M consent judgment
   ([§6.1](#61-daybreak-v-takahashi-the-heroes-journey-2025-26)).
3. Risk rises steeply with scope. The project orders its work on a five-step ladder
   ([§8](#8-risk-tier-ladder)): format library → offline engine → networked client with a
   project-owned server → emulated gameplay → connectivity with official servers. The
   project works on every tier except emulated gameplay (T3, not planned).
4. The project owner has chosen **official-server compatibility** (T4). Research rates that
   the **highest-risk tier**. Until 2026-10-11 it was gated behind a written legal review and
   permission from the rights holders. On 2026-10-11 the owner decided that it is **in scope
   now, without either** ([decision 0034](decisions/0034-official-server-compatibility.md)):
   no lawyer has reviewed it and no rights holder has agreed, and the owner accepts the
   residual risks ([§9](#9-gates-and-the-official-server-target)). Two conditions stay
   binding: a ban-risk warning every user must accept before any official connection (G4),
   and the permanently forbidden list of §8 (G5). Users who connect to an official server
   do so at their own risk. The project stops if Daybreak or MEE objects
   ([§9.4](#94-how-the-gate-can-fail)).
5. The most defensible shape for the project is the OpenMW/ScummVM model: original code,
   the user's own data read in place, no assets shipped, non-commercial, a neutral name,
   and a strict clean-room process ([CLEAN_ROOM.md](CLEAN_ROOM.md)).
6. The only robust legal end state is **written permission** from Daybreak Game Company LLC
   (code and game) **and** Middle-earth Enterprises, LLC (marks and Tolkien-derived IP).

---

## 2. Who owns what

From the lotro.com site footer (read 2026-10-03):

| Rights holder | What they hold | Footer wording (abridged) |
|---|---|---|
| **Daybreak Game Company LLC** (San Diego, CA) | Copyright in the game software, client and server code, and game assets; the Standing Stone Games marks; the EULA and ToS that bind players | "THE LORD OF THE RINGS ONLINE™ interactive video game © 2026 Daybreak Game Company LLC." "Standing Stone Games and the Standing Stone Games logo are trademarks or registered trademarks of Daybreak Game Company LLC." |
| **Middle-earth Enterprises, LLC** (MEE; part of Embracer Group since 2022) | Trademarks *The Lord of the Rings Online*, *Shadows of Angmar*, *Mines of Moria*, *Siege of Mirkwood*, *Rise of Isengard*, *Riders of Rohan*, *Helm's Deep*, ***Mithril***, the MEE logo, and "the characters, items, events, and places therein"; film/game/merchandise rights | "...are trademarks of Middle-earth Enterprises, LLC used under license." |
| **The Tolkien Estate** | Literary copyright in the books | — |
| **Standing Stone Games (SSG)** | Develops and operates the game; a Daybreak brand. There is no separate SSG ToS any more: lotro.com links to Daybreak's EULA, ToS and conduct policy. | — |

Practical notes:

- Daybreak registers client and server code with the US Copyright Office (the THJ judgment
  lists EverQuest client-side and server-side registrations). Assume the LOTRO client is
  registered too, which makes statutory damages available.
- MEE enforces its marks actively (oppositions and cease-and-desist letters against
  "Lord of the …" names, and the 2022 C&D to the *Echoes of Angmar* LOTRO emulator).
  Trademark is a separate risk from copyright and is handled in [§10](#10-naming-and-trademark-rules).

---

## 3. Contract terms that bind every player and contributor

Anyone who installed the game accepted these. They are contracts; the project cannot
change them. Quoted fragments are verbatim from the versions named.

### 3.1 Daybreak End User License Agreement ("Last Updated on April 24, 2026")

| Clause | What it says (quoted fragment) | Why it matters here |
|---|---|---|
| §2 | licence is "solely for your personal, non-commercial use … solely in connection with playing Games via an authorized Account" | Use of the data with any other engine is outside the licence grant. |
| §4.1 | may not "copy (except to make one necessary back-up copy), distribute, … modify or create derivative works" | Why we never copy, repack, patch or ship game files. |
| §4.2 | may not "reverse engineer, disassemble, decompile … or attempt to … discover any source code, underlying ideas, algorithms, **file formats** or programming interfaces … by any means whatsoever, **except to the extent that this restriction is expressly prohibited by applicable law**" | Covers format research. The carve-out matters in the EU/UK ([§5](#5-eu-and-uk-law)). |
| §4.3 | may not "create, facilitate, host, link to or provide any other means through which any Game(s) may be played by others through interception, **emulation** or redirection of the communication protocols used by Daybreak, including … protocol emulation, tunneling, data mining, packet sniffing, modifying or adding components" | Directly targets protocol-compatible clients and servers. |
| §4.4 | may not "create, facilitate, or maintain any unauthorized connection to any Game(s)" | A third-party client talking to official servers is an "unauthorized connection" unless Daybreak authorises it. |
| §4.5 | may not "mine, decrypt, or modify any data transmitted between any client and server" | The official login and session traffic are encrypted ([§9.2](#92-why-it-is-the-highest-risk-tier)). |
| §4.6 | no "cheats, hacks, mods, macros, 'bots'" | — |
| §4.7 | may not "incorporate the Software … including … Daybreak Game Assets(s), into any product or service" | Why we ship no assets. |
| §5 | consent to monitoring for an "Unauthorized Third Party Program" | Detection of non-official software can lead to account action. |
| §11 | Delaware law; courts in Delaware or San Diego County | — |
| §12 | mandatory individual AAA arbitration and class waiver; court injunctions for IP remain available | THJ: arbitration was compelled, but the IP injunction stayed. |

### 3.2 Daybreak Terms of Service ("Last Updated: July 13, 2026")

| Clause | What it says (quoted fragment) | Why it matters here |
|---|---|---|
| §7.1 | must not "Attempt to play any Daybreak Game(s) on any server that is not controlled or authorized by Daybreak" | Own-server and emulated-gameplay tiers. |
| §7.1 | must not "Create, use or provide any server emulator … and to refrain from **posting or distributing any utilities, emulators or other software tools related to any Daybreak Game(s)** without the express written permission of Daybreak" | Arguably reaches **this project itself**, including the format library and tools. |
| §7.1 | must not "Modify any part of any Daybreak Game(s), including … client and/or server" | We never modify the official client or its files. |
| §7.1 | must not "Attempt to disable or circumvent any measures that we use to prevent or restrict access to our accounts, servers" | Official-server tier. |
| §7.1 | must not "Attempt to interfere with, hack into, or decipher any transmissions to or from the servers" | Official-server tier; also why we keep no official packet captures. |
| §10 | "You may not disassemble, reverse engineer, or modify any Daybreak Game(s) software … You may not decrypt or modify any data transmitted" | Repeats EULA §4.2/§4.5. |
| §12 | "The client-side software may not be … reverse engineered, modified, **used as a stand-alone game (without connecting to Daybreak's game servers), or used in connection with non-Daybreak game servers**." Breach terminates the licence automatically. | Offline and own-server use of the data is outside the licence. Whether this is a contract promise or a copyright *scope condition* is an open question ([§4.1](#41-copyright)). |
| §14.4 | may not "use Daybreak's name, trademarks, and/or the name of any Daybreak Game(s) in any Fan Site name, domain name or App name", nor names "confusingly similar"; UGC must not "suggest or imply any sponsorship" | Naming rules ([§10](#10-naming-and-trademark-rules)). |
| §25 | arbitration, with an opt-out procedure | — |
| §26.2 | Daybreak may "suspend, restrict or terminate your access … at any time and for any reason" | Account bans need no stated cause. |

### 3.3 Consequences

- **In the US, anti-reverse-engineering clauses in click-through licences are enforceable**
  and are not pre-empted by copyright law: *Bowers v. Baystate*, 320 F.3d 1317 (Fed. Cir.
  2003); *Davidson & Assocs. v. Jung* ("bnetd"), 422 F.3d 630 (8th Cir. 2005). bnetd:
  "'[P]rivate parties are free to contractually forego the limited ability to reverse
  engineer…'"
- So **every contributor who has accepted the EULA/ToS is personally exposed** to a
  breach-of-contract claim for reverse-engineering work and for distributing "software tools
  related to" the game. See the contributor-risk notice in [§13](#13-contributor-risk-notice).
- **Users are exposed too**: running the data with any non-Daybreak engine or server is
  outside ToS §12, and Daybreak may terminate any account for any reason (ToS §26.2).
- **No tier is clean under the contract.** Offline and own-server use collide with ToS §12
  ("stand-alone", "non-Daybreak game servers"); official-server use collides with EULA
  §4.3–4.5 and ToS §7.1.

### 3.4 Informal tolerance is not a licence

- On 2020-11-10 an SSG community manager wrote about an accessibility/FOV tool: "We cannot
  provide legally-binding clearance … using something like this as an accessibility tool …
  is not something we would be likely to be concerned about." That is not permission.
- Dat-reading fan tools, a GPL launcher, and FOV tools have existed for years without
  enforcement we could find. The public GitHub DMCA archive (checked 2026-10-03) contains no
  Daybreak/SSG/MEE notices about LOTRO, but it does contain Daybreak notices against a Free
  Realms repo (2016-09-14, including "packet logs") and against EverQuest client files
  (2026-09-09).

---

## 4. United States law

### 4.1 Copyright

| Authority | Holding (summary) | Lesson for us |
|---|---|---|
| *Sega v. Accolade*, 977 F.2d 1510 (9th Cir. 1992) | Disassembly as intermediate copying is fair use when it is the only way to reach functional elements and there is a legitimate reason. | Supports research, but the THJ court refused to apply it to a project that copied Daybreak's assets and avoided "its own creative work". |
| *Sony v. Connectix*, 203 F.3d 596 (9th Cir. 2000) | Intermediate copies made while writing a from-scratch emulator were fair use; the final product contained no Sony code. | The final product must contain no Daybreak code. |
| *Galoob v. Nintendo* (9th Cir. 1992); *Micro Star v. FormGen* (9th Cir. 1998) | A tool that alters display without incorporating the work is not a derivative work; distributing files that *describe* protected audiovisual content is. | The engine is fine. Shipping maps, quest text or item databases is not. |
| *Google v. Oracle*, 593 U.S. 1 (2021) | Reimplementing API declarations for interoperability was fair use. | Supports a compatible Lua plugin API surface. Never copy the original `.lua` files or documentation prose. Full record: [§17](#17-plugin-api-names-interoperability-record). |
| 17 U.S.C. §102(b) | Ideas, procedures, methods of operation are not protected. | File formats and protocols as functional facts. |
| *MDY v. Blizzard*, 629 F.3d 928 (9th Cir. 2010, amended 2011) | Breaching a licence term is copyright infringement only if the term is a *condition* with a nexus to an exclusive right; otherwise it is contract only. | **Open:** if ToS §12 is a scope condition, users' in-memory copies of game data in our engine could be infringing and the project could be a contributory infringer. |
| *Vernor v. Autodesk* (9th Cir. 2010) | Licensees may not be "owners" for §117. | Do not rely on §117. |
| THJ PI order (ECF 66, 2025-09-19) | Telling users to copy their client files into a new folder for use with the emulator was "instigating third-party infringement". | **Never copy, repack, patch or modify user data files. Read in place, read-only.** |

### 4.2 DMCA §1201 (anti-circumvention)

- **§1201(f)** lets a person who has lawfully obtained the right to use a program circumvent
  an access control "for the sole purpose of identifying and analyzing those elements of the
  program that are necessary to achieve interoperability of an independently created
  computer program with other programs", where the elements were not previously readily
  available and the acts are not infringing. (f)(2) permits developing the means, (f)(3)
  permits sharing the information for interoperability, and (f)(4) defines
  "interoperability" as "the ability of computer programs to exchange information, and of
  such programs mutually to use the information which has been exchanged."
- **bnetd** applied four elements: (1) lawful right to use; (2) information not already
  readily available; (3) sole purpose of interoperability; (4) no infringement. bnetd failed
  (4) because its server let unauthorised copies be played. **We never support pirated or
  unofficial installs and never link to client mirrors.** bnetd also held that the EULA
  waiver of reverse engineering was enforceable.
- **MDY v. Blizzard**: in the 9th Circuit, §1201(a)(2) trafficking needs no infringement
  nexus (contrast *Chamberlain v. Skylink*, Fed. Cir. 2004). Server-side access control
  protected the game's "dynamic non-literal elements".
- **THJ PI order (2025-09-19)**: server-side account verification and authorisation are
  technological measures that "effectively control access". Redirecting the official
  client to a fan server and bypassing its login was likely circumvention, and distributing
  the result was trafficking. The court was "skeptical" that a defendant must have touched
  Daybreak's servers. **This is the main theory a networked, protocol-compatible client
  would face.** We have distinguishing facts only as long as we never modify or redirect
  the official client, never bypass Daybreak authentication, and never ship Daybreak keys.
- Other enforcement: *Nintendo v. Tropic Haze* (Yuzu, 2024, $2.4M settlement);
  *Blizzard v. Bossland* (~$8.5M, 2017); *Blizzard v. Reeves* ($88M default, 2010).
- The 2024 Copyright Office preservation exemption (37 CFR 201.40) covers only libraries,
  archives and museums, only for games whose servers have shut down. It does not help a
  live game.

### 4.3 Trademark

- Nominative fair use permits plain-text, factual reference ("compatible with data files
  from The Lord of the Rings Online™") with no logo, stylisation or implied endorsement.
- Daybreak's own line in the 2015 Project 1999 letter: the game name may be used
  "descriptively in plain text (without a logo or stylization)" with a non-affiliation
  disclaimer.

---

## 5. EU and UK law

The EU and UK give lawful users **mandatory** rights that a contract cannot remove.

| Provision | Content | Effect here |
|---|---|---|
| Directive 2009/24/EC **Art 5(3)** | A person with a right to use a program may, without authorisation, "observe, study or test the functioning of the program in order to determine the ideas and principles which underlie any element of the program" while loading, displaying, running, transmitting or storing it. | Black-box study: reading our own data files, watching our own client's behaviour, logs, frames and traffic. This is the project's default research method. |
| **Art 6(1)** | Decompilation is allowed where "indispensable to obtain the information necessary to achieve the interoperability of an independently created computer program with other programs", by a person with a right to use, where the information is not already readily available, confined to the necessary parts. | Disassembly is lawful only in this narrow case. Not authorised by default in this project ([CLEAN_ROOM.md §4](CLEAN_ROOM.md#4-research-methods)). |
| **Art 6(2)** | The information may not be used for other goals, given to others except as needed for interoperability, or used for "a computer program substantially similar in its expression". | Specs carry functional facts only, never expression. |
| **Art 6(3)** | Must not unreasonably prejudice the rightholder or conflict with normal exploitation. | Open question for a live subscription game. |
| **Art 8** | "Any contractual provisions contrary to Article 6 or to the exceptions provided for in Article 5(2) and (3) shall be null and void." | EULA §4.2 is void in the EU to that extent. Delaware choice of law does not remove a consumer's mandatory protection (Rome I Art 6). |
| CJEU *SAS Institute v WPL*, C-406/10 (2012) | "the functionality, programming language and data file format of a computer program" are not protected expression; Art 5(3) study cannot be contracted away. | Strong support for reimplementing the data formats and behaviour. |
| CJEU *Top System*, C-13/20 (2021) | Decompilation for error correction under Art 5(1), as far as necessary. | — |
| Database Directive 96/9/EC | Extracting or re-using a substantial part of a database can infringe the sui generis right. | Another reason never to ship extracted tables. |
| UK CDPA s.50B, s.50BA, s.296A | Mirror Arts 6, 5(3) and 8. | Same position in the UK. |

**Open:** whether Art 6 "interoperability with other programs" covers interoperability with
*data files* or a *fan server*, rather than with Daybreak's own servers. Interoperability with
the official servers is the clearest Art 6 case, but it is also the case the contract and
§1201 attack hardest ([§9.2](#92-why-it-is-the-highest-risk-tier)).

---

## 6. Precedents

### 6.1 Daybreak v. Takahashi ("The Heroes' Journey", 2025-26)

S.D. Cal. No. 3:25-cv-01489 (EverQuest emulator).

| Date | Event |
|---|---|
| 2025-06-14 | Complaint: copyright, DMCA §1201, Lanham Act, unfair competition, breach of contract. |
| 2025-09-19 | Preliminary injunction granted (ECF 66) on copyright and §1201 grounds. |
| 2025-10-30 | Arbitration compelled; stay of the injunction denied (ECF 84). |
| 2026-03 | Final judgment and permanent injunction on consent (ECF 104-1): $3.5M "proxy for damages", collection forborne unless breached; operators enjoined from server emulation enabling unauthorised access and from using the EVERQUEST mark. |

Facts the court weighed: the project earned money; it modified client files and redirected the
official client; it told users to copy their client folder; there was likely market
substitution. Pleading "clean-room, reverse engineered, open-source code" and *Sega* did not
help. Estoppel based on Daybreak's long tolerance of other emulators failed because Daybreak
had a record of cease-and-desist letters. Afterwards EQEmulator adopted terms saying
operation is "subject to Daybreak's permission, which it may revoke for any reason".

### 6.2 Other cases

| Project | Outcome | Lesson |
|---|---|---|
| **Project 1999** (EverQuest), 2015-04-23 letter from Daybreak's General Counsel | "not a license … preserve the peace": from-scratch emulator, no Daybreak server code, users obtain their own client, no piracy help, non-commercial, no marks except plain-text names, mandatory non-affiliation disclaimer, release-date deference. | The realistic template for a permission request. For LOTRO, MEE must also agree. |
| **City of Heroes: Homecoming** | Official NCSoft licence on 2024-01-04. | Permission is achievable, but so far only for dead games. |
| **Echoes of Angmar** (LOTRO emulator) | MEE C&D 2022-08-05; project continued; uses the original client and points users to third-party client downloads. | The exact pattern we must never follow. Do not associate with it. |
| **Middle-earth Revenge** (LOTRO RE) | Tools patch the official client with a substitute key; links to downloads of old clients. | Client-patching is the THJ circumvention pattern. Do not use or link. |
| **bnetd** (Blizzard, 2005) | Lost on contract and §1201. | See [§4.2](#42-dmca-1201-anti-circumvention). |
| **MDY v. Blizzard** (Glider bot, 2010) | §1201(a)(2) liability without infringement nexus. | See [§4.2](#42-dmca-1201-anti-circumvention). |
| **re3/reVC** (Take-Two, 2021–23) | Decompilation-derived source: DMCA takedown, lawsuit, settlement. | Decompiled or translated code is derivative. **Never import decompiler output.** |
| **ReactOS** (2006) | Disassembly allegation → development freeze, full audit, signed IP policy. | Why we keep attestation and provenance records. |
| **Wine** | Clean-room guidelines forbid disassembly, looking at vendor source, and (currently) LLM-generated code. | We allow AI assistance only under the rules in [CLEAN_ROOM.md §7](CLEAN_ROOM.md#7-ai-assistance-rules). |
| **OpenMW** | From scratch, user owns the game, no assets, own fonts; the rightsholder informally approved it. | Our model, but Morrowind is not a live online game. |
| **ClassicUO** (Ultima Online) | "Using a custom client to connect to official UO servers is strictly forbidden." | The comparable MMO client project rules out official servers. |
| **WoW private servers** | Scapegaming $88M default judgment (2010); Turtle WoW sued 2025, offline 2026. | Hosted gameplay emulation of a live game is where the lawsuits are. |

---

## 7. What these facts mean for project design

These follow from §3–§6 and are binding project policy:

1. All code is original and written from specs under [CLEAN_ROOM.md](CLEAN_ROOM.md).
2. User data files are read **in place, read-only**. No installer, tool or feature copies,
   repacks, patches, "fixes" or modifies them.
3. The repository and releases contain **no game content** ([§11](#11-content-rules)).
4. No Daybreak/SSG hostnames, keys or bulk data tables extracted from the client binary are
   hard-coded or committed. Anything of that kind is loaded at runtime from the user's own
   install, and only where [decision 0034](decisions/0034-official-server-compatibility.md)
   allows it (official endpoints and key material, for T4). Numeric constants that define an
   algorithm may be used when a spec records them
   ([decision 0012](decisions/0012-numeric-constants.md)).
5. Never modify, redirect, inject into or wrap the official client.
6. Never help piracy: documentation points only to official download channels.
7. Free and non-commercial ([§12](#12-distribution-and-money)).
8. Neutral name and mandatory disclaimer ([§10](#10-naming-and-trademark-rules)).

---

## 8. Risk tier ladder

Each tier includes everything below it. "Status" is the project's current position.

| Tier | Scope | Main exposure | Status |
|---|---|---|---|
| **T0 Format library** | Read `.dat`/`.datx` archives and decode records; CLI inspection tools (`dattool`) for personal use. | EULA §4.2 (void in EU to the Art 5(3)/6 extent); ToS §7.1 "software tools related to"; contract exposure for US contributors. | **In scope now.** |
| **T1 Offline engine** | Renderer, world walker and UI that read the user's install in place; no network. | T0 plus ToS §12 "used as a stand-alone game"; open MDY scope-condition question; MEE trademark if naming slips. | **In scope now.** |
| **T2 Own-server networked** | Client speaks a **project-defined** protocol to the in-process sandbox or a **project-owned test server** that serves only original or test content. | T1 plus ToS §7.1 "server … not controlled or authorized by Daybreak"; ToS §12 "non-Daybreak game servers". Lower if the protocol is our own and the server holds no game content. | **In scope now: sandbox and project-owned test server only.** |
| **T3 Emulated gameplay** | A server that reproduces the game's quests, NPC scripts, combat or world, or any publicly hosted shard, free or not. | Copyright in game content and server behaviour; EULA §4.3; ToS §7.1 "server emulator"; THJ, WoW private-server cases; MEE C&D precedent (EoA). | **Not planned. Requires written permission from Daybreak and MEE.** |
| **T4 Official-server connectivity** | Client connects to Daybreak's live servers: official login/launcher protocol, session encryption, the live game protocol. | **Highest.** EULA §4.3, §4.4, §4.5; ToS §7.1, §10, §12; DMCA §1201 (THJ theory); account bans for every user. | **In scope by the owner's decision of 2026-10-11, without a legal review or permission ([decision 0034](decisions/0034-official-server-compatibility.md)); G4 and G5 binding; see §9.** |

**Permanently forbidden at every tier, whatever any decision says (G5 of §9.3):**

- shipping, mirroring or linking to client binaries, dat files, patches or old clients;
- modifying, patching, redirecting or injecting into the official client;
- embedding or publishing Daybreak keys or other secrets in the repository, releases or
  documentation;
- committing packet captures from official servers (keep any private to the dirty room and
  reduce them to field-level facts; see [CLEAN_ROOM.md §4](CLEAN_ROOM.md#4-research-methods));
- using leaked, NDA-covered or decompiled Turbine/SSG/Daybreak source code;
- monetisation of any kind beyond donations for infrastructure costs;
- bypassing account authentication, subscription checks, anti-cheat or any other access
  control.

---

## 9. Gates and the official-server target

Rewritten 2026-10-11 by [decision 0034](decisions/0034-official-server-compatibility.md).
Until then this section gated T4 behind a written legal review; the text of that gate is
kept in [decision 0004](decisions/0004-server-strategy.md) and in this file's history.

### 9.1 The decision

The project owner has chosen **official-server compatibility** (T4): a client that, like the
official one, can log in to and play on Daybreak's servers. On 2026-10-11 the owner decided
that this work is **in scope now** ("the official server compatibility will be here so
change the legal and the plans"). Asked whether a lawyer's review or permission from the
rights holders exists, the owner chose to proceed on the owner's own decision and to keep
conditions G4 and G5 of §9.3 binding.

Stated plainly:

- **No legal review exists.** No qualified lawyer has reviewed the design.
- **No permission exists.** Neither Daybreak Game Company LLC nor Middle-earth Enterprises
  has been asked or has agreed.
- The owner accepts the residual risks of §9.2 with that knowledge
  ([decision 0034](decisions/0034-official-server-compatibility.md) §5). That acceptance
  binds the project's plans; it does not change the law, the contract terms of §3, or any
  user's or contributor's own exposure.

### 9.2 Why it is the highest-risk tier

Research rates T4 as the riskiest thing this project could do. These risks are **not
removed** by the owner's decision; they are the residual risks the owner accepts. Plainly:

- **Contract.** EULA §4.3 forbids providing any means of playing "through interception,
  emulation or redirection of the communication protocols"; §4.4 forbids "any unauthorized
  connection"; §4.5 forbids decrypting client–server data. ToS §7.1 forbids deciphering
  transmissions and circumventing access measures; ToS §10 repeats the decryption ban; ToS
  §12 restricts the client software. A third-party client on the live service breaches these
  on its face unless Daybreak authorises it. Contributors who accepted the EULA carry this
  personally ([§13](#13-contributor-risk-notice)).
- **DMCA §1201.** The official login uses encryption and key material that ships inside
  the official client (reported by published third-party research). The THJ court treated
  server-side authentication as an access control and the act of distributing a
  workaround as trafficking. Implementing the official login in third-party code risks
  §1201(a)(1) circumvention and (a)(2) trafficking claims, and in the 9th Circuit those
  claims do not need any copyright infringement. §1201(f) is narrow, bnetd held it waived by
  contract, and its element (4) fails if any infringement is involved. The project's
  position is that authenticating with the user's own credentials is not bypassing
  authentication (G5); a court may disagree.
- **Account bans for every user.** EULA §5 allows detection of "Unauthorized Third Party
  Programs", and ToS §26.2 allows termination "at any time and for any reason". Every person
  who logs in with this client risks their account, purchases and characters. Extended
  scripts on an official server ([§9.5](#95-extended-scripts-and-official-servers)) add to
  that risk.
- **Service integrity.** A third-party client on a live service can be treated as cheating
  or interference (ToS §7.1, §10) even if it does nothing unusual.
- **EU law helps less than it seems.** Art 6 covers *obtaining* interface information. It
  does not license connecting to a service against its terms, and Art 6(3) protects the
  rightholder's "normal exploitation". Outside the EU and UK even that does not apply.

### 9.3 The gate

Until 2026-10-11, T4 work needed **all** of conditions G1–G5 recorded in
`docs/decisions/`. Decision 0034 settles each one:

| # | Condition | Status (decision 0034) |
|---|---|---|
| G1 | A **written legal review** by a qualified IP lawyer, covering at least US and EU/UK law, of the specific design to be built. | **Set aside by the owner.** Not required for M8. Still recommended, above all before any public release (banner, [§16](#16-open-questions-for-counsel)). |
| G2 | **Written permission from Daybreak Game Company LLC** (cf. the 2015 Project 1999 letter; the 2024 Homecoming licence), and **Middle-earth Enterprises** approached separately for the marks. | **Set aside by the owner.** Not required for M8. Engagement ([§15](#15-engagement-strategy)) stays the preferred route. |
| G3 | A maintainer decision record that cites G1 and G2, lists the residual risks, and is accepted by the project owner. | **Met** by [decision 0034](decisions/0034-official-server-compatibility.md), which records the absence of G1 and G2 instead of citing them, lists the residual risks of §9.2, and was accepted by the owner on 2026-10-11. |
| G4 | A user-facing warning, shown before any official connection is possible, that states the account-ban risk and the terms in [§3](#3-contract-terms-that-bind-every-player-and-contributor). | **Binding.** Shown before any connection to an official server is possible, on first use and whenever its text changes; accepted only by the user's explicit confirmation in the client's own interface; no flag, environment variable, configuration written by anything else, plugin, script or server can accept or skip it. |
| G5 | The design still obeys the "permanently forbidden" list in [§8](#8-risk-tier-ladder): no modified official client, no shipped keys, no bypassed authentication. | **Binding**, in full, at every step of M8. |

What this allows and what it still limits:

- **Research.** Dirty-room spec writers may study the official game protocol and the login
  flow (the official launcher's part of it included) as far as interoperability needs
  them, by disassembly and decompilation of read-only copies of the client and, for its
  part of the login flow only, the official launcher
  ([decision 0011](decisions/0011-standing-m4-authorisation.md) as amended by 0034 §2) and
  by passive capture of the owner's own sessions of the unmodified client. Never anti-cheat,
  anti-tamper or code-signing checks, never any circumvention of an access control, and no
  injection, debugger or memory reading on a client connected to official servers.
  Captures stay private to the dirty room, are never committed, and are reduced to
  field-level facts; specs carry no other player's names, chat or account data.
- **Specs and code.** Official-protocol specs and a compatibility backend may be written and
  merged. No Daybreak keys, certificates or other secrets, hostnames, endpoints or account
  data appear in the repository, CI or releases; the client loads them at runtime from the
  user's own install, and a spec says only where to find them. CI never connects to an
  official server. If the service can be used only by circumventing anti-cheat,
  anti-tamper or another access control, the work stops there.
- **Tessera's own server** is unchanged: it never connects to, proxies or imitates an
  official service ([§11.1](#111-servers) rule 6).

<a id="94-how-the-gate-can-fail"></a>

### 9.4 When T4 work stops

G1 and G2 no longer exist as preconditions, so the gate cannot fail on them. T4 work still
stops if Daybreak or MEE objects: **the project will not proceed against an explicit
objection from either rights holder** (see [§14](#14-incident-response)). M8 work then stops,
the compatibility backend is withdrawn (it is its own crate for that reason), the project
returns to T2, and this section is updated to say so. The same happens if the owner
withdraws decision 0034.

<a id="95-extended-scripts-never-run-against-an-official-server"></a>

### 9.5 Extended scripts and official servers

Added 2026-10-07 by [decision 0017](decisions/0017-extended-scripting.md) (§6.4, §6.5);
rewritten 2026-10-11 by [decision 0034](decisions/0034-official-server-compatibility.md) §4.
"Extended scripts" are scripts written for Tessera's own scripting API (global `Ext`,
[§18](#18-extended-scripting)), not the plugins of [§17](#17-plugin-api-names-interoperability-record).
Until 2026-10-11 the rule was that extended scripts never run against an official server.
The owner changed it: "extended scripts will be able to run on official server its just up
to the user if they want to bear the risk to do that".

**Binding project policy:**

1. **Off by default, by the user's own choice.** Extended scripts run on an official-server
   session only after that user's **explicit opt-in**, separate from the warning of G4. The
   opt-in shows the risks: the service may treat scripts as cheating or interference
   (ToS §7.1, §10), the account can be suspended or closed (EULA §5, ToS §26.2), and the
   project cannot know what the service tolerates or protect the account. It is stored per
   user, can be withdrawn at any time, is asked again when its text changes, and is set only
   by the user in the client's own interface: no setting file written by anything else,
   flag, environment variable, plugin, script or server can set it. The user who opts in
   bears the risk.
2. **Grants still apply.** The opt-in grants no capability by itself; each script still needs
   the user's grant, and an official-server session is a backend class of its own. Any
   other backend that Tessera's builds do not know stays fail-closed.
3. **Automation stays offline only.** This is project policy kept by the maintainer, not
   part of the owner's words. Automation (`input.drive`) and every module that issues
   movement, input or game commands on the user's behalf run **offline only** and are
   never enabled on any official server, with or without the opt-in, because their effect
   falls on other players and on the service, not only on the user who accepted the risk.
   No server policy can turn them on. A later decision that wants them on any server must
   address the proxy case of item 4.
4. **It binds Tessera's own builds only.** A client-side rule can only rely on what the build
   reports about its own backends. A modified build, or a proxy run by the user that
   translates Tessera's protocol to another service, defeats any client-side rule: the
   client cannot know what lies behind a connection, and such a user is their own
   "operator", acting outside the project's builds and this policy. The project does not
   offer, document or support either.
5. **Other limits are unchanged.** `data.read` stays offline only
   ([§18.4](#184-decoded-data-and-export)), the never-offered list of
   [§18.3](#183-rules-binding-project-policy) rule 3 stands, and extended scripting is
   milestone M9, so none of this is built before then.
6. **Plugins** are not covered by this section: they are the original client's own feature
   and run on an official server as they do in the original client, under
   [§17](#17-plugin-api-names-interoperability-record).

---

## 10. Naming and trademark rules

1. The product name, crate names, binary names, domains, organisation names, social handles,
   logos and UI must **not** contain or allude to: LOTRO, Lord of the Rings, LotR,
   Middle-earth, Tolkien, Hobbit, Shire, Mordor, Angmar, Moria, Mirkwood, Isengard, Rohan,
   Helm's Deep, **Mithril** (a listed MEE mark), Turbine, Standing Stone, SSG, Daybreak, other
   Tolkien-invented words, or Elvish-styled near-homophones.
2. "Tessera" is a **working codename**. Before any public release the final name must be
   searched on USPTO, EUIPO eSearch/TMview and the WIPO Global Brand Database, and the result
   recorded in `docs/decisions/`.
3. The game may be named **only in plain text and only to state compatibility**, for example:
   "compatible with data files from The Lord of the Rings Online™". No logos, game fonts,
   colours, trade dress, screenshots in branding, or wording that implies endorsement.
4. Format-level technical terms ("DID", "landblock", "dat", "iteration") are fine.
5. **Interoperability identifiers.** The plugin API's namespace, class, member, event and file
   names that existing third-party plugins call (some contain marks from rule 1) may appear
   only inside the plugin subsystem, under the rules of
   [§17](#17-plugin-api-names-interoperability-record). They are never product, crate, binary
   or feature names and never appear in branding.
6. **Mandatory disclaimer.** This exact text must appear in the README, on any website, in
   release notes, and in the application's About screen:

> Tessera is an independent, unofficial project. It is not affiliated with, endorsed by, or
> sponsored by Daybreak Game Company LLC, Standing Stone Games, Middle-earth Enterprises,
> LLC, Warner Bros., or the Tolkien Estate. The Lord of the Rings Online is a trademark of
> Middle-earth Enterprises, LLC used under license. Requires a legally obtained installation
> of the game; no game content is distributed.

(Replace "Tessera" with the final name once chosen. The canonical copy lives in
[../DISCLAIMER.md](../DISCLAIMER.md).)

---

## 11. Content rules

The repository, its issue tracker, CI artefacts and releases must never contain:

- `.dat`/`.datx` files or fragments of them; extracted textures, meshes, animations,
  sounds, music, fonts, UI art, strings, quest or dialogue text, maps, item/quest/deed
  databases, or Lua UI files;
- client or launcher binaries, DLLs, patches, or links to unofficial copies of any of these;
- keys, certificates, hostnames or bulk data tables extracted from the client binary
  (numeric algorithm constants recorded in a spec are allowed,
  [decision 0012](decisions/0012-numeric-constants.md));
- packet captures or logs of traffic with official servers;
- decompiler or disassembler output, symbol maps, shader bytecode.

Allowed:

- **synthetic test fixtures** generated by our own code, and original content released as
  CC0 or under a licence compatible with GPL-3.0-or-later;
- **real-data tests** that read `TESSERA_GAME_DIR` (or an auto-detected install), are skipped
  when no install is present, and **never write to the install directory**;
- screenshots in bug reports only as ToS §14.1 permits (non-commercial, no logos, no implied
  endorsement), kept to what is needed.

**The one third-party binary asset: the UI fallback font**
([decision 0022](decisions/0022-bundled-ui-font.md), owner, 2026-10-08).
`crates/ui/assets/fonts/NotoSans-Regular.ttf` is **Noto Sans Regular** (version 2.015),
copyright The Noto Project Authors, licensed under the **SIL Open Font License 1.1**
(`OFL-1.1`, text in [`LICENSES/OFL-1.1.txt`](../LICENSES/OFL-1.1.txt)). The OFL allows the font
to be bundled with GPL software; the font itself stays under the OFL when Tessera is
distributed (OFL conditions 2 and 5), and its copyright notice and licence go with every copy
(`REUSE.toml` annotation, `crates/ui/assets/fonts/OFL.txt`, and the font's own `name` table).
It is bundled unmodified and under its original name, so the OFL's Reserved Font Name rules
do not apply. It is not a game font: it is Tessera's own fallback for text without a game
layout. `tools/content-allowlist.txt` exempts exactly that path from the font checks.

**Game fonts are still never committed.** The game's fonts and glyph atlases are only ever
loaded at run time from the user's install ([`fonts.md`](specs/fonts.md)). Any other
third-party binary asset needs its own decision.

Exporters and viewers are for personal use, are opt-in, and are never advertised as bulk
extraction or ripping tools. The engine must run with original placeholder content so it
has substantial uses that do not depend on game data. Extended scripts' read access to
decoded data is assessed under these rules in [§18.4](#184-decoded-data-and-export).

### 11.1 Servers

Added 2026-10-06 for ROADMAP M7 (exit criterion 4). These rules apply to `tessera-server` and
to **any** server build that speaks Tessera's own protocol, whoever runs it. The detailed,
checkable version is [specs/server-content.md](specs/server-content.md); the server's
behaviour is in [specs/test-server.md](specs/test-server.md).

> **Status: needs the project owner's confirmation before any public test build.** Until the
> owner records that confirmation here (with a date), Tessera servers are built and run only
> privately by contributors for development and LAN testing. The banner at the top of this
> document also still applies: a lawyer's review is needed before networked features are
> offered beyond the project.

1. **No game data on a server.** A server holds, reads and sends no game data: no archives,
   extracted assets, strings, quest or dialogue text, NPC scripts, or item, quest or map
   databases. It does not read any game installation, the operator's own included, and it
   never sends the static world. Each player's client draws the world from that player's own
   install.
2. **Opaque references are relayed, never collected.** A server may store and pass on the
   record numbers (DIDs and enum keys) that each player's client chooses from its own install,
   for example for a character's appearance. It never resolves them, pairs them with game
   text, or gathers them into lists or exports. A number that only a lawful install can
   resolve is not a copy of the content it names.
3. **Numeric positions.** Server content may contain a few bare numeric positions (at most 16
   per content file) as spawn points, read by the contributor from Tessera or their own
   install, with project-made names. No place names or other game text, no maps or location
   lists, and no positions that reproduce the game's placements.
4. **Original default world.** The default world is project-made: neutral names and texts,
   no game names, text or references, and usable without any game data.
5. **No gameplay reproduction.** Combat, quests, NPC behaviour and other gameplay from the
   game are tier T3 ([§8](#8-risk-tier-ladder)) and out of scope. A server provides accounts,
   presence, movement, chat, appearance as opaque references, and simple project-made test
   actors.
6. **Operators.** Private or LAN use for testing only. No public hosting: nothing listed,
   advertised or open to registration by strangers ([§12](#12-distribution-and-money)). No
   money of any kind. Accounts are created by the operator. Every player brings their own
   legally obtained installation, and nobody is given game files. Server names and messages
   follow [§10](#10-naming-and-trademark-rules). No server connects to or imitates any
   official service ([§9.3](#93-the-gate)).
7. **Committed content** is original, licensed GPL-3.0-or-later or CC0-1.0, and passes the
   review checklist of server-content.md §9.3.

Questions for counsel before a public test build: whether relaying opaque references (rule 2)
and committing numeric positions (rule 3) raise any issue under §4.1 (*Micro Star*) or the EU
database right (§5).

---

## 12. Distribution and money

- Licence: **GPL-3.0-or-later**; every source file carries
  <!-- REUSE-IgnoreStart -->`SPDX-License-Identifier: GPL-3.0-or-later`<!-- REUSE-IgnoreEnd -->. Third-party code may be imported only if its
  licence is compatible and its provenance is clean. Most published LOTRO tools are
  unlicensed (all rights reserved) and must not be copied; AGPL code is not imported.
- Free of charge. No paid tiers, perks, ads, or sales. Donations, if any, cover
  infrastructure costs only (the Project 1999 model).
- No hosted public game servers.
- What a Tessera server may hold and how it may be run: [§11.1](#111-servers).
- Documentation directs users only to official download channels for the game.

---

## 13. Contributor-risk notice

**Read this before contributing.**

- If you have accepted the Daybreak EULA/ToS (that is, you have installed or played the
  game), then studying the game's file formats or protocols, and helping to distribute
  "software tools related to" the game, may breach EULA §4.2–§4.3 and ToS §7.1 **as a
  matter of contract between you and Daybreak**, regardless of this project's licence or
  process. US courts enforce such clauses (*Bowers*, *bnetd*). EU/UK law voids them only
  to the extent of Art 5(3)/6 (UK s.296A).
- Using this software with your own install may breach ToS §12, and Daybreak may close any
  account for any reason.
- **Official-server compatibility is now in scope without a legal review**
  ([§9](#9-gates-and-the-official-server-target), decision 0034). Working on the official
  protocol or login (research, specs or code) carries the contract and DMCA §1201 exposure
  of [§9.2](#92-why-it-is-the-highest-risk-tier) personally, including trafficking theories
  for distributing it. The owner's acceptance of those risks does not cover yours. Nobody
  is required to work on it; say so if you do not want to.
- The project cannot indemnify you and offers no legal defence. A defence fund or a
  nonprofit fiscal host is being considered before any public launch.
- You may contribute under a pseudonym. Maintainers' legal identities should be known
  to each other.
- If you have **ever seen Turbine, SSG or Daybreak source code** (leaked, under NDA, as an
  employee or contractor, or reconstructed by decompilation), you must say so before
  contributing; you may not work on the affected components. Former Turbine/SSG/Daybreak
  staff must also check their employment and NDA obligations.

---

## 14. Incident response

On receiving a cease-and-desist letter, DMCA notice, or other legal contact:

1. **Freeze** public releases and merges to the affected area.
2. **Preserve** everything (litigation hold). Do not rewrite or delete history.
3. Get counsel before replying. Keep a record of every contact with Daybreak or MEE.
4. Consider a DMCA counter-notice only for code that is demonstrably clean, with counsel.
5. The project does **not** continue operating in public defiance of a rights holder's
   demand.

Repositories should be mirrored (GitHub, Codeberg, a self-hosted Forgejo) so that one
platform's action does not destroy history, not to evade a valid demand.

---

## 15. Engagement strategy

Before shipping anything beyond T1 the project should approach the rights holders. For T4
this was a hard requirement (G2 of [§9.3](#93-the-gate)) until the owner set it aside on
2026-10-11 ([decision 0034](decisions/0034-official-server-compatibility.md)); it remains
the preferred route, above all before any public release of a build that can connect to
an official server:

- **Daybreak's General Counsel** with a Project 1999-style proposal: non-commercial,
  clean-room, no Daybreak code, users bring their own install, no marks, deference on
  timing.
- **Middle-earth Enterprises** separately, since MEE controls the marks and sent the
  2022 C&D to a LOTRO emulator.

Expect at best a revocable "preserve the peace" letter. Keep written records of every
interaction.

---

## 16. Open questions for counsel

These stay open. Decision 0034 (2026-10-11) proceeds with T4 without answers to them; they
remain the questions to put to counsel, in particular before any public release.

1. Is ToS §12 ("used as a stand-alone game … or … with non-Daybreak game servers") a
   licence-scope condition (copyright) or a covenant (contract) under *MDY*'s nexus test?
2. Does the THJ "server authentication is an access control" theory reach a from-scratch
   client and a project-owned server that never touch Daybreak's client or servers?
3. How does EU Art 6 "interoperability with other programs" apply to interoperability with
   data files or a fan server; and how does Art 6(3) "normal exploitation" apply to a live
   subscription game?
4. For T4: is any lawful route to official-server connectivity available without Daybreak's
   permission, and what exposure would users carry? Does authenticating with the user's own
   credentials avoid §1201 "circumvention" (G5), and does publishing a client that
   implements the official login count as trafficking? What changes when extended scripts
   run on an official server by the user's opt-in ([§9.5](#95-extended-scripts-and-official-servers))?
5. Is any of the data in the archives encrypted or keyed? If so, a §1201(f)/Art 6 analysis is
   needed before reading it. (Published readers report zlib compression only.)
6. What is MEE's position on an engine that contains no Tolkien content but exists to render
   it?
7. Does runtime loading of the full game database by a lawful user raise EU database-right
   issues?
8. Is exposing the plugin API's names and signatures ([§17](#17-plugin-api-names-interoperability-record))
   non-infringing in the EU/UK under *SAS Institute* and fair use in the US under *Google v.
   Oracle*, given that the host is part of a client for a live game? Could the organisation
   of the API inventory in our specs be a reproduction of the official reference as a work?
9. Do the marks inside the API names (`Turbine`, `Lotro`) need permission from their holders
   when used only as interoperability identifiers, and who holds `TURBINE` today?

---

## 17. Plugin API names (interoperability record)

Added 2026-10-06. It answers the "OPEN (legal)" item of
[ARCHITECTURE.md §18.2](ARCHITECTURE.md#plugins-host) and M5 exit criterion 6 in
[ROADMAP.md](ROADMAP.md). Sources for this section were read on 2026-10-06 unless noted.

### 17.1 What Tessera exposes, and why

- M5's goal is to run the **existing third-party UI plugins** that a user has installed. They
  are Lua 5.1 programs written by independent authors against the original client's plugin
  API. A plugin reaches the host only through names: global tables, classes, methods,
  events, fields, enum members, the `.plugin` descriptor's XML elements, and file and folder
  names such as `.plugindata` and `PluginData`. If one name differs, the plugin fails.
- The root of the API is the global table **`Turbine`**. Its native packages are `Turbine`,
  `Turbine.Gameplay`, `Turbine.Gameplay.Attributes`, `Turbine.UI` and **`Turbine.UI.Lotro`**
  ([lua-plugin-api.md](specs/lua-plugin-api.md) P7). These names contain two words on the
  [§10](#10-naming-and-trademark-rules) list: *Turbine*, the name of the studio that first
  developed the game, and *Lotro*, the abbreviation of MEE's game title. The studio has been
  part of Warner Bros. since 2010 and was renamed WB Games Boston in 2018 (Wikipedia, read
  2026-10-06). Any `TURBINE` mark may therefore belong to a **third rights holder**, distinct
  from Daybreak and MEE ([§2](#2-who-owns-what)).
- The full set of names, signatures, enums and event semantics is recorded as functional
  facts in [lua-plugin-api.md](specs/lua-plugin-api.md) and
  [plugin-descriptor.md](specs/plugin-descriptor.md). Each of them uses the names only
  because a plugin cannot run without them.

### 17.2 Rules (binding project policy)

1. **Interop use only.** The original names appear only where a plugin can observe them:
   Lua globals and tables, values passed to and from plugins, descriptor and plugin-data
   parsing, and paths inside the user's plugin tree. The specs and the plugin subsystem's
   tests may also use them. Crate, module, binary and Rust type names stay neutral, and the
   original names enter the code as string data in the plugin subsystem only.
2. **No branding.** The names are never a product name, feature name, logo, menu title or
   marketing term. The plugin manager shows each plugin's own name and author. The feature
   is described in plain text, for example "runs user-interface plugins written for the
   original client's Lua API", and the game is named only as [§10](#10-naming-and-trademark-rules)
   rule 3 allows. Lua error messages and stack traces may show the names, because they come
   from the plugin's own code.
3. **Names and signatures, not expression.** We reproduce identifiers, their nesting,
   parameter order and types, enum values and observable behaviour. Descriptions in specs,
   code comments, docs and error messages are our own words. No prose, examples, comments or
   page structure from the publisher's API reference is copied.
4. **Nothing of theirs is shipped.** Tessera ships no client Lua files, none of the
   publisher's 2010 sample packages (`Plugins/Turbine/`, `Plugins/TurbinePlugins/`), no
   third-party plugins, and no stubs, annotation files or dumps generated from the official
   reference. If any of these is in the user's plugin folder, Tessera reads it **in place,
   read-only**, like any other plugin ([§7](#7-what-these-facts-mean-for-project-design)).
   Plugin data is written under Tessera's own data directory, never into the client's
   folders.
5. **Only what plugins need.** The surface is limited to what existing plugins call, built
   in the tiers of [lua-plugin-api.md](specs/lua-plugin-api.md) §17. Gameplay namespaces
   are read-only views, and the sandbox gives plugins no capability the original API does
   not give them.
6. **Separable.** The compatible names live in the plugin subsystem only. The compatibility
   surface can be disabled or withdrawn without changing the rest of the engine, if counsel
   or a rights holder requires it ([§14](#14-incident-response)).
7. **Clean-room as usual.** Implementers work from the specs only
   ([CLEAN_ROOM.md](CLEAN_ROOM.md)). Spec facts that rest on disassembly (`BIN`, under
   [decision 0011](decisions/0011-standing-m4-authorisation.md)) are used only to make
   Tessera work with users' plugins (Art 6(2)(a)).

### 17.3 Copyright: EU and UK

| Authority | What it says | Effect here |
|---|---|---|
| Directive 2009/24/EC, **recital 10** | The parts of a program that provide "interconnection and interaction" with other software "are generally known as 'interfaces'". Interoperability "can be defined as the ability to exchange information and mutually to use the information which has been exchanged." | The plugin API is an interface in this sense. Its purpose is the exchange of calls and data between the client and independently written programs. |
| **Art 1(2)** and **recital 11** | Protection "shall apply to the expression in any form of a computer program. Ideas and principles which underlie any element of a computer program, including those which underlie its interfaces, are not protected." Recital 11: "to the extent that logic, algorithms and programming languages comprise ideas and principles", they are not protected. | The API's design, its calling conventions and its use as a programming environment are not protected as a program. The implementing code is, and we never copy it. |
| **Art 6(1)–(2)** and **recital 15** | Decompilation is allowed where it is "indispensable to obtain the information necessary to achieve the interoperability of an independently created computer program with other programs". The information may not be used for other goals, or "for the development, production or marketing of a computer program substantially similar in its expression". Recital 15: the aim is "to connect all components of a computer system, including those of different manufacturers, so that they can work together." | Running users' plugins is a plain case of interoperability "with other programs": the plugins are independent programs by third-party authors. Unlike the server question in [§5](#5-eu-and-uk-law), it does not depend on Daybreak's servers. Most names were already public in the publisher's reference, so Art 6 is needed only for undocumented behaviour (rule 7). |
| **Art 6(3)** | Must not "unreasonably prejudice the rightholder's legitimate interests" or conflict "with a normal exploitation of the computer program". | The plugin API is a free feature of the client and is not sold. The Art 6(3) question for a live game ([§5](#5-eu-and-uk-law)) is about the client as a whole, not about the names. |
| CJEU *SAS Institute v World Programming*, C-406/10, 2 May 2012 (Grand Chamber), ruling 1 and paras 39–46 | "neither the functionality of a computer program nor the programming language and the format of data files used in a computer program in order to exploit certain of its functions constitute a form of expression of that program". Para 42: they are "elements of that program by means of which users exploit certain functions of that program". Para 40: protecting functionality "would amount to making it possible to monopolise ideas". | The plugin language surface, its behaviour and the `.plugin` and `.plugindata` formats are functionality, language and data-file format in this sense. WPL, like us, had no source access and worked by observing and studying (paras 44, 61). |
| *SAS*, paras 66–67 and ruling 3 | "keywords, syntax, commands and combinations of commands, options, defaults and iterations consist of words, figures or mathematical concepts which, considered in isolation, are not, as such, an intellectual creation". Only "the choice, sequence and combination" of them can be, and copying elements of a user manual can infringe the manual if that reproduces the author's expression. | Supports the use of individual names. It is also why rule 3 bans the reference's prose and structure: the manual itself is protected. |
| *SAS*, para 45 | The finding "cannot affect the possibility that the SAS language and the format of SAS Institute's data files might be protected, as works, by copyright under Directive 2001/29 if they are their author's own intellectual creation". | Residual risk ([§17.6](#176-remaining-risk)). |
| *Navitaire v easyJet* [2004] EWHC 1725 (Ch) (Pumfrey J) | Para 79: "it is not possible to suggest that a copyright subsists in the individual command names as literary works." On the compilation point, the command set as a whole was "a computer language, not a program" and was not protected. | UK authority, decided before *SAS*, that individual command names are not protected. |

### 17.4 Copyright: United States

| Authority | Holding (summary) | Effect here |
|---|---|---|
| 17 U.S.C. §102(b) | No protection for any "idea, procedure, process, system, method of operation". | Names as operating instructions of an interface. |
| *Lotus v. Borland*, 49 F.3d 807 (1st Cir. 1995); affirmed by an equally divided Court, 516 U.S. 233 (1996) | The menu command hierarchy was an uncopyrightable "method of operation". Borland's "Key Reader" let users run their existing Lotus macros. The court objected that otherwise a user would have to rewrite a macro "despite the fact that the macro is clearly the user's own work product" (at 818). | The closest facts to ours: an independent program runs the user's existing scripts by honouring the original command words. An equally divided affirmance sets no national precedent. |
| *Oracle v. Google*, 750 F.3d 1339 (Fed. Cir. 2014) | Declaring code and the structure, sequence and organisation of the Java API were copyrightable. | The Supreme Court did not reverse this point. In the US, our position rests on fair use, not on non-copyrightability. |
| *Google LLC v. Oracle America, Inc.*, 593 U.S. 1 (2021) | Copyrightability was assumed "purely for argument's sake" (slip op. 15). Held: where Google "reimplemented a user interface, taking only what was needed to allow users to put their accrued talents to work in a new and transformative program", its copying of about 11,500 lines of declaring code "was a fair use of that material as a matter of law" (slip op. 35). The Court cited testimony that "If the API labels change, then either the software wouldn't continue to work anymore or the developer … would have to learn a whole new language" (slip op. 26). | Our use is narrower (names and signatures from functional specs, not copied declaring code), non-commercial, and serves users' existing programs. Fair use is decided case by case, and the fourth factor (market effect) is the weak point for a client of a live game ([§17.6](#176-remaining-risk)). |

### 17.5 Trademark

- The names are used as **functional identifiers** inside a runtime, not as a sign of
  Tessera's origin. They never appear in branding (rules 1–2), and the project is
  non-commercial ([§12](#12-distribution-and-money)), which weakens any "use in commerce" or
  "use in the course of trade" element.
- **EU:** Regulation (EU) 2017/1001 Art 14(1)(c) and Directive (EU) 2015/2436 Art 14(1)(c)
  do not let a proprietor prohibit use of the mark "for the purpose of identifying or
  referring to goods or services as those of the proprietor …, in particular, where the use
  … is necessary to indicate the intended purpose of a product or service", only "in
  accordance with honest practices in industrial or commercial matters" (Art 14(2)).
- **US:** nominative fair use, *New Kids on the Block v. News America Publishing*, 971 F.2d
  302 (9th Cir. 1992): the thing is not readily identifiable without the mark, only as much
  of the mark is used as is reasonably necessary, and nothing suggests sponsorship. *Sega v.
  Accolade*, 977 F.2d 1510 (9th Cir. 1992), Lanham Act part: Accolade had to include the
  "S-E-G-A" initialisation code for its games to run on the console. Because that code
  regulated access and no feasible alternative was known, "Sega is primarily responsible for
  any resultant confusion." The plugin names are a softer version of the same situation:
  plugins name the namespace, so any compatible host must accept it.
- The mandatory disclaimer ([§10](#10-naming-and-trademark-rules)) answers the "no implied
  sponsorship" element. The Warner Bros. connection of `Turbine` is not yet covered by the
  engagement plan ([§15](#15-engagement-strategy)); see [§16](#16-open-questions-for-counsel)
  question 9.

### 17.6 Remaining risk

- **This is not a safe harbour.** None of these cases concerned a third-party client for a
  live online game. Counsel must review this section before M5 ships publicly
  ([§16](#16-open-questions-for-counsel) questions 8–9).
- **US copyrightability is not settled.** The Federal Circuit's 2014 holding stands, and
  *Google v. Oracle* decided only fair use, on its facts. A court weighing market effect may
  look at the whole client as a substitute for the official one (the THJ reasoning,
  [§6.1](#61-daybreak-v-takahashi-the-heroes-journey-2025-26)), not at the plugin names alone.
- **The API as a work.** *SAS* para 45 leaves open that a language may be protected under
  Directive 2001/29, and paras 66–70 protect the manual's selection and arrangement. Our
  specs list the whole API inventory, which mirrors the reference's organisation. Mitigation:
  our own wording and order, no copied text, and implementing only what plugins need.
- **Contract is unchanged.** Interoperability does not cure ToS §12 ("stand-alone game") or
  ToS §7.1 ("software tools related to"), and EU Art 8 voids contract terms only to the
  extent of Arts 5(3) and 6 ([§3.3](#33-consequences), [§13](#13-contributor-risk-notice)).
- **Disassembly-based facts.** BIN facts rest on EU Art 6 and the owner's acceptance in
  [decision 0011](decisions/0011-standing-m4-authorisation.md). Outside the EU, and under
  EULA §4.2 for US contributors, they carry the exposure described in
  [§3](#3-contract-terms-that-bind-every-player-and-contributor).
- **Third-party documentation.** The MIT licence of community API tooling covers that
  project's own work, not the publisher's reference material in or behind it (the official
  reference is © 2016 the publisher). Never vendor those stubs, dumps or mirrored pages.
- **Plugins are their authors' works.** Tessera runs what the user has installed. It does not
  host, bundle, download or redistribute plugins.
- **Marks.** `Turbine` and `Lotro` stay inside the plugin subsystem. If a holder objects,
  [§14](#14-incident-response) applies and rule 6 lets the surface be withdrawn.

---

## 18. Extended scripting

Added 2026-10-07. It records the legal points of
[decision 0017](decisions/0017-extended-scripting.md) §12. In 0017, "§12" means the
record's own §12, never [§12](#12-distribution-and-money) of this document.

### 18.1 What it is

- Extended scripts are Lua programs written for **Tessera's own** scripting API, under one
  neutral global, `Ext`, loaded from their own folder in Tessera's data directory. The API
  is Tessera's design. It is separate from the plugin compatibility surface of
  [§17](#17-plugin-api-names-interoperability-record): plugins never see `Ext`, and
  extended scripts never see the plugin API's names.
- Nothing is allowed by default. Each power is a module handed out only for a capability
  that the user granted, and the effective rights are the intersection of the script's
  request, the user's grant, the backend and the server's policy.
- Third-party scripts are their authors' works. Tessera does not host, bundle, download or
  redistribute them, as [§17.6](#176-remaining-risk) says for plugins. A signature on a
  script proves authorship only.

### 18.2 Tiers and servers

- Offline use is **T1**. Use on Tessera's own protocol is **T2**, which [§8](#8-risk-tier-ladder)
  limits to the sandbox and the **project-owned** test server. Extended-script support on
  that protocol, its script policy and its defaults exist for the project-owned test
  server only. The project neither designs for, documents nor tests third-party operators.
- The client cannot verify who runs a server: a user can point it at any address. That is
  the threat-model limit of [§9.5](#95-extended-scripts-and-official-servers)
  item 4, not a supported use.
- **Third-party Tessera-protocol servers** are T3 territory ("any publicly hosted shard,
  free or not"). Supporting them, for extended scripts or for the client at all, needs its
  own decision, with the owner's and counsel's review, that amends
  [decision 0004](decisions/0004-server-strategy.md) and §8 first.
- **Official servers** (T4, [§9](#9-gates-and-the-official-server-target)). Since
  2026-10-11 ([decision 0034](decisions/0034-official-server-compatibility.md) §4) extended
  scripts may run on an official server only after the user's explicit opt-in, off by
  default, at the user's own risk; automation stays offline only
  ([§9.5](#95-extended-scripts-and-official-servers)). Until 2026-10-11 they never ran
  there. The opt-in adds the user's own ban risk to T4; it does not change §8's tiers.

### 18.3 Rules (binding project policy)

1. **No BIN facts in the extended-script contract.** `ext-api.md` defines the extended
   widgets, their input behaviour, events and drawing in its own terms, from Tessera's
   design and general GUI conventions. It cites no fact that rests on disassembly
   ([§17.2](#172-rules-binding-project-policy) rule 7) as part of that contract. The
   implementation may share code with native panels and plugin controls, but extended
   scripts may rely only on what `ext-api.md` states. Giving extended widgets parity with
   plugin controls would need an amendment of §17.2 reviewed by counsel.
2. **The plugin surface does not grow.** Extended scripts give plugins no capability the
   original API lacks ([§17.2](#172-rules-binding-project-policy) rule 5). The bridge to
   plugins is **one-way**: a script may list, load and unload plugins, read a named
   plugin's data and run a plugin's own command, each as the user's own action and
   rate-limited; it may not write plugin data or inject chat lines that plugins receive.
   A two-way bridge needs its own decision amending §17.2 rule 5. Withdrawing the plugin
   surface (§17.2 rule 6) leaves extended scripts working.
3. **Never offered.** No module, slice or grant provides: compiling strings (`load`,
   `loadstring` or any equivalent), binary chunks or bytecode; file paths outside the
   script's folder and its own store, other than the user's file-picker dialog; sockets,
   HTTP, processes, native code or FFI; write access to the install, raw archive bytes,
   enumeration by DID range or export of decoded assets; the state of other scripts or of
   plugins (except the bridge of rule 2) and the content of UI the script does not own;
   synthesised raw input or a clipboard API; the user's OS identity (user name, home path,
   host name); custom shaders; code delivered by a server.
4. **Neutral names.** `Ext`, the module names, the crates, the script tool, the folders and
   every UI string are neutral ([§10](#10-naming-and-trademark-rules)). The plugin API's
   interoperability names stay inside the plugin subsystem
   ([§17.2](#172-rules-binding-project-policy) rules 1 and 6).
5. **No game content.** Scripts refer to DIDs that resolve in the user's install. Examples
   embed no extracted data, names or strings. The packing tool refuses `.dat`/`.datx` files
   and archive-headed files and warns that extracted game assets must not be packed
   ([§11](#11-content-rules)). Nothing in Tessera advertises scripts as extraction tools.
6. **Automation** (`input.drive`) runs offline only, where it harms no one
   ([§9.5](#95-extended-scripts-and-official-servers) item 3), and is never enabled on an
   official server, whatever the user's opt-in. The
   plugin API's anti-automation boundary stays as it is for plugins.

### 18.4 Decoded data and export

This is the [§11](#11-content-rules) assessment of the `data.read` capability, which lets a
script read decoded values of records from the user's install.

- **Personal use.** It is a viewer of the user's own install inside the user's own
  client, like the explorer, and it is opt-in per script. It gives no raw bytes and no
  enumeration by DID range, and it writes nothing.
- **No export route, by design rather than by statement:**
  - `data.read` is offered **offline only**;
  - it is **local-only**: no script holds it together with the server-channel capability
    (`session.server`), on any backend;
  - a script's store written while it held a local-only capability is **wiped** before the
    script opens it with the server channel, and every file operation on that store runs
    in order on one queue, so no late write lands after the wipe;
  - no hand-off of running state crosses a change of grants;
  - the plugin bridge, the one way to put data into plugin data, is local-only too;
  - no other module reaches the network.
- **File output.** The only files a script can produce are its own store (capped at
  16 MiB) and its script log (whatever it prints, decoded values included; capped at 2 MiB
  per script), both in Tessera's directories on the user's machine.
- **The stated limit.** No single script can send decoded data off the machine on its own.
  Programs that the user installs and runs together could still pass data to each other,
  and one of them could later send it to a server:
  - two or more extended scripts, through shared engine state (for example the game
    clock), through shared pools that answer "busy" to one script according to what others
    use, and **across sessions** through shared quotas that survive a restart (the total
    store space answering "full");
  - a script and a plugin, through a bridged command whose arguments the plugin keeps or
    turns into a quickslot that the user later clicks on a server, through plugin data,
    which persists by design, or through frame timing and window layout.

  That is a covert channel between programs the user chose to run, comparable to any
  other software on their computer. Tessera neither offers nor documents it.

### 18.5 The layout residual

Extended windows lie below plugin windows and native panels, and receive the pointer only
where nothing above catches it. A large, even transparent, extended window that receives
pointer notifications therefore learns **where** plugin windows and native panels catch
the pointer: their positions and mouse-visible shapes, **never their content**. Pointer
moves reach a window only over its own mouse-visible elements, at most once per frame. It
is a channel of layout only.

### 18.6 Remaining risk

- **This section rests on Tessera's builds.** Its guarantees hold for the builds the
  project publishes. A modified build is outside them
  ([§9.5](#95-extended-scripts-and-official-servers) item 4).
- **Open security items.** Decision 0017 lists security and design items that are still
  open and must be closed by the M5.5 threat model and its escape tests before the first
  slice ships. Two bear on this section: the actions that `input.drive` may press must be
  limited to an explicit list, because a driven screenshot would write image files outside
  the file output of §18.4; and two further channels between extended scripts (a shared
  decode cache, and the layout residual of §18.5 between scripts) must be closed or added
  to the stated limit of §18.4. This section is updated with the result.
- **Contract is unchanged.** Extended scripting does not cure ToS §12 or ToS §7.1
  ([§3.3](#33-consequences)). The account and server risks of [§9.2](#92-why-it-is-the-highest-risk-tier)
  are why §9.5 keeps extended scripts off official servers by default, asks each user to
  opt in, and keeps automation offline.

---

## 19. Sources

Read 2026-10-03 unless noted. The project's research notes hold the detailed citations.

- Daybreak EULA, "Last Updated on April 24, 2026" — daybreakgames.com/eula
- Daybreak Terms of Service, "Last Updated: July 13, 2026" — daybreakgames.com/terms-of-service
- lotro.com footer (ownership statements)
- *Daybreak Game Co. v. Takahashi*, S.D. Cal. 3:25-cv-01489: ECF 66 (PI, 2025-09-19), ECF 84
  (2025-10-30), ECF 104-1 (consent judgment, 2026-03)
- Project 1999 letter agreement, 2015-04-23 (exhibit in the THJ docket)
- *Davidson & Assocs. v. Jung*, 422 F.3d 630 (8th Cir. 2005)
- 17 U.S.C. §1201; 37 CFR 201.40
- Directive 2009/24/EC Arts 5, 6, 8; UK CDPA ss.50B, 50BA, 296A (legislation.gov.uk)
- github/dmca repository (searched 2026-10-03)
- Cases marked as summaries above (*Sega*, *Connectix*, *Galoob*, *Micro Star*, *Vernor*,
  *Top System*) were not re-read from primary sources for this document; verify before
  relying on them.

Read 2026-10-06 for [§17](#17-plugin-api-names-interoperability-record):

- Directive 2009/24/EC, recitals 10, 11, 15 and Arts 1, 5, 6, 8 — legislation.gov.uk
  (`eudr/2009/24`, the version as adopted)
- CJEU C-406/10 *SAS Institute Inc. v World Programming Ltd*, judgment of 2 May 2012 (Grand
  Chamber), EN text from the EU Publications Office (CELEX 62010CJ0406)
- *Google LLC v. Oracle America, Inc.*, 593 U.S. 1 (2021), slip opinion No. 18-956 —
  supremecourt.gov
- *Lotus Development Corp. v. Borland International, Inc.*, 49 F.3d 807 (1st Cir. 1995) —
  digital-law-online.info (34 USPQ2d 1014)
- *Navitaire Inc v easyJet Airline Co* [2004] EWHC 1725 (Ch) — caselaw.nationalarchives.gov.uk
- *Sega Enterprises Ltd. v. Accolade, Inc.*, 977 F.2d 1510 (9th Cir. 1992), Lanham Act part —
  law.resource.org
- Regulation (EU) 2017/1001 Art 14 (legislation.gov.uk, as adopted); Directive (EU)
  2015/2436 Art 14 (EU Publications Office, CELEX 32015L2436)
- Wikipedia, "WB Games Boston" (studio history)
- Summaries only, not re-read for this document: *Lotus v. Borland*, 516 U.S. 233 (1996);
  *Oracle America, Inc. v. Google Inc.*, 750 F.3d 1339 (Fed. Cir. 2014); *New Kids on the
  Block v. News America Publishing*, 971 F.2d 302 (9th Cir. 1992).
