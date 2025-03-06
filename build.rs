use std::env;
use winresource::WindowsResource;

fn main() {
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    match target_family.as_str() {
        "windows" => {
            let mut res = WindowsResource::new();
            res.set_icon("assets/icon.ico");
            res.compile().expect("Failed to compile Windows resources");
        }
        "unix" => {
            if env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
                if !std::path::Path::new("assets/icon.icns").exists() {
                    println!("cargo:warning=assets/icon.icns not found for macOS; ensure it’s provided.");
                }
            }
        }
        _ => {
            println!("cargo:warning=Unsupported target family: {}", target_family);
        }
    }

    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/icon.icns");
    println!("cargo:rerun-if-changed=shaders/vert.glsl");
    println!("cargo:rerun-if-changed=shaders/frag.glsl");
}