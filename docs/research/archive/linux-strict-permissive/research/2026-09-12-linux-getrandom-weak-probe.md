# Linux `getrandom` weak-probe audit

- Date: 2026-09-12
- Scope: one failed Task 5b final-build attempt, static source-to-ELF audit only
- Decision: **the exact `getrandom` symbol is eligible for a narrowly conditioned
  weak-probe allowlist; final runtime and release qualification remain open**

## Result and boundary

The inspected ELF contains `getrandom` as an undefined weak symbol because the
pinned Rust 1.97.1 standard library deliberately probes for a libc wrapper and
falls back to a raw Linux syscall when the weak pointer is null. In this exact
static PIE, `getrandom` is absent from the dynamic symbol table, has no dynamic
relocation, and its GOT slot is zero. The selected code therefore calls the
linked musl `syscall` implementation with x86-64 syscall number 318. It does not
import GLIBC, load a library, or select musl's separate `getrandom.lo` member.

This supports allowing the exact weak symbol only when a fresh accepted build
reproduces all of those facts and binds the code to the verified Rust target
archive. It does not support a general `getrandom` exception, a wildcard weak
symbol rule, or permission for `pidfd_getpid` or `pidfd_spawnp`.

The inspected build is not qualifying evidence for Task 5b. Cargo completed,
but build-receipt validation failed because two typed ephemeral LLVM-IR aliases
had disappeared. The binary was not executed for this audit. Runtime tracing,
functional proof results, complete contributor classification, the full
per-file license/obligation audit, packaging, and release readiness remain
separate gates.

## Exact inspected artifacts

| Artifact | SHA-256 | Role |
| --- | --- | --- |
| `final-build/target/x86_64-unknown-linux-musl/release/linux-combined-media-proof` | `ae24339fa592624d2f3ea4fb4486bed8601a48beadd39762e1cabdcbfa9a0d7a` | Failed-attempt ELF; inspection only. |
| `final-build/maps/linux_combined_media_proof-738d17c2a3e95942-a771d95132f1b9ab.map` | `bf3597891877a11aee865834de93a2152f2189f2abadceb49f1c34a1a6d1b022` | LLD map selected by the receipt. |
| `final-build/receipts/link/link-e4772749072ac6e84686e529f0c323e2cd281ea4691dc24e87d7af2dd208fbeb.json` | `a87be11235b716d5ac22e4c55060e8a90e91181500615d60e8dbcb8a91231390` | Records a successful link and binds the output and map hashes. |
| `inputs/prepared-inputs-task3.json` | `db29d85fd2957f1f1ea1f0e77a6f322619d001e4e029db366a14b224f3c9ead9` | Retained prepared-input inventory; this derivation still needs the Task 5b gate. |
| `native-build-attempt4/native-build.json` | `ce89fb8f9fe9bc5584846db14cf7d294c83983242157eebf5af9efef335f263e` | Previously accepted native-build inventory used by the attempt. |

The link receipt invokes `-static-pie`, `-nodefaultlibs`, `-Bstatic`, `-lc`, and
the prepared musl and Rust library search roots through the pinned Clang/LLD
driver. The selected LLD is Ubuntu LLD 18.1.3, SHA-256
`7ad9a0e8fe6d0e79b71172d731e33872c0274e49fceb7b516d774876d5a58ade`;
it is a development-only tool, not a runtime input.

## Pinned Rust source and binary provenance

The prepared Rust compiler reports 1.97.1 commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`. The official target archive is
`rust-std-1.97.1-x86_64-unknown-linux-musl.tar.xz`, SHA-256
`51d83178680556f73a5fa8ad865b76a1ff541867445c00fc65dc67246bc2de66`.
Streaming its recorded member produces SHA-256
`8ff60d366af7bc26c8808031a1384bb3a681ee9a65f44cb35a655e7e4cfe143c`,
exactly matching the installed
`libstd-dfb742c400e8ef30.rlib`. Its sole code member,
`std-dfb742c400e8ef30.std.1872964b1103b53d-cgu.0.rcgu.o`, contains the same
undefined weak `getrandom` and Rust random-source symbols found after LTO in the
final main-crate object.

The pinned Rust sources show the complete behavior:

- [`library/std/src/sys/random/linux.rs`](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/library/std/src/sys/random/linux.rs#L69-L148), source-byte SHA-256 `a35f347a072df13548621dc8ce3b0e7daf96b202d799f622d169158dd1ea3f21`, declares `getrandom` through `syscall!`, retries supported error cases, and falls back to `/dev/urandom` if the syscall is unavailable or blocked.
- [`library/std/src/sys/pal/unix/weak/syscall.rs`](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/library/std/src/sys/pal/unix/weak/syscall.rs), source-byte SHA-256 `0f9c4e63d9f15b2486e8c0a9af89c4c2ad88fcad7c1a5127a95d8031e13ec2d3`, first checks the weak function pointer and otherwise calls `libc::syscall(SYS_getrandom, ...)`.
- [`library/std/src/sys/pal/unix/weak/weak_linkage.rs`](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/library/std/src/sys/pal/unix/weak/weak_linkage.rs), source-byte SHA-256 `e79493505702cbe6a647bdcbc6e728c50ed3353d2ea46e8ffcfb922e202c0b15`, declares the function as `extern_weak` and represents an absent definition as `None`.
- [`library/std/src/sys/pal/unix/weak/mod.rs`](https://github.com/rust-lang/rust/blob/8bab26f4f68e0e26f0bb7960be334d5b520ea452/library/std/src/sys/pal/unix/weak/mod.rs), source-byte SHA-256 `05ad90cd8408bd351535235f55e2b97b3259fe0eb764c7bb4c76af889309fbed`, selects true ELF weak linkage for this musl target. Its `dlsym` implementation is selected only for other configured cases, including GNU/Linux's private-symbol handling.

This source path is part of Rust `std`; no application helper declares or
implements the probe. The prepared inventory records `rust-std-musl` as
`MIT OR Apache-2.0`, and the checked-in Rust copyright/license snapshot has
SHA-256 `172020dbfd5b53a226dfde77616190a48dcff519b0bc0e6deb91a8450782c4af`.

## Actual ELF and musl resolution

GNU `nm --undefined-only --format=posix` reports exactly five weak undefined
symbols in the failed-attempt ELF:

```text
__cxa_finalize w
__deregister_frame_info w
__register_frame_info w
getrandom w
gettid w
```

`readelf -Ws` identifies `getrandom` as `NOTYPE WEAK DEFAULT UND`. The ELF is
x86-64 `DYN` static PIE with no `INTERP` program header and no `DT_NEEDED`
entry. More specifically for this probe:

- `.dynsym` contains only its mandatory null entry and `.dynstr` is one byte,
  so `getrandom` is not offered to the dynamic linker.
- `.rela.dyn` has no relocation at GOT address `0x5380e8`.
- Virtual GOT address `0x5380e8` lies in the load segment whose file offset is
  `0x2000` lower; its eight backing file bytes at offset `0x5360e8` are zero.
- Disassembly of `std::sys::random::linux::hashmap_random_keys` at `0x208160`
  compares that slot with zero. Its null branch loads immediate `0x13e` (318,
  x86-64 `SYS_getrandom`) and calls the resolved `syscall` slot.

The prepared Ubuntu musl 1.2.4-2 `libc.a` has SHA-256
`657e951f4c1d02ecac4e424222034c84a189f361b090f1284f255b3ac4480d5c`.
Its symbol inventory contains a strong `getrandom` in `getrandom.lo` and a
strong `syscall` in `syscall.lo`. The final LLD map contains
`libc.a(syscall.lo)` once, defining `syscall` at `0x523a60`, and contains no
`libc.a(getrandom.lo)` contributor. This agrees with the [System V ELF ABI weak-symbol rule](https://refspecs.linuxfoundation.org/elf/gabi4+/ch4.symtab.html): an archive member is not extracted to resolve an undefined weak symbol, and an unresolved weak symbol has value zero.

The corresponding [musl 1.2.4 `getrandom.c`](https://git.musl-libc.org/cgit/musl/tree/src/linux/getrandom.c?h=v1.2.4)
is a direct syscall wrapper, while [musl 1.2.4 `syscall.c`](https://git.musl-libc.org/cgit/musl/tree/src/misc/syscall.c?h=v1.2.4)
implements the generic syscall path that the map actually selects. Musl's
[upstream copyright file](https://git.musl-libc.org/cgit/musl/tree/COPYRIGHT?h=v1.2.4)
states the project-wide MIT license. The retained Ubuntu musl package lock also
classifies both `musl` and `musl-dev` 1.2.4-2 as MIT, with archive SHA-256 values
`9f0883c20b4b746e05e947bafd99cb933f5494ffaaa6fcd360cbe1fbcf264883`
and `4b451ecb6a0f8469883058cf22a807f3bd9cc16d115cc08b7efc35fe8eb44db2`.

Neither `nm` nor the map shows `dlopen`, `dlsym`, `dlclose`, or `dlerror` in the
final ELF. Combined with the absent interpreter, absent `DT_NEEDED`, empty
dynamic symbol set, and pinned Rust source selection, this excludes runtime
library lookup for this probe. The operating-system kernel syscall is an
independent OS service, not an app-linked or app-loaded library.

This is narrow source-term evidence for Rust `std` and the selected musl path.
It is not the complete per-file license and redistribution-obligation audit of
every final contributor.

## Recommendation for Task 5b

Add `getrandom` to the exact weak-probe allowlist only with verifier checks that
bind every accepted occurrence to all of the following conditions:

1. the symbol is `WEAK UND`, never strong;
2. it is absent from `.dynsym`, dynamic relocations, `DT_NEEDED`, and the map's
   loaded-library contributors;
3. the selected map identifies Rust `std::sys::random::linux` code in a
   receipt-bound generated object;
4. the contributing `libstd-*.rlib` is byte-identical to a member of the
   verified official Rust 1.97.1 musl target archive at the pinned commit;
5. the map includes the approved musl `syscall.lo` path and excludes
   `getrandom.lo`, GLIBC, GNU CRT, `libgcc`, and `libstdc++`; and
6. all existing receipt, contributor, runtime-process, functional, and evidence
   gates pass on the fresh qualifying build.

Do not force extraction of musl `getrandom.lo`, add an application shim, strip
the symbol, or rewrite the ELF solely to make `nm -u` clean. Those changes would
alter or hide the audited mechanism without improving its dependency boundary.
The smallest evidence-preserving change is the tightly conditioned allowlist
entry plus negative fixtures for a strong symbol, dynamic symbol/relocation,
unclassified source, or any `getrandom` occurrence outside the pinned Rust
standard-library path.
