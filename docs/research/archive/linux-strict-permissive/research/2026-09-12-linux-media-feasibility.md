# Linux permissive H.264/AAC MP4 feasibility

- Date: 2026-09-12
- Task baseline: `7306d731ba3b949f3016349d376283ac8a441652`
- Final report parent: `e040d2e44aa42e40f80f2b2066aff6c192609fea`
- Scoped media-source diff between those revisions: empty
- Host: Ubuntu 24.04.4 LTS, Linux 6.8.0-139-generic, x86_64
- Decision: **partial proof; the evaluated stack is not ready to replace the production media pipeline**

## Scope and result

This spike evaluated one CPU stack for option 1: OpenH264 2.6.0 for H.264,
libxaac 0.1.13 for AAC, pinned minimp4 source for ISO BMFF, and rubato 0.16.2
for sample-rate conversion. Application helpers and everything linked or loaded
by them remain inside the permissive-only boundary. An OS-service exemption
cannot turn a codec helper into an exempt dependency.

AAC-LC encode and decode both passed in native, static-musl executables. Those
proof executables are not policy-clean artifacts: their GCC-driven final link
embedded `crtbeginS.o` and `crtendS.o` from GCC. A minimal rubato resample
invocation also passed as static-musl. The complete stack is blocked by two
architecture findings:

1. OpenH264's source is BSD-2-Clause, but its C++ archive retains required C++
   ABI/runtime symbols and its build explicitly adds `-lstdc++`. GNU
   libstdc++ is GPLv3 plus the GCC Runtime Library Exception, outside the
   current permissive-only runtime rule. A static LLVM C++ runtime closure is
   a plausible separate investigation, but was not built or inventoried here.
2. minimp4's demuxer reads and discards `ctts` composition offsets and does not
   parse or expose `stss` sync samples. It therefore cannot provide correct
   presentation timing for B-frame input or keyframe seeking without a
   substantive extension.

These are architecture gates. Do not begin the large GStreamer/GES replacement
until a complete H.264 runtime and MP4 timing/seek proof passes.

## Existing application contract

The replacement has to cover more than four codec entry points:

- `compatibility-worker/src/main.rs` probes input, extracts a frame, and
  transcodes incompatible media; its pipeline currently supplies demux,
  decode, conversion, resampling, parsing, and muxing.
- `precompose/frame_source.rs` seeks by source time and emits timed CPU RGBA
  frames after scale and frame-rate conversion.
- `precompose/audio_denoise.rs` and `audio_sync.rs` decode sources to PCM/WAV.
- `render_pipeline/gstreamer_backend.rs` performs timeline trim/speed,
  composition, audio conversion/resampling, H.264/AAC parsing, and MP4 muxing.
- `render_pipeline/probe.rs` requires container, duration, video codec,
  dimensions, frame rate, and audio codec for final validation.

A viable import path must parse movie/track metadata, `avcC` and `esds`, sample
sizes/offsets, decode timestamps, composition timestamps, durations, sync
samples, and edit lists; seek to a preceding random-access sample; convert AVCC
samples to the decoder's expected bitstream; decode/reorder video by
presentation time; decode AAC with its AudioSpecificConfig; convert I420 to
RGBA; and convert/resample PCM. Export needs RGBA-to-I420, H.264 encode, PCM
mix/resample, AAC-LC encode, SPS/PPS and AudioSpecificConfig handling, sample
timing/keyframe flags, MP4 mux/finalization, and the existing probe checks.

## Pinned candidate matrix

| Layer | Encode evidence | Decode/import evidence | License and status |
| --- | --- | --- | --- |
| [OpenH264 2.6.0](https://github.com/cisco/openh264/tree/v2.6.0), commit `652bdb7719f30b52b08e506645a7322ff1b2cc6f` | `WelsCreateSVCEncoder`, `Initialize`, and `EncodeFrame` accept CPU I420 and emit H.264 NAL data. | `WelsCreateDecoder` and `DecodeFrameNoDelay` expose decoded Y/U/V planes. AVCC-to-Annex-B adaptation, SPS/PPS injection, profile corpus, corrupt input, and seek/reorder behavior remain unproved. | BSD-2-Clause source. **Rejected for current runtime closure** because the archive requires a C++ ABI/runtime. |
| [libxaac 0.1.13](https://github.com/ittiam-systems/libxaac/tree/v0.1.13), commit `55af74f9ce7cadb99fbd7870c6487c8a12d75689` | The encoder API is specifically `ixheaace_create`, `ixheaace_process`, and `ixheaace_delete`; its documented AOT 2 is AAC-LC. Static-musl AAC-LC encoding passed. | The distinct decoder is the command-based `ia_xheaacd_dec_api`; it accepts AAC-LC and exposes 16/24-bit PCM configuration. Static-musl ADTS decoding passed. Direct MP4 ingestion did not: the app still must demux access units and configuration. | Apache-2.0. **Technically promising for both AAC directions**; proof executables include GCC CRT, and app integration, corpus, delay, and inventory tests remain. |
| [minimp4 commit `5a212a1`](https://github.com/lieff/minimp4/tree/5a212a18dba7dca09543bbc7d65619274fd2931a) | `MP4E_open/add_track/put_sample/set_dsi/set_sps/set_pps/close` cover basic H.264/AAC muxing. Output integration was not built. | `MP4D_open` and `MP4D_frame_offset` expose indexed samples and decode timestamps. `ctts` offsets are discarded and `stss` is unavailable, so common H.264 timing and random-access seeking are incomplete. | Exact source carries CC0-1.0 and has no release tag. **Requires explicit additional-license review and substantial demux work.** |
| [rubato 0.16.2](https://github.com/HEnquist/rubato/tree/v0.16.2), commit `c20dfb7fa80fa43d0781312be67efe81f2607219` | No codec role. Chunked f32/f64 sample-rate conversion supports the needed PCM stage. | Same. It does not provide channel layout policy, mixing, AAC delay handling, or WAV/container parsing. | MIT; minimal locked static-musl invocation passed. |
| Pixel/color/scale code | Required RGBA-to-I420 before OpenH264. | Required I420-to-RGBA plus crop/scale after decode. | **No candidate selected or built.** This remains a real implementation and performance gap, not implied OpenH264 functionality. |

The [OpenH264 API header](https://github.com/cisco/openh264/blob/v2.6.0/codec/api/wels/codec_api.h)
documents both encoder and decoder calls and I420 encoder input. The pinned
[libxaac encoder](https://github.com/ittiam-systems/libxaac/blob/v0.1.13/README_enc.md)
and [decoder](https://github.com/ittiam-systems/libxaac/blob/v0.1.13/README_dec.md)
documents confirm that these are separate APIs; the library name alone is not
the capability evidence. The pinned [minimp4 header](https://github.com/lieff/minimp4/blob/5a212a18dba7dca09543bbc7d65619274fd2931a/minimp4.h)
is the source for its mux/demux API and the timing limitations above.

## Actual build evidence

OpenH264 was built only as a host-glibc-compiled static archive, with assembly
disabled because NASM was unavailable:

```text
make OS=linux ARCH=x86_64 USE_ASM=No BUILDTYPE=Release libopenh264.a
exit_status=0
SHA-256 3f36a1d2a21d4bf126a872b9e051df3f4b57eef1579982ad23fe91d2a94c11bd
```

This is build and symbol evidence, not a musl codec run. After subtracting
symbols defined elsewhere in the archive, unresolved externals include
`_Unwind_Resume`, `__gxx_personality_v0`, C++ RTTI vtables,
`operator new/new[]/delete`, `__cxa_guard_*`, and `__cxa_pure_virtual`.
OpenH264 also uses `dynamic_cast`, and its pinned Makefile sets
`STATIC_LDFLAGS=-lstdc++`. The [GNU libstdc++ license page](https://gcc.gnu.org/onlinedocs/libstdc%2B%2B/manual/license.html)
confirms GPLv3 plus the runtime exception. Merely making the codec archive
static does not remove that loaded or embedded runtime requirement.

For libxaac, Ubuntu's musl 1.2.4-2, musl-dev, and musl-tools packages were
downloaded and extracted into the ignored spike directory; no system package
was installed. A private GCC specs wrapper pointed only at that musl sysroot.
Both upstream static libraries and testbench executables built with `-static`.
The executable link maps list GCC's `libgcc.a` and `libgcc_eh.a` as loaded by
the specs; neither archive contributed a member. The same maps prove that
GCC 13's `crtbeginS.o` contributed `.text`, init/fini arrays, data, and BSS,
while `crtendS.o` contributed unwind metadata. Those GCC runtime objects remain
outside the permissive-only policy even though both binaries have no undefined
symbol and no dynamic section. A release proof must link the libxaac C archives
with the eventual reviewed Rust/musl toolchain and inventory the final output.
Both objects came from Ubuntu `libgcc-13-dev` 13.3.0-6ubuntu2~24.04.1; its
copyright file identifies `gcc/crtstuff.c` as GPL-3-or-later with the GCC
Runtime Library Exception. Their respective SHA-256 hashes are
`667ff5f1edc931d44d1f2bb44d313907aa274bcc207c0beff71a398335f0d29f` and
`9c8b2caad195193301ef072895dc89bea1d23371d4c19accd821debe27773063`.

The retained AAC-LC run used the upstream 48 kHz stereo sine fixture:

```text
xaacenc -ifile:sine_2ch.wav -ofile:libxaac-musl-lc-128k.adts \
  -br:128000 -adts:1 -aot:2
xaacdec -ifile:libxaac-musl-lc-128k.adts \
  -ofile:libxaac-musl-lc-decoded.wav -pcmsz:16
encode_exit_status=0; decode_exit_status=0; total_frames=471
```

`file` identifies both executables as statically linked x86-64 ELF files, the
encoded file as AAC-LC ADTS 48 kHz stereo, and the decoded output as 16-bit
stereo PCM WAV at 48 kHz. Key SHA-256 values are:

| Artifact | SHA-256 |
| --- | --- |
| `libxaacenc.a` | `6fc542500dab087652f4579f42344893cbeb0aefb9c37c19ff46105a0a686fde` |
| `libxaacdec.a` | `05c70d39f0370f888d5a5325b8d539d2dd27871462ccc510cc7b3b44405d3966` |
| `xaacenc` | `6af1abb62a94412a981ebe905d9f6a0740d3b7bf1688f10788170ff226f10cbb` |
| `xaacdec` | `70a653022528dcb59320ae0080932429cf065f59c576b588bb3f0545dad83c9d` |
| encoded ADTS | `15f061d25d8a600f836222414c14329e6cd8b3eaf06f2b4e7c891348c02c2098` |
| decoded WAV | `a09edeb24ee8e7f050a865539ac6c39c439705b845854e54de66ebd42e69e7bd` |

The rubato throwaway pins `=0.16.2`, disables default features, and has a
generated lockfile. `cargo build --locked --release --target
x86_64-unknown-linux-musl` produced a static PIE; the one-chunk call reported
`PASS input_frames=441 output_frames=409 nonzero=409`. It proves invocation and
signal production only. It did not flush filter delay, compare fidelity, or
test whole-clip duration. Resolved packages are rubato 0.16.2 (MIT), num-traits
0.2.19 and num-integer 0.1.47 (MIT OR Apache-2.0), and autocfg 1.5.1
(Apache-2.0 OR MIT).

The next bounded container screen should start with `alfg/mp4-rust` as an
unqualified lead. Its current [`track.rs`](https://raw.githubusercontent.com/alfg/mp4-rust/master/src/track.rs)
reads `ctts` into a rendering offset and `stss` into an `is_sync` result, which
addresses the two minimp4 gaps. No revision, license, dependency graph, build,
malformed-input behavior, or app contract was verified here, so this is not a
selected replacement.

## License, patent, and evidence limits

The exact [minimp4 license](https://github.com/lieff/minimp4/blob/5a212a18dba7dca09543bbc7d65619274fd2931a/LICENSE)
is CC0-1.0: it provides a broad copyright/database-right waiver with a public
license fallback, while explicitly granting no patent rights. CC0 is not one
of the fixed evaluator's six initial IDs. It may be accepted only through an
explicit source-license review plus evaluator/inventory support; this report
does not silently treat it as eligible.

Software-license compatibility does not settle codec patents. Cisco's
[OpenH264 FAQ](https://www.openh264.org/faq.html) says a product distributing
its own source-built codec is responsible for applicable license fees. Cisco's
covered binary requires download at product installation, which conflicts with
the self-contained/offline requirement. libxaac's Apache-2.0 patent grant is
limited to contributor-licensable claims necessarily infringed by their work;
AAC standards-pool and product-distribution clearance remains a legal decision.

No `inventory-eligible`, preflight, release-qualified, broad format coverage,
performance, quality, A/V sync, seek accuracy, malformed-media robustness,
codec conformance, encoder-delay compensation, or production result is claimed.
The AAC executions completed too quickly for a retained live `/proc` map; the
ELF, dynamic-section, undefined-symbol, and final-link-map transcripts are the
runtime-closure evidence gathered here. The embedded GCC CRT finding shows why
absence of `NEEDED` and undefined symbols is insufficient. Manual
metadata/symbol inspection does not certify full compiler, static toolchain,
native binary, or notice closure.

All cloned sources, lockfiles, private toolchain packages, binaries, link maps,
and raw transcripts are ignored under `output/linux-media-feasibility/`; only
this report is committed. The evidence is therefore not reproducible from Git
alone. No production pipeline file was changed.
