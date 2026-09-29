import type { ExportProfile, ExportQuality, NleXmlExportFormat } from "@/lib/project";

export function generatedCodexEditJobId() {
  return `codex-edit-${Date.now().toString(36)}`;
}

export function generatedNleExportJobId(format: NleXmlExportFormat) {
  const target = format === "premiereXmeml" ? "premiere" : "davinci";
  return `nle-export-${target}-${Date.now().toString(36)}`;
}

export function generatedRenderJobId(quality: ExportQuality) {
  return `render-${quality}-${Date.now().toString(36)}`;
}

export function generatedRenderAttemptId() {
  return `render-attempt/${globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`}`;
}

const saveRangeJobIdPrefix = "save-range-";

export function generatedSaveRangeJobId() {
  return `${saveRangeJobIdPrefix}${Date.now().toString(36)}`;
}

/** A "Save range as media" render: a timeline render whose output is imported into Media. */
export function isSaveRangeJobId(jobId: string) {
  return jobId.startsWith(saveRangeJobIdPrefix);
}

export function generatedMediaExportJobId(profile: ExportProfile) {
  return `export-${profile}-${Date.now().toString(36)}`;
}

const videoExportJobIdPattern = /^export-(webm|mp4H264|mp4H265|proResMov)-[a-z0-9]+$/;

/**
 * The video profile a `generatedMediaExportJobId` names. In-process exports record `render_draft`
 * jobs without a start request, so after a restart the job id is all that still says it was an export.
 */
export function videoExportJobIdProfile(jobId: string): Exclude<ExportProfile, "palmierProject"> | null {
  const match = videoExportJobIdPattern.exec(jobId);
  return match ? (match[1] as Exclude<ExportProfile, "palmierProject">) : null;
}

export function generatedTranscribeMediaJobId(mediaId: string) {
  const slug = mediaId
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return `transcribe-${slug || "media"}-${Date.now().toString(36)}`;
}
