// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
// Adapted from Starling's `crates/starling-account/src/skins.rs` at 5432370d5736, itself from
// Vasysik/horizon-android (`launcher/src/skins/cloud.rs`); see THIRD_PARTY_NOTICES.md.
//! A Microsoft account's skin on its Minecraft profile (`POST /minecraft/profile/skins`: the PNG
//! and its arms), which servers then hand to the other players. Ely.by's public API takes no
//! uploads: an Ely.by account changes its skin on the website (`crate::ELY_SKINS`).
use super::*;

/// The largest PNG sent, as Horizon Android's import takes.
pub const MAX_PNG: usize = 1024 * 1024;
const BOUNDARY: &str = "meridian-skin-upload-v1";

/// The multipart form the Minecraft services take: the arms (`variant`), then the PNG.
fn form(png: &[u8], slim: bool) -> Vec<u8> {
    let variant = if slim { "slim" } else { "classic" };
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n\
         --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\n\
         Content-Type: image/png\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(png);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// Make `png` (a 64x64 or legacy 64x32 skin) the skin of the account `secrets` signs in, with
/// slim or classic arms; the profile Mojang has afterwards (the new skin's URL). Neither the
/// request nor the answer is logged: they carry the account's token.
pub(crate) fn upload(
    ep: &Endpoints,
    secrets: &Secrets,
    png: &[u8],
    slim: bool,
) -> Result<Account, String> {
    if png.len() > MAX_PNG || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Choose a PNG skin file.".into());
    }
    if secrets.minecraft_expires_at <= now() {
        return Err("Microsoft session expired. Sign in again.".into());
    }
    let token = &secrets.minecraft_token;
    let request = bearer(
        request_to("POST", &format!("{}/minecraft/profile/skins", ep.minecraft)),
        token,
    )
    .set(
        "Content-Type",
        &format!("multipart/form-data; boundary={BOUNDARY}"),
    );
    match request.send_bytes(&form(png, slim)) {
        Ok(_) => {}
        Err(ureq::Error::Status(400, _)) => {
            return Err("Minecraft turned this skin down. Use a 64x64 PNG skin.".into());
        }
        Err(ureq::Error::Status(401 | 403, _)) => {
            return Err("Microsoft session expired. Sign in again.".into());
        }
        Err(ureq::Error::Status(429, _)) => {
            return Err("Too many skin changes. Try again in a minute.".into());
        }
        Err(ureq::Error::Status(..)) => {
            return Err("The Minecraft services failed. Try again later.".into());
        }
        Err(ureq::Error::Transport(_)) => {
            return Err(
                "Could not reach the Minecraft services. Check your internet connection.".into(),
            );
        }
    }
    profile(ep, token)?.ok_or_else(|| NO_PROFILE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::sync::{Arc, Mutex};

    const UUID: &str = "069a79f444e94726a5befca90e38aaf5";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR pixels";

    /// What the mock profile service was asked: method and path, the Authorization header and
    /// the body.
    type Seen = Arc<Mutex<Vec<(String, String, Vec<u8>)>>>;

    /// Answers `answers` (status, JSON) in turn, one request per connection.
    fn serve(answers: Vec<(u16, Value)>) -> (Endpoints, Seen) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen: Seen = Arc::default();
        let log = seen.clone();
        std::thread::spawn(move || {
            for (status, body) in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (mut length, mut auth) = (0, String::new());
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    let (k, v) = header.split_once(':').unwrap();
                    match k.to_ascii_lowercase().as_str() {
                        "content-length" => length = v.trim().parse().unwrap(),
                        "authorization" => auth = v.trim().to_owned(),
                        _ => {}
                    }
                }
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).unwrap();
                let request = line
                    .split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ");
                log.lock().unwrap().push((request, auth, bytes));
                let text = body.to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                );
            }
        });
        let ep = Endpoints {
            minecraft: format!("{base}/mc"),
            ..Endpoints::live()
        };
        (ep, seen)
    }

    /// A Minecraft token with a day left.
    fn secrets() -> Secrets {
        Secrets {
            refresh_token: "refresh".into(),
            sign_in_required: false,
            minecraft_token: "mc-token".into(),
            minecraft_expires_at: now() + 86400,
            xuid: None,
            owns: true,
        }
    }

    #[test]
    fn the_profile_gets_the_skin_with_its_arms_and_returns_the_new_one() {
        let profile = json!({
            "id": UUID, "name": "Notch",
            "skins": [{"id": "s2", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/new", "variant": "SLIM"}],
            "capes": [],
        });
        let (ep, seen) = serve(vec![(200, profile.clone()), (200, profile)]);
        let kept = upload(&ep, &secrets(), PNG, true).unwrap();
        assert_eq!(
            kept.skin,
            Some((
                "http://textures.minecraft.net/texture/new".to_owned(),
                "slim".to_owned()
            ))
        );
        let seen = seen.lock().unwrap();
        let (request, auth, body) = &seen[0];
        assert_eq!(request, "POST /mc/minecraft/profile/skins");
        assert_eq!(auth, "Bearer mc-token");
        let text = String::from_utf8_lossy(body);
        assert!(text.contains("name=\"variant\"\r\n\r\nslim\r\n"), "{text}");
        assert!(body.windows(PNG.len()).any(|w| w == PNG));
        assert!(text.ends_with(&format!("\r\n--{BOUNDARY}--\r\n")));
        assert_eq!(seen[1].0, "GET /mc/minecraft/profile");
    }

    #[test]
    fn nothing_is_sent_for_a_file_that_is_no_png_or_an_expired_session() {
        // No answers: a request would fail as unreachable rather than as these.
        let (ep, seen) = serve(vec![]);
        assert_eq!(
            upload(&ep, &secrets(), b"GIF89a", false).unwrap_err(),
            "Choose a PNG skin file."
        );
        let expired = Secrets {
            minecraft_expires_at: now() - 1,
            ..secrets()
        };
        assert_eq!(
            upload(&ep, &expired, PNG, false).unwrap_err(),
            "Microsoft session expired. Sign in again."
        );
        assert!(seen.lock().unwrap().is_empty());
    }

    #[test]
    fn a_refused_skin_says_so() {
        let (ep, _) = serve(vec![(400, json!({"errorMessage": "Invalid skin"}))]);
        assert_eq!(
            upload(&ep, &secrets(), PNG, false).unwrap_err(),
            "Minecraft turned this skin down. Use a 64x64 PNG skin."
        );
    }
}
