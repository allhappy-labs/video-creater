# Archived: strict permissive-only Linux runtime (superseded)

These research notes, plans and design documents come from the 2026-09-12 attempt to ship Linux under a
"no LGPL, including system libraries" rule: a musl-based native desktop host (Iced/Wayland), an app-owned
MP4/H.264 decoder, and a custom compositor and audio mixer.

On 2026-09-13 that rule was dropped. Linux now follows the macOS license policy — no GPL, with
GStreamer/GES, WebKitGTK and glibc dynamically linked — and ships the same Tauri app. See
[`docs/development/linux.md`](../../../development/linux.md) for the current Linux port.

The documents are kept for their license audits and codec, runtime and desktop feasibility findings. They
describe work that is not on `main`; paths and relative links refer to the archived branches:

- `archive/linux-permissive-runtime` — permissive media worker, owned MP4/H.264 decoder, platform runtime seam
- `archive/linux-desktop-vm-proof` — native desktop host and Wayland/X11 VM evidence harness

Layout mirrors the original `docs/` tree: `research/`, `plans/` and `specs/` (from `docs/superpowers/`), and
`development/`.
