// SPDX-License-Identifier: GPL-3.0-or-later
//! Official world-session message layer (`docs/specs/official-messages-world.md`): the
//! compact wire value encodings (§2), the character list, creation, deletion and selection
//! (§3), world entry, portal space and the network and world clocks (§4), and object
//! lifecycle, movement, actions and property updates (§5).
//!
//! # Runtime data
//!
//! Wire decoding is dictionary driven (Provenance, "runtime PID resolution rule"):
//! property types come from a [`PropertySchemaLookup`] that the caller builds from
//! `client_gamelogic.dat` record `0x34000000`, the `0xCC` state schemas from a
//! [`PortalStateSchemaLookup`] built from `0x07000029` and its `0x0C` scriptlets (§4.3.1),
//! and the calendar from record `0x35000000` ([`WorldCalendarConfig::from_bytes`], §4.4.2).
//! No schema or calendar table is built in; the numeric `PID` constants below are the
//! protocol constants the adapter of §6 matches (decision 0012, decision 0034 §2 item 4).
//! Their names are the spec's descriptive mnemonics, not property names from the archive.
//!
//! # Hostile input
//!
//! Counts are bounded against the remaining bytes before allocating, `bool8` values
//! strictly require `0` or `1`, recursive structures enforce [`MAX_RECURSION_DEPTH`]
//! (`16`), and layouts the spec leaves `OPEN` are refused rather than guessed (§7.1).

use std::collections::{BTreeMap, HashMap};

use crate::session::{MessageRoutingHeader, SessionError, decode_message};

/// Maximum nesting depth for nested `EncodedPropertyTable` (Type 11 `Struct`, Type 12
/// `Array`) and nested `LocalizedTokenText` (argument kind 1) values (§7.1 item 2).
pub const MAX_RECURSION_DEPTH: usize = 16;

/// Most entries a decoder reserves room for before it has read them. Counts are checked
/// against the bytes that remain, but nested tables each check against the same remaining
/// bytes, so reserving the full count at every level could reserve far more memory than the
/// message holds. Vectors start at most this large and grow as entries are read
/// (engineering choice, spec silent).
pub const MAX_PREALLOC_ENTRIES: usize = 64;

/// Type 4 (`Timestamp`) "no time" value (`3.4028234663852886e38`, §2.7.1).
pub const TIMESTAMP_NO_TIME: f64 = 3.402_823_466_385_288_6e38;

/// Key of the in-game calendar record in its directory (`0x96` = `150`, §4.4.2). The record
/// DID is the one this key resolves to at run time (`0x35000000` in the install the spec
/// describes, a verification value); [`WorldCalendarConfig::decode`] is given it.
pub const CALENDAR_DIRECTORY_KEY: u32 = 0x0000_0096;

/// `state_schema_id` of `0xCC` when `sync_flags` bit 7 (`0x80`) is clear (`0x0000000C`,
/// the surface-movement player-physics script, §4.3.1).
pub const DEFAULT_PORTAL_STATE_SCHEMA_ID: u32 = 0x0000_000C;

// ---------------------------------------------------------------------------
// Protocol PID constants (§3.1.3, §3.2.5, §4.4.2, §5.2.4)
// ---------------------------------------------------------------------------

/// `character_type` (`0x0000062F`, Enum, §3.1.3, §3.2.5).
pub const PID_CHARACTER_TYPE: u32 = 0x0000_062F;
/// `agent_species` (`0x100000FC`, Enum, §3.1.3, §3.2.5, §5.2.4).
pub const PID_AGENT_SPECIES: u32 = 0x1000_00FC;
/// `agent_subspecies` (`0x10001461`, Enum, §3.1.3, §5.2.4).
pub const PID_AGENT_SUBSPECIES: u32 = 0x1000_1461;
/// `agent_genus` (`0x10000CBE`, Bitfield32, §3.1.3, §5.2.4).
pub const PID_AGENT_GENUS: u32 = 0x1000_0CBE;
/// `agent_alignment` (`0x1000069B`, Enum, §3.1.3, §5.2.4).
pub const PID_AGENT_ALIGNMENT: u32 = 0x1000_069B;
/// `agent_class` (`0x10001089`, Enum, §3.1.3, §3.2.5, §5.2.4).
pub const PID_AGENT_CLASS: u32 = 0x1000_1089;
/// `sex` (`0x10000A50`, Enum, §3.1.3, §3.2.5, §5.2.4).
pub const PID_SEX: u32 = 0x1000_0A50;
/// `nationality` (`0x10001190`, Enum, §3.1.3, §3.2.5, §5.2.4).
pub const PID_NATIONALITY: u32 = 0x1000_1190;
/// `advancement_level` (`0x10001011`, Int, §3.1.3, §5.2.4).
pub const PID_ADVANCEMENT_LEVEL: u32 = 0x1000_1011;
/// `chargen_character_level` (`0x1000663B`, Int, §3.1.3).
pub const PID_CHARGEN_CHARACTER_LEVEL: u32 = 0x1000_663B;
/// `chargen_character_class_text` (`0x10006647`, LocalizedTokenText, §3.1.3).
pub const PID_CHARGEN_CHARACTER_CLASS_TEXT: u32 = 0x1000_6647;
/// `chargen_character_last_update_time` (`0x1000664B`, Timestamp, §3.1.3).
pub const PID_CHARGEN_CHARACTER_LAST_UPDATE_TIME: u32 = 0x1000_664B;
/// `chargen_character_has_data` (`0x10003884`, Boolean, §3.1.3).
pub const PID_CHARGEN_CHARACTER_HAS_DATA: u32 = 0x1000_3884;
/// `chargen_character_availability_error` (`0x10005F96`, Enum, §3.1.3).
pub const PID_CHARGEN_CHARACTER_AVAILABILITY_ERROR: u32 = 0x1000_5F96;
/// `area_current_area` (`0x10000983`, DID reference, §3.1.3).
pub const PID_AREA_CURRENT_AREA: u32 = 0x1000_0983;
/// `area_territory` (`0x100011A1`, DID reference, §3.1.3).
pub const PID_AREA_TERRITORY: u32 = 0x1000_11A1;
/// `area_region` (`0x10001494`, DID reference, §3.1.3).
pub const PID_AREA_REGION: u32 = 0x1000_1494;
/// `area_region_id` (`0x100003FE`, Enum, §3.1.3).
pub const PID_AREA_REGION_ID: u32 = 0x1000_03FE;
/// `area_name` (`0x00000198`, LocalizedTokenText, §3.1.3).
pub const PID_AREA_NAME: u32 = 0x0000_0198;
/// `physics_current_position` (`0x0000001B`, Position, §3.1.3).
pub const PID_PHYSICS_CURRENT_POSITION: u32 = 0x0000_001B;
/// `physics_last_valid_position` (`0x000005A3`, Position, §3.1.3).
pub const PID_PHYSICS_LAST_VALID_POSITION: u32 = 0x0000_05A3;
/// `block_x` (`0x00000090`, Int, §3.1.3).
pub const PID_BLOCK_X: u32 = 0x0000_0090;
/// `surname` (`0x100010F8`, LocalizedTokenText, §3.1.3, §5.2.4).
pub const PID_SURNAME: u32 = 0x1000_10F8;
/// `title_active_title_did` (`0x1000088B`, DID reference, §3.1.3, §5.2.4).
pub const PID_TITLE_ACTIVE_TITLE_DID: u32 = 0x1000_088B;
/// `title_active_title_string` (`0x1000119C`, LocalizedTokenText, §3.1.3, §5.2.4).
pub const PID_TITLE_ACTIVE_TITLE_STRING: u32 = 0x1000_119C;
/// `phys_obj` (`0x0000047F`, DID reference, §3.1.3, §3.2.5, §5.2.4).
pub const PID_PHYS_OBJ: u32 = 0x0000_047F;
/// `appearance_instance_list` (`0x0000054F`, Array, §3.1.3, §3.2.5, §5.2.4).
pub const PID_APPEARANCE_INSTANCE_LIST: u32 = 0x0000_054F;
/// `class_appearance_list` (`0x000006C3`, Array, §5.2.4).
pub const PID_CLASS_APPEARANCE_LIST: u32 = 0x0000_06C3;
/// `account_game_account_name` (`0x00000747`, LocalizedTokenText, §3.1.3).
pub const PID_ACCOUNT_GAME_ACCOUNT_NAME: u32 = 0x0000_0747;
/// `account_billing_name` (`0x000003F7`, LocalizedTokenText, §3.1.3).
pub const PID_ACCOUNT_BILLING_NAME: u32 = 0x0000_03F7;
/// `account_auth_ticket` (`0x0000018A`, String, §3.1.3).
pub const PID_ACCOUNT_AUTH_TICKET: u32 = 0x0000_018A;
/// `account_iid` (`0x100038A6`, InstanceId, §3.1.3).
pub const PID_ACCOUNT_IID: u32 = 0x1000_38A6;
/// `billing_player_account_type` (`0x10003866`, Enum, §3.1.3).
pub const PID_BILLING_PLAYER_ACCOUNT_TYPE: u32 = 0x1000_3866;
/// `account_billing_roles` (`0x000001D6`, Bitfield, §3.1.3).
pub const PID_ACCOUNT_BILLING_ROLES: u32 = 0x0000_01D6;
/// `account_num_characters_allowed` (`0x00000499`, Int, §3.1.3).
pub const PID_ACCOUNT_NUM_CHARACTERS_ALLOWED: u32 = 0x0000_0499;
/// `character_last_used` (`0x10003885`, InstanceId, §3.1.3).
pub const PID_CHARACTER_LAST_USED: u32 = 0x1000_3885;
/// `entity_is_transient_unique` (`0x00000A94`, Boolean, §3.2.5).
pub const PID_ENTITY_IS_TRANSIENT_UNIQUE: u32 = 0x0000_0A94;
/// `avatar_mesh_head` (`0x10006DB5`, Float, §3.2.5).
pub const PID_AVATAR_MESH_HEAD: u32 = 0x1000_6DB5;
/// `avatar_mesh_ears` (`0x1000739D`, Float, §3.2.5).
pub const PID_AVATAR_MESH_EARS: u32 = 0x1000_739D;
/// `avatar_bone_scale` (`0x10000DB4`, Float, §3.2.5).
pub const PID_AVATAR_BONE_SCALE: u32 = 0x1000_0DB4;
/// `avatar_height` (`0x1000703D`, Float, §3.2.5).
pub const PID_AVATAR_HEIGHT: u32 = 0x1000_703D;
/// `avatar_bone_slider_values` (`0x100069B7`, String, §3.2.5).
pub const PID_AVATAR_BONE_SLIDER_VALUES: u32 = 0x1000_69B7;
/// `avatar_color_skin` (`0x100001E0`, Color, §3.2.5).
pub const PID_AVATAR_COLOR_SKIN: u32 = 0x1000_01E0;
/// `avatar_color_hair` (`0x10000595`, Color, §3.2.5).
pub const PID_AVATAR_COLOR_HAIR: u32 = 0x1000_0595;
/// `avatar_color_eyes` (`0x100003BA`, Color, §3.2.5).
pub const PID_AVATAR_COLOR_EYES: u32 = 0x1000_03BA;
/// `avatar_color_lips` (`0x100066D9`, Color, §3.2.5).
pub const PID_AVATAR_COLOR_LIPS: u32 = 0x1000_66D9;
/// `avatar_color_warpaint` (`0x1000612A`, Color, §3.2.5).
pub const PID_AVATAR_COLOR_WARPAINT: u32 = 0x1000_612A;
/// `world_time_offset` (`0x000001EF`, Float, §4.4.2).
pub const PID_WORLD_TIME_OFFSET: u32 = 0x0000_01EF;
/// `world_time_dilation` (`0x000006CC`, Float, §4.4.2).
pub const PID_WORLD_TIME_DILATION: u32 = 0x0000_06CC;
/// `world_present_time_of_day` (`0x00000682`, Float, §4.4.2).
pub const PID_WORLD_PRESENT_TIME_OF_DAY: u32 = 0x0000_0682;
/// `world_is_night` (`0x000001F0`, Boolean, §4.4.2).
pub const PID_WORLD_IS_NIGHT: u32 = 0x0000_01F0;
/// `world_day_type` (`0x00000235`, Enum, §4.4.2).
pub const PID_WORLD_DAY_TYPE: u32 = 0x0000_0235;
/// `entity_display_name` (`0x00000354`, LocalizedTokenText, §5.2.4).
pub const PID_ENTITY_DISPLAY_NAME: u32 = 0x0000_0354;
/// `object_category_code` (`0x00000126`, Enum, §5.2.4).
pub const PID_OBJECT_CATEGORY_CODE: u32 = 0x0000_0126;
/// `current_morale` (`0x100012D2`, Float, §5.2.4).
pub const PID_CURRENT_MORALE: u32 = 0x1000_12D2;
/// `max_morale` (`0x100008BB`, Float, §5.2.4).
pub const PID_MAX_MORALE: u32 = 0x1000_08BB;
/// `current_power` (`0x10000340`, Float, §5.2.4).
pub const PID_CURRENT_POWER: u32 = 0x1000_0340;
/// `max_power` (`0x10000520`, Float, §5.2.4).
pub const PID_MAX_POWER: u32 = 0x1000_0520;
/// `entity_life_state_code` (`0x100005D6`, Enum, §5.2.4).
pub const PID_ENTITY_LIFE_STATE_CODE: u32 = 0x1000_05D6;
/// `selected_target_entity_id` (`0x0000034F`, InstanceId, §5.2.4).
pub const PID_SELECTED_TARGET_ENTITY_ID: u32 = 0x0000_034F;

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors produced when encoding or decoding official world-session messages and wire
/// values (`docs/specs/official-messages-world.md` §1–§7).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WorldError {
    /// Input ended before the required number of bytes could be read.
    #[error("unexpected end of wire payload: needed {needed} byte(s), {remaining} remaining")]
    UnexpectedEof {
        /// Number of bytes requested.
        needed: usize,
        /// Number of unread bytes remaining in the cursor.
        remaining: usize,
    },
    /// A `bool8` field held a byte value of `2` or higher (§2.1, §7.1 item 3).
    #[error("invalid bool8 byte 0x{0:02X}: must be 0x00 or 0x01")]
    InvalidBool8(u8),
    /// The leading `base_flags` byte of an inner event payload was not `0x00` (§1.3).
    #[error("invalid event base_flags byte 0x{0:02X}: expected 0x00")]
    InvalidBaseFlags(u8),
    /// Nesting exceeded [`MAX_RECURSION_DEPTH`] (§7.1 item 2).
    #[error("maximum recursion depth ({max}) exceeded")]
    MaxRecursionDepthExceeded {
        /// Maximum allowed depth.
        max: usize,
    },
    /// A 3-tier VLE value was `>= 0x4000_0000` and cannot be represented (§2.2.1).
    #[error("3-tier VLE value 0x{0:08X} exceeds 30-bit maximum 0x3FFFFFFF")]
    Vle3Overflow(u32),
    /// A 4-tier `VariableU32` started with a byte in `0xE1..=0xFF` (§2.2.2).
    #[error("invalid VariableU32 first byte 0x{0:02X}")]
    InvalidVariableU32Prefix(u8),
    /// A `CompactPID` started with a byte in `0x81..=0xFF` (§2.4.3; `property-system.md` §10.2).
    #[error("invalid CompactPID first byte 0x{0:02X}")]
    InvalidCompactPid(u8),
    /// A count exceeded the remaining unread bytes (§7.1 item 1).
    #[error(
        "wire count {count} (minimum {min_entry_bytes} byte(s) per item) exceeds {remaining} remaining byte(s)"
    )]
    CountExceedsRemaining {
        /// Decoded element/character/byte count.
        count: usize,
        /// Minimum wire bytes required per element.
        min_entry_bytes: usize,
        /// Remaining unread bytes in the frame.
        remaining: usize,
    },
    /// A prefixed UTF-16LE string contained an unpaired surrogate.
    #[error("invalid UTF-16LE string payload")]
    InvalidUtf16,
    /// A character above U+00FF was passed to an 8-bit string writer.
    #[error("character U+{0:04X} cannot be encoded in an 8-bit Latin-1 string")]
    InvalidLatin1Char(u32),
    /// A property ID was not found in the property schema (§7.1 item 4).
    #[error("unknown property ID 0x{0:08X} in wire property set")]
    UnknownPropertyId(u32),
    /// A property type code outside `1..=22` was encountered (§2.7.1).
    #[error("invalid property type code {0}: must be in 1..=22")]
    InvalidPropertyTypeCode(u32),
    /// A TSize hint byte was `23` or higher (`property-system.md` §17.2).
    #[error("invalid TSize bucket hint {0}: must be in 0..=22")]
    InvalidTSizeHint(u8),
    /// A wire `LocalizedTokenText` had arguments: the per-argument key prefix is `OPEN` (§2.6).
    #[error("wire LocalizedTokenText arguments are not supported (arg_key layout is OPEN)")]
    UnsupportedWireTextArguments,
    /// A wire `LocalizedTokenText` argument kind outside `1..=7` reached a value decoder (§2.6).
    #[error("invalid LocalizedTokenText argument kind {0}")]
    InvalidStringArgKind(u16),
    /// An `EntityAppearanceDesc` had bit 0 (`base_model`) set: the bit test that gates
    /// `model_b` is `OPEN` (§2.8), so the record cannot be framed.
    #[error("EntityAppearanceDesc base_model is not decodable (model_b gate is OPEN)")]
    UnsupportedAppearanceBaseModel,
    /// An `EntityAppearanceDesc` or `PartEntry` flags word had a bit outside its defined set
    /// (`0x000007E3` / `0x0000002F`, §2.8).
    #[error("undefined EntityAppearanceDesc flag bits {0:#010x}")]
    InvalidAppearanceFlags(u32),
    /// A `0xCC` packet referenced a `state_schema_id` the schema lookup does not know (§4.3.1).
    #[error("unknown 0xCC state_schema_id 0x{0:08X}")]
    UnknownPortalStateSchema(u32),
    /// A `0xCC` value had the wrong number of boolean or enum state entries for its schema.
    #[error(
        "portal state schema 0x{schema_id:08X} requires {expected_bools} bool(s) and {expected_enums} enum(s), got {actual_bools} and {actual_enums}"
    )]
    PortalStateSchemaLengthMismatch {
        /// Schema identifier.
        schema_id: u32,
        /// Expected boolean state count.
        expected_bools: usize,
        /// Actual boolean state count.
        actual_bools: usize,
        /// Expected enum state count.
        expected_enums: usize,
        /// Actual enum state count.
        actual_enums: usize,
    },
    /// A flags field had a bit the spec does not define (and whose payload is unknown).
    #[error("undefined {field} bits {bits:#06x}")]
    UndefinedFlagBits {
        /// The field.
        field: &'static str,
        /// The undefined bits that were set.
        bits: u32,
    },
    /// A batch event ID was neither `0x0D` nor `0x4C`, or a deletion/property event ID did
    /// not belong to the decoder it was given to.
    #[error("event ID 0x{0:08X} does not belong to this decoder")]
    WrongEventId(u32),
    /// A `0x35000000` record did not start with its own DID or had an unusable value (§4.4.2).
    #[error("invalid world calendar record: {0}")]
    InvalidWorldCalendar(&'static str),
    /// A count does not fit the wire field that carries it (for example more than 255
    /// actions in one `0x24` slot, §5.5 row 10).
    #[error("count {count} does not fit its wire field")]
    CountOverflow {
        /// The count.
        count: usize,
    },
    /// Trailing unread bytes remained after decoding a fixed-framing payload.
    #[error("{0} trailing byte(s) remained after decoding payload")]
    TrailingBytes(usize),
    /// A decoded `f32` position or velocity component was NaN or infinite
    /// (`official-messages-play.md` §9.1 rule 4).
    #[error("non-finite {field} value in movement payload")]
    NonFiniteFloat {
        /// Name of the offending field.
        field: &'static str,
    },
    /// A decoded world coordinate lay outside `[-100000.0, 100000.0]` metres
    /// (`official-messages-play.md` §9.1 rule 4).
    #[error(
        "{field} coordinate outside [-{MAX_WORLD_COORDINATE_M}, {MAX_WORLD_COORDINATE_M}] metres"
    )]
    CoordinateOutOfRange {
        /// Name of the offending field.
        field: &'static str,
    },
    /// The `MessageEnvelope` around an event failed to decode (`official-session.md` §9.1).
    #[error("message envelope: {0}")]
    Envelope(#[from] SessionError),
}

/// Largest absolute world coordinate (metres) accepted in `0xCE`
/// (`official-messages-play.md` §9.1 rule 4).
pub const MAX_WORLD_COORDINATE_M: f32 = 100_000.0;

/// Rejects a NaN or infinite position or velocity component
/// (`official-messages-play.md` §9.1 rule 4).
pub(crate) fn require_finite(field: &'static str, value: f32) -> Result<f32, WorldError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(WorldError::NonFiniteFloat { field })
    }
}

/// Rejects a non-finite world coordinate or one outside `±MAX_WORLD_COORDINATE_M`
/// (`official-messages-play.md` §9.1 rule 4).
pub(crate) fn require_world_coordinate(field: &'static str, value: f32) -> Result<f32, WorldError> {
    let value = require_finite(field, value)?;
    if (-MAX_WORLD_COORDINATE_M..=MAX_WORLD_COORDINATE_M).contains(&value) {
        Ok(value)
    } else {
        Err(WorldError::CoordinateOutOfRange { field })
    }
}

/// Rejects `bits & !defined` (an undefined flag bit whose payload is unknown).
fn require_defined_bits(field: &'static str, bits: u32, defined: u32) -> Result<(), WorldError> {
    let undefined = bits & !defined;
    if undefined == 0 {
        Ok(())
    } else {
        tracing::debug!(field, undefined, "undefined flag bits refused");
        Err(WorldError::UndefinedFlagBits {
            field,
            bits: undefined,
        })
    }
}

/// Generates `to_bytes` / `from_bytes` for an event type whose codec needs no schema.
macro_rules! plain_codec {
    ($t:ty) => {
        impl $t {
            /// Encodes the inner event payload to a standalone byte vector.
            ///
            /// # Errors
            ///
            /// Propagates the encoder's error.
            pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
                let mut w = WireWriter::new();
                self.encode(&mut w)?;
                Ok(w.into_vec())
            }

            /// Decodes the inner event payload from a standalone byte slice; trailing bytes
            /// are an error.
            ///
            /// # Errors
            ///
            /// Propagates the decoder's error.
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
                let mut r = WireReader::new(bytes);
                let msg = Self::decode(&mut r)?;
                r.finish()?;
                Ok(msg)
            }
        }
    };
}

/// Generates `to_bytes` / `from_bytes` for an event type whose decoder takes a property schema.
macro_rules! schema_codec {
    ($t:ty) => {
        impl $t {
            /// Encodes the inner event payload to a standalone byte vector.
            ///
            /// # Errors
            ///
            /// Propagates the encoder's error.
            pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
                let mut w = WireWriter::new();
                self.encode(&mut w)?;
                Ok(w.into_vec())
            }

            /// Decodes the inner event payload from a standalone byte slice using `schema`;
            /// trailing bytes are an error.
            ///
            /// # Errors
            ///
            /// Propagates the decoder's error.
            pub fn from_bytes(
                bytes: &[u8],
                schema: &dyn PropertySchemaLookup,
            ) -> Result<Self, WorldError> {
                let mut r = WireReader::new(bytes);
                let msg = Self::decode(&mut r, schema)?;
                r.finish()?;
                Ok(msg)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Bounded Cursor / Reader and Writer (§2.1–§2.3, §7.1)
// ---------------------------------------------------------------------------

/// Bounded, zero-panic cursor over an untrusted byte slice enforcing the
/// hostile-input invariants of `docs/specs/official-messages-world.md` §7.1.
#[derive(Debug, Clone)]
pub struct WireReader<'a> {
    buf: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> WireReader<'a> {
    /// Creates a new reader at offset `0` and recursion depth `0`.
    #[must_use]
    pub fn new(buf: &'a [u8]) -> Self {
        Self {
            buf,
            pos: 0,
            depth: 0,
        }
    }

    /// Current byte offset from the start of the underlying slice.
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Number of unread bytes remaining in the slice.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// Returns `true` when all bytes have been consumed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Current recursion depth for nested structures.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Returns the unread tail slice.
    #[must_use]
    pub fn remaining_slice(&self) -> &'a [u8] {
        self.buf.get(self.pos..).unwrap_or(&[])
    }

    /// Verifies that no unread bytes remain.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::TrailingBytes`] if `self.remaining() > 0`.
    pub fn finish(&self) -> Result<(), WorldError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(WorldError::TrailingBytes(self.remaining()))
        }
    }

    /// Checks that `count` items of at least `min_entry_bytes` each can fit in
    /// the remaining unread bytes before allocating (§7.1 item 1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::CountExceedsRemaining`] if
    /// `count * min_entry_bytes > self.remaining()`.
    pub fn ensure_count(&self, count: usize, min_entry_bytes: usize) -> Result<(), WorldError> {
        let min_bytes = count.saturating_mul(min_entry_bytes);
        if min_bytes > self.remaining() {
            return Err(WorldError::CountExceedsRemaining {
                count,
                min_entry_bytes,
                remaining: self.remaining(),
            });
        }
        Ok(())
    }

    /// Executes `f` at `self.depth + 1`, failing with
    /// [`WorldError::MaxRecursionDepthExceeded`] if the new depth would exceed
    /// [`MAX_RECURSION_DEPTH`] (`16`, §7.1 item 2).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::MaxRecursionDepthExceeded`] or the inner error.
    pub fn with_recursion<T, F>(&mut self, f: F) -> Result<T, WorldError>
    where
        F: FnOnce(&mut Self) -> Result<T, WorldError>,
    {
        let next = self.depth.saturating_add(1);
        if next > MAX_RECURSION_DEPTH {
            return Err(WorldError::MaxRecursionDepthExceeded {
                max: MAX_RECURSION_DEPTH,
            });
        }
        self.depth = next;
        let res = f(self);
        self.depth -= 1;
        res
    }

    /// Reads exactly `len` bytes as a sub-slice and advances the cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnexpectedEof`] if fewer than `len` bytes remain.
    pub fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], WorldError> {
        let end = self.pos.checked_add(len).ok_or(WorldError::UnexpectedEof {
            needed: len,
            remaining: self.remaining(),
        })?;
        let slice = self
            .buf
            .get(self.pos..end)
            .ok_or(WorldError::UnexpectedEof {
                needed: len,
                remaining: self.remaining(),
            })?;
        self.pos = end;
        Ok(slice)
    }

    /// Creates a sub-reader over the next `len` bytes and advances `self` past them.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnexpectedEof`] if fewer than `len` bytes remain.
    pub fn sub_reader(&mut self, len: usize) -> Result<WireReader<'a>, WorldError> {
        let slice = self.read_bytes(len)?;
        Ok(WireReader {
            buf: slice,
            pos: 0,
            depth: self.depth,
        })
    }

    /// Reads a single unsigned byte (`u8`).
    pub fn read_u8(&mut self) -> Result<u8, WorldError> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    /// Reads a single signed byte (`i8`).
    pub fn read_i8(&mut self) -> Result<i8, WorldError> {
        Ok(self.read_u8()? as i8)
    }

    /// Reads a strict `bool8` (`0x00` = `false`, `0x01` = `true`), rejecting
    /// `0x02..=0xFF` (§2.1, §7.1 item 3).
    pub fn read_bool8(&mut self) -> Result<bool, WorldError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(WorldError::InvalidBool8(other)),
        }
    }

    /// Reads and verifies the leading `u8 base_flags = 0x00` byte of an inner event payload (§1.3).
    pub fn read_base_flags(&mut self) -> Result<(), WorldError> {
        let b = self.read_u8()?;
        if b != 0 {
            return Err(WorldError::InvalidBaseFlags(b));
        }
        Ok(())
    }

    /// Reads a little-endian `u16`.
    pub fn read_u16_le(&mut self) -> Result<u16, WorldError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Reads a big-endian `u16`.
    pub fn read_u16_be(&mut self) -> Result<u16, WorldError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    /// Reads a little-endian `i16`.
    pub fn read_i16_le(&mut self) -> Result<i16, WorldError> {
        let b = self.read_bytes(2)?;
        Ok(i16::from_le_bytes([b[0], b[1]]))
    }

    /// Reads a little-endian `u32`.
    pub fn read_u32_le(&mut self) -> Result<u32, WorldError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a big-endian `u32`.
    pub fn read_u32_be(&mut self) -> Result<u32, WorldError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a little-endian `i32`.
    pub fn read_i32_le(&mut self) -> Result<i32, WorldError> {
        let b = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a little-endian `u64`.
    pub fn read_u64_le(&mut self) -> Result<u64, WorldError> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a little-endian `i64`.
    pub fn read_i64_le(&mut self) -> Result<i64, WorldError> {
        let b = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a little-endian IEEE 754 `f32`.
    pub fn read_f32_le(&mut self) -> Result<f32, WorldError> {
        let b = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a little-endian IEEE 754 `f64`.
    pub fn read_f64_le(&mut self) -> Result<f64, WorldError> {
        let b = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a 3-tier VLE (30-bit unsigned, 1 / 2 / 4 bytes, no `0xE0` escape; §2.2.1).
    pub fn read_vle3(&mut self) -> Result<u32, WorldError> {
        let b0 = self.read_u8()?;
        if (b0 & 0x80) == 0 {
            Ok(u32::from(b0))
        } else if (b0 & 0x40) == 0 {
            let b1 = self.read_u8()?;
            Ok((u32::from(b0 & 0x3F) << 8) | u32::from(b1))
        } else {
            let b1 = self.read_u8()?;
            let w2 = self.read_u16_le()?;
            Ok((u32::from(b0 & 0x3F) << 24) | (u32::from(b1) << 16) | u32::from(w2))
        }
    }

    /// Reads a 4-tier `VariableU32` (full 32-bit unsigned, 1 / 2 / 4 / 5 bytes with the `0xE0`
    /// prefix; first bytes `0xE1..=0xFF` are rejected; §2.2.2, `official-session.md` §5.6.1).
    pub fn read_vle4(&mut self) -> Result<u32, WorldError> {
        let b0 = self.read_u8()?;
        if (b0 & 0x80) == 0 {
            Ok(u32::from(b0))
        } else if (b0 & 0x40) == 0 {
            let b1 = self.read_u8()?;
            Ok((u32::from(b0 & 0x3F) << 8) | u32::from(b1))
        } else if (b0 & 0x20) == 0 {
            let b1 = self.read_u8()?;
            let w2 = self.read_u16_le()?;
            Ok((u32::from(b0 & 0x1F) << 24) | (u32::from(b1) << 16) | u32::from(w2))
        } else if b0 == 0xE0 {
            self.read_u32_le()
        } else {
            Err(WorldError::InvalidVariableU32Prefix(b0))
        }
    }

    /// Reads a 16-bit VLE (1 / 2 / 3 / 4 bytes; §2.2.3).
    pub fn read_vle16(&mut self) -> Result<u16, WorldError> {
        let b0 = self.read_u8()?;
        if (b0 & 0x80) == 0 {
            Ok(u16::from(b0))
        } else if (b0 & 0x40) == 0 {
            let b1 = self.read_u8()?;
            Ok((u16::from(b0 & 0x3F) << 8) | u16::from(b1))
        } else if (b0 & 0x20) == 0 {
            let _discard = self.read_u8()?;
            self.read_u16_le()
        } else {
            self.read_u16_le()
        }
    }

    /// Reads a `SignedNormalizedByte` and returns its value in `[-1.0, 1.0]` (§2.3.1).
    pub fn read_signed_normalized(&mut self) -> Result<f32, WorldError> {
        Ok(decode_signed_normalized(self.read_i8()?))
    }

    /// Reads a prefixed UTF-16LE string (3-tier VLE code-unit count $N$ + $2N$ bytes; §2.3, §7.1).
    pub fn read_utf16le_string(&mut self) -> Result<String, WorldError> {
        let count = self.read_vle3()? as usize;
        self.ensure_count(count, 2)?;
        let raw = self.read_bytes(count * 2)?;
        let mut units = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
        for &pair in raw.as_chunks::<2>().0 {
            units.push(u16::from_le_bytes(pair));
        }
        String::from_utf16(&units).map_err(|_| WorldError::InvalidUtf16)
    }

    /// Reads a prefixed 8-bit Latin-1 / Pascal string (3-tier VLE byte count $N$ + $N$ bytes; §2.3, §7.1).
    pub fn read_pascal_string(&mut self) -> Result<String, WorldError> {
        let count = self.read_vle3()? as usize;
        self.ensure_count(count, 1)?;
        let raw = self.read_bytes(count)?;
        Ok(raw.iter().map(|&b| char::from(b)).collect())
    }
}

/// Growable byte writer for official world-session wire payloads (§2.1–§2.3).
#[derive(Debug, Clone, Default)]
pub struct WireWriter {
    buf: Vec<u8>,
}

impl WireWriter {
    /// Creates an empty `WireWriter`.
    #[must_use]
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Creates a `WireWriter` with pre-allocated byte capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    /// Number of bytes written so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Returns `true` if no bytes have been written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Borrows the written bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the writer and returns the underlying byte vector.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Appends a raw byte slice.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Appends a single `u8`.
    pub fn write_u8(&mut self, val: u8) {
        self.buf.push(val);
    }

    /// Appends a single `i8`.
    pub fn write_i8(&mut self, val: i8) {
        self.buf.push(val as u8);
    }

    /// Appends a `bool8` (`0x00` or `0x01`, §2.1).
    pub fn write_bool8(&mut self, val: bool) {
        self.buf.push(u8::from(val));
    }

    /// Appends the leading `u8 base_flags = 0x00` byte of an inner event payload (§1.3).
    pub fn write_base_flags(&mut self) {
        self.buf.push(0x00);
    }

    /// Appends a little-endian `u16`.
    pub fn write_u16_le(&mut self, val: u16) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a big-endian `u16`.
    pub fn write_u16_be(&mut self, val: u16) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    /// Appends a little-endian `i16`.
    pub fn write_i16_le(&mut self, val: i16) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a little-endian `u32`.
    pub fn write_u32_le(&mut self, val: u32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a big-endian `u32`.
    pub fn write_u32_be(&mut self, val: u32) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    /// Appends a little-endian `i32`.
    pub fn write_i32_le(&mut self, val: i32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a little-endian `u64`.
    pub fn write_u64_le(&mut self, val: u64) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a little-endian `i64`.
    pub fn write_i64_le(&mut self, val: i64) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a little-endian IEEE 754 `f32`.
    pub fn write_f32_le(&mut self, val: f32) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a little-endian IEEE 754 `f64`.
    pub fn write_f64_le(&mut self, val: f64) {
        self.buf.extend_from_slice(&val.to_le_bytes());
    }

    /// Appends a 3-tier VLE (30-bit unsigned, 1 / 2 / 4 bytes; §2.2.1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::Vle3Overflow`] if `val >= 0x4000_0000`.
    pub fn write_vle3(&mut self, val: u32) -> Result<(), WorldError> {
        if val <= 0x0000_007F {
            self.write_u8(val as u8);
            Ok(())
        } else if val <= 0x0000_3FFF {
            self.write_u8(0x80 | ((val >> 8) as u8 & 0x3F));
            self.write_u8((val & 0xFF) as u8);
            Ok(())
        } else if val <= 0x3FFF_FFFF {
            self.write_u8(0xC0 | ((val >> 24) as u8 & 0x3F));
            self.write_u8(((val >> 16) & 0xFF) as u8);
            self.write_u16_le((val & 0xFFFF) as u16);
            Ok(())
        } else {
            Err(WorldError::Vle3Overflow(val))
        }
    }

    /// Appends a 4-tier VLE (full 32-bit unsigned, 1 / 2 / 4 / 5 bytes; §2.2.2).
    pub fn write_vle4(&mut self, val: u32) {
        if val <= 0x0000_007F {
            self.write_u8(val as u8);
        } else if val <= 0x0000_3FFF {
            self.write_u8(0x80 | ((val >> 8) as u8 & 0x3F));
            self.write_u8((val & 0xFF) as u8);
        } else if val <= 0x1FFF_FFFF {
            self.write_u8(0xC0 | ((val >> 24) as u8 & 0x1F));
            self.write_u8(((val >> 16) & 0xFF) as u8);
            self.write_u16_le((val & 0xFFFF) as u16);
        } else {
            self.write_u8(0xE0);
            self.write_u32_le(val);
        }
    }

    /// Appends a 16-bit VLE (1 / 2 / 4 bytes; §2.2.3).
    pub fn write_vle16(&mut self, val: u16) {
        if val <= 0x007F {
            self.write_u8(val as u8);
        } else if val <= 0x3FFF {
            self.write_u8(0x80 | ((val >> 8) as u8 & 0x3F));
            self.write_u8((val & 0xFF) as u8);
        } else {
            self.write_u8(0xC0);
            self.write_u8(0x00);
            self.write_u16_le(val);
        }
    }

    /// Appends a prefixed UTF-16LE string (3-tier VLE code-unit count + $2N$ bytes; §2.3).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::Vle3Overflow`] if the UTF-16 code-unit count exceeds `0x3FFF_FFFF`.
    pub fn write_utf16le_string(&mut self, s: &str) -> Result<(), WorldError> {
        let units: Vec<u16> = s.encode_utf16().collect();
        let len_u32 = u32::try_from(units.len())
            .map_err(|_| WorldError::CountOverflow { count: units.len() })?;
        self.write_vle3(len_u32)?;
        for u in units {
            self.write_u16_le(u);
        }
        Ok(())
    }

    /// Appends a prefixed 8-bit Latin-1 / Pascal string (3-tier VLE byte count + $N$ bytes; §2.3).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::InvalidLatin1Char`] if any character code point exceeds U+00FF.
    pub fn write_pascal_string(&mut self, s: &str) -> Result<(), WorldError> {
        let mut bytes = Vec::with_capacity(s.len());
        for ch in s.chars() {
            let cp = u32::from(ch);
            let b = u8::try_from(cp).map_err(|_| WorldError::InvalidLatin1Char(cp))?;
            bytes.push(b);
        }
        let len_u32 = u32::try_from(bytes.len())
            .map_err(|_| WorldError::CountOverflow { count: bytes.len() })?;
        self.write_vle3(len_u32)?;
        self.write_bytes(&bytes);
        Ok(())
    }
}

/// Encodes a 30-bit unsigned integer using the 3-tier VLE format (§2.2.1).
///
/// # Errors
///
/// Returns [`WorldError::Vle3Overflow`] if `val >= 0x4000_0000`.
pub fn encode_vle3(val: u32) -> Result<Vec<u8>, WorldError> {
    let mut w = WireWriter::with_capacity(4);
    w.write_vle3(val)?;
    Ok(w.into_vec())
}

/// Decodes a 3-tier VLE from the start of `bytes`, returning `(value, bytes_consumed)` (§2.2.1).
///
/// # Errors
///
/// Returns [`WorldError::UnexpectedEof`] if `bytes` is truncated.
pub fn decode_vle3(bytes: &[u8]) -> Result<(u32, usize), WorldError> {
    let mut r = WireReader::new(bytes);
    let val = r.read_vle3()?;
    Ok((val, r.position()))
}

/// Encodes a 32-bit unsigned integer using the 4-tier VLE format (§2.2.2).
#[must_use]
pub fn encode_vle4(val: u32) -> Vec<u8> {
    let mut w = WireWriter::with_capacity(5);
    w.write_vle4(val);
    w.into_vec()
}

/// Decodes a 4-tier VLE from the start of `bytes`, returning `(value, bytes_consumed)` (§2.2.2).
///
/// # Errors
///
/// Returns [`WorldError::UnexpectedEof`] if `bytes` is truncated.
pub fn decode_vle4(bytes: &[u8]) -> Result<(u32, usize), WorldError> {
    let mut r = WireReader::new(bytes);
    let val = r.read_vle4()?;
    Ok((val, r.position()))
}

/// Encodes a 16-bit unsigned integer using the 16-bit VLE format (§2.2.3).
#[must_use]
pub fn encode_vle16(val: u16) -> Vec<u8> {
    let mut w = WireWriter::with_capacity(4);
    w.write_vle16(val);
    w.into_vec()
}

/// Decodes a `SignedNormalizedByte`: `clamp(b / 127.0, -1.0, 1.0)` (§2.3.1).
#[must_use]
pub fn decode_signed_normalized(b: i8) -> f32 {
    (f32::from(b) / 127.0).clamp(-1.0, 1.0)
}

/// Encodes a `SignedNormalizedByte`: clamp to `[-1.0, 1.0]`, multiply by `127.0`, round
/// toward zero (§2.3.1). NaN encodes as `0`.
#[must_use]
pub fn encode_signed_normalized(value: f32) -> i8 {
    if value.is_nan() {
        return 0;
    }
    (value.clamp(-1.0, 1.0) * 127.0).trunc() as i8
}

/// Decodes a 16-bit VLE from the start of `bytes`, returning `(value, bytes_consumed)` (§2.2.3).
///
/// # Errors
///
/// Returns [`WorldError::UnexpectedEof`] if `bytes` is truncated.
pub fn decode_vle16(bytes: &[u8]) -> Result<(u16, usize), WorldError> {
    let mut r = WireReader::new(bytes);
    let val = r.read_vle16()?;
    Ok((val, r.position()))
}

// ---------------------------------------------------------------------------
// CompactDid (§2.4.2) and CompactPid (§2.4.3)
// ---------------------------------------------------------------------------

/// Wire-mode compact 32-bit Data ID / Enum / PropertyID value (`u8 high_byte`
/// followed by 3-tier VLE `low_24`; `docs/specs/official-messages-world.md` §2.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CompactDid(pub u32);

impl CompactDid {
    /// Writes this `CompactDid` to `writer` (2, 3, or 5 bytes).
    ///
    /// # Errors
    ///
    /// Never fails for a 24-bit low payload, but propagates `Result` for consistency.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        let high_byte = (self.0 >> 24) as u8;
        let low_24 = self.0 & 0x00FF_FFFF;
        writer.write_u8(high_byte);
        writer.write_vle3(low_24)
    }

    /// Reads a `CompactDid` from `reader`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnexpectedEof`] if input is truncated.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let high_byte = reader.read_u8()?;
        let low_24 = reader.read_vle3()? & 0x00FF_FFFF;
        Ok(Self((u32::from(high_byte) << 24) | low_24))
    }

    /// Encodes this `CompactDid` into a standalone byte vector.
    pub fn to_bytes(self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::with_capacity(5);
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes a `CompactDid` from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let val = Self::decode(&mut r)?;
        r.finish()?;
        Ok(val)
    }
}

/// Wire-mode compact Property ID (`CompactPID`, 2 or 5 bytes;
/// `docs/specs/official-messages-world.md` §2.4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CompactPid(pub u32);

impl CompactPid {
    /// Writes this `CompactPid` to `writer` (2 bytes big-endian for
    /// `0x0000_0000..=0x0000_3FFF` and `0x1000_0000..=0x1000_3FFF` with bit
    /// `0x4000`, or 5-byte `0x80` + `u32 LE` escape for all other PIDs).
    pub fn encode(&self, writer: &mut WireWriter) {
        let pid = self.0;
        let high_16 = pid >> 16;
        let low_16 = pid & 0xFFFF;
        if high_16 == 0x0000 && low_16 < 0x4000 {
            writer.write_u16_be(low_16 as u16);
        } else if high_16 == 0x1000 && low_16 < 0x4000 {
            writer.write_u16_be((low_16 as u16) | 0x4000);
        } else {
            writer.write_u8(0x80);
            writer.write_u32_le(pid);
        }
    }

    /// Reads a `CompactPid` from `reader` according to the §2.4.3 reader rules
    /// (`property-system.md` §10.2).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnexpectedEof`] if input is truncated, or
    /// [`WorldError::InvalidCompactPid`] for a first byte in `0x81..=0xFF` (a Tessera
    /// hostile-input rule: no conformant writer emits one).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let b0 = reader.read_u8()?;
        if b0 == 0x80 {
            Ok(Self(reader.read_u32_le()?))
        } else if b0 > 0x80 {
            tracing::debug!(
                first_byte = b0,
                "CompactPID with first byte 0x81..=0xFF rejected"
            );
            Err(WorldError::InvalidCompactPid(b0))
        } else {
            let b1 = reader.read_u8()?;
            let word = u16::from_be_bytes([b0, b1]);
            if (b0 & 0x40) != 0 {
                Ok(Self(0x1000_0000 | u32::from(word & 0x3FFF)))
            } else {
                Ok(Self(u32::from(word)))
            }
        }
    }

    /// Encodes this `CompactPid` into a standalone byte vector.
    #[must_use]
    pub fn to_bytes(self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(5);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes a `CompactPid` from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let val = Self::decode(&mut r)?;
        r.finish()?;
        Ok(val)
    }
}

// ---------------------------------------------------------------------------
// WirePosition (§2.5.1) and Quantized 1-Byte Heading (§2.5.2)
// ---------------------------------------------------------------------------

/// `Position` flag bit 0 (`0x01`): `region` is non-zero (§2.5.1).
pub const POS_FLAG_REGION: u8 = 0x01;
/// `Position` flag bit 1 (`0x02`): `block_x` and `block_y` are present (§2.5.1).
pub const POS_FLAG_BLOCK: u8 = 0x02;
/// `Position` flag bit 2 (`0x04`): `instance` (`u16 LE`) is present (§2.5.1).
pub const POS_FLAG_INSTANCE: u8 = 0x04;
/// `Position` flag bit 3 (`0x08`): `cell` is present (`!= 0xFFFF`, §2.5.1).
pub const POS_FLAG_CELL: u8 = 0x08;
/// `Position` flag bit 4 (`0x10`): `offset` (`3 * f32 LE`) is present (§2.5.1).
pub const POS_FLAG_OFFSET: u8 = 0x10;
/// `Position` flag bit 5 (`0x20`): `orientation` (`4 * f32 LE`) is present in Type 17 (§2.5.1).
pub const POS_FLAG_ORIENTATION: u8 = 0x20;
/// `Position` flag bit 6 (`0x40`): `region == 1` (elides `region` byte on wire when paired with `0x01`, §2.5.1).
pub const POS_FLAG_REGION_IS_ONE: u8 = 0x40;
/// `Position` flag bit 7 (`0x80`): `cell == 0` (elides `cell` `u16` on wire when paired with `0x08`, §2.5.1).
pub const POS_FLAG_CELL_IS_ZERO: u8 = 0x80;

/// Landblock/cell coordinate position and optional quaternion orientation
/// (`docs/specs/official-messages-world.md` §2.5.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WirePosition {
    /// Explicit wire/stored flag bitmask.
    pub flags: u8,
    /// Region identifier (defaults to `0` when `0x01` is clear, or `1` when `0x01 | 0x40` are set).
    pub region: u8,
    /// Landblock X coordinate (defaults to `0xFF` when `0x02` is clear).
    pub block_x: u8,
    /// Landblock Y coordinate (defaults to `0xFF` when `0x02` is clear).
    pub block_y: u8,
    /// Instance layer ID (defaults to `0x0000` when `0x04` is clear).
    pub instance: u16,
    /// Cell ID (defaults to `0xFFFF` when `0x08` is clear, or `0x0000` when `0x08 | 0x80` are set).
    pub cell: u16,
    /// Local `(x, y, z)` offset in metres within the landblock or cell.
    pub offset: [f32; 3],
    /// Quaternion `(w, x, y, z)` orientation (used only by full Property Type 17).
    pub orientation: [f32; 4],
}

impl Default for WirePosition {
    fn default() -> Self {
        Self {
            flags: 0,
            region: 0,
            block_x: 0xFF,
            block_y: 0xFF,
            instance: 0,
            cell: 0xFFFF,
            offset: [0.0, 0.0, 0.0],
            orientation: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

impl WirePosition {
    /// Constructs a `WirePosition` and automatically computes `flags` according
    /// to `property-system.md` V11 and `official-messages-world.md` §2.5.1.
    #[must_use]
    pub fn from_components(
        region: u8,
        block_x: u8,
        block_y: u8,
        instance: u16,
        cell: u16,
        offset: [f32; 3],
        orientation: [f32; 4],
    ) -> Self {
        let mut pos = Self {
            flags: 0,
            region,
            block_x,
            block_y,
            instance,
            cell,
            offset,
            orientation,
        };
        pos.flags = pos.compute_flags(true);
        pos
    }

    /// Computes the canonical `flags` byte for this position (`property-system.md` V11).
    #[must_use]
    pub fn compute_flags(&self, include_orientation: bool) -> u8 {
        let mut flags = 0u8;
        if self.region != 0 {
            flags |= POS_FLAG_REGION;
            if self.region == 1 {
                flags |= POS_FLAG_REGION_IS_ONE;
            }
        }
        if (self.block_x, self.block_y) != (0xFF, 0xFF) {
            flags |= POS_FLAG_BLOCK;
        }
        if self.instance != 0 {
            flags |= POS_FLAG_INSTANCE;
        }
        if self.cell != 0xFFFF {
            flags |= POS_FLAG_CELL;
            if self.cell == 0 {
                flags |= POS_FLAG_CELL_IS_ZERO;
            }
        }
        if self.offset != [0.0, 0.0, 0.0] {
            flags |= POS_FLAG_OFFSET;
        }
        if include_orientation && self.orientation != [1.0, 0.0, 0.0, 0.0] {
            flags |= POS_FLAG_ORIENTATION;
        }
        flags
    }

    /// Encodes a full Type 17 `Position` in wire mode (elides `region` when `0x01 | 0x40` are set, elides `cell` when `0x08 | 0x80`
    /// are set, and includes `orientation` when `0x20` is set; §2.5.1).
    pub fn encode_wire(&self, writer: &mut WireWriter) {
        self.encode_inner(writer, true, true);
    }

    /// Decodes a full Type 17 `Position` in wire mode (§2.5.1).
    pub fn decode_wire(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, true, true)
    }

    /// Encodes a base `Position` in wire mode (stops after `offset`, omitting
    /// `orientation`; used in `0xCE`, `0xCC`, `0x146`–`0x148`; §2.5.1).
    pub fn encode_base(&self, writer: &mut WireWriter) {
        self.encode_inner(writer, true, false);
    }

    /// Decodes a base `Position` in wire mode (stops after `offset`; §2.5.1).
    pub fn decode_base(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, true, false)
    }

    /// Encodes a full `Position` in stored `.dat` mode (`region` and `cell` bytes are written even when `0x40` / `0x80` are set; §2.5.1).
    pub fn encode_stored(&self, writer: &mut WireWriter) {
        self.encode_inner(writer, false, true);
    }

    /// Decodes a full `Position` in stored `.dat` mode (`property-system.md` §4.6).
    pub fn decode_stored(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, false, true)
    }

    fn encode_inner(&self, writer: &mut WireWriter, wire_mode: bool, allow_orientation: bool) {
        let flags = if allow_orientation {
            self.flags
        } else {
            self.flags & !POS_FLAG_ORIENTATION
        };
        writer.write_u8(flags);
        if (flags & POS_FLAG_REGION) != 0 && (!wire_mode || (flags & POS_FLAG_REGION_IS_ONE) == 0) {
            writer.write_u8(self.region);
        }
        if (flags & POS_FLAG_BLOCK) != 0 {
            writer.write_u8(self.block_x);
            writer.write_u8(self.block_y);
        }
        if (flags & POS_FLAG_INSTANCE) != 0 {
            writer.write_u16_le(self.instance);
        }
        if (flags & POS_FLAG_CELL) != 0 && (!wire_mode || (flags & POS_FLAG_CELL_IS_ZERO) == 0) {
            writer.write_u16_le(self.cell);
        }
        if (flags & POS_FLAG_OFFSET) != 0 {
            writer.write_f32_le(self.offset[0]);
            writer.write_f32_le(self.offset[1]);
            writer.write_f32_le(self.offset[2]);
        }
        if allow_orientation && (flags & POS_FLAG_ORIENTATION) != 0 {
            writer.write_f32_le(self.orientation[0]);
            writer.write_f32_le(self.orientation[1]);
            writer.write_f32_le(self.orientation[2]);
            writer.write_f32_le(self.orientation[3]);
        }
    }

    fn decode_inner(
        reader: &mut WireReader<'_>,
        wire_mode: bool,
        allow_orientation: bool,
    ) -> Result<Self, WorldError> {
        // A base Position has no orientation bytes (§2.5.1 row 7), so bit 5 says nothing
        // there; it is dropped, as the encoder drops it (whether a peer ever sets it on a
        // base Position: spec silent).
        let raw_flags = reader.read_u8()?;
        let flags = if allow_orientation {
            raw_flags
        } else {
            raw_flags & !POS_FLAG_ORIENTATION
        };
        let region = if (flags & POS_FLAG_REGION) != 0 {
            if wire_mode && (flags & POS_FLAG_REGION_IS_ONE) != 0 {
                1
            } else {
                reader.read_u8()?
            }
        } else {
            0
        };
        let (block_x, block_y) = if (flags & POS_FLAG_BLOCK) != 0 {
            (reader.read_u8()?, reader.read_u8()?)
        } else {
            (0xFF, 0xFF)
        };
        let instance = if (flags & POS_FLAG_INSTANCE) != 0 {
            reader.read_u16_le()?
        } else {
            0
        };
        let cell = if (flags & POS_FLAG_CELL) != 0 {
            if wire_mode && (flags & POS_FLAG_CELL_IS_ZERO) != 0 {
                0
            } else {
                reader.read_u16_le()?
            }
        } else {
            0xFFFF
        };
        let offset = if (flags & POS_FLAG_OFFSET) != 0 {
            [
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ]
        } else {
            [0.0, 0.0, 0.0]
        };
        let orientation = if allow_orientation && (flags & POS_FLAG_ORIENTATION) != 0 {
            [
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ]
        } else {
            [1.0, 0.0, 0.0, 0.0]
        };
        Ok(Self {
            flags,
            region,
            block_x,
            block_y,
            instance,
            cell,
            offset,
            orientation,
        })
    }
}

/// Encodes a horizontal yaw angle in radians into a quantized 1-byte heading
/// (`0..=255` mapping `[0, 2π)`; `docs/specs/official-messages-world.md` §2.5.2).
#[must_use]
pub fn encode_heading_u8(angle_radians: f32) -> u8 {
    let tau = 2.0 * std::f32::consts::PI;
    let wrapped = angle_radians.rem_euclid(tau);
    let scaled = (wrapped / tau) * 256.0 + 0.5;
    (scaled.floor() as i32 & 0xFF) as u8
}

/// Decodes a quantized 1-byte heading (`0..=255`) into radians in `[0, 2π)`
/// (`docs/specs/official-messages-world.md` §2.5.2).
#[must_use]
pub fn decode_heading_u8(raw_angle: u8) -> f32 {
    let tau = 2.0 * std::f32::consts::PI;
    (f32::from(raw_angle) * (tau / 256.0)).rem_euclid(tau)
}

// ---------------------------------------------------------------------------
// LocalizedTokenText (§2.6; stored form `property-system.md` §17.4)
// ---------------------------------------------------------------------------

/// Mode payload of a [`LocalizedTokenText`] (§2.6 rows 1–2).
#[derive(Debug, Clone, PartialEq)]
pub enum LocalizedTokenTextContent {
    /// Mode `1`: literal UTF-16LE string.
    Literal(String),
    /// Any mode byte other than `1`: string-table reference.
    Reference {
        /// Original mode byte (kept so a rewrite reproduces it).
        mode: u8,
        /// 32-bit string token ID.
        token_id: u32,
        /// 32-bit string-table DID (`0x25xxxxxx`).
        table_did: u32,
    },
}

/// Kind-specific value of a `LocalizedTokenText` argument (§2.6 kind table;
/// `property-system.md` §17.4.1).
#[derive(Debug, Clone, PartialEq)]
pub enum LocalizedTokenTextArgPayload {
    /// Kind `1`: nested `LocalizedTokenText`.
    Nested(Box<LocalizedTokenText>),
    /// Kind `2`: `f64` value with decimal places.
    Float {
        /// Floating-point value.
        value: f64,
        /// Number of decimal places.
        decimal_places: u16,
    },
    /// Kind `3`: `f64` value with decimal places and format options.
    FloatFormatted {
        /// Floating-point value.
        value: f64,
        /// Number of decimal places.
        decimal_places: u16,
        /// Formatting flags.
        format_flags: u8,
        /// Radix (`0xFFFF` = locale default).
        radix: u16,
    },
    /// Kind `4`: fixed 8-byte signed integer (the 8-byte form of §2.6).
    FixedInt(i64),
    /// Kind `5`: fixed 8-byte signed integer with format options (the 11-byte form).
    FixedIntFormatted {
        /// Signed 64-bit integer value.
        value: i64,
        /// Formatting flags.
        format_flags: u8,
        /// Radix.
        radix: u16,
    },
    /// Kind `6`: compact wire integer (`u8 is_32bit` + `u64 LE` or 4-tier `VariableU32`); a
    /// stored kind 6 is an unconditional 8-byte integer (`is_32bit == false`).
    CompactInt {
        /// `true` (`0x01`): a `VariableU32` of the low 32 bits follows on the wire.
        is_32bit: bool,
        /// Reconstructed 64-bit value.
        value: u64,
    },
    /// Kind `7`: compact wire integer with format options.
    CompactIntFormatted {
        /// `true` (`0x01`): a `VariableU32` of the low 32 bits follows on the wire.
        is_32bit: bool,
        /// Reconstructed 64-bit value.
        value: u64,
        /// Formatting flags.
        format_flags: u8,
        /// Radix.
        radix: u16,
    },
}

impl LocalizedTokenTextArgPayload {
    /// Writes the kind-specific value (after the 6-byte value header) in the wire (`wire`) or
    /// stored encoding (§2.6 kinds 1–7).
    ///
    /// # Errors
    ///
    /// Propagates nested encoding errors.
    pub fn encode(&self, writer: &mut WireWriter, wire: bool) -> Result<(), WorldError> {
        self.encode_with_depth(writer, wire, 1)
    }

    fn encode_with_depth(
        &self,
        writer: &mut WireWriter,
        wire: bool,
        depth: usize,
    ) -> Result<(), WorldError> {
        let compact = |writer: &mut WireWriter, is_32bit: bool, value: u64| {
            if wire {
                writer.write_bool8(is_32bit);
                if is_32bit {
                    writer.write_vle4(value as u32);
                } else {
                    writer.write_u64_le(value);
                }
            } else {
                writer.write_u64_le(value);
            }
        };
        match self {
            Self::Nested(nested) => nested.encode_inner(writer, wire, depth + 1)?,
            Self::Float {
                value,
                decimal_places,
            } => {
                writer.write_f64_le(*value);
                writer.write_u16_le(*decimal_places);
            }
            Self::FloatFormatted {
                value,
                decimal_places,
                format_flags,
                radix,
            } => {
                writer.write_f64_le(*value);
                writer.write_u16_le(*decimal_places);
                writer.write_u8(*format_flags);
                writer.write_u16_le(*radix);
            }
            Self::FixedInt(val) => writer.write_i64_le(*val),
            Self::FixedIntFormatted {
                value,
                format_flags,
                radix,
            } => {
                writer.write_i64_le(*value);
                writer.write_u8(*format_flags);
                writer.write_u16_le(*radix);
            }
            Self::CompactInt { is_32bit, value } => compact(writer, *is_32bit, *value),
            Self::CompactIntFormatted {
                is_32bit,
                value,
                format_flags,
                radix,
            } => {
                compact(writer, *is_32bit, *value);
                writer.write_u8(*format_flags);
                writer.write_u16_le(*radix);
            }
        }
        Ok(())
    }

    /// Reads the kind-specific value of `kind` (`1..=7`) after the 6-byte value header, in the
    /// wire (`wire`) or stored encoding (§2.6). Kinds `0` and `>= 8` carry no value bytes.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::InvalidStringArgKind`] for a kind outside `1..=7`,
    /// [`WorldError::InvalidBool8`] for an `is_32bit` byte above `1`, or a truncation error.
    pub fn decode(reader: &mut WireReader<'_>, kind: u16, wire: bool) -> Result<Self, WorldError> {
        let compact = |reader: &mut WireReader<'_>| -> Result<(bool, u64), WorldError> {
            if wire {
                let is_32bit = reader.read_bool8()?;
                let value = if is_32bit {
                    u64::from(reader.read_vle4()?)
                } else {
                    reader.read_u64_le()?
                };
                Ok((is_32bit, value))
            } else {
                Ok((false, reader.read_u64_le()?))
            }
        };
        Ok(match kind {
            1 => {
                let nested =
                    reader.with_recursion(|r| LocalizedTokenText::decode_inner(r, wire))?;
                Self::Nested(Box::new(nested))
            }
            2 => Self::Float {
                value: reader.read_f64_le()?,
                decimal_places: reader.read_u16_le()?,
            },
            3 => Self::FloatFormatted {
                value: reader.read_f64_le()?,
                decimal_places: reader.read_u16_le()?,
                format_flags: reader.read_u8()?,
                radix: reader.read_u16_le()?,
            },
            4 => Self::FixedInt(reader.read_i64_le()?),
            5 => Self::FixedIntFormatted {
                value: reader.read_i64_le()?,
                format_flags: reader.read_u8()?,
                radix: reader.read_u16_le()?,
            },
            6 => {
                let (is_32bit, value) = compact(reader)?;
                Self::CompactInt { is_32bit, value }
            }
            7 => {
                let (is_32bit, value) = compact(reader)?;
                Self::CompactIntFormatted {
                    is_32bit,
                    value,
                    format_flags: reader.read_u8()?,
                    radix: reader.read_u16_le()?,
                }
            }
            other => return Err(WorldError::InvalidStringArgKind(other)),
        })
    }
}

/// One argument entry of a **stored** `LocalizedTokenText` argument table
/// (`property-system.md` §17.4 step 4 and §17.4.1): `u32` key, `u16` kind, and for kinds
/// `1..=7` the 6-byte value header (`u16` kind again, `u32` variable ID) and the value.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalizedTokenTextArg {
    /// Variable name-hash key (`u32 LE`).
    pub key: u32,
    /// Entry kind selector (`u16 LE`).
    pub kind: u16,
    /// Value header kind (`u16 LE`), present when `kind` is in `1..=7`.
    pub header_kind: u16,
    /// Value header variable ID (`u32 LE`), present when `kind` is in `1..=7`.
    pub var_id: u32,
    /// Kind-specific value (`Some` for `kind` in `1..=7`, `None` otherwise).
    pub payload: Option<LocalizedTokenTextArgPayload>,
}

/// A tokenized text descriptor (`LocalizedTokenText`, property type 13): wire form of
/// `official-messages-world.md` §2.6, stored form of `property-system.md` §17.4.
///
/// **Arguments on the wire:** the width and meaning of the per-argument key prefix
/// (`arg_key`) when `arg_count > 0` are `OPEN` (§2.6). The wire codec therefore handles only
/// an empty argument table and refuses anything else
/// ([`WorldError::UnsupportedWireTextArguments`]); the stored codec handles arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalizedTokenText {
    /// Literal text or `(mode, token_id, table_did)` reference.
    pub content: LocalizedTokenTextContent,
    /// Optional three authoring strings (`[token_name, source_text, comment]`) when
    /// `has_authoring_strings` is `1`.
    pub authoring_strings: Option<[String; 3]>,
    /// Argument entries (empty in every wire value Tessera reads or writes).
    pub arguments: Vec<LocalizedTokenTextArg>,
}

impl LocalizedTokenText {
    /// Creates a literal UTF-16LE `LocalizedTokenText` with no authoring strings and no arguments.
    #[must_use]
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            content: LocalizedTokenTextContent::Literal(text.into()),
            authoring_strings: None,
            arguments: Vec::new(),
        }
    }

    /// Creates a (`mode = 0`) string-table reference with no authoring strings and no arguments.
    #[must_use]
    pub fn reference(token_id: u32, table_did: u32) -> Self {
        Self {
            content: LocalizedTokenTextContent::Reference {
                mode: 0,
                token_id,
                table_did,
            },
            authoring_strings: None,
            arguments: Vec::new(),
        }
    }

    /// Encodes the wire form: no TSize hint byte before `arg_count` (§2.6).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnsupportedWireTextArguments`] if `arguments` is non-empty.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_inner(writer, true, 1)
    }

    /// Decodes the wire form (§2.6).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnsupportedWireTextArguments`] for `arg_count > 0`, or a
    /// framing error.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, true)
    }

    /// Encodes the stored `.dat` form: a TSize hint byte (`1`) before `arg_count`
    /// (`property-system.md` §17.4; §2.6 Vector 6).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] if a string or count cannot be represented.
    pub fn encode_stored(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_inner(writer, false, 1)
    }

    /// Decodes the stored `.dat` form (`property-system.md` §17.4).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors or a TSize hint above `22`.
    pub fn decode_stored(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, false)
    }

    /// Encodes the wire form to a standalone byte vector.
    ///
    /// # Errors
    ///
    /// See [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes the wire form from a standalone byte slice.
    ///
    /// # Errors
    ///
    /// See [`Self::decode`]; trailing bytes are an error.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let val = Self::decode(&mut r)?;
        r.finish()?;
        Ok(val)
    }

    fn encode_inner(
        &self,
        writer: &mut WireWriter,
        wire: bool,
        depth: usize,
    ) -> Result<(), WorldError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(WorldError::MaxRecursionDepthExceeded {
                max: MAX_RECURSION_DEPTH,
            });
        }
        if wire && !self.arguments.is_empty() {
            return Err(WorldError::UnsupportedWireTextArguments);
        }
        match &self.content {
            LocalizedTokenTextContent::Literal(s) => {
                writer.write_u8(1);
                writer.write_utf16le_string(s)?;
            }
            LocalizedTokenTextContent::Reference {
                mode,
                token_id,
                table_did,
            } => {
                let effective_mode = if *mode == 1 { 0 } else { *mode };
                writer.write_u8(effective_mode);
                writer.write_u32_le(*token_id);
                writer.write_u32_le(*table_did);
            }
        }
        match &self.authoring_strings {
            Some([s0, s1, s2]) => {
                writer.write_bool8(true);
                writer.write_pascal_string(s0)?;
                writer.write_pascal_string(s1)?;
                writer.write_pascal_string(s2)?;
            }
            None => writer.write_bool8(false),
        }
        if !wire {
            writer.write_u8(1);
        }
        let arg_count =
            u32::try_from(self.arguments.len()).map_err(|_| WorldError::CountOverflow {
                count: self.arguments.len(),
            })?;
        writer.write_vle3(arg_count)?;
        for arg in &self.arguments {
            writer.write_u32_le(arg.key);
            writer.write_u16_le(arg.kind);
            if (1..=7).contains(&arg.kind) {
                writer.write_u16_le(arg.header_kind);
                writer.write_u32_le(arg.var_id);
                if let Some(payload) = &arg.payload {
                    payload.encode_with_depth(writer, wire, depth)?;
                }
            }
        }
        Ok(())
    }

    fn decode_inner(reader: &mut WireReader<'_>, wire: bool) -> Result<Self, WorldError> {
        let mode = reader.read_u8()?;
        let content = if mode == 1 {
            LocalizedTokenTextContent::Literal(reader.read_utf16le_string()?)
        } else {
            let token_id = reader.read_u32_le()?;
            let table_did = reader.read_u32_le()?;
            LocalizedTokenTextContent::Reference {
                mode,
                token_id,
                table_did,
            }
        };
        let authoring_strings = if reader.read_bool8()? {
            Some([
                reader.read_pascal_string()?,
                reader.read_pascal_string()?,
                reader.read_pascal_string()?,
            ])
        } else {
            None
        };
        if !wire {
            let hint = reader.read_u8()?;
            if hint > 22 {
                return Err(WorldError::InvalidTSizeHint(hint));
            }
        }
        let arg_count = reader.read_vle3()? as usize;
        if wire && arg_count > 0 {
            tracing::debug!(
                arg_count,
                "wire LocalizedTokenText with arguments refused (OPEN)"
            );
            return Err(WorldError::UnsupportedWireTextArguments);
        }
        reader.ensure_count(arg_count, 6)?;
        let mut arguments = Vec::with_capacity(arg_count.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..arg_count {
            let key = reader.read_u32_le()?;
            let kind = reader.read_u16_le()?;
            if (1..=7).contains(&kind) {
                let header_kind = reader.read_u16_le()?;
                let var_id = reader.read_u32_le()?;
                let payload = LocalizedTokenTextArgPayload::decode(reader, kind, wire)?;
                arguments.push(LocalizedTokenTextArg {
                    key,
                    kind,
                    header_kind,
                    var_id,
                    payload: Some(payload),
                });
            } else {
                arguments.push(LocalizedTokenTextArg {
                    key,
                    kind,
                    header_kind: 0,
                    var_id: 0,
                    payload: None,
                });
            }
        }
        Ok(Self {
            content,
            authoring_strings,
            arguments,
        })
    }
}

// ---------------------------------------------------------------------------
// Dictionary-Driven Wire PropertyValue & EncodedPropertyTable (§2.7)
// ---------------------------------------------------------------------------

/// The 22 property value type codes in the master property dictionary (`0x34000000`;
/// `docs/specs/official-messages-world.md` §2.7.1, `docs/specs/property-system.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PropertyType {
    /// Code `1`: 8-bit Pascal string (`Prefixed 8-bit string`).
    String = 1,
    /// Code `2`: `u32 LE` string token.
    StringToken = 2,
    /// Code `3`: animated float curve (`Waveform`).
    Waveform = 3,
    /// Code `4`: `f64 LE` timestamp.
    Timestamp = 4,
    /// Code `5`: `u8` tri-state (`0` = false, `1` = true, `255` = undefined).
    TriState = 5,
    /// Code `6`: `3 * f32 LE` vector (`x, y, z`).
    Vector3 = 6,
    /// Code `7`: `u64 LE` `InstanceID`.
    InstanceId = 7,
    /// Code `8`: `Enum` (`CompactDid` on wire, `u32 LE` stored).
    Enum = 8,
    /// Code `9`: `f32 LE` scalar.
    Float = 9,
    /// Code `10`: `PropertyId` (`CompactDid` on wire, `u32 LE` stored).
    PropertyId = 10,
    /// Code `11`: nested `EncodedPropertyTable` (`Struct`).
    Struct = 11,
    /// Code `12`: `u32 LE` count + `(CompactPid, PropertyValue)` list (`Array`).
    Array = 12,
    /// Code `13`: `LocalizedTokenText`.
    LocalizedTokenText = 13,
    /// Code `14`: `u64 LE` 64-bit bitfield.
    Bitfield64 = 14,
    /// Code `15`: signed 32-bit integer (4-tier VLE on wire, `i32 LE` stored).
    Int = 15,
    /// Code `16`: `u32 LE` colour (`B, G, R, A`).
    Color = 16,
    /// Code `17`: `Position` (wire `Position` with region/cell elision on wire).
    Position = 17,
    /// Code `18`: `u32 LE` 32-bit bitfield.
    Bitfield32 = 18,
    /// Code `19`: `i64 LE` / `u64 LE` 64-bit integer.
    Int64 = 19,
    /// Code `20`: `u32 LE` `DID reference` (always 4-byte `u32 LE`, even on wire).
    DidRef = 20,
    /// Code `21`: strict `bool8` (`0x00` or `0x01`).
    Boolean = 21,
    /// Code `22`: variable-width bitfield (3-tier VLE bit count + packed bytes).
    Bitfield = 22,
}

impl PropertyType {
    /// Converts a raw numeric type code (`1..=22`) into a [`PropertyType`].
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::InvalidPropertyTypeCode`] if `code` is outside `1..=22`.
    pub fn from_code(code: u32) -> Result<Self, WorldError> {
        match code {
            1 => Ok(Self::String),
            2 => Ok(Self::StringToken),
            3 => Ok(Self::Waveform),
            4 => Ok(Self::Timestamp),
            5 => Ok(Self::TriState),
            6 => Ok(Self::Vector3),
            7 => Ok(Self::InstanceId),
            8 => Ok(Self::Enum),
            9 => Ok(Self::Float),
            10 => Ok(Self::PropertyId),
            11 => Ok(Self::Struct),
            12 => Ok(Self::Array),
            13 => Ok(Self::LocalizedTokenText),
            14 => Ok(Self::Bitfield64),
            15 => Ok(Self::Int),
            16 => Ok(Self::Color),
            17 => Ok(Self::Position),
            18 => Ok(Self::Bitfield32),
            19 => Ok(Self::Int64),
            20 => Ok(Self::DidRef),
            21 => Ok(Self::Boolean),
            22 => Ok(Self::Bitfield),
            other => Err(WorldError::InvalidPropertyTypeCode(other)),
        }
    }

    /// Returns the numeric code (`1..=22`).
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Minimal dictionary lookup mapping a 32-bit Property ID (`PID`) to its
/// [`PropertyType`] (`1..=22`, `docs/specs/official-messages-world.md` §2.7.1).
pub trait PropertySchemaLookup {
    /// Looks up the [`PropertyType`] for `pid`, returning `None` if `pid` is unknown.
    fn property_type(&self, pid: u32) -> Option<PropertyType>;
}

impl PropertySchemaLookup for BTreeMap<u32, PropertyType> {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.get(&pid).copied()
    }
}

impl<S: std::hash::BuildHasher> PropertySchemaLookup for HashMap<u32, PropertyType, S> {
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.get(&pid).copied()
    }
}

impl<F> PropertySchemaLookup for F
where
    F: Fn(u32) -> Option<PropertyType>,
{
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self(pid)
    }
}

/// Type 3 (`Waveform`) property payload (`docs/specs/official-messages-world.md` §2.7.1,
/// `docs/specs/property-system.md` §4.2).
#[derive(Debug, Clone, PartialEq)]
pub enum WireWaveform {
    /// Subtype `1`: constant float (`1 * f32 LE`).
    Constant(f32),
    /// Any subtype other than `1` or `10`: ten float coefficients (`10 * f32 LE`).
    Standard {
        /// Waveform curve mode (`0`, `2..=9`, or `> 10`).
        subtype: u32,
        /// Ten float curve parameters.
        coeffs: [f32; 10],
    },
    /// Subtype `10`: ten float coefficients plus keyframed `(f32, f32)` pairs.
    Keyframed {
        /// Ten float curve parameters.
        coeffs: [f32; 10],
        /// Keyframe period in seconds.
        period: f32,
        /// Strict `bool8` loop flag.
        loop_flag: bool,
        /// Keyframe `(time, value)` pairs.
        pairs: Vec<(f32, f32)>,
    },
}

impl WireWaveform {
    /// Writes this waveform to `writer`.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        match self {
            Self::Constant(v0) => {
                writer.write_u32_le(1);
                writer.write_f32_le(*v0);
            }
            Self::Standard { subtype, coeffs } => {
                writer.write_u32_le(*subtype);
                for c in coeffs {
                    writer.write_f32_le(*c);
                }
            }
            Self::Keyframed {
                coeffs,
                period,
                loop_flag,
                pairs,
            } => {
                writer.write_u32_le(10);
                for c in coeffs {
                    writer.write_f32_le(*c);
                }
                writer.write_f32_le(*period);
                writer.write_bool8(*loop_flag);
                let count = u32::try_from(pairs.len())
                    .map_err(|_| WorldError::CountOverflow { count: pairs.len() })?;
                writer.write_u32_le(count);
                for &(t, v) in pairs {
                    writer.write_f32_le(t);
                    writer.write_f32_le(v);
                }
            }
        }
        Ok(())
    }

    /// Reads a waveform from `reader`.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let subtype = reader.read_u32_le()?;
        if subtype == 1 {
            return Ok(Self::Constant(reader.read_f32_le()?));
        }
        let mut coeffs = [0.0f32; 10];
        for c in &mut coeffs {
            *c = reader.read_f32_le()?;
        }
        if subtype == 10 {
            let period = reader.read_f32_le()?;
            let loop_flag = reader.read_bool8()?;
            let count = reader.read_u32_le()? as usize;
            reader.ensure_count(count, 8)?;
            let mut pairs = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
            for _ in 0..count {
                pairs.push((reader.read_f32_le()?, reader.read_f32_le()?));
            }
            Ok(Self::Keyframed {
                coeffs,
                period,
                loop_flag,
                pairs,
            })
        } else {
            Ok(Self::Standard { subtype, coeffs })
        }
    }
}

/// Decoded property value across all 22 property types (`docs/specs/official-messages-world.md` §2.7.1).
#[derive(Debug, Clone, PartialEq)]
pub enum EncodedPropertyValue {
    /// Type `1`: 8-bit Pascal string.
    String(String),
    /// Type `2`: `u32 LE` string token.
    StringToken(u32),
    /// Type `3`: animated float curve.
    Waveform(WireWaveform),
    /// Type `4`: `f64 LE` timestamp.
    Timestamp(f64),
    /// Type `5`: `u8` tri-state (`0`, `1`, `255`).
    TriState(u8),
    /// Type `6`: `3 * f32 LE` vector (`x, y, z`).
    Vector3([f32; 3]),
    /// Type `7`: `u64 LE` `InstanceID`.
    InstanceId(u64),
    /// Type `8`: `Enum` (`CompactDid` on wire, `u32 LE` stored).
    Enum(u32),
    /// Type `9`: `f32 LE` scalar.
    Float(f32),
    /// Type `10`: `PropertyId` (`CompactDid` on wire, `u32 LE` stored).
    PropertyId(u32),
    /// Type `11`: nested `EncodedPropertyTable` (`Struct`).
    Struct(EncodedPropertyTable),
    /// Type `12`: array of `EncodedPropertyEntry` (`u32 LE` count + `(CompactPid, EncodedPropertyValue)` on wire).
    Array(Vec<EncodedPropertyEntry>),
    /// Type `13`: `LocalizedTokenText`.
    LocalizedTokenText(LocalizedTokenText),
    /// Type `14`: `u64 LE` 64-bit bitfield.
    Bitfield64(u64),
    /// Type `15`: signed 32-bit integer (4-tier VLE on wire, `i32 LE` stored).
    Int(i32),
    /// Type `16`: `[u8; 4]` colour (`[B, G, R, A]`, `u32 LE`).
    Color([u8; 4]),
    /// Type `17`: full `WirePosition` (with wire elision of `region` and `cell` on wire).
    Position(WirePosition),
    /// Type `18`: `u32 LE` 32-bit bitfield.
    Bitfield32(u32),
    /// Type `19`: `i64 LE` 64-bit integer.
    Int64(i64),
    /// Type `20`: `u32 LE` `DID reference` (4-byte `u32 LE` on both wire and stored).
    DidRef(u32),
    /// Type `21`: strict `bool8`.
    Boolean(bool),
    /// Type `22`: variable-width bitfield (3-tier VLE bit count $b$ + $\lceil b/8 \rceil$ bytes).
    Bitfield {
        /// Number of bits $b$.
        bit_count: u32,
        /// Packed bit bytes ($\lceil b/8 \rceil$ bytes, LSB-first).
        bytes: Vec<u8>,
    },
}

impl EncodedPropertyValue {
    /// Returns the [`PropertyType`] corresponding to this value variant.
    #[must_use]
    pub const fn property_type(&self) -> PropertyType {
        match self {
            Self::String(_) => PropertyType::String,
            Self::StringToken(_) => PropertyType::StringToken,
            Self::Waveform(_) => PropertyType::Waveform,
            Self::Timestamp(_) => PropertyType::Timestamp,
            Self::TriState(_) => PropertyType::TriState,
            Self::Vector3(_) => PropertyType::Vector3,
            Self::InstanceId(_) => PropertyType::InstanceId,
            Self::Enum(_) => PropertyType::Enum,
            Self::Float(_) => PropertyType::Float,
            Self::PropertyId(_) => PropertyType::PropertyId,
            Self::Struct(_) => PropertyType::Struct,
            Self::Array(_) => PropertyType::Array,
            Self::LocalizedTokenText(_) => PropertyType::LocalizedTokenText,
            Self::Bitfield64(_) => PropertyType::Bitfield64,
            Self::Int(_) => PropertyType::Int,
            Self::Color(_) => PropertyType::Color,
            Self::Position(_) => PropertyType::Position,
            Self::Bitfield32(_) => PropertyType::Bitfield32,
            Self::Int64(_) => PropertyType::Int64,
            Self::DidRef(_) => PropertyType::DidRef,
            Self::Boolean(_) => PropertyType::Boolean,
            Self::Bitfield { .. } => PropertyType::Bitfield,
        }
    }

    /// Encodes this property value in its wire encoding (§2.7.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_with_depth(writer, 1)
    }

    fn encode_with_depth(&self, writer: &mut WireWriter, depth: usize) -> Result<(), WorldError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(WorldError::MaxRecursionDepthExceeded {
                max: MAX_RECURSION_DEPTH,
            });
        }
        match self {
            Self::String(s) => writer.write_pascal_string(s)?,
            Self::StringToken(tok) => writer.write_u32_le(*tok),
            Self::Waveform(wf) => wf.encode(writer)?,
            Self::Timestamp(ts) => writer.write_f64_le(*ts),
            Self::TriState(v) => writer.write_u8(*v),
            Self::Vector3([x, y, z]) => {
                writer.write_f32_le(*x);
                writer.write_f32_le(*y);
                writer.write_f32_le(*z);
            }
            Self::InstanceId(id) => writer.write_u64_le(*id),
            Self::Enum(val) | Self::PropertyId(val) => {
                CompactDid(*val).encode(writer)?;
            }
            Self::Float(v) => writer.write_f32_le(*v),
            Self::Struct(ps) => {
                ps.encode_with_depth(writer, depth + 1)?;
            }
            Self::Array(items) => {
                let count = u32::try_from(items.len())
                    .map_err(|_| WorldError::CountOverflow { count: items.len() })?;
                writer.write_u32_le(count);
                for entry in items {
                    entry.encode_with_depth(writer, depth + 1)?;
                }
            }
            Self::LocalizedTokenText(si) => si.encode(writer)?,
            Self::Bitfield64(v) => writer.write_u64_le(*v),
            Self::Int(v) => writer.write_vle4(*v as u32),
            Self::Color(bgra) => writer.write_bytes(bgra),
            Self::Position(pos) => pos.encode_wire(writer),
            Self::Bitfield32(v) => writer.write_u32_le(*v),
            Self::Int64(v) => writer.write_i64_le(*v),
            Self::DidRef(did) => writer.write_u32_le(*did),
            Self::Boolean(b) => writer.write_bool8(*b),
            Self::Bitfield { bit_count, bytes } => {
                writer.write_vle3(*bit_count)?;
                let expected_bytes = (*bit_count as usize).div_ceil(8);
                if bytes.len() >= expected_bytes {
                    writer.write_bytes(&bytes[..expected_bytes]);
                } else {
                    writer.write_bytes(bytes);
                    for _ in bytes.len()..expected_bytes {
                        writer.write_u8(0);
                    }
                }
            }
        }
        Ok(())
    }

    /// Decodes a property value of `prop_type` in its wire encoding (§2.7.1).
    pub fn decode(
        reader: &mut WireReader<'_>,
        prop_type: PropertyType,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        match prop_type {
            PropertyType::String => Ok(Self::String(reader.read_pascal_string()?)),
            PropertyType::StringToken => Ok(Self::StringToken(reader.read_u32_le()?)),
            PropertyType::Waveform => Ok(Self::Waveform(WireWaveform::decode(reader)?)),
            PropertyType::Timestamp => Ok(Self::Timestamp(reader.read_f64_le()?)),
            PropertyType::TriState => Ok(Self::TriState(reader.read_u8()?)),
            PropertyType::Vector3 => Ok(Self::Vector3([
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ])),
            PropertyType::InstanceId => Ok(Self::InstanceId(reader.read_u64_le()?)),
            PropertyType::Enum => Ok(Self::Enum(CompactDid::decode(reader)?.0)),
            PropertyType::Float => Ok(Self::Float(reader.read_f32_le()?)),
            PropertyType::PropertyId => Ok(Self::PropertyId(CompactDid::decode(reader)?.0)),
            PropertyType::Struct => {
                let ps = reader.with_recursion(|r| EncodedPropertyTable::decode(r, schema))?;
                Ok(Self::Struct(ps))
            }
            PropertyType::Array => reader.with_recursion(|r| {
                let count = r.read_u32_le()? as usize;
                r.ensure_count(count, 2)?;
                let mut entries = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
                for _ in 0..count {
                    entries.push(EncodedPropertyEntry::decode(r, schema)?);
                }
                Ok(Self::Array(entries))
            }),
            PropertyType::LocalizedTokenText => Ok(Self::LocalizedTokenText(
                LocalizedTokenText::decode(reader)?,
            )),
            PropertyType::Bitfield64 => Ok(Self::Bitfield64(reader.read_u64_le()?)),
            PropertyType::Int => Ok(Self::Int(reader.read_vle4()? as i32)),
            PropertyType::Color => {
                let b = reader.read_bytes(4)?;
                Ok(Self::Color([b[0], b[1], b[2], b[3]]))
            }
            PropertyType::Position => Ok(Self::Position(WirePosition::decode_wire(reader)?)),
            PropertyType::Bitfield32 => Ok(Self::Bitfield32(reader.read_u32_le()?)),
            PropertyType::Int64 => Ok(Self::Int64(reader.read_i64_le()?)),
            PropertyType::DidRef => Ok(Self::DidRef(reader.read_u32_le()?)),
            PropertyType::Boolean => Ok(Self::Boolean(reader.read_bool8()?)),
            PropertyType::Bitfield => {
                let bit_count = reader.read_vle3()?;
                let byte_len = (bit_count as usize).div_ceil(8);
                reader.ensure_count(byte_len, 1)?;
                let raw = reader.read_bytes(byte_len)?;
                Ok(Self::Bitfield {
                    bit_count,
                    bytes: raw.to_vec(),
                })
            }
        }
    }
}

/// A single `(CompactPID, PropertyValue)` entry in a [`EncodedPropertyTable`], a
/// Type 12 `Array`, or a single-property network event (`0x88`, `0x92`, `0xB0`,
/// `0x147`, `0x149`; `docs/specs/official-messages-world.md` §2.7.2).
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedPropertyEntry {
    /// Property ID (`0` for a 2-byte `00 00` null entry with no value).
    pub pid: u32,
    /// Property value (`None` when `pid == 0`, `Some` when `pid != 0`).
    pub value: Option<EncodedPropertyValue>,
}

impl EncodedPropertyEntry {
    /// Creates a non-null property entry.
    #[must_use]
    pub fn new(pid: u32, value: EncodedPropertyValue) -> Self {
        Self {
            pid,
            value: Some(value),
        }
    }

    /// Creates a null property entry (`pid = 0`, 2 bytes `00 00` on wire).
    #[must_use]
    pub const fn null() -> Self {
        Self {
            pid: 0,
            value: None,
        }
    }

    /// Encodes this entry in wire mode.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_with_depth(writer, 1)
    }

    fn encode_with_depth(&self, writer: &mut WireWriter, depth: usize) -> Result<(), WorldError> {
        if self.pid == 0 {
            CompactPid(0).encode(writer);
            Ok(())
        } else if let Some(val) = &self.value {
            CompactPid(self.pid).encode(writer);
            val.encode_with_depth(writer, depth)
        } else {
            CompactPid(0).encode(writer);
            Ok(())
        }
    }

    /// Decodes a single property entry in wire mode using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let pid = CompactPid::decode(reader)?.0;
        if pid == 0 {
            return Ok(Self::null());
        }
        let prop_type = schema
            .property_type(pid)
            .ok_or(WorldError::UnknownPropertyId(pid))?;
        let val = EncodedPropertyValue::decode(reader, prop_type, schema)?;
        Ok(Self {
            pid,
            value: Some(val),
        })
    }
}

/// Wire `EncodedPropertyTable` (3-tier VLE count + `(CompactPID, PropertyValue)` entries, no TSize hint byte,
/// no duplicate `u32` key; `docs/specs/official-messages-world.md` §2.7.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EncodedPropertyTable {
    /// Ordered entries in the property set.
    pub entries: Vec<EncodedPropertyEntry>,
}

impl EncodedPropertyTable {
    /// Creates an empty `EncodedPropertyTable` (encodes as the single byte `0x00`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Number of entries in the set.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the set contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Appends a `(pid, value)` property to the set.
    pub fn push(&mut self, pid: u32, value: EncodedPropertyValue) {
        self.entries.push(EncodedPropertyEntry::new(pid, value));
    }

    /// Appends a 2-byte null entry (`pid = 0`) to the set.
    pub fn push_null(&mut self) {
        self.entries.push(EncodedPropertyEntry::null());
    }

    /// Returns the first value matching `pid` (`property-system.md` S4).
    #[must_use]
    pub fn get(&self, pid: u32) -> Option<&EncodedPropertyValue> {
        self.entries
            .iter()
            .find(|e| e.pid == pid)
            .and_then(|e| e.value.as_ref())
    }

    /// Encodes this `EncodedPropertyTable` to `writer` (§2.7.2).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_with_depth(writer, 1)
    }

    fn encode_with_depth(&self, writer: &mut WireWriter, depth: usize) -> Result<(), WorldError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(WorldError::MaxRecursionDepthExceeded {
                max: MAX_RECURSION_DEPTH,
            });
        }
        let count = u32::try_from(self.entries.len()).map_err(|_| WorldError::CountOverflow {
            count: self.entries.len(),
        })?;
        writer.write_vle3(count)?;
        for entry in &self.entries {
            entry.encode_with_depth(writer, depth)?;
        }
        Ok(())
    }

    /// Decodes a `EncodedPropertyTable` from `reader` using `schema` (§2.7.2, §7.1).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let count = reader.read_vle3()? as usize;
        reader.ensure_count(count, 2)?;
        let mut entries = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..count {
            entries.push(EncodedPropertyEntry::decode(reader, schema)?);
        }
        Ok(Self { entries })
    }

    /// Encodes this `EncodedPropertyTable` into a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes a `EncodedPropertyTable` from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let ps = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(ps)
    }
}

// ---------------------------------------------------------------------------
// Event envelope: FragmentedBlob payload -> inner event payload (§1.1)
// ---------------------------------------------------------------------------

/// A world or play event taken out of a reassembled `FragmentedBlob` (§1.1.1–§1.1.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEnvelope {
    /// Numeric event ID (`blobHeaderWord`, Field 8a of the fragment header).
    pub event_id: u32,
    /// Target entity `InstanceID`: the fragment's `recipientId` (Field 7; `2` with
    /// `DefaultRecipient`, `official-session.md` §5.6.3).
    pub target_entity_id: u64,
    /// The optional routing header (`envelopeFlags` bit 1). The spec does not say how its
    /// fields relate to the target, so the target stays `recipientId`.
    pub routing_header: Option<MessageRoutingHeader>,
    /// Inner event payload, starting at offset 0 with `u8 base_flags = 0x00` (§1.3).
    pub inner_payload: Vec<u8>,
}

/// Decodes the `MessageEnvelope` of a world or play event (`official-session.md` §9.1) and
/// checks the leading `base_flags` byte (§1.1.2, §1.3). Transport control messages and the
/// envelope containers are direct-payload messages and are decoded by [`crate::session`].
///
/// # Errors
///
/// Returns [`WorldError::Envelope`] for envelope errors (empty, reserved flag bits, zlib
/// limits, routing header) and [`WorldError::InvalidBaseFlags`] or
/// [`WorldError::UnexpectedEof`] when the inner payload does not start with `0x00`.
pub fn decode_event_envelope(
    event_id: u32,
    recipient_id: u64,
    blob_payload: &[u8],
    max_decompressed_bytes: usize,
) -> Result<EventEnvelope, WorldError> {
    let env = decode_message(event_id, blob_payload, max_decompressed_bytes)?;
    match env.body.first() {
        Some(0) => {}
        Some(&other) => return Err(WorldError::InvalidBaseFlags(other)),
        None => {
            return Err(WorldError::UnexpectedEof {
                needed: 1,
                remaining: 0,
            });
        }
    }
    Ok(EventEnvelope {
        event_id,
        target_entity_id: recipient_id,
        routing_header: env.routing_header,
        inner_payload: env.body,
    })
}

/// Wraps an inner event payload (already starting with `base_flags = 0x00`) into an
/// outgoing `MessageEnvelope`: `envelopeFlags = 0x00` (uncompressed, no routing header)
/// followed by the payload (§1.1.2 send path).
#[must_use]
pub fn encode_event_envelope(inner_payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + inner_payload.len());
    out.push(0x00);
    out.extend_from_slice(inner_payload);
    out
}

// ---------------------------------------------------------------------------
// Type 22 bitfield (§2.7.1), shared by property values and action events
// ---------------------------------------------------------------------------

/// A variable-width bitfield: 3-tier VLE bit count *b*, then `ceil(b / 8)` bytes
/// (`floor((b + 7) / 32)` `u32` words and `ceil(b / 8) mod 4` trailing bytes, §2.7.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireBitfield {
    /// Number of bits *b*.
    pub bit_count: u32,
    /// The `ceil(b / 8)` packed bytes.
    pub bytes: Vec<u8>,
}

impl WireBitfield {
    /// Writes the bitfield, zero-filling or truncating `bytes` to `ceil(b / 8)`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::Vle3Overflow`] if `bit_count >= 0x40000000`.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_vle3(self.bit_count)?;
        let expected = (self.bit_count as usize).div_ceil(8);
        for i in 0..expected {
            writer.write_u8(self.bytes.get(i).copied().unwrap_or(0));
        }
        Ok(())
    }

    /// Reads a bitfield, checking `ceil(b / 8)` against the remaining bytes (§7.1 item 1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on truncation.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let bit_count = reader.read_vle3()?;
        let byte_len = (bit_count as usize).div_ceil(8);
        reader.ensure_count(byte_len, 1)?;
        Ok(Self {
            bit_count,
            bytes: reader.read_bytes(byte_len)?.to_vec(),
        })
    }
}

// ---------------------------------------------------------------------------
// EntityAppearanceDesc (§2.8)
// ---------------------------------------------------------------------------

/// Defined `EntityAppearanceDesc.flags` bits (`0x000007E3`, §2.8).
pub const APPEARANCE_FLAGS_VALID: u32 = 0x0000_07E3;
/// Defined `PartEntry.part_flags` bits (`0x01`, `0x02`, `0x04`, `0x08`, `0x20`, §2.8).
pub const PART_FLAGS_VALID: u32 = 0x0000_002F;

/// One 20-byte tint record (`flags` bit 7, §2.8): five `u32` LE words. The rendering
/// meaning of `field_d` and `field_e` is `OPEN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppearanceTint {
    /// `slot`.
    pub slot: u32,
    /// `key`.
    pub key: u32,
    /// `color_bgra`.
    pub color_bgra: u32,
    /// `field_d` (meaning `OPEN`).
    pub field_d: u32,
    /// `field_e` (meaning `OPEN`).
    pub field_e: u32,
}

/// One `(k, v1, v2)` entry of a `SubPartTable` (§2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SubPartEntry {
    /// `k`.
    pub k: u32,
    /// `v1`.
    pub v1: u32,
    /// `v2`.
    pub v2: u32,
}

/// A `SubPartTable`: TSize (`u8` hint + 3-tier VLE count) of `(u32 k, u32 v1, u32 v2)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubPartTable {
    /// TSize hint (`0..=22`), kept for byte-exact rewriting.
    pub hint: u8,
    /// Entries in stored order.
    pub entries: Vec<SubPartEntry>,
}

/// A `PartEntry` (§2.8): `part_flags` plus the fields its bits select.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartEntry {
    /// `part_id_1..4` (bits `0x01`, `0x02`, `0x04`, `0x08`).
    pub part_ids: [Option<u32>; 4],
    /// `sub_map` (bit `0x20`): `u32` count, then `(u32 sub_key, SubPartTable)` entries.
    pub sub_map: Option<Vec<(u32, SubPartTable)>>,
}

impl PartEntry {
    fn flags(&self) -> u32 {
        let mut f = 0u32;
        for (i, id) in self.part_ids.iter().enumerate() {
            if id.is_some() {
                f |= 1 << i;
            }
        }
        if self.sub_map.is_some() {
            f |= 0x20;
        }
        f
    }

    fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u32_le(self.flags());
        for id in self.part_ids.iter().flatten() {
            writer.write_u32_le(*id);
        }
        if let Some(map) = &self.sub_map {
            let count = u32::try_from(map.len())
                .map_err(|_| WorldError::CountOverflow { count: map.len() })?;
            writer.write_u32_le(count);
            for (key, table) in map {
                writer.write_u32_le(*key);
                writer.write_u8(table.hint);
                let n =
                    u32::try_from(table.entries.len()).map_err(|_| WorldError::CountOverflow {
                        count: table.entries.len(),
                    })?;
                writer.write_vle3(n)?;
                for e in &table.entries {
                    writer.write_u32_le(e.k);
                    writer.write_u32_le(e.v1);
                    writer.write_u32_le(e.v2);
                }
            }
        }
        Ok(())
    }

    fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let flags = reader.read_u32_le()?;
        if flags & !PART_FLAGS_VALID != 0 {
            return Err(WorldError::InvalidAppearanceFlags(flags));
        }
        let mut part_ids = [None; 4];
        for (i, slot) in part_ids.iter_mut().enumerate() {
            if flags & (1 << i) != 0 {
                *slot = Some(reader.read_u32_le()?);
            }
        }
        let sub_map = if flags & 0x20 != 0 {
            let count = reader.read_u32_le()? as usize;
            // Each entry is at least a 4-byte key and a 2-byte empty TSize.
            reader.ensure_count(count, 6)?;
            let mut map = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
            for _ in 0..count {
                let key = reader.read_u32_le()?;
                let hint = reader.read_u8()?;
                if hint > 22 {
                    return Err(WorldError::InvalidTSizeHint(hint));
                }
                let n = reader.read_vle3()? as usize;
                reader.ensure_count(n, 12)?;
                let mut entries = Vec::with_capacity(n.min(MAX_PREALLOC_ENTRIES));
                for _ in 0..n {
                    entries.push(SubPartEntry {
                        k: reader.read_u32_le()?,
                        v1: reader.read_u32_le()?,
                        v2: reader.read_u32_le()?,
                    });
                }
                map.push((key, SubPartTable { hint, entries }));
            }
            Some(map)
        } else {
            None
        };
        Ok(Self { part_ids, sub_map })
    }
}

/// Visual model, scale and part overrides of a character or entity (§2.8; used in
/// [`CharacterSummaryEntry`] and `0x89`). Each `Option` field is present exactly when its
/// `flags` bit is set; the encoder derives `flags` from the fields.
///
/// Bit 0 (`base_model`) cannot be decoded: which high-byte value of `model_a` makes
/// `model_b` follow is `OPEN` (§2.8), so a descriptor with bit 0 is refused with
/// [`WorldError::UnsupportedAppearanceBaseModel`]. The encoder writes the given words as
/// they are (synthetic tests only).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityAppearanceDesc {
    /// Bit 0 (`0x0001`): `model_a` and, when present, `model_b`.
    pub base_model: Option<(u32, Option<u32>)>,
    /// Bit 1 (`0x0002`): `secondary_did`.
    pub secondary_did: Option<u32>,
    /// Bit 5 (`0x0020`): `scale` (`sx, sy, sz`; `(1, 1, 1)` when absent).
    pub scale: Option<[f32; 3]>,
    /// Bit 6 (`0x0040`): `param_a` (`1.0` when absent).
    pub param_a: Option<f32>,
    /// Bit 7 (`0x0080`): `tint_entries`.
    pub tint_entries: Option<Vec<AppearanceTint>>,
    /// Bit 8 (`0x0100`): `primary_part`.
    pub primary_part: Option<PartEntry>,
    /// Bit 9 (`0x0200`): `part_map` (TSize hint + `(u32 key, PartEntry)` entries; the hint
    /// byte is kept even on the wire).
    pub part_map: Option<(u8, Vec<(u32, PartEntry)>)>,
    /// Bit 10 (`0x0400`): `param_b` (`1.0` when absent).
    pub param_b: Option<f32>,
}

impl EntityAppearanceDesc {
    /// The `flags` word implied by the present fields.
    #[must_use]
    pub fn flags(&self) -> u32 {
        let mut f = 0u32;
        if self.base_model.is_some() {
            f |= 0x0001;
        }
        if self.secondary_did.is_some() {
            f |= 0x0002;
        }
        if self.scale.is_some() {
            f |= 0x0020;
        }
        if self.param_a.is_some() {
            f |= 0x0040;
        }
        if self.tint_entries.is_some() {
            f |= 0x0080;
        }
        if self.primary_part.is_some() {
            f |= 0x0100;
        }
        if self.part_map.is_some() {
            f |= 0x0200;
        }
        if self.param_b.is_some() {
            f |= 0x0400;
        }
        f
    }

    /// Writes the descriptor (§2.8 field order).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] if a count cannot be represented.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u32_le(self.flags());
        if let Some((a, b)) = self.base_model {
            writer.write_u32_le(a);
            if let Some(b) = b {
                writer.write_u32_le(b);
            }
        }
        if let Some(did) = self.secondary_did {
            writer.write_u32_le(did);
        }
        if let Some([x, y, z]) = self.scale {
            writer.write_f32_le(x);
            writer.write_f32_le(y);
            writer.write_f32_le(z);
        }
        if let Some(a) = self.param_a {
            writer.write_f32_le(a);
        }
        if let Some(tints) = &self.tint_entries {
            let count = u32::try_from(tints.len())
                .map_err(|_| WorldError::CountOverflow { count: tints.len() })?;
            writer.write_u32_le(count);
            for t in tints {
                for w in [t.slot, t.key, t.color_bgra, t.field_d, t.field_e] {
                    writer.write_u32_le(w);
                }
            }
        }
        if let Some(part) = &self.primary_part {
            part.encode(writer)?;
        }
        if let Some((hint, map)) = &self.part_map {
            writer.write_u8(*hint);
            let n = u32::try_from(map.len())
                .map_err(|_| WorldError::CountOverflow { count: map.len() })?;
            writer.write_vle3(n)?;
            for (key, part) in map {
                writer.write_u32_le(*key);
                part.encode(writer)?;
            }
        }
        if let Some(b) = self.param_b {
            writer.write_f32_le(b);
        }
        Ok(())
    }

    /// Reads a descriptor (§2.8).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::InvalidAppearanceFlags`] for bits outside `0x000007E3`,
    /// [`WorldError::UnsupportedAppearanceBaseModel`] for bit 0, or a framing error.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let flags = reader.read_u32_le()?;
        if flags & !APPEARANCE_FLAGS_VALID != 0 {
            return Err(WorldError::InvalidAppearanceFlags(flags));
        }
        if flags & 0x0001 != 0 {
            tracing::debug!("EntityAppearanceDesc with base_model refused (model_b gate OPEN)");
            return Err(WorldError::UnsupportedAppearanceBaseModel);
        }
        let secondary_did = (flags & 0x0002 != 0)
            .then(|| reader.read_u32_le())
            .transpose()?;
        let scale = if flags & 0x0020 != 0 {
            Some([
                reader.read_f32_le()?,
                reader.read_f32_le()?,
                reader.read_f32_le()?,
            ])
        } else {
            None
        };
        let param_a = (flags & 0x0040 != 0)
            .then(|| reader.read_f32_le())
            .transpose()?;
        let tint_entries = if flags & 0x0080 != 0 {
            let count = reader.read_u32_le()? as usize;
            reader.ensure_count(count, 20)?;
            let mut tints = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
            for _ in 0..count {
                tints.push(AppearanceTint {
                    slot: reader.read_u32_le()?,
                    key: reader.read_u32_le()?,
                    color_bgra: reader.read_u32_le()?,
                    field_d: reader.read_u32_le()?,
                    field_e: reader.read_u32_le()?,
                });
            }
            Some(tints)
        } else {
            None
        };
        let primary_part = (flags & 0x0100 != 0)
            .then(|| PartEntry::decode(reader))
            .transpose()?;
        let part_map = if flags & 0x0200 != 0 {
            let hint = reader.read_u8()?;
            if hint > 22 {
                return Err(WorldError::InvalidTSizeHint(hint));
            }
            let n = reader.read_vle3()? as usize;
            reader.ensure_count(n, 8)?;
            let mut map = Vec::with_capacity(n.min(MAX_PREALLOC_ENTRIES));
            for _ in 0..n {
                let key = reader.read_u32_le()?;
                map.push((key, PartEntry::decode(reader)?));
            }
            Some((hint, map))
        } else {
            None
        };
        let param_b = (flags & 0x0400 != 0)
            .then(|| reader.read_f32_le())
            .transpose()?;
        Ok(Self {
            base_model: None,
            secondary_did,
            scale,
            param_a,
            tint_entries,
            primary_part,
            part_map,
            param_b,
        })
    }
}

// ---------------------------------------------------------------------------
// Character list, creation, deletion and selection (§3)
// ---------------------------------------------------------------------------

/// One character descriptor of `0x6C` and `0x93` (`CharacterSummaryEntry`, §3.1.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterSummaryEntry {
    /// `character_id` (`u64` LE `InstanceID`).
    pub character_id: u64,
    /// `name` (prefixed UTF-16LE string).
    pub name: String,
    /// `status_or_world` (`u32` LE).
    pub status_or_world: u32,
    /// `public_props` (§3.1.3).
    pub public_props: EncodedPropertyTable,
    /// `private_props` (§3.1.3).
    pub private_props: EncodedPropertyTable,
    /// `metadata_a` (`u32` LE).
    pub metadata_a: u32,
    /// `visual_desc` (§2.8).
    pub visual_desc: EntityAppearanceDesc,
    /// `metadata_b` (`u32` LE).
    pub metadata_b: u32,
    /// `metadata_c` (`u32` LE, usually `0xFFFFFFFF`).
    pub metadata_c: u32,
    /// `metadata_d` (`u32` LE).
    pub metadata_d: u32,
    /// `metadata_e` (`u32` LE, usually `0xFFFFFFFF`).
    pub metadata_e: u32,
    /// `timestamp_or_meta` (`u64` LE, always present).
    pub timestamp_or_meta: u64,
}

impl CharacterSummaryEntry {
    /// Smallest encoding: id, empty name, status, two empty tables, metadata A, an empty
    /// appearance (`flags` only), metadata B–E and the 64-bit field.
    pub const MIN_WIRE_LEN: usize = 8 + 1 + 4 + 1 + 1 + 4 + 4 + 16 + 8;

    /// The first value of `pid` in `public_props`, else in `private_props` (§3.1.3; the §6
    /// adapter matches the `PID_*` constants of this module).
    #[must_use]
    pub fn property(&self, pid: u32) -> Option<&EncodedPropertyValue> {
        self.public_props
            .get(pid)
            .or_else(|| self.private_props.get(pid))
    }

    /// Writes the entry (§3.1.2 field order).
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u64_le(self.character_id);
        writer.write_utf16le_string(&self.name)?;
        writer.write_u32_le(self.status_or_world);
        self.public_props.encode(writer)?;
        self.private_props.encode(writer)?;
        writer.write_u32_le(self.metadata_a);
        self.visual_desc.encode(writer)?;
        for w in [
            self.metadata_b,
            self.metadata_c,
            self.metadata_d,
            self.metadata_e,
        ] {
            writer.write_u32_le(w);
        }
        writer.write_u64_le(self.timestamp_or_meta);
        Ok(())
    }

    /// Reads an entry using `schema` (§3.1.2).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on any framing or schema error.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        Ok(Self {
            character_id: reader.read_u64_le()?,
            name: reader.read_utf16le_string()?,
            status_or_world: reader.read_u32_le()?,
            public_props: EncodedPropertyTable::decode(reader, schema)?,
            private_props: EncodedPropertyTable::decode(reader, schema)?,
            metadata_a: reader.read_u32_le()?,
            visual_desc: EntityAppearanceDesc::decode(reader)?,
            metadata_b: reader.read_u32_le()?,
            metadata_c: reader.read_u32_le()?,
            metadata_d: reader.read_u32_le()?,
            metadata_e: reader.read_u32_le()?,
            timestamp_or_meta: reader.read_u64_le()?,
        })
    }
}

/// Event `0x6C` (`character_list_response`, S→C): the account's characters and account
/// properties (§3.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterListResponse {
    /// `characters` (`u32` LE count + entries).
    pub characters: Vec<CharacterSummaryEntry>,
    /// `account_props` (§3.1.3).
    pub account_props: EncodedPropertyTable,
}

impl CharacterListResponse {
    /// Numeric event ID (`0x0000006C`).
    pub const EVENT_ID: u32 = 0x0000_006C;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        let count =
            u32::try_from(self.characters.len()).map_err(|_| WorldError::CountOverflow {
                count: self.characters.len(),
            })?;
        writer.write_u32_le(count);
        for ch in &self.characters {
            ch.encode(writer)?;
        }
        self.account_props.encode(writer)
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on any framing or schema error.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let count = reader.read_u32_le()? as usize;
        reader.ensure_count(count, CharacterSummaryEntry::MIN_WIRE_LEN)?;
        let mut characters = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..count {
            characters.push(CharacterSummaryEntry::decode(reader, schema)?);
        }
        Ok(Self {
            characters,
            account_props: EncodedPropertyTable::decode(reader, schema)?,
        })
    }
}
schema_codec!(CharacterListResponse);

/// Event `0x154` (`character_name_check_request`, C→S, §3.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterNameCheckRequest {
    /// Proposed character name.
    pub name: String,
}

impl CharacterNameCheckRequest {
    /// Numeric event ID (`0x00000154`).
    pub const EVENT_ID: u32 = 0x0000_0154;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::Vle3Overflow`] for an over-long name.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_utf16le_string(&self.name)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            name: reader.read_utf16le_string()?,
        })
    }
}
plain_codec!(CharacterNameCheckRequest);

/// Event `0xDC` (`character_create_request`, C→S, §3.2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreateRequest {
    /// `creation_props` (§3.2.5).
    pub creation_props: EncodedPropertyTable,
    /// Requested character name.
    pub name: String,
    /// `char_gen_type` (`1` Free Peoples, `2` Monster Play, `4` session play; §6 uses `1`).
    pub char_gen_type: u8,
    /// `flag_a` (`bool8`).
    pub flag_a: bool,
    /// `flag_b` (`bool8`).
    pub flag_b: bool,
    /// `creation_param` (`u32` LE, default `0`).
    pub creation_param: u32,
}

impl CharacterCreateRequest {
    /// Numeric event ID (`0x000000DC`).
    pub const EVENT_ID: u32 = 0x0000_00DC;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.creation_props.encode(writer)?;
        writer.write_utf16le_string(&self.name)?;
        writer.write_u8(self.char_gen_type);
        writer.write_bool8(self.flag_a);
        writer.write_bool8(self.flag_b);
        writer.write_u32_le(self.creation_param);
        Ok(())
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing, schema or `bool8` errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            creation_props: EncodedPropertyTable::decode(reader, schema)?,
            name: reader.read_utf16le_string()?,
            char_gen_type: reader.read_u8()?,
            flag_a: reader.read_bool8()?,
            flag_b: reader.read_bool8()?,
            creation_param: reader.read_u32_le()?,
        })
    }
}
schema_codec!(CharacterCreateRequest);

/// Event `0x93` (`character_create_ack`, S→C, §3.2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreateAck {
    /// The newly created character.
    pub character: CharacterSummaryEntry,
}

impl CharacterCreateAck {
    /// Numeric event ID (`0x00000093`).
    pub const EVENT_ID: u32 = 0x0000_0093;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.character.encode(writer)
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or schema errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            character: CharacterSummaryEntry::decode(reader, schema)?,
        })
    }
}
schema_codec!(CharacterCreateAck);

/// Event `0x79` (`character_create_reject`, S→C, §3.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterCreateReject {
    /// `result_code` (`u32` LE).
    pub result_code: u32,
    /// `name_error_code` (`u32` LE).
    pub name_error_code: u32,
}

impl CharacterCreateReject {
    /// Numeric event ID (`0x00000079`).
    pub const EVENT_ID: u32 = 0x0000_0079;

    /// Writes the 9-byte inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.result_code);
        writer.write_u32_le(self.name_error_code);
        Ok(())
    }

    /// Reads the 9-byte inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            result_code: reader.read_u32_le()?,
            name_error_code: reader.read_u32_le()?,
        })
    }
}
plain_codec!(CharacterCreateReject);

/// Events `0x7A` (`character_delete_request`) and `0xD1` (`monster_character_delete_request`),
/// C→S, 9 bytes (§3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDeleteRequest {
    /// `character_id`.
    pub character_id: u64,
    /// `true` for a Monster Play character (`0xD1`), `false` for Free Peoples (`0x7A`).
    pub monster_play: bool,
}

impl CharacterDeleteRequest {
    /// Event ID for a Free Peoples character (`0x0000007A`).
    pub const EVENT_ID_FREE_PEOPLES: u32 = 0x0000_007A;
    /// Event ID for a Monster Play character (`0x000000D1`).
    pub const EVENT_ID_MONSTER_PLAY: u32 = 0x0000_00D1;

    /// The top-level event ID for this request.
    #[must_use]
    pub const fn event_id(&self) -> u32 {
        if self.monster_play {
            Self::EVENT_ID_MONSTER_PLAY
        } else {
            Self::EVENT_ID_FREE_PEOPLES
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.character_id);
        Ok(())
    }

    /// Reads the inner payload of event `event_id` (`0x7A` or `0xD1`).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another ID, or a framing error.
    pub fn decode(reader: &mut WireReader<'_>, event_id: u32) -> Result<Self, WorldError> {
        let monster_play = match event_id {
            Self::EVENT_ID_FREE_PEOPLES => false,
            Self::EVENT_ID_MONSTER_PLAY => true,
            other => return Err(WorldError::WrongEventId(other)),
        };
        reader.read_base_flags()?;
        Ok(Self {
            character_id: reader.read_u64_le()?,
            monster_play,
        })
    }

    /// Encodes to a standalone byte vector.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes event `event_id` from a standalone byte slice.
    ///
    /// # Errors
    ///
    /// See [`Self::decode`]; trailing bytes are an error.
    pub fn from_bytes(bytes: &[u8], event_id: u32) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, event_id)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x15` (`character_delete_status`, S→C, 13 bytes, §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDeleteStatus {
    /// `character_id`.
    pub character_id: u64,
    /// `result_code` (`i32` LE; `0` means deleted, §6).
    pub result_code: i32,
}

impl CharacterDeleteStatus {
    /// Numeric event ID (`0x00000015`).
    pub const EVENT_ID: u32 = 0x0000_0015;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.character_id);
        writer.write_i32_le(self.result_code);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            character_id: reader.read_u64_le()?,
            result_code: reader.read_i32_le()?,
        })
    }
}
plain_codec!(CharacterDeleteStatus);

/// Event `0x6D` (`character_operation_error`, S→C, 5 bytes, §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterOperationError {
    /// `error_code` (`u32` LE).
    pub error_code: u32,
}

impl CharacterOperationError {
    /// Numeric event ID (`0x0000006D`).
    pub const EVENT_ID: u32 = 0x0000_006D;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.error_code);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            error_code: reader.read_u32_le()?,
        })
    }
}
plain_codec!(CharacterOperationError);

/// Event `0xB3` (`account_eviction_notice`, S→C, §3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountEvictionNotice {
    /// `boot_reason` (`u32` LE).
    pub boot_reason: u32,
    /// `reason_text` (wire `LocalizedTokenText`).
    pub reason_text: LocalizedTokenText,
}

impl AccountEvictionNotice {
    /// Numeric event ID (`0x000000B3`).
    pub const EVENT_ID: u32 = 0x0000_00B3;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates text encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.boot_reason);
        self.reason_text.encode(writer)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            boot_reason: reader.read_u32_le()?,
            reason_text: LocalizedTokenText::decode(reader)?,
        })
    }
}
plain_codec!(AccountEvictionNotice);

/// Event `0x19E` (`platform_account_id_report`, C→S, 9 bytes, §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformAccountIdReport {
    /// `platform_id` (`u64` LE).
    pub platform_id: u64,
}

impl PlatformAccountIdReport {
    /// Numeric event ID (`0x0000019E`).
    pub const EVENT_ID: u32 = 0x0000_019E;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.platform_id);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            platform_id: reader.read_u64_le()?,
        })
    }
}
plain_codec!(PlatformAccountIdReport);

/// Event `0x6F` (`character_enter_world_request`, C→S, §3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterEnterWorldRequest {
    /// `character_id`.
    pub character_id: u64,
    /// `client_props` (empty `0x00` unless entry properties are attached).
    pub client_props: EncodedPropertyTable,
}

impl CharacterEnterWorldRequest {
    /// Numeric event ID (`0x0000006F`).
    pub const EVENT_ID: u32 = 0x0000_006F;

    /// A request with an empty `client_props` table.
    #[must_use]
    pub fn new(character_id: u64) -> Self {
        Self {
            character_id,
            client_props: EncodedPropertyTable::new(),
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.character_id);
        self.client_props.encode(writer)
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or schema errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            character_id: reader.read_u64_le()?,
            client_props: EncodedPropertyTable::decode(reader, schema)?,
        })
    }

    /// The outgoing `MessageEnvelope` (`envelopeFlags = 0x00` + inner payload; §7.2 Vector 7).
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn to_envelope_bytes(&self) -> Result<Vec<u8>, WorldError> {
        Ok(encode_event_envelope(&self.to_bytes()?))
    }
}
schema_codec!(CharacterEnterWorldRequest);

// ---------------------------------------------------------------------------
// World entry, portal space and session synchronization (§4)
// ---------------------------------------------------------------------------

/// Generates an event whose inner payload is only `u8 base_flags = 0x00`.
macro_rules! empty_event {
    ($(#[$doc:meta])* $name:ident, $id:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        pub struct $name;

        impl $name {
            /// Numeric event ID.
            pub const EVENT_ID: u32 = $id;

            /// Writes the 1-byte inner payload (`0x00`).
            ///
            /// # Errors
            ///
            /// Never fails.
            pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
                writer.write_base_flags();
                Ok(())
            }

            /// Reads the 1-byte inner payload (`0x00`).
            ///
            /// # Errors
            ///
            /// Returns [`WorldError`] if the byte is missing or not `0x00`.
            pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
                reader.read_base_flags()?;
                Ok(Self)
            }
        }
        plain_codec!($name);
    };
}

/// Event `0x43` (`world_name_announcement`, S→C, §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldNameAnnouncement {
    /// `world_name`.
    pub world_name: LocalizedTokenText,
}

impl WorldNameAnnouncement {
    /// Numeric event ID (`0x00000043`).
    pub const EVENT_ID: u32 = 0x0000_0043;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates text encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.world_name.encode(writer)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            world_name: LocalizedTokenText::decode(reader)?,
        })
    }
}
plain_codec!(WorldNameAnnouncement);

/// Event `0xAA` (`display_localized_text`, S→C, §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayLocalizedText {
    /// `target_or_channel_mask` (`u64` LE): target entity or chat-channel selector.
    pub target_or_channel_mask: u64,
    /// `message`.
    pub message: LocalizedTokenText,
}

impl DisplayLocalizedText {
    /// Numeric event ID (`0x000000AA`).
    pub const EVENT_ID: u32 = 0x0000_00AA;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates text encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.target_or_channel_mask);
        self.message.encode(writer)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            target_or_channel_mask: reader.read_u64_le()?,
            message: LocalizedTokenText::decode(reader)?,
        })
    }
}
plain_codec!(DisplayLocalizedText);

/// Event `0xFA` (`server_shutdown_text`, S→C, §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerShutdownText {
    /// `last_words`.
    pub last_words: LocalizedTokenText,
}

impl ServerShutdownText {
    /// Numeric event ID (`0x000000FA`).
    pub const EVENT_ID: u32 = 0x0000_00FA;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates text encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.last_words.encode(writer)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            last_words: LocalizedTokenText::decode(reader)?,
        })
    }
}
plain_codec!(ServerShutdownText);

/// Event `0x90` (`player_session_ready`, S→C, §4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSessionReady {
    /// `player_id`: the active player avatar's `InstanceID`.
    pub player_id: u64,
    /// `session_props`.
    pub session_props: EncodedPropertyTable,
}

impl PlayerSessionReady {
    /// Numeric event ID (`0x00000090`).
    pub const EVENT_ID: u32 = 0x0000_0090;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates value encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.player_id);
        self.session_props.encode(writer)
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or schema errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            player_id: reader.read_u64_le()?,
            session_props: EncodedPropertyTable::decode(reader, schema)?,
        })
    }
}
schema_codec!(PlayerSessionReady);

/// The boolean and enum state `PID`s of one `0xCC` state schema, built at run time from the
/// player-physics script table `0x07000029` and its `0x0C` scriptlets (§4.3.1 construction
/// algorithm). Order matters: bit `i` of the packed booleans is `bool_pids[i]`, and the
/// enum VLEs follow `enum_pids` in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortalStateSchema {
    /// Boolean (Type 21) condition `PID`s.
    pub bool_pids: Vec<u32>,
    /// Enum (Type 8) condition `PID`s.
    pub enum_pids: Vec<u32>,
}

/// Run-time lookup of `0xCC` state schemas by `state_schema_id` (§4.3.1).
pub trait PortalStateSchemaLookup {
    /// The schema for `state_schema_id`, or `None` if it is unknown.
    fn portal_state_schema(&self, state_schema_id: u32) -> Option<&PortalStateSchema>;
}

impl PortalStateSchemaLookup for BTreeMap<u32, PortalStateSchema> {
    fn portal_state_schema(&self, state_schema_id: u32) -> Option<&PortalStateSchema> {
        self.get(&state_schema_id)
    }
}

impl<S: std::hash::BuildHasher> PortalStateSchemaLookup for HashMap<u32, PortalStateSchema, S> {
    fn portal_state_schema(&self, state_schema_id: u32) -> Option<&PortalStateSchema> {
        self.get(&state_schema_id)
    }
}

/// Event `0xCC` (`portal_position_sync`, S→C): portal / teleport placement and physics
/// state (§4.3.1, Vector 8).
#[derive(Debug, Clone, PartialEq)]
pub struct PortalPositionSync {
    /// `sync_flags` bit 0 (`sync_bit_0`; its effect is `OPEN`). Bits 1–6 follow from the
    /// velocity fields and bit 7 from `state_schema_id`.
    pub sync_bit_0: bool,
    /// `sequence_or_reason` (`u32` LE).
    pub sequence_or_reason: u32,
    /// Base wire `Position` (no quaternion).
    pub position: WirePosition,
    /// Quantized heading (§2.5.2).
    pub heading_raw: u8,
    /// `movement_mode`.
    pub movement_mode: u8,
    /// `vel_x`, `vel_y`, `vel_z` (bits `0x02`, `0x04`, `0x08`; `0.0` when absent).
    pub linear_velocity: [Option<f32>; 3],
    /// `ang_vel_x`, `ang_vel_y`, `ang_vel_z` (bits `0x10`, `0x20`, `0x40`).
    pub angular_velocity: [Option<f32>; 3],
    /// `anim_or_context_did` (Compact `DID`).
    pub anim_or_context_did: CompactDid,
    /// `state_schema_id` (written, with bit 7, only when it is not
    /// [`DEFAULT_PORTAL_STATE_SCHEMA_ID`]).
    pub state_schema_id: u32,
    /// Boolean states, one per `bool_pids` entry of the schema.
    pub bool_states: Vec<bool>,
    /// Enum states, one per `enum_pids` entry of the schema.
    pub enum_states: Vec<u32>,
    /// `extra_blob` (`u32` LE size + bytes).
    pub extra_blob: Vec<u8>,
}

impl PortalPositionSync {
    /// Numeric event ID (`0x000000CC`).
    pub const EVENT_ID: u32 = 0x0000_00CC;

    /// The boolean state of `pid` under `schema`, if `pid` is one of its boolean `PID`s.
    #[must_use]
    pub fn bool_state_for_pid(&self, schema: &PortalStateSchema, pid: u32) -> Option<bool> {
        let idx = schema.bool_pids.iter().position(|&p| p == pid)?;
        self.bool_states.get(idx).copied()
    }

    /// The enum state of `pid` under `schema`, if `pid` is one of its enum `PID`s.
    #[must_use]
    pub fn enum_state_for_pid(&self, schema: &PortalStateSchema, pid: u32) -> Option<u32> {
        let idx = schema.enum_pids.iter().position(|&p| p == pid)?;
        self.enum_states.get(idx).copied()
    }

    fn sync_flags(&self) -> u8 {
        let mut f = u8::from(self.sync_bit_0);
        for (i, v) in self.linear_velocity.iter().enumerate() {
            if v.is_some() {
                f |= 0x02 << i;
            }
        }
        for (i, v) in self.angular_velocity.iter().enumerate() {
            if v.is_some() {
                f |= 0x10 << i;
            }
        }
        if self.state_schema_id != DEFAULT_PORTAL_STATE_SCHEMA_ID {
            f |= 0x80;
        }
        f
    }

    /// Writes the inner payload, checking the state counts against `schemas` (§4.3.1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnknownPortalStateSchema`] or
    /// [`WorldError::PortalStateSchemaLengthMismatch`].
    pub fn encode_with(
        &self,
        writer: &mut WireWriter,
        schemas: &dyn PortalStateSchemaLookup,
    ) -> Result<(), WorldError> {
        let schema = schemas
            .portal_state_schema(self.state_schema_id)
            .ok_or(WorldError::UnknownPortalStateSchema(self.state_schema_id))?;
        if self.bool_states.len() != schema.bool_pids.len()
            || self.enum_states.len() != schema.enum_pids.len()
        {
            return Err(WorldError::PortalStateSchemaLengthMismatch {
                schema_id: self.state_schema_id,
                expected_bools: schema.bool_pids.len(),
                actual_bools: self.bool_states.len(),
                expected_enums: schema.enum_pids.len(),
                actual_enums: self.enum_states.len(),
            });
        }
        writer.write_base_flags();
        let flags = self.sync_flags();
        writer.write_u8(flags);
        writer.write_u32_le(self.sequence_or_reason);
        self.position.encode_base(writer);
        writer.write_u8(self.heading_raw);
        writer.write_u8(self.movement_mode);
        for v in self
            .linear_velocity
            .iter()
            .chain(self.angular_velocity.iter())
            .flatten()
        {
            writer.write_f32_le(*v);
        }
        self.anim_or_context_did.encode(writer)?;
        if flags & 0x80 != 0 {
            writer.write_u32_le(self.state_schema_id);
        }
        for chunk in self.bool_states.chunks(8) {
            let mut packed = 0u8;
            for (bit, &on) in chunk.iter().enumerate() {
                if on {
                    packed |= 1 << bit;
                }
            }
            writer.write_u8(packed);
        }
        for &val in &self.enum_states {
            writer.write_vle3(val)?;
        }
        let extra_len =
            u32::try_from(self.extra_blob.len()).map_err(|_| WorldError::CountOverflow {
                count: self.extra_blob.len(),
            })?;
        writer.write_u32_le(extra_len);
        writer.write_bytes(&self.extra_blob);
        Ok(())
    }

    /// Reads the inner payload; the state section is framed by the schema `schemas` gives
    /// for the packet's `state_schema_id` (§4.3.1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnknownPortalStateSchema`] for an unknown schema, a non-finite
    /// position or velocity error, or a framing error.
    pub fn decode_with(
        reader: &mut WireReader<'_>,
        schemas: &dyn PortalStateSchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let flags = reader.read_u8()?;
        let sequence_or_reason = reader.read_u32_le()?;
        let position = WirePosition::decode_base(reader)?;
        for (field, v) in ["pos_x", "pos_y", "pos_z"].into_iter().zip(position.offset) {
            require_finite(field, v)?;
        }
        let heading_raw = reader.read_u8()?;
        let movement_mode = reader.read_u8()?;
        let names = [
            "vel_x",
            "vel_y",
            "vel_z",
            "ang_vel_x",
            "ang_vel_y",
            "ang_vel_z",
        ];
        let mut kinematics = [None; 6];
        for (bit, (slot, field)) in kinematics.iter_mut().zip(names).enumerate() {
            if flags & (0x02 << bit) != 0 {
                *slot = Some(require_finite(field, reader.read_f32_le()?)?);
            }
        }
        let anim_or_context_did = CompactDid::decode(reader)?;
        let state_schema_id = if flags & 0x80 != 0 {
            reader.read_u32_le()?
        } else {
            DEFAULT_PORTAL_STATE_SCHEMA_ID
        };
        let schema = schemas
            .portal_state_schema(state_schema_id)
            .ok_or(WorldError::UnknownPortalStateSchema(state_schema_id))?;
        let n_bool = schema.bool_pids.len();
        let raw_bool = reader.read_bytes(n_bool.div_ceil(8))?;
        let bool_states = (0..n_bool)
            .map(|i| raw_bool.get(i / 8).is_some_and(|b| (b >> (i % 8)) & 1 != 0))
            .collect();
        let n_enum = schema.enum_pids.len();
        reader.ensure_count(n_enum, 1)?;
        let mut enum_states = Vec::with_capacity(n_enum.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..n_enum {
            enum_states.push(reader.read_vle3()?);
        }
        let extra_blob_size = reader.read_u32_le()? as usize;
        reader.ensure_count(extra_blob_size, 1)?;
        let extra_blob = reader.read_bytes(extra_blob_size)?.to_vec();
        let [vx, vy, vz, ax, ay, az] = kinematics;
        Ok(Self {
            sync_bit_0: flags & 0x01 != 0,
            sequence_or_reason,
            position,
            heading_raw,
            movement_mode,
            linear_velocity: [vx, vy, vz],
            angular_velocity: [ax, ay, az],
            anim_or_context_did,
            state_schema_id,
            bool_states,
            enum_states,
            extra_blob,
        })
    }

    /// Encodes to a standalone byte vector.
    ///
    /// # Errors
    ///
    /// See [`Self::encode_with`].
    pub fn to_bytes(&self, schemas: &dyn PortalStateSchemaLookup) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode_with(&mut w, schemas)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice; trailing bytes are an error.
    ///
    /// # Errors
    ///
    /// See [`Self::decode_with`].
    pub fn from_bytes(
        bytes: &[u8],
        schemas: &dyn PortalStateSchemaLookup,
    ) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode_with(&mut r, schemas)?;
        r.finish()?;
        Ok(msg)
    }
}

empty_event!(
    /// Event `0xC7` (`portal_transition_complete`, S→C, §4.3.1).
    PortalTransitionComplete,
    0x0000_00C7
);
empty_event!(
    /// Event `0x153` (`portal_state_notice`, S→C, §4.3.1).
    PortalStateNotice,
    0x0000_0153
);
empty_event!(
    /// Event `0xF4` (`portal_ready_ack`, C→S, §4.3.1, Vector 7): sent once the destination
    /// landblock or cell has loaded.
    PortalReadyAck,
    0x0000_00F4
);

/// The network clock of §4.4.1: latency-compensated samples from `ClockSyncHeader`
/// (`0x01000000`), forward snaps, and the 5-tier downward slew that never moves time
/// backward.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetworkClock {
    synced_time: f64,
    raw_elapsed: f64,
    clock_offset: f64,
    slew_rate: f64,
    slew_debt: f64,
    prev_server_time: f64,
    pending: Option<(f64, f64, bool)>,
}

impl NetworkClock {
    /// A clock that reads `raw_time` until the first sample arrives.
    #[must_use]
    pub fn new(raw_time: f64) -> Self {
        Self {
            synced_time: raw_time,
            raw_elapsed: raw_time,
            ..Self::default()
        }
    }

    /// The current synchronized network time (`synced_time`, seconds).
    #[must_use]
    pub const fn synced_time(&self) -> f64 {
        self.synced_time
    }

    /// The current downward slew rate (seconds per second).
    #[must_use]
    pub const fn slew_rate(&self) -> f64 {
        self.slew_rate
    }

    /// The excess still to be bled off (seconds).
    #[must_use]
    pub const fn slew_debt(&self) -> f64 {
        self.slew_debt
    }

    /// The base slew rate for `excess = synced_time - target` (§4.4.1 tier table).
    #[must_use]
    pub fn base_slew_rate_for_excess(excess: f64) -> f64 {
        if excess < 0.015 {
            0.0
        } else if excess < 0.030 {
            0.00025
        } else if excess < 0.060 {
            0.0025
        } else if excess < 0.120 {
            0.01
        } else if excess < 0.240 {
            0.025
        } else {
            0.05
        }
    }

    /// Records a `ClockSyncHeader` sample `server_time` that arrived at local time `t_recv`;
    /// the next [`Self::update`] applies it.
    pub fn on_clock_sync(&mut self, server_time: f64, t_recv: f64) {
        self.pending = Some((server_time, t_recv, false));
    }

    /// Records a forced sample (for example the `ConnectChallengeHeader` `serverTime` seed):
    /// it always snaps.
    pub fn on_forced_sync(&mut self, server_time: f64, t_recv: f64) {
        self.pending = Some((server_time, t_recv, true));
    }

    /// Advances to local time `t_now`, bleeding off slew debt and applying a pending sample
    /// (`target = server_time + (t_now - t_recv)`), and returns `synced_time` (§4.4.1).
    pub fn update(&mut self, t_now: f64) -> f64 {
        let dt = (t_now - self.raw_elapsed).max(0.0);
        self.raw_elapsed = self.raw_elapsed.max(t_now);
        if self.slew_debt > 0.0 && dt > 0.0 {
            let step = self.slew_debt.min(dt * self.slew_rate);
            self.slew_debt -= step;
            self.clock_offset -= step;
        }
        self.synced_time = self.raw_elapsed + self.clock_offset;

        if let Some((server_time, t_recv, forced)) = self.pending.take() {
            let target = server_time + (t_now - t_recv);
            if forced || target > self.synced_time {
                self.synced_time = target;
                self.clock_offset = target - self.raw_elapsed;
                self.slew_rate = 0.0;
                self.slew_debt = 0.0;
                self.prev_server_time = 0.0;
            } else {
                let excess = self.synced_time - target;
                if excess < 0.015 {
                    self.slew_rate = 0.0;
                    self.slew_debt = 0.0;
                    self.prev_server_time = 0.0;
                } else {
                    self.slew_debt = excess;
                    let mut rate = Self::base_slew_rate_for_excess(excess);
                    let span = target - self.prev_server_time;
                    if self.prev_server_time > 0.0 && span > 0.0 && excess / span > rate {
                        rate += 0.0002;
                    }
                    self.slew_rate = rate;
                    self.prev_server_time = target;
                }
            }
        }
        self.synced_time
    }

    /// A wire Type 4 value to a client timestamp: relative values add `synced_time`;
    /// absolute ones (definition flag `0x10`) and "no time" pass through (§4.4.1 item 3).
    #[must_use]
    pub fn decode_wire_timestamp(&self, wire_val: f64, is_absolute: bool) -> f64 {
        if wire_val == TIMESTAMP_NO_TIME || is_absolute {
            wire_val
        } else {
            wire_val + self.synced_time
        }
    }

    /// A client timestamp to its wire Type 4 value (§4.4.1 item 3).
    #[must_use]
    pub fn encode_wire_timestamp(&self, timestamp: f64, is_absolute: bool) -> f64 {
        if timestamp == TIMESTAMP_NO_TIME || is_absolute {
            timestamp
        } else {
            timestamp - self.synced_time
        }
    }
}

/// The in-game calendar record `0x35000000` of `client_general.dat` (§4.4.2), read at run
/// time. The spec's table gives the record's verification values; none is built in.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldCalendarConfig {
    /// Record DID at `+0x00` (the DID [`CALENDAR_DIRECTORY_KEY`] resolved to).
    pub record_did: u32,
    /// `day_length_seconds` (`f32`, `> 0`).
    pub day_length_seconds: f32,
    /// `days_per_year` (`i32`, `> 0`).
    pub days_per_year: i32,
    /// `calendar_epoch_offset` (`f64` seconds).
    pub calendar_epoch_offset: f64,
    /// `base_year`.
    pub base_year: i32,
    /// `dusk_threshold` (fraction of the day at which night begins).
    pub dusk_threshold: f32,
    /// `dawn_threshold` (fraction of the day at which day begins).
    pub dawn_threshold: f32,
    /// `tod_phases`: `(start_fraction, string_token)`.
    pub tod_phases: Vec<(f32, u32)>,
    /// `aux_tokens`.
    pub aux_tokens: Vec<u32>,
    /// `months`: `(start_day_of_year, month_string_token)`.
    pub months: Vec<(u32, u32)>,
    /// `era_string_token`.
    pub era_string_token: u32,
}

/// Calendar state at one instant (§4.4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldCalendarSnapshot {
    /// `world_present_time_of_day` (fraction of the current day).
    pub present_time_of_day: f32,
    /// `world_is_night`.
    pub is_night: bool,
    /// Calendar year (`year_index + base_year`).
    pub calendar_year: i32,
    /// Zero-based day of the year.
    pub day_of_year: i32,
}

impl WorldCalendarConfig {
    /// Writes the record (synthetic tests; the client only reads it).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] if a list is too long to count.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        let count = |n: usize| u32::try_from(n).map_err(|_| WorldError::CountOverflow { count: n });
        writer.write_u32_le(self.record_did);
        writer.write_f32_le(self.day_length_seconds);
        writer.write_i32_le(self.days_per_year);
        writer.write_f64_le(self.calendar_epoch_offset);
        writer.write_i32_le(self.base_year);
        writer.write_f32_le(self.dusk_threshold);
        writer.write_f32_le(self.dawn_threshold);
        writer.write_u32_le(count(self.tod_phases.len())?);
        for &(frac, tok) in &self.tod_phases {
            writer.write_f32_le(frac);
            writer.write_u32_le(tok);
        }
        writer.write_u32_le(count(self.aux_tokens.len())?);
        for &tok in &self.aux_tokens {
            writer.write_u32_le(tok);
        }
        writer.write_u32_le(count(self.months.len())?);
        for &(day, tok) in &self.months {
            writer.write_u32_le(day);
            writer.write_u32_le(tok);
        }
        writer.write_u32_le(self.era_string_token);
        Ok(())
    }

    /// Reads the record (§4.4.2 field order). `expected_did` is the DID that
    /// [`CALENDAR_DIRECTORY_KEY`] resolved to in its directory; the record must start with it.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::InvalidWorldCalendar`] if the record does not start with
    /// `expected_did`, has a non-positive or non-finite day length, a non-positive
    /// days-per-year, or non-finite thresholds; or a framing error.
    pub fn decode(reader: &mut WireReader<'_>, expected_did: u32) -> Result<Self, WorldError> {
        let record_did = reader.read_u32_le()?;
        if record_did != expected_did {
            return Err(WorldError::InvalidWorldCalendar(
                "record DID is not the DID its directory key resolved to",
            ));
        }
        let day_length_seconds = reader.read_f32_le()?;
        let days_per_year = reader.read_i32_le()?;
        let calendar_epoch_offset = reader.read_f64_le()?;
        let base_year = reader.read_i32_le()?;
        let dusk_threshold = reader.read_f32_le()?;
        let dawn_threshold = reader.read_f32_le()?;
        if !(day_length_seconds.is_finite() && day_length_seconds > 0.0) {
            return Err(WorldError::InvalidWorldCalendar(
                "day length is not positive",
            ));
        }
        if days_per_year <= 0 {
            return Err(WorldError::InvalidWorldCalendar(
                "days per year is not positive",
            ));
        }
        if !(calendar_epoch_offset.is_finite()
            && dusk_threshold.is_finite()
            && dawn_threshold.is_finite())
        {
            return Err(WorldError::InvalidWorldCalendar(
                "non-finite calendar value",
            ));
        }
        let n = reader.read_u32_le()? as usize;
        reader.ensure_count(n, 8)?;
        let mut tod_phases = Vec::with_capacity(n.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..n {
            tod_phases.push((reader.read_f32_le()?, reader.read_u32_le()?));
        }
        let n = reader.read_u32_le()? as usize;
        reader.ensure_count(n, 4)?;
        let mut aux_tokens = Vec::with_capacity(n.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..n {
            aux_tokens.push(reader.read_u32_le()?);
        }
        let n = reader.read_u32_le()? as usize;
        reader.ensure_count(n, 8)?;
        let mut months = Vec::with_capacity(n.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..n {
            months.push((reader.read_u32_le()?, reader.read_u32_le()?));
        }
        let era_string_token = reader.read_u32_le()?;
        Ok(Self {
            record_did,
            day_length_seconds,
            days_per_year,
            calendar_epoch_offset,
            base_year,
            dusk_threshold,
            dawn_threshold,
            tod_phases,
            aux_tokens,
            months,
            era_string_token,
        })
    }

    /// Whether `present_time_of_day` is night: below `dawn_threshold` or at or above
    /// `dusk_threshold` (§4.4.2, `world_is_night`).
    #[must_use]
    pub fn is_night(&self, present_time_of_day: f32) -> bool {
        present_time_of_day < self.dawn_threshold || present_time_of_day >= self.dusk_threshold
    }
}
impl WorldCalendarConfig {
    /// Encodes the record to a standalone byte vector.
    ///
    /// # Errors
    ///
    /// Propagates the encoder's error.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes the record from a standalone byte slice (see [`Self::decode`]); trailing
    /// bytes are an error.
    ///
    /// # Errors
    ///
    /// Propagates the decoder's error.
    pub fn from_bytes(bytes: &[u8], expected_did: u32) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, expected_did)?;
        r.finish()?;
        Ok(msg)
    }
}

/// The in-game calendar clock of §4.4.2 steps 1–4: `world_time_offset` added to the network
/// time, `world_time_dilation` integrated per tick, and the calendar day advanced at
/// initialisation and whenever the time of day passes `1.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldClock {
    config: WorldCalendarConfig,
    time_offset: f64,
    time_dilation: f64,
    t_dilated: Option<f64>,
    t_raw_prev: f64,
    t_day_start: f64,
    calendar_year: i32,
    day_of_year: i32,
}

impl WorldClock {
    /// A clock over `config` with `world_time_offset = 0.0` and `world_time_dilation = 1.0`.
    #[must_use]
    pub fn new(config: WorldCalendarConfig) -> Self {
        Self {
            config,
            time_offset: 0.0,
            time_dilation: 1.0,
            t_dilated: None,
            t_raw_prev: 0.0,
            t_day_start: 0.0,
            calendar_year: 0,
            day_of_year: 0,
        }
    }

    /// Sets `world_time_offset` (`PID_WORLD_TIME_OFFSET`). Non-finite values are ignored.
    pub fn set_time_offset(&mut self, seconds: f32) {
        if seconds.is_finite() {
            self.time_offset = f64::from(seconds);
        }
    }

    /// Sets `world_time_dilation` (`PID_WORLD_TIME_DILATION`). Non-finite values are ignored.
    pub fn set_time_dilation(&mut self, factor: f32) {
        if factor.is_finite() {
            self.time_dilation = f64::from(factor);
        }
    }

    fn roll_day(&mut self, t_dilated: f64) {
        let day_len = f64::from(self.config.day_length_seconds);
        let year_len = f64::from(self.config.days_per_year) * day_len;
        let t_cal = t_dilated + self.config.calendar_epoch_offset;
        let year_index = (t_cal / year_len).floor();
        let t_rem = t_cal - year_index * year_len;
        let day = (t_rem / day_len)
            .floor()
            .clamp(0.0, f64::from(self.config.days_per_year - 1));
        self.calendar_year = (year_index as i32).saturating_add(self.config.base_year);
        self.day_of_year = day as i32;
        self.t_day_start = t_dilated - (t_rem - day * day_len);
    }

    /// Advances to network time `synced_time` (§4.4.2 steps 1–4).
    pub fn tick(&mut self, synced_time: f64) -> WorldCalendarSnapshot {
        let t_raw = synced_time + self.time_offset;
        let day_len = f64::from(self.config.day_length_seconds);
        let t_dilated = match self.t_dilated {
            None => {
                self.roll_day(t_raw);
                t_raw
            }
            Some(prev) => prev + self.time_dilation * (t_raw - self.t_raw_prev),
        };
        self.t_dilated = Some(t_dilated);
        self.t_raw_prev = t_raw;
        let mut tod = (t_dilated - self.t_day_start) / day_len;
        if tod > 1.0 || tod < 0.0 {
            self.roll_day(t_dilated);
            tod = (t_dilated - self.t_day_start) / day_len;
        }
        let present_time_of_day = tod as f32;
        WorldCalendarSnapshot {
            present_time_of_day,
            is_night: self.config.is_night(present_time_of_day),
            calendar_year: self.calendar_year,
            day_of_year: self.day_of_year,
        }
    }
}

// ---------------------------------------------------------------------------
// Object lifecycle and movement (§5.1–§5.5)
// ---------------------------------------------------------------------------

/// One framed sub-event of `0x0D` / `0x4C` (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubEventFrame {
    /// Non-zero `sub_event_id`: `u32` LE `payload_size` and the inner payload (starting at
    /// `base_flags`, no envelope, inheriting the batch's target entity).
    Event {
        /// Numeric event ID of the inner event (e.g. `0x98`, `0x52`, `0xCE`, `0x89`).
        sub_event_id: u32,
        /// Inner event payload.
        payload: Vec<u8>,
    },
    /// `sub_event_id` `0`: a 16-bit VLE `null_field_a` and a `u16` LE `null_field_b`
    /// (their meaning is `OPEN`).
    Null {
        /// `null_field_a`.
        field_a: u16,
        /// `null_field_b`.
        field_b: u16,
    },
}

impl SubEventFrame {
    fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        match self {
            Self::Event {
                sub_event_id,
                payload,
            } => {
                writer.write_vle4(*sub_event_id);
                let len = u32::try_from(payload.len()).map_err(|_| WorldError::CountOverflow {
                    count: payload.len(),
                })?;
                writer.write_u32_le(len);
                writer.write_bytes(payload);
            }
            Self::Null { field_a, field_b } => {
                writer.write_vle4(0);
                writer.write_vle16(*field_a);
                writer.write_u16_le(*field_b);
            }
        }
        Ok(())
    }

    fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let sub_event_id = reader.read_vle4()?;
        if sub_event_id == 0 {
            Ok(Self::Null {
                field_a: reader.read_vle16()?,
                field_b: reader.read_u16_le()?,
            })
        } else {
            let payload_size = reader.read_u32_le()? as usize;
            reader.ensure_count(payload_size, 1)?;
            Ok(Self::Event {
                sub_event_id,
                payload: reader.read_bytes(payload_size)?.to_vec(),
            })
        }
    }
}

/// Compound events `0x0D` (`aoi_enter_batch`) and `0x4C` (`sub_event_batch`), S→C, the
/// canonical layout of §5.1 (`official-session.md` §9.4.1 points here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubEventBatch {
    /// [`Self::EVENT_ID_AOI_ENTER`] or [`Self::EVENT_ID_SUB_EVENT`].
    pub event_id: u32,
    /// The framed sub-events, in order.
    pub frames: Vec<SubEventFrame>,
    /// `entry_batch_tail_flag`: present on a top-level `0x0D`, absent on `0x4C`.
    pub entry_batch_tail_flag: Option<u8>,
}

impl SubEventBatch {
    /// `aoi_enter_batch` (`0x0000000D`).
    pub const EVENT_ID_AOI_ENTER: u32 = 0x0000_000D;
    /// `sub_event_batch` (`0x0000004C`).
    pub const EVENT_ID_SUB_EVENT: u32 = 0x0000_004C;

    /// Writes the inner payload; a `0x0D` writes its tail flag (`0` when `None`).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for an `event_id` other than `0x0D` / `0x4C`.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        if self.event_id != Self::EVENT_ID_AOI_ENTER && self.event_id != Self::EVENT_ID_SUB_EVENT {
            return Err(WorldError::WrongEventId(self.event_id));
        }
        writer.write_base_flags();
        let count = u32::try_from(self.frames.len()).map_err(|_| WorldError::CountOverflow {
            count: self.frames.len(),
        })?;
        writer.write_vle4(count);
        for frame in &self.frames {
            frame.encode(writer)?;
        }
        if self.event_id == Self::EVENT_ID_AOI_ENTER {
            writer.write_u8(self.entry_batch_tail_flag.unwrap_or(0));
        }
        Ok(())
    }

    /// Reads a top-level `0x0D` or `0x4C` inner payload (§5.1, §7.1 item 1:
    /// `sub_event_count <= remaining / 4`, each `payload_size <= remaining`).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another event ID, or a framing error.
    pub fn decode(reader: &mut WireReader<'_>, event_id: u32) -> Result<Self, WorldError> {
        if event_id != Self::EVENT_ID_AOI_ENTER && event_id != Self::EVENT_ID_SUB_EVENT {
            return Err(WorldError::WrongEventId(event_id));
        }
        reader.read_base_flags()?;
        let count = reader.read_vle4()? as usize;
        reader.ensure_count(count, 4)?;
        let mut frames = Vec::with_capacity(count.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..count {
            frames.push(SubEventFrame::decode(reader)?);
        }
        let entry_batch_tail_flag = if event_id == Self::EVENT_ID_AOI_ENTER {
            Some(reader.read_u8()?)
        } else {
            None
        };
        Ok(Self {
            event_id,
            frames,
            entry_batch_tail_flag,
        })
    }

    /// Encodes to a standalone byte vector.
    ///
    /// # Errors
    ///
    /// See [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes event `event_id` from a standalone byte slice; trailing bytes are an error.
    ///
    /// # Errors
    ///
    /// See [`Self::decode`].
    pub fn from_bytes(bytes: &[u8], event_id: u32) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, event_id)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x98` (entity creation, S→C, 13 bytes, §5.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityCreate {
    /// `template_did` (typically a `0x70xxxxxx` game object).
    pub template_did: u32,
    /// `entity_id`.
    pub entity_id: u64,
}

impl EntityCreate {
    /// Numeric event ID (`0x00000098`).
    pub const EVENT_ID: u32 = 0x0000_0098;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.template_did);
        writer.write_u64_le(self.entity_id);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            template_did: reader.read_u32_le()?,
            entity_id: reader.read_u64_le()?,
        })
    }
}
plain_codec!(EntityCreate);

empty_event!(
    /// Event `0x40` (`entity_despawn`, S→C, §5.3): the target is the blob's `recipientId`.
    EntityDespawn,
    0x0000_0040
);

/// Event `0xD0` (`missing_entity_query`, C→S, 9 bytes, §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingEntityQuery {
    /// `entity_id`.
    pub entity_id: u64,
}

impl MissingEntityQuery {
    /// Numeric event ID (`0x000000D0`).
    pub const EVENT_ID: u32 = 0x0000_00D0;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.entity_id);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            entity_id: reader.read_u64_le()?,
        })
    }
}
plain_codec!(MissingEntityQuery);

/// Extended attachment fields of `0xCE` (`mode_flags` bit 3 in attachment mode, §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttachmentExtension {
    /// `attach_param` (`u32` LE).
    pub attach_param: u32,
    /// `attach_sub` (`u8`).
    pub attach_sub: u8,
    /// `attach_flag` (`bool8`).
    pub attach_flag: bool,
}

/// Placement of an entity in `0xCE` (§5.4 rows 10a–10c).
#[derive(Debug, Clone, PartialEq)]
pub enum EntityPlacement {
    /// `mode_flags` bits 2 and 4 clear: base wire `Position` and a 1-byte heading.
    WorldWithHeading {
        /// Base wire `Position`.
        position: WirePosition,
        /// Quantized heading (§2.5.2).
        heading_raw: u8,
    },
    /// Bit 2 clear, bit 4 set: full wire `Position` (with the quaternion when its own
    /// `flags` bit 5 is set) and no separate heading.
    WorldFull {
        /// Full wire `Position`.
        position: WirePosition,
    },
    /// Bit 2 set: attached to `parent_id` at `attach_slot`.
    Attachment {
        /// `parent_id`.
        parent_id: u64,
        /// `attach_slot` (3-tier VLE).
        attach_slot: u32,
        /// Present exactly when `mode_flags` bit 3 is set.
        extension: Option<AttachmentExtension>,
    },
}

/// Event `0xCE` (`position_update`, S→C, §5.4). Every `Option` field is present exactly
/// when its `header_flags` bit is set; the encoder derives `header_flags` and the
/// placement bits of `mode_flags` from the fields.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionUpdate {
    /// `entity_id`.
    pub entity_id: u64,
    /// `seq_byte` (`teleport_stamp`): present only when `header_flags` bit 3 (`0x08`) is set;
    /// echoed in `0x24` `header_byte` (§5.5). `mode_flags` never decides it.
    pub seq_byte: Option<u8>,
    /// `vel_x`, `vel_y`, `vel_z` (`header_flags` bits `0x10`, `0x20`, `0x40`; `0.0` when absent).
    pub velocity: [Option<f32>; 3],
    /// `anim_state` (`header_flags` bit `0x01`; `0` when absent).
    pub anim_state: Option<u32>,
    /// World position or parent attachment.
    pub placement: EntityPlacement,
    /// `interp_param_a` (`header_flags` bit `0x02`; `5` when absent; meaning `OPEN`).
    pub interp_param_a: Option<u8>,
    /// `interp_param_b` (`header_flags` bit `0x04`; `2` when absent; meaning `OPEN`).
    pub interp_param_b: Option<u8>,
}

impl PositionUpdate {
    /// Numeric event ID (`0x000000CE`).
    pub const EVENT_ID: u32 = 0x0000_00CE;
    /// Defined `header_flags` bits (§5.4).
    pub const HEADER_FLAGS_VALID: u8 = 0x7F;
    /// Defined `mode_flags` bits (§5.4: `0x04`, `0x08`, `0x10`).
    pub const MODE_FLAGS_VALID: u8 = 0x1C;

    /// The `header_flags` byte implied by the present fields.
    #[must_use]
    pub fn header_flags(&self) -> u8 {
        let mut f = 0u8;
        if self.anim_state.is_some() {
            f |= 0x01;
        }
        if self.interp_param_a.is_some() {
            f |= 0x02;
        }
        if self.interp_param_b.is_some() {
            f |= 0x04;
        }
        if self.seq_byte.is_some() {
            f |= 0x08;
        }
        for (i, v) in self.velocity.iter().enumerate() {
            if v.is_some() {
                f |= 0x10 << i;
            }
        }
        f
    }

    /// The `mode_flags` byte implied by the placement.
    #[must_use]
    pub fn mode_flags(&self) -> u8 {
        match &self.placement {
            EntityPlacement::WorldWithHeading { .. } => 0,
            EntityPlacement::WorldFull { .. } => 0x10,
            EntityPlacement::Attachment { extension, .. } => {
                0x04 | if extension.is_some() { 0x08 } else { 0 }
            }
        }
    }

    /// `interp_param_a` with its default (`5`).
    #[must_use]
    pub fn interp_param_a_or_default(&self) -> u8 {
        self.interp_param_a.unwrap_or(5)
    }

    /// `interp_param_b` with its default (`2`).
    #[must_use]
    pub fn interp_param_b_or_default(&self) -> u8 {
        self.interp_param_b.unwrap_or(2)
    }

    /// Writes the inner payload (§5.4 field order).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::Vle3Overflow`] for an `anim_state` or `attach_slot` above 30 bits.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u8(self.header_flags());
        writer.write_u64_le(self.entity_id);
        writer.write_u8(self.mode_flags());
        if let Some(seq) = self.seq_byte {
            writer.write_u8(seq);
        }
        for v in self.velocity.iter().flatten() {
            writer.write_f32_le(*v);
        }
        if let Some(anim) = self.anim_state {
            writer.write_vle3(anim)?;
        }
        match &self.placement {
            EntityPlacement::WorldWithHeading {
                position,
                heading_raw,
            } => {
                position.encode_base(writer);
                writer.write_u8(*heading_raw);
            }
            EntityPlacement::WorldFull { position } => position.encode_wire(writer),
            EntityPlacement::Attachment {
                parent_id,
                attach_slot,
                extension,
            } => {
                writer.write_u64_le(*parent_id);
                writer.write_vle3(*attach_slot)?;
                if let Some(ext) = extension {
                    writer.write_u32_le(ext.attach_param);
                    writer.write_u8(ext.attach_sub);
                    writer.write_bool8(ext.attach_flag);
                }
            }
        }
        if let Some(a) = self.interp_param_a {
            writer.write_u8(a);
        }
        if let Some(b) = self.interp_param_b {
            writer.write_u8(b);
        }
        Ok(())
    }

    /// Reads the inner payload (§5.4). Undefined `header_flags` / `mode_flags` bits are
    /// refused; velocities must be finite and world coordinates within
    /// `±MAX_WORLD_COORDINATE_M` (`official-messages-play.md` §9.1 rule 4).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on any of the above or a framing error.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let header_flags = reader.read_u8()?;
        require_defined_bits(
            "0xCE header_flags",
            u32::from(header_flags),
            u32::from(Self::HEADER_FLAGS_VALID),
        )?;
        let entity_id = reader.read_u64_le()?;
        let mode_flags = reader.read_u8()?;
        require_defined_bits(
            "0xCE mode_flags",
            u32::from(mode_flags),
            u32::from(Self::MODE_FLAGS_VALID),
        )?;
        // Bit 3 selects extended attachment data and bit 4 the full world Position; §5.4
        // gives bit 3 no meaning without bit 2 (attachment mode) and bit 4 none with it. Such
        // a combination could not be written back, so it is refused (engineering choice,
        // spec silent).
        let attach = mode_flags & 0x04 != 0;
        if (!attach && mode_flags & 0x08 != 0) || (attach && mode_flags & 0x10 != 0) {
            tracing::debug!(mode_flags, "0xCE mode_flags combination refused");
            return Err(WorldError::UndefinedFlagBits {
                field: "0xCE mode_flags",
                bits: u32::from(mode_flags & if attach { 0x10 } else { 0x08 }),
            });
        }
        let seq_byte = (header_flags & 0x08 != 0)
            .then(|| reader.read_u8())
            .transpose()?;
        let mut velocity = [None; 3];
        for (i, (slot, field)) in velocity
            .iter_mut()
            .zip(["vel_x", "vel_y", "vel_z"])
            .enumerate()
        {
            if header_flags & (0x10 << i) != 0 {
                *slot = Some(require_finite(field, reader.read_f32_le()?)?);
            }
        }
        let anim_state = (header_flags & 0x01 != 0)
            .then(|| reader.read_vle3())
            .transpose()?;
        let placement = if mode_flags & 0x04 != 0 {
            let parent_id = reader.read_u64_le()?;
            let attach_slot = reader.read_vle3()?;
            let extension = if mode_flags & 0x08 != 0 {
                Some(AttachmentExtension {
                    attach_param: reader.read_u32_le()?,
                    attach_sub: reader.read_u8()?,
                    attach_flag: reader.read_bool8()?,
                })
            } else {
                None
            };
            EntityPlacement::Attachment {
                parent_id,
                attach_slot,
                extension,
            }
        } else if mode_flags & 0x10 != 0 {
            let position = WirePosition::decode_wire(reader)?;
            check_world_position(&position)?;
            EntityPlacement::WorldFull { position }
        } else {
            let position = WirePosition::decode_base(reader)?;
            check_world_position(&position)?;
            EntityPlacement::WorldWithHeading {
                position,
                heading_raw: reader.read_u8()?,
            }
        };
        let interp_param_a = (header_flags & 0x02 != 0)
            .then(|| reader.read_u8())
            .transpose()?;
        let interp_param_b = (header_flags & 0x04 != 0)
            .then(|| reader.read_u8())
            .transpose()?;
        Ok(Self {
            entity_id,
            seq_byte,
            velocity,
            anim_state,
            placement,
            interp_param_a,
            interp_param_b,
        })
    }
}
plain_codec!(PositionUpdate);

/// Checks the local offset (and the quaternion, when present) of a `0xCE` position
/// (`official-messages-play.md` §9.1 rule 4).
fn check_world_position(position: &WirePosition) -> Result<(), WorldError> {
    for (field, v) in ["pos_x", "pos_y", "pos_z"].into_iter().zip(position.offset) {
        require_world_coordinate(field, v)?;
    }
    for v in position.orientation {
        require_finite("orientation", v)?;
    }
    Ok(())
}

/// Event `0x24` (`client_physics_sync`, C→S, §5.5): the 4-slot rolling movement window.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientPhysicsSync {
    /// `entity_id`.
    pub entity_id: u64,
    /// `header_byte`: the `teleport_stamp` (`seq_byte`) of the latest server `0xCE`.
    pub teleport_stamp: u8,
    /// `sequence` of slot 3 (slots 0–2 are `sequence - 3 .. sequence - 1`, clamped at 0).
    pub sequence: u32,
    /// `move_state[4]`.
    pub move_state: [u8; 4],
    /// `slot_flags[4]` as received. Only bit 0 (`0x01`, actions) and bit 4 (`0x10`,
    /// properties) are used; the bits under mask `0xEE` are ignored on receive, and the
    /// encoder writes them as `0` and derives bits 0 and 4 from the slot contents.
    pub slot_flags: [u8; 4],
    /// `axis_a[4]` (`SignedNormalizedByte`: lateral / strafe).
    pub axis_a: [i8; 4],
    /// `axis_b[4]` (`SignedNormalizedByte`: forward / backward).
    pub axis_b: [i8; 4],
    /// `axis_c[4]` (`SignedNormalizedByte`: yaw / turn).
    pub axis_c: [i8; 4],
    /// Per-slot queued actions `(action_id, ActionPayload)`.
    pub slot_actions: [Vec<(CompactDid, ActionPayload)>; 4],
    /// Per-slot property tables.
    pub slot_props: [Option<EncodedPropertyTable>; 4],
    /// `trailer_u32` (meaning `OPEN`; a sender writes `0`, a receiver ignores it).
    pub trailer_u32: u32,
}

impl ClientPhysicsSync {
    /// Numeric event ID (`0x00000024`).
    pub const EVENT_ID: u32 = 0x0000_0024;
    /// Length of the inner payload with no slot actions or properties (`38` bytes).
    pub const BASE_BYTE_LEN: usize = 38;
    /// `slot_flags` bits whose meaning is `OPEN` (`0xEE`): cleared on send, ignored on receive.
    pub const SLOT_FLAGS_OPEN_MASK: u8 = 0xEE;

    /// A message with no slot actions, no slot properties and `trailer_u32 = 0`.
    #[must_use]
    pub fn new_base(
        entity_id: u64,
        teleport_stamp: u8,
        sequence: u32,
        move_state: [u8; 4],
        axes: [[i8; 4]; 3],
    ) -> Self {
        let [axis_a, axis_b, axis_c] = axes;
        Self {
            entity_id,
            teleport_stamp,
            sequence,
            move_state,
            slot_flags: [0; 4],
            axis_a,
            axis_b,
            axis_c,
            slot_actions: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            slot_props: [None, None, None, None],
            trailer_u32: 0,
        }
    }

    /// The four slot sequence numbers `max(0, sequence - 3) ..= sequence` (§5.5).
    #[must_use]
    pub fn slot_sequences(&self) -> [u32; 4] {
        [
            self.sequence.saturating_sub(3),
            self.sequence.saturating_sub(2),
            self.sequence.saturating_sub(1),
            self.sequence,
        ]
    }

    /// The `slot_flags` the encoder writes: bit 0 for slots with actions, bit 4 for slots
    /// with a property table, and nothing else.
    #[must_use]
    pub fn encoded_slot_flags(&self) -> [u8; 4] {
        let mut flags = [0u8; 4];
        for (i, f) in flags.iter_mut().enumerate() {
            if !self.slot_actions[i].is_empty() {
                *f |= 0x01;
            }
            if self.slot_props[i].is_some() {
                *f |= 0x10;
            }
        }
        flags
    }

    /// Writes the inner payload (§5.5 field order).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] if a slot has more than 255 actions or a value fails to encode.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.entity_id);
        writer.write_u8(self.teleport_stamp);
        writer.write_u32_le(self.sequence);
        writer.write_bytes(&self.move_state);
        let flags = self.encoded_slot_flags();
        writer.write_bytes(&flags);
        for axis in [&self.axis_a, &self.axis_b, &self.axis_c] {
            for &v in axis {
                writer.write_i8(v);
            }
        }
        if flags.iter().any(|&f| f & 0x01 != 0) {
            for slot in &self.slot_actions {
                let count = u8::try_from(slot.len())
                    .map_err(|_| WorldError::CountOverflow { count: slot.len() })?;
                writer.write_u8(count);
                for (action_id, payload) in slot {
                    action_id.encode(writer)?;
                    payload.encode(writer)?;
                }
            }
        }
        for props in self.slot_props.iter().flatten() {
            props.encode(writer)?;
        }
        writer.write_u32_le(self.trailer_u32);
        Ok(())
    }

    /// Reads the inner payload using `schema` (§5.5, §7.1 item 1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing, schema or `ActionPayload` errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let entity_id = reader.read_u64_le()?;
        let teleport_stamp = reader.read_u8()?;
        let sequence = reader.read_u32_le()?;
        let mut move_state = [0u8; 4];
        move_state.copy_from_slice(reader.read_bytes(4)?);
        let mut slot_flags = [0u8; 4];
        slot_flags.copy_from_slice(reader.read_bytes(4)?);
        let mut axes = [[0i8; 4]; 3];
        for axis in &mut axes {
            for v in axis.iter_mut() {
                *v = reader.read_i8()?;
            }
        }
        let mut slot_actions: [Vec<(CompactDid, ActionPayload)>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        if slot_flags.iter().any(|&f| f & 0x01 != 0) {
            for slot in &mut slot_actions {
                let count = usize::from(reader.read_u8()?);
                // A (CompactDid, ActionPayload) pair is at least 2 + 11 bytes.
                reader.ensure_count(count, 13)?;
                for _ in 0..count {
                    let did = CompactDid::decode(reader)?;
                    slot.push((did, ActionPayload::decode(reader, schema)?));
                }
            }
        }
        let mut slot_props: [Option<EncodedPropertyTable>; 4] = [None, None, None, None];
        for (prop, &flag) in slot_props.iter_mut().zip(slot_flags.iter()) {
            if flag & 0x10 != 0 {
                *prop = Some(EncodedPropertyTable::decode(reader, schema)?);
            }
        }
        let trailer_u32 = reader.read_u32_le()?;
        let [axis_a, axis_b, axis_c] = axes;
        Ok(Self {
            entity_id,
            teleport_stamp,
            sequence,
            move_state,
            slot_flags,
            axis_a,
            axis_b,
            axis_c,
            slot_actions,
            slot_props,
            trailer_u32,
        })
    }
}
schema_codec!(ClientPhysicsSync);

// ---------------------------------------------------------------------------
// Actions (§5.6)
// ---------------------------------------------------------------------------

/// The sub-action fields of an [`ActionPayload`] (`action_flags` bit 13, `0x2000`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSubEntry {
    /// `sub_action_id` (`VariableU32`).
    pub sub_action_id: u32,
    /// `sub_entry_id` (`VariableU32`).
    pub sub_entry_id: u32,
    /// `sub_entry_flag` (`bool8`).
    pub sub_entry_flag: bool,
}

/// The `ActionPayload` of `0x75`, `0x24`, `0x5A`, `0xA8`, `0x63` and `0xBD` (§5.6.2): 11 to
/// 56 bytes before `action_props`. Every `Option` field is present exactly when its
/// `action_flags` bit is set; the encoder derives `action_flags`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionPayload {
    /// `action_mode` (`u8`; `10` when constructed).
    pub action_mode: u8,
    /// `scalar_param` (bit `0x0040`; `0.0` when absent).
    pub scalar_param: Option<f32>,
    /// Sub-action fields (bit `0x2000`).
    pub sub_entry: Option<ActionSubEntry>,
    /// `target_id_a` (bit `0x0001`; `0` when absent).
    pub target_id_a: Option<u64>,
    /// `target_id_b` (bit `0x4000`; `0` when absent).
    pub target_id_b: Option<u64>,
    /// `action_props` (bit `0x0002`).
    pub action_props: Option<EncodedPropertyTable>,
    /// `state_code` (bit `0x0008`; `3` when absent; meaning `OPEN`).
    pub state_code: Option<u32>,
    /// `speed_or_scale` (bit `0x0010`; `1.0` when absent).
    pub speed_or_scale: Option<f32>,
    /// `timestamp` (`f64` LE network time, always present).
    pub timestamp: f64,
    /// `aux_index` (bit `0x0020`; `0xFFFFFFFF` when absent; meaning `OPEN`).
    pub aux_index: Option<u32>,
}

impl Default for ActionPayload {
    fn default() -> Self {
        Self {
            action_mode: 0x0A,
            scalar_param: None,
            sub_entry: None,
            target_id_a: None,
            target_id_b: None,
            action_props: None,
            state_code: None,
            speed_or_scale: None,
            timestamp: 0.0,
            aux_index: None,
        }
    }
}

impl ActionPayload {
    /// Defined wire `action_flags` bits (§5.6.2). Bit 2 (`0x0004`) is used only by stored
    /// `.dat` descriptors and is always clear on the wire.
    pub const FLAGS_VALID: u16 =
        0x0001 | 0x0002 | 0x0008 | 0x0010 | 0x0020 | 0x0040 | 0x2000 | 0x4000;

    /// The `action_flags` word implied by the present fields.
    #[must_use]
    pub fn action_flags(&self) -> u16 {
        let mut f = 0u16;
        if self.target_id_a.is_some() {
            f |= 0x0001;
        }
        if self.action_props.is_some() {
            f |= 0x0002;
        }
        if self.state_code.is_some() {
            f |= 0x0008;
        }
        if self.speed_or_scale.is_some() {
            f |= 0x0010;
        }
        if self.aux_index.is_some() {
            f |= 0x0020;
        }
        if self.scalar_param.is_some() {
            f |= 0x0040;
        }
        if self.sub_entry.is_some() {
            f |= 0x2000;
        }
        if self.target_id_b.is_some() {
            f |= 0x4000;
        }
        f
    }

    /// Writes the payload (§5.6.2 field order).
    ///
    /// # Errors
    ///
    /// Propagates `action_props` encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u8(self.action_mode);
        writer.write_u16_le(self.action_flags());
        if let Some(v) = self.scalar_param {
            writer.write_f32_le(v);
        }
        if let Some(sub) = &self.sub_entry {
            writer.write_vle4(sub.sub_action_id);
            writer.write_vle4(sub.sub_entry_id);
            writer.write_bool8(sub.sub_entry_flag);
        }
        if let Some(id) = self.target_id_a {
            writer.write_u64_le(id);
        }
        if let Some(id) = self.target_id_b {
            writer.write_u64_le(id);
        }
        if let Some(props) = &self.action_props {
            props.encode(writer)?;
        }
        if let Some(code) = self.state_code {
            writer.write_vle4(code);
        }
        if let Some(speed) = self.speed_or_scale {
            writer.write_f32_le(speed);
        }
        writer.write_f64_le(self.timestamp);
        if let Some(aux) = self.aux_index {
            writer.write_vle4(aux);
        }
        Ok(())
    }

    /// Reads a payload using `schema` (§5.6.2). `action_flags` bits outside
    /// [`Self::FLAGS_VALID`] (including the stored-only `0x0004`) are refused.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on undefined flag bits, framing or schema errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let action_mode = reader.read_u8()?;
        let flags = reader.read_u16_le()?;
        require_defined_bits(
            "ActionPayload action_flags",
            u32::from(flags),
            u32::from(Self::FLAGS_VALID),
        )?;
        let has = |bit: u16| flags & bit != 0;
        let scalar_param = has(0x0040).then(|| reader.read_f32_le()).transpose()?;
        let sub_entry = if has(0x2000) {
            Some(ActionSubEntry {
                sub_action_id: reader.read_vle4()?,
                sub_entry_id: reader.read_vle4()?,
                sub_entry_flag: reader.read_bool8()?,
            })
        } else {
            None
        };
        let target_id_a = has(0x0001).then(|| reader.read_u64_le()).transpose()?;
        let target_id_b = has(0x4000).then(|| reader.read_u64_le()).transpose()?;
        let action_props = if has(0x0002) {
            Some(EncodedPropertyTable::decode(reader, schema)?)
        } else {
            None
        };
        let state_code = has(0x0008).then(|| reader.read_vle4()).transpose()?;
        let speed_or_scale = has(0x0010).then(|| reader.read_f32_le()).transpose()?;
        let timestamp = reader.read_f64_le()?;
        let aux_index = has(0x0020).then(|| reader.read_vle4()).transpose()?;
        Ok(Self {
            action_mode,
            scalar_param,
            sub_entry,
            target_id_a,
            target_id_b,
            action_props,
            state_code,
            speed_or_scale,
            timestamp,
            aux_index,
        })
    }
}

/// Event `0x75` (client action command, C→S, §5.6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ClientActionCommand {
    /// `entity_id`.
    pub entity_id: u64,
    /// `action_id` (Compact `DID`).
    pub action_id: CompactDid,
    /// The `ActionPayload`.
    pub payload: ActionPayload,
}

impl ClientActionCommand {
    /// Numeric event ID (`0x00000075`).
    pub const EVENT_ID: u32 = 0x0000_0075;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.entity_id);
        self.action_id.encode(writer)?;
        self.payload.encode(writer)
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or payload errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            entity_id: reader.read_u64_le()?,
            action_id: CompactDid::decode(reader)?,
            payload: ActionPayload::decode(reader, schema)?,
        })
    }
}
schema_codec!(ClientActionCommand);

/// Events `0x5A` (server action start) and `0xA8` (the same followed by a Type 22
/// bitfield), S→C (§5.6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionStart {
    /// `action_id` (Compact `DID`).
    pub action_id: CompactDid,
    /// `action_seq` (`VariableU32`).
    pub action_seq: u32,
    /// The `ActionPayload`.
    pub payload: ActionPayload,
    /// The trailing bitfield: present exactly for `0xA8`.
    pub bitfield: Option<WireBitfield>,
}

impl ActionStart {
    /// Event ID without the bitfield (`0x0000005A`).
    pub const EVENT_ID: u32 = 0x0000_005A;
    /// Event ID with the bitfield (`0x000000A8`).
    pub const EVENT_ID_WITH_BITFIELD: u32 = 0x0000_00A8;

    /// The event ID for this value.
    #[must_use]
    pub const fn event_id(&self) -> u32 {
        if self.bitfield.is_some() {
            Self::EVENT_ID_WITH_BITFIELD
        } else {
            Self::EVENT_ID
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        writer.write_vle4(self.action_seq);
        self.payload.encode(writer)?;
        if let Some(bits) = &self.bitfield {
            bits.encode(writer)?;
        }
        Ok(())
    }

    /// Reads event `event_id` (`0x5A` or `0xA8`) using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another ID, or a framing error.
    pub fn decode(
        reader: &mut WireReader<'_>,
        event_id: u32,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let with_bits = match event_id {
            Self::EVENT_ID => false,
            Self::EVENT_ID_WITH_BITFIELD => true,
            other => return Err(WorldError::WrongEventId(other)),
        };
        reader.read_base_flags()?;
        let action_id = CompactDid::decode(reader)?;
        let action_seq = reader.read_vle4()?;
        let payload = ActionPayload::decode(reader, schema)?;
        let bitfield = with_bits
            .then(|| WireBitfield::decode(reader))
            .transpose()?;
        Ok(Self {
            action_id,
            action_seq,
            payload,
            bitfield,
        })
    }

    /// Decodes event `event_id` from a standalone byte slice; trailing bytes are an error.
    ///
    /// # Errors
    ///
    /// See [`Self::decode`].
    pub fn from_bytes(
        bytes: &[u8],
        event_id: u32,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, event_id, schema)?;
        r.finish()?;
        Ok(msg)
    }

    /// Encodes to a standalone byte vector.
    ///
    /// # Errors
    ///
    /// See [`Self::encode`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }
}

/// Event `0xA7` (server action step completion, S→C, §5.6.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionStepComplete {
    /// `action_id`.
    pub action_id: CompactDid,
    /// `action_seq`.
    pub action_seq: u32,
    /// `completion_timestamp` (`f64` LE).
    pub completion_timestamp: f64,
}

impl ActionStepComplete {
    /// Numeric event ID (`0x000000A7`).
    pub const EVENT_ID: u32 = 0x0000_00A7;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates `action_id` encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        writer.write_vle4(self.action_seq);
        writer.write_f64_le(self.completion_timestamp);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            action_id: CompactDid::decode(reader)?,
            action_seq: reader.read_vle4()?,
            completion_timestamp: reader.read_f64_le()?,
        })
    }
}
plain_codec!(ActionStepComplete);

/// Event `0x86` (server action cancellation, S→C, §5.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCancel {
    /// `action_id`.
    pub action_id: CompactDid,
    /// `action_seq`.
    pub action_seq: u32,
    /// `reason_code` (`u32` LE).
    pub reason_code: u32,
}

impl ActionCancel {
    /// Numeric event ID (`0x00000086`).
    pub const EVENT_ID: u32 = 0x0000_0086;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates `action_id` encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        writer.write_vle4(self.action_seq);
        writer.write_u32_le(self.reason_code);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            action_id: CompactDid::decode(reader)?,
            action_seq: reader.read_vle4()?,
            reason_code: reader.read_u32_le()?,
        })
    }
}
plain_codec!(ActionCancel);

/// One completed action of `0x63` (`count_b` entries).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActionChainCompletion {
    /// `action_id`.
    pub action_id: CompactDid,
    /// `action_seq`.
    pub action_seq: u32,
    /// `timestamp` (`f64` LE).
    pub timestamp: f64,
    /// `flag` (`bool8`).
    pub flag: bool,
}

/// Event `0x63` (server action chain synchronization, S→C, §5.6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionChainSync {
    /// `chain_mode` (`u8`).
    pub chain_mode: u8,
    /// `count_a` entries: `(action_id, action_seq, ActionPayload)`.
    pub actions: Vec<(CompactDid, u32, ActionPayload)>,
    /// `count_b` entries.
    pub completions: Vec<ActionChainCompletion>,
}

impl ActionChainSync {
    /// Numeric event ID (`0x00000063`).
    pub const EVENT_ID: u32 = 0x0000_0063;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        let count = |n: usize| u32::try_from(n).map_err(|_| WorldError::CountOverflow { count: n });
        writer.write_base_flags();
        writer.write_u8(self.chain_mode);
        writer.write_vle4(count(self.actions.len())?);
        for (did, seq, payload) in &self.actions {
            did.encode(writer)?;
            writer.write_vle4(*seq);
            payload.encode(writer)?;
        }
        writer.write_vle4(count(self.completions.len())?);
        for c in &self.completions {
            c.action_id.encode(writer)?;
            writer.write_vle4(c.action_seq);
            writer.write_f64_le(c.timestamp);
            writer.write_bool8(c.flag);
        }
        Ok(())
    }

    /// Reads the inner payload using `schema` (counts bounded per §7.1 item 1).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or payload errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let chain_mode = reader.read_u8()?;
        let count_a = reader.read_vle4()? as usize;
        // Compact DID (2) + VariableU32 (1) + ActionPayload (11).
        reader.ensure_count(count_a, 14)?;
        let mut actions = Vec::with_capacity(count_a.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..count_a {
            let did = CompactDid::decode(reader)?;
            let seq = reader.read_vle4()?;
            actions.push((did, seq, ActionPayload::decode(reader, schema)?));
        }
        let count_b = reader.read_vle4()? as usize;
        // Compact DID (2) + VariableU32 (1) + f64 (8) + bool8 (1).
        reader.ensure_count(count_b, 12)?;
        let mut completions = Vec::with_capacity(count_b.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..count_b {
            completions.push(ActionChainCompletion {
                action_id: CompactDid::decode(reader)?,
                action_seq: reader.read_vle4()?,
                timestamp: reader.read_f64_le()?,
                flag: reader.read_bool8()?,
            });
        }
        Ok(Self {
            chain_mode,
            actions,
            completions,
        })
    }
}
schema_codec!(ActionChainSync);

/// One `ActionLayerEntry` of `0xBD`.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionLayerEntry {
    /// `action_did` (Compact `DID`).
    pub action_did: CompactDid,
    /// `action_seq` (`VariableU32`).
    pub action_seq: u32,
    /// The `ActionPayload`.
    pub payload: ActionPayload,
    /// The Type 22 bitfield.
    pub bitfield: WireBitfield,
}

/// Event `0xBD` (`entity_anim_layer_state`, S→C, §5.6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityAnimLayerState {
    /// The layers (3-tier VLE count).
    pub layers: Vec<ActionLayerEntry>,
}

impl EntityAnimLayerState {
    /// Numeric event ID (`0x000000BD`).
    pub const EVENT_ID: u32 = 0x0000_00BD;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        let n = u32::try_from(self.layers.len()).map_err(|_| WorldError::CountOverflow {
            count: self.layers.len(),
        })?;
        writer.write_vle3(n)?;
        for layer in &self.layers {
            layer.action_did.encode(writer)?;
            writer.write_vle4(layer.action_seq);
            layer.payload.encode(writer)?;
            layer.bitfield.encode(writer)?;
        }
        Ok(())
    }

    /// Reads the inner payload using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or payload errors.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let n = reader.read_vle3()? as usize;
        // Compact DID (2) + VariableU32 (1) + ActionPayload (11) + bitfield (1).
        reader.ensure_count(n, 15)?;
        let mut layers = Vec::with_capacity(n.min(MAX_PREALLOC_ENTRIES));
        for _ in 0..n {
            layers.push(ActionLayerEntry {
                action_did: CompactDid::decode(reader)?,
                action_seq: reader.read_vle4()?,
                payload: ActionPayload::decode(reader, schema)?,
                bitfield: WireBitfield::decode(reader)?,
            });
        }
        Ok(Self { layers })
    }
}
schema_codec!(EntityAnimLayerState);

/// Event `0x39` (entity action queue enable / disable, S→C, §5.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionQueueEnable {
    /// `enabled` (`bool8`).
    pub enabled: bool,
}

impl ActionQueueEnable {
    /// Numeric event ID (`0x00000039`).
    pub const EVENT_ID: u32 = 0x0000_0039;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_bool8(self.enabled);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing or `bool8` errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            enabled: reader.read_bool8()?,
        })
    }
}
plain_codec!(ActionQueueEnable);

empty_event!(
    /// Event `0x60` (entity action state reset, S→C, §5.6.1).
    ActionStateReset,
    0x0000_0060
);

// ---------------------------------------------------------------------------
// Visual and property updates (§5.7–§5.8)
// ---------------------------------------------------------------------------

/// Event `0x89` (visual description update, S→C, §5.7).
#[derive(Debug, Clone, PartialEq)]
pub struct VisualDescriptionUpdate {
    /// The new `EntityAppearanceDesc`.
    pub visual_desc: EntityAppearanceDesc,
}

impl VisualDescriptionUpdate {
    /// Numeric event ID (`0x00000089`).
    pub const EVENT_ID: u32 = 0x0000_0089;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.visual_desc.encode(writer)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors (see [`EntityAppearanceDesc::decode`]).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            visual_desc: EntityAppearanceDesc::decode(reader)?,
        })
    }
}
plain_codec!(VisualDescriptionUpdate);

/// What a property event applies to (§5.8.1 "Target scope").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyScope {
    /// The target entity (`FragmentedBlob` `recipientId`).
    Entity,
    /// The session property table.
    Session,
}

/// Events `0x52` (entity) and `0xFE` (session): a bulk `EncodedPropertyTable` (§5.2.2, §5.8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyTableUpdate {
    /// Entity (`0x52`) or session (`0xFE`).
    pub scope: PropertyScope,
    /// The properties.
    pub properties: EncodedPropertyTable,
}

impl PropertyTableUpdate {
    /// Entity event ID (`0x00000052`).
    pub const EVENT_ID_ENTITY: u32 = 0x0000_0052;
    /// Session event ID (`0x000000FE`).
    pub const EVENT_ID_SESSION: u32 = 0x0000_00FE;

    /// The event ID for this value.
    #[must_use]
    pub const fn event_id(&self) -> u32 {
        match self.scope {
            PropertyScope::Entity => Self::EVENT_ID_ENTITY,
            PropertyScope::Session => Self::EVENT_ID_SESSION,
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.properties.encode(writer)
    }

    /// Reads event `event_id` (`0x52` or `0xFE`) using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another ID, or a framing/schema error.
    pub fn decode(
        reader: &mut WireReader<'_>,
        event_id: u32,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let scope = match event_id {
            Self::EVENT_ID_ENTITY => PropertyScope::Entity,
            Self::EVENT_ID_SESSION => PropertyScope::Session,
            other => return Err(WorldError::WrongEventId(other)),
        };
        reader.read_base_flags()?;
        Ok(Self {
            scope,
            properties: EncodedPropertyTable::decode(reader, schema)?,
        })
    }
}

/// Events `0x88` / `0x92` (entity) and `0xB0` / `0x149` (session): one `(CompactPID, value)`
/// entry (§5.8.1).
#[derive(Debug, Clone, PartialEq)]
pub struct SinglePropertyUpdate {
    /// The event ID (`0x88`, `0x92`, `0xB0` or `0x149`).
    pub event_id: u32,
    /// The entry.
    pub entry: EncodedPropertyEntry,
}

impl SinglePropertyUpdate {
    /// Entity event IDs (`0x88`, `0x92`).
    pub const EVENT_IDS_ENTITY: [u32; 2] = [0x0000_0088, 0x0000_0092];
    /// Session event IDs (`0xB0`, `0x149`).
    pub const EVENT_IDS_SESSION: [u32; 2] = [0x0000_00B0, 0x0000_0149];

    /// The scope of `event_id`, or `None` if it is not a single-property update.
    #[must_use]
    pub fn scope_of(event_id: u32) -> Option<PropertyScope> {
        if Self::EVENT_IDS_ENTITY.contains(&event_id) {
            Some(PropertyScope::Entity)
        } else if Self::EVENT_IDS_SESSION.contains(&event_id) {
            Some(PropertyScope::Session)
        } else {
            None
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.entry.encode(writer)
    }

    /// Reads event `event_id` using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another ID, or a framing/schema error.
    pub fn decode(
        reader: &mut WireReader<'_>,
        event_id: u32,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        if Self::scope_of(event_id).is_none() {
            return Err(WorldError::WrongEventId(event_id));
        }
        reader.read_base_flags()?;
        Ok(Self {
            event_id,
            entry: EncodedPropertyEntry::decode(reader, schema)?,
        })
    }
}

/// Events `0x0E` / `0xE7` (entity) and `0x76` / `0x14A` (session): remove `property_id`
/// (`u32` LE, §5.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyRemoval {
    /// The event ID (`0x0E`, `0xE7`, `0x76` or `0x14A`).
    pub event_id: u32,
    /// `property_id`.
    pub property_id: u32,
}

impl PropertyRemoval {
    /// Entity event IDs (`0x0E`, `0xE7`).
    pub const EVENT_IDS_ENTITY: [u32; 2] = [0x0000_000E, 0x0000_00E7];
    /// Session event IDs (`0x76`, `0x14A`).
    pub const EVENT_IDS_SESSION: [u32; 2] = [0x0000_0076, 0x0000_014A];

    /// The scope of `event_id`, or `None` if it is not a removal event.
    #[must_use]
    pub fn scope_of(event_id: u32) -> Option<PropertyScope> {
        if Self::EVENT_IDS_ENTITY.contains(&event_id) {
            Some(PropertyScope::Entity)
        } else if Self::EVENT_IDS_SESSION.contains(&event_id) {
            Some(PropertyScope::Session)
        } else {
            None
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.property_id);
        Ok(())
    }

    /// Reads event `event_id`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another ID, or a framing error.
    pub fn decode(reader: &mut WireReader<'_>, event_id: u32) -> Result<Self, WorldError> {
        if Self::scope_of(event_id).is_none() {
            return Err(WorldError::WrongEventId(event_id));
        }
        reader.read_base_flags()?;
        Ok(Self {
            event_id,
            property_id: reader.read_u32_le()?,
        })
    }
}

/// Event `0x9F` (entity string event, S→C, §5.8.1): a prefixed 8-bit string whose meaning
/// is `OPEN`; a receiver decodes it and ignores it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityStringEvent {
    /// The string.
    pub text: String,
}

impl EntityStringEvent {
    /// Numeric event ID (`0x0000009F`).
    pub const EVENT_ID: u32 = 0x0000_009F;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::InvalidLatin1Char`] for characters above U+00FF.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_pascal_string(&self.text)
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            text: reader.read_pascal_string()?,
        })
    }
}
plain_codec!(EntityStringEvent);

/// The operation of a landblock property event (§5.8.1 `0x146`–`0x148`).
#[derive(Debug, Clone, PartialEq)]
pub enum LandblockPropertyOp {
    /// `0x146`: `u8 flag` + `EncodedPropertyTable`.
    Table {
        /// `flag`.
        flag: u8,
        /// The properties.
        properties: EncodedPropertyTable,
    },
    /// `0x147`: one property entry.
    Single(EncodedPropertyEntry),
    /// `0x148`: remove `property_id` (`u32` LE).
    Remove(u32),
}

/// Events `0x146`, `0x147` and `0x148` (landblock property events, S→C, §5.8.1): a base
/// wire `Position` naming the landblock / cell, then the operation.
#[derive(Debug, Clone, PartialEq)]
pub struct LandblockPropertyEvent {
    /// Base wire `Position` (no quaternion).
    pub position: WirePosition,
    /// The operation.
    pub op: LandblockPropertyOp,
}

impl LandblockPropertyEvent {
    /// `0x00000146` (table).
    pub const EVENT_ID_TABLE: u32 = 0x0000_0146;
    /// `0x00000147` (single property).
    pub const EVENT_ID_SINGLE: u32 = 0x0000_0147;
    /// `0x00000148` (removal).
    pub const EVENT_ID_REMOVE: u32 = 0x0000_0148;

    /// The event ID for this value.
    #[must_use]
    pub const fn event_id(&self) -> u32 {
        match self.op {
            LandblockPropertyOp::Table { .. } => Self::EVENT_ID_TABLE,
            LandblockPropertyOp::Single(_) => Self::EVENT_ID_SINGLE,
            LandblockPropertyOp::Remove(_) => Self::EVENT_ID_REMOVE,
        }
    }

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Propagates encoding errors.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.position.encode_base(writer);
        match &self.op {
            LandblockPropertyOp::Table { flag, properties } => {
                writer.write_u8(*flag);
                properties.encode(writer)
            }
            LandblockPropertyOp::Single(entry) => entry.encode(writer),
            LandblockPropertyOp::Remove(pid) => {
                writer.write_u32_le(*pid);
                Ok(())
            }
        }
    }

    /// Reads event `event_id` (`0x146`, `0x147` or `0x148`) using `schema`.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::WrongEventId`] for another ID, or a framing/schema error.
    pub fn decode(
        reader: &mut WireReader<'_>,
        event_id: u32,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        if !(Self::EVENT_ID_TABLE..=Self::EVENT_ID_REMOVE).contains(&event_id) {
            return Err(WorldError::WrongEventId(event_id));
        }
        reader.read_base_flags()?;
        let position = WirePosition::decode_base(reader)?;
        let op = match event_id {
            Self::EVENT_ID_TABLE => LandblockPropertyOp::Table {
                flag: reader.read_u8()?,
                properties: EncodedPropertyTable::decode(reader, schema)?,
            },
            Self::EVENT_ID_SINGLE => {
                LandblockPropertyOp::Single(EncodedPropertyEntry::decode(reader, schema)?)
            }
            _ => LandblockPropertyOp::Remove(reader.read_u32_le()?),
        };
        Ok(Self { position, op })
    }
}

/// Event `0x8E` (predicted property acknowledgement, C→S / S→C, §5.8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredictedPropertyAck {
    /// `property_id` (`u32` LE).
    pub property_id: u32,
    /// `sequence` (`u32` LE, default `0xFFFFFFFF`).
    pub sequence: u32,
}

impl PredictedPropertyAck {
    /// Numeric event ID (`0x0000008E`).
    pub const EVENT_ID: u32 = 0x0000_008E;

    /// Writes the inner payload.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.property_id);
        writer.write_u32_le(self.sequence);
        Ok(())
    }

    /// Reads the inner payload.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError`] on framing errors.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self {
            property_id: reader.read_u32_le()?,
            sequence: reader.read_u32_le()?,
        })
    }
}
plain_codec!(PredictedPropertyAck);
