# Linux compatibility: bundled runtime and permissive dependencies

Date: 2026-09-12
Status: option 1 boundary approved; narrow X11, Wayland, audio, canonical-core,
codec-runtime, and progressive-MP4 proofs passed; production host, common MP4,
artifact qualification, packaging, and release feasibility remain partial.
Source baseline: `1f6ff8e7`.

## Objective

Deliver a native Linux Video Creater with app-owned tools and libraries, no user
toolchain setup, and exclusively permissively licensed application dependencies,
including system libraries. Preserve
the existing project format, reversible editing, structured agent proposals, and
the quality of the macOS workflow. This specification does not authorize changing
the existing macOS runtime or relaxing the new license requirement.

This supersedes the earlier Linux plan wherever that plan proposes bundling
GStreamer/GES or treats a limited editor preview as the product release. The
earlier plan is a source inventory, not an approved implementation sequence.

## Requirements and explicit boundaries

1. Every application utility, codec implementation, media helper, model inference
   runtime, agent executable, MCP executable, and required script interpreter must
   be included in the installed application with its runtime dependency closure.
2. Users must not install FFmpeg, GStreamer, Node, Python, npm, pnpm, Cargo, Codex,
   a model server, or other tools to make an advertised local feature work.
   No first-launch download may supply a missing executable or shared library.
3. Bundled software and every library linked or loaded by the app or its helpers
   must use an explicitly reviewed permissive license, including libraries taken
   from the system. An existing dependency, application-owned helper process,
   optional feature, or dynamic link is not an exemption. Copyleft components
   must not be hidden in helper binaries.
4. Baseline model weights ship with the app so the baseline local workflow works
   offline immediately. Optional larger models may retain the existing explicit
   in-app download flow, with verified hashes and separately reviewed licenses.
   All inference tools are always bundled. This conservative baseline may increase
   installer size; the implementation must measure and report that cost.
5. Provider and agent services that inherently require network access remain
   network-dependent. Their local clients are bundled; the specification does not
   promise offline hosted inference or require shipping third-party cloud servers.
   No local Temporal service may be an undeclared runtime prerequisite.
6. User decision, 2026-09-12, clarified by selection of option 1: **no LGPL in the
   app, its helpers, or any libraries they link or load**. This includes static
   code, direct/transitive dynamic dependencies, and native driver/client libraries.
   A library's presence on the user's machine does not make it acceptable.
   Enumerate every permitted application dependency explicitly.
7. Independent OS services are permitted: the installed window compositor/X
   server, audio server, portal, and credential service are outside the app's
   library-license boundary. Their internal dependencies do not disqualify the
   app. Document protocols, service requirements, and failure behavior. All
   client-side code and libraries inside application processes remain in scope.
   An app-supplied helper, codec server, inference server, or browser is not an
   independent OS service; moving app functionality to a process does not exempt it.
   The Linux kernel and installed OS are not replaced or relicensed by this project.

Candidate test environment: Ubuntu 24.04 LTS x86_64 using the installed desktop
services. Their internal libraries no longer block this candidate under option 1;
the app's own dependency audit and protocol tests must still pass. Wayland, X11,
other distributions, and ARM64 require separate evidence. Select an install format only after auditing
its launcher, application payload, and required runtime dependencies; neither
`.deb` nor AppImage receives an automatic exemption.

## Conflict with the current architecture

The current app requires GStreamer/GES and its build policy accepts LGPL entries.
GStreamer identifies its license as LGPL in its [official licensing advisory](https://gstreamer.freedesktop.org/documentation/application-development/appendix/licensing.html).
Commercial usability does not make that a permissive license for this policy.

Relevant repository boundaries:

- `scripts/gstreamer-runtime-policy.mjs` explicitly allows `LGPL-2.1-or-later`.
- `src-tauri/crates/compatibility-worker/Cargo.toml` depends on GStreamer even
  though the local crate itself declares MIT.
- `src-tauri/src/render_pipeline/` and `precompose/` couple media processing to
  GStreamer and AVFoundation. Feature gating alone does not replace that behavior.
- Shared build hooks in `package.json` and `src-tauri/tauri.conf.json` build Apple
  helpers. Several helper builders only accept Apple Silicon targets.
- Core ML/FluidAudio, semantic encoding, audio enhancement, Keychain, and native
  notifications/reveal require Linux implementations or explicit parity blockers.

The project pipeline skill currently describes GES-era implementation guidance.
Any future approved replacement must update affected guidance and readiness
contracts together; preserve EDL-first semantics and Rust ownership throughout.

## Architectural choices

| Approach | Fit and tradeoff | Disposition |
| --- | --- | --- |
| Port the existing GStreamer/GES bundle | Greatest media reuse, but includes LGPL software | Rejected under the stated requirement |
| Retain standard Tauri Linux; replace only application media/model backends | GTK/WebKitGTK and the usual libc path still introduce prohibited libraries | Rejected after the user's system-library clarification |
| Replace Linux desktop host, libc path, and media stack | Preserves reusable project logic but requires a new audited runtime architecture | Required direction; feasibility must be established before implementation planning |

Do not silently choose a different webview shell: its complete dependency graph,
codecs, packaging helpers, and licenses need the same audit. Moving an application
codec to a system package would violate the bundled-tools requirement.

### Desktop/runtime feasibility gate

The standard Tauri Linux GTK/WebKitGTK stack is excluded by this policy; see the
[primary-source license findings](../../research/2026-09-12-linux-permissive-runtime.md).
glibc is also excluded: GNU identifies it as LGPL in the
[GNU C Library project documentation](https://www.gnu.org/s/libc/).
A permissively licensed libc such as musl is a candidate, not proof of a complete
compliant application. Rebuilding only the main Rust executable does not fix a
glibc-linked sidecar, loaded graphics driver, web engine, or audio dependency.

Before selecting a desktop host, produce a small native proof covering window
creation, editor presentation, keyboard/input, CPU frame preview, audio output,
and file access, with the full application loaded-library inventory and a separate
record of independent service protocols and requirements.
Evaluate React reuse only if the chosen web engine's complete build qualifies.
If no suitable engine qualifies, scope a Linux presentation-layer replacement
explicitly; preserving React is a preference, not permission to violate policy.
Do not adopt Electron, CEF, another webview, or a native UI toolkit solely from
its top-level license. Audit bundled agent executables and all model workers too.

The proof must cover lazy-loaded modules and supported GPU driver paths as well
as startup. Begin with a software-rendered correctness path; acceleration remains
unavailable until every driver/client library loaded into application processes
qualifies. Access to independent display/audio services through an audited client
protocol is allowed even when those services internally use LGPL libraries.
Service availability, negotiated formats, errors, and reconnection still require
tests; the license boundary is not proof of functional compatibility.

Tauri's documented Linux AppImage media path adds GStreamer files. Therefore
enabling `bundleMediaFramework` is not an acceptable shortcut under this policy.
Build on the oldest claimed supported base and audit the actual package rather
than assuming AppImage guarantees compatibility. See [Tauri packaging guidance](https://v2.tauri.app/distribute/appimage/).

## Component architecture

Keep the existing React editor and Rust canonical project/job boundaries wherever
the chosen desktop host permits. Add a narrow platform runtime interface at the
existing backend boundaries, avoiding platform conditionals throughout the UI.

| Component | Responsibility and contract |
| --- | --- |
| Runtime inventory/resolver | Resolve versioned app-owned artifacts; validate target, checksum, protocol, and dependency classification before launch |
| Media probe/decode backend | Produce metadata, timed video frames, PCM audio, thumbnails, and seek results without mutating source files |
| Timeline compositor | Consume the existing validated render plan; compose cuts, transforms, effects, captions, layers, and mixed audio on one timebase |
| Delivery backend | Encode/mux declared output profiles; finalize atomically after artifact validation |
| Model workers | Provide timestamped transcription, speech analysis, enhancement, and semantic embeddings through versioned Rust-owned jobs |
| Desktop services | Audited client protocols for independent OS credentials, dialogs, notifications, reveal, clipboard, and desktop integration |
| Capability reporter | Expose backend identity, supported operations/formats, and actionable failure reasons to Settings and editor controls |

Reuse existing protocols where sufficient. Any extension is versioned and rejects
unknown/incompatible versions. Rust launches helpers using absolute paths derived
from the trusted application root, passes arguments without shell interpolation,
and owns timeouts, cancellation, progress, child-process cleanup, logs, and artifacts.
Helpers write only allocated job output locations, never canonical project files.

No runtime search of `PATH`, shell initialization, package-manager directories,
developer checkouts, Python environments, or globally installed plugins is allowed.
Sanitize helper library/plugin search variables. Preserve only documented desktop
session variables needed for display, audio, and secure-store integration.

Preview must use the approved media backend or an audited equivalent. Do not
accidentally restore excluded codecs through HTML video playback in the webview.
CPU rendering is the correctness baseline; hardware acceleration is optional and
must fall back without changing timeline semantics or supported baseline formats.

## License and provenance contract

Maintain a machine-readable allowlist with exact SPDX expressions and component
reviews. Initial candidate families are MIT, Apache-2.0, BSD-2-Clause,
BSD-3-Clause, ISC, and Zlib. Other licenses require an explicit review before
allowlisting; compatibility with a permissive license is insufficient. The
[LLVM term review](../../research/2026-09-12-linux-llvm-license-policy.md)
subsequently approved the exact combined term
`Apache-2.0 WITH LLVM-exception`. No other exception-bearing term is approved.

- Deny GPL, LGPL, AGPL, MPL, and other copyleft-only choices for application
  software and every linked/loaded system library, including the full dependency
  closure of each app-owned process. Independent OS service internals are excluded.
  Deny unknown, missing, noncommercial, source-available-only, and ambiguous terms.
- For `OR` expressions record the selected permitted branch; for `AND` expressions
  every obligation must pass. Review vendored files and native linked dependencies,
  not just top-level package metadata.
- Inventory executable and shared-library contents, statically linked code,
  frontend packages, plugins, inference execution providers, fonts, templates,
  weights, tokenizers, model conversions, and installer runtime payloads.
- Review model and asset licenses separately from the engine's software license.
  A permissive inference engine does not establish rights to ship its weights.
- Each item records version/commit, source URL, artifact SHA-256, target, enabled
  features/build flags, license expression, license text/notice paths, transitive
  dependencies, distribution classification, and review status.
- Keep an independent OS service inventory separate from the application library
  graph. It records service/protocol, purpose, availability checks, and failure
  behavior. The declaration preflight's `system` classification still means an
  app-linked/loaded system component, not a license exemption for OS services.
  Do not remove client libraries from the application graph by classifying them
  as services. Do not put an app-owned executable in the independent service list.
- Include notices and an SBOM in the package and release evidence. Build-time-only
  tools are recorded separately and must not leak into the shipped payload.
- Source licensing review does not establish codec patent rights. Record those
  distribution questions separately; do not label a source license a patent clearance.

No new dependency is approved by being named as a research candidate. A release
gate must examine the actual versioned binary and its dependency closure.

### Research shortlist

The [primary-source research note](../../research/2026-09-12-linux-permissive-runtime.md)
records evidence and qualifications for these candidates:

| Function | Candidate | Unresolved acceptance question |
| --- | --- | --- |
| H.264 | OpenH264, BSD-2-Clause | Input profile coverage, output quality, and distribution terms; no reliance on install-time Cisco downloads |
| AAC | libxaac, Apache-2.0 | AAC-LC integration, encoder delay, supported channels/rates, robustness, and distribution review |
| MP4 container | minimp4, CC0-1.0 | Explicit license-policy review plus seeking, edit lists, rotation, timestamps, and malformed input handling |
| WebM | libvpx/libwebm/Opus, BSD-family | Exact artifact closure, CPU performance, muxing, and seeking |
| Transcription | whisper.cpp and original Whisper weights, MIT | Word timestamps are experimental; alignment quality and baseline model footprint need measurement |
| Additional inference | ONNX Runtime, MIT | Each model, execution provider, conversion, and transitive library must independently qualify |

This list does not supply a timeline compositor, a validated desktop host, or a
complete replacement for GES. Reuse audited existing render-plan, graphics, and
GPU code where possible, and prove the missing composition/audio/timebase work
before estimating the port as a packaging task.

### Subsequent prerequisite outcomes

The completed [canonical core probe](../../../native/linux-core-probe/README.md)
uses the real `video-creater` library with default features disabled. Five locked
musl tests and a real CLI run persisted and reopened the complete typed fixture;
the default desktop feature list and dialog dependency remain enabled. This proves
the narrow reuse boundary, not a Linux editor, qualified dependency closure,
cross-platform project roundtrip, or macOS behavior.

The completed [codec runtime proof](../../research/2026-09-12-linux-codec-runtime-proof.md)
rebuilt pinned OpenH264 and libxaac inputs for static musl executables. OpenH264
encoded and decoded four deterministic frames, and the separate AAC encoder and
decoder completed the retained AAC-LC roundtrip. Final maps identified no GNU CRT,
libgcc, libstdc++, or glibc contributions and ELF inspection found only five
expected weak probes. Each map still contains 12 unqualified Rustup `.rlib`
archives and two transient rustc `.rcgu.o` inputs whose hashes were unavailable.
The AAC output is 580 samples longer than the input; that is only an output-length
measurement, not encoder-delay, priming, padding, gapless, or A/V-sync proof.

The completed [MP4 timing proof](../../research/2026-09-12-linux-mp4-timing-proof.md)
shows that `mp4` 0.14.0 exposes the progressive `ctts` and `stss` data needed for
the two table gaps in the earlier minimp4 candidate. Seven static-musl tests passed,
including table-level timing and preceding-sync selection, but the released crate
can panic on inconsistent sample tables. The result remains limited to a guarded
progressive subset; edit lists, fragmented MP4, real decoder reordering, full
malformed-input robustness, and end-to-end common MP4 remain open.

## Bundling and installation contract

The application contains versioned directories for executables, native libraries,
model runtimes, baseline weights, skills, templates/fonts, notices, and manifests.
Paths are relative in manifests and resolved under an immutable installed root.
Writable XDG data/cache/state directories hold projects, logs, job artifacts, and
optional downloaded models; the executable resolver never loads tools from them.

An installer may place app-owned files in a private system installation directory;
that still counts as bundled. It must not obtain application tools from distro
packages as a substitute for shipping them. Only approved platform dependencies
may be installer dependencies. Never install libraries into global search paths.

Build scripts dispatch using an explicit target triple. Shared hooks must not
invoke Swift, Xcode, Apple frameworks, or macOS signing on Linux. Verify recursively
all ELF interpreter/dependency entries and dynamically loaded modules. Reject
absolute build-machine paths, unresolved libraries, and unclassified dependencies.

Missing or corrupt bundled tools produce an in-app repair/reinstall diagnostic;
the application does not advise package-manager installation. Updates replace a
matched app/runtime manifest atomically and preserve user projects. Recovery must
never mix helpers from different protocol/runtime versions.

## Functional acceptance contract

The Linux product release requires the following, not merely successful compilation:

| Area | Required behavior |
| --- | --- |
| Editing | Import, trim, split, reorder, seek, preview, undo/redo, save, reopen, and media relink |
| Input | Common H.264/AAC MP4 plus documented WebM, WAV, and still-image fixtures; no host codec installation |
| Output | Valid MP4 H.264/AAC and WebM delivery on the CPU baseline; exact supported profiles documented |
| Graphics | Existing supported caption/layer/precompose behavior preserves timing, alpha, transforms, and visual review metadata |
| Local intelligence | Bundled baseline transcription with word timing; real EDL selection and caption remapping after cuts |
| Models | Per-feature speech analysis, enhancement, and semantic parity recorded; unavailable features cannot be claimed as full parity |
| Agent/MCP | Bundled clients launch, validate proposals through Rust, apply/reject, cancel, and recover after restart |
| Desktop | Native file operations, Ctrl shortcuts, focus, scaling, dialogs, notifications, and accurate platform labels |
| Credentials | Secure save/replace/delete/restart; no plaintext fallback, secrets in UI, or diagnostic leakage |
| Projects | macOS/Linux round-trip preserves canonical data, unknown capability references, originals, and source ranges |

The common MP4 input/output requirement is a feasibility gate, not a claim that a
complete permissive codec stack has already been selected. If it cannot be met,
the release remains blocked; changing the advertised format support requires an
explicit product decision. The same applies to a missing core editing capability.

Existing visual contracts remain mandatory: structured proposals include
`visualTreatment`, `motion`, `safeZone`, and `avoid`; visual layers follow a real
EDL and render review checks timing, legibility, and alpha over actual footage.

Secure-store unavailability must be reported without blocking offline editing.
Optional feature failure must not masquerade as a healthy subsystem. Linux health
must not depend on AVFoundation or Core ML being present.

## Verification and release gates

1. **Dependency feasibility:** first pass the desktop/libc/client-protocol proof above;
   then select exact media/model artifacts and prove their complete license closure.
   Demonstrate CPU MP4 decode/encode and word-timed
   transcription before committing to the replacement implementation.
2. **Contract tests:** verify proposal/state boundaries, protocol mismatch, denied
   paths, missing/corrupt helpers, unsupported profiles, cancellation, and restart.
3. **Real media tests:** measure fixture duration, streams, frame content, A/V sync,
   seek accuracy, cut boundaries, caption alignment, overlay alpha/timing, and
   output reopening with an independent validation implementation. Store tolerances
   and expected outcomes per fixture before recording a pass.
4. **Package isolation:** install on a clean supported desktop without developer
   tools, with sanitized PATH and no development library overrides. With network
   disabled, complete the baseline local workflow using shipped model weights.
   Trace executable and library resolution, including lazy-loaded paths.
5. **Desktop acceptance:** test the real installed WebView/native host on Wayland
   and X11, with software rendering and available hardware acceleration. Browser
   fixtures and headless tests do not prove native preview, dialogs, or credentials.
6. **Lifecycle:** verify upgrade, failed-update recovery, uninstall, retained user
   data, filesystem permissions, non-ASCII paths, and read-only installation roots.
7. **Release audit:** zero unclassified/denied payload items or libraries loaded
   by app-owned processes, including glibc and lazily loaded libraries; record
   independent OS service requirements separately; include signed release
   metadata, hashes, SBOM, notices, source revision, runtime manifest, and reports.
   Pin third-party CI actions to immutable SHAs.
8. **macOS regression:** run affected native and packaged checks on a Mac. Never
   infer macOS signing, media, or packaged behavior from Ubuntu results.

Report passed, failed, blocked, and unverified checks separately. A missing desktop
session, unavailable hardware, or incomplete license review cannot count as a pass.

## Decisions required before implementation planning

- **Desktop/runtime selection:** the user has resolved the license boundary:
  LGPL libraries linked/loaded by app-owned processes are forbidden, while
  independent OS services are permitted. Prove a replacement host, libc path,
  client protocols, and helper dependency closure. No candidate has yet passed
  the full gate; standard Tauri Linux is not an implementation option.
- **Permissive media feasibility:** a versioned decode/compose/encode/mux dependency
  set must pass the CPU format and license gates. There is no approved replacement
  for GES in this draft.
- **Model selection:** exact baseline weights, quality targets, alignment behavior,
  binary size, memory use, and redistribution terms require measured evidence.

These are explicit blocking decisions, not implementation TODOs that may be
silently resolved by adding forbidden dependencies or reducing advertised features.

## Review and evidence status

This document specifies requirements and a proposed direction. It does not claim
an implementable permissive-only Linux stack has been proven. No app build,
runtime test, package installation, codec benchmark, or dependency-closure audit
was performed to validate the proposed architecture during spec drafting.

Implementation preflight subsequently ran against checkout `d7cb4741` in an
isolated branch. The [desktop feasibility report](../../research/2026-09-12-linux-runtime-feasibility.md)
records a failure under the previous broader policy, which included dependencies
inside independent OS services. The user's option 1 selection supersedes that
service-based blocker. The report remains historical evidence, not the current
verdict for Ubuntu. The [inventory preflight](../../development/linux-permissive-preflight.md)
still applies unchanged to application libraries and helpers; it validates
declarations only. A bounded native-host prototype is authorized under the new
boundary. Passing it does not establish full editor, audio, Wayland, model, or
packaged-release compatibility.

Under option 1, the [X11 host spike](../../research/2026-09-12-linux-host-spike.md)
passed mapped-window, CPU pixel, synthetic input, direct file-access, and clean
quit checks on Xvfb. The [audio spike](../../research/2026-09-12-linux-audio-spike.md)
sent and recovered synthetic PCM through a private PulseAudio null sink and
monitor, with one underflow notification. Both tested clients were static musl
executables with retained ELF and live mapping evidence, not production app builds.
Neither graph received an `inventory-eligible` result; exact static toolchain
provenance remains open. The checker first added bounded recognition of
`LLVM-exception` terms so an explicitly declared MIT alternative could be selected,
while the unchanged initial six-ID policy still denied every selected exception-bearing
term. Following the explicit source-policy review, the checker now also treats the exact
atomic term `Apache-2.0 WITH LLVM-exception` as eligible. It continues to deny GPL,
LGPL, arbitrary base-license combinations, and unknown exceptions; this policy update
does not qualify either historical graph or establish source/binary provenance.
Independent source-policy, specification, and code review accepted the exact-term
implementation with zero findings after 31 focused and 40 combined tests passed.

The [X11 development harness](../../../native/linux-host-probe/README.md) now
keeps the probe source, lockfile and verification runner in Git. It verifies
the synthetic pixel/input/file lifecycle and captures ELF/live-map evidence
against a private Xvfb. Regression tests cover repeated input, startup failure,
and graceful CLI interruption without leaving owned children. This improves
reproducibility of the protocol proof; it does not promote that probe into the
production UI or resolve static toolchain licensing.

The [canonical core probe](../../../native/linux-core-probe/README.md) now consumes
the real Rust project/storage library with default features disabled. Commit
`e24a3896` moved the existing dialog dependency under the existing app-runtime
feature without changing the default feature list, and five locked musl tests plus
a real CLI run proved full typed fixture persistence. Its Rustup static inputs and
complete linked source closure remain unqualified; it does not prove a Linux editor,
package, user-project migration, cross-platform roundtrip, or Mac behavior.

The [client screening note](../../research/2026-09-12-linux-option1-clients.md)
identifies candidate native Wayland and audio routes. Next production-design
work must select a usable UI architecture and validate real desktop behavior,
Wayland, PipeWire, permissions, media codecs/composition, and packaging. The
obsolete independent-service blocker must not be used to stop that work, but
these narrow prototypes must not be presented as a complete Linux editor.

The subsequent [Wayland proof](../../research/2026-09-12-linux-wayland-spike.md)
used a static-musl `wayrs-client` against private headless Weston. It configured
an xdg-shell window, submitted one CPU buffer, received a frame callback, and
exited cleanly, with live mappings retained. The pixel dump is client-generated;
compositor screenshot readback, input, and real desktop acceptance are untested.
Exact compiler CRT provenance is still unresolved.

The [media feasibility report](../../research/2026-09-12-linux-media-feasibility.md)
is the historical baseline that found GNU startup objects in the AAC proof,
OpenH264's GNU C++ runtime dependency, and missing composition/sync tables in the
tested minimp4 source. The completed
[codec runtime proof](../../research/2026-09-12-linux-codec-runtime-proof.md)
found no identified GNU runtime contributions in the final maps and exercised
four H.264 frames plus separate AAC encode/decode paths. Its final maps still
leave 12 Rustup `.rlib` archives and two transient rustc objects per executable
unqualified, and the 580-sample AAC output-length difference does not measure
delay or gapless sync.

The completed [MP4 timing proof](../../research/2026-09-12-linux-mp4-timing-proof.md)
resolved the two minimp4 table gaps for a restricted progressive subset with
`mp4` 0.14.0. Its seven-test result explicitly catches a released-crate panic on
malformed inconsistent sample tables; edit lists, fragments, real codec reordering,
and complete malformed-input handling remain open. These prerequisite tasks passed
scoped independent review, but a complete production/common-MP4 workflow, full
runtime-source and payload closure, UI architecture, model workers, bundled agent
clients, packaging, real desktop acceptance, and Mac verification remain open.
