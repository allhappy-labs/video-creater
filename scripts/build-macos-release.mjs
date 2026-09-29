#!/usr/bin/env node
import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

import {
  auditPackagedMachO,
  assertMatchingDeveloperIdSignature,
  parseCodeSignatureDetails,
} from "./rewrite-gstreamer-rpaths.mjs";
import { notarizeAndVerifyDmg } from "./macos-notarization.mjs";
import { verifyGstreamerRuntime } from "./verify-gstreamer-runtime.mjs";
import {
  configuredReleaseExecutables,
  evaluatePackagedRuntimeRecords,
} from "./release-runtime-policy.mjs";
import {
  assertReleaseSourceStable,
  collectReleaseSourceEvidence,
} from "./release-source-evidence.mjs";
import { runPackagedSettingsAcceptance } from "./settings-packaged-acceptance.mjs";

const repoRoot = resolve(import.meta.dirname, "..");
const keychainService = "video-creater-notary";
const cargoTargetDir = resolve(
  repoRoot,
  process.env.CARGO_TARGET_DIR?.trim() || "src-tauri/target",
);
const configuredReleaseEvidenceDir =
  process.env.VIDEO_CREATER_RELEASE_EVIDENCE_DIR?.trim();
const prepareMcpSidecarOnly = process.argv.slice(2).includes("--prepare-mcp-sidecar");
const developmentMcpSidecar = process.argv.slice(2).includes("--development");
const preflightOnly = process.argv.slice(2).includes("--preflight");
const unknownArgs = process.argv.slice(2).filter(
  (value) => !["--preflight", "--prepare-mcp-sidecar", "--development"].includes(value),
);

if (unknownArgs.length > 0) {
  fail(`unknown argument: ${unknownArgs[0]}`);
}
if (developmentMcpSidecar && !prepareMcpSidecarOnly) {
  fail("--development requires --prepare-mcp-sidecar");
}
if (prepareMcpSidecarOnly) {
  prepareMcpSidecar({ development: developmentMcpSidecar });
  process.exit(0);
}
if (process.platform !== "darwin") {
  fail("macOS release signing must run on macOS");
}

const source = collectReleaseSourceEvidence({ repoRoot });
const releaseEvidenceDir = resolve(
  repoRoot,
  configuredReleaseEvidenceDir ||
    `output/settings-readiness/final-gate/release-${source.shortCommit}`,
);

const identity = resolveDeveloperIdIdentity(process.env.APPLE_SIGNING_IDENTITY);
const teamId = resolveTeamId(process.env.APPLE_TEAM_ID, identity);
const appleId = resolveAppleId(process.env.APPLE_ID);
const password = resolveNotarizationPassword(appleId);
verifyNotarizationAuthentication(appleId, password, teamId);

const preflight = {
  schemaVersion: 1,
  status: "passed",
  mode: preflightOnly ? "preflight" : "release",
  signingIdentity: identity,
  teamId,
  notarizationAuth: process.env.APPLE_PASSWORD?.trim()
    ? "environment-app-specific-password"
    : `keychain:${keychainService}`,
  notarizationAuthentication: "verified",
};

if (preflightOnly) {
  console.log(JSON.stringify(preflight, null, 2));
  process.exit(0);
}

const releaseEnv = {
  ...process.env,
  CARGO_TARGET_DIR: cargoTargetDir,
  APPLE_ID: appleId,
  APPLE_TEAM_ID: teamId,
  APPLE_PASSWORD: password,
  APPLE_SIGNING_IDENTITY: identity,
};
runOrFail(
  "pnpm",
  [
    "tauri",
    "build",
    "--ci",
    "--bundles",
    "app,dmg",
    "--",
    "--no-default-features",
    "--features",
    "app-runtime,custom-protocol,coreml-inspect,ges-render,gpu-render,graphics-render",
  ],
  { env: releaseEnv, stdio: "inherit" },
);
const sourceAfterBuild = collectReleaseSourceEvidence({ repoRoot });
assertReleaseSourceStable(source, sourceAfterBuild);

const appPath = resolve(
  cargoTargetDir,
  "release/bundle/macos/Video Creater.app",
);
const dmgPath = latestDmg(resolve(cargoTargetDir, "release/bundle/dmg"));
const evidenceDir = releaseEvidenceDir;
mkdirSync(evidenceDir, { recursive: true });

const renderRuntimeRoot = join(appPath, "Contents/Resources/render-runtime");
const renderRuntimeManifestPath = join(renderRuntimeRoot, "manifest.json");
const codesignVerification = checked("codesign", ["--verify", "--deep", "--strict", "--verbose=2", appPath]);
const codesignDetails = checked("codesign", ["-dv", "--verbose=4", appPath]);
const appSignature = parseCodeSignatureDetails(codesignDetails.output);
assertMatchingDeveloperIdSignature({
  path: appPath,
  signature: appSignature,
  expectedAuthority: identity,
  expectedTeamId: teamId,
});
const mcpServerPath = join(appPath, "Contents/MacOS/video-creater-mcp-server");
const mcpServerCodesign = checked("codesign", [
  "--verify",
  "--strict",
  "--verbose=2",
  mcpServerPath,
]);
const mcpServerSignatureDetails = checked("codesign", ["-dv", "--verbose=4", mcpServerPath]);
const mcpServerSignature = parseCodeSignatureDetails(mcpServerSignatureDetails.output);
assertMatchingDeveloperIdSignature({
  path: mcpServerPath,
  signature: mcpServerSignature,
  expectedAuthority: identity,
  expectedTeamId: teamId,
});
const codexPath = join(appPath, "Contents/MacOS/video-creater-codex");
const codexCodesign = checked("codesign", [
  "--verify",
  "--strict",
  "--verbose=2",
  codexPath,
]);
const codexSignatureDetails = checked("codesign", ["-dv", "--verbose=4", codexPath]);
const codexSignature = parseCodeSignatureDetails(codexSignatureDetails.output);
assertMatchingDeveloperIdSignature({
  path: codexPath,
  signature: codexSignature,
  expectedAuthority: identity,
  expectedTeamId: teamId,
});
const gatekeeper = checked("spctl", ["--assess", "--type", "execute", "--verbose=4", appPath]);
const appStaple = checked("xcrun", ["stapler", "validate", appPath]);
const dmgNotarization = notarizeAndVerifyDmg({
  appleId,
  dmgPath,
  password,
  teamId,
});
const packagedMachOFiles = auditPackagedMachO({
  appPath,
  expectedAuthority: appSignature.authority,
  expectedTeamId: appSignature.teamId,
});
const releaseRuntimeFailures = evaluatePackagedRuntimeRecords({
  records: packagedMachOFiles,
  requiredExecutables: configuredReleaseExecutables({ repoRoot }),
});
if (releaseRuntimeFailures.length > 0) {
  fail(`packaged runtime policy failed:\n${releaseRuntimeFailures.join("\n")}`);
}
const runtimeManifestSha256 = sha256(renderRuntimeManifestPath);
const factoryProbe = verifyGstreamerRuntime({
  runtimeRoot: renderRuntimeRoot,
  target: "aarch64-apple-darwin",
});
const nestedSignedMachOFiles = packagedMachOFiles.filter((entry) =>
  entry.path.startsWith("Contents/Resources/render-runtime/"),
);

runOrFail("node", [
  "scripts/verify-compatibility-runtime.mjs",
  "--target",
  "aarch64-apple-darwin",
  "--path",
  join(appPath, "Contents/MacOS/video-creater-compatibility-decoder"),
  "--runtime",
  join(appPath, "Contents/Resources/compatibility-runtime"),
  "--require-developer-id",
]);

runOrFail("node", [
  "scripts/verify-avfoundation-exporter.mjs",
  "--target",
  "aarch64-apple-darwin",
  "--path",
  join(appPath, "Contents/MacOS/video-creater-avfoundation-exporter"),
  "--require-signature",
  "--report",
  join(evidenceDir, "avfoundation-exporter.json"),
]);
runOrFail("node", [
  "scripts/verify-precompose-sidecar.mjs",
  "--target",
  "aarch64-apple-darwin",
  "--path",
  join(appPath, "Contents/MacOS/video-creater-precompose-worker"),
  "--require-signature",
  "--report",
  join(evidenceDir, "precompose-worker.json"),
]);
checked("codesign", [
  "--verify",
  "--strict",
  join(appPath, "Contents/MacOS/video-creater-semantic-encoder"),
]);
checked("codesign", [
  "--verify",
  "--strict",
  join(appPath, "Contents/MacOS/video-creater-fluidaudio-transcribe"),
]);
checked("codesign", [
  "--verify",
  "--strict",
  join(appPath, "Contents/MacOS/video-creater-audio-enhance"),
]);

const packagedSettingsAcceptance = await runPackagedSettingsAcceptance({
  appPath,
  evidenceDir,
  source,
});
const sourceAfterAcceptance = collectReleaseSourceEvidence({ repoRoot });
assertReleaseSourceStable(source, sourceAfterAcceptance);

const report = {
  ...preflight,
  source,
  app: artifactEvidence(appPath),
  dmg: artifactEvidence(dmgPath),
  verification: {
    codesign: codesignVerification.output.trim(),
    gatekeeper: gatekeeper.output.trim(),
    appStaple: appStaple.output.trim(),
    dmgNotarization,
    dmgStaple: dmgNotarization.stapleValidation,
    dmgGatekeeper: dmgNotarization.gatekeeper,
    packageManagerIndependentMachO: true,
    packagedMachOFiles,
    renderRuntime: {
      runtimeManifestSha256,
      factoryProbe,
      nestedSignedMachOFiles,
    },
    mcpServer: {
      ...artifactEvidence(mcpServerPath),
      codesign: mcpServerCodesign.output.trim(),
      signature: mcpServerSignature,
    },
    codex: {
      ...artifactEvidence(codexPath),
      version: "0.141.0",
      codesign: codexCodesign.output.trim(),
      signature: codexSignature,
    },
    packagedSettingsAcceptance,
    packagedHelpers: [
      "video-creater-avfoundation-exporter",
      "video-creater-precompose-worker",
      "video-creater-semantic-encoder",
      "video-creater-fluidaudio-transcribe",
      "video-creater-audio-enhance",
      "video-creater-mcp-server",
      "video-creater-codex",
    ],
  },
};
const reportPath = join(evidenceDir, "report.json");
writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({ status: "passed", reportPath, appPath, dmgPath }, null, 2));

function prepareMcpSidecar({ development }) {
  const rustc = checked("rustc", ["-vV"]);
  const host = rustc.output.match(/^host:\s*(.+)$/m)?.[1];
  const target = process.env.TAURI_ENV_TARGET_TRIPLE?.trim() || host;
  if (!target || target === "universal-apple-darwin") {
    fail(`unsupported MCP sidecar target: ${target || "<missing>"}`);
  }
  const windows = target.includes("windows");
  const executable = `video-creater-mcp-server${windows ? ".exe" : ""}`;
  const profile = development ? "debug" : "release";
  const cargoArgs = [
    "build",
    "--locked",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--bin",
    "video-creater-mcp-server",
    "--target",
    target,
    "--no-default-features",
    "--features",
    "mcp-server",
  ];
  if (!development) cargoArgs.push("--release");
  if (process.env.VIDEO_CREATER_OFFLINE_BUILD === "1") cargoArgs.push("--offline");
  runOrFail("cargo", cargoArgs, {
    env: { ...process.env, CARGO_TARGET_DIR: cargoTargetDir },
    stdio: "inherit",
  });
  const built = join(cargoTargetDir, target, profile, executable);
  if (!existsSync(built)) fail(`built MCP sidecar is missing: ${built}`);
  const bundled = resolve(
    repoRoot,
    "src-tauri/binaries",
    `video-creater-mcp-server-${target}${windows ? ".exe" : ""}`,
  );
  mkdirSync(dirname(bundled), { recursive: true });
  copyFileSync(built, bundled);
  chmodSync(bundled, 0o755);
  console.log(JSON.stringify({ status: "prepared", target, profile, bundled }, null, 2));
}

function resolveDeveloperIdIdentity(configuredIdentity) {
  const result = run("security", ["find-identity", "-v", "-p", "codesigning"]);
  if (result.status !== 0) fail("could not inspect macOS signing identities");
  const identities = [...result.output.matchAll(/"(Developer ID Application:[^"]+)"/g)]
    .map((match) => match[1]);
  const uniqueIdentities = [...new Set(identities)];
  if (configuredIdentity?.trim()) {
    const identity = configuredIdentity.trim();
    if (!identity.startsWith("Developer ID Application:")) {
      fail("APPLE_SIGNING_IDENTITY must be a Developer ID Application identity");
    }
    if (!uniqueIdentities.includes(identity)) {
      fail("APPLE_SIGNING_IDENTITY is not installed as a valid codesigning identity");
    }
    return identity;
  }
  if (uniqueIdentities.length === 0) {
    fail("no valid Developer ID Application identity is installed");
  }
  if (uniqueIdentities.length > 1) {
    fail("multiple Developer ID Application identities are installed; set APPLE_SIGNING_IDENTITY");
  }
  return uniqueIdentities[0];
}

function resolveTeamId(configuredTeamId, identity) {
  const configured = configuredTeamId?.trim();
  if (configured) return configured;
  const match = identity.match(/\(([A-Z0-9]{10})\)$/);
  if (!match) {
    fail("APPLE_TEAM_ID is not set and could not be derived from the signing identity");
  }
  return match[1];
}

function resolveAppleId(configuredAppleId) {
  const configured = configuredAppleId?.trim();
  if (configured) return configured;
  const keychain = run("security", ["find-generic-password", "-s", keychainService]);
  const account = keychain.output.match(/"acct"<blob>="([^"]+)"/)?.[1]?.trim();
  if (keychain.status !== 0 || !account) {
    fail(`APPLE_ID is not set and Keychain item ${keychainService} has no readable account`);
  }
  return account;
}

function resolveNotarizationPassword(appleId) {
  const configured = process.env.APPLE_PASSWORD?.trim();
  if (configured) return configured;
  const keychain = run("security", [
    "find-generic-password",
    "-a",
    appleId,
    "-s",
    keychainService,
    "-w",
  ]);
  const password = keychain.output.trim();
  if (keychain.status !== 0 || !password) {
    fail(
      `APPLE_PASSWORD is not set and Keychain item ${keychainService} was not found for APPLE_ID`,
    );
  }
  return password;
}

function verifyNotarizationAuthentication(appleId, password, teamId) {
  const result = spawnSync(
    "xcrun",
    [
      "notarytool",
      "history",
      "--apple-id",
      appleId,
      "--password",
      password,
      "--team-id",
      teamId,
      "--output-format",
      "json",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: "pipe",
      timeout: 30_000,
    },
  );
  if (result.error || result.status !== 0) {
    fail(
      "Apple notarization authentication failed; unlock the Apple ID and replace the app-specific password in Keychain before building",
    );
  }
}

function latestDmg(directory) {
  if (!existsSync(directory)) fail(`release DMG directory does not exist: ${directory}`);
  const candidates = readdirSync(directory)
    .filter((name) => name.endsWith(".dmg"))
    .map((name) => join(directory, name))
    .sort((left, right) => statSync(right).mtimeMs - statSync(left).mtimeMs);
  if (!candidates[0]) fail(`release DMG was not produced in ${directory}`);
  return candidates[0];
}

function artifactEvidence(path) {
  const artifactStat = statSync(path);
  const hashedPath = artifactStat.isDirectory()
    ? join(path, "Contents/MacOS/video-creater")
    : path;
  const bytes = statSync(hashedPath).size;
  return {
    path,
    fileName: basename(path),
    hashedPath,
    bytes,
    sha256: sha256(hashedPath),
  };
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function checked(command, args) {
  const result = run(command, args);
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed: ${result.output.trim()}`);
  }
  return result;
}

function runOrFail(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.stdio ?? "inherit",
    env: options.env ?? process.env,
  });
  if (result.error || result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed`);
  }
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
  });
  return {
    status: result.status,
    output: `${result.stdout || ""}${result.stderr || ""}`,
  };
}

function fail(message) {
  console.error(`macOS release: ${message}`);
  process.exit(1);
}
