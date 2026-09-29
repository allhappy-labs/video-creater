use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

const EXCLUDED_CPP: &[&str] = &["tvgLoader.cpp"];

fn collect_cpp_files(directory: &str) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().is_none_or(|extension| extension != "cpp") {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !EXCLUDED_CPP.contains(&name) {
                files.push(path.to_string_lossy().into_owned());
            }
        }
    }
    files.sort();
    files
}

fn emit_rerun_directives(directories: &[&str]) {
    for directory in directories {
        println!("cargo:rerun-if-changed={directory}");
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "h" || extension == "cpp")
                {
                    println!("cargo:rerun-if-changed={}", path.display());
                }
            }
        }
    }
}

fn reject_forbidden_features() -> io::Result<()> {
    let target = env::var("TARGET").unwrap_or_default();
    if target.starts_with("wasm32") {
        return Err(io::Error::other(
            "Video Creater's vendored dotLottie runtime is native CPU-only",
        ));
    }
    if cfg!(any(
        feature = "audio",
        feature = "c_api",
        feature = "dev",
        feature = "state-machines",
        feature = "theming",
        feature = "tracking_allocator",
        feature = "tvg-gl",
        feature = "tvg-log",
        feature = "tvg-otf",
        feature = "tvg-simd",
        feature = "tvg-threads",
        feature = "tvg-ttf",
        feature = "tvg-wg",
        feature = "wasm-bindgen-api",
        feature = "webgl",
        feature = "webgpu"
    )) {
        return Err(io::Error::other(
            "this vendored dotLottie runtime supports only the reviewed native CPU feature set",
        ));
    }
    Ok(())
}

fn validate_reviewed_tvg_features() -> io::Result<()> {
    if !cfg!(all(
        feature = "tvg-cpu",
        feature = "tvg-lottie-expressions",
        feature = "tvg-png",
        feature = "tvg-jpg",
        feature = "tvg-webp"
    )) {
        return Err(io::Error::other(
            "the reviewed CPU, expression, PNG, JPEG, and WebP feature set is required",
        ));
    }
    Ok(())
}

fn build_thorvg() -> io::Result<()> {
    validate_reviewed_tvg_features()?;

    let target = env::var("TARGET").unwrap_or_default();
    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(io::Error::other)?);
    let mut config = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(out_dir.join("config.h"))?;
    writeln!(config, "#define THORVG_VERSION_STRING \"1.0.1\"")?;
    writeln!(config, "#define THORVG_LOTTIE_LOADER_SUPPORT")?;
    writeln!(config, "#define THORVG_FILE_IO_SUPPORT 1")?;
    writeln!(config, "#define THORVG_CPU_ENGINE_SUPPORT")?;
    writeln!(config, "#define THORVG_LOTTIE_EXPRESSIONS_SUPPORT")?;
    writeln!(config, "#define THORVG_PNG_LOADER_SUPPORT")?;
    writeln!(config, "#define THORVG_JPG_LOADER_SUPPORT")?;
    writeln!(config, "#define THORVG_WEBP_LOADER_SUPPORT")?;
    writeln!(config, "#define TVG_STATIC")?;
    writeln!(config, "#define WIN32_LEAN_AND_MEAN")?;
    config.flush()?;

    let source_directories = vec![
        "deps/thorvg/inc",
        "deps/thorvg/src/common",
        "deps/thorvg/src/bindings/capi",
        "deps/thorvg/src/loaders/lottie",
        "deps/thorvg/src/loaders/raw",
        "deps/thorvg/src/loaders/png",
        "deps/thorvg/src/loaders/jpg",
        "deps/thorvg/src/loaders/webp",
        "deps/thorvg/src/loaders/webp/dec",
        "deps/thorvg/src/loaders/webp/dsp",
        "deps/thorvg/src/loaders/webp/utils",
        "deps/thorvg/src/loaders/webp/webp",
        "deps/thorvg/src/renderer",
        "deps/thorvg/src/renderer/cpu_engine",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/api",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/ecma/base",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/ecma/builtin-objects",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/ecma/builtin-objects/typedarray",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/ecma/operations",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/include",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/jcontext",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/jmem",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/jrt",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/lit",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/parser/js",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/parser/regexp",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/vm",
        "deps/thorvg/src/loaders/lottie/jerryscript/jerry-port/common",
    ];
    emit_rerun_directives(&source_directories);

    let compiler = env::var("CXX").ok();
    let windows_msvc = target.contains("windows-msvc");
    let mut build = cc::Build::new();
    if let Some(compiler) = compiler {
        build.compiler(compiler);
    } else if !windows_msvc {
        build.compiler("clang++");
    }
    build
        .std("c++14")
        .cpp(true)
        .include(&out_dir)
        .includes(&source_directories)
        .files(
            source_directories
                .iter()
                .flat_map(|directory| collect_cpp_files(directory))
                .collect::<Vec<_>>(),
        )
        .warnings(false);
    if windows_msvc {
        build.define("NOMINMAX", None);
    }
    build.compile("thorvg");

    if target.contains("apple") {
        println!("cargo:rustc-link-lib=dylib=c++");
    } else if env::var("CARGO_CFG_UNIX").is_ok() {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    bindgen::Builder::default()
        .header("deps/thorvg/src/bindings/capi/thorvg_capi.h")
        .generate()
        .map_err(|_| io::Error::other("failed to generate ThorVG bindings"))?
        .write_to_file(out_dir.join("bindings.rs"))?;
    println!("cargo:root={}", out_dir.display());
    Ok(())
}

fn main() -> io::Result<()> {
    reject_forbidden_features()?;
    if cfg!(feature = "tvg") {
        build_thorvg()?;
    }
    Ok(())
}
