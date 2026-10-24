// SPDX-License-Identifier: GPL-3.0-or-later
//! Decision 0024 P8 exit criterion 1, the part "tooltips of 50 items and 50 skills
//! included", on the user's install (read-only; `item-tooltips.md`, `skill-tooltips.md`,
//! `scenario-feed.md` §8).
//!
//! The committed synthetic scenario `tests/scenarios/parity-tooltips.scenario` names its
//! records by seeded selectors only. This test resolves them against the install with the
//! client's resolver over `tessera-gamedata` (`scenario::resolver::GameDataResolver`) and
//! takes, per family, the records of the item (or skill) bindings in declaration order and,
//! within a binding, in its resolved (canonical) order, each record once: ordinals 1–50
//! (decision 0043 §1 item 6). The scenario draws more than 50 per family, so overlaps and a
//! patch that drops a candidate still leave 50; fewer than 50 distinct records in either
//! family is a failure that names the shortfall. The resolved scenario is also played
//! through the sandbox feed, which must show each drawn item once (one `ItemAdded` per
//! record of the item bindings), the 50 quickslot records once each, and skip no step.
//!
//! Each record's hover tooltip goes through the client's own public path: the tooltip
//! model of `tessera-gamedata` (`CharacterData::engine`), the client's conversion
//! (`character::tooltip::convert` for items, `combat::view::exam_tooltip` for skills) and
//! the game UI's examination panel (`ExamTooltips::build`) in a [`UiHost`] booted from the
//! install (`UiBoot`), at a fixed UI scale of 1 (*Se* = 1). It is painted by the GPU UI pass
//! offscreen and cropped to the panel's box.
//!
//! **Panel audit.** Before a picture is taken, the built panel's displayed elements are
//! checked against its input, so that a picture with a defect is never blessed as a
//! baseline: every displayed element that draws something (a base image or a text) lies
//! inside the frame's displayed box (nothing cut off, `item-tooltips.md` §4–§6); every image the input names (the examination icon of
//! `item-tooltips.md` §6.3 and `skill-tooltips.md` §11.1, image rows, coin icons) is the
//! base image of a displayed element inside the frame; every coin of a worth row has an
//! icon (§6.11); and a record whose model shows a name (`named`, `NameState::Shown`) has a
//! displayed `ExaminationName` element with text (§6.2). A failed audit is a failure, and
//! its picture is neither written nor compared.
//!
//! **Warm-up.** The first record's tooltip is built and destroyed once, and the UI settled,
//! before the pictures: the panel measures its texts when it is built, so a tooltip built
//! while its fonts still load is laid out with the elements' design sizes (a client defect
//! of the first hover, recorded in the P8 status), which this test does not judge.
//!
//! **Output** (decision 0031 tier 2, `ui_common/goldens.rs`): `item-NN.png`, `skill-NN.png`
//! and a local `index.txt` (ordinal, family, DID, the record's tooltip category with its
//! row-failure and lookup-miss counts, and the record's display name, so the owner can find
//! each record in the original client) in
//! `${TESSERA_GOLDEN_DIR:-${XDG_CACHE_HOME:-~/.cache}/tessera/goldens}/tessera-client/
//! parity_tooltips_real_data/<function>/<install version>/`, never in a work tree and never
//! in the install. A missing picture is written (`blessed`), an existing one compared with
//! the render crate's tolerance; `TESSERA_GOLDEN_REQUIRE=1` and
//! `TESSERA_GOLDEN_BLESS_LOCAL=1` work as for every tier-2 golden.
//!
//! A failure is: a shortfall of records, a classification error that escapes the
//! categories, a model that does not build or convert, a panel with template failures or an
//! empty frame, a failed panel audit, a UI that does not settle before the picture, or a
//! golden mismatch. A record categorised without a tooltip to paint (`NoTooltip`,
//! `LayoutUnreadable`, `ElementMissing`; P1 and P2 criterion 2) is not a failure: it is
//! noted with its category in the index and counted, and not painted (decision 0043 §1
//! item 6). `RenderedNoName` and row failures are painted and noted.
//!
//! Skipped (reported as passed) unless `TESSERA_GAME_DIR` is set; also skipped when no GPU
//! adapter exists (unless `TESSERA_REQUIRE_GPU=1`). Run with `TESSERA_GAME_DIR=… cargo test
//! --release -p tessera-client --test parity_tooltips_real_data -- --nocapture`. Prints
//! counts and paths only, never game text.

#[path = "../../../crates/render/tests/support/golden.rs"]
mod golden;
#[path = "ui_common/goldens.rs"]
mod goldens;
#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tessera_client::character::CharacterData;
use tessera_client::character::tooltip::convert;
use tessera_client::combat::headless::{CLEAR, SETTLE_LIMIT};
use tessera_client::combat::view::exam_tooltip;
use tessera_client::scenario::resolver::GameDataResolver;
use tessera_client::ui_host::boot::UiBoot;
use tessera_client::ui_host::{UiHost, UiOptions};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::Did;
use tessera_gamedata::tooltip::{
    Category, CombatCategory, CombatRequest, CombatSubject, NameState, SkillViewer, TooltipRequest,
    Viewer,
};
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{Family, HeadlessDriver, Scenario, SelectorResolver as _};
use tessera_net::{DataId, Session};
use tessera_render::{OffscreenFormat, PassSpace, Renderer, RgbaImage};
use tessera_settings::{UiPrefs, UiScale};
use tessera_ui::game::IconImage;
use tessera_ui::game::examine::{
    BuiltTooltip, ExamKey, ExamRow, ExamTooltip, ExamTooltips, RowContent,
};
use tessera_ui::{IBox, ImageRef, Ui};

/// Records per family (P8 criterion 1).
const PER_FAMILY: usize = 50;

/// The screen of the UI host: at least the design floor (1024 × 768, `ui-scaling.md` §4.3)
/// and at most 1440 rows, so *S*auto would be 1 too (§4.2); tall enough for every tooltip of
/// the sample.
const SCREEN: (u32, u32) = (1024, 1440);

/// The level of the scenario's player (`player "Rook" level=50`), used as the viewer level.
const VIEWER_LEVEL: i32 = 50;

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn scenario_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/parity-tooltips.scenario")
}

/// Compares a GPU picture with its golden under the render crate's default tolerance.
fn gpu_compare(want: &goldens::Rgba, got: &goldens::Rgba) -> (bool, String, goldens::Rgba) {
    let image = |g: &goldens::Rgba| RgbaImage {
        width: g.width,
        height: g.height,
        data: g.data.clone(),
    };
    let tol = golden::Tolerance::default();
    let (c, diff) = golden::compare(&image(want), &image(got), tol);
    (
        c.passes(tol),
        format!(
            "{} outliers, {} strict mismatches of {} pixels, max difference {}",
            c.outliers, c.strict_mismatches, c.pixels, c.max_diff
        ),
        goldens::Rgba {
            width: diff.width,
            height: diff.height,
            data: diff.data,
        },
    )
}

/// The records of every binding of `family`, in declaration order and binding order, each
/// once.
fn ordered(
    scenario: &Scenario,
    resolved: &tessera_net::scenario::ResolvedScenario,
    family: Family,
) -> Vec<Did> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (name, _, f, _) in scenario.selectors() {
        if f != family {
            continue;
        }
        for c in resolved.binding(name).unwrap_or_default() {
            if seen.insert(c.did.0) {
                out.push(Did(c.did.0));
            }
        }
    }
    out
}

/// Whether a row (or a row nested in it) shows an icon or image.
fn has_icon(r: &ExamRow) -> bool {
    match &r.content {
        RowContent::Image(_) | RowContent::Icon { image: Some(_), .. } => true,
        RowContent::Pair { left, right } => has_icon(left) || has_icon(right),
        RowContent::List(rows) => rows.iter().any(has_icon),
        _ => false,
    }
}

/// Most displayed elements the panel audit reads of one tooltip (Tessera cap).
const MAX_AUDIT_NODES: usize = 4096;

/// What the built panel must show of its input (the panel audit, module docs).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Wanted {
    /// Every image the input names (icons, icon layers, image rows, coin icons), each once.
    images: Vec<ImageRef>,
    /// Coins of worth rows without an icon in the input.
    coins_without_icon: usize,
    /// Whether the model shows a name, so the panel must show an `ExaminationName` text.
    name: bool,
}

impl Wanted {
    /// The images and coins of `rows` (hidden rows excluded); `name` from the model.
    fn of(rows: &[ExamRow], name: bool) -> Self {
        let mut w = Self {
            name,
            ..Self::default()
        };
        for r in rows {
            w.add(r, 0);
        }
        w
    }

    fn image(&mut self, img: &ImageRef) {
        if !self.images.contains(img) {
            self.images.push(img.clone());
        }
    }

    fn add(&mut self, r: &ExamRow, depth: usize) {
        if r.hidden || depth > tessera_ui::game::examine::MAX_ROW_DEPTH {
            return;
        }
        match &r.content {
            RowContent::Image(img)
            | RowContent::Icon {
                image: Some(IconImage::Composed(img)),
                ..
            } => {
                self.image(img);
            }
            RowContent::Icon {
                image: Some(IconImage::Layers(layers)),
                ..
            } => {
                for img in layers.iter().take(tessera_ui::game::MAX_ICON_LAYERS) {
                    self.image(img);
                }
            }
            RowContent::Money { coins, .. } => {
                for c in coins {
                    match &c.icon {
                        Some(img) => self.image(img),
                        None => self.coins_without_icon += 1,
                    }
                }
            }
            RowContent::Pair { left, right } => {
                self.add(left, depth + 1);
                self.add(right, depth + 1);
            }
            RowContent::List(rows) => {
                for r in rows {
                    self.add(r, depth + 1);
                }
            }
            _ => {}
        }
    }
}

/// One displayed element of a built tooltip, as the panel audit reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    /// `UIElementID`.
    id: u32,
    /// `UIElementType` (0 for code-made elements).
    ty: u32,
    /// The displayed box (screen pixels, inclusive edges).
    bx: IBox,
    /// The base image.
    image: Option<ImageRef>,
    /// The shown text.
    text: String,
}

impl Node {
    /// Whether the element draws anything itself: a base image or a non-blank text. An
    /// element that draws nothing (an empty field of a row template) cannot show outside
    /// the frame.
    fn draws(&self) -> bool {
        self.image.is_some() || !self.text.trim().is_empty()
    }
}

/// The displayed descendants of `frame` (the frame itself excluded), at most
/// [`MAX_AUDIT_NODES`].
fn displayed_nodes(ui: &mut Ui, frame: tessera_ui::ElementKey) -> Vec<Node> {
    let mut out = Vec::new();
    let mut stack: Vec<tessera_ui::ElementKey> = ui.children(frame);
    stack.reverse();
    while let Some(k) = stack.pop() {
        if out.len() >= MAX_AUDIT_NODES {
            break;
        }
        // The frame is displayed, so a visible child of a displayed element is displayed.
        if !ui.is_visible(k) {
            continue;
        }
        let ty = ui.element_type(k).unwrap_or(0);
        out.push(Node {
            id: ui.element_id(k).unwrap_or(0),
            ty,
            bx: ui.displayed_box(k),
            image: ui.base_image(k).map(|(img, _)| img),
            text: ui.text_of(k),
        });
        let mut cs = ui.children(k);
        cs.reverse();
        stack.extend(cs);
    }
    out
}

/// Whether `inner` lies inside `outer` (inclusive edges).
fn inside(inner: IBox, outer: IBox) -> bool {
    inner.l >= outer.l && inner.t >= outer.t && inner.r <= outer.r && inner.b <= outer.b
}

/// The panel audit (module docs): the defects of a built tooltip whose frame shows at
/// `frame` with the displayed descendants `nodes`; empty when it passes.
fn audit(frame: IBox, nodes: &[Node], want: &Wanted, name_type: u32) -> Vec<String> {
    let mut defects = Vec::new();
    let outside: Vec<&Node> = nodes
        .iter()
        .filter(|n| n.draws() && n.bx.w() > 0 && n.bx.h() > 0 && !inside(n.bx, frame))
        .collect();
    if let Some(first) = outside.first() {
        defects.push(format!(
            "{} displayed elements outside the frame box {frame:?}, first id {:#x} type {:#x} \
             at {:?}",
            outside.len(),
            first.id,
            first.ty,
            first.bx
        ));
    }
    let shown = |img: &ImageRef| {
        nodes
            .iter()
            .any(|n| n.image.as_ref() == Some(img) && inside(n.bx, frame))
    };
    let missing: Vec<&ImageRef> = want.images.iter().filter(|i| !shown(i)).collect();
    if !missing.is_empty() {
        defects.push(format!(
            "{} of {} input images not shown inside the frame (first {:?})",
            missing.len(),
            want.images.len(),
            missing[0]
        ));
    }
    if want.coins_without_icon > 0 {
        defects.push(format!(
            "{} worth-row coins without an icon in the input",
            want.coins_without_icon
        ));
    }
    if want.name
        && !nodes.iter().any(|n| {
            n.ty == name_type && name_type != 0 && !n.text.trim().is_empty() && inside(n.bx, frame)
        })
    {
        defects.push("the model shows a name, the panel no ExaminationName text".to_owned());
    }
    defects
}

/// Reads the built tooltip and runs [`audit`] on it.
fn audit_built(ui: &mut Ui, built: &BuiltTooltip, want: &Wanted, name_type: u32) -> Vec<String> {
    let frame = ui.displayed_box(built.frame);
    let nodes = displayed_nodes(ui, built.frame);
    audit(frame, &nodes, want, name_type)
}

/// The index column of an item's tooltip category.
fn item_category(o: &tessera_gamedata::tooltip::RecordOutcome) -> String {
    let mut c = match &o.category {
        Ok(c) => format!("{c:?}"),
        Err(_) => "Error".to_owned(),
    };
    if !o.row_failures.is_empty() {
        let _ = write!(c, " rows={}", o.row_failures.len());
    }
    if o.category.is_ok() && !o.named {
        c.push_str(" unnamed");
    }
    c
}

/// The index column of a skill's tooltip category (`NoName` for `RenderedNoName`).
fn skill_category(o: &tessera_gamedata::tooltip::CombatOutcome) -> String {
    let mut c = match &o.category {
        Ok(CombatCategory::RenderedNoName) => "NoName".to_owned(),
        Ok(c) => format!("{c:?}"),
        Err(_) => "Error".to_owned(),
    };
    if !o.row_failures.is_empty() {
        let _ = write!(c, " rows={}", o.row_failures.len());
    }
    if o.lookup_misses > 0 {
        let _ = write!(c, " misses={}", o.lookup_misses);
    }
    c
}

/// One picture to take.
struct Shot {
    /// `item-NN` or `skill-NN`.
    name: String,
    did: Did,
    input: ExamTooltip,
    /// What the panel must show.
    want: Wanted,
}

/// Runs UI frames until the host has settled, for at most [`SETTLE_LIMIT`] of wall-clock time
/// (as the combat screens do); returns whether it settled.
fn settle(host: &mut UiHost, renderer: &mut Renderer<'_>) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < SETTLE_LIMIT {
        let _ = host.frame_without_plugins(Some(renderer));
        if host.settled() {
            let _ = host.frame_without_plugins(Some(renderer));
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    false
}

/// Renders the UI offscreen at the screen size.
fn render(host: &mut UiHost, renderer: &mut Renderer<'_>) -> RgbaImage {
    let target = renderer
        .create_offscreen_target(SCREEN.0, SCREEN.1, OffscreenFormat::Rgba8Srgb)
        .expect("offscreen target");
    let mut frame = renderer.begin_offscreen_frame(&target).expect("frame");
    frame.pass("scene", PassSpace::Screen).clear(CLEAR);
    host.draw(&mut frame, false);
    renderer.end_frame(frame).expect("end frame");
    renderer.read_pixels(&target).expect("read back")
}

/// The part of `img` inside `bx` (inclusive edges), clamped to the picture; `None` when
/// nothing of it is on the picture.
fn crop(img: &RgbaImage, bx: IBox) -> Option<goldens::Rgba> {
    let x0 = u32::try_from(bx.l.max(0)).ok()?;
    let y0 = u32::try_from(bx.t.max(0)).ok()?;
    let x1 = u32::try_from(bx.r.saturating_add(1).max(0))
        .ok()?
        .min(img.width);
    let y1 = u32::try_from(bx.b.saturating_add(1).max(0))
        .ok()?
        .min(img.height);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y1 {
        let start = ((y * img.width + x0) * 4) as usize;
        data.extend_from_slice(img.data.get(start..start + (w * 4) as usize)?);
    }
    Some(goldens::Rgba {
        width: w,
        height: h,
        data,
    })
}

/// Counts the events of the scenario played with the install: (`ItemAdded`,
/// `QuickslotChanged`, distinct quickslot contents, skipped steps).
fn play_counts(
    resolved: Arc<tessera_net::scenario::ResolvedScenario>,
) -> (usize, usize, usize, String) {
    let config = SandboxConfig {
        scenario: Some(Arc::clone(&resolved)),
        ..SandboxConfig::default()
    };
    let session = Session::new(SandboxBackend::new(config).expect("sandbox"));
    let mut driver = HeadlessDriver::new(session, resolved, Some(Vec::new()));
    let _ = driver.run();
    let log = String::from_utf8(driver.into_log().expect("log")).expect("utf-8 log");
    let mut items = 0;
    let mut slots = 0;
    let mut contents = BTreeSet::new();
    let mut end = String::new();
    for line in log.lines() {
        let mut words = line.split(' ');
        let (_, kind) = (words.next(), words.next());
        match kind {
            Some("ItemAdded") => items += 1,
            Some("QuickslotChanged") => {
                slots += 1;
                if let Some(c) = line.split(' ').find(|w| w.starts_with("content=")) {
                    contents.insert(c.to_owned());
                }
            }
            Some("end") => {
                end = line
                    .split(' ')
                    .filter(|w| w.starts_with("reason=") || w.starts_with("skipped="))
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            _ => {}
        }
    }
    (items, slots, contents.len(), end)
}

#[test]
#[allow(clippy::too_many_lines)]
fn fifty_item_and_fifty_skill_tooltips_are_painted() {
    let Some(dir) = game_dir() else { return };
    // Decision 0031 tier 2: refuse a golden folder in a work tree before any work.
    let golden_dir = goldens::golden_dir(
        env!("CARGO_PKG_NAME"),
        "parity_tooltips_real_data/fifty_item_and_fifty_skill_tooltips_are_painted",
        &goldens::install_version(&dir),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut flags = goldens::Flags::from_env();
    flags.sheet = flags.sheet.map(|s| s.join("parity_tooltips_real_data"));

    // 1. The selectors against the install, in a fixed order.
    let t0 = Instant::now();
    let bytes = std::fs::read(scenario_path()).expect("scenario");
    let scenario = Scenario::parse(&bytes).expect("parses");
    let mut resolver = GameDataResolver::open(&dir).expect("install opens");
    let resolved = Arc::new(scenario.resolve(&mut resolver).expect("resolves"));
    let items = ordered(&scenario, &resolved, Family::Item);
    let skills = ordered(&scenario, &resolved, Family::Skill);
    // Every `give` step gives every record of its binding (`scenario-feed.md` §8).
    let drawn_items: usize = scenario
        .selectors()
        .into_iter()
        .filter(|(_, _, f, _)| *f == Family::Item)
        .map(|(name, _, _, _)| resolved.binding(name).map_or(0, |b| b.len()))
        .sum();
    eprintln!(
        "resolved: {} distinct item records, {} distinct skill records ({:?})",
        items.len(),
        skills.len(),
        t0.elapsed()
    );
    let mut short = Vec::new();
    if items.len() < PER_FAMILY {
        short.push(format!("items: {} of {PER_FAMILY}", items.len()));
    }
    if skills.len() < PER_FAMILY {
        short.push(format!("skills: {} of {PER_FAMILY}", skills.len()));
    }
    assert!(short.is_empty(), "shortfall of distinct records: {short:?}");
    let items = &items[..PER_FAMILY];
    let skills = &skills[..PER_FAMILY];

    // The feed shows each record once: one bag item and one quickslot per record.
    let (added, slotted, distinct_slots, end) = play_counts(Arc::clone(&resolved));
    eprintln!(
        "played: {added} ItemAdded, {slotted} QuickslotChanged ({distinct_slots} distinct), {end}"
    );
    assert_eq!(
        added, drawn_items,
        "every drawn item record goes into the bag once"
    );
    assert_eq!(
        slotted, PER_FAMILY,
        "every skill record goes on a quickslot once"
    );
    assert_eq!(
        distinct_slots, PER_FAMILY,
        "quickslot contents are distinct"
    );
    assert!(end.contains("skipped=0"), "no step skipped: {end}");

    // 2. The hover tooltips through the client's path.
    let data = CharacterData::open(&dir).expect("character data");
    let engine = data.engine();
    let viewer = Viewer {
        level: Some(VIEWER_LEVEL),
        ..Viewer::default()
    };
    let skill_viewer = SkillViewer {
        level: Some(VIEWER_LEVEL),
        ..SkillViewer::default()
    };
    let mut failures: Vec<String> = Vec::new();
    let mut shots: Vec<Shot> = Vec::new();
    let mut index =
        String::from("# ordinal\tfamily\tDID\tcategory\tname (local only, never commit)\n");
    // Records categorised without a tooltip to paint (noted in the index, not painted).
    let mut unpainted: Vec<String> = Vec::new();
    for (n, &did) in items.iter().enumerate() {
        let name = format!("item-{:02}", n + 1);
        let shown = resolver
            .display_name(DataId(did.0), None)
            .unwrap_or_default();
        let outcome = engine.classify(did, &viewer);
        let category = item_category(&outcome);
        let _ = writeln!(index, "{:02}\titem\t{did}\t{category}\t{shown}", n + 1);
        match &outcome.category {
            Ok(Category::Rendered | Category::RowFailure) => {}
            Ok(_) => {
                unpainted.push(format!("{name} ({did}): {category}"));
                continue;
            }
            Err(e) => {
                failures.push(format!("{name} ({did}): classification error: {e}"));
                continue;
            }
        }
        let model = match engine.build(&TooltipRequest::hover(did), &viewer) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name} ({did}): did not build: {e}"));
                continue;
            }
        };
        let key = ExamKey {
            instance: 0,
            record: did.0,
        };
        match convert(&data, &model, key) {
            Some(input) => {
                let want = Wanted::of(&input.rows, outcome.named);
                shots.push(Shot {
                    name,
                    did,
                    input,
                    want,
                });
            }
            None => failures.push(format!("{name} ({did}): not converted")),
        }
    }
    for (n, &did) in skills.iter().enumerate() {
        let name = format!("skill-{:02}", n + 1);
        let shown = resolver
            .display_name(DataId(did.0), None)
            .unwrap_or_default();
        let subject = CombatSubject::Skill(did);
        let outcome = engine.classify_combat(&subject);
        let category = skill_category(&outcome);
        let _ = writeln!(index, "{:02}\tskill\t{did}\t{category}\t{shown}", n + 1);
        match &outcome.category {
            Ok(CombatCategory::Rendered | CombatCategory::RenderedNoName) => {}
            Ok(_) => {
                unpainted.push(format!("{name} ({did}): {category}"));
                continue;
            }
            Err(e) => {
                failures.push(format!("{name} ({did}): classification error: {e}"));
                continue;
            }
        }
        let model = match engine.build_combat(&CombatRequest::hover(subject), &skill_viewer) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name} ({did}): did not build: {e}"));
                continue;
            }
        };
        let key = ExamKey {
            instance: 0,
            record: did.0,
        };
        match exam_tooltip(&data, &model, key) {
            Some(input) => {
                let want = Wanted::of(&input.rows, outcome.name == NameState::Shown);
                shots.push(Shot {
                    name,
                    did,
                    input,
                    want,
                });
            }
            None => failures.push(format!("{name} ({did}): not converted")),
        }
    }
    let with_icon = |prefix: &str| {
        shots
            .iter()
            .filter(|s| s.name.starts_with(prefix) && s.input.rows.iter().any(has_icon))
            .count()
    };
    eprintln!(
        "modelled: {} tooltips ({} item, {} skill inputs with an icon or image row), {} \
         categorised without a tooltip (noted, not painted), {} failures",
        shots.len(),
        with_icon("item-"),
        with_icon("skill-"),
        unpainted.len(),
        failures.len()
    );
    for u in &unpainted {
        eprintln!("  not painted: {u}");
    }
    let index_path = golden_dir.join("index.txt");
    std::fs::create_dir_all(&golden_dir).expect("golden directory");
    std::fs::write(&index_path, index).expect("index written");
    eprintln!("index: {}", index_path.display());

    // 3. The game UI from the install at UI scale 1, painted by the GPU UI pass.
    let Some(mut renderer) = gpu::headless_renderer(CLEAR) else {
        eprintln!(
            "no GPU adapter; {} tooltips modelled, none painted",
            shots.len()
        );
        assert!(
            failures.is_empty(),
            "{} failures: {failures:?}",
            failures.len()
        );
        return;
    };
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("archive set"));
    let empty = tempfile::tempdir().expect("tempdir");
    let booted = UiBoot::spawn(set, dir.clone(), Some(empty.path().to_path_buf()))
        .expect("spawn")
        .wait()
        .expect("UI data");
    let vocab = booted.char_vocab.clone();
    let mut host = UiHost::from_boot(
        booted,
        &UiOptions {
            screen: SCREEN,
            hud: false,
            prefs: UiPrefs {
                ui_scale: UiScale::Fixed(1.0),
                ..UiPrefs::default()
            },
            ..UiOptions::default()
        },
    );
    let se = host.scale().effective_scale();
    assert!((se - 1.0).abs() < 1e-9, "effective UI scale {se}, not 1");
    let tips = ExamTooltips::new(vocab);
    let name_type = tips.vocab().types.exam_name;
    // The empty UI first, so the pictures wait only for their own tooltip.
    assert!(
        settle(&mut host, &mut renderer),
        "the empty UI did not settle"
    );

    // Warm-up (module docs): the tooltip fonts load before the first picture.
    if let Some(first) = shots.first() {
        let built = {
            let mut ui = host.ui_mut();
            tips.build(&mut ui, &first.input)
        };
        if let Ok(b) = built {
            let _ = settle(&mut host, &mut renderer);
            host.ui_mut().destroy(b.frame);
            let _ = settle(&mut host, &mut renderer);
        }
    }

    let t1 = Instant::now();
    let (mut blessed, mut matched, mut clipped, mut defective) = (0, 0, 0, 0);
    for shot in &shots {
        let built = {
            let mut ui = host.ui_mut();
            tips.build(&mut ui, &shot.input)
        };
        let built = match built {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("{} ({}): panel: {e}", shot.name, shot.did));
                continue;
            }
        };
        if !built.failures.is_empty() || built.size.0 <= 0 || built.size.1 <= 0 {
            failures.push(format!(
                "{} ({}): size {:?}, {} template failures",
                shot.name,
                shot.did,
                built.size,
                built.failures.len()
            ));
        }
        // The panel's own layout position is not (0, 0) in every layout; the picture starts
        // at the top left corner of the screen.
        host.ui_mut().move_to(built.frame, 0, 0);
        let picture = if settle(&mut host, &mut renderer) {
            let bx = host.ui().displayed_box(built.frame);
            if bx.l < 0 || bx.t < 0 || bx.r >= SCREEN.0 as i32 || bx.b >= SCREEN.1 as i32 {
                clipped += 1;
            }
            let defects = {
                let mut ui = host.ui_mut();
                audit_built(&mut ui, &built, &shot.want, name_type)
            };
            if defects.is_empty() {
                let pic = crop(&render(&mut host, &mut renderer), bx);
                if pic.is_none() {
                    failures.push(format!("{}: panel {bx:?} is off the screen", shot.name));
                }
                pic
            } else {
                // A defective picture is never blessed as a baseline (module docs).
                defective += 1;
                for d in defects {
                    failures.push(format!("{} ({}): {d}", shot.name, shot.did));
                }
                None
            }
        } else {
            failures.push(format!("{}: the UI did not settle", shot.name));
            None
        };
        host.ui_mut().destroy(built.frame);
        let Some(pic) = picture else { continue };
        match goldens::check_with_compare(&golden_dir, &shot.name, &pic, &flags, gpu_compare) {
            Ok(goldens::Outcome::Blessed(_)) => blessed += 1,
            Ok(goldens::Outcome::Passed(_)) => matched += 1,
            Err(e) => failures.push(e),
        }
    }
    eprintln!(
        "painted {} tooltips in {:?}: {blessed} blessed, {matched} passed, {defective} failed \
         the panel audit (not painted), {clipped} taller or wider than the screen; goldens in {}",
        blessed + matched,
        t1.elapsed(),
        golden_dir.display()
    );
    assert!(
        failures.is_empty(),
        "{} failures: {failures:?}",
        failures.len()
    );
    assert_eq!(clipped, 0, "tooltips cut by the screen");
    assert_eq!(
        blessed + matched + unpainted.len(),
        2 * PER_FAMILY,
        "every record painted or noted with its category"
    );
}

// Synthetic tests of the panel audit and the index columns (no install needed).

fn node(id: u32, ty: u32, bx: IBox, image: Option<ImageRef>, text: &str) -> Node {
    Node {
        id,
        ty,
        bx,
        image,
        text: text.to_owned(),
    }
}

const NAME_TYPE: u32 = 0x1000_0008;

/// A name-and-icon tooltip input like the install's: `List[List[Icon], Text]`, plus a worth
/// row with two coins.
fn sample_rows(coin_icon: Option<ImageRef>) -> Vec<ExamRow> {
    let icon = ExamRow {
        content: RowContent::Icon {
            image: Some(IconImage::Layers(vec![ImageRef::Did(1), ImageRef::Did(2)])),
            ghosted: false,
        },
        ..ExamRow::default()
    };
    let name_icon = ExamRow {
        content: RowContent::List(vec![
            ExamRow {
                content: RowContent::List(vec![icon]),
                ..ExamRow::default()
            },
            ExamRow::text("Invented name"),
        ]),
        ..ExamRow::default()
    };
    let money = ExamRow {
        content: RowContent::Money {
            label: Some("Worth".to_owned()),
            coins: vec![
                tessera_ui::game::examine::ExamCoin {
                    text: "5".to_owned(),
                    icon: coin_icon.clone(),
                },
                tessera_ui::game::examine::ExamCoin {
                    text: "48".to_owned(),
                    icon: coin_icon,
                },
            ],
        },
        ..ExamRow::default()
    };
    let hidden = ExamRow {
        content: RowContent::Image(ImageRef::Did(99)),
        hidden: true,
        ..ExamRow::default()
    };
    vec![name_icon, money, hidden]
}

#[test]
fn wanted_collects_nested_icon_layers_and_coins_but_not_hidden_rows() {
    let w = Wanted::of(&sample_rows(Some(ImageRef::Did(7))), true);
    assert_eq!(
        w.images,
        vec![ImageRef::Did(1), ImageRef::Did(2), ImageRef::Did(7)]
    );
    assert_eq!(w.coins_without_icon, 0);
    assert!(w.name);
    let w = Wanted::of(&sample_rows(None), false);
    assert_eq!(w.images, vec![ImageRef::Did(1), ImageRef::Did(2)]);
    assert_eq!(w.coins_without_icon, 2);
    assert!(!w.name);
}

#[test]
fn audit_passes_a_complete_panel() {
    let frame = IBox::from_xywh(0, 0, 239, 120);
    let want = Wanted::of(&sample_rows(Some(ImageRef::Did(7))), true);
    let nodes = [
        node(
            1,
            0,
            IBox::from_xywh(9, 13, 32, 32),
            Some(ImageRef::Did(1)),
            "",
        ),
        node(
            2,
            0,
            IBox::from_xywh(9, 13, 32, 32),
            Some(ImageRef::Did(2)),
            "",
        ),
        node(
            3,
            NAME_TYPE,
            IBox::from_xywh(45, 5, 180, 48),
            None,
            "Invented name",
        ),
        node(
            4,
            0,
            IBox::from_xywh(120, 80, 32, 22),
            Some(ImageRef::Did(7)),
            "",
        ),
        // An empty element outside the frame is not a defect.
        node(5, 0, IBox::from_xywh(400, 0, 0, 0), None, ""),
    ];
    assert_eq!(audit(frame, &nodes, &want, NAME_TYPE), Vec::<String>::new());
}

#[test]
fn audit_reports_a_missing_icon_a_missing_name_and_overflow() {
    let frame = IBox::from_xywh(0, 0, 239, 120);
    let want = Wanted::of(&sample_rows(None), true);
    // The name-and-icon row lost its icon entry and its name; the worth row's coin texts
    // run past the frame (the defects of the review findings).
    let nodes = [
        node(1, 0x1000_008a, IBox::from_xywh(5, 5, 180, 48), None, ""),
        node(2, 0, IBox::from_xywh(430, 60, 204, 16), None, "48"),
        node(3, NAME_TYPE, IBox::from_xywh(45, 5, 180, 48), None, "  "),
        // An element that draws nothing outside the frame is not a defect.
        node(4, 0, IBox::from_xywh(5, 130, 199, 17), None, ""),
    ];
    let d = audit(frame, &nodes, &want, NAME_TYPE);
    assert_eq!(d.len(), 4, "{d:?}");
    assert!(
        d[0].starts_with("1 displayed elements outside the frame"),
        "{d:?}"
    );
    assert!(d[1].starts_with("2 of 2 input images not shown"), "{d:?}");
    assert!(
        d[2].starts_with("2 worth-row coins without an icon"),
        "{d:?}"
    );
    assert!(d[3].contains("no ExaminationName text"), "{d:?}");
}

#[test]
fn audit_needs_the_image_inside_the_frame() {
    let frame = IBox::from_xywh(0, 0, 100, 50);
    let want = Wanted {
        images: vec![ImageRef::Did(1)],
        ..Wanted::default()
    };
    let nodes = [node(
        1,
        0,
        IBox::from_xywh(90, 0, 32, 32),
        Some(ImageRef::Did(1)),
        "",
    )];
    let d = audit(frame, &nodes, &want, NAME_TYPE);
    assert_eq!(d.len(), 2, "{d:?}");
    // No name type resolved: a name cannot be found, so it is reported.
    let want = Wanted {
        name: true,
        ..Wanted::default()
    };
    assert_eq!(audit(frame, &[], &want, 0).len(), 1);
}

#[test]
fn index_categories_name_row_failures_misses_and_no_name() {
    let o = tessera_gamedata::tooltip::RecordOutcome {
        category: Ok(Category::Rendered),
        row_failures: Vec::new(),
        non_empty: Vec::new(),
        deferred: 0,
        named: false,
    };
    assert_eq!(item_category(&o), "Rendered unnamed");
    let o = tessera_gamedata::tooltip::RecordOutcome {
        category: Ok(Category::NoTooltip),
        named: true,
        ..o
    };
    assert_eq!(item_category(&o), "NoTooltip");
    let o = tessera_gamedata::tooltip::CombatOutcome {
        category: Ok(CombatCategory::RenderedNoName),
        name: NameState::Empty,
        row_failures: Vec::new(),
        non_empty: Vec::new(),
        deferred: 0,
        lookup_misses: 2,
        icon: None,
        quickslot_icon: None,
    };
    assert_eq!(skill_category(&o), "NoName misses=2");
}
