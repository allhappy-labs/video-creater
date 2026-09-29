//! Fail-closed replacement for the upstream sherpa-onnx-sys build script.
//!
//! It never downloads anything. On Linux it links the shared libraries from
//! `SHERPA_ONNX_LIB_DIR`, which must be a TTS-disabled source build produced by
//! `scripts/build-linux-speech-worker.mjs` (identified by its build marker).

use std::env;
use std::path::PathBuf;

const BUILD_MARKER: &str = "video-creater-sherpa-onnx-build.json";

fn main() {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }

    let Some(lib_dir) = env::var_os("SHERPA_ONNX_LIB_DIR").map(PathBuf::from) else {
        // Workspace checks and lints compile this crate without linking a binary. Emit no link
        // directives: any executable that needs the native runtime then fails to link instead
        // of picking up an unreviewed library.
        println!(
            "cargo:warning=SHERPA_ONNX_LIB_DIR is not set; sherpa-onnx is not linked. Build the \
             Linux speech worker with scripts/build-linux-speech-worker.mjs."
        );
        return;
    };
    let marker = lib_dir.join(BUILD_MARKER);
    let marker_text = std::fs::read_to_string(&marker).unwrap_or_else(|error| {
        panic!(
            "{} is missing ({error}); SHERPA_ONNX_LIB_DIR must be a Video Creater source build",
            marker.display()
        )
    });
    if !marker_text.contains("\"ttsEnabled\": false") {
        panic!("{} does not certify a TTS-disabled build", marker.display());
    }
    for required in ["libsherpa-onnx-c-api.so", "libonnxruntime.so"] {
        if !lib_dir.join(required).exists() {
            panic!("{} is missing {required}", lib_dir.display());
        }
    }
    println!("cargo:rerun-if-changed={}", marker.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=sherpa-onnx-c-api");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");
    println!("cargo:lib_dir={}", lib_dir.display());
}
