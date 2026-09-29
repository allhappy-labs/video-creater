# Remote Web Host 02 — Secure Gateway, Pairing, and Transport Plan

**Goal:** Serve the production client through a loopback Rust gateway, authenticate paired tailnet
devices, and connect the browser to the shared service over versioned RPC and events.

**Spec:** [Remote Web Host Design](../specs/2026-09-24-remote-web-host-design.md)  
**Depends on:** plan 01  
**Feeds:** plans 03–04

## Task 1 — Add the host feature and binary

**Create:**

- `src-tauri/src/bin/video-creater-host.rs`
- `src-tauri/src/web_host/mod.rs`
- `src-tauri/src/web_host/config.rs`
- `src-tauri/src/web_host/assets.rs`
- `src-tauri/tests/web_host_startup.rs`

**Modify:** `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`, `package.json`, release policy files

- [ ] Select a small Rust HTTP stack after licence review; pin it in `Cargo.lock` and record its
  purpose. Enable only required features.
- [ ] Add a `web-host` feature and headless binary that embeds or serves the production `dist/`.
- [ ] Bind `127.0.0.1` by default. Reject non-loopback bind unless a developer-only
  `--allow-insecure-direct-bind` flag is present, and print a conspicuous warning when it is.
- [ ] Add `/healthz` with build and protocol compatibility only—no user, project, path, or secret.
- [ ] Add graceful signal handling, connection draining, and structured readiness output.
- [ ] Add `pnpm host:dev` that builds the production frontend and starts the host, never Vite on a
  remote interface.

**Commit:** `feat(remote): add the loopback web host`

## Task 2 — Implement trusted-proxy identity and pairing

**Create:**

- `src-tauri/src/web_host/identity.rs`
- `src-tauri/src/web_host/pairing.rs`
- `src-tauri/src/web_host/session.rs`
- `src-tauri/tests/web_host_auth.rs`

- [ ] Accept Tailscale identity headers only from a loopback peer in configured Tailscale Serve
  mode; remove any untrusted inbound copies before auth decisions.
- [ ] Add five-minute, single-use pairing codes with bounded attempts and constant-time comparison.
- [ ] Store only hashes of random device credentials plus display metadata in an owner-readable
  app-data file written atomically.
- [ ] Set `Secure`, `HttpOnly`, `SameSite=Strict` cookies; implement rotation, expiry, logout, list,
  revoke, and revoke-all.
- [ ] Require exact Origin and a session-bound CSRF token on mutations and WebSocket upgrades.
- [ ] Cover forged proxy headers, replayed codes, rate limiting, cookie flags, CSRF, foreign Origin,
  expired sessions, and revoke in tests.

**Commit:** `feat(remote): require identity and device pairing`

## Task 3 — Add the explicit RPC registry and idempotency

**Create:**

- `src-tauri/src/web_host/rpc.rs`
- `src-tauri/src/web_host/registry.rs`
- `src-tauri/src/web_host/idempotency.rs`
- `src-tauri/tests/web_host_rpc.rs`

- [ ] Generate or hand-build an explicit registry from the operation inventory; never dispatch by
  reflection or arbitrary function name.
- [ ] Validate body size, JSON shape, auth scope, project identity, editor lease, expected revision,
  and capability before calling the service.
- [ ] Return the stable success/error envelopes from the spec and map internal errors without
  leaking paths or debug chains.
- [ ] Cache bounded mutation results by session and request ID long enough for safe retry. Reject a
  reused ID whose payload hash differs.
- [ ] Add per-session concurrency and request-rate limits with useful `retryAfterMs` errors.

**Commit:** `feat(remote): expose an allowlisted idempotent RPC API`

## Task 4 — Add resumable events and editor leases

**Create:**

- `src-tauri/src/web_host/event_hub.rs`
- `src-tauri/src/web_host/editor_lease.rs`
- `src-tauri/tests/web_host_events.rs`
- `src-tauri/tests/web_host_editor_lease.rs`

- [ ] Broadcast ordered, bounded event envelopes from the shared `EventSink`.
- [ ] Resume from a retained sequence; emit `snapshotRequired` when the gap is no longer retained.
- [ ] Bound per-client buffers and disconnect slow consumers without blocking jobs.
- [ ] Implement one renewable writer lease per project with release, disconnect grace, explicit
  takeover, audit record, and immediate invalidation of the prior token.
- [ ] Prove two simultaneous mutations cannot both pass after takeover and that background jobs do
  not depend on the browser lease.

**Commit:** `feat(remote): stream events and serialize project editors`

## Task 5 — Implement the frontend remote transport and connection UI

**Create:**

- `src/lib/runtime/adapters/remote-transport.ts`
- `src/lib/runtime/adapters/remote-transport.test.ts`
- `src/lib/runtime/remote-session.ts`
- focused connection/pairing components and tests under `src/components/runtime/`

**Modify:** `src/lib/runtime/bootstrap.ts`, `src/components/runtime/desktop-host-connection.tsx`,
`src/App.tsx`

- [ ] Discover same-origin `/api/v1/session`; never accept a host URL from an arbitrary query
  parameter in production.
- [ ] Implement RPC, request IDs, CSRF, WebSocket subscribe/resume, disconnect/reconnect with bounded
  backoff, event unlisten, and protocol mismatch handling.
- [ ] Select remote runtime only after an authenticated session; otherwise render connection or
  pairing state without loading project data.
- [ ] Show reconnecting, revoked, incompatible, read-only, and takeover states in plain language.
- [ ] Use the existing desktop and phone layouts after connection; do not create a parallel editor.
- [ ] Run focused tests, `rtk pnpm lint`, and `rtk pnpm build`.

**Commit:** `feat(remote): connect browsers to the authenticated host`

## Exit Criteria

- A production browser build can pair and perform non-file RPC against the real Rust host.
- Unauthenticated, cross-origin, revoked, and non-allowlisted calls fail closed.
- Tauri continues to use `TauriTransport`; fixtures continue to use `FixtureTransport`.

