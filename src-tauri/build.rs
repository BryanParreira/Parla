use std::env;
#[cfg(target_os = "macos")]
use std::{path::PathBuf, process::Command};

// swift-rs' own bridge functions, which the Rust side of swift-rs calls.
#[cfg(target_os = "macos")]
const SWIFT_RS_EXPORTS: [&str; 4] =
    ["_retain_object", "_release_object", "_string_from_bytes", "_data_from_bytes"];

/// Xcode 27 builds @_cdecl functions as local symbols in release archives. swift-rs
/// 1.0.8 promotes Parla's own ones back to global but not its bridge helpers, so the
/// app fails to link with "Undefined symbols: _retain_object". Promote those too.
#[cfg(target_os = "macos")]
fn globalize_swift_rs_exports() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let profile = if env::var("PROFILE").as_deref() == Ok("release") { "release" } else { "debug" };
    let archive = out_dir.join("swift-rs/parla-swift").join(profile).join("libparla-swift.a");
    if !archive.exists() {
        return;
    }
    let sysroot = Command::new(env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--print", "sysroot"])
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string());
    let host = env::var("HOST").unwrap_or_default();
    let objcopy = sysroot
        .map(|root| PathBuf::from(root).join("lib/rustlib").join(host).join("bin/llvm-objcopy"))
        .filter(|path| path.exists());
    let Some(objcopy) = objcopy else {
        println!("cargo:warning=llvm-objcopy not found; run `rustup component add llvm-tools`");
        return;
    };
    let status = Command::new(objcopy)
        .args(SWIFT_RS_EXPORTS.map(|symbol| format!("--globalize-symbol={symbol}")))
        .arg(&archive)
        .status();
    assert!(status.is_ok_and(|s| s.success()), "failed to globalize swift-rs exports");
}

fn main() {
    // The Swift engine is macOS's; Windows and Linux, or macOS testing their engine, use
    // the Rust one instead.
    let desktop = env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos")
        || env::var_os("CARGO_FEATURE_DESKTOP_ENGINE").is_some();
    println!("cargo::rustc-check-cfg=cfg(desktop_engine)");
    println!("cargo:rerun-if-changed=build.rs");
    if desktop {
        assert!(
            env::var_os("CARGO_FEATURE_DESKTOP_ENGINE").is_some(),
            "Windows and Linux builds need `--features desktop-engine`"
        );
        println!("cargo:rustc-cfg=desktop_engine");
        // ONNX Runtime's event tracing and espeak's registry lookups live in advapi32,
        // which the static speech library expects the app to link.
        if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            println!("cargo:rustc-link-lib=advapi32");
        }
    } else {
        link_swift();
    }
    tauri_build::build()
}

#[cfg(target_os = "macos")]
fn link_swift() {
    swift_rs::SwiftLinker::new("15.0")
        .with_package("parla-swift", "./swift-lib/")
        .link();
    globalize_swift_rs_exports();

    // speech-swift is built against the OS Swift runtime rather than bundling one.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    // Apple Intelligence only exists on macOS 26+; weak linking keeps Parla
    // launching on macOS 15, where Enhance simply reports itself unavailable.
    println!("cargo:rustc-link-arg=-Wl,-weak_framework,FoundationModels");
    println!("cargo:rerun-if-changed=swift-lib/src");
    println!("cargo:rerun-if-changed=swift-lib/Package.swift");
}

#[cfg(not(target_os = "macos"))]
fn link_swift() {}
