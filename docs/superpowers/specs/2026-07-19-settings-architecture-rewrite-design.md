# Settings Architecture Rewrite Design

## Context

Video Creater's current Settings window mixes five different kinds of product
surface:

1. application preferences;
2. project configuration;
3. local resource installation;
4. provider credential setup;
5. runtime diagnostics and repair.

This mixture makes ordinary context look unhealthy. With no project open,
project-only Skills and Storage rows report `Unavailable`. Optional remote
providers report `Failed` because every catalog model is enabled by default.
Agent components can be ready in the page while the category badge remains
failed because the shell and page own separate health requests. General reports
ready while rendering update and notification warnings. One category refresh
failure can replace every page with a global alert.

The native application audit on 2026-07-19 also found:

- a working in-app Hugging Face transcription download path hidden inside a
  diagnostics-shaped Models page;
- a blank Project Skills page that reports action required without identifying
  an affected skill or repair;
- a large generation-model checkbox grid that consumes most of Providers;
- a dead updater status and enabled `Check for Updates...` menu command even
  though the build has no updater;
- permanently disabled Save and Save As commands despite canonical project
  actions being persisted as they are applied;
- fake traffic-light controls inside Project Home beneath the real native window
  controls;
- Settings remaining mounted while hidden, which can leave focus inside an
  invisible view after Back;
- a production build warning caused by circular first-party manual chunks.

The existing operational-settings design correctly established direct model
download, Keychain secret storage, bundled GStreamer/GES, bounded health probes,
operation journaling, and safe repair. This design preserves those working
contracts but replaces its six-category health-dashboard information
architecture.

## Research Basis

The scope model follows established professional application patterns:

- Final Cut Pro groups preferences around workflows such as editing, playback,
  and import rather than presenting runtime probes as preferences:
  <https://support.apple.com/en-tj/guide/final-cut-pro/ver8e3f2f66/mac>.
- Descript separates application settings from project settings and keeps model
  selection and transcription defaults discoverable:
  <https://help.descript.com/hc/en-us/articles/10606771145869-App-settings> and
  <https://help.descript.com/hc/en-us/articles/10164125738381-Project-settings>.
- VS Code distinguishes global user settings from workspace settings and moves
  MCP server setup into a dedicated management flow:
  <https://code.visualstudio.com/docs/configure/settings> and
  <https://code.visualstudio.com/docs/agent-customization/mcp-servers>.

Video Creater will adapt these scope boundaries to its existing compact dark
desktop visual language, shadcn primitives, Lucide iconography, split-project
manifest, and Rust validation boundary. It will not imitate another product's
visual styling.

## Decision

Replace the single mixed Settings health dashboard with three explicitly scoped
surfaces:

1. **App Settings** (`Command-,`) for preferences and global resource setup.
2. **Project Settings** for values and maintenance scoped to the open project.
3. **System Health** for diagnostics, self-tests, runtime versions, and repair.

The App Settings sidebar contains:

- General;
- Projects;
- AI & Models;
- Integrations;
- Storage;
- Advanced.

No App Settings navigation item carries a health badge. A preference category
cannot be failed, and an optional unconfigured integration is not an application
failure.

## Product Principles

### Preferences Are Choices

A control belongs in App Settings only when it changes global behavior, defines
a default for future projects, configures a global resource, or establishes an
integration the user intentionally selected.

Static technical state, component probes, self-tests, raw paths, operation
journals, and repair commands do not qualify as preferences.

### Context Is Neutral

The following are neutral states:

- no project is open;
- notifications have never been requested;
- an optional provider has no credential;
- an optional generation model is not selected;
- Temporal is not configured while Desktop execution is selected;
- project-only diagnostics are not applicable.

Neutral states use copy such as `Not configured`, `Not enabled`, or `Open a
project to inspect`. They never use warning triangles, `Failed`, or `Action
required` unless an active user-requested workflow is blocked.

### Blocked Actions Explain the Missing Configuration

Any editor or workflow control blocked by missing configuration renders an
inline configuration notice at the point of use. It includes:

- the blocked capability;
- the missing setting or credential in user language;
- the affected provider, model, or runtime when relevant;
- a `Configure in Settings` action that deep-links to the exact category and
  control;
- a retry or automatic re-evaluation after Settings closes.

Examples include:

- `OpenAI image generation needs an OpenAI provider key.`
- `Transcription needs a downloaded transcription model.`
- `No generation models are enabled.`
- `Temporal execution is selected but no Temporal service is configured.`

A disabled control without an adjacent explanation is not acceptable. The
notice is not a fake feature placeholder: it appears only for an implemented
capability with a real missing prerequisite.

### Secrets Are Keychain-Only

Provider keys are entered through App Settings and stored in macOS Keychain.
The webview receives only provider ID, credential presence, source=`keychain`,
and validation metadata. It never receives a stored secret value.

Runtime environment variables are not an App Settings concept. App Settings
does not display, edit, recommend, or treat environment variables as credential
fallbacks. The desktop generation runtime resolves product credentials from
Keychain only. Test and developer harnesses may inject fixture credentials
outside the product path, but this is not exposed by the application UI or
health contract.

### Diagnostics Are Local and Recoverable

System Health can report a failure, but each failure is scoped to one component.
A failed provider validation cannot block model management, and a failed render
probe cannot replace Project Settings. Every recoverable failure includes a
local retry and a concrete recovery action.

## App Settings

### General

#### Privacy

Keep `Require confirmation before provider uploads` enabled by default. Copy
must clarify that local media stays local unless a remote generation request
needs that media and the user approves the upload.

#### Notifications

Keep `Render completion notifications` disabled by default.

- Before permission is requested, show `Not enabled`, not `Action required`.
- Enabling the toggle requests native permission just in time.
- Denied permission shows `Open macOS Settings` with the backend-provided
  recovery detail.
- Native delivery failure is local to this row and does not mark General failed.

#### Exclusions

General does not contain Updates, render-runtime state, component health, or an
appearance selector. Updates return only after a signed updater and trusted
release endpoint exist. Appearance returns only after the app has a complete
light/system theme rather than a nonfunctional selector.

### Projects

#### New Project Defaults

Expose defaults copied into a project only when that project is created:

- width and height with common presets and validated custom dimensions;
- frame rate with supported canonical values;
- loudness target in LUFS;
- caption delivery: Burn in, Embedded, or Off.

Existing projects do not change when these defaults change. The page explains
that project-specific values are edited through File > Project Settings.

#### Project Saving

The page may state once that Video Creater saves canonical project edits as they
are applied. It does not expose an autosave interval for an immediate-persistence
model.

#### Exclusions

Do not expose unsupported choices for linked media, proxies, optimized media,
color management, background rendering, or audio devices. Imported media
continues to be copied into the project. Each deferred control returns only with
a complete underlying runtime, persistence, error, and recovery contract.

Timeline snapping is not a setting. It remains the local pressed-state toggle in
the timeline actions row described by
`2026-06-25-timeline-snapping-toggle-design.md`. It is enabled by default, does
not persist, and affects the active editing surface only.

### AI & Models

#### Local Transcription

Preserve the working in-app model manager and make it the primary surface:

- model name, source, installed size, version, runtime, and active state;
- Download, Cancel, Resume/Retry, Update, Use, and Remove;
- download bytes/files and verification progress;
- source, immutable revision, license, and verified timestamp;
- Verify as a secondary action;
- manual folder import only under Advanced recovery.

Speech analysis is presented as part of the local speech stack, with its own
artifact state, rather than as an unexplained peer catalog.

Catalog, runtime, and speech-status load failures render independently with
local Retry actions. A catalog failure must not hide already installed model
state.

#### Generation Model Selection

Replace the full-page checkbox grid with one compact, searchable multi-select
combobox.

The closed trigger shows a short summary such as:

- `No remote models enabled`;
- `7 models enabled`;
- `OpenAI and Replicate models` when that summary fits.

The popover:

- groups models by Image, Video, and Audio;
- supports text search over model and provider names;
- uses checkboxes with immediate persistence;
- shows provider name and compact configured/not-configured metadata;
- offers group-level Select all and Clear actions;
- supports full keyboard navigation, Space toggling, Escape closure, and focus
  return;
- scrolls within a bounded height rather than expanding the page;
- shows `Configure provider` on models whose provider has no Keychain key.

No model is implicitly selected merely because it appears in the catalog. New
installations default to built-in local/mock capabilities only. Remote model
selection uses a positive `enabledGenerationModelIds` allowlist rather than the
current negative denylist that makes every new catalog entry enabled.

The model combobox controls visibility and availability in generation pickers;
it does not validate provider credentials or turn missing credentials into
Settings health.

#### Transcription Defaults

Language remains `Auto` until the native transcription runtime exposes a
validated supported-language list. Do not render a one-option combo box.
Auto-transcription on import, automatic proxy generation, and automatic speaker
analysis remain deferred until their batch progress, cancellation, failure, and
project-persistence behavior is designed.

### Integrations

Integrations contains provider account-style rows, not a health dashboard.

Each provider row shows:

- display name and supported capabilities;
- whether a Keychain credential exists;
- account label and non-secret validation metadata when available;
- Connect, Update key, Validate, and Disconnect actions;
- a local validation error with Retry.

Unconfigured providers are grouped under `Available integrations`. Configured
providers appear first. There is no `Needs attention` group for providers the
user has never chosen.

The credential form writes directly to Keychain. It does not show an environment
variable name, environment fallback, runtime source field, or copyable secret.
Disconnect requires confirmation when enabled generation models depend on the
provider and explains which models will become unavailable.

Generation execution backend selection moves to Advanced. Generation-model
selection moves to AI & Models.

### Storage

Storage contains global, user-manageable storage only:

- suggested parent folder for new projects, with Ask for a folder as default;
- disposable application cache usage;
- installed model usage, with a link to AI & Models for model removal;
- free space on relevant volumes;
- Reveal in Finder;
- Refresh;
- exact cleanup preview and confirmation.

Low-space warnings appear only when free space blocks a known operation. Project
media, transcripts, renders, generated media, and project logs never appear in
global Settings.

### Advanced

#### Execution

Desktop execution is the default. Temporal is shown as an advanced option only
when its product configuration contract exists. Missing Temporal configuration
does not affect Desktop readiness.

#### Developer Integration

Move MCP client configuration here. With a project open, it generates and copies
the project-pinned client configuration. With no project open, it shows a neutral
instruction to open a project. MCP server self-tests and raw protocol diagnostics
remain in System Health.

#### Recovery

Advanced exposes:

- Open System Health;
- Reveal logs;
- Reset app preferences with confirmation;
- advanced local-model folder import.

Raw shell repair commands and environment-variable instructions are never normal
product UI.

## Project Settings

Project Settings opens through File > Project Settings and is enabled only when
a canonical split project is open. It is a separate surface from App Settings
and always names the active project in its header.

### Format & Output

Edit existing project-owned values:

- project name;
- width and height;
- frame rate;
- loudness target;
- caption delivery mode.

The form validates finite, supported values before applying one structured
project action. A successful change persists through the existing Rust project
action boundary and participates in project undo/redo where supported. It affects
future preview/render/export work and does not rewrite existing media.

### Storage

Show only active-project inventory:

- project root;
- imported media;
- transcripts and analysis;
- generated media;
- render artifacts;
- workflow and log artifacts.

Actions include Reveal, Refresh, exact cleanup preview, selected render-artifact
cleanup, and safe disposable-project-cache cleanup. They cannot delete canonical
manifests, accepted project media, installed global models, or unrelated files.

### Agent Guidance

The healthy default is one compact row: `Bundled project guidance active`.
Individual mandatory skills appear only in an expanded detail view or when one
fails verification.

Repair appears only for missing or damaged bundled guidance. It previews exact
paths, backs up differing content, preserves custom skills and unrelated project
instructions, requires confirmation, and reruns verification. A missing project
is never represented because Project Settings cannot open without a project.

### Deferred Project Controls

Media analysis defaults, auto-transcription, linked media, proxies, color
management, recovery snapshots, and default export destinations are intentionally
absent until their underlying behavior is complete. This prevents another set of
placeholder controls.

## System Health

System Health opens through Help > System Health. It is the only surface with an
overall readiness summary and `Check all required systems`.

### Required Readiness

Only components required by installed application capabilities affect overall
readiness:

- bundled GStreamer/GES composition runtime;
- reviewed GStreamer plugin policy;
- AVFoundation delivery exporter on supported macOS builds;
- compatibility decoder;
- Core ML transcription helper when local transcription is enabled;
- integrity of an installed active local model;
- canonical project schema when a project is open.

### Diagnostic Sections

- **Rendering:** GStreamer/GES, plugin policy, AVFoundation, compatibility
  decoder, versions, and bounded self-tests.
- **Local AI:** native helper, active-model integrity, speech-analysis runtime,
  and model-load self-test.
- **Agent Runtime:** Codex app-server, Video Creater MCP server, and proposal
  validation self-tests.
- **Project Integrity:** split-project validation and bundled guidance, only
  when a project is open.
- **Environment:** app version, build type, bundled component versions, free
  space, and log locations.

Optional provider credentials and optional remote models never contribute to
System Health. Update capability is absent until a real updater exists.

### Error Isolation

System Health owns one snapshot and one operation stream. A category refresh
updates only that category. Failure to refresh one category preserves the last
known state of every other category and renders a local error with Retry. The UI
never derives global readiness from a missing project or optional integration.

## Deep-Link Contract

Settings navigation accepts a typed target rather than only a category:

```ts
type AppSettingsTarget =
  | { category: "general"; item?: "privacy" | "notifications" }
  | { category: "projects"; item?: "newProjectDefaults" }
  | { category: "aiModels"; item?: "transcription" | "generationModels" }
  | { category: "integrations"; provider?: ProviderId }
  | { category: "storage"; item?: "projectLocation" | "cache" }
  | { category: "advanced"; item?: "execution" | "mcp" | "recovery" };
```

Opening a target selects the category, scrolls the item into view, focuses its
heading or primary control, and announces the destination. Closing Settings
returns focus to the originating blocked action. If Settings changes the missing
configuration, the originating surface re-evaluates readiness without requiring
an application restart.

Use a shared `ConfigurationNotice` component for blocked product actions. It
accepts a typed target and never constructs category names or DOM selectors from
freeform strings.

## Persistence

### App Preferences

Replace browser `localStorage` preference ownership with a schema-versioned
Rust-owned preferences file under application support, written through an atomic
temporary-file and rename boundary.

The new preference model contains no secret and no environment-variable field:

```rust
pub struct AppPreferencesV2 {
    pub schema_version: u32,
    pub project_location: ProjectLocationPreference,
    pub require_provider_upload_confirmation: bool,
    pub render_completion_notifications: bool,
    pub new_project_defaults: NewProjectDefaults,
    pub enabled_generation_model_ids: Vec<String>,
    pub generation_execution_backend: GenerationExecutionBackend,
}
```

Rust validates and normalizes every write. The frontend receives a typed copy,
applies optimistic UI only when safe, and reconciles against the accepted Rust
value.

Migrate non-secret values from `video-creater.appSettings.v1` once. Do not carry
forward `providerCredentialEnvVar`. Because the current negative model denylist
implicitly enables every catalog entry, replace it rather than converting it:
remote `enabledGenerationModelIds` begins empty. Preserve project location,
upload confirmation, notifications, and valid execution-backend choice.

Workspace layout, timeline view state, editor-tour dismissal, and local chat
transcript storage are separate editor-state concerns and are not silently moved
into App Settings in this rewrite.

### Provider Credentials

Provider secrets remain in macOS Keychain under stable provider IDs. App
preferences store no secret, secret alias, environment-variable name, or secret
source. Removing a Keychain entry updates provider metadata and invalidates
dependent generation availability immediately.

### Project Settings

Project format/output settings continue to use the existing `renderSettings`
manifest fields. Project name uses the existing manifest identity. New
project-scoped settings require explicit schema additions, migrations, Rust
validation, and split-project sidecars before any UI is added.

## Frontend Ownership

The rewrite replaces the current giant owner plus child-owned fetches with:

- one App Settings preference owner;
- one AI/model resource owner;
- one provider credential owner;
- one System Health owner;
- presentational category pages that receive typed state and actions;
- focused operation hooks that cannot overwrite newer results.

App Settings preferences remain usable when System Health is unavailable. No
global `categoryRefreshError` can replace unrelated pages.

Settings unmounts when closed. Back and deep-linked dismissal restore focus to
the exact originating control. Loading and error regions use local live-region
announcements. The native minimum-width desktop layout is the real product
target; remove unreachable narrow-screen settings navigation that exists only in
browser fixtures.

## Native Menu Changes

- Remove `Check for Updates...` until updater support exists.
- Add File > Project Settings when a project is open.
- Remove permanently disabled Save and Save As commands if the verified
  immediate-persistence contract covers every canonical mutation. If any
  canonical mutation is discovered to remain unsaved, implement Save correctly
  instead of leaving a disabled command.
- Replace Help > MCP Setup and Help > Project Skills with deep links to Advanced
  and Project Settings only when those destinations are contextually available.
- Add Help > System Health.
- Preserve Editor Tour, Keyboard Shortcuts, and Send Feedback.

## Visual Structure

Reuse the current compact Settings shell, typography, spacing scale, borders,
buttons, form controls, and Lucide icons.

- Sidebar selection uses the existing accent treatment without health badges.
- Header subtitle names the scope (`App preferences` or the active project), not
  `Global preferences - No project open` on every page.
- Preference rows align label, explanation, and control in a stable two-column
  grid.
- Empty space is acceptable; pages are not filled with diagnostics to look busy.
- Long technical detail is absent from App Settings and disclosed only in System
  Health.
- The generation model combobox is bounded and compact.
- Project Home removes its fake traffic-light row and relies on native window
  controls.

No new color system, custom icon family, hand-drawn SVG, or visual asset is
introduced.

## Error and State Language

Use these states consistently:

- **Ready:** a required capability was verified;
- **Needs action:** a user-requested capability is blocked and has a recovery;
- **Failed:** an attempted operation or required self-test failed;
- **Checking:** bounded work is active;
- **Not configured:** optional setup is absent;
- **Not enabled:** the user has not opted into a feature;
- **Not applicable:** context does not apply.

`Unavailable` is reserved for platform/build limitations, not missing project
context. Settings navigation never shows these states.

## Build and Code Quality

- Keep TypeScript strict with `allowJs: false`,
  `noUncheckedIndexedAccess`, and `exactOptionalPropertyTypes`.
- All new frontend source is TypeScript/TSX.
- Do not add a Clippy suppression. Refactor argument-heavy or complex code into
  typed request/context structures unless an exceptional suppression is
  independently justified.
- GStreamer/GES, AVFoundation helpers, model helpers, and other required native
  runtimes remain bundled. No Homebrew or package-manager runtime dependency is
  introduced.
- Remove the Vite circular first-party chunk warning by correcting or deleting
  the manual chunk boundary and updating its policy tests. Do not suppress the
  warning.
- Preserve unrelated dirty work, including `.superpowers/sdd/progress.md`.

## Verification Strategy

Implementation uses red-first tests.

### Rust

- atomic app-preference read/write, validation, corruption recovery, and v1
  migration;
- no environment-variable field or fallback in product provider contracts;
- Keychain-only credential presence, validation, removal, and secret redaction;
- required/optional/context-neutral health aggregation;
- project-setting validation and atomic persistence;
- bounded component self-tests and local error isolation;
- exact cleanup preview and deletion boundaries.

### Frontend

- Settings categories and absence of sidebar health badges;
- neutral notification, no-project, and optional-provider states;
- generation-model combobox keyboard, search, grouping, checkbox, summary, and
  bounded-layout behavior;
- provider Connect/Update/Validate/Disconnect flows without environment UI;
- `ConfigurationNotice` deep links and focus return;
- locally isolated load/operation failures;
- Settings unmount and focus restoration;
- Project Settings format validation and project action persistence;
- System Health required-only rollup;
- native-menu routing and removed dead commands.

### Native Application

Build and open the fresh Tauri `.app`, then capture and inspect at least:

1. every App Settings category with no project open;
2. AI & Models with the compact model combobox open;
3. Integrations with configured and unconfigured providers;
4. an editor action blocked by a missing provider key and its Settings deep
   link;
5. Project Settings for a real split project;
6. System Health in healthy and locally failed states;
7. focus return after closing Settings;
8. Project Home with only native traffic lights.

Screenshots are compared at the same viewport. Visual review checks truncation,
spacing, density, scrolling, focus, control state, warnings, and whether any
healthy or neutral state is rendered as an error.

### Completion Gates

- frontend lint, strict typecheck, tests, and production build pass without the
  circular chunk warning;
- Rust formatting, Clippy, targeted tests, and the proportional full suite pass;
- settings acceptance/e2e fixtures pass;
- required runtime packaging verification passes against the fresh app bundle;
- native walkthrough finds no placeholder pages, contradictory statuses, dead
  commands, missing recovery action, or environment-variable provider setup;
- working tree contains only intentional changes and preserved user-owned work;
- final implementation review has no unresolved findings.

## Non-Goals

- Do not implement a light theme merely to populate General.
- Do not implement an updater without signed release infrastructure.
- Do not add linked media, proxies, color management, background rendering,
  automatic transcription, or audio-device controls as settings-only shells.
- Do not turn Temporal into a desktop requirement.
- Do not replace GStreamer/GES with AVFoundation.
- Do not expose provider secrets, environment variables, raw repair commands, or
  package-manager instructions.
- Do not make optional providers or models part of required application health.
- Do not persist timeline snapping as an App Setting.
