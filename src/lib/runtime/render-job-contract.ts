import contract from "../../../contracts/render-jobs-v1.json";
import type { MediaRenderAdmission, MediaRenderAttempt, ProjectMediaRenderResult } from "../project";

export const renderJobOperations = contract.operations;

function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function project(value: unknown): boolean {
  return object(value) && typeof value.id === "string" && Number.isSafeInteger(value.contentRevision)
    && Array.isArray(value.jobs) && object(value.timeline);
}

function result(value: unknown): value is ProjectMediaRenderResult {
  return object(value) && project(value.project) && typeof value.outputPath === "string"
    && object(value.renderReport) && object(value.projectRenderReport);
}

/** Validate wire envelopes before consuming canonical values. Rust validates project semantics. */
export function parseRenderAdmission(value: unknown, jobId: string, attemptId: string): MediaRenderAdmission {
  if (!object(value) || value.admissionProtocol !== contract.protocol || value.jobId !== jobId
    || value.attemptId !== attemptId || !Number.isSafeInteger(value.sourceRevision)
    || (value.sourceRevision as number) < 0 || !project(value.project)) {
    throw new Error("Render admission did not match the requested attempt.");
  }
  return value as unknown as MediaRenderAdmission;
}

export function parseRenderAttempt(value: unknown): MediaRenderAttempt {
  if (object(value)) {
    if (value.status === "pending") return { status: "pending" };
    if (value.status === "completed" && result(value.result)) return { status: "completed", result: value.result };
    if (value.status === "failed" && typeof value.message === "string"
      && (value.interrupted === undefined || typeof value.interrupted === "boolean")) {
      return { status: "failed", message: value.message, interrupted: value.interrupted === true };
    }
  }
  throw new Error("Render attempt response is invalid.");
}
