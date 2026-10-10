// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data test of the character screens (`docs/specs/character-screens.md` §9, decision
//! 0024 P1 exit criterion 1) against the user's own install at `TESSERA_GAME_DIR`,
//! read-only:
//!
//! 1. every race, gender entry and class of the install is offered by the screen model and
//!    accepted by `player-character.md` §5.4;
//! 2. for each (race, gender) the appearance-control list builds and the default figure
//!    assembles through the walker's figure path (with a randomised figure through the
//!    assembler), failures reported by category;
//! 3. for each (race, gender, nationality) a random name is generated through the screen,
//!    or the generator's empty result is reported;
//! 4. a headless scenario run per (race, gender) creates a character with the screen's
//!    command and enters the world with it;
//! 5. against Tessera's test server run in process: with `character_creation` on, one
//!    creation per race yields the character with the command's body and its world entry
//!    succeeds, a second creation with the same name is refused, and a second client sees
//!    the created look; with it off the New button is ghosted and nothing is sent.
//!
//! Skipped (reported as passed) without `TESSERA_GAME_DIR`; step 5 also without loopback
//! UDP. Prints counts and keys only, never game text or generated names. Use `--release`.

#[path = "character_screens_common.rs"]
mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use common::Driver;
use tessera_client::screens::model::Stage;
use tessera_client::screens::viewport::{FigureChoice, character_spec, figure_request};
use tessera_client::screens::{
    Action, ButtonRule, CharacterScreens, ScreenData, ScreenModel, ScreenOptions, Status,
    viewport_state,
};
use tessera_client::walker::characters::catalog::CharacterCatalog;
use tessera_client::walker::characters::figure::{FigureContext, build_figure};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::chargen::{ChoiceVerdict, CreationChoice};
use tessera_mesh::assembly::{
    AssemblyVocabulary, CharacterAssembler, DiagnosticCategory, GameDataSource,
};
use tessera_net::domain::CharacterCommand;
use tessera_net::sandbox::{SandboxBackend, SandboxConfig};
use tessera_net::scenario::{EmptyResolver, Scenario};
use tessera_net::{Body, Command, Event, Look, Session};
use tessera_text::Strings;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

struct Env {
    dir: PathBuf,
    store: GameDataStore,
    data: Arc<ScreenData>,
}

fn env() -> Option<Env> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping the character-screen real-data test");
        return None;
    };
    let store = GameDataStore::open(&dir).expect("install opens");
    let strings = Strings::open_install(&dir, None).expect("strings open");
    let data = Arc::new(ScreenData::load(&store, Some(&strings)).expect("screen data loads"));
    Some(Env { dir, store, data })
}

/// The (species, sex) pairs of the install, in screen order.
fn bodies(data: &ScreenData) -> Vec<(u32, u32)> {
    data.chargen
        .races
        .iter()
        .flat_map(|r| r.genders.iter().map(move |g| (r.species(), g.sex)))
        .collect()
}

fn connected(m: &mut ScreenModel, names: &[&str]) {
    m.handle_event(
        &Event::Connected {
            characters: names
                .iter()
                .map(|n| tessera_net::CharacterSummary {
                    name: (*n).to_owned(),
                })
                .collect(),
        },
        std::time::Instant::now(),
    );
}

/// A name the install's check accepts, from the screen's random-name button (seeds tried
/// in order); `None` when the generator gives none.
fn accepted_name(m: &mut ScreenModel, seed: u64) -> Option<String> {
    let now = std::time::Instant::now();
    for s in 0..64 {
        m.apply(
            Action::RandomName(seed.wrapping_mul(1_000).wrapping_add(s)),
            now,
        );
        let n = m.creation().name.clone();
        if !n.is_empty() && m.check_name(&n).is_ok() {
            return Some(n);
        }
    }
    None
}

#[test]
fn steps_1_to_3_through_the_screen_model() {
    let Some(env) = env() else {
        return;
    };
    let data = &env.data;
    let now = std::time::Instant::now();
    let mut m = ScreenModel::new(
        Arc::clone(data),
        tessera_client::screens::BackendKind::PlainSandbox,
    );
    connected(&mut m, &["Wanderer"]);
    m.apply(Action::New, now);
    assert_eq!(m.stage(), Stage::RaceClass);

    // Step 1.
    let mut offered = 0usize;
    let mut genders = 0usize;
    for r in &data.chargen.races {
        let species = r.species();
        m.apply(Action::Race(species), now);
        assert_eq!(m.creation().species, Some(species));
        for g in &r.genders {
            m.apply(Action::Gender(g.sex), now);
            assert_eq!(m.creation().sex, Some(g.sex), "{species:#x}/{:#x}", g.sex);
            genders += 1;
            for class in data.chargen.class_order(species) {
                m.apply(Action::Class(class), now);
                assert_eq!(m.creation().class, Some(class));
                assert!(m.continue_button().is_enabled());
                let nationality = data
                    .chargen
                    .default_nationality(species)
                    .expect("nationality");
                assert_eq!(
                    data.chargen.check(&CreationChoice {
                        species,
                        sex: g.sex,
                        nationality,
                        class
                    }),
                    ChoiceVerdict::Valid
                );
                offered += 1;
            }
        }
    }
    eprintln!(
        "step 1: {} races, {genders} gender entries, {offered} (race, gender, class) choices offered and valid",
        data.chargen.races.len()
    );
    assert_eq!(offered, data.chargen.offered().len());
    assert!(
        data.chargen.problems().is_empty(),
        "{:?}",
        data.chargen.problems()
    );

    // Step 2 (lists) and step 3.
    let mut names = (0usize, 0usize, 0usize);
    let mut notes = 0usize;
    for (species, sex) in bodies(data) {
        if m.stage() == Stage::Appearance {
            m.apply(Action::Back, now);
        }
        m.apply(Action::Race(species), now);
        m.apply(Action::Gender(sex), now);
        m.apply(Action::Continue, now);
        assert_eq!(m.stage(), Stage::Appearance, "{species:#x}/{sex:#x}");
        let d = m.current_avatar().expect("control details");
        assert!(!d.controls.is_empty(), "CSI4 {species:#x}/{sex:#x}");
        notes += d.notes.len();
        let writable = d.writable().len();
        let kinds: BTreeMap<String, usize> = d.controls.iter().fold(BTreeMap::new(), |mut a, c| {
            *a.entry(format!("{:?}", std::mem::discriminant(&c.choices)))
                .or_default() += 1;
            a
        });
        eprintln!(
            "step 2: {species:#x}/{sex:#x}: {} controls, {writable} written properties, {} kinds, {} notes",
            d.controls.len(),
            kinds.len(),
            d.notes.len()
        );
        // Randomised choices make a valid look within the protocol's limits.
        m.apply(
            Action::Randomise(u64::from(species) << 16 | u64::from(sex)),
            now,
        );
        if let Some(look) = m.creation().appearance.look(m.current_avatar().unwrap()) {
            assert!(look.is_valid());
        }
        for (nat, _) in data.chargen.nationalities_for(species) {
            m.apply(Action::Nationality(nat), now);
            let seed = u64::from(species) << 32 | u64::from(sex) << 8 | u64::from(nat);
            m.apply(Action::Name(String::new()), now);
            m.apply(Action::RandomName(seed), now);
            let n = m.creation().name.clone();
            if n.is_empty() {
                names.2 += 1;
                eprintln!("step 3: {species:#x}/{sex:#x}/{nat:#x}: the generator gave no name");
            } else if m.check_name(&n).is_ok() {
                names.0 += 1;
            } else {
                names.1 += 1;
                eprintln!(
                    "step 3: {species:#x}/{sex:#x}/{nat:#x}: the generated name fails the check"
                );
            }
        }
    }
    eprintln!(
        "step 3: names {} accepted, {} not accepted, {} empty; {notes} control notes",
        names.0, names.1, names.2
    );
    assert!(names.0 > 0, "the generator produces names");
}

#[test]
fn step_2_figures_through_the_walker_path() {
    let Some(env) = env() else {
        return;
    };
    let data = &env.data;
    let set = Arc::new(ArchiveSet::open(&env.dir, SetOptions::default()).expect("archives"));
    let catalog = Arc::new(CharacterCatalog::build(Arc::clone(&set)));
    let ctx = FigureContext::standalone(
        Arc::clone(&set),
        Arc::clone(&catalog),
        tessera_render::MeshConfig::default(),
    );
    let gd = env.store.object_store().game_data();
    let vocab = AssemblyVocabulary::from_game_data(gd).expect("vocabulary");
    let mut asm = CharacterAssembler::new(GameDataSource::new(gd).expect("source"), vocab);
    let now = std::time::Instant::now();
    let mut m = ScreenModel::new(
        Arc::clone(data),
        tessera_client::screens::BackendKind::PlainSandbox,
    );
    connected(&mut m, &["Wanderer"]);
    m.apply(Action::New, now);
    let mut categories: BTreeMap<DiagnosticCategory, usize> = BTreeMap::new();
    let (mut figures, mut entities, mut entity_failures) = (0usize, 0usize, 0usize);
    for (species, sex) in bodies(data) {
        if m.stage() == Stage::Appearance {
            m.apply(Action::Back, now);
        }
        m.apply(Action::Race(species), now);
        m.apply(Action::Gender(sex), now);
        // The race screen's figure: the gender entry's race-select entity.
        let v = viewport_state(&m);
        assert!(v.scene.is_some(), "a UI scene for {species:#x}");
        assert!(v.camera.is_some(), "a camera for {species:#x}");
        if let FigureChoice::Entity(_) = v.figure {
            let req = figure_request(&v, data).expect("entity request");
            match build_figure(&ctx, &req, &|| false) {
                Ok(f) => {
                    entities += 1;
                    eprintln!(
                        "step 2: {species:#x}/{sex:#x} race-select entity: {} parts, {} notes",
                        f.parts.len(),
                        f.notes.len()
                    );
                }
                Err(e) => {
                    entity_failures += 1;
                    eprintln!(
                        "step 2: {species:#x}/{sex:#x} race-select entity does not build: {e}"
                    );
                }
            }
        }
        m.apply(Action::Continue, now);
        // The default figure through the walker's figure path.
        let v = viewport_state(&m);
        let req = figure_request(&v, data).expect("character request");
        assert_eq!(req.body, Body::Character { species, sex });
        let f = build_figure(&ctx, &req, &|| false)
            .unwrap_or_else(|e| panic!("default figure {species:#x}/{sex:#x}: {e}"));
        assert!(
            !f.parts.is_empty(),
            "figure {species:#x}/{sex:#x} has parts"
        );
        // A randomised figure through the assembler (styles, sliders, heights).
        m.apply(Action::Randomise(u64::from(species) ^ u64::from(sex)), now);
        // Every set value of a control that writes a property enters the look (head
        // styles write the float `AC_Mesh_Head`: the chosen block's variant), and each
        // style's modifier selects its own block.
        if let Some(detail) = m.current_avatar() {
            let values = &m.creation().appearance.values;
            for (i, v) in values {
                let c = detail.control(*i).expect("a control");
                if c.writes.is_some() {
                    assert!(
                        c.look_value(v).is_some(),
                        "{species:#x}/{sex:#x} control {i} ({:?}) has no look value",
                        c.writes
                    );
                }
                if let tessera_client::screens::ControlValue::Style(s) = v {
                    let modifier = c.style_variant(*s).expect("a variant");
                    if let tessera_client::screens::controls::ControlChoices::Styles {
                        variants,
                        ..
                    } = &c.choices
                    {
                        assert_eq!(
                            tessera_client::screens::controls::style_for_modifier(
                                variants, modifier
                            ),
                            Some(*s)
                        );
                    }
                }
            }
        }
        let v = viewport_state(&m);
        let spec = character_spec(&v, data).expect("spec");
        let c = asm
            .assemble_character(&spec)
            .unwrap_or_else(|e| panic!("randomised figure {species:#x}/{sex:#x}: {e}"));
        assert!(!c.parts.is_empty());
        for d in &c.diagnostics {
            *categories.entry(d.category).or_default() += 1;
        }
        eprintln!(
            "step 2: {species:#x}/{sex:#x}: figure {} parts in {:.0} ms, {} notes; randomised: {} styles, {} parts",
            f.parts.len(),
            f.build_ms,
            f.notes.len(),
            spec.styles.len(),
            c.parts.len()
        );
        figures += 1;
    }
    eprintln!(
        "step 2: {figures} default figures, {entities} race-select entities ({entity_failures} failed); diagnostics by category: {categories:?}"
    );
    assert_eq!(figures, bodies(data).len());
}

/// A scenario that accepts `name` with the command's look and the given body.
fn acceptance_scenario(
    name: &str,
    species: u32,
    sex: u32,
) -> Arc<tessera_net::scenario::ResolvedScenario> {
    let text = format!(
        "tessera-scenario 1\ntick 100ms\nenter \"{name}\"\non create-character name=\"{name}\"\n+0s accept-character \"{name}\" level=1 species={species} sex={sex} look=asked\nend\n"
    );
    let s = Scenario::parse(text.as_bytes()).expect("parses");
    Arc::new(s.resolve(&mut EmptyResolver).expect("resolves"))
}

#[test]
fn step_4_headless_scenario_creates_and_enters() {
    let Some(env) = env() else {
        return;
    };
    let data = &env.data;
    let mut created = 0usize;
    for (i, (species, sex)) in bodies(data).into_iter().enumerate() {
        // The name comes from the screen's generator (checked); a fixed fallback otherwise.
        let mut probe = ScreenModel::new(
            Arc::clone(data),
            tessera_client::screens::BackendKind::PlainSandbox,
        );
        connected(&mut probe, &["Wanderer"]);
        probe.apply(Action::New, std::time::Instant::now());
        probe.apply(Action::Race(species), std::time::Instant::now());
        probe.apply(Action::Gender(sex), std::time::Instant::now());
        probe.apply(Action::Continue, std::time::Instant::now());
        let name = accepted_name(&mut probe, i as u64).unwrap_or_else(|| "Tessera".to_owned());

        let session = Session::new(
            SandboxBackend::new(SandboxConfig {
                scenario: Some(acceptance_scenario(&name, species, sex)),
                ..SandboxConfig::default()
            })
            .expect("sandbox"),
        );
        let screens = CharacterScreens::new(&session, Arc::clone(data), ScreenOptions::default());
        let mut d = Driver::new(session, screens);
        d.until("connected", 20, |d| {
            d.session.state() == tessera_net::SessionState::Connected
        });
        d.act(Action::New);
        d.act(Action::Race(species));
        d.act(Action::Gender(sex));
        d.act(Action::Continue);
        d.act(Action::Randomise(i as u64 + 1));
        d.act(Action::Name(name.clone()));
        let sent_look = d
            .screens
            .model()
            .current_avatar()
            .and_then(|a| d.screens.model().creation().appearance.look(a));
        d.act(Action::Create);
        d.until("the creation", 20, |d| {
            d.screens.model().stage() == Stage::Select
        });
        let row = d.screens.model().selected_row().expect("selected").clone();
        assert_eq!(row.name, name);
        assert_eq!(row.body, Some(Body::Character { species, sex }));
        assert_eq!(row.look, sent_look, "look=asked copies the command's look");
        d.act(Action::Enter);
        d.until("world entry", 50, |d| {
            matches!(d.screens.status(), Status::Entered(_))
        });
        let appearance = d.events.iter().find_map(|e| match e {
            Event::AppearanceChanged { appearance, .. } => Some(appearance.clone()),
            _ => None,
        });
        let a = appearance.expect("the player's base appearance");
        assert_eq!(a.body, Body::Character { species, sex });
        assert_eq!(a.look.as_deref().cloned(), sent_look);
        eprintln!(
            "step 4: {species:#x}/{sex:#x}: created and entered ({} look entries)",
            sent_look.as_ref().map_or(0, |l| l.entries.len())
        );
        created += 1;
    }
    assert_eq!(created, bodies(data).len());
}

// ----- step 5: the test server ------------------------------------------------------------

mod server {
    use std::time::Duration;

    use tessera_server::accounts::{self, HashParams};
    use tessera_server::characters::{CharacterFile, CharacterRecord, StoredAppearance};
    use tessera_server::{Config, Content, DataDir, Server, ServerHandle, ServerOptions};

    pub const PASSWORD: &str = "correct horse battery";

    pub struct TestServer {
        pub handle: Option<ServerHandle>,
        _dir: tempfile::TempDir,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(h) = self.handle.take() {
                h.shutdown(Duration::ZERO);
            }
        }
    }

    impl TestServer {
        pub fn handle(&self) -> &ServerHandle {
            self.handle.as_ref().expect("running")
        }
    }

    /// A server with accounts `maker` (no characters) and `watcher` (character Watcher),
    /// or `None` without loopback UDP.
    pub fn start(creation: bool) -> Option<TestServer> {
        if let Err(e) = std::net::UdpSocket::bind("127.0.0.1:0") {
            eprintln!("SKIPPED: no loopback UDP ({e})");
            return None;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let data = DataDir::new(dir.path().join("data"));
        let mut config = Config::default();
        config.server.listen = vec!["127.0.0.1:0".parse().unwrap()];
        config.server.character_creation = creation;
        config.use_hash_floor();
        config.accounts.hash_workers = 2;
        config.accounts.attempts_per_address_per_min = 1_000;
        config.log.stats_interval_s = 0;
        data.create().expect("data dir");
        accounts::add_accounts(
            &data,
            HashParams::from_config(&config.accounts),
            &[("maker", PASSWORD), ("watcher", PASSWORD)],
        )
        .expect("accounts");
        std::fs::create_dir_all(data.characters_dir()).expect("characters dir");
        let file = CharacterFile {
            account: "watcher".into(),
            characters: vec![CharacterRecord {
                name: "Watcher".into(),
                created: "2026-10-10T00:00:00Z".into(),
                instance: 0,
                position: None,
                appearance: StoredAppearance::Valid(tessera_protocol::types::Appearance::new(
                    tessera_protocol::types::Body::Character {
                        species: 23,
                        sex: 0x1000,
                    },
                )),
                look: tessera_protocol::types::Look::none(),
            }],
        };
        std::fs::write(data.character_path("watcher"), file.to_toml()).expect("character file");
        let handle = Server::start(ServerOptions {
            data_dir: data,
            config,
            content: Content::default_world(),
        })
        .expect("the test server starts");
        Some(TestServer {
            handle: Some(handle),
            _dir: dir,
        })
    }
}

fn remote(server: &server::TestServer, account: &str) -> Session {
    let config = tessera_net::remote::RemoteConfig::new(
        &server.handle().connect_string(),
        account,
        tessera_net::remote::Password::new(server::PASSWORD),
    )
    .expect("config");
    tessera_net::remote::session(config).expect("session")
}

const STEP: Duration = Duration::from_secs(20);

#[test]
fn step_5_test_server_creation_and_looks() {
    let Some(env) = env() else {
        return;
    };
    let data = &env.data;
    let Some(server) = server::start(true) else {
        return;
    };
    // The watcher enters the world first, to see the created character arrive.
    let mut watcher = remote(&server, "watcher");
    let mut wevents = Vec::new();
    let now = std::time::Instant::now();
    watcher.send(now, Command::Connect).expect("connect");
    let deadline = std::time::Instant::now() + STEP;
    let mut entered = false;
    while !entered {
        assert!(std::time::Instant::now() < deadline, "watcher entry");
        let now = std::time::Instant::now();
        for e in watcher.poll(now) {
            if let Event::Connected { .. } = e {
                watcher
                    .send(
                        now,
                        Command::EnterWorld {
                            character: "Watcher".into(),
                        },
                    )
                    .expect("enter");
            }
            entered |= matches!(e, Event::WorldEntered { .. });
            wevents.push(e);
        }
        std::thread::sleep(common::FRAME);
    }

    let session = remote(&server, "maker");
    let screens = CharacterScreens::new(
        &session,
        Arc::clone(data),
        ScreenOptions {
            server_key: Some("in-process".into()),
            settings: None,
        },
    );
    let mut d = Driver::new(session, screens);
    d.until_real("connected", STEP, |d| {
        d.session.state() == tessera_net::SessionState::Connected
    });
    assert!(
        d.screens.model().new_button().is_enabled(),
        "CHARACTER_CREATION advertised"
    );

    // One (race, gender) per race: the first gender entry.
    let mut first_name = None;
    let mut first_look: Option<Look> = None;
    for (i, r) in data.chargen.races.iter().enumerate() {
        let species = r.species();
        let sex = r.genders.first().expect("CSI2").sex;
        d.act(Action::New);
        d.act(Action::Race(species));
        d.act(Action::Gender(sex));
        d.act(Action::Continue);
        d.act(Action::Randomise(0x5EED + i as u64));
        let mut probe = d.screens.model().clone();
        let name = accepted_name(&mut probe, 77 + i as u64)
            .unwrap_or_else(|| format!("Maker{}", char::from(b'a' + i as u8)));
        d.act(Action::Name(name.clone()));
        let look = d
            .screens
            .model()
            .current_avatar()
            .and_then(|a| d.screens.model().creation().appearance.look(a));
        d.act(Action::Create);
        d.until_real("the creation", STEP, |d| {
            d.screens.model().pending_creation().is_none()
        });
        let m = d.screens.model();
        assert_eq!(
            m.stage(),
            Stage::Select,
            "created {species:#x}: {:?}",
            m.error_texts()
        );
        let row = m.selected_row().unwrap();
        assert_eq!(row.name, name);
        assert_eq!(row.body, Some(Body::Character { species, sex }));
        eprintln!(
            "step 5: {species:#x}/{sex:#x} created on the test server ({} look entries)",
            look.as_ref().map_or(0, |l| l.entries.len())
        );
        if first_name.is_none() {
            first_name = Some(name);
            first_look = look;
        }
    }
    let name = first_name.expect("a race");

    // The same name again: refused (NAME_TAKEN, §7.1.1 step 4).
    let r = &data.chargen.races[0];
    d.act(Action::New);
    d.act(Action::Race(r.species()));
    d.act(Action::Continue);
    d.act(Action::Name(name.clone()));
    let errors_before = d.screens.model().error_texts().len();
    d.act(Action::Create);
    d.until_real("the refusal", STEP, |d| {
        d.screens.model().pending_creation().is_none()
    });
    let m = d.screens.model();
    assert_eq!(m.stage(), Stage::Appearance, "refused: the screen stays");
    assert!(m.error_texts().len() > errors_before, "a refusal line");
    eprintln!("step 5: the second creation with the same name was refused");

    // Enter with the first created character.
    d.act(Action::Back);
    d.act(Action::Back);
    let i = d
        .screens
        .model()
        .rows()
        .iter()
        .position(|r| r.name == name)
        .unwrap();
    d.act(Action::Select(i));
    d.act(Action::Enter);
    d.until_real("world entry", STEP, |d| {
        matches!(d.screens.status(), Status::Entered(_))
    });

    // The watcher sees the created character with its look (protocol 1.1, decision 0025).
    let deadline = std::time::Instant::now() + STEP;
    let mut seen = None;
    while seen.is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the watcher sees the new character: {:?}",
            wevents.iter().map(Event::name).collect::<Vec<_>>()
        );
        let now = std::time::Instant::now();
        d.now = now;
        d.screens.pump(&mut d.session, now, &mut d.events);
        wevents.extend(watcher.poll(now));
        // The created character's appearance: from its `EntityCreated`, or a later
        // `AppearanceChanged` of that entity.
        let mut id = None;
        for e in &wevents {
            match e {
                Event::EntityCreated {
                    id: i,
                    name: n,
                    appearance,
                    ..
                } if *n == name => {
                    id = Some(*i);
                    seen = Some(appearance.clone());
                }
                Event::AppearanceChanged { id: i, appearance } if Some(*i) == id => {
                    seen = Some(appearance.clone());
                }
                _ => {}
            }
        }
        std::thread::sleep(common::FRAME);
    }
    let seen = seen.unwrap();
    assert!(
        first_look.as_ref().is_some_and(|l| !l.entries.is_empty()),
        "the randomised creation has a look to compare"
    );
    assert_eq!(
        seen.look.as_deref().cloned(),
        first_look,
        "the other client sees the look"
    );
    eprintln!(
        "step 5: the second client sees the look ({} entries)",
        seen.look.as_ref().map_or(0, |l| l.entries.len())
    );
    drop(server);

    // character_creation off: New ghosted, nothing sent.
    let Some(off) = server::start(false) else {
        return;
    };
    let session = remote(&off, "maker");
    let screens = CharacterScreens::new(&session, Arc::clone(data), ScreenOptions::default());
    let mut d = Driver::new(session, screens);
    d.until_real("connected", STEP, |d| {
        d.session.state() == tessera_net::SessionState::Connected
    });
    assert!(matches!(
        d.screens.model().new_button(),
        ButtonRule::Ghosted(Some(_))
    ));
    d.act(Action::New);
    assert_eq!(d.screens.model().stage(), Stage::Select);
    let mut probe = d.screens.model().clone();
    assert!(probe.take_commands().is_empty(), "nothing sent");
    // The command itself is refused by the backend too (§7.1.1 step 2).
    let refused = d.session.send(
        std::time::Instant::now(),
        Command::Character(CharacterCommand::CreateCharacter {
            name: "Nobody".into(),
            appearance: Some(Box::new(tessera_net::Appearance::new(Body::Character {
                species: data.chargen.races[0].species(),
                sex: data.chargen.races[0].genders[0].sex,
            }))),
            class: None,
            nationality: None,
        }),
    );
    assert!(refused.is_err());
    eprintln!("step 5: with character_creation off New is ghosted and nothing is sent");
}
