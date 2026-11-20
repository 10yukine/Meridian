use super::widgets::{Canvas, count_label, log_levels, profile_stats_line};
use super::{Action, AuthForm, ButtonId, LICENSES, MobileLauncher, Route, SOURCE_CODE};
use crate::hud::{HudAtlas, HudBatch};
use crate::launcher_backend::{
    Account, Backend, ELY_SKINS, Head, ModInfo, ModSearch, PickedSkin, Profile, ProfileMod,
    ProfileState, Progress, Setup, SignIn, SkinUpload, Stage, VANILLA, profile_id,
    valid_offline_name,
};
use crate::ui_theme::Ui;
use std::sync::{Arc, Mutex};

/// The stub's skin side, shared with the test: the file the picker "chose", the uploads asked
/// for, and how the last one went.
#[derive(Default)]
struct SkinStub {
    picked: Option<Result<PickedSkin, String>>,
    uploads: Vec<(Vec<u8>, bool)>,
    status: SkinUpload,
}

struct StubBackend {
    accounts: Vec<Account>,
    active: Option<usize>,
    sign_in: SignIn,
    setup: Setup,
    profiles: Vec<Profile>,
    selected_profile: String,
    mods: Vec<ProfileMod>,
    search: ModSearch,
    /// The install cache [`Backend::free_space`] frees.
    reclaim: u64,
    skins: Arc<Mutex<SkinStub>>,
    /// The selected profile's game log.
    log: Vec<String>,
    /// The game's arguments, as kept.
    args: String,
}

impl Default for StubBackend {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            active: None,
            sign_in: SignIn::Idle,
            setup: Setup::Needed,
            profiles: vec![Profile {
                id: VANILLA.into(),
                name: "Vanilla".into(),
                description: "Minecraft as it comes, no mods.".into(),
                mods: 0,
                size: 672_000_000,
                worlds: 1,
                state: ProfileState::Available,
            }],
            selected_profile: VANILLA.into(),
            mods: vec![ProfileMod {
                info: ModInfo {
                    id: "faster-chunks".into(),
                    name: "Faster Chunks".into(),
                    description: "Builds chunk meshes sooner.".into(),
                    version: "1.0.0".into(),
                    size: 2_400_000,
                },
                enabled: true,
            }],
            search: ModSearch::Done(vec![
                ModInfo {
                    id: "faster-chunks".into(),
                    name: "Faster Chunks".into(),
                    description: "Builds chunk meshes sooner.".into(),
                    version: "1.0.0".into(),
                    size: 2_400_000,
                },
                ModInfo {
                    id: "zoom".into(),
                    name: "Zoom".into(),
                    description: "Hold a key to zoom in.".into(),
                    version: "1.0.0".into(),
                    size: 300_000,
                },
            ]),
            reclaim: 0,
            skins: Arc::default(),
            log: Vec::new(),
            args: String::new(),
        }
    }
}

impl Backend for StubBackend {
    fn account(&self) -> Option<Account> {
        self.active.and_then(|i| self.accounts.get(i).cloned())
    }
    fn accounts(&self) -> Vec<Account> {
        self.accounts.clone()
    }
    fn active_account(&self) -> Option<usize> {
        self.active
    }
    fn switch_account(&mut self, index: usize) -> Result<(), String> {
        if index < self.accounts.len() {
            self.active = Some(index);
            Ok(())
        } else {
            Err("No such account.".into())
        }
    }
    fn remove_account(&mut self, index: usize) -> Result<(), String> {
        if index >= self.accounts.len() {
            return Err("No such account.".into());
        }
        self.accounts.remove(index);
        self.active = if self.accounts.is_empty() {
            None
        } else {
            Some(0)
        };
        Ok(())
    }
    fn player_head(&self, _account: &Account) -> Option<Head> {
        Some([220u8; 8 * 8 * 4])
    }
    fn sign_in_microsoft(&mut self) {
        self.sign_in = SignIn::Code {
            url: "https://www.microsoft.com/link".into(),
            code: "TESTCODE".into(),
        };
    }
    fn sign_in_ely(&mut self, username: &str, password: &str, _otp: &str) -> Result<(), String> {
        if password == "badpass" {
            self.sign_in = SignIn::Failed("Invalid credentials".into());
            return Ok(());
        }
        let acct = Account::Ely {
            name: username.to_owned(),
        };
        self.accounts.push(acct.clone());
        self.active = Some(self.accounts.len() - 1);
        self.sign_in = SignIn::Done(acct);
        Ok(())
    }
    fn sign_in(&self) -> SignIn {
        self.sign_in.clone()
    }
    fn sign_in_offline(&mut self, name: &str) -> Result<Account, String> {
        if !valid_offline_name(name) {
            return Err("Bad offline name".into());
        }
        let a = Account::Offline {
            name: name.to_owned(),
        };
        self.accounts.push(a.clone());
        self.active = Some(self.accounts.len() - 1);
        Ok(a)
    }
    fn sign_out(&mut self) {
        if let Some(i) = self.active {
            let _ = self.remove_account(i);
        }
        self.sign_in = SignIn::Idle;
    }
    fn setup(&self) -> Setup {
        self.setup.clone()
    }
    fn start_setup(&mut self) {
        self.setup = Setup::Running(Progress {
            stage: Stage::Download,
            done: 268_800_000,
            total: 672_000_000,
            bytes_per_second: 67_200_000,
            note: Some("Retrying asset host".into()),
        });
    }
    fn cancel_setup(&mut self) {
        self.setup = Setup::Needed;
    }
    fn reclaimable(&self) -> Option<u64> {
        Some(self.reclaim)
    }
    fn free_space(&mut self) -> Result<u64, String> {
        Ok(std::mem::take(&mut self.reclaim))
    }
    fn take_picked_skin(&mut self) -> Option<Result<PickedSkin, String>> {
        self.skins.lock().unwrap().picked.take()
    }
    fn use_skin(&mut self, png: Vec<u8>, slim: bool) {
        let mut skins = self.skins.lock().unwrap();
        skins.uploads.push((png, slim));
        skins.status = SkinUpload::Uploading;
    }
    fn skin_upload(&self) -> SkinUpload {
        self.skins.lock().unwrap().status.clone()
    }
    fn game_log(&self, _profile: &str) -> Vec<String> {
        self.log.clone()
    }
    fn game_args(&self) -> String {
        self.args.clone()
    }
    fn set_game_args(&mut self, text: &str) -> Result<(), String> {
        crate::launcher_backend::launch::split_args(text)?;
        self.args = text.trim().into();
        Ok(())
    }
    fn profiles(&self) -> Vec<Profile> {
        self.profiles.clone()
    }
    fn selected_profile(&self) -> String {
        self.selected_profile.clone()
    }
    fn select_profile(&mut self, id: &str) -> Result<(), String> {
        self.selected_profile = id.to_owned();
        Ok(())
    }
    fn remove_profile(&mut self, id: &str) -> Result<(), String> {
        self.profiles.retain(|p| p.id != id);
        self.selected_profile = VANILLA.into();
        Ok(())
    }
    fn create_profile(
        &mut self,
        name: &str,
        _preset: Option<&str>,
        _mods: &[String],
    ) -> Result<String, String> {
        let id = profile_id(name, &[]);
        self.profiles.push(Profile {
            id: id.clone(),
            name: name.trim().into(),
            description: "Custom".into(),
            mods: 1,
            size: 2_400_000,
            worlds: 1,
            state: ProfileState::Installed,
        });
        Ok(id)
    }
    fn rename_profile(&mut self, id: &str, name: &str) -> Result<(), String> {
        if let Some(p) = self.profiles.iter_mut().find(|p| p.id == id) {
            p.name = name.trim().into();
        }
        Ok(())
    }
    fn mods(&self, profile: &str) -> Vec<ProfileMod> {
        if profile == VANILLA {
            Vec::new()
        } else {
            self.mods.clone()
        }
    }
    fn set_mod_enabled(&mut self, _profile: &str, id: &str, enabled: bool) -> Result<(), String> {
        if let Some(m) = self.mods.iter_mut().find(|m| m.info.id == id) {
            m.enabled = enabled;
        }
        Ok(())
    }
    fn search_mods(&mut self, _query: &str) {}
    fn mod_search(&self) -> ModSearch {
        self.search.clone()
    }
    fn add_mod(&mut self, _profile: &str, id: &str) -> Result<(), String> {
        self.mods.push(ProfileMod {
            info: ModInfo {
                id: id.into(),
                name: "Zoom".into(),
                description: "Zoom in.".into(),
                version: "1.0.0".into(),
                size: 300_000,
            },
            enabled: true,
        });
        Ok(())
    }
    fn remove_mod(&mut self, _profile: &str, id: &str) -> Result<(), String> {
        self.mods.retain(|m| m.info.id != id);
        Ok(())
    }
}

fn with_ui<R>(size: [u32; 2], cursor_dp: (f32, f32), f: impl FnOnce(&mut Ui) -> R) -> R {
    thread_local! {
        static ATLAS: HudAtlas = HudAtlas::build(
            &horizon_assets::resource_source::ResourceSource::directory(
                std::path::PathBuf::from("/nonexistent/horizon-test-assets"),
            ),
        ).unwrap();
    }
    ATLAS.with(|atlas| {
        let mut batch = HudBatch::scratch(size);
        let dummy = Ui::new(atlas, &mut batch, (0.0, 0.0));
        let density = dummy.density;
        let cursor_gui = (
            cursor_dp.0 * density / batch.scale,
            cursor_dp.1 * density / batch.scale,
        );
        let mut ui = Ui::new(atlas, &mut batch, cursor_gui);
        f(&mut ui)
    })
}

fn click_button(launcher: &mut MobileLauncher, size: [u32; 2], id: ButtonId) -> Option<Action> {
    let target = with_ui(size, (0.0, 0.0), |ui| {
        let hits = launcher.layout(&mut Canvas::Hit(ui));
        let hit = hits
            .iter()
            .find(|h| h.id == id)
            .unwrap_or_else(|| panic!("expected button {id:?} present"));
        (hit.x + hit.w * 0.5, hit.y + hit.h * 0.5)
    });
    with_ui(size, target, |ui| launcher.click(ui))
}

#[test]
fn pluralization_handles_singular_and_plural_counts() {
    assert_eq!(count_label(0, "world", "worlds"), "0 worlds");
    assert_eq!(count_label(1, "world", "worlds"), "1 world");
    assert_eq!(count_label(2, "world", "worlds"), "2 worlds");
    assert_eq!(count_label(1, "mod", "mods"), "1 mod");
    let prof = Profile {
        id: "pack".into(),
        name: "My Pack".into(),
        description: String::new(),
        mods: 1,
        size: 3_800_000,
        worlds: 1,
        state: ProfileState::Installed,
    };
    assert_eq!(profile_stats_line(&prof), "1 mod, 1 world, 3.8 MB");
}

#[test]
fn offline_flow_leads_to_home_and_starts_install() {
    let mut launcher = MobileLauncher::new(Box::new(StubBackend::default()));
    let size = [1040, 700];
    click_button(&mut launcher, size, ButtonId::OpenOfflineEntry);
    for _ in 0..6 {
        launcher.backspace();
    }
    launcher.typed("A14Player");
    click_button(&mut launcher, size, ButtonId::ConfirmOffline);

    assert_eq!(
        launcher.backend.account(),
        Some(Account::Offline {
            name: "A14Player".into()
        })
    );
    click_button(&mut launcher, size, ButtonId::InstallSelected);
    assert_eq!(launcher.route, Route::Install);
    assert!(matches!(launcher.backend.setup(), Setup::Running(_)));
}

#[test]
fn settings_navigation_preserves_running_setup_progress() {
    let mut launcher = MobileLauncher::new(Box::new(StubBackend::default()));
    let size = [1040, 700];
    launcher.backend.sign_in_offline("Steve").unwrap();
    launcher.backend.start_setup();
    launcher.route = Route::Install;

    click_button(&mut launcher, size, ButtonId::OpenSettings);
    assert_eq!(launcher.route, Route::Settings);
    assert!(matches!(launcher.backend.setup(), Setup::Running(_)));

    click_button(&mut launcher, size, ButtonId::TopProgressBanner);
    assert_eq!(launcher.route, Route::Install);
    assert!(matches!(launcher.backend.setup(), Setup::Running(_)));
}

#[test]
fn ely_under_more_options_wipes_secrets_on_submit() {
    let mut launcher = MobileLauncher::new(Box::new(StubBackend::default()));
    let size = [1040, 700];
    click_button(&mut launcher, size, ButtonId::ToggleMoreAuth);
    click_button(&mut launcher, size, ButtonId::OpenElyEntry);
    launcher.typed("elyuser\tsecret123\t654321");
    click_button(&mut launcher, size, ButtonId::ConfirmEly);
    assert!(launcher.ely_password.is_empty());
    assert!(launcher.ely_otp.is_empty());
    assert_eq!(
        launcher.backend.account(),
        Some(Account::Ely {
            name: "elyuser".into()
        })
    );

    let mut fail_l = MobileLauncher::new(Box::new(StubBackend::default()));
    click_button(&mut fail_l, size, ButtonId::ToggleMoreAuth);
    click_button(&mut fail_l, size, ButtonId::OpenElyEntry);
    fail_l.typed("elyuser\tbadpass");
    click_button(&mut fail_l, size, ButtonId::ConfirmEly);
    assert!(matches!(fail_l.backend.sign_in(), SignIn::Failed(_)));
    click_button(&mut fail_l, size, ButtonId::RetrySignIn);
    assert_eq!(fail_l.auth_form, Some(AuthForm::Ely));
}

#[test]
fn full_spec_flow_accounts_profiles_mods_skins_and_remove_confirmation() {
    let mut launcher = MobileLauncher::new(Box::new(StubBackend::default()));
    let size = [1040, 700];

    launcher.backend.sign_in_offline("Steve").unwrap();
    launcher.backend.sign_in_offline("Alex").unwrap();
    click_button(&mut launcher, size, ButtonId::OpenAccountSwitcher);
    click_button(&mut launcher, size, ButtonId::SwitchAccount(0));
    assert_eq!(
        launcher.backend().account(),
        Some(Account::Offline {
            name: "Steve".into()
        })
    );

    // Host hooks: crash note and wants_text() suppressed while profile popup is open.
    launcher.note("The game closed unexpectedly last time.");
    assert_eq!(
        launcher.status_note.as_deref(),
        Some("The game closed unexpectedly last time.")
    );
    assert!(!launcher.wants_text());

    // Floating profile popup via bottom profile bar + scrim/close dismiss + Settings font toggle + Skins
    assert!(!launcher.profile_popup_open);
    click_button(&mut launcher, size, ButtonId::ToggleProfilePicker);
    assert!(launcher.profile_popup_open);
    assert!(!launcher.wants_text());
    click_button(&mut launcher, size, ButtonId::CloseProfilePopup);
    assert!(!launcher.profile_popup_open);

    click_button(&mut launcher, size, ButtonId::OpenSettings);
    assert_eq!(launcher.route, Route::Settings);
    assert!(!crate::ui_theme::pixel_font());
    click_button(&mut launcher, size, ButtonId::ToggleLauncherFont);
    assert!(crate::ui_theme::pixel_font());
    click_button(&mut launcher, size, ButtonId::ToggleLauncherFont);
    assert!(!crate::ui_theme::pixel_font());

    click_button(&mut launcher, size, ButtonId::OpenSkins);
    assert_eq!(launcher.route, Route::Skins);
    click_button(&mut launcher, size, ButtonId::GoHome);

    // Open Mods once on Vanilla, then create a custom profile and confirm rail Mods switches to it.
    click_button(&mut launcher, size, ButtonId::OpenMods);
    assert_eq!(launcher.route, Route::Mods);

    // Create profile with mixed-case display name distinct from its slug id
    click_button(&mut launcher, size, ButtonId::OpenNewProfile);
    launcher.typed("My Pack\n");
    assert_eq!(launcher.backend().selected_profile(), "profile-1");
    assert_eq!(launcher.backend().profiles()[1].name, "My Pack");
    // A new profile starts empty: its mods come from the registry beside it (Add).
    for id in ["faster-chunks", "zoom"] {
        launcher.backend.add_mod("profile-1", id).unwrap();
    }

    // Switch back to Vanilla and back to My Pack via the floating Profile Switcher popup.
    click_button(&mut launcher, size, ButtonId::ToggleProfilePicker);
    assert!(launcher.profile_popup_open);
    click_button(&mut launcher, size, ButtonId::SelectProfile(0));
    assert!(!launcher.profile_popup_open);
    assert_eq!(launcher.backend().selected_profile(), VANILLA);
    click_button(&mut launcher, size, ButtonId::ToggleProfilePicker);
    click_button(&mut launcher, size, ButtonId::SelectProfile(1));
    assert!(!launcher.profile_popup_open);
    assert_eq!(launcher.backend().selected_profile(), "profile-1");

    click_button(&mut launcher, size, ButtonId::OpenMods);
    assert_eq!(launcher.route, Route::Mods);
    click_button(&mut launcher, size, ButtonId::ToggleProfileMod(0));
    assert!(!launcher.backend().mods("profile-1")[0].enabled);
    click_button(&mut launcher, size, ButtonId::RemoveProfileMod(0));
    assert_eq!(launcher.backend().mods("profile-1").len(), 1);

    // The pencil on its tab: a new name typed over, Done keeps it.
    click_button(&mut launcher, size, ButtonId::RenameTabProfile);
    launcher.typed("Survival Plus");
    click_button(&mut launcher, size, ButtonId::SaveRenameProfile);
    assert_eq!(launcher.backend().profiles()[1].name, "Survival Plus");

    click_button(&mut launcher, size, ButtonId::DeleteTabProfile);
    assert!(launcher.confirm_remove_profile.is_some());
    click_button(&mut launcher, size, ButtonId::ConfirmRemoveProfile);
    assert_eq!(launcher.backend().selected_profile(), VANILLA);
    assert_eq!(launcher.backend().profiles().len(), 1);
}

fn assert_hits_fit_without_overlap(launcher: &MobileLauncher, size: [u32; 2], label: &str) {
    with_ui(size, (0.0, 0.0), |ui| {
        let hits = launcher.layout(&mut Canvas::Hit(ui));
        assert!(!hits.is_empty(), "{label}: expected interactive hit rects");
        for (i, a) in hits.iter().enumerate() {
            assert!(
                a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= ui.w && a.y + a.h <= ui.h,
                "{label}: rect {:?} ({},{},{}x{}) out of {}x{} bounds",
                a.id,
                a.x,
                a.y,
                a.w,
                a.h,
                ui.w,
                ui.h
            );
            for b in hits.iter().skip(i + 1) {
                let overlap_x = a.x < b.x + b.w && b.x < a.x + a.w;
                let overlap_y = a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(
                    !(overlap_x && overlap_y),
                    "{label}: {:?} ({},{},{}x{}) overlaps {:?} ({},{},{}x{})",
                    a.id,
                    a.x,
                    a.y,
                    a.w,
                    a.h,
                    b.id,
                    b.x,
                    b.y,
                    b.w,
                    b.h
                );
            }
        }
    });
}

#[test]
fn all_screens_fit_without_overlap_on_desktop_and_phone_landscape() {
    for size in [
        [1040, 700],
        [2400, 1080],
        [914, 411],
        [800, 360],
        [2400, 560],
        [1600, 450],
    ] {
        let mut l = MobileLauncher::new(Box::new(StubBackend::default()));
        assert_hits_fit_without_overlap(&l, size, "welcome");

        click_button(&mut l, size, ButtonId::ToggleMoreAuth);
        assert_hits_fit_without_overlap(&l, size, "welcome-more-options");

        click_button(&mut l, size, ButtonId::OpenElyEntry);
        assert_hits_fit_without_overlap(&l, size, "ely-entry");

        click_button(&mut l, size, ButtonId::CancelEly);
        click_button(&mut l, size, ButtonId::OpenOfflineEntry);
        assert_hits_fit_without_overlap(&l, size, "offline-entry");

        click_button(&mut l, size, ButtonId::CancelOffline);
        click_button(&mut l, size, ButtonId::SignInMicrosoft);
        assert_hits_fit_without_overlap(&l, size, "ms-code");

        let mut ready = MobileLauncher::new(Box::new(StubBackend {
            accounts: vec![
                Account::Microsoft {
                    name: "Steve".into(),
                },
                Account::Offline {
                    name: "Alex".into(),
                },
            ],
            active: Some(0),
            sign_in: SignIn::Idle,
            setup: Setup::Ready,
            profiles: vec![
                Profile {
                    id: VANILLA.into(),
                    name: "Vanilla".into(),
                    description: "Base".into(),
                    mods: 0,
                    size: 672_000_000,
                    worlds: 1,
                    state: ProfileState::Installed,
                },
                Profile {
                    id: "my-pack".into(),
                    name: "My Pack".into(),
                    description: "Custom".into(),
                    mods: 2,
                    size: 4_000_000,
                    worlds: 3,
                    state: ProfileState::Installed,
                },
            ],
            selected_profile: "my-pack".into(),
            ..StubBackend::default()
        }));
        assert_hits_fit_without_overlap(&ready, size, "home-ready");

        click_button(&mut ready, size, ButtonId::ToggleProfilePicker);
        assert_hits_fit_without_overlap(&ready, size, "profile-popup");

        click_button(&mut ready, size, ButtonId::OpenAccountSwitcher);
        assert!(!ready.profile_popup_open);
        assert_hits_fit_without_overlap(&ready, size, "account-switcher");

        click_button(&mut ready, size, ButtonId::OpenMods);
        assert_hits_fit_without_overlap(&ready, size, "mod-manager");
        click_button(&mut ready, size, ButtonId::RenameTabProfile);
        assert_hits_fit_without_overlap(&ready, size, "rename-profile");
        click_button(&mut ready, size, ButtonId::SaveRenameProfile);

        click_button(&mut ready, size, ButtonId::ModManagerPickProfile(0));
        assert_hits_fit_without_overlap(&ready, size, "mod-manager-vanilla");

        click_button(&mut ready, size, ButtonId::OpenSkins);
        assert_hits_fit_without_overlap(&ready, size, "skins");

        click_button(&mut ready, size, ButtonId::OpenSettings);
        assert_hits_fit_without_overlap(&ready, size, "settings");

        click_button(&mut ready, size, ButtonId::OpenMods);
        click_button(&mut ready, size, ButtonId::ModManagerPickProfile(1));
        click_button(&mut ready, size, ButtonId::DeleteTabProfile);
        assert_hits_fit_without_overlap(&ready, size, "confirm-remove-profile");
        click_button(&mut ready, size, ButtonId::CancelRemoveProfile);

        let mut installing = MobileLauncher::new(Box::new(StubBackend {
            accounts: vec![Account::Offline {
                name: "Steve".into(),
            }],
            active: Some(0),
            setup: Setup::Running(Progress {
                stage: Stage::Download,
                done: 268_800_000,
                total: 672_000_000,
                bytes_per_second: 67_200_000,
                note: None,
            }),
            ..StubBackend::default()
        }));
        installing.route = Route::Install;
        assert_hits_fit_without_overlap(&installing, size, "install-running");
        click_button(&mut installing, size, ButtonId::OpenSettings);
        assert_hits_fit_without_overlap(&installing, size, "settings-during-install");
    }
}

fn assert_text_entry_visible(
    launcher: &mut MobileLauncher,
    size: [u32; 2],
    label: &str,
    required: &[ButtonId],
) {
    assert!(
        launcher.wants_text(),
        "{label}: expected wants_text() == true"
    );
    assert_hits_fit_without_overlap(launcher, size, label);
    with_ui(size, (0.0, 0.0), |ui| {
        let hits = launcher.layout(&mut Canvas::Hit(ui));
        for &id in required {
            let hit = hits
                .iter()
                .find(|h| h.id == id)
                .unwrap_or_else(|| panic!("{label} @ {size:?}: missing required control {id:?}"));
            assert!(
                hit.w >= 48.0 && hit.h >= 30.0,
                "{label} @ {size:?}: control {id:?} too small ({}x{})",
                hit.w,
                hit.h
            );
        }
    });
}

#[test]
fn text_entry_screens_fit_above_phone_keyboard() {
    // Desktop/legacy pixel keyboard sizes + Android native-density A32 keyboard-up sizes (~200-250dp).
    for size in [
        [2400, 560],
        [1600, 450],
        [1280, 320],
        [914, 250],
        [914, 213],
        [914, 200],
    ] {
        // 1. Play Offline name (empty with validation hint + filled valid name)
        let mut auth = MobileLauncher::new(Box::new(StubBackend::default()));
        click_button(&mut auth, size, ButtonId::OpenOfflineEntry);
        assert_text_entry_visible(
            &mut auth,
            size,
            "kb-offline-empty",
            &[
                ButtonId::FocusOfflineName,
                ButtonId::ConfirmOffline,
                ButtonId::CancelOffline,
            ],
        );
        auth.typed("Player_One");
        assert_text_entry_visible(
            &mut auth,
            size,
            "kb-offline-filled",
            &[
                ButtonId::FocusOfflineName,
                ButtonId::ConfirmOffline,
                ButtonId::CancelOffline,
            ],
        );

        // 2. Ely.by sign-in (username, password, 2FA focus states + error message)
        click_button(&mut auth, size, ButtonId::CancelOffline);
        click_button(&mut auth, size, ButtonId::ToggleMoreAuth);
        click_button(&mut auth, size, ButtonId::OpenElyEntry);
        for (tag, focus) in [
            ("kb-ely-user", ButtonId::FocusElyUsername),
            ("kb-ely-pass", ButtonId::FocusElyPassword),
            ("kb-ely-otp", ButtonId::FocusElyOtp),
        ] {
            click_button(&mut auth, size, focus);
            assert_text_entry_visible(
                &mut auth,
                size,
                tag,
                &[
                    ButtonId::FocusElyUsername,
                    ButtonId::FocusElyPassword,
                    ButtonId::FocusElyOtp,
                    ButtonId::ConfirmEly,
                    ButtonId::CancelEly,
                ],
            );
        }
        auth.ely_error = Some("Invalid username or password.".into());
        assert_text_entry_visible(
            &mut auth,
            size,
            "kb-ely-error",
            &[
                ButtonId::FocusElyUsername,
                ButtonId::FocusElyPassword,
                ButtonId::FocusElyOtp,
                ButtonId::ConfirmEly,
                ButtonId::CancelEly,
            ],
        );

        // 3. New Profile (clean + validation error + extra mods list) & host note()
        let mut signed_in = MobileLauncher::new(Box::new(StubBackend {
            accounts: vec![Account::Offline {
                name: "Steve".into(),
            }],
            active: Some(0),
            setup: Setup::Ready,
            profiles: vec![
                Profile {
                    id: VANILLA.into(),
                    name: "Vanilla".into(),
                    description: "Base".into(),
                    mods: 0,
                    size: 672_000_000,
                    worlds: 1,
                    state: ProfileState::Installed,
                },
                Profile {
                    id: "my-pack".into(),
                    name: "My Pack".into(),
                    description: "Custom".into(),
                    mods: 1,
                    size: 3_000_000,
                    worlds: 1,
                    state: ProfileState::Installed,
                },
            ],
            selected_profile: "my-pack".into(),
            ..StubBackend::default()
        }));
        signed_in.note("The game closed unexpectedly last time.");
        assert_eq!(
            signed_in.status_note.as_deref(),
            Some("The game closed unexpectedly last time.")
        );

        click_button(&mut signed_in, size, ButtonId::OpenMods);
        click_button(&mut signed_in, size, ButtonId::OpenNewProfile);
        assert_text_entry_visible(
            &mut signed_in,
            size,
            "kb-new-profile",
            &[ButtonId::FocusRenameInput, ButtonId::SaveRenameProfile],
        );
        signed_in.mods_error = Some("Enter 1 to 32 characters.".into());
        assert_text_entry_visible(
            &mut signed_in,
            size,
            "kb-new-profile-error",
            &[ButtonId::FocusRenameInput, ButtonId::SaveRenameProfile],
        );
        click_button(&mut signed_in, size, ButtonId::SaveRenameProfile);
        click_button(&mut signed_in, size, ButtonId::RenameTabProfile);
        assert_text_entry_visible(
            &mut signed_in,
            size,
            "kb-rename",
            &[ButtonId::FocusRenameInput, ButtonId::SaveRenameProfile],
        );

        // 5. Mod Manager search input focused
        click_button(&mut signed_in, size, ButtonId::SaveRenameProfile);
        click_button(&mut signed_in, size, ButtonId::FocusModSearch);
        assert_text_entry_visible(
            &mut signed_in,
            size,
            "kb-mod-search",
            &[ButtonId::FocusModSearch, ButtonId::SubmitModSearch],
        );
    }
}

#[test]
fn new_profile_is_made_at_once_and_named_in_place_on_its_tab() {
    let shows = |l: &MobileLauncher, size: [u32; 2], id: ButtonId| {
        with_ui(size, (0.0, 0.0), |ui| {
            l.layout(&mut Canvas::Hit(ui)).iter().any(|h| h.id == id)
        })
    };
    // 914x213: the A32 with its keyboard up.
    let (phone, keyboard) = ([914, 411], [914, 213]);
    let mut l = MobileLauncher::new(Box::new(StubBackend {
        accounts: vec![Account::Offline {
            name: "Steve".into(),
        }],
        active: Some(0),
        setup: Setup::Ready,
        ..StubBackend::default()
    }));
    click_button(&mut l, phone, ButtonId::OpenMods);
    click_button(&mut l, phone, ButtonId::OpenNewProfile);
    // Made and shown at once, its name ready to type: only that row above the keyboard.
    assert_eq!(l.backend().profiles()[1].name, "Profile 1");
    assert_eq!(l.mods_profile_id.as_deref(), Some("profile-1"));
    assert!(l.wants_text());
    assert!(shows(&l, keyboard, ButtonId::FocusRenameInput));
    assert!(!shows(&l, keyboard, ButtonId::OpenAccountSwitcher));

    // Tapping away keeps the default name; the next one is "Profile 2".
    click_button(&mut l, phone, ButtonId::ModManagerPickProfile(0));
    assert!(!l.wants_text());
    assert_eq!(l.backend().profiles()[1].name, "Profile 1");
    click_button(&mut l, phone, ButtonId::OpenNewProfile);
    assert_eq!(l.backend().profiles()[2].name, "Profile 2");

    // Typing a name and Enter renames it in place.
    l.typed("Pack\n");
    assert_eq!(l.backend().profiles()[2].name, "Pack");
    assert!(!l.wants_text());
    // The pencil and the bin belong to the modded tab shown, never to Vanilla.
    assert!(shows(&l, phone, ButtonId::RenameTabProfile));
    click_button(&mut l, phone, ButtonId::ModManagerPickProfile(0));
    assert!(!shows(&l, phone, ButtonId::RenameTabProfile));
    assert!(!shows(&l, phone, ButtonId::DeleteTabProfile));
}

#[test]
fn settings_frees_the_install_cache_only_when_asked_and_installed() {
    let shows = |l: &MobileLauncher, size: [u32; 2], id: ButtonId| {
        with_ui(size, (0.0, 0.0), |ui| {
            l.layout(&mut Canvas::Hit(ui)).iter().any(|h| h.id == id)
        })
    };
    let signed_in = |setup: Setup, reclaim: u64| {
        MobileLauncher::new(Box::new(StubBackend {
            accounts: vec![Account::Offline {
                name: "Steve".into(),
            }],
            active: Some(0),
            setup,
            reclaim,
            ..StubBackend::default()
        }))
    };
    for size in [[914, 411], [856, 384], [2400, 1080]] {
        // Nothing to free, or still installing: no offer.
        let mut none = signed_in(Setup::Ready, 0);
        click_button(&mut none, size, ButtonId::OpenSettings);
        assert!(!shows(&none, size, ButtonId::FreeSpace), "{size:?}");
        let mut busy = signed_in(Setup::Needed, 800_000_000);
        click_button(&mut busy, size, ButtonId::OpenSettings);
        assert!(!shows(&busy, size, ButtonId::FreeSpace), "{size:?}");

        let mut l = signed_in(Setup::Ready, 800_000_000);
        click_button(&mut l, size, ButtonId::OpenSettings);
        assert_hits_fit_without_overlap(&l, size, "settings-free-space");
        // It asks first; any other tap is a no.
        click_button(&mut l, size, ButtonId::FreeSpace);
        assert!(shows(&l, size, ButtonId::ConfirmFreeSpace), "{size:?}");
        assert_hits_fit_without_overlap(&l, size, "free-space-confirm");
        click_button(&mut l, size, ButtonId::CancelFreeSpace);
        assert!(!shows(&l, size, ButtonId::ConfirmFreeSpace));
        assert_eq!(l.backend().reclaimable(), Some(800_000_000));
        click_button(&mut l, size, ButtonId::FreeSpace);
        click_button(&mut l, size, ButtonId::ConfirmFreeSpace);
        assert_eq!(l.backend().reclaimable(), Some(0));
        assert_eq!(l.freed, Some(Ok(800_000_000)));
        assert!(!shows(&l, size, ButtonId::FreeSpace), "{size:?}");
        assert_eq!(l.route, Route::Settings);
    }
}

#[test]
fn rail_places_settings_right_after_skins() {
    let launcher = MobileLauncher::new(Box::new(StubBackend {
        accounts: vec![Account::Offline {
            name: "Steve".into(),
        }],
        active: Some(0),
        setup: Setup::Ready,
        ..StubBackend::default()
    }));

    for size in [[1040, 700], [914, 411]] {
        with_ui(size, (0.0, 0.0), |ui| {
            let hits = launcher.layout(&mut Canvas::Hit(ui));
            let mods = hits.iter().find(|h| h.id == ButtonId::OpenMods).unwrap();
            let skins = hits.iter().find(|h| h.id == ButtonId::OpenSkins).unwrap();
            let settings = hits
                .iter()
                .find(|h| h.id == ButtonId::OpenSettings)
                .unwrap();
            // Nav stack sits under logo; Settings follows Skins with the exact same step as Mods -> Skins.
            let step_mods_skins = skins.y - mods.y;
            let step_skins_settings = settings.y - skins.y;
            assert!(
                step_mods_skins > 0.0 && (step_skins_settings - step_mods_skins).abs() < 0.5,
                "size {size:?}: expected Settings right after Skins (mods->skins={step_mods_skins}, skins->settings={step_skins_settings})"
            );
        });
    }
}

#[test]
fn profile_dropdown_matches_bar_width_opens_upward_and_scrolls_long_lists() {
    let mut profiles = vec![Profile {
        id: VANILLA.into(),
        name: "Vanilla".into(),
        description: "Base".into(),
        mods: 0,
        size: 672_000_000,
        worlds: 1,
        state: ProfileState::Installed,
    }];
    for i in 1..=11 {
        profiles.push(Profile {
            id: format!("pack-{i}"),
            name: format!("Modpack {i}"),
            description: "Custom pack".into(),
            mods: i,
            size: (i as u64) * 12_000_000,
            worlds: i % 3,
            state: ProfileState::Installed,
        });
    }

    for size in [[1040, 700], [914, 411]] {
        let mut launcher = MobileLauncher::new(Box::new(StubBackend {
            accounts: vec![Account::Offline {
                name: "Steve".into(),
            }],
            active: Some(0),
            setup: Setup::Ready,
            profiles: profiles.clone(),
            selected_profile: VANILLA.into(),
            ..StubBackend::default()
        }));

        click_button(&mut launcher, size, ButtonId::ToggleProfilePicker);
        assert!(launcher.profile_popup_open);
        assert_hits_fit_without_overlap(&launcher, size, "profile-dropdown-long");

        with_ui(size, (0.0, 0.0), |ui| {
            let hits = launcher.layout(&mut Canvas::Hit(ui));
            let bar = hits
                .iter()
                .find(|h| h.id == ButtonId::ToggleProfilePicker)
                .unwrap();
            let first_row = hits
                .iter()
                .find(|h| h.id == ButtonId::SelectProfile(0))
                .unwrap();
            let scroll_down = hits
                .iter()
                .find(|h| h.id == ButtonId::ProfileScrollDown)
                .expect("expected scrollbar down region when 12 profiles overflow dropdown");

            // Dropdown opens upward strictly above the bottom bar and fits inside the bar's width.
            assert!(
                first_row.y + first_row.h < bar.y,
                "size {size:?}: dropdown should open upward above the profile bar"
            );
            assert!(
                first_row.x >= bar.x && scroll_down.x + scroll_down.w <= bar.x + bar.w,
                "size {size:?}: dropdown rows + scrollbar must match profile bar horizontal bounds"
            );
        });

        // Scroll down via scrollbar tap + scroll wheel until the final profile is visible and selectable.
        click_button(&mut launcher, size, ButtonId::ProfileScrollDown);
        assert_eq!(launcher.profile_scroll, 1);
        for _ in 0..12 {
            launcher.scroll(-1.0);
        }
        assert_hits_fit_without_overlap(&launcher, size, "profile-dropdown-scrolled-bottom");
        click_button(&mut launcher, size, ButtonId::SelectProfile(11));
        assert!(!launcher.profile_popup_open);
        assert_eq!(launcher.backend().selected_profile(), "pack-11");
    }
}

#[test]
fn launcher_font_choice_persists_in_horizon_txt_across_restarts() {
    let unique = format!(
        "meridian-font-pref-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let dir = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("horizon.txt"),
        "friendsRelay:wss://relay.example\n",
    )
    .unwrap();

    let size = [914, 411];
    crate::ui_theme::set_pixel_font(false);

    let make_launcher = |dir: &std::path::Path| {
        MobileLauncher::new(Box::new(StubBackend {
            accounts: vec![Account::Offline {
                name: "Steve".into(),
            }],
            active: Some(0),
            setup: Setup::Ready,
            ..StubBackend::default()
        }))
        .with_prefs_dir(dir.to_path_buf())
    };

    let mut first = make_launcher(&dir);
    assert!(!crate::ui_theme::pixel_font());
    click_button(&mut first, size, ButtonId::OpenSettings);
    click_button(&mut first, size, ButtonId::ToggleLauncherFont);
    assert!(crate::ui_theme::pixel_font());
    assert_eq!(
        crate::ui_theme::preference(&dir, "launcherFont").as_deref(),
        Some("pixel")
    );
    assert_eq!(
        crate::ui_theme::preference(&dir, "friendsRelay").as_deref(),
        Some("wss://relay.example")
    );

    // Simulate a fresh launcher process after switching back from the game.
    crate::ui_theme::set_pixel_font(false);
    let mut restarted = make_launcher(&dir);
    assert!(crate::ui_theme::pixel_font());

    // Toggle back to modern and verify that persists across the next restart too.
    click_button(&mut restarted, size, ButtonId::OpenSettings);
    click_button(&mut restarted, size, ButtonId::ToggleLauncherFont);
    assert!(!crate::ui_theme::pixel_font());
    assert_eq!(
        crate::ui_theme::preference(&dir, "launcherFont").as_deref(),
        Some("modern")
    );

    crate::ui_theme::set_pixel_font(true);
    let _third = make_launcher(&dir);
    assert!(!crate::ui_theme::pixel_font());

    crate::ui_theme::set_pixel_font(false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skins_put_on_a_chosen_png_and_send_ely_to_its_website() {
    let skins = Arc::new(Mutex::new(SkinStub::default()));
    let mut l = MobileLauncher::new(Box::new(StubBackend {
        accounts: vec![
            Account::Microsoft {
                name: "Steve".into(),
            },
            Account::Ely {
                name: "Alex".into(),
            },
            Account::Offline {
                name: "Notch".into(),
            },
        ],
        active: Some(0),
        setup: Setup::Ready,
        skins: skins.clone(),
        ..StubBackend::default()
    }));
    let size = [914, 411];
    let ids = |l: &MobileLauncher| {
        with_ui(size, (0.0, 0.0), |ui| {
            l.layout(&mut Canvas::Hit(ui))
                .iter()
                .map(|h| h.id)
                .collect::<Vec<_>>()
        })
    };
    click_button(&mut l, size, ButtonId::OpenSkins);
    assert_eq!(
        click_button(&mut l, size, ButtonId::PickSkin),
        Some(Action::PickSkin)
    );
    // Nothing to put on until the picker hands a file over.
    l.poll();
    assert!(!ids(&l).contains(&ButtonId::UseSkin));

    skins.lock().unwrap().picked = Some(Ok(PickedSkin {
        png: b"skin.png".to_vec(),
        face: [7; 8 * 8 * 4],
        skin: vec![9; 64 * 64 * 4].into(),
    }));
    l.poll();
    // Classic until the player picks Slim.
    assert!(!l.skin_slim);
    for size in [[914, 411], [856, 384], [2400, 1080]] {
        assert_hits_fit_without_overlap(&l, size, "skin-chosen");
    }
    click_button(&mut l, size, ButtonId::SkinArms(true));
    click_button(&mut l, size, ButtonId::UseSkin);
    assert_eq!(
        skins.lock().unwrap().uploads,
        vec![(b"skin.png".to_vec(), true)]
    );
    // While it goes on there is no second button to tap.
    assert!(!ids(&l).contains(&ButtonId::UseSkin));
    skins.lock().unwrap().status = SkinUpload::Done;
    l.poll();
    assert!(
        l.skin_pick.is_none(),
        "uploaded: the account's own head shows it"
    );

    skins.lock().unwrap().picked = Some(Err("Choose a 64x64 PNG skin.".into()));
    l.poll();
    assert_eq!(l.skin_error.as_deref(), Some("Choose a 64x64 PNG skin."));

    l.backend.switch_account(1).unwrap();
    assert_eq!(
        click_button(&mut l, size, ButtonId::OpenElySkins),
        Some(Action::OpenUrl(ELY_SKINS.into()))
    );
    assert!(
        !ids(&l).contains(&ButtonId::PickSkin),
        "Ely.by: on its website"
    );
    l.backend.switch_account(2).unwrap();
    assert!(
        ids(&l).contains(&ButtonId::PickSkin),
        "offline: kept on this phone"
    );
}

#[test]
fn log_lines_take_their_entrys_level() {
    let lines: Vec<String> = [
        "[08:53:52] [unnamed/INFO]: Horizon 0.1.0",
        "[08:53:52] [Narrator/ERROR]: Error while loading the narrator",
        "    at a stack frame",
        "[08:53:52] [unnamed/WARN]: Missing downlevel flags",
        "The underlying API does not support enough features",
        "[08:53:53] [Render thread/FATAL]: Out of memory",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(log_levels(&lines), [0, 2, 2, 1, 1, 2]);
}

#[test]
fn home_shows_the_game_log_and_copies_the_level_chosen() {
    let mut l = MobileLauncher::new(Box::new(StubBackend {
        accounts: vec![Account::Offline {
            name: "Steve".into(),
        }],
        active: Some(0),
        setup: Setup::Ready,
        log: [
            "[1] [main/INFO]: started",
            "[2] [main/ERROR]: broke",
            "    at here",
        ]
        .map(str::to_owned)
        .to_vec(),
        ..StubBackend::default()
    }));
    for size in [[914, 411], [856, 384], [2400, 1080]] {
        assert_hits_fit_without_overlap(&l, size, "home-log");
    }
    let size = [914, 411];
    assert_eq!(
        click_button(&mut l, size, ButtonId::CopyLog),
        Some(Action::CopyText(
            "[1] [main/INFO]: started\n[2] [main/ERROR]: broke\n    at here".into()
        ))
    );
    assert!(l.log_copied);
    click_button(&mut l, size, ButtonId::LogLevel(2));
    assert!(!l.log_copied, "another tap: Copy again");
    assert_eq!(
        click_button(&mut l, size, ButtonId::CopyLog),
        Some(Action::CopyText(
            "[2] [main/ERROR]: broke\n    at here".into()
        ))
    );
}

#[test]
fn settings_keeps_game_arguments_and_links_about() {
    let mut l = MobileLauncher::new(Box::new(StubBackend {
        accounts: vec![Account::Offline {
            name: "Steve".into(),
        }],
        active: Some(0),
        setup: Setup::Ready,
        ..StubBackend::default()
    }));
    let ids = |l: &MobileLauncher, size: [u32; 2]| {
        with_ui(size, (0.0, 0.0), |ui| {
            l.layout(&mut Canvas::Hit(ui))
                .iter()
                .map(|h| h.id)
                .collect::<Vec<_>>()
        })
    };
    click_button(&mut l, [914, 411], ButtonId::OpenSettings);
    // Below everything: the arguments, then About, at every phone size.
    for size in [[914, 411], [856, 384], [2400, 1080]] {
        assert_hits_fit_without_overlap(&l, size, "settings");
        let shown = ids(&l, size);
        for id in [
            ButtonId::ToggleLauncherFont,
            ButtonId::FocusGameArgs,
            ButtonId::OpenSourceCode,
            ButtonId::OpenLicenses,
        ] {
            assert!(shown.contains(&id), "{size:?}: {id:?}");
        }
    }
    let size = [914, 411];
    click_button(&mut l, size, ButtonId::FocusGameArgs);
    assert!(l.wants_text());
    // With the keyboard up, the field is what shows; the rail stays.
    let keyboard = ids(&l, [914, 213]);
    assert!(keyboard.contains(&ButtonId::FocusGameArgs));
    assert!(keyboard.contains(&ButtonId::OpenMods));
    assert!(!keyboard.contains(&ButtonId::OpenAccountSwitcher));
    l.typed(r#"--render-distance 8 --world "New World""#);
    l.typed("\n");
    assert!(!l.wants_text());
    assert_eq!(
        l.backend().game_args(),
        r#"--render-distance 8 --world "New World""#
    );
    // An open quote is refused, with the reason, and the kept arguments stay.
    click_button(&mut l, size, ButtonId::FocusGameArgs);
    l.typed(r#" "x"#);
    click_button(&mut l, size, ButtonId::GoHome);
    assert!(l.game_args_error.is_some());
    assert_eq!(
        l.backend().game_args(),
        r#"--render-distance 8 --world "New World""#
    );
    // Play refused on them (the host checks them as the game will): Settings says why.
    click_button(&mut l, size, ButtonId::GoHome);
    l.game_args_refused("missing value after --bogus");
    assert_eq!(l.route, Route::Settings);
    assert_eq!(
        l.game_args_error.as_deref(),
        Some("missing value after --bogus")
    );
    assert_eq!(
        click_button(&mut l, size, ButtonId::OpenSourceCode),
        Some(Action::OpenUrl(SOURCE_CODE.into()))
    );
    assert_eq!(
        click_button(&mut l, size, ButtonId::OpenLicenses),
        Some(Action::OpenUrl(LICENSES.into()))
    );
}

#[test]
fn a_question_pops_up_over_the_page_and_alone_takes_taps() {
    let mut l = MobileLauncher::new(Box::new(StubBackend {
        accounts: vec![Account::Offline {
            name: "Steve".into(),
        }],
        active: Some(0),
        setup: Setup::Ready,
        ..StubBackend::default()
    }));
    let size = [914, 411];
    click_button(&mut l, size, ButtonId::OpenMods);
    click_button(&mut l, size, ButtonId::OpenNewProfile);
    click_button(&mut l, size, ButtonId::SaveRenameProfile);
    click_button(&mut l, size, ButtonId::DeleteTabProfile);
    let shown = with_ui(size, (0.0, 0.0), |ui| {
        l.layout(&mut Canvas::Hit(ui))
            .iter()
            .map(|h| h.id)
            .collect::<Vec<_>>()
    });
    assert_eq!(
        shown,
        [
            ButtonId::CancelRemoveProfile,
            ButtonId::ConfirmRemoveProfile
        ]
    );
    for size in [[914, 411], [856, 384], [2400, 1080]] {
        assert_hits_fit_without_overlap(&l, size, "delete-popup");
    }
    click_button(&mut l, size, ButtonId::CancelRemoveProfile);
    assert_eq!(l.backend().profiles().len(), 2, "kept");
}
