# Linux desktop

Video Creater runs as the same Tauri desktop app on Ubuntu 24.04 LTS x86_64: the React editor
in WebKitGTK and the Rust backend with GStreamer/GES composition. Linux follows the macOS
license rule: no GPL components; LGPL GStreamer, GES, GTK and WebKitGTK are dynamically linked.

## Build prerequisites

Install the system development packages once:

```bash
sudo apt install build-essential cmake pkg-config curl libwebkit2gtk-4.1-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libges-1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
  gstreamer1.0-gl gstreamer1.0-x bubblewrap clang libclang-dev
uv tool install meson && uv tool install ninja   # or: pipx install meson ninja
```

Then install JavaScript dependencies and start the desktop runtime:

```bash
rtk pnpm install
rtk pnpm dev
```

`pnpm dev` runs `scripts/tauri-dev.mjs`, which prepares the host platform's helpers
(`prepare:tauri:dev:linux` on Linux) before `tauri dev`. Tauri merges
`src-tauri/tauri.linux.conf.json` automatically. The first preparation downloads and builds
pinned sources (FFmpeg, gst-libav, NASM, sherpa-onnx, ONNX Runtime) into
`~/.cache/video-creater-linux-build` and takes several minutes; later runs reuse the cache.

## Architecture differences from macOS

| Area | macOS | Linux |
| --- | --- | --- |
| H.264/HEVC/AAC/ProRes codecs | VideoToolbox, AudioToolbox, AVFoundation exporter | Bundled FFmpeg 6.1 built `--disable-gpl --disable-nonfree` with an app-built gst-libav plugin; OpenH264 for H.264 encode; HEVC encode through VA-API only |
| Render runtime | Bundled GStreamer runtime from Homebrew | System GStreamer 1.24 core and GES with an app plugin directory: license-filtered system plugins plus the bundled FFmpeg plugin (`scripts/build-linux-media-runtime.mjs`) |
| Source probing | AVFoundation exporter | GStreamer Discoverer in the compatibility worker |
| Webview media | `asset://` protocol | Authenticated `127.0.0.1` HTTP media server (WebKitGTK does not play custom URI schemes) |
| Transcription and speech analysis | FluidAudio / Core ML | sherpa-onnx (Parakeet TDT 0.6B v3, Silero VAD, pyannote + WeSpeaker), built from source without TTS |
| Audio enhancement | DeepFilterNet3 on Core ML | DeepFilterNet3 through Rust libDF (tract), model embedded |
| Semantic encoder | SigLIP 2 on Core ML | SigLIP 2 ONNX through ONNX Runtime |
| Credentials | Keychain | Secret Service over D-Bus (GNOME Keyring, KWallet); no plaintext fallback |
| Reveal / notifications | Finder, UserNotifications | `org.freedesktop.FileManager1` with `xdg-open` fallback; `org.freedesktop.Notifications` |
| Codex sidecar | darwin npm vendor binary | linux-x64 npm vendor binary plus bundled `rg`; sandboxing uses system `bwrap` |

The Linux plugin directory is built from an explicit file list. Ubuntu's `gstreamer1.0-libav`
(linked against a GPL FFmpeg), `faad`, `x265`, and other GPL or unreviewed plugins are never
visible to the app, its workers, or the webview. Render pipelines additionally reject any
auto-plugged encoder or muxer outside the reviewed policy.

## Supported formats

- Verified import: MP4 H.264/AAC (probe, preview playback, composition, transcription), WAV and PNG
  through the existing render suites.
- Decoders in the reviewed runtime: H.264, HEVC, ProRes, MPEG-4 Part 2, MJPEG, DNxHD, VP8, VP9;
  AAC, MP3, AC-3/E-AC-3, ALAC, Opus, Vorbis, FLAC, PCM; PNG and JPEG images.
- Verified export: WebM (VP8/Opus), MP4 H.264/AAC, ProRes 422 MOV (Proxy for draft). MP4 HEVC/AAC
  is offered only when a VA-API encoder is present.

## Packaging

```bash
rtk pnpm release:linux:preflight
rtk pnpm release:linux
```

`release:linux` builds a `.deb` with `tauri build --bundles deb`, then audits the package:
required payload files, and the ELF `NEEDED` entries of every shipped binary against GPL media
libraries and distribution FFmpeg builds. The report is written to
`output/linux-release/<commit>/report.json`. The package depends on the Ubuntu GStreamer 1.24
packages, WebKitGTK 4.1 and bubblewrap; it recommends GNOME Keyring and the GTK desktop portal.
AppImage is not produced: an AppImage would bundle a different libgstreamer than the host plugins
it links.

## Verification

```bash
rtk pnpm verify:linux-media-runtime
cargo test --manifest-path src-tauri/Cargo.toml --test linux_media_e2e
```

`scripts/linux-desktop-smoke.mjs` drives the real WebKitGTK window through tauri-driver on Xvfb
(display `:94`, WebDriver port 4444), using the redesigned editor's accessible names. Runs share the
display, the port, `/tmp/video-creater-editor-project` and the app preferences, so run them one at a
time. Keep each run small with `--only "<step name substring>[,<substring>]"`.

Packaged app, extracted from the audited `.deb` without installing it:

```bash
rtk dpkg-deb -x <package.deb> /tmp/vc-deb-root
VIDEO_CREATER_APP_SUPPORT_DIR=/tmp/vc-smoke/app-support rtk dbus-run-session -- \
  env -u VIDEO_CREATER_RENDER_RUNTIME_ROOT node scripts/linux-desktop-smoke.mjs \
  --app /tmp/vc-deb-root/usr/bin/video-creater \
  --tauri-driver <tauri-driver> --native-driver <WebKitWebDriver> --fake-audio \
  --keyring-root <gnome-keyring root> \
  [--native-menu --xdotool-root <xdotool root> [--gst-tools-root <gstreamer1.0-tools root>]] \
  [--agent-flows [--agent-backend claude --claude-model haiku]] --release-report output/linux-release/<commit12>/report.json \
  --out output/<run>
```

`VIDEO_CREATER_APP_SUPPORT_DIR` gives the extracted package its own app-support folder. The render
runtime's fingerprint hashes only its manifest, so without it the package could reuse a runtime
materialized earlier whose links point into another extraction.

Debug build, for the Temporal steps:

```bash
VIDEO_CREATER_RENDER_RUNTIME_ROOT=<runtime-dir> rtk dbus-run-session -- node scripts/linux-desktop-smoke.mjs \
  --dev-server --app src-tauri/target/debug/video-creater \
  --tauri-driver <tauri-driver> --native-driver <WebKitWebDriver> --fake-audio \
  --temporal --temporal-cli <temporal> --temporal-worker src-tauri/target/debug/video-creater-temporal-worker \
  --out output/<run>
```

Release builds leave out the `temporal-worker` feature, so the Temporal steps need the debug build.
Against the package, add `--temporal-unavailable` instead (steps named `Temporal unavailable: …`):
with Temporal execution selected, an export must be refused with its reason shown, leave one failed
job, and produce no artifact, file or render folder.

A run that stops before its steps (for example an app that exits at startup) records the last output
lines of each background process as `processLogTails` in `evidence.json`, and the panic or load error
found there as `fatalCause`. An export step ends as soon as its job is recorded as failed.

The core steps cover:

- project creation from the project home, media import, and sample editor preview playback and seeking;
- MP4 export from the Export popover (Format and Resolution segments, then "Export video"). The
  export is found through its new `exportArtifacts` record, and completion is read from the
  Background tasks indicator, whose labels are recorded as `progressLabels`;
- Premiere XML and project package exports from the Export popover's footer links;
- App settings opened from the gear "Editor menu", checked for macOS-only wording;
- Ctrl+Z after a timeline delete: Linux Undo/Redo menu items have no accelerator, so the webview
  keydown handler must restore the clip in the editor and in the saved project;
- an audio-denoise render through the DeepFilterNet3 helper;
- provider credential save and delete through the Secret Service (with `--keyring-root`);
- agent self-tests (MCP server, proposal validator, and both conversation-turn backends) and
  semantic search status. The two support rows must pass and at least one backend must be ready:
  Codex and Claude are alternatives, so a Claude-only install passes this step. Every row's
  diagnosis is recorded either way;
- a scan of the running app and WebKit processes for GPL media libraries.

Opt-in steps:

- `--native-menu --xdotool-root <dir>`: real X key events from `xdotool`. One Ctrl+Z restores
  exactly one of two deleted clips, and F10 → Edit → Undo in the GTK menu bar restores the other
  (with a menu bar click as the recorded fallback). `--gst-tools-root <dir>` adds X screenshots
  (`ctrlz-*.png`, `menu-*.png`) through `gst-launch-1.0 ximagesrc`.
- `--agent-flows`: AI flow 1 (a safe edit auto-applies with result frames, then Show changes and
  Undo) and flow 2 (a track removal waits for review and is dismissed) through the app's conversation
  turn. It spends model usage.
  - `--agent-backend claude [--claude-model haiku|sonnet|opus]` (the default) stores `agentBackend: "claude"` (and
    the model) through `update_app_preferences` before the first prompt, so the turns take the same
    stored-preference path Agent settings writes, and requires the `agent.claude` health row to be
    ready. The app runs the user's own installed `claude` from `PATH` (or the
    `claudeExecutablePath` preference) with their existing subscription login; the CLI is never
    bundled and no API key is involved. The run restores `agentBackend: "automatic"` when the
    flows finish.
  - `--agent-backend codex` runs the turns through the app's bundled Codex app-server with the
    user's existing Codex login instead, and requires the `agent.codex` health row to be ready.
- `--export-tasks`: an export to a chosen folder (`/tmp/vc-export-target`) under a chosen name,
  revealed from the Background tasks popover's "Show in folder"; the pill's progress fraction while a
  1080p render runs; and a timeline delete, its save and a media import during that render, with the
  webview's round-trip times recorded. The folder chooser is a GTK window with no accessible name, so
  the first step needs `--xdotool-root` and is skipped without it.
- `--temporal --temporal-cli <temporal>`: starts a Temporal dev server and the app worker, installs
  the transcription and speech models from App settings → AI & Models, exports through the Temporal
  worker, and transcribes imported speech from the Captions tab before running speech analysis.
- `--release-report <report.json>`: records the package path, SHA-256 and commit in the evidence.

`<out>/evidence.json` records every step as `passed`, `failed` or `skipped`. A skipped step names
the missing flag or prerequisite, and skipped steps never count as passed. `summary` counts the
statuses. `runContext` records the app kind (`packaged` or `debug`), the commit, the package from
`--release-report`, the `VIDEO_CREATER_*` overrides and the `--only` patterns. Files the checks rely
on are copied out of the temporary project:

- `export-mp4/renders/<jobId>/pipeline-report.json`;
- `denoise/`: the denoise render's `pipeline-report.json` and every denoise cache `manifest.json`;
- `agent-flow-1/frames/renders/<jobId>/preview-qa/preview-frames/*.png`: the result frames;
- `temporal-export-mp4/`: the Temporal export's render report.

`--fake-audio` routes audio to a null sink on hosts without an audio server. Test tools can be
staged without root under `/tmp/vc-smoke-tools`: `cargo install tauri-driver --root <dir>`, and
Ubuntu packages fetched with `apt-get download` and extracted with `dpkg-deb -x`:

- `webkit2gtk-driver` (its version must match the installed WebKitGTK);
- `xdotool libxdo3` (BSD-3-Clause);
- `gnome-keyring libgck-1-0 libgcr-base-3-1 gcr`: GPL-2.0-or-later, used only as an out-of-process
  D-Bus service inside the run's private `dbus-run-session`, never linked or shipped;
- `gstreamer1.0-tools` (LGPL).

The debug app needs the dev resources that `pnpm dev` prepares: the render runtime (or
`VIDEO_CREATER_RENDER_RUNTIME_ROOT` pointing at a `build:linux-media-runtime --output` tree), the
bundled sample project (`src-tauri/resources/sample-project` copied to `sample-project/` next to
the binary), and the sidecars. The denoise step needs the `video-creater-audio-enhance` helper
next to the binary, staged in `src-tauri/binaries`, or named by `VIDEO_CREATER_AUDIO_ENHANCER`.
Without `--dev-server`, a debug build needs the frontend served on `127.0.0.1:1420`.
`scripts/linux-desktop-smoke-selectors.test.ts` (part of `test:source-quality`) fails when a label
the smoke looks up no longer exists in `src/`.

## Limitations

- Ubuntu 24.04 x86_64 is the verified target. Other distributions and ARM64 are not verified.
- HEVC export requires a VA-API encoder (Intel/AMD GPU with Mesa or intel-media drivers).
- Temporal execution requires a running Temporal service and `video-creater-temporal-worker`, and
  a build with the `temporal-worker` feature. Transcription from the editor runs in the app process
  by default and needs neither.
- OpenH264 is built by Ubuntu from source, so Cisco's binary patent license does not apply;
  H.264/AAC patent licensing is not assessed by this project.
- The semantic encoder's ONNX export declares no license of its own; Apache-2.0 is inherited from
  the upstream Google model and should be reviewed before distribution.
