use super::{AtomicBool, Path, fetch, sources};
use horizon_prepare::java::Jdk;
use sha2::{Digest, Sha256};
use std::{
    ffi::{CStr, OsStr, OsString},
    fs,
    num::NonZeroUsize,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
};

const URL: &str = "https://github.com/AngelAuraMC/angelauramc-openjdk-build/releases/download/download_jre25/jre25-android-arm64.tar.xz";
const DIGEST: &str = "d3eb7afe2240c26728a1bb440502c5f18ac3883e932d202dd7f0c9bcbbce4c37";
pub(super) const LENGTH: usize = 38_031_580;

fn native_dir() -> Result<PathBuf, String> {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::uninit();
    // The package manager installs the Java helper beside libmeridian.
    let found = unsafe { libc::dladdr(native_dir as *const () as *const _, info.as_mut_ptr()) };
    if found == 0 {
        return Err("Unable to locate the installed Java launcher.".into());
    }
    let info = unsafe { info.assume_init() };
    if info.dli_fname.is_null() {
        return Err("Android did not return a native library path.".into());
    }
    let name = unsafe { CStr::from_ptr(info.dli_fname) };
    Path::new(OsStr::from_bytes(name.to_bytes()))
        .parent()
        .map(Path::to_owned)
        .ok_or_else(|| "Missing native library directory.".into())
}

/// Whether the runtime is unpacked and is the pinned archive's: `install` then downloads nothing.
pub(super) fn installed(root: &Path) -> bool {
    let runtime = root.join(".local/java25");
    runtime.join("lib/modules").is_file()
        && runtime.join("lib/libjli.so").is_file()
        && fs::read_to_string(runtime.join(".archive-sha256"))
            .as_deref()
            .ok()
            == Some(DIGEST)
}

pub(super) fn install(
    root: &Path,
    agent: &ureq::Agent,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<Jdk, String> {
    let native = native_dir()?;
    let helper = native.join("libmeridian_java.so");
    if !helper.is_file() || !native.join("libc++_shared.so").is_file() {
        return Err("This APK is missing the installed Java launcher.".into());
    }
    let runtime = root.join(".local/java25");
    let temp = root.join(".local/launcher-tmp");
    fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    if !installed(root) {
        let bytes = fetch(agent, URL, LENGTH as u64, cancel, progress)?;
        if bytes.len() != LENGTH || format!("{:x}", Sha256::digest(&bytes)) != DIGEST {
            return Err("The Android Java runtime checksum did not match.".into());
        }
        let staging = runtime.with_extension("partial");
        for old in [&staging, &runtime] {
            if old.exists() {
                fs::remove_dir_all(old).map_err(|e| e.to_string())?;
            }
        }
        fs::create_dir(&staging).map_err(|e| e.to_string())?;
        let decoder = xz4rust::XzDecoder::in_heap_with_alloc_dict_size(1 << 20, 64 << 20);
        let reader = xz4rust::XzReader::new_with_buffer_size_and_decoder(
            std::io::Cursor::new(bytes),
            NonZeroUsize::new(1 << 16).unwrap(),
            decoder,
        );
        tar::Archive::new(reader)
            .unpack(&staging)
            .map_err(|e| format!("Unable to extract the Android Java runtime: {e}"))?;
        sources::write_file(&staging.join(".archive-sha256"), DIGEST.as_bytes())?;
        fs::rename(staging, &runtime).map_err(|e| e.to_string())?;
    }
    let libraries =
        std::env::join_paths([runtime.join("lib"), native]).map_err(|e| e.to_string())?;
    let env = [
        ("JAVA_HOME", runtime.clone().into_os_string()),
        (
            "JAVA_TOOL_OPTIONS",
            format!("-Djava.io.tmpdir={}", temp.display()).into(),
        ),
        ("TMPDIR", temp.into_os_string()),
        ("LD_LIBRARY_PATH", libraries),
    ]
    .map(|(key, value)| (OsString::from(key), value))
    .to_vec();
    Ok(Jdk::with_launcher(&runtime, helper, env))
}
