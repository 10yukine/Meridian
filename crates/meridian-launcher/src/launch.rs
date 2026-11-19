//! The hand-off between the launcher and the game (`docs/launcher-spec.md`, "Hand-off"): each
//! switch is a fresh process, which `launch.json` in the app's folder tells what to play.
use crate::{GameSession, VANILLA, valid_offline_name};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const FILE: &str = "launch.json";
const SESSION: &str = "launch-session.json";

/// What the launcher asks the next process to play.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Launch {
    /// The profile's id: its folder ([`crate::profile_dir`]) is the game folder.
    pub profile: String,
    /// The offline account's name; an online account's comes with its session.
    pub name: Option<String>,
    /// An online account: its session waits in [`session_file`].
    pub session: bool,
}

/// The online account's session for the game (`--session-file`): mode 0600, read once and
/// deleted by the game.
pub fn session_file(root: &Path) -> PathBuf {
    root.join(SESSION)
}

/// The prepared resources, which every profile's game shares.
pub fn resources_dir(root: &Path) -> PathBuf {
    horizon_prepare::resources_dir(root, "26.3", None)
}

/// Writes the hand-off for the next process: the session first, then the file that names it.
pub fn write(root: &Path, launch: &Launch, session: Option<&GameSession>) -> io::Result<()> {
    remove(&session_file(root))?;
    if let Some(session) = session {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        io::Write::write_all(
            &mut options.open(session_file(root))?,
            session.as_json().as_bytes(),
        )?;
    }
    fs::write(root.join(FILE), serde_json::to_vec(launch)?)
}

/// The launch the previous process asked for, deleted before it is read, so one the game can't
/// start with doesn't start it again and again. None (and a session left behind deleted) when
/// there is none or it doesn't hold up: the launcher shows.
pub fn take(root: &Path) -> Option<Launch> {
    let path = root.join(FILE);
    let launch = fs::read(&path)
        .ok()
        .and_then(|text| {
            fs::remove_file(&path).ok()?;
            serde_json::from_slice::<Launch>(&text).ok()
        })
        .filter(|launch| {
            (launch.profile == VANILLA || crate::real::profiles::safe_id(&launch.profile))
                && launch.name.as_deref().is_none_or(valid_offline_name)
        });
    if !launch.as_ref().is_some_and(|launch| launch.session) {
        let _ = remove(&session_file(root));
    }
    launch
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "meridian-launch-{:016x}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_launch_is_taken_once_and_a_bad_one_never() {
        let root = scratch();
        let offline = Launch {
            profile: VANILLA.into(),
            name: Some("Steve".into()),
            session: false,
        };
        write(&root, &offline, None).unwrap();
        assert_eq!(take(&root), Some(offline));
        assert_eq!(take(&root), None);

        let online = Launch {
            profile: "my-pack".into(),
            name: None,
            session: true,
        };
        write(&root, &online, Some(&GameSession(r#"{"name":"Alex"}"#.into()))).unwrap();
        assert_eq!(take(&root), Some(online));
        // The game reads (and deletes) the session; it's still there for it.
        assert!(session_file(&root).is_file());

        for profile in ["../saves", "Bad Name", ""] {
            let bad = Launch {
                profile: profile.into(),
                name: None,
                session: true,
            };
            write(&root, &bad, Some(&GameSession("{}".into()))).unwrap();
            assert_eq!(take(&root), None, "{profile}");
            assert!(!session_file(&root).exists(), "{profile}");
            assert!(!root.join(FILE).exists(), "{profile}");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
