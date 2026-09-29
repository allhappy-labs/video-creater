# Linux permissive native-host feasibility spike

Date: 2026-09-12. Checkout: `feat/linux-permissive-runtime` at `174c4d69`
before this report.

## Decision

**Narrow result: pass. Full Linux host result: partial and unverified.** A single
development-only Rust candidate created and mapped a real 640×360 X11 window,
sent a CPU-generated BGRA pixel buffer with X11 `PutImage`, received key and
button events, changed the rendered pixels after each event, read an explicitly
provided repository file, and exited through an X11 client message. The same
executable was a static musl PIE with no ELF interpreter or `DT_NEEDED` entries.
Its live process mapped no shared library files.

This establishes that the approved independent-display-service boundary can
support a very small native X11 client without GTK, WebKit, libX11, libxcb,
glibc, or another dynamically loaded GUI library in the client process. It does
not establish a usable Video Creater Linux UI, Wayland support, real keyboard or
mouse-device behavior, text and font rendering, clipboard, IME, accessibility,
drag and drop, file-picker portals, GPU rendering, audio, media handling,
packaging, or a production dependency certification.

The prototype source, lockfile, binaries, link map, logs, and captures are
retained under ignored `output/linux-host-spike/`. They are not production code
and are not included in this commit. Reproduction from Git alone is therefore
not promised: retain this worktree's ignored source snapshot, or promote a
reviewed version in a later explicitly scoped implementation.

## Candidate and boundary

The only candidate tested was:

- Rust 1.97.1 (`8bab26f4f68e0e26f0bb7960be334d5b520ea452`).
- `x86_64-unknown-linux-musl` static target.
- `x11rb = 0.13.2`, exactly pinned, with `default-features = false`.
- `x11rb::rust_connection::RustConnection`, which implements the X11 wire
  protocol in the client. The optional `allow-unsafe-code`/XCB FFI path was not
  enabled. The [0.13.2 manifest](https://docs.rs/crate/x11rb/0.13.2/source/Cargo.toml)
  declares `MIT OR Apache-2.0` and shows `libc`, `as-raw-xcb-connection`, and
  `libloading` as optional; the [upstream 0.13.2 documentation](https://docs.rs/crate/x11rb/0.13.2)
  explains that the `allow-unsafe-code` feature enables `XCBConnection` and its
  libxcb use.
- `/usr/bin/Xvfb :99 -screen 0 800x600x24 -nolisten tcp` as an independent
  development host display service. Xvfb and its process-internal libraries are
  outside the application/helper dependency graph under the selected option 1
  boundary. Xvfb is neither linked nor bundled into the candidate.

The standalone Cargo workspace lives at `output/linux-host-spike/` so it is
detached from the production workspace. No production source or manifest was
changed.

## Actual execution evidence

The second and final functional run used Xvfb PID 609300 and client PID 616920.
The client mapped X11 window ID 2097152. A separate invocation of the same static
executable acted as the protocol probe.

| Check | Observed result | Status |
| --- | --- | --- |
| Window | `GetGeometry` returned 640×360 for mapped window 2097152. | Passed |
| CPU pixels | Client filled a 921,600-byte BGRA buffer and sent it with `PutImage`; probe read pixels back with `GetImage`. | Passed |
| Key input path | Synthetic X11 `KeyPress`, keycode 65, was received and logged; accent pixel changed from `0x002dd4bf` to `0x00fb923c`. | Passed, synthetic protocol input only |
| Pointer input path | Synthetic X11 button 1 at window coordinates (140,130) was received and logged; accent pixel changed back to `0x002dd4bf`. | Passed, synthetic protocol input only |
| File access | Main process read `../../README.md`: 4,256 bytes, FNV-1a `c4eaff2ffee50ab8`, and used the digest to size a rendered bar. | Passed, direct path only |
| Capture | X11 `GetImage` produced P6 640×360 `window.ppm`; a controller converted it losslessly to RGB PNG and visually confirmed a dark framed/grid surface, cyan top bar, and purple lower bar. | Passed, spike graphic only |
| Quit path | Probe sent the private X11 client message; main process logged `quit-message`, destroyed its window, and wrote `stopped`. | Passed |

Probe output:

```text
probe-ok window=2097152 geometry=640x360 pixel-before=0x002dd4bf pixel-after-key=0x00fb923c pixel-after-button=0x002dd4bf
```

Event log:

```text
file-read path=../../README.md bytes=4256 fnv1a=c4eaff2ffee50ab8
window-mapped id=2097152 width=640 height=360
key-press detail=65 state=0
button-press detail=1 x=140 y=130
quit-message
```

No physical keyboard or pointing device was attached to Xvfb. This proves event
decoding, dispatch, state mutation, and redraw over the real X11 connection; it
does not prove desktop focus, input-method, compositor, or hardware input behavior.

## ELF and live-process evidence

Tested binary:

```text
output/linux-host-spike/target/x86_64-unknown-linux-musl/release/linux-host-spike
590384 bytes
SHA-256 4b0bf45eb64051b0eacf535745874900b3b0e021ed7734c251ed372165200bde
ELF 64-bit LSB pie executable, x86-64, static-pie linked, stripped
Build ID 5b351ed05221555d0b1423083b4798478e1cb44d
```

`readelf -lW` had no `INTERP` program header. `readelf -dW` had no
`NEEDED`, `RUNPATH`, or `RPATH` entries. `ldd` reported `statically linked`.
The raw `/proc/616920/maps` output from the functional run was inspected but not
retained, so the retained artifacts do not support reconstructing that PID's
map after exit.

A focused follow-up audit ran the byte-identical tested binary on private Xvfb
display `:101` at 2026-09-12 10:01 UTC. The independent display-server PID was
657025 and the client PID was 657224. Raw `/proc/657224/maps` and an ELF/fd
transcript were retained before stopping both exact processes. The complete
file-backed mapping set was the tested executable itself; the other mappings
were anonymous memory, heap, stack, `[vvar]`, `[vdso]`, and `[vsyscall]`. File
descriptors 0–2 were the controlling PTY and descriptor 3 was the Unix-domain
X11 socket. There were no runtime shared-object mappings. This focused audit
did not repeat the already-passed pixel, input, file-read, or quit checks.

A separate linker-map build produced a byte-identical executable and recorded
these static native/runtime archive inputs:

| Link input | Evidence and license status |
| --- | --- |
| Rust standard-library code | LTO folded Rust `std`, `core`, allocation, and Cargo dependency code into the application code-generation object. The exact Rust 1.97.1 `std` manifest declares `MIT OR Apache-2.0`; the [Rust project copyright statement](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/COPYRIGHT) notes third-party materials and is the authoritative starting point for a later shipping audit. |
| `libcompiler_builtins-f84c2cc06aceddaf.rlib` | Link map names the Rust sysroot archive. The exact toolchain revision's [compiler-builtins source](https://github.com/rust-lang/rust/tree/8bab26f4f68e0e26f0bb7960be334d5b520ea452/library/compiler-builtins) is part of the Rust source distribution. Exact binary provenance still needs a release SBOM/notices check before shipping. |
| `self-contained/libc.a` plus `rcrt1.o`, `crtbeginS.o`, `crti.o`, `crtendS.o`, `crtn.o` | Link map identifies musl object members such as `__libc_start_main.lo`, allocation, file, socket, and polling functions. Upstream states that [musl as a whole uses the MIT license](https://git.musl-libc.org/cgit/musl/tree/COPYRIGHT), with permissively licensed incorporated portions. The installed rustup artifact did not expose its exact musl source revision. |
| `self-contained/libunwind.a` | Link map identifies five x86_64 LLVM libunwind object members. LLVM declares [Apache-2.0 with LLVM exception](https://github.com/llvm/llvm-project/blob/main/llvm/LICENSE.TXT), and libunwind source files carry that SPDX identifier. The installed rustup artifact did not expose its exact LLVM source revision. |

The link map is `output/linux-host-spike/runtime-state-2/link.map`, 1,496,371
bytes, SHA-256 `b924fadbaca959468bbaf48c8bfcd299e2ead802432a3562bd3488e66728109b`.
It closes the static archive-name question for this binary. It does not replace
release provenance, notices, source-file license scanning, or a reproducible
toolchain/SBOM gate.

## Cargo graph and declared licenses

`Cargo.lock` is version 4 and pins registry checksums. `cargo tree --locked
--target x86_64-unknown-linux-musl` selected this external target graph:

```text
x11rb 0.13.2
├── gethostname 1.1.0
│   └── rustix 1.1.4
│       ├── bitflags 2.13.2
│       └── linux-raw-sys 0.12.1
├── rustix 1.1.4
└── x11rb-protocol 0.13.2
```

Complete `cargo metadata --locked` registry package inventory, including
target-specific packages not selected by the musl build:

| Package | Version | Declared SPDX expression | Selected in musl target graph |
| --- | --- | --- | --- |
| `bitflags` | 2.13.2 | `MIT OR Apache-2.0` | Yes |
| `errno` | 0.3.14 | `MIT OR Apache-2.0` | No |
| `gethostname` | 1.1.0 | `Apache-2.0` | Yes |
| `libc` | 0.2.189 | `MIT OR Apache-2.0` | No; this is the Rust crate, distinct from statically linked musl |
| `linux-raw-sys` | 0.12.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Yes |
| `rustix` | 1.1.4 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | Yes |
| `windows-link` | 0.2.1 | `MIT OR Apache-2.0` | No |
| `windows-sys` | 0.61.2 | `MIT OR Apache-2.0` | No |
| `x11rb` | 0.13.2 | `MIT OR Apache-2.0` | Yes |
| `x11rb-protocol` | 0.13.2 | `MIT OR Apache-2.0` | Yes |

The metadata expressions offer permissive license choices, but **no
`inventory-eligible` result was obtained for this Cargo graph**. The checked-in
evaluator rejects `WITH LLVM-exception` while parsing an expression, so it rejects
the raw `linux-raw-sys` and `rustix` expressions even though each also offers
plain Apache-2.0 or MIT alternatives. The manual source and metadata screening
in this report is not evaluator approval and does not override that fail-closed
behavior. This spike did not change the evaluator or submit a declared
inventory. The table is the complete Cargo metadata result for the retained
lockfile, not an independent source-file or packaged-artifact license
certification.

## Retained snapshot and artifacts

All paths below are relative to the repository worktree and are ignored by Git:

| Artifact | SHA-256 |
| --- | --- |
| `output/linux-host-spike/Cargo.toml` | `d8b9fff73c75c451e63e3fc21da911f15daa07ef773603e1456e3bda7a715874` |
| `output/linux-host-spike/Cargo.lock` | `868a23f919499eafc8d1181b30f2507b0c2ca8dbb8a25a281135ada9cd96c523` |
| `output/linux-host-spike/src/main.rs` | `47f306b423eaea82b99903c42e133e8a12d5892baca92cf3d054418666c8f356` |
| `output/linux-host-spike/runtime-state-2/events.log` | `ff91dfc25afb0075b40137072dec8fc74b8f1f5e902f083f1331cafc8b1d10b6` |
| `output/linux-host-spike/runtime-state-2/window.ppm` | `3ced3e078768fb97267492f693eddd9b09a07f3ea6080cb33c2313436a519fcf` |
| `output/linux-host-spike/runtime-state-2/window.png` | `ec05931b7694873b93b2a23c470ee7f48fed57d163054621e3c301e06d50d9d6` |
| `output/linux-host-spike/runtime-state-2/link.map` | `b924fadbaca959468bbaf48c8bfcd299e2ead802432a3562bd3488e66728109b` |
| `output/linux-host-spike/audit-state/proc-maps.txt` | `0662351c2d171f3806cd2e31a0906f749f6c0ba5d3c731c3680d4d8c425be037` |
| `output/linux-host-spike/audit-state/elf-and-fds.txt` | `960f84e62fa4b57dbcb1139ca3469b82c7a5ff0bb9fcef118da12668371e5b38` |

The source snapshot totals 410 lines across `Cargo.toml`, `Cargo.lock`, and
`src/main.rs`. Preserve those three files to rebuild after removing the target
caches. Deleting the ignored `output/linux-host-spike/` directory deletes the
only retained prototype source.

## Reproduction commands

Run from the repository worktree. The repository requires the `rtk` prefix.
The first command is a reversible development-toolchain setup step.

```bash
rtk rustup target add x86_64-unknown-linux-musl
rtk cargo build --manifest-path output/linux-host-spike/Cargo.toml --locked --release --target x86_64-unknown-linux-musl
```

In terminal A, start the independent local display server:

```bash
cd output/linux-host-spike
rtk Xvfb :99 -screen 0 800x600x24 -nolisten tcp
```

In terminal B, start the candidate:

```bash
cd output/linux-host-spike
rtk env DISPLAY=:99 ./target/x86_64-unknown-linux-musl/release/linux-host-spike --state-dir runtime-state-repro --open ../../README.md
```

While terminal B is running, inspect the exact app PID, ELF, and mappings:

```bash
cd output/linux-host-spike
rtk file target/x86_64-unknown-linux-musl/release/linux-host-spike
rtk readelf -lW target/x86_64-unknown-linux-musl/release/linux-host-spike
rtk readelf -dW target/x86_64-unknown-linux-musl/release/linux-host-spike
rtk ldd target/x86_64-unknown-linux-musl/release/linux-host-spike
rtk pgrep -f '^./target/x86_64-unknown-linux-musl/release/linux-host-spike --state-dir runtime-state-repro'
rtk sed -n '1,240p' /proc/APP_PID/maps
rtk ls -l /proc/APP_PID/fd
```

Replace `APP_PID` with the exact `pgrep` result. In terminal C, run the probe;
it captures the image and sends the clean quit message:

```bash
cd output/linux-host-spike
rtk env DISPLAY=:99 ./target/x86_64-unknown-linux-musl/release/linux-host-spike --probe runtime-state-repro
rtk sed -n '1,160p' runtime-state-repro/events.log
rtk sha256sum runtime-state-repro/window.ppm
```

Generate a separate link-map build without replacing the tested target:

```bash
cd output/linux-host-spike
rtk env CARGO_TARGET_DIR=target-link-map RUSTFLAGS=-Clink-arg=-Wl,-Map=runtime-state-repro/link.map cargo build --locked --release --target x86_64-unknown-linux-musl
rtk cargo metadata --locked --format-version 1
rtk cargo tree --locked --target x86_64-unknown-linux-musl
```

## Remaining gates

This spike supports further design work on a custom native host. It does not
justify selecting this 299-line prototype as the production host. A production
decision still needs a scoped UI architecture and tests for the omitted desktop
capabilities, real X11 desktop and Wayland strategy, secure portal integration,
distribution packaging, clean-machine execution, and exact notice/SBOM and
binary-provenance review for the pinned Rust toolchain and every bundled asset.
The Linux media pipeline remains a separate feasibility problem.
