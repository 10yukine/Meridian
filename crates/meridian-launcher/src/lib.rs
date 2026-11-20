//! Meridian's launcher contract: what the launcher's screens (`mobile_launcher`) ask for and the
//! backend (sign-in, downloads, preparing the game files) provides. The screens
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

/// A skin file the player chose: the PNG, its face, and the whole skin as 64x64 RGBA for the
/// figure. The player picks the arms, as Minecraft's own launchers have them do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedSkin {
    pub png: Vec<u8>,
    pub face: Head,
    pub skin: std::sync::Arc<[u8]>,
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

/// The launcher's backend, implemented by [`Real`].
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
    /// The account's whole skin (64x64 RGBA, once loaded) and whether its arms are slim.
    fn player_skin(&self, _account: &Account) -> Option<(std::sync::Arc<[u8]>, bool)> {
        None
    }
    /// The arguments the game starts with, as the player typed them.
    fn game_args(&self) -> String {
        String::new()
    }
    /// Keeps `text` as the game's arguments: words, "quoted words" as one.
    fn set_game_args(&mut self, _text: &str) -> Result<(), String> {
        Err("Game arguments are unavailable in this launcher.".into())
    }
    /// The last lines of `profile`'s game log (`logs/latest.log` in its folder), oldest first;
    /// none before its game has run.
    fn game_log(&self, _profile: &str) -> Vec<String> {
        Vec::new()
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
    fn offline_names() {
        assert!(valid_offline_name("Steve_01"));
        assert!(valid_offline_name("a"));
        assert!(!valid_offline_name(""));
        assert!(!valid_offline_name("seventeen_chars__"));
        assert!(!valid_offline_name("has space"));
        assert!(!valid_offline_name("ünïcode"));
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
