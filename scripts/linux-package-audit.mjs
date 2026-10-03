import { spawnSync } from "node:child_process";
import { openSync, closeSync, readSync, readdirSync, realpathSync, statSync } from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

export const DENIED_LIBRARY_PATTERNS = [
  /^libavcodec\.so/, /^libavformat\.so/, /^libavutil\.so/, /^libavfilter\.so/,
  /^libswscale\.so/, /^libswresample\.so/, /^libpostproc\.so/, /^libx264\.so/,
  /^libx265\.so/, /^libfaad\.so/, /^libmpeg2/, /^libdvdread\.so/, /^libdvdnav\.so/,
  /^libespeak/, /^libreadline\.so/,
];
export function deniedLibraries(names) {
  return names.filter((name) => DENIED_LIBRARY_PATTERNS.some((pattern) => pattern.test(basename(name))));
}

function inside(root, path) {
  const child = relative(root, path);
  return child === "" || (!child.startsWith(`..${sep}`) && child !== ".." && !isAbsolute(child));
}
function systemLibrary(path) {
  return /^\/(?:usr\/)?lib(?:64)?\//.test(path) && !path.startsWith("/usr/lib/Video Creater/");
}
function elfFiles(root, packageRoot, failures) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory()) return elfFiles(path, packageRoot, failures);
    if (entry.isSymbolicLink()) {
      let target;
      try { target = realpathSync(path); }
      catch { failures.push(`${relative(packageRoot, path)}: broken packaged symlink`); return []; }
      if (!inside(packageRoot, target)) {
        failures.push(`${relative(packageRoot, path)}: packaged symlink points outside artifact: ${target}`);
        return [];
      }
      // The target directory is traversed through its own entry; do not follow directory loops.
      if (!statSync(target).isFile()) return [];
    } else if (!entry.isFile()) return [];
    const fd = openSync(path, "r");
    const bytes = Buffer.alloc(4);
    try { readSync(fd, bytes, 0, 4, 0); } finally { closeSync(fd); }
    return bytes.equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])) ? [path] : [];
  });
}
function capture(command, args, env) {
  const result = spawnSync(command, args, { env, encoding: "utf8", timeout: 30_000, maxBuffer: 8 * 1024 * 1024 });
  if (result.error) throw new Error(`${command} could not inspect packaged ELF: ${result.error.message}`);
  return { status: result.status, output: `${result.stdout ?? ""}\n${result.stderr ?? ""}` };
}

/** Inspect only trusted binaries produced by this repository; ldd invokes the ELF loader. */
export function auditLinuxElfTree(packageRoot) {
  const root = realpathSync(packageRoot);
  const env = { ...process.env, LC_ALL: "C" };
  // Build-machine overrides can hide missing packaged dependencies or inject libraries.
  for (const key of Object.keys(env)) if (key.startsWith("LD_")) delete env[key];
  const records = [];
  const failures = [];
  for (const path of elfFiles(root, root, failures).sort()) {
    const file = relative(root, path);
    const dynamic = capture("readelf", ["-d", "--wide", path], env);
    if (dynamic.status !== 0) throw new Error(`${file}: readelf failed: ${dynamic.output.trim()}`);
    const needed = [...dynamic.output.matchAll(/\(NEEDED\)\s+Shared library: \[([^\]]+)\]/g)].map((match) => match[1]);
    const runpaths = [...dynamic.output.matchAll(/\((RUNPATH|RPATH)\)\s+Library (?:runpath|rpath): \[([^\]]*)\]/g)]
      .flatMap((match) => match[2].split(":").map((value) => ({ kind: match[1], value })));
    for (const { kind, value } of runpaths) {
      const expanded = value.replace(/\$\{ORIGIN\}|\$ORIGIN/g, dirname(path));
      const originRelative = /^\$(?:ORIGIN|\{ORIGIN\})(?:\/|$)/.test(value);
      if ((!originRelative || !inside(root, resolve(expanded))) && !systemLibrary(value)) {
        failures.push(`${file}: forbidden ${kind} ${value || "<current directory>"}`);
      }
    }
    for (const name of needed) {
      if (name.includes("/") && !(isAbsolute(name) && systemLibrary(name))) {
        failures.push(`${file}: forbidden dependency path ${name}`);
      }
    }
    const loaded = capture("ldd", [path], env);
    const unresolved = loaded.output.split("\n").filter((line) => /=>\s+not found/.test(line)).map((line) => line.trim());
    for (const missing of unresolved) failures.push(`${file}: ${missing}`);
    if (loaded.status !== 0 && !(needed.length === 0 && /statically linked|not a dynamic executable/.test(loaded.output))) {
      failures.push(`${file}: loader inspection failed: ${loaded.output.trim()}`);
    }
    const resolved = [...loaded.output.matchAll(/(?:=>\s+|^\s*)(\/\S+)\s+\(0x[0-9a-f]+\)/gm)].map((match) => match[1]);
    for (const dependency of resolved) {
      const actual = realpathSync(dependency);
      if (!inside(root, actual) && !systemLibrary(actual)) failures.push(`${file}: external dependency ${dependency}`);
    }
    for (const denied of new Set(deniedLibraries([...needed, ...resolved]))) failures.push(`${file}: denied dependency ${basename(denied)}`);
    records.push({ file, needed, runpaths, resolvedDependencies: resolved, unresolved });
  }
  return { records, failures: [...new Set(failures)].sort() };
}
