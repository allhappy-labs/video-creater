import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import { isAbsolute, join, normalize, sep } from "node:path";

const manifestSchemaVersion = 1;

export const requiredLibraries = Object.freeze([
  "libgstreamer-1.0.0.dylib",
  "libgstbase-1.0.0.dylib",
  "libgstapp-1.0.0.dylib",
  "libgstvideo-1.0.0.dylib",
  "libgstaudio-1.0.0.dylib",
  "libgstpbutils-1.0.0.dylib",
  "libges-1.0.0.dylib",
]);

const nativeRuntimeFactories = [
  "appsink",
  "decodebin",
  "qtdemux",
  "matroskademux",
  "pngdec",
  "h264parse",
  "h265parse",
  "vtdec",
  "vp8dec",
  "vp9dec",
  "videoconvert",
  "videoscale",
  "videorate",
  "videocrop",
  "capsfilter",
  "mp4mux",
  "qtmux",
  "aacparse",
  "atdec",
  "atenc",
  "vtenc_h264",
  "vtenc_h265",
  "vtenc_prores",
  "webmmux",
  "vp8enc",
  "vp9enc",
  "opusenc",
  "opusdec",
];

const renderAndPrecomposeFactories = [
  "filesrc",
  "filesink",
  "multifilesrc",
  "queue",
  "appsrc",
  "videoflip",
  "volume",
  "audioconvert",
  "audiomixer",
  "audiorate",
  "audioresample",
  // Pitch-preserving audio clip speed (audiofx), required on Linux too.
  "scaletempo",
  "autoaudiosink",
  "autovideosink",
  "compositor",
  "rotate",
  "gldownload",
  "videotestsrc",
  "audiotestsrc",
];

export const requiredFactories = Object.freeze([
  ...new Set([...nativeRuntimeFactories, ...renderAndPrecomposeFactories]),
]);

export const requiredPlugins = Object.freeze([
  "app",
  "applemedia",
  "audioconvert",
  "audiofx",
  "audiomixer",
  "audioparsers",
  "audioresample",
  "audiorate",
  "audiotestsrc",
  "autodetect",
  "compositor",
  "coreelements",
  "encoding",
  "geometrictransform",
  "ges",
  "gio",
  "imagefreeze",
  "isomp4",
  "matroska",
  "multifile",
  "nle",
  "opengl",
  "opus",
  "osxaudio",
  "playback",
  "png",
  "typefindfunctions",
  "videoconvertscale",
  "videocrop",
  "videofilter",
  "videoparsersbad",
  "videorate",
  "videotestsrc",
  "volume",
  "vpx",
]);

const deniedPluginPattern =
  /(?:libav|ffmpeg|x264|x265|fdkaac|fdk-aac|faac|openh264)/i;
const packageManagerDependencyPattern = /(?:^|\/)(?:opt\/homebrew|usr\/local)(?:\/|$)/;
const sha256Pattern = /^[a-f0-9]{64}$/;
const reviewedLicenseExpressions = new Set([
  "BSD-2-Clause AND BSD-3-Clause",
  "BSD-3-Clause",
  "CC0-1.0",
  "IJG AND Zlib AND BSD-3-Clause",
  "LGPL-2.1-or-later",
  "MIT",
  "libpng-2.0",
]);

const corePackages = ["gstreamer", "gstreamersourcerelease"];
const basePackages = ["gstreamerbaseplugins", "gstreamerbasepluginssourcerelease"];
const goodPackages = ["gstreamergoodplugins", "gstreamergoodpluginssourcerelease"];
const badPackages = ["gstreamerbadplugins", "gstreamerbadpluginssourcerelease"];

const factoryPolicy = new Map([
  ["filesrc", policy("coreelements", corePackages)],
  ["filesink", policy("coreelements", corePackages)],
  ["queue", policy("coreelements", corePackages)],
  ["capsfilter", policy("coreelements", corePackages)],
  ["multifilesrc", policy("multifile", goodPackages)],
  ["appsrc", policy("app", basePackages)],
  ["appsink", policy("app", basePackages)],
  ["decodebin", policy("playback", basePackages)],
  ["videoconvert", policy(["videoconvertscale", "videoconvert"], basePackages)],
  ["videoscale", policy(["videoconvertscale", "videoscale"], basePackages)],
  ["videorate", policy("videorate", basePackages)],
  ["videocrop", policy("videocrop", goodPackages)],
  ["videoflip", policy("videofilter", goodPackages)],
  ["volume", policy("volume", basePackages)],
  ["audioconvert", policy("audioconvert", basePackages)],
  ["audiomixer", policy("audiomixer", basePackages)],
  ["audiorate", policy("audiorate", basePackages)],
  ["audioresample", policy("audioresample", basePackages)],
  ["scaletempo", policy("audiofx", goodPackages)],
  ["autoaudiosink", policy("autodetect", goodPackages)],
  ["autovideosink", policy("autodetect", goodPackages)],
  ["compositor", policy("compositor", basePackages)],
  ["rotate", policy("geometrictransform", badPackages)],
  ["gldownload", policy("opengl", basePackages)],
  ["videotestsrc", policy("videotestsrc", basePackages)],
  ["audiotestsrc", policy("audiotestsrc", basePackages)],
  ["pngdec", policy("png", goodPackages)],
  ["qtdemux", policy("isomp4", goodPackages)],
  ["mp4mux", policy("isomp4", goodPackages)],
  ["qtmux", policy("isomp4", goodPackages)],
  ["matroskademux", policy("matroska", goodPackages)],
  ["webmmux", policy("matroska", goodPackages)],
  ["h264parse", policy("videoparsersbad", badPackages)],
  ["h265parse", policy("videoparsersbad", badPackages)],
  ["vtdec", policy("applemedia", badPackages)],
  ["vp8dec", policy("vpx", goodPackages)],
  ["vp9dec", policy("vpx", goodPackages)],
  ["vp8enc", policy("vpx", goodPackages)],
  ["vp9enc", policy("vpx", goodPackages)],
  ["aacparse", policy("audioparsers", goodPackages)],
  ["atdec", policy("osxaudio", goodPackages)],
  ["atenc", policy("osxaudio", goodPackages)],
  ["vtenc_h264", policy("applemedia", badPackages)],
  ["vtenc_h265", policy("applemedia", badPackages)],
  ["vtenc_prores", policy("applemedia", badPackages)],
  ["opusenc", policy("opus", basePackages)],
  ["opusdec", policy("opus", basePackages)],
]);

const gstreamerRuntimeFiles = new Set([
  ...requiredLibraries.map((library) => `lib/${library}`),
  ...requiredPlugins.map((plugin) => `plugins/libgst${plugin}.dylib`),
  "lib/libgstcheck-1.0.0.dylib",
  "lib/libgstcodecparsers-1.0.0.dylib",
  "lib/libgstcontroller-1.0.0.dylib",
  "lib/libgstgl-1.0.0.dylib",
  "lib/libgstriff-1.0.0.dylib",
  "lib/libgstrtp-1.0.0.dylib",
  "lib/libgsttag-1.0.0.dylib",
  "lib/libgstvalidate-1.0.0.dylib",
  "libexec/gst-plugin-scanner",
  "licenses/GStreamer-LICENSE.txt",
]);

const reviewedRuntimeComponents = new Map([
  [
    "gstreamer",
    componentReview({
      component: "GStreamer",
      formulaLicense: "LGPL-2.0-or-later AND LGPL-2.1-or-later AND MIT",
      files: gstreamerRuntimeFiles,
      license: "LGPL-2.1-or-later",
      repositoryUrl: "https://gitlab.freedesktop.org/gstreamer/gstreamer",
    }),
  ],
  [
    "glib",
    componentReview({
      component: "GLib",
      formulaLicense: "LGPL-2.1-or-later",
      files: [
        "lib/libgio-2.0.0.dylib",
        "lib/libglib-2.0.0.dylib",
        "lib/libgmodule-2.0.0.dylib",
        "lib/libgobject-2.0.0.dylib",
      ],
      license: "LGPL-2.1-or-later",
      repositoryUrl: "https://gitlab.gnome.org/GNOME/glib",
    }),
  ],
  [
    "gettext",
    componentReview({
      component: "GNU gettext libintl",
      formulaLicense: "GPL-3.0-or-later AND LGPL-2.1-or-later",
      files: ["lib/libintl.8.dylib"],
      license: "LGPL-2.1-or-later",
      repositoryUrl: "https://git.savannah.gnu.org/git/gettext.git",
    }),
  ],
  [
    "graphene",
    componentReview({
      component: "Graphene",
      formulaLicense: "MIT",
      files: ["lib/libgraphene-1.0.0.dylib"],
      license: "MIT",
      repositoryUrl: "https://github.com/ebassi/graphene",
    }),
  ],
  [
    "jpeg-turbo",
    componentReview({
      component: "libjpeg-turbo",
      formulaLicense: "IJG AND Zlib AND BSD-3-Clause",
      files: ["lib/libjpeg.8.dylib"],
      license: "IJG AND Zlib AND BSD-3-Clause",
      repositoryUrl: "https://github.com/libjpeg-turbo/libjpeg-turbo",
    }),
  ],
  [
    "json-glib",
    componentReview({
      component: "JSON-GLib",
      formulaLicense: "LGPL-2.1-or-later",
      files: ["lib/libjson-glib-1.0.0.dylib"],
      license: "LGPL-2.1-or-later",
      repositoryUrl: "https://gitlab.gnome.org/GNOME/json-glib",
    }),
  ],
  [
    "libpng",
    componentReview({
      component: "libpng",
      formulaLicense: "libpng-2.0",
      files: ["lib/libpng16.16.dylib"],
      license: "libpng-2.0",
      repositoryUrl: "https://github.com/pnggroup/libpng",
    }),
  ],
  [
    "libvpx",
    componentReview({
      component: "libvpx",
      formulaLicense: "BSD-3-Clause",
      files: ["lib/libvpx.12.dylib"],
      license: "BSD-3-Clause",
      repositoryUrl: "https://chromium.googlesource.com/webm/libvpx",
    }),
  ],
  [
    "libx11",
    componentReview({
      component: "libX11",
      formulaLicense: "MIT",
      files: ["lib/libX11-xcb.1.dylib", "lib/libX11.6.dylib"],
      license: "MIT",
      repositoryUrl: "https://gitlab.freedesktop.org/xorg/lib/libx11",
    }),
  ],
  [
    "libxau",
    componentReview({
      component: "libXau",
      formulaLicense: "MIT",
      files: ["lib/libXau.6.dylib"],
      license: "MIT",
      repositoryUrl: "https://gitlab.freedesktop.org/xorg/lib/libxau",
    }),
  ],
  [
    "libxcb",
    componentReview({
      component: "libxcb",
      formulaLicense: "MIT",
      files: ["lib/libxcb.1.dylib"],
      license: "MIT",
      repositoryUrl: "https://gitlab.freedesktop.org/xorg/lib/libxcb",
    }),
  ],
  [
    "libxdmcp",
    componentReview({
      component: "libXdmcp",
      formulaLicense: "MIT",
      files: ["lib/libXdmcp.6.dylib"],
      license: "MIT",
      repositoryUrl: "https://gitlab.freedesktop.org/xorg/lib/libxdmcp",
    }),
  ],
  [
    "opus",
    componentReview({
      component: "Opus",
      formulaLicense: "BSD-3-Clause",
      files: ["lib/libopus.0.dylib"],
      license: "BSD-3-Clause",
      repositoryUrl: "https://gitlab.xiph.org/xiph/opus",
    }),
  ],
  [
    "orc",
    componentReview({
      component: "ORC",
      formulaLicense: "BSD-2-Clause AND BSD-3-Clause",
      files: ["lib/liborc-0.4.0.dylib"],
      license: "BSD-2-Clause AND BSD-3-Clause",
      repositoryUrl: "https://gitlab.freedesktop.org/gstreamer/orc",
    }),
  ],
  [
    "pcre2",
    componentReview({
      component: "PCRE2",
      formulaLicense: "BSD-3-Clause",
      files: ["lib/libpcre2-8.0.dylib"],
      license: "BSD-3-Clause",
      repositoryUrl: "https://github.com/PCRE2Project/pcre2",
    }),
  ],
  [
    "video-creater",
    componentReview({
      component: "Video Creater",
      files: [
        "libexec/gstreamer-runtime-probe",
        "licenses/THIRD_PARTY_NOTICES.md",
      ],
      license: "MIT",
      repositoryUrl: "https://git.home.olhapi.com/olhapi/video-creater",
    }),
  ],
]);

export const reviewedRuntimeFormulaNames = Object.freeze(
  [...reviewedRuntimeComponents.keys()].filter((name) => name !== "video-creater"),
);

export function validateCuratedSelection({ libraries, plugins }) {
  requireStringArray(libraries, "libraries");
  requireStringArray(plugins, "plugins");

  for (const library of libraries) {
    if (!requiredLibraries.includes(library)) {
      throw new Error(`unreviewed GStreamer library: ${library}`);
    }
  }
  for (const plugin of plugins) {
    if (deniedPluginPattern.test(plugin)) {
      throw new Error(`denied GStreamer plugin: ${plugin}`);
    }
    if (!requiredPlugins.includes(plugin)) {
      throw new Error(`unreviewed GStreamer plugin: ${plugin}`);
    }
  }
  requireMembers(libraries, requiredLibraries, "library");
  requireMembers(plugins, requiredPlugins, "plugin");
}

export function evaluateFactoryProvenance(factory) {
  const expected = factoryPolicy.get(factory?.name);
  if (!expected) {
    return {
      verdict: "denied",
      reason: `GStreamer factory '${factory?.name ?? "<missing>"}' is not in the reviewed allowlist.`,
    };
  }
  const pluginName = canonical(factory.pluginName);
  const packageName = canonical(factory.package);
  const license = String(factory.license ?? "").trim().toLowerCase();
  if (
    !expected.pluginNames.includes(pluginName) ||
    !expected.packageFamilies.includes(packageName) ||
    !isLgplCompatible(license)
  ) {
    return {
      verdict: "denied",
      reason: `GStreamer factory '${factory.name}' provenance does not match the reviewed LGPL-compatible allowlist.`,
    };
  }
  return {
    verdict: "allowed",
    reason: `GStreamer factory '${factory.name}' is in the reviewed LGPL-compatible allowlist.`,
  };
}

export function resolveReviewedRuntimeComponent({
  owner,
  file,
  installedVersion,
  formula,
}) {
  const ownerName = String(owner ?? "").trim();
  const fileName = String(file ?? "").trim();
  const version = String(installedVersion ?? "").trim();
  const review = reviewedRuntimeComponents.get(ownerName);
  if (!review || !review.files.has(fileName)) {
    throw new Error(
      `unreviewed runtime owner/file: ${ownerName || "<missing>"}/${fileName || "<missing>"}`,
    );
  }
  if (!isReviewedSpdxLicense(review.license)) {
    throw new Error(`runtime component requires reviewed SPDX license: ${ownerName}`);
  }

  let sourceUrl = review.repositoryUrl;
  if (review.formulaLicense) {
    if (
      canonical(formula?.name) !== canonical(ownerName) ||
      formula?.license !== review.formulaLicense
    ) {
      throw new Error(`runtime formula requires reviewed SPDX license: ${ownerName}`);
    }
    const stableVersion = String(formula?.versions?.stable ?? "").trim();
    const stableUrl = String(formula?.urls?.stable?.url ?? "").trim();
    if (version && version === stableVersion) sourceUrl = stableUrl;
  }
  validateCorrespondingSourceUrl(sourceUrl);
  return {
    component: `${review.component}${version ? ` ${version}` : ""}`,
    file: fileName,
    license: review.license,
    sourceUrl,
  };
}

export function validateCorrespondingSourceUrl(sourceUrl) {
  const rawUrl = String(sourceUrl ?? "").trim();
  let parsed;
  try {
    parsed = new URL(rawUrl);
  } catch {
    throw new Error(`upstream corresponding-source URL is required: ${rawUrl}`);
  }
  if (
    !["https:", "http:"].includes(parsed.protocol) ||
    /^(?:ghcr\.io|localhost)$/i.test(parsed.hostname) ||
    /(?:^|\/)(?:opt\/homebrew|usr\/local)(?:\/|$)/i.test(parsed.pathname) ||
    /(?:^|\/)(?:blobs?|bottles?)(?:\/|$)/i.test(parsed.pathname)
  ) {
    throw new Error(`upstream corresponding-source URL is required: ${rawUrl}`);
  }
  return parsed.href;
}

export function renderThirdPartyNotices(components) {
  if (!Array.isArray(components) || components.length === 0) {
    throw new Error("runtime license inventory must not be empty");
  }
  const reviewed = new Map();
  for (const component of components) {
    const name = String(component?.component ?? "").trim();
    const file = String(component?.file ?? "").trim();
    const sourceUrl = String(component?.sourceUrl ?? "").trim();
    const license = String(component?.license ?? "").trim();
    if (!name || !file || !isReviewedSpdxLicense(license)) {
      throw new Error(
        `runtime component requires exact file and reviewed SPDX license: ${name || "<missing>"}`,
      );
    }
    validateCorrespondingSourceUrl(sourceUrl);
    reviewed.set(`${name}\0${file}\0${sourceUrl}\0${license}`, {
      name,
      file,
      sourceUrl,
      license,
    });
  }
  return [
    "# Render Runtime Third-Party Notices",
    "",
    "This directory contains a dynamically linked, curated GStreamer and GES runtime.",
    "",
    "## LGPL relinking and replacement",
    "",
    "Upstream corresponding source for every staged component/file is available from the URLs below.",
    "The libraries remain dynamically linked so users may replace or relink LGPL components with ABI-compatible modified builds for debugging or modification.",
    "The file names identify runtime payloads only; the URLs identify their upstream source projects or archives.",
    "Retain this notice when redistributing the runtime.",
    "",
    "## Components",
    "",
    ...[...reviewed.values()].flatMap((component) => [
      `- ${component.name}`,
      `  - Runtime file: ${component.file}`,
      `  - License: ${component.license}`,
      `  - Upstream corresponding source: ${component.sourceUrl}`,
    ]),
    "",
    "Excluded by policy: gst-libav, FFmpeg/libav, x264, x265, FDK-AAC, FAAC, and OpenH264.",
    "",
  ].join("\n");
}

export function validateRuntimeManifest(
  manifest,
  { runtimeRoot, requireCompleteRuntime = true } = {},
) {
  if (!manifest || typeof manifest !== "object") {
    throw new Error("runtime manifest must be an object");
  }
  if (manifest.schemaVersion !== manifestSchemaVersion) {
    throw new Error(`runtime manifest schema must be ${manifestSchemaVersion}`);
  }
  for (const field of ["gstreamerVersion", "gesVersion"]) {
    if (typeof manifest[field] !== "string" || manifest[field].trim() === "") {
      throw new Error(`runtime manifest ${field} is required`);
    }
  }
  if (!Array.isArray(manifest.files)) {
    throw new Error("runtime manifest files must be an array");
  }

  const seenPaths = new Set();
  for (const entry of manifest.files) {
    validateFileEntry(entry, runtimeRoot);
    if (seenPaths.has(entry.path)) {
      throw new Error(`duplicate runtime manifest path: ${entry.path}`);
    }
    seenPaths.add(entry.path);
  }

  if (!requireCompleteRuntime) return;

  if (!seenPaths.has("libexec/gst-plugin-scanner")) {
    throw new Error("required gst-plugin-scanner is absent from the runtime manifest");
  }
  if (!seenPaths.has("lib/libges-1.0.0.dylib")) {
    throw new Error("required GES library libges-1.0.0.dylib is absent from the runtime manifest");
  }
  requireStringArray(manifest.factories, "factories", true);
  const factoriesByName = new Map(
    manifest.factories.map((factory) => [factory?.name, factory]),
  );
  for (const name of requiredFactories) {
    const factory = factoriesByName.get(name);
    if (!factory) throw new Error(`required factory is absent: ${name}`);
    const decision = evaluateFactoryProvenance(factory);
    if (decision.verdict !== "allowed") throw new Error(decision.reason);
  }

  validateCuratedSelection({
    libraries: manifest.libraries,
    plugins: manifest.plugins,
  });
  for (const library of requiredLibraries) {
    if (!seenPaths.has(`lib/${library}`)) {
      throw new Error(`required library is absent from runtime files: ${library}`);
    }
  }
  for (const plugin of requiredPlugins) {
    if (!seenPaths.has(`plugins/libgst${plugin}.dylib`)) {
      throw new Error(`required plugin is absent from runtime files: ${plugin}`);
    }
  }
}

export function sha256File(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function isSystemDependency(path) {
  return path.startsWith("/System/Library/") || path.startsWith("/usr/lib/");
}

export function rejectDeniedDependency(path) {
  if (deniedPluginPattern.test(path)) {
    throw new Error(`denied codec dependency: ${path}`);
  }
}

export function rejectExternalPackageManagerDependency(path) {
  if (packageManagerDependencyPattern.test(path)) {
    throw new Error(`external package-manager dependency: ${path}`);
  }
}

function validateFileEntry(entry, runtimeRoot) {
  if (!entry || typeof entry !== "object") {
    throw new Error("runtime manifest file entry must be an object");
  }
  if (
    typeof entry.path !== "string" ||
    entry.path.trim() === "" ||
    isAbsolute(entry.path) ||
    normalize(entry.path).split(sep).includes("..")
  ) {
    throw new Error(`runtime manifest file path must be relative: ${entry.path}`);
  }
  if (!Number.isInteger(entry.bytes) || entry.bytes < 0) {
    throw new Error(`runtime manifest file bytes are invalid: ${entry.path}`);
  }
  if (typeof entry.sha256 !== "string" || !sha256Pattern.test(entry.sha256)) {
    throw new Error(`runtime file entry requires immutable SHA-256: ${entry.path}`);
  }
  if (typeof entry.license !== "string" || entry.license.trim() === "") {
    throw new Error(`runtime file license is required: ${entry.path}`);
  }
  if (!isReviewedSpdxLicense(entry.license)) {
    throw new Error(`runtime file requires reviewed SPDX license: ${entry.path}`);
  }
  if (
    typeof entry.sourceComponent !== "string" ||
    entry.sourceComponent.trim() === "" ||
    typeof entry.sourceFile !== "string" ||
    entry.sourceFile !== entry.path ||
    typeof entry.sourceUrl !== "string" ||
    entry.sourceUrl.trim() === ""
  ) {
    throw new Error(`runtime file source provenance is required: ${entry.path}`);
  }
  validateCorrespondingSourceUrl(entry.sourceUrl);
  if (!Array.isArray(entry.machODependencies)) {
    throw new Error(`runtime Mach-O dependency list is required: ${entry.path}`);
  }
  for (const dependency of entry.machODependencies) {
    rejectDeniedDependency(dependency);
    rejectExternalPackageManagerDependency(dependency);
  }
  if (!runtimeRoot) return;
  const filePath = join(runtimeRoot, entry.path);
  if (!existsSync(filePath)) {
    throw new Error(`runtime file is missing: ${entry.path}`);
  }
  const stats = statSync(filePath);
  if (!stats.isFile() || stats.size !== entry.bytes || sha256File(filePath) !== entry.sha256) {
    throw new Error(`runtime file hash mismatch: ${entry.path}`);
  }
}

function policy(pluginNames, packageFamilies) {
  return {
    pluginNames: (Array.isArray(pluginNames) ? pluginNames : [pluginNames]).map(canonical),
    packageFamilies: packageFamilies.map(canonical),
  };
}

function canonical(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]/g, "");
}

function isLgplCompatible(license) {
  const tokens = licenseTokens(license);
  const hasLgplMarker =
    tokens.some(isLgplLicenseToken) ||
    containsTokenPhrase(tokens, [
      "gnu",
      "lesser",
      "general",
      "public",
      "license",
    ]);
  return (
    hasLgplMarker &&
    tokens.length > 0 &&
    tokens.every(isLgplLicenseComponent) &&
    hasValidLgplOrLaterUsage(tokens)
  );
}

function isReviewedSpdxLicense(license) {
  return reviewedLicenseExpressions.has(String(license ?? "").trim());
}

function componentReview({
  component,
  files,
  formulaLicense = null,
  license,
  repositoryUrl,
}) {
  return {
    component,
    files: files instanceof Set ? files : new Set(files),
    formulaLicense,
    license,
    repositoryUrl,
  };
}

function licenseTokens(license) {
  return String(license ?? "")
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
}

function isGplLicenseToken(token) {
  return token === "gpl" || isLicenseVersionSuffix(token.slice(3), token.startsWith("gpl"));
}

function isLgplLicenseToken(token) {
  return token === "lgpl" || isLicenseVersionSuffix(token.slice(4), token.startsWith("lgpl"));
}

function isLicenseVersionSuffix(suffix, hasPrefix) {
  if (!hasPrefix) return false;
  const value = suffix.startsWith("v") ? suffix.slice(1) : suffix;
  return value !== "" && /^\d+$/.test(value);
}

function isLgplLicenseComponent(token) {
  return (
    isLgplLicenseToken(token) ||
    ["gnu", "lesser", "general", "public", "license", "only", "or", "later"].includes(
      token,
    ) ||
    isLicenseVersionToken(token)
  );
}

function isLicenseVersionToken(token) {
  const value = token.startsWith("v") ? token.slice(1) : token;
  return value !== "" && /^\d+$/.test(value);
}

function hasValidLgplOrLaterUsage(tokens) {
  return tokens.every((token, index) => {
    if (token === "or") return tokens[index + 1] === "later";
    if (token === "later") return index > 0 && tokens[index - 1] === "or";
    return true;
  });
}

function containsTokenPhrase(tokens, phrase) {
  return tokens.some(
    (_, index) =>
      index + phrase.length <= tokens.length &&
      phrase.every((token, offset) => tokens[index + offset] === token),
  );
}

function requireStringArray(value, label, objects = false) {
  if (!Array.isArray(value)) throw new Error(`${label} must be an array`);
  if (objects) return;
  if (value.some((entry) => typeof entry !== "string" || entry.trim() === "")) {
    throw new Error(`${label} must contain non-empty strings`);
  }
}

function requireMembers(actual, required, label) {
  const values = new Set(actual);
  for (const entry of required) {
    if (!values.has(entry)) throw new Error(`required ${label} is absent: ${entry}`);
  }
}
