//! Safe ZIP import and export of worlds, resource packs, and shader packs for a profile.
use std::{
    fs::{self, File},
    io::{self, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Maximum total uncompressed bytes accepted when importing a world ZIP (512 MiB).
pub const MAX_UNPACKED_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum size for imported pack ZIPs (512 MiB).
pub const MAX_PACK_BYTES: u64 = 512 * 1024 * 1024;

fn slugify(name: &str, fallback: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(48).collect();
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug
    }
}

fn unique_child(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem}-{n}{ext}")))
        .find(|p| !p.exists())
        .expect("unbounded")
}

/// Validates a ZIP entry path and returns its clean relative components.
/// Rejects `..`, drive letters, absolute paths, and backslashes.
fn validate_zip_path(raw: &str) -> Result<Vec<String>, String> {
    if raw.is_empty() || raw.contains('\\') || raw.starts_with('/') {
        return Err(format!("Unsafe path in ZIP archive: {raw}"));
    }
    if raw.as_bytes().get(1) == Some(&b':') {
        return Err(format!("Unsafe drive path in ZIP archive: {raw}"));
    }
    let mut parts = Vec::new();
    for comp in Path::new(raw).components() {
        match comp {
            Component::Normal(seg) => {
                let s = seg
                    .to_str()
                    .ok_or_else(|| format!("Invalid UTF-8 path in ZIP archive: {raw}"))?;
                if s.contains(':') {
                    return Err(format!("Unsafe path in ZIP archive: {raw}"));
                }
                parts.push(s.to_owned());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("Unsafe path in ZIP archive: {raw}"));
            }
        }
    }
    Ok(parts)
}

fn copy_zip_atomically(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let parent = dest.parent().ok_or_else(|| "Invalid destination".to_owned())?;
    fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let part = dest.with_file_name(format!(
        "{}.part",
        dest.file_name().unwrap_or_default().to_string_lossy()
    ));
    let copy_res = (|| -> io::Result<()> {
        let mut src = File::open(zip_path)?;
        let mut dst = File::create(&part)?;
        io::copy(&mut src, &mut dst)?;
        dst.flush()?;
        Ok(())
    })();
    if let Err(e) = copy_res {
        let _ = fs::remove_file(&part);
        return Err(format!("{}: {e}", dest.display()));
    }
    fs::rename(&part, dest).map_err(|e| {
        let _ = fs::remove_file(&part);
        format!("{}: {e}", dest.display())
    })
}

/// Validates and extracts a world ZIP (`<= 512 MiB` unpacked) into `<profile_home>/saves/<folder>/`.
pub fn import_world(
    zip_path: &Path,
    profile_home: &Path,
    display_name: &str,
) -> Result<String, String> {
    let file = File::open(zip_path).map_err(|e| format!("Could not open world ZIP: {e}"))?;
    let mut archive =
        ZipArchive::new(BufReader::new(file)).map_err(|e| format!("Invalid world ZIP: {e}"))?;

    let mut total_unpacked: u64 = 0;
    let mut root_level_dat = false;
    let mut nested_top: Option<String> = None;

    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Could not read ZIP entry: {e}"))?;
        let name = entry
            .name()
            .map_err(|e| format!("Invalid ZIP entry name: {e}"))?;
        let parts = validate_zip_path(&name)?;
        if !entry.is_dir() {
            total_unpacked = total_unpacked
                .checked_add(entry.size())
                .ok_or_else(|| "World ZIP is too large to unpack.".to_owned())?;
            if total_unpacked > MAX_UNPACKED_BYTES {
                return Err("World ZIP unpacks to more than 512 MiB.".into());
            }
            if parts.len() == 1 && parts[0] == "level.dat" {
                root_level_dat = true;
            } else if parts.len() == 2 && parts[1] == "level.dat" && parts[0] != "__MACOSX" {
                if nested_top.is_none() {
                    nested_top = Some(parts[0].clone());
                } else if nested_top.as_deref() != Some(parts[0].as_str()) {
                    return Err("World ZIP contains multiple worlds.".into());
                }
            }
        }
    }

    let strip_top = if root_level_dat {
        None
    } else if let Some(top) = nested_top {
        Some(top)
    } else {
        return Err("That ZIP does not contain a Minecraft world (missing level.dat).".into());
    };

    let stem = display_name
        .strip_suffix(".zip")
        .or_else(|| display_name.strip_suffix(".ZIP"))
        .unwrap_or(display_name);
    let base_name = if !stem.trim().is_empty() && stem != "picked-import.bin" {
        stem
    } else {
        strip_top.as_deref().unwrap_or("world")
    };
    let saves_dir = profile_home.join("saves");
    fs::create_dir_all(&saves_dir).map_err(|e| format!("Could not create saves folder: {e}"))?;
    let target = unique_child(&saves_dir, &slugify(base_name, "world"), "");
    let folder_name = target
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut nonce = [0u8; 8];
    let _ = getrandom::fill(&mut nonce);
    let staging = saves_dir.join(format!(
        ".import-{:016x}",
        u64::from_ne_bytes(nonce)
    ));
    fs::create_dir_all(&staging).map_err(|e| format!("Could not stage world import: {e}"))?;

    let extract_res = (|| -> Result<(), String> {
        let mut written_bytes: u64 = 0;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("Could not read ZIP entry: {e}"))?;
            let name = entry
                .name()
                .map_err(|e| format!("Invalid ZIP entry name: {e}"))?
                .into_owned();
            let parts = validate_zip_path(&name)?;
            let rel_parts: &[String] = match &strip_top {
                Some(top) => {
                    if parts.first().map(String::as_str) != Some(top.as_str()) {
                        continue;
                    }
                    &parts[1..]
                }
                None => &parts,
            };
            if rel_parts.is_empty() {
                continue;
            }
            let mut out_path = staging.clone();
            for seg in rel_parts {
                out_path.push(seg);
            }
            if entry.is_dir() {
                fs::create_dir_all(&out_path)
                    .map_err(|e| format!("Could not create folder: {e}"))?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Could not create folder: {e}"))?;
                }
                let remaining = MAX_UNPACKED_BYTES.saturating_sub(written_bytes);
                let mut dst = File::create(&out_path)
                    .map_err(|e| format!("Could not write world file: {e}"))?;
                let mut limited = (&mut entry).take(remaining + 1);
                let copied = io::copy(&mut limited, &mut dst)
                    .map_err(|e| format!("Could not unpack world file: {e}"))?;
                written_bytes = written_bytes.saturating_add(copied);
                if written_bytes > MAX_UNPACKED_BYTES {
                    return Err("World ZIP unpacks to more than 512 MiB.".into());
                }
            }
        }
        if !staging.join("level.dat").is_file() {
            return Err("That ZIP does not contain a Minecraft world (missing level.dat).".into());
        }
        Ok(())
    })();

    if let Err(e) = extract_res {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    fs::rename(&staging, &target).map_err(|e| {
        let _ = fs::remove_dir_all(&staging);
        format!("Could not finalize imported world: {e}")
    })?;
    Ok(folder_name)
}

/// Validates a resource pack ZIP (`pack.mcmeta`) and copies it atomically into
/// `<profile_home>/resourcepacks/<unique_name>.zip`.
pub fn import_resource_pack(
    zip_path: &Path,
    profile_home: &Path,
    display_name: &str,
) -> Result<String, String> {
    let meta = fs::metadata(zip_path).map_err(|e| format!("Could not read ZIP: {e}"))?;
    if meta.len() > MAX_PACK_BYTES {
        return Err("Resource pack ZIP is larger than 512 MiB.".into());
    }
    let file = File::open(zip_path).map_err(|e| format!("Could not open resource pack: {e}"))?;
    let mut archive = ZipArchive::new(BufReader::new(file))
        .map_err(|e| format!("Invalid resource pack ZIP: {e}"))?;

    let mut has_pack_mcmeta = false;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Could not read ZIP entry: {e}"))?;
        let name = entry
            .name()
            .map_err(|e| format!("Invalid ZIP entry name: {e}"))?;
        let parts = validate_zip_path(&name)?;
        if !entry.is_dir() && parts.len() == 1 && parts[0] == "pack.mcmeta" {
            has_pack_mcmeta = true;
            break;
        }
    }
    if !has_pack_mcmeta {
        return Err("That ZIP is not a resource pack (missing pack.mcmeta).".into());
    }

    let stem = display_name
        .strip_suffix(".zip")
        .or_else(|| display_name.strip_suffix(".ZIP"))
        .unwrap_or(display_name);
    let dir = profile_home.join("resourcepacks");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create resourcepacks folder: {e}"))?;
    let dest = unique_child(&dir, &slugify(stem, "resource-pack"), ".zip");
    copy_zip_atomically(zip_path, &dest)?;
    Ok(dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned())
}

/// Validates a shader pack ZIP (`shaders/` or `horizon-shader.toml`) and copies it atomically
/// into `<profile_home>/shaderpacks/<unique_name>.zip`.
pub fn import_shader_pack(
    zip_path: &Path,
    profile_home: &Path,
    display_name: &str,
) -> Result<String, String> {
    let meta = fs::metadata(zip_path).map_err(|e| format!("Could not read ZIP: {e}"))?;
    if meta.len() > MAX_PACK_BYTES {
        return Err("Shader pack ZIP is larger than 512 MiB.".into());
    }
    let file = File::open(zip_path).map_err(|e| format!("Could not open shader pack: {e}"))?;
    let mut archive = ZipArchive::new(BufReader::new(file))
        .map_err(|e| format!("Invalid shader pack ZIP: {e}"))?;

    let mut valid_shader = false;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Could not read ZIP entry: {e}"))?;
        let name = entry
            .name()
            .map_err(|e| format!("Invalid ZIP entry name: {e}"))?;
        let parts = validate_zip_path(&name)?;
        if parts.first().map(String::as_str) == Some("shaders")
            || parts.get(1).map(String::as_str) == Some("shaders")
            || (parts.len() == 1 && parts[0] == "horizon-shader.toml")
            || (parts.len() == 2 && parts[1] == "horizon-shader.toml")
        {
            valid_shader = true;
            break;
        }
    }
    if !valid_shader {
        return Err("That ZIP is not a shader pack (missing shaders/ or horizon-shader.toml).".into());
    }

    let stem = display_name
        .strip_suffix(".zip")
        .or_else(|| display_name.strip_suffix(".ZIP"))
        .unwrap_or(display_name);
    let dir = profile_home.join("shaderpacks");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create shaderpacks folder: {e}"))?;
    let dest = unique_child(&dir, &slugify(stem, "shader-pack"), ".zip");
    copy_zip_atomically(zip_path, &dest)?;
    Ok(dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned())
}

fn zip_dir_recursive(
    writer: &mut ZipWriter<File>,
    dir: &Path,
    prefix: &str,
    options: SimpleFileOptions,
) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let rel = if prefix.is_empty() {
            name_str.into_owned()
        } else {
            format!("{prefix}/{name_str}")
        };
        if file_type.is_dir() {
            writer.add_directory(format!("{rel}/"), options)?;
            zip_dir_recursive(writer, &entry.path(), &rel, options)?;
        } else if file_type.is_file() {
            writer.start_file(&rel, options)?;
            let mut f = File::open(entry.path())?;
            io::copy(&mut f, writer)?;
        }
    }
    Ok(())
}

/// Zips `<profile_home>/saves/<world_folder>` atomically to
/// `<root>/exports/meridian-world-<slug>-<unix_secs>.zip`.
pub fn export_world(
    root: &Path,
    profile_home: &Path,
    world_folder: &str,
) -> Result<PathBuf, String> {
    if world_folder.is_empty()
        || world_folder == "."
        || world_folder == ".."
        || world_folder.contains('/')
        || world_folder.contains('\\')
    {
        return Err("Invalid world folder name.".into());
    }
    let saves_dir = profile_home.join("saves");
    let world_dir = saves_dir.join(world_folder);
    let canon_saves = saves_dir
        .canonicalize()
        .map_err(|e| format!("Could not access saves folder: {e}"))?;
    let canon_world = world_dir
        .canonicalize()
        .map_err(|e| format!("Could not access world folder: {e}"))?;
    if canon_world.parent() != Some(canon_saves.as_path()) || !canon_world.join("level.dat").is_file()
    {
        return Err("Selected folder is not a valid Minecraft world.".into());
    }

    let exports_dir = root.join("exports");
    fs::create_dir_all(&exports_dir)
        .map_err(|e| format!("Could not create exports folder: {e}"))?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let slug = slugify(world_folder, "world");
    let dest = exports_dir.join(format!("meridian-world-{slug}-{secs}.zip"));
    let part = exports_dir.join(format!("meridian-world-{slug}-{secs}.zip.part"));

    let write_res = (|| -> io::Result<()> {
        let file = File::create(&part)?;
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip_dir_recursive(&mut writer, &canon_world, &slug, options)?;
        writer.finish()?;
        Ok(())
    })();
    if let Err(e) = write_res {
        let _ = fs::remove_file(&part);
        return Err(format!("Could not export world: {e}"));
    }
    fs::rename(&part, &dest).map_err(|e| {
        let _ = fs::remove_file(&part);
        format!("Could not finalize world export: {e}")
    })?;
    Ok(dest)
}

/// Lists valid world folder names under `<profile_home>/saves/`, newest modified first.
pub fn list_worlds(profile_home: &Path) -> Vec<String> {
    let saves = profile_home.join("saves");
    let Ok(entries) = fs::read_dir(&saves) else {
        return Vec::new();
    };
    let mut worlds: Vec<(SystemTime, String)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let level_dat = path.join("level.dat");
        if path.is_dir() && level_dat.is_file() {
            let modified = fs::metadata(&level_dat)
                .and_then(|m| m.modified())
                .or_else(|_| fs::metadata(&path).and_then(|m| m.modified()))
                .unwrap_or(UNIX_EPOCH);
            worlds.push((modified, name));
        }
    }
    worlds.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    worlds.into_iter().map(|(_, name)| name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "meridian-files-{:016x}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut w = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn rejects_zip_slip_and_missing_level_dat() {
        let root = scratch();
        let bad_zip = root.join("slip.zip");
        make_zip(&bad_zip, &[("../evil.txt", b"no"), ("level.dat", b"x")]);
        assert!(import_world(&bad_zip, &root, "Evil").is_err());

        let backslash_zip = root.join("bs.zip");
        make_zip(&backslash_zip, &[("a\\level.dat", b"x")]);
        assert!(import_world(&backslash_zip, &root, "Bs").is_err());

        let no_level = root.join("empty.zip");
        make_zip(&no_level, &[("readme.txt", b"hello")]);
        assert!(import_world(&no_level, &root, "Empty").is_err());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn world_export_and_import_round_trip() {
        let root = scratch();
        let profile = root.join("profile");
        let world_dir = profile.join("saves/My World");
        fs::create_dir_all(world_dir.join("region")).unwrap();
        fs::write(world_dir.join("level.dat"), b"nbt-bytes").unwrap();
        fs::write(world_dir.join("region/r.0.0.mca"), b"chunk-bytes").unwrap();

        assert_eq!(list_worlds(&profile), vec!["My World".to_string()]);
        assert!(export_world(&root, &profile, "../outside").is_err());

        let exported = export_world(&root, &profile, "My World").unwrap();
        assert!(exported.is_file());
        assert!(
            exported
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("meridian-world-my-world-")
        );

        let profile2 = root.join("profile2");
        let imported = import_world(&exported, &profile2, "Imported World.zip").unwrap();
        assert_eq!(imported, "imported-world");
        assert_eq!(
            fs::read(profile2.join("saves/imported-world/level.dat")).unwrap(),
            b"nbt-bytes"
        );
        assert_eq!(
            fs::read(profile2.join("saves/imported-world/region/r.0.0.mca")).unwrap(),
            b"chunk-bytes"
        );
        assert_eq!(list_worlds(&profile2), vec!["imported-world".to_string()]);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn imports_resource_and_shader_packs() {
        let root = scratch();
        let rp = root.join("rp.zip");
        make_zip(&rp, &[("pack.mcmeta", b"{}")]);
        let rp_name = import_resource_pack(&rp, &root, "Faithful 32x.zip").unwrap();
        assert_eq!(rp_name, "faithful-32x.zip");
        assert!(root.join("resourcepacks/faithful-32x.zip").is_file());

        let bad_rp = root.join("bad_rp.zip");
        make_zip(&bad_rp, &[("readme.txt", b"hi")]);
        assert!(import_resource_pack(&bad_rp, &root, "Bad.zip").is_err());

        let sp = root.join("sp.zip");
        make_zip(&sp, &[("shaders/gbuffers_terrain.fsh", b"void main(){}")]);
        let sp_name = import_shader_pack(&sp, &root, "Complementary.zip").unwrap();
        assert_eq!(sp_name, "complementary.zip");
        assert!(root.join("shaderpacks/complementary.zip").is_file());

        let horizon_sp = root.join("hsp.zip");
        make_zip(&horizon_sp, &[("horizon-shader.toml", b"name = 'x'")]);
        let hsp_name = import_shader_pack(&horizon_sp, &root, "Horizon Shader.zip").unwrap();
        assert_eq!(hsp_name, "horizon-shader.zip");

        fs::remove_dir_all(root).unwrap();
    }
}
