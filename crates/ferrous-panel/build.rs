use std::path::Path;

fn main() {
    let dist_dir = Path::new("../../dist");
    println!("cargo:rerun-if-changed=../../dist");
    println!("cargo:rerun-if-changed=../../dist/index.html");

    if let Ok(entries) = std::fs::read_dir(dist_dir) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    let assets_dir = dist_dir.join("assets");
    if let Ok(entries) = std::fs::read_dir(assets_dir) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
}
