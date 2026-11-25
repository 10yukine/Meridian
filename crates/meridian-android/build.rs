//! Carries the game's wasm std member (`horizon-wasm-std.wasm`, built by `Build-Apk.ps1` with
//! `cargo run -p cargo-horizon-mod -- wasm-std`) in the library: `MERIDIAN_WASM_STD` names it.
//! Without it the member is empty and wasm mods are refused with "no std", as before.
use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    write_env_file("MERIDIAN_WASM_STD", &out_dir.join("horizon-wasm-std.wasm"));
    write_env_file("MERIDIAN_BUILTIN_LOGICAL_ZOOM", &out_dir.join("logical_zoom.hmod"));
}

fn write_env_file(var: &str, out: &std::path::Path) {
    println!("cargo:rerun-if-env-changed={var}");
    let bytes = match env::var_os(var) {
        Some(path) => {
            println!("cargo:rerun-if-changed={}", PathBuf::from(&path).display());
            fs::read(&path).unwrap_or_else(|why| panic!("{var} {path:?}: {why}"))
        }
        None => Vec::new(),
    };
    fs::write(out, bytes).unwrap_or_else(|why| panic!("write {}: {why}", out.display()));
}
