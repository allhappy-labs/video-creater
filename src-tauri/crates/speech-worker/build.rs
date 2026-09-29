//! Adds the relocatable runtime search path for the bundled sherpa-onnx and ONNX Runtime
//! shared libraries. Paths are relative to the helper executable (`$ORIGIN`):
//!
//! - `$ORIGIN`: libraries staged beside the helper.
//! - `$ORIGIN/speech-runtime/lib`: Tauri development target directories.
//! - `$ORIGIN/../resources/speech-runtime/lib`: `src-tauri/binaries` development staging.
//! - `$ORIGIN/../lib/Video Creater/speech-runtime/lib`: deb/AppImage resource layout.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let native = std::env::var_os("CARGO_FEATURE_NATIVE").is_some();
    let linux = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux");
    if !(native && linux) {
        return;
    }
    let search_path = [
        "$ORIGIN",
        "$ORIGIN/speech-runtime/lib",
        "$ORIGIN/../resources/speech-runtime/lib",
        "$ORIGIN/../lib/Video Creater/speech-runtime/lib",
    ]
    .join(":");
    println!("cargo:rustc-link-arg-bins=-Wl,--enable-new-dtags");
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{search_path}");
}
