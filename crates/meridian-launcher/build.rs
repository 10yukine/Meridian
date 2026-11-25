use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn sources(root: &Path, dir: &Path, entries: &mut Vec<String>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            sources(root, &path, entries);
        } else if path.extension().is_some_and(|ext| ext == "java") {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            entries.push(format!("({relative:?}, include_bytes!({path:?})),"));
        }
    }
}

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = crate_dir.parent().unwrap().parent().unwrap();
    let mut entries = Vec::new();
    sources(root, &root.join("mods/horizon/src/test/java"), &mut entries);
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("exporters.rs");
    fs::write(out, format!("&[{}]", entries.join("\n"))).unwrap();
}
