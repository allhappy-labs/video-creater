import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

import { validateRuntimeManifest } from "./gstreamer-runtime-policy.mjs";

const repoRoot = resolve(import.meta.dirname, "..");

test("macOS release requires the complete production render feature set", () => {
  const source = readFileSync(
    resolve(repoRoot, "scripts/build-macos-release.mjs"),
    "utf8",
  );
  const featureArgument = source.match(
    /"--features",\s*"([^"]+)"/,
  )?.[1];

  assert.ok(featureArgument, "release command must pass an explicit feature list");
  assert.deepEqual(featureArgument.split(","), [
    "app-runtime",
    "custom-protocol",
    "coreml-inspect",
    "ges-render",
    "gpu-render",
    "graphics-render",
  ]);
});

test("custom protocol is release-only and does not disable the dev URL", () => {
  const manifest = readFileSync(
    resolve(repoRoot, "src-tauri/Cargo.toml"),
    "utf8",
  );
  const defaultFeatures = manifest.match(/^default\s*=\s*(\[[^\]]*\])$/m)?.[1];
  const appRuntime = manifest.match(/^app-runtime\s*=\s*(\[[^\]]*\])$/m)?.[1];

  assert.ok(defaultFeatures, "default feature set must be declared");
  assert.doesNotMatch(
    defaultFeatures,
    /custom-protocol/,
    "default builds must retain Tauri devUrl and hot reload",
  );
  assert.ok(appRuntime, "app-runtime feature must be declared");
  assert.doesNotMatch(
    appRuntime,
    /tauri\/custom-protocol/,
    "default app-runtime must retain Tauri devUrl and hot reload",
  );
  assert.match(
    manifest,
    /^custom-protocol\s*=\s*\["tauri\/custom-protocol"\]$/m,
    "release builds need a dedicated feature that embeds the frontend",
  );
});

test("Tauri dev starts the web server immediately after explicit runtime preparation", () => {
  const config = JSON.parse(
    readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const packageJson = JSON.parse(
    readFileSync(resolve(repoRoot, "package.json"), "utf8"),
  );

  assert.equal(
    config.build.beforeDevCommand,
    "pnpm dev:web-runtime",
    "cold helper builds must finish before Tauri starts its bounded devUrl poll",
  );
  assert.match(
    packageJson.scripts["prepare:tauri:dev"],
    /^pnpm build:gstreamer-runtime:dev && /,
  );
  assert.equal(packageJson.scripts["tauri:dev"], "node scripts/tauri-dev.mjs");
});

test("Tauri release stages and bundles the curated render runtime before compilation", () => {
  const config = JSON.parse(
    readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
  );

  assert.match(
    config.build.beforeBuildCommand,
    /^pnpm build:gstreamer-runtime && /,
  );
  assert.equal(
    config.build.beforeBundleCommand,
    "pnpm prepare:gstreamer-release-bundle",
  );
  assert.deepEqual(config.bundle.resources, {
    "resources/render-runtime/": "render-runtime/",
    "resources/compatibility-runtime/": "compatibility-runtime/",
    "vendor/dotlottie-rs/LICENSES/Apache-2.0.txt":
      "codex-runtime/LICENSE-APACHE-2.0.txt",
    "resources/codex-runtime/provenance.json":
      "codex-runtime/provenance.json",
    "resources/sample-project/": "sample-project/",
  });
});

test("package scripts expose the release-runtime policy and pre-sign hook", () => {
  const packageJson = JSON.parse(
    readFileSync(resolve(repoRoot, "package.json"), "utf8"),
  );

  assert.equal(
    packageJson.scripts["test:gstreamer-release-policy"],
    "node --test scripts/gstreamer-release-policy.test.ts",
  );
  assert.equal(
    packageJson.scripts["prepare:gstreamer-release-bundle"],
    "node scripts/rewrite-gstreamer-rpaths.mjs",
  );
});

test("pre-sign relocation maps curated libraries through the packaged runtime", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs").catch(
    () => null,
  );
  assert.ok(relocation, "the pre-sign relocation module must exist");

  const plan = relocation.planExecutableRelocation({
    dependencies: [
      "/usr/lib/libSystem.B.dylib",
      "/opt/homebrew/Cellar/gstreamer/1.28.2/lib/libgstreamer-1.0.0.dylib",
      "@rpath/libgstbase-1.0.0.dylib",
    ],
    rpaths: [],
    runtimeLibraryBasenames: new Set([
      "libgstreamer-1.0.0.dylib",
      "libgstbase-1.0.0.dylib",
    ]),
  });

  assert.deepEqual(plan, {
    dependencyChanges: [
      {
        from: "/opt/homebrew/Cellar/gstreamer/1.28.2/lib/libgstreamer-1.0.0.dylib",
        to: "@rpath/libgstreamer-1.0.0.dylib",
      },
    ],
    deleteRpaths: [],
    addRpaths: [
      "@executable_path/../Resources/render-runtime/lib",
    ],
  });
});

test("pre-sign relocation removes package-manager rpaths behind @rpath dependencies", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  const plan = relocation.planExecutableRelocation({
    dependencies: [
      "/usr/lib/libSystem.B.dylib",
      "@rpath/libgstreamer-1.0.0.dylib",
    ],
    rpaths: [
      "/usr/local/lib",
      "/opt/homebrew/Cellar/gstreamer/1.28.2/lib",
      "@loader_path/../Frameworks",
    ],
    runtimeLibraryBasenames: new Set(["libgstreamer-1.0.0.dylib"]),
  });

  assert.deepEqual(plan, {
    dependencyChanges: [],
    deleteRpaths: [
      "/opt/homebrew/Cellar/gstreamer/1.28.2/lib",
      "/usr/local/lib",
    ],
    addRpaths: [
      "@executable_path/../Resources/render-runtime/lib",
    ],
  });
});

test("pre-sign relocation preserves benign rpaths", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const packagedRpath =
    "@executable_path/../Resources/render-runtime/lib";

  const plan = relocation.planExecutableRelocation({
    dependencies: ["@rpath/libgstreamer-1.0.0.dylib"],
    rpaths: [
      "/usr/lib",
      "/System/Library/Frameworks",
      "@loader_path/../Frameworks",
      "@executable_path/../Frameworks",
      packagedRpath,
    ],
    runtimeLibraryBasenames: new Set(["libgstreamer-1.0.0.dylib"]),
  });

  assert.deepEqual(plan, {
    dependencyChanges: [],
    deleteRpaths: [],
    addRpaths: [],
  });
});

test("compatibility helpers retain their dedicated packaged runtime rpath", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const compatibilityRpath =
    "@executable_path/../Resources/compatibility-runtime/lib";

  const plan = relocation.planExecutableRelocation({
    dependencies: [
      "/opt/homebrew/opt/gstreamer/lib/libgstreamer-1.0.0.dylib",
    ],
    rpaths: ["/opt/homebrew/lib"],
    runtimeLibraryBasenames: new Set(["libgstreamer-1.0.0.dylib"]),
    runtimeRpath: compatibilityRpath,
  });

  assert.deepEqual(plan, {
    dependencyChanges: [
      {
        from: "/opt/homebrew/opt/gstreamer/lib/libgstreamer-1.0.0.dylib",
        to: "@rpath/libgstreamer-1.0.0.dylib",
      },
    ],
    deleteRpaths: ["/opt/homebrew/lib"],
    addRpaths: [compatibilityRpath],
  });
});

test("packaged runtimes remove inherited package-manager rpaths", async () => {
  const policy = await import("./packaged-runtime-rpaths.mjs");

  assert.deepEqual(
    policy.planPackagedRuntimeRpaths({
      existingRpaths: [
        "/Volumes/Build Host/Homebrew/Cellar/jpeg-turbo/3.1.4.1/lib",
        "/opt/local/lib",
        "/nix/store/fixture/lib",
        "/private/tmp/build output/lib",
        "@loader_path",
        "@loader_path",
      ],
      requiredRpaths: ["@loader_path"],
    }),
    {
      deleteRpaths: [
        "/Volumes/Build Host/Homebrew/Cellar/jpeg-turbo/3.1.4.1/lib",
        "/opt/local/lib",
        "/nix/store/fixture/lib",
        "/private/tmp/build output/lib",
        "@loader_path",
      ],
      addRpaths: [],
    },
  );
  assert.throws(
    () =>
      policy.planPackagedRuntimeRpaths({
        existingRpaths: [],
        requiredRpaths: ["/Volumes/Build Host/runtime/lib"],
      }),
    /unsafe required packaged runtime rpath/i,
  );
});

test("LC_RPATH parser preserves spaces and fails closed on malformed commands", async () => {
  const policy = await import("./packaged-runtime-rpaths.mjs");
  const output = [
    "Load command 4",
    "          cmd LC_RPATH",
    "      cmdsize 88",
    "         path /Volumes/Build Host/Homebrew/Cellar/jpeg turbo/lib (offset 12)",
    "Load command 5",
    "          cmd LC_RPATH",
    "      cmdsize 48",
    "         path @loader_path/../lib (offset 12)",
  ].join("\n");

  assert.deepEqual(policy.parseMachORpaths(output), [
    "/Volumes/Build Host/Homebrew/Cellar/jpeg turbo/lib",
    "@loader_path/../lib",
  ]);
  assert.throws(
    () =>
      policy.parseMachORpaths([
        "Load command 4",
        "          cmd LC_RPATH",
        "      cmdsize 48",
        "         path missing-offset",
      ].join("\n")),
    /malformed LC_RPATH/i,
  );
});

test("runtime rpath reconciliation re-reads mutations and is idempotent", async () => {
  const policy = await import("./packaged-runtime-rpaths.mjs");
  const path = "/tmp/fixture helper";
  const required = "@loader_path/../lib";
  let rpaths = [
    "/Volumes/Build Host/Homebrew/Cellar/jpeg turbo/lib",
    required,
    required,
  ];
  const mutations = [];
  const run = (command, args) => {
    if (command === "otool") {
      return {
        stdout: rpaths.flatMap((rpath, index) => [
          `Load command ${index}`,
          "          cmd LC_RPATH",
          "      cmdsize 88",
          `         path ${rpath} (offset 12)`,
        ]).join("\n"),
        stderr: "",
      };
    }
    assert.equal(command, "install_name_tool");
    mutations.push([...args]);
    if (args[0] === "-delete_rpath") {
      const index = rpaths.indexOf(args[1]);
      assert.notEqual(index, -1);
      rpaths.splice(index, 1);
    } else {
      assert.equal(args[0], "-add_rpath");
      rpaths.push(args[1]);
    }
    return { stdout: "", stderr: "" };
  };

  const first = policy.reconcilePackagedRuntimeRpaths({
    path,
    requiredRpaths: [required],
    run,
  });
  assert.deepEqual(first.finalRpaths, [required]);
  assert.deepEqual(rpaths, [required]);
  const firstMutationCount = mutations.length;

  const second = policy.reconcilePackagedRuntimeRpaths({
    path,
    requiredRpaths: [required],
    run,
  });
  assert.deepEqual(second.initialPlan, { deleteRpaths: [], addRpaths: [] });
  assert.equal(mutations.length, firstMutationCount);

  assert.throws(
    () =>
      policy.reconcilePackagedRuntimeRpaths({
        path,
        requiredRpaths: [],
        run(command) {
          return command === "otool"
            ? {
                stdout: [
                  "Load command 0",
                  "          cmd LC_RPATH",
                  "      cmdsize 88",
                  "         path /Volumes/Build Host/runtime/lib (offset 12)",
                ].join("\n"),
                stderr: "",
              }
            : { stdout: "", stderr: "" };
        },
      }),
    /did not converge/i,
  );
});

test("relocation deletes forbidden rpaths before adding the packaged runtime", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const commands = [];
  let dependencies = [
    "/opt/homebrew/lib/libgstreamer-1.0.0.dylib",
  ];
  let rpaths = ["/usr/local/lib"];
  const command = (name, args) => {
    commands.push([name, ...args]);
    if (name === "otool" && args[0] === "-L") {
      return {
        stdout: [
          `${args[1]}:`,
          ...dependencies.map((dependency) => `\t${dependency} (compatibility version 0.0.0, current version 0.0.0)`),
        ].join("\n"),
        stderr: "",
      };
    }
    if (name === "otool" && args[0] === "-l") {
      return {
        stdout: rpaths
          .map((rpath) => [
            "Load command 1",
            "          cmd LC_RPATH",
            "      cmdsize 48",
            `         path ${rpath} (offset 12)`,
          ].join("\n"))
          .join("\n"),
        stderr: "",
      };
    }
    if (name === "install_name_tool" && args[0] === "-change") {
      dependencies = dependencies.map((dependency) =>
        dependency === args[1] ? args[2] : dependency
      );
      return { stdout: "", stderr: "" };
    }
    if (name === "install_name_tool" && args[0] === "-delete_rpath") {
      rpaths = rpaths.filter((rpath) => rpath !== args[1]);
      return { stdout: "", stderr: "" };
    }
    if (name === "install_name_tool" && args[0] === "-add_rpath") {
      rpaths.push(args[1]);
      return { stdout: "", stderr: "" };
    }
    throw new Error(`unexpected command: ${name} ${args.join(" ")}`);
  };

  relocation.relocateExecutable({
    binaryPath: "/tmp/video-creater-render-proposal",
    manifest: {
      files: [
        {
          path: "lib/libgstreamer-1.0.0.dylib",
          machODependencies: [],
        },
      ],
    },
    command,
  });

  assert.deepEqual(
    commands.filter(([name]) => name === "install_name_tool"),
    [
      [
        "install_name_tool",
        "-change",
        "/opt/homebrew/lib/libgstreamer-1.0.0.dylib",
        "@rpath/libgstreamer-1.0.0.dylib",
        "/tmp/video-creater-render-proposal",
      ],
      [
        "install_name_tool",
        "-delete_rpath",
        "/usr/local/lib",
        "/tmp/video-creater-render-proposal",
      ],
      [
        "install_name_tool",
        "-add_rpath",
        "@executable_path/../Resources/render-runtime/lib",
        "/tmp/video-creater-render-proposal",
      ],
    ],
  );
});

test("pre-bundle discovery covers Cargo bins and configured external helpers", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const existing = new Set([
    "/target/release/video-creater",
    "/target/release/video-creater-render-proposal",
    "/repo/src-tauri/binaries/video-creater-compatibility-decoder-aarch64-apple-darwin",
    "/repo/src-tauri/binaries/video-creater-avfoundation-exporter-aarch64-apple-darwin",
  ]);

  const discovered = relocation.selectBundleExecutableCandidates({
    repoRoot: "/repo",
    targetRoot: "/target",
    targetTriple: "aarch64-apple-darwin",
    profile: "release",
    cargoBinNames: [
      "video-creater",
      "video-creater-render-proposal",
      "video-creater-missing",
    ],
    externalBins: [
      "binaries/video-creater-compatibility-decoder",
      "binaries/video-creater-avfoundation-exporter",
    ],
    exists: (path) => existing.has(path),
  });

  assert.deepEqual(
    discovered.map(({ packagedName, runtimeOwner }) => ({
      packagedName,
      runtimeOwner,
    })),
    [
      { packagedName: "video-creater", runtimeOwner: "render" },
      {
        packagedName: "video-creater-render-proposal",
        runtimeOwner: "render",
      },
      {
        packagedName: "video-creater-compatibility-decoder",
        runtimeOwner: "compatibility",
      },
      {
        packagedName: "video-creater-avfoundation-exporter",
        runtimeOwner: "system",
      },
    ],
  );
});

test("post-sign audit fails instead of proposing a Mach-O mutation", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  assert.throws(
    () =>
      relocation.assertNoPostSignMutation({
        dependencyChanges: [
          {
            from: "/opt/homebrew/lib/libges-1.0.0.dylib",
            to: "@rpath/libges-1.0.0.dylib",
          },
        ],
        deleteRpaths: [],
        addRpaths: [],
      }),
    /post-sign mutation required.*libges-1\.0\.0\.dylib/i,
  );
});

test("check-only relocation rejects a forbidden rpath mutation", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  assert.throws(
    () =>
      relocation.assertNoPostSignMutation({
        dependencyChanges: [],
        deleteRpaths: [
          "/opt/homebrew/Cellar/gstreamer/1.28.2/lib",
        ],
        addRpaths: [],
      }),
    /post-sign mutation required.*delete rpath.*homebrew/i,
  );
});

test("post-sign audit rejects a forbidden rpath even with curated @rpath dependencies", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const appRoot = mkdtempSync(join(tmpdir(), "video-creater-rpath-audit-"));
  const binaryPath = join(appRoot, "Contents", "MacOS", "video-creater");
  mkdirSync(join(appRoot, "Contents", "MacOS"), { recursive: true });
  writeFileSync(binaryPath, "fixture");

  try {
    assert.throws(
      () =>
        relocation.auditPackagedMachO({
          appPath: appRoot,
          expectedAuthority: "Developer ID Application: Example (ABCDEFGHIJ)",
          expectedTeamId: "ABCDEFGHIJ",
          command: (name, args) => {
            if (name === "file") {
              return {
                stdout: `${args[0]}: Mach-O 64-bit executable arm64\n`,
                stderr: "",
              };
            }
            if (name === "otool" && args[0] === "-L") {
              return {
                stdout: `${args[1]}:\n\t@rpath/libgstreamer-1.0.0.dylib (compatibility version 0.0.0, current version 0.0.0)\n`,
                stderr: "",
              };
            }
            if (name === "otool" && args[0] === "-l") {
              return {
                stdout: [
                  "Load command 1",
                  "          cmd LC_RPATH",
                  "      cmdsize 48",
                  "         path /opt/homebrew/Cellar/gstreamer/1.28.2/lib (offset 12)",
                ].join("\n"),
                stderr: "",
              };
            }
            throw new Error(`unexpected command: ${name} ${args.join(" ")}`);
          },
        }),
      /forbidden packaged rpath.*video-creater.*homebrew/i,
    );
  } finally {
    rmSync(appRoot, { recursive: true, force: true });
  }
});

test("bundle-wide audit rejects forbidden dependencies in any helper", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  assert.throws(
    () =>
      relocation.assertNoForbiddenPackageDependencies({
        path: "Contents/MacOS/video-creater-render-proposal",
        dependencies: [
          "/usr/lib/libSystem.B.dylib",
          "/opt/homebrew/opt/glib/lib/libglib-2.0.0.dylib",
        ],
      }),
    /forbidden packaged dependency.*render-proposal.*homebrew/i,
  );
  assert.doesNotThrow(() =>
    relocation.assertNoForbiddenPackageDependencies({
      path: "Contents/MacOS/video-creater",
      dependencies: [
        "/usr/lib/libSystem.B.dylib",
        "@rpath/libgstreamer-1.0.0.dylib",
      ],
    }),
  );
});

test("nested runtime Mach-O files stay loader-relative and manifest-backed", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const manifest = {
    files: [
      {
        path: "lib/libgstreamer-1.0.0.dylib",
        machODependencies: ["/usr/lib/libSystem.B.dylib"],
      },
      {
        path: "plugins/libgstges.dylib",
        machODependencies: [
          "@loader_path/../lib/libgstreamer-1.0.0.dylib",
          "/System/Library/Frameworks/Foundation.framework/Versions/C/Foundation",
        ],
      },
      {
        path: "licenses/THIRD_PARTY_NOTICES.md",
        machODependencies: [],
      },
    ],
  };

  assert.deepEqual(relocation.nestedMachOPaths(manifest), [
    "lib/libgstreamer-1.0.0.dylib",
    "plugins/libgstges.dylib",
  ]);
  assert.doesNotThrow(() =>
    relocation.assertNestedDependenciesRelocated(manifest),
  );
  assert.throws(
    () =>
      relocation.assertNestedDependenciesRelocated({
        files: [
          {
            path: "plugins/libgstges.dylib",
            machODependencies: ["/opt/homebrew/lib/libges-1.0.0.dylib"],
          },
        ],
      }),
    /nested runtime dependency is not loader-relative/i,
  );
});

test("release evidence proves the signed runtime needs no later mutation", () => {
  const source = readFileSync(
    resolve(repoRoot, "scripts/build-macos-release.mjs"),
    "utf8",
  );

  assert.doesNotMatch(source, /install_name_tool/);
  assert.match(source, /runtimeManifestSha256/);
  assert.match(source, /factoryProbe/);
  assert.match(source, /nestedSignedMachOFiles/);
  assert.match(source, /auditPackagedMachO/);
  assert.match(source, /parseCodeSignatureDetails/);
  assert.match(source, /assertMatchingDeveloperIdSignature/);
});

test("release signature policy rejects ad-hoc nested Mach-O files", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  assert.throws(
    () =>
      relocation.assertMatchingDeveloperIdSignature({
        path: "plugins/libgstges.dylib",
        signature: {
          adhoc: true,
          authority: null,
          teamId: null,
        },
        expectedAuthority: "Developer ID Application: Olhapi (ABCDEFGHIJ)",
        expectedTeamId: "ABCDEFGHIJ",
      }),
    /ad-hoc signature is forbidden.*libgstges\.dylib/i,
  );
});

test("release signature policy rejects a mismatched nested TeamIdentifier", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  assert.throws(
    () =>
      relocation.assertMatchingDeveloperIdSignature({
        path: "lib/libges-1.0.0.dylib",
        signature: {
          adhoc: false,
          authority: "Developer ID Application: Other Vendor (KLMNOPQRST)",
          teamId: "KLMNOPQRST",
        },
        expectedAuthority: "Developer ID Application: Olhapi (ABCDEFGHIJ)",
        expectedTeamId: "ABCDEFGHIJ",
      }),
    /TeamIdentifier mismatch.*KLMNOPQRST.*ABCDEFGHIJ/i,
  );
});

test("release signature policy accepts the app Developer ID identity", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const signature = relocation.parseCodeSignatureDetails(`
Authority=Developer ID Application: Olhapi (ABCDEFGHIJ)
Authority=Developer ID Certification Authority
TeamIdentifier=ABCDEFGHIJ
`);

  assert.deepEqual(signature, {
    adhoc: false,
    authority: "Developer ID Application: Olhapi (ABCDEFGHIJ)",
    teamId: "ABCDEFGHIJ",
  });
  assert.doesNotThrow(() =>
    relocation.assertMatchingDeveloperIdSignature({
      path: "lib/libges-1.0.0.dylib",
      signature,
      expectedAuthority: "Developer ID Application: Olhapi (ABCDEFGHIJ)",
      expectedTeamId: "ABCDEFGHIJ",
    }),
  );
});

test("release signing fails before mutation when Developer ID configuration is absent", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");

  assert.throws(
    () => relocation.requireReleaseSigningConfiguration({}),
    /APPLE_SIGNING_IDENTITY is required/i,
  );
  assert.throws(
    () =>
      relocation.requireReleaseSigningConfiguration({
        APPLE_SIGNING_IDENTITY:
          "Developer ID Application: Olhapi (ABCDEFGHIJ)",
      }),
    /APPLE_TEAM_ID is required/i,
  );
});

test("release CLI checks signing configuration before touching the app binary", () => {
  const root = mkdtempSync(join(tmpdir(), "video-creater-signing-preflight-"));
  try {
    const binaryPath = join(root, "video-creater");
    writeFileSync(binaryPath, "untouched application bytes");
    const before = hash(binaryPath);
    const environment = { ...process.env };
    delete environment.APPLE_SIGNING_IDENTITY;
    delete environment.APPLE_TEAM_ID;
    delete environment.TAURI_ENV_DEBUG;

    const result = spawnSync(
      process.execPath,
      [
        "scripts/rewrite-gstreamer-rpaths.mjs",
        "--binary",
        binaryPath,
        "--runtime",
        "src-tauri/resources/render-runtime",
      ],
      {
        cwd: repoRoot,
        encoding: "utf8",
        env: environment,
      },
    );

    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /APPLE_SIGNING_IDENTITY is required/i);
    assert.equal(hash(binaryPath), before);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("nested signing uses Developer ID hardened-runtime options and verifies output", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const root = mkdtempSync(join(tmpdir(), "video-creater-signing-command-"));
  try {
    mkdirSync(join(root, "lib"), { recursive: true });
    const path = join(root, "lib", "libfixture.dylib");
    writeFileSync(path, "fixture");
    const calls = [];
    const signed = relocation.signNestedRuntime({
      manifest: {
        files: [
          {
            path: "lib/libfixture.dylib",
            machODependencies: [],
          },
        ],
      },
      runtimeRoot: root,
      identity: "Developer ID Application: Olhapi (ABCDEFGHIJ)",
      teamId: "ABCDEFGHIJ",
      command(command, args) {
        calls.push([command, args]);
        if (args[0] === "-dvvv") {
          return {
            stdout: "",
            stderr: [
              "Authority=Developer ID Application: Olhapi (ABCDEFGHIJ)",
              "TeamIdentifier=ABCDEFGHIJ",
            ].join("\n"),
          };
        }
        writeFileSync(path, "developer-id-signed fixture");
        return { stdout: "", stderr: "" };
      },
    });

    assert.deepEqual(calls[0], [
      "codesign",
      [
        "--force",
        "--sign",
        "Developer ID Application: Olhapi (ABCDEFGHIJ)",
        "--timestamp",
        "--options",
        "runtime",
        path,
      ],
    ]);
    assert.equal(signed[0].teamId, "ABCDEFGHIJ");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("manifest verification remains exact after nested package signing", async () => {
  const relocation = await import("./rewrite-gstreamer-rpaths.mjs");
  const root = mkdtempSync(join(tmpdir(), "video-creater-signed-runtime-"));
  try {
    mkdirSync(join(root, "lib"), { recursive: true });
    mkdirSync(join(root, "licenses"), { recursive: true });
    const machoPath = join(root, "lib", "libfixture.dylib");
    const noticePath = join(root, "licenses", "NOTICE.txt");
    writeFileSync(machoPath, "unsigned-mach-o");
    writeFileSync(noticePath, "license bytes");
    const noticeHash = hash(noticePath);
    const manifest = {
      schemaVersion: 1,
      gstreamerVersion: "1.28.2",
      gesVersion: "1.28.2",
      files: [
        fixtureFile(root, "lib/libfixture.dylib", []),
        fixtureFile(root, "licenses/NOTICE.txt", []),
      ],
    };

    writeFileSync(machoPath, "developer-id-signed-mach-o");
    relocation.refreshRuntimeManifestIntegrity(manifest, root);

    assert.notEqual(manifest.files[0].sha256, hashText("unsigned-mach-o"));
    assert.equal(manifest.files[0].sha256, hash(machoPath));
    assert.equal(manifest.files[1].sha256, noticeHash);
    assert.doesNotThrow(() =>
      validateRuntimeManifest(manifest, {
        runtimeRoot: root,
        requireCompleteRuntime: false,
      }),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function fixtureFile(root, path, machODependencies) {
  const filePath = join(root, path);
  return {
    path,
    bytes: readFileSync(filePath).byteLength,
    sha256: hash(filePath),
    license: "MIT",
    sourceComponent: "fixture",
    sourceFile: path,
    sourceUrl: "https://example.invalid/fixture",
    machODependencies,
  };
}

function hash(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function hashText(value) {
  return createHash("sha256").update(value).digest("hex");
}
