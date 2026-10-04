<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Legal position and project policy

> **NOT LEGAL ADVICE.** This document is engineering research written by
> non-lawyers so that the project can make careful decisions. It is not a legal opinion,
> it may be wrong, and the law changes. Anyone who contributes, distributes or runs this
> software is responsible for their own legal position. **Before any public release, and
> before any networked feature beyond the local sandbox, the project must obtain review by
> a qualified IP lawyer** (see [§9](#9-gates-and-the-official-server-target)).

Status: adopted 2026-10-03. Sources were fetched and read on 2026-10-03 unless another
date is given. Quoted contract text is from the versions dated below; Daybreak can change
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
   project-owned server → emulated gameplay → connectivity with official servers.
4. The project owner has chosen **official-server compatibility** as the long-term target.
   Research rates that the **highest-risk tier**. It is therefore **gated**: it is not in the
   base, no code for it may land until the gate in [§9](#9-gates-and-the-official-server-target)
   is passed, and until then development targets the in-process sandbox and a
   project-owned test server.
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
| *Google v. Oracle*, 593 U.S. 1 (2021) | Reimplementing API declarations for interoperability was fair use. | Supports a compatible Lua plugin API surface. Never copy the original `.lua` files or documentation prose. |
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
4. No Daybreak/SSG hostnames, keys or tables extracted from the client binary are
   hard-coded or committed. Anything of that kind is loaded at runtime from the user's own
   install, and only where a passed gate allows it.
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
| **T4 Official-server connectivity** | Client connects to Daybreak's live servers: official login/launcher protocol, session encryption, the live game protocol. | **Highest.** EULA §4.3, §4.4, §4.5; ToS §7.1, §10, §12; DMCA §1201 (THJ theory); account bans for every user. | **Owner's long-term target. Gated; see §9.** |

**Permanently forbidden at every tier, with or without the gate:**

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

### 9.1 The decision

The project owner has chosen **official-server compatibility** (T4) as the long-term
target: a client that, like the official one, could log in to and play on Daybreak's servers.
This document records that decision and the conditions attached to it.

### 9.2 Why it is the highest-risk tier

Research rates T4 as the riskiest thing this project could do. Plainly:

- **Contract.** EULA §4.3 forbids providing any means of playing "through interception,
  emulation or redirection of the communication protocols"; §4.4 forbids "any unauthorized
  connection"; §4.5 forbids decrypting client–server data. ToS §7.1 forbids deciphering
  transmissions and circumventing access measures; ToS §10 repeats the decryption ban; ToS
  §12 restricts the client software. A third-party client on the live service breaches these
  on its face unless Daybreak authorises it.
- **DMCA §1201.** The official login uses encryption and key material that ships inside
  the official client (reported by published third-party research). The THJ court treated
  server-side authentication as an access control and the act of distributing a
  workaround as trafficking. Implementing the official login in third-party code risks
  §1201(a)(1) circumvention and (a)(2) trafficking claims, and in the 9th Circuit those
  claims do not need any copyright infringement. §1201(f) is narrow, bnetd held it waived by
  contract, and its element (4) fails if any infringement is involved.
- **Account bans for every user.** EULA §5 allows detection of "Unauthorized Third Party
  Programs", and ToS §26.2 allows termination "at any time and for any reason". Every person
  who logs in with this client would be risking their account, purchases and characters.
- **Service integrity.** A third-party client on a live service can be treated as cheating
  or interference (ToS §7.1, §10) even if it does nothing unusual.
- **EU law helps less than it seems.** Art 6 covers *obtaining* interface information. It
  does not license connecting to a service against its terms, and Art 6(3) protects the
  rightholder's "normal exploitation".

### 9.3 The gate

T4 work is **not implemented in the base**, and no code, spec or test for it may be merged
until **all** of these are done and recorded in `docs/decisions/`:

| # | Condition |
|---|---|
| G1 | A **written legal review** by a qualified IP lawyer, covering at least US and EU/UK law, of the specific design to be built. |
| G2 | **Written permission from Daybreak Game Company LLC** is strongly preferred (cf. the 2015 Project 1999 letter; the 2024 Homecoming licence). **Middle-earth Enterprises** must be approached separately for the marks. If permission is refused or unanswered, G1 must say explicitly whether proceeding is advisable at all. |
| G3 | A maintainer decision record that cites G1 and G2, lists the residual risks, and is accepted by the project owner. |
| G4 | A user-facing warning, shown before any official connection is possible, that states the account-ban risk and the terms in [§3](#3-contract-terms-that-bind-every-player-and-contributor). |
| G5 | The design still obeys the "permanently forbidden" list in [§8](#8-risk-tier-ladder): no modified official client, no shipped keys, no bypassed authentication. |

Before the gate passes:

- **Research is limited.** Dirty-room specs about the official login, its encryption or
  key handling must not be written. Specs describing *observed* game-protocol structure may
  be written only if a maintainer approves them as needed for T2 design, and they must be
  marked as such.
- **Code is limited.** `crates/net` provides a transport-agnostic session layer and a local
  sandbox backend only. It contains no Daybreak hostnames, ports, keys, login flows or
  official protocol handling, and the default build cannot reach official servers.
- **Development targets** the in-process sandbox and a project-owned test server that uses a
  project-defined protocol and serves only original or synthetic content.

### 9.4 How the gate can fail

If G1 advises against T4, or Daybreak or MEE object, the project stays at T2 permanently and
this section is updated to say so. The project will not proceed against an explicit
objection from either rights holder (see [§14](#14-incident-response)).

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
5. **Mandatory disclaimer.** This exact text must appear in the README, on any website, in
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
- keys, certificates, hostnames or tables extracted from the client binary;
- packet captures or logs of traffic with official servers;
- decompiler or disassembler output, symbol maps, shader bytecode.

Allowed:

- **synthetic test fixtures** generated by our own code, and original content released as
  CC0 or under a licence compatible with GPL-3.0-or-later;
- **real-data tests** that read `TESSERA_GAME_DIR` (or an auto-detected install), are skipped
  when no install is present, and **never write to the install directory**;
- screenshots in bug reports only as ToS §14.1 permits (non-commercial, no logos, no implied
  endorsement), kept to what is needed.

Exporters and viewers are for personal use, are opt-in, and are never advertised as bulk
extraction or ripping tools. The engine must run with original placeholder content so it
has substantial uses that do not depend on game data.

---

## 12. Distribution and money

- Licence: **GPL-3.0-or-later**; every source file carries
  <!-- REUSE-IgnoreStart -->`SPDX-License-Identifier: GPL-3.0-or-later`<!-- REUSE-IgnoreEnd -->. Third-party code may be imported only if its
  licence is compatible and its provenance is clean. Most published LOTRO tools are
  unlicensed (all rights reserved) and must not be copied; AGPL code is not imported.
- Free of charge. No paid tiers, perks, ads, or sales. Donations, if any, cover
  infrastructure costs only (the Project 1999 model).
- No hosted public game servers.
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

Before shipping anything beyond T1, and as a hard requirement for T4, the project should
approach:

- **Daybreak's General Counsel** with a Project 1999-style proposal: non-commercial,
  clean-room, no Daybreak code, users bring their own install, no marks, deference on
  timing.
- **Middle-earth Enterprises** separately, since MEE controls the marks and sent the
  2022 C&D to a LOTRO emulator.

Expect at best a revocable "preserve the peace" letter. Keep written records of every
interaction.

---

## 16. Open questions for counsel

1. Is ToS §12 ("used as a stand-alone game … or … with non-Daybreak game servers") a
   licence-scope condition (copyright) or a covenant (contract) under *MDY*'s nexus test?
2. Does the THJ "server authentication is an access control" theory reach a from-scratch
   client and a project-owned server that never touch Daybreak's client or servers?
3. How does EU Art 6 "interoperability with other programs" apply to interoperability with
   data files or a fan server; and how does Art 6(3) "normal exploitation" apply to a live
   subscription game?
4. For T4: is any lawful route to official-server connectivity available without Daybreak's
   permission, and what exposure would users carry?
5. Is any of the data in the archives encrypted or keyed? If so, a §1201(f)/Art 6 analysis is
   needed before reading it. (Published readers report zlib compression only.)
6. What is MEE's position on an engine that contains no Tolkien content but exists to render
   it?
7. Does runtime loading of the full game database by a lawful user raise EU database-right
   issues?

---

## 17. Sources

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
- Cases marked as summaries above (*Sega*, *Connectix*, *Galoob*, *Micro Star*, *Google v.
  Oracle*, *Vernor*, *SAS Institute*, *Top System*) were not re-read from primary sources for
  this document; verify before relying on them.
