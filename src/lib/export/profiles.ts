import type {
  ExportEncodeTier,
  ExportProfile,
  ExportProfileAvailability,
  ExportQuality,
  ExportQualityAvailability,
  ExportQualityUnavailableReasons,
} from "@/lib/project";

export type InProcessExportProfile = Exclude<ExportProfile, "palmierProject">;

export function inProcessExportLabel(profile: InProcessExportProfile, quality: ExportQuality, encodeTier?: ExportEncodeTier) {
  const qualityLabel = quality === "draft" ? "Draft" : encodeTier === "master" ? "Master" : "Final";
  switch (profile) {
    case "webm":
      return `${qualityLabel} WebM`;
    case "mp4H264":
      return `H.264 ${qualityLabel}`;
    case "mp4H265":
      return `H.265 (HEVC) ${qualityLabel}`;
    case "proResMov":
      return `ProRes ${qualityLabel}`;
  }
}

export const fallbackExportProfileAvailability: ExportProfileAvailability[] = [
  {
    profile: "webm",
    label: "Legacy WebM",
    available: false,
    container: "webm",
    extension: "webm",
    mimeType: "video/webm",
    videoCodec: "vp8",
    audioCodec: "opus",
    requiredRuntime: ["gstreamer:webmmux", "gstreamer:vp8enc", "gstreamer:opusenc"],
    policyStatus: "missingRuntime",
    unavailableReason: "Native export capabilities are being checked.",
    qualityAvailability: { draft: false, final: false },
    qualityUnavailableReasons: {
      draft: "Native export capabilities are being checked.",
      final: "Native export capabilities are being checked.",
    },
  },
  {
    profile: "mp4H264",
    label: "MP4 / H.264",
    available: false,
    container: "mp4",
    extension: "mp4",
    mimeType: "video/mp4",
    videoCodec: "h264",
    audioCodec: "aac",
    requiredRuntime: ["gstreamer:mp4mux", "gstreamer:vtenc_h264", "gstreamer:atenc", "gstreamer:aacparse"],
    policyStatus: "missingRuntime",
    unavailableReason: "Native export capabilities are being checked.",
    qualityAvailability: { draft: false, final: false },
    qualityUnavailableReasons: {
      draft: "Native export capabilities are being checked.",
      final: "Native export capabilities are being checked.",
    },
  },
  {
    profile: "mp4H265",
    label: "MP4 / H.265",
    available: false,
    container: "mp4",
    extension: "mp4",
    mimeType: "video/mp4",
    videoCodec: "hevc",
    audioCodec: "aac",
    requiredRuntime: ["gstreamer:mp4mux", "gstreamer:vtenc_h265", "gstreamer:atenc", "gstreamer:aacparse"],
    policyStatus: "missingRuntime",
    unavailableReason: "Native export capabilities are being checked.",
    qualityAvailability: { draft: false, final: false },
    qualityUnavailableReasons: {
      draft: "Native export capabilities are being checked.",
      final: "Native export capabilities are being checked.",
    },
  },
  {
    profile: "proResMov",
    label: "ProRes MOV",
    available: false,
    container: "mov",
    extension: "mov",
    mimeType: "video/quicktime",
    videoCodec: "prores",
    audioCodec: "pcm",
    requiredRuntime: ["gstreamer:qtmux", "gstreamer:vtenc_prores"],
    policyStatus: "missingRuntime",
    unavailableReason: "Native export capabilities are being checked.",
    qualityAvailability: { draft: false, final: false },
    qualityUnavailableReasons: {
      draft: "Native export capabilities are being checked.",
      final: "Native export capabilities are being checked.",
    },
  },
];

export function mediaExportOutputPath(
  projectId: string,
  profile: ExportProfile,
  extension: string,
  jobId: string,
) {
  return `exports/${projectId}-${profile}-${jobId}.${extension}`;
}

export function exportProfileById(
  profiles: readonly ExportProfileAvailability[],
  profile: ExportProfile,
): ExportProfileAvailability {
  const resolved =
    profiles.find((candidate) => candidate.profile === profile) ??
    fallbackExportProfileAvailability.find((candidate) => candidate.profile === profile) ??
    fallbackExportProfileAvailability[0];
  if (!resolved) throw new Error("At least one export profile must be configured.");
  return resolved;
}

export function exportProfileDisabledReason(
  profile: ExportProfileAvailability,
  canExportMediaProfiles: boolean,
  temporalExecution: boolean,
) {
  if (!profile.available) {
    return profile.unavailableReason?.trim() || "Unavailable in this build";
  }

  if (!canExportMediaProfiles) {
    return temporalExecution
      ? "Save as a schema-v2 split project to start Temporal export workflows."
      : "Save as a schema-v2 split project to export media.";
  }

  return temporalExecution ? "Start Temporal export workflow" : "Render in the desktop app";
}

export function exportPolicyStatusLabel(status: ExportProfileAvailability["policyStatus"]) {
  switch (status) {
    case "approved":
      return "Approved";
    case "missingRuntime":
      return "Missing runtime";
    case "policyGated":
      return "Policy gated";
    case "unsupportedBuild":
      return "Unsupported build";
  }
}

export function exportProfileRuntimeDetail(profile: ExportProfileAvailability) {
  if (profile.available || profile.requiredRuntime.length === 0) {
    return null;
  }

  return `${exportPolicyStatusLabel(profile.policyStatus)} · Requires ${profile.requiredRuntime.join(", ")}`;
}

export type ExportDestination = "video" | "timeline" | "palmier-project";

type ExportSheetProfileId =
  | "draftWebm"
  | "finalWebm"
  | "premiereXmeml"
  | "davinciFcpxml"
  | ExportProfile;

export interface ExportSheetProfile {
  id: ExportSheetProfileId;
  destination: ExportDestination;
  label: string;
  codec?: "h264" | "h265" | "prores" | "vp8";
  fileType: string;
  available: boolean;
  unavailableReason?: string;
  qualityAvailability?: ExportQualityAvailability;
  qualityUnavailableReasons?: ExportQualityUnavailableReasons;
  version?: string;
  target?: string;
}

export type ExportResolution = "match-timeline" | "hd" | "full-hd" | "4k";

export const resolutionOptions: ReadonlyArray<{
  id: ExportResolution;
  label: string;
  width?: number;
  height?: number;
}> = [
  { id: "match-timeline", label: "Match Timeline" },
  { id: "hd", label: "HD (up to 1280×720)", width: 1280, height: 720 },
  { id: "full-hd", label: "Full HD (1920×1080)", width: 1920, height: 1080 },
  { id: "4k", label: "4K (3840×2160)", width: 3840, height: 2160 },
];

export function defaultResolutionForQuality(
  quality: ExportQuality,
  timeline: { width: number; height: number },
  overridden: boolean,
  current: ExportResolution,
): ExportResolution {
  if (overridden) return current;
  if (quality === "final") return "match-timeline";
  return timeline.width > 1280 || timeline.height > 720 ? "hd" : "match-timeline";
}

export function dimensionsForResolution(
  resolution: ExportResolution,
  timeline: { width: number; height: number },
) {
  const preset = resolutionOptions.find((candidate) => candidate.id === resolution);
  if (resolution === "hd") {
    return draftExportDimensions(timeline.width, timeline.height);
  }
  return {
    width: preset?.width ?? evenDimension(timeline.width),
    height: preset?.height ?? evenDimension(timeline.height),
  };
}

export function evenDimension(value: number) {
  const dimension = Math.max(0, Math.floor(value));
  return dimension - (dimension % 2);
}

export function draftExportDimensions(width: number, height: number) {
  const scale = Math.min(1280 / width, 720 / height, 1);
  return {
    width: evenDimension(Math.round(width * scale)),
    height: evenDimension(Math.round(height * scale)),
  };
}

export function profileQualityAvailability(profile: ExportSheetProfile | null) {
  return (
    profile?.qualityAvailability ?? {
      draft: profile?.available ?? false,
      final: profile?.available ?? false,
    }
  );
}

export function isCapabilityCheckedVideoProfile(profile: ExportSheetProfile | null) {
  return profile?.destination === "video";
}

export function firstProfile(
  profiles: readonly ExportSheetProfile[],
  destination: ExportDestination,
) {
  const matching = profiles.filter((profile) => profile.destination === destination);
  return matching.find((profile) => profile.available) ?? matching[0] ?? null;
}

export function firstDraftVideoProfile(profiles: readonly ExportSheetProfile[]) {
  const availableDraftProfiles = profiles.filter((profile) => {
    const availability = profileQualityAvailability(profile);
    return profile.destination === "video" && profile.available && availability.draft;
  });
  return (
    availableDraftProfiles.find((profile) => profile.id === "draftWebm") ??
    availableDraftProfiles.find((profile) => profile.id === "webm") ??
    availableDraftProfiles[0] ??
    null
  );
}

export function formatDurationTimecode(durationSeconds: number, fps: number) {
  const safeDuration = Math.max(0, durationSeconds);
  const safeFps = Math.max(1, Math.round(fps));
  const wholeSeconds = Math.floor(safeDuration);
  const frames = Math.min(
    safeFps - 1,
    Math.floor((safeDuration - wholeSeconds) * safeFps),
  );
  const hours = Math.floor(wholeSeconds / 3600);
  const minutes = Math.floor((wholeSeconds % 3600) / 60);
  const seconds = wholeSeconds % 60;
  return [hours, minutes, seconds, frames]
    .map((value) => String(value).padStart(2, "0"))
    .join(":");
}

export function formatEstimatedSize(bytes: number | null | undefined) {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes) || bytes < 0) {
    return "—";
  }
  if (bytes < 1_000) return `${Math.round(bytes)} B`;
  if (bytes < 1_000_000) return `${(bytes / 1_000).toFixed(1)} KB`;
  if (bytes < 1_000_000_000) return `${(bytes / 1_000_000).toFixed(1)} MB`;
  return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
}
