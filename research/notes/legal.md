<!-- SPDX-License-Identifier: GPL-3.0-or-later -->


# Legal framework for a clean-room LOTRO client reimplementation (research notes, 2026-10-03)

NOT LEGAL ADVICE. This is engineering-oriented research. Get a real IP lawyer before going public, especially before any networked or server component ships.

Raw sources I downloaded are in the scratchpad: `$TESSERA_RE_DIR/legal/`. Files: `dbg_eula.txt`, `dbg_tos.txt` (current Daybreak EULA/ToS as text), `thj66.txt` (THJ preliminary-injunction order), `thj104.txt` (THJ consent judgment), `p99.txt` (Project 1999 letter agreement), `bnetd.txt` (8th Cir. opinion), `eu_a5/a6/a8.html` (Directive 2009/24 articles), `uk_50B/50BA/296A.html`, `mer_inv.txt` (MER LOTRO protocol notes). The GitHub DMCA repo clone is in `~/.cache/tessera/clones/dmca`.

## 0. Bottom line

1. **No path is risk-free while LOTRO is a live, paid game.** In 2025–26 the rightsholder litigated against an emulator and won: Daybreak v. Takahashi ("The Heroes' Journey", EverQuest), S.D. Cal. 3:25-cv-01489. Daybreak got a preliminary injunction on copyright and DMCA §1201 grounds, then a $3.5M consent judgment. Middle-earth Enterprises (MEE) sent a C&D to the LOTRO rogue server Echoes of Angmar in 2022. Blizzard ended Turtle WoW in 2026.
2. **The OpenMW/ScummVM model is the most defensible form.** That means: an engine written from scratch, the user's own data files read in place, no assets shipped, no connection to official servers, non-commercial, and a neutral name. It still runs into the EULA/ToS (contract) and some untested §1201 and "volitional copying" theories. The THJ court treated "telling users to copy their client files into a new folder for use as assets" as instigating infringement.
3. **Risk climbs steeply by scope:**
   - format libraries and viewers: lowest
   - offline engine or world explorer
   - networked client talking to a project-owned server
   - full gameplay emulation of a live game: this is where every lawsuit happened
   - talking to official servers or shipping client files: forbidden, do not do
4. **The only robust "legal" end state is permission.** It would have to come from Daybreak (copyright and code) AND MEE (Tolkien/LOTRO marks and story IP). Precedents: Daybreak's 2015 Project 1999 letter agreement, which I have verbatim. NCSoft's January 2024 license to City of Heroes: Homecoming. Bethesda's informal OK for OpenMW.

## 1. Who owns what (verified from the lotro.com footer, 2026-10-03)

- "THE LORD OF THE RINGS ONLINE™ interactive video game © 2026 Daybreak Game Company LLC."
- "Standing Stone Games and the Standing Stone Games logo are trademarks or registered trademarks of Daybreak Game Company LLC."
- "The Lord of the Rings Online, Shadows of Angmar, Mines of Moria, Siege of Mirkwood, Rise of Isengard, Riders of Rohan, Helm's Deep, Mithril, Middle-earth Enterprises logo, and The Lord of the Rings and the characters, items, events, and places therein are trademarks of Middle-earth Enterprises, LLC used under license."
- So the "LOTRO"/"Lord of the Rings Online" marks belong to MEE (Embracer Group since 2022), not SSG. **"Mithril" is listed as a mark.**
- lotro.com's legal links all point to Daybreak's documents: daybreakgames.com/eula, /terms-of-service, /conduct-policy, /legal. There is no separate SSG ToS any more; standingstonegames.com/legal redirects to a Daybreak maintenance page.
- Daybreak registers client and server code with the US Copyright Office. Example from the THJ judgment: "EverQuest: Rain of Fear (Client-Side Code 2012) PA0002533846", "Server-Side Code PA0002533847". Expect LOTRO client code to be registered too, which enables statutory damages.
- Tolkien Estate: literary copyright in the books. MEE: film, game and merchandise rights plus trademarks. MEE enforces aggressively, e.g. "Lord of the Wings" opposition, the "Lord of the Bins" C&D, the "Lord of the Fries" dispute (2025), the "Hobbit" wine dispute, and the 2022 EoA C&D.
- Asheron's Call (precedent only) stayed with WB/Turbine. The USPTO registration 2279820 was cancelled under §8 in 2006, but WB still sent C&Ds to emulators in September 2017.

## 2. Current contract terms that bind every LOTRO player/contributor (fetched 2026-10-03)

### Daybreak EULA ("Last Updated on April 24, 2026"), covering LOTRO

**§2 License:** the license is "solely for your personal, non-commercial use … solely in connection with playing Games via an authorized Account".

**§4 says you shall not, without written permission:**
- **4.1** "copy (except to make one necessary back-up copy), distribute, … modify or create derivative works … of the Software"
- **4.2** "reverse engineer, disassemble, decompile or otherwise reduce the Software … to a human-readable form or attempt to reconstruct or discover any source code, underlying ideas, algorithms, **file formats** or programming interfaces of the Software … by any means whatsoever, **except to the extent that this restriction is expressly prohibited by applicable law**"
- **4.3** "create, facilitate, host, link to or provide any other means through which any Game(s) may be played by others through interception, **emulation** or redirection of the communication protocols used by Daybreak, including … protocol emulation, tunneling, data mining, packet sniffing, modifying or adding components …"
- **4.4** "create, facilitate, or maintain any unauthorized connection to any Game(s)"
- **4.5** "mine, decrypt, or modify any data transmitted between any client and server"
- **4.6** forbids cheats, hacks, mods, macros and bots
- **4.7** "incorporate the Software … including … Daybreak Game Assets … into any product or service"

**Other EULA terms:**
- §5 consents to anti-cheat monitoring for "Unauthorized Third Party Program[s]".
- §11 sets Delaware law, with venue in Delaware or San Diego County.
- §12 requires mandatory AAA individual arbitration and a class waiver. It carves out court injunctions for IP infringement.

### Daybreak Terms of Service ("Last Updated: July 13, 2026")

**§7.1 Rules of Conduct says you must not:**
- "Attempt to play any Daybreak Game(s) on any server that is not controlled or authorized by Daybreak"
- "**Create, use or provide any server emulator** or other site where any Daybreak Game(s) may be played, and to refrain from **posting or distributing any utilities, emulators or other software tools related to any Daybreak Game(s)** without the express written permission of Daybreak"
- "Modify any part of any Daybreak Game(s), including … client and/or server"
- "Attempt to interfere with, hack into, or decipher any transmissions"

**Other ToS sections:**
- §10 Cheating: "You may not disassemble, reverse engineer, or modify any Daybreak Game(s) software … You may not decrypt or modify any data transmitted …"
- §12 License: "The client-side software may not be … disassembled, reverse engineered, modified, **used as a stand-alone game (without connecting to Daybreak's game servers), or used in connection with non-Daybreak game servers**." Breach auto-terminates the license.
- §13 Restrictions: no RE, decompiling or derivative works, and no incorporating a game "into any product or service".
- §14.4 UGC: you may not "use Daybreak's name, trademarks, and/or the name of any Daybreak Game(s) in any Fan Site name, domain name or App name", nor marks "confusingly similar". UGC must not imply endorsement.
- §25 Arbitration, with an opt-out procedure.

**Contract consequences**
- In the US, anti-RE clauses in click/shrink-wrap licenses are enforceable and not preempted. Authorities: Bowers v. Baystate, 320 F.3d 1317 (Fed. Cir. 2003); Davidson & Assocs. v. Jung (bnetd), 422 F.3d 630 (8th Cir. 2005).
- bnetd held: "Appellants contractually accepted restrictions on their ability to reverse engineer by their agreement to the terms of the TOU and EULA. '[P]rivate parties are free to contractually forego the limited ability to reverse engineer…'"
- So any contributor who installed LOTRO is personally exposed to breach-of-contract claims for RE work, for distributing "software tools related to" LOTRO, and (as an end user) for running the dats on a non-Daybreak engine or server.
- The THJ consent judgment also used EULA acceptance to establish personal jurisdiction in S.D. Cal.
- THJ shows the arbitration clause cuts both ways. Defendants successfully compelled arbitration (ECF 84, 2025-10-30), but the court kept the IP injunction in place.

**SSG informal tolerance (not a license)**
- On 2020-11-10, SSG community manager Cordovan wrote on the old forums, about an FOV/injection tool: "We cannot provide legally-binding clearance to a thing to forever be ok to develop as an add-on, so I won't be doing that. I will say, however, that using something like this as an accessibility tool … is not something we would be likely to be concerned about. Now, if others use it to cheat, then I guess we'd have to worry about it." It is archived on the Wayback Machine (link in the PalantirTools README).
- Dat-reading companion tools (LotroCompanion, announced on official lotro.com forums), the GPL OneLauncher launcher, and FOV/freecam tools have existed for years without enforcement I could find.
- **The github/dmca repo contains no notices from Daybreak/SSG/WB/MEE about LOTRO** (grep of the full repo, 2026-10-03). It does contain:
  - **2016-09-14 Daybreak notice (Free Realms):** it targeted a repo with Joshsora's *reverse-engineered server code, utilities AND "packet logs"*, citing copyright, trademark, the EULA and tortious interference.
  - **2026-09-09 Daybreak notice:** it targeted EverQuest RoF2 client files (eqgame.exe, S3D, DLLs) in `Aishrod/eqemupatcher`. It cites the EULA's ban on "any means through which the Game may be played by others, such as through server emulators."
  - GitHub's note on the 2026 notice: it "did not find sufficient information to determine a valid anti-circumvention claim" but processed the copyright part.

## 3. US law

### Copyright: reverse engineering and intermediate copying

**Sega v. Accolade, 977 F.2d 1510 (9th Cir. 1992)**
- Disassembly as "intermediate copying" is fair use "where such disassembly and copying is the only way to gain access to the functional elements … and where there is a legitimate reason."
- Important limit from the THJ order (2025): Sega did not help THJ because THJ "copy[ied] and exploit[ed] Daybreak's expressive audiovisual assets" and Accolade did not "avoid performing its own creative work."

**Sony v. Connectix, 203 F.3d 596 (9th Cir. 2000)**
- Repeated intermediate copies of the PS BIOS made while building a from-scratch emulator (VGS) were fair use. The final product contained no Sony code, and the injunction was dissolved.

**Galoob v. Nintendo, 964 F.2d 965 (9th Cir. 1992) and Micro Star v. FormGen, 154 F.3d 1107 (9th Cir. 1998)** (from memory)
- Galoob: a tool that alters the display without incorporating the work in concrete form was not a derivative work.
- Micro Star: distributing new level files that describe protected audiovisual expression was a derivative work.
- Lesson: the engine is fine. Shipping content files that describe LOTRO's world (maps, quest text, item databases) is not.

**Google v. Oracle, 593 U.S. 1 (2021)** (from memory)
- Reimplementing API declarations for interoperability was fair use.
- Relevant for reimplementing the LOTRO Lua plugin API surface (Turbine.UI.* names) so existing user plugins run. Do not copy Turbine's .lua files or the API documentation prose.

**17 U.S.C. §102(b) and the license-scope cases**
- §102(b): ideas, procedures, methods of operation and formats are not protected.
- MDY v. Blizzard, 629 F.3d 928 (9th Cir. 2010, amended 2011): breaching a ToU term is copyright infringement only if the term is a *condition* limiting license scope with a "nexus" to an exclusive right. Otherwise it is only contract.
- Vernor v. Autodesk (9th Cir. 2010): licensees are not "owners", so §117 essential-step copying may not apply (from memory).
- Risk: Daybreak ToS §12 ("not used … with non-Daybreak game servers") could be argued to be a scope condition. If so, end users' RAM copies of dat content loaded by our engine for emulated play could count as infringement, and the project as contributory or inducing. **This is unresolved.**

**THJ volitional-conduct holding (PI order, ECF 66, 2025-09-19)**
- Requiring users to "copy the physical intellectual property assets … in their Steam content folder … and paste a copy of the entirety of the games' client-side [IP] into the newly-created folder THJ" was "instigating third-party infringement".
- Defendants' modifications counted: 234 files altered, 4,215 inserted, 66 deleted.
- → **Never copy, repack, patch or modify user data files. Read them in place, read-only.**

### DMCA §1201

**§1201(f) verbatim:**
- (f)(1): "a person who has lawfully obtained the right to use a copy of a computer program may circumvent a technological measure that effectively controls access to a particular portion of that program for the sole purpose of identifying and analyzing those elements of the program that are necessary to achieve interoperability of an independently created computer program with other programs, and that have not previously been readily available to the person engaging in the circumvention, to the extent any such acts of identification and analysis do not constitute infringement under this title."
- (f)(2) allows developing means "if such means are necessary to achieve such interoperability".
- (f)(3) allows sharing the information "solely for the purpose of enabling interoperability … to the extent that doing so does not constitute infringement … or violate applicable law other than this section".
- (f)(4): "interoperability" means "the ability of computer programs to exchange information, and of such programs mutually to use the information which has been exchanged."

**bnetd (8th Cir.) elements test for (f):**
1. lawful right to use;
2. information not previously readily available;
3. sole purpose of interoperability;
4. no infringement.
- bnetd failed element 4 because its server did not check CD keys, so "unauthorized copies of the Blizzard games were freely played."
- → **Do not support pirated or unofficial installations, and do not link to client mirrors.**

**MDY v. Blizzard on §1201**
- §1201(a)(2) needs no infringement nexus in the 9th Circuit. This rejects Chamberlain v. Skylink (Fed. Cir. 2004), which required one.
- Warden, by controlling server access, protected WoW's "dynamic non-literal elements" (the audiovisual experience generated during server-connected play).

**THJ PI order on §1201(a)(1)/(a)(2)**
- Server-side account verification, subscription validation and ongoing authorization are TPMs that "effectively control access."
- Editing eqhost.txt to redirect the official client to a fan server, and bypassing login in eqlsClient.ini, was likely circumvention.
- The court rejected "we never touch Daybreak's servers": it was "skeptical of a reading of the statute that would impose a requirement of attempted or direct interaction with the copyright holder's servers."
- Distributing THJ counted as trafficking (§1201(a)(2)).
- **This is the central theory a networked LOTRO reimplementation would face.** It is only a PI ruling, but it is the most recent and the closest on point.
- Distinguishing facts available to us:
  - we would not redirect or modify Daybreak's client;
  - dats are readable without keys (they are zlib-compressed, not encrypted, per open-source readers such as jtauber/lotro);
  - no Daybreak auth is ever invoked or bypassed.

**Other §1201 enforcement** (verified via search):
- Nintendo v. Tropic Haze (Yuzu), March 2024: $2.4M settlement and a permanent injunction under the circumvention and trafficking theory (Switch keys).
- Blizzard v. Bossland: about $8.5M, 2017, cheat bots that evaded Warden.
- Blizzard v. Reeves/Scapegaming: $88M default judgment, 2010, for a paid WoW private server.
- → **Never ship, embed or document how to obtain any Daybreak secret.** That includes the official server RSA public key + any client-side key material, launcher/GLS ticket handling for official servers, and patcher authentication.

**Copyright Office 2024 triennial (37 CFR 201.40)**
- The video-game preservation exemption covers only eligible libraries, archives and museums, and only for games whose server support has been discontinued.
- Remote/off-premises access was refused in October 2024.
- None of this helps a live game like LOTRO.

### Trademark (US)

- **Nominative fair use** (New Kids on the Block, 9th Cir. 1992; Toyota v. Tabari, 2010; from memory) permits plain-text descriptive reference, e.g. "Requires a legally obtained installation of The Lord of the Rings Online™". That allowance is limited to plain-text factual reference: no logos or stylization, and no implied endorsement.
- The P99 letter shows Daybreak's own preferred line: use the game name only "descriptively in plain text (without a logo or stylization)" and add a disclaimer of non-affiliation.

## 4. EU / UK law (stronger RE rights, mandatory)

**Directive 2009/24/EC (verbatim, fetched from legislation.gov.uk):**
- **Art 5(1):** lawful-acquirer acts "necessary for the use … in accordance with its intended purpose, including for error correction". Contract can override it.
- **Art 5(3):** "The person having a right to use a copy of a computer program shall be entitled, without the authorisation of the rightholder, to observe, study or test the functioning of the program in order to determine the ideas and principles which underlie any element of the program if he does so while performing any of the acts of loading, displaying, running, transmitting or storing the program which he is entitled to do." This is black-box study, including watching your own network traffic while running the game.
- **Art 6(1):** decompilation without authorisation is allowed where "indispensable to obtain the information necessary to achieve the interoperability of an independently created computer program with other programs", if:
  - (a) it is done by the licensee or another person with a right to use;
  - (b) the information is not previously readily available;
  - (c) it is "confined to the parts of the original program which are necessary".
- **Art 6(2):** the information must not be:
  - (a) used for other goals;
  - (b) given to others except when necessary for interoperability;
  - (c) "used for the development, production or marketing of a computer program **substantially similar in its expression**".
- **Art 6(3):** Berne three-step test. No use that "unreasonably prejudices the rightholder's legitimate interests or conflicts with a normal exploitation".
- **Art 8:** "Any contractual provisions contrary to Article 6 or to the exceptions provided for in Article 5(2) and (3) shall be null and void." So EULA §4.2 is void in the EU as far as it conflicts with Art 5(3)/6. Delaware choice of law does not strip EU consumers of mandatory protections (Rome I Art 6; from memory).

**CJEU cases:**
- **SAS Institute v WPL, C-406/10 (2012):** "the functionality, programming language and data file format of a computer program would not constitute forms of expression" and are not protected. Art 5(3) study cannot be contracted away. This strongly supports reimplementing the dat formats and the behaviour.
- **Top System v Belgian State, C-13/20 (2021-10-06):** a lawful acquirer may decompile to correct errors under Art 5(1), but only as far as necessary and subject to contract.
- **Nintendo v PC Box, C-355/12 (2014)** (from memory): TPM protection must be proportionate and consider other legitimate uses.
- **Database right, Directive 96/9/EC** (from memory): extracting or reusing substantial parts of a game's database (item/quest tables) can infringe. This is one more reason never to ship extracted data.

**UK CDPA** (verbatim fetched; mirrors the Directive):
- s.50B decompilation;
- s.50BA observe/study/test;
- s.296A voids contrary contract terms.
- Germany: §§69d(3), 69e UrhG (from memory).

**Practical implication.** If any binary analysis is ever done, do it in the EU/UK by a lawful licensee, confined to interop, and output only prose specifications. US contributors who accepted the EULA face Bowers/Davidson contract exposure even where fair use would cover the copying. EU-only analysts should still treat the "interoperability with other programs" point as open. Interop with official servers is clearly within Art 6. Interop with a fan server or just the data files is less clear.

## 5. Precedents and how they stayed or didn't stay legal

**The Heroes' Journey (EverQuest, 2025–26)**

Docket (CourtListener, S.D. Cal. 3:25-cv-01489, Judge Bashant):
- 2025-06-14: complaint. Claims were copyright, DMCA, Lanham Act, UCL and breach.
- 2025-09-19: PI granted (ECF 66).
- 2025-10-30: arbitration compelled; stay of PI denied (ECF 84).
- 2025-12-31: alternative service on another admin (ECF 99).
- 2026-01-30: arbitration status letter.
- 2026-03: "Final Judgment and Permanent Injunction upon Consent" (ECF 104-1) with a $3.5M "proxy for damages". Collection is forborne unless the defendants breach. The PI enjoined "operating any server emulation software that enables unauthorized access to Daybreak's EverQuest copyrighted works" and using the EVERQUEST mark.

Facts the court weighed:
- THJ was commercial (estimated up to $100k/month);
- it modified client files and redirected the official client;
- the court found likely market substitution, with a decline in official server population;
- THJ invoked "clean-room, reverse engineered, open-source code" and Sega, without success.

Fallout:
- Defendants' estoppel and laches arguments based on Daybreak's long tolerance of EQEmu and P99 failed. The record showed Daybreak "regularly sent cease and desist letters".
- EQEmulator then adopted terms (per EFF/Techdirt, December 2025): "your operation of an EQEmulator server is subject to Daybreak's permission, which it may revoke for any reason or no reason".
- Daybreak launched "EverQuest Legends" (official) afterwards.
- EFF called Daybreak a "copyright bully".

**Project 1999 letter agreement (2015-04-23)**

Signed by Daybreak GC David Youssefi; exhibit in the THJ docket. Key terms:
- "not a license … preserve the peace";
- emulator "created from scratch", with no access to or use of Daybreak server software;
- no copying, modifying or distributing client software (data-file mods only with written approval);
- users must obtain their own client, and the project may not assist piracy;
- no emulators for other Daybreak games, plus content-era limits;
- non-commercial (PayPal donations for costs only);
- bound by the ToS/EULA;
- no Daybreak marks or assets in promotion, except the plain-text game name used descriptively;
- mandatory disclaimer "…is not associated or affiliated in any way with Daybreak Game Company LLC";
- release-date deference to Daybreak.

This is the realistic template for any LOTRO permission request. For LOTRO, MEE's consent would also be needed.

**Echoes of Angmar (LOTRO, 2022–present)**
- MEE C&D on 2022-08-05: "we must ask that you immediately cease all of your unauthorized use of Echoes of Angmar, and all other Tolkien-related IP on all platforms". The C&D came from MEE, not SSG.
- The project ignored it and is still operating as of 2026-10. Its site says "You use the original client to connect to our server … You will need the original 2007 EU (Codemasters) client patched to Book 11. We don't provide the client but it can be found around the Internet". The Lutris installer pulls `LOTRO_Bk11_High_Res_Multi_Language.zip` from a third-party mirror.
- MassivelyOP described it as "based on original code from the time"; the code's origin is unverified.
- This is exactly the risky pattern: original client, redirection, and a mirror link for the client. Do not associate with it.

**Middle-earth Revenge (MER, GitHub org)**
- A LOTRO RE/emulation library: "for educational purposes only … never will host any private servers without explicit permission".
- It also points to a OneDrive folder of "legacy LotRO clients", which is distribution of copyrighted software. **Do not link to it or use it.**
- Its 2013 tAmMo tools describe the protocol:
  - a 128-bit session key is RSA-encrypted with a 1024-bit public key embedded in the client;
  - RC4 is used for the account/GLS ticket block, then the session is encrypted;
  - checksum tables come from server-supplied bases;
  - the tools "inject the public key into the client", i.e. modify the client.
- That client-patching approach is the THJ-style circumvention pattern. A clean reimplementation should use its own keys and its own protocol, and never patch the official client or ship Daybreak's key.

**Asheron's Call / ACEmulator**
- WB C&D in September 2017, after the game closed in January 2017. One emulator was shut down and videos taken down.
- ACE continues today. Its disclaimer: "for educational and non-commercial purposes only, use of the game client is for interoperability with the emulated server … not associated or affiliated in any way with Turbine, Inc. or WB Games Inc."
- Tolerance correlates with the game being dead.

**SWGEmu**
- Since about 2004 it has claimed legality on the basis of from-scratch code and no distributed SOE assets. Its tolerance by SOE/Daybreak/Disney is unofficial (seen in search). SWG itself has been closed since 2011.

**City of Heroes: Homecoming**
- It began on the original server code in 2019; NCSoft granted Homecoming Servers LLC a limited license on 2024-01-04. Only possible because the game was dead.

**OpenMW**
- GPLv3, "All source code is written from scratch", user must "legally own Morrowind", ships no assets, and has its own fonts.
- Bethesda "is well aware of OpenMW and has given us approval to continue … so long as certain conditions are followed" (e.g. no promotion of Morrowind running on Android).

**ClassicUO** (BSD-4/ISC)
- "does not distribute any copyrighted game assets … you'll need to legally obtain a copy of the Ultima Online Classic Client. **Using a custom client to connect to official UO servers is strictly forbidden.**"

**WoW private servers**
- Scapegaming: $88M default judgment, 2010.
- Nostalrius: C&D in 2016 (from memory).
- Turtle WoW (seen in search, unverified primary source): sued August 2025, ruling and settlement April 2026, offline 2026-05-15, with operators barred from successors.

**re3/reVC (Take-Two, 2021)**
- Decompilation-derived "reversed source code" got a DMCA takedown (github/dmca 2021-02-19) and then a lawsuit (N.D. Cal., September 2021). It was dismissed with prejudice in April 2023 after a confidential settlement.
- → Decompiled or translated code is "derivative source code". **Never import decompiler output.**

**Wine "Clean Room Guidelines"** (verbatim, current wiki):
- DO write black-box conformance tests.
- DON'T disassemble Microsoft code; look at any MS source (even "public"); examine debug symbols; use +relay with native components; look at ReactOS code; or "use an LLM tool to generate code. There's no guarantee that the training material of that LLM respects our Clean Room Guidelines".
- Wine FAQ: anyone "who has seen Microsoft Windows source code (stolen, under an NDA, disassembled, or otherwise)" cannot contribute.

**ReactOS (2006)**
- An allegation of disassembly-derived code led to a development freeze, a full audit, and a signed IP-policy agreement for every developer.

## 6. Reusing published LOTRO RE work: license matrix (from clones in `scratchpad/clones`, 2026-10-03)

**OSI-licensed, usable under license terms (verify provenance before importing):**

| License | Projects |
|---|---|
| GPL-3.0 | Middle-earth-Revenge/DATUnpacker, Middle-earth-Revenge/LocalDataExtractor, Lynx3d/pylotro, Mirarkitty/lotro_mesh_viewer, Endevir/Elanor, JuneStepp/OneLauncher |
| AGPL-3.0 | Endevir/DatExplorer, Mogwai-TheFurry/DatExplorer (only if the project itself is AGPL) |
| Apache-2.0 | Mirarkitty/lotro_extractor (GPLv3-compatible one way) |
| MIT | Endevir/LOTRO-Enchanced-text-patcher, gitlab kilves/lotro-mesh-converter, gitlab evjonell/lotro-cl, mklinik/lotro-fov, digero/maestro, lunarwtr/vscode-lotro-api, koniecdev/LotroKoniecDev |
| GPL-2.0 (check whether "or later") | Middle-earth-Revenge/LotroDepotDownloader |
| ISC | n0la/ddolauncher |

Asheron's Call relatives:
- MIT: Chorizite/DatReaderWriter, Chorizite.ACProtocol, amoeba/asheron-rs, amoeba/gromnie;
- GPL-3.0: ACEmulator/ACViewer;
- AGPL-3.0: ACEmulator/ACE.

**No license file, so all rights reserved.** Read them only as factual documentation of formats and protocols; do not copy code:
- the LotroCompanion repos (lotro-companion, lotro-core, lotro-tools, lotro-data*, lotro-maps, lotro-jukebox);
- jtauber/lotro;
- gitlab lotro/{server,gls,analyzer,unpacker,htdocs} (bwgypyth);
- Middle-earth-Revenge/lotro-server (tAmMo tools) and MER PacketMaster/VortexPacketAnalyzer;
- Xaymar/RE-DDO, Dromo, Eorlthain, Hrimvi, aya-po map viewer, lunarwtr/lotro-api-docs, NeuralFoundry freecam.

**Data repos** such as LotroCompanion/lotro-data contain material extracted from Daybreak's dats. Never vendor them; that is Daybreak/MEE expression and EU database-right material.

**Recommended project license: GPL-3.0-or-later.** It is the OpenMW precedent and can absorb GPL-3/Apache-2/MIT/ISC code. Choose AGPL only if you want to reuse AGPL code such as ACE or DatExplorer. Track every file with SPDX headers (REUSE).

## 7. Risk-tiered scope recommendation

**Tier A (do first, lowest risk):**
- format library: .dat container, DID/property system, meshes, textures, audio, strings;
- tools: viewer/exporter for personal use only, no bulk export features advertised;
- offline renderer / world walker that reads a user's existing install in place.

Requirements for Tier A:
- ship original placeholder/test content so the engine runs without LOTRO data (substantial non-infringing use);
- detect a genuine install, e.g. by the presence of the official launcher/registry entries, but implement no DRM bypass and do not refuse "unknown" data in a way that implies piracy support.

**Tier B (medium):** a networked client speaking a **project-defined, documented protocol** to a **project-owned reference server** that serves only original or test content. A protocol "compatibility layer" with LOTRO's wire format is legally more exposed (EULA 4.3–4.5, the THJ TPM theory) and should be a separate, later decision after legal review.

**Tier C (high; do not do without Daybreak + MEE written permission):** a gameplay-emulating server reproducing LOTRO quests, NPC scripts, combat and world, or any publicly hosted shard, even a free one.

**Tier X (never):**
- connecting to official servers or implementing official login/GLS/patcher auth;
- shipping or linking client binaries, dats, patches or mirrors;
- modifying or redirecting the official client;
- embedding Daybreak keys;
- packet captures from official servers in the repo;
- any monetization;
- using leaked Turbine/SSG source or server code.

## 8. Concrete guardrails

### A. Clean-room process (roles adapted for an AI-assisted team)

1. **Spec writers ("dirty room").** They may run the official client, observe their own traffic and files, and read published RE material.
   - Default: black-box only, i.e. Art 5(3)-style observe/study/test plus reading data files.
   - If binary disassembly is ever authorized, do it in an EU/UK jurisdiction, by a lawful licensee, confined to interop needs.
   - Output only **prose specs/tables** (field layouts, enums, state machines) in a separate `spec/` repo. No code, no pseudo-code transliteration, no copied strings beyond the minimal identifiers needed for interop.
2. **Reviewer/filter.** Strips expressive content and checks that every spec item is functional fact. Keep the review record.
3. **Implementers ("clean room").** They never see the binaries, disassembly or decompiler output, and never read unlicensed or derived code. They implement from specs plus black-box tests. Commits reference spec IDs (git trailer `Spec-Ref:`).
4. **AI-specific rules:**
   - never paste decompiled or disassembled output, or Daybreak files, into an LLM session that writes project code;
   - run Ghidra/IDA/MCP-assisted analysis only in a dirty-room session whose sole output is prose spec;
   - record AI-tool use per commit (`Assisted-by:` trailer);
   - run license/similarity scans (ScanCode Toolkit) on AI-generated code to catch verbatim reproduction of unlicensed repos.
   - Wine bans LLM code outright. Decide consciously and document the policy.
5. Anyone who has seen Turbine/SSG/Daybreak source code (leaked, NDA, ex-employee, or decompiled) may not contribute to the affected components. This mirrors Wine. Ex-Turbine/SSG staff also need a trade-secret/NDA check.

### B. Content and assets
- The repo contains no Daybreak/MEE assets or extracted data: no textures, meshes, sounds, music, fonts, UI art, strings, quest text, maps, item databases or Lua UI files.
- All defaults are original or freely licensed (OpenMW ships its own fonts).
- **The engine reads dats in place, read-only, at runtime, from the user's official install path.** No installer copies, repacks, patches or "fixes" the dats. This avoids the THJ "copy the client folder" reproduction theory.
- No bulk-export UI marketed as a ripping tool. Exporters stay personal-use and are opt-in.
- Screenshots and videos of LOTRO content are only as permitted by ToS §14.1: non-commercial, no logos, no implied endorsement.

### C. Network
- Hard-code no Daybreak hosts.
- The default build cannot connect to official servers, and the docs state clearly that it must not, as ClassicUO does.
- No official-auth/GLS/patcher implementation and no Daybreak keys.
- No packet captures from official servers in repos or issue trackers. This is the Free Realms 2016 DMCA precedent. If captures are needed for spec work, keep them private to the dirty room and reduce them to field-level facts.

### D. Distribution and money
- Free, non-commercial, no paid tiers or perks, no ads.
- Donations, if any, only for infrastructure costs (the P99 model).
- Do not facilitate piracy: no links to client mirrors (contrast EoA and MER), and docs point only to official download channels.

### E. Naming and trademark
- Project name, domain, org and handles must not contain or allude to: LOTRO, Lord of the Rings, LotR, Middle-earth, Tolkien, Hobbit, Shire, Mordor, Angmar, Moria, Mirkwood, Isengard, Rohan, Helm's Deep, **Mithril (a listed mark)**, Turbine, Standing Stone, SSG, Daybreak, or Tolkien-invented words and Elvish-styled near-homophones.
- Use an invented neutral name and clear it on USPTO, EUIPO eSearch/TMview and WIPO Global Brand Database.
- Use LOTRO names only in plain text for compatibility: "compatible with data files from The Lord of the Rings Online™". No ring logo, game fonts or trade dress.
- Mandatory disclaimer on README, site and app: "[Project] is an independent, unofficial project. It is not affiliated with, endorsed by, or sponsored by Daybreak Game Company LLC, Standing Stone Games, Middle-earth Enterprises, LLC, Warner Bros., or the Tolkien Estate. The Lord of the Rings Online is a trademark of Middle-earth Enterprises, LLC used under license. Requires a legally obtained installation of the game; no game content is distributed."

### F. Contributor policy
- **DCO 1.1** sign-off (`Signed-off-by:`) enforced in CI, plus a project-specific **Clean-Room Attestation** in CONTRIBUTING.md that each contributor agrees to (as ReactOS did after 2006). It certifies that:
  1. no Turbine/SSG/Daybreak source code has been seen, leaked or otherwise;
  2. the contribution is not derived from decompiler or disassembler output or from unlicensed code;
  3. it contains no game assets, extracted data or keys;
  4. the contributor's role is "spec" or "implementation";
  5. AI tool use is disclosed.
- Contributor-risk notice: anyone who accepted the Daybreak EULA/ToS may personally breach §4.2, §4.3 and ToS §7.1 by RE or by distributing "software tools related to" LOTRO. Contributors should understand this, and may contribute pseudonymously.
- Recommend a contributor indemnity/defense fund, or fiscal hosting in a nonprofit, before going public.

### G. Repo hygiene and CI tooling
- **REUSE** (SPDX headers on every file) and **ScanCode Toolkit** license/similarity scans.
- **gitleaks/trufflehog** with a rule set for RSA/RC4 key blobs.
- File-type allowlist and magic-number checks rejecting DAT/DDS/OGG/WAV/BIK/font/zip blobs.
- A max blob size limit, Git LFS disabled, and a trademark wordlist linter on identifiers and UI strings.
- Mandatory `Spec-Ref:` trailers for format/protocol code.

### H. Hosting, jurisdiction, governance
- Mirror the repos on GitHub, Codeberg and a self-hosted Forgejo. Know the DMCA counter-notice process; re3 and youtube-dl filed counter-notices.
- Consider an EU-based steward or association for the stronger Art 5/6/8 position.
- Keep core maintainers' legal identities clear. Hidden-operator tactics (as in THJ) look bad in court.

### I. Engagement strategy
- Before shipping anything beyond Tier A, consider approaching Daybreak's General Counsel with the P99-style deal: non-commercial, clean-room, no server-side code, own-client requirement, no marks, date deference.
- MEE separately: MEE wrote the EoA C&D and controls the Tolkien IP and LOTRO marks.
- Expect at best a "preserve the peace" non-license letter, revocable at will.
- Keep records of every interaction with Daybreak/MEE. THJ's estoppel argument failed because it lacked conduct directed specifically at the defendants.

### J. Incident response
- On a C&D or DMCA notice: freeze public releases, don't delete history destructively (litigation hold), get counsel, consider a counter-notice only for clean code.
- Never keep operating while publicly defying a C&D (the EoA posture).

## 9. Open questions to resolve with counsel

- Whether Daybreak ToS §12's "not used … with non-Daybreak game servers / as a stand-alone game" is a license-scope condition (copyright) or a covenant (contract), under MDY's nexus test.
- Whether the THJ "server auth = access control over dynamic non-literal elements" theory reaches a from-scratch client plus own server that never touches Daybreak's client or servers. 9th Cir. MDY vs. Fed. Cir. Chamberlain.
- How the EU's Art 6 "interoperability with other programs" applies to interop with data files and a fan server rather than with official servers; and Art 6(3) "normal exploitation" for a live subscription game.
- Whether any LOTRO dat content or iteration data is encrypted or keyed. If so, (f)/Art 6 analysis is needed before reading it. Open-source readers show only zlib.
- MEE's position on an engine that contains no Tolkien content but exists only to render it.
- EU database right exposure for runtime loading of the full world database (probably fine for a lawful user; unverified).
