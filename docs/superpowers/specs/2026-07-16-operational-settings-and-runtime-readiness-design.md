# Operational Settings And Runtime Readiness Design

## Context

Video Creater's Settings window has the correct category-led shell but several categories expose implementation notes instead of complete product workflows.

The most visible failure is transcription model setup. The Rust model store already supports downloading the pinned Parakeet Core ML artifacts from Hugging Face, staging them in the global model store, verifying them, cancelling an active download, and removing them. The React application imports `downloadTranscriptionModel`, but `App` never passes a download action to `ModelSettings`, and the model row renders only `Verify` for a missing model. The resulting UI incorrectly tells the user to copy compiled bundles into an application-support directory.

Other settings have the same product-level problem:

- Agent & MCP is a static explanation with no health check or setup action.
- Skills displays raw internal identifiers without proving that the mandatory context is available.
- Storage uses a free-text value that looks like a folder policy but does not provide a folder picker, usage, free-space state, or cleanup.
- Export runtime settings expose Homebrew and pnpm commands to ordinary users.
- General offers an update preference although no updater runtime is configured.
- Long-running model actions report failures only through `console.error`.

The render-runtime explanation is also inaccurate. GStreamer with GES remains required for timeline composition, proposal and draft renders, graphics frame-sequence composition, provider-input renders, media discovery, compatibility paths, and WebM output. AVFoundation is a specialized bundled macOS exporter preferred for supported H.264, H.265, and ProRes delivery exports. Development builds enable the `ges-render` feature, but the current macOS release command explicitly removes it. Settings calls GStreamer a fallback while release packaging can omit a runtime used by active workflows.

The approved direction is one operational Settings surface with live state, direct in-app actions, progress, narrow recovery, and truthful runtime ownership.

## Decision

Keep the existing six-category Settings information architecture:

- General
- Models
- Agent & MCP
- Skills
- Storage
- Providers

Convert every category into an operational control surface backed by Rust-owned health and action contracts.

GStreamer/GES becomes required bundled application infrastructure. Production releases must include and enable the reviewed runtime and curated plugins. AVFoundation remains the preferred native delivery encoder for supported macOS H.264, H.265, and ProRes profiles.

Read-only content remains only when it proves live state, explains a blocked action, records provenance or licensing, or provides diagnostics. Raw commands, internal paths, environment details, and skill identifiers move behind an expandable diagnostics disclosure.

## Goals

- Let a user install every supported open-source local model from Settings without manually placing files.
- Show progress, cancellation, verification, retry, removal, source, revision, license, and disk use for model assets.
- Make each Settings category report live ready, action-required, checking, failed, or unavailable state.
- Provide direct actions for every recoverable state.
- Persist or recover long-running operation state across Settings navigation and application relaunch.
- Bundle and validate the required GStreamer/GES renderer in production builds.
- Keep AVFoundation as the preferred native delivery encoder for supported macOS formats.
- Replace developer-facing runtime commands with in-app diagnostics and repair.
- Make update, notification, storage, agent, skill, and provider controls truthful.
- Preserve Rust ownership of validation, canonical project mutation, secrets, runtime policy, and filesystem safety.
- Preserve the current compact professional Settings visual language and accessible category navigation.

## Non-Goals

- Do not replace GStreamer/GES with a new compositor in this work.
- Do not download GStreamer after first launch as a normal setup step.
- Do not add a general model marketplace or unpinned arbitrary model sources.
- Do not expose provider secret values to the webview.
- Do not let Settings modify canonical project JSON directly.
- Do not add an account, subscription, or bundled-credit system.
- Do not silently move existing projects, global models, or canonical render artifacts.
- Do not make Temporal a requirement for local desktop editing or MCP health.
- Do not claim automatic updates are supported until a signed updater and trusted release endpoint exist.
- Do not retain placeholder controls merely to fill a category.

## Product Principles

### Operational, Not Explanatory

Each visible settings group must answer:

1. What is the current state?
2. Does the user need to act?
3. What in-app action completes or repairs it?
4. What is the progress or result?
5. Where can technical detail be inspected if the action fails?

If a group cannot answer those questions and has no editable preference, it does not belong in the primary Settings content.

### Truthful Runtime Ownership

Settings describes runtime roles as follows:

- **GStreamer/GES render engine:** required timeline composition, draft and compatibility runtime.
- **AVFoundation delivery exporter:** preferred macOS H.264, H.265, and ProRes final encoder when the selected profile, quality, and source set are supported.
- **Compatibility decoder:** bundled isolated decoder for approved unsupported-source preparation.
- **FluidAudio/Core ML transcription:** required native runtime for the active local transcription model on supported macOS systems.

An unavailable AVFoundation exporter must not mark GStreamer unavailable. An unavailable GStreamer/GES runtime must mark affected composition and draft workflows blocked even when AVFoundation can encode a simple final export.

### Safe Recovery

Repair actions must be scoped to the affected asset or runtime. A generic repair action cannot delete project media, model installations, transcripts, render outputs, or canonical project files.

## Shared Settings Health Contract

Rust exposes one snapshot command and focused action commands. The frontend does not infer readiness from file paths, copied commands, or unrelated worker status.

```rust
pub struct SettingsHealthSnapshot {
    pub generated_at: String,
    pub overall: SettingsHealthState,
    pub models: ModelsHealth,
    pub render_system: RenderSystemHealth,
    pub agent: AgentIntegrationHealth,
    pub skills: SkillsHealth,
    pub storage: StorageHealth,
    pub providers: ProvidersHealth,
    pub updates: UpdateHealth,
    pub notifications: NotificationHealth,
}

pub enum SettingsHealthState {
    Ready,
    ActionRequired,
    Checking,
    Failed,
    Unavailable,
}
```

Every health item includes:

- stable component ID;
- user-facing label;
- state;
- concise summary;
- optional action ID and label;
- last checked timestamp;
- optional diagnostic code and detail;
- optional provenance such as version, revision, source, license, or path.

The initial command is `get_settings_health_snapshot`. Focused commands may refresh one category without repeating expensive network or runtime checks. The frontend refreshes the affected category after every action and offers `Check all systems` from General.

Health checks must be bounded. A hung external process, Keychain request, model source, or runtime probe cannot freeze Settings.

## Long-Running Operation Contract

Model downloads, runtime checks, skill repair, cache cleanup, and future updater actions use a common operation shape:

```rust
pub struct SettingsOperation {
    pub id: String,
    pub kind: SettingsOperationKind,
    pub target_id: String,
    pub phase: String,
    pub state: SettingsOperationState,
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub unit: Option<String>,
    pub cancellable: bool,
    pub message: String,
    pub error: Option<SettingsOperationError>,
    pub started_at: String,
    pub updated_at: String,
}
```

Commands that start long work return the operation immediately rather than holding the webview request until completion. Rust stores active operation metadata and emits typed progress events. The frontend also polls on mount as a recovery path.

Rust persists a small operation journal under application support. On process startup, an operation left queued, running, or cancelling by the previous process is reconciled against the target store. It becomes succeeded when the target is already valid, or failed with an `Interrupted` recovery action when work did not finish. Model retry may reuse completed staged files when their hashes still match; otherwise it restarts acquisition safely. The UI never presents an operation from a dead process as still running.

Operation states are:

- queued;
- running;
- cancelling;
- succeeded;
- failed;
- cancelled.

An error includes a stable code, a user-facing message, a recovery action, and optional technical detail. Console logging can supplement this contract but cannot be the only error surface.

## Models

### Catalog And Empty State

The Models page always renders catalog entries returned by Rust, even when no model is installed. `models.length === 0` means the catalog failed or the build exposes no supported models; it must not be presented as "No transcription models installed."

The page groups:

- Transcription;
- Speech analysis;
- Generation model availability preferences.

### Transcription Models

For Parakeet TDT 0.6B v3:

- `Download model` calls the existing pinned Hugging Face acquisition path.
- The source remains `FluidInference/parakeet-tdt-0.6b-v3-coreml` at the catalog-pinned immutable revision.
- Files install under the global app-managed model root shared by all projects.
- Download uses a staging directory and never exposes a partial installation as ready.
- Progress includes files and bytes where the provider supplies both.
- The user can cancel an active download.
- A cancelled or failed download can retry safely.
- Successful acquisition automatically verifies the manifest and Core ML layout.
- `Verify` remains available for an installed model.
- `Remove` requires confirmation and affects only that model.
- Manual folder import is available under Advanced recovery through a native folder picker.

Primary copy uses user language:

- `Download model`
- `Downloading 214 MB of 485 MB`
- `Checking model`
- `Ready for transcription`
- `Download failed`
- `Retry download`

Primary copy must not say "copy the compiled Core ML bundle" or require the user to inspect the global model path.

The diagnostics disclosure shows:

- model ID;
- Hugging Face repository and revision;
- artifact format;
- local path;
- required and installed file counts;
- installed bytes;
- verified timestamp;
- license;
- runtime ID;
- last error code and detail.

### Speech Analysis Models

The existing Silero VAD and FluidAudio diarization set uses the same visual and operation contract:

- install;
- progress;
- cancel if supported by the store;
- verify;
- retry;
- remove;
- pinned revisions and licenses.

The page must not make transcription appear ready when only speech-analysis models are installed, or vice versa.

### Runtime Readiness

Model installation and runtime readiness remain separate:

- installation proves model artifacts;
- runtime readiness proves the bundled FluidAudio helper can load the active model on the current platform.

The page shows both states and gives a targeted retry check when the helper is unavailable.

## Agent & MCP

Agent & MCP reports three independent components:

### Codex App-Server

The health check:

- resolves the configured `codex` executable;
- launches `codex app-server --stdio` with a bounded timeout;
- performs the initialize handshake;
- terminates the probe process;
- reports ready, missing executable, incompatible protocol, timeout, or launch failure.

The action is `Run app-server self-test`. If Codex is missing, Settings explains that Codex must be installed and provides a documentation link only when the application has a trusted configured URL. It does not expose a shell command as the main recovery.

### Video Creater MCP Server

The release bundle includes the `video-creater-mcp-server` executable. The health check launches it against a temporary valid project fixture, performs an MCP initialize request, validates the protocol response, and terminates it.

When a project is open, Settings shows project-aware connection status and offers `Copy client configuration` for supported clients. When no project is open, it states that project-specific configuration is available from the editor and offers `Open a project`.

MCP health must not display Temporal endpoint or worker status. Temporal remains a separate execution-backend concern.

### Proposal Validation

The Rust proposal validator reports built-in readiness through a deterministic self-test using a valid and invalid fixture. This proves that structured project actions are accepted or rejected at the canonical mutation boundary.

Technical protocol versions, executable paths, fixture details, and raw responses stay under diagnostics.

## Skills

The primary Skills page shows friendly capability names:

- Edit planning and render pipeline;
- Video graphics and overlays;
- Editor interface and visual QA.

Each row proves:

- the mandatory skill exists;
- its content can be loaded;
- the compiled or bundled canonical copy has a known checksum;
- project context uses the expected skill version;
- the app-server prompt includes the mandatory skill contract where required.

The existing raw names and `.agents/skills/.../SKILL.md` paths move into diagnostics.

`Verify skills` reruns all checks.

`Repair bundled skills` is available only when a skill is missing or differs from the bundled canonical copy. Repair:

- shows the affected files;
- requires confirmation;
- backs up an existing differing file;
- materializes the bundled canonical content atomically;
- reruns verification.

Repair does not overwrite unrelated project instructions or custom skills.

## Storage

### Project Location

Replace the free-text `Default project storage location` pseudo-policy with:

- `Ask for a folder` as the default;
- `Use suggested parent folder` with a native folder picker;
- the selected absolute parent path when configured.

Projects remain split project folders. The setting controls the initial folder suggested for New Project; it does not relocate existing projects.

### Storage Inventory

Rust reports:

- global model storage;
- disposable application caches;
- current project media;
- current project transcripts;
- current project render artifacts;
- current project workflow/log artifacts;
- available free space for every relevant volume.

When no project is open, project-scoped rows say `Open a project to inspect`.

### Actions

The page offers:

- `Choose folder`;
- `Reveal in Finder`;
- `Refresh usage`;
- `Clear disposable cache`;
- model-specific removal through Models;
- render-artifact cleanup scoped to the open project with explicit selection and confirmation.

`Clear disposable cache` lists exactly what will be removed and cannot delete:

- imported project media;
- generated project media;
- transcripts;
- model installations;
- canonical project manifests or timeline files;
- accepted render outputs.

Free-space failures identify the volume and the operation that is blocked.

## Providers

Keep the existing macOS Keychain boundary and no-readback secret contract.

Improve the page by grouping providers into:

- Configured;
- Needs attention;
- Optional.

Each provider shows:

- credential source: Keychain, environment fallback, or missing;
- live account validation state;
- account name or balance when supported;
- enabled generation models that depend on the provider;
- last checked timestamp.

Actions remain:

- Save;
- Replace;
- Remove Keychain credential;
- Retry validation;
- Refresh all.

Rejected credentials remain in the editable draft field and show the provider error inline. Environment values remain read-only fallbacks and cannot be removed from Settings.

Generation model preferences remain on Models, but disabling or enabling a model updates provider dependency summaries.

## General

### Privacy

Retain confirmation before provider uploads. Copy names the boundary precisely: local media stays local until the user approves a provider-backed request that requires an upload.

### Notifications

Render-completion notifications remain configurable only when notification permission and the native delivery path are available. General shows permission state and provides `Enable notifications` or `Open System Settings` when action is required.

### Updates

Remove the current inert update-policy selector.

Until a signed updater runtime and trusted update endpoint are configured, General shows:

- installed application version;
- `Updates are not configured for this build`;
- no pretend notification toggle.

When updater capability is added later, the same health contract can expose `Check for updates`, available version, download progress, signature verification, and install/relaunch. This design does not invent an update endpoint.

### Render System

General shows a concise render-system summary:

- GStreamer/GES composition runtime;
- curated plugin set and policy;
- AVFoundation delivery exporter;
- compatibility decoder;
- supported delivery formats and quality levels.

Actions:

- `Check render system`;
- `Show format details`;
- `Open diagnostics`.

There are no `Copy repair command` or `Copy verify command` actions.

## Required GStreamer/GES Production Runtime

### Packaging

Production releases must enable the `ges-render` Cargo feature.

Add a dedicated render-runtime packaging flow that reuses the proven mechanics of the compatibility-runtime builder without conflating the two runtime roles. It bundles:

- required GStreamer and GES shared libraries;
- the reviewed core, base, good, and explicitly approved bad-plugin libraries;
- required runtime support libraries;
- a machine-readable runtime manifest;
- license and notice files;
- source and replacement instructions required by the LGPL obligations.

The bundle lives under a stable application resource directory such as:

```text
Video Creater.app/Contents/Resources/render-runtime/
  lib/
  plugins/
  manifest.json
  licenses/
```

Build tooling:

- stages only allowlisted libraries and plugins;
- rejects unreviewed or denied factories;
- rewrites Mach-O install names and rpaths to application-relative paths;
- rejects Homebrew and `/usr/local` references in the final bundle;
- signs every nested Mach-O with the release identity;
- records versions and hashes in the manifest.

### Runtime Resolution

Packaged builds resolve the bundled render runtime first and do not depend on Homebrew.

Development builds may fall back to configured or standard local GStreamer locations. The diagnostics report identifies whether the runtime is bundled or development-local.

The renderer and availability probe must use the same environment and registry. A Settings probe cannot report ready against a different plugin path from the actual render job.

### Plugin Policy

The existing fail-closed plugin policy remains authoritative. Packaging does not broaden the allowlist implicitly. Any new plugin or factory requires:

- license review;
- policy entry;
- packaging test;
- runtime probe;
- real render evidence.

### AVFoundation Relationship

AVFoundation remains bundled as a signed sidecar and is selected for supported macOS final project exports when:

- the selected format maps to a native profile;
- the requested quality is supported;
- the source and graphics request can be represented;
- the capability handshake succeeds.

GStreamer/GES remains available for composition, drafts, WebM, provider inputs, compatibility, and supported fallback cases. Settings reports the two engines separately.

## Frontend Architecture

Split the current monolithic `model-settings.tsx` into focused components:

- `settings-shell.tsx`;
- `general-settings.tsx`;
- `models-settings.tsx`;
- `agent-mcp-settings.tsx`;
- `skills-settings.tsx`;
- `storage-settings.tsx`;
- `providers-settings.tsx`;
- shared health, operation, diagnostic, and confirmation components.

The shell owns:

- category navigation;
- desktop and narrow layout;
- snapshot loading;
- category badges;
- global refresh;
- operation event subscription.

Category components receive typed state and callbacks. They do not invoke Tauri directly except through focused library adapters.

Visual rules:

- preserve the 220-pixel desktop rail and horizontal narrow rail;
- keep compact headings and controls;
- use flat sections separated by borders;
- use cards only for repeated model or provider items when framing materially helps;
- never nest cards;
- put the primary recovery action beside the affected status;
- keep diagnostics collapsed by default;
- show progress near the operation target;
- use accessible live regions for status changes;
- preserve keyboard category navigation and focus after actions.

## Error Handling

- Model source unavailable: preserve installed model state, keep staged data isolated, and offer retry.
- Model hash or manifest mismatch: mark failed, never ready, and offer redownload or advanced import.
- Download cancellation: terminate ownership of the active operation and leave no ready manifest.
- App-server timeout: terminate the probe child and report timeout without blocking Settings.
- MCP server missing from bundle: mark release health failed.
- Skill mismatch: show verify failure; repair only the selected bundled skill after confirmation.
- GStreamer library or plugin missing: mark composition runtime failed and block affected render actions with a route to Settings.
- GStreamer policy denial: identify the denied factory; do not offer a broad bypass.
- AVFoundation unavailable: mark native delivery formats or qualities unavailable without marking GStreamer failed.
- Folder permission failure: retain the previous setting and show the rejected folder.
- Insufficient disk space: show volume, available bytes, and required bytes.
- Keychain unavailable: preserve credential drafts and offer retry.
- Notification permission denied: keep notifications off and route to System Settings.
- Updater unavailable: show truthful unavailable status and no nonfunctional preference.

## Testing

Implementation uses test-driven development.

### Rust Tests

- Settings health snapshot combines independent component states without conflating failures.
- Health checks are bounded and child processes are terminated.
- Model catalog remains visible when the model is missing.
- Parakeet download uses the pinned Hugging Face source.
- Download progress, cancellation, retry, staging, verification, and removal state transitions.
- Active model download state can be recovered after the frontend remounts.
- Speech-analysis model operations use the same state contract.
- FluidAudio runtime readiness remains separate from installation readiness.
- Codex app-server initialize self-test success, missing binary, protocol error, and timeout.
- Bundled MCP server initialize self-test.
- Proposal validator valid/invalid fixture self-test.
- Skill checksum verification and narrowly scoped atomic repair.
- Storage inventory and free-space checks.
- Cache cleanup deletion whitelist.
- Provider health summaries do not serialize secrets.
- Notification and update capability reports are truthful.
- Render-system health distinguishes GStreamer/GES, AVFoundation, and compatibility decoder.
- Packaged runtime resolution does not use Homebrew paths.
- Plugin policy remains fail closed.

### Frontend Tests

- Missing transcription model renders `Download model`.
- Download action calls the correct adapter.
- Download progress, cancellation, failure, retry, verification, ready, and removal states.
- Errors appear inline rather than only in the console.
- Empty catalog and missing installation are distinct.
- Agent & MCP components render separate health and self-test actions.
- Temporal status is absent from MCP health.
- Skills show friendly names and diagnostics contain exact identifiers.
- Storage uses a folder picker action rather than a free-text pseudo-policy.
- Cleanup confirmation lists scope.
- Providers group configured, needs-attention, and optional entries.
- General removes the inert update-policy selector.
- Render system contains no copied shell commands.
- Category badges and global refresh update correctly.
- Desktop and narrow category navigation preserve roving focus.
- Status changes use accessible live regions.

### Packaging And Integration Tests

- Build the production application with `ges-render`.
- Verify the main executable and nested runtime libraries have only application-relative or system dependencies.
- Verify every bundled Mach-O is signed.
- Verify runtime manifest hashes and license files.
- Launch the packaged app on a clean macOS environment without Homebrew.
- Run a real GStreamer/GES selected-range draft with video, audio, and a graphics layer.
- Run a provider-input range render.
- Probe representative media through the packaged runtime.
- Run a WebM export.
- Run AVFoundation H.264, H.265, and ProRes exports.
- Verify output duration, nonzero streams, expected dimensions, artifact paths, and logs.
- Verify a missing or tampered required runtime causes release verification to fail.

### Visual QA

Capture and inspect:

- all six categories at desktop width;
- all six categories at narrow width;
- model missing, downloading, failed, and ready states;
- GStreamer failed with AVFoundation ready;
- GStreamer ready with AVFoundation failed;
- provider rejected credential;
- storage permission and low-space failures;
- expanded diagnostics;
- long paths, translations, and large progress values.

Check overflow, truncation, focus visibility, keyboard access, disabled states, live announcements, and reduced motion.

## Implementation Slices

### Slice 1: Shared Health And Model Completion

- Introduce shared health and operation contracts.
- Wire the existing transcription download command into the UI.
- Add real progress, cancellation, retry, verification, and inline errors.
- Bring speech-analysis models onto the same interaction contract.
- Split Models from the monolithic Settings component.

### Slice 2: Required Render Runtime And General

- Bundle the GStreamer/GES render runtime.
- Enable `ges-render` in production releases.
- Add render-system health and packaged runtime verification.
- Remove terminal repair commands.
- Replace inert updater and notification controls with capability-backed state.

### Slice 3: Agent, MCP, And Skills

- Add bounded app-server, MCP server, validator, and skill checks.
- Bundle the MCP server binary.
- Add friendly skill presentation and safe repair.

### Slice 4: Storage And Providers

- Add native folder selection, storage inventory, free-space state, reveal, and safe cleanup.
- Group provider state and show model dependencies.
- Preserve Keychain and no-readback guarantees.

### Slice 5: Integration And Visual QA

- Complete clean-machine packaged verification.
- Complete real draft/final render evidence.
- Run Settings desktop/narrow visual QA.
- Remove remaining placeholder copy and stale tests.

Each slice must leave Settings truthful. A category may show `Not available in this build` while its slice is unfinished, but it may not expose a control that pretends to work.

## Acceptance Criteria

- A missing Parakeet model can be downloaded from Hugging Face entirely inside Settings.
- Model actions show progress, cancellation, verification, retry, removal, errors, revision, license, and disk use.
- No ordinary model setup requires manual file placement.
- GStreamer/GES is explicitly identified as required composition infrastructure.
- Production releases include `ges-render` and a signed, reviewed, license-compliant bundled runtime.
- A clean packaged app performs real GStreamer/GES draft rendering without Homebrew.
- AVFoundation remains the preferred supported macOS delivery exporter and reports health independently.
- Agent & MCP proves app-server, MCP server, and validator health independently.
- MCP health does not use Temporal worker status.
- Skills proves mandatory context and offers narrowly scoped repair.
- Storage uses native folder selection and exposes safe usage and cleanup.
- Provider credentials remain in Keychain, secrets are never read back, and failures remain actionable.
- General contains no inert update setting and no copied developer commands.
- Every category covers loading, ready, action-required, failed, and unavailable states.
- Long-running actions restore their current state after navigation and become a truthful succeeded or interrupted/retryable state after relaunch.
- Settings remains usable at desktop and narrow widths with keyboard and screen-reader support.
