# Linux codec target-runtime proof

- Date: 2026-09-12
- Architecture tested: x86-64 Linux
- Decision: **functional codec/runtime gate passed; release artifact qualification remains open**

## Result and boundary

Pinned OpenH264 2.6.0 and libxaac 0.1.13 were rebuilt for
`x86_64-unknown-linux-musl` with a private Clang/LLD toolchain. Three Rust-owned
static PIE proof executables ran successfully: OpenH264 encoded and decoded four
deterministic I420 frames, and the distinct libxaac encoder and decoder processed
the retained AAC-LC fixture. Their final LLD maps have no contributing GCC
`crtstuff`, `libgcc`, `libstdc++`, or glibc artifacts.

This resolves the two concrete runtime failures in the earlier media proof:
GCC's `crtbeginS.o`/`crtendS.o` no longer enter the AAC binaries, and OpenH264 no
longer needs GNU libstdc++. It does not qualify these proof executables for
release. Twelve rustup-supplied Rust `.rlib` artifacts and two transient
rustc-generated `.rcgu.o` files contribute to each final binary. This spike has
no retained distribution manifest or source/build attestation that ties the
exact archives to reviewed source and license texts, and rustc removed the
transient objects before they could be hashed. They are identified below and
left unqualified. No license was inferred from their directory names.

This is a standalone investigation. It changes no production media code and
does not prove the broader import, timeline, container, color-conversion,
resampling, or packaging contract.

## Reviewed source and tool pins

| Source | Exact pin | Retained license evidence | Role and source-term result |
| --- | --- | --- | --- |
| OpenH264 | tag `v2.6.0`, commit `652bdb7719f30b52b08e506645a7322ff1b2cc6f` | `LICENSE` SHA-256 `dd5c1c9668512530fa5a96e4c29ac4033d70a7eeb0eed7a42fddb6dd794ebdbb` | Codec source; BSD-2-Clause. |
| libxaac | tag `v0.1.13`, commit `55af74f9ce7cadb99fbd7870c6487c8a12d75689` | `LICENSE` SHA-256 `e9bfdabc6c9e85a9bdae4c7194d5228ee10cc39603c57aa12cb03c36463d16ad` | AAC encoder and decoder source; Apache-2.0. |
| LLVM runtimes | tag `llvmorg-18.1.8`, commit `3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff` | libc++ `539dd7aed86e8a4f12cbdd0e6c50c189c7d74847e4fecc64ce2c6ee3a01da38b`; libc++abi `e2b35be49f7284a45b7baca8fc7b3ab7440e7902392b2528a457816b5bb2a15c`; libunwind `b5efebcaca80879234098e52d1725e6d9eb8fb96a19fce625d39184b705f7b6d`; compiler-rt `1a8f1058753f1ba890de984e48f0242a3a5c29a6a8f2ed9fd813f36985387e8d` | libc++ headers, C++ ABI, unwind, and compiler CRT source; exact combined term `Apache-2.0 WITH LLVM-exception`, approved separately in [the LLVM runtime license policy review](./2026-09-12-linux-llvm-license-policy.md). |
| musl packages | Ubuntu `1.2.4-2`, amd64 | package `copyright` SHA-256 `dd775ee077963e377a0ab31e1db27229442d5cfd18f183c2df1e6a552522f585` | Retained sysroot, libc and musl CRT; package text identifies musl as MIT with separately listed permissive/public-domain files. |

These are top-level source-license snapshots, not a complete obligation audit.
This spike did not trace every contributing source file or archive member to
per-file terms, inventory LLVM's separately licensed third-party material, or
derive and verify the complete notice set for a distributed artifact. The
retained libxaac `NOTICE` (SHA-256
`855075190a51406c8c0ae82b77b5a0083cf7a37248edf44a2db541b93d54783c`)
records Android Open Source Project copyright and Ittiam attribution, but
retaining and reading that file does not establish that all per-file,
third-party, attribution, and redistribution obligations are identified or
satisfied. That remains a release qualification gap.

The retained upstream LLVM instructions were read before selecting the runtime.
`libcxx/docs/BuildingLibcxx.rst` (SHA-256
`ced166854a67d120e1acbbc5ea85304b4589c5efc04de42260423234481afb06`)
defines the `runtimes` CMake build for libc++, libc++abi, and libunwind.
`libunwind/docs/BuildingLibunwind.rst` (SHA-256
`bdac51cc9099436a35b97d78207d81af72583a66ea8102dc8469921e6f708563`)
identifies CMake as supported and Clang as preferred.

Build-only Ubuntu 24.04 packages were downloaded and extracted privately; none
was installed or included in a proof executable:

| Tool package | Version | SHA-256 |
| --- | --- | --- |
| `clang-18` | `1:18.1.3-1ubuntu1` | `628b16701014ef7ad648380b20ea74b90dd543857f933b3e34d2fc042783de25` |
| `libclang-common-18-dev` | `1:18.1.3-1ubuntu1` | `d41631bb752815a8196261d1030c096591c59678c8adfbf2e6a740a691b49e80` |
| `lld-18` | `1:18.1.3-1ubuntu1` | `c574594d714bf445f4c2323fa99374c05b79be09a3899edbf9d4b7265c9ccc57` |
| `llvm-18-linker-tools` | `1:18.1.3-1ubuntu1` | `50f0bcfa784e42f8bf0eac31aeb6a20798fb6174cc31efe021f12ae71af91b23` |

The retained sysroot package hashes are
`4b451ecb6a0f8469883058cf22a807f3bd9cc16d115cc08b7efc35fe8eb44db2`
for `musl-dev`,
`46c01d212d3eb3a1322693089037f0a5c92383a089d39c392db3c86c19ffb229`
for `musl-tools`, and
`9f0883c20b4b746e05e947bafd99cb933f5494ffaaa6fcd360cbe1fbcf264883`
for `musl`.

## Target runtime build

The exact scripts and verbose transcripts are retained under ignored
`output/linux-codec-runtime-proof/`. CMake targeted
`x86_64-unknown-linux-musl`, used only the retained sysroot, disabled shared
runtimes, selected compiler-rt builtins, selected LLVM's unwinder for
libc++abi, enabled the static unwinder, and set libc++'s musl mode. Build
parallelism was capped at three.

The components actually required by the proof are:

| Artifact | SHA-256 | Final use |
| --- | --- | --- |
| `libc++abi.a` | `ac0f934d79a38dacc0a423ca20bbd38dc42c72e4acc7b8a35b4254ce5b972c34` | OpenH264 allocation, RTTI, guard, pure-virtual, exception-personality, and ABI symbols. |
| `libunwind.a` | `2c691679362ab4cdcca2c163d532e1a916fadeebfc1a26996c6b57375982bb04` | Unwinding required by Rust and OpenH264/C++ ABI inputs. |
| compiler-rt `crtbeginS.o` | `4a18df51efe689807a0855cb437a6f5accf6d8b0f5911770ec6c7de0eb6487b4` | Replaces rustup's unattributed `crtbeginS.o`, whose compiler comment reports GCC 9.4.0. |
| compiler-rt `crtendS.o` | `3763ada3d474034f5bc0b756f6ccd17d3586accd87ad3b62b73ed152bad064d2` | Passed after all final inputs, replacing rustup's unattributed `crtendS.o`, whose compiler comment reports GCC 9.4.0; it contributes no selected section in the LLD maps. |
| musl `rcrt1.o` | `e5867f7fccfa94384406ed3bf21a8db8756621f99d464c2c624de9f817291d7d` | Static-PIE process entry. |
| musl `crti.o` / `crtn.o` | `d92d0ca1f8ae3c6fa11ae848e93da11dac9f090981bb831d60c86ae99a9aff13` / `6152eb43c1a39390a105eb459701e2606c4f7c3c4b17b8b2cad83cb690cb5670` | musl init/fini framing. |
| musl `libc.a` | `657e951f4c1d02ecac4e424222034c84a189f361b090f1284f255b3ac4480d5c` | C/POSIX runtime, including musl's integrated pthread interfaces. |

The compiler-rt CRT objects were compiled directly from the pinned
`compiler-rt/lib/builtins/crtbegin.c` and `crtend.c`, whose hashes are
`60864582fef00bbbc11bef91805596ce906b70b3c4e0c5bf9f3f8d100cc77380`
and `6d5a61b8f2f6fa541a12ec52aea40b5085e40fcf8626984d318523cb30335d06`.
The exact flags were:

```text
--target=x86_64-unknown-linux-musl
--sysroot=<retained-musl-root>
-isystem <retained-musl-root>/usr/include/x86_64-linux-musl
-std=c11 -DCRT_HAS_INITFINI_ARRAY -DEH_USE_FRAME_REGISTRY
-fPIC -Wno-pedantic -O2
```

The musl CRT provenance was checked independently of compiler identity.
`dpkg-deb -c` places `rcrt1.o`, `crti.o`, and `crtn.o` in the exact retained
`musl-dev_1.2.4-2_amd64.deb`. Embedded DWARF names the sources
`crt/rcrt1.c`, `crt/x86_64/crti.s`, and `crt/x86_64/crtn.s` under
`/usr/src/musl-1.2.4-2`. `rcrt1.o` reports GCC 13.2 as its compiler, while
the assembly files report GNU `as`; those strings identify development tools,
not GCC runtime source. The rejected earlier objects instead came from GCC's
`gcc/crtstuff.c`. This distinction is based on package contents and embedded
source paths, not a containing directory or filename.

The aggregate libc++ build was deliberately bounded. Two sparse-checkout setup
errors were repaired by using `LIBCXX_INCLUDE_BENCHMARKS=OFF` and retaining
`llvm/utils/llvm-lit`; a musl locale error was repaired with
`LIBCXX_HAS_MUSL_LIBC=ON`. The final attempt built libc++abi and libunwind, but
full `libc++.a` stopped at:

```text
libcxx/src/atomic.cpp:19:12: fatal error: 'linux/futex.h' file not found
```

The retained musl sysroot has no Linux UAPI headers. Importing an unreviewed
host header tree would have weakened the proof, so the attempt stopped there.
OpenH264's selected objects require the C++ ABI and unwinder but no out-of-line
libc++ standard-library object, and no `libc++.a` appears in its final map. The
missing full archive is therefore a toolchain-completeness gap rather than a
blocker for this measured codec invocation.

## OpenH264 functional result

OpenH264 was rebuilt with the private Clang target driver, libc++ headers,
`USE_ASM=No`, and no system install. Its static archive is
`5e2bf20844cdf787c67f7e629f9d8866a6bfae78085eaee93fd0ae2d719291a6`.
A C++ harness generated four 64x48 I420 frames containing deterministic spatial
and temporal variation, encoded them at 30 fps, wrote Annex-B bytes, and fed
each access unit to an OpenH264 decoder. The run returned zero and reported:

```text
H264_PASS encoded_frames=4 decoded_frames=4 width=64 height=48 bytes=2332 y_min=0 y_max=255 y_sum=1566141
```

`file` recognizes the output as H.264/JVT baseline level 1.1. The executable is
a static PIE and has SHA-256
`381c1344082cc5b6d6621951603d4e057ef90c66823bab38fa81b4b7fcbaf532`;
the Annex-B stream hash is
`b5de82ef66b52f2d3f6f384203181e6b56c9c97b5441f8f7398b28933cbe9800`.
This is a same-library roundtrip. It is not independent bitstream-conformance,
profile-corpus, corrupt-input, AVCC conversion, reordering, or seek validation.

## libxaac functional result

The libxaac C archives and upstream testbench objects were rebuilt with the
same Clang/musl target. The encoder and decoder testbench `main` functions were
renamed to C symbols; minimal Rust entry points forward the original argv and
own each final rustc/musl link. The codec archives are:

| Artifact | SHA-256 |
| --- | --- |
| `libxaacenc.a` | `221cec84522eaf30b9722041521ea7383b6009b1011ed0b661ab6b1107199b0b` |
| `libxaacdec.a` | `422c5c3a126efbac7f846c0a6a0607f318ef81570c5dc9c4d763891cf71a692c` |
| encoder executable | `213b31b498ea856ccf1b4e286ea89f07b47b69d7724008a6e15f20b604fefd5c` |
| decoder executable | `170d48c2c10a152ff01d6798f69c2d1be7db85fb7860cd7c77b93b8bbe44aaaa` |

The retained source WAV hash is
`f7ed9ced39cd35af5d72d0b7db2faa60e64414d75be007233c1e0ff4af7b24c9`.
The exact successful invocation was:

```text
xaacenc-rust-musl -ifile:<sine_2ch.wav> -ofile:<output.adts> -br:128000 -adts:1 -aot:2
xaacdec-rust-musl -ifile:<output.adts> -ofile:<decoded.wav>
```

Both exited zero. A separate structural parser verified these measured shapes:

```text
input:   PCM16 stereo 48000 Hz, 480700 sample frames, 10.014583 s
ADTS:    AAC object type 2, stereo 48000 Hz, 470 ADTS frames,
         481280 coded sample frames, 10.026667 s
decoded: PCM16 stereo 48000 Hz, 481280 sample frames, 10.026667 s,
         sample min=-26214 max=26263 nonzero=956368
```

The independent ADTS parser counts 470 physical frames; the decoder log reports
`TOTAL FRAMES : [ 471]`. Decoded length equals `470 * 1024` samples. Relative to
the input, output length is 580 samples, or 12.083 ms, longer and remains within
one 1024-sample AAC frame. This is an output-length/padding bound only. Leading
encoder priming, trailing padding, content alignment, cross-correlation, and
gapless A/V synchronization were not measured.

The ADTS and decoded WAV hashes are
`15f061d25d8a600f836222414c14329e6cd8b3eaf06f2b4e7c891348c02c2098`
and `a09edeb24ee8e7f050a865539ac6c39c439705b845854e54de66ebd42e69e7bd`.
They match the earlier GCC-linked proof outputs, providing a useful
determinism check while the executables and codec archives differ.

## Final link and ELF audit

Rustc was `1.97.1 (8bab26f4f 2026-07-14)`, exact commit
`8bab26f4f68e0e26f0bb7960be334d5b520ea452`, targeting
`x86_64-unknown-linux-musl`. The logging linker wrapper replaces rustup's
self-contained `rcrt1.o`, `crti.o`, `crtn.o`, `crtbegin*.o`, and `crtend*.o`
with the retained musl and pinned compiler-rt objects before invoking the
private Clang/LLD driver. For OpenH264, `--no-dependent-libraries` prevents the
combined libc++abi archive's metadata from requesting a separate `pthread`
archive; musl supplies those interfaces in the explicitly linked `libc.a`.

The replaced rustup `crtbeginS.o` and `crtendS.o` hashes are
`297960e338581a38bbfcbee45169a847bcdc5527ccaf68abe95b72b3b0856bed`
and `0d11009c048ae289cdf184d726b767debc2972321e143838d28d7b6802b69c7c`.
Their `.comment` sections report GCC 9.4.0, but no retained evidence identifies
their source files or terms, so the report does not infer more. The final maps
prove that neither object contributes to the proof executables.

The three complete LLD maps were parsed into a unique contributing-artifact
inventory and every retained input was hashed. Results were 23 contributing
artifacts for H.264, 24 for AAC encode, and 25 for AAC decode, with zero path
matches for GCC CRT, libgcc, libstdc++, glibc, or the replaced rustup CRT files.
The maps explicitly select the pinned compiler-rt `crtbeginS.o`, musl
`rcrt1.o`/`crti.o`/`crtn.o`/`libc.a`, and source-built LLVM `libunwind.a`;
OpenH264 additionally selects source-built `libc++abi.a`. The link command
passes pinned compiler-rt `crtendS.o`, although garbage collection leaves it
without a selected output section.

All three executables are x86-64 ELF `DYN` static PIEs. Their dynamic sections
contain no `NEEDED` entries and their dynamic string tables contain only the
terminating byte. `nm -u` reports no strong unresolved symbol. It reports only
the five expected weak probes `__cxa_finalize`, `__cxa_thread_atexit_impl`,
`__deregister_frame_info`, `__register_frame_info`, and `gettid`.

The maps show the same 12 rustup `.rlib` archives contributing to all three
executables:

| Contributing rustup archive | SHA-256 |
| --- | --- |
| `libaddr2line-95e9a04e236b857f.rlib` | `709edae3937aae1b626899fab7599fb44595d61886647d1f1190d033b6e70aac` |
| `libadler2-e024d94ef6aa67ce.rlib` | `130b910eadcc9ca86ed4e1a05ad42ac8148ccfff5d0a80b09ce2d131a6423a35` |
| `liballoc-fb94d9d043f9fb44.rlib` | `75fbf231e9ae326ea0e4a0823d7083494e5c9e7567a962b8297deb709b97d5c5` |
| `libcompiler_builtins-f84c2cc06aceddaf.rlib` | `8a089f03dea685c0ad73d561540406f2ea0729788c162d76d17ea1832804a095` |
| `libcore-c02d9f641a39cc82.rlib` | `bf3446467f5e245df9d69444bd526519725d322943b03d54e58282be4a7f6e39` |
| `libgimli-92d6fd8bfac3132b.rlib` | `a24b43f9ab5473d118b7327e1570bca2a5a42ad3230ffef9af8f53cc5335a3e6` |
| `libmemchr-8b08ae4541584a89.rlib` | `166d75ed808e03ca6bea319c9feef233213dc6ee9c1c0b393cfd22b09f0dabaf` |
| `libminiz_oxide-274d4973cf8ab077.rlib` | `bbc07477ac102bae03ab621e4b0b56d03979b875be81b425dcd6ac4d298036a9` |
| `libobject-20635b902cc05745.rlib` | `93f827d7afbba006161fa6c5620037fc7098460e7c6ffb25269d847272b39ac7` |
| `libpanic_abort-a2cd76a07df7e60d.rlib` | `cf8c3cf2b71671b1e00122db691b9bb2904c5256ddf7f8dfd8f1e19c8759bee5` |
| `librustc_demangle-f6bc3c477c996d3e.rlib` | `958875ab7028fcda43ab2354bfd395f4228dca392692c1b7a72796a3d03ba6b9` |
| `libstd-dfb742c400e8ef30.rlib` | `8ff60d366af7bc26c8808031a1384bb3a681ee9a65f44cb35a655e7e4cfe143c` |

Each final map also names two contributing rustc-generated objects that were
deleted automatically after linking:

| Proof executable | Transient contributing objects from its final map |
| --- | --- |
| H.264 | `h264-roundtrip.306ls2zbxnennfetxa67dueja.rcgu.o`; `h264-roundtrip.h264_main.4b95d474c88679b-cgu.0.rcgu.o` |
| AAC encoder | `xaacenc-rust-musl.32eic4qqmxgfk1ux949ahzzu1.rcgu.o`; `xaacenc-rust-musl.aac_encoder_main.e5d8629d47215148-cgu.0.rcgu.o` |
| AAC decoder | `xaacdec-rust-musl.8v5m7pg3yao44yafzw0xdzp0l.rcgu.o`; `xaacdec-rust-musl.aac_decoder_main.93615a23d5e5c20c-cgu.0.rcgu.o` |

The map inventory records each exact link-time path as
`sha256=unavailable-temporary-link-input`. The retained Rust entry-point source
and rustc invocation explain how the objects were generated, but their exact
embedded bytes were not retained or hashed. They therefore remain separately
unqualified inputs alongside the 12 rustup archives. A release proof must
retain and hash them, or demonstrate a reviewed reproducible build that
regenerates and verifies them.

The link commands also pass other rustup `.rlib` archives from the target
distribution, but their members do not appear in the final LLD maps. The table
is limited to embedded contributions. The exact rustc commit and hashes make
the gap reproducible; they do not close it. A release recipe still needs a
pinned Rust distribution or source-built standard library whose manifests,
source revisions, license texts, build options, and resulting archive hashes
are retained and reviewed.

## Evidence and remaining gates

Ignored evidence is retained in `output/linux-codec-runtime-proof/`:

- `build-llvm-runtimes.sh`, `build-runtime-closure.sh`,
  `build-h264-proof.sh`, and `build-aac-proof.sh` preserve the exact recipes.
- `evidence/llvm-runtimes-build*.txt` retains every setup failure and the
  bounded full-libc++ failure; `evidence/runtime-closure-build.txt` retains the
  successful libc++abi/libunwind and compiler-rt build commands.
- `evidence/openh264-musl-build-2.txt`, `h264-proof-build-final.txt`, and
  `h264-roundtrip-run-final.txt` retain the codec build, complete link, and run.
- `evidence/aac-proof-build.txt` retains the initial sysroot include failure;
  `aac-proof-build-2.txt`, `aac-encode-run.txt`, `aac-decode-run.txt`, and
  `aac-stream-shape.txt` retain the repair, complete links, and measured run.
- `evidence/h264-final-link.map`, `aac-encoder-final-link.map`, and
  `aac-decoder-final-link.map` are the raw maps;
  `final-link-map-audit.txt` is the complete normalized input/hash audit, and
  `final-elf-and-crt-audit.txt` contains the ELF, undefined-symbol, comment,
  and CRT DWARF provenance inspection. `rustup-crt-inspection.txt` records the
  exact replaced rustup files, hashes, and compiler comments.

The codec/runtime experiment can advance to a combined proof once the Rust
runtime artifact gap is closed. Production selection still requires the MP4
timing/seek gate and a combined path that demuxes real MP4 H.264/AAC, converts
AVCC/Annex-B and codec configuration correctly, handles decoder presentation
order and seek, converts pixels, resamples/mixes PCM, muxes timestamped output,
measures AAC priming/padding alignment, validates malformed input, and exercises
a representative codec/profile corpus.

Software source terms do not settle H.264 or AAC patent clearance. The patent
and distribution caveats from the earlier feasibility report remain legal and
product decisions.
