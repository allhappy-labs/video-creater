import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// @ts-expect-error The repository does not generate declarations for scripts/*.mjs.
import { notarizeAndVerifyDmg } from "../scripts/macos-notarization.mjs";

const repoRoot = process.cwd();

describe("macOS release automation", () => {
  it("fails closed around Developer ID signing and notarized package evidence", () => {
    const packageJson = JSON.parse(
      readFileSync(join(repoRoot, "package.json"), "utf8"),
    ) as { scripts?: Record<string, string> };
    const source = readFileSync(
      join(repoRoot, "scripts/build-macos-release.mjs"),
      "utf8",
    );
    const relocationSource = readFileSync(
      join(repoRoot, "scripts/rewrite-gstreamer-rpaths.mjs"),
      "utf8",
    );
    const notarizationSource = readFileSync(
      join(repoRoot, "scripts/macos-notarization.mjs"),
      "utf8",
    );

    expect(packageJson.scripts?.["release:macos:preflight"]).toBe(
      "node scripts/build-macos-release.mjs --preflight",
    );
    expect(packageJson.scripts?.["release:macos"]).toBe(
      "node scripts/build-macos-release.mjs",
    );
    expect(source).toContain('startsWith("Developer ID Application:")');
    expect(source).toContain("no valid Developer ID Application identity is installed");
    expect(source).toContain("video-creater-notary");
    expect(source).toContain('"notarytool",');
    expect(source).toContain('"history",');
    expect(source).toContain("Apple notarization authentication failed");
    expect(source).toContain("could not be derived from the signing identity");
    expect(source).toContain("has no readable account");
    expect(source).toContain('"--bundles"');
    expect(source).toContain('"app,dmg"');
    expect(source).toContain("notarizeAndVerifyDmg({");
    expect(notarizationSource).toContain('"submit",');
    expect(notarizationSource).toContain('"--wait",');
    expect(notarizationSource).toContain('submission.status !== "Accepted"');
    expect(notarizationSource).toContain('"stapler", "staple", dmgPath');
    expect(notarizationSource).toContain('"stapler", "validate"');
    expect(source).toContain('"--assess", "--type", "execute"');
    expect(notarizationSource).toMatch(
      /runCommand\("spctl", \[\s*"--assess",\s*"--type",\s*"open"/,
    );
    expect(notarizationSource).toContain('"context:primary-signature"');
    expect(source).toContain("dmgNotarization");
    expect(source).toContain("dmgGatekeeper");
    expect(source).toContain("assertMatchingDeveloperIdSignature");
    expect(source).toContain("expectedAuthority: identity");
    expect(source).toContain("expectedTeamId: teamId");
    expect(relocationSource).toContain("Developer ID authority mismatch for");
    expect(source).toContain("auditPackagedMachO");
    expect(source).toContain("packageManagerIndependentMachO: true");
    expect(source).toContain('"--require-developer-id"');
    expect(source).toContain("collectReleaseSourceEvidence");
    expect(source).toContain("assertReleaseSourceStable");
    expect(source).toContain("release-${source.shortCommit}");
    expect(source).toContain("source,");
  });

  it("submits, accepts, staples, validates, and Gatekeeper-assesses the exact DMG in order", () => {
    const calls: Array<{ args: string[]; command: string }> = [];
    const results = [
      {
        status: 0,
        stdout: JSON.stringify({
          id: "b8f714f5-d1dc-417f-b5ba-06bc246d5e84",
          message: "Successfully received submission info",
          status: "Accepted",
        }),
        stderr: "",
      },
      { status: 0, stdout: "stapled", stderr: "" },
      { status: 0, stdout: "validated", stderr: "" },
      { status: 0, stdout: "accepted source=Notarized Developer ID", stderr: "" },
    ];

    const evidence = notarizeAndVerifyDmg({
      appleId: "release@example.com",
      dmgPath: "/release/Video Creater.dmg",
      password: "app-specific-secret",
      runCommand(command: string, args: string[]) {
        calls.push({ command, args });
        return results[calls.length - 1];
      },
      teamId: "TEAM123456",
    });

    expect(calls).toEqual([
      {
        command: "xcrun",
        args: [
          "notarytool",
          "submit",
          "/release/Video Creater.dmg",
          "--apple-id",
          "release@example.com",
          "--password",
          "app-specific-secret",
          "--team-id",
          "TEAM123456",
          "--wait",
          "--output-format",
          "json",
        ],
      },
      {
        command: "xcrun",
        args: ["stapler", "staple", "/release/Video Creater.dmg"],
      },
      {
        command: "xcrun",
        args: ["stapler", "validate", "/release/Video Creater.dmg"],
      },
      {
        command: "spctl",
        args: [
          "--assess",
          "--type",
          "open",
          "--context",
          "context:primary-signature",
          "--verbose=4",
          "/release/Video Creater.dmg",
        ],
      },
    ]);
    expect(evidence).toEqual({
      gatekeeper: "accepted source=Notarized Developer ID",
      id: "b8f714f5-d1dc-417f-b5ba-06bc246d5e84",
      message: "Successfully received submission info",
      staple: "stapled",
      stapleValidation: "validated",
      status: "Accepted",
    });
  });

  it.each([
    ["malformed", { status: 0, stdout: "not-json", stderr: "" }, /malformed JSON/],
    [
      "rejected",
      {
        status: 0,
        stdout: JSON.stringify({ id: "rejected-id", status: "Rejected" }),
        stderr: "",
      },
      /was not accepted.*Rejected/,
    ],
  ])("fails closed on %s DMG notarization output before stapling", (_name, result, expected) => {
    let calls = 0;
    expect(() =>
      notarizeAndVerifyDmg({
        appleId: "release@example.com",
        dmgPath: "/release/Video Creater.dmg",
        password: "app-specific-secret",
        runCommand() {
          calls += 1;
          return result;
        },
        teamId: "TEAM123456",
      }),
    ).toThrow(expected);
    expect(calls).toBe(1);
  });

  it("redacts notarization credentials from command failures", () => {
    const password = "app-specific-secret";
    let thrown: Error | undefined;
    try {
      notarizeAndVerifyDmg({
        appleId: "release@example.com",
        dmgPath: "/release/Video Creater.dmg",
        password,
        runCommand() {
          return {
            status: 1,
            stdout: "",
            stderr: `authentication failed for ${password}`,
          };
        },
        teamId: "TEAM123456",
      });
    } catch (error) {
      thrown = error as Error;
    }
    expect(thrown?.message).toContain("<redacted>");
    expect(thrown?.message).not.toContain(password);
  });

  it("rejects an Accepted response with a whitespace-only submission id before stapling", () => {
    let calls = 0;
    expect(() =>
      notarizeAndVerifyDmg({
        appleId: "release@example.com",
        dmgPath: "/release/Video Creater.dmg",
        password: "app-specific-secret",
        runCommand() {
          calls += 1;
          return {
            status: 0,
            stdout: JSON.stringify({ id: "   ", status: "Accepted" }),
            stderr: "",
          };
        },
        teamId: "TEAM123456",
      }),
    ).toThrow(/was not accepted/);
    expect(calls).toBe(1);
  });

  it("rejects an Accepted non-UUID submission id containing the team id before stapling", () => {
    let calls = 0;
    expect(() =>
      notarizeAndVerifyDmg({
        appleId: "release@example.com",
        dmgPath: "/release/Video Creater.dmg",
        password: "app-specific-secret",
        runCommand() {
          calls += 1;
          return {
            status: 0,
            stdout: JSON.stringify({ id: "  TEAM123456  ", status: "Accepted" }),
            stderr: "",
          };
        },
        teamId: "TEAM123456",
      }),
    ).toThrow(/was not accepted/);
    expect(calls).toBe(1);
  });

  it("redacts the team id from failures and returned evidence", () => {
    const teamId = "TEAM123456";
    let failure: Error | undefined;
    try {
      notarizeAndVerifyDmg({
        appleId: "release@example.com",
        dmgPath: "/release/Video Creater.dmg",
        password: "app-specific-secret",
        runCommand() {
          return {
            status: 1,
            stdout: "",
            stderr: `notary rejected team ${teamId}`,
          };
        },
        teamId,
      });
    } catch (error) {
      failure = error as Error;
    }
    expect(failure?.message).toContain("<redacted>");
    expect(failure?.message).not.toContain(teamId);

    const results = [
      {
        status: 0,
        stdout: JSON.stringify({
          id: "d0d6e83d-9152-4de3-a018-f47a742f9852",
          message: `accepted for team ${teamId}`,
          status: "Accepted",
        }),
        stderr: "",
      },
      { status: 0, stdout: `stapled for ${teamId}`, stderr: "" },
      { status: 0, stdout: `validated for ${teamId}`, stderr: "" },
      { status: 0, stdout: `Gatekeeper accepted ${teamId}`, stderr: "" },
    ];
    let calls = 0;
    const evidence = notarizeAndVerifyDmg({
      appleId: "release@example.com",
      dmgPath: "/release/Video Creater.dmg",
      password: "app-specific-secret",
      runCommand() {
        const result = results[calls];
        calls += 1;
        return result;
      },
      teamId,
    });
    expect(JSON.stringify(evidence)).toContain("<redacted>");
    expect(JSON.stringify(evidence)).not.toContain(teamId);
  });
});
