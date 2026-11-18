use crate::{Account, account, ely};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub(super) const SAVE_ERROR: &str = "Could not save your account. Try signing in again.";

pub(super) fn private_account_file(root: &Path) -> Option<PathBuf> {
    // Only accept Android's internal data directories, including the path resolved through
    // /data/data on the primary user. A game folder on shared storage never receives credentials.
    let root = root.canonicalize().ok()?;
    ["/data/user", "/data/user_de", "/data/data"]
        .iter()
        .any(|private| root.starts_with(private))
        .then(|| root.join("launcher-account.json"))
}

// Keep only the material needed to restore the selected account. This type deliberately has no Debug.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub(super) enum Remembered {
    Offline {
        name: String,
    },
    Microsoft {
        refresh_token: String,
        #[serde(default)]
        profile: Option<account::Account>,
    },
    Ely(ely::Session),
}

impl Remembered {
    pub(super) fn account(&self) -> Account {
        match self {
            Self::Offline { name } => Account::Offline { name: name.clone() },
            Self::Microsoft { profile, .. } => Account::Microsoft {
                name: profile
                    .as_ref()
                    .map_or_else(|| "Microsoft account".into(), |p| p.name.clone()),
            },
            Self::Ely(session) => Account::Ely {
                name: session.name.clone(),
            },
        }
    }

    pub(super) fn same_player(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Microsoft {
                    profile: Some(a), ..
                },
                Self::Microsoft {
                    profile: Some(b), ..
                },
            ) => a.uuid == b.uuid,
            (Self::Ely(a), Self::Ely(b)) => a.uuid() == b.uuid(),
            _ => self.account() == other.account(),
        }
    }
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Shelf {
    pub entries: Vec<Remembered>,
    pub active: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Stored {
    Shelf(Shelf),
    Legacy(Remembered),
}

impl Shelf {
    pub(super) fn load(path: &Path) -> io::Result<Self> {
        remove_if_present(&path.with_extension("tmp"))?;
        let file = match fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        match serde_json::from_reader(file).map_err(io::Error::other)? {
            Stored::Shelf(shelf) if shelf.active.is_none_or(|i| i < shelf.entries.len()) => {
                Ok(shelf)
            }
            Stored::Legacy(entry) => Ok(Self {
                entries: vec![entry],
                active: Some(0),
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid active account",
            )),
        }
    }

    pub(super) fn save(&self, path: &Path) -> io::Result<()> {
        if self.entries.is_empty() {
            return remove(path);
        }
        let temp = path.with_extension("tmp");
        remove_if_present(&temp)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temp)?;
            let bytes = serde_json::to_vec(self).map_err(io::Error::other)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = remove_if_present(&temp);
        }
        result
    }
}

pub(super) fn remove(path: &Path) -> io::Result<()> {
    remove_if_present(&path.with_extension("tmp"))?;
    remove_if_present(path)
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_previous_single_account_without_exposing_a_session() {
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).unwrap();
        let name = format!(
            "meridian-old-account-{:016x}.json",
            u64::from_ne_bytes(nonce)
        );
        let path = std::env::temp_dir().join(name);
        fs::write(&path, br#"{"provider":"offline","name":"OldPlayer"}"#).unwrap();
        let shelf = Shelf::load(&path).unwrap();
        assert_eq!(shelf.active, Some(0));
        assert_eq!(
            shelf.entries[0].account(),
            Account::Offline {
                name: "OldPlayer".into()
            }
        );
        remove(&path).unwrap();
    }
}
