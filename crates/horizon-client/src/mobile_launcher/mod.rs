//! Meridian mobile launcher UI (`docs/launcher-spec.md`):
//! North Star visual identity (`#080310` -> `#170e29`, `#a575f6` / `#a990ff` accents) over
//! `crate::ui_theme` so no prepared pack is required.
//!
//! Shell layout:
//! - Left sidebar (above top bar, under bottom bar): Meridian North Star logo, then Home, Mods,
//!   Skins, and Settings in one stack.
//! - Top bar (right of sidebar): "Meridian Launcher" title, optional background install pill,
//!   version badge (top right).
//! - Bottom dock bar (full width): Player head (account switcher), single-line Profile bar
//!   (opens an upward bar-width Profile dropdown), and Play/Install CTA.

mod auth;
mod home;
mod install;
mod mods;
mod settings;
#[cfg(test)]
mod tests;
mod widgets;

use crate::launcher_backend::{
    Account, Backend, ELY_SKINS, ModSearch, PickedSkin, Profile, ProfileState, Setup, SignIn,
    SkinUpload, VANILLA,
};
use crate::menu_style;
use crate::ui_theme::{self, Face, Ui, color};
use std::path::PathBuf;
use widgets::{
    Canvas, HitRect, RailIcon, Tone, account_provider_tag, account_short, draw_backdrop,
    draw_button, draw_icon_chevron, draw_north_star_badge, draw_play_button,
    draw_player_head_with_dot, draw_rail_item, effective_profile_state, palette,
    profile_stats_line, stage_fraction, stage_name, stage_pill_text, truncate_to_fit,
};

/// Action requested by clicking on [`MobileLauncher`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Prepared files and selected profile are ready: launch the game in `profile_dir`.
    Play,
    /// Open an external web link (for example Microsoft's device-code verification page).
    OpenUrl(String),
    /// Open the platform's file picker for a PNG skin (`MobileLauncher::poll` takes the file).
    PickSkin,
    /// Put this on the clipboard (Copy on the game log).
    CopyText(String),
    /// Close the launcher window.
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Home,
    Install,
    Mods,
    Skins,
    AccountSwitcher,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthForm {
    Offline,
    Ely,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElyField {
    Username,
    Password,
    Otp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SignInProvider {
    Microsoft,
    Ely,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ButtonId {
    // Persistent shell (top, sidebar, bottom)
    GoHome,
    TopProgressBanner,
    OpenMods,
    OpenSkins,
    OpenSettings,
    OpenAccountSwitcher,
    ToggleProfilePicker,
    // Onboarding & Auth
    SignInMicrosoft,
    OpenOfflineEntry,
    ToggleMoreAuth,
    OpenElyEntry,
    CancelAddAccount,
    FocusOfflineName,
    ConfirmOffline,
    CancelOffline,
    FocusElyUsername,
    FocusElyPassword,
    FocusElyOtp,
    ConfirmEly,
    CancelEly,
    OpenBrowserUrl,
    CancelSignIn,
    RetrySignIn,
    // Home & Profiles
    OpenNewProfile,
    SelectProfile(usize),
    ProfileScrollUp,
    ProfileScrollDown,
    InstallSelected,
    PlaySelected,
    ViewInstallScreen,
    Quit,
    // Install screen
    CancelSetup,
    RetrySetup,
    // Account switcher
    SwitchAccount(usize),
    RemoveAccount(usize),
    AccountsPrevPage,
    AccountsNextPage,
    AddAccount,
    // New Profile makes one at once; its tab renames it in place and deletes it.
    RenameTabProfile,
    DeleteTabProfile,
    // Mod manager
    ModManagerPickProfile(usize),
    ToggleProfileMod(usize),
    RemoveProfileMod(usize),
    ProfileModsPrev,
    ProfileModsNext,
    FocusModSearch,
    SubmitModSearch,
    AddRegistryMod(usize),
    RegistryModsPrev,
    RegistryModsNext,
    // Overlays
    CloseProfilePopup,
    CancelRemoveProfile,
    ConfirmRemoveProfile,
    FocusRenameInput,
    SaveRenameProfile,
    // Settings
    SettingsBack,
    ToggleLauncherFont,
    /// Settings: free the install cache (asks first).
    FreeSpace,
    ConfirmFreeSpace,
    CancelFreeSpace,
    // Skins
    PickSkin,
    SkinArms(bool),
    UseSkin,
    OpenElySkins,
    // Home's game log
    LogLevel(u8),
    CopyLog,
}

/// Interactive Meridian launcher backed by [`Backend`].
pub struct MobileLauncher {
    backend: Box<dyn Backend>,
    /// Folder holding `horizon.txt` for persisted launcher UI preferences (`None` in unit tests).
    prefs_dir: Option<PathBuf>,
    route: Route,
    profile_popup_open: bool,
    // Auth & account addition
    adding_account: bool,
    accounts_before_add: usize,
    show_more_auth: bool,
    auth_form: Option<AuthForm>,
    sign_in_provider: SignInProvider,
    offline_name: String,
    offline_error: Option<String>,
    ely_field: ElyField,
    ely_username: String,
    ely_password: String,
    ely_otp: String,
    ely_error: Option<String>,
    status_note: Option<String>,
    // Paging & dropdown scroll
    profile_scroll: usize,
    account_page: usize,
    // Mod manager state
    mods_profile_id: Option<String>,
    mods_list_page: usize,
    mod_search_query: String,
    mod_search_focused: bool,
    mod_registry_page: usize,
    mods_error: Option<String>,
    // Modals
    confirm_remove_profile: Option<(String, String, usize)>,
    confirm_free_space: bool,
    /// What Settings' "Free up space" did: the bytes freed, or why not.
    freed: Option<Result<u64, String>>,
    /// The Mods tab being renamed in place: its id and the name typed so far (empty: the old
    /// name stays).
    profile_rename: Option<(String, String)>,
    /// The skin file chosen on Skins.
    skin_pick: Option<PickedSkin>,
    /// Put the chosen skin on with slim arms (the player's choice; Classic at first).
    skin_slim: bool,
    /// Why the chosen file can't be a skin, or no picker opened.
    skin_error: Option<String>,
    /// Putting `skin_pick` on was started and hasn't finished.
    skin_uploading: bool,
    /// The game log's lines shown from this level up: 0 all, 1 warnings, 2 errors.
    log_level: u8,
    /// The last tap copied the game log.
    log_copied: bool,
}

#[cfg(not(test))]
fn default_prefs_dir() -> Option<PathBuf> {
    std::env::var_os("HORIZON_HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
}

#[cfg(test)]
fn default_prefs_dir() -> Option<PathBuf> {
    None
}

impl MobileLauncher {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        let prefs_dir = default_prefs_dir();
        if let Some(dir) = &prefs_dir {
            ui_theme::load_pixel_font_pref(dir);
        }
        Self {
            backend,
            prefs_dir,
            route: Route::Home,
            profile_popup_open: false,
            adding_account: false,
            accounts_before_add: 0,
            show_more_auth: false,
            auth_form: None,
            sign_in_provider: SignInProvider::Microsoft,
            offline_name: String::new(),
            offline_error: None,
            ely_field: ElyField::Username,
            ely_username: String::new(),
            ely_password: String::new(),
            ely_otp: String::new(),
            ely_error: None,
            status_note: None,
            profile_scroll: 0,
            account_page: 0,
            mods_profile_id: None,
            mods_list_page: 0,
            mod_search_query: String::new(),
            mod_search_focused: false,
            mod_registry_page: 0,
            mods_error: None,
            confirm_remove_profile: None,
            confirm_free_space: false,
            freed: None,
            profile_rename: None,
            skin_pick: None,
            skin_slim: false,
            skin_error: None,
            skin_uploading: false,
            log_level: 0,
            log_copied: false,
        }
    }

    /// Once a frame: takes the skin file the platform's picker handed over.
    pub fn poll(&mut self) {
        match self.backend.take_picked_skin() {
            Some(Ok(pick)) => {
                self.skin_pick = Some(pick);
                self.skin_error = None;
            }
            Some(Err(why)) => self.skin_error = Some(why),
            None => {}
        }
        if self.skin_uploading && self.backend.skin_upload() != SkinUpload::Uploading {
            self.skin_uploading = false;
            // Put on: the account's own head shows it now.
            if self.backend.skin_upload() == SkinUpload::Done {
                self.skin_pick = None;
            }
        }
    }

    /// The platform has no file picker to open.
    pub fn no_file_picker(&mut self) {
        self.skin_error = Some("This device has no file picker.".into());
    }

    /// Bind launcher UI preferences (`horizon.txt`) to an explicit folder and load them immediately.
    pub fn with_prefs_dir(mut self, dir: PathBuf) -> Self {
        ui_theme::load_pixel_font_pref(&dir);
        self.prefs_dir = Some(dir);
        self
    }

    /// Scroll active lists (for example the profile dropdown when more profiles exist than fit).
    /// Positive `delta_y` scrolls upward toward earlier items; negative scrolls downward.
    pub fn scroll(&mut self, delta_y: f32) {
        if !self.profile_popup_open || delta_y.abs() < 0.01 {
            return;
        }
        let max_idx = self.backend.profiles().len().saturating_sub(1);
        if delta_y > 0.0 {
            self.profile_scroll = self.profile_scroll.saturating_sub(1);
        } else {
            self.profile_scroll = (self.profile_scroll + 1).min(max_idx);
        }
    }

    pub fn backend(&self) -> &dyn Backend {
        &*self.backend
    }

    /// True while a background sign-in, setup, profile install, or mod search needs redraws.
    pub fn is_busy(&self) -> bool {
        matches!(
            self.backend.sign_in(),
            SignIn::Starting | SignIn::Code { .. }
        ) || matches!(self.backend.setup(), Setup::Running(_))
            || matches!(self.backend.mod_search(), ModSearch::Searching)
            || self
                .backend
                .profiles()
                .iter()
                .any(|p| matches!(p.state, ProfileState::Installing(_)))
    }

    fn is_onboarding(&self, accounts: &[Account]) -> bool {
        accounts.is_empty() || (self.adding_account && accounts.len() <= self.accounts_before_add)
    }

    fn clear_ely_secrets(&mut self) {
        self.ely_password.clear();
        self.ely_otp.clear();
    }

    fn active_text_mut(&mut self) -> Option<&mut String> {
        if self.profile_popup_open {
            return None;
        }
        if self.profile_rename.is_some() {
            return self.profile_rename.as_mut().map(|(_, text)| text);
        }
        let accounts = self.backend.accounts();
        if self.is_onboarding(&accounts) {
            return match self.auth_form {
                Some(AuthForm::Offline) => Some(&mut self.offline_name),
                Some(AuthForm::Ely) => Some(match self.ely_field {
                    ElyField::Username => &mut self.ely_username,
                    ElyField::Password => &mut self.ely_password,
                    ElyField::Otp => &mut self.ely_otp,
                }),
                None => None,
            };
        }
        match self.route {
            Route::Mods if self.mod_search_focused => Some(&mut self.mod_search_query),
            _ => None,
        }
    }

    fn max_chars_for_active(&self) -> usize {
        if self.profile_rename.is_some() {
            return 32;
        }
        match self.auth_form {
            Some(AuthForm::Offline) => 16,
            Some(AuthForm::Ely) => match self.ely_field {
                ElyField::Username | ElyField::Password => 128,
                ElyField::Otp => 6,
            },
            None => 64,
        }
    }

    fn char_allowed_for_active(&self, ch: char) -> bool {
        if self.profile_rename.is_some() {
            return !ch.is_control();
        }
        if self.auth_form == Some(AuthForm::Offline) {
            return ch.is_ascii_alphanumeric() || ch == '_';
        }
        if self.auth_form == Some(AuthForm::Ely) && self.ely_field == ElyField::Otp {
            return ch.is_ascii_digit();
        }
        !ch.is_control()
    }

    pub fn typed(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\r' || ch == '\n' {
                self.submit_active_form();
                return;
            }
            if ch == '\t' && self.auth_form == Some(AuthForm::Ely) {
                self.ely_field = match self.ely_field {
                    ElyField::Username => ElyField::Password,
                    ElyField::Password => ElyField::Otp,
                    ElyField::Otp => ElyField::Username,
                };
                continue;
            }
            if !self.char_allowed_for_active(ch) {
                continue;
            }
            let max_len = self.max_chars_for_active();
            if let Some(buf) = self.active_text_mut()
                && buf.chars().count() < max_len
            {
                buf.push(ch);
            }
        }
    }

    /// A line for the player on the screen's status note (the last game closed unexpectedly).
    pub fn note(&mut self, text: impl Into<String>) {
        self.status_note = Some(text.into());
    }

    /// Whether a text field has the focus: the phone's keyboard shows for it.
    pub fn wants_text(&mut self) -> bool {
        self.active_text_mut().is_some()
    }

    pub fn backspace(&mut self) {
        if let Some(buf) = self.active_text_mut() {
            buf.pop();
        }
    }

    fn submit_active_form(&mut self) {
        if self.profile_rename.is_some() {
            self.handle_button(ButtonId::SaveRenameProfile);
            return;
        }
        let accounts = self.backend.accounts();
        if self.is_onboarding(&accounts) {
            match self.auth_form {
                Some(AuthForm::Offline) => {
                    self.handle_button(ButtonId::ConfirmOffline);
                }
                Some(AuthForm::Ely) => self.submit_ely(),
                None => {}
            }
            return;
        }
        match self.route {
            Route::Mods if self.mod_search_focused => {
                self.handle_button(ButtonId::SubmitModSearch);
            }
            _ => {}
        }
    }

    fn submit_ely(&mut self) {
        let user = self.ely_username.trim().to_owned();
        if user.is_empty() {
            self.ely_field = ElyField::Username;
            self.ely_error = Some("Enter your Ely.by username or email.".to_owned());
            return;
        }
        if self.ely_password.is_empty() {
            self.ely_field = ElyField::Password;
            self.ely_error = Some("Enter your Ely.by password.".to_owned());
            return;
        }
        let result = self
            .backend
            .sign_in_ely(&user, &self.ely_password, self.ely_otp.trim());
        self.clear_ely_secrets();
        match result {
            Ok(()) => {
                self.ely_error = None;
                self.sign_in_provider = SignInProvider::Ely;
                if self.backend.account().is_some()
                    && !matches!(
                        self.backend.sign_in(),
                        SignIn::Starting | SignIn::Code { .. }
                    )
                {
                    self.auth_form = None;
                    self.adding_account = false;
                    self.route = Route::Home;
                }
            }
            Err(err) => {
                self.ely_error = Some(err);
            }
        }
    }

    fn selected_profile_snapshot(&self) -> (Vec<Profile>, String, usize) {
        let profiles = self.backend.profiles();
        let selected_id = self.backend.selected_profile();
        let idx = profiles
            .iter()
            .position(|p| p.id == selected_id)
            .unwrap_or(0);
        let id = profiles
            .get(idx)
            .map_or_else(|| VANILLA.to_owned(), |p| p.id.clone());
        (profiles, id, idx)
    }

    pub(super) fn effective_mods_profile_id(&self, profiles: &[Profile]) -> String {
        if let Some(id) = &self.mods_profile_id
            && profiles.iter().any(|p| &p.id == id)
        {
            return id.clone();
        }
        let sel = self.backend.selected_profile();
        if sel != VANILLA && profiles.iter().any(|p| p.id == sel) {
            return sel;
        }
        profiles
            .iter()
            .find(|p| p.id != VANILLA)
            .or_else(|| profiles.first())
            .map_or_else(|| VANILLA.to_owned(), |p| p.id.clone())
    }

    pub fn draw(&self, ui: &mut Ui) {
        self.layout(&mut Canvas::Draw(ui));
    }

    pub fn click(&mut self, ui: &Ui) -> Option<Action> {
        let hits = self.layout(&mut Canvas::Hit(ui));
        let id = hits
            .iter()
            .rev()
            .find(|h| h.contains(ui.cursor))
            .map(|h| h.id)?;
        self.handle_button(id)
    }

    fn handle_button(&mut self, id: ButtonId) -> Option<Action> {
        if self.adding_account && self.backend.accounts().len() > self.accounts_before_add {
            self.adding_account = false;
            self.auth_form = None;
        }
        // Opening New Profile focuses its name; tapping anything else (a mod, a preset) doesn't
        // keep the keyboard up.
        // A tap anywhere else keeps the name being typed on a Mods tab.
        if !matches!(id, ButtonId::FocusRenameInput | ButtonId::SaveRenameProfile)
            && let Some((tab, name)) = self.profile_rename.take()
            && !name.trim().is_empty()
            && let Err(err) = self.backend.rename_profile(&tab, name.trim())
        {
            self.mods_error = Some(err);
        }
        // Any other tap answers "Free up space?" with no.
        self.confirm_free_space = matches!(id, ButtonId::FreeSpace | ButtonId::ConfirmFreeSpace);
        self.log_copied = false;

        match id {
            ButtonId::GoHome | ButtonId::SettingsBack => {
                self.profile_popup_open = false;
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                self.mod_search_focused = false;
                self.route = Route::Home;
            }
            ButtonId::ToggleProfilePicker => {
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                self.mod_search_focused = false;
                self.profile_popup_open = !self.profile_popup_open;
                if self.profile_popup_open {
                    let (_, _, sel_idx) = self.selected_profile_snapshot();
                    self.profile_scroll = sel_idx.saturating_sub(2);
                }
            }
            ButtonId::CloseProfilePopup => {
                self.profile_popup_open = false;
            }
            ButtonId::FreeSpace | ButtonId::CancelFreeSpace => {}
            ButtonId::ConfirmFreeSpace => {
                self.confirm_free_space = false;
                self.freed = Some(self.backend.free_space());
            }
            ButtonId::ToggleLauncherFont => {
                let next = !ui_theme::pixel_font();
                if let Some(dir) = &self.prefs_dir {
                    ui_theme::save_pixel_font_pref(dir, next);
                } else {
                    ui_theme::set_pixel_font(next);
                }
            }
            ButtonId::TopProgressBanner | ButtonId::ViewInstallScreen => {
                self.profile_popup_open = false;
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                if matches!(
                    self.backend.setup(),
                    Setup::Running(_) | Setup::Failed { .. }
                ) {
                    self.route = Route::Install;
                } else {
                    self.route = Route::Home;
                }
            }
            ButtonId::OpenMods => {
                self.profile_popup_open = false;
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                self.mods_profile_id = Some(self.backend.selected_profile());
                self.mods_list_page = 0;
                self.mod_registry_page = 0;
                self.mods_error = None;
                self.mod_search_focused = false;
                if matches!(self.backend.mod_search(), ModSearch::Idle) {
                    self.backend.search_mods(&self.mod_search_query);
                }
                self.route = Route::Mods;
            }
            ButtonId::OpenSkins => {
                self.profile_popup_open = false;
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                self.mod_search_focused = false;
                self.route = Route::Skins;
            }
            ButtonId::OpenSettings => {
                self.profile_popup_open = false;
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                self.mod_search_focused = false;
                self.route = if self.route == Route::Settings {
                    Route::Home
                } else {
                    Route::Settings
                };
            }
            ButtonId::OpenAccountSwitcher => {
                self.profile_popup_open = false;
                self.confirm_remove_profile = None;
                self.profile_rename = None;
                self.mod_search_focused = false;
                self.route = if self.route == Route::AccountSwitcher {
                    Route::Home
                } else {
                    Route::AccountSwitcher
                };
            }
            ButtonId::SignInMicrosoft => {
                self.sign_in_provider = SignInProvider::Microsoft;
                self.auth_form = None;
                self.offline_error = None;
                self.ely_error = None;
                self.clear_ely_secrets();
                self.status_note = None;
                self.backend.sign_in_microsoft();
            }
            ButtonId::OpenOfflineEntry => {
                if !self.adding_account
                    && matches!(
                        self.backend.sign_in(),
                        SignIn::Starting | SignIn::Code { .. } | SignIn::Failed(_)
                    )
                {
                    self.backend.sign_out();
                }
                self.ely_error = None;
                self.clear_ely_secrets();
                self.offline_error = None;
                self.auth_form = Some(AuthForm::Offline);
                self.status_note = None;
            }
            ButtonId::ToggleMoreAuth => {
                self.show_more_auth = !self.show_more_auth;
            }
            ButtonId::OpenElyEntry => {
                if !self.adding_account
                    && matches!(
                        self.backend.sign_in(),
                        SignIn::Starting | SignIn::Code { .. } | SignIn::Failed(_)
                    )
                {
                    self.backend.sign_out();
                }
                self.offline_error = None;
                self.ely_error = None;
                self.ely_field = ElyField::Username;
                self.clear_ely_secrets();
                self.auth_form = Some(AuthForm::Ely);
                self.status_note = None;
            }
            ButtonId::CancelAddAccount => {
                self.adding_account = false;
                self.auth_form = None;
                self.clear_ely_secrets();
                self.route = Route::Home;
            }
            ButtonId::FocusOfflineName => {}
            ButtonId::ConfirmOffline => {
                let name = self.offline_name.trim().to_owned();
                match self.backend.sign_in_offline(&name) {
                    Ok(_) => {
                        self.auth_form = None;
                        self.offline_error = None;
                        self.adding_account = false;
                        self.show_more_auth = false;
                        self.status_note = None;
                        self.route = Route::Home;
                    }
                    Err(err) => {
                        self.offline_error = Some(err);
                    }
                }
            }
            ButtonId::CancelOffline => {
                self.auth_form = None;
                self.offline_error = None;
            }
            ButtonId::FocusElyUsername => self.ely_field = ElyField::Username,
            ButtonId::FocusElyPassword => self.ely_field = ElyField::Password,
            ButtonId::FocusElyOtp => self.ely_field = ElyField::Otp,
            ButtonId::ConfirmEly => self.submit_ely(),
            ButtonId::CancelEly => {
                self.auth_form = None;
                self.ely_error = None;
                self.clear_ely_secrets();
            }
            ButtonId::OpenBrowserUrl => {
                if let SignIn::Code { url, .. } = self.backend.sign_in() {
                    self.status_note = Some(format!("Open {url} and enter the code."));
                    return Some(Action::OpenUrl(url));
                }
            }
            ButtonId::CancelSignIn => {
                if self.backend.account().is_none() {
                    self.backend.sign_out();
                }
                self.status_note = None;
                self.adding_account = false;
                if self.sign_in_provider == SignInProvider::Ely {
                    self.auth_form = Some(AuthForm::Ely);
                    self.ely_error = None;
                    self.clear_ely_secrets();
                }
            }
            ButtonId::RetrySignIn => {
                self.status_note = None;
                match self.sign_in_provider {
                    SignInProvider::Microsoft => self.backend.sign_in_microsoft(),
                    SignInProvider::Ely => {
                        self.auth_form = Some(AuthForm::Ely);
                        self.ely_field = ElyField::Password;
                        self.ely_error = None;
                        self.clear_ely_secrets();
                    }
                }
            }
            ButtonId::SelectProfile(idx) => {
                if let Some(p) = self.backend.profiles().get(idx) {
                    let _ = self.backend.select_profile(&p.id);
                    self.status_note = None;
                    self.profile_popup_open = false;
                }
            }
            ButtonId::ProfileScrollUp => {
                self.profile_scroll = self.profile_scroll.saturating_sub(1);
            }
            ButtonId::ProfileScrollDown => {
                let max_idx = self.backend.profiles().len().saturating_sub(1);
                self.profile_scroll = (self.profile_scroll + 1).min(max_idx);
            }
            ButtonId::InstallSelected => {
                self.profile_popup_open = false;
                let (profiles, sel_id, _) = self.selected_profile_snapshot();
                let setup = self.backend.setup();
                if sel_id == VANILLA || !matches!(setup, Setup::Ready) {
                    if !matches!(setup, Setup::Running(_)) {
                        self.backend.install_profile(VANILLA);
                        if matches!(self.backend.setup(), Setup::Needed | Setup::Failed { .. }) {
                            self.backend.start_setup();
                        }
                    }
                    self.route = Route::Install;
                } else if profiles.iter().any(|p| p.id == sel_id) {
                    self.backend.install_profile(&sel_id);
                }
            }
            ButtonId::PlaySelected => {
                self.profile_popup_open = false;
                let (profiles, sel_id, _) = self.selected_profile_snapshot();
                let setup = self.backend.setup();
                let ready = matches!(setup, Setup::Ready)
                    && profiles.iter().find(|p| p.id == sel_id).is_some_and(|p| {
                        effective_profile_state(p, &setup) == ProfileState::Installed
                    });
                if ready {
                    return Some(Action::Play);
                }
            }
            ButtonId::RetrySetup => {
                self.status_note = None;
                self.backend.start_setup();
                self.route = Route::Install;
            }
            ButtonId::CancelSetup => {
                self.backend.cancel_setup();
                self.route = Route::Home;
            }
            ButtonId::SwitchAccount(idx) => {
                if let Err(err) = self.backend.switch_account(idx) {
                    self.status_note = Some(err);
                } else {
                    self.status_note = None;
                    self.route = Route::Home;
                }
            }
            ButtonId::RemoveAccount(idx) => {
                if let Err(err) = self.backend.remove_account(idx) {
                    self.status_note = Some(err);
                } else if self.backend.accounts().is_empty() {
                    self.route = Route::Home;
                    self.adding_account = false;
                }
            }
            ButtonId::AccountsPrevPage => {
                self.account_page = self.account_page.saturating_sub(1);
            }
            ButtonId::AccountsNextPage => {
                self.account_page = self.account_page.saturating_add(1);
            }
            ButtonId::AddAccount => {
                self.accounts_before_add = self.backend.accounts().len();
                self.adding_account = true;
                self.route = Route::Home;
                self.auth_form = None;
                self.show_more_auth = false;
                self.offline_error = None;
                self.ely_error = None;
                self.clear_ely_secrets();
            }
            ButtonId::OpenNewProfile => {
                self.profile_popup_open = false;
                let profiles = self.backend.profiles();
                // "Profile 1", "Profile 2"...: the first name no profile has (its folder is made
                // unique by `profile_id`).
                let name = (1..)
                    .map(|n| format!("Profile {n}"))
                    .find(|name| profiles.iter().all(|p| !p.name.eq_ignore_ascii_case(name)))
                    .unwrap_or_default();
                match self.backend.create_profile(&name, None, &[]) {
                    Ok(id) => {
                        let _ = self.backend.select_profile(&id);
                        self.route = Route::Mods;
                        self.mods_profile_id = Some(id.clone());
                        self.mods_list_page = 0;
                        self.mods_error = None;
                        // Its name, ready to type over; tapping away keeps "Profile 1".
                        self.profile_rename = Some((id, String::new()));
                    }
                    Err(err) => self.mods_error = Some(err),
                }
            }
            ButtonId::ModManagerPickProfile(idx) => {
                if let Some(p) = self.backend.profiles().get(idx) {
                    self.mods_profile_id = Some(p.id.clone());
                    self.mods_list_page = 0;
                    self.mods_error = None;
                }
            }
            ButtonId::ToggleProfileMod(idx) => {
                let profiles = self.backend.profiles();
                let pid = self.effective_mods_profile_id(&profiles);
                if let Some(pm) = self.backend.mods(&pid).get(idx)
                    && let Err(err) = self.backend.set_mod_enabled(&pid, &pm.info.id, !pm.enabled)
                {
                    self.mods_error = Some(err);
                }
            }
            ButtonId::RemoveProfileMod(idx) => {
                let profiles = self.backend.profiles();
                let pid = self.effective_mods_profile_id(&profiles);
                if let Some(pm) = self.backend.mods(&pid).get(idx)
                    && let Err(err) = self.backend.remove_mod(&pid, &pm.info.id)
                {
                    self.mods_error = Some(err);
                }
            }
            ButtonId::ProfileModsPrev => {
                self.mods_list_page = self.mods_list_page.saturating_sub(1);
            }
            ButtonId::ProfileModsNext => {
                self.mods_list_page = self.mods_list_page.saturating_add(1);
            }
            ButtonId::FocusModSearch => {
                self.mod_search_focused = true;
            }
            ButtonId::SubmitModSearch => {
                self.mod_search_focused = true;
                self.mod_registry_page = 0;
                self.backend.search_mods(self.mod_search_query.trim());
            }
            ButtonId::AddRegistryMod(idx) => {
                let profiles = self.backend.profiles();
                let pid = self.effective_mods_profile_id(&profiles);
                if let ModSearch::Done(mods) = self.backend.mod_search()
                    && let Some(m) = mods.get(idx)
                {
                    match self.backend.add_mod(&pid, &m.id) {
                        Ok(()) => self.mods_error = None,
                        Err(err) => self.mods_error = Some(err),
                    }
                }
            }
            ButtonId::RegistryModsPrev => {
                self.mod_registry_page = self.mod_registry_page.saturating_sub(1);
            }
            ButtonId::RegistryModsNext => {
                self.mod_registry_page = self.mod_registry_page.saturating_add(1);
            }
            ButtonId::RenameTabProfile => {
                let pid = self.effective_mods_profile_id(&self.backend.profiles());
                if pid != VANILLA {
                    self.profile_rename = Some((pid, String::new()));
                }
            }
            ButtonId::FocusRenameInput => {}
            ButtonId::SaveRenameProfile => {
                if let Some((tab, name)) = self.profile_rename.take()
                    && !name.trim().is_empty()
                {
                    match self.backend.rename_profile(&tab, name.trim()) {
                        Ok(()) => self.mods_error = None,
                        Err(err) => {
                            // Still typing: the reason shows under the tabs.
                            self.mods_error = Some(err);
                            self.profile_rename = Some((tab, name));
                        }
                    }
                }
            }
            ButtonId::DeleteTabProfile => {
                let profiles = self.backend.profiles();
                let pid = self.effective_mods_profile_id(&profiles);
                if pid != VANILLA
                    && let Some(p) = profiles.iter().find(|p| p.id == pid)
                {
                    self.confirm_remove_profile = Some((p.id.clone(), p.name.clone(), p.worlds));
                }
            }
            ButtonId::CancelRemoveProfile => {
                self.confirm_remove_profile = None;
            }
            ButtonId::ConfirmRemoveProfile => {
                if let Some((id, name, _)) = self.confirm_remove_profile.take() {
                    match self.backend.remove_profile(&id) {
                        Ok(()) => {
                            self.status_note =
                                Some(format!("Removed {name} and its saved worlds."));
                        }
                        Err(err) => {
                            self.status_note = Some(err);
                        }
                    }
                }
            }
            ButtonId::Quit => return Some(Action::Quit),
            ButtonId::PickSkin => {
                self.skin_error = None;
                return Some(Action::PickSkin);
            }
            ButtonId::SkinArms(slim) => self.skin_slim = slim,
            ButtonId::UseSkin => {
                if let Some(pick) = &self.skin_pick
                    && !self.skin_uploading
                {
                    self.backend.use_skin(pick.png.clone(), self.skin_slim);
                    self.skin_uploading = true;
                }
            }
            ButtonId::OpenElySkins => return Some(Action::OpenUrl(ELY_SKINS.into())),
            ButtonId::LogLevel(level) => self.log_level = level,
            ButtonId::CopyLog => {
                let lines = self.backend.game_log(&self.backend.selected_profile());
                let shown: Vec<&str> = lines
                    .iter()
                    .zip(widgets::log_levels(&lines))
                    .filter(|(_, level)| *level >= self.log_level)
                    .map(|(line, _)| line.as_str())
                    .collect();
                self.log_copied = true;
                return Some(Action::CopyText(shown.join("\n")));
            }
        }
        None
    }

    fn layout(&self, ui: &mut Canvas<'_, '_>) -> Vec<HitRect> {
        let mut hits = Vec::new();
        let styles = &menu_style::get().settings;
        let col_success = color::hex(styles.success, 255);
        let col_warn = color::hex(styles.warn, 255);
        let col_danger = color::hex(styles.danger, 255);

        let (ui_w, ui_h) = (ui.ro().w, ui.ro().h);
        if let Some(p) = ui.paint() {
            draw_backdrop(p, ui_w, ui_h);
        }

        let kb_h = ui_h < 300.0;
        let short_h = ui_h < 580.0;
        let compact_w = ui_w < 860.0;
        let wide_w = ui_w >= 1200.0;
        let pad = if kb_h {
            10.0
        } else if compact_w {
            12.0
        } else if short_h {
            14.0
        } else {
            18.0
        };
        let top_h = if kb_h {
            34.0
        } else if ui_h < 380.0 {
            42.0
        } else if short_h {
            46.0
        } else {
            54.0
        };

        let accounts = self.backend.accounts();
        let active_idx = self.backend.active_account();
        let account = active_idx.and_then(|i| accounts.get(i).cloned());
        let onboarding = self.is_onboarding(&accounts);
        let setup = self.backend.setup();
        let sign_in = self.backend.sign_in();
        let profiles = self.backend.profiles();

        if onboarding {
            self.draw_onboarding_top_bar(ui, &mut hits, ui_w, pad, top_h);
            let v_margin = if kb_h {
                4.0
            } else if short_h {
                8.0
            } else {
                18.0
            };
            let content_top = top_h + v_margin;
            let content_h = (ui_h - content_top - v_margin).max(0.0);
            let card_w = (ui_w - pad * 2.0).min(if wide_w { 760.0 } else { 640.0 });
            let card_x = ((ui_w - card_w) * 0.5).round();

            if self.route == Route::Settings {
                self.draw_settings_view(
                    ui,
                    &mut hits,
                    card_x,
                    content_top,
                    card_w,
                    content_h,
                    true,
                    &account,
                    &setup,
                    &profiles,
                    col_success,
                    col_danger,
                );
                return hits;
            }
            match &sign_in {
                SignIn::Starting | SignIn::Code { .. } | SignIn::Failed(_)
                    if account.is_none() || self.adding_account =>
                {
                    self.draw_signin_progress(
                        ui,
                        &mut hits,
                        card_x,
                        content_top,
                        card_w,
                        content_h,
                        &sign_in,
                        col_warn,
                    );
                    return hits;
                }
                _ => {}
            }
            match self.auth_form {
                Some(AuthForm::Offline) => self.draw_offline_entry(
                    ui,
                    &mut hits,
                    card_x,
                    content_top,
                    card_w,
                    content_h,
                    col_danger,
                ),
                Some(AuthForm::Ely) => self.draw_ely_entry(
                    ui,
                    &mut hits,
                    card_x,
                    content_top,
                    card_w,
                    content_h,
                    col_danger,
                ),
                None => self.draw_welcome(
                    ui,
                    &mut hits,
                    card_x,
                    content_top,
                    card_w,
                    content_h,
                    !accounts.is_empty() && self.adding_account,
                ),
            }
            return hits;
        }

        // ---- Signed-in Shell: Left Sidebar (above top bar, under bottom bar) + Top Bar + Full-Width Bottom Dock ----
        // While the keyboard is up for a field, the form gets the room above it: no rail, and no
        // dock rising behind it with the account switcher.
        // ponytail: "keyboard up" is the keyboard-lifted height (kb_h); a tablet's keyboard leaves
        // more room and keeps the dock. Pass the keyboard's height from the host if that matters.
        let typing = kb_h
            && !self.profile_popup_open
            && (self.profile_rename.is_some()
                || self.route == Route::Mods && self.mod_search_focused);
        let bottom_h = if typing {
            0.0
        } else if kb_h {
            46.0
        } else if ui_h < 380.0 {
            54.0
        } else if short_h {
            58.0
        } else {
            68.0
        };
        let rail_w = if typing {
            0.0
        } else if kb_h {
            58.0
        } else if compact_w {
            64.0
        } else if short_h {
            68.0
        } else {
            72.0
        };
        let rail_h = (ui_h - bottom_h).max(0.0);

        let mut bg_hits = Vec::new();

        // Top bar drawn first (right of left panel); left panel drawn above it; bottom dock caps full width on top.
        {
            let chrome_hits = if self.profile_popup_open {
                &mut bg_hits
            } else {
                &mut hits
            };
            self.draw_workspace_header(
                ui,
                chrome_hits,
                rail_w,
                ui_w,
                pad,
                top_h,
                &setup,
                &profiles,
            );
            if !typing {
                self.draw_rail(ui, chrome_hits, rail_w, rail_h, top_h, short_h);
            }
        }
        let (prof_x, prof_w) = if typing {
            (0.0, 0.0)
        } else {
            self.draw_bottom_bar(
                ui,
                &mut hits,
                ui_w,
                ui_h - bottom_h,
                bottom_h,
                pad,
                compact_w,
                &account,
                &setup,
                &profiles,
                col_success,
                col_warn,
                col_danger,
            )
        };

        let v_margin = if kb_h {
            4.0
        } else if short_h {
            6.0
        } else {
            12.0
        };
        let main_x = rail_w + pad;
        let main_y = top_h + v_margin;
        let main_w = (ui_w - rail_w - pad * 2.0).max(280.0);
        let main_h = (rail_h - top_h - v_margin * 2.0).max(0.0);
        let modal_w = main_w.min(if wide_w { 720.0 } else { 600.0 });
        let modal_x = main_x + ((main_w - modal_w) * 0.5).round();

        let top_hits = if self.profile_popup_open {
            &mut bg_hits
        } else {
            &mut hits
        };

        if self.confirm_free_space {
            self.draw_free_space_confirm(ui, top_hits, modal_x, main_y, modal_w, main_h);
            return hits;
        }
        if let Some((_, ref name, worlds)) = self.confirm_remove_profile {
            self.draw_remove_confirm(
                ui, top_hits, modal_x, main_y, modal_w, main_h, name, worlds, col_danger,
            );
            return hits;
        }

        match self.route {
            Route::Install => match setup {
                Setup::Running(ref progress) => self.draw_setup_running(
                    ui,
                    top_hits,
                    modal_x,
                    main_y,
                    modal_w,
                    main_h,
                    progress,
                    col_success,
                ),
                Setup::Failed { stage, ref error } => self.draw_setup_failed(
                    ui, top_hits, modal_x, main_y, modal_w, main_h, stage, error, col_danger,
                ),
                Setup::Needed | Setup::Ready => self.draw_home(
                    ui,
                    top_hits,
                    main_x,
                    main_y,
                    main_w,
                    main_h,
                    &setup,
                    &profiles,
                    col_success,
                    col_danger,
                ),
            },
            Route::AccountSwitcher => self.draw_account_switcher(
                ui,
                top_hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &accounts,
                active_idx,
                col_success,
            ),
            Route::Mods => self.draw_mods_view(
                ui,
                top_hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &profiles,
                col_success,
                col_danger,
            ),
            Route::Skins => {
                self.draw_skins_view(
                    ui,
                    top_hits,
                    main_x,
                    main_y,
                    main_w,
                    main_h,
                    &account,
                    col_success,
                    col_danger,
                );
            }
            Route::Settings => self.draw_settings_view(
                ui,
                top_hits,
                main_x,
                main_y,
                main_w,
                main_h,
                false,
                &account,
                &setup,
                &profiles,
                col_success,
                col_danger,
            ),
            Route::Home => self.draw_home(
                ui,
                top_hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &setup,
                &profiles,
                col_success,
                col_danger,
            ),
        }

        if self.profile_popup_open {
            self.draw_profile_popup(
                ui,
                &mut hits,
                ui_w,
                top_h,
                ui_h - bottom_h,
                prof_x,
                prof_w,
                &setup,
                &profiles,
                col_success,
                col_danger,
            );
        }
        hits
    }

    fn draw_onboarding_top_bar(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        ui_w: f32,
        pad: f32,
        top_h: f32,
    ) {
        let tight_top = top_h < 40.0;
        let btn_h = (top_h - if tight_top { 6.0 } else { 10.0 }).clamp(26.0, 42.0);
        let btn_y = ((top_h - btn_h) * 0.5).round();
        let settings_w =
            (ui.ro().measure(Face::Medium, ui_theme::body(), "Settings") + 24.0).max(86.0);
        if let Some(p) = ui.paint() {
            p.rounded(0.0, 0.0, ui_w, top_h, 0.0, palette::rail());
            p.rounded(0.0, top_h - 1.0, ui_w, 1.0, 0.0, palette::border_subtle());
            let badge_sz = (top_h - if tight_top { 8.0 } else { 14.0 }).clamp(24.0, 36.0);
            draw_north_star_badge(p, pad, ((top_h - badge_sz) * 0.5).round(), badge_sz);
            let title_sz = if tight_top { 15.5 } else { 18.5 };
            p.text_line(
                Face::SemiBold,
                title_sz,
                top_h,
                pad + badge_sz + 10.0,
                0.0,
                palette::text(),
                "Meridian Launcher",
            );
            let ver = concat!("Minecraft 26.3   v", env!("CARGO_PKG_VERSION"));
            let vw = p.measure(Face::Mono, 11.5, ver);
            p.text_line(
                Face::Mono,
                11.5,
                top_h,
                ui_w - pad - settings_w - 14.0 - vw,
                0.0,
                palette::muted(),
                ver,
            );
        }
        draw_button(
            ui,
            hits,
            ButtonId::OpenSettings,
            ui_w - pad - settings_w,
            btn_y,
            settings_w,
            btn_h,
            if self.route == Route::Settings {
                Tone::Banner
            } else {
                Tone::Ghost
            },
            "Settings",
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_workspace_header(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        rail_w: f32,
        ui_w: f32,
        pad: f32,
        top_h: f32,
        setup: &Setup,
        profiles: &[Profile],
    ) {
        let tight_top = top_h < 40.0;
        let ver = concat!("Minecraft 26.3   v", env!("CARGO_PKG_VERSION"));
        let ver_pill_h = (top_h - if tight_top { 10.0 } else { 18.0 }).clamp(22.0, 28.0);
        let ver_pill_y = ((top_h - ver_pill_h) * 0.5).round();
        let ver_pill_w = ui.ro().measure(Face::Mono, 11.5, ver) + 22.0;
        let ver_pill_x = ui_w - pad - ver_pill_w;
        let title_sz = if tight_top {
            15.5
        } else if top_h < 50.0 {
            17.5
        } else {
            19.5
        };

        if let Some(p) = ui.paint() {
            // Top bar spans only to the right of the left rail so the left rail sits above it.
            p.rounded(rail_w, 0.0, ui_w - rail_w, top_h, 0.0, palette::rail());
            p.rounded(
                rail_w,
                top_h - 1.0,
                ui_w - rail_w,
                1.0,
                0.0,
                palette::border_subtle(),
            );
            p.text_line(
                Face::SemiBold,
                title_sz,
                top_h,
                rail_w + pad,
                0.0,
                palette::text(),
                "Meridian Launcher",
            );

            // Top right: the version, plain.
            p.text_line(
                Face::Mono,
                11.5,
                ver_pill_h,
                ver_pill_x + 22.0,
                ver_pill_y,
                palette::muted(),
                ver,
            );
        }

        // Background progress pill to the left of the version badge while an install runs off-screen.
        let bg_progress = if self.route != Route::Install {
            match setup {
                Setup::Running(p) => Some(stage_pill_text(p)),
                _ => profiles.iter().find_map(|p| match &p.state {
                    ProfileState::Installing(prog) => {
                        let pct = (stage_fraction(prog) * 100.0).round() as u32;
                        Some(format!("{} {pct}%", p.name))
                    }
                    _ => None,
                }),
            }
        } else {
            None
        };
        if let Some(summary) = bg_progress {
            let btn_h = (top_h - if tight_top { 6.0 } else { 14.0 }).clamp(26.0, 36.0);
            let btn_y = ((top_h - btn_h) * 0.5).round();
            let wordmark_w = ui
                .ro()
                .measure(Face::SemiBold, title_sz, "Meridian Launcher");
            let max_pill_w = (ver_pill_x - 10.0 - (rail_w + pad + wordmark_w + 16.0)).max(110.0);
            let pill_w =
                (ui.ro().measure(Face::Mono, ui_theme::small(), &summary) + 24.0).min(max_pill_w);
            draw_button(
                ui,
                hits,
                ButtonId::TopProgressBanner,
                ver_pill_x - 10.0 - pill_w,
                btn_y,
                pill_w,
                btn_h,
                Tone::Banner,
                &summary,
            );
        }
    }

    fn draw_rail(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        w: f32,
        h: f32,
        top_h: f32,
        short_h: bool,
    ) {
        if let Some(p) = ui.paint() {
            // Left panel spans from y=0 (above the top bar) down to the top edge of the bottom dock.
            p.rounded(0.0, 0.0, w, h, 0.0, palette::rail());
            p.rounded(w - 1.0, 0.0, 1.0, h, 0.0, palette::border_subtle());
        }

        // Top of left panel: integrated Meridian North Star logo tile
        let tight_rail = h < 275.0;
        let ultra_tight = h < 190.0;
        let badge_sz = if ultra_tight {
            26.0
        } else if tight_rail {
            32.0
        } else if short_h {
            36.0
        } else {
            42.0
        };
        let badge_x = ((w - badge_sz) * 0.5).round();
        let badge_y = ((top_h - badge_sz) * 0.5).max(4.0).round();
        let logo_hit = HitRect {
            id: ButtonId::GoHome,
            x: badge_x,
            y: badge_y,
            w: badge_sz,
            h: badge_sz,
        };
        if let Some(p) = ui.paint() {
            draw_north_star_badge(p, badge_x, badge_y, badge_sz);
            let div_y = if ultra_tight {
                badge_y + badge_sz + 4.0
            } else {
                (badge_y + badge_sz + if tight_rail { 5.0 } else { 8.0 }).max(top_h - 1.0)
            };
            p.rounded(12.0, div_y, w - 24.0, 1.0, 0.0, palette::border_subtle());
        }
        hits.push(logo_hit);

        let item_w = (w - 14.0).clamp(44.0, 56.0);
        let item_x = ((w - item_w) * 0.5).round();
        let mut sy = if ultra_tight {
            badge_y + badge_sz + 8.0
        } else {
            (badge_y + badge_sz + if tight_rail { 10.0 } else { 14.0 }).max(top_h + 8.0)
        };
        let gap: f32 = if ultra_tight {
            3.0
        } else if tight_rail {
            4.0
        } else if short_h {
            6.0
        } else {
            8.0
        };
        let bot_pad: f32 = if ultra_tight {
            4.0
        } else if tight_rail {
            6.0
        } else if short_h {
            8.0
        } else {
            14.0
        };
        let avail_rail = (h - sy - bot_pad).max(96.0);
        let max_item_h = ((avail_rail - gap * 3.0) * 0.25).floor().max(22.0);
        let item_h = (if short_h { 46.0_f32 } else { 52.0 }).min(max_item_h);

        // Single clean nav stack: Home, Mods, Skins, Settings (right after Skins).
        let page = self.route;
        let nav_items = [
            (
                ButtonId::GoHome,
                RailIcon::Home,
                "Home",
                matches!(page, Route::Home | Route::Install),
            ),
            (
                ButtonId::OpenMods,
                RailIcon::Mods,
                "Mods",
                page == Route::Mods,
            ),
            (
                ButtonId::OpenSkins,
                RailIcon::Skins,
                "Skins",
                self.route == Route::Skins,
            ),
            (
                ButtonId::OpenSettings,
                RailIcon::Settings,
                "Settings",
                self.route == Route::Settings,
            ),
        ];
        for (id, icon, label, active) in nav_items {
            draw_rail_item(
                ui, hits, id, item_x, sy, item_w, item_h, active, icon, label,
            );
            sy += item_h + gap;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_bottom_bar(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        ui_w: f32,
        y: f32,
        h: f32,
        pad: f32,
        compact_w: bool,
        account: &Option<Account>,
        setup: &Setup,
        profiles: &[Profile],
        col_success: u32,
        col_warn: u32,
        col_danger: u32,
    ) -> (f32, f32) {
        if let Some(p) = ui.paint() {
            p.rounded(0.0, y, ui_w, h, 0.0, palette::rail());
            p.rounded(0.0, y, ui_w, 1.0, 0.0, palette::border_subtle());
        }

        let tight_dock = h < 54.0;
        let bar_h = (h - if tight_dock { 8.0 } else { 14.0 }).clamp(36.0, 50.0);
        let by = y + ((h - bar_h) * 0.5).round();
        let gap = if compact_w { 10.0 } else { 14.0 };
        let acct_w = if compact_w { 168.0 } else { 200.0 };
        let play_w = if compact_w { 156.0 } else { 180.0 };
        let play_x = ui_w - pad - play_w;

        let mid_left = pad + acct_w + gap;
        let mid_right = play_x - gap;
        let mid_avail = (mid_right - mid_left).max(160.0);
        let prof_w = mid_avail.min(if compact_w { 390.0 } else { 448.0 });
        let ideal_prof_x = ((ui_w - prof_w) * 0.5).round();
        let prof_x = ideal_prof_x.clamp(mid_left, (mid_right - prof_w).max(mid_left));

        // 1. Bottom-left: Borderless Player Head + Account Switcher
        let acct_hit = HitRect {
            id: ButtonId::OpenAccountSwitcher,
            x: pad,
            y: by,
            w: acct_w,
            h: bar_h,
        };
        let (acct_name, acct_sub, dot_col) = match account {
            Some(a @ Account::Microsoft { .. }) => {
                (account_short(a), account_provider_tag(a), col_success)
            }
            Some(a @ Account::Ely { .. }) => {
                (account_short(a), account_provider_tag(a), col_success)
            }
            Some(a @ Account::Offline { .. }) => {
                (account_short(a), account_provider_tag(a), col_warn)
            }
            None => ("Account", "Not signed in", palette::muted()),
        };
        if let Some(p) = ui.paint() {
            let active = self.route == Route::AccountSwitcher;
            let hovered = acct_hit.contains(p.cursor);
            if active || hovered {
                p.rounded(
                    pad,
                    by,
                    acct_w,
                    bar_h,
                    10.0,
                    if active {
                        color::with_alpha(palette::accent(), 0.16)
                    } else {
                        palette::surface()
                    },
                );
                if active {
                    p.border(
                        pad,
                        by,
                        acct_w,
                        bar_h,
                        10.0,
                        color::with_alpha(palette::accent_star(), 0.65),
                    );
                }
            }

            let head_sz = (bar_h - 8.0).clamp(28.0, 38.0);
            let head_y = by + ((bar_h - head_sz) * 0.5).round();
            let head = account.as_ref().and_then(|a| self.backend.player_head(a));
            draw_player_head_with_dot(
                p,
                pad + 6.0,
                head_y,
                head_sz,
                head.as_ref(),
                acct_name,
                dot_col,
            );

            let tx = pad + head_sz + 14.0;
            let max_tw = (acct_w - head_sz - 24.0).max(40.0);
            let fit_n = truncate_to_fit(p, Face::SemiBold, 13.5, acct_name, max_tw - 14.0);
            let nw = p.measure(Face::SemiBold, 13.5, &fit_n);
            let name_y = by
                + if bar_h < 42.0 {
                    3.0
                } else if bar_h < 46.0 {
                    5.0
                } else {
                    7.0
                };
            let name_h = if bar_h < 42.0 { 16.0 } else { 18.0 };
            p.text_line(
                Face::SemiBold,
                13.5,
                name_h,
                tx,
                name_y,
                palette::text(),
                &fit_n,
            );
            draw_icon_chevron(
                p,
                tx + nw + 8.0,
                name_y + name_h * 0.5,
                6.0,
                palette::muted(),
                active,
            );

            let fit_sub = truncate_to_fit(p, Face::Regular, 10.5, acct_sub, max_tw);
            p.text_line(
                Face::Regular,
                10.5,
                14.0,
                tx,
                name_y + name_h,
                palette::muted(),
                &fit_sub,
            );
        }
        hits.push(acct_hit);

        // 2. Bottom-center: Single-line Profile Selector Pill (toggles upward bar-width dropdown)
        let selected_id = self.backend.selected_profile();
        let selected = profiles
            .iter()
            .find(|p| p.id == selected_id)
            .or_else(|| profiles.first());
        let sel_name = selected.map_or("Vanilla", |p| p.name.as_str());
        let sel_id = selected.map_or(VANILLA, |p| p.id.as_str());
        let sel_state = selected.map_or(ProfileState::Available, |p| {
            effective_profile_state(p, setup)
        });
        let pill_h = (bar_h - if bar_h < 42.0 { 2.0 } else { 6.0 }).clamp(34.0, 44.0);
        let pill_y = by + ((bar_h - pill_h) * 0.5).round();
        let prof_hit = HitRect {
            id: ButtonId::ToggleProfilePicker,
            x: prof_x,
            y: pill_y,
            w: prof_w,
            h: pill_h,
        };
        if let Some(p) = ui.paint() {
            let open = self.profile_popup_open;
            let hovered = prof_hit.contains(p.cursor);
            let fill = if open {
                color::with_alpha(palette::accent(), 0.16)
            } else if hovered {
                palette::surface_hover()
            } else {
                palette::surface()
            };
            let bdr = if open || hovered {
                color::with_alpha(palette::accent_star(), 0.75)
            } else {
                palette::border()
            };
            p.rounded(prof_x, pill_y, prof_w, pill_h, 11.0, fill);
            p.border(prof_x, pill_y, prof_w, pill_h, 11.0, bdr);
            let state_dot = match &sel_state {
                ProfileState::Installed => col_success,
                ProfileState::Installing(_) | ProfileState::Available => palette::accent_star(),
                ProfileState::Failed(_) => col_danger,
            };
            p.rounded(
                prof_x + 14.0,
                pill_y + ((pill_h - 8.0) * 0.5).round(),
                8.0,
                8.0,
                4.0,
                state_dot,
            );

            let summary = match (&sel_state, selected) {
                (ProfileState::Installing(prog), _) => {
                    format!(
                        "Installing {}%",
                        (stage_fraction(prog) * 100.0).round() as u32
                    )
                }
                (_, Some(prof)) => profile_stats_line(prof),
                _ => "...".to_owned(),
            };

            let tx = prof_x + 30.0;
            let chev_x = prof_x + prof_w - 18.0;
            let max_tw = (chev_x - 10.0 - tx).max(60.0);
            let name_fit = truncate_to_fit(p, Face::SemiBold, 13.5, sel_name, max_tw * 0.46);
            let nw = p.measure(Face::SemiBold, 13.5, &name_fit);
            p.text_line(
                Face::SemiBold,
                13.5,
                pill_h,
                tx,
                pill_y,
                palette::text(),
                &name_fit,
            );
            let rem_w = max_tw - nw;
            if rem_w > 46.0 {
                let sub = format!("   {summary}");
                let fit_s = truncate_to_fit(p, Face::Regular, ui_theme::small(), &sub, rem_w);
                p.text_line(
                    Face::Regular,
                    ui_theme::small(),
                    pill_h,
                    tx + nw,
                    pill_y,
                    palette::muted(),
                    &fit_s,
                );
            }
            draw_icon_chevron(
                p,
                chev_x,
                pill_y + pill_h * 0.5,
                7.0,
                palette::muted(),
                open,
            );
        }
        hits.push(prof_hit);

        // 3. Bottom-right: Primary Play / Install CTA
        let base_ready = matches!(setup, Setup::Ready);
        if account.is_none() {
            draw_play_button(
                ui,
                hits,
                ButtonId::OpenAccountSwitcher,
                play_x,
                by,
                play_w,
                bar_h,
                "Pick Account",
                false,
            );
        } else if sel_id == VANILLA || !base_ready {
            match setup {
                Setup::Needed => draw_play_button(
                    ui,
                    hits,
                    ButtonId::InstallSelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    if sel_id == VANILLA {
                        "Install Game"
                    } else {
                        "Install Base Game"
                    },
                    false,
                ),
                Setup::Running(prog) => {
                    let pct = (stage_fraction(prog) * 100.0).round() as u32;
                    draw_button(
                        ui,
                        hits,
                        ButtonId::ViewInstallScreen,
                        play_x,
                        by,
                        play_w,
                        bar_h,
                        Tone::Banner,
                        &format!("{} {pct}%", stage_name(prog.stage)),
                    );
                }
                Setup::Failed { .. } => draw_button(
                    ui,
                    hits,
                    ButtonId::InstallSelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    Tone::Danger,
                    "Retry Install",
                ),
                Setup::Ready => draw_play_button(
                    ui,
                    hits,
                    ButtonId::PlaySelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    "Play",
                    true,
                ),
            }
        } else {
            match sel_state {
                ProfileState::Available => draw_play_button(
                    ui,
                    hits,
                    ButtonId::InstallSelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    "Install Profile",
                    false,
                ),
                ProfileState::Installing(ref prog) => {
                    let frac = stage_fraction(prog);
                    let pct = (frac * 100.0).round() as u32;
                    if let Some(p) = ui.paint() {
                        p.rounded(play_x, by, play_w, bar_h, 11.0, palette::surface());
                        p.border(play_x, by, play_w, bar_h, 11.0, palette::accent_star());
                        if frac > 0.0 {
                            p.rounded(
                                play_x + 3.0,
                                by + 3.0,
                                ((play_w - 6.0) * frac).max(12.0),
                                bar_h - 6.0,
                                8.0,
                                color::with_alpha(palette::accent_fill(), 0.78),
                            );
                        }
                        let txt = format!("Installing {pct}%...");
                        let tw = p.measure(Face::SemiBold, 14.0, &txt);
                        p.text_line(
                            Face::SemiBold,
                            14.0,
                            bar_h,
                            play_x + ((play_w - tw) * 0.5).round(),
                            by,
                            palette::text(),
                            &txt,
                        );
                    }
                }
                ProfileState::Failed(_) => draw_button(
                    ui,
                    hits,
                    ButtonId::InstallSelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    Tone::Danger,
                    "Retry Install",
                ),
                ProfileState::Installed => draw_play_button(
                    ui,
                    hits,
                    ButtonId::PlaySelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    "Play",
                    true,
                ),
            }
        }
        (prof_x, prof_w)
    }
}
