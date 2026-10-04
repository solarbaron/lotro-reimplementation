// SPDX-License-Identifier: GPL-3.0-or-later
//
// ImHex pattern for the .dat/.datx archive container: file header, directory B-tree node
// and free-space table. Written only from docs/specs/dat-container.md (section numbers
// below refer to it). It is a second, independent rendering of that spec; where it
// disagrees with crates/dat, one of them or the spec is wrong.
//
// Use on your own install only, read-only. Patterns, highlights and bookmarks made on real
// archives are dirty-room material: keep them out of the repository.
//
// Variables you can set in ImHex's "Settings" tab of the pattern editor:
//   inspect_node   absolute offset of another directory node to decode (copy it from a
//                  child pointer of the root); 0 = off
//   show_free      decode the free-space table (§4.5)

#pragma description Archive container header + directory node (docs/specs/dat-container.md)
#pragma endian little
#pragma pattern_limit 1000000

import std.io;
import std.sys;
import type.time;

namespace tessera {

    // §3.4: byte 0x14F.
    enum DatasetType : u8 {
        Portal = 1,
        CellOrMap = 2,
        Local = 3,
    };

    // §3.4: 0x14C read as four bytes.
    struct DatasetDescriptor {
        u8 zero0 [[comment("always 0")]];
        u8 zero1 [[comment("always 0")]];
        u8 kind [[comment("archive kind within the dataset type (§3.4 table)")]];
        DatasetType dataset_type;
    };

    // §3.3: 0x140 .. 0x1A8.
    struct Superblock {
        u32 magic [[comment("must be 0x00005442")]];
        std::assert(magic == 0x5442, "bad magic at 0x140: not an archive (§3.3, I-2)");
        u32 min_alloc [[comment("minimum allocation size, \"block size\" (§4.4)")]];
        u32 file_size [[comment("equals the real file length (I-3)")]];
        DatasetDescriptor dataset;
        u32 subset [[comment("region number for cell/map, 2 for local, else 0")]];
        u32 free_table_offset [[comment("offset of the free-space table allocation (§4.5)")]];
        u32 unused_158 [[comment("0 in all observed archives")]];
        u32 free_count [[comment("number of {size, offset} pairs in the free-space table")]];
        u32 root_offset [[comment("offset of the root directory node allocation")]];
        u32 unused_164[3] [[comment("0 in all observed archives")]];
        u32 master_map_did [[format("tessera::fmt_did"), comment("non-zero only in the general archive")]];
        u32 engine_version [[comment("112 observed; 111 = legacy layout (§4.3)")]];
        u32 game_data_version [[comment("3 portal/local, 0 cell/map")]];
        u8 identifier[16] [[comment("GUID-like identifier (meaning OPEN)")]];
        u32 minor_version [[comment("meaning OPEN")]];
        u32 superblock_len [[comment("104 observed (= 0x1A8 - 0x140)")]];
        u32 entry_size [[comment("32 observed; nodes cannot be parsed otherwise (I-6)")]];
        u32 pointer_size [[comment("8 observed")]];
        u32 free_bytes [[comment("sum over free extents of (size - 8)")]];
        u32 free_table_size [[comment("size of the free-space table allocation")]];
        u32 root_size [[comment("size of the root node allocation (>= 2460)")]];
        std::assert_warn(entry_size == 32, "0x194 is not 32: directory entries will be misread (I-6)");
    };

    // §3: 0x000 .. 0x400.
    struct Header {
        char version[256] [[comment("NUL-padded version string; empty in observed archives")]];
        u8 journal[64] [[comment("empty journal = 00 50 4C 00 then zeros (§3.2, I-7)")]];
        Superblock superblock;
        padding[0x400 - 0x1A8];
    };

    // §4.1: 8 bytes in front of nodes, record first extents and the free-space table.
    struct AllocHeader {
        u32 frag_count [[comment("extra fragments; their {length, offset} table is at the END of this allocation (§4.2)")]];
        u32 legacy_word [[comment("0 = modern layout; non-zero = legacy chained layout (§4.3)")]];
    };

    fn fmt_did(u32 did) {
        return std::format("0x{:08X}", did);
    };

    // §5.1: child pointer {size, offset}; {0, 0} = none.
    struct ChildPtr {
        u32 size;
        u32 offset;
    } [[format("tessera::fmt_child")]];

    fn fmt_child(ChildPtr p) {
        if (p.size == 0 && p.offset == 0)
            return "none";
        return std::format("{} bytes @ 0x{:08X}", p.size, p.offset);
    };

    // §5.3: storage flags.
    bitfield EntryFlags {
        compressed : 1 [[comment("payload = u32 inflated length + zlib stream (§6.1)")]];
        bit1 : 1 [[comment("always set in observed data, meaning OPEN")]];
        unused : 14 [[comment("always 0 (I-17)")]];
    };

    // §5.2: 32-byte directory entry.
    struct Entry {
        EntryFlags flags;
        u16 stamp [[comment("small per-record version number; not needed for reading")]];
        u32 did [[format("tessera::fmt_did"), comment("data identifier, the B-tree key")]];
        u32 offset [[comment("absolute offset of the record's first extent (its AllocHeader)")]];
        u32 length [[comment("stored payload length, incl. the 4-byte prefix if compressed")]];
        type::time32_t time [[comment("last write, Unix seconds UTC")]];
        u32 iteration [[comment("per-record revision counter")]];
        u32 alloc_size [[comment("size of the FIRST extent only")]];
        u32 reserved [[comment("0 (I-16)")]];
    } [[format("tessera::fmt_entry")]];

    fn fmt_entry(Entry e) {
        return std::format("{} len {} @ 0x{:08X}", tessera::fmt_did(e.did), e.length, e.offset);
    };

    // §5.1: directory node = 8-byte allocation header + 2452-byte payload.
    // A leaf has child pointer 0 = {0, 0}. Only children 0..n and entries 0..n-1 are
    // valid; the rest of both arrays is stale data and must be ignored.
    struct Node {
        AllocHeader alloc;
        std::assert_warn(alloc.frag_count == 0 && alloc.legacy_word == 0, "node allocation header is not {0, 0} (I-9)");
        ChildPtr children[62];
        u32 count [[comment("number of valid entries, 1..61")]];
        std::assert(count <= 61, "entry count above 61: corrupt node or wrong offset (I-10)");
        Entry entries[count];
        padding[(61 - count) * 32];
    } [[format("tessera::fmt_node")]];

    fn fmt_node(Node n) {
        if (n.children[0].size == 0 && n.children[0].offset == 0)
            return std::format("leaf, {} entries", n.count);
        return std::format("internal, {} entries, {} children", n.count, n.count + 1);
    };

    // §4.5: free-space table allocation.
    struct FreeExtent {
        u32 size;
        u32 offset;
    } [[format("tessera::fmt_child")]];

    struct FreeTable<auto Count> {
        AllocHeader alloc;
        FreeExtent extents[Count];
    };

}

u32 inspect_node in;
bool show_free in;

tessera::Header header @ 0x00;
tessera::Node root @ header.superblock.root_offset;

// Optional views. They sit in a zero-sized struct because placements inside a top-level
// `if` block are not kept as patterns.
struct Extras {
    if (inspect_node != 0)
        tessera::Node node @ inspect_node;
    if (show_free && header.superblock.free_table_offset != 0)
        tessera::FreeTable<header.superblock.free_count> free_table @ header.superblock.free_table_offset;
};

Extras extras @ 0x00;
