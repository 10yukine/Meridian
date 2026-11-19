//! Meridian's patch: a pipeline cache the device gives every graphics and compute pipeline that
//! brings none, kept in the file `HORIZON_VK_PIPELINE_CACHE` names. A later process starts with
//! the driver's compiled pipelines instead of compiling them all again, seconds on a phone,
//! where each switch between Meridian's launcher and its game is a fresh process. Off unless the
//! variable is set.
use super::Arc;
use ash::vk;
use parking_lot::Mutex;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
    time::{Duration, Instant},
};

const VARIABLE: &str = "HORIZON_VK_PIPELINE_CACHE";
/// A larger file is not one this cache wrote.
const MAX_FILE: usize = 64 << 20;
/// Saved once no pipeline has been made for this long: after a start's burst, not during it.
const IDLE: Duration = Duration::from_secs(1);
const TICK: Duration = Duration::from_millis(500);

pub(super) struct DiskCache {
    shared: Arc<Shared>,
}

struct Shared {
    device: ash::Device,
    raw: vk::PipelineCache,
    path: PathBuf,
    /// The device still exists. The saver holds it while it saves, and the device's end takes
    /// it, so a save never touches a destroyed device.
    alive: Mutex<bool>,
    dirty: AtomicBool,
    /// When the last pipeline was made.
    made: Mutex<Instant>,
}

/// The device's cache, from the file when its header is this device's; None when the variable
/// isn't set or the driver refuses a cache.
pub(super) fn open(device: &ash::Device, properties: &vk::PhysicalDeviceProperties) -> Option<DiskCache> {
    let path = PathBuf::from(std::env::var_os(VARIABLE)?);
    // Taken off the disk before the driver reads it: if data it doesn't like crashes it, the
    // next start doesn't load it again.
    let data = std::fs::read(&path)
        .ok()
        .filter(|data| data.len() <= MAX_FILE && this_device(data, properties));
    let _ = std::fs::remove_file(&path);
    let mut info = vk::PipelineCacheCreateInfo::default();
    if let Some(data) = &data {
        info = info.initial_data(data);
    }
    // SAFETY: a valid device; the data, when given, has this device's header.
    let raw = unsafe { device.create_pipeline_cache(&info, None) }.ok()?;
    let shared = Arc::new(Shared {
        device: device.clone(),
        raw,
        path,
        alive: Mutex::new(true),
        // What was read is written back: the file was taken.
        dirty: AtomicBool::new(data.is_some()),
        made: Mutex::new(Instant::now()),
    });
    let saver = Arc::clone(&shared);
    let _ = std::thread::Builder::new()
        .name("vk-pipeline-cache".into())
        .spawn(move || saver.watch());
    Some(DiskCache { shared })
}

/// `VkPipelineCacheHeaderVersionOne`, little-endian whatever the host: the data is this
/// device's and driver's.
fn this_device(data: &[u8], properties: &vk::PhysicalDeviceProperties) -> bool {
    let word = |at: usize| {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    word(0).is_some_and(|size| size >= 32)
        && word(4) == Some(vk::PipelineCacheHeaderVersion::ONE.as_raw() as u32)
        && word(8) == Some(properties.vendor_id)
        && word(12) == Some(properties.device_id)
        && data.get(16..32) == Some(&properties.pipeline_cache_uuid[..])
}

impl DiskCache {
    pub(super) fn raw(&self) -> vk::PipelineCache {
        self.shared.raw
    }

    /// A pipeline was made with the cache: it has something new to save.
    pub(super) fn made(&self) {
        *self.shared.made.lock() = Instant::now();
        self.shared.dirty.store(true, Relaxed);
    }

    /// Before the device goes: the last save, then the cache's end; the saver stops.
    ///
    /// # Safety
    /// The device is valid and nothing makes a pipeline with the cache any more.
    pub(super) unsafe fn close(&self) {
        let mut alive = self.shared.alive.lock();
        self.shared.save();
        *alive = false;
        unsafe { self.shared.device.destroy_pipeline_cache(self.shared.raw, None) };
    }
}

impl Shared {
    fn watch(&self) {
        loop {
            std::thread::sleep(TICK);
            let alive = self.alive.lock();
            if !*alive {
                return;
            }
            if self.dirty.load(Relaxed) && self.made.lock().elapsed() >= IDLE {
                self.save();
            }
        }
    }

    /// Writes the cache's data beside the file, then over it: a process ended mid-write leaves
    /// the last whole one. Called with `alive` held and true.
    fn save(&self) {
        if !self.dirty.swap(false, Relaxed) {
            return;
        }
        // SAFETY: the device is alive (`alive` is held); a cache made without
        // EXTERNALLY_SYNCHRONIZED may be read while pipelines are made with it.
        let Ok(data) = (unsafe { self.device.get_pipeline_cache_data(self.raw) }) else {
            return;
        };
        let temp = self.path.with_extension("tmp");
        if std::fs::write(&temp, &data)
            .and_then(|()| std::fs::rename(&temp, &self.path))
            .is_err()
        {
            let _ = std::fs::remove_file(&temp);
        }
    }
}
