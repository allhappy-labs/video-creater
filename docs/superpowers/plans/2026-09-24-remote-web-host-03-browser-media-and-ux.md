# Remote Web Host 03 — Browser Media, Files, and Remote UX Plan

**Goal:** Complete the real browser workflow for projects, uploads, playback, editing, tasks,
exports, and host-specific capability differences.

**Spec:** [Remote Web Host Design](../specs/2026-09-24-remote-web-host-design.md)  
**Depends on:** plans 01–02  
**Feeds:** plan 04

## Task 1 — Add host-managed project discovery

**Create:**

- `src-tauri/src/web_host/project_catalog.rs`
- `src-tauri/tests/web_host_project_catalog.rs`
- remote project-home tests in `src/components/home/`

- [ ] Configure one or more allowed project roots locally on the host.
- [ ] Return opaque project IDs, names, timestamps, and safe thumbnails; never absolute paths.
- [ ] Resolve canonical paths beneath allowed roots on every open, including after symlink changes.
- [ ] Support create/open/recent in the browser. Keep arbitrary host folder selection desktop-only.
- [ ] Label host settings and storage facts as “On <host>”.

**Commit:** `feat(remote): browse projects without exposing host paths`

## Task 2 — Implement bounded upload and browser import

**Create:**

- `src-tauri/src/web_host/upload.rs`
- `src-tauri/tests/web_host_upload.rs`
- `src/lib/runtime/adapters/browser-file-upload.ts`
- frontend tests for picker, drop, progress, cancel, retry, and failure

- [ ] Stream uploads to a host-chosen staging filename with declared and actual size limits, free
  space checks, MIME/media probing, cancellation, hash, and atomic completion.
- [ ] Treat the client filename as display metadata only; reject traversal and ambiguous names.
- [ ] Convert a completed staging ID into the existing Rust media import path, then revoke the ID.
- [ ] Clean partial uploads immediately and expose expired completed uploads to safe storage cleanup.
- [ ] Adapt browser file input and drag/drop without changing Tauri native dialog/drop behavior.

**Commit:** `feat(remote): upload and import browser media safely`

## Task 3 — Replace raw paths with scoped media and artifact IDs

**Create:**

- `src-tauri/src/web_host/resource_ticket.rs`
- `src-tauri/src/web_host/media.rs`
- `src-tauri/src/web_host/artifact.rs`
- focused range/security tests

**Modify:** `src/lib/runtime/adapters/remote-transport.ts`

- [ ] Mint short-lived opaque tickets scoped to session, project, resource, and access kind.
- [ ] Implement GET/HEAD and a single RFC-compatible byte range with correct 206/416 headers,
  content type, length, cache policy, and cancellation.
- [ ] Resolve and authorize canonical files only after ticket validation; never decode a path from
  the URL.
- [ ] Implement attachment downloads for exports and packages with safe display filenames.
- [ ] Make `RemoteTransport.mediaUrl` return scoped URLs and refresh an expired ticket without
  changing canonical project data.
- [ ] Prove seeking a fixture video transfers only requested ranges.

**Commit:** `feat(remote): stream scoped media and artifacts`

## Task 4 — Complete remote capability UX

**Modify:** editor, Home, Settings, Export, Background tasks, and runtime capability modules/tests

- [ ] Replace desktop-only affordances with browser equivalents: upload instead of native file
  path, download instead of reveal, host destination plus download instead of choose-folder.
- [ ] Disable operations with an explanation when no safe browser equivalent exists.
- [ ] Keep Export as the single export entry point and Background tasks as the only job surface.
- [ ] Show connection health compactly without adding a rail or exposing transport vocabulary.
- [ ] Make read-only/editor-lease state visible at the edit target; allow explicit takeover with a
  Radix confirmation dialog.
- [ ] Preserve minimum 24 px phone touch targets, safe-area padding, bottom sheets, keyboard focus,
  reduced motion, and existing dark theme tokens.
- [ ] Add browser notification support only after user permission; task state remains authoritative.

**Commit:** `feat(remote): adapt editor workflows for browser capabilities`

## Task 5 — Prove the remote edit-to-render vertical slice locally

**Create:** `e2e/remote-host.spec.ts`, `e2e/remote-host-phone.spec.ts`, test harness scripts

- [ ] Launch the real Rust host on loopback with isolated app-data and project roots; use a
  test-only pairing bootstrap injected by process configuration, never compiled into release.
- [ ] Create/open project, upload tiny media, play/seek, edit, save/reload, start an agent fixture
  through the real service boundary, apply/review, render, follow task progress, and download MP4.
- [ ] Repeat the core flow at 1440×900 and 402×874 without `__EDITOR_FIXTURE_RUNTIME__` or
  `__TAURI_INTERNALS__`.
- [ ] Probe the MP4 for expected duration and streams and assert artifact/log references exist on
  the host.
- [ ] Disconnect during render and prove reconnect plus snapshot/event recovery.

**Commit:** `test(remote): cover the real browser editing workflow`

## Exit Criteria

- The complete workflow works against the real loopback Rust host in production frontend mode.
- Desktop-only behavior is never reported as remotely successful.
- Media, uploads, and downloads expose no host path and pass focused abuse tests.

