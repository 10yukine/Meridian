// Copyright (c) 2026 Ludovic Marie and the Horizon contributors.
// SPDX-License-Identifier: MIT
//! Ely.by's Yggdrasil exchange, adapted from Starling's starling-account/src/ely.rs.
use crate::account::{Body, now, send, str_field};
use serde_json::{Value, json};
use std::time::Duration;

const AUTH: &str = "https://authserver.ely.by";

/// The access token lives only as long as the launcher's selected account.
pub(crate) struct Session {
    pub name: String,
    uuid: String,
    token: String,
    expires_at: u64,
}

impl Session {
    pub fn game_session(&self) -> Result<String, String> {
        if self.expires_at <= now() {
            return Err("Ely.by session expired. Sign in again.".into());
        }
        Ok(json!({
            "name": self.name,
            "uuid": self.uuid,
            "access_token": self.token,
            "user_type": "ely",
        })
        .to_string())
    }
}

fn exchange(endpoint: &str, body: Value) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .timeout_connect(Duration::from_secs(10))
        .redirects(0)
        .user_agent(concat!("meridian/", env!("CARGO_PKG_VERSION")))
        .build();
    let (status, value) = send(agent.post(endpoint), Some(Body::Json(body)), "Ely.by")?;
    match status {
        200 => Ok(value),
        401 if value["errorMessage"] == "Account protected with two factor auth." => {
            Err("Enter the six-digit code from your authenticator.".into())
        }
        400 | 401 | 403 => Err("Ely.by refused those credentials.".into()),
        429 => Err("Too many Ely.by sign-in attempts. Try again in a moment.".into()),
        _ => Err("Ely.by sign-in is unavailable. Try again later.".into()),
    }
}

fn decode(value: &Value, client: &str) -> Result<Session, String> {
    let profile = &value["selectedProfile"];
    let uuid = str_field(profile, "id")
        .unwrap_or_default()
        .replace('-', "")
        .to_lowercase();
    let name = str_field(profile, "name").unwrap_or_default();
    if uuid.len() != 32
        || !uuid.bytes().all(|b| b.is_ascii_hexdigit())
        || !crate::valid_offline_name(&name)
    {
        return Err("Ely.by returned an invalid player profile.".into());
    }
    let token = str_field(value, "accessToken")
        .filter(|t| !t.is_empty())
        .ok_or("Ely.by returned no access token.")?;
    if value
        .get("clientToken")
        .and_then(Value::as_str)
        .is_some_and(|t| t != client)
    {
        return Err("Ely.by returned a session for a different client.".into());
    }
    Ok(Session {
        name,
        uuid,
        token,
        expires_at: now().saturating_add(3600),
    })
}

// Passwords and OTPs are used only for this request, never stored or included in errors.
pub(crate) fn authenticate(username: &str, password: &str, otp: &str) -> Result<Session, String> {
    authenticate_with(AUTH, username, password, otp)
}

fn authenticate_with(
    base: &str,
    username: &str,
    password: &str,
    otp: &str,
) -> Result<Session, String> {
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Err("Enter your Ely.by username or email and password.".into());
    }
    if !otp.is_empty() && (otp.len() != 6 || !otp.bytes().all(|b| b.is_ascii_digit())) {
        return Err("Use a six-digit Ely.by authenticator code.".into());
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| "Could not create an Ely.by client token.")?;
    let client: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let password = if otp.is_empty() {
        password.to_owned()
    } else {
        format!("{password}:{otp}")
    };
    let value = exchange(
        &format!("{base}/auth/authenticate"),
        json!({ "username": username, "password": password, "clientToken": client, "requestUser": true }),
    )?;
    decode(&value, &client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    #[test]
    fn ely_sign_in_sends_otp_and_returns_a_provider_specific_session() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fixture = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(line.starts_with("POST /auth/authenticate "));
            let mut length = None;
            loop {
                line.clear();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some((key, value)) = line.split_once(':')
                    && key.eq_ignore_ascii_case("content-length")
                {
                    length = Some(value.trim().parse::<usize>().unwrap());
                }
            }
            let mut bytes = vec![0; length.unwrap()];
            reader.read_exact(&mut bytes).unwrap();
            let body: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body["username"], "Player");
            assert_eq!(body["password"], "test-password:123456");
            let response = json!({
                "clientToken": body["clientToken"], "accessToken": "fixture-token",
                "selectedProfile": {"name": "Player", "id": "0123456789abcdef0123456789abcdef"}
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
        });
        let session = authenticate_with(&base, "Player", "test-password", "123456").unwrap();
        fixture.join().unwrap();
        let handoff: Value = serde_json::from_str(&session.game_session().unwrap()).unwrap();
        assert_eq!(handoff["user_type"], "ely");
        assert_eq!(handoff["access_token"], "fixture-token");
        assert!(handoff.get("password").is_none());
        assert!(handoff.get("clientToken").is_none());
    }

    #[test]
    fn ely_rejects_invalid_otp_and_mismatched_client_identity() {
        let error = authenticate("Player", "test-password", "abc123")
            .err()
            .unwrap();
        assert!(error.contains("six-digit"));
        assert!(!error.contains("test-password"));
        let body = json!({ "clientToken":"wrong", "accessToken":"private", "selectedProfile":
            {"name":"Player", "id":"0123456789abcdef0123456789abcdef"} });
        assert!(decode(&body, "expected").is_err());
    }
}
