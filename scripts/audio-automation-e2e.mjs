#!/usr/bin/env node

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repoRoot = resolve(import.meta.dirname, "..");
const outputArgument = process.argv.slice(2).find((value) => value !== "--");
const outputRoot = resolve(repoRoot, outputArgument ?? "output/audio-automation-evidence");
const exporter = resolve(
  repoRoot,
  process.env.VIDEO_CREATER_AVFOUNDATION_EXPORTER ??
    "src-tauri/binaries/video-creater-avfoundation-exporter-aarch64-apple-darwin",
);
rmSync(outputRoot, { recursive: true, force: true });
mkdirSync(outputRoot, { recursive: true });
const source = resolve(outputRoot, "source.mp4");

checked("gst-launch-1.0", [
  "-q",
  "mp4mux", "name=mux", "faststart=true", "!", "filesink", `location=${source}`,
  "videotestsrc", "num-buffers=96", "pattern=black", "!",
  "video/x-raw,width=320,height=180,framerate=24/1", "!", "vtenc_h264", "!",
  "h264parse", "!", "queue", "!", "mux.video_0",
  "audiotestsrc", "num-buffers=188", "wave=sine", "freq=440", "!",
  "audio/x-raw,rate=48000", "!", "avenc_aac", "!", "aacparse", "!", "queue", "!",
  "mux.audio_0",
]);

const control = runExport("control", []);
const automated = runExport("automated", [
  { atSeconds: 0, valueDb: -30, easing: "easeInOut" },
  { atSeconds: 2, valueDb: 0, easing: "easeOut" },
  { atSeconds: 4, valueDb: -18, easing: "linear" },
]);
const windows = [
  { id: "opening", start: 0.1, duration: 0.2 },
  { id: "automation", start: 1.0, duration: 0.2 },
  { id: "peak", start: 1.9, duration: 0.2 },
  { id: "closing", start: 3.7, duration: 0.2 },
];
const measurements = Object.fromEntries(
  windows.map((window) => [
    window.id,
    {
      control: rms(control.outputPath, window.start, window.duration),
      automated: rms(automated.outputPath, window.start, window.duration),
    },
  ]),
);
const checks = {
  fadeInApplied: measurements.opening.automated < measurements.peak.automated * 0.12,
  volumeAutomationApplied:
    measurements.automation.automated < measurements.automation.control * 0.45,
  automationReachesPeak:
    measurements.peak.automated > measurements.automation.automated * 2.5,
  fadeOutApplied: measurements.closing.automated < measurements.peak.automated * 0.12,
};
const report = {
  schemaVersion: 1,
  status: Object.values(checks).every(Boolean) ? "passed" : "failed",
  exporterProtocol: 5,
  source,
  control: control.outputPath,
  automated: automated.outputPath,
  measurements,
  checks,
};
writeFileSync(resolve(outputRoot, "audio-automation-evidence.json"), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
if (report.status !== "passed") process.exit(1);

function runExport(id, volumeKeyframes) {
  const outputPath = resolve(outputRoot, `${id}.mp4`);
  const request = {
    protocol: "video-creater.avfoundation-export",
    schemaVersion: 5,
    requestId: `audio-automation-${id}`,
    outputPath,
    profile: "h264",
    quality: "final",
    width: 320,
    height: 180,
    fps: 24,
    videoClips: [{
      sourcePath: source,
      sourceStartSeconds: 0,
      sourceEndSeconds: 4,
      timelineStartSeconds: 0,
      timelineDurationSeconds: 4,
      trackIndex: 0,
      opacity: 1,
      fadeInSeconds: 0,
      fadeOutSeconds: 0,
      transform: { centerX: 0.5, centerY: 0.5, width: 1, height: 1, flipHorizontal: false, flipVertical: false },
      crop: { top: 0, right: 0, bottom: 0, left: 0 },
    }],
    audioClips: [{
      sourcePath: source,
      sourceStartSeconds: 0,
      sourceEndSeconds: 4,
      timelineStartSeconds: 0,
      timelineDurationSeconds: 4,
      trackIndex: 0,
      volumeDb: 0,
      fadeInSeconds: 0.75,
      fadeOutSeconds: 0.75,
      ...(volumeKeyframes.length > 0 ? { volumeKeyframes } : {}),
    }],
    overlays: [],
  };
  const run = spawnSync(exporter, [], {
    cwd: repoRoot,
    input: `${JSON.stringify(request)}\n`,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  const events = (run.stdout ?? "").split(/\r?\n/).filter(Boolean).map((line) => JSON.parse(line));
  writeFileSync(resolve(outputRoot, `${id}-request.json`), `${JSON.stringify(request, null, 2)}\n`);
  writeFileSync(resolve(outputRoot, `${id}-events.json`), `${JSON.stringify(events, null, 2)}\n`);
  if (run.status !== 0 || events.at(-1)?.event !== "completed") {
    throw new Error(`${id} export failed: ${run.stderr || run.stdout}`);
  }
  return { outputPath, events };
}

function rms(path, start, duration) {
  const run = spawnSync("ffmpeg", [
    "-v", "error", "-ss", String(start), "-t", String(duration), "-i", path,
    "-map", "0:a:0", "-ac", "1", "-ar", "48000", "-f", "s16le", "pipe:1",
  ], { cwd: repoRoot, maxBuffer: 8 * 1024 * 1024 });
  if (run.status !== 0 || !Buffer.isBuffer(run.stdout) || run.stdout.length < 2) {
    throw new Error(`audio measurement failed: ${run.stderr?.toString() ?? "no PCM"}`);
  }
  let sum = 0;
  const samples = Math.floor(run.stdout.length / 2);
  for (let offset = 0; offset + 1 < run.stdout.length; offset += 2) {
    const sample = run.stdout.readInt16LE(offset) / 32768;
    sum += sample * sample;
  }
  return Number(Math.sqrt(sum / samples).toFixed(6));
}

function checked(command, args) {
  const run = spawnSync(command, args, { cwd: repoRoot, encoding: "utf8" });
  if (run.status !== 0) throw new Error(`${command} failed: ${run.stderr || run.stdout}`);
}
