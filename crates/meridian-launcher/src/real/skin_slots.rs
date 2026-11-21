//! Each account's four saved skin slots (`<root>/skins/<account>/slot-0..3.png` and `slots.json`).
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const DIR: &str = "skins";
const META: &str = "slots.json";

#[derive(Clone, Default, Serialize, Deserialize)]
struct Meta {
    /// Each slot's arm type (`Some(true)` for slim, `Some(false)` for classic).
    slots: [Option<bool>; 4],
    /// Which slot (0..4) the account is wearing.
    worn: Option<usize>,
}

#[derive(Default)]
pub(super) struct Store(Mutex<BTreeMap<String, [Option<SkinSlot>; 4]>>);

pub(super) fn account_key(entry: &Remembered) -> Option<String> {
    match entry {
        Remembered::Microsoft {
            profile: Some(profile),
            ..
        } => {
            let uuid = heads::parse_uuid(&profile.uuid)?;
            Some(format!(
                "microsoft-{}",
                horizon_protocol::crypto::uuid_string(uuid)
            ))
        }
        Remembered::Offline { name } if valid_offline_name(name) => {
            Some(format!("offline-{name}"))
        }
        _ => None,
    }
}

fn slot_dir(root: &Path, key: &str) -> PathBuf {
    root.join(DIR).join(key)
}

fn read_skin_file(path: &Path) -> Option<(Vec<u8>, Arc<[u8]>)> {
    let file = fs::File::open(path).ok()?;
    let mut png = Vec::new();
    file.take(account::skins::MAX_PNG as u64 + 1)
        .read_to_end(&mut png)
        .ok()?;
    if png.len() > account::skins::MAX_PNG {
        return None;
    }
    let (_, skin) = heads::decode(&png)?;
    Some((png, skin))
}

fn load_from_disk(dir: &Path) -> Option<[Option<SkinSlot>; 4]> {
    let meta_path = dir.join(META);
    if !meta_path.is_file() {
        return None;
    }
    let meta: Meta = fs::read(&meta_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    Some(std::array::from_fn(|i| {
        let slim = meta.slots[i]?;
        let (png, skin) = read_skin_file(&dir.join(format!("slot-{i}.png")))?;
        Some(SkinSlot {
            png,
            skin,
            slim,
            worn: meta.worn == Some(i),
        })
    }))
}

/// Slot 1's starting skin before `slots.json` exists: a Microsoft account's skin on
/// `minecraft.net` once downloaded (else the game's default skin for its UUID), or an offline
/// account's chosen skin (else the default skin for its name).
fn seed_slot(
    root: &Path,
    entry: &Remembered,
    downloaded: Option<(Arc<[u8]>, Arc<[u8]>)>,
) -> Option<(Vec<u8>, Arc<[u8]>, bool)> {
    match entry {
        Remembered::Microsoft {
            profile: Some(profile),
            ..
        } => {
            if let Some((_, arms)) = &profile.skin {
                let (png, skin) = downloaded?;
                Some((png.to_vec(), skin, arms == "slim"))
            } else {
                let uuid = heads::parse_uuid(&profile.uuid)?;
                let (file, slim) = heads::default_skin_file(root, uuid);
                let (png, skin) = read_skin_file(&file)?;
                Some((png, skin, slim))
            }
        }
        Remembered::Offline { name } => {
            if let Some((file, slim)) = crate::launch::offline_skin(root, name)
                && let Some((png, skin)) = read_skin_file(&file)
            {
                return Some((png, skin, slim));
            }
            let uuid = horizon_protocol::crypto::offline_uuid(name);
            let (file, slim) = heads::default_skin_file(root, uuid);
            let (png, skin) = read_skin_file(&file)?;
            Some((png, skin, slim))
        }
        _ => None,
    }
}

fn write_meta(dir: &Path, slots: &[Option<SkinSlot>; 4]) -> Result<(), String> {
    let meta = Meta {
        slots: std::array::from_fn(|i| slots[i].as_ref().map(|s| s.slim)),
        worn: slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.worn)),
    };
    let bytes = serde_json::to_vec(&meta).map_err(|e| e.to_string())?;
    sources::write_file(&dir.join(META), &bytes)
}

impl Store {
    pub(super) fn get(
        &self,
        root: &Path,
        entry: &Remembered,
        downloaded: impl FnOnce() -> Option<(Arc<[u8]>, Arc<[u8]>)>,
    ) -> [Option<SkinSlot>; 4] {
        let Some(key) = account_key(entry) else {
            return [None, None, None, None];
        };
        if let Some(cached) = self.0.lock().unwrap().get(&key) {
            return cached.clone();
        }
        let dir = slot_dir(root, &key);
        if let Some(loaded) = load_from_disk(&dir) {
            self.0.lock().unwrap().insert(key, loaded.clone());
            return loaded;
        }
        let Some((png, skin, slim)) = seed_slot(root, entry, downloaded()) else {
            return [None, None, None, None];
        };
        let seeded = [
            Some(SkinSlot {
                png: png.clone(),
                skin,
                slim,
                worn: true,
            }),
            None,
            None,
            None,
        ];
        if sources::write_file(&dir.join("slot-0.png"), &png).is_ok()
            && write_meta(&dir, &seeded).is_ok()
        {
            self.0.lock().unwrap().insert(key, seeded.clone());
        }
        seeded
    }

    pub(super) fn save(
        &self,
        root: &Path,
        entry: &Remembered,
        slot: usize,
        png: Vec<u8>,
        slim: bool,
        downloaded: impl FnOnce() -> Option<(Arc<[u8]>, Arc<[u8]>)>,
    ) -> Result<(), String> {
        if slot >= 4 {
            return Err("Invalid skin slot.".into());
        }
        if png.len() > account::skins::MAX_PNG {
            return Err("That file is too big for a skin.".into());
        }
        let (_, skin) =
            heads::decode(&png).ok_or_else(|| "Choose a 64x64 PNG skin.".to_owned())?;
        let key = account_key(entry).ok_or_else(|| "No active account.".to_owned())?;
        let mut slots = self.get(root, entry, downloaded);
        let dir = slot_dir(root, &key);
        sources::write_file(&dir.join(format!("slot-{slot}.png")), &png)?;
        for existing in slots.iter_mut().flatten() {
            existing.worn = false;
        }
        slots[slot] = Some(SkinSlot {
            png,
            skin,
            slim,
            worn: true,
        });
        write_meta(&dir, &slots)?;
        self.0.lock().unwrap().insert(key, slots);
        Ok(())
    }

    pub(super) fn remove(&self, key: &str) {
        self.0.lock().unwrap().remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_png(pixel: [u8; 4]) -> Vec<u8> {
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, 64, 64);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut rgba = vec![0u8; 64 * 64 * 4];
        rgba[..4].copy_from_slice(&pixel);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
        png
    }

    #[test]
    fn seeds_slot_one_saves_slots_per_account_and_survives_corrupt_files() {
        let root =
            std::env::temp_dir().join(format!("meridian-skin-slots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let steve = Remembered::Offline {
            name: "Steve".into(),
        };
        let alex = Remembered::Offline {
            name: "Alex".into(),
        };
        let ms = Remembered::Microsoft {
            refresh_token: "token".into(),
            profile: Some(account::Account {
                uuid: "5627dd98-e6be-3c21-b8a8-e92344183641".into(),
                name: "Owner".into(),
                skin: Some((
                    "https://textures.minecraft.net/texture/abcd".into(),
                    "slim".into(),
                )),
                cape: None,
            }),
        };

        let store = Store::default();
        // Before Setup prepares the default skins (and before the Microsoft skin downloads),
        // slot 1 stays empty rather than locking in a missing skin.
        assert_eq!(store.get(&root, &steve, || None), [None, None, None, None]);
        assert_eq!(store.get(&root, &ms, || None), [None, None, None, None]);

        // Once the game's default skin for Steve (`entity/player/wide/efe`) is in resources,
        // slot 1 seeds from it and marks it worn.
        let (default_file, default_slim) =
            heads::default_skin_file(&root, horizon_protocol::crypto::offline_uuid("Steve"));
        assert!(!default_slim);
        let png_default = sample_png([10, 20, 30, 255]);
        sources::write_file(&default_file, &png_default).unwrap();
        let steve_slots = store.get(&root, &steve, || None);
        assert_eq!(steve_slots[0].as_ref().unwrap().png, png_default);
        assert!(!steve_slots[0].as_ref().unwrap().slim);
        assert!(steve_slots[0].as_ref().unwrap().worn);
        assert!(steve_slots[1..].iter().all(Option::is_none));

        // Once the Microsoft account's skin has downloaded, slot 1 seeds with its PNG and arms.
        let png_ms = sample_png([40, 50, 60, 255]);
        let (_, skin_ms) = heads::decode(&png_ms).unwrap();
        let ms_slots = store.get(&root, &ms, || Some((png_ms.clone().into(), skin_ms)));
        assert_eq!(ms_slots[0].as_ref().unwrap().png, png_ms);
        assert!(ms_slots[0].as_ref().unwrap().slim);
        assert!(ms_slots[0].as_ref().unwrap().worn);

        // Saving into slot 2 keeps slot 1 and moves `worn` to slot 2; Alex's slots stay separate.
        let png_two = sample_png([90, 100, 110, 255]);
        store
            .save(&root, &steve, 1, png_two.clone(), true, || None)
            .unwrap();
        let reloaded = Store::default().get(&root, &steve, || None);
        assert!(!reloaded[0].as_ref().unwrap().worn);
        assert_eq!(reloaded[1].as_ref().unwrap().png, png_two);
        assert!(reloaded[1].as_ref().unwrap().slim);
        assert!(reloaded[1].as_ref().unwrap().worn);
        assert_eq!(
            Store::default().get(&root, &alex, || None),
            [None, None, None, None]
        );

        // A corrupt slot PNG reads as an empty slot while the other slots keep working.
        let steve_dir = slot_dir(&root, &account_key(&steve).unwrap());
        fs::write(steve_dir.join("slot-1.png"), b"not a png").unwrap();
        let after_corrupt = Store::default().get(&root, &steve, || None);
        assert!(after_corrupt[0].is_some());
        assert!(after_corrupt[1].is_none());

        let _ = fs::remove_dir_all(&root);
    }
}
