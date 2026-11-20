use super::*;
use std::io::Cursor;

const RETRY_DELAY: Duration = Duration::from_secs(30);

enum Status {
    Loading,
    /// The face, and the whole skin (64x64 RGBA) for the Skins page's figure.
    Ready(Head, Arc<[u8]>),
    RetryAt(Instant),
}

struct Entry {
    player: Account,
    url: String,
    status: Arc<Mutex<Status>>,
}

#[derive(Default)]
pub(super) struct Cache(Mutex<Vec<Entry>>);

impl Cache {
    pub(super) fn get(&self, player: &Account, url: String) -> Option<Head> {
        let mut entries = self.0.lock().unwrap();
        let entry = match entries.iter_mut().find(|entry| &entry.player == player) {
            Some(entry) => entry,
            None => {
                entries.push(Entry {
                    player: player.clone(),
                    url: url.clone(),
                    status: Arc::new(Mutex::new(Status::RetryAt(Instant::now()))),
                });
                entries.last_mut().unwrap()
            }
        };
        if entry.url != url {
            entry.url = url.clone();
            entry.status = Arc::new(Mutex::new(Status::RetryAt(Instant::now())));
        }
        let mut status = entry.status.lock().unwrap();
        match *status {
            Status::Ready(face, _) => return Some(face),
            Status::Loading => return None,
            Status::RetryAt(at) if Instant::now() < at => return None,
            Status::RetryAt(_) => {}
        }

        *status = Status::Loading;
        let worker_status = entry.status.clone();
        if std::thread::Builder::new()
            .name("launcher-head".into())
            .spawn(move || {
                let result = load(&url);
                *worker_status.lock().unwrap() = result.map_or_else(
                    || Status::RetryAt(Instant::now() + RETRY_DELAY),
                    |(face, skin)| Status::Ready(face, skin),
                );
            })
            .is_err()
        {
            *status = Status::RetryAt(Instant::now() + RETRY_DELAY);
        }
        None
    }

    /// The whole skin [`Self::get`] loaded for `player`, once it has.
    pub(super) fn skin(&self, player: &Account) -> Option<Arc<[u8]>> {
        let entries = self.0.lock().unwrap();
        let entry = entries.iter().find(|entry| &entry.player == player)?;
        match &*entry.status.lock().unwrap() {
            Status::Ready(_, skin) => Some(skin.clone()),
            _ => None,
        }
    }

    pub(super) fn remove(&self, player: &Account) {
        self.0
            .lock()
            .unwrap()
            .retain(|entry| &entry.player != player);
    }
}

pub(super) fn url(entry: &Remembered) -> Option<String> {
    match entry {
        Remembered::Microsoft {
            profile: Some(profile),
            ..
        } => {
            let (url, _) = profile.skin.as_ref()?;
            let hash = url
                .strip_prefix("https://textures.minecraft.net/texture/")
                .or_else(|| url.strip_prefix("http://textures.minecraft.net/texture/"))?;
            (!hash.is_empty() && hash.bytes().all(|c| c.is_ascii_hexdigit()))
                .then(|| format!("https://textures.minecraft.net/texture/{hash}"))
        }
        Remembered::Ely(session) if valid_offline_name(&session.name) => Some(format!(
            "https://skinsystem.ely.by/skins/{}.png",
            session.name
        )),
        _ => None,
    }
}

/// An offline player's face: the default skin the game gives that name, read from the prepared
/// resources under `root`. Missing until Setup is Ready, so it retries like a failed download.
pub(super) fn offline_skin(root: &Path, player: &Account) -> Option<String> {
    let Account::Offline { name } = player else {
        return None;
    };
    let skin = horizon_protocol::crypto::default_skin(horizon_protocol::crypto::offline_uuid(name));
    // The skin it chose on Skins, else the default one its name wears.
    let file = crate::launch::offline_skin(root, name).map_or_else(
        || crate::launch::resources_dir(root).join(format!("assets/minecraft/textures/{skin}.png")),
        |(file, _)| file,
    );
    Some(format!("file:{}", file.display()))
}

fn load(url: &str) -> Option<(Head, Arc<[u8]>)> {
    if let Some(path) = url.strip_prefix("file:") {
        return decode(&fs::read(path).ok()?);
    }
    let response = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build()
        .get(url)
        .call()
        .ok()?;
    let mut png = Vec::new();
    response
        .into_reader()
        .take(1 << 20)
        .read_to_end(&mut png)
        .ok()?;
    decode(&png)
}

#[cfg(test)]
fn face(png: &[u8]) -> Option<Head> {
    decode(png).map(|(face, _)| face)
}

/// A 64x64 or legacy 64x32 skin: its face with the hat over it, and the whole skin as 64x64
/// RGBA. A legacy skin's left arm and leg are its right ones (the game mirrors them; a front
/// view shows the difference only on an uneven limb).
// Same face and hat coordinates as the desktop launcher (launcher/src/heads.rs).
pub(super) fn decode(png: &[u8]) -> Option<(Head, Arc<[u8]>)> {
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_limits(png::Limits { bytes: 1 << 20 });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    if !matches!((reader.info().width, reader.info().height), (64, 32 | 64)) {
        return None;
    }
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    let channels = frame.color_type.samples();
    let rgba = |x: usize, y: usize| {
        let at = y * frame.line_size + x * channels;
        match &pixels[at..at + channels] {
            [r, g, b, a] => [*r, *g, *b, *a],
            [r, g, b] => [*r, *g, *b, 255],
            [l, a] => [*l, *l, *l, *a],
            [l] => [*l, *l, *l, 255],
            _ => unreachable!("PNG color type"),
        }
    };
    let mut out = [0; 256];
    for y in 0..8 {
        for x in 0..8 {
            let base = rgba(8 + x, 8 + y);
            let hat = rgba(40 + x, 8 + y);
            let a = u16::from(hat[3]);
            let px = (y * 8 + x) * 4;
            for c in 0..3 {
                out[px + c] =
                    ((u16::from(base[c]) * (255 - a) + u16::from(hat[c]) * a + 127) / 255) as u8;
            }
            out[px + 3] = 255;
        }
    }
    let mut skin = vec![0u8; 64 * 64 * 4];
    for y in 0..frame.height as usize {
        for x in 0..64 {
            skin[(y * 64 + x) * 4..][..4].copy_from_slice(&rgba(x, y));
        }
    }
    if frame.height == 32 {
        // (from x, to x, width): the right leg and arm, copied to where the left ones go.
        for (from, to, w) in [(0, 16, 16), (40, 32, 16)] {
            for y in 16..32 {
                let row = |x: usize, y: usize| (y * 64 + x) * 4;
                skin.copy_within(row(from, y)..row(from + w, y), row(to, y + 32));
            }
        }
    }
    Some((out, skin.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_legacy_skin_lends_its_right_limbs_to_the_left() {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 64, 32);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut rgba = vec![0u8; 64 * 32 * 4];
            // The right leg's front (x 4, y 20) and the right arm's (x 44, y 20).
            rgba[(20 * 64 + 4) * 4..][..4].copy_from_slice(&[1, 2, 3, 255]);
            rgba[(20 * 64 + 44) * 4..][..4].copy_from_slice(&[4, 5, 6, 255]);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&rgba)
                .unwrap();
        }
        let (_, skin) = decode(&png).unwrap();
        assert_eq!(skin.len(), 64 * 64 * 4);
        let at = |x: usize, y: usize| &skin[(y * 64 + x) * 4..][..4];
        assert_eq!(at(20, 52), [1, 2, 3, 255], "left leg's front");
        assert_eq!(at(36, 52), [4, 5, 6, 255], "left arm's front");
    }

    #[test]
    fn waits_before_retrying_a_failed_head() {
        let player = Account::Offline {
            name: "Test".into(),
        };
        let cache = Cache::default();
        let url = "invalid-url".to_owned();
        let future = Instant::now() + RETRY_DELAY;
        let status = Arc::new(Mutex::new(Status::RetryAt(future)));
        cache.0.lock().unwrap().push(Entry {
            player: player.clone(),
            url: url.clone(),
            status: status.clone(),
        });

        assert!(cache.get(&player, url.clone()).is_none());
        let next = || match *status.lock().unwrap() {
            Status::RetryAt(at) => Some(at),
            _ => None,
        };
        assert_eq!(next(), Some(future));
        let due = Instant::now();
        *status.lock().unwrap() = Status::RetryAt(due);
        assert!(cache.get(&player, url).is_none());
        assert_ne!(next(), Some(due));
    }

    #[test]
    fn composites_alpha_from_the_hat_layer() {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 64, 64);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            let mut data = vec![0; 64 * 64 * 4];
            data[(8 * 64 + 8) * 4..][..4].copy_from_slice(&[100, 150, 200, 255]);
            data[(8 * 64 + 40) * 4..][..4].copy_from_slice(&[0, 0, 200, 128]);
            writer.write_image_data(&data).unwrap();
        }
        assert_eq!(&face(&bytes).unwrap()[..4], &[50, 75, 200, 255]);
    }

    /// Values from Java 21: `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)` and
    /// `Math.floorMod(uuid.hashCode(), 18)` into vanilla's default skin list.
    #[test]
    fn an_offline_player_gets_vanillas_default_skin_face() {
        use horizon_protocol::crypto::{default_skin, offline_uuid, uuid_string};
        for (name, uuid, skin) in [
            (
                "Steve",
                "5627dd98-e6be-3c21-b8a8-e92344183641",
                "entity/player/wide/efe",
            ),
            (
                "PlayerTester",
                "da04adc8-08d2-35d0-afb4-ec18f48f11e9",
                "entity/player/wide/zuri",
            ),
            (
                "Alex",
                "36532b5e-c442-3dbb-a24c-c7e55d0f979a",
                "entity/player/slim/makena",
            ),
        ] {
            assert_eq!(uuid_string(offline_uuid(name)), uuid, "{name}");
            assert_eq!(default_skin(offline_uuid(name)), skin, "{name}");
        }
        let root = Path::new("root");
        let steve = Account::Offline {
            name: "Steve".into(),
        };
        let source = offline_skin(root, &steve).unwrap();
        assert!(source.starts_with("file:"));
        assert!(
            source
                .replace('\\', "/")
                .ends_with("textures/entity/player/wide/efe.png")
        );
        let microsoft = Account::Microsoft {
            name: "Steve".into(),
        };
        assert_eq!(offline_skin(root, &microsoft), None);
        // Before Setup the file is missing: no face yet (retried later), never a panic.
        assert_eq!(load(&source), None);
    }
}
