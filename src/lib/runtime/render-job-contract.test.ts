import { describe, expect, it } from "vitest";
import contract from "../../../contracts/render-jobs-v1.json";
import { createSampleProject } from "../sample-project";
import { parseRenderAdmission, parseRenderAttempt, renderJobOperations } from "./render-job-contract";

describe("Rust-owned render job v1 envelopes", () => {
  it("accepts checked admission and attempt goldens", () => {
    const admission = { admissionProtocol: contract.protocol, project: { ...createSampleProject(), contentRevision: 4 }, jobId: contract.input.jobId, attemptId: contract.input.attemptId, sourceRevision: 4 };
    expect(Object.keys(admission).sort()).toEqual(contract.admissionFields);
    expect(parseRenderAdmission(admission, contract.input.jobId, contract.input.attemptId)).toBe(admission);
    for (const attempt of contract.attempts) expect(parseRenderAttempt(attempt)).toEqual(attempt);
    expect(renderJobOperations.attempt).toBe("load_render_attempt_in_split_project_folder");
  });

  it("rejects malformed or mismatched envelopes before using their canonical payload", () => {
    expect(() => parseRenderAttempt(null)).toThrow();
    expect(() => parseRenderAttempt({ status: "completed", result: {} })).toThrow();
    expect(() => parseRenderAttempt({ status: "failed", message: "x", interrupted: "true" })).toThrow();
    expect(() => parseRenderAdmission({ admissionProtocol: 2 }, "job", "attempt")).toThrow();
    expect(() => parseRenderAdmission({ admissionProtocol: 1, jobId: "old", attemptId: "attempt", sourceRevision: 1, project: {} }, "job", "attempt")).toThrow();
  });
});
