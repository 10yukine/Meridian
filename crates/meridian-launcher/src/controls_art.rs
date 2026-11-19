// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! The touch controls' Bedrock artwork (MCPE look), downloaded from Mojang's own `bedrock-samples`
//! at a pinned revision and checked file by file: never shipped in the APK or the repository.
//! The files land in the prepared resources' `textures/gui/bedrock`, where the game's atlas finds
//! them (`gui/bedrock/...` sprites); without them the controls keep Meridian's own look. Adapted
//! from Starling's `crates/horizon-client/src/android_control_assets.rs` and its
//! `packaging/android/bedrock-controls.json` at 5432370d5736.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path},
    time::Duration,
};

const MANIFEST: &str = include_str!("../assets/bedrock-controls.json");
/// Larger than any control texture: a file claiming more is not one of them.
const MAX_FILE: u64 = 1 << 20;

#[derive(serde::Deserialize)]
struct Manifest {
    revision: String,
    files: Vec<Entry>,
}

#[derive(serde::Deserialize)]
struct Entry {
    path: String,
    #[serde(default)]
    source: Option<String>,
    sha256: String,
    size: u64,
}

/// Where the artwork goes in the prepared resources folder `resources`.
fn folder(resources: &Path) -> std::path::PathBuf {
    resources.join("assets/minecraft/textures/gui/bedrock")
}

/// Downloads what is missing or doesn't check out; the number of files fetched.
pub fn fetch(resources: &Path) -> Result<usize, String> {
    let manifest = manifest()?;
    let target = folder(resources);
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(30))
        .build();
    let mut fetched = 0;
    for entry in &manifest.files {
        if verified(&target, entry) {
            continue;
        }
        let source = if entry.path == "LICENSE.md" {
            "LICENSE.md".to_owned()
        } else {
            format!(
                "resource_pack/textures/{}",
                entry.source.as_deref().ok_or("A control texture has no source.")?
            )
        };
        let url = format!(
            "https://raw.githubusercontent.com/Mojang/bedrock-samples/{}/{source}",
            manifest.revision
        );
        let mut bytes = Vec::new();
        agent
            .get(&url)
            .call()
            .map_err(|e| format!("{}: {e}", entry.path))?
            .into_reader()
            .take(entry.size + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}: {e}", entry.path))?;
        if !checks_out(entry, &bytes) {
            return Err(format!("{} did not match its checksum.", entry.path));
        }
        let file = target.join(&entry.path);
        fs::create_dir_all(file.parent().ok_or("A control texture has no folder.")?)
            .map_err(|e| e.to_string())?;
        fs::write(&file, &bytes).map_err(|e| e.to_string())?;
        fetched += 1;
    }
    drop_stale_data_file(resources, &manifest, fetched).map_err(|e| e.to_string())?;
    Ok(fetched)
}

/// The game reads `assets` from the prepared folder's `horizon-data.bin` when it has one, and
/// that holds only what was there when the game wrote it: a file added since reads as missing.
/// Remove it when it lacks this artwork (or holds older bytes), so the next start reads the
/// folder and writes it again with the artwork in.
fn drop_stale_data_file(resources: &Path, manifest: &Manifest, fetched: usize) -> io::Result<()> {
    use horizon_core::prepared_files;
    let Some(held) = prepared_files::files_under(&folder(resources)) else {
        return Ok(());
    };
    if fetched == 0 && manifest.files.iter().all(|e| held.contains(&e.path)) {
        return Ok(());
    }
    match fs::remove_file(resources.join(prepared_files::FILE_NAME)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The pinned list, with every path a plain relative one and every size in bounds.
fn manifest() -> Result<Manifest, String> {
    let manifest: Manifest = serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    let hex = |s: &str| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit());
    if !hex(&manifest.revision)
        || manifest.files.iter().any(|e| {
            e.size > MAX_FILE
                || e.path.is_empty()
                || !Path::new(&e.path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                // The license comes from the repository's root (`fetch`); the textures from
                // their own folder, by a plain name.
                || (e.path != "LICENSE.md" && e.source.as_deref().is_none_or(|s| s.contains("..")))
        })
    {
        return Err("The control texture list is invalid.".into());
    }
    Ok(manifest)
}

fn verified(target: &Path, entry: &Entry) -> bool {
    fs::read(target.join(&entry.path)).is_ok_and(|bytes| checks_out(entry, &bytes))
}

fn checks_out(entry: &Entry, bytes: &[u8]) -> bool {
    bytes.len() as u64 == entry.size && format!("{:x}", Sha256::digest(bytes)) == entry.sha256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pinned_list_is_valid_and_names_the_controls_drawn() {
        let manifest = manifest().unwrap();
        assert_eq!(manifest.revision, "46ba6ea985fb5a92d79a9419198f10dda14c199d");
        for name in [
            "controls/up.png",
            "controls/jump_dpad.png",
            "ui/attack_pressed.png",
            "ui/interact.png",
            "ui/joystick_frame.png",
            "ui/joystick_knob.png",
            "LICENSE.md",
        ] {
            assert!(manifest.files.iter().any(|e| e.path == name), "{name}");
        }
        // A file that doesn't check out is fetched again, never kept.
        let entry = &manifest.files[0];
        assert!(!checks_out(entry, b"not a texture"));
    }

    /// A prepared root with `horizon-data.bin` written over `art` (paths in the artwork folder).
    fn prepared(art: &[&str]) -> std::path::PathBuf {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!("meridian-art-{:016x}", u64::from_ne_bytes(nonce)));
        fs::create_dir_all(root.join("assets/minecraft/textures/gui")).unwrap();
        fs::write(root.join("horizon-resources.json"), "{}").unwrap();
        fs::write(root.join("assets/minecraft/textures/gui/icons.png"), "x").unwrap();
        for name in art {
            let file = folder(&root).join(name);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "x").unwrap();
        }
        horizon_core::prepared_files::write(&root).unwrap();
        root
    }

    #[test]
    fn a_data_file_without_the_artwork_is_dropped_and_one_with_it_kept() {
        let manifest = manifest().unwrap();
        let data = horizon_core::prepared_files::FILE_NAME;
        // Written before the download: the game would never see the artwork.
        let before = prepared(&[]);
        drop_stale_data_file(&before, &manifest, 0).unwrap();
        assert!(!before.join(data).exists());
        // Written with it: kept, unless this download just replaced some of it.
        let names: Vec<&str> = manifest.files.iter().map(|e| e.path.as_str()).collect();
        let after = prepared(&names);
        drop_stale_data_file(&after, &manifest, 0).unwrap();
        assert!(after.join(data).exists());
        drop_stale_data_file(&after, &manifest, 1).unwrap();
        assert!(!after.join(data).exists());
        let _ = fs::remove_dir_all(before);
        let _ = fs::remove_dir_all(after);
    }
}
