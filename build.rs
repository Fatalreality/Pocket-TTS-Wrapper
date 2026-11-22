//! Embeds the model configs vendored in `configs/`, so the app binary needs no copy of these files
//! and the build needs no checkout of the upstream repository, then runs the Tauri build step.

use std::fmt::Write as _;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // The upstream Pocket TTS configs, vendored into this repository.
    let configs_dir = manifest_dir.join("configs");

    println!("cargo:rerun-if-changed={}", configs_dir.display());

    let mut configs: Vec<(String, PathBuf)> = std::fs::read_dir(&configs_dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", configs_dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
        .map(|path| {
            (
                path.file_stem().unwrap().to_string_lossy().into_owned(),
                path,
            )
        })
        .collect();
    configs.sort();

    let mut out = String::from("/// The model configs vendored in `configs/`, by language name.\n");
    out.push_str("pub const CONFIGS: &[(&str, &str)] = &[\n");
    for (name, path) in configs.iter() {
        println!("cargo:rerun-if-changed={}", path.display());
        writeln!(out, "    ({name:?}, include_str!({:?})),", path).unwrap();
    }
    out.push_str("];\n");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded.rs");
    std::fs::write(out_path, out).unwrap();

    tauri_build::build();
}
