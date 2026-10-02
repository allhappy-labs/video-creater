# Codex helper staging investigation

Decision: defer a copy optimization. The measured warm helper is already close to
the cost of full-content verification, and this VM does not support copy-on-write
for the installed binary. No helper script or release behavior was changed.

Measurements use source SHA `167ea2df36930299b2a507f34034a8ff579e3080` with a dirty
workspace marker, Node 24.18.1, pnpm 10.30.3, Linux x86_64 on the Intel i5-12500 VM.
Existing caches were retained. Other agents' Cargo work overlapped these local
staging measurements; no complete helper preparation or Cargo build was started.

| Existing helper / investigation | First repeat | Second repeat |
| --- | --- | --- |
| `pnpm build:codex-sidecar:dev` | 1,099 ms | 639 ms |
| `node scripts/build-codex-sidecar.mjs --development` | 471 ms | 589 ms |
| SHA-256 of the source and both destination binaries | 513 ms | 499 ms |

The first complete preparation had a roughly 4.97-second Codex stage. That first
observation is not representative of the repeated warm stage above. Full hashes
alone cost about as much as the complete direct helper; adding them before a skip
does not demonstrate repeatable improvement. A `COPYFILE_FICLONE_FORCE` probe on an
owned temporary destination returned `ENOTSUP` on this ext4 filesystem. A fallback
copy would still perform the existing work. No hardlinks were introduced.

The existing helper validated package version `0.141.0` and selected
`x86_64-unknown-linux-gnu`, staging two independent 276,579,568-byte binaries plus
the `rg` resource. Full SHA-256 comparison verified both binaries equal to the
installed source; both destinations had mode `0755` and distinct source inodes.
The comparison read each file completely with a bounded 1-MiB buffer. No providers,
API calls, downloads, native builds, user-cache clearing or release signing ran.

Raw evidence uses the existing bounded latest-plus-twelve report writer:

- `output/build-metrics/codex-helper-stage-2026-10-01T12-00-48.255Z-791518.json`
- `output/build-metrics/codex-helper-stage-2026-10-01T12-02-41.760Z-792302.json`
- `output/build-metrics/codex-helper-copy-investigation-2026-10-01T12-03-30.749Z-792588.json`

For another machine, repeat `rtk pnpm build:codex-sidecar:dev` twice, then compare
the direct helper and a complete source/destination verification or CoW probe on
the same filesystem. Keep package version, target selection, development flags,
executable permissions and signed release staging as explicit constraints. Do not
skip using size/mtime alone or share a writable hardlink with the installed package.
Mac signing, packaged release behavior and other helper-stage invalidation remain
unverified by this Linux investigation.
