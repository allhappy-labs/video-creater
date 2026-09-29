# Option 1: audio and Wayland client screening

Date: 2026-09-12. Source screening by a Sol worker; not a runtime or complete
transitive-license audit. The [current spec](../superpowers/specs/2026-09-12-linux-compatibility-design.md)
permits independent OS services while forbidding LGPL in application processes.

## Audio recommendation

Prototype the Rust `pulseaudio` crate 0.3.1 directly before adopting a higher-level
audio framework. Its pinned manifest declares MIT and Rust dependencies, and
upstream provides playback/recording examples. The client communicates with the
native server protocol; it does not wrap the LGPL libpulse client library.
The source explicitly lacks shared-memory/memfd zero-copy support, so copying,
underruns, latency, and seek behavior need measurement.
[Manifest](https://github.com/colinmarc/pulseaudio-rs/blob/v0.3.1/Cargo.toml),
[upstream documentation](https://github.com/colinmarc/pulseaudio-rs/blob/v0.3.1/README.md).

PipeWire provides a PulseAudio-compatible server, so this is a candidate for
ordinary PipeWire desktops as well as PulseAudio desktops. Compatibility is not
established until the pinned client is tested against both selected servers.
[PipeWire Pulse protocol documentation](https://docs.pipewire.org/page_module_protocol_pulse.html).

The investigated CPAL Linux configuration includes ALSA, and Rodio documents an
ALSA dependency. Those default paths must not be accepted based solely on their
permissive Rust wrapper licenses. Any future alternative configuration requires
its own resolved graph and loaded-library audit.
[CPAL manifest](https://github.com/RustAudio/cpal/blob/master/Cargo.toml),
[Rodio Linux dependencies](https://github.com/RustAudio/rodio#dependencies-linux-only).

## Wayland candidate architecture

A candidate software-rendered client uses:

- `wayland-client` 0.31.x with its Rust backend, without a system-library backend.
- `smithay-client-toolkit` with default features disabled and only required event
  loop functionality enabled.
- A pure-Rust XKB implementation such as `kbvm` 0.1.5 for keyboard processing.
- `wl_shm` buffers for CPU-generated pixels.

These are source-screened candidates, not an approved exact dependency set. Pin
versions/features and prove the complete target graph before adopting them.
[Wayland client manifest](https://github.com/Smithay/wayland-rs/blob/master/wayland-client/Cargo.toml),
[backend documentation](https://docs.rs/wayland-backend/latest/wayland_backend/),
[SCTK manifest](https://github.com/Smithay/client-toolkit/blob/master/Cargo.toml),
[kbvm 0.1.5](https://docs.rs/crate/kbvm/0.1.5).

The investigated winit/softbuffer Wayland paths enable system backends or dynamic
library loading. They are not a drop-in match for the fully static musl proof.
This is not a claim that all configurations or future versions are permanently
excluded: a permissive library rebuilt for a compatible runtime can be considered
after a separate audit. Loading ordinary host glibc-linked libraries would
reintroduce the current policy/ABI problem.
[winit manifest](https://docs.rs/crate/winit/latest/source/Cargo.toml),
[softbuffer manifest](https://github.com/rust-windowing/softbuffer/blob/master/Cargo.toml).

An egui-based software UI is worth evaluating only after the basic Wayland client
works. `egui_software_backend` 0.0.3 is an early candidate, not a mature editor
replacement proven by this screening. Disable unneeded integration features and
audit bundled font assets separately from the renderer license.
[Renderer crate](https://docs.rs/crate/egui_software_backend/0.0.3).

## Scope and next evidence

The X11 prototype proves a smaller window/input/pixel path. It does not prove
native Wayland, the existing React editor, IME, clipboard, accessibility, drag/drop,
or editor performance. Audio playback/loopback requires its own static-client
prototype. Native Wayland then requires a separate protocol/keyboard/shared-memory
proof against the supported compositor. No large UI rewrite is selected here.

Source URLs using default branches/latest are discovery references. Their exact
revisions must be pinned in each prototype's lockfile and provenance evidence.
