# Operational Settings Program

This program implements the approved design in
`docs/superpowers/specs/2026-07-16-operational-settings-and-runtime-readiness-design.md`.

The work is split because the five slices have different native dependencies and
verification boundaries. Execute them in the order below; later plans depend on
the shared contracts created by Plan 1.

## Plan Order

1. [Settings Health and Model Operations](2026-07-16-settings-health-and-model-operations.md)
   - Introduces the shared Rust health/operation contracts.
   - Makes the existing pinned Hugging Face Parakeet downloader reachable.
   - Adds progress, cancellation, retry, verification, removal, and advanced import.
   - Splits the Settings shell and Models page from the monolith.

2. [Required Render Runtime and General Settings](2026-07-16-required-render-runtime-and-general-settings.md)
   - Enables `ges-render` in production.
   - Packages and validates the curated GStreamer/GES runtime.
   - Separates composition health from AVFoundation delivery health.
   - Replaces copied terminal commands and inert update controls with real state.

3. [Agent, MCP, and Skills Settings](2026-07-16-agent-mcp-and-skills-settings.md)
   - Adds bounded app-server, MCP server, and proposal-validator self-tests.
   - Bundles the MCP executable.
   - Verifies and narrowly repairs mandatory project skills.

4. [Storage and Provider Settings](2026-07-16-storage-and-provider-settings.md)
   - Adds native folder selection and a versioned storage preference.
   - Adds Rust-owned inventory, free-space reporting, and safe cleanup.
   - Reorganizes Keychain-backed providers by actionable status.

5. [Settings Integration and Visual QA](2026-07-16-settings-integration-and-visual-qa.md)
   - Removes the old monolith after parity is proven.
   - Adds packaged-app and real-render evidence.
   - Runs the approved Settings state matrix through visual and accessibility QA.

## Shared Contract Ownership

Plan 1 owns these definitions and all later plans extend their builders rather
than creating competing status models:

- `SettingsHealthSnapshot`
- `SettingsHealthState`
- `SettingsCategoryHealth`
- `SettingsComponentHealth`
- `SettingsOperation`
- `SettingsOperationKind`
- `SettingsOperationState`
- `SettingsOperationError`
- the `settings-operation` Tauri event
- the operation journal and interrupted-operation reconciliation

## Cross-Plan Rules

- Use test-driven development for every behavior change.
- Keep Tauri commands thin; domain logic belongs in testable Rust modules.
- Keep category components free of direct `invoke` calls. Use adapters in
  `src/lib/settings/`.
- Never infer readiness in React from paths, copied commands, or unrelated
  worker state.
- Never expose provider secret values in health snapshots, logs, or tests.
- Never delete canonical project files, project media, transcripts, accepted
  renders, or model installations through a generic cleanup action.
- Use bounded timeouts for all external process, Keychain, filesystem, and
  runtime probes.
- Keep diagnostics collapsed by default and preserve keyboard navigation.
- Commit each green task using Conventional Commits.

## Program Verification

After every plan:

```bash
rtk pnpm lint
rtk pnpm test
rtk cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

After Plan 5:

```bash
rtk pnpm verify
rtk pnpm visual:qa:browser-release
rtk pnpm release:macos
```

