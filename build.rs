use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    let out_dir = env::var("OUT_DIR").unwrap();

    match target_family.as_str() {
        "windows" => {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            res.compile().expect("Failed to compile Windows resources");
        }
        "unix" => {
            let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
            match target_os.as_str() {
                "macos" => {
                    let icon_path = Path::new("assets/icon.icns");
                    if !icon_path.exists() {
                        println!(
                            "cargo:warning=assets/icon.icns not found for macOS; ensure it’s provided."
                        );
                    } else {
                        // Create a macOS app bundle structure in the output directory
                        let bundle_dir = Path::new(&out_dir).join("vulkan_vibe_coding.app/Contents");
                        fs::create_dir_all(&bundle_dir.join("Resources")).expect("Failed to create bundle dirs");
                        fs::create_dir_all(&bundle_dir.join("MacOS")).expect("Failed to create MacOS dir");

                        // Copy the icon to the Resources folder
                        fs::copy(
                            icon_path,
                            bundle_dir.join("Resources/icon.icns"),
                        ).expect("Failed to copy icon.icns");

                        // Create Info.plist
                        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
                            <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
                            <plist version="1.0">
                            <dict>
                                <key>CFBundleName</key>
                                <string>vulkan_vibe_coding</string>
                                <key>CFBundleExecutable</key>
                                <string>vulkan_vibe_coding</string>
                                <key>CFBundleIconFile</key>
                                <string>icon.icns</string>
                                <key>CFBundleIdentifier</key>
                                <string>com.example.vulkanvibecoding</string>
                                <key>CFBundlePackageType</key>
                                <string>APPL</string>
                            </dict>
                            </plist>"#.to_string();

                        fs::write(bundle_dir.join("Info.plist"), plist_content).expect("Failed to write Info.plist");
                    }
                }
                "linux" => {
                    // Linux doesn’t require bundle creation or icon processing at build time
                    println!("cargo:warning=Linux build detected; no specific icon processing required.");
                }
                _ => {
                    println!("cargo:warning=Unsupported Unix OS: {}", target_os);
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