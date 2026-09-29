# Settings Operational Readiness

This guide describes the implemented Settings contract and the evidence currently retained for
it. [The parity tracker](parity.md) remains the canonical status record. The approved architecture
is in the [operational Settings design](superpowers/specs/2026-07-16-operational-settings-and-runtime-readiness-design.md),
and the integration work is in the [Settings integration and visual-QA plan](superpowers/plans/2026-07-16-settings-integration-and-visual-qa.md).

## User workflow

Settings owns six categories: General, Models, Agent & MCP, Skills, Storage, and Providers. Each
category badge is derived from live component health. **Check All** refreshes the complete
snapshot; a category action refreshes only its affected category so unrelated state is not
replaced by a loading placeholder.

For local models, use **Install**, **Retry**, **Verify**, or **Remove** in Models. The app downloads
open-source artifacts itself and publishes file/byte progress. Manual placement of compiled
Core ML bundles is not the normal workflow.

For destructive actions, Settings names the exact target. Model confirmation copy identifies the
model and states that other models and projects are unaffected. Storage cleanup identifies either
**disposable application cache** or selected **project render artifacts**, shows the exact paths
and byte count, and requires confirmation of that preview before any background removal starts.

## Model ownership and provenance

On macOS, Tauri resolves the app-managed model root to:

```text
~/Library/Application Support/com.olhapi.video-creater/models/
```

The active transcription model is stored beneath:

```text
nvidia__parakeet-tdt-0.6b-v3/fluid-audio-coreml/
```

Production speech-analysis models are separately scoped beneath:

```text
speech-analysis/production-v1/
```

Removing one scope cannot remove the other or project data. Both stores are created from the same
app-data model root; the app owns acquisition, staging, manifest publication, verification, and
removal.

### Parakeet TDT 0.6B v3

The transcription catalog uses this immutable source:

| Field | Value |
| --- | --- |
| Model ID | `nvidia/parakeet-tdt-0.6b-v3` |
| Hugging Face repository | `FluidInference/parakeet-tdt-0.6b-v3-coreml` |
| Revision | `aed02740059203c4a87495924f685de3722ae9ce` |
| License | `CC-BY-4.0` |
| Runtime | `fluid_audio_coreml` |
| Required artifacts | 18 exact relative paths |

App-managed download publishes into a staging directory and makes the model visible only after the
complete set passes verification. Its source-bearing installed manifest must identify the pinned
repository and revision, contain 18 unique safe relative paths, and provide a nonzero size plus
SHA-256 for every artifact. Verification rejects missing, extra-manifest, duplicate, traversal,
symlink, wrong-size, or hash-mismatched data. A failed or interrupted acquisition remains retryable;
retry replaces invalid files and verification can be run again without a network request.
Transcription acquisition owns a cancellation token and can be cancelled without publishing its
staged payload.

Verify can also recover a complete artifact tree whose manifest is missing, stale, or has invalid
source provenance. It hashes and sizes the exact catalog-required 18 files and writes a verified
source-neutral manifest (`source: None`) instead of inventing download provenance. Such a recovered
Ready state proves the local artifacts satisfy the catalog and runtime integrity contract; it does
not claim those files came from the pinned Hugging Face repository. App-managed downloads retain
their valid source-bearing repository, revision, and license metadata.

The final packaged-WebView acceptance downloaded the pinned payload through the signed app,
cancelled one attempt, force-terminated a second attempt, recovered it as
`settings.operation.interrupted`, retried, and reached Ready. The verified result contains
483,104,675 bytes across 18/18 files and 18 unique manifest paths with strong size/SHA-256 metadata.
The retained checkpoint also proves the transcription and speech-analysis readiness lanes remain
separate.

### Production speech analysis

The production Silero VAD and speaker-diarization model set uses the shared
`speechModelsDownload` operation lane. Install returns a persisted queued operation immediately,
then reports verified files and bytes. Verify checks every pinned artifact; Retry replaces invalid
files; Remove is limited to `speech-analysis/production-v1`.

This lane is intentionally non-cancellable today. The existing multi-file downloader has no safe
cancellation ownership boundary, so Settings explains that constraint instead of displaying a
cancel control that cannot be honored. If the app stops, startup reconciliation marks an unfinished
operation interrupted unless the complete target verifies as ready, and the user can retry.

## Required render and delivery runtimes

GStreamer with GES is required application infrastructure, not an optional fallback. It owns
timeline composition, selected-range and draft renders, graphics frame-sequence composition,
provider-input renders, compatibility/media paths, and WebM delivery. Production packages include
the reviewed ARM64 runtime and curated plugin manifest; Settings reports both runtime readiness and
plugin-policy readiness.

AVFoundation is an independently checked final-delivery exporter for supported macOS H.264, H.265,
and ProRes profiles. An AVFoundation failure can make those delivery choices unavailable without
claiming GStreamer failed. Conversely, AVFoundation availability cannot make the app composition-
ready when the required GStreamer/GES runtime is unavailable.

Runtime verification checks manifest inventory, required factories, GStreamer/GES initialization,
dependency closure, relocated RPATHs, architecture, signatures, and live capability/smoke probes.
The final release evidence at `output/settings-readiness/final-gate/` records GStreamer and GES
1.28.2, 71 runtime files, 34 plugins, 46 factories, 69 runtime Mach-O files, a passing real
GES-timeline probe on its first attempt, zero package-manager dependencies, and 129 packaged Mach-O
entries with the same Developer ID authority and team. Earlier retained workflow evidence also
completed a real two-clip/five-graphic GES WebM render and AVFoundation H.264/AAC, HEVC/AAC, and
ProRes/PCM renders.

The exact final app and DMG are Developer ID-signed with a secure timestamp and hardened runtime.
Apple accepted both notarization submissions, both artifacts have stapled tickets, strict deep
verification passes, and Gatekeeper accepts the app and DMG as Notarized Developer ID. Production
builds explicitly enable Tauri's `custom-protocol` feature so the signed app embeds its frontend;
development builds deliberately exclude that feature and retain the Vite `devUrl` workflow. This is
verified release evidence on the provisioned macOS host, not a claim of an independent clean-Mac
installation.

## Health and operation contracts

`SettingsHealthSnapshot` contains `generatedAt`, an overall state, and a map of category records.
Each category has `id`, aggregate `state`, and component `items`. Each item contains a stable ID and
label, state, user summary, optional action ID/label, check time, optional diagnostic code/detail,
and non-secret provenance. Valid health states are `ready`, `actionRequired`, `checking`, `failed`,
and `unavailable`; category severity is derived from its items rather than optimistic page copy.

Long-running or auditable work uses the shared operation contract. Operations are persisted at:

```text
~/Library/Application Support/com.olhapi.video-creater/settings/operations.json
```

The kinds are `modelDownload`, `speechModelsDownload`, `healthCheck`, `skillRepair`,
`storageRefresh`, `storageCleanup`, and `providerRefresh`. Lifecycle states are `queued`, `running`,
`cancelling`, `succeeded`, `failed`, and `cancelled`. The record also includes target, phase,
completed/total units, unit, cancellation capability, message, timestamps, and a structured error
with stable diagnostic code, recovery action, and optional detail.

The backend publishes every accepted transition on the `settings-operation` Tauri event. The UI
merges events, targeted refreshes, polling results, and recent journal entries without allowing an
older response to overwrite a newer operation version. Startup reconciles queued/running work:
completed verified targets become succeeded; incomplete work becomes a retryable
`settings.operation.interrupted` failure. Journal corruption is preserved for diagnosis and
recovered fail-safe; persistence failures project visible structured warnings rather than leaving a
false running state. Raw credentials are never operation fields.

Diagnostic codes are stable machine-facing identifiers such as `render.runtime.ges_init_failed`,
`settings.operation.interrupted`, `speechModels.verification.failed`, or
`agent.proposalValidator.stateMutation`. The adjacent summary and recovery action are the
user-facing guidance; users should not be told to copy an internal repair command.

## Storage safety

Inventory is read-only across global models, disposable app cache, project media, transcripts,
render artifacts, and workflow artifacts. A project must be the active trusted project before its
storage is considered for cleanup or Finder reveal.

The only cleanup targets are:

- `disposableAppCache`, limited to audited app-owned cache producers; and
- selected direct child directories under the active project's render-artifact root.

Global models, project media, transcripts, workflow artifacts, and whole project folders are
protected from generic cleanup. Preview binds exact paths, sizes, file identities, content
fingerprints, and the active-project generation into a one-use confirmation token. Traversal,
symlinks, duplicated identities, project switches, content changes, replayed tokens, or scope escape
fail closed. Cleanup runs under the shared storage/project mutation coordinator and reports partial
progress or quarantined paths if a persistence boundary becomes uncertain.

## Provider credentials

Providers use independent accounts in the macOS login Keychain service
`com.olhapi.video-creater.provider-credentials`. Saving replaces that provider's existing value;
Remove deletes only that value. The Settings DTO returns provider, display name, canonical
environment-variable name, configured state, and source. It never returns the secret.

Rust resolves a valid Keychain value first. If it is missing or unavailable, it may read the
provider's canonical environment variable as a compatibility fallback: `FAL_KEY`,
`REPLICATE_API_TOKEN`, `OPENAI_API_KEY`, `XAI_API_KEY`, `ELEVENLABS_API_KEY`, `GEMINI_API_KEY`, or
`MINIMAX_API_KEY`. Environment fallback is read-only; Settings does not overwrite or delete the
process environment. Removing a Keychain value may therefore reveal an existing environment
fallback as the current source.

Resolved credentials deliberately implement neither serialization nor debug formatting. Raw values
must not enter webview DTOs, projects, workflow requests, Settings journals, logs, diagnostics, or
evidence reports.

## Agent, MCP, validator, and skills

Agent & MCP checks three components independently so one failure does not turn the whole page into a
placeholder:

- Codex app-server launches `codex app-server --stdio` and validates its bounded initialize
  handshake.
- The bundled MCP server is probed separately, with project-aware client configuration and bounded
  process cleanup.
- The proposal validator must accept an EDL-first structured fixture, reject invalid visual timing,
  and leave the canonical project unchanged.

Skills verifies only the mandatory project files: Edit planning and render pipeline, Video graphics
and overlays, and Editor interface and visual QA. Checks compare each exact project path and bundled
checksum and confirm prompt inclusion. Repair first previews affected paths, then replaces only the
confirmed skill files while retaining backups; it does not rewrite arbitrary repository content.

## Developer verification

Run the service-backed Settings E2E:

```bash
rtk pnpm e2e:settings
```

Run all deterministic Settings browser states or the full release comparison:

```bash
rtk pnpm visual:qa:browser -- --only settings-state
rtk pnpm visual:qa:browser-release
```

Verify the bundled GStreamer/GES runtime and smoke composition:

```bash
rtk pnpm verify:gstreamer-runtime
```

For a release, first verify signing/notarization credentials, then build:

```bash
rtk pnpm release:macos:preflight
rtk pnpm release:macos
```

The release script requires a valid installed Developer ID Application identity. It reads
`APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`, `APPLE_ID`, and `APPLE_PASSWORD`; identity/team can be
derived when unambiguous, and Apple ID plus app-specific password can instead come from the login
Keychain service `video-creater-notary`. Preflight validates Apple notarization authentication before
building. A successful release must still pass notarization, stapling, Gatekeeper, dependency,
runtime, helper, and evidence-policy checks. The release command also launches the exact signed app
twice under an isolated HOME, application-support directory, temporary directory, project, and
Keychain with `PATH=/usr/bin:/bin:/usr/sbin:/sbin`. It cannot write a passing release report until
the pre-restart 2/2 and post-restart 7/7 packaged Settings checkpoints pass, the app bundle digest is
unchanged, provider canaries are absent, the release process is gone, and the user's original
Keychain default and search list are restored exactly. Signing and this packaged interaction gate
remain intentionally separate from `release:macos:preflight`.

## Evidence boundary and current gate

Evidence types are deliberately separate:

| Evidence | What it proves | What it does not prove |
| --- | --- | --- |
| Deterministic browser fixtures | Responsive Settings states, actions, progress, errors, and accessibility in a browser harness | Native Tauri/WebView, dialogs, Finder, Keychain, or package behavior |
| `rtk pnpm e2e:settings` | Local Rust service contracts, operations, redaction, scoped repair/cleanup, and retained safe reports | A user clicking through the packaged UI |
| Signed packaged non-UI runners and real renders | Bundle discovery, helpers, signatures, RPATH/dependency closure, runtime smoke, strict model reverify, and render outputs | Packaged WebView interaction or native dialog/Keychain UI |
| Automated packaged interactive acceptance | Six real Settings tabs, download cancel/restart recovery/retry, model Ready, safe cleanup, isolated Keychain replace/delete/non-readback, and Agent/MCP/skills | Native picker and Finder UI interaction, independent clean-Mac behavior, or unrelated editor workflows |
| Manual packaged native-action acceptance | Native project-folder picker persistence and Finder reveal behavior in the exact signed app | Automated repeatability, independent clean-Mac behavior, or unrelated editor workflows |
| Release/notarization | Exact final app and DMG signed, Apple-accepted, stapled, strict-verified, and Gatekeeper-accepted | Cross-platform or independent clean-Mac acceptance |

The release automation now repeats the hash-checked two-launch checkpoint flow for every signed
release and refuses to report success if it fails. It never runs more than one acceptance instance.
Its temporary HOME uses an isolated unlocked Keychain for the provider round-trip; cleanup restores
and verifies the original login-Keychain configuration before retaining sanitized evidence. The
native picker and Finder reveal remain explicit manual native-action acceptance because the
checkpoint runner does not synthesize clicks in macOS system UI. Retained prior-release evidence
shows those two manual actions passing without closing the pre-existing Finder window, but that
historical proof is not reused for a new source commit.

At the final Task 6 gate, lint passed and 1,901 frontend tests passed across 92 files. The serial Rust
run reported 445 passed, three reproducible host-environment failures, and one ignored test. Settings
E2E passed. The GStreamer verifier now retains structured evidence for its bounded timeout retry and
the final 71-file GES smoke passed on attempt one. The literal signed/notarized release command exited
zero. Visual manifest integrity passes 73/73, scoped Settings desktop/narrow baselines have zero
mismatch, and all 34 modern editor scenarios pass. The repository-wide visual comparison remains
non-green only because 15 unrelated pre-modern baselines are stale; they were not rewritten as part
of Settings readiness.

Evidence references:

- `output/settings-readiness/integration/` — native Settings E2E artifacts.
- `output/settings-readiness/packaged-app/` — sanitized packaged WebView, model, restart, cleanup,
  Keychain, picker, Finder, agent/MCP/skills, and screenshot evidence.
- `output/settings-readiness/final-gate/` — final signed/notarized release, runtime, regression, and
  visual-gate summary; each new release directory also contains
  `settings-packaged-acceptance.json` from the exact signed app.
- `output/settings-readiness/models/verification.md` — focused model implementation verification.
- `output/settings-readiness/storage-providers/verification-summary.md` — focused storage/provider
  service verification predating the final packaged interaction proof.
