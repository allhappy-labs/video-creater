# Linux audio host-service feasibility spike

- Date: 2026-09-12
- Checkout tested: `6564fc927c2a276bb1b6ee3f255822dbe9ad237d`
- Host: Ubuntu 24.04.4 LTS, Linux 6.8.0-139-generic, x86_64
- Decision: **pass for bounded native-protocol feasibility; not production-ready audio proof**

## Question and policy boundary

The approved option 1 boundary permits an independent OS audio service and does
not count that service's internal libraries as application dependencies. The
application, app-owned helpers, and everything they link or load remain subject
to the permissive-only rule. This spike tests only the application side of that
boundary. It does not propose bundling the development daemon.

The candidate was fixed before work began: [`pulseaudio` 0.3.1](https://github.com/colinmarc/pulseaudio-rs/tree/v0.3.1).
Its pinned manifest declares MIT, calls itself a native Rust PulseAudio protocol
implementation, and declares only Rust crate dependencies. The pinned README
lists playback and record examples and says shared-memory zero-copy is not
implemented. [Pinned manifest](https://github.com/colinmarc/pulseaudio-rs/blob/v0.3.1/Cargo.toml),
[pinned README](https://github.com/colinmarc/pulseaudio-rs/blob/v0.3.1/README.md).

PipeWire's first-party documentation says `libpipewire-module-protocol-pulse`
implements a complete PulseAudio server, normally exposed by `pipewire-pulse`,
and documents `unix:native` and absolute Unix socket addresses. That supports
the intended architecture, but this spike ran PulseAudio 16.1 itself and did
**not** execute PipeWire. [PipeWire Pulse protocol documentation](https://pipewire.pages.freedesktop.org/pipewire/page_module_protocol_pulse.html).

## Throwaway proof

The ignored `output/linux-audio-spike/` Cargo project pins
`pulseaudio = "=0.3.1"` and has a generated `Cargo.lock`. It builds for
`x86_64-unknown-linux-musl`. Through the crate's low-level API it opens a record
stream on `spike_null.monitor`, requests 100 ms s16le/48 kHz/stereo fragments,
waits 100 ms for activation, sends two seconds (96,000 frames) of generated 1 kHz
PCM to `spike_null`, then measures four seconds of monitor capture including an
exact periodic-pattern comparison.

The development server was a foreground PulseAudio 16.1 process extracted from
Ubuntu packages into ignored `output/linux-audio-spike/dev-service/`. It loaded
only `module-native-protocol-unix` with `auth-anonymous=1` and
`module-null-sink` configured as s16le/48 kHz/stereo. Its only listener was
`/tmp/video-creater-linux-audio-spike/native`; `ss -ltnp` showed no PulseAudio
TCP listener. D-Bus access was disabled for this process, and its runtime and
state directories were private. No package was installed and no existing user
audio service was started, stopped, or reconfigured. After evidence capture the
daemon was stopped; its socket and temporary socket directory were removed, and
no spike process remains.

The first handshake exposed one bounded compatibility detail. The crate README
example falls back to an empty cookie when no cookie file exists, but PulseAudio
16.1's `command_auth` parser requires the arbitrary cookie field to be exactly
`PA_NATIVE_COOKIE_LENGTH` before it checks whether the connection is already
authorized. The private anonymous client therefore sends a 256-byte dummy field;
the server ignores its contents under anonymous authorization. The requirement
is visible in the [PulseAudio 16.1 protocol source](https://github.com/pulseaudio/pulseaudio/blob/v16.1/src/pulsecore/protocol-native.c#L2353-L2367).

## Final measured run

The retained final run began at `2026-09-12 10:06:11+00:00`, exited zero, and
reported:

| Measurement | Result |
| --- | ---: |
| Negotiated protocol | 35 for playback and capture |
| PCM format | s16le, 48,000 Hz, 2 channels |
| Generated/sent | 96,000 frames / 384,000 bytes |
| Playback request messages | 20 |
| Captured | 192,000 frames / 768,000 bytes |
| Capture data messages | 41 |
| Capture fragment size | 19,200 bytes / 100 ms |
| Nonzero captured frames | 87,400 |
| Captured peak | 12,000, equal to generated peak |
| Measured 1 kHz amplitude over four-second capture | 5,699.93 |
| Longest exact periodic-tone run | 91,201 frames |
| Server underflow / overflow notifications | 1 / 0 |

The result proves that the static Rust client negotiated the native protocol,
sent synthetic PCM frames, and received the generated signal back through the
server's null-sink monitor. The four-second capture includes silence around the
two-second tone, so its whole-window 1 kHz amplitude is expected to be below the
generated 12,000 peak. The exact periodic run covers about 95% of the 96,000
sent frames, not all of them.

The server reported one underflow notification. This run therefore does not
prove uninterrupted or bit-exact whole-stream playback, underrun-free behavior,
speaker output, device discovery, end-to-end latency, synchronization, recovery,
or production robustness. The null sink and monitor cannot establish any of
those properties.

## Static artifact and live process evidence

The release artifact SHA-256 is
`582b2413830f9a6b00d488ba48b67ecf26a382b9a5e243b50004051d160e1601`.
`file` identified it as an x86-64 static PIE with build ID
`896c8c067fb38e1928f39fc5d17e4a6841b00ceb`; `ldd` reported
`statically linked`. `readelf -d` contained no `NEEDED` entries.

Live `/proc/663531/maps` was captured while that exact binary was holding after
the successful run. Its 15 mappings contain the proof executable, anonymous
memory, heap, stack, `vvar`, `vdso`, and `vsyscall`; no shared library or loader
file appears. This is direct evidence that the application artifact loaded no
native shared library in this run. It does not audit the separately running
PulseAudio service.

The ignored raw evidence retained beside the proof is
`final-live-run.typescript`, `client-proc-{maps,status}-final.txt`,
`{readelf,file,ldd}-transcript.txt`, `{tcp,unix}-listeners.txt`,
`cargo-target-tree.txt`, and `cargo-resolved-licenses.tsv`.
The proof source, lockfile, packages, binary, and raw evidence are all ignored; only this report is committed, so a Git checkout alone cannot reproduce the run.

## Cargo resolution and provenance

The lockfile contains the throwaway root plus 31 registry packages; the musl normal graph contains 26 third-party packages. Metadata includes MIT,
Apache-2.0, Unlicense, Unicode-3.0, and LLVM-exception expressions. The fixed
evaluator accepts only MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, and Zlib;
it does not accept Unlicense or Unicode-3.0 and rejects `WITH` syntax. Therefore
**no `inventory-eligible` or preflight result is claimed**. No native `links`
entry appears, but this manual metadata screen does not certify the full static
toolchain or native-binary license closure. Exact metadata and the target graph
are retained in `cargo-resolved-licenses.tsv` and `cargo-target-tree.txt`.

The locked `pulseaudio` package record is version 0.3.1 from the crates.io index
with checksum
`d70623bd7967a9ca4c2ae0e807fc380b291f98480fc037042305ec643a4d3373`.
Key spike hashes are:

| File | SHA-256 |
| --- | --- |
| `Cargo.toml` | `41e2ff8f2f70f479388c738c3e73b8d5818d60eaa6947ee5e7642ad1f4fbed42` |
| `Cargo.lock` | `c0a4170a28024a49aea24420ea2fa16ce0e5fec61b7c4bb23727491b1fbdd15b` |
| `src/main.rs` | `49a0676199a7bbe81609f0d9c554739d63a136df345fd9c506cede99eb0aa7de` |
| `dev-service/daemon.pa` | `0d798b0d6ccd077a0e88a99464a1f68c8d18f886a7c5a5f989fa5d00e48d2c90` |
| PulseAudio 16.1 Ubuntu `.deb` | `d26b78e296e1efab12e35a2674b8e698966efd9c194cf840f7b00f8eb64e38c1` |
| `libtdb1` Ubuntu `.deb` | `5b647b74d61b8bfe32f7bc03ffd22bac146a557df0724bd8e361b09f55237860` |
| `libspeexdsp1` Ubuntu `.deb` | `c16ccd12c2964ca5f30bd25670651550cce2d611cce2952c4807de44244da5a1` |

The `.deb` hashes pin only the disposable development service used for this
measurement. They are not application dependency approvals and are not release
artifacts.

Rust 1.97.1/Cargo 1.97.1 built the proof with
`rtk cargo build --locked --release --target x86_64-unknown-linux-musl`; the
retained transcripts record the exact run, `file`, `readelf`, `ldd`, `ss`, Cargo
tree, metadata, and hash commands and results.

This supports continued evaluation behind the independent-service boundary.
Production selection still needs a real PipeWire/Pulse desktop and hardware,
defined service/permission errors, latency, recovery, routing, hotplug,
suspend/resume, and A/V synchronization tests under the same dependency policy.
