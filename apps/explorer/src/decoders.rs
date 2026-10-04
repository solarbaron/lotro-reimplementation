// SPDX-License-Identifier: GPL-3.0-or-later
//! Asset-worker decoders: everything that reads or decodes records runs here, on
//! `tessera-assets` workers, never on the UI thread.
//!
//! Each decoder holds the shared [`DataContext`] and reads what it needs itself, so the
//! stores use [`ContextSource`] (which reads nothing) as their record source. Products
//! are plain data ready to draw: rows of text, DID links, and egui images.
//!
//! | Decoder | Product | Records |
//! |---|---|---|
//! | [`RawDecoder`] | [`RawRecord`] | any DID: bytes, entry, archive |
//! | [`TextureDecoder`] | [`TextureProduct`] | 0x41 surfaces, 0x40 texture lists |
//! | [`StringsDecoder`] | [`StringsProduct`] | 0x25 string tables |
//! | [`PropsDecoder`] | [`PropsProduct`] | 0x70–0x7F objects and property sets |
//! | [`IconDecoder`] | [`IconProduct`] | 0x41 surfaces, 0x70–0x7F objects and sets |
//! | [`RecordDecoder`] | [`RecordProduct`] | DID maps, enum mappers, dictionary, lists, sounds |
//! | [`NameIndexDecoder`] | [`NameIndex`] | the root DID map and its sub-maps, the registry |

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use eframe::egui::ColorImage;
use tessera_assets::{AssetError, AssetKey, Decoder, JobContext, LruCache, RecordSource};
use tessera_dat::Entry;
use tessera_formats::{
    AnyRecord, Did, DidKind, Record, RgbaImage, StringInfo, StringSource, StringTable, Surface,
    SurfaceFormat, TextureList, TextureListKind,
};
use tessera_material::icon::{IconStack, LayerStatus, names};
use tessera_props::{Label, PropertySet, ResolvedProperty, Value};
use tessera_text::{Message, RenderOptions, RichText, Values, render};

use crate::data::DataContext;
use crate::error::ExplorerError;
use crate::image_ops::{ChannelMask, ChannelStats, icon_image, to_color_image};

/// A record source that returns no bytes; the decoders of this module read through
/// their [`DataContext`] instead.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextSource;

impl RecordSource for ContextSource {
    fn read_record(&self, _did: u32) -> Result<Vec<u8>, AssetError> {
        Ok(Vec::new())
    }
}

/// Whether a DID is a game object (0x70–0x77) or a property set (0x78–0x7F).
#[must_use]
pub fn is_property_source(did: u32) -> bool {
    matches!(did >> 24, 0x70..=0x7F)
}

/// `did` when it can name a record: not null and with a type prefix the DID table
/// lists (`records.md` §2.1). A definition's data word is not always a DID.
#[must_use]
pub fn plausible_did(did: u32) -> Option<u32> {
    (did != 0 && tessera_formats::is_catalogued_prefix((did >> 24) as u8)).then_some(did)
}

/// Longest value text kept per property row.
pub const MAX_VALUE_TEXT: usize = 512;

/// Most rows a property view or record view produces.
pub const MAX_ROWS: usize = 250_000;

fn truncate(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

// ---------------------------------------------------------------------------------------
// Raw bytes and entry information
// ---------------------------------------------------------------------------------------

/// A record's bytes and where they came from.
#[derive(Debug, Clone)]
pub struct RawRecord {
    /// The DID.
    pub did: u32,
    /// The member the bytes were read from.
    pub member: usize,
    /// That member's display name.
    pub archive: String,
    /// Its directory entry.
    pub entry: Entry,
    /// Whether the routing table (not a fallback search) found it.
    pub routed: bool,
    /// The inflated payload.
    pub bytes: Vec<u8>,
    /// Archives the routing table names for this DID.
    pub routing: Vec<String>,
}

/// Reads records with their entries. Variant 0 reads through routing; variant `n > 0`
/// reads from member `n − 1` only.
#[derive(Debug, Clone)]
pub struct RawDecoder(pub Arc<DataContext>);

/// Reads `did` (from `member`, or through routing) with its entry.
pub fn read_raw(
    ctx: &DataContext,
    did: u32,
    member: Option<usize>,
) -> Result<RawRecord, ExplorerError> {
    let set = &ctx.set;
    let routing = set
        .candidates(did)
        .into_iter()
        .map(|i| ctx.member_name(i))
        .collect();
    let (member, entry, routed) = match member {
        Some(m) => {
            let sm = set
                .get(m)
                .ok_or_else(|| ExplorerError::NotApplicable(format!("no archive #{m}")))?;
            let entry = sm
                .archive
                .lookup(did)?
                .ok_or(ExplorerError::NotFound(did))?;
            (m, entry, false)
        }
        None => {
            let l = set.locate(did)?.ok_or(ExplorerError::NotFound(did))?;
            (l.member, l.entry, l.routed)
        }
    };
    let bytes = set.member(member).archive.read_entry(&entry)?;
    Ok(RawRecord {
        did,
        member,
        archive: ctx.member_name(member),
        entry,
        routed,
        bytes,
        routing,
    })
}

impl Decoder for RawDecoder {
    type Output = RawRecord;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-raw";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        _: &JobContext<'_>,
    ) -> Result<RawRecord, ExplorerError> {
        let member = key.variant.checked_sub(1).map(|m| m as usize);
        read_raw(&self.0, key.did, member)
    }

    fn size_of(&self, v: &RawRecord) -> usize {
        v.bytes.capacity() + 256
    }
}

// ---------------------------------------------------------------------------------------
// Textures
// ---------------------------------------------------------------------------------------

/// The level value meaning "automatic": the first level of the effective chain whose
/// larger side is at most [`AUTO_LEVEL_CAP`], else the chain's last level
/// (`texture-list.md` §5.2 step 5, with the cap as the viewer's resolution setting).
pub const AUTO_LEVEL: u8 = u8::MAX;

/// Resolution cap of [`AUTO_LEVEL`]; larger levels are shown only on request.
pub const AUTO_LEVEL_CAP: u32 = 2048;

/// The level [`AUTO_LEVEL`] shows for a product whose levels are known.
#[must_use]
pub fn auto_level(p: &TextureProduct) -> Option<usize> {
    if p.list.as_ref().is_some_and(|l| l.cube) {
        return p.effective.first().copied();
    }
    p.effective
        .iter()
        .copied()
        .find(|&i| {
            p.levels
                .get(i)
                .and_then(|l| l.header)
                .is_some_and(|h| h.width.max(h.height) <= AUTO_LEVEL_CAP)
        })
        .or_else(|| p.effective.last().copied())
}

/// What a texture request asks for: a level (or face) and the channels to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureRequest {
    /// Level index in the list, or [`AUTO_LEVEL`].
    pub level: u8,
    /// Channels to show.
    pub mask: ChannelMask,
}

impl TextureRequest {
    /// The asset-key variant encoding this request.
    #[must_use]
    pub fn variant(self) -> u32 {
        u32::from(self.level) | u32::from(self.mask.bits()) << 8
    }

    /// Decodes [`TextureRequest::variant`].
    #[must_use]
    pub fn from_variant(v: u32) -> Self {
        Self {
            level: (v & 0xFF) as u8,
            mask: ChannelMask::from_bits(((v >> 8) & 0xF) as u8),
        }
    }
}

/// The header of one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceHeader {
    /// Width (0 for JPEG).
    pub width: u32,
    /// Height (0 for JPEG).
    pub height: u32,
    /// Pixel format.
    pub format: SurfaceFormat,
    /// Category field.
    pub category: u32,
    /// Stored pixel data length.
    pub data_len: usize,
}

/// One level (or cube face) of a texture.
#[derive(Debug, Clone)]
pub struct LevelInfo {
    /// Index in the list.
    pub index: usize,
    /// Surface DID.
    pub did: u32,
    /// The archive holding it.
    pub archive: Option<String>,
    /// Its header, when it resolved.
    pub header: Option<SurfaceHeader>,
    /// Why it did not resolve.
    pub error: Option<String>,
}

/// A texture list's own fields.
#[derive(Debug, Clone)]
pub struct ListInfo {
    /// List kind name.
    pub kind: String,
    /// Whether the list is a cube map.
    pub cube: bool,
    /// Stored level count.
    pub level_count: u8,
    /// Flags byte.
    pub flags: u8,
    /// Whether any field differs from every observed record.
    pub unusual: bool,
}

/// A decoded texture ready to show.
#[derive(Debug, Clone)]
pub struct TextureProduct {
    /// The requested DID.
    pub did: u32,
    /// The list, when the DID is a texture list.
    pub list: Option<ListInfo>,
    /// Every level (one for a single surface).
    pub levels: Vec<LevelInfo>,
    /// Indexes of the effective chain (`texture-list.md` §5.2).
    pub effective: Vec<usize>,
    /// The level shown.
    pub shown: Option<usize>,
    /// The image with the requested channels applied.
    pub image: Option<Arc<ColorImage>>,
    /// Shrink factor applied to fit the GPU's texture size limit (1 = none).
    pub shrink: u32,
    /// Channel statistics of the shown level (before masking).
    pub stats: Option<ChannelStats>,
    /// The channels shown.
    pub mask: ChannelMask,
    /// Why no image is shown.
    pub error: Option<String>,
}

/// Decodes surfaces and texture lists to images ([`TextureRequest`] in the key variant).
pub struct TextureDecoder {
    ctx: Arc<DataContext>,
    decoded: Mutex<LruCache<u32, Arc<RgbaImage>>>,
    headers: Mutex<LruCache<u32, (SurfaceHeader, String)>>,
}

impl std::fmt::Debug for TextureDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureDecoder").finish_non_exhaustive()
    }
}

/// Bytes of decoded surfaces the texture decoder keeps between requests (channel and
/// level changes reuse them).
pub const TEXTURE_CACHE_BYTES: usize = 256 * 1024 * 1024;

impl TextureDecoder {
    /// A decoder over `ctx`.
    #[must_use]
    pub fn new(ctx: Arc<DataContext>) -> Self {
        Self {
            ctx,
            decoded: Mutex::new(LruCache::new(TEXTURE_CACHE_BYTES)),
            headers: Mutex::new(LruCache::new(4 * 1024 * 1024)),
        }
    }

    fn read_surface(&self, did: u32) -> Result<(Vec<u8>, String), ExplorerError> {
        let set = &self.ctx.set;
        let l = set.locate(did)?.ok_or(ExplorerError::NotFound(did))?;
        let bytes = set.read_located(&l)?;
        Ok((bytes, self.ctx.member_name(l.member)))
    }

    fn header(&self, did: u32) -> Result<(SurfaceHeader, String), ExplorerError> {
        if let Some(h) = lock(&self.headers).get(&did) {
            return Ok(h.clone());
        }
        let (bytes, archive) = self.read_surface(did)?;
        let s = Surface::parse_expecting(&bytes, did)?;
        let h = (header_of(&s), archive);
        let _ = lock(&self.headers).insert(did, h.clone(), 64);
        Ok(h)
    }

    fn pixels(&self, did: u32) -> Result<Arc<RgbaImage>, ExplorerError> {
        if let Some(img) = lock(&self.decoded).get(&did) {
            return Ok(Arc::clone(img));
        }
        let (bytes, archive) = self.read_surface(did)?;
        let s = Surface::parse_expecting(&bytes, did)?;
        let _ = lock(&self.headers).insert(did, (header_of(&s), archive), 64);
        let img = Arc::new(s.to_rgba8()?);
        let _ = lock(&self.decoded).insert(did, Arc::clone(&img), img.pixels.len() + 64);
        Ok(img)
    }

    /// Decodes `did` for `req`.
    pub fn texture(
        &self,
        did: u32,
        req: TextureRequest,
        job: &JobContext<'_>,
    ) -> Result<TextureProduct, ExplorerError> {
        let mut product = TextureProduct {
            did,
            list: None,
            levels: Vec::new(),
            effective: Vec::new(),
            shown: None,
            image: None,
            shrink: 1,
            stats: None,
            mask: req.mask,
            error: None,
        };
        match Did(did).kind() {
            DidKind::RenderSurface => {
                let level = match self.header(did) {
                    Ok((h, archive)) => LevelInfo {
                        index: 0,
                        did,
                        archive: Some(archive),
                        header: Some(h),
                        error: None,
                    },
                    Err(e) => return Err(e),
                };
                product.levels.push(level);
                product.effective.push(0);
            }
            DidKind::TextureList => {
                let (bytes, _) = self.read_surface(did)?;
                let list = TextureList::parse_expecting(&bytes, did)?;
                product.list = Some(ListInfo {
                    kind: match list.kind {
                        TextureListKind::MipChain => "mip chain".to_owned(),
                        TextureListKind::CubeMap => "cube map".to_owned(),
                        other => format!("{other:?}"),
                    },
                    cube: list.kind == TextureListKind::CubeMap,
                    level_count: list.level_count,
                    flags: list.flags,
                    unusual: list.has_unusual_fields(),
                });
                for (index, &sdid) in list.surfaces.iter().enumerate() {
                    if job.is_cancelled() {
                        return Err(ExplorerError::Cancelled);
                    }
                    let (header, archive, error) = if sdid.0 == 0 {
                        (None, None, Some("null DID".to_owned()))
                    } else {
                        match self.header(sdid.0) {
                            Ok((h, a)) => (Some(h), Some(a), None),
                            Err(e) => (None, None, Some(e.to_string())),
                        }
                    };
                    product.levels.push(LevelInfo {
                        index,
                        did: sdid.0,
                        archive,
                        header,
                        error,
                    });
                }
                if list.kind == TextureListKind::CubeMap {
                    // §5.3: each face resolves on its own.
                    product.effective = product
                        .levels
                        .iter()
                        .filter(|l| l.header.is_some())
                        .map(|l| l.index)
                        .collect();
                } else {
                    let levels = &product.levels;
                    product.effective = list
                        .effective_chain(|d| {
                            levels
                                .iter()
                                .find(|l| l.did == d.0)
                                .and_then(|l| l.header.map(|_| ()))
                        })
                        .iter()
                        .map(|l| l.index)
                        .collect();
                }
            }
            other => {
                return Err(ExplorerError::NotApplicable(format!(
                    "a {} is not a texture",
                    other.name()
                )));
            }
        }
        let shown = if req.level == AUTO_LEVEL {
            auto_level(&product)
        } else {
            let want = usize::from(req.level);
            product
                .levels
                .get(want)
                .filter(|l| l.header.is_some())
                .map(|l| l.index)
                .or_else(|| product.effective.first().copied())
        };
        product.shown = shown;
        let Some(shown) = shown else {
            product.error = Some("no level resolves (texture unavailable)".into());
            return Ok(product);
        };
        let sdid = product.levels[shown].did;
        match self.pixels(sdid) {
            Ok(img) if img.width == 0 || img.height == 0 => {
                // The GPU cannot hold an empty texture; show the header, not an image.
                product.error = Some(format!(
                    "surface has zero size ({}×{})",
                    img.width, img.height
                ));
            }
            Ok(img) => {
                if job.is_cancelled() {
                    return Err(ExplorerError::Cancelled);
                }
                product.stats = Some(ChannelStats::of(&img));
                let (ci, shrink) = to_color_image(&img, req.mask, self.ctx.max_side());
                product.image = Some(Arc::new(ci));
                product.shrink = shrink;
            }
            Err(e) => product.error = Some(e.to_string()),
        }
        Ok(product)
    }
}

fn header_of(s: &Surface<'_>) -> SurfaceHeader {
    SurfaceHeader {
        width: s.width,
        height: s.height,
        format: s.format,
        category: s.category,
        data_len: s.data.len(),
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Decoder for TextureDecoder {
    type Output = TextureProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-texture";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<TextureProduct, ExplorerError> {
        self.texture(key.did, TextureRequest::from_variant(key.variant), job)
    }

    fn size_of(&self, v: &TextureProduct) -> usize {
        v.image.as_ref().map_or(0, |i| i.pixels.len() * 4) + 1024
    }
}

// ---------------------------------------------------------------------------------------
// String tables
// ---------------------------------------------------------------------------------------

/// One rendered string-table entry.
#[derive(Debug, Clone)]
pub struct StringRow {
    /// Entry token.
    pub token: u32,
    /// Explorer-mode rendering: `${NAME}` placeholders for the variables.
    pub text: RichText,
    /// The `${VAR}` format string (`string-table.md` §8.1).
    pub format: String,
    /// Number of slots.
    pub slots: usize,
    /// Grammar issues and render diagnostics, as text.
    pub notes: Vec<String>,
}

/// A rendered string table.
#[derive(Debug, Clone)]
pub struct StringsProduct {
    /// The table DID.
    pub did: u32,
    /// The decoded table (for re-rendering an entry with user values).
    pub table: Arc<StringTable>,
    /// One row per entry, in stored order.
    pub rows: Vec<StringRow>,
    /// The table's label among the named tables, if any.
    pub named: Option<String>,
    /// The locale it was read from.
    pub locale: String,
    /// Record-level issues.
    pub issues: Vec<String>,
}

/// Renders every entry of a string table in explorer mode.
#[derive(Debug, Clone)]
pub struct StringsDecoder(pub Arc<DataContext>);

/// Renders one entry: explorer mode, no values.
#[must_use]
pub fn render_entry(msg: &Message, values: &Values) -> tessera_text::Rendered {
    render(msg, values, &RenderOptions::explorer(None))
}

/// Builds the rows of a table.
pub fn string_rows(
    table: &StringTable,
    job: &JobContext<'_>,
) -> Result<Vec<StringRow>, ExplorerError> {
    let mut rows = Vec::with_capacity(table.len().min(MAX_ROWS));
    for (i, entry) in table.iter().enumerate().take(MAX_ROWS) {
        if i % 1024 == 0 && job.is_cancelled() {
            return Err(ExplorerError::Cancelled);
        }
        let msg = Message::from_entry(entry);
        let rendered = render_entry(&msg, &Values::new());
        let mut notes: Vec<String> = msg.issues().iter().map(|i| format!("{i:?}")).collect();
        notes.extend(rendered.diagnostics.iter().map(ToString::to_string));
        rows.push(StringRow {
            token: entry.token,
            text: rendered.text,
            format: truncate(msg.format_string(None), 64 * 1024),
            slots: msg.slots().len(),
            notes,
        });
    }
    Ok(rows)
}

impl Decoder for StringsDecoder {
    type Output = StringsProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-strings";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<StringsProduct, ExplorerError> {
        let did = key.did;
        let strings = self.0.strings.as_ref().ok_or_else(|| {
            ExplorerError::NotApplicable("the install has no language archive".into())
        })?;
        let table = strings.table(did)?.ok_or_else(|| {
            ExplorerError::NotApplicable(format!(
                "{did:#010x} is not a string table of the selected locale"
            ))
        })?;
        let rows = string_rows(&table, job)?;
        let issues = table.issues().iter().map(|i| format!("{i:?}")).collect();
        let named = strings
            .named_tables()
            .tables()
            .iter()
            .find(|t| t.did == did)
            .map(|t| t.label.clone());
        let locale = strings
            .locale()
            .map(|l| format!("{} ({})", l.name, l.language_id))
            .unwrap_or_default();
        Ok(StringsProduct {
            did,
            table,
            rows,
            named,
            locale,
            issues,
        })
    }

    fn size_of(&self, v: &StringsProduct) -> usize {
        v.rows
            .iter()
            .map(|r| r.text.len() + r.format.len() + 96)
            .sum::<usize>()
            + 1024
    }
}

// ---------------------------------------------------------------------------------------
// Property sets
// ---------------------------------------------------------------------------------------

/// One row of a flattened property tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PropRow {
    /// Nesting depth (0 = top level).
    pub depth: u16,
    /// Property ID.
    pub pid: u32,
    /// Registry name, or the PID in hex.
    pub name: String,
    /// Declared type.
    pub declared: String,
    /// The value as text.
    pub value: String,
    /// Its label (enum names, display texts, bit names, DID types).
    pub label: String,
    /// Clickable DIDs: (caption, DID).
    pub links: Vec<(String, u32)>,
    /// Number of rows below this one that belong to it (struct fields, array elements).
    pub descendants: usize,
}

/// A property set ready to show.
#[derive(Debug, Clone)]
pub struct PropsProduct {
    /// The requested DID.
    pub did: u32,
    /// The property-set record actually read (+0x09000000 rule for objects).
    pub set_did: u32,
    /// Bytes after the set in the record.
    pub trailing: usize,
    /// Top-level property count.
    pub count: usize,
    /// Nesting depth.
    pub depth: usize,
    /// Flattened rows.
    pub rows: Vec<PropRow>,
    /// Whether rows were cut at [`MAX_ROWS`].
    pub truncated: bool,
}

/// Decodes and resolves property sets (`property-system.md` §5–§7).
#[derive(Debug, Clone)]
pub struct PropsDecoder(pub Arc<DataContext>);

/// The property-set DID for `did` and the set with its trailing byte count.
pub fn load_property_set(
    ctx: &DataContext,
    did: u32,
) -> Result<(u32, PropertySet, usize), ExplorerError> {
    let set_did = match did >> 24 {
        0x78..=0x7F => did,
        0x70..=0x77 => tessera_formats::object_property_set_did(Did(did))
            .map(|d| d.0)
            .ok_or_else(|| {
                ExplorerError::NotApplicable(format!("{did:#010x} has no property set"))
            })?,
        _ => {
            return Err(ExplorerError::NotApplicable(format!(
                "{did:#010x} is not an object or property set"
            )));
        }
    };
    let (set, trailing) = ctx
        .game
        .property_set_prefix(Did(set_did))?
        .ok_or(ExplorerError::NotFound(set_did))?;
    Ok((set_did, set, trailing))
}

/// The value of a property as text (bounded by [`MAX_VALUE_TEXT`]).
#[must_use]
pub fn value_text(v: &Value) -> String {
    let s = match v {
        Value::String(s) => format!("{s:?}"),
        Value::StringToken(t) => format!("token {t:#010x}"),
        Value::Waveform(w) => format!("waveform subtype {} {:?}", w.subtype, w.values),
        Value::Timestamp(t) => format!("{t}"),
        Value::TriState(t) => format!("{t}"),
        Value::Vector3(v) => format!("({}, {}, {})", v[0], v[1], v[2]),
        Value::InstanceId(i) => format!("{i:#018x}"),
        Value::Enum(e) => format!("{e}"),
        Value::Float(f) => format!("{f}"),
        Value::PropertyId(p) => format!("pid {p:#x}"),
        Value::Struct(s) => format!("struct ({} fields)", s.len()),
        Value::Array(a) => format!("array ({} elements)", a.len()),
        Value::StringInfo(si) => string_info_text(si),
        Value::Bitfield64(b) => format!("{b:#018x}"),
        Value::Int(i) => format!("{i}"),
        Value::Colour(c) => format!("#{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3]),
        Value::Position(p) => format!("{p:?}"),
        Value::Bitfield32(b) => format!("{b:#010x}"),
        Value::Int64(i) => format!("{i}"),
        Value::DidRef(d) => format!("{:#010x}", d.0),
        Value::Bool(b) => format!("{b}"),
        Value::BitfieldAny(b) => {
            let mut s = format!("{} bits: ", b.bits);
            for byte in b.bytes.iter().take(64) {
                let _ = write!(s, "{byte:02x}");
            }
            if b.bytes.len() > 64 {
                s.push('…');
            }
            s
        }
        other => format!("{other:?}"),
    };
    truncate(s, MAX_VALUE_TEXT)
}

/// A StringInfo's source as text (`table:token` or the literal).
#[must_use]
pub fn string_info_text(si: &StringInfo) -> String {
    let mut s = match &si.source {
        StringSource::Literal(t) => format!("literal {:?}", t.to_string_lossy()),
        StringSource::Table { token, table } => format!("{:#010x}:{token:#010x}", table.0),
    };
    if let Some(args) = &si.args {
        let _ = write!(s, " + {} args", args.values.len());
    }
    s
}

/// The label of a resolved property as text, display texts resolved through `strings`.
#[must_use]
pub fn label_text(label: &Label, value: &Value, strings: Option<&tessera_text::Strings>) -> String {
    let resolve = |si: &StringInfo| -> Option<String> {
        let s = strings?;
        let r = s.resolve(si, &Values::new(), &RenderOptions::explorer(None));
        Some(r.plain_text())
    };
    let mut out = match label {
        Label::Enum { label, .. } => {
            let mut s = label
                .name
                .as_deref()
                .map_or_else(|| format!("#{}", label.key), str::to_owned);
            if let Some(d) = label.display.as_deref().and_then(resolve) {
                let _ = write!(s, " “{d}”");
            }
            s
        }
        other => other.to_string(),
    };
    if let Value::StringInfo(si) = value
        && let Some(text) = resolve(si)
    {
        out = format!("“{text}”");
    }
    truncate(out, MAX_VALUE_TEXT)
}

/// The DIDs a property row links to.
#[must_use]
pub fn row_links(label: &Label, value: &Value) -> Vec<(String, u32)> {
    let mut links = Vec::new();
    if let Some(d) = value.did_ref() {
        links.push(("open".to_owned(), d.0));
    }
    if let Value::StringInfo(si) = value
        && let StringSource::Table { table, .. } = &si.source
        && !table.is_null()
    {
        links.push(("table".to_owned(), table.0));
    }
    match label {
        Label::Enum { mapper, .. } | Label::Bits { mapper, .. } if !mapper.is_null() => {
            links.push(("mapper".to_owned(), mapper.0));
        }
        Label::DidMapKey { map, target, .. } => {
            if let Some(t) = target.filter(|t| !t.is_null()) {
                links.push(("target".to_owned(), t.0));
            }
            if !map.is_null() {
                links.push(("map".to_owned(), map.0));
            }
        }
        _ => {}
    }
    links
}

/// Flattens a resolved property tree into rows (depth first, stored order), stopping
/// at `max_rows`. Returns the rows and whether the tree was cut.
pub fn flatten_props(
    resolved: &[ResolvedProperty<'_>],
    strings: Option<&tessera_text::Strings>,
    max_rows: usize,
) -> (Vec<PropRow>, bool) {
    let mut rows: Vec<PropRow> = Vec::new();
    // Properties still to visit, with their depth (popped in stored order).
    let mut stack: Vec<(&ResolvedProperty<'_>, u16)> =
        resolved.iter().rev().map(|p| (p, 0)).collect();
    let mut truncated = false;
    // (row index, depth) of open ancestors, to count descendants.
    let mut open: Vec<(usize, u16)> = Vec::new();
    while let Some((p, depth)) = stack.pop() {
        if rows.len() >= max_rows {
            truncated = true;
            break;
        }
        while open.last().is_some_and(|&(_, d)| d >= depth) {
            open.pop();
        }
        for &(i, _) in &open {
            rows[i].descendants += 1;
        }
        let value: &Value = &p.value;
        let name = p
            .name
            .as_deref()
            .map_or_else(|| format!("{:#x}", p.pid), str::to_owned);
        rows.push(PropRow {
            depth,
            pid: p.pid,
            name,
            declared: p
                .declared
                .map_or_else(|| "?".to_owned(), |t| format!("{t:?}")),
            value: value_text(value),
            label: label_text(&p.label, value, strings),
            links: row_links(&p.label, value),
            descendants: 0,
        });
        if !p.children.is_empty() {
            open.push((rows.len() - 1, depth));
            let child_depth = depth.saturating_add(1);
            stack.extend(p.children.iter().rev().map(|c| (c, child_depth)));
        }
    }
    (rows, truncated)
}

impl Decoder for PropsDecoder {
    type Output = PropsProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-props";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        _: &JobContext<'_>,
    ) -> Result<PropsProduct, ExplorerError> {
        let ctx = &self.0;
        let (set_did, set, trailing) = load_property_set(ctx, key.did)?;
        let resolved = ctx.game.resolve(&set)?;
        let (rows, truncated) = flatten_props(&resolved, ctx.strings.as_ref(), MAX_ROWS);
        Ok(PropsProduct {
            did: key.did,
            set_did,
            trailing,
            count: set.len(),
            depth: set.depth(),
            rows,
            truncated,
        })
    }

    fn size_of(&self, v: &PropsProduct) -> usize {
        v.rows
            .iter()
            .map(|r| r.name.len() + r.value.len() + r.label.len() + 128)
            .sum::<usize>()
            + 256
    }
}

// ---------------------------------------------------------------------------------------
// Icons
// ---------------------------------------------------------------------------------------

/// One layer of a composed icon.
#[derive(Debug, Clone)]
pub struct IconLayerRow {
    /// Layer role name.
    pub role: String,
    /// Surface DID.
    pub did: u32,
    /// Property it came from.
    pub property: Option<String>,
    /// What happened to it.
    pub status: String,
}

/// One icon family composed from a record.
#[derive(Debug, Clone)]
pub struct IconComposite {
    /// Family name (`item`, `skill`, ...).
    pub family: String,
    /// The composed image.
    pub image: Option<Arc<ColorImage>>,
    /// Its size in pixels.
    pub size: Option<(u32, u32)>,
    /// Layers bottom to top.
    pub layers: Vec<IconLayerRow>,
}

/// Every icon a record yields.
#[derive(Debug, Clone)]
pub struct IconProduct {
    /// The requested DID.
    pub did: u32,
    /// Composed icons (empty when the record names no icon).
    pub composites: Vec<IconComposite>,
}

/// Composes layered icons (`icon-layers.md`) for surfaces, objects and property sets.
#[derive(Debug, Clone)]
pub struct IconDecoder(pub Arc<DataContext>);

/// The icon stacks a property set names, by family.
#[must_use]
pub fn icon_stacks(
    set: &PropertySet,
    registry: &tessera_props::PropertyRegistry,
) -> Vec<(&'static str, IconStack)> {
    let lookup = |name: &str| {
        registry
            .pid(name)
            .and_then(|pid| set.get(pid))
            .and_then(Value::did_ref)
    };
    let candidates = [
        ("item", IconStack::item(lookup)),
        ("large item image", IconStack::large_item_image(lookup)),
        ("trait", IconStack::trait_icon(lookup)),
        (
            "skill",
            IconStack::single_property(lookup, names::SKILL_ICON),
        ),
        (
            "skill (large)",
            IconStack::single_property(lookup, names::SKILL_LARGE_ICON),
        ),
        (
            "skill (small)",
            IconStack::single_property(lookup, names::SKILL_SMALL_ICON),
        ),
        (
            "effect",
            IconStack::single_property(lookup, names::EFFECT_ICON),
        ),
    ];
    candidates
        .into_iter()
        .filter(|(_, s)| !s.is_empty())
        .collect()
}

impl IconDecoder {
    fn compose(&self, family: &str, stack: &IconStack) -> IconComposite {
        let ctx = &self.0;
        let c = ctx.composer.compose(stack);
        let layers = c
            .layers
            .iter()
            .map(|l| IconLayerRow {
                role: l.entry.role.name().to_owned(),
                did: l.entry.did.0,
                property: l.entry.property.map(str::to_owned),
                status: match &l.status {
                    LayerStatus::Drawn { width, height } => format!("drawn {width}×{height}"),
                    LayerStatus::Transparent { width, height } => {
                        format!("transparent {width}×{height} (absent)")
                    }
                    LayerStatus::Skipped(e) => format!("skipped: {e}"),
                },
            })
            .collect();
        IconComposite {
            family: family.to_owned(),
            size: c.image.as_ref().map(|i| (i.width, i.height)),
            image: c
                .image
                .as_ref()
                .filter(|i| i.width > 0 && i.height > 0)
                .map(|i| Arc::new(icon_image(i, ctx.max_side()))),
            layers,
        }
    }
}

impl Decoder for IconDecoder {
    type Output = IconProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-icon";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        _: &JobContext<'_>,
    ) -> Result<IconProduct, ExplorerError> {
        let did = key.did;
        let composites = if did >> 24 == 0x41 {
            vec![self.compose("surface", &IconStack::single(Did(did)))]
        } else if is_property_source(did) {
            let (_, set, _) = load_property_set(&self.0, did)?;
            let registry = self.0.game.registry()?;
            icon_stacks(&set, registry)
                .iter()
                .map(|(family, stack)| self.compose(family, stack))
                .collect()
        } else {
            return Err(ExplorerError::NotApplicable(format!(
                "{did:#010x} names no icon"
            )));
        };
        Ok(IconProduct { did, composites })
    }

    fn size_of(&self, v: &IconProduct) -> usize {
        v.composites
            .iter()
            .map(|c| c.image.as_ref().map_or(0, |i| i.pixels.len() * 4) + 256)
            .sum::<usize>()
            + 64
    }
}

// ---------------------------------------------------------------------------------------
// Generic records
// ---------------------------------------------------------------------------------------

/// One row of a record table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRow {
    /// Cells, one per column.
    pub cells: Vec<String>,
    /// A DID the row links to.
    pub link: Option<u32>,
}

/// A decoded record as a summary plus a table.
#[derive(Debug, Clone, Default)]
pub struct RecordProduct {
    /// The DID.
    pub did: u32,
    /// Record type name.
    pub type_name: String,
    /// Key/value summary lines.
    pub summary: Vec<(String, String)>,
    /// Linked DIDs of the summary: (caption, DID).
    pub links: Vec<(String, u32)>,
    /// Column names of the table.
    pub columns: Vec<&'static str>,
    /// Table rows.
    pub rows: Vec<RecordRow>,
}

/// Decodes the records `tessera-formats` knows into tables (DID maps with navigable
/// targets, enum mappers with resolved display texts, the property dictionary, texture
/// lists, sounds).
#[derive(Debug, Clone)]
pub struct RecordDecoder(pub Arc<DataContext>);

/// Builds the record view of `bytes` (record `did`).
pub fn record_product(
    ctx: &DataContext,
    did: u32,
    bytes: &[u8],
) -> Result<RecordProduct, ExplorerError> {
    let mut p = RecordProduct {
        did,
        type_name: Did(did).kind().name().to_owned(),
        ..RecordProduct::default()
    };
    let Some(rec) = AnyRecord::decode(did, bytes)? else {
        p.summary.push((
            "decoder".into(),
            "none for this type; see the hex view".into(),
        ));
        return Ok(p);
    };
    p.type_name = rec.type_name().to_owned();
    let resolve = |si: &StringInfo| -> String {
        ctx.strings.as_ref().map_or_else(
            || string_info_text(si),
            |s| {
                s.resolve(si, &Values::new(), &RenderOptions::explorer(None))
                    .plain_text()
            },
        )
    };
    match rec {
        AnyRecord::DataIdMap(map) => {
            p.summary.push(("entries".into(), map.len().to_string()));
            p.summary
                .push(("groups".into(), map.groups().len().to_string()));
            p.columns = vec!["key", "label", "value", "group"];
            for e in map.iter().take(MAX_ROWS) {
                p.rows.push(RecordRow {
                    cells: vec![
                        format!("{:#x}", e.key),
                        e.label.to_string(),
                        format!("{:#010x}", e.value.0),
                        e.group.to_string(),
                    ],
                    link: (!e.value.is_null()).then_some(e.value.0),
                });
            }
        }
        AnyRecord::EnumMapper(m) => {
            p.summary
                .push(("names".into(), m.names().len().to_string()));
            p.summary
                .push(("display texts".into(), m.displays().len().to_string()));
            if !m.base.is_null() {
                p.summary
                    .push(("base".into(), format!("{:#010x}", m.base.0)));
                p.links.push(("base mapper".into(), m.base.0));
            }
            p.columns = vec!["key", "name", "display"];
            let mut keys: Vec<u32> = m.names().iter().map(|(k, _)| *k).collect();
            keys.extend(m.displays().iter().map(|(k, _)| *k));
            keys.sort_unstable();
            keys.dedup();
            for k in keys.into_iter().take(MAX_ROWS) {
                p.rows.push(RecordRow {
                    cells: vec![
                        k.to_string(),
                        m.name(k).unwrap_or_default().to_owned(),
                        m.display(k).map(resolve).unwrap_or_default(),
                    ],
                    link: None,
                });
            }
        }
        AnyRecord::PropertyDictionary(d) => {
            p.summary
                .push(("definitions".into(), d.definitions().len().to_string()));
            p.columns = vec!["pid", "name", "type", "data"];
            for def in d.definitions().iter().take(MAX_ROWS) {
                p.rows.push(RecordRow {
                    cells: vec![
                        format!("{:#x}", def.pid),
                        d.name(def.pid).unwrap_or_default().to_owned(),
                        format!("{:?}", def.ty),
                        if def.data.is_null() {
                            String::new()
                        } else {
                            format!("{:#010x}", def.data.0)
                        },
                    ],
                    link: plausible_did(def.data.0),
                });
            }
        }
        AnyRecord::TextureList(l) => {
            p.summary.push(("kind".into(), format!("{:?}", l.kind)));
            p.summary
                .push(("level count".into(), l.level_count.to_string()));
            p.summary.push(("flags".into(), l.flags.to_string()));
            p.columns = vec!["index", "surface"];
            for (i, s) in l.surfaces.iter().enumerate() {
                p.rows.push(RecordRow {
                    cells: vec![i.to_string(), format!("{:#010x}", s.0)],
                    link: (!s.is_null()).then_some(s.0),
                });
            }
        }
        AnyRecord::Surface(s) => {
            p.summary
                .push(("format".into(), s.format.name().to_owned()));
            p.summary
                .push(("size".into(), format!("{}×{}", s.width, s.height)));
            p.summary.push(("category".into(), s.category.to_string()));
            p.summary
                .push(("data".into(), format!("{} bytes", s.data.len())));
        }
        AnyRecord::Sound(s) => {
            p.summary
                .push(("container".into(), format!("{:?}", s.container())));
            p.summary
                .push(("audio".into(), format!("{} bytes", s.audio.len())));
            p.summary.push((
                "trailer".into(),
                format!("{} / {}", s.trailer_u32, s.trailer_f32),
            ));
        }
        AnyRecord::StringTable(t) => {
            p.summary.push(("entries".into(), t.len().to_string()));
            p.summary
                .push(("note".into(), "see the strings view".into()));
        }
        other => {
            p.summary
                .push(("record".into(), other.type_name().to_owned()));
        }
    }
    Ok(p)
}

impl Decoder for RecordDecoder {
    type Output = RecordProduct;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-record";

    fn decode(
        &self,
        key: AssetKey,
        _: Vec<u8>,
        _: &JobContext<'_>,
    ) -> Result<RecordProduct, ExplorerError> {
        let member = key.variant.checked_sub(1).map(|m| m as usize);
        let raw = read_raw(&self.0, key.did, member)?;
        record_product(&self.0, key.did, &raw.bytes)
    }

    fn size_of(&self, v: &RecordProduct) -> usize {
        v.rows
            .iter()
            .map(|r| r.cells.iter().map(String::len).sum::<usize>() + 64)
            .sum::<usize>()
            + 256
    }
}

// ---------------------------------------------------------------------------------------
// Name index (search by name)
// ---------------------------------------------------------------------------------------

/// One searchable name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEntry {
    /// The name as shown (`MAP/label` for DID-map labels).
    pub text: String,
    /// Lowercased `text`, for matching.
    pub lower: String,
    /// The DID it leads to.
    pub did: Option<u32>,
    /// Where the name comes from.
    pub detail: String,
}

/// Every name the explorer can search: DID-map labels (root map and its sub-maps) and
/// property names from the registry.
#[derive(Debug, Clone, Default)]
pub struct NameIndex {
    /// The names.
    pub entries: Vec<NameEntry>,
    /// Problems met while building it.
    pub problems: Vec<String>,
    /// Whether names were dropped at [`MAX_NAMES`].
    pub truncated: bool,
}

/// Most names kept in the index.
pub const MAX_NAMES: usize = 1_000_000;

impl NameIndex {
    /// Adds a name; beyond [`MAX_NAMES`] names it is dropped and
    /// [`NameIndex::truncated`] is set.
    pub fn push(&mut self, text: String, did: Option<u32>, detail: String) {
        self.push_bounded(text, did, detail, MAX_NAMES);
    }

    fn push_bounded(&mut self, text: String, did: Option<u32>, detail: String, max: usize) {
        if self.entries.len() >= max {
            self.truncated = true;
            return;
        }
        self.entries.push(NameEntry {
            lower: text.to_lowercase(),
            text,
            did,
            detail,
        });
    }
}

/// Builds the [`NameIndex`] (key: the root DID map).
#[derive(Debug, Clone)]
pub struct NameIndexDecoder(pub Arc<DataContext>);

/// Builds the name index from `ctx`.
pub fn build_name_index(ctx: &DataContext, job: &JobContext<'_>) -> NameIndex {
    let mut idx = NameIndex::default();
    match ctx.game.root_did_map() {
        Ok(root) => {
            for e in root.iter() {
                idx.push(
                    e.label.to_string(),
                    (!e.value.is_null()).then_some(e.value.0),
                    "root DID map".into(),
                );
            }
            for e in root.iter() {
                if job.is_cancelled() {
                    break;
                }
                if e.value.prefix() != 0x28 || e.value == root.did {
                    continue;
                }
                match ctx.game.did_map(e.value) {
                    Ok(Some(sub)) => {
                        for s in sub.iter() {
                            idx.push(
                                format!("{}/{}", e.label, s.label),
                                (!s.value.is_null()).then_some(s.value.0),
                                format!("DID map {:#010x} key {:#x}", e.value.0, s.key),
                            );
                        }
                    }
                    Ok(None) => {}
                    Err(err) => idx.problems.push(format!("{}: {err}", e.label)),
                }
            }
        }
        Err(e) => idx.problems.push(format!("root DID map: {e}")),
    }
    match ctx.game.registry() {
        Ok(reg) => {
            for (def, name) in reg.iter() {
                let Some(name) = name else { continue };
                idx.push(
                    name.to_owned(),
                    plausible_did(def.data.0),
                    format!("property {:#x} ({:?})", def.pid, def.ty),
                );
            }
        }
        Err(e) => idx.problems.push(format!("property registry: {e}")),
    }
    if idx.truncated {
        idx.problems.push(format!(
            "more than {MAX_NAMES} names; the rest are not searchable"
        ));
    }
    idx
}

impl Decoder for NameIndexDecoder {
    type Output = NameIndex;
    type Error = ExplorerError;
    const NAME: &'static str = "explorer-names";

    fn decode(
        &self,
        _: AssetKey,
        _: Vec<u8>,
        job: &JobContext<'_>,
    ) -> Result<NameIndex, ExplorerError> {
        Ok(build_name_index(&self.0, job))
    }

    fn size_of(&self, v: &NameIndex) -> usize {
        v.entries
            .iter()
            .map(|e| e.text.len() * 2 + e.detail.len() + 64)
            .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn texture_request_variant_round_trip() {
        for level in [0u8, 3, AUTO_LEVEL] {
            for bits in 0..16u8 {
                let r = TextureRequest {
                    level,
                    mask: ChannelMask::from_bits(bits),
                };
                assert_eq!(TextureRequest::from_variant(r.variant()), r);
            }
        }
    }

    #[test]
    fn plausible_dids() {
        assert_eq!(plausible_did(0x2300_0001), Some(0x2300_0001));
        assert_eq!(plausible_did(0), None);
        assert_eq!(plausible_did(0x4D), None);
    }

    #[test]
    fn property_sources() {
        assert!(is_property_source(0x7000_0001));
        assert!(is_property_source(0x7900_0001));
        assert!(!is_property_source(0x4100_0001));
    }

    #[test]
    fn values_as_text() {
        assert_eq!(value_text(&Value::Int(-3)), "-3");
        assert_eq!(value_text(&Value::DidRef(Did(0x4100_0001))), "0x41000001");
        assert_eq!(value_text(&Value::Colour([1, 2, 3, 4])), "#01020304");
        assert_eq!(value_text(&Value::String("x".into())), "\"x\"");
        let long = value_text(&Value::String("a".repeat(2000)));
        assert!(long.len() <= MAX_VALUE_TEXT + 4);
        let si = StringInfo {
            source: StringSource::Table {
                token: 5,
                table: Did(0x2500_0001),
            },
            args: None,
        };
        assert_eq!(string_info_text(&si), "0x25000001:0x00000005");
    }

    fn rp(
        pid: u32,
        value: Value,
        label: Label,
        children: Vec<ResolvedProperty<'static>>,
    ) -> ResolvedProperty<'static> {
        ResolvedProperty {
            pid,
            name: Some(format!("P{pid}").into()),
            declared: None,
            value: Cow::Owned(value),
            label,
            children,
        }
    }

    #[test]
    fn flattening_counts_descendants_and_links() {
        let tree = vec![
            rp(1, Value::Int(1), Label::None, vec![]),
            rp(
                2,
                Value::Array(Vec::new()),
                Label::None,
                vec![
                    rp(3, Value::DidRef(Did(0x4100_0005)), Label::None, vec![]),
                    rp(
                        4,
                        Value::Array(Vec::new()),
                        Label::None,
                        vec![rp(5, Value::Bool(true), Label::None, vec![])],
                    ),
                ],
            ),
            rp(
                6,
                Value::Enum(2),
                Label::DidMapKey {
                    map: Did(0x2800_0012),
                    key: 2,
                    label: Some("TWO".into()),
                    target: Some(Did(0x7000_0002)),
                },
                vec![],
            ),
        ];
        let (rows, cut) = flatten_props(&tree, None, 100);
        assert!(!cut);
        let summary: Vec<(u32, u16, usize)> = rows
            .iter()
            .map(|r| (r.pid, r.depth, r.descendants))
            .collect();
        assert_eq!(
            summary,
            vec![
                (1, 0, 0),
                (2, 0, 3),
                (3, 1, 0),
                (4, 1, 1),
                (5, 2, 0),
                (6, 0, 0)
            ]
        );
        assert_eq!(rows[2].links, vec![("open".to_owned(), 0x4100_0005)]);
        assert_eq!(
            rows[5].links,
            vec![
                ("target".to_owned(), 0x7000_0002),
                ("map".to_owned(), 0x2800_0012)
            ]
        );
        assert!(rows[5].label.starts_with("TWO"), "{}", rows[5].label);
        let (rows, cut) = flatten_props(&tree, None, 2);
        assert!(cut);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn name_index_is_bounded() {
        let mut idx = NameIndex::default();
        idx.push("Hello".into(), Some(1), "x".into());
        assert_eq!(idx.entries[0].lower, "hello");
        assert!(!idx.truncated);
        for i in 0..5 {
            idx.push_bounded(format!("N{i}"), None, String::new(), 3);
        }
        assert_eq!(idx.entries.len(), 3);
        assert!(idx.truncated);
    }
}
