import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { compileRuntimeProbe, runtimeProbeEnvironment } from "./build-linux-media-runtime.mjs";
import { validateLinuxRuntimePayload } from "./linux-runtime-package.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
function hash(path) { return createHash("sha256").update(readFileSync(path)).digest("hex"); }
function execute(program, args, { env, cwd, input, logPath }) {
  const result = spawnSync(program, args, { env, cwd, input, encoding: "utf8", timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
  writeFileSync(logPath, `${result.stdout ?? ""}\n${result.stderr ?? ""}`);
  if (result.error || result.status !== 0) throw new Error(`packaged media command failed: ${program}; inspect ${logPath}: ${result.error?.message ?? result.stderr}`);
  return result.stdout;
}
function worker(program, request, context) {
  const output = execute(program, [], { ...context, input: `${JSON.stringify(request)}\n` });
  const events = output.split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
  const final = events.filter((event) => event.event === "completed" || event.event === "failed");
  if (final.length !== 1 || final[0].event !== "completed") throw new Error(`packaged helper failed acceptance: ${JSON.stringify(final)}; inspect ${context.logPath}`);
  return final[0].result;
}

function audioFixture(path) {
  const frames = 48_000 * 2 + 123;
  const data = Buffer.alloc(frames * 4);
  let seed = 7;
  for (let frame = 0; frame < frames; frame++) {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    const sample = Math.round((0.1 * Math.sin(2 * Math.PI * 180 * frame / 48_000) + (seed / 2 ** 32 - 0.5) * 0.06) * 32767);
    data.writeInt16LE(sample, frame * 4);
    data.writeInt16LE(sample, frame * 4 + 2);
  }
  const header = Buffer.alloc(44);
  header.write("RIFF"); header.writeUInt32LE(36 + data.length, 4); header.write("WAVEfmt ", 8);
  header.writeUInt32LE(16, 16); header.writeUInt16LE(1, 20); header.writeUInt16LE(2, 22);
  header.writeUInt32LE(48_000, 24); header.writeUInt32LE(192_000, 28); header.writeUInt16LE(4, 32); header.writeUInt16LE(16, 34);
  header.write("data", 36); header.writeUInt32LE(data.length, 40);
  writeFileSync(path, Buffer.concat([header, data]));
  return { frames, data };
}
function checkEnhancedWave(path, expected) {
  const wave = readFileSync(path);
  if (wave.toString("ascii", 0, 4) !== "RIFF" || wave.toString("ascii", 8, 12) !== "WAVE") throw new Error("packaged audio helper produced no WAVE output");
  let format; let data;
  for (let offset = 12; offset + 8 <= wave.length;) {
    const size = wave.readUInt32LE(offset + 4);
    if (offset + 8 + size > wave.length) throw new Error("packaged audio helper produced truncated WAVE data");
    const chunk = wave.subarray(offset + 8, offset + 8 + size);
    if (wave.toString("ascii", offset, offset + 4) === "fmt ") format = chunk;
    if (wave.toString("ascii", offset, offset + 4) === "data") data = chunk;
    offset += 8 + size + size % 2;
  }
  if (!format || format.length < 16 || format.readUInt16LE(0) !== 1 || format.readUInt16LE(2) !== 2 || format.readUInt32LE(4) !== 48_000 || format.readUInt16LE(14) !== 16 || !data || data.length !== expected.frames * 4 || data.equals(expected.data)) {
    throw new Error("packaged audio helper did not infer and preserve stereo PCM16 frames at 48 kHz");
  }
  return { frames: expected.frames, channels: 2, sampleRate: 48_000, bytes: wave.length, sha256: hash(path) };
}

/** Real encode/decode and Lottie acceptance using only packaged helpers and reviewed system dependencies. */
export function verifyLinuxPackageMedia({ packageRoot, evidenceDirectory }) {
  const root = resolve(packageRoot);
  const prefix = existsSync(join(root, "usr/bin")) ? join(root, "usr") : root;
  const decoder = join(prefix, "bin/video-creater-compatibility-decoder");
  const precompose = join(prefix, "bin/video-creater-precompose-worker");
  const audioEnhance = join(prefix, "bin/video-creater-audio-enhance");
  for (const helper of [decoder, precompose, audioEnhance]) if (!existsSync(helper)) throw new Error(`packaged render helper is missing: ${helper}`);
  const runtime = join(prefix, "lib/Video Creater/render-runtime");
  const manifest = validateLinuxRuntimePayload(runtime);
  const parent = evidenceDirectory ? resolve(evidenceDirectory) : tmpdir();
  mkdirSync(parent, { recursive: true });
  const work = mkdtempSync(join(parent, "vc-package-media-"));
  try {
    const plugins = join(work, "plugins");
    mkdirSync(plugins);
    mkdirSync(join(work, "lib"));
    for (const entry of manifest.bundledFiles.filter((entry) => entry.path.startsWith("lib/"))) {
      symlinkSync(join(runtime, entry.path), join(work, entry.path));
    }
    for (const name of manifest.linux.systemPluginFiles) {
      if (name.includes("/") || name.includes("\\") || name.startsWith(".")) throw new Error(`unsafe system plugin name: ${name}`);
      const source = join(manifest.linux.systemPluginDirectory, name);
      if (existsSync(source)) symlinkSync(source, join(plugins, name));
    }
    for (const name of manifest.linux.bundledPluginFiles) symlinkSync(join(runtime, "bundled-plugins", name), join(plugins, name));
    const cleanEnvironment = { PATH: "/usr/bin:/bin", XDG_DATA_HOME: join(work, "data"), XDG_CACHE_HOME: join(work, "cache"), ORC_CODE: "backup" };
    const env = runtimeProbeEnvironment({ pluginDirectory: plugins, scanner: manifest.linux.scanner, registry: join(work, "registry.bin"), baseEnvironment: cleanEnvironment });
    const probe = compileRuntimeProbe(join(work, "probe-build"));
    const media = join(work, "h264-aac.mp4");
    execute(probe, ["run", [
      "videotestsrc num-buffers=10 ! video/x-raw,width=64,height=64,framerate=10/1 ! videoconvert ! openh264enc ! h264parse ! queue ! mp4mux name=mux ! filesink location=" + JSON.stringify(media),
      "audiotestsrc num-buffers=10 samplesperbuffer=4800 ! audio/x-raw,rate=48000,channels=2 ! audioconvert ! avenc_aac ! aacparse ! queue ! mux.",
    ].join(" "), "30"], { env, cwd: work, logPath: join(work, "encode.log") });
    const context = { env, cwd: work };
    const request = { protocol: "video-creater.compatibility", schemaVersion: 1, requestId: "packaged-media-smoke", budgets: { timeoutMillis: 30_000, maxOutputBytes: 20 * 1024 * 1024 } };
    const probed = worker(decoder, { ...request, operation: { mode: "probe", sourcePath: media } }, { ...context, logPath: join(work, "probe.log") });
    if (!probed.probe?.video || !probed.probe?.audio || probed.probe.durationSeconds < 0.9 || probed.probe.durationSeconds > 1.2) {
      throw new Error(`packaged MP4 has incorrect duration or missing streams: ${JSON.stringify(probed)}`);
    }
    const frame = join(work, "review.bmp");
    const extracted = worker(decoder, { ...request, operation: { mode: "extractFrames", sourcePath: media, outputRoot: work, width: 64, height: 64, frames: [{ timeSeconds: 0.5, outputPath: frame }] } }, { ...context, logPath: join(work, "frames.log") });
    if (extracted.frames?.length !== 1 || statSync(frame).size <= 54) throw new Error("packaged review-frame extraction produced no pixels");
    const source = join(work, "lottie.json");
    copyFileSync(join(repoRoot, "src-tauri/crates/precompose-worker/tests/fixtures/value-layer-wiggle.json"), source);
    const rendered = worker(precompose, {
      protocol: "video-creater.precompose", schemaVersion: 2, requestId: "packaged-lottie-smoke", cacheKey: "a".repeat(64), operation: "bakeLottieRgba",
      source: { path: source, sha256: hash(source), format: "lottieJson", animationId: null },
      render: { width: 64, height: 64, fps: { numerator: 30, denominator: 1 }, firstFrame: 0, frameCount: 2, sourceStartMicros: 0, playbackRateMicros: 1_000_000, looping: false, alphaMode: "straight", colorSpace: "srgb" },
      inputs: { slots: [], marker: null, segment: null, stateMachine: null }, expressions: { enabled: true },
      budgets: { maxFrames: 2, maxPixelsPerFrame: 4096, maxSourceBytes: 1024 * 1024, maxArchiveEntries: 16, maxExpandedArchiveBytes: 1024 * 1024, maxCompressionRatio: 100, maxWallTimeMs: 30_000, maxMemoryBytes: 512 * 1024 * 1024, maxOutputBytes: 1024 * 1024 },
      output: { stagingDir: join(work, "lottie-frames") },
    }, { ...context, logPath: join(work, "lottie.log") });
    if (rendered.frameCount !== 2 || rendered.outputBytes <= 0 || !existsSync(rendered.manifestPath)) throw new Error("packaged Lottie helper did not produce two frames and a manifest");
    const audioInput = join(work, "noisy.wav");
    const expectedAudio = audioFixture(audioInput);
    const audioOutput = join(work, "enhanced.wav");
    execute(audioEnhance, ["--input", audioInput, "--output", audioOutput, "--model-directory", join(work, "audio-models"), "--strength", "1"], { ...context, logPath: join(work, "audio.log") });
    const audio = checkEnhancedWave(audioOutput, expectedAudio);
    const report = { status: "passed", packageRoot: root, isolatedEnvironment: true, mp4: { bytes: statSync(media).size, sha256: hash(media), ...probed.probe }, reviewFrame: { bytes: statSync(frame).size, sha256: hash(frame) }, lottie: rendered, audio, evidenceDirectory: evidenceDirectory ? work : null };
    writeFileSync(join(work, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
    return report;
  } finally {
    if (!evidenceDirectory) rmSync(work, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { console.log(JSON.stringify(verifyLinuxPackageMedia({ packageRoot: process.argv[2], evidenceDirectory: process.argv[3] }), null, 2)); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
