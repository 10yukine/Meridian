//! Profile folders and registry installs. File verification follows the desktop launcher's
//! browser/horizon/pack.rs, including the registry's exact dependency pins.
use super::*;
use horizon_mod_package::{
    index::{self, Index, Kind, Project, Requirement, Version},
    mod_list::{self, ModList},
    modpack::Modpack,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const INDEX: &str = "https://horizon-rust.github.io/horizon-mods/index.json";
const GAME: &str = "26.3";

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Entry {
    id: String,
    name: String,
    #[serde(default)]
    preset: Option<Requirement>,
    #[serde(default)]
    preset_applied: bool,
    installed: bool,
    mods: Vec<Choice>,
    #[serde(default)]
    installed_mods: BTreeMap<String, String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Choice {
    shown: ProfileMod,
    sha256: String,
    #[serde(default)]
    inherit: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Library {
    entries: Vec<Entry>,
    selected: Option<String>,
}

#[derive(Default)]
struct State {
    library: Library,
    installing: Option<String>,
    errors: BTreeMap<String, String>,
    usage: BTreeMap<String, (u64, usize)>,
    loading_error: Option<String>,
}

#[derive(Default)]
struct Catalog {
    query: String,
    index: Option<Index>,
    search: ModSearch,
}

#[derive(Clone)]
pub(super) struct Store {
    root: PathBuf,
    state: Arc<Mutex<State>>,
    catalog: Arc<Mutex<Catalog>>,
}

impl Store {
    pub(super) fn new(root: PathBuf) -> Self {
        let (library, loading_error) = match fs::read(root.join("launcher-profiles.json")) {
            Ok(data) => match serde_json::from_slice::<Library>(&data) {
                Ok(library) if valid_library(&library) => (library, None),
                _ => (
                    Library::default(),
                    Some("The saved profile list could not be read.".into()),
                ),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (Library::default(), None)
            }
            Err(error) => (Library::default(), Some(error.to_string())),
        };
        let store = Self {
            root,
            state: Arc::new(Mutex::new(State {
                library,
                loading_error,
                ..State::default()
            })),
            catalog: Arc::default(),
        };
        let ids = store
            .state
            .lock()
            .unwrap()
            .library
            .entries
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>();
        let reader = store.clone();
        let _ = std::thread::Builder::new()
            .name("launcher-profile-size".into())
            .spawn(move || {
                for id in std::iter::once(VANILLA.to_owned()).chain(ids) {
                    reader.refresh_usage(&id);
                }
            });
        store
    }

    fn save(&self, state: &mut State, library: Library) -> Result<(), String> {
        if let Some(error) = &state.loading_error {
            return Err(error.clone());
        }
        let bytes = serde_json::to_vec(&library).map_err(|e| e.to_string())?;
        sources::write_file(&self.root.join("launcher-profiles.json"), &bytes)?;
        state.library = library;
        Ok(())
    }

    fn refresh_usage(&self, id: &str) {
        let home = profile_dir(&self.root, id);
        let saves = home.join("saves");
        let worlds = fs::read_dir(&saves)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| {
                entry.file_type().is_ok_and(|ty| ty.is_dir())
                    && entry.path().join("level.dat").is_file()
            })
            .count();
        let size = if id == VANILLA {
            folder_bytes(&horizon_prepare::resources_dir(&self.root, GAME, None))
                + folder_bytes(&saves)
        } else {
            folder_bytes(&home)
        };
        let mut state = self.state.lock().unwrap();
        if id == VANILLA || state.library.entries.iter().any(|entry| entry.id == id) {
            state.usage.insert(id.into(), (size, worlds));
        }
    }

    pub(super) fn list(&self, base_ready: bool, setup: Setup) -> Vec<Profile> {
        let state = self.state.lock().unwrap();
        let view = |id: &str, installed: bool| {
            if state.installing.as_deref() == Some(id)
                && let Setup::Running(progress) = &setup
            {
                ProfileState::Installing(progress.clone())
            } else if let Some(error) = state.errors.get(id) {
                ProfileState::Failed(error.clone())
            } else if installed {
                ProfileState::Installed
            } else {
                ProfileState::Available
            }
        };
        let (size, worlds) = state.usage.get(VANILLA).copied().unwrap_or_default();
        let vanilla = Profile {
            id: VANILLA.into(),
            name: "Vanilla".into(),
            description: "Minecraft as it comes, no mods.".into(),
            mods: 0,
            size,
            worlds,
            state: view(VANILLA, base_ready),
        };
        std::iter::once(vanilla)
            .chain(state.library.entries.iter().map(|entry| {
                let (bytes, worlds) = state.usage.get(&entry.id).copied().unwrap_or_default();
                Profile {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    description: String::new(),
                    mods: entry.mods.len(),
                    worlds,
                    size: if entry.installed {
                        bytes
                    } else {
                        entry.mods.iter().map(|m| m.shown.info.size).sum()
                    },
                    state: view(&entry.id, base_ready && entry.installed),
                }
            }))
            .collect()
    }

    pub(super) fn selected(&self) -> String {
        self.state
            .lock()
            .unwrap()
            .library
            .selected
            .clone()
            .unwrap_or_else(|| VANILLA.into())
    }

    pub(super) fn select(&self, id: &str) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        let mut library = state.library.clone();
        if id != VANILLA && !library.entries.iter().any(|entry| entry.id == id) {
            return Err("No such profile.".into());
        }
        library.selected = (id != VANILLA).then(|| id.to_owned());
        self.save(&mut state, library)
    }

    pub(super) fn create(
        &self,
        name: &str,
        preset: Option<&str>,
        mods: &[String],
    ) -> Result<String, String> {
        if !valid_profile_name(name) {
            return Err("Use a name of 1 to 32 characters.".into());
        }
        let catalog = self.catalog.lock().unwrap();
        let mut choices = Vec::new();
        let mut seed = None;
        if preset.is_some() || !mods.is_empty() {
            let index = catalog
                .index
                .as_ref()
                .ok_or("Wait for the registry to load first.")?;
            if let Some(id) = preset {
                let pack = project(index, id, Kind::Modpack)?;
                let version = newest(index, pack).ok_or("No compatible preset version.")?;
                seed = Some(pin(pack, version));
                for required in &version.requires {
                    let (module, version) = resolve(index, required, Kind::Mod)?;
                    choices.push(choice(module, version, true));
                }
            }
            for id in mods {
                if choices.iter().any(|m| m.shown.info.id == *id) {
                    continue;
                }
                let module = project(index, id, Kind::Mod)?;
                let version = newest(index, module).ok_or("No compatible mod version.")?;
                choices.push(choice(module, version, false));
            }
        }
        drop(catalog);
        let mut state = self.state.lock().unwrap();
        let mut library = state.library.clone();
        let mut taken: Vec<_> = library.entries.iter().map(|p| p.id.clone()).collect();
        taken.extend(
            fs::read_dir(self.root.join("profiles"))
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok()),
        );
        let id = profile_id(name, &taken);
        library.entries.push(Entry {
            id: id.clone(),
            name: name.trim().into(),
            preset: seed,
            preset_applied: false,
            installed: false,
            mods: choices,
            installed_mods: BTreeMap::new(),
        });
        library.selected = Some(id.clone());
        self.save(&mut state, library)?;
        Ok(id)
    }

    fn edit(
        &self,
        id: &str,
        edit: impl FnOnce(&mut Entry) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if state.installing.as_deref() == Some(id) {
            return Err("Wait for this profile to finish installing.".into());
        }
        let mut library = state.library.clone();
        let entry = library
            .entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("No such editable profile.")?;
        let previous = entry.clone();
        edit(entry)?;
        let mut discarded = Vec::new();
        if entry.installed && previous.mods != entry.mods {
            let home = child_dir(&child_dir(&self.root, "profiles")?, id)?;
            let path = home.join(mod_list::FILE);
            let original = match fs::read(&path) {
                Ok(bytes) => Some(bytes),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.to_string()),
            };
            entry.installed_mods.retain(|id, hash| {
                let kept = entry.mods.iter().any(|m| m.shown.info.id == *id);
                if !kept {
                    discarded.push((id.clone(), hash.clone()));
                }
                kept
            });
            write_enabled(&home, &previous.installed_mods, &entry.mods)?;
            if let Err(error) = self.save(&mut state, library) {
                match original {
                    Some(bytes) => sources::write_file(&path, &bytes)?,
                    None => fs::remove_file(&path).map_err(|e| e.to_string())?,
                }
                return Err(error);
            }
        } else {
            self.save(&mut state, library)?;
        }
        state.errors.remove(id);
        drop(state);
        if !discarded.is_empty() {
            self.cleanup_removed(id, discarded);
        }
        Ok(())
    }

    fn cleanup_removed(&self, id: &str, discarded: Vec<(String, String)>) {
        let store = self.clone();
        let id = id.to_owned();
        let _ = std::thread::Builder::new()
            .name("launcher-mod-cleanup".into())
            .spawn(move || {
                let home = profile_dir(&store.root, &id);
                let mods = home.join(mod_list::DIR);
                for (module, hash) in discarded {
                    let path = mods.join(format!("{module}.hmod"));
                    if index::sha256_file(&path).as_ref() != Ok(&hash) {
                        continue;
                    }
                    let state = store.state.lock().unwrap();
                    let readded = state
                        .library
                        .entries
                        .iter()
                        .find(|p| p.id == id)
                        .is_none_or(|p| p.mods.iter().any(|m| m.shown.info.id == module));
                    if !readded
                        && state.installing.as_deref() != Some(&id)
                        && mods
                            .canonicalize()
                            .ok()
                            .and_then(|path| path.parent().map(Path::to_owned))
                            == home.canonicalize().ok()
                    {
                        let _ = fs::remove_file(path);
                    }
                }
                store.refresh_usage(&id);
            });
    }

    pub(super) fn rename(&self, id: &str, name: &str) -> Result<(), String> {
        if !valid_profile_name(name) {
            return Err("Use a name of 1 to 32 characters.".into());
        }
        self.edit(id, |entry| {
            entry.name = name.trim().into();
            Ok(())
        })
    }

    pub(super) fn remove(&self, id: &str) -> Result<(), String> {
        if !safe_id(id) {
            return Err("This profile can't be removed.".into());
        }
        let mut state = self.state.lock().unwrap();
        if state.installing.as_deref() == Some(id) {
            return Err("Wait for this profile to finish installing.".into());
        }
        let mut library = state.library.clone();
        let i = library
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .ok_or("No such profile.")?;
        library.entries.remove(i);
        if library.selected.as_deref() == Some(id) {
            library.selected = None;
        }
        let home = profile_dir(&self.root, id);
        let mut moved = None;
        match fs::symlink_metadata(&home) {
            Ok(meta) => {
                let canonical = home.canonicalize().map_err(|e| e.to_string())?;
                let parent = self
                    .root
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .join("profiles");
                if !meta.is_dir() || canonical.parent() != Some(parent.as_path()) {
                    return Err(
                        "The profile folder is outside Meridian's profiles directory.".into(),
                    );
                }
                let mut nonce = [0; 8];
                getrandom::fill(&mut nonce)
                    .map_err(|_| "Could not reserve a deleted profile folder.")?;
                let trash = parent.join(format!(".removed-{:016x}", u64::from_ne_bytes(nonce)));
                fs::rename(&canonical, &trash).map_err(|e| e.to_string())?;
                moved = Some(trash);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        if let Err(error) = self.save(&mut state, library) {
            if let Some(trash) = &moved {
                fs::rename(trash, &home)
                    .map_err(|e| format!("{error} Folder recovery failed: {e}"))?;
            }
            return Err(error);
        }
        state.usage.remove(id);
        state.errors.remove(id);
        drop(state);
        if let Some(trash) = moved {
            let _ = std::thread::Builder::new()
                .name("launcher-profile-cleanup".into())
                .spawn(move || {
                    let _ = fs::remove_dir_all(trash);
                });
        }
        Ok(())
    }

    pub(super) fn mods(&self, id: &str) -> Vec<ProfileMod> {
        self.state
            .lock()
            .unwrap()
            .library
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| {
                entry
                    .mods
                    .iter()
                    .map(|choice| choice.shown.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn enable(&self, profile: &str, id: &str, enabled: bool) -> Result<(), String> {
        self.edit(profile, |entry| {
            let choice = entry
                .mods
                .iter_mut()
                .find(|choice| choice.shown.info.id == id)
                .ok_or("No such mod in this profile.")?;
            choice.shown.enabled = enabled;
            choice.inherit = false;
            Ok(())
        })
    }

    pub(super) fn add(&self, profile: &str, id: &str) -> Result<(), String> {
        let choice = {
            let catalog = self.catalog.lock().unwrap();
            let index = catalog
                .index
                .as_ref()
                .ok_or("Wait for the registry to load first.")?;
            let p = project(index, id, Kind::Mod)?;
            choice(
                p,
                newest(index, p).ok_or("No compatible mod version.")?,
                false,
            )
        };
        self.edit(profile, |entry| {
            if !entry.mods.iter().any(|old| old.shown.info.id == id) {
                entry.mods.push(choice);
                entry.installed = false;
            }
            Ok(())
        })
    }

    pub(super) fn remove_mod(&self, profile: &str, id: &str) -> Result<(), String> {
        self.edit(profile, |entry| {
            let before = entry.mods.len();
            entry.mods.retain(|choice| choice.shown.info.id != id);
            if entry.mods.len() == before {
                return Err("No such mod in this profile.".into());
            }
            Ok(())
        })
    }

    pub(super) fn presets(&self) -> Vec<Profile> {
        let catalog = self.catalog.lock().unwrap();
        let Some(i) = &catalog.index else {
            let fetch = matches!(catalog.search, ModSearch::Idle);
            drop(catalog);
            if fetch {
                self.search("");
            }
            return Vec::new();
        };
        i.projects
            .iter()
            .filter_map(|p| {
                if p.kind != Kind::Modpack {
                    return None;
                }
                let v = newest(i, p)?;
                let required = v
                    .requires
                    .iter()
                    .map(|pin| resolve(i, pin, Kind::Mod).ok().map(|(_, v)| v.file.size))
                    .sum::<Option<u64>>()?;
                Some(Profile {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    description: p.summary.clone(),
                    mods: v.requires.len(),
                    size: required.saturating_add(v.file.size),
                    worlds: 0,
                    state: ProfileState::Available,
                })
            })
            .collect()
    }

    pub(super) fn search(&self, query: &str) {
        let mut state = self.catalog.lock().unwrap();
        let query = query.trim().to_lowercase();
        state.query = query;
        if let Some(index) = &state.index {
            state.search = ModSearch::Done(results(index, &state.query));
            return;
        }
        if matches!(state.search, ModSearch::Searching) {
            return;
        }
        state.search = ModSearch::Searching;
        let catalog = self.catalog.clone();
        let worker = std::thread::Builder::new()
            .name("launcher-mod-registry".into())
            .spawn(move || {
                let got = read_index(&agent(), &AtomicBool::new(false));
                let mut state = catalog.lock().unwrap();
                match got {
                    Ok(index) => {
                        state.search = ModSearch::Done(results(&index, &state.query));
                        state.index = Some(index);
                    }
                    Err(error) => state.search = ModSearch::Failed(error),
                }
            });
        if let Err(e) = worker {
            state.search = ModSearch::Failed(e.to_string());
        }
    }

    pub(super) fn mod_search(&self) -> ModSearch {
        self.catalog.lock().unwrap().search.clone()
    }

    pub(super) fn begin(&self, id: &str) -> Result<Option<Entry>, String> {
        let mut state = self.state.lock().unwrap();
        if state.installing.is_some() {
            return Err("Wait for setup to finish.".into());
        }
        let entry = if id == VANILLA {
            None
        } else {
            if let Some(error) = &state.loading_error {
                return Err(error.clone());
            }
            Some(
                state
                    .library
                    .entries
                    .iter()
                    .find(|p| p.id == id)
                    .cloned()
                    .ok_or("No such profile.")?,
            )
        };
        state.installing = Some(id.into());
        state.errors.remove(id);
        Ok(entry)
    }

    pub(super) fn finish(&self, id: &str, result: &Result<(), String>, base_ready: bool) {
        let mut state = self.state.lock().unwrap();
        state.installing = None;
        if let Err(error) = result {
            state.errors.insert(id.into(), error.clone());
        }
        drop(state);
        if base_ready {
            self.refresh_usage(VANILLA);
        }
        if id != VANILLA && result.is_ok() {
            self.refresh_usage(id);
        }
    }

    pub(super) fn install(
        &self,
        mut entry: Entry,
        setup: &Mutex<Setup>,
        cancel: &AtomicBool,
        registry: impl FnOnce(&ureq::Agent, &AtomicBool) -> Result<Index, String>,
    ) -> Result<(), String> {
        cancelled(cancel)?;
        let profiles = child_dir(&self.root, "profiles")?;
        let home = child_dir(&profiles, &entry.id)?;
        let downloads = child_dir(&home, ".meridian-downloads")?;
        let mods_dir = child_dir(&home, mod_list::DIR)?;
        let before = entry.installed_mods.clone();
        let mut installed = BTreeMap::new();
        if !entry.mods.is_empty() || entry.preset.is_some() {
            let agent = agent();
            stage(
                setup,
                Stage::Download,
                Some("Checking the mod registry".into()),
            );
            let index = registry(&agent, cancel)?;
            let pack = entry
                .preset
                .as_ref()
                .map(|pin| resolve(&index, pin, Kind::Modpack))
                .transpose()?;
            let mods: Vec<_> = entry
                .mods
                .iter()
                .map(|choice| {
                    resolve(
                        &index,
                        &Requirement {
                            id: choice.shown.info.id.clone(),
                            version: choice.shown.info.version.clone(),
                            sha256: choice.sha256.clone(),
                        },
                        Kind::Mod,
                    )
                })
                .collect::<Result<_, _>>()?;
            let pack = pack.filter(|_| !entry.preset_applied);
            let total = mods
                .iter()
                .map(|(_, v)| v.file.size)
                .sum::<u64>()
                .saturating_add(pack.map_or(0, |(_, v)| v.file.size));
            let mut done = 0u64;
            let started = Instant::now();
            let mut get = |p: &Project, v: &Version| {
                let bytes = checked_file(&agent, &index, p, v, &downloads, cancel, |current| {
                    *setup.lock().unwrap() = Setup::Running(Progress {
                        stage: Stage::Download,
                        done: (done + current).min(total),
                        total,
                        bytes_per_second: ((done + current) as f64
                            / started.elapsed().as_secs_f64().max(0.1))
                            as u64,
                        note: Some(format!("{} {}", p.name, v.version)),
                    });
                })?;
                done = done.saturating_add(v.file.size);
                Ok::<_, String>(bytes)
            };
            let pack_path = pack.map(|(p, v)| get(p, v)).transpose()?;
            let files = mods
                .iter()
                .map(|(p, v)| get(p, v))
                .collect::<Result<Vec<_>, _>>()?;
            cancelled(cancel)?;
            stage(
                setup,
                Stage::Prepare,
                Some("Installing profile mods".into()),
            );
            if let Some(path) = &pack_path {
                let pack = Modpack::read(path)?;
                for m in entry.mods.iter_mut().filter(|m| m.inherit) {
                    if let Some(pm) = pack
                        .manifest
                        .mods
                        .iter()
                        .find(|pm| pm.id == m.shown.info.id)
                    {
                        m.shown.enabled = pm.on;
                    }
                    m.inherit = false;
                }
                apply_overrides(path, &home, &downloads)?;
                entry.preset_applied = true;
            }
            for (choice, file) in entry.mods.iter().zip(files) {
                let id = &choice.shown.info.id;
                let target = mods_dir.join(format!("{id}.hmod"));
                if target.exists() {
                    let current = index::sha256_file(&target)?;
                    if current != choice.sha256 && before.get(id) != Some(&current) {
                        return Err(format!("{id} was installed outside this profile manager."));
                    }
                    if current == choice.sha256 {
                        installed.insert(id.clone(), current);
                        continue;
                    }
                }
                sources::write_file(&target, &fs::read(file).map_err(|e| e.to_string())?)?;
                installed.insert(id.clone(), choice.sha256.clone());
            }
        }
        cancelled(cancel)?;
        for (id, sha) in &before {
            if installed.contains_key(id) || !valid_mod_id(id) {
                continue;
            }
            let target = mods_dir.join(format!("{id}.hmod"));
            if index::sha256_file(&target).as_ref() == Ok(sha) {
                fs::remove_file(target).map_err(|e| e.to_string())?;
            }
        }
        write_enabled(&home, &before, &entry.mods)?;
        entry.installed_mods = installed;
        entry.installed = true;
        let mut state = self.state.lock().unwrap();
        let mut library = state.library.clone();
        let slot = library
            .entries
            .iter_mut()
            .find(|p| p.id == entry.id)
            .ok_or("Profile was removed.")?;
        *slot = entry;
        self.save(&mut state, library)
    }
}

fn valid_library(library: &Library) -> bool {
    let mut ids = std::collections::BTreeSet::new();
    library.entries.iter().all(|p| {
        safe_id(&p.id)
            && ids.insert(p.id.as_str())
            && valid_profile_name(&p.name)
            && p.mods.iter().all(|m| valid_mod_id(&m.shown.info.id))
            && p.installed_mods.keys().all(|id| valid_mod_id(id))
    }) && library
        .selected
        .as_deref()
        .is_none_or(|id| ids.contains(id))
}

fn valid_mod_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

fn child_dir(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let dir = parent.join(name);
    if fs::symlink_metadata(&dir).is_ok_and(|m| !m.file_type().is_dir()) {
        return Err("The profile folder is not a directory.".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file_type = fs::symlink_metadata(&dir)
        .map_err(|e| e.to_string())?
        .file_type();
    let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
    let parent = parent.canonicalize().map_err(|e| e.to_string())?;
    if !file_type.is_dir() || canonical.parent() != Some(parent.as_path()) {
        return Err("The profile's folder points outside its parent.".into());
    }
    Ok(canonical)
}

fn apply_overrides(path: &Path, home: &Path, downloads: &Path) -> Result<(), String> {
    let stage = child_dir(downloads, "overrides-stage")?;
    for written in Modpack::extract_overrides(path, &stage)? {
        let relative = written.strip_prefix(&stage).map_err(|e| e.to_string())?;
        let mut parent = home.to_path_buf();
        for component in relative.parent().into_iter().flat_map(Path::components) {
            let name = component
                .as_os_str()
                .to_str()
                .ok_or("Invalid override path.")?;
            parent = child_dir(&parent, name)?;
        }
        let target = parent.join(relative.file_name().ok_or("Missing override filename.")?);
        if !target.exists() {
            let data = fs::read(&written).map_err(|e| e.to_string())?;
            sources::write_file(&target, &data)?;
        }
    }
    Ok(())
}

fn write_enabled(
    home: &Path,
    previous: &BTreeMap<String, String>,
    choices: &[Choice],
) -> Result<(), String> {
    let mut enabled = ModList::load(home);
    for id in previous
        .keys()
        .chain(choices.iter().map(|choice| &choice.shown.info.id))
    {
        enabled.enabled.remove(id);
        enabled.disabled.remove(id);
    }
    enabled.enabled.extend(
        choices
            .iter()
            .filter(|m| m.shown.enabled)
            .map(|m| m.shown.info.id.clone()),
    );
    sources::write_file(&home.join(mod_list::FILE), enabled.serialize().as_bytes())
}

pub(crate) fn safe_id(id: &str) -> bool {
    id != VANILLA
        && (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn folder_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let ty = entry.file_type().ok()?;
            if ty.is_dir() {
                Some(folder_bytes(&entry.path()))
            } else if ty.is_file() {
                Some(entry.metadata().ok()?.len())
            } else {
                None
            }
        })
        .sum()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_read(Duration::from_secs(20))
        .timeout_connect(Duration::from_secs(10))
        .build()
}

pub(super) fn read_index(agent: &ureq::Agent, cancel: &AtomicBool) -> Result<Index, String> {
    let body = fetch(agent, INDEX, 16 << 20, cancel, |_| {})?;
    Index::parse(std::str::from_utf8(&body).map_err(|_| "Invalid registry text.")?)
}

fn newest<'a>(index: &Index, project: &'a Project) -> Option<&'a Version> {
    index.installable(project, GAME)
}

fn project<'a>(index: &'a Index, id: &str, kind: Kind) -> Result<&'a Project, String> {
    index
        .project(id)
        .filter(|p| p.kind == kind)
        .ok_or_else(|| format!("{id} is not in the Android-compatible registry."))
}

fn resolve<'a>(
    index: &'a Index,
    pin: &Requirement,
    kind: Kind,
) -> Result<(&'a Project, &'a Version), String> {
    let project = project(index, &pin.id, kind)?;
    let version = project
        .versions
        .iter()
        .find(|v| {
            v.version == pin.version
                && v.game == GAME
                && v.capabilities.unknown().is_empty()
                && v.file.sha256 == pin.sha256
                && index.revoked(&project.id, &v.file.sha256).is_none()
        })
        .ok_or_else(|| format!("{} {} is unavailable for this game.", pin.id, pin.version))?;
    Ok((project, version))
}

fn pin(p: &Project, v: &Version) -> Requirement {
    Requirement {
        id: p.id.clone(),
        version: v.version.clone(),
        sha256: v.file.sha256.clone(),
    }
}

fn info(p: &Project, v: &Version) -> ModInfo {
    ModInfo {
        id: p.id.clone(),
        name: p.name.clone(),
        description: p.summary.clone(),
        version: v.version.clone(),
        size: v.file.size,
    }
}

fn choice(p: &Project, v: &Version, inherit: bool) -> Choice {
    Choice {
        shown: ProfileMod {
            info: info(p, v),
            enabled: true,
        },
        sha256: v.file.sha256.clone(),
        inherit,
    }
}

fn results(index: &Index, query: &str) -> Vec<ModInfo> {
    index
        .projects
        .iter()
        .filter_map(|project| {
            if project.kind != Kind::Mod
                || ![&project.id, &project.name, &project.summary]
                    .iter()
                    .any(|s| s.to_lowercase().contains(query))
            {
                return None;
            }
            newest(index, project).map(|version| info(project, version))
        })
        .collect()
}

fn checked_file(
    agent: &ureq::Agent,
    registry: &Index,
    project: &Project,
    version: &Version,
    dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, String> {
    cancelled(cancel)?;
    let path = dir.join(&version.file.name);
    if index::check_file(registry, project, version, &path).is_ok() {
        progress(version.file.size);
        return Ok(path);
    }
    let mut received = 0;
    let bytes = fetch(agent, &version.file.url, version.file.size, cancel, |n| {
        received += n;
        progress(received);
    })?;
    let pending = path.with_extension("pending");
    sources::write_file(&pending, &bytes)?;
    if let Err(error) = index::check_file(registry, project, version, &pending) {
        let _ = fs::remove_file(&pending);
        return Err(error);
    }
    fs::rename(&pending, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "meridian-profiles-{:016x}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn profile_creation_install_rename_and_deletion_keep_vanilla_worlds() {
        let root = scratch();
        let vanilla = root.join("saves/Vanilla");
        fs::create_dir_all(&vanilla).unwrap();
        fs::write(vanilla.join("level.dat"), b"vanilla").unwrap();
        let store = Store::new(root.clone());
        let id = store.create("First project", None, &[]).unwrap();
        assert_eq!(id, "first-project");
        assert_eq!(store.selected(), id);
        assert!(store.select("../saves").is_err());
        assert!(store.remove(VANILLA).is_err());
        let entry = store.begin(&id).unwrap().unwrap();
        let setup = Mutex::new(Setup::Needed);
        let result = store.install(entry, &setup, &AtomicBool::new(false), read_index);
        assert!(result.is_ok(), "{result:?}");
        store.finish(&id, &result, true);
        let profile = profile_dir(&root, &id);
        let world = profile.join("saves/Isolated");
        fs::create_dir_all(&world).unwrap();
        fs::write(world.join("level.dat"), b"custom").unwrap();
        store.refresh_usage(&id);
        store.rename(&id, "Renamed").unwrap();
        let list = store.list(true, Setup::Ready);
        assert_eq!(list[0].worlds, 1);
        assert_eq!(list[1].worlds, 1);
        assert_eq!(list[1].name, "Renamed");
        assert_eq!(list[1].state, ProfileState::Installed);
        let restored = Store::new(root.clone());
        assert_eq!(restored.selected(), id);
        restored.remove(&id).unwrap();
        assert_eq!(restored.selected(), VANILLA);
        assert!(!profile.exists());
        assert_eq!(fs::read(vanilla.join("level.dat")).unwrap(), b"vanilla");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checked_pack_installs_toggles_and_removes_a_registry_mod() {
        let root = scratch();
        let catalog = Index::parse(include_str!(
            "../../../horizon-mod-package/tests/index/index-v1.json"
        ))
        .unwrap();
        let store = Store::new(root.clone());
        store.catalog.lock().unwrap().index = Some(catalog.clone());
        let id = store.create("With a pack", Some("cozy"), &[]).unwrap();
        let home = profile_dir(&root, &id);
        let cache = home.join(".meridian-downloads");
        fs::create_dir_all(&cache).unwrap();
        fs::write(
            cache.join("cozy-1.0.0.hpack"),
            include_bytes!("../../../horizon-mod-package/tests/index/cozy-1.0.0.hpack"),
        )
        .unwrap();
        fs::write(
            cache.join("coords_hud-0.1.0.hmod"),
            include_bytes!("../../../horizon-mod-package/tests/index/coords_hud-0.1.0.hmod"),
        )
        .unwrap();
        let entry = store.begin(&id).unwrap().unwrap();
        let setup = Mutex::new(Setup::Needed);
        let result = store.install(entry, &setup, &AtomicBool::new(false), |_, _| Ok(catalog));
        assert!(result.is_ok(), "{result:?}");
        store.finish(&id, &result, true);
        let module = home.join(mod_list::DIR).join("coords_hud.hmod");
        assert!(module.is_file());
        assert_eq!(store.mods(&id)[0].info.id, "coords_hud");
        store.enable(&id, "coords_hud", false).unwrap();
        assert!(!ModList::load(&home).enabled.contains("coords_hud"));
        store.enable(&id, "coords_hud", true).unwrap();
        assert!(ModList::load(&home).enabled.contains("coords_hud"));
        store.remove_mod(&id, "coords_hud").unwrap();
        assert!(!ModList::load(&home).enabled.contains("coords_hud"));
        assert!(store.mods(&id).is_empty());
        for _ in 0..50 {
            if !module.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!module.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn registry_filters_native_and_enforces_exact_dependency_hashes() {
        let hash = "a".repeat(64);
        let item = |id: &str, kind: &str| {
            serde_json::json!({
                "id": id, "kind": kind, "name": id, "summary": "test", "license": "MIT", "source": "https://example.test",
                "versions": [{
                    "version": "1.0.0", "api": "0.2", "game": "26.3", "side": "client",
                    "capabilities": ["registries"],
                    "file": {
                        "name": format!("{id}-1.0.0.hmod"), "url": format!("{}/{id}-1.0.0.hmod", index::FILE_HOST.trim_end_matches('/')),
                        "size": 42, "sha256": hash
                    }
                }]
            })
        };
        let mut catalog = Index::default();
        catalog.projects = vec![
            serde_json::from_value(item("contentmod", "mod")).unwrap(),
            serde_json::from_value(item("native_test", "native-mod")).unwrap(),
        ];
        assert_eq!(
            results(&catalog, "")
                .iter()
                .map(|info| info.id.as_str())
                .collect::<Vec<_>>(),
            ["contentmod"]
        );
        let p = project(&catalog, "contentmod", Kind::Mod).unwrap();
        let v = newest(&catalog, p).unwrap();
        let mut requirement = pin(p, v);
        assert!(resolve(&catalog, &requirement, Kind::Mod).is_ok());
        requirement.sha256 = "b".repeat(64);
        assert!(resolve(&catalog, &requirement, Kind::Mod).is_err());
    }
}
