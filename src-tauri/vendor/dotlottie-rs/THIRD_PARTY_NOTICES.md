# Vendored dotLottie Runtime Notices

This directory contains pinned source used only by the isolated Video Creater precompose worker.

| Component | Revision | License | License file |
| --- | --- | --- | --- |
| dotlottie-rs | `2ce5e48f5786c3e60301d66db4cfdff56c896895` | MIT | `LICENSE` |
| ThorVG | `73045df5398690eb1c6c0946e0b2301032337776` | MIT | `deps/thorvg/LICENSE` |
| JerryScript snapshot embedded by ThorVG | ThorVG revision above | Apache-2.0 | `deps/thorvg/src/loaders/lottie/jerryscript/jerry-core/LICENSE`, full text at `LICENSES/Apache-2.0.txt` |
| RapidJSON embedded by ThorVG | ThorVG revision above | MIT | `deps/thorvg/src/loaders/lottie/rapidjson/LICENSE` |
| libwebp embedded by ThorVG | ThorVG revision above | BSD-3-Clause plus patent grant | `deps/thorvg/src/loaders/webp/LICENSE` |
| LodePNG 20200306 | ThorVG revision above | Zlib | Notice embedded in `deps/thorvg/src/loaders/png/tvgLodePng.cpp` |
| jpgd by Richard Geldreich | ThorVG revision above | Public Domain | Notice embedded in `deps/thorvg/src/loaders/jpg/tvgJpgd.cpp` |
| Video Creater Lottie interpolator replacement | Local patch | MIT | Header in `deps/thorvg/src/loaders/lottie/tvgLottieInterpolator.cpp` |

Video Creater enables expression evaluation but does not ship dotlottie's C API, GPU/WASM build paths, state machines, theming, audio, font loaders, upstream examples, tests, or sample assets. The upstream fallback font payload is not shipped until its exact source and OFL-1.1 notice are verified.

The compiled source closure contains no MPL-2.0 code. The upstream dual-noticed interpolator was replaced with an independently implemented MIT cubic-Bezier solver.

`SOURCE.json` records source repositories, immutable revisions, archive hashes, enabled-feature policy, and local provenance patches.
