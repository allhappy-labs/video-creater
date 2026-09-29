#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { parseMachORpaths, planPackagedRuntimeRpaths } from "./packaged-runtime-rpaths.mjs";

const args = process.argv.slice(2);
const value = (flag, fallback) => { const index = args.indexOf(flag); return index >= 0 ? args[index + 1] : fallback; };
const requireDeveloperId = args.includes("--require-developer-id");
const target = value("--target", "aarch64-apple-darwin");
const worker = resolve(value("--path", `src-tauri/binaries/video-creater-compatibility-decoder-${target}`));
const runtime = resolve(value("--runtime", "src-tauri/resources/compatibility-runtime"));
const manifestPath = join(runtime, "manifest.json");
if (!existsSync(worker) || !existsSync(manifestPath)) fail("compatibility worker or runtime manifest is missing");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
if (manifest.schemaVersion !== 1 || manifest.protocol !== "video-creater.compatibility" || manifest.protocolVersion !== 1 || manifest.target !== target) fail("compatibility runtime manifest contract mismatch");
if (!manifest.licenseComponents || Object.keys(manifest.licenseComponents).length < 10) fail("compatibility runtime license inventory is incomplete");
for (const notice of Object.keys(manifest.licenseComponents)) if (!existsSync(join(runtime,"licenses",notice))) fail(`license notice missing: ${notice}`);
const files = listFiles(runtime).filter((path) => path !== manifestPath);
for (const entry of manifest.files) {
  const path = join(runtime, entry.path);
  if (!existsSync(path) || statSync(path).size !== entry.bytes || sha256(path) !== entry.sha256) fail(`runtime file hash mismatch: ${entry.path}`);
}
if (files.length !== manifest.files.length) fail("runtime contains files outside its manifest");
const allMachO = [worker, ...files.filter((path) => /\.(dylib)$/.test(path))];
const staged = new Set(allMachO.map((path) => basename(path)));
for (const path of allMachO) {
  const file = checked("file", [path]);
  if (!file.includes("arm64")) fail(`non-arm64 compatibility artifact: ${path}`);
  checked("codesign", ["--verify", "--strict", path]);
  if (requireDeveloperId) verifyDeveloperIdSignature(path);
  for (const dependency of dependencies(path)) {
    if (dependency.startsWith("/System/Library/") || dependency.startsWith("/usr/lib/")) continue;
    if (/\/opt\/homebrew|\/usr\/local|libav|ffmpeg|x264|x265|fdk|faac|openh264|gstlibav/i.test(dependency)) fail(`denied dependency: ${dependency}`);
    if (!dependency.startsWith("@rpath/") || !staged.has(basename(dependency))) fail(`unstaged dependency: ${dependency}`);
  }
  const rpathPlan = planPackagedRuntimeRpaths({
    existingRpaths: parseMachORpaths(checked("otool", ["-l", path])),
    requiredRpaths: requiredRpathsFor(path),
  });
  if (rpathPlan.deleteRpaths.length > 0 || rpathPlan.addRpaths.length > 0) {
    fail(`compatibility runtime rpath mismatch: ${path}`);
  }
}
const request = JSON.stringify({ protocol:"video-creater.compatibility", schemaVersion:1, requestId:"verify", operation:{mode:"capabilities"}, budgets:{timeoutMillis:10000,maxOutputBytes:1048576} }) + "\n";
const registry = join(process.env.TMPDIR || "/tmp", `video-creater-compatibility-registry-${process.pid}.bin`);
const probe = spawnSync(worker, [], { input: request, encoding:"utf8", env:{ DYLD_LIBRARY_PATH:join(runtime,"lib"), GST_PLUGIN_PATH_1_0:join(runtime,"plugins"), GST_PLUGIN_SYSTEM_PATH_1_0:"", GST_REGISTRY_FORK:"no", GST_REGISTRY_1_0:registry } });
if (probe.status !== 0) fail(`compatibility capability probe failed: ${probe.stderr}`);
const event = JSON.parse(probe.stdout.trim());
if (event.event !== "completed" || event.result?.mode !== "capabilities") fail("compatibility capability response is invalid");
for (const plugin of manifest.plugins) if (!existsSync(join(runtime,"plugins",`libgst${plugin}.dylib`))) fail(`required plugin missing: ${plugin}`);
console.log(JSON.stringify({schemaVersion:1,status:"passed",target,worker,workerSha256:sha256(worker),runtime,files:manifest.files.length,plugins:manifest.plugins,factories:event.result.factories},null,2));

function dependencies(path) { return checked("otool",["-L",path]).split(/\r?\n/).slice(1).map((line)=>line.trim().match(/^(\S+)/)?.[1]).filter(Boolean); }
function requiredRpathsFor(path) {
  if (path === worker) {
    return [
      "@executable_path/../Resources/compatibility-runtime/lib",
      "@executable_path/../../resources/compatibility-runtime/lib",
      "@executable_path/../resources/compatibility-runtime/lib",
    ];
  }
  return relative(runtime, path).startsWith("plugins/")
    ? ["@loader_path/../lib"]
    : ["@loader_path"];
}
function verifyDeveloperIdSignature(path) {
  const details = checked("codesign", ["-dv", "--verbose=4", path]);
  if (!/^Authority=Developer ID Application:/m.test(details)) fail(`Developer ID authority missing: ${path}`);
  if (!/^TeamIdentifier=[A-Z0-9]{10}$/m.test(details)) fail(`Developer ID team identifier missing: ${path}`);
  if (!/^Timestamp=.+$/m.test(details) || /^Timestamp=(?:none|0)$/im.test(details)) fail(`secure timestamp missing: ${path}`);
}
function checked(command, commandArgs) { const result=spawnSync(command,commandArgs,{encoding:"utf8"}); if(result.status!==0) fail(`${command} failed: ${result.stderr||result.stdout}`); return result.stdout+result.stderr; }
function sha256(path) { return createHash("sha256").update(readFileSync(path)).digest("hex"); }
function listFiles(root) { return readdirSync(root,{withFileTypes:true}).flatMap((entry)=>{const path=join(root,entry.name); return entry.isDirectory()?listFiles(path):[path];}).sort(); }
function fail(message) { console.error(message); process.exit(1); }
