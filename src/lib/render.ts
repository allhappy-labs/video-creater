export type RenderQualityProfile = "draftWebm" | "finalWebm";

export interface RenderQualityOption {
  value: RenderQualityProfile;
  label: string;
  description: string;
}

interface RenderGraphicsReport {
  layerId: string;
  renderer: "gpu" | "software" | string;
  qualityProfile: string | null;
  templateId: string | null;
  motionPresetId: string | null;
  visualQaStatus: string | null;
  cacheStatus?: string | null;
  sampledFrames: string[];
  qaMetrics: Record<string, string>;
}

interface RenderReportSummary {
  status: string;
  durationSeconds: number | null;
  outputPath: string | null;
  quality?: "draft" | "final" | null;
  requestedWidth?: number | null;
  requestedHeight?: number | null;
  actualWidth?: number | null;
  actualHeight?: number | null;
  container?: string | null;
  videoCodec?: string | null;
  audioCodec?: string | null;
}

export interface RenderCommandSpec {
  program: string;
  args: string[];
}

interface RenderPipelineError {
  code: string;
  path: string;
  message: string;
  fix: string;
  details?: Record<string, string>;
}

interface RenderStageReport {
  name: string;
  status: string;
  durationMs: number;
  details: Record<string, string>;
}

interface RenderPerformanceSummary {
  totalDurationMs: number;
  stages: RenderStageReport[];
}

interface RenderPreviewComparisonFrame {
  timelineSeconds: number;
  previewFrame: string;
  renderedFrame: string;
  diffFrame: string | null;
  mismatchRatio: number;
  passed: boolean;
}

export interface RenderPreviewComparison {
  status: "passed" | "failed" | "skipped" | string;
  comparedFrames: RenderPreviewComparisonFrame[];
}

export interface RenderPreviewComparisonRequest {
  status: "pending" | "running" | "completed" | "failed" | string;
  projectDir: string;
  projectReportId: string;
  renderReportPath: string;
  renderedVideo: string;
  durationSeconds: number;
  frameTimeSeconds: number;
  renderedFrames: string[];
  failOnMismatch?: boolean;
}

export interface RenderReport {
  jobId: string;
  summary: RenderReportSummary;
  command: RenderCommandSpec;
  stdout: string;
  stderr: string;
  streams?: {
    video: boolean;
    audio: boolean;
  } | null;
  errors: RenderPipelineError[];
  artifacts: string[];
  graphics: RenderGraphicsReport[];
  performance?: RenderPerformanceSummary | null;
  previewComparisonRequest?: RenderPreviewComparisonRequest | null;
  previewComparison?: RenderPreviewComparison | null;
}

export const renderQualityOptions: RenderQualityOption[] = [
  {
    value: "draftWebm",
    label: "Draft WebM",
    description: "Fast iteration render for reviewing timing, captions, and graphics.",
  },
  {
    value: "finalWebm",
    label: "Final WebM",
    description: "Finished WebM render intent for accepted edits.",
  },
];

export function renderQualityProfileLabel(profile: RenderQualityProfile): string {
  return renderQualityOptions.find((option) => option.value === profile)?.label ?? profile;
}

export function commandQualityProfile(command: RenderCommandSpec): RenderQualityProfile | null {
  for (const arg of command.args) {
    if (arg === "--quality=draftWebm") {
      return "draftWebm";
    }
    if (arg === "--quality=finalWebm") {
      return "finalWebm";
    }
  }
  return null;
}

export function buildSampleRenderReport(profile: RenderQualityProfile): RenderReport {
  const final = profile === "finalWebm";
  const outputPath = final ? "renders/final.webm" : "renders/draft.webm";

  return {
    jobId: final ? "sample-final-render" : "sample-draft-render",
    summary: {
      status: "ready",
      durationSeconds: 4,
      outputPath,
    },
    command: {
      program: "gstreamer-ges",
      args: [
        "--input=source.mp4",
        `--output=${outputPath}`,
        "--size=1920x1080",
        "--fps=24.000",
        `--quality=${profile}`,
      ],
    },
    stdout: "",
    stderr: "",
    streams: {
      video: true,
      audio: true,
    },
    errors: [],
    artifacts: [
      outputPath,
      "renders/graphics/shader-hook-bg/preview.png",
      "renders/graphics/shader-hook-bg/manifest.json",
      "renders/graphics/shader-hook-bg/frames/frame-000000.png",
    ],
    graphics: [
      {
        layerId: "shader-hook-bg",
        renderer: "gpu",
        qualityProfile: "hq-neon-wireframe-shader-v1",
        templateId: null,
        motionPresetId: null,
        visualQaStatus: "passed",
        cacheStatus: "miss:sample",
        sampledFrames: [
          "frames/frame-000000.png",
          "frames/frame-000012.png",
          "frames/frame-000023.png",
        ],
        qaMetrics: {
          opaqueRatio: "0.320000",
          saturatedRatio: "0.120000",
          edgeRatio: "0.004000",
          temporalDelta: "0.006500",
        },
      },
    ],
  };
}
