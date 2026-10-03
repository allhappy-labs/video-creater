#!/usr/bin/env node
// Builds the Linux (x86_64-unknown-linux-gnu) render runtime:
//   * a minimal LGPL-2.1-or-later FFmpeg (shared, $ORIGIN rpath, no GPL/nonfree/version3),
//   * gst-libav 1.24.2 compiled against that FFmpeg,
//   * a plugin directory of symlinks to license-filtered Ubuntu GStreamer plugins
//     (plus bundled-plugins/libgstlibav.so),
//   * manifest.json with the factory inventory read back from an isolated registry.
// Usage: build-linux-media-runtime.mjs [--output DIR] [--package DIR] [--cache-dir DIR] [--jobs N]
//   --package DIR additionally writes the symlink-free redistributable tree for bundling.

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const repoRoot = resolve(import.meta.dirname, "..");

export const runtimeTarget = "x86_64-unknown-linux-gnu";
export const bundledLibavPackageName = "Video Creater LGPL FFmpeg Plug-ins";
export const bundledLibavPackageOrigin = "https://github.com/olhapi/video-creater";
const ffmpegLgplLicense = "LGPL version 2.1 or later";
// Distinct SONAMEs (libavcodec_vc.so.60, ...) so the distribution's GPL-configured
// libavcodec.so.60 can never satisfy the bundled plugin, and vice versa.
export const ffmpegBuildSuffix = "_vc";
export const ffmpegLibraryNames = Object.freeze(["avcodec", "avfilter", "avformat", "avutil"]);

export const pinnedSources = Object.freeze({
  nasm: Object.freeze({
    name: "NASM",
    version: "2.16.03",
    url: "https://www.nasm.us/pub/nasm/releasebuilds/2.16.03/nasm-2.16.03.tar.xz",
    sha256: "1412a1c760bbd05db026b6c0d1657affd6631cd0a63cddb6f73cc6d4aa616148",
    license: "BSD-2-Clause",
    role: "build tool only (FFmpeg x86 assembly); not redistributed",
  }),
  ffmpeg: Object.freeze({
    name: "FFmpeg",
    // gst-libav 1.24.2 predates the FFmpeg 7 API removals (AV_CODEC_ID_AYUV,
    // AVInputFormat.read_probe, AV_OPT_TYPE_CHANNEL_LAYOUT), so the 6.1 LTS
    // branch is the newest compatible release. Signed by FFmpeg release key
    // FCF986EA15E6E293A5644F10B4322F04D67658D8.
    version: "6.1.6",
    url: "https://ffmpeg.org/releases/ffmpeg-6.1.6.tar.xz",
    sha256: "d4fcb164028dd3beee5d92c0ac72e46aac6973c75ea12dc14de07bf8f407370a",
    license: "LGPL-2.1-or-later",
    role: "bundled shared libraries (libavcodec, libavformat, libavfilter, libavutil)",
  }),
  gstLibav: Object.freeze({
    name: "gst-libav",
    version: "1.24.2",
    url: "https://gstreamer.freedesktop.org/src/gst-libav/gst-libav-1.24.2.tar.xz",
    sha256: "962838648e14ce8a78322ab16f84c7d84d86a6d7bebb6574ac05dea84a7c7c9b",
    license: "LGPL-2.1-or-later",
    role: "bundled GStreamer plugin plugins/libgstlibav.so",
  }),
});

// Only what the reviewed Linux media paths need. FFmpeg's configure resolves
// `select` dependencies (for example mpeg4 -> h263 helpers) automatically.
export const ffmpegComponents = Object.freeze({
  decoders: Object.freeze([
    "aac",
    "aac_fixed",
    "aac_latm",
    "ac3",
    "alac",
    "dnxhd",
    "eac3",
    "h264",
    "hevc",
    "mjpeg",
    "mp3",
    "mp3float",
    "mpeg4",
    "prores",
  ]),
  encoders: Object.freeze(["aac", "prores_ks"]),
  parsers: Object.freeze([
    "aac",
    "aac_latm",
    "ac3",
    "dnxhd",
    "h264",
    "hevc",
    "mjpeg",
    "mpeg4video",
    "mpegaudio",
  ]),
  // gst-libav's avdeinterlace builds "buffer ! yadif ! buffersink"; buffer and
  // buffersink are always compiled into libavfilter.
  filters: Object.freeze(["yadif"]),
});

export function ffmpegConfigureArgs({ prefix, nasmPath, rpathResponseFile }) {
  const list = (values) => values.join(",");
  return [
    `--prefix=${prefix}`,
    `--libdir=${prefix}/lib`,
    "--disable-gpl",
    "--disable-nonfree",
    "--disable-version3",
    "--enable-shared",
    "--disable-static",
    "--enable-pic",
    "--disable-programs",
    "--disable-doc",
    "--disable-debug",
    "--disable-autodetect",
    "--disable-everything",
    "--disable-network",
    "--disable-avdevice",
    "--disable-swscale",
    "--disable-swresample",
    "--disable-postproc",
    `--build-suffix=${ffmpegBuildSuffix}`,
    `--x86asmexe=${nasmPath}`,
    `--enable-decoder=${list(ffmpegComponents.decoders)}`,
    `--enable-encoder=${list(ffmpegComponents.encoders)}`,
    `--enable-parser=${list(ffmpegComponents.parsers)}`,
    `--enable-filter=${list(ffmpegComponents.filters)}`,
    // A gcc response file carries the literal $ORIGIN past configure, make and sh.
    `--extra-ldsoflags=@${rpathResponseFile}`,
  ];
}

export function gstLibavMesonArgs({ prefix }) {
  return [
    `--prefix=${prefix}`,
    "--libdir=lib",
    "--buildtype=release",
    "--wrap-mode=nodownload",
    "-Ddoc=disabled",
    "-Dtests=disabled",
    `-Dpackage-name=${bundledLibavPackageName}`,
    `-Dpackage-origin=${bundledLibavPackageOrigin}`,
    "-Dc_link_args=-Wl,-rpath,$ORIGIN/../lib",
  ];
}

// Parses FFmpeg's generated config.h / ffbuild/config.mak and rejects anything
// but an LGPL-2.1-or-later configuration.
export function assertFfmpegLgplConfiguration({ configH, configMak }) {
  const define = (name) => configH.match(new RegExp(`^#define ${name} (.+)$`, "m"))?.[1]?.trim();
  for (const name of ["CONFIG_GPL", "CONFIG_NONFREE", "CONFIG_VERSION3", "CONFIG_GPLV3", "CONFIG_LGPLV3"]) {
    if (define(name) !== "0") {
      throw new Error(`FFmpeg ${name} must be 0, got ${define(name) ?? "<missing>"}`);
    }
  }
  const license = define("FFMPEG_LICENSE");
  if (license !== JSON.stringify(ffmpegLgplLicense)) {
    throw new Error(`FFmpeg license must be "${ffmpegLgplLicense}", got ${license ?? "<missing>"}`);
  }
  for (const name of ["GPL", "NONFREE", "VERSION3", "GPLV3", "LGPLV3"]) {
    if (new RegExp(`^CONFIG_${name}=yes$`, "m").test(configMak)) {
      throw new Error(`FFmpeg config.mak enables CONFIG_${name}`);
    }
  }
  if (!/^CONFIG_SHARED=yes$/m.test(configMak) || /^CONFIG_STATIC=yes$/m.test(configMak)) {
    throw new Error("FFmpeg must be configured as shared-only libraries");
  }
  return { license: ffmpegLgplLicense, gpl: false, nonfree: false, version3: false };
}

// Returns enabled FFmpeg components from config_components.h, grouped by kind.
export function enabledFfmpegComponents(configComponentsH) {
  const kinds = ["decoder", "encoder", "parser", "bsf", "filter", "demuxer", "muxer", "protocol", "hwaccel", "indev", "outdev"];
  const grouped = Object.fromEntries(kinds.map((kind) => [`${kind}s`, []]));
  for (const match of configComponentsH.matchAll(/^#define CONFIG_([A-Z0-9_]+)_([A-Z]+) 1$/gm)) {
    const kind = match[2].toLowerCase();
    if (kinds.includes(kind)) grouped[`${kind}s`].push(match[1].toLowerCase());
  }
  for (const values of Object.values(grouped)) values.sort();
  return grouped;
}

// ---------------------------------------------------------------------------
// Plugin license policy

// GStreamer plugin license strings that are LGPL-family or permissive.
export const allowedPluginLicenses = Object.freeze(["LGPL", "BSD", "MIT/X11"]);

const deniedPluginNames = new Map([
  ["libav", "distribution gst-libav links the distribution (GPL-configured) FFmpeg; replaced by the bundled LGPL build"],
  ["faad", "GPL plugin (FAAD2)"],
  ["x264", "GPL plugin (x264)"],
  ["x265", "GPL plugin (x265)"],
  ["mpeg2enc", "GPL plugin (mjpegtools)"],
  ["mplex", "GPL plugin (mjpegtools)"],
  ["dtsdec", "GPL plugin (libdca)"],
  ["resindvd", "GPL plugin (libdvdnav)"],
  ["a52dec", "GPL plugin (liba52)"],
  ["mpeg2dec", "GPL plugin (libmpeg2)"],
  ["sid", "GPL plugin (libsidplay)"],
  ["cdio", "GPL plugin (libcdio)"],
  ["dvdread", "GPL plugin (libdvdread)"],
  ["fdkaac", "FDK-AAC library is not LGPL-compatible"],
  ["frei0r", "loads frei0r effect modules at runtime; distribution modules are GPL"],
  ["ladspa", "loads LADSPA modules at runtime; many distribution modules are GPL"],
  ["lv2", "loads LV2 modules at runtime; many distribution modules are GPL"],
  ["libvisual", "loads libvisual modules at runtime; distribution modules are GPL"],
  ["jack", "loads libjack at runtime; the distribution JACK2 client library links Berkeley DB (Sleepycat)"],
]);

// Libraries of GPL (or otherwise non-LGPL-compatible) projects, matched against
// SONAMEs in a plugin's resolved NEEDED closure.
const deniedLibraryPatterns = [
  [/^lib(?:avcodec|avformat|avutil|avfilter|avdevice|swscale|swresample|postproc)\.so/, "distribution FFmpeg (GPL-configured)"],
  [/^libx264\.so/, "x264 (GPL)"],
  [/^libx265\.so/, "x265 (GPL)"],
  [/^libfaad(?:_drm)?\.so/, "FAAD2 (GPL)"],
  [/^libmpeg2(?:convert)?\.so/, "libmpeg2 (GPL)"],
  [/^libdvd(?:nav|read|css)\.so/, "libdvdnav/libdvdread/libdvdcss (GPL)"],
  [/^lib(?:cdio|cdio_cdda|cdio_paranoia|iso9660|udf)\.so/, "libcdio (GPL)"],
  [/^liba52(?:-[0-9.]+)?\.so/, "liba52 (GPL)"],
  [/^libdca\.so/, "libdca (GPL)"],
  [/^lib(?:mjpegutils|mpeg2encpp|mplex2)(?:-[0-9.]+)?\.so/, "mjpegtools (GPL)"],
  [/^libsidplay\.so/, "libsidplay (GPL)"],
  [/^libxvidcore\.so/, "Xvid (GPL)"],
  [/^libfdk-aac\.so/, "FDK-AAC (non-LGPL-compatible)"],
  [/^liblrdf\.so/, "liblrdf (GPL)"],
  [/^libzvbi(?:-chains)?\.so/, "libzvbi (GPL components)"],
  [/^libmad\.so/, "libmad (GPL)"],
  [/^libid3tag\.so/, "libid3tag (GPL)"],
  [/^libvidstab\.so/, "vid.stab (GPL)"],
  [/^libfftw3(?:f|l|q)?(?:_threads)?\.so/, "FFTW (GPL)"],
  [/^librubberband\.so/, "Rubber Band (GPL)"],
  [/^libslang\.so/, "S-Lang (GPL)"],
  [/^libgpm\.so/, "gpm (GPL)"],
  [/^libxine\.so/, "xine-lib (GPL)"],
  [/^lib(?:readline|history)\.so/, "GNU Readline (GPL)"],
  [/^libdb(?:-[0-9.]+)?\.so/, "Berkeley DB (Sleepycat/AGPL copyleft)"],
  [/^libjbig(?:85)?\.so/, "JBIG-KIT (GPL)"],
  [/^libneon(?:-gnutls)?\.so/, "neon (packaged under GPL-2+ terms)"],
];

export function deniedLibraryReason(soname) {
  const name = basename(String(soname ?? ""));
  return deniedLibraryPatterns.find(([pattern]) => pattern.test(name))?.[1] ?? null;
}

export function classifySystemPlugin({ file, plugin, closure = [], unresolved = [] }) {
  const pluginName = plugin?.name ?? null;
  const decide = (included, reason) => ({
    file,
    plugin: pluginName,
    package: plugin?.package ?? null,
    license: plugin?.license ?? null,
    included,
    reason,
  });
  if (basename(file) === "libgstlibav.so" || pluginName === "libav") {
    return decide(false, deniedPluginNames.get("libav"));
  }
  if (!plugin) return decide(false, "not registered by an isolated scan (not a loadable GStreamer plugin)");
  if (plugin.status !== "loaded") return decide(false, "blacklisted by the isolated plugin scan");
  if (deniedPluginNames.has(pluginName)) return decide(false, deniedPluginNames.get(pluginName));
  if (/ugly/i.test(plugin.package ?? "")) return decide(false, "GStreamer Ugly plug-ins are denied by policy");
  if (!allowedPluginLicenses.includes(plugin.license)) {
    return decide(false, `plugin license '${plugin.license}' is not LGPL-family or permissive`);
  }
  for (const dependency of closure) {
    const reason = deniedLibraryReason(dependency.soname);
    if (reason) {
      return decide(false, `links ${dependency.soname} (${reason}) via ${dependency.path}`);
    }
  }
  if (unresolved.length > 0) {
    return decide(false, `unresolved ELF dependencies cannot be reviewed: ${unresolved.join(", ")}`);
  }
  return decide(true, `license ${plugin.license}; ${closure.length} resolved libraries, none denylisted`);
}

// ---------------------------------------------------------------------------
// ELF helpers

export function parseReadelfDynamic(text) {
  const entry = (tag) =>
    [...text.matchAll(new RegExp(`\\(${tag}\\)\\s+[^\\[]*\\[([^\\]]*)\\]`, "g"))].map((match) => match[1]);
  return {
    needed: entry("NEEDED"),
    soname: entry("SONAME")[0] ?? null,
    runpath: entry("RUNPATH").flatMap((value) => value.split(":")),
    rpath: entry("RPATH").flatMap((value) => value.split(":")),
  };
}

const multiarchLibraryDirectories = [
  "/lib/x86_64-linux-gnu",
  "/usr/lib/x86_64-linux-gnu",
  "/lib64",
  "/usr/lib64",
  "/lib",
  "/usr/lib",
];

function readelfDynamic(path) {
  const result = spawnSync("readelf", ["-d", "-W", path], { encoding: "utf8" });
  if (result.status !== 0) throw new Error(`readelf -d failed for ${path}: ${result.stderr}`);
  return parseReadelfDynamic(result.stdout);
}

export function neededClosure(path, { cache = new Map() } = {}) {
  const closure = new Map();
  const unresolved = new Set();
  const queue = [path];
  const seenFiles = new Set();
  while (queue.length > 0) {
    const current = queue.shift();
    const real = realpathSync(current);
    if (seenFiles.has(real)) continue;
    seenFiles.add(real);
    let dynamic = cache.get(real);
    if (!dynamic) {
      dynamic = readelfDynamic(real);
      cache.set(real, dynamic);
    }
    const origin = dirname(real);
    const searchDirectories = [...dynamic.rpath, ...dynamic.runpath]
      .map((directory) => directory.replaceAll("$ORIGIN", origin).replaceAll("${ORIGIN}", origin))
      .concat(multiarchLibraryDirectories);
    for (const soname of dynamic.needed) {
      if (closure.has(soname)) continue;
      const found = searchDirectories.map((directory) => join(directory, soname)).find((candidate) => existsSync(candidate));
      if (!found) {
        unresolved.add(soname);
        continue;
      }
      closure.set(soname, { soname, path: realpathSync(found) });
      queue.push(found);
    }
  }
  return {
    closure: [...closure.values()].sort((left, right) => left.soname.localeCompare(right.soname)),
    unresolved: [...unresolved].sort(),
  };
}

// ---------------------------------------------------------------------------
// Probe output parsing and manifest generation

export function parseProbeInventory(text) {
  const plugins = new Map();
  for (const line of text.split(/\r?\n/)) {
    const fields = line.split("\t");
    if (fields[0] === "PLUGIN") {
      const [, name, filename, license, packageName, source, origin, version, status] = fields;
      plugins.set(name, { name, filename, license, package: packageName, source, origin, version, status, features: [] });
    } else if (fields[0] === "FEATURE") {
      const [, pluginName, kind, name, rank, klass] = fields;
      plugins.get(pluginName)?.features.push({ kind, name, rank: Number(rank), klass });
    }
  }
  return [...plugins.values()];
}

// Render-pipeline factories reviewed by src-tauri/src/render_pipeline/plugin_policy.rs
// (ALLOWED_FACTORY_POLICIES and LINUX_CODEC_FACTORY_POLICIES). The Rust health probe
// evaluates every manifest `factories` entry, so only these names are listed there; the
// complete loadable set is recorded under `inventory`.
export const reviewedRenderFactories = Object.freeze({
  portable: Object.freeze([
    "aacparse", "appsink", "appsrc", "audioconvert", "audiomixer", "audiorate", "audioresample",
    "audiotestsrc", "autoaudiosink", "autovideosink", "capsfilter", "compositor", "decodebin",
    "filesink", "filesrc", "gldownload", "h264parse", "h265parse", "matroskademux", "matroskamux",
    "mp4mux", "multifilesrc", "opusdec", "opusenc", "pngdec", "pngenc", "qtdemux", "qtmux", "queue", "rotate",
    "scaletempo", "videoconvert", "videocrop", "videoflip", "videorate", "videoscale", "videotestsrc", "volume",
    "vp8dec", "vp8enc", "vp9dec", "vp9enc", "webmmux",
  ]),
  linuxCodecs: Object.freeze([
    "avdec_aac", "avdec_aac_fixed", "avdec_aac_latm", "avdec_ac3", "avdec_alac", "avdec_dnxhd",
    "avdec_eac3", "avdec_h264", "avdec_h265", "avdec_mjpeg", "avdec_mp3", "avdec_mp3float",
    "avdec_mpeg4", "avdec_prores", "avenc_aac", "avenc_prores_ks", "avidemux", "flacdec",
    "id3demux", "jpegdec", "oggdemux", "openh264dec", "openh264enc", "theoradec", "vorbisdec",
    "wavparse",
  ]),
  // Registered only when a VA-API render node exists while the registry is scanned.
  hardwareDependent: Object.freeze(["vah264dec", "vah264enc", "vah265dec", "vah265enc"]),
  macosOnly: Object.freeze(["atdec", "atenc", "vtdec", "vtdec_hw", "vtenc_h264", "vtenc_h265", "vtenc_prores"]),
});

export function selectReviewedFactories(allFactories) {
  const required = [...reviewedRenderFactories.portable, ...reviewedRenderFactories.linuxCodecs];
  const listed = new Set([...required, ...reviewedRenderFactories.hardwareDependent]);
  const byName = new Map(allFactories.map((factory) => [factory.name, factory]));
  const missing = required.filter((name) => !byName.has(name));
  if (missing.length > 0) {
    throw new Error(`reviewed render factories are missing from the staged runtime: ${missing.join(", ")}`);
  }
  return allFactories.filter((factory) => listed.has(factory.name));
}

export function manifestFactories(inventory) {
  return inventory
    .filter((plugin) => plugin.status === "loaded")
    .flatMap((plugin) =>
      plugin.features
        .filter((feature) => feature.kind === "element")
        .map((feature) => ({
          name: feature.name,
          pluginName: plugin.name,
          package: plugin.package,
          license: plugin.license,
        })),
    )
    .sort((left, right) => left.name.localeCompare(right.name));
}

export const bundledPluginDirectory = "bundled-plugins";

export function linuxRuntimeLayout({ pluginDecisions, bundledPluginFiles }) {
  return {
    systemPluginDirectory: systemLayout.pluginDirectory,
    systemPluginFiles: pluginDecisions
      .filter((decision) => decision.included)
      .map((decision) => basename(decision.file))
      .sort(),
    bundledPluginDirectory,
    bundledPluginFiles: [...bundledPluginFiles].sort(),
    scanner: systemLayout.scanner,
    gstreamerCoreLibrary: systemLayout.gstreamerLibrary,
  };
}

export function buildRuntimeManifest({
  gstreamerVersion,
  gesVersion,
  inventory,
  pluginDecisions,
  ffmpeg,
  gstLibav,
  nasm,
  bundledFiles,
  systemLinks,
  systemLibraryClosure,
  linux,
}) {
  const allFactories = manifestFactories(inventory);
  const names = new Set();
  for (const factory of allFactories) {
    if (names.has(factory.name)) throw new Error(`duplicate factory in staged runtime: ${factory.name}`);
    names.add(factory.name);
  }
  const staticPlugins = inventory.filter((plugin) => !plugin.filename).map((plugin) => plugin.name);
  const stagedPlugins = inventory.filter((plugin) => plugin.status === "loaded").map((plugin) => plugin.name).sort();
  for (const plugin of inventory) {
    if (!plugin.filename) continue;
    if (plugin.license && !allowedPluginLicenses.includes(plugin.license)) {
      throw new Error(`staged plugin ${plugin.name} has disallowed license ${plugin.license}`);
    }
    if (plugin.name === "libav" && plugin.package !== bundledLibavPackageName) {
      throw new Error(`staged libav plugin is not the bundled LGPL build: ${plugin.package}`);
    }
  }
  return {
    schemaVersion: 1,
    target: runtimeTarget,
    platform: "linux",
    linux,
    gstreamerVersion,
    gesVersion,
    plugins: stagedPlugins,
    factories: selectReviewedFactories(allFactories),
    inventory: {
      staticPlugins,
      plugins: inventory
        .filter((plugin) => plugin.status === "loaded")
        .map(({ name, filename, license, package: packageName, source, origin, version }) => ({
          name,
          filename: filename || null,
          license,
          package: packageName,
          source,
          origin,
          version,
        })),
      factories: allFactories,
    },
    ffmpeg,
    gstLibav,
    buildTools: { nasm },
    bundledFiles,
    systemLinks,
    systemLibraryClosure,
    pluginPolicy: {
      allowedLicenses: [...allowedPluginLicenses],
      included: pluginDecisions.filter((decision) => decision.included),
      excluded: pluginDecisions.filter((decision) => !decision.included),
    },
  };
}

export function renderNotice({ ffmpeg, gstLibav, systemPluginDirectory }) {
  return [
    "Video Creater Linux media runtime notice",
    "=========================================",
    "",
    "This runtime contains LGPL-licensed components that are dynamically linked and replaceable.",
    "",
    `FFmpeg ${ffmpeg.version} (${ffmpeg.license})`,
    `  Source: ${ffmpeg.url}`,
    `  SHA-256: ${ffmpeg.sha256}`,
    `  Libraries: ${ffmpeg.libraries.join(", ")} (built with --build-suffix=${ffmpegBuildSuffix}; replacements must keep these SONAMEs)`,
    `  Reported license: ${ffmpeg.reportedLicense}`,
    "  Configured without GPL, nonfree or version3 components:",
    ...ffmpeg.configureArgs.filter((arg) => !arg.startsWith("--prefix") && !arg.startsWith("--libdir") && !arg.startsWith("--x86asmexe") && !arg.startsWith("--extra-ldsoflags")).map((arg) => `    ${arg}`),
    "    --extra-ldsoflags: -Wl,-rpath,$ORIGIN",
    "",
    `gst-libav ${gstLibav.version} (${gstLibav.license})`,
    `  Source: ${gstLibav.url}`,
    `  SHA-256: ${gstLibav.sha256}`,
    `  Plugin: ${bundledPluginDirectory}/libgstlibav.so, package "${bundledLibavPackageName}"`,
    `  Meson options: ${gstLibav.mesonArgs.filter((arg) => !arg.startsWith("--prefix")).join(" ")}`,
    "",
    "Relinking and replacement (LGPL-2.1 section 6)",
    "----------------------------------------------",
    "The FFmpeg libraries in lib/ are ordinary shared objects located through a $ORIGIN run path.",
    `${bundledPluginDirectory}/libgstlibav.so locates them through the run path $ORIGIN/../lib. You may replace any of`,
    "these files with ABI-compatible builds made from modified corresponding source (for example by",
    "re-running scripts/build-linux-media-runtime.mjs with a patched source tree); no part of Video",
    "Creater is statically linked against them.",
    "",
    "Other GStreamer plugins",
    "-----------------------",
    `plugins/ holds symbolic links: libgstlibav.so to ${bundledPluginDirectory}/, all others to the operating system's GStreamer`,
    `plugins in ${systemPluginDirectory}. They are not redistributed; they are selected by the license`,
    "policy recorded in manifest.json (pluginPolicy), which excludes GPL, unknown-license and",
    "proprietary plugins, the distribution gst-libav plugin, and plugins whose dependency closure",
    "contains denylisted GPL libraries. lib/libgstreamer-1.0.so.0 and libexec/gst-plugin-scanner",
    "are links to the system GStreamer core.",
    "",
    "License texts are in this directory: FFmpeg-COPYING.LGPLv2.1.txt, FFmpeg-LICENSE.md and",
    "gst-libav-COPYING.txt.",
    "",
  ].join("\n");
}

// ---------------------------------------------------------------------------
// Build orchestration

export function runtimeProbeEnvironment({ pluginDirectory, scanner, registry, baseEnvironment = process.env }) {
  const environment = { ...baseEnvironment };
  for (const key of Object.keys(environment)) {
    if (key.startsWith("GST_")) delete environment[key];
  }
  return {
    ...environment,
    GST_PLUGIN_SYSTEM_PATH_1_0: "",
    GST_PLUGIN_SYSTEM_PATH: "",
    GST_PLUGIN_PATH_1_0: pluginDirectory,
    GST_PLUGIN_PATH: pluginDirectory,
    GST_PLUGIN_SCANNER_1_0: scanner,
    GST_PLUGIN_SCANNER: scanner,
    GST_REGISTRY_1_0: registry,
    GST_REGISTRY: registry,
    GST_REGISTRY_FORK: "yes",
  };
}

export function defaultCacheDirectory() {
  return join(process.env.XDG_CACHE_HOME || join(homedir(), ".cache"), "video-creater-linux-build");
}

export const systemLayout = Object.freeze({
  pluginDirectory: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0",
  scanner: "/usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner",
  gstreamerLibrary: "/usr/lib/x86_64-linux-gnu/libgstreamer-1.0.so.0",
});

export function compileRuntimeProbe(cacheDirectory) {
  const source = join(repoRoot, "scripts", "linux-media-runtime-probe.c");
  const flags = run("pkg-config", ["--cflags", "--libs", "gstreamer-1.0"]).stdout.trim().split(/\s+/).filter(Boolean);
  const key = sha256Text(`${readFileSync(source, "utf8")}\0${flags.join(" ")}`).slice(0, 16);
  const output = join(cacheDirectory, "probe", key, "linux-media-runtime-probe");
  if (!existsSync(output)) {
    mkdirSync(dirname(output), { recursive: true });
    run("cc", ["-O2", "-Wall", "-Werror", source, "-o", `${output}.tmp`, ...flags]);
    renameSync(`${output}.tmp`, output);
  }
  return output;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const cache = resolve(options.cacheDir);
  const outputRoot = resolve(repoRoot, options.output);
  const jobs = String(options.jobs);
  mkdirSync(cache, { recursive: true });

  const nasmPath = ensureNasm({ cache, jobs });
  const ffmpeg = ensureFfmpeg({ cache, jobs, nasmPath });
  const gstLibav = ensureGstLibav({ cache, ffmpeg });
  const probe = compileRuntimeProbe(cache);

  const candidate = `${outputRoot}.candidate-${process.pid}`;
  rmSync(candidate, { recursive: true, force: true });
  for (const directory of ["lib", "libexec", "plugins", bundledPluginDirectory, "licenses"]) {
    mkdirSync(join(candidate, directory), { recursive: true });
  }
  try {
    const bundledFiles = [];
    for (const library of ffmpeg.libraries) {
      const target = join(candidate, "lib", library);
      copyFileSync(join(ffmpeg.prefix, "lib", library), target);
      bundledFiles.push(target);
    }
    const libavPlugin = join(candidate, bundledPluginDirectory, "libgstlibav.so");
    copyFileSync(gstLibav.plugin, libavPlugin);
    bundledFiles.push(libavPlugin);
    symlinkSync(`../${bundledPluginDirectory}/libgstlibav.so`, join(candidate, "plugins", "libgstlibav.so"));

    const systemLinks = [
      linkSystemFile(candidate, "lib/libgstreamer-1.0.so.0", systemLayout.gstreamerLibrary),
      linkSystemFile(candidate, "libexec/gst-plugin-scanner", systemLayout.scanner),
    ];

    const { decisions, closureLibraries } = selectSystemPlugins({ probe });
    for (const decision of decisions.filter((entry) => entry.included)) {
      systemLinks.push(linkSystemFile(candidate, `plugins/${basename(decision.file)}`, decision.file));
    }

    verifyBundledElf({ candidate, bundledFiles, ffmpeg });

    const licenses = join(candidate, "licenses");
    copyFileSync(join(ffmpeg.source, "COPYING.LGPLv2.1"), join(licenses, "FFmpeg-COPYING.LGPLv2.1.txt"));
    copyFileSync(join(ffmpeg.source, "LICENSE.md"), join(licenses, "FFmpeg-LICENSE.md"));
    copyFileSync(join(gstLibav.source, "COPYING"), join(licenses, "gst-libav-COPYING.txt"));
    const ffmpegRecord = {
      ...pick(pinnedSources.ffmpeg, ["version", "url", "sha256", "license"]),
      reportedLicense: ffmpeg.reportedLicense,
      libraries: ffmpeg.libraries,
      configureArgs: ffmpeg.configureArgs.map((arg) => arg.replaceAll(cache, "$CACHE")),
      enabledComponents: ffmpeg.enabledComponents,
    };
    const gstLibavRecord = {
      ...pick(pinnedSources.gstLibav, ["version", "url", "sha256", "license"]),
      packageName: bundledLibavPackageName,
      packageOrigin: bundledLibavPackageOrigin,
      mesonArgs: gstLibav.mesonArgs.map((arg) => arg.replaceAll(cache, "$CACHE")),
    };
    writeFileSync(
      join(licenses, "NOTICE.txt"),
      renderNotice({ ffmpeg: ffmpegRecord, gstLibav: gstLibavRecord, systemPluginDirectory: systemLayout.pluginDirectory }),
    );

    const inventory = inventoryPlugins({
      probe,
      pluginDirectory: join(candidate, "plugins"),
      scanner: join(candidate, "libexec", "gst-plugin-scanner"),
    });
    const stagedLibav = inventory.find((plugin) => plugin.name === "libav");
    if (!stagedLibav || realpathSync(stagedLibav.filename) !== realpathSync(libavPlugin)) {
      throw new Error("bundled libgstlibav.so did not register from the staged plugin directory");
    }
    const manifest = buildRuntimeManifest({
      gstreamerVersion: run("pkg-config", ["--modversion", "gstreamer-1.0"]).stdout.trim(),
      gesVersion: run("pkg-config", ["--modversion", "gst-editing-services-1.0"]).stdout.trim(),
      inventory,
      pluginDecisions: decisions,
      ffmpeg: ffmpegRecord,
      gstLibav: gstLibavRecord,
      nasm: pick(pinnedSources.nasm, ["version", "url", "sha256", "license", "role"]),
      bundledFiles: [...bundledFiles, ...walkFiles(licenses)].map((path) => ({
        path: relative(candidate, path),
        bytes: statSync(path).size,
        sha256: sha256File(path),
      })),
      systemLinks,
      systemLibraryClosure: closureLibraries,
      linux: linuxRuntimeLayout({ pluginDecisions: decisions, bundledPluginFiles: ["libgstlibav.so"] }),
    });
    writeFileSync(join(candidate, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
    const packageCandidate = options.package ? stagePackageTree(candidate, `${resolve(repoRoot, options.package)}.candidate-${process.pid}`) : null;
    publishRuntime(candidate, outputRoot);
    if (packageCandidate) publishRuntime(packageCandidate, resolve(repoRoot, options.package));
    const bundleBytes = manifest.bundledFiles.reduce((total, file) => total + file.bytes, 0);
    console.log(
      JSON.stringify(
        {
          status: "built",
          runtimeRoot: outputRoot,
          packageRoot: options.package ? resolve(repoRoot, options.package) : null,
          ffmpeg: `${ffmpegRecord.version} (${ffmpegRecord.reportedLicense})`,
          gstLibav: gstLibavRecord.version,
          pluginsIncluded: manifest.pluginPolicy.included.length,
          pluginsExcluded: manifest.pluginPolicy.excluded.length,
          registeredPlugins: manifest.plugins.length,
          reviewedFactories: manifest.factories.length,
          inventoryFactories: manifest.inventory.factories.length,
          hardwareFactoriesAbsent: reviewedRenderFactories.hardwareDependent.filter((name) => !manifest.factories.some((factory) => factory.name === name)),
          bundleBytes,
        },
        null,
        2,
      ),
    );
  } catch (error) {
    rmSync(candidate, { recursive: true, force: true });
    throw error;
  }
}

// Copies the redistributable part of a staged runtime (no symlinks): manifest.json,
// bundled-plugins/, lib/ (bundled FFmpeg only) and licenses/. The application
// recreates plugins/, libexec/ and lib/libgstreamer-1.0.so.0 from manifest.linux.
export function stagePackageTree(runtimeRoot, destination) {
  rmSync(destination, { recursive: true, force: true });
  mkdirSync(destination, { recursive: true });
  copyFileSync(join(runtimeRoot, "manifest.json"), join(destination, "manifest.json"));
  for (const directory of [bundledPluginDirectory, "lib", "licenses"]) {
    mkdirSync(join(destination, directory), { recursive: true });
    for (const entry of readdirSync(join(runtimeRoot, directory), { withFileTypes: true })) {
      if (entry.isSymbolicLink()) continue;
      if (!entry.isFile()) throw new Error(`unexpected package entry: ${directory}/${entry.name}`);
      copyFileSync(join(runtimeRoot, directory, entry.name), join(destination, directory, entry.name));
    }
  }
  return destination;
}

function ensureNasm({ cache, jobs }) {
  const existing = spawnSync("nasm", ["-v"], { encoding: "utf8" });
  const version = existing.stdout?.match(/NASM version (\d+)\.(\d+)/);
  if (existing.status === 0 && version && (Number(version[1]) > 2 || Number(version[2]) >= 13)) {
    return "nasm";
  }
  const spec = pinnedSources.nasm;
  const root = join(cache, "build", `nasm-${spec.version}`);
  const binary = join(root, "bin", "nasm");
  return cachedStep(root, { sha256: spec.sha256 }, binary, () => {
    const source = extractSource(cache, spec, join(root, "src"));
    run("./configure", [`--prefix=${root}`], { cwd: source });
    run("make", [`-j${jobs}`, "nasm"], { cwd: source });
    mkdirSync(join(root, "bin"), { recursive: true });
    copyFileSync(join(source, "nasm"), binary);
  });
}

function ensureFfmpeg({ cache, jobs, nasmPath }) {
  const spec = pinnedSources.ffmpeg;
  const probeArgs = ffmpegConfigureArgs({ prefix: "<prefix>", nasmPath: "<nasm>", rpathResponseFile: "<rsp>" });
  const key = sha256Text(JSON.stringify({ sha256: spec.sha256, probeArgs })).slice(0, 12);
  const root = join(cache, "build", `ffmpeg-${spec.version}-${key}`);
  const prefix = join(root, "prefix");
  const rpathResponseFile = join(root, "rpath-origin.rsp");
  const configureArgs = ffmpegConfigureArgs({ prefix, nasmPath, rpathResponseFile });
  cachedStep(root, { sha256: spec.sha256, configureArgs }, join(prefix, "lib", "pkgconfig", `libavcodec${ffmpegBuildSuffix}.pc`), () => {
    const source = extractSource(cache, spec, join(root, "src"));
    const build = join(root, "build");
    rmSync(build, { recursive: true, force: true });
    mkdirSync(build, { recursive: true });
    writeFileSync(rpathResponseFile, "-Wl,-rpath,$ORIGIN\n");
    run(join(source, "configure"), configureArgs, { cwd: build });
    assertFfmpegLgplConfiguration({
      configH: readFileSync(join(build, "config.h"), "utf8"),
      configMak: readFileSync(join(build, "ffbuild", "config.mak"), "utf8"),
    });
    copyFileSync(join(build, "config_components.h"), join(root, "config_components.h"));
    run("make", [`-j${jobs}`], { cwd: build });
    run("make", ["install"], { cwd: build });
  });
  // gst-libav looks up unsuffixed pkg-config modules; alias them to the suffixed builds.
  const pkgConfigShim = join(root, "pkgconfig-shim");
  mkdirSync(pkgConfigShim, { recursive: true });
  for (const name of ffmpegLibraryNames) {
    copyFileSync(join(prefix, "lib", "pkgconfig", `lib${name}${ffmpegBuildSuffix}.pc`), join(pkgConfigShim, `lib${name}.pc`));
  }
  const libraries = readdirSync(join(prefix, "lib"))
    .filter((name) => /^lib[a-z_]+\.so\.\d+$/.test(name))
    .sort();
  const expectedLibraries = ffmpegLibraryNames.map((name) => `lib${name}${ffmpegBuildSuffix}`);
  if (JSON.stringify(libraries.map((name) => name.replace(/\.so\.\d+$/, ""))) !== JSON.stringify(expectedLibraries)) {
    throw new Error(`unexpected FFmpeg shared libraries: ${libraries.join(", ")}`);
  }
  const reportedLicense = ffmpegRuntimeLicense({ cache, prefix });
  return {
    root,
    prefix,
    pkgConfigShim,
    source: join(root, "src"),
    libraries,
    configureArgs,
    reportedLicense,
    enabledComponents: enabledFfmpegComponents(readFileSync(join(root, "config_components.h"), "utf8")),
  };
}

// Links a tiny program against the installed libraries and asks each library
// for its compiled-in license string.
function ffmpegRuntimeLicense({ cache, prefix }) {
  const directory = join(cache, "probe", "ffmpeg-license");
  mkdirSync(directory, { recursive: true });
  const source = join(directory, "license.c");
  writeFileSync(
    source,
    [
      "#include <stdio.h>",
      "#include <libavcodec/avcodec.h>",
      "#include <libavformat/avformat.h>",
      "#include <libavfilter/avfilter.h>",
      "#include <libavutil/avutil.h>",
      "int main(void) {",
      '  printf("avcodec\\t%s\\navformat\\t%s\\navfilter\\t%s\\navutil\\t%s\\n",',
      "         avcodec_license(), avformat_license(), avfilter_license(), avutil_license());",
      "  return 0;",
      "}",
      "",
    ].join("\n"),
  );
  const binary = join(directory, `license-${sha256Text(prefix).slice(0, 8)}`);
  run("cc", [source, "-o", binary, `-I${join(prefix, "include")}`, `-L${join(prefix, "lib")}`, `-Wl,-rpath,${join(prefix, "lib")}`, ...ffmpegLibraryNames.map((name) => `-l${name}${ffmpegBuildSuffix}`)]);
  const output = run(binary, []).stdout.trim().split("\n");
  for (const line of output) {
    const [library, license] = line.split("\t");
    if (license !== ffmpegLgplLicense) {
      throw new Error(`FFmpeg ${library} reports license "${license}", expected "${ffmpegLgplLicense}"`);
    }
  }
  return ffmpegLgplLicense;
}

function ensureGstLibav({ cache, ffmpeg }) {
  const spec = pinnedSources.gstLibav;
  const probeArgs = gstLibavMesonArgs({ prefix: "<prefix>" });
  const key = sha256Text(JSON.stringify({ sha256: spec.sha256, probeArgs, ffmpeg: ffmpeg.root })).slice(0, 12);
  const root = join(cache, "build", `gst-libav-${spec.version}-${key}`);
  const prefix = join(root, "prefix");
  const mesonArgs = gstLibavMesonArgs({ prefix });
  const plugin = join(prefix, "lib", "gstreamer-1.0", "libgstlibav.so");
  const environment = {
    ...process.env,
    PKG_CONFIG_PATH: [ffmpeg.pkgConfigShim, join(ffmpeg.prefix, "lib", "pkgconfig"), process.env.PKG_CONFIG_PATH].filter(Boolean).join(":"),
  };
  cachedStep(root, { sha256: spec.sha256, mesonArgs, ffmpeg: ffmpeg.root }, plugin, () => {
    const source = extractSource(cache, spec, join(root, "src"));
    const build = join(root, "build");
    rmSync(build, { recursive: true, force: true });
    run("meson", ["setup", build, source, ...mesonArgs], { cwd: root, env: environment });
    run("meson", ["compile", "-C", build, "-j", "4"], { cwd: root, env: environment });
    run("meson", ["install", "-C", build, "--strip", "--no-rebuild"], { cwd: root, env: environment });
  });
  return { root, prefix, plugin, source: join(root, "src"), mesonArgs };
}

function selectSystemPlugins({ probe }) {
  const inventory = inventoryPlugins({ probe, pluginDirectory: systemLayout.pluginDirectory, scanner: systemLayout.scanner });
  const byFile = new Map(inventory.filter((plugin) => plugin.filename).map((plugin) => [realpathSync(plugin.filename), plugin]));
  const elfCache = new Map();
  const decisions = [];
  const closureLibraries = new Map();
  const candidates = readdirSync(systemLayout.pluginDirectory)
    .filter((name) => name.endsWith(".so"))
    .sort();
  for (const name of candidates) {
    const file = join(systemLayout.pluginDirectory, name);
    const plugin = byFile.get(realpathSync(file)) ?? null;
    const { closure, unresolved } = neededClosure(file, { cache: elfCache });
    const decision = classifySystemPlugin({ file, plugin, closure, unresolved });
    decisions.push(decision);
    if (decision.included) {
      for (const dependency of closure) closureLibraries.set(dependency.soname, dependency.path);
    }
  }
  return {
    decisions,
    closureLibraries: [...closureLibraries.entries()]
      .map(([soname, path]) => ({ soname, path }))
      .sort((left, right) => left.soname.localeCompare(right.soname)),
  };
}

function inventoryPlugins({ probe, pluginDirectory, scanner }) {
  const registryDirectory = mkdtempSync(join(tmpdir(), "video-creater-linux-media-registry-"));
  try {
    return parseProbeInventory(
      run(probe, ["inventory"], {
        env: runtimeProbeEnvironment({ pluginDirectory, scanner, registry: join(registryDirectory, "registry.bin") }),
      }).stdout,
    );
  } finally {
    rmSync(registryDirectory, { recursive: true, force: true });
  }
}

function verifyBundledElf({ candidate, bundledFiles, ffmpeg }) {
  const bundledLib = join(candidate, "lib");
  for (const path of bundledFiles) {
    const dynamic = readelfDynamic(path);
    const expectedRunpath = path.includes(`${join(candidate, bundledPluginDirectory)}/`) ? "$ORIGIN/../lib" : "$ORIGIN";
    const runpaths = [...dynamic.runpath, ...dynamic.rpath];
    if (runpaths.length !== 1 || runpaths[0] !== expectedRunpath) {
      throw new Error(`${relative(candidate, path)} must have run path ${expectedRunpath}, got ${runpaths.join(":") || "<none>"}`);
    }
    const ldd = run("ldd", [path], { env: withoutLoaderOverrides() }).stdout;
    for (const line of ldd.split("\n")) {
      const match = line.match(/^\s*(lib(?:av|sw|postproc)[a-z_]*\.so\.\d+)\s+=>\s+(\S+)/);
      if (!match) continue;
      if (dirname(realpathSync(match[2])) !== realpathSync(bundledLib)) {
        throw new Error(`${relative(candidate, path)} resolves ${match[1]} outside the bundle: ${match[2]}`);
      }
    }
    if (/not found/.test(ldd)) throw new Error(`${relative(candidate, path)} has unresolved libraries:\n${ldd}`);
    for (const soname of dynamic.needed) {
      if (/^lib(?:av|sw|postproc)/.test(soname) && !ffmpeg.libraries.includes(soname)) {
        throw new Error(`${relative(candidate, path)} needs unbundled FFmpeg library ${soname}`);
      }
    }
  }
}

function linkSystemFile(root, relativePath, target) {
  const resolvedTarget = realpathSync(target);
  const path = join(root, relativePath);
  mkdirSync(dirname(path), { recursive: true });
  symlinkSync(target, path);
  return { path: relativePath, target, resolvedTarget, sha256: sha256File(resolvedTarget) };
}

function publishRuntime(candidate, published) {
  const backup = `${published}.backup-${process.pid}`;
  rmSync(backup, { recursive: true, force: true });
  mkdirSync(dirname(published), { recursive: true });
  const hadPublished = existsSync(published) || isSymlink(published);
  if (hadPublished) renameSync(published, backup);
  try {
    renameSync(candidate, published);
  } catch (error) {
    if (hadPublished) renameSync(backup, published);
    throw error;
  }
  rmSync(backup, { recursive: true, force: true });
}

function cachedStep(root, stampData, product, build) {
  const stampPath = join(root, ".complete.json");
  const stamp = JSON.stringify(stampData);
  if (existsSync(product) && existsSync(stampPath) && readFileSync(stampPath, "utf8") === stamp) {
    return product;
  }
  rmSync(stampPath, { force: true });
  mkdirSync(root, { recursive: true });
  build();
  if (!existsSync(product)) throw new Error(`build step did not produce ${product}`);
  writeFileSync(stampPath, stamp);
  return product;
}

function extractSource(cache, spec, destination) {
  const archive = downloadVerified(cache, spec);
  rmSync(destination, { recursive: true, force: true });
  mkdirSync(destination, { recursive: true });
  run("tar", ["-xf", archive, "-C", destination, "--strip-components=1"]);
  return destination;
}

function downloadVerified(cache, spec) {
  const directory = join(cache, "downloads");
  mkdirSync(directory, { recursive: true });
  const archive = join(directory, basename(new URL(spec.url).pathname));
  if (existsSync(archive) && sha256File(archive) === spec.sha256) return archive;
  const partial = `${archive}.partial-${process.pid}`;
  run("curl", ["--fail", "--location", "--silent", "--show-error", "--proto", "=https", "--output", partial, spec.url]);
  const actual = sha256File(partial);
  if (actual !== spec.sha256) {
    rmSync(partial, { force: true });
    throw new Error(`${spec.name} ${spec.version} SHA-256 mismatch: expected ${spec.sha256}, got ${actual}`);
  }
  renameSync(partial, archive);
  return archive;
}

function withoutLoaderOverrides() {
  const environment = { ...process.env };
  delete environment.LD_LIBRARY_PATH;
  delete environment.LD_PRELOAD;
  return environment;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
  if (result.error || result.status !== 0) {
    const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`.trim().split("\n").slice(-40).join("\n");
    throw new Error(`${command} ${args.slice(0, 3).join(" ")} failed (${result.error?.message ?? `exit ${result.status}`}):\n${output}`);
  }
  return result;
}

function walkFiles(root) {
  return readdirSync(root, { withFileTypes: true })
    .flatMap((entry) => (entry.isDirectory() ? walkFiles(join(root, entry.name)) : [join(root, entry.name)]))
    .sort();
}

function isSymlink(path) {
  try {
    return lstatSync(path).isSymbolicLink();
  } catch {
    return false;
  }
}

function pick(object, keys) {
  return Object.fromEntries(keys.map((key) => [key, object[key]]));
}

function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function sha256Text(text) {
  return createHash("sha256").update(text).digest("hex");
}

export function parseArgs(argv) {
  const parsed = {
    output: "src-tauri/resources/render-runtime",
    cacheDir: process.env.VIDEO_CREATER_LINUX_BUILD_CACHE || defaultCacheDirectory(),
    jobs: 4,
    package: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = () => {
      const next = argv[++index];
      if (!next || next.startsWith("--")) throw new Error(`missing value for ${argument}`);
      return next;
    };
    if (argument === "--output") parsed.output = value();
    else if (argument === "--cache-dir") parsed.cacheDir = value();
    else if (argument === "--package") parsed.package = value();
    else if (argument === "--jobs") parsed.jobs = Number(value());
    else if (argument !== "--") throw new Error(`unknown argument: ${argument}`);
  }
  if (!Number.isInteger(parsed.jobs) || parsed.jobs < 1) throw new Error("--jobs must be a positive integer");
  return parsed;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    if (process.platform !== "linux" || process.arch !== "x64") {
      throw new Error(`the Linux media runtime builds only on x86_64 Linux, not ${process.platform}/${process.arch}`);
    }
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
