import assert from "node:assert/strict";
import test from "node:test";

import {
  allowedPluginLicenses,
  assertFfmpegLgplConfiguration,
  buildRuntimeManifest,
  bundledLibavPackageName,
  classifySystemPlugin,
  deniedLibraryReason,
  enabledFfmpegComponents,
  ffmpegBuildSuffix,
  ffmpegConfigureArgs,
  gstLibavMesonArgs,
  linuxRuntimeLayout,
  manifestFactories,
  parseArgs,
  parseProbeInventory,
  parseReadelfDynamic,
  pinnedSources,
  renderNotice,
  reviewedRenderFactories,
  runtimeProbeEnvironment,
  selectReviewedFactories,
  runtimeTarget,
} from "./build-linux-media-runtime.mjs";
import { chosenDecoders, evaluateLoadedLibraries } from "./verify-linux-media-runtime.mjs";

const lgplConfigH = [
  "#define FFMPEG_LICENSE \"LGPL version 2.1 or later\"",
  "#define CONFIG_GPL 0",
  "#define CONFIG_NONFREE 0",
  "#define CONFIG_VERSION3 0",
  "#define CONFIG_GPLV3 0",
  "#define CONFIG_LGPLV3 0",
].join("\n");
const lgplConfigMak = ["!CONFIG_GPL=yes", "!CONFIG_NONFREE=yes", "CONFIG_SHARED=yes", "!CONFIG_STATIC=yes"].join("\n");

const systemPlugin = (overrides = {}) => ({
  name: "isomp4",
  filename: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstisomp4.so",
  license: "LGPL",
  package: "GStreamer Good Plugins (Ubuntu)",
  status: "loaded",
  features: [],
  ...overrides,
});

test("pins every source archive to an HTTPS URL and SHA-256", () => {
  for (const source of Object.values(pinnedSources)) {
    assert.match(source.sha256, /^[a-f0-9]{64}$/);
    assert.equal(new URL(source.url).protocol, "https:");
    assert.ok(source.url.includes(source.version));
  }
  assert.equal(pinnedSources.gstLibav.version, "1.24.2");
  assert.match(pinnedSources.ffmpeg.version, /^6\.1\.\d+$/);
});

test("configures FFmpeg as shared LGPL-only libraries with an $ORIGIN run path", () => {
  const args = ffmpegConfigureArgs({ prefix: "/p", nasmPath: "/n/nasm", rpathResponseFile: "/r.rsp" });
  for (const flag of ["--disable-gpl", "--disable-nonfree", "--disable-version3", "--enable-shared", "--disable-static", "--disable-programs", "--disable-doc", "--disable-everything", "--disable-autodetect", "--disable-network"]) {
    assert.ok(args.includes(flag), flag);
  }
  assert.ok(!args.some((arg) => /^--enable-(?:gpl|nonfree|version3|libx26[45]|libfdk)/.test(arg)));
  assert.ok(args.includes(`--build-suffix=${ffmpegBuildSuffix}`));
  assert.ok(args.includes("--extra-ldsoflags=@/r.rsp"));
  const decoders = args.find((arg) => arg.startsWith("--enable-decoder="))?.split("=")[1]?.split(",") ?? [];
  for (const decoder of ["h264", "hevc", "aac", "prores"]) assert.ok(decoders.includes(decoder), decoder);
  assert.equal(args.find((arg) => arg.startsWith("--enable-encoder=")), "--enable-encoder=aac,prores_ks");
});

test("builds gst-libav with the package identity the Rust plugin policy allowlists", () => {
  const args = gstLibavMesonArgs({ prefix: "/p" });
  assert.ok(args.includes(`-Dpackage-name=${bundledLibavPackageName}`));
  assert.equal(bundledLibavPackageName.toLowerCase().replace(/[^a-z0-9]/g, ""), "videocreaterlgplffmpegplugins");
  assert.ok(args.includes("-Dc_link_args=-Wl,-rpath,$ORIGIN/../lib"));
});

test("accepts only LGPL FFmpeg configurations", () => {
  assert.equal(assertFfmpegLgplConfiguration({ configH: lgplConfigH, configMak: lgplConfigMak }).license, "LGPL version 2.1 or later");
  assert.throws(
    () => assertFfmpegLgplConfiguration({ configH: lgplConfigH.replace("CONFIG_GPL 0", "CONFIG_GPL 1"), configMak: lgplConfigMak }),
    /CONFIG_GPL must be 0/,
  );
  assert.throws(
    () => assertFfmpegLgplConfiguration({ configH: lgplConfigH.replace("LGPL version 2.1 or later", "GPL version 2 or later"), configMak: lgplConfigMak }),
    /license must be/,
  );
  assert.throws(
    () => assertFfmpegLgplConfiguration({ configH: lgplConfigH, configMak: `${lgplConfigMak}\nCONFIG_NONFREE=yes` }),
    /enables CONFIG_NONFREE/,
  );
  assert.throws(
    () => assertFfmpegLgplConfiguration({ configH: lgplConfigH, configMak: lgplConfigMak.replace("!CONFIG_STATIC=yes", "CONFIG_STATIC=yes") }),
    /shared-only/,
  );
});

test("parses enabled FFmpeg components", () => {
  const components = enabledFfmpegComponents(
    ["#define CONFIG_H264_DECODER 1", "#define CONFIG_VP9_DECODER 0", "#define CONFIG_PRORES_KS_ENCODER 1", "#define CONFIG_YADIF_FILTER 1", "#define CONFIG_MOV_DEMUXER 0"].join("\n"),
  );
  assert.deepEqual(components.decoders, ["h264"]);
  assert.deepEqual(components.encoders, ["prores_ks"]);
  assert.deepEqual(components.filters, ["yadif"]);
  assert.deepEqual(components.demuxers, []);
});

test("denylists GPL libraries by SONAME", () => {
  for (const soname of ["libavcodec.so.60", "libx265.so.199", "libfaad.so.2", "libdvdnav.so.4", "libreadline.so.8", "libdb-5.3.so", "libjbig.so.0"]) {
    assert.ok(deniedLibraryReason(soname), soname);
  }
  for (const soname of ["libavcodec_vc.so.60", "libopenh264.so.7", "libvpx.so.9", "libpulse.so.0", "libmp3lame.so.0", "libdav1d.so.7"]) {
    assert.equal(deniedLibraryReason(soname), null, soname);
  }
});

test("classifies system plugins by provenance, license and dependency closure", () => {
  const file = "/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstisomp4.so";
  const include = classifySystemPlugin({ file, plugin: systemPlugin(), closure: [{ soname: "libc.so.6", path: "/usr/lib/x86_64-linux-gnu/libc.so.6" }] });
  assert.equal(include.included, true);

  const cases = [
    [{ file: "/x/libgstlibav.so", plugin: systemPlugin({ name: "libav" }) }, /distribution gst-libav/],
    [{ file, plugin: null }, /not registered/],
    [{ file, plugin: systemPlugin({ status: "blacklisted" }) }, /blacklisted/],
    [{ file, plugin: systemPlugin({ name: "faad", license: "GPL" }) }, /GPL plugin/],
    [{ file, plugin: systemPlugin({ name: "asf", package: "GStreamer Ugly Plugins (Ubuntu)" }) }, /Ugly/],
    [{ file, plugin: systemPlugin({ name: "mpegpsdemux", license: "unknown" }) }, /license 'unknown'/],
    [{ file, plugin: systemPlugin({ name: "vendor", license: "Proprietary" }) }, /license 'Proprietary'/],
    [{ file, plugin: systemPlugin({ name: "frei0r" }) }, /frei0r/],
    [{ file, plugin: systemPlugin({ name: "chromaprint" }), closure: [{ soname: "libavcodec.so.60", path: "/usr/lib/x86_64-linux-gnu/libavcodec.so.60" }] }, /links libavcodec\.so\.60/],
    [{ file, plugin: systemPlugin(), unresolved: ["libmissing.so.1"] }, /unresolved/],
  ];
  for (const [input, reason] of cases) {
    const decision = classifySystemPlugin(input);
    assert.equal(decision.included, false, String(reason));
    assert.match(decision.reason, reason);
  }
  assert.deepEqual([...allowedPluginLicenses], ["LGPL", "BSD", "MIT/X11"]);
});

test("parses readelf dynamic sections", () => {
  const dynamic = parseReadelfDynamic(
    [
      " 0x0000000000000001 (NEEDED)             Shared library: [libavcodec_vc.so.60]",
      " 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]",
      " 0x000000000000000e (SONAME)             Library soname: [libgstlibav.so]",
      " 0x000000000000001d (RUNPATH)            Library runpath: [$ORIGIN/../lib]",
    ].join("\n"),
  );
  assert.deepEqual(dynamic, { needed: ["libavcodec_vc.so.60", "libc.so.6"], soname: "libgstlibav.so", runpath: ["$ORIGIN/../lib"], rpath: [] });
});

const inventoryText = [
  "PLUGIN\tlibav\t/rt/plugins/libgstlibav.so\tLGPL\tVideo Creater LGPL FFmpeg Plug-ins\tgst-libav\thttps://github.com/olhapi/video-creater\t1.24.2\tloaded",
  "FEATURE\tlibav\telement\tavdec_h264\t256\tCodec/Decoder/Video",
  "FEATURE\tlibav\telement\tavenc_aac\t256\tCodec/Encoder/Audio",
  "PLUGIN\tstaticelements\t\tLGPL\tGStreamer (Ubuntu)\tgstreamer\tUnknown\t1.24.2\tloaded",
  "FEATURE\tstaticelements\telement\tbin\t0\tGeneric/Bin",
  "PLUGIN\ttypefindfunctions\t/rt/plugins/libgsttypefindfunctions.so\tLGPL\tGStreamer Base Plugins (Ubuntu)\tgst-plugins-base\tUnknown\t1.24.2\tloaded",
  "FEATURE\ttypefindfunctions\ttypefind\tvideo/quicktime\t256\t",
].join("\n");

const reviewedFeatureRows = [...reviewedRenderFactories.portable, ...reviewedRenderFactories.linuxCodecs]
  .filter((name) => !["avdec_h264", "avenc_aac"].includes(name))
  .map((name) => `FEATURE\ttypefindfunctions\telement\t${name}\t0\tTest`);
const completeInventoryText = `${inventoryText}\n${reviewedFeatureRows.join("\n")}`;

test("lists only Rust-reviewed render factories in manifest factories", () => {
  const all = [...reviewedRenderFactories.portable, ...reviewedRenderFactories.linuxCodecs, "vah264dec", "playbin3"].map((name) => ({ name }));
  const selected = selectReviewedFactories(all).map((factory) => factory.name);
  assert.ok(selected.includes("vah264dec"));
  assert.ok(!selected.includes("playbin3"));
  assert.throws(() => selectReviewedFactories(all.filter((factory) => factory.name !== "openh264enc")), /missing.*openh264enc/);
  for (const name of reviewedRenderFactories.macosOnly) assert.ok(!selected.includes(name));
  assert.ok(reviewedRenderFactories.portable.includes("scaletempo"));
  assert.ok(reviewedRenderFactories.portable.includes("pngenc"));
});

test("generates a manifest compatible with the Rust RenderRuntimeManifest", () => {
  assert.deepEqual(
    manifestFactories(parseProbeInventory(inventoryText)).map((factory) => factory.name),
    ["avdec_h264", "avenc_aac", "bin"],
  );
  const inventory = parseProbeInventory(completeInventoryText);
  const manifest = buildRuntimeManifest({
    gstreamerVersion: "1.24.2",
    gesVersion: "1.24.2",
    inventory,
    pluginDecisions: [
      { file: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgsttypefindfunctions.so", plugin: "typefindfunctions", included: true, reason: "ok" },
      { file: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstfaad.so", plugin: "faad", included: false, reason: "GPL plugin (FAAD2)" },
    ],
    ffmpeg: { version: "6.1.6" },
    gstLibav: { version: "1.24.2" },
    nasm: { version: "2.16.03" },
    bundledFiles: [],
    systemLinks: [],
    systemLibraryClosure: [],
    linux: linuxRuntimeLayout({
      pluginDecisions: [
        { file: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgsttypefindfunctions.so", included: true },
        { file: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0/libgstfaad.so", included: false },
      ],
      bundledPluginFiles: ["libgstlibav.so"],
    }),
  });
  assert.equal(manifest.target, runtimeTarget);
  assert.deepEqual(manifest.linux, {
    systemPluginDirectory: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0",
    systemPluginFiles: ["libgsttypefindfunctions.so"],
    bundledPluginDirectory: "bundled-plugins",
    bundledPluginFiles: ["libgstlibav.so"],
    scanner: "/usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner",
    gstreamerCoreLibrary: "/usr/lib/x86_64-linux-gnu/libgstreamer-1.0.so.0",
  });
  assert.equal(typeof manifest.gstreamerVersion, "string");
  assert.equal(typeof manifest.gesVersion, "string");
  assert.deepEqual(manifest.plugins, ["libav", "staticelements", "typefindfunctions"]);
  assert.ok(!manifest.factories.some((factory) => factory.name === "bin"));
  assert.ok(manifest.inventory.factories.some((factory) => factory.name === "bin"));
  assert.deepEqual(manifest.inventory.staticPlugins, ["staticelements"]);
  assert.deepEqual(manifest.factories.find((factory) => factory.name === "avdec_h264"), {
    name: "avdec_h264",
    pluginName: "libav",
    package: "Video Creater LGPL FFmpeg Plug-ins",
    license: "LGPL",
  });
  assert.equal(manifest.pluginPolicy.included.length, 1);
  assert.equal(manifest.pluginPolicy.excluded[0].plugin, "faad");
});

test("rejects staged inventories with GPL plugins or the distribution libav plugin", () => {
  const base = { gstreamerVersion: "1.24.2", gesVersion: "1.24.2", pluginDecisions: [], ffmpeg: {}, gstLibav: {}, nasm: {}, bundledFiles: [], systemLinks: [], systemLibraryClosure: [] };
  assert.throws(
    () => buildRuntimeManifest({ ...base, inventory: parseProbeInventory(completeInventoryText.replace("\tLGPL\tVideo Creater", "\tGPL\tVideo Creater")) }),
    /disallowed license GPL/,
  );
  assert.throws(
    () => buildRuntimeManifest({ ...base, inventory: parseProbeInventory(completeInventoryText.replace("Video Creater LGPL FFmpeg Plug-ins", "GStreamer libav Plugins (Ubuntu)")) }),
    /not the bundled LGPL build/,
  );
});

test("renders an LGPL relinking notice", () => {
  const notice = renderNotice({
    ffmpeg: { version: "6.1.6", license: "LGPL-2.1-or-later", url: "https://ffmpeg.org/x", sha256: "a".repeat(64), libraries: ["libavcodec_vc.so.60"], reportedLicense: "LGPL version 2.1 or later", configureArgs: ["--prefix=/x", "--disable-gpl"] },
    gstLibav: { version: "1.24.2", license: "LGPL-2.1-or-later", url: "https://gstreamer.freedesktop.org/x", sha256: "b".repeat(64), mesonArgs: ["--prefix=/x", "-Ddoc=disabled"] },
    systemPluginDirectory: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0",
  });
  assert.match(notice, /Relinking and replacement/);
  assert.match(notice, /--disable-gpl/);
  assert.doesNotMatch(notice, /--prefix=/);
});

test("isolates the GStreamer plugin environment", () => {
  const environment = runtimeProbeEnvironment({
    pluginDirectory: "/rt/plugins",
    scanner: "/rt/libexec/gst-plugin-scanner",
    registry: "/tmp/registry.bin",
    baseEnvironment: { PATH: "/usr/bin", GST_PLUGIN_PATH_1_0: "/usr/lib/x86_64-linux-gnu/gstreamer-1.0", GST_DEBUG: "3" },
  });
  assert.equal(environment.GST_PLUGIN_SYSTEM_PATH_1_0, "");
  assert.equal(environment.GST_PLUGIN_PATH_1_0, "/rt/plugins");
  assert.equal(environment.GST_REGISTRY_1_0, "/tmp/registry.bin");
  assert.equal(environment.GST_DEBUG, undefined);
  assert.equal(environment.PATH, "/usr/bin");
});

test("parses builder arguments", () => {
  assert.deepEqual(parseArgs(["--output", "/tmp/rt", "--cache-dir", "/tmp/cache", "--jobs", "2"]), { output: "/tmp/rt", cacheDir: "/tmp/cache", jobs: 2, package: null });
  assert.equal(parseArgs(["--package", "/tmp/pkg"]).package, "/tmp/pkg");
  assert.throws(() => parseArgs(["--jobs", "0"]), /positive integer/);
  assert.throws(() => parseArgs(["--bogus"]), /unknown argument/);
});

test("reports chosen decoders and foreign FFmpeg libraries from probe output", () => {
  const output = [
    "ELEMENT\tdecodebin\tGeneric/Bin/Decoder",
    "ELEMENT\th264parse\tCodec/Parser/Converter/Video",
    "ELEMENT\tavdec_h264\tCodec/Decoder/Video",
    "ELEMENT\tavdec_aac\tCodec/Decoder/Audio",
    "LOADED\t/rt/lib/libavcodec_vc.so.60",
    "LOADED\t/usr/lib/x86_64-linux-gnu/libavcodec.so.60.31.102",
    "LOADED\t/usr/lib/x86_64-linux-gnu/libavc1394.so.0.3.0",
  ].join("\n");
  assert.deepEqual(chosenDecoders(output), ["avdec_h264", "avdec_aac"]);
  assert.deepEqual(evaluateLoadedLibraries(output, "/rt/lib"), {
    ffmpeg: ["/rt/lib/libavcodec_vc.so.60", "/usr/lib/x86_64-linux-gnu/libavcodec.so.60.31.102"],
    foreign: ["/usr/lib/x86_64-linux-gnu/libavcodec.so.60.31.102"],
  });
});
