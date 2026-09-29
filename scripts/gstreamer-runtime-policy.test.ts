import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  evaluateFactoryProvenance,
  renderThirdPartyNotices,
  resolveReviewedRuntimeComponent,
  requiredFactories,
  requiredLibraries,
  requiredPlugins,
  validateCorrespondingSourceUrl,
  validateCuratedSelection,
  validateRuntimeManifest,
} from "./gstreamer-runtime-policy.mjs";
import {
  parsePluginDetails,
  runNativeRuntimeProbe,
  runProbeWithTimeoutRetry,
  verifyGstreamerRuntime,
} from "./verify-gstreamer-runtime.mjs";
import {
  machODependenciesWithoutInstallId,
  publishVerifiedRuntime,
} from "./build-gstreamer-runtime.mjs";

test("accepts the complete curated GStreamer libraries and plugins", () => {
  assert.deepEqual(requiredLibraries, [
    "libgstreamer-1.0.0.dylib",
    "libgstbase-1.0.0.dylib",
    "libgstapp-1.0.0.dylib",
    "libgstvideo-1.0.0.dylib",
    "libgstaudio-1.0.0.dylib",
    "libgstpbutils-1.0.0.dylib",
    "libges-1.0.0.dylib",
  ]);
  assert.ok(requiredFactories.includes("vtenc_h264"));
  assert.ok(requiredFactories.includes("atdec"));
  assert.ok(requiredFactories.includes("filesrc"));
  assert.ok(requiredFactories.includes("compositor"));
  // Pitch-preserving audio clip speed, as on Linux and in plugin_policy.rs.
  assert.ok(requiredFactories.includes("scaletempo"));
  assert.ok(requiredPlugins.includes("ges"));
  assert.ok(requiredPlugins.includes("gio"));
  assert.ok(requiredPlugins.includes("imagefreeze"));
  assert.ok(requiredPlugins.includes("nle"));
  assert.ok(requiredPlugins.includes("audiofx"));
  assert.doesNotThrow(() =>
    validateCuratedSelection({
      libraries: requiredLibraries,
      plugins: requiredPlugins,
    }),
  );
  assert.throws(
    () =>
      validateCuratedSelection({
        libraries: requiredLibraries,
        plugins: requiredPlugins.filter((plugin) => plugin !== "nle"),
      }),
    /required plugin is absent: nle/,
  );
});

test("retains auditable evidence for the one allowed timeout retry", () => {
  const timeout = {
    error: { code: "ETIMEDOUT", message: "probe exceeded deadline" },
    signal: "SIGTERM",
    status: null,
    stdout: "attempt one stdout",
    stderr: "attempt one stderr",
  };
  const success = { status: 0, signal: null, stdout: "ready", stderr: "" };
  const spawnedAttempts = [];
  const outcome = runProbeWithTimeoutRetry({
    spawnProbe(attempt) {
      spawnedAttempts.push(attempt);
      return attempt === 1 ? timeout : success;
    },
  });

  assert.equal(outcome.result, success);
  assert.deepEqual(spawnedAttempts, [1, 2]);
  assert.deepEqual(outcome.attempts, [
    {
      attempt: 1,
      error: { code: "ETIMEDOUT", message: "probe exceeded deadline" },
      signal: "SIGTERM",
      status: null,
      stderr: "attempt one stderr",
      stdout: "attempt one stdout",
      timedOut: true,
    },
    {
      attempt: 2,
      error: null,
      signal: null,
      status: 0,
      stderr: "",
      stdout: "ready",
      timedOut: false,
    },
  ]);
});

test("aggregates both timed-out attempts and resets the registry uniquely", () => {
  const runtimeRoot = mkdtempSync(join(tmpdir(), "video-creater-probe-retry-"));
  const registryPaths = [];
  let attempts = 0;

  assert.throws(
    () =>
      runNativeRuntimeProbe({
        runtimeRoot,
        spawnProbe({ attempt, options }) {
          attempts += 1;
          registryPaths.push(options.env.GST_REGISTRY_1_0);
          return {
            error: {
              code: "ETIMEDOUT",
              message: `deadline ${attempt}`,
            },
            signal: attempt === 1 ? "SIGTERM" : "SIGKILL",
            status: null,
            stdout: `stdout ${attempt}`,
            stderr: `stderr ${attempt}`,
          };
        },
        timeoutMs: 17,
      }),
    (error) => {
      assert.match(error.message, /timed out after 17ms/);
      for (const attempt of [1, 2]) {
        assert.match(error.message, new RegExp(`attempt ${attempt}`));
        assert.match(error.message, new RegExp(`stdout ${attempt}`));
        assert.match(error.message, new RegExp(`stderr ${attempt}`));
        assert.match(error.message, new RegExp(`deadline ${attempt}`));
      }
      assert.match(error.message, /SIGTERM/);
      assert.match(error.message, /SIGKILL/);
      return true;
    },
  );
  assert.equal(attempts, 2);
  assert.equal(new Set(registryPaths).size, 2);
  assert.match(registryPaths[0], /attempt-1\/registry\.bin$/);
  assert.match(registryPaths[1], /attempt-2\/registry\.bin$/);
});

test("does not retry ordinary process, malformed output, or provenance failures", () => {
  const runtimeRoot = mkdtempSync(join(tmpdir(), "video-creater-probe-policy-"));
  const cases = [
    {
      expected: /probe failed.*bad factory/s,
      result: { status: 1, signal: null, stdout: "", stderr: "bad factory" },
    },
    {
      expected: /returned 0 of .* required factories/,
      result: {
        status: 0,
        signal: null,
        stdout: "GES_SMOKE\tpassed\timagefreeze+gio+nle\n",
        stderr: "",
      },
    },
    {
      expected: /factory 'appsink' provenance does not match the reviewed/,
      result: {
        status: 0,
        signal: null,
        stdout:
          "FACTORY\tappsink\tunreviewed\tThird Party Plug-ins\tProprietary\n" +
          "GES_SMOKE\tpassed\timagefreeze+gio+nle\n",
        stderr: "",
      },
    },
  ];

  for (const probeCase of cases) {
    let attempts = 0;
    assert.throws(
      () =>
        runNativeRuntimeProbe({
          runtimeRoot,
          spawnProbe() {
            attempts += 1;
            return probeCase.result;
          },
        }),
      probeCase.expected,
    );
    assert.equal(attempts, 1);
  }
});

test("matches Rust first-match factory provenance and license parsing", () => {
  const allowedVideoflip = {
    name: "videoflip",
    pluginName: "videofilter",
    package: "GStreamer Good Plug-ins source release",
    license: "LGPL-2.1-or-later",
  };
  assert.equal(evaluateFactoryProvenance(allowedVideoflip).verdict, "allowed");
  assert.equal(
    evaluateFactoryProvenance({
      ...allowedVideoflip,
      pluginName: "videoflip",
    }).verdict,
    "denied",
  );
  assert.equal(
    evaluateFactoryProvenance({
      ...allowedVideoflip,
      license: "GNU Lesser General Public License",
    }).verdict,
    "allowed",
  );
  for (const license of [
    "LGPL-2.1-or-later AND MIT",
    "LGPL-2.1-or-later OR GPL-2.0",
    "LGPL-2.1-or-later WITH LLVM-exception",
    "NOASSERTION",
    "Proprietary",
  ]) {
    assert.equal(
      evaluateFactoryProvenance({
        ...allowedVideoflip,
        license,
      }).verdict,
      "denied",
      license,
    );
  }
});

test("allows scaletempo from the Good plug-ins audiofx plugin", () => {
  assert.equal(
    evaluateFactoryProvenance({
      name: "scaletempo",
      pluginName: "audiofx",
      package: "GStreamer Good Plug-ins source release",
      license: "LGPL",
    }).verdict,
    "allowed",
  );
});

test("allows the reviewed macOS AudioToolbox AAC decoder provenance", () => {
  assert.equal(
    evaluateFactoryProvenance({
      name: "atdec",
      pluginName: "osxaudio",
      package: "GStreamer Good Plug-ins source release",
      license: "LGPL",
    }).verdict,
    "allowed",
  );
  assert.equal(
    evaluateFactoryProvenance({
      name: "atdec",
      pluginName: "osxaudio",
      package: "Third Party GStreamer Plug-ins",
      license: "LGPL",
    }).verdict,
    "denied",
  );
});

test("license inventory and notices fail closed with replacement instructions", () => {
  assert.throws(
    () =>
      renderThirdPartyNotices([
        {
          component: "unknown",
          file: "lib/libunknown.dylib",
          sourceUrl: "",
          license: "NOASSERTION",
        },
      ]),
    /reviewed SPDX license/,
  );
  const notices = renderThirdPartyNotices([
    {
      component: "GStreamer 1.28.2",
      file: "lib/libgstreamer-1.0.0.dylib",
      sourceUrl: "https://gitlab.freedesktop.org/gstreamer/gstreamer",
      license: "LGPL-2.1-or-later",
    },
  ]);
  assert.match(notices, /GStreamer/);
  assert.match(notices, /lib\/libgstreamer-1\.0\.0\.dylib/);
  assert.match(notices, /LGPL-2\.1-or-later/);
  assert.match(notices, /relink/i);
  assert.match(notices, /replace/i);
  assert.match(notices, /upstream corresponding source/i);
  assert.doesNotMatch(notices, /packaged source/i);
  assert.doesNotMatch(notices, /\/opt\/homebrew|\/usr\/local\/Cellar/i);
});

test("rejects bottle, local-path, and unreviewed corresponding-source metadata", () => {
  for (const sourceUrl of [
    "https://ghcr.io/v2/homebrew/core/glib/blobs/sha256:fixture",
    "/opt/homebrew/Cellar/glib/2.88.0",
    "file:///opt/homebrew/Cellar/glib/2.88.0",
  ]) {
    assert.throws(
      () => validateCorrespondingSourceUrl(sourceUrl),
      /upstream corresponding-source URL/,
      sourceUrl,
    );
  }
  assert.throws(
    () =>
      resolveReviewedRuntimeComponent({
        owner: "glib",
        file: "lib/libglib-2.0.0.dylib",
        installedVersion: "2.88.0",
        formula: formulaFixture({
          name: "glib",
          license: "LicenseRef-internal-review-needed",
          stableVersion: "2.88.0",
          stableUrl: "https://download.gnome.org/sources/glib/2.88/glib-2.88.0.tar.xz",
        }),
      }),
    /reviewed SPDX license/,
  );
});

test("uses exact reviewed file licenses and truthful upstream source URLs", () => {
  const gettext = resolveReviewedRuntimeComponent({
    owner: "gettext",
    file: "lib/libintl.8.dylib",
    installedVersion: "1.0",
    formula: formulaFixture({
      name: "gettext",
      license: "GPL-3.0-or-later AND LGPL-2.1-or-later",
      stableVersion: "1.0",
      stableUrl: "https://ftpmirror.gnu.org/gnu/gettext/gettext-1.0.tar.gz",
    }),
  });
  assert.deepEqual(gettext, {
    component: "GNU gettext libintl 1.0",
    file: "lib/libintl.8.dylib",
    license: "LGPL-2.1-or-later",
    sourceUrl: "https://ftpmirror.gnu.org/gnu/gettext/gettext-1.0.tar.gz",
  });

  const glib = resolveReviewedRuntimeComponent({
    owner: "glib",
    file: "lib/libglib-2.0.0.dylib",
    installedVersion: "2.88.0",
    formula: formulaFixture({
      name: "glib",
      license: "LGPL-2.1-or-later",
      stableVersion: "2.88.2",
      stableUrl: "https://download.gnome.org/sources/glib/2.88/glib-2.88.2.tar.xz",
    }),
  });
  assert.equal(glib.license, "LGPL-2.1-or-later");
  assert.equal(glib.sourceUrl, "https://gitlab.gnome.org/GNOME/glib");

  assert.throws(
    () =>
      resolveReviewedRuntimeComponent({
        owner: "gettext",
        file: "lib/libgettextsrc.dylib",
        installedVersion: "1.0",
        formula: formulaFixture({
          name: "gettext",
          license: "GPL-3.0-or-later AND LGPL-2.1-or-later",
          stableVersion: "1.0",
          stableUrl: "https://ftpmirror.gnu.org/gnu/gettext/gettext-1.0.tar.gz",
        }),
      }),
    /unreviewed runtime owner\/file/,
  );
});

test("atomic runtime publication restores the previous runtime on swap failure", () => {
  const root = mkdtempSync(join(tmpdir(), "video-creater-runtime-publish-"));
  const runtime = join(root, "render-runtime");
  const candidate = join(root, "render-runtime.candidate");
  mkdirSync(runtime);
  mkdirSync(candidate);
  writeFileSync(join(runtime, "marker"), "old");
  writeFileSync(join(candidate, "marker"), "new");
  let renameCalls = 0;

  assert.throws(
    () =>
      publishVerifiedRuntime(candidate, runtime, {
        renameSync(source, target) {
          renameCalls += 1;
          if (renameCalls === 2) throw new Error("injected publish failure");
          return renameSync(source, target);
        },
      }),
    /injected publish failure/,
  );
  assert.equal(readFileSync(join(runtime, "marker"), "utf8"), "old");
});

test("dependency closure preserves compatibility install names for versioned dylibs", () => {
  assert.deepEqual(
    machODependenciesWithoutInstallId(
      "/opt/homebrew/Cellar/jpeg-turbo/3.1.3/lib/libjpeg.8.4.0.dylib",
      [
        "/opt/homebrew/opt/jpeg-turbo/lib/libjpeg.8.dylib",
        "/usr/lib/libSystem.B.dylib",
      ],
    ),
    ["/usr/lib/libSystem.B.dylib"],
  );
});

test("rejects denied and unreviewed plugins", () => {
  assert.throws(
    () =>
      validateCuratedSelection({
        libraries: requiredLibraries,
        plugins: [...requiredPlugins, "libav"],
      }),
    /denied GStreamer plugin: libav/,
  );
  assert.throws(
    () =>
      validateCuratedSelection({
        libraries: requiredLibraries,
        plugins: [...requiredPlugins, "mysterycodec"],
      }),
    /unreviewed GStreamer plugin: mysterycodec/,
  );
});

test("requires immutable SHA-256 entries for every runtime file", () => {
  const fixture = runtimeFixture();
  const appPlugin = join(fixture.root, "plugins", "libgstapp.dylib");
  writeFileSync(appPlugin, "approved app plugin");
  fixture.manifest.files.push({
    path: "plugins/libgstapp.dylib",
    bytes: 19,
    sha256: createHash("sha256").update("approved app plugin").digest("hex"),
    license: "LGPL-2.1-or-later",
    sourceComponent: "GStreamer",
    sourceFile: "plugins/libgstapp.dylib",
    sourceUrl: "https://gitlab.freedesktop.org/gstreamer/gstreamer",
    machODependencies: [],
  });

  assert.doesNotThrow(() =>
    validateRuntimeManifest(fixture.manifest, {
      runtimeRoot: fixture.root,
      requireCompleteRuntime: false,
    }),
  );

  fixture.manifest.files[0].sha256 = "mutable";
  assert.throws(
    () =>
      validateRuntimeManifest(fixture.manifest, {
        runtimeRoot: fixture.root,
        requireCompleteRuntime: false,
      }),
    /immutable SHA-256/,
  );
});

test("rejects absolute Homebrew and usr-local Mach-O dependencies", () => {
  const fixture = runtimeFixture();
  fixture.manifest.files.push(fileEntry("lib/libgstreamer-1.0.0.dylib"));
  writeFileSync(join(fixture.root, "lib", "libgstreamer-1.0.0.dylib"), "fixture");
  fixture.manifest.files[0].machODependencies = [
    "/opt/homebrew/Cellar/gstreamer/1.28.2/lib/libgstreamer-1.0.0.dylib",
  ];
  assert.throws(
    () =>
      validateRuntimeManifest(fixture.manifest, {
        runtimeRoot: fixture.root,
        requireCompleteRuntime: false,
      }),
    /external package-manager dependency/,
  );

  fixture.manifest.files[0].machODependencies = [
    "/usr/local/lib/libgstreamer-1.0.0.dylib",
  ];
  assert.throws(
    () =>
      validateRuntimeManifest(fixture.manifest, {
        runtimeRoot: fixture.root,
        requireCompleteRuntime: false,
      }),
    /external package-manager dependency/,
  );
});

test("rejects incomplete runtime manifests without scanner, GES, or factories", () => {
  const fixture = runtimeFixture();
  assert.throws(
    () => validateRuntimeManifest(fixture.manifest, { runtimeRoot: fixture.root }),
    /gst-plugin-scanner/,
  );
  writeFileSync(
    join(fixture.root, "manifest.json"),
    `${JSON.stringify(fixture.manifest)}\n`,
  );
  assert.throws(
    () =>
      verifyGstreamerRuntime({
        runtimeRoot: fixture.root,
        probe: false,
      }),
    /gst-plugin-scanner/,
  );

  fixture.manifest.files.push(fileEntry("libexec/gst-plugin-scanner"));
  writeFileSync(join(fixture.root, "libexec", "gst-plugin-scanner"), "fixture");
  assert.throws(
    () => validateRuntimeManifest(fixture.manifest, { runtimeRoot: fixture.root }),
    /libges-1\.0\.0\.dylib/,
  );

  fixture.manifest.files.push(fileEntry("lib/libges-1.0.0.dylib"));
  writeFileSync(join(fixture.root, "lib", "libges-1.0.0.dylib"), "fixture");
  assert.throws(
    () => validateRuntimeManifest(fixture.manifest, { runtimeRoot: fixture.root }),
    /required factory/,
  );
});

test("parses factory provenance in the same shape as the Rust policy", () => {
  assert.deepEqual(
    parsePluginDetails(
      "appsink",
      [
        "Factory Details:",
        "  Rank                     none (0)",
        "",
        "Plugin Details:",
        "  Name                     app",
        "  Description              Elements used to communicate with applications",
        "  Filename                 /fixture/plugins/libgstapp.dylib",
        "  Version                  1.28.2",
        "  License                  LGPL",
        "  Source module            gst-plugins-base",
        "  Binary package           GStreamer Base Plug-ins source release",
        "  Origin URL               https://gstreamer.freedesktop.org/",
        "",
        "Element Flags:",
      ].join("\n"),
    ),
    {
      name: "appsink",
      pluginName: "app",
      package: "GStreamer Base Plug-ins source release",
      license: "LGPL",
    },
  );
});

function runtimeFixture() {
  const root = mkdtempSync(join(tmpdir(), "video-creater-gstreamer-policy-"));
  for (const directory of ["lib", "plugins", "libexec", "licenses"]) {
    mkdirSync(join(root, directory), { recursive: true });
  }
  return {
    root,
    manifest: {
      schemaVersion: 1,
      target: "aarch64-apple-darwin",
      gstreamerVersion: "1.28.2",
      gesVersion: "1.28.2",
      libraries: [],
      plugins: [],
      factories: [],
      files: [],
    },
  };
}

function fileEntry(path: string) {
  const content = "fixture";
  return {
    path,
    bytes: Buffer.byteLength(content),
    sha256: createHash("sha256").update(content).digest("hex"),
    license: "LGPL-2.1-or-later",
    sourceComponent: "GStreamer",
    sourceFile: path,
    sourceUrl: "https://gitlab.freedesktop.org/gstreamer/gstreamer",
    machODependencies: [],
  };
}

function formulaFixture({
  name,
  license,
  stableVersion,
  stableUrl,
}: {
  name: string;
  license: string;
  stableVersion: string;
  stableUrl: string;
}) {
  return {
    name,
    license,
    versions: { stable: stableVersion },
    urls: { stable: { url: stableUrl } },
  };
}
