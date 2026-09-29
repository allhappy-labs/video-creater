#!/usr/bin/env node
// Validates exported Premiere XMEML and DaVinci FCPXML against their DTDs.
//
// 1. Finds xmllint (PATH, then <tools>/libxml2-utils, then extracts the libxml2-utils .deb there).
// 2. Downloads the pinned FCPXML 1.10 and XMEML v5 DTDs into <tools>/nle-dtd and checks their SHA-256.
//    The DTDs are Apple-copyrighted: they stay in the tools folder and are never committed.
// 3. Runs the ignored `validation::nle_exports_validate_against_the_dtds` test, which exports the corpus
//    and runs `xmllint --dtdvalid` on every file.
// 4. Writes <output>/report.json from the test's results.json.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { PINNED_DTDS, extractXmemlV5Dtd, sha256Hex, summarizeValidationResults } from "./nle-xml-validation-sources.mjs";

const USAGE = `Usage: node scripts/nle-xml-validation.mjs --output <dir> [--tools-dir <dir>]

  --output <dir>     Where corpus files, results.json and report.json are written (required).
  --tools-dir <dir>  Where xmllint and the DTDs are kept (default /tmp/vc-smoke-tools).
  --help             Show this help.`;

const DTD_NOTE = "Apple copyright; downloaded for validation, not committed";
const CARGO_ARGS = [
  "test",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--test",
  "project_nle_export",
  "validation::nle_exports_validate_against_the_dtds",
  "--",
  "--ignored",
  "--exact",
  "--test-threads=1",
];

class NotRun extends Error {}

function parseArguments(argv) {
  const options = { toolsDir: "/tmp/vc-smoke-tools", output: undefined, help: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--") continue;
    if (argument === "--help") {
      options.help = true;
    } else if (argument === "--tools-dir" || argument === "--output") {
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) throw new Error(`${argument} needs a value`);
      options[argument === "--output" ? "output" : "toolsDir"] = value;
      index += 1;
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  return options;
}

function run(command, args, options = {}) {
  return spawnSync(command, args, { encoding: "utf8", shell: false, ...options });
}

function firstLine(text) {
  return (text ?? "").split("\n").find((line) => line.trim())?.trim() ?? "";
}

function xmllintVersion(path) {
  const result = run(path, ["--version"]);
  return result.status === 0 ? firstLine(result.stderr || result.stdout) : undefined;
}

function resolveXmllint(toolsDir) {
  const onPath = xmllintVersion("xmllint");
  if (onPath) {
    const which = run("which", ["xmllint"]);
    return { path: firstLine(which.stdout) || "xmllint", version: onPath, source: "PATH" };
  }
  const extractRoot = join(toolsDir, "libxml2-utils");
  const extracted = join(extractRoot, "usr/bin/xmllint");
  const debsDir = join(toolsDir, "debs");
  const findDeb = () =>
    existsSync(debsDir) ? readdirSync(debsDir).filter((name) => /^libxml2-utils_.*\.deb$/.test(name)).sort().at(-1) : undefined;
  if (!existsSync(extracted)) {
    mkdirSync(debsDir, { recursive: true });
    if (!findDeb()) {
      const download = run("apt-get", ["download", "libxml2-utils"], { cwd: debsDir });
      if (download.status !== 0) {
        throw new NotRun(`apt-get download libxml2-utils failed: ${firstLine(download.stderr) || download.error?.message}`);
      }
    }
    const deb = findDeb();
    if (!deb) throw new NotRun("apt-get download libxml2-utils produced no .deb");
    const extract = run("dpkg-deb", ["-x", join(debsDir, deb), extractRoot]);
    if (extract.status !== 0) {
      throw new NotRun(`dpkg-deb -x ${deb} failed: ${firstLine(extract.stderr) || extract.error?.message}`);
    }
  }
  const version = xmllintVersion(extracted);
  if (!version) throw new NotRun(`${extracted} does not run (xmllint --version failed)`);
  const deb = findDeb();
  return {
    path: extracted,
    version,
    source: "libxml2-utils .deb",
    deb: deb ?? null,
    debSha256: deb ? sha256Hex(readFileSync(join(debsDir, deb))) : null,
  };
}

async function fetchText(url) {
  let response;
  try {
    response = await fetch(url);
  } catch (error) {
    throw new NotRun(`download of ${url} failed: ${error.message}`);
  }
  if (!response.ok) throw new NotRun(`download of ${url} failed: HTTP ${response.status}`);
  return response.text();
}

async function ensureDtd(dtdDir, pin, contentFrom) {
  const path = join(dtdDir, pin.fileName);
  if (existsSync(path) && sha256Hex(readFileSync(path)) === pin.sha256) {
    return { path, url: pin.url, sha256: pin.sha256, note: DTD_NOTE };
  }
  const text = contentFrom(await fetchText(pin.url));
  const actual = sha256Hex(text);
  if (actual !== pin.sha256) {
    throw new NotRun(`${pin.fileName} SHA-256 mismatch: expected ${pin.sha256}, got ${actual}`);
  }
  writeFileSync(path, text);
  return { path, url: pin.url, sha256: actual, note: DTD_NOTE };
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) {
    console.log(USAGE);
    return 0;
  }
  if (!options.output) throw new Error(`--output is required\n\n${USAGE}`);
  const toolsDir = resolve(options.toolsDir);
  const output = resolve(options.output);
  mkdirSync(output, { recursive: true });
  const dtdDir = join(toolsDir, "nle-dtd");
  mkdirSync(dtdDir, { recursive: true });

  const xmllint = resolveXmllint(toolsDir);
  console.log(`xmllint: ${xmllint.path} (${xmllint.version}, ${xmllint.source})`);
  const dtds = {
    fcpxml: await ensureDtd(dtdDir, PINNED_DTDS.fcpxml, (text) => text),
    xmeml: await ensureDtd(dtdDir, PINNED_DTDS.xmeml, extractXmemlV5Dtd),
  };
  console.log(`DTDs verified in ${dtdDir}`);

  const resultsPath = join(output, "results.json");
  // A results.json left by an earlier run in this folder must never be reported as this run's.
  rmSync(resultsPath, { force: true });
  rmSync(join(output, "report.json"), { force: true });
  const cargo = run("cargo", CARGO_ARGS, {
    stdio: "inherit",
    env: {
      ...process.env,
      TAURI_CONFIG: '{"bundle":{"externalBin":[],"resources":[]}}',
      VIDEO_CREATER_XMLLINT: xmllint.path,
      VIDEO_CREATER_NLE_DTD_DIR: dtdDir,
      VIDEO_CREATER_NLE_VALIDATION_OUTPUT: output,
    },
  });
  if (!existsSync(resultsPath)) {
    throw new NotRun(`cargo exited ${cargo.status ?? cargo.error?.message} without writing ${resultsPath}`);
  }
  const summary = summarizeValidationResults(JSON.parse(readFileSync(resultsPath, "utf8")));
  const report = {
    generatedAt: new Date().toISOString(),
    xmllint,
    dtds,
    cargo: { argv: ["cargo", ...CARGO_ARGS], exitCode: cargo.status },
    summary,
  };
  writeFileSync(join(output, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.log(`NLE XML validation: ${summary.passed} passed, ${summary.failed} failed (report: ${join(output, "report.json")})`);
  for (const failure of summary.failures) console.log(`- ${failure.file}\n${failure.stderr}`);
  return cargo.status === 0 && summary.failed === 0 ? 0 : 1;
}

main().then(
  (code) => process.exit(code),
  (error) => {
    if (error instanceof NotRun) {
      console.error(`NLE XML validation not run: ${error.message}`);
    } else {
      console.error(error.message);
    }
    process.exit(1);
  },
);
