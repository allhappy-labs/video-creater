# Linux bundled runtime: permissive dependency findings

Date: 2026-09-12. Scope: primary-source licensing and capability screening for a Linux app that bundles its tools and uses permissively licensed dependencies. This is research for the compatibility specification, not a completed dependency audit, legal clearance, or runtime verification. Linked default branches must be replaced by immutable revision and artifact records before selection.

## Confirmed policy update

The later option 1 selection permits independent OS services and excludes their
internal libraries from the application audit. App-owned helper processes and
all libraries linked/loaded by them remain subject to the permissive-only rule.
Read the earlier findings below with that distinction: the former service-closure
blocker is superseded, while GTK/WebKitGTK/glibc loaded by the app remain excluded.

The user explicitly rejected LGPL dependencies **including system libraries** on
2026-09-12. Options below that require a system-library exception are rejected,
not pending clarification. Standard Tauri Linux is therefore excluded. The GNU C
Library is also LGPL and fails this policy; see the [GNU project declaration](https://www.gnu.org/s/libc/).
A permissive libc replacement alone does not qualify an application: every helper,
native library, loaded driver, and service client code still requires review.
The [specification](../superpowers/specs/2026-09-12-linux-compatibility-design.md)
now requires that desktop/runtime proof before implementation planning.

## Findings that affect architecture

The existing class of multimedia stack cannot simply be packaged under a strict permissive-only policy. GStreamer is LGPL; GStreamer Editing Services is LGPL; FFmpeg's combined license is at least LGPL, even though some source files are permissive. Removing GPL codecs does not make these libraries permissive. GStreamer's Rust bindings being MIT/Apache-2.0 does not change the native library license. [GStreamer licensing](https://gstreamer.freedesktop.org/documentation/frequently-asked-questions/licensing.html), [GES](https://gstreamer.freedesktop.org/documentation/gst-editing-services/), [FFmpeg license](https://github.com/FFmpeg/FFmpeg/blob/master/LICENSE.md), [Rust binding license distinction](https://gstreamer.freedesktop.org/documentation/rust/git/docs/gstreamer_editing_services/index.html).

Tauri's documented Linux prerequisites include WebKitGTK. GTK is LGPL, and WebKit contains both BSD and LGPL portions; WebKitGTK's API reference identifies BSD and LGPL-2.1. Therefore an all-permissive bundled Linux UI cannot assume the standard Tauri GTK/WebKitGTK stack qualifies. This conclusion is an architectural inference from the dependency and license declarations, not a claim that proprietary applications cannot legally use LGPL. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [GTK](https://www.gtk.org/), [WebKit licensing](https://webkit.org/licensing-webkit/), [WebKitGTK license](https://www.webkitgtk.org/reference/webkit2gtk/stable/index.html).

The spec must state its host-platform boundary explicitly. The kernel, graphics driver, window system, audio service, and system ABI are different from app-supplied tools. Leaving a required codec or application webview on the host is not evidence of compliance with either “all tools bundled” or an unrestricted “permissive libs only” requirement. AppImage does not settle the license question; Tauri itself documents glibc compatibility constraints and GStreamer multimedia bundling. [Tauri AppImage documentation](https://v2.tauri.app/distribute/appimage/).

## Candidate components, not a validated replacement stack

| Function | Primary-source evidence | Decision implication |
| --- | --- | --- |
| H.264 encode/decode | [OpenH264](https://github.com/cisco/openh264) supports Linux and encode/decode, with a [BSD-2-Clause license](https://github.com/cisco/openh264/blob/master/LICENSE). The README documents constrained-baseline capabilities and known limits. | Credible permissive software candidate. Prove the selected revision's actual input profiles, dimensions, color formats, seek behavior, and output quality; do not promise broad phone/camera format coverage from the library name. |
| AAC encode/decode | Ittiam's [libxaac](https://github.com/ittiam-systems/libxaac) documents CMake targets including x86_64 and encoder/decoder builds; its [license](https://github.com/ittiam-systems/libxaac/blob/main/LICENSE) is Apache-2.0. [Encoder API documentation](https://github.com/ittiam-systems/libxaac/blob/main/README_enc.md) explicitly includes AAC-LC, HE-AAC, and other audio object types. | AAC is not proven impossible under permissive licensing. Evaluate a pinned libxaac build for AAC-LC, sample rates, channels, delay metadata, quality, and robustness. Codec patent/distribution evaluation remains separate. |
| Alternate AAC encoder | [VisualOn vo-aacenc](https://github.com/mstorsjo/vo-aacenc) is an Apache-2.0 AAC encoder implementation. | Secondary candidate only; this research has not established comparative quality, maintenance sufficiency, or suitability. |
| MP4 mux/demux | [minimp4](https://github.com/lieff/minimp4) provides MP4 mux/demux under CC0-1.0. | Screen the precise license against policy; validate edit lists, timestamps, rotation metadata, fragmented MP4, malformed inputs, and large-file support before adopting. A container library alone supplies no codec or editor. |
| VP8/VP9 | The WebM project identifies [libvpx as its VP8/VP9 SDK](https://www.webmproject.org/code/); the [official mirror](https://github.com/webmproject/libvpx) identifies BSD-3-Clause. | Strong codec candidate for WebM proxies and exports; benchmark practical CPU encode/decode. |
| WebM container | [libwebm license](https://github.com/webmproject/libwebm/blob/main/LICENSE.TXT) is BSD-style with three conditions; [project overview](https://www.webmproject.org/code/) identifies the library. | Candidate container component; it does not replace timeline composition or guarantee compatibility with every preview path. |
| Opus audio | [Opus licensing](https://opus-codec.org/license/) identifies BSD-3-Clause encoder/decoder and separate royalty-free patent grants. | Strong audio candidate for WebM. Do not bundle the whole opus-tools package unquestioned: upstream identifies opusinfo as GPLv2. |
| Local transcription | [whisper.cpp license](https://github.com/ggml-org/whisper.cpp/blob/master/LICENSE) is MIT; [its README](https://github.com/ggml-org/whisper.cpp) documents word-level timestamps as experimental. | Candidate bundled CPU speech helper. Test word alignment against actual editor fixtures; experimental timestamps are not proof of parity. Avoid its FFmpeg-dependent karaoke generation example and prebuilt images that include FFmpeg. |
| Speech model weights | [OpenAI Whisper's license section](https://github.com/openai/whisper#license) explicitly places code and model weights under MIT. | Pin the exact source model, conversion recipe, resulting checksum, tokenizer, and license. Third-party converted or fine-tuned models require their own provenance review. |
| ONNX inference | [ONNX Runtime license](https://github.com/microsoft/onnxruntime/blob/main/LICENSE) is MIT. | Engine eligibility does not establish any loaded model's eligibility or the eligibility of every enabled execution provider and binary dependency. Pin and audit engine, providers, and each model independently. |

## OpenH264 bundling and patent coverage are separate

Cisco says source users are responsible for applicable licensing fees and that its binary royalty arrangement requires the module to be downloaded at install time from Cisco. Therefore a project-built OpenH264 binary can be considered under the BSD software-license policy, but cannot be described as carrying Cisco's binary royalty coverage. A first-run or install-time Cisco codec download also fails the requested fully bundled/offline tool contract. The spec needs an explicit distribution/patent gate for any bundled H.264/AAC choice; permissive source licensing alone is insufficient evidence of that clearance. [Cisco FAQ](https://www.openh264.org/faq.html).

## Choices that should not pass a permissive-only gate

- FFmpeg/GStreamer/GES: LGPL or stronger, as documented above.
- GTK/WebKitGTK: LGPL or mixed LGPL/BSD, as documented above.
- Symphonia: [its upstream license section](https://github.com/pdeljanov/Symphonia#license) states MPL-2.0, so “pure Rust” does not make it permissive.
- FDK-AAC: [the actual NOTICE](https://github.com/mstorsjo/fdk-aac/blob/master/NOTICE) contains custom Fraunhofer terms; do not treat it as an ordinary MIT/BSD/Apache dependency or auto-approve it by name.
- An entire browser distribution, inference binary, container image, or tool suite: a top-level permissive license is not a transitive artifact audit.

## Required evidence before implementation selection

1. An exact accepted-license allowlist and an explicit host OS boundary; copyleft and custom/unknown licenses fail closed without changing policy implicitly.
2. A complete shipped-file and linked-library inventory: runtime executables, shared/static dependencies, loaded plugins, browsers, fonts, models, tokenizers, and tool resources. Record source revision, build options, hashes, license expressions, notices, and any patent/distribution decision separately.
3. A feasibility spike proving a permissive desktop shell or proving a deliberately approved alternative policy. No compliant Tauri/WebKitGTK replacement was validated by this research.
4. A native media-worker spike proving demux, decode, seek, frame delivery, resample/mix, timeline composition, caption/overlay rasterization, encode, and mux using only approved components. The table above is not proof that these form a working editor.
5. A clean-machine offline test with no development tools, no host FFmpeg/GStreamer/Node/Python/Chromium dependency, no executable discovery from PATH, and no first-run tool/model download. Host platform services must match the documented boundary.
6. Measured codec/profile and transcript fixture results. Unsupported formats must produce explicit errors; do not silently substitute WebM-only compatibility for an approved MP4/H.264/AAC requirement.

No Linux package, codec integration, graphical runtime, speech model, or macOS regression was built or tested for this note.
