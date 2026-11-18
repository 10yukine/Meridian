//! Adapted from launcher/src/mojang.rs for first-run resource preparation: the version JSON, the client
//! jar, this system's libraries and the assets (the sounds), each checked against its sha1,
//! in the official launcher's layout in the launcher's data folder. sources.rs then finds them
//! there like any launcher's. Nothing of Mojang's is part of Horizon.
//!
//! Each file is checked against the size and sha1 Mojang lists, and written whole. A download
//! stopped half way (cancelled, no network) keeps the files it finished: the next one fetches
//! only the rest.

use crate::sources::{Fetch, Pins, fetch_manifest, sha1_file, sha1_hex, write_file};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

const CANCELLED: &str = "Download cancelled. Retry to continue.";
const OBJECTS: &str = "https://resources.download.minecraft.net";

struct File {
    url: String,
    sha1: String,
    size: Option<u64>,
    path: PathBuf,
}

/// Downloads the pinned version into `dir`, `jobs` files at a time; files already there and
/// whole are kept. Required files and assets must all verify,
/// and `cancel` stops the download between files.
pub(crate) fn download(
    pins: &Pins,
    dir: &Path,
    os: &str,
    server_sha1: &str,
    jobs: usize,
    fetch: Fetch,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
) -> Result<(), String> {
    let v = &pins.version;
    let versions = dir.join("versions").join(v);
    let manifest = versions.join(format!("{v}.json"));
    fetch_manifest(pins, &manifest, fetch, log)?;
    let manifest = read_json(&manifest)?;
    let client = &manifest["downloads"]["client"];
    // A version Horizon doesn't pin takes its client's sha1 from its version file, itself
    // checked against the sha1 Mojang's version list gives (`Pins::for_version`).
    let client_sha1 = match pins.client_sha1.as_str() {
        "" => text(client, "sha1")?,
        pinned => pinned.to_string(),
    };
    let mut game = vec![File {
        url: text(client, "url")?,
        sha1: client_sha1,
        size: client["size"].as_u64(),
        path: versions.join(format!("{v}.jar")),
    }];
    let server = &manifest["downloads"]["server"];
    game.push(File {
        url: text(server, "url")?,
        sha1: server_sha1.to_owned(),
        size: server["size"].as_u64(),
        path: versions.join(format!("{v}-server.jar")),
    });
    let libraries = manifest["libraries"]
        .as_array()
        .ok_or("The version file has no libraries")?;
    for library in libraries.iter().filter(|l| allowed(l, os)) {
        // Before 1.19 a library's natives for this system are a classifier of its own.
        let native = native_classifier(library, os)
            .map(|key| &library["downloads"]["classifiers"][key.as_str()]);
        for artifact in [Some(&library["downloads"]["artifact"]), native]
            .into_iter()
            .flatten()
            .filter(|a| !a.is_null())
        {
            game.push(File {
                url: text(artifact, "url")?,
                sha1: text(artifact, "sha1")?,
                size: artifact["size"].as_u64(),
                path: dir
                    .join("libraries")
                    .join(relative(&text(artifact, "path")?)?),
            });
        }
    }
    let libraries = game.len() - 2;
    // The logging setup the game is started with (`logging.client.argument`): for 1.7 to 1.18,
    // Mojang's fixed one is what keeps Log4Shell out.
    let logging = &manifest["logging"]["client"]["file"];
    if !logging.is_null() {
        game.push(File {
            url: text(logging, "url")?,
            sha1: text(logging, "sha1")?,
            size: logging["size"].as_u64(),
            path: dir
                .join("assets/log_configs")
                .join(relative(&text(logging, "id")?)?),
        });
    }
    log(format!(
        "Downloading Minecraft {v} from Mojang: the game and {libraries} libraries"
    ));
    fetch_all(&game, jobs, fetch, cancel, log)?;
    let index = &manifest["assetIndex"];
    let index = File {
        url: text(index, "url")?,
        sha1: text(index, "sha1")?,
        size: index["size"].as_u64(),
        path: dir
            .join("assets/indexes")
            .join(relative(&format!("{}.json", text(index, "id")?))?),
    };
    let assets = fetch_all(std::slice::from_ref(&index), 1, fetch, cancel, log)
        .and_then(|_| objects(&read_json(&index.path)?, dir))
        .and_then(|objects| {
            log(format!("Downloading the sounds ({} files)", objects.len()));
            // A connection that fails now and then is tried again, and a file that still fails
            // does not stop the others: the next download fetches what is left.
            let retrying = |url: &str| {
                let mut got = fetch(url);
                for _ in 0..2 {
                    if got.is_ok() {
                        break;
                    }
                    got = fetch(url);
                }
                got
            };
            fetch_each(&objects, jobs, &retrying, cancel, log, false)
        });
    assets
}
fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn text(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("The version file has no {key} in {v}"))
}

/// A path from the version file, refused if it could leave the folder.
pub(crate) fn relative(path: &str) -> Result<PathBuf, String> {
    let safe = !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    safe.then(|| PathBuf::from(path))
        .ok_or_else(|| format!("Unsafe path {path:?} in the version file"))
}

/// Whether a library's rules let this system (`osx`, `windows`, `linux`) use it: the last
/// rule that matches decides.
fn allowed(library: &Value, os: &str) -> bool {
    let Some(rules) = library["rules"].as_array() else {
        return true;
    };
    rules
        .iter()
        .rfind(|rule| rule["os"]["name"].as_str().is_none_or(|name| name == os))
        .is_some_and(|rule| rule["action"] == "allow")
}

/// The classifier holding a library's natives for `os` (`natives-osx`), from its `natives` map;
/// `${arch}` is this system's word size, as old versions name Windows' natives.
pub(crate) fn native_classifier(library: &Value, os: &str) -> Option<String> {
    let arch = if cfg!(target_pointer_width = "64") {
        "64"
    } else {
        "32"
    };
    library["natives"][os]
        .as_str()
        .map(|key| key.replace("${arch}", arch))
}

/// The asset index's objects, once each.
fn objects(index: &Value, dir: &Path) -> Result<Vec<File>, String> {
    let objects = index["objects"]
        .as_object()
        .ok_or("The asset index has no objects")?;
    let sizes: std::collections::HashMap<&str, u64> = objects
        .values()
        .filter_map(|o| Some((o["hash"].as_str()?, o["size"].as_u64()?)))
        .collect();
    let mut hashes: Vec<&str> = objects
        .values()
        .filter_map(|o| o["hash"].as_str())
        .collect();
    hashes.sort_unstable();
    hashes.dedup();
    hashes
        .into_iter()
        .map(|hash| {
            if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("Bad asset hash {hash:?} in the asset index"));
            }
            let path = format!("{}/{hash}", &hash[..2]);
            Ok(File {
                url: format!("{OBJECTS}/{path}"),
                sha1: hash.to_string(),
                size: sizes.get(hash).copied(),
                path: dir.join("assets/objects").join(path),
            })
        })
        .collect()
}

/// One file from Mojang, refused unless it has the size and sha1 Mojang lists.
fn fetch_file(fetch: Fetch, file: &File) -> Result<Vec<u8>, String> {
    let body = fetch(&file.url)?;
    if let Some(size) = file.size.filter(|&size| size != body.len() as u64) {
        return Err(format!(
            "is {} bytes, not the {size} Mojang lists",
            body.len()
        ));
    }
    match sha1_hex(&body) {
        actual if actual == file.sha1 => Ok(body),
        actual => Err(format!("is not the one Horizon pins (sha1 {actual})")),
    }
}

/// Downloads the files not already there, `jobs` at a time, with a line every second; stops at
/// the first error, or between files once `cancel` is set.
fn fetch_all(
    files: &[File],
    jobs: usize,
    fetch: Fetch,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
) -> Result<(), String> {
    fetch_each(files, jobs, fetch, cancel, log, true)
}

/// `fetch_all`, which with `stop_on_error` false goes on past a file that fails and reports how
/// many did, with the first one's error, once the rest are done.
fn fetch_each(
    files: &[File],
    jobs: usize,
    fetch: Fetch,
    cancel: &AtomicBool,
    log: &dyn Fn(String),
    stop_on_error: bool,
) -> Result<(), String> {
    let errors = AtomicUsize::new(0);
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed = Mutex::new(None);

    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..jobs.clamp(1, files.len().max(1)))
            .map(|_| {
                scope.spawn(|| {
                    while let Some(file) = files.get(next.fetch_add(1, Relaxed)) {
                        if stop_on_error && failed.lock().unwrap().is_some() {
                            break;
                        }
                        if cancel.load(Relaxed) {
                            failed.lock().unwrap().get_or_insert(CANCELLED.to_string());
                            break;
                        }
                        if sha1_file(&file.path).as_ref() != Some(&file.sha1) {
                            let got = fetch_file(fetch, file)
                                .map_err(|e| format!("{} {e}", file.url))
                                .and_then(|body| write_file(&file.path, &body));
                            if let Err(e) = got {
                                errors.fetch_add(1, Relaxed);
                                failed.lock().unwrap().get_or_insert(e);
                            }
                        }
                        done.fetch_add(1, Relaxed);
                    }
                })
            })
            .collect();
        let mut waited = 0;
        while !workers.iter().all(|w| w.is_finished()) {
            std::thread::sleep(Duration::from_millis(100));
            waited += 1;
            if waited % 10 == 0 {
                log(format!("{} of {} files", done.load(Relaxed), files.len()));
            }
        }
    });

    match failed.into_inner().unwrap() {
        Some(e) if !stop_on_error && e != CANCELLED => Err(format!(
            "{} of {} files failed, the first: {e}",
            errors.load(Relaxed),
            files.len()
        )),
        Some(e) => Err(e),
        None => Ok(()),
    }
}
