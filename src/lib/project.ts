import { parseRenderAdmission, parseRenderAttempt, renderJobOperations } from "./runtime/render-job-contract";
import { backendRequest } from "@/lib/runtime/backend-client";
import type { EditJobRequest } from "./edit";
import type {
  RenderPreviewComparison,
  RenderPreviewComparisonRequest,
  RenderQualityProfile,
  RenderReport,
} from "./render";
import type {
  Timeline,
  TimelineItem,
  TimelinePatch,
  TimelineTrack,
  TimelineTransition,
  TransitionKind,
} from "./timeline";
import { applyDetachAudio } from "./timeline-ops/detach-audio";
import {
  applyClipReverse,
  isReversedItem,
  sourceSubrange,
  trimmedSourceRange,
  type SourceWindow,
} from "./timeline-ops/reverse";
import { maintainTransitionsAfterAction } from "./timeline-ops/transition-maintenance";
import { applyTransitionAction } from "./timeline-ops/transitions";

export type MediaKind = "video" | "audio" | "image" | "lottie" | "generated";

export interface MediaAsset {
  id: string;
  name?: string | null;
  relativePath: string;
  kind: MediaKind;
  durationSeconds: number;
  quality?: ExportQuality | null;
  requestedWidth?: number | null;
  requestedHeight?: number | null;
  actualWidth?: number | null;
  actualHeight?: number | null;
  width: number | null;
  height: number | null;
  fps: number | null;
  folderId?: string | null;
}

interface MediaAnalysisMoment {
  mediaId: string;
  sourceIn: number;
  sourceOut: number;
  visualActionScore?: number;
  audioEnergyScore?: number;
  label?: string;
}

interface ProjectMediaSilenceRange {
  mediaId: string;
  sourceIn: number;
  sourceOut: number;
  confidence: number;
  label?: string | null;
}

export interface MediaFolder {
  id: string;
  name: string;
  parentId: string | null;
}

export type ProjectMediaSearchScope = "visual" | "spoken" | "both" | "metadata" | "generated";
type ProjectMediaVisualStatus =
  | "ready"
  | "notInstalled"
  | "indexing"
  | "unavailable"
  | "failed";
type ProjectMediaSpokenStatus = "ready" | "noTranscripts" | "indexing";

interface ProjectMediaSearchResultItem {
  kind: "spoken" | "visual" | "metadata" | "generated";
  result: Record<string, unknown>;
}

export interface ProjectMediaSearchResult {
  query: string;
  limit: number;
  visualStatus: ProjectMediaVisualStatus;
  spokenStatus: ProjectMediaSpokenStatus;
  semanticEncoder?: {
    status: "installed" | "notInstalled" | string;
    model?: { id: string; version: string; dimensions: number } | null;
    manifestConfigured: boolean;
    licenseReviewed: boolean;
    hashVerified: boolean;
    message: string;
  };
  groups: {
    spoken: Record<string, unknown>[];
    visual: Record<string, unknown>[];
    metadata: Record<string, unknown>[];
    generated: Record<string, unknown>[];
  };
  results: ProjectMediaSearchResultItem[];
  returned: number;
  indexStatus?: {
    stored: boolean;
    source?: "stored" | "memory";
    reason?: "missing" | "stale" | "unavailable" | "notPersisted";
    schemaVersion: number;
    projectUpdatedAt: string;
    storedError?: string | null;
    storedSchemaVersion?: number | null;
    storedProjectUpdatedAt?: string | null;
  };
}

type GeneratedAssetStatus =
  | "queued"
  | "running"
  | "cancelled"
  | "failed"
  | "completed";
export type GenerationPlacementIntent = "library" | "timeline" | `replace:${string}`;

export interface GeneratedAsset {
  schemaVersion: number;
  id: string;
  kind: MediaKind;
  status: GeneratedAssetStatus;
  name?: string | null;
  targetFolderId?: string | null;
  placementIntent?: GenerationPlacementIntent | null;
  prompt: string;
  model: GenerationModel;
  references: GeneratedAssetReferences;
  settings: GeneratedAssetSettings;
  outputs: GeneratedAssetOutput[];
  createdAt: string;
  parentAssetId: string | null;
  retryOfAssetId: string | null;
}

interface GenerationModel {
  provider: string;
  id: string;
}

interface GeneratedAssetReferences {
  mediaIds: string[];
  sourceVideoMediaRef?: string | null;
  firstFrameMediaId: string | null;
  lastFrameMediaId: string | null;
  referenceImageMediaRefs?: string[];
  referenceVideoMediaRefs?: string[];
  referenceAudioMediaRefs?: string[];
  providerInputUrls?: string[];
}

export interface GeneratedAssetSettings {
  width: number | null;
  height: number | null;
  durationSeconds: number | null;
  fps: number | null;
  aspectRatio: string | null;
  resolution?: string | null;
  numImages?: number | null;
  quality?: string | null;
  generateAudio?: boolean | null;
  category?: string | null;
  voice?: string | null;
  lyrics?: string | null;
  styleInstructions?: string | null;
  instrumental?: boolean | null;
  videoSourceStartFrame?: number | null;
  videoSourceEndFrame?: number | null;
  videoSourceStartSeconds?: number | null;
  videoSourceEndSeconds?: number | null;
  timelineStartSeconds?: number | null;
}

interface GeneratedAssetOutput {
  mediaId: string;
  relativePath: string;
  sourceUrl?: string | null;
  width: number;
  height: number;
  durationSeconds: number;
  fps: number;
}

type RenderReportStatus = "queued" | "running" | "failed" | "completed";
type RenderReportCheckStatus = "passed" | "failed" | "skipped";

export interface ProjectRenderReport {
  schemaVersion: number;
  id: string;
  status: RenderReportStatus;
  outputPath: string;
  durationSeconds: number;
  streams: RenderReportStreams;
  checks: Record<string, RenderReportCheckStatus>;
  artifacts: string[];
  previewComparisonRequest?: RenderPreviewComparisonRequest | null;
  previewComparison?: RenderPreviewComparison | null;
  logPath: string;
  createdAt: string;
}

type ProjectExportArtifactKind =
  | "nle_xml"
  | "webm"
  | "mp4"
  | "mov"
  | "project_bundle";

export interface ProjectExportArtifact {
  schemaVersion: number;
  id: string;
  kind: ProjectExportArtifactKind;
  format: string;
  path: string;
  mimeType: string;
  jobId?: string | null;
  createdAt: string;
}

interface ProjectWebmRenderResult {
  project: VideoProject;
  renderReport: RenderReport;
  projectRenderReport: ProjectRenderReport;
  outputPath: string;
}

export interface ProjectMediaRenderResult {
  project: VideoProject;
  renderReport: RenderReport;
  projectRenderReport: ProjectRenderReport;
  outputPath: string;
  /** The saved export, when the render was given an output name and folder. */
  exportArtifact?: ProjectExportArtifact | null;
}

export interface ProjectPreviewRenderComparisonRunResult {
  project: VideoProject;
  renderReport: RenderReport;
  projectRenderReport: ProjectRenderReport;
  evidenceReport: string;
}

export type ExportProfile =
  | "webm"
  | "mp4H264"
  | "mp4H265"
  | "proResMov"
  | "palmierProject";
export type ExportQuality = "draft" | "final";
/** Master is the highest-quality encode of a Final MP4 or WebM export. */
export type ExportEncodeTier = "standard" | "master";

/** An export's file name and folder. No folder means the project's `exports/` folder. */
export interface ExportOutput {
  fileName: string;
  directory?: string | null;
}

/** Export settings recorded on an export job so Retry can restore them. */
export interface JobExportSettings {
  profile: Exclude<ExportProfile, "palmierProject">;
  quality: ExportQuality;
  width: number;
  height: number;
  fps?: number | null;
  encodeTier?: ExportEncodeTier;
  output?: ExportOutput | null;
}

/** Optional export settings accepted by both export commands. */
type ExportEncodeOptions = {
  /** Output frame rate; omitted renders at the timeline rate. */
  fps?: number;
  encodeTier?: ExportEncodeTier;
  output?: ExportOutput;
};
type ExportPolicyStatus =
  | "approved"
  | "missingRuntime"
  | "policyGated"
  | "unsupportedBuild";

export interface ExportQualityAvailability {
  draft: boolean;
  final: boolean;
}

export interface ExportQualityUnavailableReasons {
  draft?: string | null;
  final?: string | null;
}

export interface ExportProfileAvailability {
  profile: ExportProfile;
  label: string;
  available: boolean;
  container: string;
  extension: string;
  mimeType: string;
  videoCodec: string;
  audioCodec?: string | null;
  requiredRuntime: string[];
  policyStatus: ExportPolicyStatus;
  unavailableReason?: string | null;
  qualityAvailability: ExportQualityAvailability;
  qualityUnavailableReasons: ExportQualityUnavailableReasons;
}

interface RenderReportStreams {
  video: boolean;
  audio: boolean;
}

export type ProjectJobStatus =
  | "queued"
  | "running"
  | "progress"
  | "blocked"
  | "failed"
  | "cancelled"
  | "completed";

interface TemporalWorkflowMetadata {
  workflowId: string;
  workflowType: string;
  taskQueue: string;
  runId: string | null;
  activityTypes: string[];
}

export interface ProjectJobSummary {
  id: string;
  kind: string;
  status: ProjectJobStatus;
  updatedAt: string;
  workflow?: TemporalWorkflowMetadata | null;
  startRequest?: TemporalWorkflowStartRequest | null;
  providerRequest?: ProjectJobProviderRequest | null;
  /** Plain-language reason a failed job stopped (set by `recordJobFailure`). */
  failureReason?: string | null;
  exportSettings?: JobExportSettings | null;
}

interface ProjectJobProviderRequest {
  provider: string;
  requestId: string;
  statusUrl: string;
  responseUrl: string;
  cancelUrl: string;
  submittedAt: string;
}

export interface TemporalWorkflowStartRequest {
  workflowId: string;
  workflowType: string;
  taskQueue: string;
  input: Record<string, unknown>;
  searchAttributes: Record<string, unknown>;
  activityTypes: string[];
  idReusePolicy: string;
}

export interface TemporalWorkflowStartResult {
  status: "started" | "unavailable";
  workflowId: string;
  workflowType: string;
  taskQueue: string;
  runId: string | null;
  message: string;
}

interface TemporalWorkerToolStatus {
  name: string;
  available: boolean;
  path?: string | null;
  installHint?: string | null;
}

export interface TemporalWorkerEnvironmentReport {
  ready: boolean;
  featureEnabled: boolean;
  taskQueue: string;
  localServiceTarget: string;
  localWebUiUrl: string;
  localDevCommand: string;
  workerRunCommand: string;
  featureName: string;
  tools: TemporalWorkerToolStatus[];
}

export interface TemporalGenerateMediaBrief {
  name?: string | null;
  targetFolderId?: string | null;
  placementIntent?: GenerationPlacementIntent | null;
  prompt?: string;
  model?: GenerationModel;
  references?: GeneratedAssetReferences;
  settings?: GeneratedAssetSettings;
}

type TranscriptRepairKind =
  | "word_text"
  | "word_timing"
  | "word_text_and_timing";

interface TranscriptWord {
  text: string;
  startSeconds: number;
  endSeconds: number;
  confidence?: number | null;
  speaker?: string | null;
}

interface TranscriptSegment {
  text: string;
  startSeconds: number;
  endSeconds: number;
}

interface TranscriptRepair {
  id: string;
  kind: TranscriptRepairKind;
  wordIndex: number;
  before: TranscriptWord;
  after: TranscriptWord;
  createdAt: string;
}

export interface Transcript {
  id: string;
  mediaId: string;
  engine?: string | null;
  rawArtifactPath?: string | null;
  repairs: TranscriptRepair[];
  segments: TranscriptSegment[];
  words: TranscriptWord[];
}

export interface VideoProject {
  schemaVersion: number;
  contentRevision?: number;
  id: string;
  name: string;
  createdAt: string;
  updatedAt: string;
  media: MediaAsset[];
  mediaAnalysis?: MediaAnalysisMoment[];
  mediaSilenceRanges?: ProjectMediaSilenceRange[];
  mediaFolders?: MediaFolder[];
  generatedAssets: GeneratedAsset[];
  renderReports: ProjectRenderReport[];
  exportArtifacts?: ProjectExportArtifact[];
  transcripts: Transcript[];
  timelines?: ProjectTimeline[];
  activeTimelineId?: string | null;
  timeline: Timeline;
  renderSettings: {
    width: number;
    height: number;
    fps: number;
    loudnessLufs: number;
    captions: "burn_in" | "mux" | "off";
  };
  codexThreadId: string | null;
  jobs: ProjectJobSummary[];
}

export interface ProjectTimeline {
  id: string;
  name: string;
  timeline: Timeline;
}

const unfinishedJobStatuses: ReadonlySet<ProjectJobStatus> = new Set([
  "queued",
  "running",
  "progress",
  "blocked",
]);

function projectJobUpdatedAtMs(job: ProjectJobSummary) {
  const timestamp = Date.parse(job.updatedAt);
  return Number.isFinite(timestamp) ? timestamp : Number.NEGATIVE_INFINITY;
}

export function orderRecentProjectJobs(
  jobs: readonly ProjectJobSummary[],
  limit = 3,
): ProjectJobSummary[] {
  if (limit <= 0) {
    return [];
  }

  return jobs
    .map((job, index) => ({ job, index, updatedAtMs: projectJobUpdatedAtMs(job) }))
    .sort((left, right) => {
      const updatedAtDifference = right.updatedAtMs - left.updatedAtMs;
      return updatedAtDifference === 0 ? left.index - right.index : updatedAtDifference;
    })
    .slice(0, limit)
    .map(({ job }) => job);
}

interface ImportSkippedFile {
  sourcePath: string;
  reason: string;
}

export interface ImportMediaResult {
  project: VideoProject;
  imported: MediaAsset[];
  skipped: ImportSkippedFile[];
}

export interface ProjectWriteReport {
  manifestPath: string;
  writtenFiles: string[];
  removedFiles: string[];
  recoveryPending?: boolean;
}

export interface ProjectActionWriteResult {
  project: VideoProject;
  report: ProjectWriteReport;
}

export type NleXmlExportFormat = "premiereXmeml" | "davinciFcpxml";

export interface NleXmlExportCommandResult {
  project: VideoProject;
  exportPath: string;
  job: ProjectJobSummary;
}

export interface CodexEditProposal {
  mediaId: string;
  clips: CodexProposalClip[];
  captions: unknown[];
  overlays: unknown[];
  hyperframes: unknown[];
  gpuVisuals: unknown[];
  projectActions: ProjectAction[];
  renderReview: CodexRenderReview;
}

interface CodexProposalClip {
  mediaId?: string;
  sourceIn: number;
  sourceOut: number;
  reason: string;
}

interface CodexRenderReview {
  durationSeconds: number;
  streamCheckRequired: boolean;
  captionAlignmentRequired: boolean;
  overlayTimingRequired: boolean;
  visualFrameEvidenceRequired: boolean;
  artifactPathsRequired: boolean;
  logReferenceRequired: boolean;
}

export interface CodexVideoEditCommandResult {
  project: VideoProject;
  threadId: string;
  threadResponse: unknown;
  turnResponse: unknown;
  proposal: CodexEditProposal | null;
  proposalValidationIssues: ProjectValidationIssue[] | null;
}

export interface CodexConversationRange {
  startSeconds: number;
  endSeconds: number;
}

export interface CodexConversationFocus {
  primaryMediaId?: string | null;
  mediaIds: string[];
  timelineItemIds: string[];
  timelineRange?: CodexConversationRange | null;
}

/** Preset-free conversation request: the user's words plus an internal focus. */
export interface CodexConversationEditRequest {
  prompt: string;
  focus: CodexConversationFocus;
  createdAt: string;
}

export interface CodexConversationEditProposal {
  summary: string;
  edl: CodexProposalClip[];
  projectActions: ProjectAction[];
  renderReview: CodexRenderReview | null;
}

export type CodexProposalRiskLevel = "safe" | "review";

export interface CodexProposalRisk {
  level: CodexProposalRiskLevel;
  reasons: Array<{ code: string; message: string }>;
}

export interface CodexProposalImpact {
  summary: string;
  beforeDurationSeconds: number;
  afterDurationSeconds: number;
  affectedItemIds: string[];
  affectedRanges: CodexConversationRange[];
  previewTimestamp: number;
}

/** The exact bundle Rust validated, materialized, and classified. */
export interface CodexPreparedProposal {
  actions: ProjectAction[];
  actionIds: string[];
  risk: CodexProposalRisk;
  impact: CodexProposalImpact;
}

export interface CodexConversationEditCommandResult {
  project: VideoProject;
  threadId: string;
  threadResponse: unknown;
  turnResponse: unknown;
  proposal: CodexConversationEditProposal | null;
  /** Null whenever validation fails; the only bundle the client may apply. */
  preparedProposal: CodexPreparedProposal | null;
  proposalValidationIssues: ProjectValidationIssue[] | null;
}

/** Result of a guarded, atomic conversation apply; risk and impact are recomputed in Rust. */
export interface ProjectAgentApplyResult {
  project: VideoProject;
  report: ProjectWriteReport;
  /** Pass to `undoLatestCodexConversationEdit` to bind Undo to this result. */
  historyEntryId: string;
  actionIds: string[];
  risk: CodexProposalRisk;
  impact: CodexProposalImpact;
  /** Session bookkeeping problems after the edit committed; the edit still applied. */
  warnings: string[];
}

/** Undo either restores the pre-apply snapshot or explains, in plain language, why it cannot. */
export type ProjectAgentUndoOutcome =
  | {
      status: "undone";
      project: VideoProject;
      report: ProjectWriteReport;
      entryId: string;
      actionCount: number;
      remainingAgentHistory: number;
      warnings: string[];
      /**
       * Generated assets the undone project no longer has: the generations the batch recorded (with
       * the output media completion added). The backend already cancelled the ones still running.
       */
      removedGeneratedAssetIds: string[];
    }
  | { status: "conflict"; entryId: string; message: string }
  | { status: "unavailable"; message: string };

export interface AppServerConversationEntry {
  id: string;
  projectId: string;
  threadId: string;
  turnId?: string | null;
  turnStatus?: string | null;
  prompt: string;
  // Stored as submitted: legacy one-click entries hold an EditJobRequest.
  request: EditJobRequest | CodexConversationEditRequest | Record<string, unknown>;
  hasProposal: boolean;
  threadResponse: unknown;
  turnResponse: unknown;
  /** The chat session this turn was filed under. Entries written before 2026-09-16 have none. */
  sessionId?: string | null;
}

export interface AppServerConversationHistory {
  schemaVersion: number;
  entries: AppServerConversationEntry[];
}

interface ProjectAgentSessionTurn {
  id: string;
  createdAt: string;
  fullTurn: unknown;
  toolResults: unknown[];
  proposalStatus: string;
  appliedActionIds: string[];
}

export interface ProjectAgentSession {
  id: string;
  title: string;
  threadId?: string | null;
  /** Which agent this chat last talked to. Absent on chats written before backends were pluggable. */
  provider?: string | null;
  /** The provider's own session handle, when the backend has one of its own. */
  providerSessionId?: string | null;
  createdAt: string;
  updatedAt: string;
  turns: ProjectAgentSessionTurn[];
  proposalStatus: string;
  appliedActionIds: string[];
}

export interface ProjectAgentSessionManifest {
  schemaVersion: number;
  projectId: string;
  activeSessionId?: string | null;
  sessions: ProjectAgentSession[];
  deletedSessions: ProjectAgentSession[];
}

export type ProjectAgentSessionAction =
  | { type: "create"; id: string; title: string; threadId?: string | null; timestamp: string }
  | { type: "select"; sessionId: string }
  | { type: "rename"; sessionId: string; title: string; timestamp: string }
  | { type: "delete"; sessionId: string; timestamp: string }
  | { type: "restore"; sessionId: string; timestamp: string };

export interface ProjectValidationIssue {
  path: string;
  message: string;
  fix: string;
}

export interface ProjectValidationReport {
  ok: boolean;
  issues: ProjectValidationIssue[];
}

export type UpdateProjectSettingsAction = {
  type: "updateProjectSettings";
  name: string;
  renderSettings: VideoProject["renderSettings"];
};

export type ProjectAction =
  | {
      type: "addItems";
      targetTrackId: string;
      items: readonly TimelineItem[];
    }
  | {
      type: "insertItems";
      targetTrackId: string;
      insertSeconds: number;
      items: readonly TimelineItem[];
    }
  | {
      type: "removeItems";
      itemIds: readonly string[];
    }
  | {
      type: "moveItems";
      moves: readonly ProjectActionMove[];
    }
  | {
      type: "reorderItems";
      reorder: ProjectActionReorder;
    }
  | {
      type: "resizeItems";
      resizes: readonly ProjectActionResize[];
    }
  | {
      type: "trimItems";
      trims: readonly ProjectActionTrim[];
    }
  | ProjectActionRippleTrim
  | {
      type: "rippleDeleteRanges";
      ranges: readonly ProjectActionRippleDeleteRange[];
    }
  | {
      type: "splitItems";
      splits: readonly ProjectActionSplit[];
    }
  | {
      type: "createTrack";
      track: TimelineTrack;
      afterTrackId?: string;
    }
  | {
      type: "createTimeline";
      timelineId: string;
      name: string;
      duplicateActive: boolean;
      sourceTimelineId?: string;
    }
  | {
      type: "setActiveTimeline";
      timelineId: string;
    }
  | {
      type: "renameTimeline";
      timelineId: string;
      name: string;
    }
  | {
      type: "deleteTimeline";
      timelineId: string;
    }
  | {
      type: "decomposeTimelineItem";
      itemId: string;
    }
  | ProjectActionRemoveTracks
  | {
      type: "setTrackLocked";
      trackId: string;
      locked: boolean;
    }
  | {
      type: "reorderTrack";
      trackId: string;
      targetTrackId: string;
      placement: "before" | "after";
    }
  | {
      type: "setTrackSyncLocked";
      trackId: string;
      syncLocked: boolean;
    }
  | {
      type: "setTrackEnabled";
      trackId: string;
      enabled: boolean;
    }
  | {
      type: "editCaptionText";
      itemId: string;
      text: string;
    }
  | {
      type: "editTextItem";
      itemId: string;
      text: string;
    }
  | {
      type: "updateAudioFadeOut";
      itemId: string;
      fadeOutSeconds: number;
    }
  | {
      type: "updateAudioFades";
      itemId: string;
      fadeInSeconds: number;
      fadeOutSeconds: number;
    }
  | {
      type: "updateAudioVolume";
      itemId: string;
      volumeDb: number | null;
    }
  | {
      type: "updateVisualClipOpacity";
      itemId: string;
      opacity: number;
    }
  | {
      type: "updateVisualClipTransform";
      itemId: string;
      transform: ProjectActionVisualTransform;
    }
  | {
      type: "updateVisualClipCrop";
      itemId: string;
      crop: ProjectActionVisualCrop;
    }
  | {
      type: "updateVisualClipFades";
      itemId: string;
      fadeInSeconds: number;
      fadeOutSeconds: number;
    }
  | {
      type: "updateVisualClipSpeed";
      itemId: string;
      speed: number;
    }
  | {
      type: "updateAudioClipSpeed";
      itemId: string;
      speed: number;
    }
  | {
      type: "detachAudio";
      itemId: string;
      audioItemId: string;
      targetTrackId: string;
      linkGroupId: string;
    }
  | {
      type: "updateClipReverse";
      itemId: string;
      reverse: boolean;
    }
  | {
      type: "setItemKeyframes";
      itemId: string;
      property: ProjectActionKeyframeProperty;
      keyframes: readonly ProjectActionKeyframe[];
    }
  | {
      type: "upsertItemKeyframe";
      itemId: string;
      property: ProjectActionKeyframeProperty;
      keyframe: ProjectActionKeyframe;
    }
  | {
      type: "moveItemKeyframe";
      itemId: string;
      property: ProjectActionKeyframeProperty;
      fromSeconds: number;
      toSeconds: number;
    }
  | {
      type: "deleteItemKeyframe";
      itemId: string;
      property: ProjectActionKeyframeProperty;
      atSeconds: number;
    }
  | {
      type: "upsertEffectParameterKeyframe";
      itemId: string;
      effectInstanceId: string;
      parameterKey: string;
      keyframe: ProjectActionKeyframe;
    }
  | {
      type: "moveEffectParameterKeyframe";
      itemId: string;
      effectInstanceId: string;
      parameterKey: string;
      fromSeconds: number;
      toSeconds: number;
    }
  | {
      type: "deleteEffectParameterKeyframe";
      itemId: string;
      effectInstanceId: string;
      parameterKey: string;
      atSeconds: number;
    }
  | {
      type: "updateItemProperties";
      updates: readonly ProjectActionItemPropertiesUpdate[];
    }
  | {
      type: "linkItems";
      itemIds: readonly string[];
      linkGroupId: string;
    }
  | {
      type: "unlinkItems";
      itemIds: readonly string[];
    }
  | {
      type: "updateItemEffects";
      itemIds: readonly string[];
      effects: readonly ProjectActionEffect[];
    }
  | {
      type: "updateItemColorGrade";
      itemIds: readonly string[];
      reset: boolean;
      grade: ProjectActionColorGrade;
    }
  | {
      type: "applyCaptionRepair";
      repair: ProjectActionCaptionRepair;
    }
  | {
      type: "editTranscriptWords";
      edits: readonly ProjectActionTranscriptWordEdit[];
    }
  | {
      type: "recordGeneratedAsset";
      asset: ProjectActionGeneratedAsset;
    }
  | {
      type: "updateGeneratedAssetStatus";
      assetId: string;
      status: GeneratedAssetStatus;
    }
  | {
      type: "updateGeneratedAssetReferences";
      assetId: string;
      references: GeneratedAssetReferences;
    }
  | {
      type: "completeGeneratedAsset";
      assetId: string;
      outputs: readonly GeneratedAssetOutput[];
      completion?: ProjectActionGeneratedAssetCompletion | null;
      replacement?: ProjectActionReplaceGeneratedOutput | null;
    }
  | {
      type: "replaceTimelineItemWithGeneratedOutput";
      replacement: ProjectActionReplaceGeneratedOutput;
    }
  | {
      type: "assignMediaFolder";
      mediaId: string;
      folderId: string | null;
    }
  | {
      type: "createMediaFolder";
      folder: MediaFolder;
    }
  | {
      type: "renameMediaFolder";
      folderId: string;
      name: string;
    }
  | {
      type: "deleteMediaFolder";
      folderId: string;
    }
  | {
      type: "renameMedia";
      mediaId: string;
      name: string;
    }
  | {
      /** Removes the media, its transcripts and its clips on the active timeline. */
      type: "deleteMedia";
      mediaIds: readonly string[];
    }
  | UpdateProjectSettingsAction
  | {
      type: "attachRenderReport";
      report: ProjectRenderReport;
    }
  | {
      type: "recordExportArtifact";
      artifact: ProjectExportArtifact;
    }
  | {
      type: "recordJob";
      job: ProjectJobSummary;
    }
  | {
      type: "updateJobStatus";
      jobId: string;
      status: ProjectJobStatus;
      updatedAt: string;
      runId?: string | null;
    }
  | {
      type: "updateJobProviderRequest";
      jobId: string;
      providerRequest: ProjectJobProviderRequest;
    }
  | {
      type: "recordJobFailure";
      jobId: string;
      reason: string;
      updatedAt: string;
      runId?: string | null;
    }
  | {
      type: "updateTemplateItems";
      updates: readonly ProjectActionTemplateUpdate[];
    }
  | {
      type: "updateTextOverlayItems";
      updates: readonly ProjectActionTextOverlayUpdate[];
    }
  | {
      type: "updateTemplateOverride";
      override: ProjectActionTemplateOverrideUpdate;
    }
  | {
      type: "addTransition";
      trackId: string;
      transition: TimelineTransition;
    }
  | {
      type: "updateTransition";
      trackId: string;
      transitionId: string;
      kind?: TransitionKind;
      durationSeconds?: number;
    }
  | {
      type: "removeTransition";
      trackId: string;
      transitionId: string;
    };

/** Mirrors `ProjectAction::RemoveTracks` in `src-tauri/src/project/action.rs`. */
interface ProjectActionRemoveTracks {
  type: "removeTracks";
  trackIds: readonly string[];
}

interface ProjectActionMove {
  itemId: string;
  targetTrackId: string;
  startSeconds: number;
}

interface ProjectActionReorder {
  targetTrackId: string;
  itemIds: readonly string[];
  startSeconds: number;
  gapSeconds: number;
}

interface ProjectActionResize {
  itemId: string;
  durationSeconds: number;
}

interface ProjectActionTrim {
  itemId: string;
  startSeconds: number;
  durationSeconds: number;
  sourceIn?: number;
  sourceOut?: number;
}

export interface ProjectActionRippleTrim {
  type: "rippleTrimItem";
  itemId: string;
  edge: "left" | "right";
  deltaSeconds: number;
  propagateLinked: boolean;
  syncLockedTrackIds: readonly string[];
}

export interface ProjectActionRippleTrimPreviewPlan {
  durationDeltaSeconds: number;
  resizes: readonly {
    itemId: string;
    durationSeconds: number;
    sourceIn?: number;
    sourceOut?: number;
  }[];
  shifts: readonly { itemId: string; startSeconds: number }[];
  affectedTrackIds: readonly string[];
}

export interface ProjectActionRippleDeleteRange {
  startSeconds: number;
  endSeconds: number;
  trackIds: readonly string[];
}

interface ProjectActionSplit {
  itemId: string;
  newItemId: string;
  splitSeconds: number;
}

export interface ProjectActionVisualTransform {
  centerX?: number;
  centerY?: number;
  width?: number;
  height?: number;
  flipHorizontal?: boolean;
  flipVertical?: boolean;
}

export interface ProjectActionVisualCrop {
  cropTop?: number;
  cropRight?: number;
  cropBottom?: number;
  cropLeft?: number;
}

export type ProjectActionKeyframeProperty =
  | "opacity"
  | "volumeDb"
  | "positionX"
  | "positionY"
  | "scale"
  | "scaleX"
  | "scaleY"
  | "rotationDegrees"
  | "cropTop"
  | "cropRight"
  | "cropBottom"
  | "cropLeft";

export interface ProjectActionKeyframe {
  atSeconds: number;
  value: number;
  easing?: ProjectActionKeyframeEasing | null;
}

export type CanonicalProjectActionKeyframeEasing =
  | "linear"
  | "hold"
  | "easeIn"
  | "easeOut"
  | "easeInOut";

type ProjectActionKeyframeEasing =
  | CanonicalProjectActionKeyframeEasing
  | "smooth";

function canonicalProjectActionKeyframeEasing(
  easing: ProjectActionKeyframeEasing | null | undefined,
): CanonicalProjectActionKeyframeEasing | undefined {
  if (easing == null) return undefined;
  switch (easing) {
    case "linear":
    case "hold":
    case "easeIn":
    case "easeOut":
    case "easeInOut":
      return easing;
    case "smooth":
      return "easeInOut";
    default:
      throw new Error(`Unsupported keyframe easing ${String(easing)}.`);
  }
}

function canonicalizeProjectActionKeyframes(
  keyframes: readonly ProjectActionKeyframe[],
): ProjectActionKeyframe[] {
  const canonical = keyframes
    .map((keyframe) => {
      if (!Number.isFinite(keyframe.atSeconds) || !Number.isFinite(keyframe.value)) {
        throw new Error("Keyframe time and value must be finite numbers.");
      }
      const easing = canonicalProjectActionKeyframeEasing(keyframe.easing);
      return {
        atSeconds: keyframe.atSeconds,
        value: keyframe.value,
        ...(easing ? { easing } : {}),
      };
    })
    .sort((left, right) => left.atSeconds - right.atSeconds);
  for (let index = 1; index < canonical.length; index += 1) {
    const previous = canonical[index - 1];
    const current = canonical[index];
    if (previous && current && previous.atSeconds === current.atSeconds) {
      throw new Error(`Duplicate keyframe timestamp ${current.atSeconds}.`);
    }
  }
  return canonical;
}

interface ProjectActionItemPropertiesUpdate {
  itemId: string;
  set: Record<string, unknown>;
  remove: readonly string[];
}

export interface ProjectActionEffect {
  effectInstanceId: string;
  effectType: string;
  enabled: boolean;
  params: Record<string, unknown>;
}

export function legacyEffectInstanceId(effectType: string, occurrence: number): string {
  const slug = effectType.trim().replace(/[^A-Za-z0-9._-]/g, "-");
  return `legacy:${slug}:${occurrence + 1}`;
}

export function canonicalizeProjectActionEffects(
  effects: readonly (Omit<ProjectActionEffect, "effectInstanceId"> & {
    effectInstanceId?: string;
  })[],
): ProjectActionEffect[] {
  const occurrences = new Map<string, number>();
  const instanceIds = new Set<string>();
  return effects.map((effect) => {
    const occurrence = occurrences.get(effect.effectType) ?? 0;
    occurrences.set(effect.effectType, occurrence + 1);
    const effectInstanceId = effect.effectInstanceId?.trim()
      || legacyEffectInstanceId(effect.effectType, occurrence);
    if (
      effectInstanceId.length > 160
      || !/^[A-Za-z0-9._:-]+$/.test(effectInstanceId)
    ) {
      throw new Error(`Effect instance id ${effectInstanceId} is invalid.`);
    }
    if (instanceIds.has(effectInstanceId)) {
      throw new Error(`Duplicate effect instance id ${effectInstanceId}.`);
    }
    instanceIds.add(effectInstanceId);
    return { ...effect, effectInstanceId };
  });
}

interface VisualEffectParamDescriptor {
  key: string;
  label: string;
  min: number;
  max: number;
  defaultValue: number;
  unit: string;
}

export interface VisualEffectDescriptor {
  id: string;
  displayName: string;
  category: string;
  params: VisualEffectParamDescriptor[];
  resourceKey?: string | null;
  colorEffect: boolean;
  controlSchema?: Record<string, unknown> | null;
}

export interface VisualEffectCatalog {
  source: string;
  effectCount: number;
  canonicalOrder: string[];
  effects: VisualEffectDescriptor[];
}

export async function listVisualEffectCatalog(): Promise<VisualEffectCatalog> {
  return backendRequest("list_visual_effect_catalog");
}

export interface ProjectActionColorGrade {
  exposure?: number | null;
  contrast?: number | null;
  saturation?: number | null;
  temperature?: number | null;
  tint?: number | null;
  vibrance?: number | null;
  highlights?: number | null;
  shadows?: number | null;
  blacks?: number | null;
  whites?: number | null;
  shadowsHue?: number | null;
  shadowsAmount?: number | null;
  shadowsLum?: number | null;
  midsHue?: number | null;
  midsAmount?: number | null;
  midsGamma?: number | null;
  highsHue?: number | null;
  highsAmount?: number | null;
  highsGain?: number | null;
  masterCurve?: number[][] | null;
  redCurve?: number[][] | null;
  greenCurve?: number[][] | null;
  blueCurve?: number[][] | null;
  hueCurves?: Record<string, unknown> | null;
  lut?: Record<string, unknown> | null;
}

interface ProjectActionTemplateUpdate {
  itemId: string;
  startSeconds: number;
  durationSeconds: number;
  templateFields: Record<string, string>;
}

interface ProjectActionTextOverlayUpdate {
  itemId: string;
  startSeconds: number;
  durationSeconds: number;
  text: string;
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
}

export interface ProjectActionTemplateOverrideUpdate {
  templateId: string;
  name: string;
  fields: Record<string, string>;
  style: Record<string, unknown>;
  visualTreatment: string;
  motion: string;
  safeZone: string;
  avoid: string;
}

interface ProjectActionCaptionRepair {
  captionItemId: string;
  transcriptId: string;
  wordIndex: number;
  text: string;
  startSeconds: number;
  endSeconds: number;
  repairId: string;
  createdAt: string;
}

interface ProjectActionTranscriptWordEdit {
  transcriptId: string;
  wordIndex: number;
  text?: string;
  startSeconds?: number;
  endSeconds?: number;
  repairId: string;
  createdAt: string;
}

interface ProjectActionGeneratedAsset {
  id: string;
  kind: MediaKind;
  status: GeneratedAssetStatus;
  name?: string | null;
  targetFolderId?: string | null;
  placementIntent?: GenerationPlacementIntent | null;
  prompt: string;
  model: GenerationModel;
  references: GeneratedAssetReferences;
  settings: GeneratedAssetSettings;
  outputs: readonly GeneratedAssetOutput[];
  createdAt: string;
  parentAssetId: string | null;
  retryOfAssetId: string | null;
}

interface ProjectActionReplaceGeneratedOutput {
  itemId: string;
  mediaId: string;
}

interface ProjectActionGeneratedAssetCompletion {
  generatedAssetId: string;
  generatedOutputMediaId: string;
  placementIntent: GenerationPlacementIntent | "library" | "timeline" | string;
  references: GeneratedAssetReferences;
}

export function projectActionFromTimelinePatch(patch: TimelinePatch): ProjectAction {
  switch (patch.type) {
    case "moveItem":
      return {
        type: "moveItems",
        moves: [
          {
            itemId: patch.itemId,
            targetTrackId: patch.targetTrackId,
            startSeconds: patch.startSeconds,
          },
        ],
      };
    case "resizeItem":
      return {
        type: "resizeItems",
        resizes: [
          {
            itemId: patch.itemId,
            durationSeconds: patch.durationSeconds,
          },
        ],
      };
    case "trimItem":
      return {
        type: "trimItems",
        trims: [
          {
            itemId: patch.itemId,
            startSeconds: patch.startSeconds,
            durationSeconds: patch.durationSeconds,
            ...(patch.sourceIn !== undefined ? { sourceIn: patch.sourceIn } : {}),
            ...(patch.sourceOut !== undefined ? { sourceOut: patch.sourceOut } : {}),
          },
        ],
      };
    case "editCaptionText":
      return {
        type: "editCaptionText",
        itemId: patch.itemId,
        text: patch.text,
      };
  }
}

function sortTimelineItemsByStart(items: readonly TimelineItem[]): TimelineItem[] {
  return [...items].sort((left, right) => left.startSeconds - right.startSeconds);
}

function timelineDurationSeconds(tracks: readonly TimelineTrack[]): number {
  return tracks
    .flatMap((track) => track.items)
    .map((item) => item.startSeconds + item.durationSeconds)
    .reduce((duration, itemEnd) => Math.max(duration, itemEnd), 0);
}

function projectWithTimelineTracks(
  project: VideoProject,
  tracks: readonly TimelineTrack[],
): VideoProject {
  const timeline = {
    ...project.timeline,
    durationSeconds: timelineDurationSeconds(tracks),
    tracks: tracks.map((track) => ({
      ...track,
      items: sortTimelineItemsByStart(track.items),
    })),
  };
  return withActiveTimeline(project, timeline);
}

function withActiveTimeline(project: VideoProject, timeline: Timeline): VideoProject {
  const activeTimelineId = project.activeTimelineId ?? "main";
  const timelines = project.timelines?.length
    ? project.timelines.map((entry) =>
        entry.id === activeTimelineId ? { ...entry, timeline } : entry,
      )
    : [{ id: activeTimelineId, name: "Timeline 1", timeline }];
  return {
    ...project,
    timeline,
    timelines,
    activeTimelineId,
  };
}

function generatedAssetFromProjectAction(
  asset: ProjectActionGeneratedAsset,
): GeneratedAsset {
  return {
    ...asset,
    schemaVersion: 1,
    outputs: [...asset.outputs],
  };
}

function updateTimelineItem(
  project: VideoProject,
  itemId: string,
  update: (item: TimelineItem) => TimelineItem,
): VideoProject {
  let changed = false;
  const tracks = project.timeline.tracks.map((track) => {
    let trackChanged = false;
    const items = track.items.map((item) => {
      if (item.id !== itemId) {
        return item;
      }

      const nextItem = update(item);
      if (nextItem !== item) {
        trackChanged = true;
      }
      return nextItem;
    });

    if (!trackChanged) {
      return track;
    }

    changed = true;
    return { ...track, items: sortTimelineItemsByStart(items) };
  });

  if (!changed) {
    return project;
  }

  return projectWithTimelineTracks(project, tracks);
}

function updateTimelineItemsById(
  project: VideoProject,
  itemIds: Set<string>,
  update: (item: TimelineItem) => TimelineItem,
): VideoProject {
  let changed = false;
  const tracks = project.timeline.tracks.map((track) => {
    let trackChanged = false;
    const items = track.items.map((item) => {
      if (!itemIds.has(item.id)) {
        return item;
      }

      const nextItem = update(item);
      if (nextItem !== item) {
        trackChanged = true;
      }
      return nextItem;
    });

    if (!trackChanged) {
      return track;
    }

    changed = true;
    return { ...track, items: sortTimelineItemsByStart(items) };
  });

  return changed ? projectWithTimelineTracks(project, tracks) : project;
}

function itemKeyframes(
  item: TimelineItem,
  property: ProjectActionKeyframeProperty,
): ProjectActionKeyframe[] {
  const keyframes = item.properties.keyframes;
  if (!keyframes || typeof keyframes !== "object" || Array.isArray(keyframes)) return [];
  const propertyKeyframes = (keyframes as Record<string, unknown>)[property];
  return Array.isArray(propertyKeyframes)
    ? canonicalizeProjectActionKeyframes(propertyKeyframes as ProjectActionKeyframe[])
    : [];
}

function itemWithKeyframes(
  item: TimelineItem,
  property: ProjectActionKeyframeProperty,
  keyframes: readonly ProjectActionKeyframe[],
): TimelineItem {
  const properties = { ...item.properties };
  const existing =
    properties.keyframes &&
    typeof properties.keyframes === "object" &&
    !Array.isArray(properties.keyframes)
      ? { ...(properties.keyframes as Record<string, unknown>) }
      : {};
  const canonical = canonicalizeProjectActionKeyframes(keyframes);
  if (property === "volumeDb" && item.kind !== "audio_clip") {
    throw new Error(`Keyframe property volumeDb requires an audio clip.`);
  }
  if (
    property !== "volumeDb" &&
    item.kind !== "video_clip" &&
    item.kind !== "image_clip" &&
    item.kind !== "lottie_clip" &&
    item.kind !== "generated_clip" &&
    item.kind !== "overlay" &&
    item.kind !== "hyperframe_scene"
  ) {
    throw new Error(`Keyframe property ${property} requires a visual clip.`);
  }
  const valueBounds: Record<ProjectActionKeyframeProperty, readonly [number, number]> = {
    opacity: [0, 1],
    volumeDb: [-60, 24],
    positionX: [-10_000, 10_000],
    positionY: [-10_000, 10_000],
    scale: [0.01, 100],
    scaleX: [0.01, 100],
    scaleY: [0.01, 100],
    rotationDegrees: [-360, 360],
    cropTop: [0, 1],
    cropRight: [0, 1],
    cropBottom: [0, 1],
    cropLeft: [0, 1],
  };
  const [minimum, maximum] = valueBounds[property];
  for (const keyframe of canonical) {
    if (keyframe.atSeconds < 0 || keyframe.atSeconds > item.durationSeconds) {
      throw new Error(`Keyframe time must be inside item ${item.id}.`);
    }
    if (keyframe.value < minimum || keyframe.value > maximum) {
      throw new Error(`Keyframe value must be between ${minimum} and ${maximum}.`);
    }
  }
  if (canonical.length > 0) existing[property] = canonical;
  else delete existing[property];
  if (Object.keys(existing).length > 0) properties.keyframes = existing;
  else delete properties.keyframes;
  return { ...item, properties };
}

function effectParameterKeyframes(
  item: TimelineItem,
  effectInstanceId: string,
  parameterKey: string,
): ProjectActionKeyframe[] {
  const lanes = item.properties.effectParameterKeyframes;
  if (!lanes || typeof lanes !== "object" || Array.isArray(lanes)) return [];
  const instance = (lanes as Record<string, unknown>)[effectInstanceId];
  if (!instance || typeof instance !== "object" || Array.isArray(instance)) return [];
  const keyframes = (instance as Record<string, unknown>)[parameterKey];
  return Array.isArray(keyframes)
    ? canonicalizeProjectActionKeyframes(keyframes as ProjectActionKeyframe[])
    : [];
}

function itemWithEffectParameterKeyframes(
  item: TimelineItem,
  effectInstanceId: string,
  parameterKey: string,
  keyframes: readonly ProjectActionKeyframe[],
): TimelineItem {
  if (!/^[A-Za-z0-9._:-]+$/.test(effectInstanceId)) {
    throw new Error(`Effect instance id ${effectInstanceId} is invalid.`);
  }
  if (!/^[A-Za-z0-9_-]+$/.test(parameterKey)) {
    throw new Error(`Effect parameter key ${parameterKey} is invalid.`);
  }
  const effects = canonicalizeProjectActionEffects(
    Array.isArray(item.properties.effects)
      ? item.properties.effects as Array<Omit<ProjectActionEffect, "effectInstanceId"> & { effectInstanceId?: string }>
      : [],
  );
  const effect = effects.find((candidate) => candidate.effectInstanceId === effectInstanceId);
  if (!effect) {
    throw new Error(`Effect instance ${effectInstanceId} was not found on ${item.id}.`);
  }
  if (typeof effect.params[parameterKey] !== "number") {
    throw new Error(`Effect parameter ${effect.effectType}.${parameterKey} is not numeric.`);
  }
  const canonical = canonicalizeProjectActionKeyframes(keyframes);
  for (const keyframe of canonical) {
    if (keyframe.atSeconds < 0 || keyframe.atSeconds > item.durationSeconds) {
      throw new Error(`Keyframe time must be inside item ${item.id}.`);
    }
  }
  const properties: TimelineItem["properties"] = { ...item.properties, effects };
  const allLanes =
    properties.effectParameterKeyframes
    && typeof properties.effectParameterKeyframes === "object"
    && !Array.isArray(properties.effectParameterKeyframes)
      ? { ...(properties.effectParameterKeyframes as Record<string, unknown>) }
      : {};
  const instanceLanes =
    allLanes[effectInstanceId]
    && typeof allLanes[effectInstanceId] === "object"
    && !Array.isArray(allLanes[effectInstanceId])
      ? { ...(allLanes[effectInstanceId] as Record<string, unknown>) }
      : {};
  if (canonical.length > 0) instanceLanes[parameterKey] = canonical;
  else delete instanceLanes[parameterKey];
  if (Object.keys(instanceLanes).length > 0) allLanes[effectInstanceId] = instanceLanes;
  else delete allLanes[effectInstanceId];
  if (Object.keys(allLanes).length > 0) properties.effectParameterKeyframes = allLanes;
  else delete properties.effectParameterKeyframes;
  return { ...item, properties };
}

export function planProjectRippleTrim(
  project: VideoProject,
  action: ProjectActionRippleTrim,
): ProjectActionRippleTrimPreviewPlan {
  if (!Number.isFinite(action.deltaSeconds) || action.deltaSeconds === 0) {
    throw new Error("Ripple trim delta must be a finite non-zero number.");
  }
  const locations = new Map(
    project.timeline.tracks.flatMap((track) =>
      track.items.map((item) => [item.id, { track, item }] as const),
    ),
  );
  const lead = locations.get(action.itemId);
  if (!lead) throw new Error(`Timeline item ${action.itemId} was not found.`);
  const durationDeltaSeconds =
    action.edge === "left" ? -action.deltaSeconds : action.deltaSeconds;
  const linkGroupId =
    typeof lead.item.properties.linkGroupId === "string"
      ? lead.item.properties.linkGroupId
      : null;
  const targetIds = new Set([action.itemId]);
  if (action.propagateLinked && linkGroupId) {
    for (const { item } of locations.values()) {
      if (item.properties.linkGroupId === linkGroupId) targetIds.add(item.id);
    }
  }
  const boundaries = new Map<string, number>();
  for (const targetId of targetIds) {
    const target = locations.get(targetId);
    if (!target) throw new Error(`Timeline item ${targetId} was not found.`);
    if (target.track.locked) throw new Error(`Track ${target.track.id} is locked.`);
    const end = target.item.startSeconds + target.item.durationSeconds;
    boundaries.set(target.track.id, Math.min(boundaries.get(target.track.id) ?? end, end));
  }
  const seenSyncTracks = new Set<string>();
  const leadEnd = lead.item.startSeconds + lead.item.durationSeconds;
  for (const trackId of action.syncLockedTrackIds) {
    if (!trackId || seenSyncTracks.has(trackId)) {
      throw new Error(`Duplicate sync-locked track ${trackId}.`);
    }
    seenSyncTracks.add(trackId);
    const track = project.timeline.tracks.find((candidate) => candidate.id === trackId);
    if (!track) throw new Error(`Track ${trackId} was not found.`);
    if (track.locked) throw new Error(`Track ${trackId} is locked.`);
    if (!boundaries.has(trackId)) boundaries.set(trackId, leadEnd);
  }

  const resizes: Array<ProjectActionRippleTrimPreviewPlan["resizes"][number]> = [];
  for (const track of project.timeline.tracks) {
    for (const item of track.items) {
      if (!targetIds.has(item.id)) continue;
      const durationSeconds = item.durationSeconds + durationDeltaSeconds;
      if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
        throw new Error(`Ripple trim would make ${item.id} non-positive.`);
      }
      const speed =
        typeof item.properties.speed === "number" ? item.properties.speed : 1;
      if (!Number.isFinite(speed) || speed <= 0) {
        throw new Error(`Timeline item ${item.id} has invalid speed.`);
      }
      const configuredIn = item.properties.sourceIn;
      const configuredOut = item.properties.sourceOut;
      let sourceRange: [number, number] | null = null;
      if (typeof configuredIn === "number" && typeof configuredOut === "number") {
        sourceRange = [configuredIn, configuredOut];
      } else if (configuredIn !== undefined || configuredOut !== undefined) {
        throw new Error(`Timeline item ${item.id} has an incomplete source range.`);
      } else if (item.source.type === "media") {
        sourceRange = [0, item.durationSeconds * speed];
      }
      if (!sourceRange) {
        resizes.push({ itemId: item.id, durationSeconds });
        continue;
      }
      const [sourceIn, sourceOut] = trimmedSourceRange(
        { sourceIn: sourceRange[0], sourceOut: sourceRange[1], speed, reverse: isReversedItem(item) },
        action.edge,
        durationDeltaSeconds,
      );
      if (
        !Number.isFinite(sourceIn) ||
        !Number.isFinite(sourceOut) ||
        sourceIn < 0 ||
        sourceOut <= sourceIn ||
        Math.abs(sourceOut - sourceIn - durationSeconds * speed) > 0.000_001
      ) {
        throw new Error(`Ripple trim creates an invalid source range for ${item.id}.`);
      }
      if (item.source.type === "media") {
        const mediaId = item.source.mediaId;
        const media = project.media.find((candidate) => candidate.id === mediaId);
        if (!media || sourceOut > media.durationSeconds) {
          throw new Error(`Ripple trim exceeds source media for ${item.id}.`);
        }
      }
      resizes.push({ itemId: item.id, durationSeconds, sourceIn, sourceOut });
    }
  }
  const shifts = project.timeline.tracks.flatMap((track) => {
    const boundary = boundaries.get(track.id);
    if (boundary === undefined) return [];
    return track.items.flatMap((item) =>
      !targetIds.has(item.id) && item.startSeconds >= boundary
        ? [{ itemId: item.id, startSeconds: item.startSeconds + durationDeltaSeconds }]
        : [],
    );
  });
  const resizeById = new Map(resizes.map((resize) => [resize.itemId, resize]));
  const shiftById = new Map(shifts.map((shift) => [shift.itemId, shift]));
  for (const track of project.timeline.tracks.filter((candidate) => boundaries.has(candidate.id))) {
    const planned = track.items
      .map((item) => ({
        ...item,
        startSeconds: shiftById.get(item.id)?.startSeconds ?? item.startSeconds,
        durationSeconds: resizeById.get(item.id)?.durationSeconds ?? item.durationSeconds,
      }))
      .sort((left, right) => left.startSeconds - right.startSeconds);
    for (let index = 1; index < planned.length; index += 1) {
      const previous = planned[index - 1];
      const current = planned[index];
      if (
        previous &&
        current &&
        previous.startSeconds + previous.durationSeconds > current.startSeconds + 0.000_001
      ) {
        throw new Error(
          `Ripple trim would collide on track ${track.id}: ${previous.id} with ${current.id}.`,
        );
      }
    }
  }
  return {
    durationDeltaSeconds,
    resizes,
    shifts,
    affectedTrackIds: project.timeline.tracks
      .filter((track) => boundaries.has(track.id))
      .map((track) => track.id),
  };
}

function rescaleItemKeyframes(
  properties: Record<string, unknown>,
  previousDurationSeconds: number,
  nextDurationSeconds: number,
): Record<string, unknown> {
  if (
    !Number.isFinite(previousDurationSeconds) ||
    previousDurationSeconds <= 0 ||
    !Number.isFinite(nextDurationSeconds) ||
    nextDurationSeconds <= 0
  ) {
    return properties;
  }
  const keyframes = properties.keyframes;
  if (!keyframes || typeof keyframes !== "object" || Array.isArray(keyframes)) {
    return properties;
  }

  let changed = false;
  const scale = nextDurationSeconds / previousDurationSeconds;
  const nextKeyframes = Object.fromEntries(
    Object.entries(keyframes as Record<string, unknown>).map(
      ([propertyName, propertyKeyframes]) => {
        if (!Array.isArray(propertyKeyframes)) {
          return [propertyName, propertyKeyframes];
        }

        return [
          propertyName,
          propertyKeyframes.map((keyframe) => {
            if (
              !keyframe ||
              typeof keyframe !== "object" ||
              Array.isArray(keyframe)
            ) {
              return keyframe;
            }
            const atSeconds = (keyframe as { atSeconds?: unknown }).atSeconds;
            if (typeof atSeconds !== "number" || !Number.isFinite(atSeconds)) {
              return keyframe;
            }

            changed = true;
            return {
              ...keyframe,
              atSeconds: Math.min(
                Math.max(atSeconds * scale, 0),
                nextDurationSeconds,
              ),
            };
          }),
        ];
      },
    ),
  );

  return changed
    ? {
        ...properties,
        keyframes: nextKeyframes,
      }
    : properties;
}

const keyframeTimeEpsilon = 0.000_000_1;

function timelineItemSpeed(item: TimelineItem): number {
  const value = item.properties.speed;
  const speed = typeof value === "number" ? value : 1;
  if (!Number.isFinite(speed) || speed <= 0) {
    throw new Error(`Timeline item ${item.id} has invalid speed.`);
  }
  return speed;
}

function timelineItemSourceWindow(item: TimelineItem): SourceWindow | null {
  if (item.source.type !== "media") return null;
  const speed = timelineItemSpeed(item);
  const sourceIn = typeof item.properties.sourceIn === "number"
    ? item.properties.sourceIn : 0;
  const sourceOut = typeof item.properties.sourceOut === "number"
    ? item.properties.sourceOut
    : sourceIn + item.durationSeconds * speed;
  return { sourceIn, sourceOut, speed, reverse: isReversedItem(item) };
}

function sampleKeyframeLane(
  keyframes: readonly ProjectActionKeyframe[],
  seconds: number,
): number {
  const first = keyframes[0];
  if (!first) throw new Error("Cannot sample an empty keyframe lane.");
  if (seconds <= first.atSeconds) return first.value;
  const last = keyframes[keyframes.length - 1];
  if (!last) throw new Error("Cannot sample an empty keyframe lane.");
  if (seconds >= last.atSeconds) return last.value;
  for (let index = 0; index < keyframes.length - 1; index += 1) {
    const left = keyframes[index];
    const right = keyframes[index + 1];
    if (!left || !right) continue;
    if (seconds > right.atSeconds) continue;
    const progress = Math.min(1, Math.max(0,
      (seconds - left.atSeconds) / (right.atSeconds - left.atSeconds),
    ));
    const eased = (() => {
      switch (canonicalProjectActionKeyframeEasing(left.easing) ?? "linear") {
        case "hold": return 0;
        case "easeIn": return progress * progress;
        case "easeOut": return 1 - (1 - progress) * (1 - progress);
        case "easeInOut": return progress * progress * (3 - 2 * progress);
        case "linear": return progress;
      }
    })();
    return left.value + (right.value - left.value) * eased;
  }
  return last.value;
}

function partitionKeyframeLane(
  lane: unknown,
  startSeconds: number,
  endSeconds: number,
  originalDurationSeconds: number,
): ProjectActionKeyframe[] {
  if (!Array.isArray(lane)) throw new Error("Keyframe lane must be an array.");
  const canonical = canonicalizeProjectActionKeyframes(lane as ProjectActionKeyframe[]);
  const first = canonical[0];
  if (!first) return [];
  if (canonical.some((keyframe) => keyframe.atSeconds < 0 || keyframe.atSeconds > originalDurationSeconds)) {
    throw new Error("Keyframe time must be inside the original item duration.");
  }
  const next = canonical
    .filter((keyframe) =>
      keyframe.atSeconds + keyframeTimeEpsilon >= startSeconds
      && keyframe.atSeconds <= endSeconds + keyframeTimeEpsilon)
    .map((keyframe) => ({
      ...keyframe,
      atSeconds: Math.min(endSeconds - startSeconds,
        Math.max(0, keyframe.atSeconds - startSeconds)),
    }));
  const boundary = (sampleSeconds: number, outputSeconds: number): ProjectActionKeyframe => {
    const source = [...canonical].reverse().find(
      (keyframe) => keyframe.atSeconds <= sampleSeconds + keyframeTimeEpsilon,
    ) ?? first;
    return {
      atSeconds: outputSeconds,
      value: sampleKeyframeLane(canonical, sampleSeconds),
      ...(source.easing ? { easing: source.easing } : {}),
    };
  };
  if (
    Math.abs(startSeconds) > keyframeTimeEpsilon
    && !((next[0]?.atSeconds ?? Number.POSITIVE_INFINITY) <= keyframeTimeEpsilon)
  ) {
    next.unshift(boundary(startSeconds, 0));
  }
  const duration = endSeconds - startSeconds;
  if (
    endSeconds < originalDurationSeconds - keyframeTimeEpsilon
    && !next.some((keyframe) => Math.abs(keyframe.atSeconds - duration) <= keyframeTimeEpsilon)
  ) {
    next.push(boundary(endSeconds, duration));
  }
  return canonicalizeProjectActionKeyframes(next);
}

function partitionKeyframeLanes(
  lanes: unknown,
  startSeconds: number,
  endSeconds: number,
  originalDurationSeconds: number,
): Record<string, ProjectActionKeyframe[]> {
  if (!lanes || typeof lanes !== "object" || Array.isArray(lanes)) {
    throw new Error("Keyframe lanes must be an object.");
  }
  return Object.fromEntries(Object.entries(lanes).map(([name, lane]) => [
    name,
    partitionKeyframeLane(lane, startSeconds, endSeconds, originalDurationSeconds),
  ]));
}

function itemPropertiesForSubrange(
  properties: TimelineItem["properties"],
  startSeconds: number,
  endSeconds: number,
  originalDurationSeconds: number,
): TimelineItem["properties"] {
  const next: TimelineItem["properties"] = structuredClone(properties);
  if (properties.keyframes !== undefined) {
    next.keyframes = partitionKeyframeLanes(
      properties.keyframes, startSeconds, endSeconds, originalDurationSeconds,
    );
  }
  const effectLanes = properties.effectParameterKeyframes;
  if (effectLanes !== undefined) {
    if (!effectLanes || typeof effectLanes !== "object" || Array.isArray(effectLanes)) {
      throw new Error("Effect parameter keyframes must be an object.");
    }
    next.effectParameterKeyframes = Object.fromEntries(
      Object.entries(effectLanes as Record<string, unknown>).map(([instanceId, lanes]) => [
        instanceId,
        partitionKeyframeLanes(lanes, startSeconds, endSeconds, originalDurationSeconds),
      ]),
    );
  }
  const duration = endSeconds - startSeconds;
  if (startSeconds > keyframeTimeEpsilon) delete next.fadeInSeconds;
  else if (typeof next.fadeInSeconds === "number") {
    next.fadeInSeconds = Math.min(next.fadeInSeconds, duration);
  }
  if (endSeconds < originalDurationSeconds - keyframeTimeEpsilon) delete next.fadeOutSeconds;
  else if (typeof next.fadeOutSeconds === "number") {
    next.fadeOutSeconds = Math.min(next.fadeOutSeconds, duration);
  }
  return next;
}

function uniqueFragmentId(
  itemId: string,
  kind: "overwrite" | "ripple" | "insert",
  seconds: number,
  ids: Set<string>,
): string {
  const base = `${itemId}-${kind}-${Math.round(seconds * 1000)}`;
  let candidate = base;
  let suffix = 2;
  while (ids.has(candidate)) candidate = `${base}-${suffix++}`;
  ids.add(candidate);
  return candidate;
}

function overwriteTimelineItems(
  existingItems: readonly TimelineItem[],
  incomingItems: readonly TimelineItem[],
): TimelineItem[] {
  let items = [...existingItems];
  const ids = new Set(items.map((item) => item.id));
  const ranges = [...incomingItems]
    .map((item) => [item.startSeconds, item.startSeconds + item.durationSeconds] as const)
    .sort((left, right) => left[0] - right[0]);
  for (const [startSeconds, endSeconds] of ranges) {
    items = items.flatMap((item) => {
      const itemEnd = item.startSeconds + item.durationSeconds;
      if (itemEnd <= startSeconds || item.startSeconds >= endSeconds) return [item];
      if (item.startSeconds >= startSeconds && itemEnd <= endSeconds) return [];
      const sourceWindow = timelineItemSourceWindow(item);
      const leftDuration = Math.max(0, startSeconds - item.startSeconds);
      const rightDuration = Math.max(0, itemEnd - endSeconds);
      const fragments: TimelineItem[] = [];
      if (leftDuration > 0) {
        const properties = itemPropertiesForSubrange(
          item.properties, 0, leftDuration, item.durationSeconds,
        );
        if (sourceWindow) {
          [properties.sourceIn, properties.sourceOut] = sourceSubrange(sourceWindow, 0, leftDuration, item.durationSeconds);
        }
        fragments.push({ ...item, durationSeconds: leftDuration, properties });
      }
      if (rightDuration > 0) {
        const skipped = endSeconds - item.startSeconds;
        const properties = itemPropertiesForSubrange(
          item.properties, skipped, item.durationSeconds, item.durationSeconds,
        );
        if (sourceWindow) {
          [properties.sourceIn, properties.sourceOut] = sourceSubrange(
            sourceWindow, skipped, item.durationSeconds, item.durationSeconds,
          );
        }
        fragments.push({
          ...item,
          id: leftDuration > 0
            ? uniqueFragmentId(item.id, "overwrite", endSeconds, ids)
            : item.id,
          startSeconds: endSeconds,
          durationSeconds: rightDuration,
          properties,
        });
      }
      return fragments;
    });
  }
  return [...items, ...incomingItems].sort((left, right) => left.startSeconds - right.startSeconds);
}

function insertTimelineItems(
  existingItems: readonly TimelineItem[],
  incomingItems: readonly TimelineItem[],
  insertSeconds: number,
): TimelineItem[] {
  const insertedDuration = incomingItems.reduce((sum, item) => sum + item.durationSeconds, 0);
  const ids = new Set(existingItems.map((item) => item.id));
  const shifted = existingItems.flatMap((item) => {
    const itemEnd = item.startSeconds + item.durationSeconds;
    if (item.startSeconds < insertSeconds && insertSeconds < itemEnd) {
      const leftDuration = insertSeconds - item.startSeconds;
      const rightDuration = itemEnd - insertSeconds;
      const sourceWindow = timelineItemSourceWindow(item);
      const leftProperties = itemPropertiesForSubrange(
        item.properties, 0, leftDuration, item.durationSeconds,
      );
      const rightProperties = itemPropertiesForSubrange(
        item.properties, leftDuration, item.durationSeconds, item.durationSeconds,
      );
      if (sourceWindow) {
        [leftProperties.sourceIn, leftProperties.sourceOut] = sourceSubrange(
          sourceWindow, 0, leftDuration, item.durationSeconds,
        );
        [rightProperties.sourceIn, rightProperties.sourceOut] = sourceSubrange(
          sourceWindow, leftDuration, item.durationSeconds, item.durationSeconds,
        );
      }
      return [
        { ...item, durationSeconds: leftDuration, properties: leftProperties },
        {
          ...item,
          id: uniqueFragmentId(item.id, "insert", insertSeconds, ids),
          startSeconds: insertSeconds + insertedDuration,
          durationSeconds: rightDuration,
          properties: rightProperties,
        },
      ];
    }
    return [{
      ...item,
      startSeconds: item.startSeconds >= insertSeconds
        ? item.startSeconds + insertedDuration : item.startSeconds,
    }];
  });
  let cursor = insertSeconds;
  const inserted = incomingItems.map((item) => {
    const next = { ...item, startSeconds: cursor };
    cursor += item.durationSeconds;
    return next;
  });
  return [...shifted, ...inserted].sort((left, right) => left.startSeconds - right.startSeconds);
}

/** Applies a timeline patch like its project action, including transition maintenance. */
export function applyTimelinePatchLocally(
  project: VideoProject,
  patch: TimelinePatch,
): VideoProject {
  return maintainTransitionsAfterAction(project, applyTimelinePatchWithoutMaintenance(project, patch));
}

function applyTimelinePatchWithoutMaintenance(
  project: VideoProject,
  patch: TimelinePatch,
): VideoProject {
  switch (patch.type) {
    case "moveItem": {
      let movedItem: TimelineItem | null = null;
      const tracksWithoutMovedItem = project.timeline.tracks.map((track) => {
        const items = track.items.filter((item) => {
          if (item.id !== patch.itemId) {
            return true;
          }

          movedItem = { ...item, startSeconds: patch.startSeconds };
          return false;
        });

        return items.length === track.items.length ? track : { ...track, items };
      });

      if (!movedItem) {
        return project;
      }

      let targetTrackFound = false;
      const tracks = tracksWithoutMovedItem.map((track) => {
        if (track.id !== patch.targetTrackId) {
          return track;
        }

        targetTrackFound = true;
        return {
          ...track,
          items: sortTimelineItemsByStart([...track.items, movedItem as TimelineItem]),
        };
      });

      if (!targetTrackFound) {
        return project;
      }

      return projectWithTimelineTracks(project, tracks);
    }
    case "resizeItem":
      return updateTimelineItem(project, patch.itemId, (item) => ({
        ...item,
        durationSeconds: patch.durationSeconds,
        properties: rescaleItemKeyframes(
          item.properties,
          item.durationSeconds,
          patch.durationSeconds,
        ),
      }));
    case "trimItem":
      return updateTimelineItem(project, patch.itemId, (item) => ({
        ...item,
        startSeconds: patch.startSeconds,
        durationSeconds: patch.durationSeconds,
        properties: {
          ...item.properties,
          ...(patch.sourceIn !== undefined ? { sourceIn: patch.sourceIn } : {}),
          ...(patch.sourceOut !== undefined ? { sourceOut: patch.sourceOut } : {}),
        },
      }));
    case "editCaptionText":
      return updateTimelineItem(project, patch.itemId, (item) => {
        if (item.source.type !== "text") {
          return item;
        }

        return {
          ...item,
          label: patch.text,
          source: { ...item.source, text: patch.text },
          properties: {
            ...item.properties,
            text: patch.text,
            textEdited: true,
          },
        };
      });
  }
}

/** A job id Rust accepts as one path component (`validate_job_id`). */
function validateJobId(jobId: string): void {
  if (!jobId.trim()) throw new Error("job id cannot be empty");
  if (jobId.includes("/") || jobId.includes("\\") || jobId === "." || jobId === "..") {
    throw new Error(`job id must be a safe path segment: ${jobId}`);
  }
}

/** The Rust `record_job_failure` field checks; a terminal job is still left unchanged. */
function validateJobFailure(project: VideoProject, action: Extract<ProjectAction, { type: "recordJobFailure" }>): void {
  validateJobId(action.jobId);
  if (!action.reason.trim()) throw new Error("job failure reason cannot be empty");
  if (!action.updatedAt.trim()) throw new Error("job updatedAt cannot be empty");
  if (typeof action.runId === "string" && !action.runId.trim()) throw new Error("job workflow metadata is missing: runId");
  if (!project.jobs.some((job) => job.id === action.jobId)) throw new Error(`job was not found: ${action.jobId}`);
}

/** A non-terminal status update carrying the run id of a job that run already finished. */
function lateStartOfFinishedRun(job: ProjectJobSummary, status: ProjectJobSummary["status"], runId: string | null | undefined): boolean {
  return Boolean(runId) && runId === job.workflow?.runId && !unfinishedJobStatuses.has(job.status) && unfinishedJobStatuses.has(status);
}

/**
 * Applies a project action to a local snapshot, then keeps transitions valid the way Rust
 * `apply_project_action` does after every action.
 */
export function applyProjectActionLocally(
  project: VideoProject,
  action: ProjectAction,
): VideoProject {
  return maintainTransitionsAfterAction(project, applyProjectActionWithoutMaintenance(project, action));
}

function applyProjectActionWithoutMaintenance(
  project: VideoProject,
  action: ProjectAction,
): VideoProject {
  switch (action.type) {
    case "addItems": {
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        if (track.id !== action.targetTrackId) {
          return track;
        }

        changed = true;
        return {
          ...track,
          items: overwriteTimelineItems(track.items, action.items),
        };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "insertItems": {
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        if (track.id !== action.targetTrackId) return track;
        changed = true;
        return {
          ...track,
          items: insertTimelineItems(track.items, action.items, action.insertSeconds),
        };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "createTimeline": {
      const activeTimelineId = project.activeTimelineId ?? "main";
      const timelines = project.timelines?.length
        ? project.timelines.map((entry) =>
            entry.id === activeTimelineId ? { ...entry, timeline: project.timeline } : entry,
          )
        : [{ id: activeTimelineId, name: "Timeline 1", timeline: project.timeline }];
      if (timelines.some((timeline) => timeline.id === action.timelineId)) return project;
      const sourceTimeline = action.sourceTimelineId
        ? timelines.find((entry) => entry.id === action.sourceTimelineId)?.timeline
        : project.timeline;
      if (action.duplicateActive && !sourceTimeline) return project;
      const timeline = action.duplicateActive
        ? structuredClone(sourceTimeline!)
        : { durationSeconds: 0, tracks: [] };
      return {
        ...project,
        timeline,
        timelines: [...timelines, { id: action.timelineId, name: action.name, timeline }],
        activeTimelineId: action.timelineId,
      };
    }
    case "setActiveTimeline": {
      const timelines = project.timelines ?? [];
      const selected = timelines.find((timeline) => timeline.id === action.timelineId);
      return selected
        ? { ...project, timeline: selected.timeline, timelines, activeTimelineId: selected.id }
        : project;
    }
    case "renameTimeline": {
      // Like Rust `timeline_library`, a project without a timeline library gets its implicit
      // "Timeline 1" entry materialized before the rename.
      const activeTimelineId = project.activeTimelineId ?? "main";
      const timelines = project.timelines?.length
        ? project.timelines
        : [{ id: activeTimelineId, name: "Timeline 1", timeline: project.timeline }];
      if (!timelines.some((timeline) => timeline.id === action.timelineId)) return project;
      return {
        ...project,
        activeTimelineId,
        timelines: timelines.map((timeline) =>
          timeline.id === action.timelineId ? { ...timeline, name: action.name } : timeline,
        ),
      };
    }
    case "deleteTimeline": {
      const activeTimelineId = project.activeTimelineId ?? "main";
      const timelines = project.timelines?.length
        ? project.timelines.map((entry) =>
            entry.id === activeTimelineId ? { ...entry, timeline: project.timeline } : entry,
          )
        : [{ id: activeTimelineId, name: "Timeline 1", timeline: project.timeline }];
      if (timelines.length <= 1 || !timelines.some((entry) => entry.id === action.timelineId)) {
        return project;
      }
      const referenced = timelines.some(
        (entry) =>
          entry.id !== action.timelineId &&
          entry.timeline.tracks.some((track) =>
            track.items.some(
              (item) =>
                item.source.type === "timeline" &&
                item.source.timelineId === action.timelineId,
            ),
          ),
      );
      if (referenced) return project;
      const remaining = timelines.filter((entry) => entry.id !== action.timelineId);
      if (action.timelineId !== activeTimelineId) {
        return { ...project, timelines: remaining };
      }
      const nextActive = remaining[0];
      if (!nextActive) return project;
      return {
        ...project,
        timeline: nextActive.timeline,
        timelines: remaining,
        activeTimelineId: nextActive.id,
      };
    }
    case "decomposeTimelineItem": {
      const wrapperTrack = project.timeline.tracks.find((track) =>
        track.items.some((item) => item.id === action.itemId),
      );
      const wrapper = wrapperTrack?.items.find((item) => item.id === action.itemId);
      const timelineSource = wrapper?.source;
      if (!wrapperTrack || !wrapper || timelineSource?.type !== "timeline" || Object.keys(wrapper.properties).length > 0) {
        return project;
      }
      const nestedTimeline = project.timelines?.find(
        (timeline) => timeline.id === timelineSource.timelineId,
      )?.timeline;
      if (!nestedTimeline) return project;

      const existingItemIds = new Set(
        project.timeline.tracks.flatMap((track) =>
          track.items.filter((item) => item.id !== wrapper.id).map((item) => item.id),
        ),
      );
      const existingTrackIds = new Set(project.timeline.tracks.map((track) => track.id));
      const existingTransitionIds = new Set(
        project.timeline.tracks.flatMap((track) => (track.transitions ?? []).map((transition) => transition.id)),
      );
      const numericProperty = (item: TimelineItem, key: string) => {
        const value = item.properties[key];
        return typeof value === "number" && Number.isFinite(value) ? value : null;
      };
      const uniqueId = (base: string, existing: Set<string>) => {
        let candidate = base;
        let suffix = 2;
        while (existing.has(candidate)) candidate = `${base}-${suffix++}`;
        existing.add(candidate);
        return candidate;
      };
      const wrapperEnd = wrapper.startSeconds + wrapper.durationSeconds;
      const decomposedTracks = nestedTimeline.tracks.flatMap((nestedTrack) => {
        const decomposedItemIds = new Map<string, string>();
        const items = nestedTrack.items.flatMap((child) => {
          const startSeconds = wrapper.startSeconds + child.startSeconds;
          const endSeconds = Math.min(startSeconds + child.durationSeconds, wrapperEnd);
          if (endSeconds <= startSeconds) return [];
          const durationSeconds = endSeconds - startSeconds;
          const sourceIn = numericProperty(child, "sourceIn");
          const speed = numericProperty(child, "speed") ?? 1;
          const id = uniqueId(`${wrapper.id}-${child.id}`, existingItemIds);
          decomposedItemIds.set(child.id, id);
          return [{
            ...structuredClone(child),
            id,
            startSeconds,
            durationSeconds,
            properties: {
              ...child.properties,
              ...(endSeconds < startSeconds + child.durationSeconds && sourceIn !== null
                ? isReversedItem(child)
                  ? { sourceIn: (numericProperty(child, "sourceOut") ?? sourceIn + child.durationSeconds * speed) - durationSeconds * speed }
                  : { sourceOut: sourceIn + durationSeconds * speed }
                : {}),
            },
          }];
        });
        if (items.length === 0) return [];
        const trackId = uniqueId(`decomposed-${wrapper.id}-${nestedTrack.id}`, existingTrackIds);
        // Carry transitions between surviving children; maintenance then drops or clamps any the
        // wrapper's clipping made invalid.
        const transitions = (nestedTrack.transitions ?? []).flatMap((transition) => {
          const leftItemId = decomposedItemIds.get(transition.leftItemId);
          const rightItemId = decomposedItemIds.get(transition.rightItemId);
          if (leftItemId === undefined || rightItemId === undefined) return [];
          return [{
            ...transition,
            id: uniqueId(`${wrapper.id}-${transition.id}`, existingTransitionIds),
            leftItemId,
            rightItemId,
          }];
        });
        const { transitions: _nestedTransitions, ...nestedTrackFields } = structuredClone(nestedTrack);
        return [{
          ...nestedTrackFields,
          id: trackId,
          name: `${wrapper.label}: ${nestedTrack.name}`,
          locked: false,
          enabled: wrapperTrack.enabled !== false && nestedTrack.enabled !== false,
          items,
          ...(transitions.length > 0 ? { transitions } : {}),
        }];
      });
      const tracks = project.timeline.tracks.map((track) =>
        track.id === wrapperTrack.id
          ? { ...track, items: track.items.filter((item) => item.id !== wrapper.id) }
          : track,
      );
      return projectWithTimelineTracks(project, [...tracks, ...decomposedTracks]);
    }
    case "removeItems": {
      const itemIds = new Set(action.itemIds);
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        const items = track.items.filter((item) => !itemIds.has(item.id));
        if (items.length === track.items.length) {
          return track;
        }

        changed = true;
        return { ...track, items };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "moveItems": {
      const movesByItemId = new Map(action.moves.map((move) => [move.itemId, move]));
      const targetTrackIds = new Set(action.moves.map((move) => move.targetTrackId));
      if (
        ![...targetTrackIds].every((targetTrackId) =>
          project.timeline.tracks.some((track) => track.id === targetTrackId),
        )
      ) {
        return project;
      }

      const movedItems: TimelineItem[] = [];
      const tracksWithoutMovedItems = project.timeline.tracks.map((track) => {
        const items = track.items.filter((item) => {
          const move = movesByItemId.get(item.id);
          if (!move) {
            return true;
          }

          movedItems.push({ ...item, startSeconds: move.startSeconds });
          return false;
        });

        return items.length === track.items.length ? track : { ...track, items };
      });

      if (movedItems.length === 0) {
        return project;
      }

      const tracks = tracksWithoutMovedItems.map((track) => {
        const incomingItems = movedItems.filter(
          (item) => movesByItemId.get(item.id)?.targetTrackId === track.id,
        );
        return incomingItems.length > 0
          ? { ...track, items: [...track.items, ...incomingItems] }
          : track;
      });
      return projectWithTimelineTracks(project, tracks);
    }
    case "reorderItems": {
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        if (track.id !== action.reorder.targetTrackId) {
          return track;
        }

        const itemById = new Map(track.items.map((item) => [item.id, item]));
        const reorderedItems = action.reorder.itemIds.flatMap((itemId, index) => {
          const item = itemById.get(itemId);
          if (!item) {
            return [];
          }

          return [
            {
              ...item,
              startSeconds:
                action.reorder.startSeconds + index * action.reorder.gapSeconds,
            },
          ];
        });
        const reorderedItemIds = new Set(reorderedItems.map((item) => item.id));
        changed = reorderedItems.length > 0;
        return {
          ...track,
          items: [
            ...track.items.filter((item) => !reorderedItemIds.has(item.id)),
            ...reorderedItems,
          ],
        };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "resizeItems":
      return action.resizes.reduce(
        (currentProject, resize) =>
          updateTimelineItem(currentProject, resize.itemId, (item) => ({
            ...item,
            durationSeconds: resize.durationSeconds,
            properties: rescaleItemKeyframes(
              item.properties,
              item.durationSeconds,
              resize.durationSeconds,
            ),
          })),
        project,
      );
    case "trimItems":
      return action.trims.reduce(
        (currentProject, trim) =>
          updateTimelineItem(currentProject, trim.itemId, (item) => ({
            ...item,
            startSeconds: trim.startSeconds,
            durationSeconds: trim.durationSeconds,
            properties: {
              ...item.properties,
              ...(trim.sourceIn !== undefined ? { sourceIn: trim.sourceIn } : {}),
              ...(trim.sourceOut !== undefined ? { sourceOut: trim.sourceOut } : {}),
            },
          })),
        project,
      );
    case "rippleTrimItem": {
      const plan = planProjectRippleTrim(project, action);
      const resizeById = new Map(plan.resizes.map((resize) => [resize.itemId, resize]));
      const shiftById = new Map(plan.shifts.map((shift) => [shift.itemId, shift]));
      const tracks = project.timeline.tracks.map((track) => ({
        ...track,
        items: track.items.map((item) => {
          const resize = resizeById.get(item.id);
          const shift = shiftById.get(item.id);
          if (!resize && !shift) return item;
          return {
            ...item,
            startSeconds: shift?.startSeconds ?? item.startSeconds,
            durationSeconds: resize?.durationSeconds ?? item.durationSeconds,
            properties: resize
              ? (() => {
                  const originalDuration = item.durationSeconds;
                  const subrangeStart = action.edge === "left"
                    ? originalDuration - resize.durationSeconds : 0;
                  const subrangeEnd = action.edge === "left"
                    ? originalDuration : resize.durationSeconds;
                  return {
                    ...itemPropertiesForSubrange(
                      item.properties, subrangeStart, subrangeEnd, originalDuration,
                    ),
                    ...(resize.sourceIn !== undefined ? { sourceIn: resize.sourceIn } : {}),
                    ...(resize.sourceOut !== undefined ? { sourceOut: resize.sourceOut } : {}),
                  };
                })()
              : item.properties,
          };
        }),
      }));
      return projectWithTimelineTracks(project, tracks);
    }
    case "rippleDeleteRanges": {
      let nextProject = project;
      const ranges = [...action.ranges].sort(
        (left, right) => right.startSeconds - left.startSeconds,
      );
      for (const range of ranges) {
        const duration = range.endSeconds - range.startSeconds;
        if (duration <= 0) {
          continue;
        }
        const trackIds = new Set(range.trackIds);
        let changed = false;
        const tracks = nextProject.timeline.tracks.map((track) => {
          if (!trackIds.has(track.id)) {
            return track;
          }
          const items = track.items.flatMap((item) => {
            const itemEnd = item.startSeconds + item.durationSeconds;
            if (itemEnd <= range.startSeconds) {
              return [item];
            }
            if (item.startSeconds >= range.endSeconds) {
              changed = true;
              return [{ ...item, startSeconds: item.startSeconds - duration }];
            }
            if (item.startSeconds >= range.startSeconds && itemEnd <= range.endSeconds) {
              changed = true;
              return [];
            }

            const sourceWindow = timelineItemSourceWindow(item);
            const leftDuration = Math.max(0, range.startSeconds - item.startSeconds);
            const rightDuration = Math.max(0, itemEnd - range.endSeconds);
            const nextItems: TimelineItem[] = [];

            if (leftDuration > 0) {
              const properties = itemPropertiesForSubrange(
                item.properties, 0, leftDuration, item.durationSeconds,
              );
              if (sourceWindow) {
                [properties.sourceIn, properties.sourceOut] = sourceSubrange(
                  sourceWindow, 0, leftDuration, item.durationSeconds,
                );
              }
              nextItems.push({
                ...item,
                durationSeconds: leftDuration,
                properties,
              });
            }
            if (rightDuration > 0) {
              const skipped = range.endSeconds - item.startSeconds;
              const properties = itemPropertiesForSubrange(
                item.properties, skipped, item.durationSeconds, item.durationSeconds,
              );
              if (sourceWindow) {
                [properties.sourceIn, properties.sourceOut] = sourceSubrange(
                  sourceWindow, skipped, item.durationSeconds, item.durationSeconds,
                );
              }
              nextItems.push({
                ...item,
                id:
                  leftDuration > 0
                    ? `${item.id}-ripple-${Math.round(range.endSeconds * 1000)}`
                    : item.id,
                startSeconds: range.startSeconds,
                durationSeconds: rightDuration,
                properties,
              });
            }
            changed = true;
            return nextItems;
          });
          return { ...track, items };
        });
        if (changed) {
          nextProject = projectWithTimelineTracks(nextProject, tracks);
        }
      }
      return nextProject;
    }
    case "splitItems": {
      let changed = false;
      const splitByItemId = new Map(action.splits.map((split) => [split.itemId, split]));
      const tracks = project.timeline.tracks.map((track) => ({
        ...track,
        items: track.items.flatMap((item) => {
          const split = splitByItemId.get(item.id);
          if (
            !split ||
            split.splitSeconds <= item.startSeconds ||
            split.splitSeconds >= item.startSeconds + item.durationSeconds
          ) {
            return [item];
          }

          changed = true;
          const localSplitSeconds = split.splitSeconds - item.startSeconds;
          const sourceWindow = timelineItemSourceWindow(item);
          const leftProperties = itemPropertiesForSubrange(
            item.properties, 0, localSplitSeconds, item.durationSeconds,
          );
          const rightProperties = itemPropertiesForSubrange(
            item.properties, localSplitSeconds, item.durationSeconds, item.durationSeconds,
          );
          if (sourceWindow) {
            [leftProperties.sourceIn, leftProperties.sourceOut] = sourceSubrange(
              sourceWindow, 0, localSplitSeconds, item.durationSeconds,
            );
            [rightProperties.sourceIn, rightProperties.sourceOut] = sourceSubrange(
              sourceWindow, localSplitSeconds, item.durationSeconds, item.durationSeconds,
            );
          }
          const rightDurationSeconds =
            item.startSeconds + item.durationSeconds - split.splitSeconds;
          return [
            {
              ...item,
              durationSeconds: split.splitSeconds - item.startSeconds,
              properties: leftProperties,
            },
            {
              ...item,
              id: split.newItemId,
              label: `${item.label} split`,
              startSeconds: split.splitSeconds,
              durationSeconds: rightDurationSeconds,
              properties: rightProperties,
            },
          ];
        }),
      }));
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "createTrack": {
      const tracks = [...project.timeline.tracks];
      const insertIndex =
        action.afterTrackId === undefined
          ? tracks.length
          : tracks.findIndex((track) => track.id === action.afterTrackId) + 1;
      if (insertIndex <= 0 || tracks.some((track) => track.id === action.track.id)) {
        return project;
      }

      tracks.splice(insertIndex, 0, action.track);
      return projectWithTimelineTracks(project, tracks);
    }
    case "removeTracks": {
      // Like the Rust action, the whole batch is rejected when the list is empty or any
      // id is blank, unknown, or names a locked track.
      const trackIds = new Set(action.trackIds.map((trackId) => trackId.trim()));
      const rejected =
        trackIds.size === 0 ||
        [...trackIds].some((trackId) => {
          const track = project.timeline.tracks.find((candidate) => candidate.id === trackId);
          return !track || track.locked;
        });
      if (rejected) return project;
      return projectWithTimelineTracks(
        project,
        project.timeline.tracks.filter((track) => !trackIds.has(track.id)),
      );
    }
    case "setTrackLocked": {
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        if (track.id !== action.trackId) {
          return track;
        }

        changed = true;
        return { ...track, locked: action.locked };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "reorderTrack": {
      const tracks = [...project.timeline.tracks];
      const sourceIndex = tracks.findIndex((track) => track.id === action.trackId);
      const targetIndex = tracks.findIndex((track) => track.id === action.targetTrackId);
      const source = tracks[sourceIndex];
      const target = tracks[targetIndex];
      if (!source || !target || source.id === target.id || (source.kind === "audio") !== (target.kind === "audio")) {
        return project;
      }
      const [track] = tracks.splice(sourceIndex, 1);
      if (!track) return project;
      const remainingTargetIndex = tracks.findIndex((candidate) => candidate.id === target.id);
      const insertionIndex = action.placement === "after"
        ? remainingTargetIndex + 1
        : remainingTargetIndex;
      tracks.splice(insertionIndex, 0, track);
      return projectWithTimelineTracks(project, tracks);
    }
    case "setTrackSyncLocked": {
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        if (track.id !== action.trackId) return track;
        changed = true;
        return { ...track, syncLocked: action.syncLocked };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "setTrackEnabled": {
      let changed = false;
      const tracks = project.timeline.tracks.map((track) => {
        if (track.id !== action.trackId) {
          return track;
        }

        changed = true;
        return { ...track, enabled: action.enabled };
      });
      return changed ? projectWithTimelineTracks(project, tracks) : project;
    }
    case "editCaptionText":
    case "editTextItem":
      return updateTimelineItem(project, action.itemId, (item) => {
        if (item.source.type !== "text") {
          return item;
        }

        return {
          ...item,
          label: action.text,
          source: { ...item.source, text: action.text },
          properties: {
            ...item.properties,
            text: action.text,
            textEdited: true,
          },
        };
      });
    case "updateTextOverlayItems":
      return action.updates.reduce(
        (currentProject, update) =>
          updateTimelineItem(currentProject, update.itemId, (item) => {
            if (item.source.type !== "text") {
              return item;
            }

            return {
              ...item,
              startSeconds: update.startSeconds,
              durationSeconds: update.durationSeconds,
              label: update.text,
              source: { ...item.source, text: update.text },
              properties: {
                ...item.properties,
                text: update.text,
                visualTreatment: update.visualTreatment,
                motion: update.motion,
                safeZone: update.safeZone,
                avoid: update.avoid,
                textEdited: true,
              },
            };
          }),
        project,
      );
    case "updateVisualClipOpacity":
      return updateTimelineItem(project, action.itemId, (item) => ({
        ...item,
        properties: { ...item.properties, opacity: action.opacity },
      }));
    case "updateVisualClipTransform":
      return updateTimelineItem(project, action.itemId, (item) => {
        const currentTransform =
          item.properties.transform &&
          typeof item.properties.transform === "object" &&
          !Array.isArray(item.properties.transform)
            ? item.properties.transform
            : {};
        return {
          ...item,
          properties: {
            ...item.properties,
            transform: { ...currentTransform, ...action.transform },
          },
        };
      });
    case "updateVisualClipCrop":
      return updateTimelineItem(project, action.itemId, (item) => {
        const properties = { ...item.properties };
        for (const [key, value] of Object.entries(action.crop)) {
          if (value && value > 0) {
            properties[key] = value;
          } else {
            delete properties[key];
          }
        }
        return { ...item, properties };
      });
    case "updateVisualClipFades":
      return updateTimelineItem(project, action.itemId, (item) => {
        const properties = { ...item.properties };
        if (action.fadeInSeconds > 0) properties.fadeInSeconds = action.fadeInSeconds;
        else delete properties.fadeInSeconds;
        if (action.fadeOutSeconds > 0) properties.fadeOutSeconds = action.fadeOutSeconds;
        else delete properties.fadeOutSeconds;
        return { ...item, properties };
      });
    case "updateAudioFades":
      return updateTimelineItem(project, action.itemId, (item) => {
        const properties = { ...item.properties };
        if (action.fadeInSeconds > 0) properties.fadeInSeconds = action.fadeInSeconds;
        else delete properties.fadeInSeconds;
        if (action.fadeOutSeconds > 0) properties.fadeOutSeconds = action.fadeOutSeconds;
        else delete properties.fadeOutSeconds;
        return { ...item, properties };
      });
    case "updateVisualClipSpeed":
      return updateTimelineItem(project, action.itemId, (item) => {
        const properties = { ...item.properties };
        if (action.speed === 1) delete properties.speed;
        else properties.speed = action.speed;
        return { ...item, properties };
      });
    case "updateAudioClipSpeed": {
      // Rust `update_audio_clip_speed` order: speed range, item, locked track, clip kind.
      if (!Number.isFinite(action.speed) || action.speed < 0.1 || action.speed > 8) {
        throw new Error(`audio clip speed is invalid: ${action.itemId}`);
      }
      const track = project.timeline.tracks.find((candidate) => candidate.items.some((item) => item.id === action.itemId));
      const target = track?.items.find((item) => item.id === action.itemId);
      if (!track || !target) throw new Error(`Timeline item ${action.itemId} was not found.`);
      if (track.locked) throw new Error(`track is locked: ${track.id}`);
      if (target.kind !== "audio_clip") throw new Error(`Timeline item ${action.itemId} is not an audio clip.`);
      return updateTimelineItem(project, action.itemId, (item) => {
        const properties = { ...item.properties };
        if (action.speed === 1) delete properties.speed;
        else properties.speed = action.speed;
        return { ...item, properties };
      });
    }
    case "detachAudio":
      return projectWithTimelineTracks(project, applyDetachAudio(project, action));
    case "updateClipReverse":
      return projectWithTimelineTracks(project, applyClipReverse(project, action));
    case "setItemKeyframes":
      return updateTimelineItem(project, action.itemId, (item) =>
        itemWithKeyframes(item, action.property, action.keyframes),
      );
    case "upsertItemKeyframe":
      return updateTimelineItem(project, action.itemId, (item) => {
        const keyframes = itemKeyframes(item, action.property).filter(
          (keyframe) => keyframe.atSeconds !== action.keyframe.atSeconds,
        );
        keyframes.push(action.keyframe);
        return itemWithKeyframes(item, action.property, keyframes);
      });
    case "moveItemKeyframe":
      return updateTimelineItem(project, action.itemId, (item) => {
        const keyframes = itemKeyframes(item, action.property);
        const source = keyframes.find(
          (keyframe) => keyframe.atSeconds === action.fromSeconds,
        );
        if (!source) {
          throw new Error(
            `Keyframe was not found at ${action.fromSeconds} seconds for ${action.itemId}.${action.property}.`,
          );
        }
        const remaining = keyframes.filter(
          (keyframe) =>
            keyframe.atSeconds !== action.fromSeconds &&
            keyframe.atSeconds !== action.toSeconds,
        );
        remaining.push({ ...source, atSeconds: action.toSeconds });
        return itemWithKeyframes(item, action.property, remaining);
      });
    case "deleteItemKeyframe":
      return updateTimelineItem(project, action.itemId, (item) => {
        const keyframes = itemKeyframes(item, action.property);
        const remaining = keyframes.filter(
          (keyframe) => keyframe.atSeconds !== action.atSeconds,
        );
        if (remaining.length === keyframes.length) {
          throw new Error(
            `Keyframe was not found at ${action.atSeconds} seconds for ${action.itemId}.${action.property}.`,
          );
        }
        return itemWithKeyframes(item, action.property, remaining);
      });
    case "upsertEffectParameterKeyframe":
      return updateTimelineItem(project, action.itemId, (item) => {
        const keyframes = effectParameterKeyframes(
          item,
          action.effectInstanceId,
          action.parameterKey,
        ).filter((keyframe) => keyframe.atSeconds !== action.keyframe.atSeconds);
        keyframes.push(action.keyframe);
        return itemWithEffectParameterKeyframes(
          item,
          action.effectInstanceId,
          action.parameterKey,
          keyframes,
        );
      });
    case "moveEffectParameterKeyframe":
      return updateTimelineItem(project, action.itemId, (item) => {
        const keyframes = effectParameterKeyframes(
          item,
          action.effectInstanceId,
          action.parameterKey,
        );
        const source = keyframes.find(
          (keyframe) => keyframe.atSeconds === action.fromSeconds,
        );
        if (!source) {
          throw new Error(
            `Keyframe was not found at ${action.fromSeconds} seconds for ${action.itemId}.${action.effectInstanceId}.${action.parameterKey}.`,
          );
        }
        const remaining = keyframes.filter(
          (keyframe) =>
            keyframe.atSeconds !== action.fromSeconds
            && keyframe.atSeconds !== action.toSeconds,
        );
        remaining.push({ ...source, atSeconds: action.toSeconds });
        return itemWithEffectParameterKeyframes(
          item,
          action.effectInstanceId,
          action.parameterKey,
          remaining,
        );
      });
    case "deleteEffectParameterKeyframe":
      return updateTimelineItem(project, action.itemId, (item) => {
        const keyframes = effectParameterKeyframes(
          item,
          action.effectInstanceId,
          action.parameterKey,
        );
        const remaining = keyframes.filter(
          (keyframe) => keyframe.atSeconds !== action.atSeconds,
        );
        if (remaining.length === keyframes.length) {
          throw new Error(
            `Keyframe was not found at ${action.atSeconds} seconds for ${action.itemId}.${action.effectInstanceId}.${action.parameterKey}.`,
          );
        }
        return itemWithEffectParameterKeyframes(
          item,
          action.effectInstanceId,
          action.parameterKey,
          remaining,
        );
      });
    case "updateItemProperties": {
      const updates = new Map(action.updates.map((update) => [update.itemId, update]));
      return updateTimelineItemsById(project, new Set(updates.keys()), (item) => {
        const update = updates.get(item.id);
        if (!update) return item;
        const properties = { ...item.properties, ...update.set };
        for (const key of update.remove) delete properties[key];
        for (const [key, value] of Object.entries(properties)) {
          if (value === null) delete properties[key];
        }
        return { ...item, properties };
      });
    }
    case "linkItems": {
      const itemIds = new Set(action.itemIds);
      return updateTimelineItemsById(project, itemIds, (item) => ({
        ...item,
        properties: { ...item.properties, linkGroupId: action.linkGroupId },
      }));
    }
    case "unlinkItems": {
      const itemIds = new Set(action.itemIds);
      return updateTimelineItemsById(project, itemIds, (item) => ({
        ...item,
        properties: Object.fromEntries(
          Object.entries(item.properties).filter(([key]) => key !== "linkGroupId"),
        ),
      }));
    }
    case "updateItemEffects": {
      const itemIds = new Set(action.itemIds);
      const effects = canonicalizeProjectActionEffects(action.effects);
      const retainedIds = new Set(effects.map((effect) => effect.effectInstanceId));
      return updateTimelineItemsById(project, itemIds, (item) => {
        const properties = { ...item.properties };
        if (effects.length > 0) properties.effects = effects;
        else delete properties.effects;
        const existingLanes = properties.effectParameterKeyframes;
        if (existingLanes && typeof existingLanes === "object" && !Array.isArray(existingLanes)) {
          const lanes = Object.fromEntries(
            Object.entries(existingLanes).filter(([effectInstanceId]) =>
              retainedIds.has(effectInstanceId)),
          );
          if (Object.keys(lanes).length > 0) properties.effectParameterKeyframes = lanes;
          else delete properties.effectParameterKeyframes;
        }
        return { ...item, properties };
      });
    }
    case "updateItemColorGrade": {
      const itemIds = new Set(action.itemIds);
      return updateTimelineItemsById(project, itemIds, (item) => {
        const currentGrade =
          !action.reset &&
          item.properties.colorGrade &&
          typeof item.properties.colorGrade === "object" &&
          !Array.isArray(item.properties.colorGrade)
            ? item.properties.colorGrade
            : {};
        const grade = Object.fromEntries(
          Object.entries({
            ...currentGrade,
            ...action.grade,
          }).filter(([, value]) => value !== null && value !== undefined),
        );
        return {
          ...item,
          properties:
            Object.keys(grade).length > 0
              ? { ...item.properties, colorGrade: grade }
              : Object.fromEntries(
                  Object.entries(item.properties).filter(([key]) => key !== "colorGrade"),
                ),
        };
      });
    }
    case "updateTemplateItems":
      return action.updates.reduce(
        (currentProject, update) =>
          updateTimelineItem(currentProject, update.itemId, (item) => ({
            ...item,
            startSeconds: update.startSeconds,
            durationSeconds: update.durationSeconds,
            properties: {
              ...item.properties,
              templateFields: update.templateFields,
            },
          })),
        project,
      );
    case "assignMediaFolder": {
      let changed = false;
      const media = project.media.map((asset) => {
        if (asset.id !== action.mediaId) {
          return asset;
        }

        changed = true;
        return { ...asset, folderId: action.folderId };
      });
      return changed ? { ...project, media } : project;
    }
    case "createMediaFolder": {
      if (project.mediaFolders?.some((folder) => folder.id === action.folder.id)) {
        return project;
      }

      return {
        ...project,
        mediaFolders: [...(project.mediaFolders ?? []), action.folder],
      };
    }
    case "renameMediaFolder": {
      let changed = false;
      const mediaFolders = (project.mediaFolders ?? []).map((folder) => {
        if (folder.id !== action.folderId) {
          return folder;
        }

        changed = true;
        return { ...folder, name: action.name };
      });
      return changed ? { ...project, mediaFolders } : project;
    }
    case "deleteMediaFolder": {
      const currentMediaFolders = project.mediaFolders ?? [];
      const mediaFolders = currentMediaFolders.filter(
        (folder) => folder.id !== action.folderId,
      );
      if (mediaFolders.length === currentMediaFolders.length) {
        return project;
      }

      const media = project.media.map((asset) =>
        asset.folderId === action.folderId ? { ...asset, folderId: null } : asset,
      );
      return { ...project, media, mediaFolders };
    }
    case "renameMedia": {
      const name = action.name.trim();
      if (!name || !project.media.some((asset) => asset.id === action.mediaId)) {
        return project;
      }

      return {
        ...project,
        media: project.media.map((asset) => (asset.id === action.mediaId ? { ...asset, name } : asset)),
      };
    }
    case "deleteMedia": {
      const mediaIds = new Set(action.mediaIds.map((mediaId) => mediaId.trim()));
      const referencedByGeneration = (mediaId: string) =>
        project.generatedAssets.some(
          (asset) =>
            asset.references.mediaIds.includes(mediaId) ||
            asset.references.firstFrameMediaId === mediaId ||
            asset.references.lastFrameMediaId === mediaId ||
            asset.outputs.some((output) => output.mediaId === mediaId),
        );
      if (
        mediaIds.size === 0 ||
        [...mediaIds].some(
          (mediaId) => !project.media.some((asset) => asset.id === mediaId) || referencedByGeneration(mediaId),
        )
      ) {
        return project;
      }

      const tracks = project.timeline.tracks.map((track) => ({
        ...track,
        items: track.items.filter((item) => item.source.type !== "media" || !mediaIds.has(item.source.mediaId)),
      }));
      return {
        ...projectWithTimelineTracks(project, tracks),
        media: project.media.filter((asset) => !mediaIds.has(asset.id)),
        transcripts: project.transcripts.filter((transcript) => !mediaIds.has(transcript.mediaId)),
      };
    }
    case "recordJob": {
      if (project.jobs.some((job) => job.id === action.job.id)) {
        return project;
      }

      return { ...project, jobs: [...project.jobs, action.job] };
    }
    case "updateJobStatus": {
      let changed = false;
      const jobs = project.jobs.map((job) => {
        if (job.id !== action.jobId) {
          return job;
        }
        if (job.status === "cancelled" && action.status !== "cancelled") {
          return job;
        }
        // A run that already finished keeps its result: its late "started" write is stale.
        if (lateStartOfFinishedRun(job, action.status, action.runId)) {
          return job;
        }

        changed = true;
        const { failureReason, ...rest } = job;
        return {
          ...(action.status === "failed" && failureReason !== undefined
            ? { ...rest, failureReason }
            : rest),
          status: action.status,
          updatedAt: action.updatedAt,
          ...(job.workflow === undefined
            ? {}
            : {
                workflow:
                  job.workflow && action.runId !== undefined
                    ? { ...job.workflow, runId: action.runId }
                    : job.workflow,
              }),
        };
      });
      return changed ? { ...project, jobs } : project;
    }
    case "recordJobFailure": {
      validateJobFailure(project, action);
      let changed = false;
      const jobs = project.jobs.map((job) => {
        if (job.id !== action.jobId || !unfinishedJobStatuses.has(job.status)) {
          return job;
        }

        changed = true;
        return {
          ...job,
          status: "failed" as const,
          updatedAt: action.updatedAt,
          failureReason: action.reason.trim(),
          ...(job.workflow && action.runId
            ? { workflow: { ...job.workflow, runId: action.runId } }
            : {}),
        };
      });
      return changed ? { ...project, jobs } : project;
    }
    case "updateJobProviderRequest": {
      let changed = false;
      const jobs = project.jobs.map((job) => {
        if (job.id !== action.jobId) {
          return job;
        }

        changed = true;
        return { ...job, providerRequest: action.providerRequest };
      });
      return changed ? { ...project, jobs } : project;
    }
    case "recordGeneratedAsset": {
      if (project.generatedAssets.some((asset) => asset.id === action.asset.id)) {
        return project;
      }

      return {
        ...project,
        generatedAssets: [
          ...project.generatedAssets,
          generatedAssetFromProjectAction(action.asset),
        ],
      };
    }
    case "updateGeneratedAssetStatus": {
      let changed = false;
      const generatedAssets = project.generatedAssets.map((asset) => {
        if (asset.id !== action.assetId) {
          return asset;
        }
        if (asset.status === "cancelled" && action.status !== "cancelled") {
          return asset;
        }

        changed = true;
        return { ...asset, status: action.status };
      });
      return changed ? { ...project, generatedAssets } : project;
    }
    case "updateGeneratedAssetReferences": {
      let changed = false;
      const generatedAssets = project.generatedAssets.map((asset) => {
        if (asset.id !== action.assetId) {
          return asset;
        }

        changed = true;
        return { ...asset, references: action.references };
      });
      return changed ? { ...project, generatedAssets } : project;
    }
    case "editTranscriptWords":
      return editTranscriptWordsLocally(project, action.edits) ?? project;
    case "applyCaptionRepair":
      return applyCaptionRepairLocally(project, action.repair) ?? project;
    case "addTransition":
    case "updateTransition":
    case "removeTransition": {
      const result = applyTransitionAction(project, action);
      return "error" in result ? project : projectWithTimelineTracks(project, result.tracks);
    }
    default:
      return project;
  }
}

/** Rust `validate_transcript_word_range`. */
function transcriptWordRangeIsValid(word: TranscriptWord, mediaDurationSeconds: number): boolean {
  return (
    Number.isFinite(word.startSeconds) &&
    word.startSeconds >= 0 &&
    Number.isFinite(word.endSeconds) &&
    word.endSeconds >= word.startSeconds &&
    word.endSeconds <= mediaDurationSeconds
  );
}

/** Rust `validate_transcript_timing`: each word starts at or after the previous word ends. */
function transcriptTimingIsMonotonic(words: readonly TranscriptWord[]): boolean {
  return words.every((word, index) => index === 0 || word.startSeconds >= words[index - 1]!.endSeconds);
}

/** Rust `transcript_repair_kind`: an unchanged word still counts as a timing repair. */
function transcriptRepairKind(before: TranscriptWord, after: TranscriptWord): TranscriptRepairKind {
  const nearlyEqual = (left: number, right: number) => Math.abs(left - right) <= 0.000_001;
  const textChanged = before.text !== after.text;
  const timingChanged =
    !nearlyEqual(before.startSeconds, after.startSeconds) ||
    !nearlyEqual(before.endSeconds, after.endSeconds);
  if (textChanged && timingChanged) return "word_text_and_timing";
  return textChanged ? "word_text" : "word_timing";
}

function transcriptWordIndexIsValid(transcript: Transcript, wordIndex: number): boolean {
  return Number.isInteger(wordIndex) && wordIndex >= 0 && wordIndex < transcript.words.length;
}

function mediaDurationSeconds(project: VideoProject, mediaId: string): number | null {
  return project.media.find((media) => media.id === mediaId)?.durationSeconds ?? null;
}

/**
 * Rust `edit_transcript_words`: every edit applies in order to a copy of the transcripts, and any
 * invalid edit rejects the whole batch (`null`).
 */
function editTranscriptWordsLocally(
  project: VideoProject,
  edits: readonly ProjectActionTranscriptWordEdit[],
): VideoProject | null {
  if (edits.length === 0) return null;

  const transcripts = [...project.transcripts];
  for (const edit of edits) {
    const transcriptIndex = transcripts.findIndex((transcript) => transcript.id === edit.transcriptId);
    const transcript = transcripts[transcriptIndex];
    if (!transcript) return null;
    const mediaDuration = mediaDurationSeconds(project, transcript.mediaId);
    if (mediaDuration === null || !transcriptWordIndexIsValid(transcript, edit.wordIndex)) return null;

    const before = transcript.words[edit.wordIndex]!;
    const after: TranscriptWord = { ...before };
    if (edit.text !== undefined) {
      const text = edit.text.trim();
      if (text.length === 0) return null;
      after.text = text;
    }
    if (edit.startSeconds !== undefined) after.startSeconds = edit.startSeconds;
    if (edit.endSeconds !== undefined) after.endSeconds = edit.endSeconds;
    if (!transcriptWordRangeIsValid(after, mediaDuration)) return null;

    const words = transcript.words.map((word, index) => (index === edit.wordIndex ? after : word));
    if (!transcriptTimingIsMonotonic(words)) return null;
    transcripts[transcriptIndex] = {
      ...transcript,
      words,
      repairs: [
        ...transcript.repairs,
        {
          id: edit.repairId,
          kind: transcriptRepairKind(before, after),
          wordIndex: edit.wordIndex,
          before,
          after,
          createdAt: edit.createdAt,
        },
      ],
    };
  }

  return { ...project, transcripts };
}

/**
 * Rust `apply_caption_repair`: the caption cue takes the repaired text and timing and is tagged
 * with the repair, and the transcript word is replaced and its repair recorded. Invalid repairs
 * return `null`.
 */
function applyCaptionRepairLocally(
  project: VideoProject,
  repair: ProjectActionCaptionRepair,
): VideoProject | null {
  const text = repair.text.trim();
  if (text.length === 0) return null;
  if (!Number.isFinite(repair.startSeconds) || repair.startSeconds < 0) return null;
  if (!Number.isFinite(repair.endSeconds) || repair.endSeconds <= repair.startSeconds) return null;

  const track = project.timeline.tracks.find((candidate) =>
    candidate.items.some((item) => item.id === repair.captionItemId),
  );
  const caption = track?.items.find((item) => item.id === repair.captionItemId);
  if (!track || !caption || track.locked) return null;
  if (caption.kind !== "caption" || caption.source.type !== "text") return null;

  const transcriptIndex = project.transcripts.findIndex((transcript) => transcript.id === repair.transcriptId);
  const transcript = project.transcripts[transcriptIndex];
  if (!transcript) return null;
  const mediaDuration = mediaDurationSeconds(project, transcript.mediaId);
  if (mediaDuration === null || !transcriptWordIndexIsValid(transcript, repair.wordIndex)) return null;

  const before = transcript.words[repair.wordIndex]!;
  const after: TranscriptWord = {
    text,
    startSeconds: repair.startSeconds,
    endSeconds: repair.endSeconds,
    confidence: before.confidence ?? null,
    speaker: before.speaker ?? null,
  };
  if (!transcriptWordRangeIsValid(after, mediaDuration)) return null;
  const words = transcript.words.map((word, index) => (index === repair.wordIndex ? after : word));
  if (!transcriptTimingIsMonotonic(words)) return null;

  const repairedCaption: TimelineItem = {
    ...caption,
    startSeconds: repair.startSeconds,
    durationSeconds: repair.endSeconds - repair.startSeconds,
    source: { type: "text", text },
    properties: {
      ...caption.properties,
      captionRepairId: repair.repairId,
      transcriptId: repair.transcriptId,
      wordIndex: repair.wordIndex,
      textEdited: true,
    },
  };
  const tracks = project.timeline.tracks.map((candidate) =>
    candidate === track
      ? {
          ...candidate,
          items: candidate.items.map((item) => (item === caption ? repairedCaption : item)),
        }
      : candidate,
  );
  const transcripts = project.transcripts.map((candidate, index) =>
    index === transcriptIndex
      ? {
          ...candidate,
          words,
          repairs: [
            ...candidate.repairs,
            {
              id: repair.repairId,
              kind: transcriptRepairKind(before, after),
              wordIndex: repair.wordIndex,
              before,
              after,
              createdAt: repair.createdAt,
            },
          ],
        }
      : candidate,
  );

  return projectWithTimelineTracks({ ...project, transcripts }, tracks);
}

export async function importMediaToProject(input: {
  projectDir: string;
  project: VideoProject;
  sourcePaths: string[];
  /** Display names by source path, such as a saved timeline range's name. */
  names?: Record<string, string>;
}): Promise<ImportMediaResult> {
  return backendRequest("import_media_to_project", input);
}

export interface CreateMatteCommandResult {
  project: VideoProject;
  media: MediaAsset;
}

export async function createMatteInSplitProjectFolder(input: {
  projectDir: string;
  request: {
    hex: string;
    aspectRatio: string;
    name?: string;
    folderId?: string;
  };
}): Promise<CreateMatteCommandResult> {
  return backendRequest("create_matte_in_split_project_folder", input);
}

export async function saveSplitProjectToFolder(input: {
  projectDir: string;
  project: VideoProject;
  expectedRevision: number;
  /** False restores an existing canonical snapshot without activating a desktop session. */
  activateProject?: boolean;
}): Promise<ProjectActionWriteResult> {
  return backendRequest("save_split_project_to_folder", input);
}

export async function loadSplitProjectFromFolder(input: {
  projectDir: string;
}): Promise<VideoProject> {
  return backendRequest("load_split_project_from_folder", input);
}

/** A running job's lease-free progress snapshot (0–1); bookkeeping outside the project and undo. */
export interface JobProgressSnapshot {
  jobId: string;
  progress: number;
  updatedAt: string;
}

/** The outcome of reconciling unfinished Temporal jobs with their workflows. */
export interface TemporalJobReconciliation {
  /** The project after recording failures; null when the workflow service was unreachable. */
  project: VideoProject | null;
  failedJobIds: string[];
  serviceReachable: boolean;
  detail: string | null;
}

/** Reads progress snapshots without waiting for the project lease a render holds. */
export async function loadJobProgressFromSplitProjectFolder(input: {
  projectDir: string;
}): Promise<JobProgressSnapshot[]> {
  return backendRequest("load_job_progress_from_split_project_folder", input);
}

/** Fails unfinished Temporal jobs whose workflows are closed, missing or never started. */
export async function reconcileTemporalJobsInSplitProjectFolder(input: {
  projectDir: string;
  updatedAt: string;
}): Promise<TemporalJobReconciliation> {
  return backendRequest("reconcile_temporal_jobs_in_split_project_folder", input);
}

interface PrecomposePreviewReport {
  stage: string;
  itemId: string;
  mediaId: string;
  preparedMediaId: string;
  fingerprint: string;
  cacheHit: boolean;
  expressionsEnabled: boolean;
  compositorBackend: string | null;
  compositorFallback: string | null;
  workerManifest: string;
  intermediate: string;
}

export interface PreparedProjectPreview {
  project: VideoProject;
  reports: PrecomposePreviewReport[];
  frameSequences: PreparedPreviewFrameSequence[];
}

interface PreparedPreviewFrameSequence {
  itemId: string;
  preparedMediaId: string;
  startSeconds: number;
  durationSeconds: number;
  fps: number;
  framePaths: string[];
}

export async function prepareProjectPreview(input: {
  projectDir: string;
  project: VideoProject;
}): Promise<PreparedProjectPreview> {
  return backendRequest("prepare_project_preview", input);
}

/** Paths are relative to the project folder; `previewFrame` is the captured PNG. */
export interface PreparedPreviewFrameResult {
  project: VideoProject;
  playheadSeconds: number;
  previewFrame: string;
  sourceOutput: string;
  evidenceReport: string;
  renderReport: RenderReport;
  projectRenderReport: ProjectRenderReport;
}

/**
 * Captures one frame of the saved split project's primary timeline: a one-frame native render on
 * macOS, the Rust canonical frame sampler with graphics overlays elsewhere. Image clips, text sources
 * and unfinished generations can't be captured on Linux yet, and the call rejects them.
 * `playheadSeconds` must be before the timeline end; `jobId` names the render folder and job record.
 */
export function captureCanonicalPreviewFrameInSplitProjectFolder(input: {
  projectDir: string;
  playheadSeconds: number;
  jobId: string;
  updatedAt: string;
}): Promise<PreparedPreviewFrameResult> {
  return backendRequest("capture_canonical_preview_frame_in_split_project_folder", input);
}

export function projectNeedsCanonicalPreview(project: VideoProject): boolean {
  const lottieMediaIds = new Set(
    project.media.filter((asset) => asset.kind === "lottie").map((asset) => asset.id),
  );
  return [project.timeline, ...(project.timelines ?? []).map((entry) => entry.timeline)].some((timeline) => timeline.tracks.some((track) =>
    track.enabled !== false && (
    // Reversed video and audio play from prepared reversed intermediates.
    track.items.some(isReversedItem) ||
    track.kind === "video" &&
    track.items.some((item) => {
      if (item.source.type === "media" && lottieMediaIds.has(item.source.mediaId)) {
        return true;
      }
      const blendMode = item.properties.blendMode;
      if (typeof blendMode === "string" && !["normal", "over"].includes(blendMode)) {
        return true;
      }
      const effects = item.properties.effects;
      if (Array.isArray(effects) && effects.some((effect) =>
        effect !== null && typeof effect === "object" && !Array.isArray(effect) &&
        (effect as Record<string, unknown>).enabled !== false)) {
        return true;
      }
      const colorGrade = item.properties.colorGrade;
      return (
        colorGrade !== null &&
        typeof colorGrade === "object" &&
        !Array.isArray(colorGrade) &&
        Object.keys(colorGrade).length > 0
      );
    })),
  ));
}

export async function validateSplitProjectFolder(input: {
  projectDir: string;
}): Promise<ProjectValidationReport> {
  return backendRequest("validate_split_project_folder", input);
}

export async function loadAppServerConversationHistoryFromSplitProjectFolder(input: {
  projectDir: string;
}): Promise<AppServerConversationHistory> {
  return backendRequest("load_app_server_conversation_history_from_split_project_folder", input);
}

export async function loadAgentSessionsFromSplitProjectFolder(input: {
  projectDir: string;
}): Promise<ProjectAgentSessionManifest> {
  return backendRequest("load_agent_sessions_from_split_project_folder", input);
}

export async function applyAgentSessionActionToSplitProjectFolder(input: {
  projectDir: string;
  projectId: string;
  action: ProjectAgentSessionAction;
}): Promise<ProjectAgentSessionManifest> {
  return backendRequest("apply_agent_session_action_to_split_project_folder", input);
}

export async function searchProjectMedia(input: {
  projectDir: string;
  query: string;
  limit?: number;
  scope?: ProjectMediaSearchScope;
  mediaId?: string;
}): Promise<ProjectMediaSearchResult> {
  return backendRequest("search_project_media", input);
}

export interface ProjectSearchIndexRebuildReport {
  projectId: string;
  indexPath: string;
  mediaCount: number;
  transcriptCount: number;
  generatedAssetCount: number;
}

export async function rebuildProjectSearchIndex(input: {
  projectDir: string;
}): Promise<ProjectSearchIndexRebuildReport> {
  return backendRequest("rebuild_project_search_index", input);
}

export interface TimelineFilmstripReport {
  mediaId: string;
  sourceFingerprint: string;
  cacheKey: string;
  cacheHit: boolean;
  sourceIn: number;
  sourceOut: number;
  speed: number;
  zoomBucket: number;
  heightBucket: number;
  samplingPolicy: string;
  frames: Array<{ timeSeconds: number; relativePath: string }>;
}

export async function cacheTimelineFilmstripInSplitProjectFolder(input: {
  projectDir: string;
  mediaId: string;
  sourceIn: number;
  sourceOut: number;
  speed: number;
  zoomBucket: number;
  heightBucket: number;
  clipPixelWidth: number;
}): Promise<TimelineFilmstripReport> {
  return backendRequest("cache_timeline_filmstrip_in_split_project_folder", input);
}

export async function migrateSingleFileProjectToSplit(input: {
  projectDir: string;
}): Promise<ProjectWriteReport> {
  return backendRequest("migrate_single_file_project_to_split", input);
}

export async function buildTemporalGenerateMediaStartRequest(input: {
  projectId: string;
  projectDir: string;
  assetId: string;
  jobId: string;
  mockMode: boolean;
} & TemporalGenerateMediaBrief): Promise<TemporalWorkflowStartRequest> {
  return backendRequest("build_temporal_generate_media_start_request", { ...input });
}

export async function buildTemporalCodexEditStartRequest(input: {
  projectId: string;
  projectRoot?: string;
  projectDir: string;
  jobId: string;
  request: EditJobRequest;
}): Promise<TemporalWorkflowStartRequest> {
  return backendRequest("build_temporal_codex_edit_start_request", input);
}

export async function buildTemporalTranscribeMediaStartRequest(input: {
  projectId: string;
  projectDir: string;
  mediaId: string;
  jobId: string;
  languageMode: string;
}): Promise<TemporalWorkflowStartRequest> {
  return backendRequest("build_temporal_transcribe_media_start_request", input);
}

export interface ExportMediaStartInput extends ExportEncodeOptions {
  projectId: string;
  projectDir: string;
  jobId: string;
  profile: Exclude<ExportProfile, "palmierProject">;
  quality: ExportQuality;
  width: number;
  height: number;
  outputPath: string;
}

export async function buildTemporalExportMediaStartRequest(
  input: ExportMediaStartInput,
): Promise<TemporalWorkflowStartRequest> {
  return backendRequest("build_temporal_export_media_start_request", { ...input });
}

export async function buildTemporalExportProjectBundleStartRequest(input: {
  projectId: string;
  projectDir: string;
  jobId: string;
  outputPath: string;
}): Promise<TemporalWorkflowStartRequest> {
  return backendRequest("build_temporal_export_project_bundle_start_request", input);
}

export async function buildTemporalExportNleXmlStartRequest(input: {
  projectId: string;
  projectDir: string;
  jobId: string;
  format: NleXmlExportFormat;
  outputPath: string;
}): Promise<TemporalWorkflowStartRequest> {
  return backendRequest("build_temporal_export_nle_xml_start_request", input);
}

export async function renderMediaToSplitProjectFolder(input: ExportEncodeOptions & {
  projectDir: string;
  projectId: string;
  profile: Exclude<ExportProfile, "palmierProject">;
  quality: ExportQuality;
  width: number;
  height: number;
  jobId: string;
  attemptId: string;
  updatedAt: string;
  rangeStartSeconds?: number;
  rangeEndSeconds?: number;
  timelineId?: string;
  /** What the job records for Retry (`ExportStartPlan.settings`); used with `output`. */
  exportSettings?: JobExportSettings;
  /** Admission validates this revision before durably queuing immutable render input. */
  expectedRevision?: number;
}, options: { onAdmitted?: (admission: MediaRenderAdmission) => void | Promise<void> } = {}): Promise<ProjectMediaRenderResult> {
  const response = await backendRequest<ProjectMediaRenderResult | MediaRenderAdmission>(renderJobOperations.admit, {
    ...input,
    ...(input.expectedRevision === undefined ? {} : { admissionProtocol: 1 }),
  });
  if (!("admissionProtocol" in response)) return response;
  const admission = parseRenderAdmission(response, input.jobId, input.attemptId);
  await options.onAdmitted?.(admission);
  // Each poll is a separate short request; the accepted render never owns the editor queue.
  for (;;) {
    const attempt = parseRenderAttempt(await backendRequest<unknown>(renderJobOperations.attempt, {
      projectDir: input.projectDir, jobId: input.jobId, attemptId: input.attemptId,
    }));
    if (attempt.status === "completed") return attempt.result;
    if (attempt.status === "failed") {
      if (attempt.interrupted) await backendRequest(renderJobOperations.recover, { projectDir: input.projectDir, jobId: input.jobId, attemptId: input.attemptId });
      throw new Error(attempt.message);
    }
    if (attempt.status !== "pending") throw new Error("Render attempt response is invalid.");
    await new Promise<void>((resolve) => setTimeout(resolve, 500));
  }
}

export interface MediaRenderAdmission {
  admissionProtocol: 1;
  project: VideoProject;
  jobId: string;
  attemptId: string;
  sourceRevision: number;
}

export async function readProjectSnapshotFromSplitProjectFolder(input: { projectDir: string }): Promise<VideoProject> {
  return backendRequest("read_project_snapshot_from_split_project_folder", input);
}

export type MediaRenderAttempt =
  | { status: "pending" }
  | { status: "completed"; result: ProjectMediaRenderResult }
  | { status: "failed"; message: string; interrupted?: boolean };

export async function cancelRenderJobInSplitProjectFolder(input: {
  projectDir: string;
  jobId: string;
  attemptId: string;
  updatedAt: string;
}): Promise<ProjectActionWriteResult> {
  return backendRequest("cancel_render_job_in_split_project_folder", input);
}

export async function loadRenderPipelineReportFromSplitProjectFolder(input: {
  projectDir: string;
  jobId: string;
}): Promise<RenderReport> {
  return backendRequest("load_render_pipeline_report_from_split_project_folder", input);
}

export async function runPreviewRenderComparisonRequestInSplitProjectFolder(input: {
  projectDir: string;
  request: RenderPreviewComparisonRequest;
  updatedAt: string;
}): Promise<ProjectPreviewRenderComparisonRunResult> {
  return backendRequest("run_preview_render_comparison_request_in_split_project_folder", input);
}

export async function buildTemporalStartResultAction(input: {
  job: ProjectJobSummary;
  runId: string;
  updatedAt: string;
}): Promise<ProjectAction> {
  return backendRequest("build_temporal_start_result_action", input);
}

export async function startTemporalWorkflow(input: {
  job: ProjectJobSummary;
}): Promise<TemporalWorkflowStartResult> {
  return backendRequest("start_temporal_workflow", input);
}

export async function runGenerateMediaInProcess(input: {
  startRequest: TemporalWorkflowStartRequest;
  updatedAt: string;
}): Promise<VideoProject> {
  return backendRequest("run_generate_media_in_process", input);
}

export async function getTemporalWorkerEnvironmentReport(): Promise<TemporalWorkerEnvironmentReport> {
  return backendRequest("get_temporal_worker_environment_report");
}

export async function getExportProfileAvailabilityReport(): Promise<ExportProfileAvailability[]> {
  return backendRequest("get_export_profile_availability_report");
}

export type GenerationModelUiCapabilities =
  | {
      durations: number[];
      resolutions: string[] | null;
      aspectRatios: string[];
      supportsFirstFrame: boolean;
      supportsLastFrame: boolean;
      maxReferenceImages: number;
      maxReferenceVideos: number;
      maxReferenceAudios: number;
      maxTotalReferences: number | null;
      maxCombinedVideoRefSeconds: number | null;
      maxCombinedAudioRefSeconds: number | null;
      framesAndReferencesExclusive: boolean;
      referenceTagNoun: string;
      requiresSourceVideo: boolean;
      requiresReferenceImage: boolean;
    }
  | {
      resolutions: string[] | null;
      aspectRatios: string[];
      qualities: string[] | null;
      supportsImageReference: boolean;
      maxImages: number;
    }
  | {
      category: string;
      voices: string[] | null;
      defaultVoice: string | null;
      supportsLyrics: boolean;
      supportsInstrumental: boolean;
      supportsStyleInstructions: boolean;
      durations: number[] | null;
      minPromptLength: number;
      inputs: string[];
      promptLabel: string;
      minSeconds: number;
      maxSeconds: number;
    }
  | {
      speed: string;
      p75DurationSeconds: number;
      supportedTypes: string[];
    };

export interface GenerationModelCatalogPayload {
  loaded: boolean;
  generationCatalog?: {
    source: "remote" | "cache" | "builtin" | string;
    catalogVersion: string;
    capabilitiesVersion: string;
    stale: boolean;
    cacheStatus: string;
    hashVerified: boolean;
    signatureVerified: boolean;
    remoteConfigured: boolean;
  };
  generationModels: Array<{
    provider: string;
    id: string;
    kind?: string | null;
    allowedEndpoints: string[];
    responseShape: "video" | "images" | "audio" | "upscaledImage";
    cancellationCapability?: "provider" | "local" | "none";
    uiCapabilities: GenerationModelUiCapabilities;
    paidOnly: boolean;
    [key: string]: unknown;
  }>;
  providerCredentialsExposed: boolean;
}

export async function listGenerationModelCatalog(): Promise<GenerationModelCatalogPayload> {
  return backendRequest("list_generation_model_catalog");
}

export async function buildTemporalGenerateMediaFailureActions(input: {
  job: ProjectJobSummary;
  assetId: string;
  runId?: string | null;
  updatedAt: string;
}): Promise<ProjectAction[]> {
  return backendRequest("build_temporal_generate_media_failure_actions", input);
}

export interface CancelGenerateMediaProviderRequestResult {
  requestId: string;
  cancelUrl: string;
  status: "CANCELLATION_REQUESTED" | "ALREADY_COMPLETED" | "NOT_FOUND";
}

export interface CancelInProcessGenerationResult {
  outcome: "cancelled" | "alreadyCancelled" | "alreadyTerminal";
  project: VideoProject;
}

export async function cancelGenerateMediaInProcess(input: {
  projectDir: string;
  jobId: string;
  updatedAt: string;
}): Promise<CancelInProcessGenerationResult> {
  return backendRequest("cancel_generate_media_in_process", input);
}

export async function cancelGenerateMediaProviderRequestInSplitProjectFolder(input: {
  projectDir: string;
  jobId: string;
}): Promise<CancelGenerateMediaProviderRequestResult> {
  return backendRequest("cancel_generate_media_provider_request_in_split_project_folder", input);
}

export async function applyProjectActionToProject(input: {
  project: VideoProject;
  action: ProjectAction;
}): Promise<VideoProject> {
  return backendRequest("apply_project_action_to_project", input);
}

export async function applyProjectActionToSplitProjectFolder(input: {
  projectDir: string;
  expectedRevision?: number;
  action: ProjectAction;
}): Promise<ProjectActionWriteResult> {
  return backendRequest("apply_project_action_to_split_project_folder", input);
}

export async function updateProjectSettingsInSplitProjectFolder(input: {
  projectDir: string;
  name: string;
  renderSettings: VideoProject["renderSettings"];
}): Promise<ProjectActionWriteResult> {
  return backendRequest("update_project_settings_in_split_project_folder", input);
}

export async function applyProjectActionsToSplitProjectFolder(input: {
  projectDir: string;
  expectedRevision?: number;
  actions: ProjectAction[];
}): Promise<ProjectActionWriteResult> {
  return backendRequest("apply_project_actions_to_split_project_folder", input);
}

export async function completeMockGeneratedAssetInSplitProjectFolder(input: {
  projectDir: string;
  assetId: string;
  updatedAt: string;
  replacementItemId?: string | null;
}): Promise<ProjectActionWriteResult> {
  return backendRequest("complete_mock_generated_asset_in_split_project_folder", input);
}

export async function retryGeneratedAssetOutputDownloadInSplitProjectFolder(input: {
  projectDir: string;
  assetId: string;
  outputMediaId: string;
}): Promise<ProjectActionWriteResult> {
  return backendRequest("retry_generated_asset_output_download_in_split_project_folder", input);
}

export async function exportNleXmlToSplitProjectFolder(input: {
  projectDir: string;
  format: NleXmlExportFormat;
  jobId: string;
  updatedAt: string;
  timelineId?: string;
}): Promise<NleXmlExportCommandResult> {
  return backendRequest("export_nle_xml_to_split_project_folder", input);
}

export async function exportPalmierProjectPackageToSplitProjectFolder(input: {
  projectDir: string;
  jobId: string;
  outputPath: string;
  updatedAt: string;
}): Promise<NleXmlExportCommandResult> {
  return backendRequest("export_palmier_project_package_to_split_project_folder", input);
}

/**
 * "Show in folder" for a file the project records as export output: an `exportArtifacts[].path`, or
 * a render report's `outputPath` or `logPath`. `artifactPath` is project-relative, or absolute under
 * `projectDir`. An export saved to a folder outside the project is revealed by its exact recorded
 * absolute artifact path, when that file still exists and isn't a symlink. Anything else rejects
 * with "This file isn't a recorded export of this project."
 */
export async function revealExportArtifactInSplitProjectFolder(input: {
  projectDir: string;
  artifactPath: string;
}): Promise<void> {
  return backendRequest("reveal_export_artifact_in_split_project_folder", input);
}

export async function startCodexVideoEditForProject(input: {
  projectRoot?: string;
  projectDir?: string;
  project: VideoProject;
  request: EditJobRequest;
}): Promise<CodexVideoEditCommandResult> {
  return backendRequest("start_codex_video_edit_for_project", input);
}

export async function startCodexConversationEditForProject(input: {
  projectRoot?: string;
  projectDir?: string;
  project: VideoProject;
  request: CodexConversationEditRequest;
}): Promise<CodexConversationEditCommandResult> {
  return backendRequest("start_codex_conversation_edit_for_project", input);
}

/** Rust re-prepares the proposal and refuses stale IDs or unapproved review-level bundles. */
export async function applyCodexConversationProposal(input: {
  projectDir: string;
  proposal: CodexConversationEditProposal;
  actionIds: string[];
  reviewApproved: boolean;
  sessionId?: string | null;
}): Promise<ProjectAgentApplyResult> {
  return backendRequest("apply_codex_conversation_proposal", input);
}

export async function undoLatestCodexConversationEdit(input: {
  projectDir: string;
  historyEntryId?: string | null;
}): Promise<ProjectAgentUndoOutcome> {
  return backendRequest("undo_latest_codex_conversation_edit", input);
}

export function materializeSampleProjectMedia(input: {
  projectDir: string;
}): Promise<void> {
  return backendRequest("materialize_sample_project_media", input);
}

export function requestTemporalJobSummary(input: {
  kind: string;
  projectId: string;
  jobId: string;
  status: ProjectJobStatus;
  updatedAt: string;
}): Promise<ProjectJobSummary> {
  return backendRequest("build_temporal_job_summary", input);
}

export interface ProjectSpeakerIdentity {
  id: string;
  name: string;
  color: string;
}

export interface ProjectSpeakerRegistry {
  speakers?: ProjectSpeakerIdentity[];
}

export function getProjectSpeakerRegistry(input: {
  projectDir: string;
}): Promise<ProjectSpeakerRegistry> {
  return backendRequest("get_project_speaker_registry", input);
}

export function analyzeProjectSpeech(input: {
  projectDir: string;
  mediaId: string;
  preparedPcmPath: string;
}): Promise<void> {
  return backendRequest("analyze_project_speech", input);
}

export function renameProjectSpeaker(input: {
  projectDir: string;
  speakerId: string;
  name: string;
}): Promise<ProjectSpeakerRegistry> {
  return backendRequest("rename_project_speaker", input);
}

export async function cancelCodexVideoEditForProject(input: {
  projectRoot?: string;
  projectDir?: string;
}): Promise<boolean> {
  return backendRequest("cancel_codex_video_edit_for_project", input);
}

/**
 * Stops the project's in-flight Codex turn; resolves `false` when none was running.
 * The stopped start call rejects with "app-server turn was interrupted" and persists nothing.
 */
export async function cancelCodexConversationEditForProject(input: {
  projectRoot?: string;
  projectDir?: string;
}): Promise<boolean> {
  return backendRequest("cancel_codex_conversation_edit_for_project", input);
}
