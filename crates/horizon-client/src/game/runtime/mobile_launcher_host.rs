// game-module-cfg: target_os = "android"
// game-module-visibility: pub
//! Meridian's launcher on Android (`docs/launcher-spec.md`, "Hand-off"): the launcher screens
//! (`horizon_client::mobile_launcher`) in a process of their own, before any game starts. Play
//! writes the hand-off (`launcher_backend::launch`); the platform then starts the app again, and
//! that process plays.
use super::*;
use horizon_client::launcher_backend::{
    Account, Real,
    launch::{self, Launch},
};
use horizon_client::mobile_launcher::{Action, MobileLauncher};
use horizon_client::ui_theme::Ui;
use winit::event::{MouseButton, Touch, TouchPhase};

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
    settings: Settings,
    /// Play was pressed and the hand-off written.
    play: bool,
    error: Option<Box<dyn std::error::Error>>,
}

/// Shows the launcher in `root` (the app's folder) until the player plays or leaves: true when
/// the hand-off for the game is written.
pub fn run(root: &std::path::Path) -> Result<bool> {
    let settings = Settings::load(&Game::options_path());
    // The player's language, from the strings compiled in: there may be no prepared ones yet.
    horizon_client::lang::set(horizon_client::lang::Language::load(
        &[],
        &settings.language,
    ));
    let mut host = Host {
        root: root.to_path_buf(),
        window: None,
        renderer: None,
        gui: UiResources {
            atlas: HudAtlas::build(&horizon_assets::resource_source::ResourceSource::directory(
                launch::resources_dir(root),
            ))?,
            panorama: Arc::new(horizon_client::gfx::panorama::Faces::colour([0; 4])),
            splashes: Arc::from(""),
        },
        screen: MobileLauncher::new(Box::new(Real::new(root.to_path_buf()))),
        pointer: (0.0, 0.0),
        keyboard: false,
        settings,
        play: false,
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

    /// `f` over a Ui of the window's size, the cursor where the pointer is.
    fn with_ui<R>(&mut self, f: impl FnOnce(&mut MobileLauncher, &mut Ui) -> R) -> R {
        let mut batch = HudBatch::new(self.size(), self.settings.gui_scale_inputs());
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
        let mut batch = HudBatch::new(self.size(), self.settings.gui_scale_inputs());
        {
            let cursor = (
                self.pointer.0 as f32 / batch.scale,
                self.pointer.1 as f32 / batch.scale,
            );
            let mut ui = Ui::new(&self.gui.atlas, &mut batch, cursor);
            self.screen.draw(&mut ui);
            ui.finish();
        }
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        match renderer.render(&loading_frame(&batch)) {
            Ok(true) => {}
            // Not shown (the first frame often isn't, nor one in the background): the glyphs
            // it rasterized never reached the atlas, so draw them again next frame.
            Ok(false) => horizon_client::ui_theme::reset_atlas(),
            Err(e) => eprintln!("Meridian: launcher frame failed: {e}"),
        }
    }

    fn click(&mut self, event_loop: &ActiveEventLoop) {
        match self.with_ui(|screen, ui| screen.click(ui)) {
            Some(Action::Play) => self.hand_off(event_loop),
            Some(Action::OpenUrl(url)) => {
                if !horizon_client_render::android_runtime::open_link(&url) {
                    eprintln!("Meridian: no browser opened the sign-in page");
                }
            }
            Some(Action::Quit) => event_loop.exit(),
            None => {}
        }
        self.sync_keyboard();
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

    /// The phone's keyboard up while a text field has the focus, away otherwise.
    fn sync_keyboard(&mut self) {
        let wanted = self.screen.wants_text();
        if wanted != self.keyboard
            && let Some(window) = &self.window
        {
            window.set_ime_allowed(wanted);
            self.keyboard = wanted;
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
        match made {
            Ok((window, renderer)) => {
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
                self.sync_keyboard();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Sign-in and downloads move on without input: redraw ten times a second.
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(100),
        ));
    }
}
