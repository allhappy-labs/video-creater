# Linux desktop runtime feasibility evidence

> Historical result under the former service-inclusive policy. The user later
> selected option 1: independent OS services are permitted, while LGPL libraries
> linked or loaded by the app and its helpers remain forbidden. The service-based
> blocker below is superseded; application-level library findings remain relevant.
> See the [current specification](../superpowers/specs/2026-09-12-linux-compatibility-design.md).

Date: 2026-09-12
Checkout: `917c74334ea7b930cc56c739effce9168641a409`
Host: Ubuntu 24.04.4 LTS, Linux 6.8.0-139-generic, x86_64
Decision: no qualifying Ubuntu desktop host path is currently established

## Policy and evidence boundary

The [Linux compatibility design](../superpowers/specs/2026-09-12-linux-compatibility-design.md)
prohibits LGPL and other denied copyleft dependencies in the application,
bundled utilities, required system libraries, helper processes, and required
desktop-service closure. Dynamic linking, installed system components, and IPC
do not create exemptions. The Linux kernel is the declared operating-system
boundary.

This report records why dependent implementation cannot start from a supported
host selection. It is not an impossibility theorem, adopts no policy exemption,
and selects no unverified toolkit, engine, compositor, audio server, or session.

## Source findings

| Path | Primary or first-party evidence | Conclusion under the policy |
| --- | --- | --- |
| Application libc | Musl's [upstream copyright file](https://git.musl-libc.org/cgit/musl/tree/COPYRIGHT) says musl as a whole uses the MIT license. Rust supports [`x86_64-unknown-linux-musl`](https://doc.rust-lang.org/rustc/platform-support.html) and [static CRT linkage](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes). | A musl application binary is plausible in isolation. It does not replace desktop services. |
| Ubuntu Wayland | Canonical's [Noble `gnome-shell` record](https://packages.ubuntu.com/noble/gnome-shell) lists `libc6`, GTK 4, WebKit, GStreamer, PipeWire, GLib, and Mutter dependencies. | The ordinary Ubuntu GNOME/Wayland service closure is ineligible. |
| Ubuntu X11 | Canonical's [Noble Xorg record](https://packages.ubuntu.com/noble/xserver-xorg-core) lists `libc6`, `libgcrypt20`, `libsystemd0`, and `libudev1`; it describes `libgcrypt20` as LGPL. | The specification's X11 acceptance path is independently ineligible. |
| Ubuntu audio | Canonical's [PipeWire](https://packages.ubuntu.com/noble/pipewire-bin) record lists `libc6` and ALSA; [WirePlumber](https://packages.ubuntu.com/noble/wireplumber) lists `libc6`, GLib, PipeWire, and D-Bus. | A new permissive client does not remove the required audio-service closure. |
| Desktop integration | Canonical's [portal record](https://packages.ubuntu.com/noble/xdg-desktop-portal) lists `libc6`, GLib, PipeWire, and `libsystemd0`. | Standard portal-based file and desktop integration is ineligible. |
| React engine | Chromium's [Linux dependency list](https://chromium.googlesource.com/chromium/src/+/a2bee684f3c4224a8836957b917c167d9bb9a349/chrome/installer/linux/rpm/additional_deps) says it dynamically loads GTK 3 or 4. Servo's [license at the inspected revision](https://github.com/servo/servo/blob/84bec05b56d3669ccd12891557347d88f3fd8cad/LICENSE) is MPL-2.0. | Neither source establishes a permitted React-engine artifact closure; Servo is expressly denied. |

The [GNU C Library project](https://www.gnu.org/software/libc/) identifies glibc
as GNU Lesser General Public License software. A permissive client-side native
window library or software renderer cannot alter the Ubuntu compositor/X server
closure. A musl-native presentation layer remains conceivable research, but no
complete qualifying implementation was established and none is selected here.

## Current host evidence

These read-only commands inspect this VM and cached or installed artifacts. They
do not test an Ubuntu desktop session or a release package.

```bash
rtk git rev-parse HEAD
rtk bash -lc 'uname -srmo; . /etc/os-release; printf "%s\nDISPLAY=%s\nWAYLAND_DISPLAY=%s\nXDG_SESSION_TYPE=%s\n" "$PRETTY_NAME" "${DISPLAY-}" "${WAYLAND_DISPLAY-}" "${XDG_SESSION_TYPE-}"; rustup target list --installed; if command -v musl-gcc >/dev/null; then command -v musl-gcc; else printf "musl-gcc: absent\n"; fi'
rtk dpkg-query -W -f='${binary:Package} ${Version} ${db:Status-Abbrev}\n' libc6 libglib2.0-0t64 libgtk-3-0t64 libwebkit2gtk-4.1-0 libasound2t64 libpulse0 xdg-desktop-portal
rtk bash -lc 'for bin in /usr/libexec/xdg-desktop-portal /home/olhapi/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome; do printf "%s\n" "$bin"; readelf -l "$bin" | sed -n "/interpreter/p"; readelf -d "$bin" | sed -n "s/.*Shared library: \[\(.*\)\]/\1/p" | sort | rg "^(libc|libglib|libgobject|libgio|libjson-glib|libpipewire|libsystemd|libasound|libudev|libX11|libxcb)"; done'
rtk bash -lc 'for p in libglib2.0-0t64 libasound2t64 libsecret-1-0 xdg-desktop-portal; do f="/usr/share/doc/$p/copyright"; printf "%s\n" "$p"; grep -n -m 4 -E "^(Files: \*|License:)" "$f"; done'
rtk bash -lc 'apt-cache depends mutter xserver-xorg-core pipewire-bin wireplumber xdg-desktop-portal | rg "^(  Depends: (libc6|libglib2.0-0t64|libgcrypt20|libsystemd0|libudev1|libasound2t64|libpipewire-0.3-0t64)|[^ ])"'
```

Results:

```text
917c74334ea7b930cc56c739effce9168641a409
Linux 6.8.0-139-generic x86_64 GNU/Linux
Ubuntu 24.04.4 LTS
DISPLAY=
WAYLAND_DISPLAY=
XDG_SESSION_TYPE=
x86_64-unknown-linux-gnu
musl-gcc: absent
libasound2t64:amd64 1.2.11-1ubuntu0.3 ii
libc6:amd64 2.39-0ubuntu8.9 ii
libglib2.0-0t64:amd64 2.80.0-6ubuntu3.8 ii
libgtk-3-0t64:amd64 3.24.41-4ubuntu1.3 ii
libpulse0:amd64 1:16.1+dfsg1-2ubuntu10.1 ii
libwebkit2gtk-4.1-0:amd64 2.52.6-0ubuntu0.24.04.1 ii
xdg-desktop-portal 1.18.4-1ubuntu2.24.04.2 ii
```

The session and musl gaps are environmental gaps, not proof that no compliant
implementation can exist. They prevent the required desktop proof on this host.
The exact filtered `apt-cache depends` output was:

```text
mutter
  Depends: libc6
  Depends: libglib2.0-0t64
xserver-xorg-core
  Depends: libc6
  Depends: libgcrypt20
  Depends: libsystemd0
  Depends: libudev1
pipewire-bin
  Depends: libasound2t64
  Depends: libc6
  Depends: libpipewire-0.3-0t64
wireplumber
  Depends: libc6
  Depends: libglib2.0-0t64
  Depends: libpipewire-0.3-0t64
xdg-desktop-portal
  Depends: libc6
  Depends: libglib2.0-0t64
  Depends: libpipewire-0.3-0t64
  Depends: libsystemd0
```

The exact filtered `readelf` output was:

```text
/usr/libexec/xdg-desktop-portal
      [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]
libc.so.6
libgio-2.0.so.0
libglib-2.0.so.0
libgobject-2.0.so.0
libjson-glib-1.0.so.0
libpipewire-0.3.so.0
libsystemd.so.0
/home/olhapi/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome
      [Requesting program interpreter: /lib64/ld-linux-x86-64.so.2]
libX11.so.6
libasound.so.2
libc.so.6
libcairo.so.2
libcups.so.2
libgio-2.0.so.0
libglib-2.0.so.0
libgobject-2.0.so.0
libudev.so.1
libxcb.so.1
```

Installed copyright files identified LGPL terms in GLib, ALSA, libsecret, and
the portal. The cached browser is host evidence, not a proposed or audited
release artifact.

## Decision

No qualifying host path for the specified Ubuntu desktop is proven. The smallest
blocker is the display-service closure: both required Ubuntu Wayland/GNOME and
X11 paths include glibc and other denied libraries. Audio and portal integration
provide independent blocking closures.

A custom musl user session with its own audited compositor, X server where
required, seat/input handling, audio, dialogs, credential store, and launcher is
distinct new product scope. It needs a separate installation and privilege model,
exact dependency inventory, and real desktop evidence before selection. The next
product decision under the settled policy is whether to authorize that separate
feasibility scope or leave the Ubuntu desktop configuration unsupported.

Downstream Linux host and media implementation remain blocked by the runtime
prerequisite. The inventory preflight may proceed as a separate declaration
check. Its `inventory-eligible` result does not inspect binaries, trace loaded
libraries, validate services, prove runtime behavior, or certify compatibility.
