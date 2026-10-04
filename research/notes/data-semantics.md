
# LOTRO game-data semantics: what is inside the .dat archives

The scope is what the decoded records mean, not the container. Every fact below comes from reading source code or docs in a scratchpad clone, or from decompiling the LotroCompanion jar. Clones live under `/tmp/claude-1000/-home-solarbaron-git-lotro-reimplementation/5f7fd9e6-9dcf-4273-b5fe-0179f3226498/scratchpad/clones/`. The decompiled `delta-lotro-dat-utils-11.0` sources are in `.../scratchpad/jars/datutils-src/` and its bundled resources in `.../scratchpad/jars/res/`.

## 0. Legal and provenance status of each source

- **LotroCompanion (Damien MORCELLET, "dmorcellet")**
  - The GitHub repos have no LICENSE file.
  - The shipped distribution does: `lotro-companion-distrib/lotro-companion/doc/LICENSE.txt` is Apache-2.0. Its NOTICE says "The rest of the software (delta-*.jar) has been developed by Damien MORCELLET", available on github.com/dmorcellet and github.com/LotroCompanion. SourceForge also lists the project as Apache 2.0.
  - So the `delta-lotro-dat-utils` jar can be studied, decompiled and even reused with attribution.
  - The jar's source is NOT on GitHub; only compiled jars are published:
    - `lotro-companion-distrib/lotro-companion/lib/delta-lotro-dat-utils-11.0.jar`
    - SourceForge files such as `delta-lotro-dat-utils-11.7.jar`
    - lotro-tools depends on 11.11; Maven groupId `com.dam.delta4j`.
  - I decompiled 11.0 with CFR 0.152; it gave 175 classes.
- **Mirarkitty/lotro_mesh_viewer**: MIT, except `havok_anim.py`, which is GPLv3 because it derives from PredatorCZ/HavokLib.
- **Mirarkitty/lotro_extractor**: Apache-2.0. It redistributes `classes.xml` and `StringHashMap.txt` from the Companion jar under Apache §4.
- **jtauber/lotro**: no license, so all rights reserved. Read it for facts only.
- **Middle-earth-Revenge DATUnpacker and LocalDataExtractor**: GPLv3 wrappers around the proprietary Turbine `DatExport.dll`. **Avoid the DLL.**
- **koniecdev/LotroKoniecDev**: bundles the proprietary `datexport.dll` (Turbine; its own notice says it is "proprietary library originally by Turbine"). **Do not use it or ship it.**
- **Hrimvi/LotroDataClientToolPatches**: a cheat tool (ESP, auto-target). **Avoid** any association with it.
- **Name tables bundled in the Companion jar** (`wlib/classes.xml` with 2036 classes, `functions.xml`, `StringHashMap.txt` hash→name, `utils/hash/KnownVariables.txt`):
  - These are names of client script classes and functions. They were probably harvested from client data or the binary.
  - Low risk as facts, but a clean-room project should regenerate them from the user's own data where possible. WLib class/attribute hashes can be resolved by hashing candidate names (see §9).

## 1. DID (data ID) space: high byte = record type

A DID is a u32. The high byte is the type; the low 24 bits are a serial.

Archive routing below is from `delta.games.lotro.dat.archive.DATFilesDirectory.getArchiveKeys()`. The search order is the order listed.

| DID range | Archive(s) searched | Meaning (from Companion and/or mesh_viewer) |
|---|---|---|
| 0x01xxxxxx | general | "Setup"/model: placement matrices, skeleton and part meshes (~1,000; not fully parsed). Stored uncompressed with a different header shape. |
| 0x04xxxxxx | general | Havok skeletons (`hkaSkeleton`/`hkaBone`), ~28,500. DATUnpacker found the packfile magic `57 E0 E0 57` at offset 8. For held items the record ends with a `[u8 count][count × u32 mesh DID]` trailer. |
| 0x05xxxxxx | client_anim.dat (not in the Companion map) | Havok animation clips, ~25,400 |
| 0x06xxxxxx | mesh (+ `client_mesh_aux_1.datx`) | GfxObj meshes, ~39,800. `0x0600____` mostly static; `0x0601____` skinned-heavy. |
| 0x07xxxxxx | gamelogic | Script tables (`ScriptTableLoader`): VFX, sound, animation nodes. Examples: 0x07000009, 0x0700042E, 0x070017F8, 0x0700042F ("Notes"), 0x0700000D, 0x0700001C, 0x07000008, 0x07000000. |
| 0x0Axxxxxx | sound, sound_aux_1, local | Sounds: SoundInfo records plus raw OGG/WAV payload records. Localized voice-over is in local. |
| 0x0C | gamelogic? | Other game entities/props, ~29,000 (mesh_viewer) |
| 0x0E000004–0x0E4FFFFF | general | Singleton geo tables. 0x0E000004 = PLACES (named positions, >6k). 0x0E000006 = map notes. 0x0E400000 = quest map notes. |
| 0x0F | general | ? |
| 0x18xxxxxx | general | Block-map property descriptors, one per region. Region 1 → 0x18000000; live: 2 → 0x18000014, 3 → 0x18000015, 4 and 5 → 0x1800001A, 14 → 0x18000014. |
| 0x1F | general | Held-item visual templates, ~29,000. The last u32 is a 0x04 DID (a "type 9" entry `09 00 00 00 01 00 00 00 <DID>`). |
| 0x20 | general | "APR" worn-appearance / wardrobe records (~4,200), one per body type. Also chargen style files. |
| 0x22 | general, then local | UI layouts. 0x22000041 = map UI, 0x22000033 = toolbar, 0x2200084B = slot icons, 0x22000889 = mounted combat, 0x220008BB = travel map. |
| 0x23 | general | EnumMappers (enum tables) |
| 0x25000000–0x26FFFFFF | local | String tables, per language |
| 0x28 | general | DataIdMap / DID directory. 0x28000000 is the master directory, with labels such as `EMAPPER`, `WEENIECONTENT`, `STRINGTABLE`. |
| 0x2A | general | ? |
| 0x2B | general | Compiled HLSL shader instances (~400, ~1 MB each, uncompressed) |
| 0x30 | general | Materials, ~24,000 |
| 0x31 | general | Surfaces / render properties, ~13,700, 22 bytes of true content |
| 0x34000000 | gamelogic | Master property dictionary (PropertyID → name, type) |
| 0x40 | general | Texture mip chain / image list (ordered 0x41 DIDs), ~80,000 |
| 0x41 | highres, highres_aux_1, highres_aux_2, surface, surface_aux_1, surface_aux_2, local | Textures and icons ("RenderSurface") |
| 0x47 | gamelogic | Entity records: avatar chargen and the PhysObj of held items, ~40,000 |
| 0x56xxxxxx | gamelogic | WLib (compiled script library). 0x56000000 = class metadata. |
| 0x70000000–0x77FFFFFF | gamelogic | WState objects: items, quests, skills, effects, NPCs and so on (~380,000) |
| 0x78000000–0x7FFFFFFF | gamelogic, then local | Bare DB property sets. 0x78000001 = UI_MapNoteIcons_Map ("UNIQUEDB"), 0x78000011 = bag icons. The 0x79xxxxxx range is the DBProperties of a 0x70 WState. |
| 0x800RXXYY | cell_R | Landblock data: terrain heightmap |
| 0x801RXXYY | map_R | Block map: per-landblock property indices (Area_DID, ambient music…) |
| 0x802RXXYY | cell_R | Landblock info: cells, static entities, links, weenies |
| 0x804RXXYY | cell_R | Landblock properties (UI_Map_GameMap minimap tiles) |

In the landblock rows, R is the region (1–5, 14 = 0xE), XX = blockX and YY = blockY, each 0–255.

Two further rules:
- **The DBPROPERTIES_OFFSET rule.** The property bag of any WState/content object DID `d` is at `d + 0x09000000`. So 0x70xxxxxx ↔ 0x79xxxxxx, and the Generator_ProfileDID, Area, Dungeon, item and NPC records all follow it. `DataIdentificationTools` gets the class of a WState from the u32 at byte offset 4 of the 0x70 blob, and its name from the property `Name` (or `Area_Name`) in the 0x79 blob.
- **Old client.** jtauber's 2008–2014 scripts also saw 0x2701xxxx and 0x2702xxxx cell records in `client_cell_2.dat`.

Archive file names (`DatFilesManager`): `client_<key>.dat`, plus `.datx` for aux archives. The keys are:
- general, anim, gamelogic
- `local_English` / `local_EN_GB` / `local_FR` / `local_DE` / `local_Russian`
- highres, highres_aux_1, highres_aux_2, mesh, surface, surface_aux_1, surface_aux_2, sound, sound_aux_1
- `cell_{1,2,3,4,5,14}` and `map_{1,2,3,4,5,14}`

The default locale is the newest-modified local file. Patch overlay (`PatchFilesManager`):
- `patch.dir` is `[i32 n][n × u32 DID]`.
- Each patch is `<DID %08X>.patch`, or `local/<locale>/<DID>.patch` for localized data.
- A patch file is a 16-byte header `[i32 ?][i32 version][i32 compSize][i32 uncompSize]` followed by a zlib payload (or raw data when compSize == uncompSize).

## 2. Container recap (only what the semantic readers depend on)

`DATArchive`:
- **Superblock** at 0x140: magic 0x5442 ("BT"), blockSize, fileSize, version, version2, journalOffset, freeTail, freeBlocks, rootNodeOffset at +32, datPackVersion at +52, engineVersion at +56.
- **Directory node**: 2452 bytes. It holds 62 `(blockSize, offset)` subdirectory pointers, the file count at +496 (at most 61), and 32-byte entries starting at +500.
- **Entry fields**: `flags:u16, policy:u16, fileId:u32, offset:u32, size:u32, timestamp:u32, version:u32, blockSize:u32, ?:u32`.
- **Compression**: `flags & 1` means compressed. The data is then `[u32 uncompressedSize][zlib]`; jtauber saw `78 DA` at payload offset 4.
- **Data blocks**: `[u32 numExtraBlocks][u32 legacy]` precedes the first chunk; when `legacy != 0` the archive uses the old reverse-chained read. If datPackVersion == 111 the old block format is used.
- **Lookup**: binary search inside a node, then descend into `subdirs[l]`.
- **Decoded records** generally begin with their own DID as a u32.

## 3. Primitive encodings used by every decoder (`BufferUtils`)

- All values are little-endian.
- **VLE** (`readVle`):
  - Read `a = u8`.
  - If `a == 0xE0` the value is the next u32.
  - Else if `!(a & 0x80)` the value is `a`.
  - Else read `b = u8`. If `!(a & 0x40)` the value is `((a & 0x7F) << 8) | b`.
  - Else read `c = u16`; the value is `((a & 0x3F) << 24) | (b << 16) | c`.
- **TSize**: skip 1 byte (a "bucket/type" byte), then a VLE.
- **PascalString**: VLE length followed by ISO-8859-1 bytes.
- **PrefixedUtf16String**: VLE character count followed by UTF-16LE (2 bytes per character).
- **AsciiString**: u8 length followed by ASCII.
- **Boolean**: a u8 that must be 0 or 1.
- **Hash for names** (`HashUtils`, a PJW/ELF variant):
  - Start with `h = 0`.
  - For each ASCII byte: `h = (h << 4) + b`; `hi = h & 0xF0000000`; `h ^= hi >> 24`.
  - Finish with `h & 0x0FFFFFFF`.
  - It is used to resolve StringInfo variable names (KnownVariables) and WLib attribute name hashes.

## 4. Property system

### 4.1 Master property dictionary: DID 0x34000000 (`PropertyDefinitionsLoader.decodeMasterProperty`)

```
u32 did(=0x34000000); skip 8
TSize nNames; nNames × { u32 pid; PascalString name }
skip 2
TSize nDefs; nDefs × { u32 pid; propertyDef }
propertyDef: u32 pid(echo) ; u32 typeCode(1..22) ; u32 ; u32 ; u32 data (enum DID / bitfield enum DID) ; u32 ;
             u32 v5 ; if v5&0x800 defaultValue ; if v5&0x1000 minValue ; if v5&0x2000 maxValue  (not for PROPERTY_ID/STRUCT/ARRAY)
             u32 marker == 0x3FC00000 ; skip 5 ;
             VLE nChildren ; nChildren × {u32 childPid; u32 childPid(echo)}
             u32 n ; n × u32 pid (must already be children) ; u32 0
```

Notes:
- Property IDs with bit 0x10000000 are the "0x1000xxxx" family. Appearance keys, part tags and property names all use it.
- The SoA book-11 legacy client lacks two definitions, 0x1000140D and 0x100011F9.

### 4.2 Property types (`PropertyType`; the code is LOTRO's, the DDO code is in parentheses)

| Code | Type | Wire format |
|---|---|---|
| 1 (6) | STRING | PascalString |
| 2 (18) | STRING_TOKEN | u32 |
| 3 (10) | WAVE_FORM | `u32 type`. Type 10: 10 floats, 1 float, 1 bool, `u32 n`, then 2n floats. Live type 1: 1 float. Type > 1: 10 floats. |
| 4 (13) | TIMESTAMP | f64 |
| 5 (21) | TRI_STATE | u8 |
| 6 (4) | VECTOR | 3 × f32 |
| 7 (11) | INSTANCE_ID | u64 |
| 8 (8) | ENUM_MAPPER | u32. The definition's `data` field is the enum DID: 0x23xxxxxx is an EnumMapper, 0x28xxxxxx a DataIdMap. |
| 9 (3) | FLOAT | f32 |
| 10 (19) | PROPERTY_ID | u32 (a property ID) |
| 11 (17) | STRUCT | skip u8, then `u8 nFields`, then nFields × property(doublePid) |
| 12 (16) | ARRAY | `u32 n`, then n × property(single pid) |
| 13 (7) | STRING_INFO | see 4.4 |
| 14 (15) | BITFIELD_64 | u64 |
| 15 (2) | INT | u32 (signed int) |
| 16 (5) | COLOR | 4 × u8 on disk. **In memory it is 4 × f32 RGBA** (per lotro_extractor). |
| 17 (12) | POSITION | see 6.1 |
| 18 (14) | BIT_FIELD32 | u32 |
| 19 (20) | LONG64 | u64 |
| 20 (9) | DATA_FILE | u32 (a DID) |
| 21 (1) | BOOLEAN | u8 |
| 22 (22) | BIT_FIELD | VLE bit count, then ceil(n/8) bytes, LSB first |

Bit fields of types 18, 14 and 22 are labelled through the EnumMapper named by the definition's `data` field.

### 4.3 Property sets (`DBPropertiesLoader`)

```
resource:   u32 DID ; properties
properties: TSize n ; n × property(doublePid=true)
property:   u32 pid (0 = null) ; [u32 pid echo if doublePid] ; value by registry type
```

- **Network / in-memory variant**:
  - Count is a VLE.
  - Packed PID: `a = u8`. If `a == 0x80` the PID is a u32. Otherwise `c = (a << 8) | u8`, and the PID is `0x10000000 | (c & 0x3FFF)` if `c & 0x4000`, else `c`.
  - INT values are VLE.
  - Enum and PID values are `u8 (0 | 16) + VLE`, where 16 means OR in 0x10000000.
- **Embedded property sets** appear in landblock info, UI layouts, script tables, entity descriptors and wardrobe sections.

### 4.4 StringInfo (`PropertyUtils.readStringInfoProperty`)

```
bool isLiteral
  literal: PrefixedUtf16String
  else   : u32 token ; u32 stringTableDID(0x25/0x26)
bool hasArgs
  yes: 3 × PascalString ; VLE nRepl ; nRepl × { u8 dataType(1=StringInfo,2=float,4=int) ; u32 varNameHash ;
                                                 if dataType!=1: u8 ==1 ; value (StringInfo | f32 | VLE) }
  no : u8 ==1 ; u8 ==0
```

### 4.5 String tables: 0x25/0x26 in client_local_*.dat (`StringTableLoader`)

```
u32 DID ; u32 unk (0..2) ; TSize nEntries ;
entry: u32 token ; u32 0  (= 64-bit "GossipId" in KoniecDev) ;
       u32 nParts ; nParts × PrefixedUtf16String   (text pieces between variables)
       u32 nVars  ; nVars × u32 variableID (name hash)     (nVars == nParts-1)
       bool hasNames ; [u32 n ; n × PrefixedUtf16String variable names]
```

- The pieces are interleaved with `${VAR}` slots when rendered.
- **Grammar tags.** A variable reference can carry an option list such as `${NAME:tag-options}`. Single-character tags select the variant (`TagsManager`):

| Tag | Meaning |
|---|---|
| 1 | singular |
| B | beorning |
| C | captain |
| D | dwarf |
| E | empty |
| f/F | feminine |
| G | burglar |
| H | champion |
| I | minstrel / neuter |
| K | rune-keeper |
| L | lore-master / elf |
| m/M | masculine |
| n/N | proper noun |
| O | hobbit |
| p/P | plural |
| R | brawler |
| S | plural s |
| T | hunter |
| U | guardian |
| v/V | initial vowel |

  The renderer picks the option with the most matching tags; negative tags are allowed.
- Tooling iterates 0x25000000–0x26FFFFFF. The STRINGTABLE label in 0x28000000 lists named tables.

### 4.6 EnumMapper: 0x23xxxxxx (`EnumMapperLoader`)

```
u32 DID ; u32 baseDID (inherit entries, 0 = none) ;
TSize n ; n × { u32 key ; PascalString label }     (internal names)
TSize m ; m × { u32 key ; StringInfo }             (localized display names)
```

Known enums:

| DID | Content |
|---|---|
| 0x23000091 | entity type |
| 0x230000CE | UI draw mode |
| 0x230000D1 | UI element ID |
| 0x230000D2 | UI state ID |
| 0x230000D6 (587202774) | map level |
| 587203489 | class specialization (internal name in `strings`, display name in `log_strings`) |

### 4.7 DataIdMap / DID directory: 0x28xxxxxx (`DataIdMapLoader`)

```
u32 DID ; repeat until EOF { TSize n ; n × {u32 key ; u32 DID} ; TSize n2(==n) ; n × {u32 key ; AsciiString label} }
```

- 0x28000000 is the root map, with labels EMAPPER, WEENIECONTENT and STRINGTABLE. Each label points to a sub-map from label to DID.
- Examples reachable this way:

| Label | DID |
|---|---|
| QuestArcDirectory | 0x7900E36F |
| WorldJoinControl | 0x7001B7CE |
| RegionControl | 0x7001819C |
| GameSetDirectory | 0x79009869 |
| TraitControl / class data | 0x7900025B, 0x7900020E (AdvTable_LevelTableList), 0x7900020F, 0x79000210 (nationalities), 0x79000229 (stat tomes) |
| Deeds directory | 0x79000255 |
| Item sorting | 0x79000230 |
| Recipes | 0x7900021E |
| Factions | 0x7900A452 |
| Paper items | 0x79024867 |
| Mob loot | 0x70000251 (+offset) |
| ItemMungingControl | 0x70000252 |
| Item advancement | 0x7000EAA6 |
| Mounts | 0x70048B29 |
| Cosmetic pets | 0x7004832E |
| Trait data WState | 0x7000025B |

## 5. WLib and WState (script object system)

### 5.1 WLib: DID 0x56000000 in gamelogic (`WLibLoader`)

```
u32 DID ; u32 stopCode ; u32 version (16, 30, 36, 38; v38 has an extra u32, v36 skips 16 bytes)
loop: u32 fourcc ; if fourcc == 0xFEDC1234 { u32 chunkType ; (v30/38: u32 size + payload) ; chunk }
                   else if fourcc == stopCode { u8 1 (not DDO) } else error
```

`lotro_extractor/docs/wlib.md` writes the magic as "0xB8C577F4"; that is a doc error. The correct value is −19131852 = 0xFEDC1234.

Chunk types:

| Type | Content |
|---|---|
| 1 | bytecode |
| 2 | messages (UTF-16) |
| 3 | import |
| 4 | export (classes, functions) |
| 5 | synthetic package table |
| 6 | vtable |
| 7 | GWM functions |
| 8 | reference variables |
| 16 | CLASS_DEFS |
| 64 | unknown |
| 256 | function location (end marker 0xD00DF00D) |
| 257 | chunk101 |
| 258 | source line numbers (file names!) |
| 259 | frame info |
| 512 | CLASS_VARS |
| 1024 | PARENTS_MAP |

Chunk layouts:
- **CLASS_DEFS**: `VLE n`, then n × `{bool defined; u16 classIndex; u16 pairCount; u32 rawSize; pairs (u32 default, u32 type); VLE nRefs × u16; VLE nOffs × u32; VLE nIdx × u16}`.
- **CLASS_VARS**: `TSize nClasses`, then for each `{u32 classIndex; TSize nVars; {u32 nameHash; u16 slotIndex; u8 typeCode}}`.
  - Type codes: 1 = REFERENCE, 2 = INTEGER, 3 = FLOAT, 130 = LONG, 131 = SIGNED_DOUBLE, 195 = DOUBLE.
- **PARENTS_MAP**: u32-prefixed array of the parent class index of each class, in CLASS_DEFS order.
- Attributes are not flattened from the parent class.

Class names come from `classes.xml`. Examples:

| Index | Name |
|---|---|
| 1 | CChatClient |
| 11 | AAHash |
| 17 | AArray |
| 1654 | PlayerAvatar |

Function IDs (`functions.xml`) encode `classIndex << 16 | n`; for example 65537 = CChatClient::SendToCharacter.

### 5.2 WState blobs: 0x70000000–0x77FFFFFF (`WStateLoader.decodeWState`)

```
u32 DID ; u32 classDefIndex ;
[live] TSize nImports × { u32 dboType ; u32 DID ; u8 c }  (dboType 69 → WState DID 0x70/0x76; 78 → appearance 0x20) ; VLE ; VLE ; bool
u32 classChunkSize ; classBundle ;
[live] bool doLinks ; links: u32 n × { u8 ; u32 ; u32 ; u32 ; TSize nProps × (u32 pid, u32 pid) ; u8 }
[live] bool ; TSize n × (u32 pid, u32 pid)    (available properties)
classBundle (live): VLE nRefs × u32 reference ; u16 nClassDefs × { u16 classIndex ; u16 nAttrs × {u32 nameHash ; u8 typeCode} } ;
                    nRefs × dataItem
dataItem: u32 marker: 0x08000000 → u64 IID ; 0x20000000 → u32 DID ; 0 or 0x10000000 → u16 classIndex + embedded
```

- **Embedded classes.**
  - If WLib defines the class, its attributes are read in load order: u32 for REFERENCE/INT, f32 for FLOAT, 2 × u32 for LONG/DOUBLE.
  - Otherwise a native loader is chosen by class index:

| Class index | Native loader |
|---|---|
| 11 / 35 | AAHash |
| 13 / 37 | AAMultiHash |
| 17 / 25 / 176 / 182 | AArray (176 and 182 hold references) |
| 18 | AHashSet |
| 23 | ALHash |
| 39 | BaseProperty |
| 57 | DynamicBitset |
| 97 / 117 | LRHash |
| 101 | LAMultiMap |
| 104 | LArray |
| 105 | LHashSet |
| 134 | NHashSet |
| 138 | NRHash |
| 160 / 161 | Position |
| 166 | PropertiesSet |
| 175 | RandomSelectionTable |
| 199 | StringInfo |
| 225 | String |
| 407 | DiscoveredMapNoteData |
| 415 | GameplayOptionsProfile |
| 2479 | QuestEventTargetLocation |
| 2567 | BankRepositoryDataAdaptor |
| 3103 | BankRepositoryData |
| 3461 | generic "III" |
| 3740 | BankType |

- **Network variant.** Each value is followed by a u32 type code, and attributes are sorted by slot index.
- **classDefIndex at offset 4** identifies what the record is (`WStateClass`):

| classDefIndex | Meaning |
|---|---|
| 795 | IClothing |
| 796 | chisel |
| 797 | shield |
| 799 | weapon |
| 802 | jewel |
| 827 | skill |
| 1398 | quest / accomplishment |
| 1477 | trait (also 1478, 1483, 2525, 3509) |
| 1494 | virtue |
| 3438 | steed trait |
| 713 / 734 / 748 / 763 | effects |
| 709 | aura |
| 718 | flag effect |
| 725 | instant vital |
| 755 | vital-over-time |
| 2219 | relic |
| 1824 | set |
| 1235 / 1237 | loot |
| 2728 | skirmish loot |
| 1406 | quest treasure |
| 1470 | title |
| 1049 | emote |
| 1723 | mob |
| 1724 | NPC |
| 2338 | legendary title |
| 2134 / 2152 / 4015 / 4024 / 4025 | IA legacies |
| 2532 | web store item |
| 1385 / 1389 / 3918 | progressions |
| 1391 / 1392 | property metadata |

  Many more property-modification effect classes exist: 716, 717, 739, 752, 753, 764, 780, 2156, 2258, 2259, 2441, 2459, 3218, 3690, 3833 (per Eorlthain/lotro-skill-effects).

## 6. Geography: terrain, landblocks, cells, maps, dungeons

### 6.1 Position encoding (`GeoLoader.readPosition`)

```
u8 flags (0 = null) ; &1 u8 region ; &2 u8 bx, u8 by ; &4 u16 instance ; &8 u16 cell ; &0x10 3×f32 xyz ; &0x20 4×f32 quaternion
```

- In the network form, `0x40` inhibits region and `0x80` inhibits cell.
- A landblock is 160 × 160 m; a region has 256 × 256 landblocks.
- **World ↔ map coordinates** (`PositionDecoder`):
  - `ox = bx*160 + x`, `oy = by*160 + y`.
  - Longitude = `(ox − 29360) × 0.005`; latitude = `(oy − 24880) × 0.005`.
  - These are the in-game "S/N, E/W" coordinates.

### 6.2 Landblock data and heightmap: 0x800RXXYY in cell_R (`HeightMapDataLoader`, jtauber `cell.py`)

```
u32 DID ; u32 lbiDID (= 0x00200000 + DID, i.e. 0x802RXXYY, or 0) ; u32 0 ; u32 0x441 (=33×33) ; 33×33 u16 heights ...
```

- Uncompressed size is often 0x1168.
- Heights are indexed as `x*33 + y`, with 5 m spacing.
- The height scale is **open**:
  - Companion uses `value / 32768`.
  - jtauber and Companion both visualise with `log1.045((16h) % 65535 + 1)`.
  - The rest of the record (after the heights) is undecoded: terrain types/textures, probably.

### 6.3 Landblock info: 0x802RXXYY (lotro-tools `LandblockInfoLoader`, readable source)

```
u32 DID ; u32 flags (1=cells,2=crosslinks,4=header indices) ; u32 sizeHint ;
[2] TSize × crosslink {u32 fromIdx ; u32 fromLbi ; u32 toLbi ; u32 linkType ; u32 toIdx ; u32 mask}
[4] TSize × {u16 idx ; u32 n × u16}
[1] TSize × cell { u16 index ; Position ; u32 cflags ; u32 cellMeshDID ; u32 nNeigh × {u32 i ; u16 cell ; u16 neighbour ; 3×u16 0 ; bool} ;
                   (cflags&0x20: u16 index echo, u32-prefixed u16 arrays ×2, u16 ; else u32-prefixed u16 arrays ×2) ;
                   cflags&4: properties (Dungeon_DID, Appearance_InstanceList, Physics/Render_AdjustableScale) }
u32 0 ; u32 n × EntityDescriptor ; u32 n × link ; properties ; u32 0 ; TSize n × weenie {u64 IID ; properties} ; [u32]
link: PascalString worldBuilderName ; u64 iid ; u64 toIid ; u64 fromIid ; u32 type ; properties ; bool cross ; [u32,u32,u32,u32]
```

Link types:

| Code | Type |
|---|---|
| 0x01 | PatrolOnce |
| 0x02 | Patrol |
| 0x07 | Generator_PositionSet |
| 0x08 | Generator |
| 0x09 | Segment |
| 0x0B | FlyingPatrol |
| 0x0C | Script |
| 0x10 | Path |
| 0x13 | AIRemoteDetector |
| 0x17 | SegmentBridge |
| 0x1000000B–0x10000013 | House_* |
| 0x10000019 | Phasing |

Weenies carry `Generator_ProfileDID`, which resolves through `GeneratorProfile_Profile` arrays to spawned NPC/mob DIDs (`GeneratorLoader`).

### 6.4 EntityDescriptor (`EntityDescLoader`): static scenery and props

```
u32 flags ; &1 u32 DID ; &2 u32 type (enum 0x23000091) ; &4 u64 IID ; &8 u32 physObjDID ;
&0x10 3×f32 pos + 4×f32 rotation quaternion ; &0x20 3×f32 vector ; &0x80 properties ; &0x40 u32 ; &0x100 PascalString name ; &0x200 VLE n × u32 DIDs
```

The `&0x80` properties block comes before the `&0x40` u32 in the stream.

### 6.5 Block map: 0x801RXXYY in map_R (`BlockMapLoader`)

```
u32 DID ; TSize n × { u32 key ; (live: u16 / SoA: u8) index }
```

- The value is `descriptor[region].get(key)[index]`.
- **Descriptor record** (0x18 DIDs): `u32 DID; u32 0; TSize n × {u32 pid; u32 pid; u32 blockMapKey; u8; u32 count; count × property(single pid)}`.
- **Properties obtained this way**: Area_DID, Ambient_MusicRegion, Ambient_MusicType, Ambient_SoundEnum_BlockMapOverride.
- This is how areas, music and ambient sound are attached to terrain.

### 6.6 Landblock properties and minimap: 0x804RXXYY

- PropertiesSet with `UI_Map_GameMap` → `UI_Map_MapImage` (one 0x41 tile per landblock).
- `UI_Map_MapImageArray` gives per-ContentLayer variants (`UI_Map_ContentLayer`). `MinimapLoader` tiles these at 50 px per block.
- **Content layers** are the phasing / world-state variants, e.g. Gondor Besieged.

### 6.7 Areas, dungeons and map notes

- **Areas** (+0x09000000): Area_Icon, Area_Territory → Area_Region → Area_RegionID, Ambient_MusicType, Area_Music.
- **RegionControl** (0x7001819C + offset): Region_Map_List / Region_ID.
- **Dungeons**: Dungeon_MapData → UI map props (UI_Map_GameMap, UI_Map_MapImage, UI_Map_Scale), Dungeon_ParentDungeon, Dungeon_Music.
- **Map georeferencing properties**: UI_Map_BlockOffsetX/Y, UI_Map_PixelOffsetX/Y, UI_Map_Scale, UI_Map_Child_Map, UI_Map_AreaDIDs_Array.
- **Map notes** (0x0E000006): `{Position; u32 areaDID; u32 dungeonDID; u32 noteDID; ...}`.
- **Places** (0x0E000004): `TSize n × {UTF16 name; Position; u32 areaOrDungeonDID ×2}`.

## 7. Meshes, materials, textures and shaders (lotro_mesh_viewer, with visual validation)

### 7.1 Item → geometry chains

- **Worn items:** item 0x70 → props at 0x79 → `Item_WornAppearanceMapList` (per species/sex: AppearanceKey 0x1000xxxx plus a 0x20 APR DID) → APR draw entry with `key == Item_AppearanceKey` → mesh 0x06 plus material 0x30.
- **Held items:** item → `PhysObj` (0x47) → 0x47 record `[u32 classTag][u32 selfDID][u32 7][u32 templateDID][TSize props…]` (it may chain to a parent 0x47) → 0x1F template → last u32 is a 0x04 → trailer mesh DIDs.
- **Chargen:** 0x47 avatar record (16-byte header, then a property set) → AppearanceUI_Controls → AppearanceUI_APRKey / AppearanceUI_APRFile (0x20) → blocks act as styles.
- **Live appearance properties:**
  - AC_Color_{Skin,Hair,Eyes,Lips,WarPaint} and AC_Texture_* indexes.
  - AC_BoneScale_Test and AC_Avatar_Height.
  - AC_BoneSlider_Value_Array: a hex string, one byte per slider, 0x80 = default.
  - Appearance_InstanceList[{Appearance_Key, Appearance_AprFile, Appearance_Modifier = q ≈ style/100}].
  - Appearance keys: head 0x10000009, hair 0x10000032, beard 0x10000018.
- **Species codes** (Item_SpeciesOfWearer): 23 Man, 65 Elf, 73 Dwarf, 81 Hobbit, 114 Beorning. 117 is probably High Elf and 120 probably Stout-axe; 125 is unknown.
- **Sex** (Item_SexOfWearer): 0x1000 male, 0x2000 female.

### 7.2 APR / wardrobe 0x20 grammar (validated on 4,181 of 4,192 records)

```
u32 DID ; u32 0 ; u8 sectionType(0,2..6) ; VLE count ;
entry: u32 key ; u32 nblocks ; block × nblocks:
  f32 q (dye floatCode / style index, 0.01 steps) ; u32 gc ; f32 d ;
  gc × {u32 x (0 | 0x2B shader) ; u32 material 0x30 ; u32 A ; u32 B ; f32 g} ;
  u32 pc ; f32 e ; pc × {u32 partTag ; u32 mesh 0x06 ; f32 lod} ; u32 0
trailer: u8 1 ; VLE n × {u32 key ; u32 value}
```

Part tags are specific to each record family. Examples:
- Dress/chest garment: 0x1000000C; hands: 0x10000001.
- Hats: 0x10000005.
- Legs: 0x10000003.
- Meshes under about 2 KB are three-vertex "none/hidden" stubs.

### 7.3 GfxObj mesh, 0x06

```
u32 DID ; u32 flags (0x00000001 static, 0x1000____ skinned) ;
u32 S × u32 surface(0x31) ; u32 T × u32 material(0x30) ;
per submesh: u32 vcount ; vcount × vertex(stride s, inferred) ; f32 bbox[6] ; u32 nBone × u32 boneData
index region, per submesh: u32 icount ; icount × u16 (submesh-local triangle list)
vertex: f32 pos[3] ; f32 normal[3] ; f32 uv[2] @24 ; tail (tangent frame, bone idx u8×k, weights f32×k)
```

- There is no stride field. The decoder detects it by finding consecutive unit normals whose run length matches vcount. Strides can differ between submeshes of one mesh.
- **Skin-tail layout by stride** (stride: influences / index offset / weight offset):

| Stride | Influences | Index offset | Weight offset |
|---|---|---|---|
| 61 | 1 | 56 | 57 |
| 65 | 1 | 60 | 61 |
| 66 | 2 | 56 | 58 |
| 70 | 2 | 60 | 62 |
| 71 | 3 | 56 | 59 |
| 73 | 1 | 68 | 69 |
| 74 | 2 | 64 | 66 |
| 75 | 3 | 60 | 63 |
| 76 | 4 | 56 | 60 |
| 79 | 3 | 64 | 67 |
| 80 | 4 | 60 | 64 |
| 84 | 4 | 64 | 68 |

- Bone indices index the `hkaSkeleton` bone array directly.
- Coordinates are Z-up. UVs outside [0, 1] are legitimate (tiling). Do not V-flip DXT data.

### 7.4 Materials, surfaces and textures

- **0x31 surface** (22 bytes): `[self DID][0x2B shader DID][slot key 0x10000050][u32 nMaterials = 1][0x30 material DID][u16 1]`.
- **0x30 material**: slot list `{u32 slotId, u32 slotId2, u32 type = 4, 0x40 DID, params…}`. The first 0x40 slot is the diffuse.
- **0x40 mip chain / image list**:
  - Ordered 0x41 DIDs, one per resolution.
  - DATUnpacker's description: `u32 DID; u8 type; u8 count (type 4 → 6); u16; u8; count × u32 0x41 DID; u32 0; u32 0x0F`.
- **0x41 texture / icon** (Companion `ImageLoader`, jtauber `surface.py`):
  - Layout: `u32 DID; u32 unk; u32 width; u32 height; u32 format; u32 length; data`.
  - Formats:

| Format code | Meaning |
|---|---|
| 20 (0x14) | D3DFMT_R8G8B8 (stored BGR) |
| 21 (0x15) | A8R8G8B8 (BGRA) |
| 28 (0x1C) | A8 |
| 'DXT1' / 'DXT3' / 'DXT5' | FourCC |
| 500 (0x1F4) | JPEG; the payload is a JFIF stream (FF D8 FF) |

  - These format codes are the D3DFORMAT enumeration values.
  - Placeholder textures are 8 px or smaller.
- **Item icons are layered**: Icon_Layer_ImageDID, BackgroundDID, OverlayDID, UnderlayDID, ShadowDID, LargeImageDID, ImageOverrideDID. `DatIconsUtils.buildImage` composites the layers.
- **0x2B shaders** are compiled HLSL blobs with no name field.
  - Classify them by counting uniform-name occurrences: `c_AlphaTestThreshold` (alpha-test cutout), `c_MaterialDyeColor` (dyeable), `c_SpecularMetallicAmount` (metallic), `c_MaterialSpecularColor`.
  - Example DIDs: 0x2B0009DE is the opaque face shader; 0x2B0009DF and 0x2B0009B7 are alpha-tested hair.
  - For a reimplementation, write your own shaders driven by these flags, or translate the user's own DXBC at run time.

### 7.5 Dyes

- A dye is a single scalar "floatCode" from about 0.01 to 0.80; 99.0 means unique / not dyeable. There are 45 named dyes in lotro-data `lore/colors.xml`.
- Render rule: `final = alpha < 128 ? tex.rgb * dyeRGB : tex.rgb`. The diffuse alpha channel is the dye mask; the dyeable area is stored as grey.
- The APR block `q` axis gives per-dye material variants.

## 8. Animation and skeletons (Havok 6.x; Turbine licensed Havok Physics/Animation/Behavior 6.0 in Aug 2008)

- **0x05 clips**, in `client_anim.dat`.
  - Layout: `f32 duration; u32 numTransformTracks; u8 0; u32 tagfileSize + 1; Havok binary tagfile`.
  - Magic `1E 0D B0 CA CE FA 11 D0`, tagfile version 3. DATUnpacker saw a packfile at offset 17 in 2011 data.
  - Clip classes: about 24,800 `hkaSplineCompressedAnimation` (all observed use 40-bit quaternions) and about 600 `hkaInterleavedUncompressedAnimation`.
  - No hkaAnimationBinding or hkaSkeletonMapper records exist. Tracks map to bones by position; annotation tracks carry bone names.
- **0x04 skeletons**: hkaSkeleton/hkaBone with parentIndices and referencePose.
  - Bone counts: Hobbit M/F and Dwarf M 62; NPC human 36; Man and Elf M/F 89 (unprefixed bone names); Beorning about 38+.
  - Bones include cape/skirt physics chains and `lweapon`/`rweapon` attach points.
- **Readers**:
  - exyorha/hkxparse (MIT) parses tagfiles and packfiles.
  - PredatorCZ/HavokLib (GPLv3) has the spline decompressor. Re-implement it clean-room if the project must not be GPL.

## 9. Sound (`client_sound.dat`, `sound_aux_1.datx`, local voice-over)

- **SoundInfo record** (`SoundInfoLoader.decodeSoundInfo`): `u32 DID; u32 flags; 5 × f32; u32 soundDataID; [PascalString name]`.
- **Raw sound record** (`decodeSound`): `u32 DID; u32 size; size bytes (RIFF WAVE or OggS Vorbis); u32; f32`.
- Usage is wired through script tables (0x07, SOUND node type 48) and properties:
  - SoundScript_SoundID
  - Area_Music / Ambient_MusicType (enum)
  - Dungeon_Music
  - Music_Instrument_Type
  - Quest VO: Quest_VODataID (pid 268439417), QuestDispenser_VOSoundID / _VOScriptID
  - Entity_AdditionalScriptTables
- LotroCompanion's lotro-jukebox catalogues every sound.

## 10. Script tables: 0x07 (`ScriptTableLoader`)

```
u32 type ; VLE nIncludes × u32 DID ; u8 bucket ; VLE nScripts × { u32 key ; VLE nNodes × node } ; props ; DIDs
```

| Node type | Name |
|---|---|
| 1 | DECAL |
| 2 | COLLECTION |
| 3 | ENTITY_VFX |
| 4 | EXECUTE |
| 5 | RUN |
| 6 | SEQUENCE |
| 7 | CINEMATIC_CAMERA |
| 9 | PHYSIC_IMPULSE |
| 10 | ON_STOP |
| 11 | WAVEFORM_BLEND |
| 12 | SWITCH |
| 13 | LIGHT |
| 14 | WHEN_RUNNING |
| 15 | DISTANCE_CHECK |
| 17 | RANDOM |
| 19 | SCRIPTLET |
| 25 | LABEL |
| 26 | HEADING_CHECK |
| 27 | GAME_CALLBACK |
| 33 | MODIFY_COMPOSITOR_COLOR |
| 36 | ANIMATION |
| 40 | SET_PROPERTY |
| 41 | MESH_FX |
| 48 | SOUND |
| 50 | WHILE |
| 52 | REMOVE_PROPERTY |
| 54 | BEAM |
| 57 | SCALAR_BLEND |

- **Target types**: INVALID, COMBINATION, PARAMETER, PROPERTY, THE_PLAYER, SEQUENCE, RANDOM, LINK, FIT, INITIATOR, FILTER, MONITOR, DISTANCE_FIT, THIS, CHILD.
- **Port types**: PARAMETER, RESPONSE, RUN_ON, SWITCH_VALUE, DEFAULT, FINAL_RESULT, NOT_RUNNING, RUNNING, CURRENT_RESULT, FOR_CURRENT_VALUE, WHEN_TEST, TARGET, SWITCH_TRANSITION.
- This is the client-side VFX, sound and animation event graph.

## 11. UI layouts: 0x22 (`UILayoutLoader`)

```
u32 DID ; u32 ; u32 ; TSize n × element
element: u32 id (enum 0x230000D1) ; u32 0 ; u8 ; i32 x,y,w,h ; u32 argb ; bool ; properties ; u8 n × UIData ;
         u32 index ; u32 elementID ; u32 typeID ; u32 baseElementID ; u32 baseLayoutDID ; u32 stateID ; 4×u32 margins ;
         TSize n × state {u32 stateID ; u32 ; u32 ; skip 18 ; properties ; u8 n × UIData} ; TSize n × child element
UIData: u32 type ; u32 ; 1 MOVIE{u32,PascalString,u32} 4 IMAGE{u32 imageDID,u32,PascalString filename} 5 MESSAGE{u32,f32}
        6 ANIMATE_VALUE{3f} 8 ANIMATION{f32 dur ; u32 drawMode ; bool loop ; u32 n × imageDID} 9 COLORED_IMAGE{u32 DID,u32,u32}
        10 CURSOR{3u32} 11 SOUNDINFO{u32} 12 STATE{u32,f32} 13 SCALE{2f}
```

- lunarwtr/lotro-api-docs `skinning/skinning_element_asset_assocation.xml` maps every skinnable UI element ID to its 0x41 ImageID, ArtAssetID and geometry. It is the ground truth for UI art and for the user-skin format (SkinDefinition.xml).
- Eorlthain's `data/ui.json` holds the client's tooltip colours and font metrics.

## 12. Gameplay data for quests, deeds, items and skills (property naming conventions)

Everything is property-driven: a WState at 0x70 plus a PropertiesSet at 0x79. Commonly used names:

- **Items**: Name, Item_Class, Item_Quality, Item_Level, Item_EquipmentCategory, Item_Socket_Array, Item_Decoration_*, Item_Music_Instrument_Type, EffectGenerator_EffectID, ItemAdvancement_*, PackageItem_*, WeenieType, Icon_Layer_*, PhysObj, Item_WornAppearanceMapList, Item_AppearanceKey, Item_SpeciesOfWearer, Item_SexOfWearer.
- **Quests and deeds**: Quest_Category, Quest_Scope, Quest_RoleArray (QuestDispenser_NPC, _RoleName, _Action), Quest_QuestsToComplete, Quest_ObjectiveIndex, Quest_LootTable, QuestTreasure_*, Accomplishment_List, Usage_QuestRequirements*, WorldEvent_WorldPropertyName.
  - Quest status codes: 0x20000000 failed, 0x30000000 completed, 0x40000000 abandoned, 0x50000000 tapped.
- **Skills**: Skill_Category, Skill_SkillType, Skill_Toggle_*, Skill_Vital_*, Skill_SmallIcon, Skill_RequiredTrait, Skill_Effect*.
- **Effects**: Effect_*, EffectGenerator_*, Mod_Array, Threshold_*.
- **Mobs**: Agent_Species, Agent_Subspecies, Agent_Genus, Agent_Class, Agent_Alignment, LootGen_*.
- **Traits**: Trait_*, AdvTable_*, Vital_*Regen.
- **Crafting**: CraftRecipe_*, CraftProfession_*.
- **Maps**: UI_Map_*.
- lotro-tools has an extraction class for each domain. It is the best "semantic Rosetta stone", and lotro-data's `lore/*.xml` shows the resulting normalized schemas.

## 13. Open questions and gaps (nobody has published these)

- Terrain beyond the 33×33 heightmap: height scale, terrain texture / blend maps, vegetation and scenery placement (0x801/0x804 records partly cover it), water.
- Interior cell geometry (the cellMeshDID in landblock info) and portals.
- 0x01 Setup records and the EnvCell-like structures.
- Environment, sky, weather and time of day: no public decoder found. In AC lineage the comparable record is the 0x0D Environment type (this is not confirmed for LOTRO). The WAVE_FORM property type exists for animated parameters.
- Particle / VFX systems (MESH_FX, ENTITY_VFX, BEAM, DECAL script nodes); light sources.
- Fonts: AC used 0x40 for fonts; in LOTRO 0x40 is texture lists, so the font type ID is unknown.
- `hkaInterleavedUncompressedAnimation` decoding, cloth physics, the 0x2B shader constants layout and the 0x1F template full grammar.
- WAVE_FORM and TIMESTAMP semantics.

## 14. Practical recommendations for the reimplementation

- Build one property-registry-driven decoder: read 0x34000000 at startup, then decode every PropertiesSet generically, so nothing is hardcoded per type.
- Port the Companion layouts (Apache-2.0) and the lotro_mesh_viewer layouts (MIT) into the project's language, with attribution. That covers properties, StringInfo, string tables, enums, DataIdMaps, WLib/WState, positions, landblock info, block maps, UI layouts, images, sounds, meshes, APR records and materials.
- Treat Havok tagfiles with a clean-room parser written from hkxparse (MIT).
- Use a validation discipline like Mirarkitty's: render and inspect, check for slivers, and confirm on two independent examples.
