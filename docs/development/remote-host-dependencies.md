# Remote host dependency record

The `web-host` Cargo feature directly enables:

- **Axum 0.8** (MIT): bounded HTTP routing and the Tokio listener integration.
- **tower-http 0.6** (MIT): production static-file service with SPA fallback.
- **Tokio 1.52** (MIT): async TCP, signal handling, and graceful shutdown.

Only HTTP/1, JSON, filesystem serving, tracing/header helpers, networking, runtime, and signal
features are enabled. The host binds loopback by default; TLS and remote exposure belong to the
Tailscale Serve boundary described in the remote-host design.
