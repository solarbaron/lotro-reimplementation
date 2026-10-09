// SPDX-License-Identifier: GPL-3.0-or-later
//! Media items of UI layout state blocks (`ui-layout.md` §4.6, §6).

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::reader::{PREALLOC_CAP, Reader};

/// Smallest encoding of a media item: kind tag, kind and a 4-byte body (§4.7).
pub const MIN_MEDIA_ITEM_LEN: usize = 12;

/// Media kind tag (§6.1). Kind 2 and kinds above 13 are not created by the client; a decoder
/// rejects them (§4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum MediaKind {
    /// 1: play a video.
    Movie = 1,
    /// 3: continue the media list at another index (unused in data).
    Jump = 3,
    /// 4: set the element's base image.
    Image = 4,
    /// 5: send a UI message to the element's listeners.
    Message = 5,
    /// 6: animate the element's surface opacity.
    Fade = 6,
    /// 7: set a hit-test mask image (unused in data).
    HitMask = 7,
    /// 8: flip-book animation of the base image.
    Animation = 8,
    /// 9: set the element's overlay layer (image and/or colour).
    ColouredImage = 9,
    /// 10: set the mouse cursor over the element.
    Cursor = 10,
    /// 11: play a UI sound.
    Sound = 11,
    /// 12: switch the element to a state.
    State = 12,
    /// 13: wait a random time.
    Pause = 13,
}

impl MediaKind {
    /// Every kind the client creates, in code order.
    pub const ALL: [Self; 12] = [
        Self::Movie,
        Self::Jump,
        Self::Image,
        Self::Message,
        Self::Fade,
        Self::HitMask,
        Self::Animation,
        Self::ColouredImage,
        Self::Cursor,
        Self::Sound,
        Self::State,
        Self::Pause,
    ];

    /// The kind for a stored tag, or `None` for 0, 2 and values above 13.
    #[must_use]
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            1 => Self::Movie,
            3 => Self::Jump,
            4 => Self::Image,
            5 => Self::Message,
            6 => Self::Fade,
            7 => Self::HitMask,
            8 => Self::Animation,
            9 => Self::ColouredImage,
            10 => Self::Cursor,
            11 => Self::Sound,
            12 => Self::State,
            13 => Self::Pause,
            _ => return None,
        })
    }

    /// The stored tag.
    #[must_use]
    pub fn code(self) -> u32 {
        self as u32
    }

    /// Short name (this crate's own, §6.1).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Movie => "Movie",
            Self::Jump => "Jump",
            Self::Image => "Image",
            Self::Message => "Message",
            Self::Fade => "Fade",
            Self::HitMask => "HitMask",
            Self::Animation => "Animation",
            Self::ColouredImage => "ColouredImage",
            Self::Cursor => "Cursor",
            Self::Sound => "Sound",
            Self::State => "State",
            Self::Pause => "Pause",
        }
    }
}

/// A draw-mode value (`DrawModeType` enum, §3.3), kept raw: the client maps 0 and values
/// above 8 to the same operation as Normal (§9.6.1, UL17), which `tessera-ui` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct DrawMode(pub u32);

impl DrawMode {
    /// 0 `Undef`.
    pub const UNDEF: Self = Self(0);
    /// 1 `Colorize`.
    pub const COLORIZE: Self = Self(1);
    /// 2 `Normal`.
    pub const NORMAL: Self = Self(2);
    /// 3 `Multiply`.
    pub const MULTIPLY: Self = Self(3);
    /// 4 `Alphablend`.
    pub const ALPHABLEND: Self = Self(4);
    /// 5 `Overlay`.
    pub const OVERLAY: Self = Self(5);
    /// 6 `Grayscale`.
    pub const GRAYSCALE: Self = Self(6);
    /// 7 `Screen`.
    pub const SCREEN: Self = Self(7);
    /// 8 `None`: an Image or Coloured-image item with this mode clears its layer (§6.2, §6.3).
    pub const NONE: Self = Self(8);

    /// The label of the value in this install's `DrawModeType` mapper (§3.3), or `None`
    /// outside 0–8.
    #[must_use]
    pub fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            0 => "Undef",
            1 => "Colorize",
            2 => "Normal",
            3 => "Multiply",
            4 => "Alphablend",
            5 => "Overlay",
            6 => "Grayscale",
            7 => "Screen",
            8 => "None",
            _ => return None,
        })
    }

    /// Whether this is the "clear the layer" mode 8.
    #[must_use]
    pub fn is_none(self) -> bool {
        self == Self::NONE
    }
}

/// A colour stored as one u32 0xAARRGGBB (bytes B, G, R, A in the record; §6.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Argb(pub u32);

impl Argb {
    /// Alpha channel.
    #[must_use]
    pub fn a(self) -> u8 {
        (self.0 >> 24) as u8
    }
    /// Red channel.
    #[must_use]
    pub fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }
    /// Green channel.
    #[must_use]
    pub fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }
    /// Blue channel.
    #[must_use]
    pub fn b(self) -> u8 {
        self.0 as u8
    }
}

/// One media item (§6.1), in the order the record stores its body fields.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaItem {
    /// Kind 1: play a video named by DID or by a file path relative to the install.
    Movie {
        /// Movie DID (DB-object type 0x08); 0 when the path names the video.
        did: Did,
        /// File path (Latin-1, `/` separators), possibly empty.
        path: String,
        /// Scale the video to the screen, keeping its aspect ratio.
        fit_to_screen: bool,
    },
    /// Kind 3: continue the list at `target` (unused in data; arithmetic OPEN, §6.8).
    Jump {
        /// Target index.
        target: u32,
        /// Chance of acting, 0–1.
        probability: f32,
    },
    /// Kind 4: set the element's base image.
    Image {
        /// Render surface DID (0x41), or 0.
        did: Did,
        /// Draw mode; [`DrawMode::NONE`] removes the base image.
        draw_mode: DrawMode,
        /// File path of a JPEG image when the DID is 0 (usually empty).
        path: String,
    },
    /// Kind 5: send a UI message (`UIElementMessage`).
    Message {
        /// Message ID.
        message: u32,
        /// Chance of acting, 0–1.
        probability: f32,
    },
    /// Kind 6: animate `UICore_Element_ObjectOpacity`.
    Fade {
        /// Start opacity.
        start: f32,
        /// End opacity.
        end: f32,
        /// Duration in seconds.
        duration: f32,
    },
    /// Kind 7: set a hit-test mask image (unused in data).
    HitMask {
        /// Mask image DID; 0 removes the mask.
        did: Did,
    },
    /// Kind 8: flip-book animation.
    Animation {
        /// Duration in seconds.
        duration: f32,
        /// Draw mode of each frame.
        draw_mode: DrawMode,
        /// Restart after the last frame.
        looping: bool,
        /// Frame surfaces (0x41), in order.
        frames: Vec<Did>,
    },
    /// Kind 9: set the overlay layer.
    ColouredImage {
        /// Image DID (0x41), or 0 for a plain colour fill.
        did: Did,
        /// Draw mode; [`DrawMode::NONE`] removes the overlay.
        draw_mode: DrawMode,
        /// Fill or tint colour.
        colour: Argb,
    },
    /// Kind 10: cursor shown over the element.
    Cursor {
        /// Cursor image DID (0x41); 0 restores the default cursor.
        did: Did,
        /// Hot spot x in pixels within the image.
        hotspot_x: u32,
        /// Hot spot y in pixels within the image.
        hotspot_y: u32,
    },
    /// Kind 11: play a UI sound.
    Sound {
        /// Sound-info DID (0x2A).
        did: Did,
    },
    /// Kind 12: switch the element to a state.
    State {
        /// State ID (`UIStateID`); 0 = back to no state.
        state: u32,
        /// Chance of acting, 0–1.
        probability: f32,
    },
    /// Kind 13: wait a random time between `min` and `max` seconds.
    Pause {
        /// Minimum seconds.
        min: f32,
        /// Maximum seconds (may be below `min` in data).
        max: f32,
    },
}

impl MediaItem {
    /// The item's kind.
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        match self {
            Self::Movie { .. } => MediaKind::Movie,
            Self::Jump { .. } => MediaKind::Jump,
            Self::Image { .. } => MediaKind::Image,
            Self::Message { .. } => MediaKind::Message,
            Self::Fade { .. } => MediaKind::Fade,
            Self::HitMask { .. } => MediaKind::HitMask,
            Self::Animation { .. } => MediaKind::Animation,
            Self::ColouredImage { .. } => MediaKind::ColouredImage,
            Self::Cursor { .. } => MediaKind::Cursor,
            Self::Sound { .. } => MediaKind::Sound,
            Self::State { .. } => MediaKind::State,
            Self::Pause { .. } => MediaKind::Pause,
        }
    }

    /// The draw mode of an Image, Animation or Coloured-image item.
    #[must_use]
    pub fn draw_mode(&self) -> Option<DrawMode> {
        match self {
            Self::Image { draw_mode, .. }
            | Self::Animation { draw_mode, .. }
            | Self::ColouredImage { draw_mode, .. } => Some(*draw_mode),
            _ => None,
        }
    }

    /// The file path of a Movie or Image item.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Movie { path, .. } | Self::Image { path, .. } => Some(path),
            _ => None,
        }
    }

    /// Calls `f` with every DID field of the item, zero DIDs included (one call per field,
    /// one per animation frame; §10 counting rule).
    pub fn for_each_did(&self, mut f: impl FnMut(Did)) {
        match self {
            Self::Movie { did, .. }
            | Self::Image { did, .. }
            | Self::HitMask { did }
            | Self::ColouredImage { did, .. }
            | Self::Cursor { did, .. }
            | Self::Sound { did } => f(*did),
            Self::Animation { frames, .. } => frames.iter().copied().for_each(f),
            Self::Jump { .. }
            | Self::Message { .. }
            | Self::Fade { .. }
            | Self::State { .. }
            | Self::Pause { .. } => {}
        }
    }

    /// Reads one item: the kind tag, the repeated kind and the body (§4.6, §6.1).
    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self> {
        let tag_at = r.position();
        let tag = r.u32()?;
        let Some(kind) = MediaKind::from_code(tag) else {
            return Err(FormatError::Unsupported {
                what: UNKNOWN_MEDIA_KIND,
                offset: tag_at,
                value: tag,
            });
        };
        let kind_at = r.position();
        let again = r.u32()?;
        if again != tag {
            // §12.1: a key/ID mismatch (here UL4) is reported as malformed.
            return Err(FormatError::malformed(
                "UI media item",
                kind_at,
                format!("repeated kind {again} differs from kind tag {tag} (UL4)"),
            ));
        }
        Ok(match kind {
            MediaKind::Movie => Self::Movie {
                did: r.did()?,
                path: r.pascal_string()?,
                fit_to_screen: r.bool8()?,
            },
            MediaKind::Jump => Self::Jump {
                target: r.u32()?,
                probability: r.f32()?,
            },
            MediaKind::Image => Self::Image {
                did: r.did()?,
                draw_mode: DrawMode(r.u32()?),
                path: r.pascal_string()?,
            },
            MediaKind::Message => Self::Message {
                message: r.u32()?,
                probability: r.f32()?,
            },
            MediaKind::Fade => Self::Fade {
                start: r.f32()?,
                end: r.f32()?,
                duration: r.f32()?,
            },
            MediaKind::HitMask => Self::HitMask { did: r.did()? },
            MediaKind::Animation => {
                let duration = r.f32()?;
                let draw_mode = DrawMode(r.u32()?);
                let looping = r.bool8()?;
                let count = r.u32()?;
                let mut frames = r.alloc_vec::<Did>(count, 4, PREALLOC_CAP)?;
                for _ in 0..count {
                    let d = r.did()?;
                    r.push(&mut frames, d, count)?;
                }
                Self::Animation {
                    duration,
                    draw_mode,
                    looping,
                    frames,
                }
            }
            MediaKind::ColouredImage => Self::ColouredImage {
                did: r.did()?,
                draw_mode: DrawMode(r.u32()?),
                colour: Argb(r.u32()?),
            },
            MediaKind::Cursor => Self::Cursor {
                did: r.did()?,
                hotspot_x: r.u32()?,
                hotspot_y: r.u32()?,
            },
            MediaKind::Sound => Self::Sound { did: r.did()? },
            MediaKind::State => Self::State {
                state: r.u32()?,
                probability: r.f32()?,
            },
            MediaKind::Pause => Self::Pause {
                min: r.f32()?,
                max: r.f32()?,
            },
        })
    }
}

/// The `what` of the [`FormatError::Unsupported`] error for an unknown media kind (§13
/// category "Unknown media kind").
pub const UNKNOWN_MEDIA_KIND: &str = "UI media kind";
