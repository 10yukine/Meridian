use super::*;
use std::io::Cursor;

const RETRY_DELAY: Duration = Duration::from_secs(30);

enum Status {
    Loading,
    Ready(Head),
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
            Status::Ready(face) => return Some(face),
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
                    Status::Ready,
                );
            })
            .is_err()
        {
            *status = Status::RetryAt(Instant::now() + RETRY_DELAY);
        }
        None
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

fn load(url: &str) -> Option<Head> {
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
    face(&png)
}

// Same face and hat coordinates as the desktop launcher (launcher/src/heads.rs).
fn face(png: &[u8]) -> Option<Head> {
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
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
