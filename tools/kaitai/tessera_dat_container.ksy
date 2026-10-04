# SPDX-License-Identifier: GPL-3.0-or-later
#
# Kaitai Struct description of the .dat/.datx archive container: file header, directory
# B-tree (followed lazily through child pointers), entries and the free-space table.
# Written only from docs/specs/dat-container.md; section numbers refer to it. It is an
# independent rendering of the spec for cross-checking crates/dat, not a reader to ship.
#
# Use on your own install only, read-only (Web IDE: https://ide.kaitai.io runs locally in
# the browser). Never commit anything produced from real archives.
#
# Compile check: kaitai-struct-compiler -t python --outdir /tmp/ksy-out tools/kaitai/tessera_dat_container.ksy
meta:
  id: tessera_dat_container
  title: Archive container (.dat / .datx), header and directory B-tree
  file-extension:
    - dat
    - datx
  license: GPL-3.0-or-later
  ks-version: "0.10"
  endian: le
doc: |
  Read-only view of the data archive container described in
  docs/specs/dat-container.md. Integers are little-endian u32 unless noted.
  Values observed in one install are documented, never assumed: sizes always come
  from the pointer, entry or header field that refers to an allocation (§4).
doc-ref: docs/specs/dat-container.md

seq:
  - id: version
    type: strz
    size: 256
    encoding: ASCII
    doc: NUL-padded version string; empty in all observed archives (§3.1).
  - id: journal
    size: 64
    doc: Transaction journal; empty pattern is 00 50 4C 00 followed by zeros (§3.2, I-7).
  - id: superblock
    type: superblock
  - id: header_padding
    size: 0x400 - 0x1a8
    doc: Zero padding up to the first allocation at 0x400 (§3.3).

instances:
  root:
    pos: superblock.root_offset
    size: superblock.root_size
    type: node
    doc: Root directory node (§5.1); its size comes from 0x1A4, never assumed to be 2460.
  free_table:
    pos: superblock.free_table_offset
    size: superblock.free_table_size
    type: free_table(superblock.free_count)
    if: superblock.free_table_offset != 0 and superblock.free_table_size >= 8 + 8 * superblock.free_count
    doc: Free-space table (§4.5).

types:
  superblock:
    doc: File header fields at 0x140..0x1A8 (§3.3).
    seq:
      - id: magic
        type: u4
        valid: 0x5442
        doc: Archive magic 0x00005442 (I-2).
      - id: min_alloc
        type: u4
        doc: Minimum allocation size, "block size" (§4.4); must be > 0 (I-4).
      - id: file_size
        type: u4
        doc: Equals the real file length (I-3).
      - id: dataset_zero
        size: 2
        doc: Bytes 0x14C..0x14D, always 0 (§3.4).
      - id: archive_kind
        type: u1
        doc: Byte 0x14E, archive kind within the dataset type (§3.4 table).
      - id: dataset_type
        type: u1
        enum: dataset_type
        doc: Byte 0x14F (§3.4).
      - id: subset
        type: u4
        doc: Region number for cell/map archives, 2 for local, otherwise 0.
      - id: free_table_offset
        type: u4
      - id: unused_158
        type: u4
        doc: 0 in all observed archives.
      - id: free_count
        type: u4
        doc: Number of {size, offset} pairs in the free-space table.
      - id: root_offset
        type: u4
      - id: unused_164
        type: u4
        repeat: expr
        repeat-expr: 3
        doc: 0 in all observed archives.
      - id: master_map_did
        type: u4
        doc: Non-zero only in the general archive (meaning reported, not confirmed).
      - id: engine_version
        type: u4
        doc: 112 observed; 111 is reported to mean the legacy layout (§4.3, I-5).
      - id: game_data_version
        type: u4
      - id: identifier
        size: 16
        doc: GUID-like identifier (meaning OPEN).
      - id: minor_version
        type: u4
      - id: superblock_len
        type: u4
        doc: 104 observed (I-6).
      - id: entry_size
        type: u4
        valid: 32
        doc: Directory entry size; nodes cannot be decoded if this is not 32 (I-6).
      - id: pointer_size
        type: u4
        doc: 8 observed (I-6).
      - id: free_bytes
        type: u4
        doc: Sum over free extents of (size - 8) (I-22).
      - id: free_table_size
        type: u4
      - id: root_size
        type: u4
        doc: Root node allocation size, >= 2460 (I-8).

  alloc_header:
    doc: |
      8-byte header of directory nodes, record first extents and the free-space table
      (§4.1). Fragments and free extents have none.
    seq:
      - id: frag_count
        type: u4
        doc: |
          Number of extra fragments. Their {length, offset} table is the LAST
          8 * frag_count bytes of the first extent (§4.2).
      - id: legacy_word
        type: u4
        doc: 0 in the modern layout; non-zero selects the legacy chained layout (§4.3).
    instances:
      is_legacy:
        value: legacy_word != 0

  node:
    doc: |
      Directory node (§5.1): 8-byte allocation header, then a 2452-byte payload of
      62 child pointers, the entry count and 61 entry slots. Only children 0..num_entries
      and entries 0..num_entries-1 are valid; the remaining slots hold stale data (§5.1).
    seq:
      - id: alloc
        type: alloc_header
      - id: children
        type: child_ptr(_index)
        repeat: expr
        repeat-expr: 62
      - id: num_entries
        type: u4
        valid:
          max: 61
        doc: Number of valid entries (I-10).
      - id: entries
        type: entry
        repeat: expr
        repeat-expr: num_entries
      - id: stale_entries
        size: (61 - num_entries) * 32
        doc: Unused entry slots; ignore.
    instances:
      is_leaf:
        value: children[0].len_node == 0 and children[0].offset == 0
        doc: A node is a leaf iff child pointer 0 is {0, 0} (§5.1, I-11).

  child_ptr:
    params:
      - id: index
        type: u4
    seq:
      - id: len_node
        type: u4
        doc: Allocation size of the child node.
      - id: offset
        type: u4
        doc: Absolute file offset of the child node.
    instances:
      is_valid:
        value: index <= _parent.num_entries and offset != 0 and len_node >= 2460
        doc: Pointers past count are stale; valid ones are non-zero and >= 2460 bytes (I-11, I-12).
      node:
        io: _root._io
        pos: offset
        size: len_node
        type: node
        if: is_valid
        doc: The child node, parsed lazily (in-order traversal, §5.6).

  entry:
    doc: 32-byte directory entry (§5.2).
    seq:
      - id: flags
        type: u2
        doc: Bit 0 = zlib-compressed; bit 1 always set in observed data (§5.3).
      - id: stamp
        type: u2
        doc: Small per-record version number; not needed for reading.
      - id: did
        type: u4
        doc: Data identifier, the B-tree key.
      - id: offset
        type: u4
        doc: Absolute offset of the record's first extent (its alloc_header).
      - id: length
        type: u4
        doc: Stored payload length; includes the 4-byte inflated-length prefix if compressed.
      - id: time
        type: u4
        doc: Last write, Unix seconds UTC.
      - id: iteration
        type: u4
        doc: Per-record revision counter.
      - id: alloc_size
        type: u4
        doc: Size of the FIRST extent allocation only (smaller than length + 8 when fragmented).
      - id: reserved
        type: u4
        doc: Always 0 (I-16).
    instances:
      is_compressed:
        value: (flags & 1) == 1
      is_metadata:
        value: did >= 0xffff0000
        doc: Archive metadata records 0xFFFF00xx (§7).
      first_extent_header:
        io: _root._io
        pos: offset
        type: alloc_header
        if: offset >= 0x400
        doc: Allocation header of the record's first extent (§4.1).

  free_table:
    params:
      - id: num_extents
        type: u4
    seq:
      - id: alloc
        type: alloc_header
      - id: extents
        type: free_extent
        repeat: expr
        repeat-expr: num_extents

  free_extent:
    doc: Free extent; raw stale bytes with no header of their own (§4.5).
    seq:
      - id: size
        type: u4
      - id: offset
        type: u4

enums:
  dataset_type:
    1: portal
    2: cell_or_map
    3: local
