use super::widgets::{Canvas, count_label, profile_stats_line};
use super::{Action, AuthForm, ButtonId, MobileLauncher, Route};
use crate::hud::{HudAtlas, HudBatch};
use crate::launcher_backend::{
    Account, Backend, Fake, Head, ModInfo, ModSearch, Profile, ProfileMod, ProfileState, Progress,
    Setup, SignIn, Stage, VANILLA, profile_id, valid_offline_name,
};
use crate::ui_theme::Ui;

struct StubBackend {
    accounts: Vec<Account>,
    active: Option<usize>,
    sign_in: SignIn,
    setup: Setup,
    profiles: Vec<Profile>,
    selected_profile: String,
    mods: Vec<ProfileMod>,
    search: ModSearch,
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
    fn presets(&self) -> Vec<Profile> {
        vec![Profile {
            id: "performance".into(),
            name: "Performance".into(),
            description: "Faster chunks.".into(),
            mods: 2,
            size: 3_500_000,
            worlds: 0,
            state: ProfileState::Available,
        }]
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
    assert_eq!(profile_stats_line(&prof), "1 mod  |  1 world  |  3.8 MB");
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
    let mut launcher = MobileLauncher::new(Box::new(Fake::default()));
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

    // Profile picker via bottom profile bar + sidebar Skins navigation
    click_button(&mut launcher, size, ButtonId::ToggleProfilePicker);
    assert_eq!(launcher.route, Route::Profiles);
    click_button(&mut launcher, size, ButtonId::OpenSkins);
    assert_eq!(launcher.route, Route::Skins);
    click_button(&mut launcher, size, ButtonId::GoHome);

    // Create profile with mixed-case display name distinct from its slug id
    click_button(&mut launcher, size, ButtonId::OpenNewProfile);
    launcher.typed("My Pack");
    click_button(&mut launcher, size, ButtonId::SelectPreset(0));
    click_button(&mut launcher, size, ButtonId::CreateProfile);
    assert_eq!(launcher.backend().selected_profile(), "my-pack");
    assert_eq!(launcher.backend().profiles()[1].name, "My Pack");

    click_button(&mut launcher, size, ButtonId::OpenModsForSelected);
    assert_eq!(launcher.route, Route::Mods);
    click_button(&mut launcher, size, ButtonId::ToggleProfileMod(0));
    assert!(!launcher.backend().mods("my-pack")[0].enabled);
    click_button(&mut launcher, size, ButtonId::RemoveProfileMod(0));
    assert_eq!(launcher.backend().mods("my-pack").len(), 1);

    click_button(&mut launcher, size, ButtonId::GoHome);
    click_button(&mut launcher, size, ButtonId::OpenRenameSelected);
    for _ in 0..7 {
        launcher.backspace();
    }
    launcher.typed("Survival Plus");
    click_button(&mut launcher, size, ButtonId::SaveRenameProfile);
    assert_eq!(launcher.backend().profiles()[1].name, "Survival Plus");

    click_button(&mut launcher, size, ButtonId::AskRemoveSelected);
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
    for size in [[1040, 700], [800, 360]] {
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
        assert_hits_fit_without_overlap(&ready, size, "profiles-picker");

        click_button(&mut ready, size, ButtonId::OpenAccountSwitcher);
        assert_hits_fit_without_overlap(&ready, size, "account-switcher");

        click_button(&mut ready, size, ButtonId::GoHome);
        click_button(&mut ready, size, ButtonId::OpenNewProfile);
        assert_hits_fit_without_overlap(&ready, size, "new-profile");

        click_button(&mut ready, size, ButtonId::OpenMods);
        assert_hits_fit_without_overlap(&ready, size, "mod-manager");

        click_button(&mut ready, size, ButtonId::OpenSkins);
        assert_hits_fit_without_overlap(&ready, size, "skins");

        click_button(&mut ready, size, ButtonId::OpenSettings);
        assert_hits_fit_without_overlap(&ready, size, "settings");

        click_button(&mut ready, size, ButtonId::GoHome);
        click_button(&mut ready, size, ButtonId::OpenRenameSelected);
        assert_hits_fit_without_overlap(&ready, size, "rename-profile");

        click_button(&mut ready, size, ButtonId::CancelRenameProfile);
        click_button(&mut ready, size, ButtonId::AskRemoveSelected);
        assert_hits_fit_without_overlap(&ready, size, "confirm-remove-profile");

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
