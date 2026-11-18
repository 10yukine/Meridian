use crate::ely;
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub(super) const SAVE_ERROR: &str = "Could not save your account. Try signing in again.";

// Keep only the material needed to restore the selected account. This type deliberately has no Debug.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub(super) enum Remembered {
    Offline { name: String },
    Microsoft { refresh_token: String },
    Ely(ely::Session),
}

impl Remembered {
    pub(super) fn load(path: &Path) -> io::Result<Option<Self>> {
        remove_if_present(&path.with_extension("tmp"))?;
        match fs::File::open(path) {
            Ok(file) => serde_json::from_reader(file)
                .map(Some)
                .map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn save(&self, path: &Path) -> io::Result<()> {
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
