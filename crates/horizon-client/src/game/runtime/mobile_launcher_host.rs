// game-module-cfg: target_os = "android"
// game-module-visibility: pub
//! Meridian's launcher on Android (`docs/launcher-spec.md`, "Hand-off"): the launcher screens
//! (`horizon_client::mobile_launcher`) in a process of their own, before any game starts. Play
//! writes the hand-off (`launcher_backend::launch`); the platform then starts the app again, and
//! that process plays.
use super::*;
use horizon_client::launcher_backend::{
    Account, Real, Setup, controls_art,
    launch::{self, Launch},
};
use horizon_client::mobile_launcher::{Action, MobileLauncher};
use horizon_client::ui_theme::Ui;
use horizon_client_render::android_runtime;
use horizon_client_render::gamepad::{Button, PadState};
use winit::event::{MouseButton, Touch, TouchPhase};

/// How fast a controller moves the pointer at full tilt, in window pixels a second.
const PAD_SPEED: f64 = 1100.0;
/// Stick tilt below this doesn't move the pointer.
const PAD_DEADZONE: f32 = 0.25;
/// A brief grace for a nearly finished cache. A longer wait costs more than an uncached start.
const MAX_FILE_WAIT: Duration = Duration::from_secs(1);

enum GameFiles {
    Pending,
    Writing(std::thread::JoinHandle<()>),
    Done,
}

struct Host {
    root: std::path::PathBuf,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    gui: UiResources,
    screen: MobileLauncher,
    /// The pointer (or finger) in physical pixels.
    pointer: (f64, f64),
    /// The phone's keyboard is up for a text field.
    keyboard: bool,
    /// How far the keyboard covers the window, in pixels (polled: the window doesn't shrink).
    ime: u32,
    /// The controller last time, and when the pointer last moved by it: its cursor shows while
    /// it's in use, a finger hides it.
    pad: PadState,
    pad_used: Option<Instant>,
    last_tick: Instant,
    settings: Settings,
    /// Play was pressed and the hand-off written.
    play: bool,
    game_files: GameFiles,
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
    let gui = UiResources {
        atlas: HudAtlas::build(&horizon_assets::resource_source::ResourceSource::directory(
            launch::resources_dir(root),
        ))?,
        panorama: Arc::new(horizon_client::gfx::panorama::Faces::colour([0; 4])),
        splashes: Arc::from(""),
    };
    eprintln!("Meridian: launcher atlas after {} ms", started.elapsed().as_millis());
    let mut screen = MobileLauncher::new(Box::new(backend));
    if launch::closed_unexpectedly(root) {
        screen.note("The game closed unexpectedly last time.");
    }
    let mut host = Host {
        root: root.to_path_buf(),
        window: None,
        renderer: None,
        gui,
        screen,
        pointer: (0.0, 0.0),
        keyboard: false,
        ime: 0,
        pad: PadState::default(),
        pad_used: None,
        last_tick: Instant::now(),
        settings,
        play: false,
        game_files: GameFiles::Pending,
        play_requested_at: None,
        started,
        shown: false,
        error: None,
    };
    super::event_loop::new_event_loop()?.run_app(&mut host)?;
    host.error.map_or(Ok(host.play), Err)
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
        match renderer.render(&loading_frame(&batch)) {
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
                self.prepare_game_files();
                if matches!(&self.game_files, GameFiles::Writing(job) if !job.is_finished()) {
                    self.play_requested_at.get_or_insert_with(Instant::now);
                    self.screen.note("Finishing game files…");
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
            Some(Action::PickSkin) => {
                if !android_runtime::pick_file() {
                    self.screen.no_file_picker();
                }
            }
            Some(Action::Quit) => event_loop.exit(),
            None => {}
        }
        // A tap on a field brings the keyboard back even after Back put it away.
        self.sync_keyboard(true);
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

    /// The selected profile with the account's session or offline name, for the next process.
    fn hand_off(&mut self, event_loop: &ActiveEventLoop) {
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
            // A finger is the pointer while it's down, and taps where it lifts.
            WindowEvent::Touch(Touch {
                phase, location, ..
            }) => {
                self.pad_used = None;
                self.pointer = (location.x, location.y);
                if phase == TouchPhase::Ended {
                    self.click(event_loop);
                }
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
        self.ime = android_runtime::ime_height();
        self.screen.poll();
        self.pad_input(event_loop, seconds);
        self.prepare_game_files();
        if matches!(&self.game_files, GameFiles::Writing(job) if job.is_finished()) {
            self.game_files = GameFiles::Done;
        }
        if self.play_requested_at.is_some_and(|at| {
            matches!(self.game_files, GameFiles::Done)
                || now.saturating_duration_since(at) >= MAX_FILE_WAIT
        }) {
            self.play_requested_at = None;
            self.hand_off(event_loop);
        }
        // Sign-in and downloads move on without input: redraw ten times a second, and every
        // frame while a controller moves the pointer.
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        let pad_moving = self
            .pad_used
            .is_some_and(|at| at.elapsed() < Duration::from_secs(1));
        let next = if pad_moving { 16 } else { 100 };
        event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(next)));
    }
}
