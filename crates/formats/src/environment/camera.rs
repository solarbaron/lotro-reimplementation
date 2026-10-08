// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera-state records (prefix 0x08, DB type 0x0D CAMERASTATE; `walker.md` §11.5).
//!
//! A camera state is a 24-byte header (camera entity, smoothing, clip planes, four flags,
//! controller type) followed by a body whose layout depends on the controller type
//! (§11.5.3). The record has **no** own-DID field (`records.md` §1.6, prologue "–"), so
//! [`CameraState`] does not implement [`Record`](crate::Record): the caller knows the DID it
//! looked up, and [`CameraState::parse`] cannot check it.
//!
//! The flag bytes are "u8 bool" in the spec; the decoder rejects values other than 0 and 1
//! like every other flag of this crate.

use super::put;
use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::reader::Reader;

/// Type prefix of camera-state records.
pub const CAMERA_STATE_PREFIX: u8 = 0x08;

/// Stored size of the header (`walker.md` §11.5.1).
pub const CAMERA_HEADER_LEN: usize = 24;

/// Stored size of the target block (`walker.md` §11.5.2).
pub const TARGET_BLOCK_LEN: usize = 44;

/// Controller type 1, third person.
pub const CONTROLLER_THIRD_PERSON: u32 = 1;
/// Controller type 2, first person.
pub const CONTROLLER_FIRST_PERSON: u32 = 2;
/// Controller type 3, cinematic.
pub const CONTROLLER_CINEMATIC: u32 = 3;
/// Controller type 4, flight.
pub const CONTROLLER_FLIGHT: u32 = 4;

/// The target block inside bodies of types 1, 2 and 4 (`walker.md` §11.5.2, 44 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TargetBlock {
    /// Offset 0: a field whose kind was not resolved (zero in every observed record).
    pub unresolved: [u8; 8],
    /// Holding location of the target to aim at (0 = none).
    pub holding_location: u32,
    /// Bone of the target to aim at (0 = none).
    pub bone: u32,
    /// Offset from the target point (x, y, z), metres.
    pub offset: [f32; 3],
    /// Yaw offset, degrees.
    pub yaw_offset: f32,
    /// Pitch offset, degrees.
    pub pitch_offset: f32,
    /// Height factor *f* (`walker.md` §11.7 step 1).
    pub height_factor: f32,
    /// Height factor under water.
    pub height_factor_under_water: f32,
}

impl TargetBlock {
    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            unresolved: r.array::<8>()?,
            holding_location: r.u32()?,
            bone: r.u32()?,
            offset: [r.f32()?, r.f32()?, r.f32()?],
            yaw_offset: r.f32()?,
            pitch_offset: r.f32()?,
            height_factor: r.f32()?,
            height_factor_under_water: r.f32()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.unresolved);
        put::u32(out, self.holding_location);
        put::u32(out, self.bone);
        put::f32s(out, &self.offset);
        put::f32s(
            out,
            &[
                self.yaw_offset,
                self.pitch_offset,
                self.height_factor,
                self.height_factor_under_water,
            ],
        );
    }
}

/// Body of controller type 1, third person (`walker.md` §11.5.3, 137 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ThirdPersonBody {
    /// Angle smoothing time, seconds.
    pub angle_smoothing: f32,
    /// Zoom smoothing time, seconds.
    pub zoom_smoothing: f32,
    /// Target-position smoothing time, seconds.
    pub target_smoothing: f32,
    /// Two values without a name in the client's field list (OPEN).
    pub unnamed: [f32; 2],
    /// Target block.
    pub target: TargetBlock,
    /// Constrain pitch.
    pub constrain_pitch: bool,
    /// Pitch limit, high, degrees.
    pub pitch_high: f32,
    /// Pitch limit, low, degrees.
    pub pitch_low: f32,
    /// Constrain yaw.
    pub constrain_yaw: bool,
    /// Offset range (use OPEN).
    pub offset_range: [f32; 3],
    /// Yaw limit, high, degrees.
    pub yaw_high: f32,
    /// Yaw limit, low, degrees.
    pub yaw_low: f32,
    /// Mouse-look rotation relative to the target.
    pub mouse_look_relative: bool,
    /// Detached mouse.
    pub detached_mouse: bool,
    /// Minimum distance, metres.
    pub min_distance: f32,
    /// Maximum distance, metres.
    pub max_distance: f32,
    /// Field of view, degrees.
    pub fov: f32,
    /// Initial pitch, degrees.
    pub initial_pitch: f32,
    /// Initial yaw, degrees.
    pub initial_yaw: f32,
    /// Initial distance, metres.
    pub initial_distance: f32,
    /// Supports aligning to the slope.
    pub align_to_slope: bool,
    /// Minimum target height, metres (use OPEN).
    pub min_target_height: f32,
    /// Target fade distance, metres.
    pub target_fade_distance: f32,
    /// Target fade opacity, 0–1.
    pub target_fade_opacity: f32,
    /// Look-at offset smoothing time, seconds.
    pub look_at_smoothing: f32,
}

/// Body of controller type 2, first person (`walker.md` §11.5.3, 65 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FirstPersonBody {
    /// Field of view, degrees.
    pub fov: f32,
    /// Constrain pitch.
    pub constrain_pitch: bool,
    /// Pitch limit, high, degrees.
    pub pitch_high: f32,
    /// Pitch limit, low, degrees.
    pub pitch_low: f32,
    /// Target block.
    pub target: TargetBlock,
    /// Initial yaw, degrees.
    pub initial_yaw: f32,
    /// Look-from offset smoothing time, seconds.
    pub look_from_smoothing: f32,
}

/// Body of controller type 3, cinematic (`walker.md` §11.5.3, 12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CinematicBody {
    /// Field of view, degrees.
    pub fov: f32,
    /// Target smoothing time, seconds.
    pub target_smoothing: f32,
    /// Angle smoothing time, seconds.
    pub angle_smoothing: f32,
}

/// Body of controller type 4, flight (`walker.md` §11.5.3, 70 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FlightBody {
    /// Field of view, degrees.
    pub fov: f32,
    /// Initial yaw, degrees.
    pub initial_yaw: f32,
    /// Constrain pitch.
    pub constrain_pitch: bool,
    /// Pitch limit, high, degrees.
    pub pitch_high: f32,
    /// Pitch limit, low, degrees.
    pub pitch_low: f32,
    /// Constrain roll.
    pub constrain_roll: bool,
    /// Roll limit, left, degrees.
    pub roll_left: f32,
    /// Roll limit, right, degrees.
    pub roll_right: f32,
    /// Target block.
    pub target: TargetBlock,
}

/// The controller body, by controller type (`walker.md` §11.5.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraBody {
    /// Type 1.
    ThirdPerson(ThirdPersonBody),
    /// Type 2.
    FirstPerson(FirstPersonBody),
    /// Type 3.
    Cinematic(CinematicBody),
    /// Type 4.
    Flight(FlightBody),
}

impl CameraBody {
    /// The stored controller type (mapper 0x230000d4 values 1–4).
    #[must_use]
    pub fn controller_type(&self) -> u32 {
        match self {
            Self::ThirdPerson(_) => CONTROLLER_THIRD_PERSON,
            Self::FirstPerson(_) => CONTROLLER_FIRST_PERSON,
            Self::Cinematic(_) => CONTROLLER_CINEMATIC,
            Self::Flight(_) => CONTROLLER_FLIGHT,
        }
    }

    /// The field of view, degrees (every body type has one).
    #[must_use]
    pub fn fov(&self) -> f32 {
        match self {
            Self::ThirdPerson(b) => b.fov,
            Self::FirstPerson(b) => b.fov,
            Self::Cinematic(b) => b.fov,
            Self::Flight(b) => b.fov,
        }
    }

    /// The target block (types 1, 2 and 4).
    #[must_use]
    pub fn target(&self) -> Option<&TargetBlock> {
        match self {
            Self::ThirdPerson(b) => Some(&b.target),
            Self::FirstPerson(b) => Some(&b.target),
            Self::Cinematic(_) => None,
            Self::Flight(b) => Some(&b.target),
        }
    }

    /// Stored size of the body in bytes (137, 65, 12, 70).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        match self {
            Self::ThirdPerson(_) => 137,
            Self::FirstPerson(_) => 65,
            Self::Cinematic(_) => 12,
            Self::Flight(_) => 70,
        }
    }
}

/// A camera-state record (prefix 0x08; `walker.md` §11.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraState {
    /// The camera entity (a 0x47 entity descriptor in the observed install).
    pub entity: Did,
    /// Collision pull-in smoothing time, seconds (Inferred).
    pub collision_smoothing: f32,
    /// Near clip distance, metres.
    pub near_clip: f32,
    /// Far clip distance, metres.
    pub far_clip: f32,
    /// The camera uses physics (collides).
    pub uses_physics: bool,
    /// The camera uses the water surface physics.
    pub uses_water_physics: bool,
    /// The camera follows the player under water.
    pub follows_under_water: bool,
    /// Automatic transitions disabled (Inferred).
    pub auto_transitions_disabled: bool,
    /// The controller body; its variant is the controller type.
    pub body: CameraBody,
}

impl CameraState {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "camera state";

    /// Decodes a complete camera-state payload. There is no own DID to check; a controller
    /// type outside 1–4 fails with [`FormatError::Unsupported`] (its body length is
    /// unknown), a flag byte other than 0 or 1 with [`FormatError::InvalidBool`].
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let entity = r.did()?;
        let collision_smoothing = r.f32()?;
        let near_clip = r.f32()?;
        let far_clip = r.f32()?;
        let uses_physics = r.bool8()?;
        let uses_water_physics = r.bool8()?;
        let follows_under_water = r.bool8()?;
        let auto_transitions_disabled = r.bool8()?;
        let type_at = r.position();
        let body = match r.u32()? {
            CONTROLLER_THIRD_PERSON => CameraBody::ThirdPerson(read_third_person(&mut r)?),
            CONTROLLER_FIRST_PERSON => CameraBody::FirstPerson(FirstPersonBody {
                fov: r.f32()?,
                constrain_pitch: r.bool8()?,
                pitch_high: r.f32()?,
                pitch_low: r.f32()?,
                target: TargetBlock::read(&mut r)?,
                initial_yaw: r.f32()?,
                look_from_smoothing: r.f32()?,
            }),
            CONTROLLER_CINEMATIC => CameraBody::Cinematic(CinematicBody {
                fov: r.f32()?,
                target_smoothing: r.f32()?,
                angle_smoothing: r.f32()?,
            }),
            CONTROLLER_FLIGHT => CameraBody::Flight(FlightBody {
                fov: r.f32()?,
                initial_yaw: r.f32()?,
                constrain_pitch: r.bool8()?,
                pitch_high: r.f32()?,
                pitch_low: r.f32()?,
                constrain_roll: r.bool8()?,
                roll_left: r.f32()?,
                roll_right: r.f32()?,
                target: TargetBlock::read(&mut r)?,
            }),
            value => {
                return Err(FormatError::Unsupported {
                    what: "camera controller type",
                    offset: type_at,
                    value,
                });
            }
        };
        r.expect_end(Self::NAME)?;
        Ok(Self {
            entity,
            collision_smoothing,
            near_clip,
            far_clip,
            uses_physics,
            uses_water_physics,
            follows_under_water,
            auto_transitions_disabled,
            body,
        })
    }

    /// Stored size of the record in bytes (161, 89, 36 or 94).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        CAMERA_HEADER_LEN + self.body.encoded_len()
    }

    /// Re-encodes the record byte for byte (`walker.md` §11.5).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.encoded_len());
        put::did(&mut out, self.entity);
        put::f32s(
            &mut out,
            &[self.collision_smoothing, self.near_clip, self.far_clip],
        );
        put::bool8(&mut out, self.uses_physics);
        put::bool8(&mut out, self.uses_water_physics);
        put::bool8(&mut out, self.follows_under_water);
        put::bool8(&mut out, self.auto_transitions_disabled);
        put::u32(&mut out, self.body.controller_type());
        match &self.body {
            CameraBody::ThirdPerson(b) => write_third_person(&mut out, b),
            CameraBody::FirstPerson(b) => {
                put::f32(&mut out, b.fov);
                put::bool8(&mut out, b.constrain_pitch);
                put::f32s(&mut out, &[b.pitch_high, b.pitch_low]);
                b.target.write(&mut out);
                put::f32s(&mut out, &[b.initial_yaw, b.look_from_smoothing]);
            }
            CameraBody::Cinematic(b) => {
                put::f32s(&mut out, &[b.fov, b.target_smoothing, b.angle_smoothing]);
            }
            CameraBody::Flight(b) => {
                put::f32s(&mut out, &[b.fov, b.initial_yaw]);
                put::bool8(&mut out, b.constrain_pitch);
                put::f32s(&mut out, &[b.pitch_high, b.pitch_low]);
                put::bool8(&mut out, b.constrain_roll);
                put::f32s(&mut out, &[b.roll_left, b.roll_right]);
                b.target.write(&mut out);
            }
        }
        out
    }
}

fn read_third_person(r: &mut Reader<'_>) -> Result<ThirdPersonBody> {
    Ok(ThirdPersonBody {
        angle_smoothing: r.f32()?,
        zoom_smoothing: r.f32()?,
        target_smoothing: r.f32()?,
        unnamed: [r.f32()?, r.f32()?],
        target: TargetBlock::read(r)?,
        constrain_pitch: r.bool8()?,
        pitch_high: r.f32()?,
        pitch_low: r.f32()?,
        constrain_yaw: r.bool8()?,
        offset_range: [r.f32()?, r.f32()?, r.f32()?],
        yaw_high: r.f32()?,
        yaw_low: r.f32()?,
        mouse_look_relative: r.bool8()?,
        detached_mouse: r.bool8()?,
        min_distance: r.f32()?,
        max_distance: r.f32()?,
        fov: r.f32()?,
        initial_pitch: r.f32()?,
        initial_yaw: r.f32()?,
        initial_distance: r.f32()?,
        align_to_slope: r.bool8()?,
        min_target_height: r.f32()?,
        target_fade_distance: r.f32()?,
        target_fade_opacity: r.f32()?,
        look_at_smoothing: r.f32()?,
    })
}

fn write_third_person(out: &mut Vec<u8>, b: &ThirdPersonBody) {
    put::f32s(
        out,
        &[
            b.angle_smoothing,
            b.zoom_smoothing,
            b.target_smoothing,
            b.unnamed[0],
            b.unnamed[1],
        ],
    );
    b.target.write(out);
    put::bool8(out, b.constrain_pitch);
    put::f32s(out, &[b.pitch_high, b.pitch_low]);
    put::bool8(out, b.constrain_yaw);
    put::f32s(out, &b.offset_range);
    put::f32s(out, &[b.yaw_high, b.yaw_low]);
    put::bool8(out, b.mouse_look_relative);
    put::bool8(out, b.detached_mouse);
    put::f32s(
        out,
        &[
            b.min_distance,
            b.max_distance,
            b.fov,
            b.initial_pitch,
            b.initial_yaw,
            b.initial_distance,
        ],
    );
    put::bool8(out, b.align_to_slope);
    put::f32s(
        out,
        &[
            b.min_target_height,
            b.target_fade_distance,
            b.target_fade_opacity,
            b.look_at_smoothing,
        ],
    );
}

/// Decodes a camera-state payload (for fuzz targets and tools).
pub fn decode_camera_state(data: &[u8]) -> Result<CameraState> {
    CameraState::parse(data)
}
