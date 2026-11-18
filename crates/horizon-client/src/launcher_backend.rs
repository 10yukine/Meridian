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
    Failed { stage: Stage, error: String },
    /// The prepared game files are in place: Play can start the game.
    Ready,
}

/// The launcher's backend. Implemented by Codex's real one, and by [`Fake`] meanwhile.
pub trait Backend: Send {
    fn account(&self) -> Option<Account>;
    /// Starts Microsoft sign-in; [`Self::sign_in`] follows it.
    fn sign_in_microsoft(&mut self);
    fn sign_in(&self) -> SignIn;
    /// Makes `name` the offline account, or says why it can't be (see [`valid_offline_name`]).
    fn sign_in_offline(&mut self, name: &str) -> Result<Account, String>;
    fn sign_out(&mut self);
    fn setup(&self) -> Setup;
    /// Starts setup, or retries it after a failure.
    fn start_setup(&mut self);
    fn cancel_setup(&mut self);
}

/// A player name Minecraft accepts: 1 to 16 of `A-Z`, `a-z`, `0-9` and `_`.
pub fn valid_offline_name(name: &str) -> bool {
    (1..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// A backend that only pretends, so the screens can be built on a desktop before the real one
/// lands. Sign-in shows a code for 5 s, then succeeds. Setup runs through the four stages in about
/// 20 s; the first run fails at 40 % of the download, so the failure screen can be seen, and a
/// retry then succeeds.
// ponytail: delete once the real backend is wired in.
#[derive(Default)]
pub struct Fake {
    account: Option<Account>,
    microsoft_since: Option<Instant>,
    setup_since: Option<Instant>,
    runs: u32,
}

impl Backend for Fake {
    fn account(&self) -> Option<Account> {
        match self.sign_in() {
            SignIn::Done(account) => Some(account),
            _ => self.account.clone(),
        }
    }
    fn sign_in_microsoft(&mut self) {
        self.microsoft_since = Some(Instant::now());
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
            _ => SignIn::Done(Account::Microsoft {
                name: "Player".into(),
            }),
        }
    }
    fn sign_in_offline(&mut self, name: &str) -> Result<Account, String> {
        if !valid_offline_name(name) {
            return Err("Use 1 to 16 letters, digits or underscores.".into());
        }
        let account = Account::Offline { name: name.into() };
        self.account = Some(account.clone());
        Ok(account)
    }
    fn sign_out(&mut self) {
        self.account = None;
        self.microsoft_since = None;
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
            2..12 => (Stage::Download, at(2, 10, DOWNLOAD), DOWNLOAD, DOWNLOAD / 10),
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(matches!(fake.setup(), Setup::Running(Progress { stage: Stage::Check, .. })));
        assert!(fake.sign_in_offline("bad name").is_err());
        assert_eq!(
            fake.sign_in_offline("Steve"),
            Ok(Account::Offline { name: "Steve".into() })
        );
        assert_eq!(fake.account(), Some(Account::Offline { name: "Steve".into() }));
    }
}
