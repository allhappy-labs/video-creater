# Remote Web Host 04 — Acceptance, Hardening, and Packaging Plan

**Goal:** Ship a repeatable Tailscale-first host experience and retain honest cross-device evidence.

**Spec:** [Remote Web Host Design](../specs/2026-09-24-remote-web-host-design.md)  
**Depends on:** plans 01–03

## Task 1 — Integrate and verify Tailscale Serve

**Create:**

- `src-tauri/src/web_host/tailscale.rs`
- `src-tauri/tests/web_host_tailscale.rs`
- `docs/development/remote-access.md`

- [x] Detect the Tailscale CLI/daemon without privilege escalation and report signed-out, stopped,
  unauthorized, and ready states separately.
- [x] Print or display a URL only after checking the current device MagicDNS name/tailnet address and
  verifying the Serve route reaches this host's `/healthz`.
- [x] Provide exact opt-in setup commands, but never silently modify Serve or firewall state.
- [x] Verify proxy identity headers are present and trusted only on loopback.
- [x] Document start, stop, status, pairing, revoke, logs, and recovery for desktop and headless use.

**Commit:** `feat(remote): verify the Tailscale Serve exposure path`

## Task 2 — Add desktop Remote access settings

**Create:** focused Settings components/tests and Rust settings operations for remote-host state

- [ ] Add `Settings → Remote access` with Start/Stop, verified URL, QR/code, paired devices, revoke,
  active sessions, editor leases, and recent security events.
- [ ] Reuse Background tasks for host startup work; do not add a second generic job surface.
- [ ] Ensure the app closes the listener and WebSockets immediately on Stop and app shutdown while
  host-owned render/generation jobs follow their existing lifecycle rules.
- [x] Verify Settings never displays a credential, cookie, token, absolute secret path, or raw
  identity header.

**Commit:** `feat(remote): manage browser access from Settings`

## Task 3 — Package and supervise the headless host

**Modify:** Linux/macOS build scripts, package manifests, provenance and release policy tests

**Create:** a sample systemd **user** unit and packaging tests

- [x] Package `video-creater-host` and the production frontend with the same compatible protocol
  version and runtime resources as the desktop app.
- [x] Add preflight checks for media runtime, sidecars, models, writable app data, project roots,
  credential service, and port availability.
- [x] Provide a systemd user-service template with restart throttling, a private runtime directory,
  restrictive umask, and no root/privileged/container requirements.
- [x] Add clean shutdown and crash-recovery evidence for durable jobs and temporary uploads.
- [ ] On macOS, plan and run signing/notarization only on a Mac; do not claim it from Ubuntu.

**Commit:** `build(remote): package and supervise the web host`

## Task 4 — Run the security and compatibility gates

- [x] Run forged identity, CSRF, Origin, WebSocket, replay, rate-limit, upload-size, disk-pressure,
  traversal, symlink, media-ticket, artifact-ticket, lease-takeover, revoke, and log-redaction tests.
- [x] Fuzz or property-test range parsing, percent decoding, event resume windows, and RPC envelope
  decoding with bounded inputs.
- [x] Verify host/client protocol skew in both directions produces a clear block, not partial use.
- [ ] Run dependency licence and vulnerability checks according to repository policy.
- [ ] Run `rtk pnpm verify:frontend`, host-feature Rust format/clippy/tests, existing Tauri acceptance,
  and the relevant native release lane. Report environmental failures as blocked and rerun after
  correction.

**Commit:** `test(remote): harden the authenticated host boundary`

## Task 5 — Retain real cross-device acceptance evidence

- [ ] On this Ubuntu machine, verify current Tailscale state and start the packaged headless host.
  Record the MagicDNS/tailnet URL only after the live check passes.
- [ ] From a second tailnet laptop, pair, open/create a project, upload fixture media, edit, run the
  agent fixture through the real host, render, reconnect, and download the artifact.
- [ ] From a phone, repeat open/play/seek/edit/task/download at the real device viewport and capture
  the pairing, editor, and completed-task states.
- [ ] Connect a second client, prove read-only behavior, take over deliberately, and prove the old
  writer is rejected without project corruption.
- [x] Probe the downloaded MP4 and record duration, streams, caption/overlay timing, host artifact
  reference, and render log reference.
- [x] Retain a redacted report with build hashes, protocol version, devices/viewport classes, test
  timestamps, checks and failures. Do not retain cookies, codes, tokens, credentials, personal
  tailnet identity, or private host paths.
- [ ] Update README, runtime documentation, and backlog status from `planned` to `verified` only when
  the entire stated boundary passes.

**Commit:** `docs(remote): record cross-device acceptance evidence`

## Final Definition of Done

A user can start Video Creater on the host, receive a verified tailnet URL, pair a laptop or phone,
complete the real editing and export workflow, disconnect/reconnect safely, and revoke the device.
All sensitive compute and credentials remain on the host, all mutations are validated and
revisioned, and the retained report proves the packaged cross-device boundary rather than a fixture.
