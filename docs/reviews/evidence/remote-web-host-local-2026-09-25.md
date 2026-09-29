# Remote web host local acceptance — 2026-09-25

## Result

**Implemented and verified in local browsers. Tailnet-device verification is blocked.**

The production Rust host completed the authenticated editor workflow at desktop and phone
viewports on loopback. This machine did not produce a current shareable URL because its Tailscale
daemon is stopped and starting the system service requires interactive host authentication. No
MagicDNS name, tailnet address, pairing code, cookie, CSRF value, identity header, credential, or
private project path is retained here.

## Build under test

- Source commit: `d3d08ff361a60b7ac5208a68744c4dd1b3f0ce80`
  (`feat(remote): ship production web host`)
- Application version: `0.1.0`
- Remote protocol: `1`
- Package class: Linux x86-64, 278 files, 337 MiB
- Package location: `output/remote-host-package/linux-3451661e032bbcf9d529a468ad23f69a/`
- Host binary SHA-256: `b467b6d502a071edb069258a74b08935107dad3102193ea8e9ae4fdd49c038e9`
- Codex sidecar SHA-256: `1f37cb63f6c8c3e5e9dddb247a822706f153c00e553efe8c0b05f62d0e4f1cab`
- Bundled `rg` SHA-256: `ebeaf56f8a25e102e9419933423738b3a2a613a444fd749d695e15eba53f71f2`
- Package manifest SHA-256: `e1cc556e9a59d03faf610496cf9f52af53521eb97817eddcaa3374f4cb7ce766`
- Final package smoke: `/healthz` returned `status=ok`, protocol `1`, build `0.1.0`; the hashed
  production application shell returned successfully. The packaged host also launched its exact
  sibling Codex 0.141.0 sidecar, completed an app-server `initialize` handshake in isolated state,
  and reaped the child process.

## Browser workflow

`rtk pnpm test:remote-host` passed 2 of 2 tests against the real Rust host in 42.4 seconds:

| Client class | Viewport | Verified workflow |
| --- | --- | --- |
| Desktop browser | 1440×900 | Pair, create project, upload WebM, apply a real host EDL proposal, seek canonical media, reload, reopen, render while disconnected, reconnect, recover task state, download MP4 |
| Phone browser | 402×874 | Pair, create project, upload WebM, apply a real host EDL proposal, edit, render, monitor completion, download MP4 |

The run used no Tauri globals and no frontend fixture transport. Both downloads matched their
host artifacts byte-for-byte. Each retained host render report proved:

- duration greater than 0.7 seconds and less than 1.0 second;
- video and audio streams present;
- stream, duration, artifact-path, and log-reference checks passed;
- MP4 output larger than 1 KiB and a readable host render log;
- captions and overlays were not part of this EDL-only acceptance case, so their timing was not
  applicable.

## Automated gates

- Frontend: 275 test files, 2,685 tests passed.
- Remote Settings: 51 focused Settings/accessibility/integration tests passed.
- Remote browser: 2 Playwright tests passed.
- Fixture browser regression: 72 Playwright tests passed after isolating the real-host specs to
  their dedicated server configuration.
- Release visual QA: 7 expected frames, zero mismatches, manifest and baseline integrity passed.
- Rust host boundary: 47 tests passed across authentication, sessions, pairing, identity trust,
  editor leases, events/resume, HTTP, media ranges, project catalog, tickets, RPC, startup,
  Tailscale classification, uploads, traversal, symlink, disk reserve, replay, hard-bounded
  idempotency, cancellation, and rate limits. A browser-discovered transaction-snapshot catalog
  race was reproduced from the retained trace, fixed, regression-tested, and the full browser flow
  was rerun successfully.
- Rust quality: `cargo fmt --check` and host-feature `cargo clippy -- -D warnings` passed.
- Frontend type/lint and production Vite build passed.
- 121 source-quality policy tests and Knip passed.
- Packaging policy test and release package build passed, including the staged Codex initialize
  smoke rather than only a `--version` check.
- Production dependency license enumeration completed with permissive licenses in the installed
  dependency set.

The npm vulnerability query was not completed: registry egress for transmitting dependency
metadata was not approved in this session. `cargo-audit` and `cargo-deny` are not installed. This is
an explicit verification gap, not a passing result.

`systemd-analyze --user verify` could not bind a private user-bus socket in this sandbox and the
package is intentionally not installed into the sample unit's final home-directory path. Static
unit/package policy tests passed; clean-account instructions and `StateDirectory` creation are
documented, but a live installed-user-unit start remains part of the tailnet/operator acceptance.

## Tailnet boundary

The final detector result was `daemon_stopped`, `healthVerified=false`, with no URL. Attempts to
start the existing system service reached the host authentication boundary and could not proceed
without an interactive administrator credential. Consequently, this report does **not** claim:

- a current verified Tailscale Serve URL;
- a physical second laptop or phone run;
- macOS packaging, signing, notarization, or Safari/WebKit verification.

After an operator starts Tailscale, follow
[Remote access](../../development/remote-access.md), start the packaged user service, configure the
exact Serve proxy, and require `state=ready` plus `healthVerified=true` before sharing the URL.
