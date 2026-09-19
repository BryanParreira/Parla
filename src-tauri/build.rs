use std::{env, fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

// speech-swift 0.0.9 treats every token id below 274 as a control token, but ids
// 234–243 are the digits 0–9 in the Parakeet TDT v3 vocabulary, so numbers were
// silently dropped from transcripts.
const DIGIT_FILTER_ORIGINAL: &str = "if tokenId >= firstTextTokenId {";
const DIGIT_FILTER_PATCHED: &str =
    "if tokenId >= firstTextTokenId || (234...243).contains(tokenId) {";

fn patch_speech_swift() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let scratch = out_dir.join("swift-rs").join("parla-swift");

    let status = Command::new("swift")
        .args(["package", "--package-path", "swift-lib", "--scratch-path"])
        .arg(&scratch)
        .arg("resolve")
        .status()
        .expect("failed to run swift package resolve");
    assert!(status.success(), "resolving Swift dependencies failed");

    let decoder = scratch.join("checkouts/speech-swift/Sources/ParakeetASR/TDTGreedyDecoder.swift");
    let source = fs::read_to_string(&decoder)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", decoder.display()));
    if source.contains(DIGIT_FILTER_PATCHED) {
        return;
    }
    assert_eq!(
        source.matches(DIGIT_FILTER_ORIGINAL).count(),
        1,
        "speech-swift's decoder changed; revisit the digit token patch in build.rs"
    );

    let mut permissions = fs::metadata(&decoder)
        .expect("failed to read decoder permissions")
        .permissions();
    permissions.set_mode(permissions.mode() | 0o200);
    fs::set_permissions(&decoder, permissions).expect("failed to make decoder writable");
    fs::write(
        &decoder,
        source.replacen(DIGIT_FILTER_ORIGINAL, DIGIT_FILTER_PATCHED, 1),
    )
    .expect("failed to patch speech-swift decoder");
}

fn main() {
    patch_speech_swift();

    swift_rs::SwiftLinker::new("15.0")
        .with_package("parla-swift", "./swift-lib/")
        .link();

    // speech-swift is built against the OS Swift runtime rather than bundling one.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    // Apple Intelligence only exists on macOS 26+; weak linking keeps Parla
    // launching on macOS 15, where Enhance simply reports itself unavailable.
    println!("cargo:rustc-link-arg=-Wl,-weak_framework,FoundationModels");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=swift-lib/src");
    println!("cargo:rerun-if-changed=swift-lib/Package.swift");

    tauri_build::build()
}
