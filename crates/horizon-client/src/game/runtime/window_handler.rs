//! winit's `ApplicationHandler` for the game window: window and device events, shutting down,
//! and the idle step (`about_to_wait`).
use super::*;

impl Game<'_> {
    /// A run that a person's stray key, click or mouse move would spoil: a screenshot sequence, a
    /// `HORIZON_TICK_SCRIPT` scenario or a `record-replay`. While one runs, the keyboard, mouse
    /// and pad at this computer don't reach the game (`locks_out`, `poll_pads`), the window's
    /// focus stays as the run found it (`held_focus`) and its pointer grab stays inside the game
    /// (`internal_grab`).
    pub(super) fn scripted_run(&self) -> bool {
        self.screenshot_sequence.is_some() || self.tick_script.playing() || self.repro.replaying()
    }
}

/// A key, a modifier, a click, the wheel, the pointer or the input method. Resize and close are
/// not, so a person can still close the window. A modifier change counts: `reconcile` would
/// release a Shift or Ctrl the run holds.
fn is_input(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::KeyboardInput { .. }
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::Ime(_)
    )
}

/// A person's input during a scripted run is dropped. What the run itself sends (the control
/// socket, a replay, the game's own) still plays.
fn locks_out(origin: input_trace::InputOrigin, scripted: bool, event: &WindowEvent) -> bool {
    scripted && origin == input_trace::InputOrigin::User && is_input(event)
}

/// A person clicking into the window or away from it during a scripted run. A replay's look and
/// clicks act by the focus and the pointer grab, which that would change mid-run: the run keeps
/// the focus it started with, and the last change applies once it ends (`settle_after_run`).
fn held_focus(
    origin: input_trace::InputOrigin,
    scripted: bool,
    event: &WindowEvent,
) -> Option<bool> {
    match event {
        WindowEvent::Focused(focused) if scripted && origin == input_trace::InputOrigin::User => {
            Some(*focused)
        }
        _ => None,
    }
}

/// What a person's event does at an agent's window no one has clicked yet.
#[derive(Debug, PartialEq)]
enum Unclaimed {
    /// A mouse button pressed inside it: the window is theirs from this press on.
    Claims,
    /// Their keys, wheel, input method and button releases, and the focus the system gives the
    /// window or takes from it: the game the agent drives hears none of them.
    Dropped,
    /// Everything else, and whatever the agent or the game sends.
    Passes,
}

/// Only a press claims the window. The focus doesn't: Windows and X11 can hand it to a window
/// nobody clicked, and the person's keys and mouse would then steer the agent's game (#15449).
/// The pointer's position still passes, so the press that claims a menu lands where it points.
fn at_unclaimed(
    origin: input_trace::InputOrigin,
    unclaimed: bool,
    event: &WindowEvent,
) -> Unclaimed {
    if !unclaimed || origin != input_trace::InputOrigin::User {
        return Unclaimed::Passes;
    }
    match event {
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            ..
        } => Unclaimed::Claims,
        WindowEvent::CursorMoved { .. } => Unclaimed::Passes,
        WindowEvent::Focused(_) => Unclaimed::Dropped,
        _ if is_input(event) => Unclaimed::Dropped,
        _ => Unclaimed::Passes,
    }
}

/// The Shift, Ctrl or Alt a person holds at an agent's window no one has clicked yet. The game
/// doesn't hear of them, but winit tells of modifiers only as they change: the last state is
/// kept for the claim, where a Ctrl held since before the click would otherwise read as up.
fn held_modifiers(
    origin: input_trace::InputOrigin,
    unclaimed: bool,
    event: &WindowEvent,
) -> Option<winit::event::Modifiers> {
    match event {
        WindowEvent::ModifiersChanged(modifiers)
            if unclaimed && origin == input_trace::InputOrigin::User =>
        {
            Some(*modifiers)
        }
        _ => None,
    }
}

impl Game<'_> {
    /// winit's `window_event`. The control socket and the pad menu call it too, with the
    /// input they make up.
    pub(super) fn handle_window_event(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        id: WindowId,
        event: WindowEvent,
        origin: input_trace::InputOrigin,
    ) {
        let input = matches!(
            event,
            WindowEvent::KeyboardInput { .. }
                | WindowEvent::MouseInput { .. }
                | WindowEvent::MouseWheel { .. }
                | WindowEvent::Ime(_)
        );
        self.with_user_input(input, |game| {
            game.handle_window_event_as(event_loop, id, event, origin);
        });
    }

    pub(super) fn handle_window_event_as(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        id: WindowId,
        mut event: WindowEvent,
        origin: input_trace::InputOrigin,
    ) {
        // A headless game has no window: its events (the frame, the control socket's input)
        // carry `WindowId::dummy()`.
        let ours = match &self.window {
            Some(w) => w.id() == id,
            None => self.options.headless,
        };
        if !ours {
            return;
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if self.pad.touch.editing() && is_input(&event) {
            if let WindowEvent::KeyboardInput { event, .. } = &event
                && event.state == ElementState::Pressed
                && event.physical_key == PhysicalKey::Code(KeyCode::Escape)
            {
                self.pad.touch.cancel_editor();
                self.set_screen(Screen::Playing);
            }
            return;
        }
        if let Some(modifiers) = held_modifiers(origin, self.unclaimed_window, &event) {
            self.unclaimed_modifiers = Some(modifiers);
        }
        if locks_out(origin, self.scripted_run(), &event) {
            return;
        }
        if let Some(focused) = held_focus(origin, self.scripted_run(), &event) {
            self.focus_after_run = Some(focused);
            return;
        }
        match at_unclaimed(origin, self.unclaimed_window, &event) {
            Unclaimed::Claims => self.claim_window(event_loop),
            Unclaimed::Dropped => return,
            Unclaimed::Passes => {}
        }
        follow_keymap(&mut event);
        // MacosUtil.setCtrlClickEmulatesRightClick: Control + left click is a right click.
        if let WindowEvent::MouseInput { state, button, .. } = &mut event
            && *button == MouseButton::Left
            && self.right_click_emulation.is_right_click(
                *state == ElementState::Pressed,
                self.modifiers.control_key(),
                cfg!(target_os = "macos") && self.settings.ctrl_click_emulates_right_click,
            )
        {
            *button = MouseButton::Right;
        }
        // Who caused it, before the event is handled: the trace wants the
        // window's own keys, never the ones the socket made up.
        self.trace_window_event(origin, &event);
        if let WindowEvent::KeyboardInput { event, .. } = &event
            && let PhysicalKey::Code(key) = event.physical_key
        {
            if event.state == ElementState::Pressed {
                self.physical_keys.insert(key);
                horizon_client::keybinds::learn_layout(event);
            } else {
                self.physical_keys.remove(&key);
            }
        }
        if matches!(
            &event,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                ..
            }
        ) && !self.input_made_pointer_event()
        {
            self.pad.keyboard_took_over = true;
        }
        if let WindowEvent::MouseInput { state, button, .. } = &event {
            if *state == ElementState::Pressed {
                self.physical_buttons.insert(*button);
            } else {
                self.physical_buttons.remove(button);
            }
        }
        let mouse = if !self.pad_synthetic_input()
            && let WindowEvent::MouseInput { state, button, .. } = &event
        {
            Some((
                glfw_button(*button),
                *state == ElementState::Pressed,
                glfw_modifiers(self.modifiers),
            ))
        } else {
            None
        };
        if let Some((button, pressed, modifiers)) = mouse {
            let mut canceled = false;
            self.native_mouse_button_pre(button, pressed, modifiers, &mut canceled);
            if canceled {
                return;
            }
        }
        if mouse.is_some()
            && let WindowEvent::MouseInput { state, button, .. } = &event
        {
            self.mod_mappings_button(*button, *state == ElementState::Pressed);
        }
        let resized = matches!(event, WindowEvent::Resized(_));
        self.handle_window_event_inner(event_loop, event);
        if resized {
            self.notify_window_resize();
        }
        // A title screen the event asked for opens before the next event.
        self.settle_title_mode();
        if let Some((button, pressed, modifiers)) = mouse {
            self.native_mouse_button_post(button, pressed, modifiers);
        }
    }

    /// The loop ended: close the pads' output, save the world.
    pub(super) fn shut_down(&mut self) {
        if self.error.is_none()
            && let Some(renderer) = &mut self.renderer
        {
            renderer.drop_hdr_marker();
        }
        self.end_screenshot_sequence(Err("cancelled: the game is closing".into()));
        self.pad_close_output();
        if let Err(e) = self.save_for_exit() {
            log::warn!("Horizon: failed to save the world on exit: {e}");
        }
        self.finish_profile();
        // The client's last moment: its world is left and saved, its options and
        // resources are still held. A dedicated server and a splitscreen guest skip
        // it, as `client_setup` does.
        if self.options.is_client_start() {
            self.native_client_stopping();
        }
    }

    /// winit's `about_to_wait`: what happens between frames (stop requests, the closing
    /// save, world entry, the control socket, pads), then the next frame.
    pub(super) fn idle(&mut self, event_loop: &dyn EventLoopCtl) {
        // A splitscreen player's game has nothing to save: P1's shuts the process down.
        if horizon_threads::stop_requests() > 0
            && !self.options.split_guest
            && self.pending_save.as_ref().is_none_or(|save| !save.quit)
        {
            // SIGTERM, SIGINT or SIGHUP: the shutdown hook's server halt, as a window close.
            if let Err(error) = self.begin_closing_save(true) {
                self.error = Some(error);
                event_loop.exit();
                return;
            }
        }
        if self.pending_save.is_some() {
            match self.poll_closing_save() {
                Ok(true) => event_loop.exit(),
                Ok(false) => {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    // Yield between polls even when disk I/O is slow.
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        Instant::now() + Duration::from_millis(4),
                    ));
                }
                Err(error) => {
                    self.error = Some(error);
                    self.exit_on_error(event_loop);
                }
            }
            return;
        }
        if let Err(error) = horizon_client::spans::time("world_entry", || self.resume_world_entry())
        {
            self.error = Some(error);
            self.exit_on_error(event_loop);
            return;
        }
        // The world a backup made before its upgrade opens (EditWorldScreen's callback).
        if let Some(folder) = self.title.as_mut().and_then(|t| t.take_open_request())
            && let Err(error) = self.open_world(&folder, None, None, false, None)
        {
            self.error = Some(error);
            event_loop.exit();
            return;
        }
        // A Realm's address came back (GetServerDetailsTask hands over to ConnectScreen).
        if let Some(action) = self.title.as_mut().and_then(|t| t.take_connect_request())
            && let Err(error) = self.title_action(event_loop, action)
        {
            self.error = Some(error);
            event_loop.exit();
            return;
        }
        horizon_client::spans::time("server_packs", || self.tick_server_packs());
        horizon_client::spans::time("control", || self.poll_control(event_loop));
        if self.pending_save.is_some() {
            return;
        }
        horizon_client::spans::time("pads", || self.poll_pads(event_loop));
        self.settle_after_run(event_loop);
        self.pause_if_inactive();
        if let Some(limit) = self.options.time_limit()
            && self.started.elapsed() > limit
        {
            self.error = Some(format!("run timed out after {} s", limit.as_secs()).into());
            event_loop.exit();
            return;
        }
        if let Some(w) = &self.window {
            if self.options.offscreen {
                // Offscreen frames don't wait for the window to be drawn: an
                // occluded window (a locked macOS session) gets no redraws.
                let id = w.id();
                self.handle_window_event(
                    event_loop,
                    id,
                    WindowEvent::RedrawRequested,
                    input_trace::InputOrigin::Game,
                );
            } else {
                w.request_redraw();
            }
        } else if self.options.headless {
            // The headless loop paces the frames itself (`run_headless`).
            self.handle_window_event(
                event_loop,
                WindowId::dummy(),
                WindowEvent::RedrawRequested,
                input_trace::InputOrigin::Game,
            );
            return;
        }
        // Consume the last frame's result: a hidden compositor may stop sending
        // redraws altogether, leaving a successful result stale indefinitely.
        event_loop.set_control_flow(event_loop::redraw_flow(
            &mut self.frame_drawn,
            self.options.offscreen,
            Instant::now(),
        ));
    }
}

impl ApplicationHandler for Game<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // Back on screen after `suspended` (Android): a new surface for the same window.
            if let Some(renderer) = &mut self.renderer
                && let Err(error) = renderer.recreate_surface()
            {
                log::error!("Horizon: the window's surface did not come back: {error}");
            }
            self.audio.resume();
            return;
        }
        debug_keys::warm_debug_cpu();
        // The Window constructor: created fullscreen when options.txt or --fullscreen says so.
        let fullscreen = self.settings.fullscreen || self.options.fullscreen;
        let size = self.options.window_size.map(f64::from);
        self.fullscreen = horizon_client::window::FullscreenState::new(fullscreen);
        let attributes =
            horizon_client::window::with_icon(Window::default_attributes().with_title("Horizon"));
        // Wayland names the window by its app id, which the horizon.desktop entry matches.
        #[cfg(target_os = "linux")]
        let attributes = winit::platform::wayland::WindowAttributesExtWayland::with_name(
            attributes, "horizon", "horizon",
        );
        // A game an agent started opens without the focus, as splitscreen's windows do, and
        // the person keeps their keyboard and pointer until they click it (`unclaimed_window`).
        // X11 activates it anyway, and Windows can too: the focus claims nothing.
        let agent_started = self.options.control.is_some();
        self.unclaimed_window = agent_started && !self.options.offscreen;
        // `--offscreen` renders into a texture: its window would only show white.
        let window_step = startup::step("window.create");
        let window = match event_loop.create_window(
            attributes
                .with_inner_size(winit::dpi::LogicalSize::new(size[0], size[1]))
                .with_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)))
                .with_visible(!self.options.offscreen)
                .with_active(!agent_started),
        ) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(e.into());
                event_loop.exit();
                return;
            }
        };
        horizon_client::window::settle_created_fullscreen(&window, fullscreen);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = hwnd(&window) {
            horizon_threads::system_menu::suppress(hwnd);
        }
        drop(window_step);
        match self.new_renderer(window.clone()) {
            Ok(r) => {
                self.renderer = Some(r);
                self.world_renderer_ready = true;
            }
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        }
        self.resize_state = Some(self.resize_state_of(window.inner_size()));
        self.window = Some(window);
        self.ime_allowed = false;
        self.ime_area = None;
        self.apply_settings();
        self.previous = Instant::now();
    }
    /// Android takes the window's surface whenever the app leaves the screen, and may end the
    /// process afterwards without warning: pause, let the surface go, and save the world
    /// (on the server thread, so Android isn't kept waiting). Desktop platforms never call it.
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if self.screen == Screen::Playing && self.player.is_some() && !self.options.no_pause {
            self.pause_focus = None;
            self.set_screen(Screen::Paused);
        }
        if let Some(renderer) = &mut self.renderer {
            renderer.drop_surface();
        }
        // ponytail: the audio stream keeps running silently in the background; stop the
        // stream itself if battery use while away matters.
        self.audio.pause();
        self.audio
            .stop_sounds(None, Some(horizon_client_render::sound_engine::MUSIC));
        if let Some(server) = &self.integrated {
            server.post(|game| {
                if let Err(error) = game.save_game() {
                    log::error!("Horizon: saving the world before the background failed: {error}");
                }
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Meridian: phones deliver fingers, not a mouse.
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if let WindowEvent::Touch(touch) = event {
            self.touch(event_loop, id, touch);
            return;
        }
        let key = if self.window.as_ref().is_some_and(|w| w.id() == id)
            && !self.scripted_run()
            && !self.unclaimed_window
            && let WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } = &event
        {
            Some((event.clone(), glfw_modifiers(self.modifiers)))
        } else {
            None
        };
        self.handle_window_event(event_loop, id, event, input_trace::InputOrigin::User);
        if let Some((event, modifiers)) = key {
            let name = match event.physical_key {
                PhysicalKey::Code(code) => horizon_client::keybinds::key_name(code),
                PhysicalKey::Unidentified(_) => None,
            };
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let layout = {
                use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
                event.key_without_modifiers()
            };
            // winit has no key without modifiers on Android or iOS; the key as typed is close
            // (layout_key_name lowercases letters).
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let layout = event.logical_key.clone();
            let layout_key = horizon_client::keybinds::layout_key_name(event.physical_key, &layout);
            let raw_code = horizon_client::keybinds::raw_key_code(event.physical_key);
            let action = match (event.state, event.repeat) {
                (winit::event::ElementState::Released, _) => mod_api::KeyAction::Release,
                (_, false) => mod_api::KeyAction::Press,
                (_, true) => mod_api::KeyAction::Repeat,
            };
            // Still the key's handling: a mod's key hook may reach the clipboard.
            self.with_user_input(true, |game| {
                game.key_input(name, layout_key, raw_code, action, modifiers);
            });
        }
    }
    fn device_event(&mut self, event_loop: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        // A composited view renders offscreen but its keyboard/mouse owner still
        // controls a visible native window. A headless offscreen run never does.
        if self.pending_save.is_none()
            && !self.scripted_run()
            && !self.unclaimed_window
            && self.captured
            && self.focused
            && self.keyboard_mouse
            && (!self.options.offscreen || self.split_window)
            && self.screen == Screen::Playing
            && let DeviceEvent::MouseMotion { delta: (x, y) } = event
        {
            if self.pad_radial_captures() {
                return;
            }
            if x != 0.0 || y != 0.0 {
                self.pad_yield_input(event_loop);
            }
            // MouseHandler.onMove with the pointer grabbed; applied once per frame.
            self.note_input(
                input_trace::InputOrigin::User,
                input_trace::TraceKind::Motion { x, y },
            );
            self.mouse.on_move(0.0, 0.0, x, y, true, true);
            self.last_input = Instant::now();
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Every event-loop pass, drawn or not: a window that stops redrawing (a lost
        // device, an occluded window) must still end after a server crash.
        self.exit_after_server_crash();
        #[cfg(target_os = "android")]
        self.android_mouse(event_loop);
        self.idle(event_loop);
    }
    /// The application is ending. On macOS, Quit (Cmd+Q) ends it through
    /// applicationWillTerminate without a CloseRequested; vanilla's GLFW
    /// window turns it into a window close, so Minecraft.stop saves the world.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.shut_down();
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
impl Game<'_> {
    /// A finger (Meridian): the touch controls while playing; anywhere else (menus,
    /// inventories) the pointer, so a tap clicks where it lands, and a tap on the screen it
    /// started on opens the keyboard when a text field has focus.
    // ponytail: one finger at a time outside play; in play, attack and use act on the crosshair
    // (the controls' tap-at-the-finger `aim` isn't wired yet).
    fn touch(&mut self, event_loop: &ActiveEventLoop, id: WindowId, touch: winit::event::Touch) {
        use winit::event::TouchPhase;
        let [w, h] = self
            .renderer
            .as_ref()
            .map_or([1, 1], |r| [r.config.width.max(1), r.config.height.max(1)]);
        let started = touch.phase == TouchPhase::Started;
        let ended = matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled);
        let point = [
            touch.location.x as f32 / w as f32,
            touch.location.y as f32 / h as f32,
        ];
        if self.pad.touch.editing() {
            self.pad.touch.viewport(w, h);
            self.pad.touch.editor_touch(touch.id, point, touch.phase);
            if !self.pad.touch.editing() {
                self.set_screen(Screen::Playing);
            }
            return;
        }
        let device_id = touch.device_id;
        let release = |game: &mut Self, event_loop: &ActiveEventLoop| {
            game.window_event(
                event_loop,
                id,
                WindowEvent::MouseInput {
                    device_id,
                    state: ElementState::Released,
                    button: MouseButton::Left,
                },
            );
        };
        if self.screen == Screen::Playing && self.player.is_some() {
            let controls = &mut self.pad.touch;
            controls.force_crosshair = true;
            controls.viewport(w, h);
            let drag = controls.update(touch.id, point, started, ended);
            // Degrees, as the touch settings scale them; the game turns in 0.15° units.
            let turn = (drag != [0.0; 2]).then(|| {
                horizon_client_render::touch::camera_delta(drag, w, h, &controls.settings)
            });
            // The controls' key presses (the offhand swap) and the top bar's Chat, F3 and F5 act
            // as those key bindings; its Touch button opens the control hub.
            let chat = controls.chat_requested;
            let mut actions = controls.key_events();
            for (requested, action) in [
                (&mut controls.chat_requested, "key.chat"),
                (&mut controls.debug_requested, "key.debug.overlay"),
                (&mut controls.perspective_requested, "key.togglePerspective"),
            ] {
                if std::mem::take(requested) {
                    actions.extend([(action.to_owned(), true), (action.to_owned(), false)]);
                }
            }
            let settings = std::mem::take(&mut controls.settings_requested);
            if let Some([yaw, pitch]) = turn {
                self.look_turn(f64::from(yaw / 0.15), f64::from(pitch / 0.15));
                self.last_input = Instant::now();
            }
            for (action, down) in actions {
                let action = action.strip_prefix("action:").unwrap_or(&action);
                if let Err(error) = self.pad_action(action, down) {
                    log::warn!("Horizon: touch control {action}: {error}");
                }
            }
            if settings {
                self.set_screen(Screen::Paused);
                if self.screen == Screen::Paused {
                    self.pad.touch.open_editor();
                }
            }
            // The Chat button asks to type: the keyboard comes with it.
            if chat {
                self.show_keyboard();
            }
            // A finger that pressed a menu button which closed into play lifts here.
            if ended {
                release(self, event_loop);
            }
            return;
        }
        // Over the keyboard, the chat is drawn lifted (`lift_over_keyboard`): so is the finger.
        #[cfg(target_os = "android")]
        let touch = winit::event::Touch {
            location: winit::dpi::PhysicalPosition::new(
                touch.location.x,
                touch.location.y + f64::from(self.keyboard_lift()),
            ),
            ..touch
        };
        self.window_event(
            event_loop,
            id,
            WindowEvent::CursorMoved {
                device_id,
                position: touch.location,
            },
        );
        if started {
            self.touch_press_focused = self.text_field_focused();
            self.window_event(
                event_loop,
                id,
                WindowEvent::MouseInput {
                    device_id,
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                },
            );
        } else if ended {
            release(self, event_loop);
            // A finger leaves no pointer behind: nothing stays hovered (or shows its tooltip).
            self.window_event(
                event_loop,
                id,
                WindowEvent::CursorMoved {
                    device_id,
                    position: winit::dpi::PhysicalPosition::new(-1.0e6, -1.0e6),
                },
            );
            if std::mem::take(&mut self.touch_press_focused) && self.text_field_focused() {
                self.show_keyboard();
            }
        }
    }

    /// A mouse on Android (Meridian): the events `meridian-android`'s input hook took from winit,
    /// replayed as a computer's mouse (pointer, buttons, wheel, and the view's turn).
    #[cfg(target_os = "android")]
    fn android_mouse(&mut self, event_loop: &ActiveEventLoop) {
        use horizon_client_render::android_runtime::{MouseEvent, take_mouse_events};
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        for (device_id, event) in take_mouse_events() {
            match event {
                MouseEvent::Moved(x, y) => self.window_event(
                    event_loop,
                    id,
                    WindowEvent::CursorMoved {
                        device_id,
                        position: winit::dpi::PhysicalPosition::new(x, y),
                    },
                ),
                MouseEvent::Button(button, down) => self.window_event(
                    event_loop,
                    id,
                    WindowEvent::MouseInput {
                        device_id,
                        state: if down {
                            ElementState::Pressed
                        } else {
                            ElementState::Released
                        },
                        button,
                    },
                ),
                MouseEvent::Wheel(x, y) => self.window_event(
                    event_loop,
                    id,
                    WindowEvent::MouseWheel {
                        device_id,
                        delta: MouseScrollDelta::LineDelta(x, y),
                        phase: winit::event::TouchPhase::Moved,
                    },
                ),
                MouseEvent::Motion(x, y) => {
                    self.device_event(event_loop, device_id, DeviceEvent::MouseMotion { delta: (x, y) })
                }
            }
        }
    }

    /// Opens the phone's keyboard for the focused text field (Back hides it again).
    fn show_keyboard(&mut self) {
        if self.text_field_focused()
            && let Some(window) = &self.window
        {
            window.set_ime_allowed(true);
            self.ime_allowed = true;
            self.ime_area = None;
        }
    }

    /// Meridian: how far the phone's keyboard covers the window while the chat is open, in
    /// pixels. The fullscreen window doesn't shrink for it, so the HUD and touches lift by it.
    #[cfg(target_os = "android")]
    fn keyboard_lift(&self) -> u32 {
        if self.screen == Screen::Chat {
            horizon_client_render::android_runtime::ime_height()
        } else {
            0
        }
    }

    /// The chat's HUD above the phone's keyboard. Adapted from Starling's `lift_over_keyboard`
    /// (`crates/horizon-client/src/game/touch.rs`) at 5432370d5736.
    #[cfg(target_os = "android")]
    pub(super) fn lift_over_keyboard(&self, hud: &mut HudBatch) {
        let lift = self.keyboard_lift();
        if lift == 0 {
            return;
        }
        let gui = lift as f32 / hud.scale;
        for vertex in &mut hud.vertices {
            vertex.pos[1] -= gui;
        }
        for (_, blend) in &mut hud.blends {
            if let horizon_client_render::hud::Blend::Depth(rect)
            | horizon_client_render::hud::Blend::DepthGlint(rect) = blend
            {
                rect[1] = rect[1].saturating_sub(lift);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use input_trace::InputOrigin;
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::event::{Ime, Modifiers, TouchPhase};
    use winit::keyboard::ModifiersState;

    // A key's `KeyEvent` can't be made outside winit; these are the person's other inputs.
    fn inputs() -> Vec<WindowEvent> {
        let device_id = DeviceId::dummy();
        vec![
            WindowEvent::MouseInput {
                device_id,
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
            WindowEvent::MouseWheel {
                device_id,
                delta: MouseScrollDelta::LineDelta(0.0, 1.0),
                phase: TouchPhase::Moved,
            },
            WindowEvent::CursorMoved {
                device_id,
                position: PhysicalPosition::new(10.0, 20.0),
            },
            WindowEvent::ModifiersChanged(Modifiers::from(ModifiersState::empty())),
            WindowEvent::Ime(Ime::Commit("a".into())),
        ]
    }

    #[test]
    fn a_person_is_locked_out_of_a_scripted_run_only() {
        for event in inputs() {
            assert!(locks_out(InputOrigin::User, true, &event), "{event:?}");
            assert!(!locks_out(InputOrigin::User, false, &event), "{event:?}");
        }
    }

    #[test]
    fn what_the_run_sends_still_plays() {
        for event in inputs() {
            assert!(!locks_out(InputOrigin::Agent, true, &event), "{event:?}");
            assert!(!locks_out(InputOrigin::Game, true, &event), "{event:?}");
        }
    }

    #[test]
    fn a_person_can_still_close_the_window() {
        for event in [
            WindowEvent::CloseRequested,
            WindowEvent::Resized(PhysicalSize::new(854, 480)),
        ] {
            assert!(!locks_out(InputOrigin::User, true, &event), "{event:?}");
            assert_eq!(
                held_focus(InputOrigin::User, true, &event),
                None,
                "{event:?}"
            );
        }
    }

    #[test]
    fn only_a_persons_press_claims_an_agents_window() {
        let device_id = DeviceId::dummy();
        let press = WindowEvent::MouseInput {
            device_id,
            state: ElementState::Pressed,
            button: MouseButton::Left,
        };
        assert_eq!(
            at_unclaimed(InputOrigin::User, true, &press),
            Unclaimed::Claims
        );
        // The focus alone claims nothing, and the game doesn't hear of it.
        for focused in [true, false] {
            let event = WindowEvent::Focused(focused);
            assert_eq!(
                at_unclaimed(InputOrigin::User, true, &event),
                Unclaimed::Dropped
            );
        }
        let release = WindowEvent::MouseInput {
            device_id,
            state: ElementState::Released,
            button: MouseButton::Left,
        };
        for event in inputs().into_iter().skip(1).chain([release]) {
            let want = match event {
                WindowEvent::CursorMoved { .. } => Unclaimed::Passes,
                _ => Unclaimed::Dropped,
            };
            assert_eq!(
                at_unclaimed(InputOrigin::User, true, &event),
                want,
                "{event:?}"
            );
        }
        for event in [
            WindowEvent::CloseRequested,
            WindowEvent::Resized(PhysicalSize::new(854, 480)),
        ] {
            assert_eq!(
                at_unclaimed(InputOrigin::User, true, &event),
                Unclaimed::Passes,
                "{event:?}"
            );
        }
    }

    #[test]
    fn a_persons_modifiers_are_kept_for_the_claim() {
        let shift = Modifiers::from(ModifiersState::SHIFT);
        let event = WindowEvent::ModifiersChanged(shift);
        assert_eq!(held_modifiers(InputOrigin::User, true, &event), Some(shift));
        assert_eq!(held_modifiers(InputOrigin::User, false, &event), None);
        assert_eq!(held_modifiers(InputOrigin::Agent, true, &event), None);
        assert_eq!(
            held_modifiers(InputOrigin::User, true, &WindowEvent::Focused(true)),
            None
        );
    }

    #[test]
    fn the_agent_still_drives_its_unclaimed_window() {
        let events = inputs().into_iter().chain([WindowEvent::Focused(true)]);
        for event in events {
            for origin in [InputOrigin::Agent, InputOrigin::Game] {
                assert_eq!(
                    at_unclaimed(origin, true, &event),
                    Unclaimed::Passes,
                    "{event:?}"
                );
            }
        }
    }

    #[test]
    fn an_ordinary_window_hears_everything() {
        let events = inputs().into_iter().chain([WindowEvent::Focused(true)]);
        for event in events {
            assert_eq!(
                at_unclaimed(InputOrigin::User, false, &event),
                Unclaimed::Passes,
                "{event:?}"
            );
        }
    }

    #[test]
    fn a_persons_focus_change_waits_for_the_run_to_end() {
        for focused in [true, false] {
            let event = WindowEvent::Focused(focused);
            assert_eq!(held_focus(InputOrigin::User, true, &event), Some(focused));
            assert_eq!(held_focus(InputOrigin::User, false, &event), None);
            assert!(!locks_out(InputOrigin::User, true, &event));
        }
    }
}
