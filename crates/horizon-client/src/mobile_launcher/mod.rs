//! Meridian mobile launcher UI (`docs/launcher-spec.md`):
//! North Star visual identity (`#08020f` -> `#100719`, `#a575f6` / `#a990ff` accents) over
//! `crate::ui_theme` so no prepared pack is required.
//!
//! Shell layout:
//! - Top bar: North Star logo + wordmark (top left) and optional background install progress pill.
//! - Left sidebar: Home, Mods, Skins, Settings.
//! - Bottom dock bar: Player head (account switcher), Profile selector bar, and Play/Install CTA.

mod auth;
mod home;
mod install;
mod mods;
mod settings;
#[cfg(test)]
mod tests;
mod widgets;

use crate::launcher_backend::{
    Account, Backend, ModSearch, Profile, ProfileState, Setup, SignIn, VANILLA, valid_profile_name,
};
use crate::menu_style;
use crate::ui_theme::{self, Face, Ui, color};
use widgets::{
    Canvas, HitRect, Tone, account_short, draw_backdrop, draw_button, draw_north_star_logo,
    draw_player_head, draw_sidebar_item, effective_profile_state, palette, profile_badge_parts,
    profile_stats_line, stage_fraction, stage_name, stage_pill_text, truncate_to_fit,
};

/// Action requested by clicking on [`MobileLauncher`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Prepared files and selected profile are ready: launch the game in `profile_dir`.
    Play,
    /// Open an external web link (for example Microsoft's device-code verification page).
    OpenUrl(String),
    /// Close the launcher window.
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Home,
    Profiles,
    Install,
    NewProfile,
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
    ProfilesPrevPage,
    ProfilesNextPage,
    InstallSelected,
    PlaySelected,
    ViewInstallScreen,
    OpenModsForSelected,
    OpenRenameSelected,
    AskRemoveSelected,
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
    // New profile
    CancelNewProfile,
    FocusNewProfileName,
    SelectPresetNone,
    SelectPreset(usize),
    ToggleNewProfileMod(usize),
    NewProfilePrevMods,
    NewProfileNextMods,
    CreateProfile,
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
    CancelRemoveProfile,
    ConfirmRemoveProfile,
    FocusRenameInput,
    SaveRenameProfile,
    CancelRenameProfile,
    // Settings
    SettingsBack,
}

/// Interactive Meridian launcher backed by [`Backend`].
pub struct MobileLauncher {
    backend: Box<dyn Backend>,
    route: Route,
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
    // Paging
    profile_page: usize,
    account_page: usize,
    // New profile state
    new_profile_name: String,
    new_profile_preset: Option<String>,
    new_profile_mods: Vec<String>,
    new_profile_mods_page: usize,
    new_profile_error: Option<String>,
    // Mod manager state
    mods_profile_id: Option<String>,
    mods_list_page: usize,
    mod_search_query: String,
    mod_search_focused: bool,
    mod_registry_page: usize,
    mods_error: Option<String>,
    // Modals
    confirm_remove_profile: Option<(String, String, usize)>,
    rename_modal: Option<(String, String, Option<String>)>,
}

impl MobileLauncher {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self {
            backend,
            route: Route::Home,
            adding_account: false,
            accounts_before_add: 0,
            show_more_auth: false,
            auth_form: None,
            sign_in_provider: SignInProvider::Microsoft,
            offline_name: "Player".to_owned(),
            offline_error: None,
            ely_field: ElyField::Username,
            ely_username: String::new(),
            ely_password: String::new(),
            ely_otp: String::new(),
            ely_error: None,
            status_note: None,
            profile_page: 0,
            account_page: 0,
            new_profile_name: String::new(),
            new_profile_preset: None,
            new_profile_mods: Vec::new(),
            new_profile_mods_page: 0,
            new_profile_error: None,
            mods_profile_id: None,
            mods_list_page: 0,
            mod_search_query: String::new(),
            mod_search_focused: false,
            mod_registry_page: 0,
            mods_error: None,
            confirm_remove_profile: None,
            rename_modal: None,
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
        if self.rename_modal.is_some() {
            return self.rename_modal.as_mut().map(|(_, text, _)| text);
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
            Route::NewProfile => Some(&mut self.new_profile_name),
            Route::Mods if self.mod_search_focused => Some(&mut self.mod_search_query),
            _ => None,
        }
    }

    fn max_chars_for_active(&self) -> usize {
        if self.rename_modal.is_some() || self.route == Route::NewProfile {
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
        if self.rename_modal.is_some() || self.route == Route::NewProfile {
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

    pub fn backspace(&mut self) {
        if let Some(buf) = self.active_text_mut() {
            buf.pop();
        }
    }

    fn submit_active_form(&mut self) {
        if self.rename_modal.is_some() {
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
            Route::NewProfile => {
                self.handle_button(ButtonId::CreateProfile);
            }
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

        match id {
            ButtonId::GoHome | ButtonId::SettingsBack => {
                self.confirm_remove_profile = None;
                self.rename_modal = None;
                self.mod_search_focused = false;
                self.route = Route::Home;
            }
            ButtonId::ToggleProfilePicker => {
                self.confirm_remove_profile = None;
                self.rename_modal = None;
                self.mod_search_focused = false;
                self.route = if self.route == Route::Profiles {
                    Route::Home
                } else {
                    Route::Profiles
                };
            }
            ButtonId::TopProgressBanner | ButtonId::ViewInstallScreen => {
                self.confirm_remove_profile = None;
                self.rename_modal = None;
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
                self.confirm_remove_profile = None;
                self.rename_modal = None;
                let profiles = self.backend.profiles();
                let pid = self.effective_mods_profile_id(&profiles);
                self.mods_profile_id = Some(pid);
                self.mods_list_page = 0;
                self.mod_registry_page = 0;
                self.mods_error = None;
                self.mod_search_focused = false;
                if matches!(self.backend.mod_search(), ModSearch::Idle) {
                    self.backend.search_mods(&self.mod_search_query);
                }
                self.route = Route::Mods;
            }
            ButtonId::OpenModsForSelected => {
                self.mods_profile_id = Some(self.backend.selected_profile());
                return self.handle_button(ButtonId::OpenMods);
            }
            ButtonId::OpenSkins => {
                self.confirm_remove_profile = None;
                self.rename_modal = None;
                self.mod_search_focused = false;
                self.route = Route::Skins;
            }
            ButtonId::OpenSettings => {
                self.confirm_remove_profile = None;
                self.rename_modal = None;
                self.mod_search_focused = false;
                self.route = if self.route == Route::Settings {
                    Route::Home
                } else {
                    Route::Settings
                };
            }
            ButtonId::OpenAccountSwitcher => {
                self.confirm_remove_profile = None;
                self.rename_modal = None;
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
                    if self.route == Route::Profiles {
                        self.route = Route::Home;
                    }
                }
            }
            ButtonId::ProfilesPrevPage => {
                self.profile_page = self.profile_page.saturating_sub(1);
            }
            ButtonId::ProfilesNextPage => {
                self.profile_page = self.profile_page.saturating_add(1);
            }
            ButtonId::InstallSelected => {
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
                self.new_profile_name.clear();
                self.new_profile_preset = None;
                self.new_profile_mods.clear();
                self.new_profile_mods_page = 0;
                self.new_profile_error = None;
                self.mod_search_focused = false;
                if matches!(self.backend.mod_search(), ModSearch::Idle) {
                    self.backend.search_mods("");
                }
                self.route = Route::NewProfile;
            }
            ButtonId::CancelNewProfile => {
                self.route = Route::Home;
            }
            ButtonId::FocusNewProfileName => {}
            ButtonId::SelectPresetNone => {
                self.new_profile_preset = None;
            }
            ButtonId::SelectPreset(idx) => {
                if let Some(p) = self.backend.presets().get(idx) {
                    self.new_profile_preset = if self.new_profile_preset.as_deref() == Some(&p.id) {
                        None
                    } else {
                        Some(p.id.clone())
                    };
                }
            }
            ButtonId::ToggleNewProfileMod(idx) => {
                if let ModSearch::Done(mods) = self.backend.mod_search()
                    && let Some(m) = mods.get(idx)
                {
                    if let Some(pos) = self.new_profile_mods.iter().position(|id| id == &m.id) {
                        self.new_profile_mods.remove(pos);
                    } else {
                        self.new_profile_mods.push(m.id.clone());
                    }
                }
            }
            ButtonId::NewProfilePrevMods => {
                self.new_profile_mods_page = self.new_profile_mods_page.saturating_sub(1);
            }
            ButtonId::NewProfileNextMods => {
                self.new_profile_mods_page = self.new_profile_mods_page.saturating_add(1);
            }
            ButtonId::CreateProfile => {
                let name = self.new_profile_name.trim().to_owned();
                if !valid_profile_name(&name) {
                    self.new_profile_error = Some("Enter 1 to 32 characters.".to_owned());
                    return None;
                }
                match self.backend.create_profile(
                    &name,
                    self.new_profile_preset.as_deref(),
                    &self.new_profile_mods,
                ) {
                    Ok(id) => {
                        let _ = self.backend.select_profile(&id);
                        self.new_profile_error = None;
                        self.route = Route::Home;
                        self.status_note = Some(format!("Created profile \"{name}\"."));
                    }
                    Err(err) => {
                        self.new_profile_error = Some(err);
                    }
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
            ButtonId::OpenRenameSelected => {
                let (profiles, sel_id, _) = self.selected_profile_snapshot();
                if sel_id != VANILLA
                    && let Some(p) = profiles.iter().find(|p| p.id == sel_id)
                {
                    self.rename_modal = Some((p.id.clone(), p.name.clone(), None));
                }
            }
            ButtonId::FocusRenameInput => {}
            ButtonId::SaveRenameProfile => {
                if let Some((id, new_name, _)) = self.rename_modal.take() {
                    match self.backend.rename_profile(&id, &new_name) {
                        Ok(()) => {
                            self.status_note = Some(format!("Renamed to \"{}\".", new_name.trim()));
                        }
                        Err(err) => {
                            self.rename_modal = Some((id, new_name, Some(err)));
                        }
                    }
                }
            }
            ButtonId::CancelRenameProfile => {
                self.rename_modal = None;
            }
            ButtonId::AskRemoveSelected => {
                let (profiles, sel_id, _) = self.selected_profile_snapshot();
                if sel_id != VANILLA
                    && let Some(p) = profiles.iter().find(|p| p.id == sel_id)
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

        let short_h = ui_h < 460.0;
        let compact_w = ui_w < 860.0;
        let pad = if compact_w { 12.0 } else { 20.0 };
        let top_h = if short_h { 44.0 } else { 52.0 };

        let accounts = self.backend.accounts();
        let active_idx = self.backend.active_account();
        let account = active_idx.and_then(|i| accounts.get(i).cloned());
        let onboarding = self.is_onboarding(&accounts);
        let setup = self.backend.setup();
        let sign_in = self.backend.sign_in();
        let profiles = self.backend.profiles();

        // ---- Top Strip: Logo (top left) + optional background progress pill ----
        self.draw_top_bar(
            ui, &mut hits, ui_w, pad, top_h, onboarding, &setup, &profiles,
        );

        if onboarding {
            let v_margin = if short_h { 8.0 } else { 18.0 };
            let content_top = top_h + v_margin;
            let content_h = (ui_h - content_top - v_margin).max(240.0);
            let card_w = (ui_w - pad * 2.0).min(640.0);
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
                        col_danger,
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

        // ---- Signed-in Shell: Left Sidebar + Bottom Dock Bar + Main Content Area ----
        let bottom_h = if short_h { 60.0 } else { 72.0 };
        let sidebar_w = if compact_w { 132.0 } else { 164.0 };
        let mid_h = (ui_h - top_h - bottom_h).max(200.0);

        self.draw_sidebar(ui, &mut hits, top_h, sidebar_w, mid_h, short_h);
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
        );

        let v_margin = if short_h { 8.0 } else { 16.0 };
        let main_x = sidebar_w + pad;
        let main_y = top_h + v_margin;
        let main_w = (ui_w - sidebar_w - pad * 2.0).max(280.0);
        let main_h = (mid_h - v_margin * 2.0).max(180.0);
        let modal_w = main_w.min(600.0);
        let modal_x = main_x + ((main_w - modal_w) * 0.5).round();

        if let Some((ref id, ref name, worlds)) = self.confirm_remove_profile {
            self.draw_remove_confirm(
                ui, &mut hits, modal_x, main_y, modal_w, main_h, id, name, worlds, col_danger,
            );
            return hits;
        }
        if let Some((ref id, ref draft, ref err)) = self.rename_modal {
            self.draw_rename_modal(
                ui,
                &mut hits,
                modal_x,
                main_y,
                modal_w,
                main_h,
                id,
                draft,
                err.as_deref(),
                col_danger,
            );
            return hits;
        }

        match self.route {
            Route::Install => match setup {
                Setup::Running(ref progress) => self.draw_setup_running(
                    ui,
                    &mut hits,
                    modal_x,
                    main_y,
                    modal_w,
                    main_h,
                    progress,
                    col_success,
                    col_warn,
                ),
                Setup::Failed { stage, ref error } => self.draw_setup_failed(
                    ui, &mut hits, modal_x, main_y, modal_w, main_h, stage, error, col_danger,
                ),
                Setup::Needed | Setup::Ready => self.draw_home(
                    ui,
                    &mut hits,
                    main_x,
                    main_y,
                    main_w,
                    main_h,
                    &setup,
                    &profiles,
                    col_success,
                    col_danger,
                    None,
                ),
            },
            Route::Profiles => self.draw_home(
                ui,
                &mut hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &setup,
                &profiles,
                col_success,
                col_danger,
                Some("Switch Profile"),
            ),
            Route::AccountSwitcher => self.draw_account_switcher(
                ui,
                &mut hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &accounts,
                active_idx,
                col_success,
            ),
            Route::NewProfile => self.draw_new_profile(
                ui, &mut hits, main_x, main_y, main_w, main_h, &profiles, col_danger,
            ),
            Route::Mods => self.draw_mods_view(
                ui,
                &mut hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &profiles,
                col_success,
                col_danger,
            ),
            Route::Skins => {
                self.draw_skins_view(ui, &mut hits, main_x, main_y, main_w, main_h, &account);
            }
            Route::Settings => self.draw_settings_view(
                ui,
                &mut hits,
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
                &mut hits,
                main_x,
                main_y,
                main_w,
                main_h,
                &setup,
                &profiles,
                col_success,
                col_danger,
                None,
            ),
        }
        hits
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_top_bar(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        ui_w: f32,
        pad: f32,
        top_h: f32,
        onboarding: bool,
        setup: &Setup,
        profiles: &[Profile],
    ) {
        if let Some(p) = ui.paint() {
            p.rounded(0.0, 0.0, ui_w, top_h, 0.0, palette::card());
            p.rounded(0.0, top_h - 1.0, ui_w, 1.0, 0.0, palette::border());
        }

        let btn_h = (top_h - 8.0).max(36.0);
        let btn_y = ((top_h - btn_h) * 0.5).round();
        let wordmark_w = ui.ro().measure(Face::SemiBold, 19.0, "Meridian");
        let brand_w = 34.0 + wordmark_w + 16.0;
        let logo_hit = HitRect {
            id: ButtonId::GoHome,
            x: pad,
            y: btn_y,
            w: brand_w,
            h: btn_h,
        };
        if let Some(p) = ui.paint() {
            draw_north_star_logo(p, pad + 2.0, ((top_h - 26.0) * 0.5).round(), 26.0);
            p.text_line(
                Face::SemiBold,
                19.0,
                top_h,
                pad + 34.0,
                0.0,
                palette::text(),
                "Meridian",
            );
        }
        if !onboarding {
            hits.push(logo_hit);
        }

        if onboarding {
            let settings_w =
                (ui.ro().measure(Face::Medium, ui_theme::body(), "Settings") + 24.0).max(86.0);
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
                    Tone::Secondary
                },
                "Settings",
            );
            return;
        }

        // Top right: only show a background progress pill while an install runs off-screen.
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
            let max_pill_w = (ui_w - pad * 2.0 - brand_w - 16.0).max(120.0);
            let pill_w =
                (ui.ro().measure(Face::Mono, ui_theme::small(), &summary) + 24.0).min(max_pill_w);
            draw_button(
                ui,
                hits,
                ButtonId::TopProgressBanner,
                ui_w - pad - pill_w,
                btn_y,
                pill_w,
                btn_h,
                Tone::Banner,
                &summary,
            );
        }
    }

    fn draw_sidebar(
        &self,
        ui: &mut Canvas<'_, '_>,
        hits: &mut Vec<HitRect>,
        top_y: f32,
        w: f32,
        h: f32,
        short_h: bool,
    ) {
        if let Some(p) = ui.paint() {
            p.rounded(0.0, top_y, w, h, 0.0, palette::card());
            p.rounded(w - 1.0, top_y, 1.0, h, 0.0, palette::border());
        }

        let sx = 10.0;
        let sw = w - 20.0;
        let mut sy = top_y + if short_h { 10.0 } else { 16.0 };
        let item_h = 48.0;
        let gap = if short_h { 8.0 } else { 10.0 };

        let items = [
            (
                ButtonId::GoHome,
                "Home",
                matches!(self.route, Route::Home | Route::Install | Route::NewProfile),
            ),
            (ButtonId::OpenMods, "Mods", self.route == Route::Mods),
            (ButtonId::OpenSkins, "Skins", self.route == Route::Skins),
            (
                ButtonId::OpenSettings,
                "Settings",
                self.route == Route::Settings,
            ),
        ];
        for (id, label, active) in items {
            draw_sidebar_item(ui, hits, id, sx, sy, sw, item_h, active, label);
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
    ) {
        if let Some(p) = ui.paint() {
            p.rounded(0.0, y, ui_w, h, 0.0, palette::card());
            p.rounded(0.0, y, ui_w, 1.0, 0.0, palette::border());
        }

        let bar_h = (h - 12.0).clamp(48.0, 54.0);
        let by = y + ((h - bar_h) * 0.5).round();
        let gap = 12.0;
        let acct_w = if compact_w { 168.0 } else { 200.0 };
        let play_w = if compact_w { 184.0 } else { 220.0 };
        let prof_x = pad + acct_w + gap;
        let prof_w = (ui_w - pad * 2.0 - acct_w - play_w - gap * 2.0).max(160.0);
        let play_x = ui_w - pad - play_w;

        // 1. Bottom-left: Player Head (Account Switcher)
        let acct_hit = HitRect {
            id: ButtonId::OpenAccountSwitcher,
            x: pad,
            y: by,
            w: acct_w,
            h: bar_h,
        };
        let (acct_name, acct_sub, dot_col) = match account {
            Some(a @ Account::Microsoft { .. }) => (account_short(a), "Microsoft", col_success),
            Some(a @ Account::Ely { .. }) => (account_short(a), "Ely.by", col_success),
            Some(a @ Account::Offline { .. }) => (account_short(a), "Offline", col_warn),
            None => ("Account", "Sign in", palette::muted()),
        };
        if let Some(p) = ui.paint() {
            let active = self.route == Route::AccountSwitcher;
            let fill = if active {
                color::with_alpha(palette::accent(), 0.22)
            } else if acct_hit.contains(p.cursor) {
                palette::surface_hover()
            } else {
                palette::surface()
            };
            let bdr = if active {
                palette::accent_star()
            } else {
                palette::border()
            };
            p.rounded(pad, by, acct_w, bar_h, 10.0, fill);
            p.border(pad, by, acct_w, bar_h, 10.0, bdr);

            let head_sz = (bar_h - 14.0).clamp(30.0, 36.0);
            let head_y = by + ((bar_h - head_sz) * 0.5).round();
            let head = account.as_ref().and_then(|a| self.backend.player_head(a));
            draw_player_head(p, pad + 8.0, head_y, head_sz, head.as_ref(), acct_name);

            let tx = pad + head_sz + 16.0;
            let max_tw = (acct_w - head_sz - 34.0).max(40.0);
            let fit_n = truncate_to_fit(p, Face::SemiBold, 14.0, acct_name, max_tw);
            p.text_line(
                Face::SemiBold,
                14.0,
                18.0,
                tx,
                by + 7.0,
                palette::text(),
                &fit_n,
            );
            p.text_line(
                Face::Mono,
                11.0,
                16.0,
                tx,
                by + 25.0,
                palette::muted(),
                acct_sub,
            );
            p.rounded(
                pad + acct_w - 14.0,
                by + ((bar_h - 6.0) * 0.5).round(),
                6.0,
                6.0,
                3.0,
                dot_col,
            );
        }
        hits.push(acct_hit);

        // 2. Bottom-center: Profile Bar selector (dropdown style)
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
        let prof_hit = HitRect {
            id: ButtonId::ToggleProfilePicker,
            x: prof_x,
            y: by,
            w: prof_w,
            h: bar_h,
        };
        if let Some(p) = ui.paint() {
            let open = self.route == Route::Profiles;
            let fill = if open {
                color::with_alpha(palette::accent(), 0.22)
            } else if prof_hit.contains(p.cursor) {
                palette::surface_hover()
            } else {
                palette::surface()
            };
            let bdr = if open {
                palette::accent_star()
            } else {
                palette::border()
            };
            p.rounded(prof_x, by, prof_w, bar_h, 10.0, fill);
            p.border(prof_x, by, prof_w, bar_h, 10.0, bdr);

            let badge_sz = (bar_h - 14.0).clamp(30.0, 36.0);
            let badge_y = by + ((bar_h - badge_sz) * 0.5).round();
            p.rounded(
                prof_x + 8.0,
                badge_y,
                badge_sz,
                badge_sz,
                7.0,
                if sel_id == VANILLA {
                    palette::card()
                } else {
                    color::with_alpha(palette::accent(), 0.26)
                },
            );
            p.border(
                prof_x + 8.0,
                badge_y,
                badge_sz,
                badge_sz,
                7.0,
                palette::accent_star(),
            );
            draw_north_star_logo(
                p,
                prof_x + 8.0 + ((badge_sz - 20.0) * 0.5).round(),
                badge_y + ((badge_sz - 20.0) * 0.5).round(),
                20.0,
            );

            let chev = if open { "^" } else { "v" };
            let chev_x = prof_x + prof_w - 22.0;
            p.text_line(Face::Mono, 14.0, bar_h, chev_x, by, palette::muted(), chev);

            let (state_lbl, state_col) =
                profile_badge_parts(&sel_state, true, col_success, col_danger);
            let st_w = p.measure(Face::Mono, 11.0, &state_lbl) + 14.0;
            let st_x = chev_x - st_w - 10.0;
            let tx = prof_x + badge_sz + 18.0;
            let show_badge = st_x > tx + 110.0;
            if show_badge {
                let st_y = by + ((bar_h - 22.0) * 0.5).round();
                p.rounded(
                    st_x,
                    st_y,
                    st_w,
                    22.0,
                    5.0,
                    color::with_alpha(state_col, 0.18),
                );
                p.border(st_x, st_y, st_w, 22.0, 5.0, state_col);
                p.text_line(
                    Face::Mono,
                    11.0,
                    22.0,
                    st_x + 7.0,
                    st_y,
                    state_col,
                    &state_lbl,
                );
            }

            let text_right = if show_badge {
                st_x - 10.0
            } else {
                chev_x - 8.0
            };
            let max_tw = (text_right - tx).max(50.0);
            let fit_n = truncate_to_fit(p, Face::SemiBold, 15.0, sel_name, max_tw);
            p.text_line(
                Face::SemiBold,
                15.0,
                18.0,
                tx,
                by + 6.0,
                palette::text(),
                &fit_n,
            );

            let sub = selected
                .map(profile_stats_line)
                .unwrap_or_else(|| "0 mods  |  0 worlds".to_owned());
            let fit_s = truncate_to_fit(p, Face::Mono, 11.0, &sub, max_tw);
            p.text_line(
                Face::Mono,
                11.0,
                16.0,
                tx,
                by + 26.0,
                palette::muted(),
                &fit_s,
            );
        }
        hits.push(prof_hit);

        // 3. Bottom-right: Primary Play / Install CTA
        let base_ready = matches!(setup, Setup::Ready);
        if account.is_none() {
            draw_button(
                ui,
                hits,
                ButtonId::OpenAccountSwitcher,
                play_x,
                by,
                play_w,
                bar_h,
                Tone::Primary,
                "Pick Account",
            );
        } else if sel_id == VANILLA || !base_ready {
            match setup {
                Setup::Needed => draw_button(
                    ui,
                    hits,
                    ButtonId::InstallSelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    Tone::Primary,
                    if sel_id == VANILLA {
                        "Install Game"
                    } else {
                        "Install Base Game"
                    },
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
                Setup::Ready => draw_button(
                    ui,
                    hits,
                    ButtonId::PlaySelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    Tone::Primary,
                    "Play",
                ),
            }
        } else {
            match sel_state {
                ProfileState::Available => draw_button(
                    ui,
                    hits,
                    ButtonId::InstallSelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    Tone::Primary,
                    "Install Profile",
                ),
                ProfileState::Installing(ref prog) => {
                    let frac = stage_fraction(prog);
                    let pct = (frac * 100.0).round() as u32;
                    if let Some(p) = ui.paint() {
                        p.rounded(play_x, by, play_w, bar_h, 10.0, palette::surface());
                        p.border(play_x, by, play_w, bar_h, 10.0, palette::accent_star());
                        if frac > 0.0 {
                            p.rounded(
                                play_x + 2.0,
                                by + 2.0,
                                ((play_w - 4.0) * frac).max(10.0),
                                bar_h - 4.0,
                                8.0,
                                color::with_alpha(palette::accent(), 0.35),
                            );
                        }
                        let txt = format!("Installing {pct}%...");
                        let tw = p.measure(Face::Medium, ui_theme::sizes().ui, &txt);
                        p.text_line(
                            Face::Medium,
                            ui_theme::sizes().ui,
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
                ProfileState::Installed => draw_button(
                    ui,
                    hits,
                    ButtonId::PlaySelected,
                    play_x,
                    by,
                    play_w,
                    bar_h,
                    Tone::Primary,
                    "Play",
                ),
            }
        }
    }
}
