# Linux (Ubuntu) Tauri Port Implementation Plan

**Goal:** The real Video Creater desktop app (React editor + Rust backend) builds, runs, and
packages on Ubuntu 24.04 LTS x86_64 with feature parity to macOS wherever a Linux
implementation exists, and honest capability reporting where it does not.

**Supersedes:** the 2026-09-12 strict "no LGPL including system libraries" Linux direction
(branches `feat/linux-permissive-runtime`, `feat/linux-desktop-vm-proof`). Those branches
remain as reference only.

## Policy decisions (user, 2026-09-13)

1. Linux follows the macOS license rule: no GPL; reviewed LGPL-compatible GStreamer plugins;
   GStreamer core and GES dynamically linked; glibc, GTK and WebKitGTK are ordinary system
   libraries.
2. macOS obtains H.264/HEVC/AAC/ProRes from Apple frameworks. Linux obtains them from a
   bundled, dynamically linked, minimal FFmpeg built with `--disable-gpl --disable-nonfree`
   plus an app-built `gst-libav` plugin. H.264 encode uses OpenH264 (BSD) or VA-API when
   present; HEVC encode requires VA-API.
3. Distribution GPL media components (Ubuntu `gstreamer1.0-libav` linked to a GPL FFmpeg,
   `faad`, `x265`, `-ugly`) are never loaded by the app or its webview.

## Architecture

```text
Tauri 2 (WebKitGTK)  ── React editor (unchanged, platform-aware copy)
  └ Rust backend
      ├ render runtime: system libgstreamer/GES + app plugin directory
      │    <runtime>/plugins = symlinks to license-filtered system plugins
      │                      + bundled libgstlibav.so (LGPL FFmpeg, $ORIGIN rpath)
      ├ source probe: GStreamer Discoverer (AVFoundation probe on macOS)
      ├ export: GES encoders per platform (AVFoundation exporter on macOS)
      ├ compatibility worker: platform factory sets
      ├ model helpers (same stdin/stdout contracts as the Swift helpers)
      │    transcription + speech analysis: sherpa-onnx (Parakeet TDT v3, Silero, pyannote/WeSpeaker)
      │    audio enhancement: DeepFilterNet3 (Rust libDF)
      │    semantic encoder: SigLIP 2 ONNX (ort)
      ├ credentials: Secret Service (D-Bus)
      ├ reveal / notifications: org.freedesktop FileManager1 / Notifications
      └ Codex + MCP sidecars: Linux builds
```

`tauri.linux.conf.json` overrides build hooks, `externalBin`, resources and bundle targets
(`deb`) so macOS configuration and its policy tests stay unchanged.

## Workstreams

### A. Render runtime and plugin policy
- [x] Linux render-runtime builder: FFmpeg (LGPL, pinned) + gst-libav 1.24 against it; license-filtered
      plugin symlink directory; `manifest.json` with factory inventory; scanner and core lib links.
- [x] `render_runtime.rs`: Debian multiarch scanner/lib layout, packaged resource dir for deb/AppImage,
      XDG app-support root.
- [x] `plugin_policy.rs` (+ JS copy): distro package suffix normalization; Linux reviewed factories
      (`avdec_h264/h265/aac/prores…`, `avenc_aac`, `avenc_prores_ks`, `openh264enc/dec`, `va*`),
      still denying GPL/system-libav provenance.

### B. Media paths
- [x] Source probe via GStreamer Discoverer on non-macOS.
- [x] Output profiles per platform (MP4 H.264: VA-API → OpenH264 + FFmpeg AAC; HEVC: VA-API only;
      ProRes: `avenc_prores_ks` + PCM); read `macos_only`; encoder property mapping.
- [x] Frame decode ranking and compatibility worker factory sets for Linux; no `DYLD_*` on Linux.
- [x] Webview media over an authenticated loopback HTTP server (WebKitGTK ignores custom schemes).
- [x] `decode_to_wav` via GStreamer instead of `afconvert`.
- [x] Health: `render.avfoundation` not required off macOS; Linux delivery capability items.

### C. Local intelligence helpers
- [x] `video-creater-speech` Linux helper: transcription + `speechAnalysis` modes, same JSON contract;
      Linux model catalog entries (pinned HF revisions, hashes) and runtime registration.
- [x] `video-creater-audio-enhance` Linux helper with the same CLI.
- [x] `video-creater-semantic-encoder` Linux helper with the same `prepare|serve` protocol.

### D. Desktop services and UI
- [x] Secret Service `CredentialStore`; unavailable service reported, never plaintext fallback.
- [x] Reveal in file manager; desktop notifications capability; Linux menu without macOS-only items.
- [x] Platform info command; Ctrl/Keychain/Finder/Core ML copy made platform-aware.
- [x] Codex sidecar for Linux (musl vendor binary + resources); MCP sidecar build.

### E. Build, packaging, docs
- [x] `prepare:tauri:dev` dispatch by platform; `build-linux-*` scripts; `tauri.linux.conf.json`; icons.
- [ ] `release:linux` producing an audited `.deb` with checksums and a report (AppImage dropped: it would
      bundle a libgstreamer that differs from the host plugins it links).
- [x] README and runtime docs: Linux setup, supported formats, parity gaps.

## Verification

- Rust: default workspace check on Linux, focused tests for every changed module.
- Real media: import H.264/AAC MP4, HEVC, WebM, WAV, PNG; export WebM, MP4 H.264, ProRes; validate
  streams/duration with GStreamer Discoverer.
- Desktop: launch under Xvfb, drive with WebKitWebDriver; open project, import, edit, preview, export.
- Package: install the `.deb` contents into a clean prefix and run; inspect loaded plugins/libraries
  for GPL components.
- macOS: not verifiable from Ubuntu; shared changes are cfg-scoped and existing policy tests must pass.
