// Copyright (c) 2026 Ludovic Marie and the Horizon contributors
// SPDX-License-Identifier: MIT
//! The diagnostic ZIP a player sends with a bug report: the game's log tail, its crash reports,
//! and what the platform adds (Android's exit history). Designed after Starling's diagnostics
//! export (`crates/horizon-client/src/diagnostics.rs` at v0.9.0, MIT, Ludovic Marie, the Horizon
//! contributors and the Starling authors), reduced to what Meridian writes: no performance
//! recordings, no tombstone summaries. Nothing leaves the phone until the player shares the file.
use serde_json::json;
use std::{
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// How much of the end of one log goes in.
const FILE_LIMIT: u64 = 256 * 1024;
/// The crash reports that go in, newest first.
const CRASH_REPORTS: usize = 5;
/// The ZIPs kept; older ones are deleted by the next export.
const KEEP: usize = 3;
const PREFIX: &str = "meridian-diagnostics-";
/// Lines holding these words are dropped whole.
const PRIVATE: &[&str] = &[
    "access_token",
    "access token",
    "accesstoken",
    "refresh_token",
    "refresh token",
    "refreshtoken",
    "authorization",
    "bearer ",
    "password",
    "client_secret",
    "client secret",
    "xbl3.0",
    "chat message",
    "connecting to",
    "joining server",
    "player name",
    "username",
];

/// Where the ZIPs go, under the launcher's home.
pub fn dir(root: &Path) -> PathBuf {
    root.join("diagnostics")
}

/// Names the player typed or signed in with, which the log can repeat.
#[derive(Default)]
pub struct Hide<'a> {
    /// Every account's name (Microsoft, Ely.by, offline): becomes `<player>`.
    pub players: &'a [String],
    /// Profile names, the player's own words: becomes `<profile>`.
    pub profiles: &'a [String],
}

/// Best-effort redaction, not a promise: a line naming a credential goes whole; account and
/// profile names, e-mail addresses, UUIDs, servers (IPv4 and bracketed IPv6 addresses,
/// `host.name:port`), web addresses and the app's private paths are replaced.
// ponytail: a host name with neither a dot nor a port stays; add Starling's domain pass if
// reports show them. Names match as whole words only, so one inside a longer word stays.
pub fn redact(text: &str, hide: &Hide) -> String {
    text.lines().map(|line| redact_line(line, hide)).collect::<Vec<_>>().join("\n")
}

fn redact_line(line: &str, hide: &Hide) -> String {
    let lower = line.to_ascii_lowercase();
    if PRIVATE.iter().any(|word| lower.contains(word)) {
        return "[private log line removed]".into();
    }
    let line = replace_names(line, hide.players, "<player>");
    let line = replace_names(&line, hide.profiles, "<profile>");
    let mut out = String::with_capacity(line.len());
    let mut segment = String::new();
    for c in line.chars() {
        if c.is_whitespace() || "=,;()\"'".contains(c) {
            out.push_str(&redact_segment(&segment));
            segment.clear();
            out.push(c);
        } else {
            segment.push(c);
        }
    }
    out.push_str(&redact_segment(&segment));
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `text` with each of `names` (case-sensitive, whole word) replaced by `with`.
fn replace_names(text: &str, names: &[String], with: &str) -> String {
    let mut names: Vec<&String> = names.iter().filter(|n| !n.is_empty()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let mut text = text.to_owned();
    for name in names {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for (at, _) in text.match_indices(name.as_str()) {
            let end = at + name.len();
            if at < last
                || text[..at].chars().next_back().is_some_and(is_word)
                || text[end..].chars().next().is_some_and(is_word)
            {
                continue;
            }
            out.push_str(&text[last..at]);
            out.push_str(with);
            last = end;
        }
        out.push_str(&text[last..]);
        text = out;
    }
    text
}

fn redact_segment(segment: &str) -> String {
    let segment = replace_uuids(segment);
    let kept = segment.trim_end_matches(['.', ':', ']', '>', '}']).len();
    let (core, suffix) = segment.split_at(kept);
    let hidden = if core.contains("://")
        || core.contains("/data/user")
        || core.contains("/storage/emulated")
    {
        "[redacted]"
    } else if core
        .split_once('@')
        .is_some_and(|(user, host)| !user.is_empty() && host.contains('.'))
    {
        "<email>"
    } else if is_server(core) {
        "<server>"
    } else {
        return segment;
    };
    format!("{hidden}{suffix}")
}

/// An IPv4 or bracketed IPv6 address, or `host.name:port`, optionally with a port.
fn is_server(text: &str) -> bool {
    if let Some((inner, _)) = text.strip_prefix('[').and_then(|t| t.split_once(']')) {
        return inner.parse::<std::net::Ipv6Addr>().is_ok();
    }
    let (host, port) = match text.rsplit_once(':') {
        Some((host, port))
            if !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (host, true)
        }
        _ => (text, false),
    };
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return true;
    }
    let name = host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
    let source = matches!(
        host.rsplit('.').next().unwrap_or_default(),
        "rs" | "so" | "java" | "json" | "toml" | "txt" | "log" | "wgsl" | "zip" | "apk" | "png" | "dex"
    );
    port && name && !source && (host == "localhost" || host.contains('.'))
}

/// `text` with every UUID (hyphenated or 32 hex digits) replaced by `<uuid>`.
fn replace_uuids(text: &str) -> String {
    let bytes = text.as_bytes();
    let (mut out, mut i) = (String::with_capacity(text.len()), 0);
    while i < bytes.len() {
        if (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            && let Some(n) = uuid_len(&bytes[i..])
        {
            out.push_str("<uuid>");
            i += n;
            continue;
        }
        let Some(c) = text[i..].chars().next() else { break };
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn uuid_len(b: &[u8]) -> Option<usize> {
    let ends = |n: usize| b.len() == n || !b[n].is_ascii_alphanumeric();
    if b.len() >= 36
        && b[..36].iter().enumerate().all(|(i, c)| {
            if matches!(i, 8 | 13 | 18 | 23) { *c == b'-' } else { c.is_ascii_hexdigit() }
        })
        && ends(36)
    {
        return Some(36);
    }
    (b.len() >= 32 && b[..32].iter().all(u8::is_ascii_hexdigit) && ends(32)).then_some(32)
}

/// The last `limit` bytes of `path`.
fn tail(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(limit)))?;
    let mut data = Vec::new();
    file.take(limit).read_to_end(&mut data)?;
    Ok(data)
}

/// The newest `n` crash reports in `root/crash-reports`: regular files only, never links.
fn crash_reports(root: &Path, n: usize) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root.join("crash-reports")) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().take(n).map(|(_, path)| path).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Writes the diagnostic ZIP into [`dir`] and returns its path. `extras` are files the platform
/// adds, by name (Android's exit history). Explicit player action only: nothing but the game's
/// log, its crash reports, `extras` and a manifest goes in (no accounts, sessions, settings,
/// game arguments or worlds), all redacted, and nothing is uploaded.
pub fn export(root: &Path, extras: &[(&str, String)], hide: &Hide) -> Result<PathBuf, String> {
    let folder = dir(root);
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    // What an export the app's end cut short left.
    for entry in fs::read_dir(&folder).map_err(|e| e.to_string())?.flatten() {
        if entry.path().extension().is_some_and(|x| x == "partial") {
            let _ = fs::remove_file(entry.path());
        }
    }
    let path = folder.join(format!("{PREFIX}{}.zip", now()));
    let stage = path.with_extension("partial");
    if let Err(error) = write_zip(root, extras, hide, &stage).and_then(|()| fs::rename(&stage, &path))
    {
        let _ = fs::remove_file(&stage);
        return Err(error.to_string());
    }
    let mut old: Vec<_> = fs::read_dir(&folder)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "zip")
                && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(PREFIX))
        })
        .collect();
    // Oldest first by the time in the name (a text sort puts 9 after 10).
    old.sort_by_key(|p| {
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        stem.trim_start_matches(PREFIX).parse::<u64>().unwrap_or(0)
    });
    let excess = old.len().saturating_sub(KEEP);
    for file in old.into_iter().take(excess) {
        let _ = fs::remove_file(file);
    }
    Ok(path)
}

fn write_zip(
    root: &Path,
    extras: &[(&str, String)],
    hide: &Hide,
    stage: &Path,
) -> io::Result<()> {
    let mut archive = zip::ZipWriter::new(fs::File::create(stage)?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .large_file(false);
    let mut included = Vec::new();
    let mut add = |name: String, text: &str| -> io::Result<()> {
        archive.start_file(&name, options).map_err(io::Error::other)?;
        archive.write_all(text.as_bytes())?;
        included.push(name);
        Ok(())
    };
    let mut files = vec![(root.join("logs/latest.log"), "latest.log".to_owned())];
    for report in crash_reports(root, CRASH_REPORTS) {
        let name =
            report.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        files.push((report, format!("crash-reports/{name}")));
    }
    for (file, name) in files {
        let is_file = fs::symlink_metadata(&file).is_ok_and(|m| m.is_file());
        if let Some(data) = is_file.then(|| tail(&file, FILE_LIMIT).ok()).flatten() {
            add(name, &redact(&String::from_utf8_lossy(&data), hide))?;
        }
    }
    for (name, text) in extras {
        add((*name).to_owned(), &redact(text, hide))?;
    }
    let manifest = json!({
        "schema": 1,
        "version": env!("CARGO_PKG_VERSION"),
        "arch": std::env::consts::ARCH,
        "collected_at": now(),
        "included": included,
        "note": "Logs are bounded tails with best-effort redaction. Review before sharing. No accounts, settings or worlds are included.",
    });
    archive.start_file("manifest.json", options).map_err(io::Error::other)?;
    archive.write_all(manifest.to_string().as_bytes())?;
    archive.finish().map_err(io::Error::other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> (Vec<String>, Vec<String>) {
        (vec!["Steve_77".into(), "alex@mail.test".into()], vec!["My Pack".into()])
    }

    #[test]
    fn redaction_hides_each_kind_and_keeps_the_rest() {
        let (players, profiles) = names();
        let hide = Hide { players: &players, profiles: &profiles };
        let log = "[12:00:01] started file.rs:123 horizon_client::game 1.2.3\n\
                   got access_token abc\n\
                   Steve_77 joined; Steve_777 stays\n\
                   login alex@mail.test and bob@other.org\n\
                   profile My Pack loaded\n\
                   id 123e4567-e89b-12d3-a456-426614174000 raw 123e4567e89b12d3a456426614174000 end\n\
                   GET https://example.com/x failed\n\
                   peer 192.168.1.5:25565 closed, play.example.com:25565 down, localhost:25565 ok\n\
                   v6 [2001:db8::1]:25565 gone\n\
                   wrote /data/user/0/app/files/a.txt ok";
        assert_eq!(
            redact(log, &hide),
            "[12:00:01] started file.rs:123 horizon_client::game 1.2.3\n\
             [private log line removed]\n\
             <player> joined; Steve_777 stays\n\
             login <player> and <email>\n\
             profile <profile> loaded\n\
             id <uuid> raw <uuid> end\n\
             GET [redacted] failed\n\
             peer <server> closed, <server> down, <server> ok\n\
             v6 <server> gone\n\
             wrote [redacted] ok"
        );
    }

    #[test]
    fn export_bundles_only_logs_crashes_extras_and_a_manifest() {
        let root = std::env::temp_dir().join(format!("meridian-diag-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("logs")).unwrap();
        fs::create_dir_all(root.join("crash-reports")).unwrap();
        fs::write(root.join("logs/latest.log"), "line\npassword=hunter2\nSteve_77 here\n").unwrap();
        fs::write(root.join("crash-reports/crash-1.txt"), "boom").unwrap();
        fs::write(root.join("launcher-account.json"), "secret").unwrap();
        fs::write(root.join("launch-session.json"), "secret").unwrap();
        fs::create_dir_all(dir(&root)).unwrap();
        fs::write(dir(&root).join("old.partial"), b"cut").unwrap();
        for stamp in 1..=4 {
            fs::write(dir(&root).join(format!("{PREFIX}{stamp}.zip")), b"old").unwrap();
        }
        let (players, profiles) = names();
        let hide = Hide { players: &players, profiles: &profiles };
        let extras = [("android-exits.txt", "exit by Steve_77".to_owned())];
        let path = export(&root, &extras, &hide).unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        let mut files: Vec<_> = zip.file_names().map(|n| n.unwrap().into_owned()).collect();
        files.sort();
        assert_eq!(
            files,
            ["android-exits.txt", "crash-reports/crash-1.txt", "latest.log", "manifest.json"]
        );
        let mut log = String::new();
        zip.by_name("latest.log").unwrap().read_to_string(&mut log).unwrap();
        assert_eq!(log, "line\n[private log line removed]\n<player> here");
        let mut exits = String::new();
        zip.by_name("android-exits.txt").unwrap().read_to_string(&mut exits).unwrap();
        assert_eq!(exits, "exit by <player>");
        let left: Vec<_> = fs::read_dir(dir(&root)).unwrap().flatten().collect();
        assert_eq!(left.len(), KEEP);
        assert!(left.iter().all(|e| e.path().extension().is_some_and(|x| x == "zip")));
        let _ = fs::remove_dir_all(&root);
    }
}
