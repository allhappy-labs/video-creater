import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "..");
const notarizationSubmissionIdPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function notarizeAndVerifyDmg({
  appleId,
  dmgPath,
  password,
  runCommand = run,
  teamId,
}) {
  const secrets = [appleId, password, teamId];
  const submissionResult = runCommand("xcrun", [
    "notarytool",
    "submit",
    dmgPath,
    "--apple-id",
    appleId,
    "--password",
    password,
    "--team-id",
    teamId,
    "--wait",
    "--output-format",
    "json",
  ]);
  requireSuccess(submissionResult, "DMG notarization submission", secrets);

  let submission;
  try {
    submission = JSON.parse(submissionResult.stdout);
  } catch {
    throw new Error(
      `DMG notarization returned malformed JSON: ${redact(
        submissionResult.stdout || "no output",
        secrets,
      )}`,
    );
  }
  const submissionId =
    typeof submission?.id === "string" ? submission.id.trim() : "";
  if (
    !submission ||
    typeof submission !== "object" ||
    submission.status !== "Accepted" ||
    !notarizationSubmissionIdPattern.test(submissionId)
  ) {
    throw new Error(
      `DMG notarization was not accepted: ${redact(
        JSON.stringify({
          id: submission?.id ?? null,
          message: submission?.message ?? null,
          status: submission?.status ?? null,
        }),
        secrets,
      )}`,
    );
  }

  const staple = runCommand("xcrun", ["stapler", "staple", dmgPath]);
  requireSuccess(staple, "DMG stapling", secrets);
  const stapleValidation = runCommand("xcrun", ["stapler", "validate", dmgPath]);
  requireSuccess(stapleValidation, "DMG staple validation", secrets);
  const gatekeeper = runCommand("spctl", [
    "--assess",
    "--type",
    "open",
    "--context",
    "context:primary-signature",
    "--verbose=4",
    dmgPath,
  ]);
  requireSuccess(gatekeeper, "DMG Gatekeeper assessment", secrets);

  return {
    gatekeeper: redact(output(gatekeeper).trim(), secrets),
    id: submissionId,
    message: redact(submission.message ?? "", secrets) || null,
    staple: redact(output(staple).trim(), secrets),
    stapleValidation: redact(output(stapleValidation).trim(), secrets),
    status: submission.status,
  };
}

function requireSuccess(result, label, secrets) {
  if (result.error || result.status !== 0) {
    throw new Error(
      `${label} failed: ${redact(
        result.error?.message || result.stderr || result.stdout || "no command output",
        secrets,
      )}`,
    );
  }
}

function output(result) {
  return `${result.stdout || ""}${result.stderr || ""}`;
}

function redact(value, secrets) {
  let redacted = String(value);
  for (const secret of secrets) {
    if (!secret) continue;
    redacted = redacted.split(secret).join("<redacted>");
  }
  return redacted;
}

function run(command, args) {
  return spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
  });
}
