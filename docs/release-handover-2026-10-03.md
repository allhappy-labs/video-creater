# Release hardening handover — 2026-10-03

## Current state

- The working tree is clean on `main`; the final product-code change is `b1faf305adc4399970e9f9a01614f9319abd0998` (`fix(remote): route workflows for historical project identities`).
- Local `main` is ahead of `origin/main` by the release-hardening commits plus documentation-only commits for this handover; nothing in this release pass has been pushed or deployed.
- Both isolated follow-up audits are now accepted (see below). Neither needed a source change, so the `b1faf305` package evidence still applies.
- The scope was preview and remote-app-server reliability, then broader release hardening. The editor UI was intentionally left alone apart from minor reliability fixes; there is no redesign in this work.
- Linux evidence is current for this commit where noted below. It is not evidence of a signed macOS release, a deployed remote host, or physical-device behavior.

## What changed

The integrated release-hardening commits are `e0aeefa6..b1faf305`:

- Restored reliable remote preview media playback: source and timeline video play, seek, replay, audio, Lottie animation, shaders/motion, and animated export are covered by the remote test path.
- Made remote workflow identity handling safe across reloads and historical project IDs. Job responses no longer overwrite a canonical project locator, and custom historical IDs are used only for volatile routing rather than persisted recovery data.
- Bounded silent Temporal/workflow connection attempts, tightened lease reconnect and revoked/expired WebSocket handling, and made workflow/catalog capability reconciliation truthful.
- Made export publication atomic, preserved an earlier export when a replacement fails, avoided recursive bundle exports, and bounded compositor memory/cancellation paths.
- Optimized only exact-pixel-center compositor transforms while retaining the canonical sampler for all other coordinates.
- Hardened desktop and remote-runtime packaging checks, media provenance, notices, ELF dependency policy, and smoke-evidence token redaction.

## Verified evidence

| Area | Result | Evidence |
| --- | --- | --- |
| Frontend suite | `pnpm test`: 2,981 tests passed at `b1faf305`. Lint, unused-code, and source-quality checks passed. | `output/release-readiness/b1faf305adc4/report.json` |
| Remote browser path | 10 Playwright cases passed against the remote configuration: desktop and phone source/timeline decode, play, seek, replay, Lottie frames, audio recovery, shader/motion, interactive export, reload/new-tab recovery, reconnect/render/download. | `output/release-readiness/b1faf305adc4/report.json` |
| Fixture browser/visual path | 75 browser cases passed and seven visual comparisons had zero mismatches at `6af58c0e`. The final `b1faf305` change is remote-identity TypeScript only and the full final frontend suite passed. | `output/release-readiness/b1faf305adc4/report.json` |
| Native tests | 923 Rust library tests passed (9 intentionally ignored); 58 integration targets passed with 2,044 tests (11 ignored); workspace helpers passed 83 tests (2 intentionally ignored). | `output/release-hardening-native-integrations.json` |
| Rendering | 30-second 1920x1080/30fps render completed: 900 frames, no swap, peak RSS about 980 MiB; first/middle/final output frames exactly matched canonical raw frames. Cancellation returned in 60.839 ms without partial output. | `output/release-readiness/b1faf305adc4/native-render/report.json` |
| Long-GOP video | H.264 fixture decoded and independently sought to frames 45, 99, 165, 179, 15, and 99 with exact RGBA matches; callback abort completed in 16.8 ms. A non-fatal OpenH264 initialization diagnostic was logged, while GStreamer reached EOS with no bus error. | `output/release-readiness/b1faf305adc4/long-gop/audit.log` |
| Remote runtime package | 84 staged files; all 83 manifest payload hashes independently matched; 16 ELF checks passed. H.264/AAC, Lottie, BMP review, and real audio inference all passed. | `output/remote-host-package/render-runtime/manifest.json` |
| Debian package | 82 of 82 package policy checks passed; 20 ELF checks passed; media, Lottie, and audio checks passed. The retained package is 185,707,268 bytes with SHA-256 `f16e377d34cfcf4fdaa661c5162417edff300e47f6cdbe2da2da19abb8711325`. | `output/linux-release/b1faf305adc4/report.json` |
| Packaged GTK smoke | 16 passed, 0 failed, 1 skipped. The bundled WebKit video advanced 3.009 seconds and sought two seconds; a real H.264/AAC export was 8.023 seconds at 1280x720. Audio denoise RMS improved from 0.02821 to 0.01178. | `output/linux-desktop-smoke/b1faf305adc4/evidence.json` |
| Native menu and keyring (follow-up) | 4 passed, 0 failed, 0 skipped against the same packaged `.deb` (SHA-256 above). A real X `Ctrl+Z` restored exactly one of two deleted clips; GTK Edit → Undo (F10, Right, Down, Return) restored the other. A synthetic OpenAI credential was set, listed as `keychain`, deleted, and listed as `missing` through a private GNOME Keyring on a private session bus. | `output/linux-desktop-smoke/b1faf305adc4-native-gaps-r2/evidence.json` |
| Temporal unavailable, packaged (follow-up) | 5 passed. The packaged app reports `featureEnabled: false`. With Temporal selected, an MP4 export was refused with status `unavailable`, the job was marked failed, the reason was shown in the editor, and no artifact, export file, or render directory appeared in the following 20 seconds. | `output/temporal-local-verification/b1faf305adc4/packaged-unavailable/evidence.json` |
| Temporal live worker, debug build (follow-up) | 4 passed. Debug app and worker rebuilt from the current source with default features; official Temporal CLI 1.8.3 dev server with in-memory persistence. One MP4 export completed through the worker with a Temporal run ID: 4,939,742 bytes, video and audio streams present. | `output/temporal-local-verification/b1faf305adc4/live-worker/evidence.json` |
| Security | JavaScript and Rust vulnerability/unsound advisories had no unresolved findings. Eleven upstream Rust unmaintained warnings remain visible for dependency stewardship. | `output/release-readiness/b1faf305adc4/report.json` |

The 30-second renderer ran on llvmpipe software graphics at roughly 2.43 fps. It demonstrates correctness and bounded memory on this VM; it is not a hardware-GPU or real-time performance claim.

## Artifact locations

| Artifact | Location |
| --- | --- |
| Consolidated release report | `output/release-readiness/b1faf305adc4/report.json` |
| Native render frames, movie, and measurements | `output/release-readiness/b1faf305adc4/native-render/` |
| Long-GOP fixture and audit | `output/release-readiness/b1faf305adc4/long-gop/` |
| Debian package report and retained `.deb` | `output/linux-release/b1faf305adc4/` |
| Packaged desktop smoke evidence | `output/linux-desktop-smoke/b1faf305adc4/` |
| Remote host runtime manifest | `output/remote-host-package/render-runtime/manifest.json` |
| Native menu and keyring follow-up | `output/linux-desktop-smoke/b1faf305adc4-native-gaps-r2/` |
| Temporal follow-up (packaged, live worker, exported MP4) | `output/temporal-local-verification/b1faf305adc4/` |

These artifacts are local release evidence and are not committed with the source tree.

## Accepted follow-ups

Both isolated checks started during this pass were completed afterwards and are accepted. The checkout was at the documentation-only commit `54313fb0`; `git diff b1faf305 54313fb0` touches only this file.

**Native safety audit.** The first attempt (`output/linux-desktop-smoke/b1faf305adc4-native-gaps/`) recorded no steps: it was launched with `VIDEO_CREATER_PROVIDER_KEYCHAIN_SERVICE` alone, and the app deliberately refuses to start with a partial settings-acceptance environment (`VIDEO_CREATER_SETTINGS_ACCEPTANCE_ROOT is required`). That is a launch error, not a product defect, and it stays on record as a failed run. The rerun dropped the override and passed:

```
XDG_DATA_HOME=<out>/isolated-state/data XDG_CONFIG_HOME=<out>/isolated-state/config \
XDG_CACHE_HOME=<out>/isolated-state/cache XDG_RUNTIME_DIR=<out>/isolated-state/runtime \
dbus-run-session -- node scripts/linux-desktop-smoke.mjs --app <extracted .deb>/usr/bin/video-creater \
  --release-report output/linux-release/b1faf305adc4/report.json \
  --out output/linux-desktop-smoke/b1faf305adc4-native-gaps-r2 --display :97 --fake-audio \
  --only "native menu:,provider credentials round-trip" --native-menu \
  --xdotool-root ~/.cache/vc-desktop-tools/acceptance-b1/root --gst-tools-root <same> --keyring-root <same> \
  --tauri-driver ~/.cache/vc-desktop-tools/cargo/bin/tauri-driver \
  --native-driver ~/.cache/vc-desktop-tools/root/usr/bin/WebKitWebDriver
```

The credential went to the app's default service name inside the private keyring only; the test secret appears in no retained file.

**Temporal audit.** Two runs:

- Packaged: `output/temporal-local-verification/b1faf305adc4/unavailable-audit.mjs` is a one-off driver built on the smoke helpers and kept with the evidence, not in `scripts/`. Its first attempt failed on its own probe (a webview-side `invoke` wrapper does not observe the app's calls); that run is kept as `packaged-unavailable-attempt1-probe-defect/`. The app behaved identically in both runs.
- Live worker: `scripts/linux-desktop-smoke.mjs --dev-server --app src-tauri/target/debug/video-creater --temporal --temporal-cli ~/.cache/vc-desktop-tools/temporal/temporal --temporal-worker src-tauri/target/debug/video-creater-temporal-worker --only "render system health,select Temporal execution,export MP4 through the Temporal worker"`. The cached CLI archive matches the SHA-256 published for `temporal_cli_1.8.3_linux_amd64.tar.gz`. The build log is `live-worker/debug-build.log`.

Limits of the Temporal evidence: the live run is a debug build, not the package, and covers one export. Transcription through the worker and model download were not run. Selecting Temporal in Advanced settings is accepted without a warning; the unavailable state is reported when work is dispatched, and the message is developer-oriented ("Rebuild with default features…"). That is truthful but could be friendlier in a packaged build; it was left alone because the UI is out of scope for this pass.

## Remaining release gates

| Gate | Why it remains open | Required next evidence |
| --- | --- | --- |
| macOS packaged app | This is an Ubuntu VM; no AppKit, signing, notarization, or macOS media stack was exercised. | Named macOS runner/device, signed package build, launch/preview/export smoke, and notarization/signing records. |
| Deployed remote HTTPS host | A tailnet Serve mapping exists, but there is no verified active app-server backend or intended public production URL in this handover. | The intended host URL, deployed SHA, public health/readiness result, and device acceptance of source playback, timeline playback, seeking, replay, audio, Lottie, shaders, and export. |
| Credentials and providers | The keyring round-trip passed with a synthetic secret in a private keyring. No live keyring account, model provider, external agent, or production quota was used. | Controlled account test, bounded logs, provider/quota confirmation, and cleanup evidence. |
| Hardware media | VM checks used software graphics and a fake audio sink. | Physical audio device and target GPU playback/export checks. |

Do not treat local Tailscale routing as deployment proof. Do not weaken secure-cookie behavior, persist pairing material, or place credentials in test logs while pursuing the remote-host gate.

## Safe continuation sequence

1. Start with `rtk git status --short --branch` and verify the current commit before accepting any follow-up.
2. The two isolated audits are accepted; nothing further is needed for them unless the source changes.
3. For any source change, rerun its focused tests, the relevant package/smoke gate, and the consolidated frontend/native checks affected by the change.
4. Use a real deployed HTTPS host and a macOS runner only after the environment and URL are supplied. Record deployed SHA, health/readiness, exact test coverage, and any provider quota evidence.
5. Before release approval, compare the final source commit with the artifact report paths above; rebuild artifacts whenever the commit changes.
6. Push or deploy only with separate authorization. Preserve the unrelated worktrees until their owners have completed their work.

## Repository housekeeping

The completed release branch was fast-forwarded into local `main` and removed. Three unrelated worktrees remain and must be preserved:

- `check-project-dependencies`
- `support-custom-video-agent-keys`
- `build-remote-connectable-frontend`

No pull request was created for this pass.
