import type { ExportEncodeTier, ExportMediaStartInput, ExportOutput, ExportProfileAvailability, ExportQuality, JobExportSettings, VideoProject } from "@/lib/project";
import { exportFileNameProblem } from "./export-naming";
import { draftExportDimensions, evenDimension, exportProfileById, exportProfileDisabledReason, mediaExportOutputPath, type InProcessExportProfile } from "./profiles";

type ExportFormat = "mp4" | "prores" | "webm";
type ExportResolutionChoice = "720p" | "1080p" | "4k";
type ExportQualityChoice = "draft" | "high" | "master";
type ExportVideoCodec = "h264" | "h265";

/** What the Export popover asks for. */
export interface ExportChoices {
  /** The export's display name, shown in its completion toast. */
  readonly name: string;
  readonly format: ExportFormat;
  readonly resolution: ExportResolutionChoice;
  readonly quality: ExportQualityChoice;
  /** Advanced: the MP4 codec. ProRes and WebM have one codec each. */
  readonly codec: ExportVideoCodec;
  /** Advanced: the output frame rate; null renders at the timeline rate. */
  readonly fps: number | null;
  /** The absolute folder to save into; null saves into the project's `exports/` folder. */
  readonly directory: string | null;
}

export interface ExportFrameRateOption {
  readonly value: number | null;
  readonly label: string;
}

export interface ExportChoiceOption<T extends string> {
  readonly value: T;
  readonly label: string;
  /** Why the option can't be chosen; null when it can. */
  readonly disabledReason: string | null;
}

export interface ExportPlanInput {
  readonly choices: ExportChoices;
  /** The export capability report (`getExportProfileAvailabilityReport`). */
  readonly profiles: readonly ExportProfileAvailability[];
  readonly project: Pick<VideoProject, "id" | "schemaVersion"> & { readonly renderSettings: Pick<VideoProject["renderSettings"], "width" | "height" | "fps"> };
  readonly projectDir: string;
  readonly jobId: string;
}

/** A video export resolved for both execution backends. */
export interface ExportStartPlan {
  readonly kind: "video";
  readonly choices: ExportChoices;
  readonly name: string;
  readonly jobId: string;
  readonly profile: InProcessExportProfile;
  readonly quality: ExportQuality;
  readonly width: number;
  readonly height: number;
  /** The frame rate the renderer writes: the chosen or timeline rate, capped at 24 for Draft. */
  readonly fps: number;
  readonly encodeTier: ExportEncodeTier;
  readonly codecLabel: string;
  readonly extension: string;
  /** The project-relative output path a Temporal export writes. */
  readonly outputPath: string;
  /** `renderMediaToSplitProjectFolder` input, minus the per-run attempt id and timestamp. */
  readonly render: {
    readonly projectDir: string;
    readonly projectId: string;
    readonly profile: InProcessExportProfile;
    readonly quality: ExportQuality;
    readonly width: number;
    readonly height: number;
    readonly jobId: string;
    /** Sent only when it differs from the timeline rate. */
    readonly fps?: number;
    /** Sent only for Master. */
    readonly encodeTier?: ExportEncodeTier;
    readonly output: ExportOutput;
  };
  readonly temporal: ExportMediaStartInput;
  /**
   * The settings the export job records for Retry: the chosen resolution at full size and the chosen
   * frame rate, so a Draft export's reduced size and rate are re-applied on Retry, not kept.
   */
  readonly settings: JobExportSettings;
  /** Why this export can't start; null when it can. */
  readonly blockedReason: string | null;
}

const formatLabels: Record<ExportFormat, string> = { mp4: "MP4", prores: "ProRes", webm: "WebM" };
const resolutionOptions: readonly { readonly value: ExportResolutionChoice; readonly label: string; readonly shortSide: number }[] = [
  { value: "720p", label: "720p", shortSide: 720 },
  { value: "1080p", label: "1080p", shortSide: 1080 },
  { value: "4k", label: "4K", shortSide: 2160 },
];
const qualityLabels: Record<ExportQualityChoice, string> = { draft: "Draft", high: "High", master: "Master" };
const codecLabels: Record<ExportVideoCodec, string> = { h264: "H.264", h265: "H.265" };

const masterProResReason = "Master quality is for MP4 and WebM exports; ProRes already exports at mastering quality.";
const splitProjectReason = "Save as a schema-v2 split project to export media.";
const draftMaxFps = 24;
const commonFrameRates = [23.976, 24, 25, 29.97, 30, 50, 59.94, 60] as const;

function fpsLabel(fps: number): string {
  return String(Number(fps.toFixed(3)));
}

/** The Advanced frame-rate choices: the timeline rate, then common rates. */
export function exportFrameRateOptions(timelineFps: number): readonly ExportFrameRateOption[] {
  return [{ value: null, label: `Timeline (${fpsLabel(timelineFps)} fps)` }, ...commonFrameRates.map((value) => ({ value, label: `${fpsLabel(value)} fps` }))];
}

function profileFor(format: ExportFormat, codec: ExportVideoCodec): InProcessExportProfile {
  if (format === "prores") return "proResMov";
  if (format === "webm") return "webm";
  return codec === "h265" ? "mp4H265" : "mp4H264";
}

function codecLabelFor(profile: InProcessExportProfile): string {
  switch (profile) {
    case "mp4H264":
      return "H.264";
    case "mp4H265":
      return "H.265";
    case "proResMov":
      return "ProRes";
    case "webm":
      return "VP8";
  }
}

function backendQuality(quality: ExportQualityChoice): ExportQuality {
  return quality === "draft" ? "draft" : "final";
}

function encodeTierFor(quality: ExportQualityChoice): ExportEncodeTier {
  return quality === "master" ? "master" : "standard";
}

/** The profile's own reason, as `exportProfileDisabledReason` words it; null when it is available. */
function unavailableReason(profile: ExportProfileAvailability): string | null {
  return profile.available ? null : exportProfileDisabledReason(profile, true, false);
}

function qualityReason(profile: ExportProfileAvailability, quality: ExportQualityChoice): string | null {
  if (quality === "master" && profile.profile === "proResMov") return masterProResReason;
  const backend = backendQuality(quality);
  if (profile.qualityAvailability[backend]) return null;
  return profile.qualityUnavailableReasons[backend]?.trim() || `${qualityLabels[quality]} quality is unavailable for this format.`;
}

/** Scales the timeline so its short side matches the resolution, keeping the aspect ratio. */
function resolutionDimensions(resolution: ExportResolutionChoice, timeline: { readonly width: number; readonly height: number }) {
  const shortSide = resolutionOptions.find((option) => option.value === resolution)?.shortSide ?? 1080;
  const width = Math.max(2, timeline.width);
  const height = Math.max(2, timeline.height);
  const scale = shortSide / Math.min(width, height);
  return { width: evenDimension(Math.round(width * scale)), height: evenDimension(Math.round(height * scale)) };
}

export function exportPlan(input: ExportPlanInput): ExportStartPlan {
  const { choices, profiles, project, projectDir, jobId } = input;
  const profile = profileFor(choices.format, choices.codec);
  const availability = exportProfileById(profiles, profile);
  const quality = backendQuality(choices.quality);
  const encodeTier = encodeTierFor(choices.quality);
  const full = resolutionDimensions(choices.resolution, project.renderSettings);
  const { width, height } = quality === "draft" ? draftExportDimensions(full.width, full.height) : full;
  const timelineFps = project.renderSettings.fps > 0 ? project.renderSettings.fps : 30;
  const chosenFps = choices.fps ?? timelineFps;
  const fps = quality === "draft" ? Math.min(chosenFps, draftMaxFps) : chosenFps;
  const outputPath = mediaExportOutputPath(project.id, profile, availability.extension, jobId);
  const splitProject = project.schemaVersion >= 2 && projectDir.trim().length > 0;
  const blockedReason =
    exportFileNameProblem(choices.name) ??
    (splitProject ? null : splitProjectReason) ??
    unavailableReason(availability) ??
    qualityReason(availability, choices.quality);
  const output: ExportOutput = { fileName: choices.name.trim(), directory: choices.directory };
  // The backend renders at the timeline rate by default, so only a different rate is sent.
  const encodeSettings = { ...(fps !== timelineFps ? { fps } : {}), ...(encodeTier === "master" ? { encodeTier } : {}), output };
  return {
    kind: "video",
    choices,
    name: choices.name.trim(),
    jobId,
    profile,
    quality,
    width,
    height,
    fps,
    encodeTier,
    codecLabel: codecLabelFor(profile),
    extension: availability.extension,
    outputPath,
    render: { projectDir, projectId: project.id, profile, quality, width, height, jobId, ...encodeSettings },
    temporal: { projectId: project.id, projectDir, jobId, profile, quality, width, height, outputPath, ...encodeSettings },
    settings: { profile, quality, width: full.width, height: full.height, ...(choices.fps === null ? {} : { fps: choices.fps }), ...(encodeTier === "master" ? { encodeTier } : {}), output },
    blockedReason,
  };
}

export function isExportStartPlan(value: unknown): value is ExportStartPlan {
  return typeof value === "object" && value !== null && (value as { kind?: unknown }).kind === "video" && "render" in value && "temporal" in value;
}

/** A stored plan under a fresh job id (Retry); null when `stored` is not an export plan. */
export function replanExport(stored: unknown, jobId: string): ExportStartPlan | null {
  if (!isExportStartPlan(stored)) return null;
  const outputPath = mediaExportOutputPath(stored.temporal.projectId, stored.profile, stored.extension, jobId);
  return { ...stored, jobId, outputPath, render: { ...stored.render, jobId }, temporal: { ...stored.temporal, jobId, outputPath } };
}

export interface ExportChoiceOptionSet {
  readonly format: readonly ExportChoiceOption<ExportFormat>[];
  readonly resolution: readonly ExportChoiceOption<ExportResolutionChoice>[];
  readonly quality: readonly ExportChoiceOption<ExportQualityChoice>[];
  readonly codec: readonly ExportChoiceOption<ExportVideoCodec>[];
}

/** The popover's segmented options, each disabled with its reason when the chosen profile can't use it. */
export function exportChoiceOptions(profiles: readonly ExportProfileAvailability[], choices: ExportChoices): ExportChoiceOptionSet {
  const h264 = exportProfileById(profiles, "mp4H264");
  const h265 = exportProfileById(profiles, "mp4H265");
  const proRes = exportProfileById(profiles, "proResMov");
  const webm = exportProfileById(profiles, "webm");
  const chosen = exportProfileById(profiles, profileFor(choices.format, choices.codec));

  const format: ExportChoiceOption<ExportFormat>[] = [
    { value: "mp4", label: formatLabels.mp4, disabledReason: h264.available || h265.available ? null : unavailableReason(h264) },
    // Unavailable formats stay listed, disabled with the capability report's reason.
    { value: "prores", label: formatLabels.prores, disabledReason: unavailableReason(proRes) },
    { value: "webm", label: formatLabels.webm, disabledReason: unavailableReason(webm) },
  ];
  const resolution: ExportChoiceOption<ExportResolutionChoice>[] = resolutionOptions.map((option) => ({ value: option.value, label: option.label, disabledReason: null }));
  const quality: ExportChoiceOption<ExportQualityChoice>[] = (["draft", "high", "master"] as const).map((value) => ({
    value,
    label: qualityLabels[value],
    disabledReason: chosen.available ? qualityReason(chosen, value) : null,
  }));
  const singleCodec = choices.format === "webm" ? "WebM always uses VP8." : choices.format === "prores" ? "ProRes always uses the ProRes codec." : null;
  const codec: ExportChoiceOption<ExportVideoCodec>[] = [
    { value: "h264", label: codecLabels.h264, disabledReason: singleCodec ?? unavailableReason(h264) },
    { value: "h265", label: codecLabels.h265, disabledReason: singleCodec ?? unavailableReason(h265) },
  ];
  return { format, resolution, quality, codec };
}

/** MP4 (H.264 when it can) at 1080p High, or the first available format. */
export function defaultExportChoices(profiles: readonly ExportProfileAvailability[], name: string): ExportChoices {
  const h264 = exportProfileById(profiles, "mp4H264").available;
  const h265 = exportProfileById(profiles, "mp4H265").available;
  const format: ExportFormat = h264 || h265 ? "mp4" : exportProfileById(profiles, "webm").available ? "webm" : exportProfileById(profiles, "proResMov").available ? "prores" : "mp4";
  return { name, format, resolution: "1080p", quality: "high", codec: !h264 && h265 ? "h265" : "h264", fps: null, directory: null };
}

function resolutionForShortSide(width: number, height: number): ExportResolutionChoice {
  const shortSide = Math.min(width, height);
  if (shortSide >= 2160) return "4k";
  return shortSide >= 1080 ? "1080p" : "720p";
}

/**
 * Choices preset from an earlier job (Retry without a stored plan). Recorded export settings restore
 * everything; otherwise the job's profile and quality preset what they can.
 */
export function exportChoicesFromPreset(
  base: ExportChoices,
  preset: { readonly profile: string | null; readonly quality: string | null; readonly settings?: JobExportSettings | null },
): ExportChoices {
  const byProfile: Record<string, Pick<ExportChoices, "format" | "codec">> = {
    mp4H264: { format: "mp4", codec: "h264" },
    mp4H265: { format: "mp4", codec: "h265" },
    proResMov: { format: "prores", codec: base.codec },
    webm: { format: "webm", codec: base.codec },
  };
  const settings = preset.settings;
  if (settings) {
    const quality: ExportQualityChoice = settings.quality === "draft" ? "draft" : settings.encodeTier === "master" ? "master" : "high";
    return {
      ...base,
      ...(byProfile[settings.profile] ?? {}),
      quality,
      resolution: resolutionForShortSide(settings.width, settings.height),
      fps: settings.fps ?? null,
      name: settings.output?.fileName ?? base.name,
      directory: settings.output?.directory ?? null,
    };
  }
  const format = preset.profile ? byProfile[preset.profile] : undefined;
  const quality: ExportQualityChoice | null = preset.quality === "draft" ? "draft" : preset.quality === "final" ? "high" : null;
  return { ...base, ...(format ?? {}), ...(quality ? { quality } : {}) };
}

/** Approximate video bitrate, following the native renderer's draft, final and Master bitrate rules. */
function videoKbps(plan: ExportStartPlan): number {
  const resolutionFactor = (Math.max(1, plan.width) * Math.max(1, plan.height)) / (1920 * 1080);
  if (plan.profile === "proResMov") return 147_000 * resolutionFactor * (plan.fps / 30);
  const fpsFactor = Math.min(2, Math.max(1, plan.fps / 30));
  const final = Math.min(24_000, Math.max(6_000, 6_000 * resolutionFactor * fpsFactor));
  const master = Math.min(60_000, Math.max(12_000, final * 2));
  const rate = plan.quality === "draft" ? Math.min(8_000, Math.max(1_200, final * 0.35)) : plan.encodeTier === "master" ? master : final;
  return plan.profile === "mp4H265" ? rate * 0.7 : rate;
}

/** Estimated file size in bytes, for the popover summary line. */
export function estimatedExportSize(plan: ExportStartPlan, durationSeconds: number): number {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return 0;
  const audioKbps = plan.profile === "proResMov" ? 1_536 : 192;
  const bytesPerSecond = Math.round(((videoKbps(plan) + audioKbps) * 1_000) / 8);
  return bytesPerSecond * Math.round(durationSeconds * 1_000) / 1_000;
}
