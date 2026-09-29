# Linux Wayland native-client feasibility spike

Date: 2026-09-12. Branch: `feat/linux-permissive-runtime`.

## Decision

**Narrow result: pass. Native-desktop and license-closure result: partial.** One
development-only Rust client opened a real Unix connection to an independent
Wayland compositor, discovered and bound `wl_compositor`, `wl_shm`, and
`xdg_wm_base`, created an xdg-shell toplevel, submitted one CPU-generated
480×270 XRGB8888 `wl_shm` buffer, received a compositor frame callback, destroyed
its protocol objects, flushed the connection, and exited zero.

The tested client is a static musl PIE. It has no ELF interpreter, `DT_NEEDED`,
`RPATH`, or `RUNPATH` entry. Its captured live process mapped no shared-object
file and held only its PTY descriptors plus the Wayland Unix socket. This is
direct evidence that this client run did not dynamically load libwayland,
libX11, GTK, WebKit, or glibc.

This proves a small software-rendered Wayland client is feasible within the
approved independent-display-service boundary. It does not prove a Video
Creater UI, display on a physical or nested desktop, compositor screenshot
readback, keyboard or pointer input, fonts, clipboard, IME, accessibility,
portals, drag and drop, GPU rendering, desktop compatibility, packaging, or
full dependency certification.

The source, lockfile, vendored crates, binary, package extracts, link map, and
raw evidence are retained under ignored `output/linux-wayland-spike/`. They are
not production code and are not part of this commit. A Git checkout alone
therefore cannot reproduce this proof; preserve the ignored snapshot or promote
a reviewed copy in a separately scoped change.

## Candidate and service boundary

Exactly one client implementation was tested:

- Rust 1.97.1, commit
  `8bab26f4f68e0e26f0bb7960be334d5b520ea452`, targeting
  `x86_64-unknown-linux-musl`.
- [`wayrs-client` 1.3.1](https://docs.rs/crate/wayrs-client/1.3.1), exactly
  pinned. Its release documentation calls it a Rust implementation of the
  Wayland wire protocol, identifies `wayrs-client` as the socket/object/callback
  implementation, and explicitly says it has no libwayland compatibility.
- `wayrs-protocols` 0.14.11+1.45, resolved by the exact lockfile from the
  `=0.14.11` requirement with only `xdg-shell` enabled.
- No EGL, Vulkan, OpenGL, `wayrs-egl`, or shared-memory helper crate. The client
  writes the XRGB bytes to an ordinary private file with Rust `std`, transfers
  its descriptor through the Wayland protocol, and uses no client-side mmap.

The exact published wayrs packages declare MIT, and the retained package source
contains the upstream [MIT license](https://github.com/MaxVerevkin/wayrs/blob/main/LICENSE).
The [Wayland core protocol](https://wayland.freedesktop.org/docs/html/apa.html)
defines `wl_shm.create_pool` as transferring a file descriptor which the server
maps, requires the compositor to advertise usable formats, defines XRGB8888's
little-endian layout, and permits destroying the pool after buffer creation.

The development service was Weston 13.0.0-4build3 from privately extracted
Ubuntu packages. It ran the headless backend, Pixman renderer, and kiosk shell
on `/tmp/vc-wayland.aNJKEs/wayland-spike`. `WESTON_MODULE_MAP` pointed to the
privately extracted backend and shell. There was no network listener. Weston
and every library loaded inside its independent service process are explicitly
outside the application/helper graph under the selected option 1 boundary.
They are not proposed as shipped application dependencies.

## Exact execution result

The focused retained run reached its audit marker at `2026-09-12T10:33:52Z`.
The Weston process was PID 1586437 and the client was PID 1595016. The complete
client event log ends:

```text
global:wl_compositor:5
global:wl_shm:1
global:xdg_wm_base:5
shm-format:xrgb8888
toplevel-configure:800x600
xdg-configure:3
buffer-committed:480x270:xrgb8888
toplevel-configure:800x600
xdg-configure:4
frame-done:401432996
clean-exit
```

The compositor sent two configure sequences. The client acknowledged both and
submitted the buffer exactly once. The frame callback arrived after that
commit. While paused at `audit-ready`, the controller copied the exact process
maps, status, command line, and descriptor targets. It then created the requested
hold marker; the client destroyed the buffer, toplevel, xdg surface, Wayland
surface, and wm-base objects, flushed, wrote `clean-exit`, and exited zero. The
post-run assertion retained in `audit-state/verification.txt` is:

```text
PASS pixel-size=518400 globals=wl_compositor,wl_shm,xdg_wm_base format=xrgb8888 commit=1 frame-callback=1 clean-exit=1 process-gone=1
```

`audit-state/pixels.xrgb` is the 518,400-byte client buffer used to create the
`wl_buffer`. `audit-state/pixels.ppm` is a lossless client-generated view of
those same bytes. It is a **client buffer dump, not a compositor screenshot**.
The callback confirms the submitted surface's frame lifecycle; it does not
independently read back or validate compositor pixels. The headless run had no
real desktop and tested no input path.

## ELF, maps, and link inputs

The audited executable is:

```text
output/linux-wayland-spike/target/x86_64-unknown-linux-musl/release/linux-wayland-spike
2359392 bytes
SHA-256 678ac07074c1d0f44549655695079418834a2d03fb1ebd4c8e9e0e90533602da
ELF 64-bit LSB pie executable, x86-64, static-pie linked
Build ID ef88b866efcdc26cb25c0706d7908f4965f0ba4a
```

`readelf -l` recorded no `INTERP` program header. `readelf -d` contained no
`NEEDED`, `RPATH`, or `RUNPATH`; `ldd` reported `statically linked`; and
`nm -u` emitted zero lines. The retained `/proc/1595016/maps` contains four
file-backed ranges for that exact executable plus anonymous memory, heap,
stack, `[vvar]`, `[vdso]`, and `[vsyscall]`. It contains zero `.so` paths.
The retained descriptors were 0–2 on `/dev/pts/5` and descriptor 4 on one Unix
socket. No framebuffer backing file remained open after its descriptor was
transferred and the request flushed.

A separate offline build from the retained vendored source generated
`audit-state/linker-map.txt` without replacing the audited executable. The map
is 1,280,859 bytes, SHA-256
`d5589f32e82f1d228b96950ec56d5867bd7459ebca6086dcae462692733c4fc6`.
It identifies these external static toolchain inputs:

| Input | Evidence and current status |
| --- | --- |
| Rust standard-library and dependency code | LTO folded this into the application code-generation object. Rust's [linkage reference](https://doc.rust-lang.org/reference/linkage.html) explains that a binary links Rust and native dependencies and that the musl target defaults to a static C runtime. The exact toolchain copyright files are retained. |
| `libcompiler_builtins-f84c2cc06aceddaf.rlib` | Exact local archive SHA-256 `8a089f03dea685c0ad73d561540406f2ea0729788c162d76d17ea1832804a095`. Exact release source/notices were not reconstructed into a shipping SBOM. |
| `libc.a`, `rcrt1.o`, `crti.o`, `crtn.o` | The paths and object/member names are consistent with the rustup target's musl self-contained runtime. Exact hashes are retained in `static-toolchain-inputs.sha256`. Musl states that [musl as a whole uses the MIT license](https://git.musl-libc.org/cgit/musl/tree/COPYRIGHT), with permissive notices for incorporated material. The rustup artifact did not expose its exact musl source revision, so the path evidence is not treated as exact source correspondence. |
| `crtbeginS.o`, `crtendS.o` | Exact hashes are retained, but placement beside the musl target libraries does not establish whether these compiler CRT objects came from GCC, LLVM compiler-rt, or another source. No license is assigned from the directory name alone; exact source and license remain unresolved. |
| `libunwind.a` | Exact local archive SHA-256 `eb16569e611a5d6cdda2e7604fd6402a3ad810817dd8d63ed50baa2ce8070aaa`. LLVM libunwind uses [Apache-2.0 with LLVM exception](https://github.com/llvm/llvm-project/blob/llvmorg-22.1.0/libunwind/LICENSE.TXT); the rustup artifact did not expose its exact LLVM source revision. |

The live maps close the dynamic-library question for this run. The link map and
hashes close the static archive-name question for this build. They do not supply
release provenance, exact source correspondence, required notices, or a full
artifact SBOM.

## Cargo graph and declared licenses

`Cargo.lock` version 4 pins registry checksums. The complete musl build graph,
including procedural-macro build dependencies, contains these 11 third-party
packages:

| Package | Version | Declared SPDX expression | Role |
| --- | --- | --- | --- |
| `wayrs-client` | 1.3.1 | `MIT` | client wire implementation |
| `wayrs-core` | 1.0.5 | `MIT` | runtime marshalling and transport |
| `libc` | 0.2.189 | `MIT OR Apache-2.0` | Rust FFI declarations used by `wayrs-core`; distinct from musl |
| `wayrs-protocols` | 0.14.11+1.45 | `MIT` | generated xdg-shell protocol |
| `wayrs-scanner` | 0.15.4 | `MIT` | procedural macro, build only |
| `wayrs-proto-parser` | 3.0.1 | `MIT` | scanner dependency, build only |
| `quick-xml` | 0.37.5 | `MIT` | scanner dependency, build only |
| `memchr` | 2.8.3 | `Unlicense OR MIT` | scanner dependency, build only |
| `proc-macro2` | 1.0.107 | `MIT OR Apache-2.0` | scanner dependency, build only |
| `quote` | 1.0.47 | `MIT OR Apache-2.0` | scanner dependency, build only |
| `unicode-ident` | 1.0.24 | `(MIT OR Apache-2.0) AND Unicode-3.0` | scanner dependency, build only |

No package declares `links`, and no Cargo package in the graph declares LGPL.
The procedural-macro branch executes at build time and is not present as loaded
runtime code. Exact target metadata, tree, feature tree, checksums, and all 812
vendored files are retained.

**No `inventory-eligible` result was obtained.** This spike did not submit a
complete declared inventory to the checked-in evaluator. In particular, exact
rustup toolchain provenance and notices remain incomplete, `libunwind.a` raises
an allowed-license-selection question, and the complete build graph includes a
mandatory `Unicode-3.0` expression outside the current six-license runtime
allowlist even though that crate is build only. The manual source and metadata
screening in this report is not evaluator approval, a source-file scan, or full
artifact certification. The evaluator was not changed for this spike.

## Private compositor and cleanup

The retained service package set consists of privately downloaded `.deb` files;
no package was installed and no global linker configuration changed. The exact
Weston and libweston package hashes are:

```text
bcf6f344c6d5bede9f3f601be1015b0550fa31dd8c7b3e51ac0fe34702dac73c  weston_13.0.0-4build3_amd64.deb
ad6a150d0a21674a797091107617ddc9884a9c8b5a4edd1465636b419b05b88f  libweston-13-0_13.0.0-4build3_amd64.deb
```

Several smaller packages were also privately extracted to satisfy the packaged
Weston ABI. Their hashes and package controls are retained in
`compositor-evidence/`. System libraries resolved by the independent Weston
process are service internals and were not counted as client dependencies.

One early attempt used unprivileged `bwrap --ro-bind / / --unshare-net` to make
the package's compiled module paths visible. Weston then failed because the
desktop-shell helpers were absent and exited. An unrestricted root bind is
outside this project's VM policy; that approach was discarded and not repeated.
It supplied no proof result. The successful path ran Weston directly, used
private package extracts through process-local `LD_LIBRARY_PATH` and
`WESTON_MODULE_MAP`, and selected `--shell=kiosk` to avoid desktop-shell helper
launches.

After evidence capture, the exact wrapper PID 1586417 and Weston PID 1586437
were stopped. The socket was removed. `findmnt` and `/proc/self/mountinfo`
contained no path matching the spike, `host-libs-mount` was empty, and
`pgrep -x weston`/`pgrep -x bwrap` found no process. The harmless private
`wayland-spike.lock` file remains under `/tmp/vc-wayland.aNJKEs/`; it is not a
socket or mount.

## Retained artifacts

All paths are relative to the worktree and ignored by Git:

| Artifact | SHA-256 |
| --- | --- |
| `output/linux-wayland-spike/Cargo.toml` | `cba6894ca8e12044040065572e3a64183c78b9e455c79e1fa8f4e5abf553c593` |
| `output/linux-wayland-spike/Cargo.lock` | `0d77d77903e3428af70c5179c2f8d8490b964ff28102fda3fac859a82869dc8b` |
| `output/linux-wayland-spike/src/main.rs` | `c61525a739d38ab3b4837a5fbc3d4b4bc118e36cb5dbe984c79d9bf36975b363` |
| `output/linux-wayland-spike/tests/probe.sh` | `c23a9b8a13e5935bb9c45751d8b2a266dc48c3884e5007cfc329d9f5125ae22c` |
| `output/linux-wayland-spike/audit-state/events.log` | `c9dfd94b37d978cd64f68d6150fcd7eb3bd878daa7ba79a2ab84aecfb17073c9` |
| `output/linux-wayland-spike/audit-state/proc-maps.txt` | `a6593cf9fcc194da9f83abb610bd54701fdb9687c34b9d0c8064e49ac95f5563` |
| `output/linux-wayland-spike/audit-state/proc-fd-targets.txt` | `cb063be1761c662a34f762d648e1e1580edbd0d70f526f3f871367cc3debe946` |
| `output/linux-wayland-spike/audit-state/pixels.xrgb` | `1ee59f3a28d5f47fe5be89f240715534fb8536eb60dadcdc4a7e31f28d43ce10` |
| `output/linux-wayland-spike/audit-state/pixels.ppm` | `5953a3d9fd2344a145616acdfb44070203a10dc90a55de61ba6641c8a2d50dad` |
| `output/linux-wayland-spike/audit-state/linker-map.txt` | `d5589f32e82f1d228b96950ec56d5867bd7459ebca6086dcae462692733c4fc6` |
| `output/linux-wayland-spike/audit-state/cargo-metadata-musl.json` | `4ca07e9e940aec93e7f4d9313e402e6012a51ac8aca4d510e6f44bc9cefb6ceb` |
| `output/linux-wayland-spike/audit-state/vendor-files.sha256` | `36b04666128291554584f936e5a197267df8bb604b3a8415fb19ae731f51e163` |
| `output/linux-wayland-spike/compositor-evidence/cleanup.txt` | `eff191f26c2f6c857006f836b4f8149e0b51144c86d22452b44857baadaf4a91` |

## Reproduction

Run from the worktree. The repository requires the `rtk` prefix. The retained
vendor tree permits an offline client rebuild:

```bash
cd output/linux-wayland-spike
rtk env CARGO_TARGET_DIR=target-repro cargo build --release --locked --offline \
  --target x86_64-unknown-linux-musl \
  --config 'source.crates-io.replace-with="vendored-sources"' \
  --config 'source.vendored-sources.directory="vendor"'
```

The service packages are already extracted under `compositor-root/`. Use a
short private runtime path because Unix socket paths are length-limited:

```bash
for deb in compositor-debs/*.deb; do
  rtk dpkg-deb -x "$deb" compositor-root
done
rtk mkdir -m 700 /tmp/vc-wayland-repro
rtk env \
  XDG_RUNTIME_DIR=/tmp/vc-wayland-repro \
  LD_LIBRARY_PATH="$PWD/compositor-root/usr/lib/x86_64-linux-gnu:$PWD/compositor-root/usr/lib/x86_64-linux-gnu/weston" \
  WESTON_MODULE_MAP="headless-backend.so=$PWD/compositor-root/usr/lib/x86_64-linux-gnu/libweston-13/headless-backend.so;kiosk-shell.so=$PWD/compositor-root/usr/lib/x86_64-linux-gnu/weston/kiosk-shell.so" \
  "$PWD/compositor-root/usr/bin/weston" \
  --backend=headless --shell=kiosk --socket=wayland-spike --idle-time=0 \
  --renderer=pixman --width=800 --height=600 --no-config
```

With Weston running, start the client and inspect it before releasing the hold:

```bash
rtk env XDG_RUNTIME_DIR=/tmp/vc-wayland-repro WAYLAND_DISPLAY=wayland-spike \
  ./target-repro/x86_64-unknown-linux-musl/release/linux-wayland-spike \
  --state-dir repro-state --hold-file repro-state/continue
rtk cat repro-state/events.log
rtk cat /proc/CLIENT_PID/maps
rtk ls -l /proc/CLIENT_PID/fd
rtk touch repro-state/continue
```

Replace `CLIENT_PID` with the exact client PID. The full integration probe can
perform the wait, protocol assertions, hold release, and clean-exit assertion:

```bash
rtk tests/probe.sh \
  target-repro/x86_64-unknown-linux-musl/release/linux-wayland-spike \
  /tmp/vc-wayland-repro repro-state
```

This result supports further work on a custom native Wayland host. Production
selection still requires checked-in reviewed source, real compositor coverage,
desktop/input/portal behavior, the actual application rendering architecture,
clean-machine packaging, and an evaluator-backed artifact inventory with exact
toolchain provenance and notices.
