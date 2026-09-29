# Remote Web Host Design

**Date:** 2026-09-24  
**Status:** Implemented; local browser acceptance passed, live tailnet acceptance blocked  
**Plans:**

1. [Runtime contract and service extraction](../plans/2026-09-24-remote-web-host-01-runtime-contract.md)
2. [Secure gateway, pairing, and remote transport](../plans/2026-09-24-remote-web-host-02-secure-gateway-and-transport.md)
3. [Browser media, files, and remote UX](../plans/2026-09-24-remote-web-host-03-browser-media-and-ux.md)
4. [Acceptance, hardening, and packaging](../plans/2026-09-24-remote-web-host-04-acceptance-and-packaging.md)

## Summary

Make Video Creater usable from a laptop or phone browser while the real application runtime stays
on one trusted Linux or macOS host. The same React editor will run in Tauri and in a browser. A new
Rust web-host process will serve the production frontend, authenticated RPC, server events, scoped
media, uploads, and downloads. It will call the same Rust application services as Tauri; it will not
proxy Tauri IPC or expose Codex app-server.

The first supported network path is **Tailscale Serve**:

```text
phone/laptop browser
        │ HTTPS on the user's tailnet
        ▼
Tailscale Serve on the host
        │ loopback HTTP + trusted identity headers
        ▼
Video Creater web host
        ├── production React assets
        ├── authenticated RPC + event stream
        ├── scoped media/upload/download routes
        └── Rust application services
              ├── canonical project files
              ├── render/transcription/generation workers
              ├── credentials on the host
              └── Claude or Codex app-server on the host
```

This is deliberately local-first and single-user. The browser never receives provider credentials,
absolute host paths, an arbitrary command surface, or direct access to an agent process.

## Current State and Readiness Verdict

The repository now contains the production web host, authenticated remote transport, scoped media
and upload/download routes, editor leases, session management, packaged Codex sidecar, Linux user
service, and desktop/phone real-host browser acceptance. The production package serves only on
loopback by default and is ready to sit behind Tailscale Serve.

Local acceptance evidence is retained in
[Remote web host local acceptance](../../reviews/evidence/remote-web-host-local-2026-09-25.md).
That run does not close the cross-device gate: this machine's Tailscale daemon is stopped, so a
verified MagicDNS URL and physical second-device run cannot be truthfully claimed until an operator
starts and authenticates the existing Tailscale service.

## Goals

- Start one host runtime on this machine and open the editor from another tailnet laptop or phone.
- Reuse one production React application and one set of Rust domain services across Tauri and web.
- Preserve Rust ownership of canonical project state, structured proposal validation, jobs, logs,
  credentials, artifacts, and agent lifecycle.
- Support real project selection, import/upload, preview and seeking, editing, AI turns, task
  progress, render/export, download, cancellation, and reconnect.
- Make remote access off by default, explicit to start, easy to stop, and observable in Settings and
  the headless CLI.
- Authenticate both the tailnet identity and a Video Creater device session.
- Provide an implementation and acceptance path that does not mistake browser fixtures for proof.

## Non-goals

- Public-internet hosting, port forwarding, anonymous LAN access, or a hosted SaaS control plane.
- Collaborative multi-user editing, CRDTs, or automatic merge of concurrent timeline writes.
- Running rendering, transcription, provider SDKs, or agents on the phone/browser.
- Sending local provider credentials or Codex/Claude login material to the browser.
- Exposing arbitrary Tauri commands, arbitrary filesystem paths, a shell, MCP, or Codex app-server.
- Native iOS or Android applications.
- Replacing the desktop app. Tauri remains a first-class client of the same service layer.

## Brainstormed Approaches

| Approach | Benefit | Fatal or material cost | Decision |
| --- | --- | --- | --- |
| Expose Vite on `0.0.0.0` | Very small change | Only serves UI; there is no backend, auth, safe media path, or production bundle | Reject |
| Remote-control the desktop with VNC | Works without product changes | Poor touch UX, video latency, no product-grade browser contract | Reject |
| Translate HTTP directly into Tauri commands | Reuses command names | Tauri state and window APIs leak into the server; hard to test and run headless | Reject |
| Separate cloud backend | Accessible anywhere | Breaks local-first media and credential boundaries; large operations burden | Defer |
| Shared Rust services with Tauri and web adapters | One domain implementation, real headless host, explicit security boundary | Requires deliberate service extraction and operation inventory | **Choose** |

## Product Decisions

| Topic | Decision |
| --- | --- |
| Network | Tailscale Serve is the first supported exposure path; the app binds loopback by default |
| Host modes | Packaged desktop toggle and a headless `video-creater-host` binary |
| Frontend | One production React build; runtime bootstrap selects Tauri or remote transport |
| Protocol | Versioned JSON RPC over HTTP plus a resumable WebSocket event stream |
| Auth | Tailscale identity at the proxy plus explicit, local device pairing and a secure session cookie |
| Authorization | One owner; per-device sessions; one active editor lease per project |
| Project discovery | Host-managed project roots and opaque project IDs; no browser-supplied absolute paths |
| File import | Browser uploads into a host staging area, then normal Rust import validation |
| Media | Opaque asset IDs, byte ranges, authorization on every request, no path in URLs |
| Exports | Server-side export followed by an authenticated browser download |
| Events | Ordered envelopes with sequence numbers; reconnect resumes or forces a snapshot refresh |
| Agents | Run only on the host through existing validated proposal paths |
| Concurrency | First writer holds an expiring editor lease; others are read-only until takeover |
| LAN fallback | Deferred until certificate and discovery UX are designed; never plain unauthenticated HTTP |

## Runtime Architecture

### Shared application service

Move command orchestration out of `src-tauri/src/main.rs` into small modules under a new
`src-tauri/src/app_service/`. The service owns dependencies and typed operations; adapters only
decode input, establish request context, call a typed method, and encode output.

```text
React BackendTransport
  ├── TauriTransport ── Tauri command adapter ─┐
  └── RemoteTransport ─ HTTP/WS adapter ───────┤
                                                ▼
                                      VideoCreaterService
                                                │
                         canonical project/domain/job modules
```

Do not begin by moving all roughly one hundred commands. Build a checked operation inventory and
move vertical slices in dependency order. Every frontend operation must end in one of:

- `remote`: supported by the shared service and remote adapter;
- `desktop-only`: has an intentional browser replacement or a disabled explanation;
- `internal`: unreachable from either client;
- `removed`: unused operation deleted after proof.

No unclassified operation passes the remote release gate.

### Web host

Add a `web-host` Cargo feature and `video-creater-host` binary. It:

1. loads host configuration from an app-data file with owner-only permissions;
2. initializes the same runtime dependencies and recovery paths as the desktop app;
3. binds an ephemeral or configured **loopback** port;
4. serves embedded production assets and `/healthz` without project data;
5. accepts application routes only from the trusted local Tailscale proxy;
6. emits the exact URL and a pairing code/QR in the terminal or desktop Settings;
7. drains jobs and sockets cleanly on shutdown.

Direct non-loopback binding is a developer-only escape hatch guarded by an explicit insecure flag;
it is not a supported product flow or an acceptance path.

### Protocol

Protocol version `v1` has these surfaces:

- `GET /api/v1/session`: connection state, capabilities, host label, protocol version, current
  editor lease, and CSRF token; no secrets or filesystem paths.
- `POST /api/v1/rpc/:operation`: a generated allowlist maps an operation to a typed service method.
- `GET /api/v1/events?after=<sequence>` upgraded to WebSocket: ordered event envelopes.
- `POST /api/v1/uploads`: bounded, streamed upload with declared length, MIME probing, cancellation,
  and staging cleanup.
- `GET /api/v1/media/:asset_id`: authorized byte-range streaming.
- `GET /api/v1/artifacts/:artifact_id`: authorized attachment download.

Successful RPC responses use `{ "ok": true, "result": ... }`. Failures use a stable code,
human-safe message, retryability, and optional field errors. Internal errors and host paths stay in
host logs. Each mutating request carries `requestId`, `projectId`, expected content revision when
applicable, and the editor-lease token. The host keeps a short idempotency record so a browser retry
cannot apply a mutation twice.

Event envelopes carry `sequence`, `event`, `projectId`, `occurredAt`, and `payload`. A reconnect
resumes after the last acknowledged sequence when retained; otherwise the host sends
`snapshotRequired` and the client reloads canonical state.

## Identity, Pairing, and Sessions

Tailscale network membership is necessary but not sufficient.

1. The host trusts identity headers only when the TCP peer is loopback and the configured proxy
   mode is Tailscale Serve. Client-supplied copies are discarded.
   The first release rejects requests with no user identity header, including tagged-device
   traffic. Users of a shared tailnet device still need explicit Video Creater pairing.
2. A new browser visits the host and sees a pairing screen.
3. The desktop UI or headless terminal displays a short-lived, single-use pairing code and QR.
4. Successful pairing creates a random device credential; only its hash and device metadata are
   stored by the host.
5. The response sets a `Secure`, `HttpOnly`, `SameSite=Strict` cookie. Session rotation and revoke
   are supported. Pairing codes expire after five minutes and are rate-limited.
6. State-changing HTTP requests require an exact allowed Origin and a session-bound CSRF token.
   WebSocket upgrade checks the same origin and session.

The UI lists paired devices, last use, tailnet identity, and revoke. Turning remote access off
closes listeners and sockets immediately but does not silently erase paired devices.

## Project and Concurrency Model

- Host configuration names allowed project roots. The browser sees project IDs and display names,
  never root paths.
- Opening a project validates that its canonical path remains beneath an allowed root after
  symlink resolution.
- One session owns the active editor lease for a project. The lease renews while connected and
  expires after a bounded disconnect grace period.
- Other sessions may view and follow progress. They cannot mutate until the owner releases the
  lease or they confirm takeover. Takeover is recorded and invalidates the previous token.
- Existing content revisions remain the final optimistic-concurrency check. A stale request returns
  `revision_conflict` and the client reloads; the server never guesses a merge.
- Background jobs survive browser disconnect and host-client lease changes.

## Browser Capability Differences

| Desktop behavior | Browser behavior |
| --- | --- |
| Native open-file dialog | Browser file picker and streamed upload |
| Native choose-folder export | Export into project/host destination, then download |
| Reveal in Finder/files | Download or show artifact details; no false “revealed” success |
| OS notifications | Browser notification only after browser permission; otherwise in-app task state |
| Local file drag/drop paths | Upload dropped file bytes |
| `asset://` / loopback media | Authenticated opaque media URL with Range support |
| Native menu | Existing responsive web controls and shortcuts |
| Settings that inspect host | Available, explicitly labeled “On <host>” |

The fixed editor layout remains unchanged. The disconnected shell becomes a focused connection and
pairing surface. On phones, pairing and connection state use a bottom sheet; the editor itself keeps
the existing phone layout. Remote status belongs in Settings and the existing top-bar connection
state, not a new permanent rail.

## Agent and Video-Pipeline Boundary

Remote access does not weaken the current production contract:

- the browser sends the user's edit intent to the host;
- Claude or Codex app-server runs on the host;
- the agent returns a structured proposal;
- Rust validates and classifies it before any canonical mutation;
- a generated edit still contains a real EDL with `sourceIn` and `sourceOut` before captions,
  overlays, titles, effects, or HyperFrames layers;
- render review still records duration, streams, caption/overlay timing, artifacts, and logs;
- paid, external, destructive, broad, and unclassified work still pauses for review.

The browser can never connect to, launch, or send arbitrary JSON-RPC to Codex app-server.

## Security Requirements

- Remote access defaults off and never silently changes firewall or Tailscale configuration.
- No `0.0.0.0` product default, public tunnel, host networking container, or wildcard CORS.
- No raw operation-to-function reflection. The RPC registry is explicit and tested.
- Request bodies, uploads, event buffers, and concurrent jobs have documented limits.
- Upload filenames are display metadata only; the host chooses staging paths.
- Media/artifact identifiers are scoped to both session and project and expire.
- Range parsing, percent decoding, symlinks, traversal, MIME confusion, and stale IDs have tests.
- Logs redact cookies, pairing codes, CSRF tokens, credentials, prompts marked private, and host
  paths returned to clients.
- Provider secrets stay in Keychain/Secret Service and are consumed only by host-side operations.
- Dependencies added for the gateway must satisfy the repository licence policy.
- Threat-model tests cover forged proxy headers, CSRF, cross-origin WebSockets, replayed request IDs,
  upload bombs, unauthorized media ranges, lease theft, and revoked sessions.

## Start and Stop Experience

### Development

```bash
rtk pnpm build
rtk pnpm host:dev
rtk tailscale serve --bg http://127.0.0.1:<reported-port>
```

The host prints the exact tailnet URL only after it verifies Tailscale Serve reaches `/healthz`.
If Tailscale is absent, it prints a blocked state and local diagnostic; it does not claim remote
readiness.

### Packaged desktop

`Settings → Remote access` contains Start/Stop, host name, verified URL, pairing QR/code, connected
devices, revoke, and recent security events. Starting it launches the in-process gateway on
loopback and verifies the proxy route before showing “Ready.”

### Headless host

`video-creater-host serve` starts the same service and prints structured readiness. It supports a
systemd user service, not a root system service. Secrets and state live in the user's app-data
directory with owner-only permissions.

## Failure and Recovery

- Network loss changes the client to reconnecting without discarding unsaved local interaction
  intent. Mutations are retried only with the same request ID.
- Host restart invalidates sockets, restores durable jobs, rotates ephemeral media URLs, and asks
  clients for a canonical snapshot.
- An expired editor lease makes the browser read-only until it reacquires or confirms takeover.
- Protocol mismatch shows “Update the host” or “Update this client”; it never attempts an unsafe
  partial fallback.
- A failed upload cleans its staging file. A completed upload not imported within 24 hours is
  recoverable cleanup inventory.
- Stopping remote access does not stop renders, transcription, generation, or agent turns already
  owned by the host.

## Acceptance Criteria

The capability is not complete until all of these pass with a production build:

1. On an Ubuntu host, start `video-creater-host`, expose it with Tailscale Serve, pair a second
   laptop, create/open a project, upload a fixture video, edit it, run one fixture-backed agent
   proposal through the real host boundary, render an MP4, and download it.
2. Repeat the core open/play/seek/edit/task/download flow at a 402×874 phone viewport on a real
   second device or a browser connected through the actual tailnet URL.
3. Video seeking sends correct byte ranges and does not transfer the full source for a short seek.
4. Disconnect during a render, reconnect, recover event state, and download the completed artifact.
5. A second paired client is read-only while the first holds the editor lease; confirmed takeover
   invalidates the first writer without corrupting the project.
6. A revoked device, forged identity header, missing/invalid CSRF token, foreign Origin, reused
   pairing code, traversal attempt, and unauthorized asset ID are rejected.
7. Desktop Tauri acceptance flows still pass through the Tauri adapter.
8. Remote browser acceptance flows pass without the DEV fixture marker and without Tauri globals.
9. Final MP4 review verifies duration, expected audio/video streams, caption alignment, overlay
   timing, artifact path on the host, and retained render log reference.
10. The readiness report records the host build, protocol version, bind address, verified tailnet
    URL, authentication mode, and checks above without recording credentials.

## Release Gates

- Unit and integration tests for the service, RPC registry, auth, sessions, editor leases, uploads,
  media ranges, event resume, and path authorization.
- Frontend tests for remote bootstrap, pairing, reconnect, read-only/takeover, browser file flows,
  capability fallbacks, and phone layout.
- Browser E2E against the real Rust host on loopback, followed by a retained cross-device Tailscale
  run. Fixture transport remains useful but cannot close this capability.
- Dependency and licence review, clippy for the new host feature, TypeScript lint/build, existing
  browser acceptance, and the relevant native release lane.
- Documentation for install, start/stop, pairing, revoke, Tailscale setup, recovery, logs, and the
  exact evidence boundary.

## Deferred Follow-ups

- Certificate-managed LAN mode for users without Tailscale.
- Multiple simultaneous writers or collaborative presence.
- Hosted relay/cloud mode.
- Push notifications while the browser is closed.
- Streaming low-resolution proxy generation for very high-bitrate remote sources.

## External References

- [Tailscale Serve](https://tailscale.com/docs/features/tailscale-serve): HTTPS reverse proxy,
  tailnet access, identity headers, header-spoofing defense, and the localhost-listener guidance.
- [Tailscale Serve CLI](https://tailscale.com/docs/reference/tailscale-cli/serve): current command
  syntax and the `127.0.0.1` proxy target constraint.
- [OpenAI Agents API architecture](https://developers.openai.com/api/docs/guides/agents-api/architecture):
  distinguishes an application server from the agent harness and execution environment. Video
  Creater's bundled Codex app-server remains a separate private stdio integration.
