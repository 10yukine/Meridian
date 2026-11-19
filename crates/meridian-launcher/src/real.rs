use super::*;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
};

#[cfg(all(target_os = "android", target_arch = "aarch64"))]
mod android;
mod remembered;
use remembered::{Remembered, Shelf};
mod heads;
mod profiles;

const LOCK: &str = include_str!("../../../research-lock.json");
const EXPORTER_SOURCES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/exporters.rs"));
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
const DOWNLOAD_JOBS: usize = 16;

enum Session {
    Microsoft(account::Account, account::Secrets),
    Ely(ely::Session),
}

impl Session {
    fn remembered(&self) -> Remembered {
        match self {
            Self::Microsoft(profile, secrets) => Remembered::Microsoft {
                profile: Some(profile.clone()),
                refresh_token: secrets.refresh_token().into(),
            },
            Self::Ely(session) => Remembered::Ely(session.clone()),
        }
    }

    fn game_session(&self) -> Result<GameSession, String> {
        match self {
            Self::Microsoft(profile, secrets) => account::game_session(profile, secrets),
            Self::Ely(session) => session.game_session(),
        }
        .map(GameSession)
    }
}

#[derive(Default)]
struct Identity {
    shelf: Shelf,
    sign_in: SignIn,
    session: Option<Session>,
}

impl Identity {
    fn commit(&mut self, shelf: Shelf, path: Option<&Path>) -> Result<(), String> {
        if let Some(path) = path {
            shelf.save(path).map_err(|_| remembered::SAVE_ERROR)?;
        }
        self.shelf = shelf;
        Ok(())
    }

    fn select(
        &mut self,
        item: Remembered,
        replace: Option<usize>,
        path: Option<&Path>,
    ) -> Result<Account, String> {
        let mut shelf = self.shelf.clone();
        let account = item.account();
        let slot = replace.or_else(|| shelf.entries.iter().position(|old| old.same_player(&item)));
        let index = match slot {
            Some(index) => {
                shelf.entries[index] = item;
                index
            }
            None => {
                shelf.entries.push(item);
                shelf.entries.len() - 1
            }
        };
        shelf.active = Some(index);
        self.commit(shelf, path)?;
        self.sign_in = SignIn::Done(account.clone());
        Ok(account)
    }
}

/// Launcher workers and their frame-safe snapshots. Pass the absolute app-private game directory.
pub struct Real {
    root: PathBuf,
    account_file: Option<PathBuf>,
    identity: Arc<Mutex<Identity>>,
    sign_in_cancel: Arc<AtomicBool>,
    setup: Arc<Mutex<Setup>>,
    setup_cancel: Arc<AtomicBool>,
    base_ready: Arc<AtomicBool>,
    heads: heads::Cache,
    profiles: profiles::Store,
}

impl Real {
    pub fn new(root: PathBuf) -> Self {
        let account_file = if cfg!(target_os = "android") {
            remembered::private_account_file(&root)
        } else {
            None
        };
        Self::with_store(root, account_file)
    }

    fn with_store(root: PathBuf, account_file: Option<PathBuf>) -> Self {
        let profiles = profiles::Store::new(root.clone());
        let resources = horizon_prepare::resources_dir(&root, "26.3", None);
        let ready =
            horizon_prepare::check(&root, &resources).is_ok_and(|missing| missing.is_empty());
        let mut backend = Self {
            root,
            account_file,
            identity: Arc::default(),
            sign_in_cancel: Arc::default(),
            setup: Arc::new(Mutex::new(if ready { Setup::Ready } else { Setup::Needed })),
            setup_cancel: Arc::default(),
            base_ready: Arc::new(AtomicBool::new(ready)),
            heads: heads::Cache::default(),
            profiles,
        };
        let Some(path) = backend.account_file.as_deref() else {
            return backend;
        };
        match Shelf::load(path) {
            Ok(shelf) => {
                backend.identity.lock().unwrap().shelf = shelf;
                backend.restore_active();
            }
            Err(_) => {
                backend.identity.lock().unwrap().sign_in =
                    SignIn::Failed("Could not restore your saved accounts. Sign in again.".into());
            }
        }
        backend
    }

    fn restore_active(&mut self) {
        let selected = {
            let state = self.identity.lock().unwrap();
            state
                .shelf
                .active
                .map(|i| (i, state.shelf.entries[i].clone()))
        };
        match selected {
            Some((i, Remembered::Microsoft { refresh_token, .. })) => {
                self.start_sign_in(Some(i), move |_, _| {
                    let client = account::client_id().ok_or(account::NOT_CONFIGURED)?;
                    let endpoints = account::Endpoints::live();
                    let ms = account::refresh(&endpoints, &client, &refresh_token)?;
                    let (profile, secrets) = account::finish_sign_in(&endpoints, ms)?;
                    Ok(Session::Microsoft(profile, secrets))
                });
            }
            Some((i, Remembered::Ely(session))) => {
                self.start_sign_in(Some(i), move |_, _| ely::restore(session).map(Session::Ely));
            }
            _ => {}
        }
    }

    fn start_sign_in(
        &mut self,
        replace: Option<usize>,
        run: impl FnOnce(&Mutex<Identity>, &AtomicBool) -> Result<Session, String> + Send + 'static,
    ) {
        self.cancel_sign_in();
        let cancel = Arc::new(AtomicBool::new(false));
        self.sign_in_cancel = cancel.clone();
        let identity = self.identity.clone();
        let file = self.account_file.clone();
        identity.lock().unwrap().sign_in = SignIn::Starting;
        let worker = std::thread::Builder::new()
            .name("launcher-sign-in".into())
            .spawn(move || {
                let result = run(&identity, &cancel);
                let mut state = identity.lock().unwrap();
                if cancel.load(Relaxed) {
                    return;
                }
                match result {
                    Ok(session) => {
                        match state.select(session.remembered(), replace, file.as_deref()) {
                            Ok(_) => state.session = Some(session),
                            Err(error) => state.sign_in = SignIn::Failed(error),
                        }
                    }
                    Err(error) => state.sign_in = SignIn::Failed(error),
                }
            });
        if let Err(e) = worker {
            self.identity.lock().unwrap().sign_in =
                SignIn::Failed(format!("Could not start sign-in: {e}"));
        }
    }

    fn cancel_sign_in(&self) {
        self.sign_in_cancel.store(true, Relaxed);
    }
}

impl Drop for Real {
    fn drop(&mut self) {
        self.cancel_sign_in();
        self.cancel_setup();
    }
}

impl Backend for Real {
    fn account(&self) -> Option<Account> {
        let state = self.identity.lock().unwrap();
        state.shelf.active.map(|i| state.shelf.entries[i].account())
    }

    fn sign_in_microsoft(&mut self) {
        self.start_sign_in(None, |identity, cancel| {
            let client = account::client_id().ok_or(account::NOT_CONFIGURED)?;
            let endpoints = account::Endpoints::live();
            let code = account::device_code(&endpoints, &client)?;
            {
                let mut state = identity.lock().unwrap();
                if cancel.load(Relaxed) {
                    return Err("Sign-in cancelled.".into());
                }
                state.sign_in = SignIn::Code {
                    url: code.verification_uri.clone(),
                    code: code.user_code.clone(),
                };
            }
            let token =
                account::wait_for_sign_in(&endpoints, &client, &code, &|| cancel.load(Relaxed))?;
            let (profile, secrets) = account::finish_sign_in(&endpoints, token)?;
            Ok(Session::Microsoft(profile, secrets))
        });
    }

    fn sign_in_ely(&mut self, username: &str, password: &str, otp: &str) -> Result<(), String> {
        let username = username.to_owned();
        let password = password.to_owned();
        let otp = otp.to_owned();
        self.start_sign_in(None, move |_, _| {
            ely::authenticate(&username, &password, &otp).map(Session::Ely)
        });
        Ok(())
    }
    fn sign_in(&self) -> SignIn {
        self.identity.lock().unwrap().sign_in.clone()
    }

    fn sign_in_offline(&mut self, name: &str) -> Result<Account, String> {
        if !valid_offline_name(name) {
            return Err("Use 1 to 16 letters, digits or underscores.".into());
        }
        self.cancel_sign_in();
        let mut state = self.identity.lock().unwrap();
        let account = state.select(
            Remembered::Offline { name: name.into() },
            None,
            self.account_file.as_deref(),
        )?;
        state.session = None;
        Ok(account)
    }

    fn accounts(&self) -> Vec<Account> {
        self.identity
            .lock()
            .unwrap()
            .shelf
            .entries
            .iter()
            .map(Remembered::account)
            .collect()
    }

    fn active_account(&self) -> Option<usize> {
        self.identity.lock().unwrap().shelf.active
    }

    fn switch_account(&mut self, index: usize) -> Result<(), String> {
        {
            let mut state = self.identity.lock().unwrap();
            if index >= state.shelf.entries.len() {
                return Err("No such account.".into());
            }
            let mut shelf = state.shelf.clone();
            shelf.active = Some(index);
            state.commit(shelf, self.account_file.as_deref())?;
            self.cancel_sign_in();
            state.session = None;
            state.sign_in = SignIn::Idle;
        }
        self.restore_active();
        Ok(())
    }

    fn remove_account(&mut self, index: usize) -> Result<(), String> {
        let restore = {
            let mut state = self.identity.lock().unwrap();
            if index >= state.shelf.entries.len() {
                return Err("No such account.".into());
            }
            let mut shelf = state.shelf.clone();
            let removed = shelf.entries.remove(index).account();
            let active_removed = shelf.active == Some(index);
            shelf.active = match shelf.active {
                Some(active) if active == index => (!shelf.entries.is_empty()).then_some(0),
                Some(active) if active > index => Some(active - 1),
                active => active,
            };
            state.commit(shelf, self.account_file.as_deref())?;
            self.heads.remove(&removed);
            // Cancel any index-based restoration, whose target shifted on removal.
            self.cancel_sign_in();
            if active_removed {
                state.session = None;
            }
            state.sign_in = SignIn::Idle;
            state.session.is_none()
        };
        if restore {
            self.restore_active();
        }
        Ok(())
    }

    fn player_head(&self, player: &Account) -> Option<Head> {
        let state = self.identity.lock().unwrap();
        let entry = state
            .shelf
            .entries
            .iter()
            .find(|item| item.account() == *player)?;
        self.heads.get(player, heads::url(entry)?)
    }

    fn sign_out(&mut self) {
        if let Some(index) = self.active_account()
            && let Err(error) = self.remove_account(index)
        {
            self.identity.lock().unwrap().sign_in = SignIn::Failed(error);
        } else if self.active_account().is_none() {
            self.cancel_sign_in();
            self.identity.lock().unwrap().sign_in = SignIn::Idle;
        }
    }

    fn game_session(&self) -> Result<Option<GameSession>, String> {
        let state = self.identity.lock().unwrap();
        match (state.session.as_ref(), state.shelf.active) {
            (Some(session), _) => session.game_session().map(Some),
            (None, Some(i)) if !matches!(state.shelf.entries[i], Remembered::Offline { .. }) => {
                Err(
                    "The online account is not ready. Wait for sign-in or choose another account."
                        .into(),
                )
            }
            _ => Ok(None),
        }
    }

    fn setup(&self) -> Setup {
        self.setup.lock().unwrap().clone()
    }

    fn profiles(&self) -> Vec<Profile> {
        self.profiles
            .list(self.base_ready.load(Relaxed), self.setup())
    }

    fn selected_profile(&self) -> String {
        self.profiles.selected()
    }
    fn select_profile(&mut self, id: &str) -> Result<(), String> {
        self.profiles.select(id)
    }
    fn remove_profile(&mut self, id: &str) -> Result<(), String> {
        self.profiles.remove(id)
    }
    fn presets(&self) -> Vec<Profile> {
        self.profiles.presets()
    }
    fn create_profile(
        &mut self,
        name: &str,
        preset: Option<&str>,
        mods: &[String],
    ) -> Result<String, String> {
        self.profiles.create(name, preset, mods)
    }
    fn rename_profile(&mut self, id: &str, name: &str) -> Result<(), String> {
        self.profiles.rename(id, name)
    }
    fn mods(&self, id: &str) -> Vec<ProfileMod> {
        self.profiles.mods(id)
    }
    fn set_mod_enabled(&mut self, profile: &str, id: &str, enabled: bool) -> Result<(), String> {
        self.profiles.enable(profile, id, enabled)
    }
    fn search_mods(&mut self, query: &str) {
        self.profiles.search(query);
    }
    fn mod_search(&self) -> ModSearch {
        self.profiles.mod_search()
    }
    fn add_mod(&mut self, profile: &str, id: &str) -> Result<(), String> {
        self.profiles.add(profile, id)
    }
    fn remove_mod(&mut self, profile: &str, id: &str) -> Result<(), String> {
        self.profiles.remove_mod(profile, id)
    }

    fn install_profile(&mut self, id: &str) {
        if matches!(self.setup(), Setup::Running(_))
            || (id == VANILLA && self.base_ready.load(Relaxed))
        {
            return;
        }
        let entry = match self.profiles.begin(id) {
            Ok(entry) => entry,
            Err(error) => {
                *self.setup.lock().unwrap() = Setup::Failed {
                    stage: Stage::Check,
                    error,
                };
                return;
            }
        };
        self.setup_cancel = Arc::default();
        let cancel = self.setup_cancel.clone();
        let setup = self.setup.clone();
        let root = self.root.clone();
        let profiles = self.profiles.clone();
        let id = id.to_owned();
        let task_id = id.clone();
        let base_ready = self.base_ready.clone();
        stage(&setup, Stage::Check, None);
        let worker = std::thread::Builder::new()
            .name("launcher-setup".into())
            .spawn(move || {
                let result: Result<(), String> = (|| {
                    if !base_ready.load(Relaxed) {
                        prepare(&root, &setup, &cancel)?;
                        base_ready.store(true, Relaxed);
                    }
                    if let Some(entry) = entry {
                        profiles.install(entry, &setup, &cancel, profiles::read_index)?;
                    }
                    Ok(())
                })();
                {
                    let mut state = setup.lock().unwrap();
                    let current = match &*state {
                        Setup::Running(progress) => progress.stage,
                        _ => Stage::Check,
                    };
                    *state = match &result {
                        Ok(()) => Setup::Ready,
                        Err(error) => Setup::Failed {
                            stage: current,
                            error: error.clone(),
                        },
                    };
                }
                profiles.finish(&task_id, &result, base_ready.load(Relaxed));
            });
        if let Err(error) = worker {
            let error = error.to_string();
            self.profiles.finish(&id, &Err(error.clone()), false);
            *self.setup.lock().unwrap() = Setup::Failed {
                stage: Stage::Check,
                error,
            };
        }
    }

    fn start_setup(&mut self) {
        self.install_profile(&self.selected_profile());
    }

    fn cancel_setup(&mut self) {
        self.setup_cancel.store(true, Relaxed);
    }
}

fn stage(state: &Mutex<Setup>, stage: Stage, note: Option<String>) {
    *state.lock().unwrap() = Setup::Running(Progress {
        stage,
        done: 0,
        total: 0,
        bytes_per_second: 0,
        note,
    });
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Relaxed) {
        Err("Setup cancelled. Retry to continue.".into())
    } else {
        Ok(())
    }
}

fn fetch(
    agent: &ureq::Agent,
    url: &str,
    max_bytes: u64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<Vec<u8>, String> {
    cancelled(cancel)?;
    if !url.starts_with("https://") {
        return Err("Setup requires HTTPS downloads.".into());
    }
    let mut reader = agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader();
    let mut data = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        cancelled(cancel)?;
        let got = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if got == 0 {
            return Ok(data);
        }
        if data.len() as u64 + got as u64 > max_bytes {
            return Err("Download exceeds the size limit.".into());
        }
        data.extend_from_slice(&chunk[..got]);
        progress(got as u64);
    }
}

fn prepare(root: &Path, state: &Mutex<Setup>, cancel: &AtomicBool) -> Result<(), String> {
    let lock: serde_json::Value = serde_json::from_str(LOCK).map_err(|e| e.to_string())?;
    let pins = sources::Pins::read(&lock).ok_or("Missing Minecraft version pins.")?;
    let server_sha1 = lock["minecraft"]["server_sha1"]
        .as_str()
        .ok_or("Missing server checksum.")?;
    let resources = horizon_prepare::resources_dir(root, &pins.version, None);
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    sources::write_file(&root.join("research-lock.json"), LOCK.as_bytes())?;
    if horizon_prepare::check(root, &resources).is_ok_and(|missing| missing.is_empty()) {
        return Ok(());
    }

    stage(
        state,
        Stage::Download,
        Some("Downloading Minecraft files".into()),
    );
    let agent = ureq::AgentBuilder::new()
        .max_idle_connections_per_host(DOWNLOAD_JOBS)
        .timeout_read(Duration::from_secs(20))
        .timeout_connect(Duration::from_secs(10))
        .build();
    let meter = || {
        let started = Instant::now();
        move |bytes| {
            if let Setup::Running(progress) = &mut *state.lock().unwrap() {
                progress.done += bytes;
                progress.bytes_per_second =
                    (progress.done as f64 / started.elapsed().as_secs_f64().max(0.1)) as u64;
            }
        }
    };
    #[cfg(all(target_os = "android", target_arch = "aarch64"))]
    let jdk = {
        stage(
            state,
            Stage::Download,
            Some("Downloading Android Java 25".into()),
        );
        let jdk = android::install(root, &agent, cancel, meter())?;
        stage(
            state,
            Stage::Download,
            Some("Downloading Minecraft files".into()),
        );
        Some(jdk)
    };
    #[cfg(not(all(target_os = "android", target_arch = "aarch64")))]
    let jdk = None;
    let game = root.join(".local/launcher-downloads");
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };
    let report = meter();
    let download = |url: &str| fetch(&agent, url, MAX_DOWNLOAD, cancel, report);
    let assets = mojang::download_game(
        &pins,
        &game,
        os,
        server_sha1,
        DOWNLOAD_JOBS,
        &download,
        cancel,
        &|note| {
            if let Setup::Running(progress) = &mut *state.lock().unwrap() {
                progress.note = Some(note);
            }
        },
    )?;
    cancelled(cancel)?;
    stage(
        state,
        Stage::Verify,
        Some("Checking downloaded files".into()),
    );
    let server = game
        .join("versions")
        .join(&pins.version)
        .join(format!("{}-server.jar", pins.version));
    if sources::sha1_file(&server).as_deref() != Some(server_sha1) {
        return Err("Minecraft server checksum mismatch.".into());
    }

    stage(
        state,
        Stage::Prepare,
        Some("Preparing game resources".into()),
    );
    for (relative, contents) in EXPORTER_SOURCES {
        let path = root.join(relative);
        if fs::read(&path).as_deref().ok() != Some(*contents) {
            sources::write_file(&path, contents)?;
        }
    }
    let options = horizon_prepare::Options {
        root: root.to_owned(),
        import: true,
        minecraft_dir: Some(game.clone()),
        server_jar: Some(server),
        java_home: None,
        inputs: Default::default(),
        output: None,
    };
    let log = |line: &str| {
        if let Setup::Running(progress) = &mut *state.lock().unwrap() {
            progress.note = Some(line.into());
        }
    };
    let progress = |fraction: f32| {
        if let Setup::Running(progress) = &mut *state.lock().unwrap() {
            progress.total = 100;
            progress.done = (fraction * 100.0) as u64;
        }
    };
    let made = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            // The main progress counter now belongs to preparation, so asset bytes don't
            // advance it. Each downloaded object is still verified and kept on cancellation.
            let fetch_asset = |url: &str| fetch(&agent, url, MAX_DOWNLOAD, cancel, |_| {});
            assets.download(DOWNLOAD_JOBS, &fetch_asset, cancel, &|_| {})
        });
        horizon_prepare::prepare_for_launcher(&options, &log, &progress, jdk, move || {
            worker
                .join()
                .map_err(|_| "The asset download stopped unexpectedly".to_owned())??;
            cancelled(cancel)
        })
    })?;
    cancelled(cancel)?;
    let missing = horizon_prepare::check(root, &made)?;
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing.join("; "))
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;

    #[test]
    fn selected_account_survives_relaunch_until_sign_out() {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "meridian-account-test-{}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("launcher-account.json");
        let launcher = || Real::with_store(dir.clone(), Some(path.clone()));

        let mut first = launcher();
        first.sign_in_offline("First").unwrap();
        let expected = first.sign_in_offline("Offline_2").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        }
        drop(first);

        let mut second = launcher();
        assert_eq!(second.account(), Some(expected));
        assert!(second.game_session().unwrap().is_none());
        assert_eq!(second.accounts().len(), 2);
        second.switch_account(0).unwrap();
        drop(second);
        let mut third = launcher();
        assert_eq!(
            third.account(),
            Some(Account::Offline {
                name: "First".into()
            })
        );
        third.sign_out();
        assert_eq!(third.accounts().len(), 1);
        third.sign_out();
        assert!(!path.exists());
        assert!(launcher().account().is_none());
        fs::remove_dir(dir).unwrap();
    }
}
