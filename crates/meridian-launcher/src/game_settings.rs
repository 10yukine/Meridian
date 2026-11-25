//! Reads and writes Horizon's per-profile game feature switches (`horizon-extras.txt`,
//! `horizon-graphics.txt`, `options.txt`) while preserving all unknown settings lines.
use crate::{GameOptions, sources};
use std::{fs, path::Path};

const EXTRAS_FILE: &str = "horizon-extras.txt";
const GRAPHICS_FILE: &str = "horizon-graphics.txt";
const OPTIONS_FILE: &str = "options.txt";
const DEFAULT_LOD_DISTANCE: u32 = 64;

fn parse_kv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            let (k, v) = trimmed.split_once(':')?;
            Some((k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect()
}

fn find_val<'a>(entries: &'a [(String, String)], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .rev()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn update_kv_file(
    path: &Path,
    updates: &[(&str, String)],
    create_header: Option<(&str, &str)>,
) -> Result<(), String> {
    let existing = fs::read_to_string(path).ok();
    let mut lines: Vec<String> = existing
        .as_deref()
        .map(|s| s.lines().map(ToOwned::to_owned).collect())
        .unwrap_or_default();
    if existing.is_none()
        && let Some((k, v)) = create_header
    {
        lines.push(format!("{k}:{v}"));
    }
    for (key, value) in updates {
        let formatted = format!("{key}:{value}");
        if let Some(pos) = lines.iter().position(|line| {
            line.trim_start()
                .split_once(':')
                .is_some_and(|(k, _)| k.trim() == *key)
        }) {
            lines[pos] = formatted;
        } else {
            lines.push(formatted);
        }
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    sources::write_file(path, out.as_bytes())
}

/// Reads `GameOptions` for `profile_home`. Missing files or keys fall back to defaults.
pub fn load(profile_home: &Path) -> GameOptions {
    let extras = fs::read_to_string(profile_home.join(EXTRAS_FILE))
        .map(|s| parse_kv(&s))
        .unwrap_or_default();
    let graphics = fs::read_to_string(profile_home.join(GRAPHICS_FILE))
        .map(|s| parse_kv(&s))
        .unwrap_or_default();
    let options = fs::read_to_string(profile_home.join(OPTIONS_FILE))
        .map(|s| parse_kv(&s))
        .unwrap_or_default();

    let lod = find_val(&graphics, "lodDistance")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    GameOptions {
        dynamic_lights: find_val(&extras, "dynamicLights") == Some("true"),
        coords: find_val(&extras, "hudCoords") == Some("true"),
        sort: find_val(&extras, "sort") == Some("true"),
        far_terrain: lod > 0,
        auto_jump: find_val(&options, "autoJump") == Some("true"),
    }
}

/// Persists `GameOptions` to `horizon-extras.txt`, `horizon-graphics.txt`, and `options.txt`,
/// preserving all unrelated lines in each file.
pub fn save(profile_home: &Path, opts: GameOptions) -> Result<(), String> {
    let bool_str = |b: bool| if b { "true" } else { "false" }.to_owned();
    update_kv_file(
        &profile_home.join(EXTRAS_FILE),
        &[
            ("dynamicLights", bool_str(opts.dynamic_lights)),
            ("hudCoords", bool_str(opts.coords)),
            ("sort", bool_str(opts.sort)),
        ],
        None,
    )?;

    let graphics_path = profile_home.join(GRAPHICS_FILE);
    let existing_lod = fs::read_to_string(&graphics_path)
        .ok()
        .and_then(|s| {
            let kv = parse_kv(&s);
            find_val(&kv, "lodDistance").and_then(|v| v.parse::<u32>().ok())
        })
        .unwrap_or(0);
    let lod = if opts.far_terrain {
        if existing_lod > 0 {
            existing_lod
        } else {
            DEFAULT_LOD_DISTANCE
        }
    } else {
        0
    };
    update_kv_file(&graphics_path, &[("lodDistance", lod.to_string())], None)?;

    update_kv_file(
        &profile_home.join(OPTIONS_FILE),
        &[("autoJump", bool_str(opts.auto_jump))],
        Some(("version", "4892")),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch() -> PathBuf {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "meridian-gsettings-{:016x}",
            u64::from_ne_bytes(nonce)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn saves_and_loads_game_options_preserving_unknown_lines() {
        let dir = scratch();
        fs::write(dir.join(EXTRAS_FILE), "customExtra:42\ndynamicLights:false\n").unwrap();
        fs::write(dir.join(GRAPHICS_FILE), "vsync:true\nlodDistance:128\n").unwrap();
        fs::write(dir.join(OPTIONS_FILE), "fov:90.0\nautoJump:false\n").unwrap();

        let loaded = load(&dir);
        assert!(loaded.far_terrain);
        assert!(!loaded.dynamic_lights);

        let updated = GameOptions {
            dynamic_lights: true,
            coords: true,
            sort: false,
            far_terrain: true,
            auto_jump: true,
        };
        save(&dir, updated).unwrap();
        assert_eq!(load(&dir), updated);

        // Unknown lines and non-zero lodDistance preserved
        let extras_text = fs::read_to_string(dir.join(EXTRAS_FILE)).unwrap();
        assert!(extras_text.contains("customExtra:42"));
        assert!(extras_text.contains("dynamicLights:true"));
        assert!(extras_text.contains("hudCoords:true"));
        let graphics_text = fs::read_to_string(dir.join(GRAPHICS_FILE)).unwrap();
        assert!(graphics_text.contains("vsync:true"));
        assert!(graphics_text.contains("lodDistance:128"));
        let options_text = fs::read_to_string(dir.join(OPTIONS_FILE)).unwrap();
        assert!(options_text.contains("fov:90.0"));
        assert!(options_text.contains("autoJump:true"));

        // Turning far_terrain off sets 0, turning back on sets 64
        let mut off = updated;
        off.far_terrain = false;
        save(&dir, off).unwrap();
        assert!(!load(&dir).far_terrain);
        let mut on = off;
        on.far_terrain = true;
        save(&dir, on).unwrap();
        assert!(
            fs::read_to_string(dir.join(GRAPHICS_FILE))
                .unwrap()
                .contains("lodDistance:64")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn creates_options_txt_with_version_when_missing() {
        let dir = scratch();
        save(
            &dir,
            GameOptions {
                auto_jump: true,
                ..Default::default()
            },
        )
        .unwrap();
        let options_text = fs::read_to_string(dir.join(OPTIONS_FILE)).unwrap();
        assert!(options_text.contains("version:4892"));
        assert!(options_text.contains("autoJump:true"));
        fs::remove_dir_all(dir).unwrap();
    }
}
