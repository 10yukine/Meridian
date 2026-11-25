// game-module-cfg: target_os = "android"
// game-module-visibility: pub
//! Meridian's launcher on Android (`docs/launcher-spec.md`, "Hand-off"): the launcher screens
//! (`horizon_client::mobile_launcher`) in a process of their own, before any game starts. Play
//! writes the hand-off (`launcher_backend::launch`); the platform then starts the app again, and
//! that process plays.
use super::*;
use horizon_client::launcher_backend::{
    Account, ImportKind, Real, Setup, Stage, VANILLA, controls_art, diagnostics, files,
    launch::{self, Launch},
    profile_dir,
};
use horizon_client::mobile_launcher::{Action, MobileLauncher, SafeInsets};
use horizon_client::ui_theme::Ui;
use horizon_client_render::android_runtime;
use horizon_client_render::gamepad::{Button, PadState};
use winit::event::{MouseButton, Touch, TouchPhase};

/// The game's arguments from Settings as the game reads them (`Options::validate`): one it
/// doesn't take stops Play here, with the reason, instead of the game closing as it starts.
fn check_game_args(args: &[String]) -> std::result::Result<(), String> {
    if args.is_empty() {
        return Ok(());
    }
    let mut command = std::process::Command::new("horizon");
    command.args(args);
    let launch = horizon_mod_package::selection::LaunchArguments::for_command(&command)
        .map_err(|e| e.to_string())?;
    crate::game::Options::validate(launch)
        .map(drop)
        .map_err(|e| e.to_string())
}

/// How fast a controller moves the pointer at full tilt, in window pixels a second.
const PAD_SPEED: f64 = 1100.0;
/// Stick tilt below this doesn't move the pointer.
const PAD_DEADZONE: f32 = 0.25;
/// A brief grace for a nearly finished cache. A longer wait costs more than an uncached start.
const MAX_FILE_WAIT: Duration = Duration::from_secs(1);
/// How far a finger moves, in UI pixels, before it scrolls instead of tapping (Android's touch
/// slop is 8 dp).
const DRAG: f64 = 8.0;

/// The finger followed: where it went down and last was (physical pixels), and whether it has
/// started scrolling, so it doesn't tap where it lifts.
struct Finger {
    id: u64,
    down: (f64, f64),
    last: (f64, f64),
    dragging: bool,
}

enum GameFiles {
    Pending,
    Writing(std::thread::JoinHandle<()>),
    Done,
}

enum FileTransferKind {
    Import,
    Export,
}

struct PendingImport {
    kind: ImportKind,
    dest: &'static str,
    profile_dir: std::path::PathBuf,
}

struct FileTransferJob {
    kind: FileTransferKind,
    job: std::thread::JoinHandle<std::result::Result<String, String>>,
}

struct Host {
    root: std::path::PathBuf,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui: UiResources,
    screen: MobileLauncher,
    /// The pointer (or finger) in physical pixels.
    pointer: (f64, f64),
    finger: Option<Finger>,
    /// The phone's keyboard is up for a text field.
    keyboard: bool,
    /// How far the keyboard covers the window, in pixels (polled: the window doesn't shrink).
    ime: u32,
    /// The display cutout's insets (left, top, right, bottom) in pixels, as last given to the
    /// launcher: the window reaches under a notch, the controls stay clear of it. `None` until
    /// the first frame gives them.
    cutout: Option<[u32; 4]>,
    /// The controller last time, and when the pointer last moved by it: its cursor shows while
    /// it's in use, a finger hides it.
    pad: PadState,
    pad_used: Option<Instant>,
    last_tick: Instant,
    /// Input (or a change) since the last frame: the next one draws at once.
    dirty: bool,
    /// When the next frame is due without input.
    next_frame: Instant,
    settings: Settings,
    /// Play was pressed and the hand-off written.
    play: bool,
    game_files: GameFiles,
    mod_compile: Option<std::thread::JoinHandle<()>>,
    mod_warmed_profile: Option<String>,
    diagnostics_export:
        Option<std::thread::JoinHandle<std::result::Result<std::path::PathBuf, String>>>,
    pending_import: Option<PendingImport>,
    file_transfer: Option<FileTransferJob>,
    play_requested_at: Option<Instant>,
    started: Instant,
    shown: bool,
    error: Option<Box<dyn std::error::Error>>,
}

/// Shows the launcher in `root` (the app's folder) until the player plays or leaves: true when
/// the hand-off for the game is written.
pub fn run(root: &std::path::Path) -> Result<bool> {
    let started = Instant::now();
    horizon_client::spans::startup::init(started);
    // Horizon's look, as the game's start gives it (`entry::start`): with it the design
    // system's fonts, built in, so the launcher's text shows before any game files exist.
    crate::entry::horizon_look::give();
    let settings = Settings::load(&Game::options_path());
    // The player's language, from the strings compiled in: there may be no prepared ones yet.
    horizon_client::lang::set(horizon_client::lang::Language::load(
        &[],
        &settings.language,
    ));
    let backend = Real::new(root.to_path_buf());
    eprintln!("Meridian: launcher backend after {} ms", started.elapsed().as_millis());
    let mut screen = MobileLauncher::new(Box::new(backend));
    if launch::closed_unexpectedly(root) {
        screen.note(horizon_client::launcher_backend::i18n::tr(
            "The game closed unexpectedly last time.",
        ));
    }
    // Text and fills are all the launcher draws: no game textures, unless its pixel font or a
    // non-Geist language needs the prepared pack's font sheets (Unifont).
    let gui = UiResources {
        atlas: if screen.needs_full_atlas() {
            full_atlas(root).unwrap_or_else(|_| HudAtlas::launcher())
        } else {
            HudAtlas::launcher()
        },
        panorama: Arc::new(horizon_client::gfx::panorama::Faces::colour([0; 4])),
        splashes: Arc::from(""),
    };
    eprintln!("Meridian: launcher atlas after {} ms", started.elapsed().as_millis());
    let mut host = Host {
        root: root.to_path_buf(),
        window: None,
        renderer: None,
        gui,
        screen,
        pointer: (0.0, 0.0),
        finger: None,
        keyboard: false,
        ime: 0,
        cutout: None,
        pad: PadState::default(),
        pad_used: None,
        last_tick: Instant::now(),
        dirty: true,
        next_frame: Instant::now(),
        settings,
        play: false,
        game_files: GameFiles::Pending,
        mod_compile: None,
        mod_warmed_profile: None,
        diagnostics_export: None,
        pending_import: None,
        file_transfer: None,
        play_requested_at: None,
        started,
        shown: false,
        error: None,
    };
    super::event_loop::new_event_loop()?.run_app(&mut host)?;
    host.error.map_or(Ok(host.play), Err)
}

/// The atlas of the prepared game files, the pack's font sheets in it.
fn full_atlas(root: &std::path::Path) -> Result<HudAtlas> {
    Ok(HudAtlas::build(
        &horizon_assets::resource_source::ResourceSource::directory(launch::resources_dir(root)),
    )?)
}

impl Host {
    fn size(&self) -> [u32; 2] {
        self.renderer
            .as_ref()
            .map_or([1280, 720], |r| [r.config.width, r.config.height])
    }

    /// A batch of the window's size; while the keyboard is up, the screens lay out above it.
    fn batch(&self) -> HudBatch {
        let size = self.size();
        let mut batch = HudBatch::new(size, self.settings.gui_scale_inputs());
        if self.ime > 0 {
            batch.height = (size[1].saturating_sub(self.ime) as f32 / batch.scale)
                .floor()
                .max(1.0);
        }
        batch
    }

    /// `f` over a Ui of the visible area, the cursor where the pointer is.
    fn with_ui<R>(&mut self, f: impl FnOnce(&mut MobileLauncher, &mut Ui) -> R) -> R {
        let mut batch = self.batch();
        let cursor = (
            self.pointer.0 as f32 / batch.scale,
            self.pointer.1 as f32 / batch.scale,
        );
        let mut ui = Ui::new(&self.gui.atlas, &mut batch, cursor);
        let result = f(&mut self.screen, &mut ui);
        ui.finish();
        result
    }

    fn draw(&mut self) {
        if self.renderer.is_none() {
            return;
        }
        let mut batch = self.batch();
        {
            let cursor = (
                self.pointer.0 as f32 / batch.scale,
                self.pointer.1 as f32 / batch.scale,
            );
            let mut ui = Ui::new(&self.gui.atlas, &mut batch, cursor);
            self.screen.draw(&mut ui);
            ui.finish();
        }
        if self.pad_used.is_some() {
            // The controller's pointer: a white square in a dark frame.
            let (x, y) = (
                self.pointer.0 as f32 / batch.scale,
                self.pointer.1 as f32 / batch.scale,
            );
            for (r, color) in [(4.0, 0xFF08_020F), (2.5, 0xFFFF_FFFF)] {
                batch.fill_quad(
                    &self.gui.atlas,
                    [[x - r, y - r], [x + r, y - r], [x + r, y + r], [x - r, y + r]],
                    color,
                );
            }
        }
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let mut frame = loading_frame(&batch);
        // A launcher pop-up blurs the page behind it (`MobileLauncher` marks where: `blur_at`), at
        // vanilla's default menu blur.
        if batch.blur_at > 0 {
            frame.menu_blur = Some(5.0);
        }
        match renderer.render(&frame) {
            Ok(true) => {
                if !std::mem::replace(&mut self.shown, true) {
                    eprintln!(
                        "Meridian: launcher shown after {} ms",
                        self.started.elapsed().as_millis()
                    );
                }
            }
            // Not shown (the first frame often isn't, nor one in the background): the glyphs
            // it rasterized never reached the atlas, so draw them again next frame.
            Ok(false) => horizon_client::ui_theme::reset_atlas(),
            Err(e) => eprintln!("Meridian: launcher frame failed: {e}"),
        }
    }

    fn click(&mut self, event_loop: &ActiveEventLoop) {
        if self.play_requested_at.is_some() {
            return;
        }
        match self.with_ui(|screen, ui| screen.click(ui)) {
            Some(Action::Play) => {
                if let Err(error) = check_game_args(&launch::game_args(&self.root)) {
                    self.screen.game_args_refused(error);
                    return;
                }
                let profile_dir = horizon_client::launcher_backend::profile_dir(
                    &self.root,
                    &self.screen.backend().selected_profile(),
                );
                launch::seed_builtin_mods(&self.root, &profile_dir);
                self.prepare_game_files();
                if self.mod_compile.is_some() {
                    self.screen.set_compiling_mods(true);
                    self.screen.note(horizon_client::launcher_backend::i18n::tr(
                        "Compiling mods…",
                    ));
                } else if android_runtime::mods_need_compile(&profile_dir) {
                    let compile_dir = profile_dir.clone();
                    match std::thread::Builder::new()
                        .name("Meridian mod compile".into())
                        .spawn(move || android_runtime::precompile_mods(&compile_dir))
                    {
                        Ok(job) => {
                            self.mod_compile = Some(job);
                            self.screen.set_compiling_mods(true);
                            self.screen.note(horizon_client::launcher_backend::i18n::tr(
                                "Compiling mods…",
                            ));
                        }
                        Err(error) => {
                            eprintln!("Meridian: mod compile worker not started: {error}");
                        }
                    }
                }
                let writing_files =
                    matches!(&self.game_files, GameFiles::Writing(job) if !job.is_finished());
                if self.mod_compile.is_some() || writing_files {
                    self.play_requested_at.get_or_insert_with(Instant::now);
                    if self.mod_compile.is_none() {
                        self.screen.note(horizon_client::launcher_backend::i18n::tr(
                            "Finishing game files…",
                        ));
                    }
                } else {
                    self.hand_off(event_loop);
                }
            }
            Some(Action::OpenUrl(url)) => {
                if !android_runtime::open_link(&url) {
                    eprintln!("Meridian: no browser opened the sign-in page");
                }
            }
            Some(Action::CopyText(text)) => {
                if !android_runtime::copy_text(&text) {
                    eprintln!("Meridian: no clipboard to copy the game log to");
                }
            }
            Some(Action::ExportDiagnostics) => self.start_diagnostics_export(),
            Some(Action::PickSkin) => {
                if !android_runtime::pick_file() {
                    self.screen.no_file_picker();
                }
            }
            Some(Action::ImportFile(kind)) => self.start_import(kind),
            Some(Action::ExportWorld(world)) => self.start_world_export(world),
            Some(Action::Quit) => event_loop.exit(),
            None => {}
        }
        if self.screen.needs_full_atlas()
            && !horizon_client::ui_theme::pack_font_ready(&self.gui.atlas)
            && matches!(self.screen.backend().setup(), Setup::Ready)
        {
            self.use_full_atlas();
        }
        // A tap on a field brings the keyboard back even after Back put it away.
        self.sync_keyboard(true);
    }

    /// The pixel font turned on: the launcher's atlas has no font sheets, so the full one is
    /// built and a renderer made for it (about a second, once), as the loading screen swaps its
    /// atlas. Without one, the launcher's atlas stays, with its modern font.
    fn use_full_atlas(&mut self) {
        let (Ok(atlas), Some(window)) = (full_atlas(&self.root), self.window.clone()) else {
            return;
        };
        // A window takes one surface at a time (Android: ERROR_NATIVE_WINDOW_IN_USE_KHR).
        if let Some(renderer) = &mut self.renderer {
            renderer.release_surface();
        }
        match loading_renderer(
            window,
            false,
            Some(&atlas),
            Some(&self.gui.panorama),
            self.renderer.as_ref(),
            self.settings.horizon.api,
            false,
        ) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.gui.atlas = atlas;
                // The new atlas is where the old one was: its glyphs are drawn again.
                horizon_client::ui_theme::reset_atlas();
            }
            Err(error) => {
                eprintln!("Meridian: no renderer for the pixel font: {error}");
                if let Some(renderer) = &mut self.renderer
                    && let Err(error) = renderer.recreate_surface()
                {
                    eprintln!("Meridian: the launcher's surface did not come back: {error}");
                }
            }
        }
    }

    /// Check the Bedrock art first, since replacing it can invalidate the prepared data file.
    fn prepare_game_files(&mut self) {
        if !matches!(self.game_files, GameFiles::Pending)
            || !matches!(self.screen.backend().setup(), Setup::Ready)
        {
            return;
        }
        let root = self.root.clone();
        let resources = launch::resources_dir(&root);
        let job = std::thread::Builder::new()
            .name("Meridian game files".into())
            .spawn(move || {
                let art = match controls_art::fetch(&resources) {
                    Ok(count) => {
                        eprintln!("Meridian: control artwork: {count} files");
                        true
                    }
                    Err(error) => {
                        eprintln!("Meridian: control artwork: {error}");
                        false
                    }
                };
                horizon_threads::idle_current();
                // The data file only once the art is settled: replacing art invalidates it.
                if art {
                    let started = Instant::now();
                    match controls_art::write_prepared_cache(&resources) {
                        Ok(true) => eprintln!(
                            "Meridian: launcher wrote horizon-data.bin in {} ms",
                            started.elapsed().as_millis()
                        ),
                        Ok(false) => {}
                        Err(error) => eprintln!("Meridian: launcher cache not written: {error}"),
                    }
                }
                // ~340 MB that only preparing used, rebuilt locally if it runs again.
                launch::drop_prepare_scratch(&root);
            });
        self.game_files = match job {
            Ok(job) => GameFiles::Writing(job),
            Err(error) => {
                eprintln!("Meridian: game files worker not started: {error}");
                GameFiles::Done
            }
        };
    }

    /// Precompiles enabled wasm mods (including built-in Smooth Zoom) quietly in the background
    /// once the first launcher frame is on screen, so tapping Play doesn't wait to compile.
    fn warm_mods_in_background(&mut self) {
        if !self.shown
            || self.mod_compile.is_some()
            || matches!(
                self.screen.backend().setup(),
                Setup::Running(p) if p.stage == Stage::Prepare
            )
        {
            return;
        }
        let selected = self.screen.backend().selected_profile();
        if self.mod_warmed_profile.as_deref() == Some(selected.as_str()) {
            return;
        }
        self.mod_warmed_profile = Some(selected.clone());
        let dir = profile_dir(&self.root, &selected);
        launch::seed_builtin_mods(&self.root, &dir);
        if !android_runtime::mods_need_compile(&dir) {
            return;
        }
        let compile_dir = dir;
        match std::thread::Builder::new()
            .name("Meridian mod precompile".into())
            .spawn(move || {
                horizon_threads::idle_current();
                let started = Instant::now();
                android_runtime::precompile_mods(&compile_dir);
                eprintln!(
                    "Meridian: background mod precompile finished in {} ms",
                    started.elapsed().as_millis()
                );
            }) {
            Ok(job) => self.mod_compile = Some(job),
            Err(error) => {
                eprintln!("Meridian: background mod precompile not started: {error}");
            }
        }
    }

    /// Writes the redacted diagnostics ZIP off the UI thread so large crash reports don't drop
    /// frames. JNI calls (`exit_reasons`, `share_file`) stay on this thread so the short-lived
    /// worker never attaches permanently to the JVM.
    fn start_diagnostics_export(&mut self) {
        if self.diagnostics_export.is_some() {
            return;
        }
        let root = self.root.clone();
        let exits = android_runtime::exit_reasons();
        let backend = self.screen.backend();
        let players: Vec<String> = backend
            .accounts()
            .into_iter()
            .map(|a| match a {
                Account::Microsoft { name } | Account::Ely { name } | Account::Offline { name } => {
                    name
                }
            })
            .collect();
        let profiles: Vec<String> = backend
            .profiles()
            .into_iter()
            .filter(|p| p.id != VANILLA)
            .map(|p| p.name)
            .collect();
        // ponytail: mirrors `Real::export_diagnostics` so ZIP I/O runs off the UI thread without
        // locking `MobileLauncher`'s `Box<dyn Backend>`; unify if `Backend` gains an async export.
        let job = std::thread::Builder::new()
            .name("Meridian diagnostics".into())
            .spawn(move || {
                let extras = if exits.is_empty() {
                    Vec::new()
                } else {
                    vec![("android-exits.txt", exits)]
                };
                let hide = diagnostics::Hide {
                    players: &players,
                    profiles: &profiles,
                };
                diagnostics::export(&root, &extras, &hide)
            });
        match job {
            Ok(job) => self.diagnostics_export = Some(job),
            Err(error) => self
                .screen
                .diagnostics_saved(Err(format!("Could not start export: {error}"))),
        }
    }

    fn poll_diagnostics_export(&mut self) {
        if !self
            .diagnostics_export
            .as_ref()
            .is_some_and(|job| job.is_finished())
        {
            return;
        }
        let Some(job) = self.diagnostics_export.take() else {
            return;
        };
        let exported = job
            .join()
            .unwrap_or_else(|_| Err("Diagnostics export stopped unexpectedly.".into()));
        let outcome = match exported {
            Ok(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "diagnostics.zip".into());
                if path.to_str().is_some_and(android_runtime::share_file) {
                    Ok(name)
                } else {
                    Err(horizon_client::launcher_backend::i18n::tr("Couldn't open the share sheet")
                        .into())
                }
            }
            Err(error) => Err(error),
        };
        self.screen.diagnostics_saved(outcome);
        self.dirty = true;
    }

    fn start_import(&mut self, kind: ImportKind) {
        const DEST: &str = "picked-import.bin";
        if self.pending_import.is_some() || self.file_transfer.is_some() {
            return;
        }
        for suffix in ["", ".part", ".name", ".cancelled"] {
            let _ = std::fs::remove_file(self.root.join(format!("{DEST}{suffix}")));
        }
        let (mime, max_bytes) = match kind {
            ImportKind::World | ImportKind::ResourcePack | ImportKind::ShaderPack => {
                ("application/zip", files::MAX_PACK_BYTES)
            }
            ImportKind::TouchLayout => ("*/*", 2 * 1024 * 1024),
        };
        if android_runtime::pick_import(mime, DEST, max_bytes) {
            let profile_dir = profile_dir(&self.root, &self.screen.backend().selected_profile());
            self.pending_import = Some(PendingImport {
                kind,
                dest: DEST,
                profile_dir,
            });
        } else {
            self.screen.file_transfer_finished(Some(Err(
                "No file picker is available on this device.".into(),
            )));
        }
    }

    fn start_world_export(&mut self, world: String) {
        if self.pending_import.is_some() || self.file_transfer.is_some() {
            return;
        }
        let root = self.root.clone();
        let profile_home = profile_dir(&self.root, &self.screen.backend().selected_profile());
        let job = std::thread::Builder::new()
            .name("Meridian world export".into())
            .spawn(move || {
                files::export_world(&root, &profile_home, &world)
                    .map(|p| p.to_string_lossy().into_owned())
            });
        match job {
            Ok(job) => {
                self.file_transfer = Some(FileTransferJob {
                    kind: FileTransferKind::Export,
                    job,
                });
            }
            Err(error) => {
                self.screen
                    .file_transfer_finished(Some(Err(format!("Could not start export: {error}"))));
            }
        }
    }

    fn poll_file_transfer(&mut self) {
        if let Some(pending) = &self.pending_import {
            let cancelled = self.root.join(format!("{}.cancelled", pending.dest));
            if cancelled.is_file() {
                let _ = std::fs::remove_file(&cancelled);
                self.pending_import = None;
                self.screen.file_transfer_finished(None);
                self.dirty = true;
                return;
            }
            let picked = self.root.join(pending.dest);
            if picked.is_file() {
                let pending = self.pending_import.take().unwrap();
                let name_path = self.root.join(format!("{}.name", pending.dest));
                let display_name = std::fs::read_to_string(&name_path)
                    .ok()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "import".to_owned());
                let job = std::thread::Builder::new()
                    .name("Meridian file import".into())
                    .spawn(move || {
                        let res = match pending.kind {
                            ImportKind::World => {
                                files::import_world(&picked, &pending.profile_dir, &display_name)
                                    .map(|folder| format!("Imported world: {folder}"))
                            }
                            ImportKind::ResourcePack => files::import_resource_pack(
                                &picked,
                                &pending.profile_dir,
                                &display_name,
                            )
                            .map(|name| format!("Imported resource pack: {name}")),
                            ImportKind::ShaderPack => files::import_shader_pack(
                                &picked,
                                &pending.profile_dir,
                                &display_name,
                            )
                            .map(|name| format!("Imported shader pack: {name}")),
                            ImportKind::TouchLayout => {
                                horizon_client_render::touch::pojav::import_file(
                                    &picked,
                                    &pending.profile_dir,
                                    &display_name,
                                )
                            }
                        };
                        let _ = std::fs::remove_file(&picked);
                        let _ = std::fs::remove_file(&name_path);
                        res
                    });
                match job {
                    Ok(job) => {
                        self.file_transfer = Some(FileTransferJob {
                            kind: FileTransferKind::Import,
                            job,
                        });
                    }
                    Err(error) => {
                        self.screen.file_transfer_finished(Some(Err(format!(
                            "Could not start import: {error}"
                        ))));
                    }
                }
                self.dirty = true;
            }
        }
        if !self
            .file_transfer
            .as_ref()
            .is_some_and(|ft| ft.job.is_finished())
        {
            return;
        }
        let Some(ft) = self.file_transfer.take() else {
            return;
        };
        let res = ft
            .job
            .join()
            .unwrap_or_else(|_| Err("File transfer stopped unexpectedly.".into()));
        let outcome = match (ft.kind, res) {
            (FileTransferKind::Import, res) => res,
            (FileTransferKind::Export, Ok(path_str)) => {
                let name = std::path::Path::new(&path_str)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "world.zip".into());
                if android_runtime::share_file(&path_str) {
                    Ok(format!("Exported: {name}"))
                } else {
                    Err(horizon_client::launcher_backend::i18n::tr("Couldn't open the share sheet")
                        .into())
                }
            }
            (FileTransferKind::Export, Err(e)) => Err(e),
        };
        self.screen.file_transfer_finished(Some(outcome));
        self.dirty = true;
    }

    /// The selected profile with the account's session or offline name, for the next process.
    fn hand_off(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = check_game_args(&launch::game_args(&self.root)) {
            self.screen.game_args_refused(error);
            return;
        }
        let backend = self.screen.backend();
        let session = match backend.game_session() {
            Ok(session) => session,
            // The online account isn't ready yet: the launcher says so and stays.
            Err(error) => {
                eprintln!("Meridian: not playing yet: {error}");
                return;
            }
        };
        let name = match backend.account() {
            Some(Account::Offline { name }) if session.is_none() => Some(name),
            _ => None,
        };
        let launch = Launch {
            profile: backend.selected_profile(),
            name,
            session: session.is_some(),
        };
        match launch::write(&self.root, &launch, session.as_ref()) {
            Ok(()) => {
                self.play = true;
                event_loop.exit();
            }
            Err(error) => eprintln!("Meridian: could not hand the game its profile: {error}"),
        }
    }

    /// The phone's keyboard up while a text field has the focus (again on a tap, after Back put
    /// it away), away once none has.
    fn sync_keyboard(&mut self, tapped: bool) {
        let wanted = self.screen.wants_text();
        let Some(window) = &self.window else { return };
        if wanted && (tapped || !self.keyboard) || !wanted && self.keyboard {
            window.set_ime_allowed(wanted);
            self.keyboard = wanted;
        }
    }

    /// A finger past [`DRAG`] scrolls the page by as much as it moves; one that didn't taps.
    fn touch(&mut self, event_loop: &ActiveEventLoop, phase: TouchPhase, id: u64, at: (f64, f64)) {
        let scale = f64::from(self.batch().scale);
        // The page follows the finger in the launcher's own units (dp).
        let density = f64::from(self.with_ui(|_, ui| ui.density));
        match (phase, &mut self.finger) {
            (TouchPhase::Started, _) => {
                self.finger = Some(Finger {
                    id,
                    down: at,
                    last: at,
                    dragging: false,
                });
            }
            (TouchPhase::Moved, Some(finger)) if finger.id == id => {
                let moved = (at.0 - finger.down.0).hypot(at.1 - finger.down.1);
                if !finger.dragging && moved >= DRAG * scale {
                    finger.dragging = true;
                    finger.last = at;
                    // Where it went down decides what it moves (on Mods: a list's pages or the tabs).
                    let down = (finger.down.0 / density, finger.down.1 / density);
                    self.screen.drag_from((down.0 as f32, down.1 as f32));
                }
                if finger.dragging {
                    self.screen.scroll(
                        ((at.0 - finger.last.0) / density) as f32,
                        ((at.1 - finger.last.1) / density) as f32,
                    );
                    finger.last = at;
                }
            }
            (TouchPhase::Ended, Some(finger)) if finger.id == id => {
                let tapped = !finger.dragging;
                self.finger = None;
                if tapped {
                    self.click(event_loop);
                }
                // A lifted finger points at nothing, so nothing stays highlighted where it was.
                self.pointer = (-1.0, -1.0);
            }
            (TouchPhase::Cancelled, _) => {
                self.finger = None;
                self.pointer = (-1.0, -1.0);
            }
            _ => {}
        }
    }

    /// A controller moves the pointer (left stick, D-pad) and its A button taps where it is.
    fn pad_input(&mut self, event_loop: &ActiveEventLoop, seconds: f64) {
        let pad = android_runtime::pad_state();
        let tilt = |value: f32| if value.abs() < PAD_DEADZONE { 0.0 } else { f64::from(value) };
        let axis = |minus: Button, plus: Button, stick: f32| {
            f64::from(i8::from(pad.held(plus)) - i8::from(pad.held(minus))) + tilt(stick)
        };
        let dx = axis(Button::DPadLeft, Button::DPadRight, pad.left[0]).clamp(-1.0, 1.0);
        let dy = axis(Button::DPadUp, Button::DPadDown, pad.left[1]).clamp(-1.0, 1.0);
        let released = self.pad.held(Button::South) && !pad.held(Button::South);
        let pressed = pad.buttons & !self.pad.buttons != 0;
        self.pad = pad;
        if dx == 0.0 && dy == 0.0 && !pressed && !released {
            return;
        }
        let [w, h] = self.size();
        if self.pad_used.is_none() {
            // The pointer starts in the middle the first time a controller moves it.
            self.pointer = (f64::from(w) / 2.0, f64::from(h) / 2.0);
        }
        self.pad_used = Some(Instant::now());
        self.pointer = (
            (self.pointer.0 + dx * PAD_SPEED * seconds).clamp(0.0, f64::from(w)),
            (self.pointer.1 + dy * PAD_SPEED * seconds).clamp(0.0, f64::from(h)),
        );
        if released {
            self.click(event_loop);
        }
    }
}

impl ApplicationHandler for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // Back on screen after `suspended`: a new surface for the same window.
            if let Some(renderer) = &mut self.renderer
                && let Err(error) = renderer.recreate_surface()
            {
                eprintln!("Meridian: the launcher's surface did not come back: {error}");
            }
            return;
        }
        eprintln!("Meridian: launcher window after {} ms", self.started.elapsed().as_millis());
        let made = event_loop
            .create_window(Window::default_attributes().with_title("Meridian"))
            .map_err(Into::into)
            .and_then(|window| {
                let window = Arc::new(window);
                let renderer = loading_renderer(
                    window.clone(),
                    false,
                    Some(&self.gui.atlas),
                    Some(&self.gui.panorama),
                    None,
                    self.settings.horizon.api,
                    false,
                )?;
                Ok((window, renderer))
            });
        eprintln!("Meridian: launcher renderer after {} ms", self.started.elapsed().as_millis());
        match made {
            Ok((window, renderer)) => {
                // Android's density (420 dpi: 2.625), not the fit of a 1040x700 window (~1.5).
                horizon_client::ui_theme::set_min_density(window.scale_factor() as f32);
                window.request_redraw();
                self.window = Some(window);
                self.renderer = Some(renderer);
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(renderer) = &mut self.renderer {
            renderer.drop_surface();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = self.window.clone().filter(|w| w.id() == id) else {
            return;
        };
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.dirty = true;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
                window.request_redraw();
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::CursorMoved { position, .. } => self.pointer = (position.x, position.y),
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => self.click(event_loop),
            // A finger is the pointer while it's down, and taps where it lifts unless it dragged.
            WindowEvent::Touch(Touch {
                phase,
                location,
                id,
                ..
            }) => {
                self.pad_used = None;
                self.pointer = (location.x, location.y);
                self.touch(event_loop, phase, id, (location.x, location.y));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Backspace) => self.screen.backspace(),
                    PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                        self.screen.typed("\n")
                    }
                    _ => {
                        if let Some(text) = &event.text {
                            self.screen.typed(text);
                        }
                    }
                }
                self.sync_keyboard(false);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let seconds = now.duration_since(self.last_tick).as_secs_f64().min(0.1);
        self.last_tick = now;
        let ime = android_runtime::ime_height();
        self.dirty |= ime != self.ime;
        self.ime = ime;
        let cutout = android_runtime::safe_insets();
        if self.cutout != Some(cutout) {
            self.cutout = Some(cutout);
            self.dirty = true;
            let density = self.with_ui(|_, ui| ui.density);
            let [left, top, right, bottom] = cutout.map(|pixels| pixels as f32 / density);
            let notch = SafeInsets {
                top,
                right,
                bottom,
                left,
            };
            // The iPhone's safe area on every phone (one look everywhere), the notch where deeper.
            self.screen.set_safe_insets(SafeInsets::PHONE.max(notch));
        }
        self.dirty |= self.screen.poll();
        self.poll_diagnostics_export();
        self.poll_file_transfer();
        self.pad_input(event_loop, seconds);
        self.prepare_game_files();
        self.warm_mods_in_background();
        if matches!(&self.game_files, GameFiles::Writing(job) if job.is_finished()) {
            self.game_files = GameFiles::Done;
        }
        if self.mod_compile.as_ref().is_some_and(|job| job.is_finished()) {
            if let Some(job) = self.mod_compile.take() {
                let _ = job.join();
            }
            if self.play_requested_at.is_some() {
                let selected = self.screen.backend().selected_profile();
                let dir = profile_dir(&self.root, &selected);
                launch::seed_builtin_mods(&self.root, &dir);
                if android_runtime::mods_need_compile(&dir)
                    && let Ok(job) = std::thread::Builder::new()
                        .name("Meridian mod compile".into())
                        .spawn(move || android_runtime::precompile_mods(&dir))
                {
                    self.mod_compile = Some(job);
                    self.screen.set_compiling_mods(true);
                    self.dirty = true;
                }
            }
            if self.mod_compile.is_none() {
                self.screen.set_compiling_mods(false);
                self.dirty = true;
            }
        }
        if self.mod_compile.is_none()
            && self.play_requested_at.is_some_and(|at| {
                matches!(self.game_files, GameFiles::Done)
                    || now.saturating_duration_since(at) >= MAX_FILE_WAIT
            })
        {
            self.play_requested_at = None;
            self.hand_off(event_loop);
        }
        // A frame after input at once; without input, every frame while a controller moves the
        // pointer or page dots pan, ten a second while an install, a sign-in or the game's files move on, and
        // one a second otherwise (sizes measured, a head downloaded), so an idle launcher
        // leaves the phone's CPU alone.
        let pad_moving = self
            .pad_used
            .is_some_and(|at| at.elapsed() < Duration::from_secs(1));
        let working = self.screen.moving()
            || self.play_requested_at.is_some()
            || matches!(self.game_files, GameFiles::Writing(_))
            || self.diagnostics_export.is_some()
            || self.pending_import.is_some()
            || self.file_transfer.is_some();
        let next = if pad_moving || self.screen.animating() {
            16
        } else if working {
            100
        } else {
            1000
        };
        // A motion starts while a frame draws, after that frame's wait was set: its next frame is
        // due in 16 ms, not after the idle second.
        if next == 16 {
            self.next_frame = self.next_frame.min(now + Duration::from_millis(16));
        }
        if self.dirty || now >= self.next_frame {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            self.dirty = false;
            self.next_frame = now + Duration::from_millis(next);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}
