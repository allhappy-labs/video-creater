#!/usr/bin/env node
// Verifies a staged Linux media runtime for real: loads its plugin directory with
// an isolated registry, checks required/denied factories and plugin provenance,
// encodes H.264+AAC MP4 and ProRes MOV clips and decodes them back.

import { spawnSync } from "node:child_process";
import { lstatSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  bundledLibavPackageName,
  compileRuntimeProbe,
  defaultCacheDirectory,
  manifestFactories,
  parseProbeInventory,
  runtimeProbeEnvironment,
  runtimeTarget,
} from "./build-linux-media-runtime.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

export const requiredFactories = Object.freeze([
  // Bundled LGPL FFmpeg through gst-libav.
  "avdec_h264",
  "avdec_h265",
  "avdec_aac",
  "avdec_prores",
  "avenc_aac",
  "avenc_prores_ks",
  // Export, render and import paths.
  "openh264enc",
  "openh264dec",
  "mp4mux",
  "qtmux",
  "qtdemux",
  "matroskademux",
  "webmmux",
  "vp8enc",
  "vp9enc",
  "vp8dec",
  "vp9dec",
  "opusenc",
  "opusdec",
  "compositor",
  // Pitch-preserving audio clip speed.
  "scaletempo",
  "h264parse",
  "h265parse",
  "aacparse",
  "vorbisdec",
  "theoradec",
  "flacdec",
  "jpegdec",
  "pngdec",
  // Reversed video intermediates.
  "pngenc",
  "wavparse",
  "oggdemux",
  "avidemux",
  "id3demux",
  "nlecomposition",
  "gesdemux",
  // WebKitGTK media playback.
  "playbin",
  "playbin3",
  "decodebin",
  "decodebin3",
  "uridecodebin",
  "uridecodebin3",
  "urisourcebin",
  "typefind",
  "appsrc",
  "appsink",
  "videoconvert",
  "videoscale",
  "audioconvert",
  "audioresample",
  "volume",
  "autoaudiosink",
  "pulsesink",
  "glupload",
  "glcolorconvert",
  "glimagesink",
  "fakesink",
  "fakevideosink",
  "tee",
  "queue",
]);

// VA-API factories register only when a render node exists at registry-scan time;
// alsasink needs the separate gstreamer1.0-alsa package.
export const optionalFactories = Object.freeze(["vah264dec", "vah265dec", "vah264enc", "vah265enc", "alsasink"]);

export const deniedPluginNames = Object.freeze([
  "faad",
  "x264",
  "x265",
  "mpeg2enc",
  "mplex",
  "dtsdec",
  "resindvd",
  "a52dec",
  "mpeg2dec",
  "sid",
  "cdio",
  "dvdread",
]);

export function parseProbeRows(text, tag) {
  return text
    .split(/\r?\n/)
    .filter((line) => line.startsWith(`${tag}\t`))
    .map((line) => line.split("\t").slice(1));
}

export function chosenDecoders(text) {
  return parseProbeRows(text, "ELEMENT")
    .filter(([, klass]) => /Decoder/.test(klass ?? "") && !/\bBin\b/.test(klass ?? ""))
    .map(([name]) => name);
}

export function evaluateLoadedLibraries(text, runtimeLib) {
  const loaded = parseProbeRows(text, "LOADED").map(([path]) => path);
  const ffmpeg = loaded.filter((path) => /\/lib(?:avcodec|avformat|avfilter|avutil|swresample|swscale|postproc)[^/]*\.so/.test(path));
  const foreign = ffmpeg.filter((path) => !path.startsWith(`${runtimeLib}/`));
  return { ffmpeg, foreign };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const runtimeRoot = realpathSync(resolve(repoRoot, options.runtime));
  const manifest = JSON.parse(readFileSync(join(runtimeRoot, "manifest.json"), "utf8"));
  if (manifest.target !== runtimeTarget) throw new Error(`runtime target mismatch: ${manifest.target}`);

  const probe = compileRuntimeProbe(resolve(options.cacheDir));
  const work = mkdtempSync(join(tmpdir(), "video-creater-linux-media-verify-"));
  const failures = [];
  try {
    const environment = runtimeProbeEnvironment({
      pluginDirectory: join(runtimeRoot, "plugins"),
      scanner: join(runtimeRoot, "libexec", "gst-plugin-scanner"),
      registry: join(work, "registry.bin"),
    });
    const runProbe = (args, { allowFailure = false, timeoutMs = 120_000 } = {}) => {
      const result = spawnSync(probe, args, { encoding: "utf8", env: environment, timeout: timeoutMs, maxBuffer: 64 * 1024 * 1024 });
      if (!allowFailure && (result.error || result.status !== 0)) {
        throw new Error(`probe ${args[0]} failed (${result.error?.message ?? `exit ${result.status}`}):\n${result.stderr}\n${result.stdout}`);
      }
      return result;
    };

    // Layout: plugins/ must be exactly the manifest's system plugin links plus the bundled plugin.
    const layout = manifest.linux;
    const expectedPluginEntries = [...layout.systemPluginFiles, ...layout.bundledPluginFiles].sort();
    const actualPluginEntries = readdirSync(join(runtimeRoot, "plugins")).sort();
    if (JSON.stringify(expectedPluginEntries) !== JSON.stringify(actualPluginEntries)) failures.push("plugins/ differs from manifest.linux");
    for (const name of layout.systemPluginFiles) {
      const path = join(runtimeRoot, "plugins", name);
      if (!lstatSync(path).isSymbolicLink() || realpathSync(path) !== realpathSync(join(layout.systemPluginDirectory, name))) {
        failures.push(`plugins/${name} is not a link to ${layout.systemPluginDirectory}/${name}`);
      }
    }
    for (const name of layout.bundledPluginFiles) {
      if (realpathSync(join(runtimeRoot, "plugins", name)) !== join(runtimeRoot, layout.bundledPluginDirectory, name)) {
        failures.push(`plugins/${name} is not a link to ${layout.bundledPluginDirectory}/${name}`);
      }
    }

    // Inventory and provenance.
    const inventory = parseProbeInventory(runProbe(["inventory"]).stdout);
    const systemLibav = inventory.filter((plugin) => plugin.name === "libav" && plugin.package !== bundledLibavPackageName);
    if (systemLibav.length > 0) failures.push(`distribution libav plugin is visible: ${systemLibav[0].filename}`);
    const libav = inventory.find((plugin) => plugin.name === "libav");
    if (!libav || realpathSync(libav.filename) !== join(runtimeRoot, layout.bundledPluginDirectory, "libgstlibav.so")) {
      failures.push("bundled libgstlibav.so is not the registered libav plugin");
    }
    const denied = inventory.filter((plugin) => deniedPluginNames.includes(plugin.name));
    for (const plugin of denied) failures.push(`denied plugin is visible: ${plugin.name}`);
    const disallowedLicenses = inventory.filter((plugin) => !["LGPL", "BSD", "MIT/X11"].includes(plugin.license));
    for (const plugin of disallowedLicenses) failures.push(`plugin ${plugin.name} has license ${plugin.license}`);
    const blacklisted = inventory.filter((plugin) => plugin.status !== "loaded").map((plugin) => plugin.name);
    if (blacklisted.length > 0) failures.push(`plugins failed to load: ${blacklisted.join(", ")}`);
    const manifestPlugins = [...manifest.plugins].sort();
    const loadedPlugins = inventory.filter((plugin) => plugin.status === "loaded").map((plugin) => plugin.name).sort();
    if (JSON.stringify(manifestPlugins) !== JSON.stringify(loadedPlugins)) failures.push("manifest plugin list differs from the loaded registry");
    const registryFactories = new Map(manifestFactories(inventory).map((factory) => [factory.name, factory]));
    for (const factory of [...manifest.factories, ...manifest.inventory.factories]) {
      if (JSON.stringify(registryFactories.get(factory.name)) !== JSON.stringify(factory)) {
        failures.push(`manifest factory provenance drift: ${factory.name}`);
      }
    }
    if (manifest.inventory.factories.length !== registryFactories.size) failures.push("manifest inventory factory count differs from the loaded registry");

    const factoryResult = runProbe(["factories", ...requiredFactories], { allowFailure: true });
    const found = parseProbeRows(factoryResult.stdout, "FACTORY").map(([name, pluginName, packageName, license, filename]) => ({ name, pluginName, package: packageName, license, filename }));
    const missing = parseProbeRows(factoryResult.stdout, "MISSING").map(([name]) => name);
    for (const name of missing) failures.push(`required factory missing: ${name}`);
    for (const factory of found.filter((entry) => /^av(?:dec|enc)_/.test(entry.name))) {
      if (factory.pluginName !== "libav" || factory.package !== bundledLibavPackageName) {
        failures.push(`${factory.name} comes from ${factory.package}, not the bundled LGPL build`);
      }
    }
    const optional = parseProbeRows(runProbe(["factories", ...optionalFactories], { allowFailure: true }).stdout, "FACTORY").map(([name]) => name);
    const deniedFactories = parseProbeRows(runProbe(["factories", "faad", "x265enc", "x264enc", "dtsdec", "mpeg2enc"], { allowFailure: true }).stdout, "FACTORY").map(([name]) => name);
    for (const name of deniedFactories) failures.push(`denied factory is visible: ${name}`);

    // H.264 + AAC MP4 round trip.
    const mp4 = join(work, "h264-aac.mp4");
    runProbe([
      "run",
      [
        "videotestsrc num-buffers=60 ! video/x-raw,width=320,height=240,framerate=30/1 ! videoconvert",
        "! openh264enc ! h264parse ! queue ! mp4mux name=mux ! filesink location=" + JSON.stringify(mp4),
        "audiotestsrc num-buffers=60 samplesperbuffer=1600 ! audio/x-raw,rate=48000,channels=2 ! audioconvert",
        "! avenc_aac ! aacparse ! queue ! mux.",
      ].join(" "),
      "60",
    ]);
    const h264Decode = runProbe(["decode", mp4, "60"]);
    const h264Decoders = chosenDecoders(h264Decode.stdout);
    if (!h264Decoders.some((name) => ["avdec_h264", "openh264dec"].includes(name))) failures.push(`H.264 decoded by ${h264Decoders.join(", ") || "<none>"}`);
    if (!h264Decoders.some((name) => ["avdec_aac", "avdec_aac_fixed"].includes(name))) failures.push(`AAC decoded by ${h264Decoders.join(", ") || "<none>"}`);
    const h264Libraries = evaluateLoadedLibraries(h264Decode.stdout, join(runtimeRoot, "lib"));
    if (h264Libraries.ffmpeg.length === 0) failures.push("no bundled FFmpeg library was mapped during H.264/AAC decode");
    for (const path of h264Libraries.foreign) failures.push(`foreign FFmpeg library mapped: ${path}`);

    // ProRes + PCM MOV round trip.
    const mov = join(work, "prores-pcm.mov");
    runProbe([
      "run",
      [
        "videotestsrc num-buffers=15 ! video/x-raw,width=320,height=240,framerate=30/1 ! videoconvert",
        "! avenc_prores_ks ! queue ! qtmux name=mux ! filesink location=" + JSON.stringify(mov),
        "audiotestsrc num-buffers=15 samplesperbuffer=1600 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! queue ! mux.",
      ].join(" "),
      "60",
    ]);
    const proresDecode = runProbe(["decode", mov, "60"]);
    const proresDecoders = chosenDecoders(proresDecode.stdout);
    if (!proresDecoders.includes("avdec_prores")) failures.push(`ProRes decoded by ${proresDecoders.join(", ") || "<none>"}`);

    const bundle = bundleSize(runtimeRoot);
    const report = {
      schemaVersion: 1,
      status: failures.length === 0 ? "passed" : "failed",
      runtimeRoot,
      target: manifest.target,
      gstreamerVersion: manifest.gstreamerVersion,
      ffmpeg: `${manifest.ffmpeg.version} (${manifest.ffmpeg.reportedLicense})`,
      gstLibav: manifest.gstLibav.version,
      registeredPlugins: loadedPlugins.length,
      manifestReviewedFactories: manifest.factories.length,
      inventoryFactories: registryFactories.size,
      excludedPlugins: manifest.pluginPolicy.excluded.map((entry) => `${entry.file.split("/").pop()}: ${entry.reason}`),
      requiredFactories: { found: found.length, missing },
      libavFactories: found.filter((entry) => entry.pluginName === "libav").map((entry) => `${entry.name} (${entry.package})`),
      optionalFactories: { found: optional, absent: optionalFactories.filter((name) => !optional.includes(name)) },
      deniedFactoriesVisible: deniedFactories,
      h264AacMp4: { bytes: statSync(mp4).size, decoders: h264Decoders, ffmpegLibrariesLoaded: h264Libraries.ffmpeg },
      proresPcmMov: { bytes: statSync(mov).size, decoders: proresDecoders },
      bundle,
      failures,
    };
    console.log(JSON.stringify(report, null, 2));
    if (failures.length > 0) process.exitCode = 1;
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

function bundleSize(root) {
  let bundledBytes = 0;
  let linkedSystemBytes = 0;
  let files = 0;
  let links = 0;
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (lstatSync(path).isSymbolicLink()) {
        links += 1;
        linkedSystemBytes += statSync(path).size;
      } else {
        files += 1;
        bundledBytes += statSync(path).size;
      }
    }
  };
  visit(root);
  return { files, bundledBytes, links, linkedSystemBytes };
}

function parseArgs(argv) {
  const parsed = {
    runtime: "src-tauri/resources/render-runtime",
    cacheDir: process.env.VIDEO_CREATER_LINUX_BUILD_CACHE || defaultCacheDirectory(),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = () => {
      const next = argv[++index];
      if (!next || next.startsWith("--")) throw new Error(`missing value for ${argument}`);
      return next;
    };
    if (argument === "--runtime") parsed.runtime = value();
    else if (argument === "--cache-dir") parsed.cacheDir = value();
    else if (argument !== "--") throw new Error(`unknown argument: ${argument}`);
  }
  return parsed;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
