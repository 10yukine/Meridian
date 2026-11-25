// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Touch layout profiles (`Default` plus user-saved `.json` files under `touch-layouts/`).

use super::layout::Settings;
use std::path::{Path, PathBuf};

/// A selectable layout profile shown on the Layout profiles page.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutProfile {
    pub name: String,
    pub builtin: bool,
    pub path: Option<PathBuf>,
    pub settings: Settings,
}

/// Validates and cleans a user-entered profile name for use as `<name>.json` under
/// `touch-layouts/`. Rejects empty names, path traversal/separators, and `"Default"`.
pub fn sanitize_profile_name(raw: &str) -> Option<String> {
    let mut cleaned = String::new();
    let mut last_space = false;
    for ch in raw.trim().chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            continue;
        }
        if ch.is_whitespace() {
            if !cleaned.is_empty() && !last_space {
                cleaned.push(' ');
                last_space = true;
            }
        } else {
            cleaned.push(ch);
            last_space = false;
        }
    }
    let trimmed: String = cleaned.trim().chars().take(28).collect();
    let trimmed = trimmed
        .trim_matches(|c: char| c == ' ' || c == '.')
        .to_owned();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.eq_ignore_ascii_case("default")
    {
        return None;
    }
    Some(trimmed)
}

/// Suggests `"Profile 1"`, `"Profile 2"`, ... skipping names already in `profiles`.
pub fn next_profile_name(profiles: &[LayoutProfile]) -> String {
    (1..)
        .map(|n| format!("Profile {n}"))
        .find(|candidate| {
            !profiles
                .iter()
                .any(|p| p.name.eq_ignore_ascii_case(candidate))
        })
        .expect("unbounded")
}

/// Saves `settings` as `<dir>/touch-layouts/<sanitized>.json`.
pub fn save_profile(dir: &Path, name: &str, settings: &Settings) -> Result<PathBuf, String> {
    let clean = sanitize_profile_name(name)
        .ok_or_else(|| "Enter a valid profile name (not Default).".to_owned())?;
    let layouts_dir = dir.join("touch-layouts");
    std::fs::create_dir_all(&layouts_dir)
        .map_err(|e| format!("Could not create touch-layouts folder: {e}"))?;
    let path = layouts_dir.join(format!("{clean}.json"));
    settings
        .save(&path)
        .map_err(|e| format!("Could not save profile: {e}"))?;
    Ok(path)
}

/// Deletes a custom `.json` profile file inside `touch-layouts/`.
pub fn delete_profile(path: &Path) -> Result<(), String> {
    let in_layouts = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        == Some("touch-layouts");
    let is_json = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !in_layouts || !is_json {
        return Err("Refusing to delete a non-profile file.".into());
    }
    std::fs::remove_file(path).map_err(|e| format!("Could not delete profile: {e}"))
}

/// `Default`, followed by any valid saved or imported `.json` profiles under `<dir>/touch-layouts/`.
pub fn list_profiles(dir: &Path) -> Vec<LayoutProfile> {
    let mut profiles = vec![LayoutProfile {
        name: "Default".into(),
        builtin: true,
        path: None,
        settings: Settings::default(),
    }];
    let mut paths: Vec<_> = std::fs::read_dir(dir.join("touch-layouts"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        })
        .collect();
    paths.sort();
    for path in paths {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(mut settings) = serde_json::from_slice::<Settings>(&bytes) else {
            continue;
        };
        settings.sanitize();
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("default"))
            .unwrap_or("Custom")
            .to_owned();
        profiles.push(LayoutProfile {
            name,
            builtin: false,
            path: Some(path),
            settings,
        });
    }
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_default_and_saved_profiles_and_manages_crud() {
        let dir =
            std::env::temp_dir().join(format!("meridian-touch-profiles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let layouts = dir.join("touch-layouts");
        std::fs::create_dir_all(&layouts).unwrap();

        let base = list_profiles(&dir);
        assert_eq!(base.len(), 1);
        assert_eq!(base[0].name, "Default");
        assert!(base[0].builtin && base[0].path.is_none());
        assert_eq!(next_profile_name(&base), "Profile 1");

        let custom = Settings {
            left_handed: true,
            button_scale: 1.4,
            ..Settings::default()
        };
        let p1 = save_profile(&dir, "  Profile 1  ", &custom).unwrap();
        assert!(p1.is_file());
        std::fs::write(layouts.join("broken.json"), b"{bad json").unwrap();

        let list = list_profiles(&dir);
        let names: Vec<_> = list.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Default", "Profile 1"]);
        assert!(!list[1].builtin && list[1].path.as_deref() == Some(p1.as_path()));
        assert!(list[1].settings.left_handed);
        assert_eq!(next_profile_name(&list), "Profile 2");

        assert_eq!(sanitize_profile_name("../Default"), None);
        assert_eq!(sanitize_profile_name("Default"), None);
        assert_eq!(
            sanitize_profile_name("  My: PvP/Layout  "),
            Some("My PvPLayout".into())
        );

        delete_profile(&p1).unwrap();
        assert_eq!(list_profiles(&dir).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
