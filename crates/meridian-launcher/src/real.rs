use super::*;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    },
};

#[cfg(all(target_os = "android", target_arch = "aarch64"))]
mod android;
mod remembered;
use remembered::Remembered;

const LOCK: &str = include_str!("../../../research-lock.json");
const EXPORTER_SOURCES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/exporters.rs"));
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;

enum Session {
    Microsoft(account::Account, account::Secrets),
    Ely(ely::Session),
}

impl Session {
    fn account(&self) -> Account {
        match self {
            Self::Microsoft(profile, _) => Account::Microsoft {
                name: profile.name.clone(),
            },
            Self::Ely(session) => Account::Ely {
                name: session.name.clone(),
            },
        }
    }

    fn remembered(&self) -> Remembered {
        match self {
            Self::Microsoft(_, secrets) => Remembered::Microsoft {
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

struct Identity {
    account: Option<Account>,
    sign_in: SignIn,
    session: Option<Session>,
}
impl Default for Identity {
    fn default() -> Self {
        Self {
            account: None,
            sign_in: SignIn::Idle,
            session: None,
        }
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
}

impl Real {
    pub fn new(root: PathBuf) -> Self {
        let account_file = cfg!(target_os = "android").then(|| root.join("launcher-account.json"));
        Self::with_store(root, account_file)
    }

    fn with_store(root: PathBuf, account_file: Option<PathBuf>) -> Self {
        let mut backend = Self {
            root,
            account_file,
            identity: Arc::default(),
            sign_in_cancel: Arc::default(),
            setup: Arc::new(Mutex::new(Setup::Needed)),
            setup_cancel: Arc::default(),
        };
        let Some(path) = backend.account_file.as_deref() else {
            return backend;
        };
        match Remembered::load(path) {
            Ok(Some(Remembered::Offline { name })) if valid_offline_name(&name) => {
                backend.identity.lock().unwrap().account = Some(Account::Offline { name });
            }
            Ok(Some(Remembered::Microsoft { refresh_token })) => {
                backend.start_sign_in(move |_, _| {
                    let client = account::client_id().ok_or(account::NOT_CONFIGURED)?;
                    let endpoints = account::Endpoints::live();
                    let ms = account::refresh(&endpoints, &client, &refresh_token)?;
                    let (profile, secrets) = account::finish_sign_in(&endpoints, ms)?;
                    Ok(Session::Microsoft(profile, secrets))
                });
            }
            Ok(Some(Remembered::Ely(session))) => {
                backend.start_sign_in(move |_, _| ely::restore(session).map(Session::Ely));
            }
            Ok(None) => {}
            Ok(Some(Remembered::Offline { .. })) | Err(_) => {
                backend.identity.lock().unwrap().sign_in =
                    SignIn::Failed("Could not restore your saved account. Sign in again.".into());
            }
        }
        backend
    }

    fn start_sign_in(
        &mut self,
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
                        if let Some(path) = file.as_deref()
                            && session.remembered().save(path).is_err()
                        {
                            state.sign_in = SignIn::Failed(remembered::SAVE_ERROR.into());
                            return;
                        }
                        let account = session.account();
                        state.sign_in = SignIn::Done(account.clone());
                        state.account = Some(account);
                        state.session = Some(session);
                    }
                    Err(error) => state.sign_in = SignIn::Failed(error),
                }
            });
        if let Err(e) = worker {
            self.identity.lock().unwrap().sign_in =
                SignIn::Failed(format!("Could not start sign-in: {e}"));
        }
    }

    fn cancel_sign_in(&mut self) {
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
        self.identity.lock().unwrap().account.clone()
    }

    fn sign_in_microsoft(&mut self) {
        self.start_sign_in(|identity, cancel| {
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
        self.start_sign_in(move |_, _| {
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
        if let Some(path) = self.account_file.as_deref() {
            Remembered::Offline { name: name.into() }
                .save(path)
                .map_err(|_| remembered::SAVE_ERROR)?;
        }
        let account = Account::Offline { name: name.into() };
        *state = Identity {
            account: Some(account.clone()),
            ..Identity::default()
        };
        Ok(account)
    }

    fn sign_out(&mut self) {
        self.cancel_sign_in();
        let mut state = self.identity.lock().unwrap();
        if let Some(path) = self.account_file.as_deref()
            && remembered::remove(path).is_err()
        {
            state.sign_in =
                SignIn::Failed("Could not remove the saved account. Try signing out again.".into());
            return;
        }
        *state = Identity::default();
    }

    fn game_session(&self) -> Result<Option<GameSession>, String> {
        self.identity
            .lock()
            .unwrap()
            .session
            .as_ref()
            .map(Session::game_session)
            .transpose()
    }

    fn setup(&self) -> Setup {
        self.setup.lock().unwrap().clone()
    }

    fn start_setup(&mut self) {
        if matches!(self.setup(), Setup::Running(_) | Setup::Ready) {
            return;
        }
        self.setup_cancel = Arc::default();
        let cancel = self.setup_cancel.clone();
        let setup = self.setup.clone();
        let root = self.root.clone();
        stage(&setup, Stage::Check, None);
        let worker = std::thread::Builder::new()
            .name("launcher-setup".into())
            .spawn(move || {
                let result = prepare(&root, &setup, &cancel);
                let mut state = setup.lock().unwrap();
                let current = match &*state {
                    Setup::Running(progress) => progress.stage,
                    _ => Stage::Check,
                };
                *state = match result {
                    Ok(()) => Setup::Ready,
                    Err(error) => Setup::Failed {
                        stage: current,
                        error,
                    },
                };
            });
        if let Err(e) = worker {
            *self.setup.lock().unwrap() = Setup::Failed {
                stage: Stage::Check,
                error: e.to_string(),
            };
        }
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
        if data.len() as u64 + got as u64 > MAX_DOWNLOAD {
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
    let started = Instant::now();
    let bytes = AtomicU64::new(0);
    let agent = ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(20))
        .timeout_connect(Duration::from_secs(10))
        .build();
    let report = |n| {
        let mut state = state.lock().unwrap();
        let done = bytes.fetch_add(n, Relaxed) + n;
        if let Setup::Running(progress) = &mut *state {
            progress.done = done;
            progress.bytes_per_second =
                (done as f64 / started.elapsed().as_secs_f64().max(0.1)) as u64;
        }
    };
    let download = |url: &str| fetch(&agent, url, cancel, report);
    #[cfg(all(target_os = "android", target_arch = "aarch64"))]
    let jdk = {
        stage(
            state,
            Stage::Download,
            Some("Downloading Android Java 25".into()),
        );
        let jdk = android::install(root, &agent, cancel, report)?;
        stage(
            state,
            Stage::Download,
            Some("Downloading Minecraft files".into()),
        );
        jdk
    };
    let game = root.join(".local/launcher-downloads");
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };
    mojang::download(
        &pins,
        &game,
        os,
        server_sha1,
        8,
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
    #[cfg(all(target_os = "android", target_arch = "aarch64"))]
    let made = horizon_prepare::prepare_with_java(&options, &log, &progress, jdk)?;
    #[cfg(not(all(target_os = "android", target_arch = "aarch64")))]
    let made = horizon_prepare::prepare_with_progress(&options, &log, &progress)?;
    cancelled(cancel)?;
    for complete in [
        horizon_prepare::sounds,
        horizon_prepare::packs,
        horizon_prepare::panorama,
        horizon_prepare::fonts,
        horizon_prepare::languages,
    ] {
        cancelled(cancel)?;
        complete(root, Some(&game), &|_| {})?;
    }
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
        second.sign_out();
        assert!(!path.exists());
        assert!(launcher().account().is_none());
        fs::remove_dir(dir).unwrap();
    }
}
