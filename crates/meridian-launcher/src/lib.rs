//! Meridian's launcher contract: what the launcher's screens (`mobile_launcher`, Antigravity) ask
//! for and the backend (sign-in, downloads, preparing the game files; Codex) provides. The screens
//! draw every frame from snapshots ([`Backend::account`], [`Backend::sign_in`], [`Backend::setup`]);
//! the backend works on its own threads, so no call here blocks a frame.
use std::time::{Duration, Instant};

/// Who plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Account {
    /// Signed in with Microsoft: online servers that check accounts work.
    Microsoft { name: String },
    /// Signed in with Ely.by, for servers using Ely.by authentication.
    Ely { name: String },
    /// A name only: single player, LAN, and servers in offline mode.
    Offline { name: String },
}

/// Microsoft's device-code sign-in, as the screen shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SignIn {
    #[default]
    Idle,
    /// Asking Microsoft for a code.
    Starting,
    /// Show `code` and send the player to `url`; the backend waits until they finish there.
    Code { url: String, code: String },
    /// Signed in; [`Backend::account`] is now this account.
    Done(Account),
    /// Why it failed, ready to show.
    Failed(String),
}

/// The setup screen's four stages, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Check,
    Download,
    Verify,
    Prepare,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    /// Work done and the total in the stage's unit: bytes while downloading, files otherwise.
    /// `total` is 0 while unknown.
    pub done: u64,
    pub total: u64,
    /// Measured download speed in bytes per second; 0 outside downloads or before a measurement.
    pub bytes_per_second: u64,
    /// What is happening right now worth telling the player, such as a retry.
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setup {
    /// No prepared game files yet, and setup hasn't started.
    Needed,
    Running(Progress),
    /// Stopped at `stage`. Files already checked stay on disk, so a retry continues from there.
    Failed {
        stage: Stage,
        error: String,
    },
    /// The prepared game files are in place: Play can start the game.
    Ready,
}

/// The profile with no modpack. It plays in the game root itself, so worlds made before profiles
/// existed stay where they were.
pub const VANILLA: &str = "vanilla";

/// A profile, as PC launchers' installations: a game folder of its own (worlds, settings, mods)
/// played with a modpack (Horizon's `.hpack`), or with none for [`VANILLA`]. Horizon only runs
/// 26.3, so profiles differ by their mods, not by version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// Stable id: [`VANILLA`], or the folder name under `profiles/` (see [`profile_dir`]).
    pub id: String,
    pub name: String,
    pub description: String,
    /// Mods in its modpack; 0 for vanilla.
    pub mods: usize,
    /// Bytes: on disk once installed, else the download.
    pub size: u64,
    /// Worlds in its own folder (worlds are separate per profile).
    pub worlds: usize,
    pub state: ProfileState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileState {
    /// Offered, not installed: Install downloads its mods.
    Available,
    /// Installing or updating, in the setup screen's units (stage Download, bytes).
    Installing(Progress),
    /// Ready to play.
    Installed,
    /// Install stopped; already checked files stay, so installing again continues.
    Failed(String),
}

/// A profile's game folder under the launcher's game root: the root itself for [`VANILLA`],
/// `profiles/<id>` otherwise. Ids are folder names the backend chose, never player input.
pub fn profile_dir(root: &std::path::Path, id: &str) -> std::path::PathBuf {
    if id == VANILLA {
        root.to_path_buf()
    } else {
        root.join("profiles").join(id)
    }
}

/// A profile name as the player types it: 1 to 32 characters, no control characters.
pub fn valid_profile_name(name: &str) -> bool {
    let name = name.trim();
    (1..=32).contains(&name.chars().count()) && !name.chars().any(char::is_control)
}

/// The folder name for a new profile called `name`: lowercase ASCII letters and digits joined by
/// `-` (never a path, `..` or [`VANILLA`]), made unique among `taken` with `-2`, `-3`…
pub fn profile_id(name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let mut slug: String = slug.trim_end_matches('-').chars().take(32).collect();
    if slug.is_empty() {
        slug = "profile".into();
    }
    let free = |id: &str| id != VANILLA && !taken.iter().any(|t| t == id);
    if free(&slug) {
        return slug;
    }
    (2..).map(|n| format!("{slug}-{n}")).find(|id| free(id)).expect("unbounded")
}

/// A mod as the mod manager shows it, from Horizon's mod registry.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// Download bytes.
    pub size: u64,
}

/// A mod in a profile, switched on or off for it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProfileMod {
    pub info: ModInfo,
    pub enabled: bool,
}

/// The mod manager's registry search ([`Backend::search_mods`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ModSearch {
    #[default]
    Idle,
    Searching,
    Done(Vec<ModInfo>),
    Failed(String),
}

/// A player's face to draw: the skin's 8×8 front with the hat layer over it, RGBA rows.
pub type Head = [u8; 8 * 8 * 4];

/// Where an Ely.by account changes its skin: Ely.by's public API takes no uploads.
pub const ELY_SKINS: &str = "https://ely.by/skins";

/// A skin file the player chose: the PNG, its face for a preview, and whether its arms look slim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedSkin {
    pub png: Vec<u8>,
    pub face: Head,
    pub slim: bool,
}

/// Where putting on a skin ([`Backend::use_skin`]) stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SkinUpload {
    #[default]
    Idle,
    Uploading,
    Done,
    Failed(String),
}

/// Sensitive session data for the in-process Play handoff. This type has no Debug implementation.
pub struct GameSession(String);

impl GameSession {
    /// Parse with `horizon_client::auth::AuthSession::parse` and discard the JSON immediately.
    pub fn as_json(&self) -> &str {
        &self.0
    }
}

/// The launcher's backend. Implemented by Codex's real one, and by [`Fake`] meanwhile.
pub trait Backend: Send {
    fn account(&self) -> Option<Account>;
    /// Starts Microsoft sign-in; [`Self::sign_in`] follows it.
    fn sign_in_microsoft(&mut self);
    /// Starts Ely.by sign-in on a worker. The password and optional OTP
    /// exist only for this request.
    fn sign_in_ely(&mut self, _username: &str, _password: &str, _otp: &str) -> Result<(), String> {
        Err("Ely.by sign-in is unavailable in this launcher.".into())
    }
    fn sign_in(&self) -> SignIn;
    /// Makes `name` the offline account, or says why it can't be (see [`valid_offline_name`]).
    fn sign_in_offline(&mut self, name: &str) -> Result<Account, String>;
    fn sign_out(&mut self);
    /// The current online session, if any. Offline accounts return None.
    fn game_session(&self) -> Result<Option<GameSession>, String> {
        Ok(None)
    }
    fn setup(&self) -> Setup;
    /// Starts setup, or retries it after a failure.
    fn start_setup(&mut self);
    fn cancel_setup(&mut self);
    /// The bytes [`Self::free_space`] would free: what only installing the game uses (Mojang's
    /// downloads and the Java that prepares them), which playing never reads. `None` while it
    /// is being measured.
    fn reclaimable(&self) -> Option<u64> {
        Some(0)
    }
    /// Removes those files, the game installed; the bytes freed. Worlds, profiles, settings and
    /// the game stay; repairing or updating the game downloads them again.
    fn free_space(&mut self) -> Result<u64, String> {
        Err("There is nothing to free up.".into())
    }
    /// The skin file the platform's picker handed over since the last call, or why it can't be a
    /// skin.
    fn take_picked_skin(&mut self) -> Option<Result<PickedSkin, String>> {
        None
    }
    /// Makes `png` the account's skin: a Microsoft account's profile gets it (on a worker), an
    /// offline account keeps it for the game on this phone. How it went: [`Self::skin_upload`].
    fn use_skin(&mut self, _png: Vec<u8>, _slim: bool) {}
    fn skin_upload(&self) -> SkinUpload {
        SkinUpload::Idle
    }
    /// Every profile, installed or offered: [`VANILLA`] first, then the modpacks.
    fn profiles(&self) -> Vec<Profile> {
        vec![Profile {
            id: VANILLA.into(),
            name: "Vanilla".into(),
            description: "Minecraft as it comes, no mods.".into(),
            mods: 0,
            size: 0,
            worlds: 0,
            state: ProfileState::Installed,
        }]
    }
    /// Starts installing (or updating) a profile's modpack; [`Self::profiles`] follows it.
    fn install_profile(&mut self, _id: &str) {}
    /// The profile Play starts (in [`profile_dir`]).
    fn selected_profile(&self) -> String {
        VANILLA.into()
    }
    fn select_profile(&mut self, id: &str) -> Result<(), String> {
        if id == VANILLA {
            Ok(())
        } else {
            Err("Profiles are unavailable in this launcher.".into())
        }
    }
    /// Deletes an installed profile's folder, **its worlds included**: the screen asks first.
    /// Vanilla can't be removed.
    fn remove_profile(&mut self, _id: &str) -> Result<(), String> {
        Err("This profile can't be removed.".into())
    }
    /// Starting points for a new profile (Performance, Quality of life…): name, description and
    /// mods, state `Available`.
    fn presets(&self) -> Vec<Profile> {
        Vec::new()
    }
    /// Makes a profile named `name` (see [`valid_profile_name`]), from `preset`'s mods plus `mods`,
    /// and returns its id ([`profile_id`]). It needs [`Self::install_profile`] before Play.
    fn create_profile(
        &mut self,
        _name: &str,
        _preset: Option<&str>,
        _mods: &[String],
    ) -> Result<String, String> {
        Err("Profiles are unavailable in this launcher.".into())
    }
    /// Renames a profile; its id and folder stay.
    fn rename_profile(&mut self, _id: &str, _name: &str) -> Result<(), String> {
        Err("Profiles are unavailable in this launcher.".into())
    }

    /// Every signed-in account, for the account switcher. Signing in adds one and makes it active.
    fn accounts(&self) -> Vec<Account> {
        self.account().into_iter().collect()
    }
    /// The index in [`Self::accounts`] of the account that plays.
    fn active_account(&self) -> Option<usize> {
        self.account().map(|_| 0)
    }
    fn switch_account(&mut self, index: usize) -> Result<(), String> {
        if self.active_account() == Some(index) {
            Ok(())
        } else {
            Err("No such account.".into())
        }
    }
    /// Signs one account out and forgets it.
    fn remove_account(&mut self, index: usize) -> Result<(), String> {
        if self.active_account() != Some(index) {
            return Err("No such account.".into());
        }
        self.sign_out();
        Ok(())
    }
    /// The account's face for the home screen and the switcher; `None` draws a default head.
    fn player_head(&self, _account: &Account) -> Option<Head> {
        None
    }

    /// A profile's mods (none for Vanilla).
    fn mods(&self, _profile: &str) -> Vec<ProfileMod> {
        Vec::new()
    }
    fn set_mod_enabled(&mut self, _profile: &str, _id: &str, _enabled: bool) -> Result<(), String> {
        Err("Mods are unavailable in this launcher.".into())
    }
    /// Searches Horizon's mod registry; [`Self::mod_search`] follows it. Empty `query`: browse.
    fn search_mods(&mut self, _query: &str) {}
    fn mod_search(&self) -> ModSearch {
        ModSearch::Idle
    }
    /// Adds a registry mod to a profile, switched on; the profile then needs installing again.
    fn add_mod(&mut self, _profile: &str, _id: &str) -> Result<(), String> {
        Err("Mods are unavailable in this launcher.".into())
    }
    fn remove_mod(&mut self, _profile: &str, _id: &str) -> Result<(), String> {
        Err("Mods are unavailable in this launcher.".into())
    }
}

/// A player name Minecraft accepts: 1 to 16 of `A-Z`, `a-z`, `0-9` and `_`.
pub fn valid_offline_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A backend that only pretends, so the screens can be built on a desktop before the real one
/// lands. Microsoft sign-in shows a code for 5 s, then adds "Player". Setup runs through the four
/// stages in about 20 s; the first run fails at 40 % of the download, so the failure screen can be
/// seen, and a retry then succeeds. Profiles install their mods in 3 s; the pretend registry
/// answers a search after 1 s.
// ponytail: delete once the real backend is wired in.
#[derive(Default)]
pub struct Fake {
    /// Offline accounts; a finished Microsoft sign-in adds "Player" after them.
    accounts: Vec<Account>,
    active: Option<usize>,
    microsoft_since: Option<Instant>,
    setup_since: Option<Instant>,
    runs: u32,
    profiles: Vec<FakeProfile>,
    selected: Option<String>,
    search: Option<(String, Instant)>,
}

struct FakeProfile {
    id: String,
    name: String,
    mods: Vec<(String, bool)>,
    installed_since: Option<Instant>,
}

/// The pretend registry: id, name, description, bytes.
const FAKE_REGISTRY: [(&str, &str, &str, u64); 5] = [
    ("faster-chunks", "Faster Chunks", "Builds chunk meshes sooner.", 2_400_000),
    ("better-lighting", "Better Lighting", "Smoother light falloff.", 1_100_000),
    ("minimap", "Minimap", "A map in the corner.", 3_900_000),
    ("zoom", "Zoom", "Hold a key to zoom in.", 300_000),
    ("more-ores", "More Ores", "Adds three new ores.", 5_600_000),
];
const FAKE_PRESETS: [(&str, &str, &str, &[&str]); 2] = [
    ("performance", "Performance", "Smoother play on phones.", &["faster-chunks", "better-lighting"]),
    ("quality-of-life", "Quality of life", "Small helpers for every day.", &["minimap", "zoom"]),
];

fn fake_mod(id: &str) -> Option<ModInfo> {
    FAKE_REGISTRY.iter().find(|m| m.0 == id).map(|&(id, name, description, size)| ModInfo {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        version: "1.0.0".into(),
        size,
    })
}

impl Fake {
    fn microsoft(&self) -> Option<Account> {
        let done = self.microsoft_since.is_some_and(|t| t.elapsed() >= Duration::from_secs(6));
        done.then(|| Account::Microsoft { name: "Player".into() })
    }
    fn custom(&mut self, id: &str) -> Result<&mut FakeProfile, String> {
        if id == VANILLA {
            return Err("Vanilla has no mods: make a new profile for them.".into());
        }
        self.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| "No such profile.".into())
    }
}

impl Backend for Fake {
    fn account(&self) -> Option<Account> {
        self.active_account().map(|i| self.accounts()[i].clone())
    }
    fn sign_in_microsoft(&mut self) {
        self.microsoft_since = Some(Instant::now());
        self.active = None;
    }
    fn sign_in(&self) -> SignIn {
        let Some(since) = self.microsoft_since else {
            return SignIn::Idle;
        };
        match since.elapsed().as_secs() {
            0 => SignIn::Starting,
            1..=5 => SignIn::Code {
                url: "https://www.microsoft.com/link".into(),
                code: "MERIDIAN".into(),
            },
            _ => SignIn::Done(Account::Microsoft { name: "Player".into() }),
        }
    }
    fn sign_in_offline(&mut self, name: &str) -> Result<Account, String> {
        if !valid_offline_name(name) {
            return Err("Use 1 to 16 letters, digits or underscores.".into());
        }
        let account = Account::Offline { name: name.into() };
        let index = match self.accounts.iter().position(|a| *a == account) {
            Some(index) => index,
            None => {
                self.accounts.push(account.clone());
                self.accounts.len() - 1
            }
        };
        self.active = Some(index);
        Ok(account)
    }
    fn sign_out(&mut self) {
        if let Some(index) = self.active_account() {
            let _ = self.remove_account(index);
        }
    }
    fn accounts(&self) -> Vec<Account> {
        self.accounts.iter().cloned().chain(self.microsoft()).collect()
    }
    fn active_account(&self) -> Option<usize> {
        // A Microsoft sign-in that just finished plays until another account is picked.
        match (self.active, self.microsoft()) {
            (Some(index), _) => Some(index),
            (None, Some(_)) => Some(self.accounts.len()),
            (None, None) => None,
        }
    }
    fn switch_account(&mut self, index: usize) -> Result<(), String> {
        if index >= self.accounts().len() {
            return Err("No such account.".into());
        }
        self.active = Some(index);
        Ok(())
    }
    fn remove_account(&mut self, index: usize) -> Result<(), String> {
        if index < self.accounts.len() {
            self.accounts.remove(index);
        } else if index == self.accounts.len() && self.microsoft().is_some() {
            self.microsoft_since = None;
        } else {
            return Err("No such account.".into());
        }
        self.active = match self.active {
            Some(a) if a == index => None,
            Some(a) if a > index => Some(a - 1),
            other => other,
        };
        Ok(())
    }
    fn player_head(&self, account: &Account) -> Option<Head> {
        let (Account::Microsoft { name } | Account::Ely { name } | Account::Offline { name }) =
            account;
        // Hair over two rows, skin below, two eyes: a different hair colour per name.
        let hue = name.bytes().fold(0u8, |h, b| h.wrapping_mul(31).wrapping_add(b));
        let mut head = [0u8; 8 * 8 * 4];
        for (i, px) in head.chunks_exact_mut(4).enumerate() {
            let (x, y) = (i % 8, i / 8);
            let rgb = match (x, y) {
                (_, 0..=1) | (0 | 7, 2) => [hue / 2, hue / 3, 40],
                (1 | 6, 4) => [255, 255, 255],
                (2 | 5, 4) => [60, 40, 160],
                _ => [198, 144, 110],
            };
            px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        Some(head)
    }
    fn setup(&self) -> Setup {
        let Some(since) = self.setup_since else {
            return Setup::Needed;
        };
        let t = since.elapsed();
        let at = |start: u64, length: u64, total: u64| {
            let into = t.saturating_sub(Duration::from_secs(start)).as_millis() as u64;
            (into.min(length * 1000) * total) / (length * 1000)
        };
        const DOWNLOAD: u64 = 672_000_000;
        let (stage, done, total, bytes_per_second) = match t.as_secs() {
            0..2 => (Stage::Check, at(0, 2, 5215), 5215, 0),
            2..12 => (
                Stage::Download,
                at(2, 10, DOWNLOAD),
                DOWNLOAD,
                DOWNLOAD / 10,
            ),
            12..14 => (Stage::Verify, at(12, 2, 5215), 5215, 0),
            14..20 => (Stage::Prepare, at(14, 6, 19), 19, 0),
            _ => return Setup::Ready,
        };
        if self.runs == 1 && stage == Stage::Download && done * 10 >= DOWNLOAD * 4 {
            return Setup::Failed {
                stage,
                error: "Connection lost while downloading (pretend).".into(),
            };
        }
        Setup::Running(Progress {
            stage,
            done,
            total,
            bytes_per_second,
            note: None,
        })
    }
    fn start_setup(&mut self) {
        self.runs += 1;
        self.setup_since = Some(Instant::now());
    }
    fn cancel_setup(&mut self) {
        self.setup_since = None;
    }
    fn profiles(&self) -> Vec<Profile> {
        let vanilla = Profile {
            id: VANILLA.into(),
            name: "Vanilla".into(),
            description: "Minecraft as it comes, no mods.".into(),
            mods: 0,
            size: 672_000_000,
            worlds: 1,
            state: match self.setup() {
                Setup::Needed => ProfileState::Available,
                Setup::Running(progress) => ProfileState::Installing(progress),
                Setup::Failed { error, .. } => ProfileState::Failed(error),
                Setup::Ready => ProfileState::Installed,
            },
        };
        let custom = self.profiles.iter().map(|p| {
            let size: u64 = p.mods.iter().filter_map(|(id, _)| fake_mod(id)).map(|m| m.size).sum();
            let state = match p.installed_since.map(|t| t.elapsed()) {
                None => ProfileState::Available,
                Some(t) if t >= Duration::from_secs(3) => ProfileState::Installed,
                Some(t) => ProfileState::Installing(Progress {
                    stage: Stage::Download,
                    done: size * t.as_millis() as u64 / 3000,
                    total: size,
                    bytes_per_second: size / 3,
                    note: None,
                }),
            };
            Profile {
                id: p.id.clone(),
                name: p.name.clone(),
                description: format!("{} mods", p.mods.len()),
                mods: p.mods.len(),
                size,
                worlds: 0,
                state,
            }
        });
        std::iter::once(vanilla).chain(custom).collect()
    }
    fn install_profile(&mut self, id: &str) {
        if id == VANILLA {
            self.start_setup();
        } else if let Ok(profile) = self.custom(id) {
            profile.installed_since = Some(Instant::now());
        }
    }
    fn selected_profile(&self) -> String {
        self.selected.clone().unwrap_or_else(|| VANILLA.into())
    }
    fn select_profile(&mut self, id: &str) -> Result<(), String> {
        if id != VANILLA && !self.profiles.iter().any(|p| p.id == id) {
            return Err("No such profile.".into());
        }
        self.selected = Some(id.into());
        Ok(())
    }
    fn remove_profile(&mut self, id: &str) -> Result<(), String> {
        let index = self.profiles.iter().position(|p| p.id == id);
        let Some(index) = index.filter(|_| id != VANILLA) else {
            return Err("This profile can't be removed.".into());
        };
        self.profiles.remove(index);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        Ok(())
    }
    fn presets(&self) -> Vec<Profile> {
        FAKE_PRESETS
            .iter()
            .map(|&(id, name, description, mods)| Profile {
                id: id.into(),
                name: name.into(),
                description: description.into(),
                mods: mods.len(),
                size: mods.iter().filter_map(|m| fake_mod(m)).map(|m| m.size).sum(),
                worlds: 0,
                state: ProfileState::Available,
            })
            .collect()
    }
    fn create_profile(
        &mut self,
        name: &str,
        preset: Option<&str>,
        mods: &[String],
    ) -> Result<String, String> {
        if !valid_profile_name(name) {
            return Err("Name it with 1 to 32 characters.".into());
        }
        let mut chosen: Vec<String> = match preset {
            None => Vec::new(),
            Some(preset) => FAKE_PRESETS
                .iter()
                .find(|p| p.0 == preset)
                .ok_or("No such preset.")?
                .3
                .iter()
                .map(|m| m.to_string())
                .collect(),
        };
        for id in mods {
            fake_mod(id).ok_or("No such mod.")?;
            if !chosen.contains(id) {
                chosen.push(id.clone());
            }
        }
        let taken: Vec<String> = self.profiles.iter().map(|p| p.id.clone()).collect();
        let id = profile_id(name, &taken);
        self.profiles.push(FakeProfile {
            id: id.clone(),
            name: name.trim().into(),
            mods: chosen.into_iter().map(|m| (m, true)).collect(),
            installed_since: None,
        });
        Ok(id)
    }
    fn rename_profile(&mut self, id: &str, name: &str) -> Result<(), String> {
        if !valid_profile_name(name) {
            return Err("Name it with 1 to 32 characters.".into());
        }
        self.custom(id)?.name = name.trim().into();
        Ok(())
    }
    fn mods(&self, profile: &str) -> Vec<ProfileMod> {
        let Some(p) = self.profiles.iter().find(|p| p.id == profile) else {
            return Vec::new();
        };
        p.mods
            .iter()
            .filter_map(|(id, enabled)| Some(ProfileMod { info: fake_mod(id)?, enabled: *enabled }))
            .collect()
    }
    fn set_mod_enabled(&mut self, profile: &str, id: &str, enabled: bool) -> Result<(), String> {
        let profile = self.custom(profile)?;
        let entry = profile.mods.iter_mut().find(|(m, _)| m == id).ok_or("No such mod.")?;
        entry.1 = enabled;
        Ok(())
    }
    fn search_mods(&mut self, query: &str) {
        self.search = Some((query.into(), Instant::now()));
    }
    fn mod_search(&self) -> ModSearch {
        let Some((query, since)) = &self.search else {
            return ModSearch::Idle;
        };
        if since.elapsed() < Duration::from_secs(1) {
            return ModSearch::Searching;
        }
        let query = query.to_lowercase();
        ModSearch::Done(
            FAKE_REGISTRY
                .iter()
                .filter(|m| m.1.to_lowercase().contains(&query))
                .filter_map(|m| fake_mod(m.0))
                .collect(),
        )
    }
    fn add_mod(&mut self, profile: &str, id: &str) -> Result<(), String> {
        fake_mod(id).ok_or("No such mod.")?;
        let profile = self.custom(profile)?;
        if !profile.mods.iter().any(|(m, _)| m == id) {
            profile.mods.push((id.into(), true));
            profile.installed_since = Some(Instant::now());
        }
        Ok(())
    }
    fn remove_mod(&mut self, profile: &str, id: &str) -> Result<(), String> {
        let profile = self.custom(profile)?;
        let before = profile.mods.len();
        profile.mods.retain(|(m, _)| m != id);
        if profile.mods.len() == before {
            return Err("No such mod.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_plays_in_the_root_and_modpacks_in_their_own_folder() {
        let root = std::path::Path::new("game");
        assert_eq!(profile_dir(root, VANILLA), root);
        assert_eq!(profile_dir(root, "performance"), root.join("profiles").join("performance"));
    }

    #[test]
    fn profile_ids_are_safe_unique_folder_names() {
        let none: Vec<String> = Vec::new();
        assert_eq!(profile_id("My Pack!", &none), "my-pack");
        assert_eq!(profile_id("../../evil", &none), "evil");
        assert_eq!(profile_id("ünïcode", &none), "n-code");
        assert_eq!(profile_id("   ", &none), "profile");
        assert_eq!(profile_id("Vanilla", &none), "vanilla-2");
        let taken = vec!["my-pack".to_string()];
        assert_eq!(profile_id("my pack", &taken), "my-pack-2");
        assert!(valid_profile_name(" Survival "));
        assert!(!valid_profile_name("   "));
        assert!(!valid_profile_name(&"x".repeat(33)));
    }

    #[test]
    fn fake_accounts_profiles_and_mods() {
        let mut fake = Fake::default();
        fake.sign_in_offline("Steve").unwrap();
        fake.sign_in_offline("Alex").unwrap();
        assert_eq!(fake.accounts().len(), 2);
        assert_eq!(fake.account(), Some(Account::Offline { name: "Alex".into() }));
        fake.switch_account(0).unwrap();
        assert_eq!(fake.account(), Some(Account::Offline { name: "Steve".into() }));
        assert!(fake.player_head(&fake.account().unwrap()).is_some());
        fake.remove_account(0).unwrap();
        assert_eq!(fake.account(), None);

        assert_eq!(fake.profiles()[0].state, ProfileState::Available);
        let id = fake.create_profile("Survival", Some("performance"), &["zoom".into()]).unwrap();
        assert_eq!(id, "survival");
        assert_eq!(fake.mods(&id).len(), 3);
        fake.select_profile(&id).unwrap();
        fake.install_profile(&id);
        assert!(matches!(fake.profiles()[1].state, ProfileState::Installing(_)));
        fake.set_mod_enabled(&id, "zoom", false).unwrap();
        assert!(!fake.mods(&id).iter().find(|m| m.info.id == "zoom").unwrap().enabled);
        assert!(fake.add_mod(VANILLA, "zoom").is_err());
        fake.search_mods("map");
        assert_eq!(fake.mod_search(), ModSearch::Searching);
        fake.remove_profile(&id).unwrap();
        assert_eq!(fake.selected_profile(), VANILLA);
    }

    #[test]
    fn offline_names() {
        assert!(valid_offline_name("Steve_01"));
        assert!(valid_offline_name("a"));
        assert!(!valid_offline_name(""));
        assert!(!valid_offline_name("seventeen_chars__"));
        assert!(!valid_offline_name("has space"));
        assert!(!valid_offline_name("ünïcode"));
    }

    #[test]
    fn fake_setup_starts_needed_and_runs() {
        let mut fake = Fake::default();
        assert_eq!(fake.setup(), Setup::Needed);
        fake.start_setup();
        assert!(matches!(
            fake.setup(),
            Setup::Running(Progress {
                stage: Stage::Check,
                ..
            })
        ));
        assert!(fake.sign_in_offline("bad name").is_err());
        assert_eq!(
            fake.sign_in_offline("Steve"),
            Ok(Account::Offline {
                name: "Steve".into()
            })
        );
        assert_eq!(
            fake.account(),
            Some(Account::Offline {
                name: "Steve".into()
            })
        );
    }
}
mod account;
pub mod controls_art;
mod ely;
pub mod launch;
mod mojang;
mod real;
mod sources;

pub use real::Real;
