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

/// Removes what only preparing used and the next preparation rebuilds without a download: the
/// data generator's output (its reports sit beside the resources) and, for every Minecraft
/// version, the preparer's copies of the jars and libraries (imported again from the download
/// cache). About 340 MB for 26.3. Worlds, accounts, settings and the prepared resources stay.
pub fn drop_prepare_scratch(root: &Path) {
    let local = root.join(".local");
    let mut scratch = vec![local.join("datagen")];
    // What a [`free_install_cache`] cut short (the app closed) left to delete.
    scratch.extend(install_cache(root).map(|dir| dir.with_extension(FREEING)));
    if let Ok(versions) = fs::read_dir(local.join("minecraft")) {
        for version in versions.flatten() {
            scratch.extend(["jars", "libraries"].map(|dir| version.path().join(dir)));
        }
    }
    for dir in scratch {
        if let Err(e) = fs::remove_dir_all(&dir)
            && e.kind() != io::ErrorKind::NotFound
        {
            eprintln!("Meridian: {} not removed: {e}", dir.display());
        }
    }
}

const FREEING: &str = "freeing";

/// What only installing the game uses and playing never reads: Mojang's downloads (every
/// version's) and the Java that prepares them. Installing or repairing the game downloads them
/// again; the prepared resources, worlds, profiles and settings are elsewhere.
fn install_cache(root: &Path) -> [PathBuf; 2] {
    let local = root.join(".local");
    [local.join("launcher-downloads"), local.join("java25")]
}

/// The bytes [`free_install_cache`] would free. Walks thousands of files: call off the UI thread.
pub fn install_cache_size(root: &Path) -> u64 {
    install_cache(root)
        .iter()
        .map(|dir| crate::real::profiles::folder_bytes(dir))
        .sum()
}

/// Frees the install cache: moves it aside at once and deletes it on a worker, so the screen
/// doesn't wait. Call only while nothing installs. A deletion cut short finishes at the next
/// [`drop_prepare_scratch`].
pub fn free_install_cache(root: &Path) -> io::Result<()> {
    let mut aside = Vec::new();
    for dir in install_cache(root) {
        let to = dir.with_extension(FREEING);
        if let Err(e) = fs::remove_dir_all(&to)
            && e.kind() != io::ErrorKind::NotFound
        {
            return Err(e);
        }
        match fs::rename(&dir, &to) {
            Ok(()) => aside.push(to),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    std::thread::spawn(move || {
        for dir in aside {
            if let Err(e) = fs::remove_dir_all(&dir) {
                eprintln!("Meridian: {} not removed: {e}", dir.display());
            }
        }
    });
    Ok(())
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

/// The skins offline accounts chose on Skins: `<name>.classic.png` or `<name>.slim.png`.
const SKINS: &str = "launcher-skins";

/// The skin the offline account `name` chose, and whether its arms are slim: what the game wears
/// for that name (`HORIZON_OFFLINE_SKIN`, `horizon_client::skins::offline_skin`).
pub fn offline_skin(root: &Path, name: &str) -> Option<(PathBuf, bool)> {
    if !valid_offline_name(name) {
        return None;
    }
    [(false, "classic"), (true, "slim")]
        .into_iter()
        .map(|(slim, arms)| (root.join(SKINS).join(format!("{name}.{arms}.png")), slim))
        .find(|(path, _)| path.is_file())
}

/// Keeps `png` (checked: a 64x64 or 64x32 PNG) as the offline account `name`'s skin, in place of
/// the one it had.
pub(crate) fn save_offline_skin(root: &Path, name: &str, png: &[u8], slim: bool) -> io::Result<()> {
    if !valid_offline_name(name) {
        return Err(io::Error::other("not an offline name"));
    }
    let dir = root.join(SKINS);
    fs::create_dir_all(&dir)?;
    let (arms, other) = if slim {
        ("slim", "classic")
    } else {
        ("classic", "slim")
    };
    // Written beside and renamed: a game starting now never reads half a skin.
    let part = dir.join(format!("{name}.part"));
    fs::write(&part, png)?;
    fs::rename(&part, dir.join(format!("{name}.{arms}.png")))?;
    remove(&dir.join(format!("{name}.{other}.png")))
}

const RUNNING: &str = "launch-running";

/// The game is about to run: a marker only its normal end removes, so the launcher can tell
/// next time that it closed unexpectedly (a crash, or Android ending it in the background).
pub fn game_started(root: &Path) -> io::Result<()> {
    fs::write(root.join(RUNNING), b"")
}

/// The game ended normally.
pub fn game_ended(root: &Path) {
    let _ = remove(&root.join(RUNNING));
}

/// Whether the last game closed unexpectedly; asking forgets it.
pub fn closed_unexpectedly(root: &Path) -> bool {
    fs::remove_file(root.join(RUNNING)).is_ok()
}

/// The settings a profile's game takes from Vanilla's when it has none of its own yet, so a new
/// profile starts the way the player plays instead of with the first-run defaults.
const SETTINGS: [&str; 3] = ["options.txt", "horizon-graphics.txt", "touch-layout.json"];

/// Copies Vanilla's settings (`root`) into the profile's folder `profile`, leaving the ones it
/// already has.
pub fn inherit_settings(root: &Path, profile: &Path) -> io::Result<()> {
    if profile == root {
        return Ok(());
    }
    for name in SETTINGS {
        let (from, to) = (root.join(name), profile.join(name));
        if from.is_file() && !to.exists() {
            fs::copy(from, to)?;
        }
    }
    Ok(())
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

    #[test]
    fn an_offline_name_keeps_one_skin_with_its_arms() {
        let root = scratch();
        assert_eq!(offline_skin(&root, "Steve"), None);
        save_offline_skin(&root, "Steve", b"wide", false).unwrap();
        let (path, slim) = offline_skin(&root, "Steve").unwrap();
        assert!(!slim);
        assert_eq!(fs::read(path).unwrap(), b"wide");
        // A new choice replaces the old one, arms included.
        save_offline_skin(&root, "Steve", b"thin", true).unwrap();
        let (path, slim) = offline_skin(&root, "Steve").unwrap();
        assert!(slim);
        assert_eq!(fs::read(path).unwrap(), b"thin");
        assert_eq!(fs::read_dir(root.join(SKINS)).unwrap().count(), 1);
        // Each name its own; a name that isn't one is neither written nor read.
        assert_eq!(offline_skin(&root, "Alex"), None);
        assert!(save_offline_skin(&root, "../x", b"x", false).is_err());
        assert_eq!(offline_skin(&root, "../x"), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepare_scratch_goes_and_everything_the_player_needs_stays() {
        let root = scratch();
        let file = |path: &str| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        };
        let gone = [
            ".local/datagen/out/reports/items.json",
            ".local/minecraft/26.3/jars/client.jar",
            ".local/minecraft/26.3/libraries/a/b.jar",
            ".local/minecraft/26.4/jars/server.jar",
        ];
        let kept = [
            ".local/minecraft/26.3/prepared.json",
            ".local/horizon-client/26.3/horizon-resources.json",
            ".local/launcher-downloads/versions/26.3/26.3.jar",
            ".local/java25/lib/modules",
            "saves/New World/level.dat",
            "profiles/modded/saves/World/level.dat",
            "launcher-account.json",
            "options.txt",
        ];
        gone.iter().chain(&kept).for_each(|path| file(path));
        drop_prepare_scratch(&root);
        drop_prepare_scratch(&root); // nothing left to remove: no error
        for path in gone {
            assert!(!root.join(path).exists(), "{path}");
        }
        for path in kept {
            assert!(root.join(path).is_file(), "{path}");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn freeing_space_takes_the_install_cache_and_nothing_the_game_plays_with() {
        let root = scratch();
        let file = |path: &str, bytes: usize| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, vec![0; bytes]).unwrap();
        };
        let cache = [
            ".local/launcher-downloads/assets/objects/ab/abcd",
            ".local/launcher-downloads/versions/26.3/26.3.jar",
            ".local/java25/lib/modules",
        ];
        let kept = [
            ".local/horizon-client/26.3/horizon-resources.json",
            ".local/horizon-client/26.3/horizon-data.bin",
            ".local/minecraft/26.3/prepared.json",
            "saves/New World/level.dat",
            "profiles/modded/mods/a.horizon",
            "launcher-account.json",
            "options.txt",
        ];
        cache.iter().for_each(|path| file(path, 1000));
        kept.iter().for_each(|path| file(path, 7));
        assert_eq!(install_cache_size(&root), 3000);

        free_install_cache(&root).unwrap();
        assert_eq!(install_cache_size(&root), 0);
        for path in cache {
            assert!(!root.join(path).exists(), "{path}");
        }
        for path in kept {
            assert!(root.join(path).is_file(), "{path}");
        }
        // The worker deletes what was moved aside.
        let aside_gone = || {
            install_cache(&root)
                .iter()
                .all(|dir| !dir.with_extension(FREEING).exists())
        };
        for _ in 0..500 {
            if aside_gone() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(aside_gone());
        // Freeing again finds nothing; a deletion cut short goes at the next scratch sweep.
        free_install_cache(&root).unwrap();
        file(".local/java25.freeing/lib/modules", 1);
        drop_prepare_scratch(&root);
        assert!(!root.join(".local/java25.freeing").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_new_profile_takes_vanillas_settings_and_keeps_its_own() {
        let root = scratch();
        let profile = root.join("profiles/pack");
        fs::create_dir_all(&profile).unwrap();
        fs::write(root.join("options.txt"), "guiScale:3").unwrap();
        fs::write(root.join("touch-layout.json"), "vanilla").unwrap();
        fs::write(profile.join("touch-layout.json"), "own").unwrap();
        inherit_settings(&root, &profile).unwrap();
        assert_eq!(fs::read_to_string(profile.join("options.txt")).unwrap(), "guiScale:3");
        assert_eq!(fs::read_to_string(profile.join("touch-layout.json")).unwrap(), "own");
        assert!(!profile.join("horizon-graphics.txt").exists());

        assert!(!closed_unexpectedly(&root));
        game_started(&root).unwrap();
        game_ended(&root);
        assert!(!closed_unexpectedly(&root));
        game_started(&root).unwrap();
        assert!(closed_unexpectedly(&root));
        assert!(!closed_unexpectedly(&root));
        fs::remove_dir_all(root).unwrap();
    }
}
