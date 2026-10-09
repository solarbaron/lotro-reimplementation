<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# WLib bytecode: the script machine of the class library

| | |
|---|---|
| **Status** | **Accepted** (revision of 2026-10-08 reviewed and passed 2026-10-08) |
| **Last updated** | 2026-10-08 |
| **Author role** | Spec writer (dirty room) |
| **Reviewer** | Spec reviewer (separate session, not the author) |
| **Review record** | 2026-10-07: first and second reviews **changes requested** (fixed by F1–F5 and R1–R3, §17). Third review, 2026-10-07 (separate session, not the author): **passed**, Accepted; R1–R3 reproduced over all records. 2026-10-08 revision, reviewed 2026-10-08 (separate session, not the author): **passed**, Accepted. Details in the review log (decision 0018). |
| **Implements into** | A script-machine crate on top of `crates/wstate` (decoder, validator, interpreter core; crate placement is the implementer's choice); `apps/explorer` (disassembly and cross-reference view, §11); `apps/dattool` (listing and statistics) |
| **Companion specs** | [`wlib.md`](wlib.md) (container §3, bytecode chunk §4.1, messages §4.2, value type codes §5, class table and method tables §6, singleton list §7, attribute lists §8, parent map §9); [`wstate.md`](wstate.md) (object records, native classes §5.3); [`records.md`](records.md) §1.4 (UTF-16 strings); [`property-system.md`](property-system.md) §9 (DB-object types); [`combat-data.md`](combat-data.md) §13.2; [`misc-records.md`](misc-records.md) §13.5, §13.6; [`plugin-runtime.md`](plugin-runtime.md) §4.6 (floating-point environment) |

---

<a id="purpose"></a>
## Purpose

The class library (record 0x56000000, `wlib.md`) carries, besides the class layouts, the
compiled code of every script class: chunk type 1 holds 1,919,904 bytes of bytecode, and the
virtual-function tables of the class table (`wlib.md` §6.4) point into it. `wlib.md` leaves the
code out of scope. This spec covers it. With it an implementer can:

- split the code block into function bodies and instructions, and validate them (§2, §3, §12);
- disassemble every instruction with its operands (§3, §5) and show cross-references:
  calls, native calls, attributes read and written, classes created, message strings and
  property-ID constants (§11);
- build an interpreter core with the original's value, frame and call model (§4, §5, §6);
- know exactly where the code leaves the script machine (native functions, §7), which is
  the part this spec does **not** specify.

What the code **means** for gameplay (what a given skill, effect or usage-plugin function
computes) is not stated here. It follows from running or reading the code with this spec,
plus a specification of the native functions it calls. §10 measures how much code each
class family has.

---

<a id="provenance"></a>
## Provenance

How sources are marked:

- **Confirmed**: observed in the user's install on 2026-10-07 with throwaway probes in the
  dirty-room scratch area. The census covers **every** function body and every instruction
  (§2), not a sample.
- **BIN**: read from the client executable under method M4 (decision 0011). "Confirmed (BIN
  + OBS)" means the code says it and every observed instance agrees; "BIN only" means the
  data cannot show it (for example what happens with a value the data never contains).
- **Reported**: published sources only. There are none for this subject (see M2 below).

| Key | Source |
|---|---|
| OBS | Observed in the user's install, 2026-10-07 (Steam install; data files dated 2026-10-04). The library record 0x56000000 decoded as in `wlib.md`; the bytecode chunk swept from the first to the last byte (362,178 instructions in 18,323 function bodies, ending exactly at the chunk length); all 2,129 class-table entries, all method tables, all 4,525 messages; the object-family census of §10 uses the root class and property names of all 383,332 object records with prefix 0x70 (`wstate.md`). |
| BIN | **M4 (decision 0011).** Client build 4906.0071.1794.4035 (`x64/lotroclient64.exe`, `lotro_live`, SHA-256 prefix d22363eb9aff0fb5), read on 2026-10-07 in the dirty room with a headless decompiler and a disassembler, on a copy of the project's analysis database (deleted after use), by address range only: the script interpreter's dispatch routine and its 220 instruction handlers and error exits; the three routines through which native code calls script functions (by function ID, virtual by function ID, virtual by slot) and their shared call helper; the library's post-load link step and the instruction-length routine it uses; the allocation helpers used by instructions 0x0C, 0x0E and 0x0F; the prototype-record loader used by 0x0D; the machine release and resume helpers; the native-function registration routines (only to pair function IDs with entry points, by instruction pattern); three registered native functions, read to establish the native calling convention; the arguments at all 1,376 client call sites of the three entry routines (1,372 of the static form, 3 of the virtual form by function ID, 1 of the virtual form by slot), by instruction pattern. A second, shorter reading on 2026-10-07 (revision session, a disassembler on the read-only executable only, no copy of the analysis database) re-counted those call sites, looked at the argument registers of the three entry routines and at the 9 sites whose function ID the pattern had not recovered, and read the handlers of 0x73 and 0x74 (§5.9). **Questions:** instruction encoding and lengths; what each instruction does; the value, frame and call model and its limits; static and virtual dispatch; which operations leave the script machine and how arguments and results cross; how message strings are used; how references, attributes and prototype records are reached. **Why M1–M3 were not enough:** the data show only byte patterns. A census can suggest that instructions are 32-bit words, but not which byte is the opcode, what any opcode does, its stack effect, its limits or its error behaviour, and not what operand values refer to. No published source describes this bytecode (M2: the research clones and the third-party documentation were searched; the only third-party description of the library, `wlib.md`'s LC and MK sources, stops at the container). A running client (M3) does not expose the script machine's internal state. **Not read:** nothing from network, login, launcher or anti-tamper code. The callers of the resume helper (§6.5) were not followed, because their purpose was not established and they may serve server-driven continuations (decision 0004). **Third reading, 2026-10-08** (revision session; a disassembler on the read-only executable, same build, plus the first reading's dirty-room notes; no copy of the analysis database was made): which instruction handlers read byte 1 and bytes 2–3 of the first word; the link step's walk and what it does with operands it cannot resolve; how frame entry and return use *L* and *A*; the path of 0x0D through the object table; the exception-unwind data of the interpreter, the three entry routines and their call helper. Why M1–M3 were not enough: these are the client's reactions to inputs the shipped data never contain, so the data cannot show them (M1), nothing is published (M2), and a running client never meets them (M3). |
| WL | [`wlib.md`](wlib.md) (Accepted): container, chunk layouts, class table, method tables, function IDs (§6.4), singleton list (§7), attribute lists (§8). |
| WS | [`wstate.md`](wstate.md): native class indices and their codec names (§5.3); the client's registry of 258 native classes (its N6, BIN). |
| CD, MR | [`combat-data.md`](combat-data.md) (family root classes, modifier classes 1339–1342) and [`misc-records.md`](misc-records.md) §13.5–§13.6 (property-response and game-callback objects). |
| PR | [`plugin-runtime.md`](plugin-runtime.md) §4.6: the client changes the floating-point exception mask in some contexts. |

| # | Fact | Source(s) | Mark |
|---|---|---|---|
| B1 | Instructions are sequences of 32-bit little-endian words; the opcode is the low byte of the first word; 220 opcodes are valid (0x00–0xBF, 0xC4–0xDF); lengths by opcode (§3) | BIN (dispatch table, instruction-length routine); OBS (the sweep ends exactly at the chunk end; 177 distinct opcodes used, all valid) | Confirmed (BIN + OBS) |
| B2 | Function bodies: each method-table offset is a body start; bodies are contiguous; each starts with the frame-entry instruction 0x10 and ends with a return (§2) | OBS (18,323 of 18,323) | Confirmed |
| B3 | Branch offsets are signed 16-bit, relative to the end of the instruction word; switch tables and their target rule (§3.4) | BIN; OBS (24,929 branch targets and 1,323 switch targets, all instruction starts in the same body) | Confirmed (BIN + OBS) |
| B4 | Cells, value stack, frame base, argument and local cells, call records, frame entry and return (§4) | BIN; OBS (134,547 local-cell accesses within the frame; return width uniform per body) | Confirmed (BIN + OBS) |
| B5 | Limits: 4,096 value cells and 512 call records per machine; the call-record limit is checked, the value stack is not; locals start filled with byte 0xCD (§4.4) | BIN | BIN only |
| B6 | Semantics and stack effect of every instruction (§5) | BIN; OBS (stack-depth consistency of §13 V15 holds in 18,314 bodies) | Confirmed (BIN + OBS) for the 177 used opcodes; BIN only for the 43 unused ones |
| B7 | The interpreter never reads a cell's type code; it writes fresh codes on results and copies whole cells (§4.1) | BIN | BIN only |
| B8 | Static calls carry a body offset; virtual calls carry a slot index into the receiver's class table; function IDs are used only when native code calls script (§6) | BIN; OBS (all 31,189 static-call operands are body starts) | Confirmed (BIN + OBS) |
| B9 | Run statuses: 1 completed, 3 error; natives return 0 to continue; a machine kept on other statuses can be resumed (§6.5) | BIN | BIN only |
| B10 | Native calls: instruction 0x15 with a native function ID (package × 65,536 + function number); calling convention; link-time resolution; every ID used is registered (§7) | BIN; OBS (2,019 distinct IDs, 29,985 sites, 320 packages, all undefined class-table entries) | Confirmed (BIN + OBS) |
| B11 | Message strings are used only by instruction 0x0F, which makes a new string object (native class 225) (§8) | BIN; OBS (all 4,525 messages referenced) | Confirmed (BIN + OBS) |
| B12 | Object creation: 0x0C (script class, runs the slot-0 initializer), 0x0E (native class), 0x0D (prototype record 0x76000000 + class) (§9) | BIN; OBS (operand censuses of §9) | Confirmed (BIN + OBS) |
| B13 | Attribute instructions address slots of the class table; `this` accesses fit the slot table and the slot width in every body (§9.2) | BIN; OBS (7,329 of 7,329) | Confirmed (BIN + OBS) |
| B14 | Native code calls script at 1,376 sites: 1,372 static by function ID (1,367 distinct constant IDs, one per site, and 5 computed), 3 virtual by function ID (one constant ID, 0x005B0001), 1 virtual by slot (slot 0); 1,362 of the 1,367 static IDs resolve in this library; 0x005B0001 resolves to slot 84 of class 91, whose body forwards to slot 85 (§6.4, §10) | BIN (call-site pattern, both readings); OBS (resolution, body of slot 84) | Confirmed (BIN + OBS) |
| B15 | Class-family census (§10) | OBS; CD, MR (family definitions) | Confirmed |
| B16 | What the client does on integer division by zero, float division by zero, and undefined opcodes (§5.12, §15) | BIN (no check exists) | BIN only (the consequence is OPEN) |
| B17 | The 0xCD fill of locals is observable in the shipped code: a definite-assignment pass over all 18,323 bodies finds exactly one body (offset 0x125B8) that can read a local before any store to it, at two sites (§4.3, §12, §13 V16) | OBS (every body); BIN (the fill, B5) | Confirmed (BIN + OBS) |
| B18 | Body sharing comes only from inheritance: each of the 4,218 shared bodies has an owner that is an ancestor of all its other owners, and every other owner holds it at the same slot indices; 1,428 of them have two owners that are not ancestor and descendant (§2) | OBS (all method tables, parent map of `wlib.md` §9) | Confirmed |
| B19 | Property-ID immediates: 8,712 integer immediates of 0x01 equal a defined property ID (3,524 distinct IDs), no float immediate does; a byte search of the whole block finds 5,993 distinct IDs (§11.2, §16) | OBS (every 0x01 instruction, every byte offset; the 27,546 defined property IDs of the install's property registry, `property-system.md`); CD (§13.2, same byte-search count) | Confirmed |
| B20 | Class ancestry of the families of §10: all 110 game-callback classes descend from class 91 (78 as children, 27 through class 916, 5 through class 946; 916 and 946 are children of 91 and override slot 85); none of the 136 descendants of class 91 overrides slot 84; usage plugins: 24 children of class 3227, 6 grandchildren (through 3603 and 3694), and class 3227 itself | OBS (parent map of `wlib.md` §9 and method tables of all classes; family membership as in B15) | Confirmed |
| B21 | Byte 1 of the first word is read only by 0x01–0x04, which push it unchanged (zero-extended, any value 0–255) as the type code of their result; every other instruction ignores byte 1, and instructions without a 16-bit field ignore bytes 2–3 (§3.1, §12) | BIN (third reading) | BIN only |
| B22 | The client validates nothing at load: the link step walks from offset 0 with the length rule of §3.2, taking 4 bytes for any opcode not listed there, and stops at the first instruction start at or beyond *L*; unresolvable 0x0C, 0x0E and 0x15 operands fail only when the instruction runs (§7.3, §12) | BIN (third reading; the first reading for §7.3) | BIN only |
| B23 | Frame entry and return use *L* and *A* as byte counts: SP moves by *L* bytes, the 0xCD fill covers *L* rounded down to a multiple of 4 bytes, return sets SP to FP − *A* bytes; neither is checked or rounded to whole cells (§12) | BIN (third reading); OBS (every *L* and *A* is a multiple of 8, *L* ≤ 416, *A* ≤ 144, re-checked 2026-10-08) | Confirmed (BIN + OBS) |
| B24 | Each execution of 0x0D decodes the record anew into newly allocated objects and pushes the new root reference; it never returns a root object made by an earlier execution (§9.3) | BIN (third reading) | BIN only |
| B25 | The interpreter, the three native-to-script entry routines and their call helper register no exception filter (their unwind data carry at most the C++ unwind handler), and the interpreter does not change the floating-point control state (§5.12) | BIN (third reading) | BIN only |

---

<a id="s1"></a>
## 1. Concepts

| Term | Meaning |
|---|---|
| **Code block** | The *L* bytes after the u32 length in chunk type 1 (`wlib.md` §4.1); *L* = 1,919,904 here. Offsets in this spec are byte offsets into it. |
| **Body** | One compiled function: a contiguous run of instructions starting at a method-table offset (§2). |
| **Word** | 32 bits, little-endian. Every instruction is one or more words; every instruction starts at an offset divisible by 4. |
| **Cell** | One value-stack or attribute slot: 8 bytes = a u32 value and a u32 type code (`wlib.md` §5). 64-bit values occupy two consecutive cells (§4.1). |
| **Machine** | One script execution context: a value stack, a call-record stack and four registers (SP, FP, PC, current record) (§4). |
| **Frame** | The cells of one active function: its arguments (below FP) and its locals (from FP upwards), then its operand cells. |
| **Call record** | Per active function: the PC to resume at after a call it makes, its FP, and its argument byte count (§4.3). |
| **Receiver** | The object a method runs on ("this"): always the **last** argument pushed, i.e. the cell just below FP. |
| **Reference** | A u32 handle naming a live object in the client's object table; 0xFFFFFFFF is null. Type code 1. |
| **Package** | The high 16 bits of a native function ID; a number in the class-index space (§7.1). |
| **Native function** | Client code that script calls with instruction 0x15 (§7). |
| **Status** | The result of one run of the machine: 1 completed, 3 error (§6.5). |
| **Link step** | The client's pass over the loaded code block that resolves the operands of 0x0C, 0x0E and 0x15 (§7.3). |

Instruction names in this spec (`push.imm32`, `call.native` and so on) are Tessera's own labels;
the data carries opcode numbers only. Opcodes that §5 lists without a name (arithmetic,
conversions, comparisons, logic, branches, switches) have no prescribed label: a tool chooses
its own, one distinct label per valid opcode (Tessera rule, 2026-10-08).

---

<a id="s2"></a>
## 2. Function bodies (Confirmed, OBS)

- The distinct non-null offsets in all method tables (`wlib.md` §6.4) are the **body
  starts**: 18,323 distinct offsets (124,804 table entries). Every one is an instruction start
  and holds instruction 0x10 (frame entry). Every 0x10 in the code block is a body start.
- Bodies are **contiguous and complete**: body *i* runs from its start to the next body start
  (the last one to *L*). Offset 0 is a body start. No byte of the block lies outside a body.
- Each body **ends with a return** (0x11, 0x12 or 0x13). Within one body all returns have the
  same width: 13,129 bodies return one cell (0x11), 238 return two cells (0x12), 4,956 return
  nothing (0x13).
- Code never falls from one body into the next, and no branch leaves its body (§3.4).
- A body can be shared: 4,218 bodies appear in the tables of more than one class (up to
  1,151 classes for one body). Sharing comes **only from inheritance** (B18): for each shared
  body one of its owners is an ancestor (parent map, `wlib.md` §9) of all its other owners,
  and every other owner holds the body at the same slot indices as that topmost owner. In
  1,428 of the 4,218 some two owners are not ancestor and descendant of each other; they are
  sibling classes that both inherit the body. No body is shared by classes without such a
  common owning ancestor.
- Static calls (0x16) name bodies directly by offset (§6.1); 9,651 distinct bodies are called
  that way, all of them also in some method table. No body is reachable only by offset.

Census of the whole block: 362,178 instructions; frame entries declare 0–416 bytes of locals
(0–52 cells) and 0–144 bytes of arguments (0–18 cells), always multiples of 8.

---

<a id="s3"></a>
## 3. Instruction encoding

<a id="s3-1"></a>
### 3.1 The instruction word (Confirmed, BIN + OBS)

| Byte(s) of word 0 | Meaning |
|---|---|
| 0 | **Opcode** (u8). The interpreter dispatches on this byte alone. |
| 1 | **Type code** for the four immediate-push instructions 0x01–0x04 (`wlib.md` §5); zero in every other instruction (OBS: all 362,178). The original reads byte 1 only in 0x01–0x04 and pushes it unchanged, whatever its value; all other instructions ignore it (B21). |
| 2–3 | **16-bit field** *k*. Signed or unsigned depending on the instruction (§5). Zero where the instruction has no such field (OBS); the original ignores it there (B21). |

Further words (operand words) follow for the instructions of §3.2.

Valid opcodes: 0x00–0xBF and 0xC4–0xDF (220). The client's dispatch table has no entry for
0xC0–0xC3 and ends at 0xDF; it does **not** check the opcode before dispatching (BIN), so any
other opcode makes the original client jump to an undefined address. The data use 177 of the
220 (OBS).

<a id="s3-2"></a>
### 3.2 Instruction lengths (Confirmed, BIN + OBS)

| Opcodes | Length | Operand words after word 0 |
|---|---|---|
| 0x02 | 12 bytes | two words: a 64-bit immediate, **high half first** |
| 0x01, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x14, 0x15, 0x16 | 8 bytes | one word *w* (meaning per instruction, §5) |
| 0x57, 0xD8 | 12 + 8·*n* bytes | default offset (i32), *n* (u32), then *n* pairs (i32 key, i32 offset) |
| 0x58, 0xD9 | 12 + 12·*n* bytes | default offset (i32), *n* (u32), then *n* triples (u32 key high half, u32 key low half, i32 offset) |
| every other valid opcode | 4 bytes | none |

This is the rule the client's link step uses to walk the block (BIN). A sweep from offset 0
with it ends exactly at *L* (OBS).

<a id="s3-3"></a>
### 3.3 Operand forms

| Form | Where | Meaning |
|---|---|---|
| *k* signed (s16) | 0x03, 0x04 (immediate); 0x43, 0x44, 0x48, 0x49, 0x52, 0x95–0x98 (frame cell index relative to FP); all branches (byte offset) | Sign-extended |
| *k* unsigned (u16) | 0x0B (cell count); 0x15 (argument bytes); 0x4A–0x51 (attribute slot index); 0x53 (slot index); 0xDC–0xDF (cell offset) | Zero-extended |
| *w* u32 | 0x01 (32-bit immediate), 0x0C (class index), 0x0D (class index; low 16 bits used), 0x0E (native class index), 0x0F (message index), 0x10 (frame sizes, §4.3), 0x14 (virtual slot index), 0x15 (native function ID), 0x16 (body offset) | As listed |

<a id="s3-4"></a>
### 3.4 Branch and switch targets (Confirmed, BIN + OBS)

- **Branch** (0x54, 0x55, 0x56, 0x99, 0x9A, 0xC4–0xD7, 0xDA, 0xDB; all 4 bytes long): the
  target is (offset of the instruction + 4 + *k*), *k* signed. A *k* of 0 targets the next
  instruction.
- **Switch** (0x57, 0x58, 0xD8, 0xD9): let *E* be the offset just after the last pair or
  triple (the end of the instruction). The target of a matching entry is *E* + its offset;
  with no match it is *E* + the default offset.
- 0x57 and 0x58 compare the key with each entry in stored order and take the first equal
  entry. 0xD8 and 0xD9 binary-search the entries, so their keys must be in ascending order
  (signed comparison); with unsorted keys the original may miss a matching entry. A count *n*
  of 0 selects the default.
- OBS: all 24,929 branch targets and all 1,323 switch targets (223 switches, all 0x57, with
  defaults) are instruction starts inside the same body. 0x58, 0xD8 and 0xD9 do not occur.

---

<a id="s4"></a>
## 4. The machine model

<a id="s4-1"></a>
### 4.1 Cells and value kinds (BIN)

- A cell is (value u32, type u32). Type codes are those of `wlib.md` §5: 1 reference, 2
  integer, 3 float, 130 64-bit integer, 131 double, 195 time stamp.
- A 64-bit value occupies two consecutive cells: the cell at the **lower** index holds the
  low 32 bits, the next one the high 32 bits, and **both** carry the same type code (the
  layout of a 64-bit attribute in the class table, `wlib.md` §6.2).
- The interpreter **never inspects** a type code. Each instruction decides from its opcode
  how to treat the cells it pops, and writes the type code of its result itself (the "pushes"
  column of §5). Loads and stores copy whole cells, type code included, so an attribute slot
  takes the type code of whatever was stored in it.
- Integer arithmetic wraps (two's complement). Float and double arithmetic is IEEE single and
  double precision.

<a id="s4-2"></a>
### 4.2 Registers and stacks (BIN)

A machine has:

- a **value stack** of cells, growing upwards; **SP** is the index of the first free cell;
- **FP**, the index of the current function's first local cell;
- **PC**, the code-block offset of the next instruction;
- a **call-record stack**; record 0 is the **base** record, below the entry function.

The original machine has room for 4,096 value cells and 512 call records, and stack-relative
addresses (§5.5) are cell indices counted from the bottom of the value stack.

<a id="s4-3"></a>
### 4.3 Calls, frame entry and return (Confirmed, BIN + OBS)

**Call** (instructions 0x0C, 0x14, 0x15, 0x16 and the native-to-script entry of §6.4):

1. The current record stores the resume PC (the offset after the calling instruction).
2. A new record becomes current, with saved FP = 0 and argument bytes = 0.
3. PC becomes the body offset.

The arguments are already on the value stack, pushed by the caller in order; the last one
pushed is nearest to the callee's FP. A method's receiver is pushed last.

**Frame entry** (0x10, the first instruction of every body). Its operand word *w* holds
*L* = *w* & 0xFFFF (bytes of locals) and *A* = *w* >> 16 (bytes of arguments). The machine:

1. sets FP to SP and stores it in the current record;
2. stores *A* in the current record;
3. fills the *L*/8 cells from FP upwards with the byte 0xCD (value and type both
   0xCDCDCDCD; the locals are not initialised);
4. sets SP to FP + *L*/8.

The frame then is:

| Cells | Holds |
|---|---|
| FP − *A*/8 … FP − 1 | the arguments, the first pushed at FP − *A*/8; for a method, the receiver at FP − 1 |
| FP … FP + *L*/8 − 1 | the locals |
| FP + *L*/8 … SP − 1 | operand cells of the expression being evaluated |

**Return** (0x11 one cell, 0x12 two cells, 0x13 none):

1. the result cells are the top 1 or 2 cells below SP;
2. SP becomes FP − *A*/8 (the arguments are removed by the callee);
3. 0x11 and 0x12 copy the result cells to SP and advance SP past them;
4. the current record is discarded; the previous record becomes current; FP and PC are
   restored from it (its saved FP and its resume PC);
5. if the record that became current is the base record, the run ends with **status 1**.

Consequences an implementer must reproduce:

- A return copies "the top cell" without checking that the operand area holds one. With an
  empty operand area and no locals the top cell is the last argument. The compiled code relies
  on this: many slot-0 initializers end with 0x11 directly after their last call, so they
  return their receiver (OBS: the stack-depth check of §13 V15 needs this rule).
- A void return discards any operand cells left on the stack; the code often omits pops
  before a 0x13 (OBS).
- The 0xCD fill of step 3 is **observable** (B17). The body at offset 0x125B8 (*L* = 24,
  *A* = 24) stores local 0 only inside four conditional arms, each selected by a 0x54 branch.
  When all four branches are taken, no arm runs and control reaches 0x127D4, which loads
  local 0 and tests it with 0x54; 0x127E0 loads it again as an argument of native function
  0x00DD0003. With the fill the loaded cell is (0xCDCDCDCD, 0xCDCDCDCD), the test sees a
  non-zero value and the native call runs with that cell as its argument; with zero-filled
  locals the call would be skipped. An interpreter must therefore fill locals exactly as in
  step 3, value **and** type code (§12). This is the only such body in the shipped data
  (§13 V16). What the native function does with that cell, whose type code is not a valid
  type code (`wlib.md` §5), is part of the native-function OPEN (§7.1).

<a id="s4-4"></a>
### 4.4 Limits and checks (BIN only)

| Check | Original client |
|---|---|
| Call-record overflow | Checked by 0x0C, 0x14, 0x15, 0x16: a call is refused when 511 records are already above the base (at most 511 active functions, natives included). The run ends with status 3 and PC left at the calling instruction. |
| Value-stack overflow or underflow | **Not checked.** 4,096 cells; pushes beyond them overwrite the call records. |
| Instruction budget | **None.** An endless loop does not end. |
| Opcode validity | Not checked (§3.1). |
| Operand ranges | Not checked: slot indices against the slot count, message indices, frame indices, switch counts. |
| Null references | Checked by attribute, address and virtual-call instructions (§5). |

Tessera's own bounds are given in §12.

---

<a id="s5"></a>
## 5. Instruction set

Stack notation: "pops a, b" means *b* is the top cell and *a* the one below it; widths are in
cells (a 64-bit value is two cells). "i32/u32" are the 32-bit value read as signed or
unsigned, "i64/u64" likewise for two cells, "f32" a float, "f64" a double (type 131 or 195).
Results get the type code in the **Pushes** column: `int` = 2, `long` = 130, `float` = 3,
`double` = 131, `time` = 195, `ref` = 1. Unless a row says otherwise the instruction is 4
bytes long and PC moves to the next instruction. "Uses" counts sites in the data (OBS).

<a id="s5-1"></a>
### 5.1 Constants

| Op | Name | Operands | Pops | Pushes | Uses |
|---|---|---|---|---|---|
| 0x01 | `push.imm32` | type *t* (byte 1), *w* | — | (*w*, *t*) | 13,715 (*t* = 2: 13,425; 3: 290) |
| 0x02 | `push.imm64` | type *t*, words *hi*, *lo* | — | 64-bit (*hi*:*lo*), both cells type *t* | 204 (*t* 130: 106, 131: 11, 195: 87) |
| 0x03 | `push.short` | type *t*, *k* s16 | — | (*k* sign-extended to 32 bits, *t*) | 5,270 (*t* = 2) |
| 0x04 | `push.short64` | type *t*, *k* s16 | — | *k* sign-extended to 64 bits, type *t* | 853 (*t* = 130) |
| 0x05 / 0x07 / 0xB6 | `push.int 0 / 1 / −1` | — | — | int | 16,697 / 5,449 / 188 |
| 0x06 / 0x08 / 0xB7 | `push.long 0 / 1 / −1` | — | — | long | 892 / 60 / 7 |
| 0xBA / 0xBC / 0xB8 | `push.float 0.0 / 1.0 / −1.0` | — | — | float | 870 / 386 / 154 |
| 0xBB / 0xBD / 0xB9 | `push.double 0.0 / 1.0 / −1.0` | — | — | double (131) | 9 / 5 / 1 |
| 0x09 | `push.null` | — | — | (0xFFFFFFFF, ref) | 2,053 |

<a id="s5-2"></a>
### 5.2 Stack manipulation

| Op | Name | Operands | Effect | Uses |
|---|---|---|---|---|
| 0x00 | `nop` | — | nothing | 0 |
| 0x0A | `pop` | — | removes 1 cell | 10,849 |
| 0x0B | `popn` | *k* u16 | removes *k* cells | 161 (*k* ≤ 2) |
| 0xBE | `dup` | — | pushes a copy of the top cell | 6 |
| 0xBF | `dup2` | — | pushes a copy of the top two cells | 0 |

<a id="s5-3"></a>
### 5.3 Frame cells (arguments and locals)

The cell index *i* is relative to FP (negative = arguments, FP − 1 = receiver of a method).

| Op | Name | Operands | Effect | Uses |
|---|---|---|---|---|
| 0x43 | `ld.frame` | *k* s16 | pushes cell FP + *k* | 16,845 |
| 0x44 | `ld.frame64` | *k* s16 | pushes cells FP + *k*, FP + *k* + 1 | 3,112 |
| 0x48 | `st.frame` | *k* s16 | pops 1 cell into FP + *k* | 6,261 |
| 0x49 | `st.frame64` | *k* s16 | pops 2 cells into FP + *k*, +1 | 219 |
| 0x9B, 0x9C, 0x45, 0x9D, 0x9E, 0x9F, 0xA0 | `ld.frame` short forms | — | pushes cell FP − 3, −2, −1, 0, 1, 2, 3 (0x45 = the receiver) | 4,035; 11,550; 26,233; 17,199; 10,670; 5,739; 4,105 |
| 0xA1–0xA7 | `ld.frame64` short forms | — | pushes cells FP + *i*, FP + *i* + 1 for *i* = −3 … 3 | 0xA3 unused; others 124–2,834 |
| 0xA8–0xAE | `st.frame` short forms | — | pops 1 cell into FP + *i*, *i* = −3 … 3 | 2–8,984 |
| 0xAF–0xB5 | `st.frame64` short forms | — | pops 2 cells into FP + *i*, +1, *i* = −3 … 3 | 0xB1 unused; others 1–443 |
| 0x46 | `st.frame.at` | — | pops index *n*; stores the (new) top cell into FP + *n*, which **stays** on the stack | 0 |
| 0x47 | `st.frame64.at` | — | pops index *n*; stores the two top cells into FP + *n*, +1; they stay | 0 |
| 0x95 / 0x97 | `inc.frame / dec.frame` | *k* s16 | adds +1 / −1 to the value of cell FP + *k* (32-bit, in place; type code unchanged) | 514 / 14 |
| 0x96 / 0x98 | `inc.frame64 / dec.frame64` | *k* s16 | same on the 64-bit value at FP + *k* | 0 / 0 |

<a id="s5-4"></a>
### 5.4 Attributes

*k* is a slot index of the object's class (`wlib.md` §6.1). The object is a reference: popped
(`*.ref` forms) or the receiver at FP − 1 (`*.this` forms). A null reference ends the run
with status 3 (PC left at the instruction). Stores leave the stored value on the stack.

| Op | Name | Pops | Pushes | Uses |
|---|---|---|---|---|
| 0x4A | `ld.attr.ref` *k* | ref | slot *k* (whole cell) | 583 |
| 0x4B | `ld.attr64.ref` *k* | ref | slots *k*, *k* + 1 | 16 |
| 0x4C | `ld.attr.this` *k* | — | slot *k* of the receiver | 3,643 |
| 0x4D | `ld.attr64.this` *k* | — | slots *k*, *k* + 1 of the receiver | 105 |
| 0x4E | `st.attr.ref` *k* | value (1 cell) below ref; pops only the ref | the value stays | 274 |
| 0x4F | `st.attr64.ref` *k* | value (2 cells) below ref; pops only the ref | the value stays | 5 |
| 0x50 | `st.attr.this` *k* | — (value stays) | — | 3,390 |
| 0x51 | `st.attr64.this` *k* | — (value stays) | — | 191 |

In the data, the instruction after an attribute store is a pop (0x0A after one-cell stores,
0x0B *k* = 2 after two-cell stores) or a return at 3,859 of the 3,860 store sites; the one
other site stores the same value into a local (a chained assignment) (OBS).

<a id="s5-5"></a>
### 5.5 Addresses (by-reference values)

An **address** is a two-cell value (base, index): base = a reference (type 1) or null, index =
a cell index (type 2).

| Op | Name | Operands | Pops | Pushes | Uses |
|---|---|---|---|---|---|
| 0x52 | `addr.frame` | *k* s16 | — | (null, absolute value-stack index of cell FP + *k*) | 3,240 |
| 0x53 | `addr.attr` | *k* u16 | ref (null ends the run, status 3) | (ref, *k*) | 69 |
| 0xDC | `ld.addr` | *k* u16 | address | the cell at index + *k* | 1,127 |
| 0xDD | `ld.addr64` | *k* u16 | address | two cells from index + *k* | 1 |
| 0xDE | `st.addr` | *k* u16 | address (value cell below it stays) | — | 1,281 |
| 0xDF | `st.addr64` | *k* u16 | address (two value cells stay) | — | 25 |

When the base is null the index names a value-stack cell (counted from the bottom of the
stack, so an address stays valid while the frame that owns the cell is alive); otherwise it
names a slot of the referenced object. Addresses let a function write into its caller's local
or into an attribute (out-parameters, and the native calls of §7.2 that return extra values).
*k* is 0 in all data.

<a id="s5-6"></a>
### 5.6 Integer and 64-bit arithmetic

All pop the operands listed and push one result of the same width with type int (32-bit) or
long (64-bit).

| 32-bit op | 64-bit op | Operation | Uses (32 / 64) |
|---|---|---|---|
| 0x17 | 0x18 | a + b | 261 / 5 |
| 0x19 | 0x1A | a − b | 167 / 10 |
| 0x1B | 0x1C | a × b (low bits) | 19 / 1 |
| 0x1D | 0x1E | a ÷ b, signed, truncated toward zero | 20 / 1 |
| 0x1F | 0x20 | remainder of a ÷ b, signed, sign of a | 17 / 0 |
| 0x21 | 0x22 | −a (pops one) | 13 / 1 |
| 0x23 | 0x24 | a + 1 (pops one) | 13 / 0 |
| 0x25 | 0x26 | a − 1 (pops one) | 3 / 0 |
| 0x87 | 0x88 | bitwise not (pops one) | 10 / 6 |
| 0x89 | 0x8A | a or b | 68 / 9 |
| 0x8B | 0x8C | a xor b | 0 / 1 |
| 0x8D | 0x8E | a and b | 212 / 49 |
| 0x8F | 0x90 | a shifted left by b | 33 / 7 |
| 0x91 | 0x92 | a shifted right arithmetically by b | 0 / 0 |
| 0x93 | 0x94 | a shifted right logically by b | 4 / 4 |

Shift counts are taken modulo 32 (32-bit) or modulo 64 (64-bit; the count is the 64-bit b).
Division and remainder do not test for a zero divisor or for the minimum value divided by −1
(BIN); see §5.12.

<a id="s5-7"></a>
### 5.7 Float and double arithmetic

| f32 op | f64 op | Operation | Pushes | Uses (f32 / f64) |
|---|---|---|---|---|
| 0x27 | 0x28 | a + b | float / double (131) | 136 / 22 |
| 0x29 | 0x2A | a − b | | 50 / 29 |
| 0x2B | 0x2C | a × b | | 187 / 5 |
| 0x2D | 0x2E | a ÷ b | | 40 / 4 |
| 0x2F | 0x30 | a × −1.0 (pops one) | | 21 / 0 |
| 0x31 | 0x32 | a + 1.0 (pops one) | | 0 / 0 |
| 0x33 | 0x34 | a − 1.0 (pops one) | | 0 / 0 |

f64 operations accept time stamps (195) and produce 131.

<a id="s5-8"></a>
### 5.8 Conversions (each pops one value)

| Op | From → to | Rule | Pushes | Uses |
|---|---|---|---|---|
| 0x35 | i32 → f32 | nearest | float | 70 |
| 0x36 | i64 → f64 | nearest | double | 0 |
| 0x37 | u32 → f32 | nearest | float | 85 |
| 0x38 | u64 → f64 | nearest | double | 8 |
| 0x39 | f32 → i32 | truncate toward zero; NaN or out of range gives 0x80000000 | int | 78 |
| 0x3A | f64 → i64 | truncate; NaN or out of range gives 0x8000000000000000 | long | 15 |
| 0x3B | i32 → i64 | sign-extend | long | 167 |
| 0x3C | i64 → i32 | low 32 bits | int | 17 |
| 0x3D | u32 → i64 | zero-extend | long | 28 |
| 0x3E | i64 → i32 | low 32 bits (same as 0x3C) | int | 13 |
| 0x3F | f32 → f64 | exact | double (131) | 111 |
| 0x40 | f64 → f32 | nearest | float | 32 |
| 0x41 | f32 → time | exact | time (195) | 20 |
| 0x42 | 64-bit → time | no conversion, the type code becomes 195 | time | 21 |

<a id="s5-9"></a>
### 5.9 Comparisons and logic (results are int 0 or 1)

Integer comparisons (pop a, b; push int):

| Relation | i32 | i64 | u32 | u64 |
|---|---|---|---|---|
| a = b | 0x5F (201) | 0x60 (32) | | |
| a ≠ b | 0x61 (63) | 0x62 (24) | | |
| a < b | 0x63 (4) | 0x64 (0) | 0x6B (5) | 0x6C (0) |
| a > b | 0x65 (7) | 0x66 (0) | 0x6D (25) | 0x6E (0) |
| a ≤ b | 0x67 (2) | 0x68 (0) | 0x6F (7) | 0x70 (0) |
| a ≥ b | 0x69 (8) | 0x6A (0) | 0x71 (25) | 0x72 (1) |

References: 0x85 a = b (65), 0x86 a ≠ b (81), comparing the 32-bit handles.

Float and double comparisons (pop a, b; push int). With a NaN operand, = and the four
ordered relations give 0 and ≠ gives 1.

| Relation | f32 | f64 |
|---|---|---|
| a = b | 0x79 (28) | 0x7A (6) |
| a ≠ b | 0x7B (27) | 0x7C (7) |
| a < b | 0x7D (60) | 0x7E (12) |
| a > b | 0x7F (84) | 0x80 (22) |
| a ≤ b | 0x81 (48) | 0x82 (4) |
| a ≥ b | 0x83 (20) | 0x84 (6) |

Logic (a value is "true" when it is not zero; a float or double NaN counts as true). For
floats and doubles (0x73–0x78) "zero" means an IEEE comparison equal to 0.0 (BIN): −0.0
counts as zero (0x73 and 0x74 give 1 for it, and it is false in 0x75–0x78), and a NaN is
unordered, so it is not zero (0x73 and 0x74 give 0 for it, and it is true in 0x75–0x78).

| Op | Operands | Result | Pushes | Uses |
|---|---|---|---|---|
| 0x59 / 0x5A | one i32 / one i64 | 1 if zero | int / **long** (both cells type 130) | 86 / 0 |
| 0x73 / 0x74 | one f32 / one f64 | 1 if 0.0 | int | 0 / 0 |
| 0x5B / 0x5C | two i32 / two i64 | 1 if either is true | int | 0 / 0 |
| 0x75 / 0x76 | two f32 / two f64 | 1 if either is true | int | 0 / 0 |
| 0x5D / 0x5E | two i32 / two i64 | 1 if both are true | int | 0 / 0 |
| 0x77 / 0x78 | two f32 / two f64 | 1 if both are true | int | 0 / 0 |

The two-operand logic forms do not occur in the data (OBS); conditions are compiled to
branches (§5.10).

<a id="s5-10"></a>
### 5.10 Branches

All are 4 bytes long; the target rule is §3.4.

| Op | Pops | Branches when | Uses |
|---|---|---|---|
| 0x56 | — | always | 4,378 |
| 0x54 / 0x55 | i32 / i64 | the value is zero | 6,899 / 4 |
| 0xDA / 0xDB | i32 / i64 | the value is not zero | 5,565 / 4 |
| 0x99 | ref | the reference is null | 965 |
| 0x9A | ref | the reference is not null | 3,182 |

**Compare-and-branch** (pop a, b). These continue with the next instruction when the
relation **holds** and branch when it does not:

| Relation | i32 | i64 |
|---|---|---|
| a = b | 0xC4 (1,070) | 0xC5 (497) |
| a ≠ b | 0xC6 (701) | 0xC7 (206) |
| a < b signed | 0xC8 (111) | 0xC9 (2) |
| a > b signed | 0xCA (152) | 0xCB (1) |
| a ≤ b signed | 0xCC (45) | 0xCD (0) |
| a ≥ b signed | 0xCE (63) | 0xCF (2) |
| a < b unsigned | 0xD0 (596) | 0xD1 (2) |
| a > b unsigned | 0xD2 (330) | 0xD3 (3) |
| a ≤ b unsigned | 0xD4 (68) | 0xD5 (0) |
| a ≥ b unsigned | 0xD6 (80) | 0xD7 (3) |

<a id="s5-11"></a>
### 5.11 Switches

| Op | Pops | Layout (§3.2) | Search | Uses |
|---|---|---|---|---|
| 0x57 | i32 key | pairs | first equal key in stored order | 223 |
| 0x58 | i64 key | triples | first equal key in stored order | 0 |
| 0xD8 | i32 key | pairs | binary search, keys ascending (signed) | 0 |
| 0xD9 | i64 key | triples | binary search, keys ascending (signed) | 0 |

<a id="s5-12"></a>
### 5.12 Undefined cases

The original client does not guard against the following (BIN); the data never need them:

- integer division or remainder by zero, and the minimum value divided by −1 (0x1D–0x20):
  a processor fault in the original;
- float division by zero (0x2D, 0x2E): infinity or a fault, depending on the floating-point
  exception mask in force (PR). The interpreter does not change the floating-point control
  state; it runs with that of the thread that started it (B25), which is not established;
- opcodes 0xC0–0xC3 and 0xE0–0xFF, and any operand out of range (§4.4).

Neither the interpreter nor the three native-to-script entry routines (§6.4) nor their call
helper registers an exception filter: their unwind data carry at most the C++ unwind handler,
so a processor fault is not handled inside the script machine and passes to the frames that
started it (B25). What those frames do with it is not established.

Tessera rule (its own): each of these ends the run with status 3 (§12).

<a id="s5-13"></a>
### 5.13 Calls, returns and object creation

| Op | Name | Length | Operands | Effect | Uses |
|---|---|---|---|---|---|
| 0x10 | `enter` | 8 | *w* = *L* \| *A* << 16 | Frame entry (§4.3); first instruction of every body | 18,323 |
| 0x11 | `ret` | 4 | — | Return one cell (§4.3) | 22,340 |
| 0x12 | `ret64` | 4 | — | Return two cells | 329 |
| 0x13 | `ret.void` | 4 | — | Return nothing | 7,189 |
| 0x16 | `call` | 8 | *w* = body offset | Static call (§6.1) | 31,189 |
| 0x14 | `call.virtual` | 8 | *w* = slot index | Virtual call on the receiver at the top of the stack (§6.2) | 8,574 |
| 0x15 | `call.native` | 8 | *k* = argument bytes; *w* = native function ID | Native call (§7) | 29,985 |
| 0x0C | `new` | 8 | *w* = class index | New script object; runs its slot-0 initializer (§9.1) | 554 |
| 0x0E | `new.native` | 8 | *w* = native class index | New native object (§9.1) | 5,950 |
| 0x0D | `ld.prototype` | 8 | *w* = class index | Object of record 0x76000000 + class (§9.3) | 5 |
| 0x0F | `ld.message` | 8 | *w* = message index | New string object holding message *w* (§8) | 6,449 |

---

<a id="s6"></a>
## 6. Dispatch

<a id="s6-1"></a>
### 6.1 Static calls (Confirmed, BIN + OBS)

0x16 carries the **body offset** itself. The compiler resolved the function at build time;
neither function IDs nor method tables are consulted. Every one of the 31,189 operands is a
body start (OBS).

<a id="s6-2"></a>
### 6.2 Virtual calls (BIN)

0x14 carries a **slot index** *v* of the virtual-function table. The receiver is the top cell
(the last argument pushed). The machine:

1. reads the receiver; null, or a handle beyond the client's object table, or an object
   without a class ends the run with status 3, PC left at the call;
2. takes `method_offsets[v]` of the receiver's class (`wlib.md` §6.1);
3. calls that body (§4.3).

The original does **not** check that *v* is inside the table or that the entry is not
0xFFFFFFFF; it then jumps to an invalid address. Tessera treats both as status 3 (§12). Slot
indices in the data reach 1,335.

<a id="s6-3"></a>
### 6.3 The slot-0 initializer

0x0C runs slot 0 of the new object's class with the new reference as receiver (§9.1). The
initializers take exactly the receiver (*A* = 8 in all 1,661 classes that have a slot-0
body) and return it
with 0x11, so the reference remains on the stack after the call (OBS). A slot-0 entry of
0xFFFFFFFF is not checked by 0x0C (BIN); no 0x0C in the data names such a class (OBS).

<a id="s6-4"></a>
### 6.4 Entry from native code: function IDs (BIN + OBS)

Native client code starts script functions on a machine it obtains; it pushes the arguments
(receiver last) and then asks by one of three forms:

| Form | Input | Body |
|---|---|---|
| Static by function ID | ID = class *P* × 65,536 + function number *f* | slot *v* = `method_indices[f]` of *P*; body = `method_offsets[v]` of *P* |
| Virtual by function ID | the receiver's class *C*, ID as above | *v* from *P* as above; body = `method_offsets[v]` of *C* |
| Virtual by slot | class *C*, slot *v* | `method_offsets[v]` of *C* |

This is `wlib.md` §6.4; the class part of the ID is masked to 15 bits. If *v* is −1 or
outside the table, or the body offset is 0xFFFFFFFF, the client logs a "missing function"
error, runs nothing and reports status 3. Otherwise it pushes a call record above the base
record and runs the machine (§4.3). With status 1 a value-returning function leaves its result
cells on the stack, where the caller reads them; the arguments are gone.

Call sites in the client's code (B14; BIN call-site pattern, OBS resolution):

| Form | Sites | Arguments at the sites |
|---|---|---|
| Static by function ID | 1,372 | 1,367 pass a constant ID, a different one at each site (1,367 distinct IDs); 5 pass an ID that is not a constant at the site (it comes from a parameter, an object field or a table entry) |
| Virtual by function ID | 3 | the class always comes from the receiver object; 1 site passes the constant ID 0x005B0001, 2 take the ID from a structure field |
| Virtual by slot | 1 | the class comes from an object; the slot is the constant 0 (the slot-0 initializer, §6.3) |

Of the 1,367 constant static IDs, 1,362 resolve to a body in this library (functions of 180
classes) and 5 do not (calling them gives the missing-function error). The constant virtual
ID 0x005B0001 is class 91, function 1: `method_indices[1]` of class 91 is slot 84, which none
of the 136 descendants of class 91 overrides, so a receiver of class 91 or any descendant runs
class 91's slot-84 body.
That body takes three arguments and the receiver and passes them on unchanged to virtual
slot 85 of the receiver, returning its one-cell result (OBS; see §10 for which slot-85
body each game-callback class runs).

Inside the bytecode, function IDs do not occur: script-to-script calls use offsets (0x16) or
slots (0x14).

<a id="s6-5"></a>
### 6.5 Run status (BIN)

| Status | Meaning |
|---|---|
| 1 | The entry function returned to the base record (§4.3). |
| 3 | Error: a checked failure of §4.4, §5.4, §6.2, an unresolved link (§7.3), or a native function that failed. |
| 0 | Returned by a native function: continue (§7.2). Not a run status. |
| other | Any non-zero status a native returns ends the run with that status. |

After a run, the client releases the machine for statuses 1 and 3 (logging the error for 3).
For any other status it **keeps** the machine with its registers, and a separate entry can
later **resume** it at its saved PC without pushing a record. The native-to-script call sites
that were read treat status 2 like 1 when releasing the machine, but read no result.

> **OPEN:** which code produces statuses other than 0, 1 and 3, and what resumes a kept
> machine. No registered native function was found to return them (a pattern scan of the
> native entry points found only 0 and 3), and the callers of the resume entry were not
> followed (BIN row). Tessera's interpreter supports statuses 0, 1 and 3 and treats any
> other native status as 3 until this is settled.

---

<a id="s7"></a>
## 7. The native-call boundary

<a id="s7-1"></a>
### 7.1 What leaves the script machine (BIN + OBS)

Only these instructions run client code outside the interpreter:

| Op | What runs outside | Sites |
|---|---|---|
| 0x15 | a registered native function | 29,985 |
| 0x0C | allocation of a script object (then the slot-0 body runs in the machine) | 554 |
| 0x0E | construction of a native object | 5,950 |
| 0x0D | loading of a prototype record through the object table | 5 |
| 0x0F | construction of a string object | 6,449 |

Everything else (arithmetic, attributes, frames, branches, script calls) stays inside.

**Native function ID** = package × 65,536 + function number, both 16-bit. In the data: 2,019
distinct IDs in 320 packages (function numbers up to 102). Every package number is an
undefined class-table entry (320 of 320), so packages share the native class-index space;
149 of them are among the client's 258 registered native classes (WS); the other 171 are
not registered as native classes. The client registers about 4,000 native functions; all 2,019 IDs used
by the bytecode are registered (BIN).

Packages with the most call sites (OBS; codec names from `wstate.md` §5.3 where it has one):

| Package | Call sites | Distinct functions | Registered native class? |
|---|---|---|---|
| 221 | 4,089 | 13 | no |
| 39 (`property`) | 3,115 | 75 | yes |
| 199 (`string_info`) | 2,691 | 22 | yes |
| 142 | 2,377 | 11 | yes |
| 182 (`refarray`) | 1,369 | 20 | yes |
| 231 | 1,074 | 8 | no |
| 166 (`property_set`) | 1,012 | 40 | yes |
| 183 | 919 | 6 | yes |
| 225 (`string`) | 840 | 21 | yes |
| 214 | 728 | 14 | no |

> **OPEN:** the behaviour of each native function. This spec fixes only how they are called;
> each package needs its own spec (or a Tessera definition) before script code that uses it
> can run. One function read as an example (package 166, function 0x16) looks up a property
> by property ID in the receiver's property set, writes the integer value through an address
> argument and returns 1 if it was found; this is given only to illustrate §7.2.

<a id="s7-2"></a>
### 7.2 Calling convention (BIN)

1. The caller pushes the arguments in order, the receiver (if any) last; *k* of the 0x15
   instruction is their total size in bytes (*A*; up to 120 in the data).
2. The machine stores the resume PC in the current record and pushes a record whose saved FP
   is the current SP and whose argument bytes are *A*; FP becomes SP. No frame entry runs.
3. The native function reads its arguments at FP − *A*/8 … FP − 1. Objects are passed as
   references; out-values are passed as addresses (§5.5).
4. On success it sets SP to FP − *A*/8, pushes its result cells (none, one or two, with their
   type codes), pops its call record (FP and PC from the previous record) and returns 0. The
   machine continues at the resume PC.
5. On failure it returns 3 (or another status, §6.5) and the run ends.

A native function may itself start script functions; it does so on another machine (§6.4).

<a id="s7-3"></a>
### 7.3 Link step (BIN)

After loading the library the client walks the code block with the length rule of §3.2 and
rewrites, in its in-memory copy, the operand word of:

- 0x0C: class index → an entry for the class (no entry if the index is beyond the class
  table or undefined, which makes the instruction fail with status 3 when it runs);
- 0x0E: native class index → an entry for the native class (status 3 when run if unknown);
- 0x15: native function ID → an entry for the function, or a stub that returns status 3 if
  the ID is not registered.

The walk takes 4 bytes for any opcode the length rule does not list, valid or not, and stops
at the first instruction start at or beyond *L*. It rejects nothing: an instruction or switch
table running past *L*, an invalid opcode and an out-of-range operand all pass, and the
operands above that cannot be resolved fail only when their instruction runs (B22).

The file holds the unresolved values listed in §3.3. Tessera may resolve these at load time
into its own tables; it must not depend on the client's rewritten form.

---

<a id="s8"></a>
## 8. Message strings (Confirmed, BIN + OBS)

- Chunk type 2's message list (`wlib.md` §4.2) is read by instruction **0x0F only** (the
  interpreter is the only reader of the loaded list found). 0x0F with message index *n* creates
  a **new** object of native class 225 (`string`, `wstate.md` §5.3) holding a copy of message
  *n* and pushes a reference to it. Each execution creates a new object.
- The client does not check *n* (BIN). In the data all 6,449 operands are below the message
  count, and every one of the 4,525 messages is referenced at least once (OBS).
- Census (OBS): 217,576 UTF-16 code units in all; the longest message has 3,040; one is
  empty. This spec does not reproduce or classify message texts.

---

<a id="s9"></a>
## 9. Objects, attributes and prototypes

<a id="s9-1"></a>
### 9.1 Creating objects (BIN + OBS)

- **0x0C** `new` *c*: allocates an instance of script class *c* whose slots are a copy of the
  class's slot table (defaults and type codes, `wlib.md` §6.1), registers it in the object
  table, pushes its reference and calls its slot-0 initializer (§6.3). It does **not** consult
  the singleton list (chunk 64, `wlib.md` §7). OBS: 554 sites name 270 distinct classes, all
  defined, all with a slot-0 body, none in chunk 64.
- **0x0E** `new.native` *n*: constructs a default object of native class *n* and pushes its
  reference. OBS: 5,950 sites, 102 distinct values, all undefined class-table entries; all are
  registered native classes (WS, BIN).

<a id="s9-2"></a>
### 9.2 Attributes (Confirmed, BIN + OBS)

- The 16-bit field of 0x4A–0x51 and 0x53 is a **slot index** of the class table, the first
  slot of the attribute for 64-bit forms. Attribute names exist only as hashes in the
  attribute lists (`wlib.md` §8); a tool maps a slot to its attribute by finding the list entry
  of the object's class whose slot range contains it.
- For `*.this` forms the class is any class whose table contains the body; all such classes
  agree on the slot's type code in all 18,323 bodies, because attribute lists are flattened
  and child slots extend the parent's (`wlib.md` §8.3) (OBS). Of the 7,329 `this` accesses,
  all lie inside the slot table of the class with the fewest slots among the body's owners,
  and every 64-bit form addresses a slot of type 130, 131 or 195 and every 32-bit form one of
  type 1, 2 or 3 (OBS).
- For `*.ref` forms the class is the referenced object's at run time; the instructions do not
  check that the object is a script object or that the slot exists (BIN).

<a id="s9-3"></a>
### 9.3 Prototype records and the singleton list (BIN + OBS)

0x0D *c* asks the client's object table for the game object with DID 0x76000000 + *c* (the
game-object DB type 0x45 range, `property-system.md` §9; only the low 16 bits of the operand
are used). It pushes a reference to the loaded object, or null if the object cannot be
loaded. The object table loads a 0x76 record only for a class in chunk 64 (`wlib.md` §7.2,
W29). OBS: 5 sites, all naming class 1603, which is in chunk 64.

Each execution decodes the record anew into newly allocated objects of the object table and
pushes the reference of the new root object, so two executions yield two distinct root
objects (BIN only, B24). The original does not look for a root object made by an earlier
execution. An interpreter loads a fresh object graph per execution (this agrees with
`wlib.md` §7.2, which says that the 0x76 record is decoded anew for each new object).

<a id="s9-4"></a>
### 9.4 References

References on the stack are handles of the client's object table, not the item indices of an
object record (`wstate.md` §5). The bytecode never contains a reference constant other than
null (0x09): references come from attributes, arguments, results and the creation
instructions above.

---

<a id="s10"></a>
## 10. How much behaviour lives in the code (Confirmed census, OBS)

Families, as selected from the data:

- **usage plugins**: root classes of the 0x70 objects that carry any `UsagePlugin_*`
  property;
- **game callbacks**: root classes of the objects that carry `GameCallback_ReturnValue`
  (`misc-records.md` §13.6);
- **property responses** / **response callbacks**: root classes of the objects with
  `PropertyResponse_*` / `PropertyResponseCallback_*` properties (`misc-records.md` §13.5);
- **effects, progressions, skills, traits**: the root classes of those families in
  `combat-data.md`;
- **modifiers**: classes 1339–1342 (`combat-data.md` §5.8, §13.2).

"Own bodies" are the table entries of a class that differ from its parent's entry at the same
slot or lie beyond the parent's table (overrides and new functions), merged over the family.
"Reachable" counts all distinct bodies in the families' tables, inherited ones included.

| Family | Classes | Objects | Own bodies | Own code (bytes) | Instructions | Native call sites (packages) | Distinct property-ID constants (§11.2) | Virtual / static calls | Reachable bodies |
|---|---|---|---|---|---|---|---|---|---|
| Game callbacks | 110 | 110 | 202 | 24,252 | 4,489 | 310 (19) | 80 | 26 / 660 | 263 |
| Usage plugins | 31 | 77 | 140 | 17,840 | 3,367 | 157 (14) | 45 | 127 / 392 | 202 |
| Property responses | 5 | 264 | 10 | 576 | 96 | 0 | 0 | 0 / 37 | 228 |
| Response callbacks | 3 | 269 | 6 | 324 | 54 | 0 | 0 | 0 / 21 | 63 |
| Effects | 87 | 32,971 | 515 | 62,988 | 12,140 | 617 (28) | 86 | 407 / 1,253 | 720 |
| Progressions | 6 | 7,558 | 36 | 2,012 | 361 | 25 (4) | 8 | 11 / 53 | 99 |
| Skills | 1 | 13,247 | 166 | 14,944 | 2,928 | 185 (14) | 80 | 73 / 166 | 223 |
| Traits | 9 | 3,963 | 112 | 8,120 | 1,519 | 93 (15) | 25 | 54 / 175 | 198 |
| Modifiers (1339–1342) | 4 | (embedded) | 78 | 19,348 | 3,620 | 334 (14) | 21 | 22 / 316 | 78 |
| Whole library | 2,129 | — | 18,323 | 1,919,904 | 362,178 | 29,985 (320) | 3,524 | 8,574 / 31,189 | 18,323 |

Further observations:

- Every class of every family has own bodies, and every class outside the modifiers
  overrides slot 0 (the initializer). Usage plugins, property responses, response callbacks,
  progressions, skills and traits also override slot 50 in every class; effects in 86 of 87.
- Game callbacks: all 110 classes descend from class 91 (parent map, `wlib.md` §9). 78 have
  class 91 as parent and override slot 85 themselves (5 of them also slots 86–88, or 86–87);
  27 have class 916 as parent and 5 have class 946, and these 32 override only slot 0.
  Classes 916 and 946 are themselves children of class 91 and each overrides slot 85 (and
  adds slot 86), so the 32 inherit their parent's slot-85 body. Class 91 has a slot-85 body of
  its own; none of the 110 runs it, every one runs a slot-85 body of its own class or of 916
  or 946. Usage plugins: 24 of the 31 classes have class 3227 as parent, 6 more have it as
  grandparent (4 through class 3603, 2 through class 3694), and the 31st is class 3227 itself; 30
  override slot 88. Which slot the client invokes for a callback or a usage plugin is not
  established (the script node of `script-table.md` that names game callbacks was not read).
- None of the 1,367 constant static function IDs of §6.4 names a class of these families or
  one of their ancestors. The one constant virtual ID, 0x005B0001, names class 91, an
  ancestor of all 110 game-callback classes: on a receiver of any of them it runs class 91's
  slot-84 body (no descendant overrides slot 84), which forwards to slot 85 of the receiver,
  that is, the class's own slot-85 body for the 78 children of class 91 and the inherited one
  of class 916 or 946 for the other 32 (§6.4). The
  other families' functions are not called by a fixed ID; they run from other script code
  (virtual slots) or through computed IDs.
- Property responses and response callbacks have almost no code of their own; their
  behaviour is in their ancestors' bodies (228 and 63 reachable).

> **OPEN:** the entry points (slot or function ID) through which the client starts a game
> callback, a usage plugin, a property response and an effect, skill or trait function. They
> are in native code outside the interpreter and were not part of this pass. For all 110
> game-callback classes (all descend from class 91) the virtual call of function ID
> 0x005B0001 (§6.4) reaches the slot-85 body each of them runs, but the native code around
> that call site was not read, so it is not established that the client starts game
> callbacks there, or only there.

---

<a id="s11"></a>
## 11. Reading the code (tools)

<a id="s11-1"></a>
### 11.1 Disassembly

1. Collect body starts from the method tables (§2); sort them.
2. For each body, decode instructions with §3 until the next body start.
3. Show, per instruction: offset, opcode name (§5), the type byte for 0x01–0x04, the 16-bit
   field with the signedness of §3.3, operand words, and branch or switch targets resolved
   to offsets (§3.4).
4. Annotate operands: 0x0C, 0x0D with the class (index, and the classes' known kind from
   `wstate.md` §11 if any); 0x0E with the native class (codec name from `wstate.md` §5.3);
   0x15 with package and function number; 0x16 with the target body and the classes whose
   tables hold it; 0x14 with the slot (the body depends on the receiver); 0x0F with the
   message index (an explorer may show the text from the user's install); attribute
   instructions with the attribute hash of §9.2.

<a id="s11-2"></a>
### 11.2 Cross-references

Exact relations (from operands): body → bodies it calls statically; body → slots it calls
virtually; body → native IDs; body → classes it creates; body → messages; body → attribute
slots of the receiver read and written.

Property IDs (heuristic): property IDs are pushed as 32-bit integer immediates (0x01 with
type 2) before the calls that take them. In the data 8,712 such immediates equal a defined
property ID (3,524 distinct IDs; `property-system.md`); no float immediate does. A tool marks
an immediate as a property reference only when its value is a defined property ID, and must
present it as probable, since an integer constant can coincide with an ID. The counts are
B19; they come from every 0x01 instruction of the block. 2,401 defined property IDs are below
0x8000 and so could also be pushed by 0x03; 4,406 short immediates of type 2 equal one of
them, but small integers coincide with these IDs too often for that to be useful, so a tool
does not mark 0x03 immediates (OBS).

> **OPEN:** which native functions take a property ID argument (and in which position).
> Once §7's native functions are specified, the property cross-reference can be made exact
> by following the immediate to the call that consumes it.

---

<a id="s12"></a>
## 12. Tessera rules (validation and limits)

The original trusts the bytecode. It validates nothing at load (B22): its link step (§7.3)
walks the block and rejects nothing; byte 1 of 0x01–0x04 is pushed as a type code whatever its
value, and byte 1 and unused bytes 2–3 of other instructions are ignored (B21); *L* and *A* are
used as byte counts without a check, so a value that is not a multiple of 8 puts SP off the
cell grid (B23); failures of V9, V10 and V12 surface only when the instruction runs (status 3,
§7.3) and failures of V8, V11 and V13 are not detected at all (§4.4). Every rule of this
section is therefore Tessera's own. Tessera treats the library as hostile input and adds these
rules; all of them hold for the shipped data (§13).

At load (a failure rejects the code block, the rest of the library stays usable). These are
invariants V1–V7, the multiple-of-8 part of V14, and the byte-1 rule:

- every method-table offset (other than 0xFFFFFFFF) is < *L*, a multiple of 4, and an
  instruction start under §3.2 when sweeping from 0; the sweep ends exactly at *L*;
- every opcode is valid (§3.1); counts *n* of switches fit the remaining bytes;
- every body starts with 0x10 and its last instruction is a return; *L*, *A* are multiples
  of 8;
- branch and switch targets are instruction starts within the body;
- static-call operands are body starts;
- frame indices of §5.3 and §5.5 lie in −*A*/8 … *L*/8 − 1 (two-cell forms: up to *L*/8 − 2);
- byte 1 is zero outside 0x01–0x04; in 0x01 and 0x03 it is 1, 2 or 3, in 0x02 and 0x04 it is
  130, 131 or 195 (any value type code of the pushed width, `wlib.md` §5; wider than the codes
  V4 observed, narrower than the original, which accepts any byte). Bytes 2–3 of an
  instruction without a 16-bit field are ignored, as in the original; a disassembler may show
  them when they are not zero.

Reported, not rejected (a tool lists them as findings; the library stays loaded):

- V8–V13 and the sharing rule of §2 (B18). An interpreter does not need them at load: the
  run-time checks below cover every case where they would matter, which matches where the
  original fails (V9, V10, V12 at run time) or improves on it (V8, V11, V13);
- the bounds of V14 (*L* ≤ 416, *A* ≤ 144). They are census values of this install, not
  limits; the value-stack check below bounds frame sizes at run time.

At run time (a failure ends the run with status 3):

- a value stack of 4,096 cells per machine, checked on every push and pop, including pops into
  the argument area; 512 call records (511 active functions);
- all checks of §4.4 and §5.4, plus: virtual slot inside the table and not 0xFFFFFFFF;
  0x0C class with a slot-0 body; attribute slot inside the object's slot table and object of
  a script class; message index in range; a zero divisor and the minimum value divided by −1
  (§5.12);
- an instruction budget per run (the original has none); its size is Tessera's choice and must
  be generous enough for the data.

Locals are filled at frame entry exactly as in §4.3 step 3: every local cell becomes
(0xCDCDCDCD, 0xCDCDCDCD), value and type code. Zero-filling is **not** allowed: one shipped
body reads a local before any store on some path and behaves differently with zero (§4.3,
B17, §13 V16).

Cross-checks (V15, V16) are optional analyses for tools, never load conditions. The original
performs neither. A tool may leave V15 out. A V16 analysis may bound its work, for example by
skipping bodies with more than 64 local cells (the shipped maximum is 52, §2), provided it
reports which bodies it skipped. Such a bound must not change the result for the shipped data:
the one body V16 names (offset 0x125B8) has 3 local cells.

The rules above answer the implementer note of 2026-10-08 (`crates/wvm`): its decoder's
load-time rejections, its reporting of V8–V13 and B18 as findings, its unenforced V14 bounds,
its V16 bound of 64 local cells and its omission of V15 are all within this section. None needs
to be stricter.

---

<a id="s13"></a>
## 13. Invariants (Confirmed on this install, OBS, 2026-10-07)

| # | Invariant |
|---|---|
| V1 | Sweeping the code block from 0 with §3.2 ends exactly at *L* (362,178 instructions). |
| V2 | The distinct method-table offsets (18,323) are exactly the offsets of the 0x10 instructions. |
| V3 | Each body ends with a return; all its returns have the same width. |
| V4 | Every opcode is valid; byte 1 is zero except in 0x01–0x04, where it is a type code (0x01: 2 or 3; 0x02: 130, 131 or 195; 0x03: 2; 0x04: 130). |
| V5 | All branch targets (24,929) and switch targets (1,323) are instruction starts inside the same body. |
| V6 | All static-call operands (31,189 sites, 9,651 bodies) are body starts. |
| V7 | All frame-cell accesses (134,547) lie inside the frame of their body. |
| V8 | All `this`-attribute accesses (7,329) fit the slot table with the right width (§9.2). |
| V9 | 0x0C operands are defined classes with a slot-0 body, not in chunk 64. |
| V10 | 0x0E operands are undefined class-table entries. |
| V11 | 0x0F operands are below the message count. |
| V12 | 0x15 packages are undefined class-table entries. |
| V13 | Slot-0 bodies declare *A* = 8. |
| V14 | *L* ≤ 416 and *A* ≤ 144, multiples of 8. |
| V15 | Stack depth: in 18,314 of the 18,323 bodies each instruction can be given one operand-stack depth (cells above FP + *L*/8) such that every instruction finds at least as many cells as it pops (an empty-area return of §4.3 excepted), and every branch target and fall-through agrees, except at a 0x13, where depths may differ (§4.3). The stack effects are those of §5 and §7.2; for 0x14 and 0x15, whose effect depends on the callee, some consistent choice exists. For the other 9 bodies the search was stopped by its time limit, not refuted. |
| V16 | Definite assignment of locals: follow every path from the start of each body (fall-through, branch targets of §3.4, every switch target; no edge out of a return) and treat a local cell (index 0 … *L*/8 − 1) as assigned once a store to it has run on **every** path to the current instruction. Stores are 0x48, 0x49, 0xA8–0xB5 and, conservatively, 0x52 (taking the cell's address, since a callee may write through it); for the two-cell forms both cells count. (0x46 and 0x47, the stores with a computed index, do not occur.) Reads are all frame-cell reads (0x43, 0x44, 0x9B–0xA7, 0x95–0x98) whose cells include a local; a two-cell read starting at FP − 1 (0xA3, or 0x44 with *k* = −1) reads local 0 as its second cell. (0xA3 and 0x44 with *k* = −1 do not occur, so they do not change the result.) Exactly one body has a read of a local that is not assigned on every path: offset 0x125B8, local 0, at 0x127D4 and 0x127E0 (§4.3). |

V1–V7, the multiple-of-8 part of V14 and the byte-1 rule of §12 are load-time checks; V8–V13,
the bounds of V14 and the sharing rule of §2 (B18) are reported findings (§12). V15 and V16 are
cross-checks; a Tessera tool may report bodies that fail them but must not reject the library
for them, and need not implement them (§12).

---

<a id="s14"></a>
## 14. Worked examples (synthetic)

All bytes below were made up for this spec. Words are shown as bytes in file order.

<a id="s14-1"></a>
### 14.1 A small function

A static function at offset 0x40 with two integer arguments a, b (pushed in that order) and
no locals. It returns a × b + 1 when a < b, else 0.

| Offset | Bytes | Instruction | Depth after |
|---|---|---|---|
| 0x40 | `10 00 00 00` `00 00 10 00` | `enter` *L* = 0, *A* = 16 | 0 |
| 0x48 | `9C 00 00 00` | `ld.frame` FP − 2 (a) | 1 |
| 0x4C | `45 00 00 00` | `ld.frame` FP − 1 (b) | 2 |
| 0x50 | `C8 00 14 00` | continue if a < b, else branch to 0x50 + 4 + 0x14 = 0x68 | 0 |
| 0x54 | `9C 00 00 00` | a | 1 |
| 0x58 | `45 00 00 00` | b | 2 |
| 0x5C | `1B 00 00 00` | a × b | 1 |
| 0x60 | `23 00 00 00` | + 1 | 1 |
| 0x64 | `11 00 00 00` | `ret` | — |
| 0x68 | `05 00 00 00` | `push.int 0` | 1 |
| 0x6C | `11 00 00 00` | `ret` | — |

A caller with SP = 20 pushes (3, int) at cell 20 and (5, int) at cell 21 and executes `16 00 00
00 40 00 00 00`. In the callee FP = 22, the arguments are cells 20 and 21. 3 < 5 holds, so the
code computes 16 and returns: SP becomes 22 − 2 = 20, cell 20 becomes (16, int), SP = 21, and
the caller continues after its call with one result cell where its two arguments were. With
a = 5, b = 3 the branch is taken and the result is (0, int).

<a id="s14-2"></a>
### 14.2 A 64-bit immediate

`02 82 00 00` `01 00 00 00` `02 00 00 00` pushes 0x0000000100000002 as a 64-bit integer: the
lower cell is (0x00000002, 130), the upper cell (0x00000001, 130). `04 82 FF FF` pushes −1 as
a 64-bit integer: both cells (0xFFFFFFFF, 130).

<a id="s14-3"></a>
### 14.3 A switch

At offset 0x100: `57 00 00 00` `10 00 00 00` `02 00 00 00` `0A 00 00 00` `00 00 00 00`
`14 00 00 00` `08 00 00 00`. Length 12 + 2 × 8 = 28, so *E* = 0x11C. Key 10 → 0x11C + 0 =
0x11C; key 20 → 0x11C + 8 = 0x124; any other key → 0x11C + 0x10 = 0x12C.

<a id="s14-4"></a>
### 14.4 A native call with an out-value

A method of the receiver *r* calls native function 0x01230004 (package 0x0123, function 4)
with an address of its local 0 and an integer 7:

| Bytes | Instruction | Stack after (top last) |
|---|---|---|
| `52 00 00 00` | `addr.frame` FP + 0 | (null, *s*) where *s* = FP + 0 as an absolute index |
| `03 02 07 00` | `push.short` 7, int | …, (7, int) |
| `45 00 00 00` | receiver | …, (*r*, ref) |
| `15 00 20 00` `04 00 23 01` | `call.native`, *k* = 32 bytes (4 cells), ID 0x01230004 | the native's result, e.g. (1, int) |

Inside the native, FP − 4 and FP − 3 are the address, FP − 2 is 7, FP − 1 is the receiver. It
may write a cell through the address into the caller's local 0, sets SP to FP − 4, pushes its
result, pops its record and returns 0.

<a id="s14-5"></a>
### 14.5 Resolving a function ID

Native code asks to run function ID 0x01230002 virtually on an object of class 0x0200. Class
*P* = 0x0123, function 2. If `method_indices[2]` of class 0x0123 is 5, the body is
`method_offsets[5]` of class 0x0200; if that entry is 0xFFFFFFFF, or class 0x0200's table has
fewer than 6 entries, the result is the missing-function error (status 3).

---

<a id="s15"></a>
## 15. Open questions

> **OPEN:** behaviour of the 2,019 native functions the bytecode calls (§7.1). Required
> before script code that uses them can run; one spec per package is the expected shape.

> **OPEN:** statuses other than 0, 1 and 3, and the resume path (§6.5).

> **OPEN:** entry points through which the client starts game callbacks, usage plugins,
> property responses and family functions (§10; for the game callbacks, all 110 of which
> descend from class 91, narrowed to the constant virtual ID of §6.4, whose call site's
> purpose is not established).

> **OPEN:** what the original does after a processor fault in the interpreter (integer
> division by zero, minimum ÷ −1) and whether float division by zero faults in its
> floating-point environment (§5.12). Narrowed 2026-10-08: the script machine itself neither
> handles the fault nor sets the floating-point state (B25); the answer lies in the frames
> outside it. Tessera's rule is status 3.

> **OPEN:** which native functions consume property IDs (§11.2).

> **OPEN:** the meaning of the second part of chunk type 2 (the arrays after the messages,
> `wlib.md` §4.2); it is empty in this install and no reader of it was traced.

---

<a id="s16"></a>
## 16. Notes for other specs

- `wlib.md` §4.1 ("the instruction set is not specified") can point here; §6.4 is unchanged.
  Its W29 says that a script creating an object of a chunk-64 class starts from the 0x76
  record. Instruction 0x0C does not consult chunk 64 (§9.1); the data never use 0x0C on such a
  class. The path W29 describes is therefore outside the interpreter (a native function or the
  object table); the two statements do not conflict for the shipped data.
- `combat-data.md` §13.2: with the instruction set known, property IDs can be counted per
  instruction instead of by byte search: 3,524 distinct defined property IDs occur as integer
  immediates (§11.2, B19), against 5,993 found by the byte search (B19 reproduces that count), which also matches operand words
  of other instructions and bytes that are not instruction-aligned. Its OPEN on the bytecode's
  progression and modifier code can now be pursued with §5–§7; the native functions those
  bodies call (§10: 25 native call sites in the progression bodies, 334 in the modifier
  bodies) still need specifying.

---

<a id="s17"></a>
## 17. Change log

| Date | Change |
|---|---|
| 2026-10-07 | First version (status Review). M1 census of the whole code block and M4 reading (BIN row) of the interpreter, the native-to-script entry routines, the link step and the native calling convention. Facts B1–B16. 15 invariants, 5 worked examples, 7 OPEN items. |
| 2026-10-07 | Revision after the first review (changes requested), status stays Review; spec writer in a separate session, with new probes over every record and a short second BIN reading (BIN row). F1: §12 no longer allows zero-filled locals; §4.3 names the one body that reads a local before any store (offset 0x125B8) and what changes with zero; new B17 and invariant V16 (definite-assignment rule). F2: §2 shared-body statement corrected (sharing comes only from inheritance; 1,428 bodies have sibling owners; the "2,140 unrelated, identical bodies" claim withdrawn); new B18. F3: call sites re-counted: 1,376 sites of the three entry routines (1,372 static, 3 virtual by ID, 1 by slot); 2 static sites the first pattern missed carry constant IDs, so 1,367 constant static IDs (1,362 resolve), 5 computed; one virtual site carries the constant ID 0x005B0001 (class 91, slot 84, forwarding to slot 85) and the by-slot site uses slot 0. BIN row, B14, §6.4 (new table), §10 bullet and OPEN, §15 updated. F4: new B19 for the property-ID counts of §11.2 and §16, plus the 0x03 observation in §11.2. F5: §5.9 states the IEEE zero test of 0x73–0x78 (−0.0 is zero, NaN is true). |
| 2026-10-07 | Revision after the second review (changes requested), status stays Review; spec writer in a separate session, own probe over the parent map and all method tables (no BIN reading). R1: §10 now says all 110 game-callback classes descend from class 91 (78 children, 27 through class 916, 5 through class 946, which are children of 91 and override slot 85), so the 0x005B0001 virtual call reaches a slot-85 body for all 110; the §10 OPEN and the third §15 OPEN narrowed to "not established that the client starts callbacks there". R2: usage plugins: 24 have class 3227 as parent, 6 as grandparent, the 31st is class 3227. R3: V16 counts every frame-cell read whose cells include a local (0xA3 and 0x44 with *k* = −1 read local 0 as their second cell; neither occurs). New B20. |
| 2026-10-07 | Third review (separate session, not the author): passed; status Accepted. No content change. |
| 2026-10-08 | Revision (status Revised; spec writer in a separate session). Answers the implementer note of §12 (`crates/wvm`): a third BIN reading (BIN row; disassembler on the read-only executable, no analysis-database copy) and a re-check of the frame sizes over every body. New B21 (byte 1 read only by 0x01–0x04, pushed unchanged; unused fields ignored), B22 (no load-time validation; link-step walk), B23 (*L*, *A* are unchecked byte counts), B24 (0x0D decodes a fresh object graph per execution), B25 (no exception filter in the script machine; no floating-point state change). §1: labels for unnamed opcodes are the tool's choice; the example names now match §5. §3.1, §7.3: client behaviour from B21, B22. §12: what the original checks; load-time rules now list the byte-1 rule; V8–V13, B18 and the V14 bounds are reported findings; V15 optional, V16 may be bounded if skips are reported; the implementer OPEN is closed (its four choices stand as Tessera rules). §13: closing sentence matches §12 (also the third review's note N1). §9.3 OPEN closed (fresh object per execution, B24); §5.12 and its §15 OPEN narrowed (B25). |
