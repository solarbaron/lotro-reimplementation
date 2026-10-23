// SPDX-License-Identifier: GPL-3.0-or-later
//! Official world-session message layer: compact wire value encodings,
//! schema-driven encoded property tables, character selection, world entry,
//! network clock, world calendar, and object lifecycle codecs
//! (`docs/specs/official-messages-world.md` §1–§7).
//!
//! All parsers treat network input as untrusted: counts are bounded against
//! remaining bytes before allocating, `bool8` values strictly require `0` or
//! `1`, and recursive structures (`EncodedPropertyTable` and `WireLocalizedText`)
//! enforce a maximum nesting depth of [`MAX_RECURSION_DEPTH`] (`16`).

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use flate2::read::ZlibDecoder;

/// Maximum nesting depth for recursive `EncodedPropertyTable` (Type 11 `Struct`
/// and Type 12 `Array`) and `WireLocalizedText` (argument Kind 1) structures
/// (`docs/specs/official-messages-world.md` §7.1 item 2).
pub const MAX_RECURSION_DEPTH: usize = 16;

/// Maximum uncompressed payload length accepted in a fragmented blob zlib envelope
/// (`0x0200_0000` = 32 MiB, `docs/specs/official-messages-world.md` §1.1.2).
pub const MAX_ENVELOPE_UNCOMPRESSED_LEN: u32 = 0x0200_0000;

/// Maximum word count accepted in a routing header `extra_words` block
/// (`0x72`, `docs/specs/official-messages-world.md` §1.1.2).
pub const MAX_ROUTING_EXTRA_WORDS: u32 = 0x72;

/// Archive mode flag bit 11 (`0x0800`): compact / wire mode
/// (`docs/specs/official-messages-world.md` §1.2).
pub const ARCHIVE_FLAG_COMPACT_WIRE: u32 = 0x0800;

/// Archive mode flag bit 14 (`0x4000`): checkpointed property mode
/// (`docs/specs/official-messages-world.md` §1.2).
pub const ARCHIVE_FLAG_CHECKPOINTED: u32 = 0x4000;

/// Fragmented blob envelope flag bit 0 (`0x01`): payload is zlib-compressed
/// (`docs/specs/official-messages-world.md` §1.1.2).
pub const ENVELOPE_FLAG_ZLIB_COMPRESSED: u8 = 0x01;

/// Fragmented blob envelope flag bit 1 (`0x02`): routing header is present
/// (`docs/specs/official-messages-world.md` §1.1.2).
pub const ENVELOPE_FLAG_HAS_ROUTING_HEADER: u8 = 0x02;

/// Sentinel `f64` value (`f32::MAX` widened to `f64`) representing "no time"
/// in Type 4 (`Timestamp`) properties (`docs/specs/official-messages-world.md` §2.7.1, §4.4.1).
pub const TIMESTAMP_NO_TIME: f64 = 3.402_823_466_385_288_6e38;

/// Expected record DID for the `WorldCalendarConfig` calendar record in `client_general.dat`
/// (`docs/specs/official-messages-world.md` §4.4.2).
pub const WORLD_CALENDAR_CONFIG_DID: u32 = 0x3500_0000;

/// Fallback default in-game day length in seconds before `0x35000000` is loaded
/// (`docs/specs/official-messages-world.md` §4.4.2).
pub const DEFAULT_FALLBACK_DAY_LENGTH_SECONDS: f32 = 100_000.0;

// ---------------------------------------------------------------------------
// Property ID Constants (§3.1.3, §3.2.5, §4.3.1, §4.4.2, §5.2.4)
// ---------------------------------------------------------------------------

/// `CharacterType` (`0x0000062F`, Type 8 `Enum`, §3.1.3 / §3.2.5).
pub const PID_CHARACTER_TYPE: u32 = 0x0000_062F;
/// `Agent_Species` (`0x100000FC`, Type 8 `Enum`, §3.1.3 / §3.2.5 / §4.3.1 / §5.2.4).
pub const PID_AGENT_SPECIES: u32 = 0x1000_00FC;
/// `Agent_Subspecies` (`0x10001461`, Type 8 `Enum`, §3.1.3 / §5.2.4).
pub const PID_AGENT_SUBSPECIES: u32 = 0x1000_1461;
/// `Agent_Genus` (`0x10000CBE`, Type 18 `Bitfield32`, §3.1.3 / §5.2.4).
pub const PID_AGENT_GENUS: u32 = 0x1000_0CBE;
/// `Agent_Alignment` (`0x1000069B`, Type 8 `Enum`, §3.1.3 / §5.2.4).
pub const PID_AGENT_ALIGNMENT: u32 = 0x1000_069B;
/// `Agent_Class` (`0x10001089`, Type 8 `Enum`, §3.1.3 / §3.2.5 / §5.2.4).
pub const PID_AGENT_CLASS: u32 = 0x1000_1089;
/// `Sex` (`0x10000A50`, Type 8 `Enum`, §3.1.3 / §3.2.5 / §5.2.4).
pub const PID_SEX: u32 = 0x1000_0A50;
/// `Nationality` (`0x10001190`, Type 8 `Enum`, §3.1.3 / §3.2.5 / §5.2.4).
pub const PID_NATIONALITY: u32 = 0x1000_1190;
/// `Advancement_Level` (`0x10001011`, Type 15 `Int`, §3.1.3 / §5.2.4).
pub const PID_ADVANCEMENT_LEVEL: u32 = 0x1000_1011;
/// `CharGen_Character_Level` (`0x1000663B`, Type 15 `Int`, §3.1.3).
pub const PID_CHARGEN_CHARACTER_LEVEL: u32 = 0x1000_663B;
/// `CharGen_Character_Class` (`0x10006647`, Type 13 `LocalizedText`, §3.1.3).
pub const PID_CHARGEN_CHARACTER_CLASS: u32 = 0x1000_6647;
/// `CharGen_Character_LastUpdateTime` (`0x1000664B`, Type 4 `Timestamp`, §3.1.3).
pub const PID_CHARGEN_CHARACTER_LAST_UPDATE_TIME: u32 = 0x1000_664B;
/// `CharGen_CharacterHasData` (`0x10003884`, Type 21 `Boolean`, §3.1.3).
pub const PID_CHARGEN_CHARACTER_HAS_DATA: u32 = 0x1000_3884;
/// `CharGen_CharacterAvailabilityError` (`0x10005F96`, Type 8 `Enum`, §3.1.3).
pub const PID_CHARGEN_CHARACTER_AVAILABILITY_ERROR: u32 = 0x1000_5F96;
/// `Area_CurrentArea` (`0x10000983`, Type 20 `DidRef`, §3.1.3).
pub const PID_AREA_CURRENT_AREA: u32 = 0x1000_0983;
/// `Area_Territory` (`0x100011A1`, Type 20 `DidRef`, §3.1.3).
pub const PID_AREA_TERRITORY: u32 = 0x1000_11A1;
/// `Area_Region` (`0x10001494`, Type 20 `DidRef`, §3.1.3).
pub const PID_AREA_REGION: u32 = 0x1000_1494;
/// `Area_RegionID` (`0x100003FE`, Type 8 `Enum`, §3.1.3).
pub const PID_AREA_REGION_ID: u32 = 0x1000_03FE;
/// `Area_Name` (`0x00000198`, Type 13 `LocalizedText`, §3.1.3).
pub const PID_AREA_NAME: u32 = 0x0000_0198;
/// `Physics_CurrentPosition` (`0x0000001B`, Type 17 `Position`, §3.1.3).
pub const PID_PHYSICS_CURRENT_POSITION: u32 = 0x0000_001B;
/// `Physics_LastValidPosition` (`0x000005A3`, Type 17 `Position`, §3.1.3).
pub const PID_PHYSICS_LAST_VALID_POSITION: u32 = 0x0000_05A3;
/// `BlockX` (`0x00000090`, Type 15 `Int`, §3.1.3).
pub const PID_BLOCK_X: u32 = 0x0000_0090;
/// `Surname` (`0x100010F8`, Type 13 `LocalizedText`, §3.1.3 / §5.2.4).
pub const PID_SURNAME: u32 = 0x1000_10F8;
/// `Title_ActiveTitleDID` (`0x1000088B`, Type 20 `DidRef`, §3.1.3 / §5.2.4).
pub const PID_TITLE_ACTIVE_TITLE_DID: u32 = 0x1000_088B;
/// `Title_ActiveTitleString` (`0x1000119C`, Type 13 `LocalizedText`, §3.1.3 / §5.2.4).
pub const PID_TITLE_ACTIVE_TITLE_STRING: u32 = 0x1000_119C;
/// `PhysObj` (`0x0000047F`, Type 20 `DidRef`, §3.1.3 / §3.2.5 / §5.2.4).
pub const PID_PHYS_OBJ: u32 = 0x0000_047F;
/// `Appearance_InstanceList` (`0x0000054F`, Type 12 `Array`, §3.1.3 / §3.2.5 / §5.2.4).
pub const PID_APPEARANCE_INSTANCE_LIST: u32 = 0x0000_054F;
/// `Appearance_ClassList` (`0x000006C3`, Type 12 `Array`, §5.2.4).
pub const PID_APPEARANCE_CLASS_LIST: u32 = 0x0000_06C3;
/// `Account_GameAccountName` (`0x00000747`, Type 13 `LocalizedText`, §3.1.3).
pub const PID_ACCOUNT_GAME_ACCOUNT_NAME: u32 = 0x0000_0747;
/// `Account_BillingName` (`0x000003F7`, Type 13 `LocalizedText`, §3.1.3).
pub const PID_ACCOUNT_BILLING_NAME: u32 = 0x0000_03F7;
/// `Account_AuthTicket` (`0x0000018A`, Type 1 `String`, §3.1.3).
pub const PID_ACCOUNT_AUTH_TICKET: u32 = 0x0000_018A;
/// `Account_IID` (`0x100038A6`, Type 7 `InstanceId`, §3.1.3).
pub const PID_ACCOUNT_IID: u32 = 0x1000_38A6;
/// `Billing@Player.AccountType` (`0x10003866`, Type 8 `Enum`, §3.1.3).
pub const PID_BILLING_PLAYER_ACCOUNT_TYPE: u32 = 0x1000_3866;
/// `Account_BillingRoles` (`0x000001D6`, Type 22 `Bitfield`, §3.1.3).
pub const PID_ACCOUNT_BILLING_ROLES: u32 = 0x0000_01D6;
/// `Account_NumCharactersAllowed` (`0x00000499`, Type 15 `Int`, §3.1.3).
pub const PID_ACCOUNT_NUM_CHARACTERS_ALLOWED: u32 = 0x0000_0499;
/// `Character_LastUsed` (`0x10003885`, Type 7 `InstanceId`, §3.1.3).
pub const PID_CHARACTER_LAST_USED: u32 = 0x1000_3885;
/// `Entity_IsTransientUnique` (`0x00000A94`, Type 21 `Boolean`, §3.2.5).
pub const PID_ENTITY_IS_TRANSIENT_UNIQUE: u32 = 0x0000_0A94;
/// `AC_Mesh_Head` (`0x10006DB5`, Type 9 `Float`, §3.2.5).
pub const PID_AC_MESH_HEAD: u32 = 0x1000_6DB5;
/// `AC_Mesh_Ears` (`0x1000739D`, Type 9 `Float`, §3.2.5).
pub const PID_AC_MESH_EARS: u32 = 0x1000_739D;
/// `AC_BoneScale_Test` (`0x10000DB4`, Type 9 `Float`, §3.2.5).
pub const PID_AC_BONE_SCALE_TEST: u32 = 0x1000_0DB4;
/// `AC_Avatar_Height` (`0x1000703D`, Type 9 `Float`, §3.2.5).
pub const PID_AC_AVATAR_HEIGHT: u32 = 0x1000_703D;
/// `AC_BoneSlider_Value_Array` (`0x100069B7`, Type 1 `String`, §3.2.5).
pub const PID_AC_BONE_SLIDER_VALUE_ARRAY: u32 = 0x1000_69B7;
/// `AC_Color_Skin` (`0x100001E0`, Type 16 `Color`, §3.2.5).
pub const PID_AC_COLOR_SKIN: u32 = 0x1000_01E0;
/// `AC_Color_Hair` (`0x10000595`, Type 16 `Color`, §3.2.5).
pub const PID_AC_COLOR_HAIR: u32 = 0x1000_0595;
/// `AC_Color_Eyes` (`0x100003BA`, Type 16 `Color`, §3.2.5).
pub const PID_AC_COLOR_EYES: u32 = 0x1000_03BA;
/// `AC_Color_Lips` (`0x100066D9`, Type 16 `Color`, §3.2.5).
pub const PID_AC_COLOR_LIPS: u32 = 0x1000_66D9;
/// `AC_Color_WarPaint` (`0x1000612A`, Type 16 `Color`, §3.2.5).
pub const PID_AC_COLOR_WAR_PAINT: u32 = 0x1000_612A;

/// `World_TimeOffset` (`0x000001EF`, Type 9 `Float`, §4.4.2).
pub const PID_WORLD_TIME_OFFSET: u32 = 0x0000_01EF;
/// `World_TimeDilation` (`0x000006CC`, Type 9 `Float`, §4.4.2).
pub const PID_WORLD_TIME_DILATION: u32 = 0x0000_06CC;
/// `World_PresentTimeOfDay` (`0x00000682`, Type 9 `Float`, §4.4.2).
pub const PID_WORLD_PRESENT_TIME_OF_DAY: u32 = 0x0000_0682;
/// `World_IsNight` (`0x000001F0`, Type 21 `Boolean`, §4.4.2).
pub const PID_WORLD_IS_NIGHT: u32 = 0x0000_01F0;
/// `World_DayType` (`0x00000235`, Type 8 `Enum`, §4.4.2).
pub const PID_WORLD_DAY_TYPE: u32 = 0x0000_0235;

/// `Name` (`0x00000354`, Type 13 `LocalizedText`, §5.2.4).
pub const PID_NAME: u32 = 0x0000_0354;
/// `WeenieType` (`0x00000126`, Type 8 `Enum`, §4.3.1 / §5.2.4).
pub const PID_WEENIE_TYPE: u32 = 0x0000_0126;
/// `Entity_Element` (`0x00000201`, Type 8 `Enum`, §4.3.1).
pub const PID_ENTITY_ELEMENT: u32 = 0x0000_0201;
/// `Movement_MovementStyle` (`0x00000687`, Type 8 `Enum`, §4.3.1).
pub const PID_MOVEMENT_STYLE: u32 = 0x0000_0687;
/// `Agent_ClassificationFilter` (`0x1000136E`, Type 8 `Enum`, §4.3.1 / §5.2.4).
pub const PID_AGENT_CLASSIFICATION_FILTER: u32 = 0x1000_136E;
/// `Combat_AnimationSet` (`0x10000469`, Type 8 `Enum`, §4.3.1).
pub const PID_COMBAT_ANIMATION_SET: u32 = 0x1000_0469;
/// `Health_CurrentLevel` (`0x100012D2`, Type 9 `Float`, §5.2.4).
pub const PID_HEALTH_CURRENT_LEVEL: u32 = 0x1000_12D2;
/// `Health_MaxLevel` (`0x100008BB`, Type 9 `Float`, §5.2.4).
pub const PID_HEALTH_MAX_LEVEL: u32 = 0x1000_08BB;
/// `Power_CurrentLevel` (`0x10000340`, Type 9 `Float`, §5.2.4).
pub const PID_POWER_CURRENT_LEVEL: u32 = 0x1000_0340;
/// `Power_MaxLevel` (`0x10000520`, Type 9 `Float`, §5.2.4).
pub const PID_POWER_MAX_LEVEL: u32 = 0x1000_0520;
/// `LifeState` (`0x100005D6`, Type 8 `Enum`, §5.2.4).
pub const PID_LIFE_STATE: u32 = 0x1000_05D6;
/// `Entity_Selection` (`0x0000034F`, Type 7 `InstanceId`, §5.2.4).
pub const PID_ENTITY_SELECTION: u32 = 0x0000_034F;

/// Default `state_schema_id` (`0x0000000C` = `12`, `Earth_ControlledSurface`)
/// in `0xCC` (`PortalPlacement`) when bit 7 (`0x80`) of `pos_sync_flags` is clear
/// (`docs/specs/official-messages-world.md` §4.3.1).
pub const DEFAULT_PORTAL_STATE_SCHEMA_ID: u32 = 0x0000_000C;

/// Auxiliary `PlayerPhysics` script state schema `0x10000E50` (`0` bools, `0` enums, §4.3.1).
pub const MOUNTED_PORTAL_STATE_SCHEMA_ID_1: u32 = 0x1000_0E50;

/// Auxiliary `PlayerPhysics` script state schema `0x10000E51` (`0` bools, `0` enums, §4.3.1).
pub const MOUNTED_PORTAL_STATE_SCHEMA_ID_2: u32 = 0x1000_0E51;

/// Exact 8 boolean `PID`s packed LSB-first into `bool_state_bytes[0]` for
/// `state_schema_id = 0x0000000C` (`Earth_ControlledSurface`, §4.3.1).
pub const DEFAULT_PORTAL_STATE_BOOL_PIDS: [u32; 8] = [
    0x0000_068A, // bit 0: Movement_ActiveMovementFrozen
    0x1000_3EA0, // bit 1: Physics_OnSlide
    0x1000_6676, // bit 2: Physics_OnIce
    0x1000_4157, // bit 3: Mount_Movement_Use_RampUp_Controller
    0x1000_0ED8, // bit 4: Stealth_IsStealthed
    0x1000_1A27, // bit 5: ScriptSystem_IsHovering
    0x1000_0ED7, // bit 6: ScriptSystem_ActiveMovementBlocked
    0x1000_0443, // bit 7: Patrol_Flying
];

/// Exact 6 enum `PID`s serialized as consecutive 3-tier VLEs for
/// `state_schema_id = 0x0000000C` (`Earth_ControlledSurface`, §4.3.1).
pub const DEFAULT_PORTAL_STATE_ENUM_PIDS: [u32; 6] = [
    0x0000_0126, // index 0: WeenieType
    0x0000_0201, // index 1: Entity_Element
    0x0000_0687, // index 2: Movement_MovementStyle
    0x1000_136E, // index 3: Agent_ClassificationFilter
    0x1000_00FC, // index 4: Agent_Species
    0x1000_0469, // index 5: Combat_AnimationSet
];

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors produced when encoding or decoding official world-session messages
/// and wire values (`docs/specs/official-messages-world.md` §1–§7).
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
    /// A nested `EncodedPropertyTable` or `WireLocalizedText` exceeded [`MAX_RECURSION_DEPTH`] (§7.1 item 2).
    #[error("maximum recursion depth ({max}) exceeded")]
    MaxRecursionDepthExceeded {
        /// Maximum allowed depth.
        max: usize,
    },
    /// A 3-tier VLE value was `>= 0x4000_0000` and cannot be represented (§2.2.1).
    #[error("3-tier VLE value 0x{0:08X} exceeds 30-bit maximum 0x3FFFFFFF")]
    Vle3Overflow(u32),
    /// A collection or string count exceeded the remaining unread bytes (§7.1 item 1).
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
    /// A character above U+00FF was passed to a Latin-1 8-bit Pascal string writer.
    #[error("character U+{0:04X} cannot be encoded in an 8-bit Latin-1 string")]
    InvalidLatin1Char(u32),
    /// A property ID in a wire `EncodedPropertyTable` was not found in the property schema (§7.1 item 4).
    #[error("unknown property ID 0x{0:08X} in wire property set")]
    UnknownPropertyId(u32),
    /// A property type code outside `1..=22` was encountered (§2.7.1).
    #[error("invalid property type code {0}: must be in 1..=22")]
    InvalidPropertyTypeCode(u32),
    /// A `TSize` collection hint byte was `23` or higher (`property-system.md` §17.2).
    #[error("invalid TSize bucket hint {0}: must be in 0..=22")]
    InvalidTSizeHint(u8),
    /// A `PortalPlacement` (`0xCC`) packet referenced an unknown `state_schema_id` (§4.3.1).
    #[error("unknown 0xCC portal state_schema_id 0x{0:08X}")]
    UnknownPortalStateSchema(u32),
    /// A `PortalPlacement` (`0xCC`) struct had the wrong number of boolean or enum state entries for its schema.
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
    /// Extended attachment data (bit 3 `0x08` of `mode_flags` in `0xCE`) is not supported.
    #[error("unsupported extended attachment mode_flags 0x{0:02X} in 0xCE")]
    UnsupportedExtendedAttachment(u8),
    /// A routing header `extra_words` count exceeded `0x72` (§1.1.2).
    #[error("routing header extra_words count 0x{0:08X} exceeds maximum 0x72")]
    InvalidRoutingExtraWordsCount(u32),
    /// A routing header's 2-byte alignment pad byte was non-zero (§1.1.2).
    #[error("routing header alignment pad byte 0x{0:02X} must be 0x00")]
    InvalidRoutingAlignmentPad(u8),
    /// A fragmented blob zlib header declared `uncompressedLen` above 32 MiB (§1.1.2).
    #[error("Fragmented blob uncompressed length {0} exceeds 32 MiB limit")]
    EnvelopeUncompressedLengthTooLarge(u32),
    /// Decompressing a zlib-compressed fragmented blob body failed.
    #[error("Fragmented blob zlib decompression failed: {0}")]
    ZlibDecompressError(String),
    /// The decompressed fragmented blob body length did not match `uncompressedLen`.
    #[error("Fragmented blob decompressed length mismatch: expected {expected}, got {actual}")]
    ZlibLengthMismatch {
        /// Declared `uncompressedLen`.
        expected: usize,
        /// Actual inflated byte count.
        actual: usize,
    },
    /// A `WorldCalendarConfig` record did not start with `DID 0x35000000` (§4.4.2).
    #[error("invalid WorldCalendarConfig record DID 0x{0:08X}: expected 0x35000000")]
    InvalidWorldCalendarDid(u32),
    /// Trailing unread bytes remained after decoding a fixed-framing payload.
    #[error("{0} trailing byte(s) remained after decoding payload")]
    TrailingBytes(usize),
    /// A wire `LocalizedText` argument kind outside `1..=7` reached a payload decoder (§2.6).
    #[error("invalid wire LocalizedText argument kind {0}")]
    InvalidStringArgKind(u16),
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
}

/// Largest absolute world coordinate (metres) accepted in `0xCE`
/// (`official-messages-play.md` §9.1 rule 4).
pub const MAX_WORLD_COORDINATE_M: f32 = 100_000.0;

/// Rejects a NaN or infinite position or velocity component
/// (`official-messages-play.md` §9.1 rule 4).
fn require_finite(field: &'static str, value: f32) -> Result<f32, WorldError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(WorldError::NonFiniteFloat { field })
    }
}

/// Rejects a non-finite world coordinate or one outside `±MAX_WORLD_COORDINATE_M`
/// (`official-messages-play.md` §9.1 rule 4).
fn require_world_coordinate(field: &'static str, value: f32) -> Result<f32, WorldError> {
    let value = require_finite(field, value)?;
    if (-MAX_WORLD_COORDINATE_M..=MAX_WORLD_COORDINATE_M).contains(&value) {
        Ok(value)
    } else {
        Err(WorldError::CoordinateOutOfRange { field })
    }
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

    /// Reads a 4-tier VLE (full 32-bit unsigned, 1 / 2 / 4 / 5 bytes with `0xE0` prefix; §2.2.2).
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
        } else {
            self.read_u32_le()
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

    /// Reads a prefixed UTF-16LE string (3-tier VLE code-unit count $N$ + $2N$ bytes; §2.3, §7.1).
    pub fn read_utf16le_string(&mut self) -> Result<String, WorldError> {
        let count = self.read_vle3()? as usize;
        self.ensure_count(count, 2)?;
        let raw = self.read_bytes(count * 2)?;
        let mut units = Vec::with_capacity(count);
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
        let len_u32 = u32::try_from(units.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
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
        let len_u32 = u32::try_from(bytes.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
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

    /// Reads a `CompactPid` from `reader` according to §2.4.3 reader rules.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::UnexpectedEof`] if input is truncated.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let b0 = reader.read_u8()?;
        if b0 == 0x80 {
            Ok(Self(reader.read_u32_le()?))
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

    /// Encodes a full Type 17 `Position` in wire mode (bit 11 `0x800` set:
    /// elides `region` when `0x01 | 0x40` are set, elides `cell` when `0x08 | 0x80`
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

    /// Encodes a full `Position` in stored `.dat` mode (bit 11 `0x800` clear:
    /// `region` and `cell` bytes are written even when `0x40` / `0x80` are set; §2.5.1).
    pub fn encode_stored(&self, writer: &mut WireWriter) {
        self.encode_inner(writer, false, true);
    }

    /// Decodes a full `Position` in stored `.dat` mode (bit 11 `0x800` clear; §2.5.1).
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
        let flags = reader.read_u8()?;
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
// WireLocalizedText (§2.6)
// ---------------------------------------------------------------------------

/// Mode payload of a [`WireLocalizedText`] (§2.6).
#[derive(Debug, Clone, PartialEq)]
pub enum WireLocalizedTextContent {
    /// Mode `1`: literal UTF-16LE string.
    Literal(String),
    /// Any mode byte other than `1` (`0`, `2`, `3..=255`): string-table reference.
    Reference {
        /// Original mode byte (`0` for standard table reference, `2` for authoring source text).
        mode: u8,
        /// 32-bit string token ID.
        token_id: u32,
        /// 32-bit string-table DID (`0x25xxxxxx`).
        table_did: u32,
    },
}

/// Argument value payload inside a [`WireLocalizedText`] argument table (§2.6).
#[derive(Debug, Clone, PartialEq)]
pub enum WireLocalizedTextArgPayload {
    /// Kind `1`: recursive `WireLocalizedText`.
    Nested(Box<WireLocalizedText>),
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
        /// Formatting flags (`bit 0` = grouping, `bit 1` = drop trailing zeroes).
        format_flags: u8,
        /// Radix (`0xFFFF` = locale default).
        radix: u16,
    },
    /// Kind `4`: fixed 8-byte signed 64-bit integer.
    FixedInt(i64),
    /// Kind `5`: fixed 8-byte signed 64-bit integer with format options.
    FixedIntFormatted {
        /// Signed 64-bit integer value.
        value: i64,
        /// Formatting flags.
        format_flags: u8,
        /// Radix.
        radix: u16,
    },
    /// Kind `6`: compact wire integer (`bool8 is_32bit` + `u64 LE` or 4-tier VLE).
    CompactInt {
        /// When `true` (`0x01`), encoded on the wire as a 4-tier VLE of the low 32 bits;
        /// when `false` (`0x00`), encoded as an 8-byte `u64 LE`.
        is_32bit: bool,
        /// Reconstructed 64-bit value.
        value: u64,
    },
    /// Kind `7`: compact wire integer with format options.
    CompactIntFormatted {
        /// When `true` (`0x01`), encoded on the wire as a 4-tier VLE of the low 32 bits;
        /// when `false` (`0x00`), encoded as an 8-byte `u64 LE`.
        is_32bit: bool,
        /// Reconstructed 64-bit value.
        value: u64,
        /// Formatting flags.
        format_flags: u8,
        /// Radix.
        radix: u16,
    },
}

/// A single entry in a [`WireLocalizedText`] argument table (§2.6).
#[derive(Debug, Clone, PartialEq)]
pub struct WireLocalizedTextArg {
    /// Variable name-hash key (`u32 LE`).
    pub key: u32,
    /// Entry kind selector (`u16 LE`).
    pub kind: u16,
    /// Common 6-byte value header kind (`u16 LE`), present when `kind` is in `1..=7`.
    pub header_kind: u16,
    /// Common 6-byte value header variable ID (`u32 LE`), present when `kind` is in `1..=7`.
    pub var_id: u32,
    /// Kind-specific payload (`Some` for `kind` in `1..=7`, `None` for `0` or `>= 8`).
    pub payload: Option<WireLocalizedTextArgPayload>,
}

/// Localized or literal string descriptor (`LocalizedText`, `docs/specs/official-messages-world.md` §2.6).
#[derive(Debug, Clone, PartialEq)]
pub struct WireLocalizedText {
    /// Literal text or `(mode, token_id, table_did)` reference.
    pub content: WireLocalizedTextContent,
    /// Optional three Pascal authoring strings (`[token_name, source_text, comment]`)
    /// when `has_authoring_strings` is `true` (`0x01`).
    pub authoring_strings: Option<[String; 3]>,
    /// Substitution argument entries.
    pub arguments: Vec<WireLocalizedTextArg>,
}

impl WireLocalizedText {
    /// Creates a literal UTF-16LE `WireLocalizedText` with no authoring strings and no arguments.
    #[must_use]
    pub fn literal(text: impl Into<String>) -> Self {
        Self {
            content: WireLocalizedTextContent::Literal(text.into()),
            authoring_strings: None,
            arguments: Vec::new(),
        }
    }

    /// Creates a standard (`mode = 0`) string-table reference with no authoring strings and no arguments.
    #[must_use]
    pub fn reference(token_id: u32, table_did: u32) -> Self {
        Self {
            content: WireLocalizedTextContent::Reference {
                mode: 0,
                token_id,
                table_did,
            },
            authoring_strings: None,
            arguments: Vec::new(),
        }
    }

    /// Encodes this `WireLocalizedText` in wire mode (bit 11 `0x800` set: no TSize
    /// hint byte before `arg_count`, and Kind 6/7 compact integers; §2.6).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_inner(writer, true, 1)
    }

    /// Decodes a `WireLocalizedText` in wire mode (bit 11 `0x800` set; §2.6).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, true)
    }

    /// Encodes this `WireLocalizedText` in stored `.dat` mode (bit 11 `0x800` clear:
    /// writes a `u8` TSize hint byte `0x01` before `arg_count` and 8-byte `u64 LE`
    /// for Kind 6/7; §2.6, Vector 6).
    pub fn encode_stored(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        self.encode_inner(writer, false, 1)
    }

    /// Decodes a `WireLocalizedText` in stored `.dat` mode (bit 11 `0x800` clear; §2.6, Vector 6).
    pub fn decode_stored(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Self::decode_inner(reader, false)
    }

    /// Encodes this `WireLocalizedText` in wire mode to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes a `WireLocalizedText` in wire mode from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let val = Self::decode(&mut r)?;
        r.finish()?;
        Ok(val)
    }

    fn encode_inner(
        &self,
        writer: &mut WireWriter,
        wire_mode: bool,
        depth: usize,
    ) -> Result<(), WorldError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(WorldError::MaxRecursionDepthExceeded {
                max: MAX_RECURSION_DEPTH,
            });
        }
        match &self.content {
            WireLocalizedTextContent::Literal(s) => {
                writer.write_u8(1);
                writer.write_utf16le_string(s)?;
            }
            WireLocalizedTextContent::Reference {
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
            None => {
                writer.write_bool8(false);
            }
        }
        if !wire_mode {
            writer.write_u8(1);
        }
        let arg_count =
            u32::try_from(self.arguments.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_vle3(arg_count)?;
        for arg in &self.arguments {
            writer.write_u32_le(arg.key);
            writer.write_u16_le(arg.kind);
            if (1..=7).contains(&arg.kind) {
                writer.write_u16_le(arg.header_kind);
                writer.write_u32_le(arg.var_id);
                if let Some(payload) = &arg.payload {
                    match payload {
                        WireLocalizedTextArgPayload::Nested(nested) => {
                            nested.encode_inner(writer, wire_mode, depth + 1)?;
                        }
                        WireLocalizedTextArgPayload::Float {
                            value,
                            decimal_places,
                        } => {
                            writer.write_f64_le(*value);
                            writer.write_u16_le(*decimal_places);
                        }
                        WireLocalizedTextArgPayload::FloatFormatted {
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
                        WireLocalizedTextArgPayload::FixedInt(val) => {
                            writer.write_i64_le(*val);
                        }
                        WireLocalizedTextArgPayload::FixedIntFormatted {
                            value,
                            format_flags,
                            radix,
                        } => {
                            writer.write_i64_le(*value);
                            writer.write_u8(*format_flags);
                            writer.write_u16_le(*radix);
                        }
                        WireLocalizedTextArgPayload::CompactInt { is_32bit, value } => {
                            if wire_mode {
                                writer.write_bool8(*is_32bit);
                                if *is_32bit {
                                    writer.write_vle4(*value as u32);
                                } else {
                                    writer.write_u64_le(*value);
                                }
                            } else {
                                writer.write_u64_le(*value);
                            }
                        }
                        WireLocalizedTextArgPayload::CompactIntFormatted {
                            is_32bit,
                            value,
                            format_flags,
                            radix,
                        } => {
                            if wire_mode {
                                writer.write_bool8(*is_32bit);
                                if *is_32bit {
                                    writer.write_vle4(*value as u32);
                                } else {
                                    writer.write_u64_le(*value);
                                }
                            } else {
                                writer.write_u64_le(*value);
                            }
                            writer.write_u8(*format_flags);
                            writer.write_u16_le(*radix);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn decode_inner(reader: &mut WireReader<'_>, wire_mode: bool) -> Result<Self, WorldError> {
        let mode = reader.read_u8()?;
        let content = if mode == 1 {
            WireLocalizedTextContent::Literal(reader.read_utf16le_string()?)
        } else {
            let token_id = reader.read_u32_le()?;
            let table_did = reader.read_u32_le()?;
            WireLocalizedTextContent::Reference {
                mode,
                token_id,
                table_did,
            }
        };
        let has_authoring = reader.read_bool8()?;
        let authoring_strings = if has_authoring {
            Some([
                reader.read_pascal_string()?,
                reader.read_pascal_string()?,
                reader.read_pascal_string()?,
            ])
        } else {
            None
        };
        if !wire_mode {
            let hint = reader.read_u8()?;
            if hint > 22 {
                return Err(WorldError::InvalidTSizeHint(hint));
            }
        }
        let arg_count = reader.read_vle3()? as usize;
        reader.ensure_count(arg_count, 6)?;
        let mut arguments = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            let key = reader.read_u32_le()?;
            let kind = reader.read_u16_le()?;
            if (1..=7).contains(&kind) {
                let header_kind = reader.read_u16_le()?;
                let var_id = reader.read_u32_le()?;
                let payload = match kind {
                    1 => {
                        let nested = reader.with_recursion(|r| Self::decode_inner(r, wire_mode))?;
                        WireLocalizedTextArgPayload::Nested(Box::new(nested))
                    }
                    2 => {
                        let value = reader.read_f64_le()?;
                        let decimal_places = reader.read_u16_le()?;
                        WireLocalizedTextArgPayload::Float {
                            value,
                            decimal_places,
                        }
                    }
                    3 => {
                        let value = reader.read_f64_le()?;
                        let decimal_places = reader.read_u16_le()?;
                        let format_flags = reader.read_u8()?;
                        let radix = reader.read_u16_le()?;
                        WireLocalizedTextArgPayload::FloatFormatted {
                            value,
                            decimal_places,
                            format_flags,
                            radix,
                        }
                    }
                    4 => WireLocalizedTextArgPayload::FixedInt(reader.read_i64_le()?),
                    5 => {
                        let value = reader.read_i64_le()?;
                        let format_flags = reader.read_u8()?;
                        let radix = reader.read_u16_le()?;
                        WireLocalizedTextArgPayload::FixedIntFormatted {
                            value,
                            format_flags,
                            radix,
                        }
                    }
                    6 => {
                        if wire_mode {
                            let is_32bit = reader.read_bool8()?;
                            let value = if is_32bit {
                                u64::from(reader.read_vle4()?)
                            } else {
                                reader.read_u64_le()?
                            };
                            WireLocalizedTextArgPayload::CompactInt { is_32bit, value }
                        } else {
                            let value = reader.read_u64_le()?;
                            WireLocalizedTextArgPayload::CompactInt {
                                is_32bit: false,
                                value,
                            }
                        }
                    }
                    7 => {
                        let (is_32bit, value) = if wire_mode {
                            let is_32bit = reader.read_bool8()?;
                            let value = if is_32bit {
                                u64::from(reader.read_vle4()?)
                            } else {
                                reader.read_u64_le()?
                            };
                            (is_32bit, value)
                        } else {
                            (false, reader.read_u64_le()?)
                        };
                        let format_flags = reader.read_u8()?;
                        let radix = reader.read_u16_le()?;
                        WireLocalizedTextArgPayload::CompactIntFormatted {
                            is_32bit,
                            value,
                            format_flags,
                            radix,
                        }
                    }
                    other => return Err(WorldError::InvalidStringArgKind(other)),
                };
                arguments.push(WireLocalizedTextArg {
                    key,
                    kind,
                    header_kind,
                    var_id,
                    payload: Some(payload),
                });
            } else {
                arguments.push(WireLocalizedTextArg {
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
    /// Code `13`: `LocalizedText`.
    LocalizedText = 13,
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
            13 => Ok(Self::LocalizedText),
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
                let count =
                    u32::try_from(pairs.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
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
            let mut pairs = Vec::with_capacity(count);
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
    /// Type `13`: `WireLocalizedText`.
    LocalizedText(WireLocalizedText),
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
            Self::LocalizedText(_) => PropertyType::LocalizedText,
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

    /// Encodes this property value in wire mode (`0x800` set; §2.7.1).
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
                let count =
                    u32::try_from(items.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
                writer.write_u32_le(count);
                for entry in items {
                    entry.encode_with_depth(writer, depth + 1)?;
                }
            }
            Self::LocalizedText(si) => si.encode(writer)?,
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

    /// Decodes a property value of `prop_type` in wire mode (`0x800` set; §2.7.1).
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
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    entries.push(EncodedPropertyEntry::decode(r, schema)?);
                }
                Ok(Self::Array(entries))
            }),
            PropertyType::LocalizedText => {
                Ok(Self::LocalizedText(WireLocalizedText::decode(reader)?))
            }
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

/// Wire-mode `EncodedPropertyTable` (bit 11 `0x800` set, bit 14 `0x4000` clear:
/// 3-tier VLE count + `(CompactPID, PropertyValue)` entries, no TSize hint byte,
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
        let count =
            u32::try_from(self.entries.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
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
        let mut entries = Vec::with_capacity(count);
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
// Fragmented blob Envelope & Optional Routing Header (§1.1.1–§1.1.2)
// ---------------------------------------------------------------------------

/// Multi-destination routing table entry when bit 0 (`0x01`) of `routingFlags` is set (§1.1.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingDestinationEntry {
    /// 3-tier VLE prefix value.
    pub prefix: u32,
    /// 32-bit routing word (`u32 LE`).
    pub word: u32,
    /// 64-bit destination ID (`u64 LE`).
    pub destination_id: u64,
}

/// Optional variable-length routing header at the start of an uncompressed
/// Fragmented blob body when `envelopeFlags` bit 1 (`0x02` `HasRoutingHeader`) is set (§1.1.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RoutingHeader {
    /// Bitmask of present optional routing sections (`0x01`, `0x02`, `0x04`, `0x08`).
    pub routing_flags: u8,
    /// Bit 0 (`0x01`): multi-destination routing table.
    pub dest_table: Vec<RoutingDestinationEntry>,
    /// Bit 1 (`0x02`): 28-byte sender/routing address block (`3 * u64 LE` + `u32 LE`).
    pub sender_addr_block: Option<([u64; 3], u32)>,
    /// Bit 2 (`0x04`): explicit target entity `InstanceID` override.
    pub override_target_id: Option<u64>,
    /// Bit 3 (`0x08`): extra `u32 LE` words (preceded on wire by `u32 BE` count $K \le 0x72$).
    pub extra_words: Vec<u32>,
}

impl RoutingHeader {
    /// Encodes this routing header and its 2-byte alignment pad (`0x00` if odd byte length; §1.1.2).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        let start_len = writer.len();
        writer.write_u8(self.routing_flags);
        if (self.routing_flags & 0x01) != 0 {
            let count = u32::try_from(self.dest_table.len())
                .map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
            writer.write_vle4(count);
            for entry in &self.dest_table {
                writer.write_vle3(entry.prefix)?;
                writer.write_u32_le(entry.word);
                writer.write_u64_le(entry.destination_id);
            }
        }
        if (self.routing_flags & 0x02) != 0 {
            let (addrs, w) = self.sender_addr_block.unwrap_or(([0; 3], 0));
            writer.write_u64_le(addrs[0]);
            writer.write_u64_le(addrs[1]);
            writer.write_u64_le(addrs[2]);
            writer.write_u32_le(w);
        }
        if (self.routing_flags & 0x04) != 0 {
            writer.write_u64_le(self.override_target_id.unwrap_or(0));
        }
        if (self.routing_flags & 0x08) != 0 {
            let k = u32::try_from(self.extra_words.len())
                .map_err(|_| WorldError::InvalidRoutingExtraWordsCount(u32::MAX))?;
            if k > MAX_ROUTING_EXTRA_WORDS {
                return Err(WorldError::InvalidRoutingExtraWordsCount(k));
            }
            writer.write_u32_be(k);
            for &word in &self.extra_words {
                writer.write_u32_le(word);
            }
        }
        let consumed = writer.len() - start_len;
        if consumed % 2 == 1 {
            writer.write_u8(0x00);
        }
        Ok(())
    }

    /// Decodes a `RoutingHeader` and verifies its 2-byte alignment pad (§1.1.2).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let start_pos = reader.position();
        let routing_flags = reader.read_u8()?;
        let mut dest_table = Vec::new();
        if (routing_flags & 0x01) != 0 {
            let count = reader.read_vle4()? as usize;
            reader.ensure_count(count, 13)?;
            dest_table.reserve(count);
            for _ in 0..count {
                let prefix = reader.read_vle3()?;
                let word = reader.read_u32_le()?;
                let destination_id = reader.read_u64_le()?;
                dest_table.push(RoutingDestinationEntry {
                    prefix,
                    word,
                    destination_id,
                });
            }
        }
        let sender_addr_block = if (routing_flags & 0x02) != 0 {
            let a0 = reader.read_u64_le()?;
            let a1 = reader.read_u64_le()?;
            let a2 = reader.read_u64_le()?;
            let w = reader.read_u32_le()?;
            Some(([a0, a1, a2], w))
        } else {
            None
        };
        let override_target_id = if (routing_flags & 0x04) != 0 {
            Some(reader.read_u64_le()?)
        } else {
            None
        };
        let mut extra_words = Vec::new();
        if (routing_flags & 0x08) != 0 {
            let k = reader.read_u32_be()?;
            if k > MAX_ROUTING_EXTRA_WORDS {
                return Err(WorldError::InvalidRoutingExtraWordsCount(k));
            }
            let k_usize = k as usize;
            reader.ensure_count(k_usize, 4)?;
            extra_words.reserve(k_usize);
            for _ in 0..k_usize {
                extra_words.push(reader.read_u32_le()?);
            }
        }
        let consumed = reader.position() - start_pos;
        if consumed % 2 == 1 {
            let pad = reader.read_u8()?;
            if pad != 0 {
                return Err(WorldError::InvalidRoutingAlignmentPad(pad));
            }
        }
        Ok(Self {
            routing_flags,
            dest_table,
            sender_addr_block,
            override_target_id,
            extra_words,
        })
    }
}

/// Decoded reassembled fragmented blob payload envelope (`docs/specs/official-messages-world.md` §1.1.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedBlobPayload {
    /// Leading `envelopeFlags` byte (`0x01` = zlib, `0x02` = routing header).
    pub envelope_flags: u8,
    /// Optional routing header when bit 1 (`0x02`) of `envelope_flags` is set.
    pub routing_header: Option<RoutingHeader>,
    /// Effective target entity `InstanceID` (`override_target_id` if present, else `default_recipient_id`).
    pub target_entity_id: u64,
    /// Inner event payload starting at offset 0 (`u8 base_flags = 0x00`, §1.3).
    pub inner_payload: Vec<u8>,
}

/// Wraps an inner event payload (already beginning with `u8 base_flags = 0x00`)
/// into a standard uncompressed fragmented blob payload (`envelopeFlags = 0x00` + `inner_payload`; §1.1.2).
#[must_use]
pub fn encode_blob_payload(inner_payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + inner_payload.len());
    out.push(0x00);
    out.extend_from_slice(inner_payload);
    out
}

/// Unwraps a reassembled fragmented blob payload (handling optional zlib decompression
/// and optional routing header) to yield the inner event payload starting at
/// offset 0 (`u8 base_flags = 0x00`; §1.1.2).
pub fn decode_blob_payload(
    blob_bytes: &[u8],
    default_recipient_id: u64,
) -> Result<DecodedBlobPayload, WorldError> {
    let mut outer = WireReader::new(blob_bytes);
    let envelope_flags = outer.read_u8()?;
    let uncompressed_cow: std::borrow::Cow<'_, [u8]> =
        if (envelope_flags & ENVELOPE_FLAG_ZLIB_COMPRESSED) != 0 {
            let uncompressed_len = outer.read_u32_le()?;
            if uncompressed_len > MAX_ENVELOPE_UNCOMPRESSED_LEN {
                return Err(WorldError::EnvelopeUncompressedLengthTooLarge(
                    uncompressed_len,
                ));
            }
            let expected = uncompressed_len as usize;
            let comp_slice = outer.remaining_slice();
            let mut decoder =
                ZlibDecoder::new(comp_slice).take((expected as u64).saturating_add(1));
            let mut inflated = Vec::with_capacity(expected.min(65_536));
            decoder
                .read_to_end(&mut inflated)
                .map_err(|e| WorldError::ZlibDecompressError(e.to_string()))?;
            if inflated.len() != expected {
                return Err(WorldError::ZlibLengthMismatch {
                    expected,
                    actual: inflated.len(),
                });
            }
            std::borrow::Cow::Owned(inflated)
        } else {
            std::borrow::Cow::Borrowed(outer.remaining_slice())
        };

    let mut body = WireReader::new(&uncompressed_cow);
    let routing_header = if (envelope_flags & ENVELOPE_FLAG_HAS_ROUTING_HEADER) != 0 {
        Some(RoutingHeader::decode(&mut body)?)
    } else {
        None
    };
    let target_entity_id = routing_header
        .as_ref()
        .and_then(|rh| rh.override_target_id)
        .unwrap_or(default_recipient_id);
    let inner_payload = body.remaining_slice().to_vec();
    if let Some(&first) = inner_payload.first() {
        if first != 0 {
            return Err(WorldError::InvalidBaseFlags(first));
        }
    } else {
        return Err(WorldError::UnexpectedEof {
            needed: 1,
            remaining: 0,
        });
    }
    Ok(DecodedBlobPayload {
        envelope_flags,
        routing_header,
        target_entity_id,
        inner_payload,
    })
}

// ---------------------------------------------------------------------------
// Character Screen Messages (§3.1–§3.5)
// ---------------------------------------------------------------------------

/// Equipped item visual override entry inside [`EntityAppearanceDesc`] (`22` bytes; §3.1.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearanceEquipmentEntry {
    /// Item class/visual DID (`u32 LE`).
    pub item_class_did: u32,
    /// Equipment slot bitmask (`u32 LE`).
    pub slot_bitmask: u32,
    /// Primary dye/colour value (`u32 LE`).
    pub primary_color: u32,
    /// Secondary dye/colour value (`u32 LE`).
    pub secondary_color: u32,
    /// Boolean visual flag A (`bool8`).
    pub item_flag_a: bool,
    /// Boolean visual flag B (`bool8`).
    pub item_flag_b: bool,
    /// Floating-point visual scalar (`f32 LE`).
    pub item_scalar: f32,
}

impl AppearanceEquipmentEntry {
    /// Encodes this 22-byte equipped item visual entry.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_u32_le(self.item_class_did);
        writer.write_u32_le(self.slot_bitmask);
        writer.write_u32_le(self.primary_color);
        writer.write_u32_le(self.secondary_color);
        writer.write_bool8(self.item_flag_a);
        writer.write_bool8(self.item_flag_b);
        writer.write_f32_le(self.item_scalar);
    }

    /// Decodes a 22-byte equipped item visual entry.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Ok(Self {
            item_class_did: reader.read_u32_le()?,
            slot_bitmask: reader.read_u32_le()?,
            primary_color: reader.read_u32_le()?,
            secondary_color: reader.read_u32_le()?,
            item_flag_a: reader.read_bool8()?,
            item_flag_b: reader.read_bool8()?,
            item_scalar: reader.read_f32_le()?,
        })
    }
}

/// Appearance/morph key-float pair inside [`EntityAppearanceDesc`] (`8` bytes; §3.1.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearanceMorphEntry {
    /// Appearance parameter key / DID (`u32 LE`).
    pub appearance_key: u32,
    /// Morph weight or scalar value (`f32 LE`).
    pub appearance_weight: f32,
}

impl AppearanceMorphEntry {
    /// Encodes this 8-byte appearance entry.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_u32_le(self.appearance_key);
        writer.write_f32_le(self.appearance_weight);
    }

    /// Decodes an 8-byte appearance entry.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Ok(Self {
            appearance_key: reader.read_u32_le()?,
            appearance_weight: reader.read_f32_le()?,
        })
    }
}

/// Auxiliary visual/effect descriptor entry inside [`EntityAppearanceDesc`] (`26` bytes; §3.1.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearanceAuxEntry {
    /// First 32-bit visual identifier (`u32 LE`).
    pub field_0: u32,
    /// Second 32-bit visual identifier (`u32 LE`).
    pub field_4: u32,
    /// First float parameter (`f32 LE`).
    pub field_8: f32,
    /// Second float parameter (`f32 LE`).
    pub field_c: f32,
    /// Third 32-bit visual identifier (`u32 LE`).
    pub field_10: u32,
    /// First byte parameter (`u8`).
    pub field_14: u8,
    /// Second byte parameter (`u8`).
    pub field_15: u8,
    /// Third float parameter (`f32 LE`).
    pub field_18: f32,
}

impl AppearanceAuxEntry {
    /// Encodes this 26-byte block-3 visual entry.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_u32_le(self.field_0);
        writer.write_u32_le(self.field_4);
        writer.write_f32_le(self.field_8);
        writer.write_f32_le(self.field_c);
        writer.write_u32_le(self.field_10);
        writer.write_u8(self.field_14);
        writer.write_u8(self.field_15);
        writer.write_f32_le(self.field_18);
    }

    /// Decodes a 26-byte block-3 visual entry.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        Ok(Self {
            field_0: reader.read_u32_le()?,
            field_4: reader.read_u32_le()?,
            field_8: reader.read_f32_le()?,
            field_c: reader.read_f32_le()?,
            field_10: reader.read_u32_le()?,
            field_14: reader.read_u8()?,
            field_15: reader.read_u8()?,
            field_18: reader.read_f32_le()?,
        })
    }
}

/// Entity/character visual appearance and equipment descriptor (`EntityAppearanceDesc`,
/// used in `CharacterSummaryEntry` and `0x89` `EntityAppearanceDescUpdate`; `docs/specs/official-messages-world.md` §3.1.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityAppearanceDesc {
    /// Equipped item visual overrides (gated by `bool8 has_equipment_array`).
    pub equipment: Vec<AppearanceEquipmentEntry>,
    /// Appearance/morph key-float pairs (gated by `bool8 has_appearance_entries`).
    pub appearance_entries: Vec<AppearanceMorphEntry>,
    /// Unconditional 32-bit animation/visual set DID (`u32 LE`).
    pub anim_set_did: u32,
    /// Auxiliary 26-byte visual/effect entries (gated by `bool8 has_block_3`).
    pub block_3: Vec<AppearanceAuxEntry>,
    /// Optional visual override `EncodedPropertyTable` (gated by `bool8 has_extra_visual_props`).
    pub extra_visual_props: Option<EncodedPropertyTable>,
    /// Trailing boolean visual flag (`bool8`).
    pub visual_flag: bool,
}

impl EntityAppearanceDesc {
    /// Encodes this `EntityAppearanceDesc` in wire mode (§3.1.2).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        if self.equipment.is_empty() {
            writer.write_bool8(false);
        } else {
            let count = u8::try_from(self.equipment.len())
                .map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
            writer.write_bool8(true);
            writer.write_u8(count);
            for item in &self.equipment {
                item.encode(writer);
            }
        }
        if self.appearance_entries.is_empty() {
            writer.write_bool8(false);
        } else {
            let count = u8::try_from(self.appearance_entries.len())
                .map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
            writer.write_bool8(true);
            writer.write_u8(count);
            for entry in &self.appearance_entries {
                entry.encode(writer);
            }
        }
        writer.write_u32_le(self.anim_set_did);
        if self.block_3.is_empty() {
            writer.write_bool8(false);
        } else {
            let count =
                u8::try_from(self.block_3.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
            writer.write_bool8(true);
            writer.write_u8(count);
            for b3 in &self.block_3 {
                b3.encode(writer);
            }
        }
        if let Some(props) = &self.extra_visual_props {
            writer.write_bool8(true);
            props.encode(writer)?;
        } else {
            writer.write_bool8(false);
        }
        writer.write_bool8(self.visual_flag);
        Ok(())
    }

    /// Decodes a `EntityAppearanceDesc` in wire mode using `schema` (§3.1.2).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let has_equip = reader.read_bool8()?;
        let mut equipment = Vec::new();
        if has_equip {
            let count = reader.read_u8()? as usize;
            reader.ensure_count(count, 22)?;
            equipment.reserve(count);
            for _ in 0..count {
                equipment.push(AppearanceEquipmentEntry::decode(reader)?);
            }
        }
        let has_appearance = reader.read_bool8()?;
        let mut appearance_entries = Vec::new();
        if has_appearance {
            let count = reader.read_u8()? as usize;
            reader.ensure_count(count, 8)?;
            appearance_entries.reserve(count);
            for _ in 0..count {
                appearance_entries.push(AppearanceMorphEntry::decode(reader)?);
            }
        }
        let anim_set_did = reader.read_u32_le()?;
        let has_b3 = reader.read_bool8()?;
        let mut block_3 = Vec::new();
        if has_b3 {
            let count = reader.read_u8()? as usize;
            reader.ensure_count(count, 26)?;
            block_3.reserve(count);
            for _ in 0..count {
                block_3.push(AppearanceAuxEntry::decode(reader)?);
            }
        }
        let has_extra = reader.read_bool8()?;
        let extra_visual_props = if has_extra {
            Some(EncodedPropertyTable::decode(reader, schema)?)
        } else {
            None
        };
        let visual_flag = reader.read_bool8()?;
        Ok(Self {
            equipment,
            appearance_entries,
            anim_set_did,
            block_3,
            extra_visual_props,
            visual_flag,
        })
    }
}

/// Per-character summary descriptor (`CharacterSummaryEntry`, used in `0x13D` `CharacterSet`
/// and `0xDD` `CharacterCreateSuccess`; `docs/specs/official-messages-world.md` §3.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterSummaryEntry {
    /// Persistent 64-bit character `InstanceID` (`u64 LE`).
    pub character_id: u64,
    /// Character display name (Prefixed UTF-16LE string).
    pub name: String,
    /// Character visual appearance and equipment descriptor (`EntityAppearanceDesc`, §3.1.2).
    pub appearance_desc: EntityAppearanceDesc,
    /// Character summary `EncodedPropertyTable` (§3.1.3).
    pub char_properties: EncodedPropertyTable,
}

impl CharacterSummaryEntry {
    /// Encodes this `CharacterSummaryEntry` in wire mode (§3.1.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u64_le(self.character_id);
        writer.write_utf16le_string(&self.name)?;
        self.appearance_desc.encode(writer)?;
        self.char_properties.encode(writer)
    }

    /// Decodes a `CharacterSummaryEntry` in wire mode using `schema` (§3.1.1).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let character_id = reader.read_u64_le()?;
        let name = reader.read_utf16le_string()?;
        let appearance_desc = EntityAppearanceDesc::decode(reader, schema)?;
        let char_properties = EncodedPropertyTable::decode(reader, schema)?;
        Ok(Self {
            character_id,
            name,
            appearance_desc,
            char_properties,
        })
    }
}

/// Trailing `(u64 LE key_id, u32 LE val_a, u32 LE val_b)` tuple in [`CharacterSet`] (`16` bytes; §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterSetExtraEntry {
    /// 64-bit key/entity ID (`u64 LE`).
    pub key_id: u64,
    /// First 32-bit metadata word (`u32 LE`).
    pub val_a: u32,
    /// Second 32-bit metadata word (`u32 LE`).
    pub val_b: u32,
}

/// Event `0x13D` (`CharacterSet`, S→C): account character list and account-level properties (§3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterSet {
    /// Free Peoples character entries (`u16 LE char_count`).
    pub characters: Vec<CharacterSummaryEntry>,
    /// Monster Play character entries (`u16 LE monster_count`).
    pub monster_characters: Vec<CharacterSummaryEntry>,
    /// Account-level `EncodedPropertyTable` (§3.1.3).
    pub account_props: EncodedPropertyTable,
    /// Account character-set status or version integer (`i32 LE`).
    pub status_or_version: i32,
    /// Trailing 16-byte account metadata tuples (`i32 LE extra_count`).
    pub extra_entries: Vec<CharacterSetExtraEntry>,
}

impl CharacterSet {
    /// Numeric event ID (`0x0000013D`).
    pub const EVENT_ID: u32 = 0x0000_013D;

    /// Encodes the `0x13D` inner event payload (§3.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        let char_count =
            u16::try_from(self.characters.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_u16_le(char_count);
        for ch in &self.characters {
            ch.encode(writer)?;
        }
        let monster_count = u16::try_from(self.monster_characters.len())
            .map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_u16_le(monster_count);
        for ch in &self.monster_characters {
            ch.encode(writer)?;
        }
        self.account_props.encode(writer)?;
        writer.write_i32_le(self.status_or_version);
        let extra_count = i32::try_from(self.extra_entries.len())
            .map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_i32_le(extra_count);
        for ex in &self.extra_entries {
            writer.write_u64_le(ex.key_id);
            writer.write_u32_le(ex.val_a);
            writer.write_u32_le(ex.val_b);
        }
        Ok(())
    }

    /// Decodes a `0x13D` inner event payload using `schema` (§3.1).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let char_count = reader.read_u16_le()? as usize;
        // Minimum CharacterSummaryEntry wire length: 8 (id) + 1 (empty name) + 9 (empty EntityAppearanceDesc) + 1 (empty props) = 19 bytes.
        reader.ensure_count(char_count, 19)?;
        let mut characters = Vec::with_capacity(char_count);
        for _ in 0..char_count {
            characters.push(CharacterSummaryEntry::decode(reader, schema)?);
        }
        let monster_count = reader.read_u16_le()? as usize;
        reader.ensure_count(monster_count, 19)?;
        let mut monster_characters = Vec::with_capacity(monster_count);
        for _ in 0..monster_count {
            monster_characters.push(CharacterSummaryEntry::decode(reader, schema)?);
        }
        let account_props = EncodedPropertyTable::decode(reader, schema)?;
        let status_or_version = reader.read_i32_le()?;
        let extra_count_i32 = reader.read_i32_le()?;
        let extra_count = usize::try_from(extra_count_i32).unwrap_or(usize::MAX / 16);
        reader.ensure_count(extra_count, 16)?;
        let mut extra_entries = Vec::with_capacity(extra_count);
        for _ in 0..extra_count {
            extra_entries.push(CharacterSetExtraEntry {
                key_id: reader.read_u64_le()?,
                val_a: reader.read_u32_le()?,
                val_b: reader.read_u32_le()?,
            });
        }
        Ok(Self {
            characters,
            monster_characters,
            account_props,
            status_or_version,
            extra_entries,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xDD` (`CharacterCreateSuccess`, S→C): newly created character descriptor (§3.2.3).
#[derive(Debug, Clone, PartialEq)]
pub struct CharacterCreateSuccess {
    /// Complete descriptor of the newly created character (`CharacterSummaryEntry`, §3.1.1).
    pub character: CharacterSummaryEntry,
    /// `false` (`0x00`) for Free Peoples, `true` (`0x01`) for Monster Play (`bool8`).
    pub is_monster_play: bool,
}

impl CharacterCreateSuccess {
    /// Numeric event ID (`0x000000DD`).
    pub const EVENT_ID: u32 = 0x0000_00DD;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.character.encode(writer)?;
        writer.write_bool8(self.is_monster_play);
        Ok(())
    }

    /// Decodes the inner event payload using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let character = CharacterSummaryEntry::decode(reader, schema)?;
        let is_monster_play = reader.read_bool8()?;
        Ok(Self {
            character,
            is_monster_play,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x154` (`CheckCharacterName`, C→S): character name validation request (§3.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckCharacterName {
    /// Proposed character name (Prefixed UTF-16LE string).
    pub name: String,
}

impl CheckCharacterName {
    /// Numeric event ID (`0x00000154`).
    pub const EVENT_ID: u32 = 0x0000_0154;

    /// Encodes the inner event payload (starting with `u8 base_flags = 0x00`).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_utf16le_string(&self.name)
    }

    /// Decodes the inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let name = reader.read_utf16le_string()?;
        Ok(Self { name })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xDC` (`CreateCharacter`, C→S): character creation or modification request (§3.2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CreateCharacter {
    /// Character-generation properties (§3.2.5).
    pub creation_props: EncodedPropertyTable,
    /// Requested character name (Prefixed UTF-16LE string).
    pub name: String,
    /// Character-generation type (`1` = `FreePeoples`, `2` = `MonsterPlayer`, `4` = `SessionPlayer`).
    pub char_gen_type: u8,
    /// Boolean creation option flag A (`bool8`).
    pub flag_a: bool,
    /// Boolean creation option flag B (`bool8`).
    pub flag_b: bool,
    /// 32-bit creation parameter (default `0`).
    pub creation_param: u32,
}

impl CreateCharacter {
    /// Numeric event ID (`0x000000DC`).
    pub const EVENT_ID: u32 = 0x0000_00DC;

    /// Encodes the inner event payload.
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

    /// Decodes the inner event payload using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let creation_props = EncodedPropertyTable::decode(reader, schema)?;
        let name = reader.read_utf16le_string()?;
        let char_gen_type = reader.read_u8()?;
        let flag_a = reader.read_bool8()?;
        let flag_b = reader.read_bool8()?;
        let creation_param = reader.read_u32_le()?;
        Ok(Self {
            creation_props,
            name,
            char_gen_type,
            flag_a,
            flag_b,
            creation_param,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x79` (`CharacterCreateError`, S→C): character creation or name-validation status/error (§3.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterCreateError {
    /// General character-generation result/error code (`u32 LE`).
    pub result_code: u32,
    /// Specific name-validation rejection code (`u32 LE`).
    pub name_error_code: u32,
}

impl CharacterCreateError {
    /// Numeric event ID (`0x00000079`).
    pub const EVENT_ID: u32 = 0x0000_0079;

    /// Encodes the 9-byte inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u32_le(self.result_code);
        writer.write_u32_le(self.name_error_code);
    }

    /// Decodes the 9-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let result_code = reader.read_u32_le()?;
        let name_error_code = reader.read_u32_le()?;
        Ok(Self {
            result_code,
            name_error_code,
        })
    }

    /// Encodes to a standalone 9-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Events `0x7A` (Free Peoples) and `0xD1` (Monster Play) (`DeleteCharacter`, C→S; §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteCharacter {
    /// Persistent 64-bit character `InstanceID`.
    pub character_id: u64,
    /// When `false`, uses event ID `0x7A`; when `true`, uses event ID `0xD1`.
    pub is_monster_play: bool,
}

impl DeleteCharacter {
    /// Event ID for deleting a Free Peoples character (`0x0000007A`).
    pub const EVENT_ID_FREE_PEOPLES: u32 = 0x0000_007A;
    /// Event ID for deleting a Monster Play character (`0x000000D1`).
    pub const EVENT_ID_MONSTER_PLAY: u32 = 0x0000_00D1;

    /// Constructs a Free Peoples character deletion request (`0x7A`).
    #[must_use]
    pub const fn free_peoples(character_id: u64) -> Self {
        Self {
            character_id,
            is_monster_play: false,
        }
    }

    /// Constructs a Monster Play character deletion request (`0xD1`).
    #[must_use]
    pub const fn monster_play(character_id: u64) -> Self {
        Self {
            character_id,
            is_monster_play: true,
        }
    }

    /// Returns the numeric top-level event ID (`0x7A` or `0xD1`).
    #[must_use]
    pub const fn event_id(&self) -> u32 {
        if self.is_monster_play {
            Self::EVENT_ID_MONSTER_PLAY
        } else {
            Self::EVENT_ID_FREE_PEOPLES
        }
    }

    /// Encodes the 9-byte inner event payload (`u8 0x00` + `u64 LE character_id`).
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u64_le(self.character_id);
    }

    /// Decodes the 9-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>, is_monster_play: bool) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let character_id = reader.read_u64_le()?;
        Ok(Self {
            character_id,
            is_monster_play,
        })
    }

    /// Encodes to a standalone 9-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], is_monster_play: bool) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, is_monster_play)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x15` (`CharacterDeletedResult`, S→C): character deletion result (§3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterDeletedResult {
    /// Deleted character `InstanceID` (`u64 LE`).
    pub character_id: u64,
    /// Deletion result code (`i32 LE`, `0` = success).
    pub result_code: i32,
}

impl CharacterDeletedResult {
    /// Numeric event ID (`0x00000015`).
    pub const EVENT_ID: u32 = 0x0000_0015;

    /// Encodes the 13-byte inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u64_le(self.character_id);
        writer.write_i32_le(self.result_code);
    }

    /// Decodes the 13-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let character_id = reader.read_u64_le()?;
        let result_code = reader.read_i32_le()?;
        Ok(Self {
            character_id,
            result_code,
        })
    }

    /// Encodes to a standalone 13-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(13);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x6D` (`CharacterError`, S→C): character selection or screen error (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterError {
    /// Character error code (`u32 LE`).
    pub error_code: u32,
}

impl CharacterError {
    /// Numeric event ID (`0x0000006D`).
    pub const EVENT_ID: u32 = 0x0000_006D;

    /// Encodes the 5-byte inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u32_le(self.error_code);
    }

    /// Decodes the 5-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let error_code = reader.read_u32_le()?;
        Ok(Self { error_code })
    }

    /// Encodes to a standalone 5-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(5);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xB3` (`AccountBooted`, S→C): account disconnect/boot notification (§3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct AccountBooted {
    /// Reason code (`u32 LE`).
    pub boot_reason: u32,
    /// Localized disconnect reason (`WireLocalizedText`, §2.6).
    pub reason_text: WireLocalizedText,
}

impl AccountBooted {
    /// Numeric event ID (`0x000000B3`).
    pub const EVENT_ID: u32 = 0x0000_00B3;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u32_le(self.boot_reason);
        self.reason_text.encode(writer)
    }

    /// Decodes the inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let boot_reason = reader.read_u32_le()?;
        let reason_text = WireLocalizedText::decode(reader)?;
        Ok(Self {
            boot_reason,
            reason_text,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x19E` (`SendSteamId`, C→S): reports the 64-bit Steam platform user ID (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendSteamId {
    /// 64-bit Steam user ID (`u64 LE`).
    pub steam_id: u64,
}

impl SendSteamId {
    /// Numeric event ID (`0x0000019E`).
    pub const EVENT_ID: u32 = 0x0000_019E;

    /// Encodes the 9-byte inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u64_le(self.steam_id);
    }

    /// Decodes the 9-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let steam_id = reader.read_u64_le()?;
        Ok(Self { steam_id })
    }

    /// Encodes to a standalone 9-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x6F` (`SelectCharacter`, C→S): character selection / world entry request (§3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct SelectCharacter {
    /// Selected 64-bit `character_id` (`u64 LE`).
    pub character_id: u64,
    /// Client session/entry property set (empty `0x00` unless entry properties are attached).
    pub client_props: EncodedPropertyTable,
}

impl SelectCharacter {
    /// Numeric event ID (`0x0000006F`).
    pub const EVENT_ID: u32 = 0x0000_006F;

    /// Creates a `SelectCharacter` request with an empty `client_props` set.
    #[must_use]
    pub fn new(character_id: u64) -> Self {
        Self {
            character_id,
            client_props: EncodedPropertyTable::new(),
        }
    }

    /// Encodes the inner event payload (`u8 0x00` + `u64 LE character_id` + `EncodedPropertyTable`).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.character_id);
        self.client_props.encode(writer)
    }

    /// Decodes the inner event payload using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let character_id = reader.read_u64_le()?;
        let client_props = EncodedPropertyTable::decode(reader, schema)?;
        Ok(Self {
            character_id,
            client_props,
        })
    }

    /// Encodes the inner event payload to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::with_capacity(10);
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Encodes this event wrapped in an uncompressed fragmented blob payload
    /// (`envelopeFlags = 0x00` + inner payload; §1.1.2, Vector 7).
    pub fn to_blob_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let inner = self.to_bytes()?;
        Ok(encode_blob_payload(&inner))
    }

    /// Decodes from a standalone inner event byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

// ---------------------------------------------------------------------------
// World Entry & Portal Space (§4.1–§4.3)
// ---------------------------------------------------------------------------

/// Event `0x43` (`WorldName`, S→C): world name announcement (§4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldName {
    /// Localized world name (`WireLocalizedText`, §2.6).
    pub world_name: WireLocalizedText,
}

impl WorldName {
    /// Numeric event ID (`0x00000043`).
    pub const EVENT_ID: u32 = 0x0000_0043;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.world_name.encode(writer)
    }

    /// Decodes the inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let world_name = WireLocalizedText::decode(reader)?;
        Ok(Self { world_name })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xAA` (`DisplayLocalizedText`, S→C): localized system/gameplay text message (§4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayLocalizedText {
    /// Associated target entity `InstanceID` (`u64 LE`).
    pub target_id: u64,
    /// Localized message (`WireLocalizedText`, §2.6).
    pub message: WireLocalizedText,
}

impl DisplayLocalizedText {
    /// Numeric event ID (`0x000000AA`).
    pub const EVENT_ID: u32 = 0x0000_00AA;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.target_id);
        self.message.encode(writer)
    }

    /// Decodes the inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let target_id = reader.read_u64_le()?;
        let message = WireLocalizedText::decode(reader)?;
        Ok(Self { target_id, message })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xFA` (`ServerLastWords`, S→C): final server shutdown/restart broadcast (§4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerLastWords {
    /// Localized shutdown message (`WireLocalizedText`, §2.6).
    pub last_words: WireLocalizedText,
}

impl ServerLastWords {
    /// Numeric event ID (`0x000000FA`).
    pub const EVENT_ID: u32 = 0x0000_00FA;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.last_words.encode(writer)
    }

    /// Decodes the inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let last_words = WireLocalizedText::decode(reader)?;
        Ok(Self { last_words })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x90` (`PlayerSessionStart`, S→C): commits the selected character into the world (§4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerSessionStart {
    /// Active player avatar's 64-bit `InstanceID` (`u64 LE`).
    pub player_id: u64,
    /// Session-start property set (`EncodedPropertyTable`, §2.7.2).
    pub session_props: EncodedPropertyTable,
}

impl PlayerSessionStart {
    /// Numeric event ID (`0x00000090`).
    pub const EVENT_ID: u32 = 0x0000_0090;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.player_id);
        self.session_props.encode(writer)
    }

    /// Decodes the inner event payload using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let player_id = reader.read_u64_le()?;
        let session_props = EncodedPropertyTable::decode(reader, schema)?;
        Ok(Self {
            player_id,
            session_props,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Returns `(n_bool, n_enum)` for the three resolved `PlayerPhysics` state schemas
/// in `client_gamelogic.dat` (`0x07000029`, `docs/specs/official-messages-world.md` §4.3.1).
pub fn portal_state_schema_counts(state_schema_id: u32) -> Result<(usize, usize), WorldError> {
    match state_schema_id {
        DEFAULT_PORTAL_STATE_SCHEMA_ID => Ok((
            DEFAULT_PORTAL_STATE_BOOL_PIDS.len(),
            DEFAULT_PORTAL_STATE_ENUM_PIDS.len(),
        )),
        MOUNTED_PORTAL_STATE_SCHEMA_ID_1 | MOUNTED_PORTAL_STATE_SCHEMA_ID_2 => Ok((0, 0)),
        other => Err(WorldError::UnknownPortalStateSchema(other)),
    }
}

/// Event `0xCC` (`PortalPlacement`, S→C): portal/teleport placement and physics state sync (§4.3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct PortalPlacement {
    /// Presence and flag bitmask (`bit 0 = flag_60`, `bits 1..=6 = vel/omega`, `bit 7 = explicit state_schema_id`).
    pub pos_sync_flags: u8,
    /// Sequence number or `TeleportReason` (`0x2300038B`, `u32 LE`).
    pub sequence_or_reason: u32,
    /// Base wire `Position` (without quaternion; §2.5.1).
    pub position: WirePosition,
    /// Quantized 1-byte heading (`u8`, §2.5.2).
    pub heading_raw: u8,
    /// Movement mode byte (`u8`).
    pub movement_mode: u8,
    /// Linear velocity `(vel_x, vel_y, vel_z)` in m/s (components gated by bits `0x02`, `0x04`, `0x08`).
    pub linear_velocity: [f32; 3],
    /// Angular velocity `(ang_vel_x, ang_vel_y, ang_vel_z)` (components gated by bits `0x10`, `0x20`, `0x40`).
    pub angular_velocity: [f32; 3],
    /// Animation or context `CompactDid` (§2.4.2).
    pub anim_or_context_did: CompactDid,
    /// Resolved `state_schema_id` (`0x0000000C` when bit 7 `0x80` of `pos_sync_flags` is clear).
    pub state_schema_id: u32,
    /// Unpacked boolean state values (`8` entries for schema `0x0C`, `0` for `0x10000E50`/`0x10000E51`).
    pub bool_states: Vec<bool>,
    /// Unpacked enum state values (`6` entries for schema `0x0C`, `0` for `0x10000E50`/`0x10000E51`).
    pub enum_states: Vec<u32>,
    /// Trailing sub-archive bytes (`u32 LE extra_blob_size` + `byte[extra_blob_size]`).
    pub extra_blob: Vec<u8>,
}

impl PortalPlacement {
    /// Numeric event ID (`0x000000CC`).
    pub const EVENT_ID: u32 = 0x0000_00CC;

    /// Looks up the boolean state value for `pid` when `state_schema_id == 0x0000000C`.
    #[must_use]
    pub fn bool_state_for_pid(&self, pid: u32) -> Option<bool> {
        if self.state_schema_id != DEFAULT_PORTAL_STATE_SCHEMA_ID {
            return None;
        }
        let idx = DEFAULT_PORTAL_STATE_BOOL_PIDS
            .iter()
            .position(|&p| p == pid)?;
        self.bool_states.get(idx).copied()
    }

    /// Looks up the enum state value for `pid` when `state_schema_id == 0x0000000C`.
    #[must_use]
    pub fn enum_state_for_pid(&self, pid: u32) -> Option<u32> {
        if self.state_schema_id != DEFAULT_PORTAL_STATE_SCHEMA_ID {
            return None;
        }
        let idx = DEFAULT_PORTAL_STATE_ENUM_PIDS
            .iter()
            .position(|&p| p == pid)?;
        self.enum_states.get(idx).copied()
    }

    /// Encodes the state-schema tail (steps 9a, 9b, 10, 11 of §4.3.1; Vector 8).
    pub fn encode_state_section(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        let (expected_bools, expected_enums) = portal_state_schema_counts(self.state_schema_id)?;
        if self.bool_states.len() != expected_bools || self.enum_states.len() != expected_enums {
            return Err(WorldError::PortalStateSchemaLengthMismatch {
                schema_id: self.state_schema_id,
                expected_bools,
                actual_bools: self.bool_states.len(),
                expected_enums,
                actual_enums: self.enum_states.len(),
            });
        }
        let byte_count = expected_bools.div_ceil(8);
        for byte_idx in 0..byte_count {
            let mut packed = 0u8;
            for bit in 0..8 {
                let bool_idx = byte_idx * 8 + bit;
                if self.bool_states.get(bool_idx).copied().unwrap_or(false) {
                    packed |= 1u8 << bit;
                }
            }
            writer.write_u8(packed);
        }
        for &val in &self.enum_states {
            writer.write_vle3(val)?;
        }
        let extra_len =
            u32::try_from(self.extra_blob.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_u32_le(extra_len);
        writer.write_bytes(&self.extra_blob);
        Ok(())
    }

    /// Decodes the state-schema tail (steps 9a, 9b, 10, 11 of §4.3.1; Vector 8) for `state_schema_id`.
    #[allow(clippy::type_complexity)]
    pub fn decode_state_section(
        reader: &mut WireReader<'_>,
        state_schema_id: u32,
    ) -> Result<(Vec<bool>, Vec<u32>, Vec<u8>), WorldError> {
        let (n_bool, n_enum) = portal_state_schema_counts(state_schema_id)?;
        let bool_byte_count = n_bool.div_ceil(8);
        let raw_bool_bytes = reader.read_bytes(bool_byte_count)?;
        let mut bool_states = Vec::with_capacity(n_bool);
        for i in 0..n_bool {
            let b = raw_bool_bytes[i / 8];
            bool_states.push(((b >> (i % 8)) & 1) != 0);
        }
        reader.ensure_count(n_enum, 1)?;
        let mut enum_states = Vec::with_capacity(n_enum);
        for _ in 0..n_enum {
            enum_states.push(reader.read_vle3()?);
        }
        let extra_blob_size = reader.read_u32_le()? as usize;
        reader.ensure_count(extra_blob_size, 1)?;
        let extra_blob = reader.read_bytes(extra_blob_size)?.to_vec();
        Ok((bool_states, enum_states, extra_blob))
    }

    /// Encodes the full `0xCC` inner event payload (§4.3.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        let mut flags = self.pos_sync_flags;
        if self.state_schema_id != DEFAULT_PORTAL_STATE_SCHEMA_ID {
            flags |= 0x80;
        }
        writer.write_u8(flags);
        writer.write_u32_le(self.sequence_or_reason);
        self.position.encode_base(writer);
        writer.write_u8(self.heading_raw);
        writer.write_u8(self.movement_mode);
        if (flags & 0x02) != 0 {
            writer.write_f32_le(self.linear_velocity[0]);
        }
        if (flags & 0x04) != 0 {
            writer.write_f32_le(self.linear_velocity[1]);
        }
        if (flags & 0x08) != 0 {
            writer.write_f32_le(self.linear_velocity[2]);
        }
        if (flags & 0x10) != 0 {
            writer.write_f32_le(self.angular_velocity[0]);
        }
        if (flags & 0x20) != 0 {
            writer.write_f32_le(self.angular_velocity[1]);
        }
        if (flags & 0x40) != 0 {
            writer.write_f32_le(self.angular_velocity[2]);
        }
        self.anim_or_context_did.encode(writer)?;
        if (flags & 0x80) != 0 {
            writer.write_u32_le(self.state_schema_id);
        }
        self.encode_state_section(writer)
    }

    /// Decodes the full `0xCC` inner event payload (§4.3.1).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let pos_sync_flags = reader.read_u8()?;
        let sequence_or_reason = reader.read_u32_le()?;
        let position = WirePosition::decode_base(reader)?;
        // Play §9.1 rule 4: position and velocity components must be finite.
        for (field, v) in ["pos_x", "pos_y", "pos_z"].into_iter().zip(position.offset) {
            require_finite(field, v)?;
        }
        let heading_raw = reader.read_u8()?;
        let movement_mode = reader.read_u8()?;
        let mut kinematics = [0.0f32; 6];
        let names = [
            "vel_x",
            "vel_y",
            "vel_z",
            "ang_vel_x",
            "ang_vel_y",
            "ang_vel_z",
        ];
        for (bit, (slot, field)) in kinematics.iter_mut().zip(names).enumerate() {
            if (pos_sync_flags & (0x02 << bit)) != 0 {
                *slot = require_finite(field, reader.read_f32_le()?)?;
            }
        }
        let [vel_x, vel_y, vel_z, ang_x, ang_y, ang_z] = kinematics;
        let anim_or_context_did = CompactDid::decode(reader)?;
        let state_schema_id = if (pos_sync_flags & 0x80) != 0 {
            reader.read_u32_le()?
        } else {
            DEFAULT_PORTAL_STATE_SCHEMA_ID
        };
        let (bool_states, enum_states, extra_blob) =
            Self::decode_state_section(reader, state_schema_id)?;
        Ok(Self {
            pos_sync_flags,
            sequence_or_reason,
            position,
            heading_raw,
            movement_mode,
            linear_velocity: [vel_x, vel_y, vel_z],
            angular_velocity: [ang_x, ang_y, ang_z],
            anim_or_context_did,
            state_schema_id,
            bool_states,
            enum_states,
            extra_blob,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xC7` (`PortalSpaceExit`, S→C): 1-byte notification to exit portal space (§4.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PortalSpaceExit;

impl PortalSpaceExit {
    /// Numeric event ID (`0x000000C7`).
    pub const EVENT_ID: u32 = 0x0000_00C7;

    /// Encodes the 1-byte inner payload (`0x00`).
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
    }

    /// Decodes the 1-byte inner payload (`0x00`).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self)
    }

    /// Encodes to `[0x00]`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![0x00]
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x153` (`PortalSpaceState`, S→C): 1-byte portal-space state notification (§4.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PortalSpaceState;

impl PortalSpaceState {
    /// Numeric event ID (`0x00000153`).
    pub const EVENT_ID: u32 = 0x0000_0153;

    /// Encodes the 1-byte inner payload (`0x00`).
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
    }

    /// Decodes the 1-byte inner payload (`0x00`).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self)
    }

    /// Encodes to `[0x00]`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![0x00]
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xF4` (`PortalSpaceReady`, C→S): 1-byte client-ready acknowledgement (§4.3.1, Vector 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PortalSpaceReady;

impl PortalSpaceReady {
    /// Numeric event ID (`0x000000F4`).
    pub const EVENT_ID: u32 = 0x0000_00F4;

    /// Encodes the 1-byte inner payload (`0x00`).
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
    }

    /// Decodes the 1-byte inner payload (`0x00`).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self)
    }

    /// Encodes to `[0x00]`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![0x00]
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

// ---------------------------------------------------------------------------
// Network Clock & WorldCalendarConfig (§4.4.1–§4.4.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingClockSync {
    pending_server_time: f64,
    pending_receipt_raw: f64,
    forced: bool,
}

/// Smoothed monotonic network clock implementing the 3-stage latency-compensated
/// seeding, forward snap, and 5-tier downward slew algorithm
/// (`docs/specs/official-messages-world.md` §4.4.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetworkClockSync {
    synced_time: f64,
    raw_elapsed: f64,
    clock_offset: f64,
    slew_rate: f64,
    slew_debt: f64,
    prev_server_time: f64,
    pending: Option<PendingClockSync>,
}

impl NetworkClockSync {
    /// Creates a new network clock seeded at `initial_server_time` and `initial_raw_time`.
    #[must_use]
    pub fn new(initial_server_time: f64, initial_raw_time: f64) -> Self {
        Self {
            synced_time: initial_server_time,
            raw_elapsed: initial_raw_time,
            clock_offset: initial_server_time - initial_raw_time,
            slew_rate: 0.0,
            slew_debt: 0.0,
            prev_server_time: 0.0,
            pending: None,
        }
    }

    /// Current smoothed monotonic server time (`f64` seconds).
    #[must_use]
    pub const fn synced_time(&self) -> f64 {
        self.synced_time
    }

    /// Most recent local hardware elapsed time (`f64` seconds).
    #[must_use]
    pub const fn raw_elapsed(&self) -> f64 {
        self.raw_elapsed
    }

    /// Current additive clock offset (`synced_time - raw_elapsed`).
    #[must_use]
    pub const fn clock_offset(&self) -> f64 {
        self.clock_offset
    }

    /// Active downward slew rate (seconds slewed per second elapsed).
    #[must_use]
    pub const fn slew_rate(&self) -> f64 {
        self.slew_rate
    }

    /// Remaining excess time to bleed off (`slew_debt` in seconds).
    #[must_use]
    pub const fn slew_debt(&self) -> f64 {
        self.slew_debt
    }

    /// Previous `pending_server_time` recorded when downward slewing started.
    #[must_use]
    pub const fn prev_server_time(&self) -> f64 {
        self.prev_server_time
    }

    /// Returns the base 5-tier downward slew rate for `excess = synced_time - target` (§4.4.1).
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

    /// Stage 1 & Stage 2 (§4.4.1): stages a `ClockSync` (`0x01000000`) sample
    /// received at UDP socket time `t_recv` and dequeued at control-queue time `t_dispatch`.
    pub fn stage_clock_sync(&mut self, server_time: f64, t_recv: f64, t_dispatch: f64) {
        self.stage_clock_sync_inner(server_time, t_recv, t_dispatch, false);
    }

    /// Stages a forced `ClockSync` sample (e.g. initial `ConnectChallenge` seed).
    pub fn stage_forced_clock_sync(&mut self, server_time: f64, t_recv: f64, t_dispatch: f64) {
        self.stage_clock_sync_inner(server_time, t_recv, t_dispatch, true);
    }

    fn stage_clock_sync_inner(
        &mut self,
        server_time: f64,
        t_recv: f64,
        t_dispatch: f64,
        forced: bool,
    ) {
        let pending_server_time = (t_dispatch - t_recv) + server_time;
        self.pending = Some(PendingClockSync {
            pending_server_time,
            pending_receipt_raw: t_dispatch,
            forced,
        });
    }

    /// Stage 3 (§4.4.1): advances the smoothed clock to local hardware time `t_now`,
    /// applying any staged `ClockSync` sample and bleeding off `slew_debt`.
    pub fn update_frame(&mut self, t_now: f64) -> f64 {
        let dt = (t_now - self.raw_elapsed).max(0.0);
        self.raw_elapsed = t_now;

        if self.slew_debt > 0.0 && dt > 0.0 {
            let step = self.slew_debt.min(dt * self.slew_rate);
            self.slew_debt -= step;
            self.clock_offset -= step;
        }
        self.synced_time = self.raw_elapsed + self.clock_offset;

        if let Some(pending) = self.pending.take() {
            let target = (t_now - pending.pending_receipt_raw) + pending.pending_server_time;
            if pending.forced || target > self.synced_time {
                // Client behind server: snap forward immediately.
                self.synced_time = target;
                self.clock_offset = target - self.raw_elapsed;
                self.slew_rate = 0.0;
                self.slew_debt = 0.0;
                self.prev_server_time = 0.0;
            } else {
                // Client ahead of server: never jump backward; slew if excess >= 0.015 s.
                let excess = self.synced_time - target;
                if excess < 0.015 {
                    self.slew_rate = 0.0;
                    self.slew_debt = 0.0;
                    self.prev_server_time = 0.0;
                } else {
                    self.slew_debt = excess;
                    let mut rate = Self::base_slew_rate_for_excess(excess);
                    if self.prev_server_time > 0.0 {
                        let server_delta = pending.pending_server_time - self.prev_server_time;
                        if server_delta > 0.0 && (excess / server_delta) > rate {
                            rate += 0.0002;
                        }
                    }
                    self.slew_rate = rate;
                    self.prev_server_time = pending.pending_server_time;
                }
            }
        }

        self.synced_time
    }

    /// Converts a wire Type 4 (`Timestamp`) value into a client timestamp (§4.4.1 item 3).
    #[must_use]
    pub fn decode_wire_timestamp(&self, wire_val: f64, is_absolute: bool) -> f64 {
        if wire_val == TIMESTAMP_NO_TIME || is_absolute {
            wire_val
        } else {
            wire_val + self.synced_time
        }
    }

    /// Converts a client timestamp into a wire Type 4 (`Timestamp`) value (§4.4.1 item 3).
    #[must_use]
    pub fn encode_wire_timestamp(&self, timestamp: f64, is_absolute: bool) -> f64 {
        if timestamp == TIMESTAMP_NO_TIME || is_absolute {
            timestamp
        } else {
            timestamp - self.synced_time
        }
    }
}

/// Decoded `0x35000000` (`WorldCalendarConfig`) calendar record from `client_general.dat`
/// (`180` bytes in the standard archive; `docs/specs/official-messages-world.md` §4.4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct WorldCalendarConfig {
    /// Record DID (`0x35000000`).
    pub did: u32,
    /// Duration of one in-game day in seconds (`11160.0` in `client_general.dat`).
    pub day_length_seconds: f32,
    /// Number of in-game days per calendar year (`360`).
    pub days_per_year: i32,
    /// Additive calendar epoch offset in seconds (`3600.0`).
    pub calendar_epoch_offset: f64,
    /// Base calendar year offset (`34`).
    pub base_year: i32,
    /// Normalized time-of-day fraction `[0.0, 1.0]` at which night begins (`0.80`).
    pub dusk_threshold: f32,
    /// Normalized time-of-day fraction `[0.0, 1.0]` at which day begins (`0.25`).
    pub dawn_threshold: f32,
    /// Display phases `(start_fraction, string_token)`.
    pub tod_phases: Vec<(f32, u32)>,
    /// Auxiliary calendar string tokens.
    pub aux_tokens: Vec<u32>,
    /// Calendar months `(start_day_of_year, month_string_token)`.
    pub months: Vec<(u32, u32)>,
    /// Calendar era string token (`0x005FB893`).
    pub era_string_token: u32,
}

/// Computed time-of-day, day/night flag, and calendar date from [`WorldCalendarConfig`] (§4.4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldCalendarSnapshot {
    /// Normalized fraction `[0.0, 1.0]` of the current in-game day (`0x00000682` `World_PresentTimeOfDay`).
    pub present_time_of_day: f32,
    /// `true` when `present_time_of_day < dawn_threshold` or `>= dusk_threshold` (`0x000001F0` `World_IsNight`).
    pub is_night: bool,
    /// Computed calendar year (`year_index + base_year`).
    pub calendar_year: i32,
    /// Zero-based day of the calendar year (`0..days_per_year`).
    pub day_of_year: i32,
    /// Dilated time at the start of the current in-game day.
    pub day_start_time: f64,
}

impl WorldCalendarConfig {
    /// Constructs the canonical `0x35000000` record matching `client_general.dat` (§4.4.2).
    #[must_use]
    pub fn canonical() -> Self {
        Self {
            did: WORLD_CALENDAR_CONFIG_DID,
            day_length_seconds: 11_160.0,
            days_per_year: 360,
            calendar_epoch_offset: 3_600.0,
            base_year: 34,
            dusk_threshold: 0.80,
            dawn_threshold: 0.25,
            tod_phases: vec![
                (0.0, 0x0FB4_F8C4),
                (0.25, 0x0004_A8DE),
                (0.50, 0x0005_565E),
                (0.75, 0x05AC_59C4),
            ],
            aux_tokens: vec![0x0004_A8C6],
            months: vec![
                (0, 0x0546_5AB1),
                (30, 0x0546_5AB2),
                (60, 0x0546_5AB3),
                (90, 0x0546_5AB4),
                (120, 0x0546_5AB5),
                (150, 0x0546_5AB6),
                (180, 0x0546_5AB7),
                (210, 0x0546_5AB8),
                (240, 0x0546_5AB9),
                (270, 0x0465_AB10),
                (300, 0x0465_AB11),
                (330, 0x0465_AB12),
            ],
            era_string_token: 0x005F_B893,
        }
    }

    /// Encodes this `0x35000000` record (`180` bytes for the canonical record; §4.4.2).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u32_le(self.did);
        writer.write_f32_le(self.day_length_seconds);
        writer.write_i32_le(self.days_per_year);
        writer.write_f64_le(self.calendar_epoch_offset);
        writer.write_i32_le(self.base_year);
        writer.write_f32_le(self.dusk_threshold);
        writer.write_f32_le(self.dawn_threshold);
        let phase_count =
            u32::try_from(self.tod_phases.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_u32_le(phase_count);
        for &(frac, tok) in &self.tod_phases {
            writer.write_f32_le(frac);
            writer.write_u32_le(tok);
        }
        let aux_count =
            u32::try_from(self.aux_tokens.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_u32_le(aux_count);
        for &tok in &self.aux_tokens {
            writer.write_u32_le(tok);
        }
        let month_count =
            u32::try_from(self.months.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_u32_le(month_count);
        for &(start_day, tok) in &self.months {
            writer.write_u32_le(start_day);
            writer.write_u32_le(tok);
        }
        writer.write_u32_le(self.era_string_token);
        Ok(())
    }

    /// Decodes a `0x35000000` `WorldCalendarConfig` from `reader` (§4.4.2).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let did = reader.read_u32_le()?;
        if did != WORLD_CALENDAR_CONFIG_DID {
            return Err(WorldError::InvalidWorldCalendarDid(did));
        }
        let day_length_seconds = reader.read_f32_le()?;
        let days_per_year = reader.read_i32_le()?;
        let calendar_epoch_offset = reader.read_f64_le()?;
        let base_year = reader.read_i32_le()?;
        let dusk_threshold = reader.read_f32_le()?;
        let dawn_threshold = reader.read_f32_le()?;
        let tod_phase_count = reader.read_u32_le()? as usize;
        reader.ensure_count(tod_phase_count, 8)?;
        let mut tod_phases = Vec::with_capacity(tod_phase_count);
        for _ in 0..tod_phase_count {
            tod_phases.push((reader.read_f32_le()?, reader.read_u32_le()?));
        }
        let aux_token_count = reader.read_u32_le()? as usize;
        reader.ensure_count(aux_token_count, 4)?;
        let mut aux_tokens = Vec::with_capacity(aux_token_count);
        for _ in 0..aux_token_count {
            aux_tokens.push(reader.read_u32_le()?);
        }
        let month_count = reader.read_u32_le()? as usize;
        reader.ensure_count(month_count, 8)?;
        let mut months = Vec::with_capacity(month_count);
        for _ in 0..month_count {
            months.push((reader.read_u32_le()?, reader.read_u32_le()?));
        }
        let era_string_token = reader.read_u32_le()?;
        Ok(Self {
            did,
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

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::with_capacity(180);
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let rec = Self::decode(&mut r)?;
        r.finish()?;
        Ok(rec)
    }

    /// Returns `true` if `present_time_of_day` falls in the nighttime interval
    /// (`< dawn_threshold` or `>= dusk_threshold`; §4.4.2).
    #[must_use]
    pub fn is_night(&self, present_time_of_day: f32) -> bool {
        present_time_of_day < self.dawn_threshold || present_time_of_day >= self.dusk_threshold
    }

    /// Evaluates the calendar year, day of year, time-of-day fraction `[0.0, 1.0)`,
    /// and `is_night` flag at `dilated_time` (§4.4.2 steps 3–4).
    #[must_use]
    pub fn evaluate(&self, dilated_time: f64) -> WorldCalendarSnapshot {
        let day_len = if self.day_length_seconds > 0.0 {
            f64::from(self.day_length_seconds)
        } else {
            f64::from(DEFAULT_FALLBACK_DAY_LENGTH_SECONDS)
        };
        let days_per_yr = f64::from(self.days_per_year.max(1));
        let year_len = days_per_yr * day_len;

        let t_cal = dilated_time + self.calendar_epoch_offset;
        let year_index = (t_cal / year_len).floor();
        let calendar_year = (year_index as i32).saturating_add(self.base_year);
        let t_rem = t_cal - year_index * year_len;
        let day_of_year = (t_rem / day_len).floor() as i32;
        let day_start_time = dilated_time - (t_rem - f64::from(day_of_year) * day_len);
        let present_time_of_day = ((dilated_time - day_start_time) / day_len) as f32;
        let is_night = self.is_night(present_time_of_day);

        WorldCalendarSnapshot {
            present_time_of_day,
            is_night,
            calendar_year,
            day_of_year,
            day_start_time,
        }
    }
}

// ---------------------------------------------------------------------------
// Object Lifecycle & Movement (§5.1–§5.6)
// ---------------------------------------------------------------------------

/// A single framed sub-event inside a `0x0D` (`EntityEntryBatch`) or `0x4C` (`EntityBatchUpdate`)
/// compound batch event (`docs/specs/official-messages-world.md` §5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubEventFrame {
    /// Non-zero `sub_event_id` with an explicit `u32 LE payload_size` and inner `payload` slice.
    Event {
        /// Numeric event ID of the inner event (e.g. `0x98`, `0x52`, `0xCE`, `0x89`).
        sub_event_id: u32,
        /// Inner event payload starting directly at offset 0 (`u8 base_flags = 0x00`).
        payload: Vec<u8>,
    },
    /// Null sub-event entry (`sub_event_id == 0`) with `null_field_a` (16-bit VLE) and `null_field_b` (`u16 LE`).
    Null {
        /// 16-bit metadata word A (`16-bit VLE`, §2.2.3).
        field_a: u16,
        /// 16-bit metadata word B (`u16 LE`).
        field_b: u16,
    },
}

impl SubEventFrame {
    /// Encodes this sub-event frame into `writer` (§5.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        match self {
            Self::Event {
                sub_event_id,
                payload,
            } => {
                writer.write_vle4(*sub_event_id);
                let len =
                    u32::try_from(payload.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
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

    /// Decodes a single `SubEventFrame` from `reader` (§5.1, §7.1).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        let sub_event_id = reader.read_vle4()?;
        if sub_event_id == 0 {
            let field_a = reader.read_vle16()?;
            let field_b = reader.read_u16_le()?;
            Ok(Self::Null { field_a, field_b })
        } else {
            let payload_size = reader.read_u32_le()? as usize;
            reader.ensure_count(payload_size, 1)?;
            let payload = reader.read_bytes(payload_size)?.to_vec();
            Ok(Self::Event {
                sub_event_id,
                payload,
            })
        }
    }
}

/// Compound sub-event batch (`0x0D` `EntityEntryBatch` or `0x4C` `EntityBatchUpdate`, S→C; §5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityBatchSubEvents {
    /// Ordered framed sub-events.
    pub sub_events: Vec<SubEventFrame>,
}

impl EntityBatchSubEvents {
    /// Event ID for `EntityEntryBatch` (`0x0000000D`).
    pub const EVENT_ID_ENTITY_ENTRY_BATCH: u32 = 0x0000_000D;
    /// Event ID for `EntityBatchUpdate` (`0x0000004C`).
    pub const EVENT_ID_ENTITY_BATCH_UPDATE: u32 = 0x0000_004C;

    /// Encodes the compound batch inner payload (§5.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        let count =
            u32::try_from(self.sub_events.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
        writer.write_vle4(count);
        for frame in &self.sub_events {
            frame.encode(writer)?;
        }
        Ok(())
    }

    /// Decodes a compound batch inner payload (§5.1, §7.1).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let count = reader.read_vle4()? as usize;
        reader.ensure_count(count, 4)?;
        let mut sub_events = Vec::with_capacity(count);
        for _ in 0..count {
            sub_events.push(SubEventFrame::decode(reader)?);
        }
        Ok(Self { sub_events })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let batch = Self::decode(&mut r)?;
        r.finish()?;
        Ok(batch)
    }
}

/// Event `0x98` (`EntityCreate`, S→C): instantiates a world entity in the object table (§5.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityCreate {
    /// Template game-object `DID` (`u32 LE`, typically `0x70xxxxxx`).
    pub template_did: u32,
    /// Runtime 64-bit `InstanceID` of the spawned entity (`u64 LE`).
    pub entity_id: u64,
}

impl EntityCreate {
    /// Numeric event ID (`0x00000098`).
    pub const EVENT_ID: u32 = 0x0000_0098;

    /// Encodes the 13-byte inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u32_le(self.template_did);
        writer.write_u64_le(self.entity_id);
    }

    /// Decodes the 13-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let template_did = reader.read_u32_le()?;
        let entity_id = reader.read_u64_le()?;
        Ok(Self {
            template_did,
            entity_id,
        })
    }

    /// Encodes to a standalone 13-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(13);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x52` (`EntityInitialProperties`, S→C): delivers a wire `EncodedPropertyTable` for an entity (§5.2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityInitialProperties {
    /// Dynamic property overrides (`EncodedPropertyTable`, §2.7.2).
    pub properties: EncodedPropertyTable,
}

impl EntityInitialProperties {
    /// Numeric event ID (`0x00000052`).
    pub const EVENT_ID: u32 = 0x0000_0052;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.properties.encode(writer)
    }

    /// Decodes the inner event payload using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let properties = EncodedPropertyTable::decode(reader, schema)?;
        Ok(Self { properties })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x40` (`DeleteEntity`, S→C): 1-byte entity deletion / entity exit event
/// (target `InstanceID` is carried in the enclosing fragmented blob's `recipientId`; §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeleteEntity;

impl DeleteEntity {
    /// Numeric event ID (`0x00000040`).
    pub const EVENT_ID: u32 = 0x0000_0040;

    /// Encodes the 1-byte inner payload (`0x00`).
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
    }

    /// Decodes the 1-byte inner payload (`0x00`).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        Ok(Self)
    }

    /// Encodes to `[0x00]`.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![0x00]
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xD0` (`QueryMissingEntity`, C→S): requests full entity data for an unknown `InstanceID` (§5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryMissingEntity {
    /// Missing entity's 64-bit `InstanceID` (`u64 LE`).
    pub entity_id: u64,
}

impl QueryMissingEntity {
    /// Numeric event ID (`0x000000D0`).
    pub const EVENT_ID: u32 = 0x0000_00D0;

    /// Encodes the 9-byte inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        writer.write_u64_le(self.entity_id);
    }

    /// Decodes the 9-byte inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let entity_id = reader.read_u64_le()?;
        Ok(Self { entity_id })
    }

    /// Encodes to a standalone 9-byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Decodes a `0xCE` base wire `Position` and checks its local offset against play §9.1 rule 4
/// (finite and within `±MAX_WORLD_COORDINATE_M` metres).
fn decode_ce_world_position(reader: &mut WireReader<'_>) -> Result<WirePosition, WorldError> {
    let position = WirePosition::decode_base(reader)?;
    for (field, v) in ["pos_x", "pos_y", "pos_z"].into_iter().zip(position.offset) {
        require_world_coordinate(field, v)?;
    }
    Ok(position)
}

/// Placement mode payload inside [`EntityPositionUpdate`] (`0xCE`, §5.4).
#[derive(Debug, Clone, PartialEq)]
pub enum EntityPlacement {
    /// Bit 2 (`0x04`) clear and bit 4 (`0x10`) clear: base wire `Position` + quantized `u8` heading.
    WorldWithHeading {
        /// Base wire `Position` (no quaternion; §2.5.1).
        position: WirePosition,
        /// Quantized 1-byte heading (§2.5.2).
        heading_raw: u8,
    },
    /// Bit 2 (`0x04`) clear and bit 4 (`0x10`) set: base wire `Position` without heading.
    WorldPositionOnly {
        /// Base wire `Position` (no quaternion; §2.5.1).
        position: WirePosition,
    },
    /// Bit 2 (`0x04`) set (and bit 3 `0x08` clear): parent entity attachment.
    ParentAttachment {
        /// Parent entity `InstanceID` (`u64 LE`).
        parent_id: u64,
        /// Attachment slot/bone ID (3-tier VLE).
        attach_slot: u32,
    },
}

/// Event `0xCE` (`EntityPositionUpdate`, S→C): entity position, velocity, and attachment replication (§5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityPositionUpdate {
    /// Bitmask of optional header fields (`0x01..=0x40`).
    pub header_flags: u8,
    /// Replicated entity's 64-bit `InstanceID` (`u64 LE`).
    pub entity_id: u64,
    /// Placement mode bitmask (`bit 2 = attachment`, `bit 4 = omit heading`).
    pub mode_flags: u8,
    /// Optional sequence/state byte when `header_flags` bit 3 (`0x08`) is set.
    pub seq_byte: Option<u8>,
    /// Linear velocity `(vel_x, vel_y, vel_z)` in m/s (gated by `header_flags` bits `0x10`, `0x20`, `0x40`).
    pub velocity: [f32; 3],
    /// Optional movement/animation state ID (3-tier VLE when `header_flags` bit 0 `0x01` is set; defaults to `0`).
    pub anim_state: u32,
    /// World position (+ optional heading) or parent attachment.
    pub placement: EntityPlacement,
    /// Interpolation parameter A (read when `header_flags` bit 1 `0x02` is set; defaults to `5`).
    pub interp_param_a: u8,
    /// Interpolation parameter B (read when `header_flags` bit 2 `0x04` is set; defaults to `2`).
    pub interp_param_b: u8,
}

impl EntityPositionUpdate {
    /// Numeric event ID (`0x000000CE`).
    pub const EVENT_ID: u32 = 0x0000_00CE;

    /// Encodes the `0xCE` inner event payload (§5.4).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        let mut hflags = self.header_flags;
        if self.seq_byte.is_some() {
            hflags |= 0x08;
        }
        let mut mflags = self.mode_flags & !(0x04 | 0x10);
        match &self.placement {
            EntityPlacement::WorldWithHeading { .. } => {}
            EntityPlacement::WorldPositionOnly { .. } => {
                mflags |= 0x10;
            }
            EntityPlacement::ParentAttachment { .. } => {
                mflags |= 0x04;
            }
        }
        if (mflags & 0x08) != 0 && (mflags & 0x04) != 0 {
            return Err(WorldError::UnsupportedExtendedAttachment(mflags));
        }
        writer.write_u8(hflags);
        writer.write_u64_le(self.entity_id);
        writer.write_u8(mflags);
        if let Some(seq) = self.seq_byte {
            writer.write_u8(seq);
        }
        if (hflags & 0x10) != 0 {
            writer.write_f32_le(self.velocity[0]);
        }
        if (hflags & 0x20) != 0 {
            writer.write_f32_le(self.velocity[1]);
        }
        if (hflags & 0x40) != 0 {
            writer.write_f32_le(self.velocity[2]);
        }
        if (hflags & 0x01) != 0 {
            writer.write_vle3(self.anim_state)?;
        }
        match &self.placement {
            EntityPlacement::WorldWithHeading {
                position,
                heading_raw,
            } => {
                position.encode_base(writer);
                writer.write_u8(*heading_raw);
            }
            EntityPlacement::WorldPositionOnly { position } => {
                position.encode_base(writer);
            }
            EntityPlacement::ParentAttachment {
                parent_id,
                attach_slot,
            } => {
                writer.write_u64_le(*parent_id);
                writer.write_vle3(*attach_slot)?;
            }
        }
        if (hflags & 0x02) != 0 {
            writer.write_u8(self.interp_param_a);
        }
        if (hflags & 0x04) != 0 {
            writer.write_u8(self.interp_param_b);
        }
        Ok(())
    }

    /// Decodes a `0xCE` inner event payload (§5.4).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let header_flags = reader.read_u8()?;
        let entity_id = reader.read_u64_le()?;
        let mode_flags = reader.read_u8()?;
        let seq_byte = if (header_flags & 0x08) != 0 {
            Some(reader.read_u8()?)
        } else {
            None
        };
        let vel_x = if (header_flags & 0x10) != 0 {
            require_finite("vel_x", reader.read_f32_le()?)?
        } else {
            0.0
        };
        let vel_y = if (header_flags & 0x20) != 0 {
            require_finite("vel_y", reader.read_f32_le()?)?
        } else {
            0.0
        };
        let vel_z = if (header_flags & 0x40) != 0 {
            require_finite("vel_z", reader.read_f32_le()?)?
        } else {
            0.0
        };
        let anim_state = if (header_flags & 0x01) != 0 {
            reader.read_vle3()?
        } else {
            0
        };
        let placement = if (mode_flags & 0x04) != 0 {
            if (mode_flags & 0x08) != 0 {
                return Err(WorldError::UnsupportedExtendedAttachment(mode_flags));
            }
            let parent_id = reader.read_u64_le()?;
            let attach_slot = reader.read_vle3()?;
            EntityPlacement::ParentAttachment {
                parent_id,
                attach_slot,
            }
        } else if (mode_flags & 0x10) != 0 {
            let position = decode_ce_world_position(reader)?;
            EntityPlacement::WorldPositionOnly { position }
        } else {
            let position = decode_ce_world_position(reader)?;
            let heading_raw = reader.read_u8()?;
            EntityPlacement::WorldWithHeading {
                position,
                heading_raw,
            }
        };
        let interp_param_a = if (header_flags & 0x02) != 0 {
            reader.read_u8()?
        } else {
            5
        };
        let interp_param_b = if (header_flags & 0x04) != 0 {
            reader.read_u8()?
        } else {
            2
        };
        Ok(Self {
            header_flags,
            entity_id,
            mode_flags,
            seq_byte,
            velocity: [vel_x, vel_y, vel_z],
            anim_state,
            placement,
            interp_param_a,
            interp_param_b,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Sub-action descriptor when bit 13 (`0x2000`) of [`ActionPayload::flags`] is set (§5.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSubEntry {
    /// Sub-action / skill identifier (4-tier VLE).
    pub sub_action_id: u32,
    /// Sub-action entry identifier (4-tier VLE).
    pub sub_entry_id: u32,
    /// Sub-action boolean flag (`bool8`).
    pub sub_entry_flag: bool,
}

/// Archive `ActionPayload` wire layout used inside `0x24` `slot_actions`, `0x75`,
/// `0x5A`, `0xA8`, and `0x63` (`docs/specs/official-messages-world.md` §5.6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct ActionPayload {
    /// Execution mode / priority code (`u8`, defaults to `10` = `0x0A`).
    pub action_mode: u8,
    /// Optional-field bitmask (`u16 LE`; bit 2 `0x0004` is always cleared on the wire).
    pub flags: u16,
    /// Bit 6 (`0x0040`): floating-point parameter (`f32 LE`, defaults to `0.0`).
    pub scalar_param: f32,
    /// Bit 13 (`0x2000`): optional sub-action entry.
    pub sub_entry: Option<ActionSubEntry>,
    /// Bit 0 (`0x0001`): primary target entity `InstanceID` (`u64 LE`, defaults to `0`).
    pub target_id_a: u64,
    /// Bit 14 (`0x4000`): secondary / item / source `InstanceID` (`u64 LE`, defaults to `0`).
    pub target_id_b: u64,
    /// Bit 1 (`0x0002`): optional per-action `EncodedPropertyTable`.
    pub action_props: Option<EncodedPropertyTable>,
    /// Bit 3 (`0x0008`): action state code (4-tier VLE, defaults to `3`).
    pub state_code: u32,
    /// Bit 4 (`0x0010`): animation speed / time-scale multiplier (`f32 LE`, defaults to `1.0`).
    pub speed_or_scale: f32,
    /// Unconditional action network timestamp in seconds (`f64 LE`).
    pub timestamp: f64,
    /// Bit 5 (`0x0020`): optional auxiliary index (4-tier VLE, defaults to `0xFFFF_FFFF`).
    pub aux_index: u32,
}

impl Default for ActionPayload {
    fn default() -> Self {
        Self {
            action_mode: 0x0A,
            flags: 0,
            scalar_param: 0.0,
            sub_entry: None,
            target_id_a: 0,
            target_id_b: 0,
            action_props: None,
            state_code: 3,
            speed_or_scale: 1.0,
            timestamp: 0.0,
            aux_index: 0xFFFF_FFFF,
        }
    }
}

impl ActionPayload {
    /// Encodes this `ActionPayload` in wire mode (§5.6.2).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_u8(self.action_mode);
        let mut flags = self.flags & !0x0004;
        if self.sub_entry.is_some() {
            flags |= 0x2000;
        }
        if self.action_props.is_some() {
            flags |= 0x0002;
        }
        writer.write_u16_le(flags);
        if (flags & 0x0040) != 0 {
            writer.write_f32_le(self.scalar_param);
        }
        if let Some(sub) = &self.sub_entry {
            writer.write_vle4(sub.sub_action_id);
            writer.write_vle4(sub.sub_entry_id);
            writer.write_bool8(sub.sub_entry_flag);
        }
        if (flags & 0x0001) != 0 {
            writer.write_u64_le(self.target_id_a);
        }
        if (flags & 0x4000) != 0 {
            writer.write_u64_le(self.target_id_b);
        }
        if let Some(props) = &self.action_props {
            props.encode(writer)?;
        }
        if (flags & 0x0008) != 0 {
            writer.write_vle4(self.state_code);
        }
        if (flags & 0x0010) != 0 {
            writer.write_f32_le(self.speed_or_scale);
        }
        writer.write_f64_le(self.timestamp);
        if (flags & 0x0020) != 0 {
            writer.write_vle4(self.aux_index);
        }
        Ok(())
    }

    /// Decodes an `ActionPayload` in wire mode using `schema` (§5.6.2).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        let action_mode = reader.read_u8()?;
        let flags = reader.read_u16_le()?;
        let scalar_param = if (flags & 0x0040) != 0 {
            reader.read_f32_le()?
        } else {
            0.0
        };
        let sub_entry = if (flags & 0x2000) != 0 {
            let sub_action_id = reader.read_vle4()?;
            let sub_entry_id = reader.read_vle4()?;
            let sub_entry_flag = reader.read_bool8()?;
            Some(ActionSubEntry {
                sub_action_id,
                sub_entry_id,
                sub_entry_flag,
            })
        } else {
            None
        };
        let target_id_a = if (flags & 0x0001) != 0 {
            reader.read_u64_le()?
        } else {
            0
        };
        let target_id_b = if (flags & 0x4000) != 0 {
            reader.read_u64_le()?
        } else {
            0
        };
        let action_props = if (flags & 0x0002) != 0 {
            Some(EncodedPropertyTable::decode(reader, schema)?)
        } else {
            None
        };
        let state_code = if (flags & 0x0008) != 0 {
            reader.read_vle4()?
        } else {
            3
        };
        let speed_or_scale = if (flags & 0x0010) != 0 {
            reader.read_f32_le()?
        } else {
            1.0
        };
        let timestamp = reader.read_f64_le()?;
        let aux_index = if (flags & 0x0020) != 0 {
            reader.read_vle4()?
        } else {
            0xFFFF_FFFF
        };
        Ok(Self {
            action_mode,
            flags,
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

/// Event `0x24` (`ClientMoveCommand`, C→S): 4-slot rolling movement/physics command
/// (`38` bytes base when no slot actions or properties are present; §5.5).
#[derive(Debug, Clone, PartialEq)]
pub struct ClientMoveCommand {
    /// Local controlled entity `InstanceID` (`u64 LE`).
    pub entity_id: u64,
    /// Movement packet flags byte (`u8`).
    pub header_byte: u8,
    /// Current movement tick sequence number (`u32 LE`, slot 3).
    pub sequence: u32,
    /// Movement state byte for each of the 4 history slots (`0..=3`).
    pub move_state: [u8; 4],
    /// Per-slot content bitmask (`bit 0 = queued actions`, `bit 4 = EncodedPropertyTable`).
    pub slot_flags: [u8; 4],
    /// Quantized movement/turn axis A for slots `0..=3`.
    pub axis_a: [u8; 4],
    /// Quantized movement/turn axis B for slots `0..=3`.
    pub axis_b: [u8; 4],
    /// Quantized movement/turn axis C for slots `0..=3`.
    pub axis_c: [u8; 4],
    /// Queued `(CompactDid, ActionPayload)` lists for slots `0..=3` (serialized for all 4 slots if any slot has bit `0x01` set).
    pub slot_actions: [Vec<(CompactDid, ActionPayload)>; 4],
    /// Optional per-slot `EncodedPropertyTable` for slots whose `slot_flags[i]` has bit `0x10` set.
    pub slot_props: [Option<EncodedPropertyTable>; 4],
    /// 32-bit movement trailer word (`u32 LE`).
    pub trailer_u32: u32,
}

impl ClientMoveCommand {
    /// Numeric event ID (`0x00000024`).
    pub const EVENT_ID: u32 = 0x0000_0024;
    /// Base wire byte length when no slot has actions (`0x01`) or properties (`0x10`).
    pub const BASE_BYTE_LEN: usize = 38;

    /// Constructs a base 38-byte `ClientMoveCommand` with no queued actions or slot property sets.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_base(
        entity_id: u64,
        header_byte: u8,
        sequence: u32,
        move_state: [u8; 4],
        axis_a: [u8; 4],
        axis_b: [u8; 4],
        axis_c: [u8; 4],
        trailer_u32: u32,
    ) -> Self {
        Self {
            entity_id,
            header_byte,
            sequence,
            move_state,
            slot_flags: [0; 4],
            axis_a,
            axis_b,
            axis_c,
            slot_actions: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            slot_props: [None, None, None, None],
            trailer_u32,
        }
    }

    /// Encodes the `0x24` inner event payload (§5.5).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        writer.write_u64_le(self.entity_id);
        writer.write_u8(self.header_byte);
        writer.write_u32_le(self.sequence);
        writer.write_bytes(&self.move_state);

        let mut flags = self.slot_flags;
        for (i, flag) in flags.iter_mut().enumerate() {
            if !self.slot_actions[i].is_empty() {
                *flag |= 0x01;
            }
            if self.slot_props[i].is_some() {
                *flag |= 0x10;
            }
        }
        writer.write_bytes(&flags);
        writer.write_bytes(&self.axis_a);
        writer.write_bytes(&self.axis_b);
        writer.write_bytes(&self.axis_c);

        let any_actions = flags.iter().any(|&f| (f & 0x01) != 0);
        if any_actions {
            for slot in &self.slot_actions {
                let count =
                    u8::try_from(slot.len()).map_err(|_| WorldError::Vle3Overflow(u32::MAX))?;
                writer.write_u8(count);
                for (action_id, payload) in slot {
                    action_id.encode(writer)?;
                    payload.encode(writer)?;
                }
            }
        }

        for (i, &flag) in flags.iter().enumerate() {
            if (flag & 0x10) != 0 {
                if let Some(props) = &self.slot_props[i] {
                    props.encode(writer)?;
                } else {
                    EncodedPropertyTable::new().encode(writer)?;
                }
            }
        }

        writer.write_u32_le(self.trailer_u32);
        Ok(())
    }

    /// Decodes a `0x24` inner event payload using `schema` (§5.5, §7.1).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let entity_id = reader.read_u64_le()?;
        let header_byte = reader.read_u8()?;
        let sequence = reader.read_u32_le()?;
        let ms = reader.read_bytes(4)?;
        let move_state = [ms[0], ms[1], ms[2], ms[3]];
        let sf = reader.read_bytes(4)?;
        let slot_flags = [sf[0], sf[1], sf[2], sf[3]];
        let aa = reader.read_bytes(4)?;
        let axis_a = [aa[0], aa[1], aa[2], aa[3]];
        let ab = reader.read_bytes(4)?;
        let axis_b = [ab[0], ab[1], ab[2], ab[3]];
        let ac = reader.read_bytes(4)?;
        let axis_c = [ac[0], ac[1], ac[2], ac[3]];

        let mut slot_actions: [Vec<(CompactDid, ActionPayload)>; 4] =
            [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let any_actions = slot_flags.iter().any(|&f| (f & 0x01) != 0);
        if any_actions {
            for slot in &mut slot_actions {
                let count = reader.read_u8()? as usize;
                // Each (CompactDid, ActionPayload) is at least 2 + 11 = 13 bytes.
                reader.ensure_count(count, 13)?;
                slot.reserve(count);
                for _ in 0..count {
                    let did = CompactDid::decode(reader)?;
                    let payload = ActionPayload::decode(reader, schema)?;
                    slot.push((did, payload));
                }
            }
        }

        let mut slot_props: [Option<EncodedPropertyTable>; 4] = [None, None, None, None];
        for (slot_prop, &flag) in slot_props.iter_mut().zip(slot_flags.iter()) {
            if (flag & 0x10) != 0 {
                *slot_prop = Some(EncodedPropertyTable::decode(reader, schema)?);
            }
        }

        let trailer_u32 = reader.read_u32_le()?;
        Ok(Self {
            entity_id,
            header_byte,
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

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::with_capacity(Self::BASE_BYTE_LEN);
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x89` (`EntityAppearanceDescUpdate`, S→C): entity visual appearance and equipment descriptor update (§5.2.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EntityAppearanceDescUpdate {
    /// Complete [`EntityAppearanceDesc`] for the target entity (§3.1.2).
    pub appearance_desc: EntityAppearanceDesc,
}

impl EntityAppearanceDescUpdate {
    /// Numeric event ID (`0x00000089`).
    pub const EVENT_ID: u32 = 0x0000_0089;

    /// Encodes the inner event payload.
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.appearance_desc.encode(writer)
    }

    /// Decodes the inner event payload using `schema`.
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let appearance_desc = EntityAppearanceDesc::decode(reader, schema)?;
        Ok(Self { appearance_desc })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x75` (`ClientActionCommand`, C→S): discrete gameplay action request (§5.6.1).
#[derive(Debug, Clone, PartialEq)]
pub struct ClientActionCommand {
    /// Action / command identifier (`CompactDid`, §2.4.2).
    pub action_id: CompactDid,
    /// Secondary action context / category identifier (`CompactDid`, §2.4.2).
    pub context_did: CompactDid,
    /// Shared [`ActionPayload`] (§5.6.2).
    pub action_payload: ActionPayload,
}

impl ClientActionCommand {
    /// Numeric event ID (`0x00000075`).
    pub const EVENT_ID: u32 = 0x0000_0075;

    /// Encodes the inner event payload (§5.6.1).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        self.context_did.encode(writer)?;
        self.action_payload.encode(writer)
    }

    /// Decodes a `0x75` inner event payload using `schema` (§5.6.1).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let action_id = CompactDid::decode(reader)?;
        let context_did = CompactDid::decode(reader)?;
        let action_payload = ActionPayload::decode(reader, schema)?;
        Ok(Self {
            action_id,
            context_did,
            action_payload,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Server-to-client entity action replication event (`0x5A` `EntityActionBegin`,
/// `0xA8` `EntityActionUpdate`, `0x63` `EntityActionCancel`; §5.6.3).
#[derive(Debug, Clone, PartialEq)]
pub struct EntityActionEvent {
    /// Replicated action identifier (`CompactDid`, §2.4.2).
    pub action_id: CompactDid,
    /// Shared [`ActionPayload`] (§5.6.2).
    pub action_payload: ActionPayload,
}

impl EntityActionEvent {
    /// Event ID for `EntityActionBegin` (`0x0000005A`).
    pub const EVENT_ID_BEGIN: u32 = 0x0000_005A;
    /// Event ID for `EntityActionUpdate` (`0x000000A8`).
    pub const EVENT_ID_UPDATE: u32 = 0x0000_00A8;
    /// Event ID for `EntityActionCancel` (`0x00000063`).
    pub const EVENT_ID_CANCEL: u32 = 0x0000_0063;

    /// Encodes the inner event payload (§5.6.3).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        self.action_payload.encode(writer)
    }

    /// Decodes the inner event payload using `schema` (§5.6.3).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let action_id = CompactDid::decode(reader)?;
        let action_payload = ActionPayload::decode(reader, schema)?;
        Ok(Self {
            action_id,
            action_payload,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x22` (`ActionErrorResponse`, S→C): action rejection or error notification (§5.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionErrorResponse {
    /// Rejected action identifier (`CompactDid`, §2.4.2).
    pub action_id: CompactDid,
    /// Action error reason identifier (`CompactDid`, §2.4.2).
    pub error_did: CompactDid,
    /// Associated target entity `InstanceID` (`u64 LE`).
    pub target_id: u64,
}

impl ActionErrorResponse {
    /// Numeric event ID (`0x00000022`).
    pub const EVENT_ID: u32 = 0x0000_0022;

    /// Encodes the inner event payload (§5.6.3).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        self.error_did.encode(writer)?;
        writer.write_u64_le(self.target_id);
        Ok(())
    }

    /// Decodes the inner event payload (§5.6.3).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let action_id = CompactDid::decode(reader)?;
        let error_did = CompactDid::decode(reader)?;
        let target_id = reader.read_u64_le()?;
        Ok(Self {
            action_id,
            error_did,
            target_id,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0x16C` (`ActionStatusResponse`, S→C): action status notification (§5.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionStatusResponse {
    /// Action identifier (`CompactDid`, §2.4.2).
    pub action_id: CompactDid,
    /// Status identifier (`CompactDid`, §2.4.2).
    pub status_did: CompactDid,
}

impl ActionStatusResponse {
    /// Numeric event ID (`0x0000016C`).
    pub const EVENT_ID: u32 = 0x0000_016C;

    /// Encodes the inner event payload (§5.6.3).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.action_id.encode(writer)?;
        self.status_did.encode(writer)
    }

    /// Decodes the inner event payload (§5.6.3).
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let action_id = CompactDid::decode(reader)?;
        let status_did = CompactDid::decode(reader)?;
        Ok(Self {
            action_id,
            status_did,
        })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Incremental single-property update event (`0x88` `PropertySetValue`,
/// `0x92` `PropertyAdjustFloat`, `0xB0` `PropertyAdjustInt`,
/// `0x147` `PropertyInsertElement`, `0x149` `PropertyRemoveElement`, S→C; §5.7).
#[derive(Debug, Clone, PartialEq)]
pub struct SinglePropertyUpdate {
    /// Single `(CompactPid, EncodedPropertyValue)` entry (§2.7.2).
    pub entry: EncodedPropertyEntry,
}

impl SinglePropertyUpdate {
    /// Event ID for `PropertySetValue` (`0x00000088`).
    pub const EVENT_ID_SET_VALUE: u32 = 0x0000_0088;
    /// Event ID for `PropertyAdjustFloat` (`0x00000092`).
    pub const EVENT_ID_ADJUST_FLOAT: u32 = 0x0000_0092;
    /// Event ID for `PropertyAdjustInt` (`0x000000B0`).
    pub const EVENT_ID_ADJUST_INT: u32 = 0x0000_00B0;
    /// Event ID for `PropertyInsertElement` (`0x00000147`).
    pub const EVENT_ID_INSERT_ELEMENT: u32 = 0x0000_0147;
    /// Event ID for `PropertyRemoveElement` (`0x00000149`).
    pub const EVENT_ID_REMOVE_ELEMENT: u32 = 0x0000_0149;

    /// Creates a `SinglePropertyUpdate` for `(pid, value)`.
    #[must_use]
    pub fn new(pid: u32, value: EncodedPropertyValue) -> Self {
        Self {
            entry: EncodedPropertyEntry::new(pid, value),
        }
    }

    /// Encodes the inner event payload (§5.7).
    pub fn encode(&self, writer: &mut WireWriter) -> Result<(), WorldError> {
        writer.write_base_flags();
        self.entry.encode(writer)
    }

    /// Decodes the inner event payload using `schema` (§5.7).
    pub fn decode(
        reader: &mut WireReader<'_>,
        schema: &dyn PropertySchemaLookup,
    ) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let entry = EncodedPropertyEntry::decode(reader, schema)?;
        Ok(Self { entry })
    }

    /// Encodes to a standalone byte vector.
    pub fn to_bytes(&self) -> Result<Vec<u8>, WorldError> {
        let mut w = WireWriter::new();
        self.encode(&mut w)?;
        Ok(w.into_vec())
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8], schema: &dyn PropertySchemaLookup) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r, schema)?;
        r.finish()?;
        Ok(msg)
    }
}

/// Event `0xB7` (`PropertyRemove`, S→C): removes a property from the target entity (§5.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyRemove {
    /// Removed property ID (`CompactPid`, §2.4.3).
    pub pid: CompactPid,
}

impl PropertyRemove {
    /// Numeric event ID (`0x000000B7`).
    pub const EVENT_ID: u32 = 0x0000_00B7;

    /// Encodes the inner event payload (`u8 0x00` + `CompactPid`).
    pub fn encode(&self, writer: &mut WireWriter) {
        writer.write_base_flags();
        self.pid.encode(writer);
    }

    /// Decodes the inner event payload.
    pub fn decode(reader: &mut WireReader<'_>) -> Result<Self, WorldError> {
        reader.read_base_flags()?;
        let pid = CompactPid::decode(reader)?;
        Ok(Self { pid })
    }

    /// Encodes to a standalone byte vector.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(6);
        self.encode(&mut w);
        w.into_vec()
    }

    /// Decodes from a standalone byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WorldError> {
        let mut r = WireReader::new(bytes);
        let msg = Self::decode(&mut r)?;
        r.finish()?;
        Ok(msg)
    }
}
