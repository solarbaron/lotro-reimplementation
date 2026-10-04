
# Turbine .dat container format (LOTRO / DDO, compared with AC / AC2): research notes

All of this was gathered on 2026-10-03 by cloning repos and reading their source. The best-documented reader turned out to be LotroCompanion's `delta-lotro-dat-utils` jar. Its source is not public, so I read its bytecode with `javap -c`. That jar is version 11.12 and ships inside `lotro-companion-24.10.0.49.6.zip` on SourceForge. Everything below comes from code, not memory. Each fact names its source(s). Anything marked **(inference)** or **(open)** has not been confirmed.

Sources used:
- **LC**: LotroCompanion dat-utils jar, `delta.games.lotro.dat.archive.*` (bytecode)
- **JT**: jtauber/lotro `dat.py` (2014, Python 2)
- **DO**: themissingdocs `DatOptimize.exe`. A .NET program whose author's page says it is "free to use, read, disassemble/reflect". I disassembled it with `monodis`.
- **MOG**: Mogwai-TheFurry/DatExplorer (DDO, C#)
- **DU**: Middle-earth-Revenge/DATUnpacker (wrapper around Turbine's datexport.dll)
- **CHZ**: Chorizite/DatReaderWriter (AC, read and write)
- **ACE**: ACEmulator ACE.DatLoader
- **DT**: gitlab jmspashett/datool (AC, Rust)
- **AC2**: nygard/PatchAC2Data
- **KD**: koniecdev/LotroKoniecDev knowledge base
- **EP**: Endevir LOTRO-Enchanced-text-patcher `datexport.py`

## 1. Archive set (current LOTRO client)

The archives are `client_<key>.dat`. The aux archives use the extension **`.datx`**. LC builds the name as `"client_"+key+".dat"+("x" if aux)`, and Steam forum error reports confirm names like `client_surface_aux_1.datx`.

| Kind | Keys |
|---|---|
| Required | `general`, `gamelogic`, `mesh`, `surface`, `sound`, `anim` (optional in LC), `local_<Locale>` (e.g. `client_local_English.dat`; LC's DATLocalFilesManager enumerates locales) |
| Optional | `highres` |
| Aux (`.datx`) | `highres_aux_1..3`, `surface_aux_1..2`, `sound_aux_1` |
| Cell | `cell_{1,2,3,4,5,14}` |
| Map | `map_{1,2,3,4,5,14}` |

LC decides the install directory is valid if `client_general.dat` exists (`DatChecks`).

Offsets in the format are 32-bit, so one archive is limited to 4 GiB. The aux `.datx` split is very likely how Turbine/SSG works around that **(inference)**. themissingdocs warns that LOTRODefrag v1.1 is required "due to changes in file format to handle more than 4GB".

## 2. File header ("superblock")

The header is the same in LOTRO, DDO and post-ToD AC. The first data block starts at 0x400: DO's rewriter writes 168 header bytes at 0x100, then seeks to 1024.

| Abs offset | Size | Meaning | Evidence |
|---|---|---|---|
| 0x000 | 256 | ASCII version string, NUL-terminated (may be empty) | CHZ `Version` |
| 0x100 | 64 | "Transactions"/journal area. An empty one starts with `00 50 4C 00` ("\0PL\0") | CHZ `WriteEmptyTransaction`. JT asserts dword@0x101 == 0x4C50 |
| 0x140 | u32 | magic **0x00005442** ("BT" little-endian) | LC MAGIC=21570, JT, CHZ, ACE, DT, MOG |
| 0x144 | u32 | block size | LC, JT, DO (0x44 relative to 0x100) |
| 0x148 | u32 | total file size. Must equal the actual file length (LC warns on mismatch) | LC, JT, DO |
| 0x14C | u32 | AC: DataSet (1 = portal, 2 = cell). LC prints it as "Version" 0x%04X; JT calls it version | LC, ACE, CHZ |
| 0x150 | u32 | AC: DataSubset. LC calls it "Version2" | |
| 0x154 | u32 | free-list head. LC labels it "Journal offset". DO walks the free chain from here | |
| 0x158 | u32 | free-list tail | |
| 0x15C | u32 | free block count | |
| 0x160 | u32 | **root B-tree node offset** | LC (+32), JT, DO (0x60) |
| 0x164 / 0x168 / 0x16C | u32 ×3 | AC: NewLRU / OldLRU / UseLRU. All zero in DDO | ACE, MOG |
| 0x170 | u32 | AC: MasterMapId. DatExport returns `didMasterMap` | |
| 0x174 | u32 | engine pack version. LC `_datPackVersion` (value **111 = old chained-block layout**). DDO log calls it "datpack version engine". DO calls it `version`. Very likely datexport's `vnumDatFile`, which KD observed as **112** throughout 2026 (U47–U49) | |
| 0x178 | u32 | game pack version ("Engine version" in LC, "datpack version game" in DDO). Likely `vnumGameData`, observed as **3** | |
| 0x17C | 16 | AC: VersionMajor GUID. datexport outputs `firstIterGuid` and `datIdStamp` | CHZ, ACE (unverified for LOTRO) |
| 0x18C | u32 | AC: VersionMinor | |
| 0x1A0 | u32 | LOTRO: size of the first free extent (DO, header[0xA0]) | |
| 0x1A4 | u32 | LOTRO: size of the root directory extent, normally **2460** (DO, header[164]; DO's DirectoryEntrySize = this − 8). LC hard-codes 2460 for the root | |

Header size:
- AC (CHZ): 400 bytes (0x000–0x18F).
- LOTRO (DO): reads 168 bytes from 0x100, so it covers up to 0x1A7.

Older layouts:
- Pre-ToD AC and AC2 put a shorter header at **0x12C**: magic, blockSize, fileSize, type, subtype, firstFree, lastFree, freeCount, root (DT, AC2).
- AC2 keeps a version record at DID 0xFFFF0001.

datexport.dll `OpenDatFileEx2` outputs these values: `didMasterMap, blockSize, vnumDatFile, vnumGameData, u64 datFileID, char* datIdStamp, char* firstIterGuid`. The Dcof flag bits are:
- UseLRU=1, Expandable=2, ReadOnly=4, Create=8, CreateIfNeeded=0x10
- OptionalFile=0x20, FreeThreaded=0x40, LoadIterations=0x80, Journalled=0x100, SkipIndexCheck=0x200

Tools typically open with flags 130 (0x82); 0x86 opens read-only.

## 3. Block/extent layer (LOTRO/DDO)

Every allocation (directory node or subfile) starts with an **8-byte block header {u32 a, u32 b}**. This is from LC `readBlockAt`.

**`b == 0`: modern extent layout.**
- `a` is the number of extra fragments (`fragCount`).
- The first extent holds `allocSize - 8 - 8*fragCount` payload bytes (capped at the logical size).
- Next come `fragCount` pairs `{u32 length, u32 offset}`.
- Read each fragment in order: seek to `offset` and read `min(length, remaining)`.
- Normally `fragCount == 0`, so one contiguous run holds the 8-byte header followed by the payload.

**`b != 0`, or the header engine version == 111: legacy chained layout** (LC `readOldBlockAt`).
- `a` is the size of the next block and `b` is its file offset.
- Each block contributes `size - 8` payload bytes, filled **from the end of the buffer backwards**. The chain is stored tail-first.
- When `a == 0`, the current block supplies the remaining bytes at the start of the buffer.
- DO likewise collects directory fragments and then calls `List.Reverse()` before concatenating.
- LC limits the walk to 1000 steps.

**Free space** (DO `PopFreeSpace`): a chain of free extents.
- Start at the offset in header 0x154 with the size in header 0x1A0.
- Each free extent begins with `{u32 nextSize, u32 nextOffset | 1}`. Bit 0 of the pointer is a "free" tag (DO masks with `& 0xFFFFFFFE`).
- The chain ends when `nextSize == 0`.
- AC uses a similar idea with fixed blocks: `next | 0x80000000` marks a free block (CHZ, AC2).

KD's observations of file growth:
- The file grows in ~1 MiB allocations ("+1 MiB" per content update).
- The launcher patches "chunks" in place (a changed SubFile is rewritten whole).

## 4. B-tree directory node (LOTRO/DDO)

The node payload is **2452 bytes**; the on-disk extent is 2460 including the 8-byte block header. Constants come from LC (`MAX_ENTRIES=61`, `ENTRY_RAW_SIZE=32`, `POINTER_RAW_SIZE=8`, `DIRECTORY_RAW_SIZE=2452`, `BASE_FILE_ENTRIES_OFFSET=496`) and JT.

| Payload offset | Size | Content |
|---|---|---|
| 0 | 62 × 8 | child pointers `{u32 extentSize, u32 offset}`. A pointer with size 0 means none. A leaf has the first pointer zero |
| 496 | u32 | entry count n (≤ 61). Children 0..n are valid |
| 500 | 61 × 32 | file entries (only the first n are valid) |

File entry (32 bytes). Field names are reconciled across LC, DO, JT and MOG:

| Off | Type | LC name | DO name | Meaning |
|---|---|---|---|---|
| 0 | u16 | flags | CompressionMode | Low bits = compression mode. **bit0 = zlib-compressed**. DDO enum: 0 none, 1 "maximum", 2 (seen on uncompressed LOTRO entries), 3 "default" compressed. DO rejects values > 3 |
| 2 | u16 | policy | Version | Per-entry format version. AC/CHZ writes 2 here |
| 4 | u32 | fileId | Id | **DID** (B-tree key, sorted ascending) |
| 8 | u32 | fileOffset | FileLoc | Absolute offset of the allocation (block header included) |
| 12 | u32 | size | Length | Stored payload length. For compressed entries this includes the 4-byte length prefix |
| 16 | u32 | timestamp | BitField | Unix time (JT prints it with `ctime`) |
| 20 | u32 | version | Iteration | Iteration counter, bumped by patches. This is the "iteration" the datexport API reports |
| 24 | u32 | blockSize | StorageLength | Bytes allocated on disk (≥ size + 8); MOG calls it "Dat Space Consumed" |
| 28 | u32 | (ignored) | Reserved | Must be 0 (DO logs "File record padding non-empty") |

**Lookup** (LC `getFileById`, JT `find_file`): binary-search the node's entries by DID. If the DID is not found and the node has children, descend into `child[lo]`. This is a classic B-tree, with entries stored in internal nodes too.

**In-order walk**: `child[0], entry[0], child[1], …, entry[n-1], child[n]`.

**Comparison with AC (ACE/CHZ/DT):**
- AC nodes are 62 × **u32** branch offsets, then a u32 count, then 61 × **24-byte** entries: `{u16 flags (bit0 compressed), u16 version, u32 id, u32 offset, u32 size, u32 date, u32 iteration}`. That is 1716–1720 bytes, stored across fixed-size **chained blocks**. Each block begins with a u32 next-block offset and carries blockSize−4 payload bytes. Portal block size is 0x400 and cell is 0x100.
- AC2/pre-ToD entries are 16 bytes `{id, offset, size, mtime}`, with the count at +248.
- LOTRO widened branches to `{size, offset}`, entries to 32 bytes (adding allocated size and reserved), and block headers to 8 bytes with extent lists.

## 5. Subfile payload

- If entry flags bit0 is set, the payload is `[u32 uncompressedLength][zlib stream]`. LC inflates bytes [4..]. The zlib header is usually 78 DA (JT and MOG also accept 78 9C).
- After decompression, or for raw entries, the payload starts with **u32 DID** (repeated), usually followed by a **u32 class/type index**. LC `DataIdentificationTools.getClassIndex` reads dword@4.
- DDO (MOG): an uncompressed payload begins with DID followed by a size dword.

## 6. DID → archive routing

This is LC `DATFilesDirectory.getArchiveKeys`. Keys are tried in order. The high byte acts as the "type prefix".

| DID range | Archive keys |
|---|---|
| 0x01xxxxxx, 0x04, 0x0E000004–0x0E4FFFFF, 0x0F, 0x18, 0x1F, 0x20, 0x23, 0x28, 0x2A, 0x2B, 0x30, 0x31, 0x40 | general |
| 0x06 | mesh. Geometry: kilves converter parses 060xxxxx; layout per its source is u32 id, u16, u16 rigged flag, u32 modelCount, … |
| 0x07, 0x34000000 (MasterProperty), 0x47, 0x56 (WLib class data, 0x56000000), 0x70–0x77 (game objects) | gamelogic |
| 0x0A | sound, then sound_aux_1, then local. Payload carries RIFF/WAVE or OggS |
| 0x22 (EnumMapper) | general, then local |
| 0x25–0x26 (string tables) | local |
| 0x41 (render surfaces/icons) | highres, highres_aux_1..3, surface, surface_aux_1..2, then local |
| 0x78–0x7F (DB properties) | gamelogic, then local. LC loads a game object's properties at **DID + 0x09000000** (`DBPROPERTIES_OFFSET`) |
| 0x80RRxxxx | Landblocks/cells. RR=0x01..0x05 and 0x0E are cell_1..5,14. 0x80[1]R = map_R. 0x80[2]R and 0x80[4]R also map to cell_R |

In 2014, JT saw cell DIDs at 0x2701xxxx/0x2702xxxx, so the ID scheme has changed over time.

Matching AC prefixes: 0x0A Wave, 0x22 EnumMapper, 0x78 DbProperties. AC uses 0x25 for DidMapper and 0x39 for MasterProperty; LOTRO uses 0x25 for strings and 0x34 for MasterProperty.

## 7. Known payload formats (beyond the container)

**Primitive encodings** (LC BufferUtils):
- **VLE** (variable-length integer):
  - b < 0x80: value is b.
  - b == 0xE0: a full u32 LE follows.
  - (b & 0xC0) == 0x80: `((b & 0x7F) << 8) | next`.
  - Otherwise: `((b & 0x3F) << 24) | (next << 16) | u16LE`. MER's JS ignores the top bits.
- **TSize/ListSize**: a 1-byte bucket-count hint, then VLE.
- **Pascal string**: VLE length followed by Latin-1 bytes.
- **Prefixed UTF-16**: VLE character count followed by UTF-16LE. **The count is in characters, not bytes.**

**String table (0x25xxxxxx, client_local_*)** per EP, KD and LC StringTableLoader:
- Header: `u32 DID`, `u32 unk (0/1/2)`, `u8 hint + VLE fragmentCount`.
- Each fragment: `u64 token` (LC reads it as u32 + u32 0), then `u32 nPieces` with each piece a VLE-prefixed UTF-16 string, then `u32 nArgRefs` × u32, then `u8 nArgStringGroups` with each group `{u32 n; n × VLE-prefixed UTF-16}`.

**Render surface (0x41)** per LC ImageLoader, JT and AC2: `u32 DID, u32 unk, u32 width, u32 height, u32 format, u32 length, pixels`.
- Formats are D3DFORMAT values: 20 (0x14) = R8G8B8, 21 (0x15) = A8R8G8B8, 28 (0x1C) = A8, 500 (0x1F4) = embedded JPEG, and FourCC 'DXT1', 'DXT3', 'DXT5'.
- DU builds DDS headers for these.

**MasterProperty (0x34000000)** per MER viewer: `u32 DID, 8 bytes pad, ListSize count, {u32 pid, pascal name}*`.

## 8. API surface of Turbine's datexport.dll

Seen in DU, Elanor, EP and KD: `OpenDatFileEx2`, `CloseDatFile`, `GetNumSubfiles`, `GetSubfileSizes(h, out did, out size, out iteration, offset, count)`, `GetSubfileVersion`, `GetSubfileCompressionFlag`, `GetSubfileData(h, did, buf, off, out version)`, `PutSubfileData(h, did, buf, off, size, version, iteration, compress)`, `PreallocateSubfile`, `PurgeSubfileData`, `Flush`, `GetNumIterations`, `CalculateFragmentation`, `ReleaseSubfileSizeData`.
- It imports CryptDecrypt.
- **It is proprietary Turbine code. The project must not ship it, link against it, or disassemble it.** At most a user could run it locally as a black-box test oracle, and even that is best avoided.

## 9. Write-side knowledge

- No open, clean LOTRO dat **writer** with public source is reachable today.
- Gi1dor/Endevir's C++ **LotroDat** library did extract, patch and emplace, with a fragmentation journal and version/iteration handling. It lived at git.endevir.ru, which now returns 502. Wayback has only the repo front page plus `LotroDat.h.tmpl` (headers: DatFile.h, Database.h, SubfileData.h, DatOperationResult.h; deps yaml-cpp, zlib). The source files were not archived.
- DO's IL has a full consistency checker and rewriter: duplicate DIDs, duplicate offsets, references into the free chain, fragments past EOF, and "file record padding non-empty".
- CHZ is the best open model of a B-tree insert/split/remove and a free-block allocator, but for the AC layout.

## 10. Open questions to settle against real data

These should be checked with a hex dump of the user's own files.
- The meaning of 0x14C and 0x150 in LOTRO, and the 0x17C–0x19F range.
- Typical LOTRO block size (0x144) and how it relates to the ~1 MiB growth increments.
- Whether 0x174 = 112 is vnumDatFile.
- When legacy chained blocks still appear (header[4] != 0).
- What the u16 at entry+2 ("policy"/version) means.
- How the journal at 0x100 (64 bytes) is used during crash recovery (the Journalled flag).
- How >4 GiB is handled (pointer units versus the aux split).

## 11. Recommended approach for the clean-room project

- Write our own reader from these facts. Facts are not copyrightable, but do not copy GPL/AGPL code into a permissively licensed project.
- Build a golden-test harness that walks every B-tree and checks the invariants DO checks.
- Make the reader read-only by default. A writer can come later, modelled on CHZ, with backup and journaling.
- Make `.datx` aux file handling and DID-range routing data-driven.
