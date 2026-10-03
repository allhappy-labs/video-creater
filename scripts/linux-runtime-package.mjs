import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { pinnedSources, runtimeTarget, stagePackageTree } from "./build-linux-media-runtime.mjs";

export function validateLinuxRuntimePayload(runtimeRoot) {
  const root = resolve(runtimeRoot);
  const manifest = JSON.parse(readFileSync(join(root, "manifest.json"), "utf8"));
  if (manifest.schemaVersion !== 1 || manifest.target !== runtimeTarget || manifest.platform !== "linux") {
    throw new Error("runtime manifest target/schema is not the supported Linux runtime");
  }
  for (const component of ["ffmpeg", "gstLibav"]) {
    if (manifest[component]?.sha256 !== pinnedSources[component].sha256 || manifest[component]?.version !== pinnedSources[component].version) {
      throw new Error(`runtime ${component} source differs from the reviewed pin`);
    }
  }
  if (!Array.isArray(manifest.bundledFiles) || manifest.linux?.bundledPluginDirectory !== "bundled-plugins") {
    throw new Error("runtime manifest lacks the redistributable file inventory");
  }
  const files = new Set();
  for (const entry of manifest.bundledFiles) {
    const path = entry.path;
    if (typeof path !== "string" || isAbsolute(path) || path.split(/[\\/]/).some((part) => part === ".." || !part)
      || !/^(?:lib|bundled-plugins|licenses)\//.test(path) || files.has(path)) {
      throw new Error(`runtime manifest contains an unsafe or duplicate payload path: ${path}`);
    }
    files.add(path);
    const source = join(root, path);
    if (!lstatSync(source).isFile()) throw new Error(`runtime bundled payload must be a regular file: ${path}`);
    const bytes = readFileSync(source);
    const hash = createHash("sha256").update(bytes).digest("hex");
    if (bytes.length !== entry.bytes || hash !== entry.sha256) throw new Error(`runtime payload bytes/hash differ from manifest: ${path}`);
  }
  for (const path of ["bundled-plugins/libgstlibav.so", "licenses/NOTICE.txt", "licenses/FFmpeg-LICENSE.md", "licenses/FFmpeg-COPYING.LGPLv2.1.txt", "licenses/gst-libav-COPYING.txt"]) {
    if (!files.has(path)) throw new Error(`runtime redistributable payload is missing ${path}`);
  }
  for (const library of ["avcodec", "avfilter", "avformat", "avutil"]) {
    if (![...files].some((path) => new RegExp(`^lib/lib${library}_vc\\.so\\.\\d+$`).test(path))) {
      throw new Error(`runtime lacks bundled lib${library}_vc`);
    }
  }
  for (const directory of ["lib", "bundled-plugins", "licenses"]) {
    for (const entry of readdirSync(join(root, directory), { withFileTypes: true })) {
      const path = `${directory}/${entry.name}`;
      // The full development runtime also has a link to distribution GStreamer core.
      if (entry.isSymbolicLink() && path === "lib/libgstreamer-1.0.so.0") continue;
      if (!entry.isFile() || !files.has(path)) throw new Error(`runtime contains unclassified bundled payload: ${path}`);
    }
  }
  return manifest;
}

export function stageRemoteRenderRuntime({ runtimeRoot, output }) {
  validateLinuxRuntimePayload(runtimeRoot);
  return stagePackageTree(runtimeRoot, join(output, "lib/Video Creater/render-runtime"));
}
