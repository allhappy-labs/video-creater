# Verification gates and runner ownership

Discovery refreshed 2026-10-01. Repository: `olhapi/video-creater` on
`git.home.olhapi.com`; source gate definitions are in `package.json` and the
native/release scripts. No tracked GitHub or Forgejo workflows were found.
Read-only `fj repo view` succeeded and `fj actions tasks` returned **0 tasks**.
That is not proof that no external CI exists. A read-only unauthenticated
branch-protection API request returned 404; required status checks and merge
protection remain **unverified**. No external configuration was changed.

| Gate | Intended runner | Owner of definition | Current evidence / purpose |
| --- | --- | --- | --- |
| `verify:dev:frontend <files>` | Linux/macOS Node + pnpm | Frontend maintainers; `focused-development.mjs` | Two TypeScript configurations and selected unit files; development feedback |
| `test:development-policy` | Linux/macOS Node + pnpm | Tooling maintainers; focused and workload policy tests | Exact-filter enforcement, bounded metrics retention and deterministic workload correctness |
| `verify:frontend` | Linux/macOS with Chromium and fonts | Frontend maintainers; `package.json` / browser policy scripts | Source/tooling, policy, type, full unit, bundle, unused, browser and visual gates; fixture browser evidence |
| `verify:dev:native <names>` | Supported Linux/macOS native toolchain and default-feature dependencies | Native maintainers; `focused-development.mjs` | Incremental exact library tests, no zero-match success; development feedback |
| `verify:native:fast <names>` | Supported Linux/macOS native toolchain | Native maintainers; `run-native-verification.mjs` | Formatting, workspace check, exact library tests in bounded verification cache |
| `verify:native:release` | macOS with Xcode, native media runtime and shipped helpers | Native/release maintainers; `run-native-verification.mjs`, `run-native-rust-tests.mjs` | Three shipped Clippy profiles, serial workspace/all-targets tests, Metal/afconvert/filmstrip/Keychain/notification/AppKit lanes; Linux cannot sign this off |
| `verify:release` / `verify` / `verify:zero-debt` | macOS runner with browser and native prerequisites | Release maintainer; package composition | Frontend plus release-runtime/source policy and bounded native release coverage |
| `release:linux:preflight`, Linux native media/smoke policy | Linux with native runtime, supported desktop environment | Linux/release maintainers; Linux scripts | Runtime packaging and real desktop evidence; separate from portable frontend success |
| `release:macos:preflight`, signed helper/runtime policy, packaged acceptance | macOS with Xcode and authorized signing/notarization credentials | Release maintainer; macOS scripts | Signing, packaging and packaged native behavior; unavailable on Ubuntu |
| Live provider / Codex / remote-host evidence | Explicitly authorized fixture or paid/service-connected environment | Integration maintainers; provider/remote scripts | Opt-in; ordinary development and these baselines do not call paid providers |

These are repository responsibilities and supported runner classes, not claims that
the runners are currently provisioned. Before configuring CI, the maintainer must
confirm external job ownership, supported runners, branch protection and required
checks through authenticated Forgejo access. Keep unavailable-platform checks
explicitly unverified. Preserve the 20 GiB free-space floor, 24 GiB verification
cache cap and exact-path cleanup validation; never automatically clean the
development target. Any future third-party workflow action must use an immutable
commit SHA. External workflow, runner and branch-policy changes require authorization.
