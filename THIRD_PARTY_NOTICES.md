# Third-party licenses and media

The root [MIT license](LICENSE) covers Video Creater's original code and documentation. It does
not relicense third-party source, dependencies, runtime libraries, model weights, or source media.
Keep the applicable upstream notices when redistributing those components or a package that
contains them.

## Source vendored in this repository

| Component | License and notices |
| --- | --- |
| dotLottie Rust integration | [MIT license](src-tauri/vendor/dotlottie-rs/LICENSE) and [its third-party notices](src-tauri/vendor/dotlottie-rs/THIRD_PARTY_NOTICES.md) |
| ThorVG | [MIT license](src-tauri/vendor/dotlottie-rs/deps/thorvg/LICENSE) |
| JerryScript in ThorVG | [Apache-2.0 notice](src-tauri/vendor/dotlottie-rs/deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/LICENSE) and [license text](src-tauri/vendor/dotlottie-rs/LICENSES/Apache-2.0.txt) |
| RapidJSON in ThorVG | [MIT notice](src-tauri/vendor/dotlottie-rs/deps/thorvg/src/loaders/lottie/rapidjson/LICENSE) |
| libwebp in ThorVG | [BSD-style license and patent grant](src-tauri/vendor/dotlottie-rs/deps/thorvg/src/loaders/webp/LICENSE) |
| sherpa-onnx-sys | [Apache-2.0 license](src-tauri/vendor/sherpa-onnx-sys/LICENSE) |

The package manifests and lockfiles identify additional dependencies obtained during builds.
Those packages remain under their own licenses. The Linux media runtime dynamically links
LGPL-licensed GStreamer/GES components; see [Linux development](docs/development/linux.md).
Redistributors must include the notices and license materials required by the exact components
in their package. Provider services and downloaded model weights have separate terms; an MIT
license for this repository grants no rights to them.

## Bundled and generated media

The bundled Edison sample is derived from public-domain U.S. government footage. Its source,
transformations, and hashes are recorded in [sample provenance](src-tauri/resources/sample-project/PROVENANCE.md).
Other test fixtures are included to exercise the application; do not infer
that an upstream source or model's terms have been replaced by the repository's MIT license.
