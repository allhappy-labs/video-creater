import { useEffect, useRef, useState } from "react";
import {
  artifactBackedFrameCount,
  deliveryQualityLabel,
  formatMismatchRatio,
  formatTimelineSeconds,
  qaMetricEntries,
  renderCheckEntries,
  renderCheckLabel,
} from "@/lib/export/render-report";
import type { TaskRecord } from "@/lib/jobs/task-records";
import {
  loadRenderPipelineReportFromSplitProjectFolder,
  runPreviewRenderComparisonRequestInSplitProjectFolder,
  type ProjectRenderReport,
} from "@/lib/project";
import type { RenderPreviewComparison, RenderPreviewComparisonRequest, RenderReport } from "@/lib/render";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { useEditorStore } from "../store/editor-store-context";
import { DetailRow, DetailSection, errorMessage, StatusBadge, type LoadState } from "./task-detail-section";

/** The render review shown for either report shape: the project's render report or the pipeline report. */
interface RenderMetrics {
  readonly status: string;
  readonly durationSeconds: number | null;
  readonly outputPath: string | null;
  readonly quality: string | null;
  readonly resolution: string | null;
  readonly codecs: string | null;
  readonly renderTimeMs: number | null;
  readonly streams: { readonly video: boolean; readonly audio: boolean } | null;
  readonly checks: readonly (readonly [string, string])[];
  readonly errors: RenderReport["errors"];
  readonly graphics: RenderReport["graphics"];
  readonly artifactCount: number;
  readonly artifacts: readonly string[];
  readonly logPath: string | null;
  readonly comparison: RenderPreviewComparison | null;
  readonly comparisonRequest: RenderPreviewComparisonRequest | null;
}

function recordedMetrics(report: ProjectRenderReport): RenderMetrics {
  return {
    status: report.status,
    durationSeconds: report.durationSeconds,
    outputPath: report.outputPath || null,
    quality: null,
    resolution: null,
    codecs: null,
    renderTimeMs: null,
    streams: report.streams,
    checks: renderCheckEntries(report),
    errors: [],
    graphics: [],
    artifactCount: report.artifacts.length,
    artifacts: report.artifacts,
    logPath: report.logPath || null,
    comparison: report.previewComparison ?? null,
    comparisonRequest: report.previewComparisonRequest ?? null,
  };
}

function pipelineMetrics(report: RenderReport): RenderMetrics {
  const { summary } = report;
  const width = summary.actualWidth ?? summary.requestedWidth;
  const height = summary.actualHeight ?? summary.requestedHeight;
  const codecs = [summary.videoCodec, summary.audioCodec].filter(Boolean).join(" · ");
  return {
    status: summary.status,
    durationSeconds: summary.durationSeconds,
    outputPath: summary.outputPath,
    quality: deliveryQualityLabel(summary),
    resolution: width && height ? `${width} × ${height}` : null,
    codecs: codecs || null,
    renderTimeMs: report.performance?.totalDurationMs ?? null,
    streams: report.streams ?? null,
    checks: [],
    errors: report.errors,
    graphics: report.graphics,
    artifactCount: report.artifacts.length,
    artifacts: report.artifacts,
    logPath: null,
    comparison: report.previewComparison ?? null,
    comparisonRequest: report.previewComparisonRequest ?? null,
  };
}

function sentenceCase(value: string): string {
  const words = value.replace(/[_-]+/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

function comparisonSummary(comparison: RenderPreviewComparison | null): string {
  if (!comparison || comparison.comparedFrames.length === 0) return "The comparison finished without comparing any frames.";
  const failed = comparison.comparedFrames.filter((frame) => !frame.passed).length;
  const total = comparison.comparedFrames.length;
  return failed === 0 ? "Preview and render match." : `Preview and render differ on ${failed} of ${total} frames.`;
}

function isFinished(record: TaskRecord): boolean {
  return record.status === "completed" || record.status === "failed" || record.status === "cancelled";
}

/**
 * Render review for a render or export task, ported from the pre-cut render report panel: the
 * project's render report when recorded, otherwise the pipeline report loaded from the folder, plus
 * "Compare preview and render".
 */
export function TaskRenderReport({ record }: { readonly record: TaskRecord }) {
  const projectDir = useEditorStore((state) => state.projectDir);
  const schemaVersion = useEditorStore((state) => state.project.schemaVersion);
  const recorded = useEditorStore((state) => state.project.renderReports.find((report) => report.id === record.id) ?? null);
  const mergeLoadedProject = useEditorStore((state) => state.mergeLoadedProject);
  const splitProject = schemaVersion >= 2 && projectDir.trim().length > 0 && !projectDir.startsWith("browser://");
  const needsPipeline = recorded === null && splitProject && isFinished(record);
  const [pipeline, setPipeline] = useState<LoadState<RenderReport>>({ status: "loading" });
  const [compare, setCompare] = useState<LoadState<RenderPreviewComparison | null> | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    if (!needsPipeline) return;
    let cancelled = false;
    setPipeline({ status: "loading" });
    loadRenderPipelineReportFromSplitProjectFolder({ projectDir, jobId: record.id })
      .then((value) => !cancelled && setPipeline({ status: "loaded", value }))
      .catch((error: unknown) => {
        if (cancelled) return;
        const message = isBackendUnavailableError(error) ? "Render reports need the desktop app." : `The render report couldn't be loaded. ${errorMessage(error)}`;
        setPipeline({ status: "failed", message });
      });
    return () => {
      cancelled = true;
    };
  }, [needsPipeline, projectDir, record.id]);

  const metrics = recorded ? recordedMetrics(recorded) : needsPipeline && pipeline.status === "loaded" ? pipelineMetrics(pipeline.value) : null;

  async function runComparison(request: RenderPreviewComparisonRequest) {
    setCompare({ status: "loading" });
    try {
      const result = await runPreviewRenderComparisonRequestInSplitProjectFolder({ projectDir, request: { ...request, status: "pending" }, updatedAt: new Date().toISOString() });
      await mergeLoadedProject(result.project);
      if (!mounted.current) return;
      if (!recorded) setPipeline({ status: "loaded", value: result.renderReport });
      setCompare({ status: "loaded", value: result.renderReport.previewComparison ?? result.projectRenderReport.previewComparison ?? null });
    } catch (error) {
      if (mounted.current) setCompare({ status: "failed", message: `The comparison couldn't run. ${errorMessage(error)}` });
    }
  }

  if (!metrics) {
    const message = !isFinished(record)
      ? "The render report appears when the render finishes."
      : !splitProject
        ? "Render reports are kept in the project folder. Save the project to a folder to review renders."
        : pipeline.status === "failed"
          ? pipeline.message
          : "Loading the render report…";
    return (
      <DetailSection title="Render report">
        <DetailRow label="Report">{message}</DetailRow>
      </DetailSection>
    );
  }

  const comparison = compare?.status === "loaded" ? compare.value : metrics.comparison;
  const request = metrics.comparisonRequest;
  return (
    <section aria-label="Render report" className="flex flex-col gap-4">
      <DetailSection title="Render">
        <DetailRow label="Result">{sentenceCase(metrics.status)}</DetailRow>
        {metrics.durationSeconds !== null && <DetailRow label="Duration">{formatTimelineSeconds(metrics.durationSeconds)}</DetailRow>}
        {metrics.quality && <DetailRow label="Quality">{metrics.quality}</DetailRow>}
        {metrics.resolution && <DetailRow label="Resolution">{metrics.resolution}</DetailRow>}
        {metrics.codecs && <DetailRow label="Codecs" mono>{metrics.codecs}</DetailRow>}
        {metrics.renderTimeMs !== null && <DetailRow label="Render time">{formatTimelineSeconds(metrics.renderTimeMs / 1000)}</DetailRow>}
        {metrics.streams && (
          <>
            <DetailRow label="Video stream">
              <StatusBadge ok={metrics.streams.video}>{metrics.streams.video ? "Present" : "Missing"}</StatusBadge>
            </DetailRow>
            <DetailRow label="Audio stream">
              <StatusBadge ok={metrics.streams.audio}>{metrics.streams.audio ? "Present" : "Missing"}</StatusBadge>
            </DetailRow>
          </>
        )}
        {metrics.outputPath && <DetailRow label="Output" mono>{metrics.outputPath}</DetailRow>}
        {metrics.logPath && <DetailRow label="Log" mono>{metrics.logPath}</DetailRow>}
        {metrics.artifactCount > 0 && <DetailRow label="Artifacts">{metrics.artifactCount === 1 ? "1 file" : `${metrics.artifactCount} files`}</DetailRow>}
      </DetailSection>

      {metrics.checks.length > 0 && (
        <DetailSection title="Checks">
          {metrics.checks.map(([check, status]) => (
            <DetailRow key={check} label={sentenceCase(renderCheckLabel(check))}>
              {status === "skipped" ? <span className="text-muted-foreground">Skipped</span> : <StatusBadge ok={status === "passed"}>{sentenceCase(status)}</StatusBadge>}
            </DetailRow>
          ))}
        </DetailSection>
      )}

      {metrics.errors.length > 0 && (
        <DetailSection title="Problems">
          {metrics.errors.map((error, index) => (
            <DetailRow key={`${error.code}-${error.path}-${index}`} label={`Problem ${index + 1}`}>
              <span className="block font-medium text-destructive">{error.message}</span>
              <span className="block text-muted-foreground">{error.fix}</span>
              <span className="block break-all font-mono text-[11px] text-dim">
                {error.code} · {error.path}
              </span>
            </DetailRow>
          ))}
        </DetailSection>
      )}

      {metrics.graphics.length > 0 && (
        <DetailSection title="Graphics">
          {metrics.graphics.map((graphic) => (
            <DetailRow key={graphic.layerId} label={graphic.layerId}>
              <span className="block">
                {graphic.renderer} · {graphic.visualQaStatus ?? "Not checked"}
              </span>
              {graphic.sampledFrames.length > 0 && (
                <span className="block text-xs text-muted-foreground">
                  {artifactBackedFrameCount(graphic.sampledFrames, [...metrics.artifacts])} of {graphic.sampledFrames.length} sampled frames saved
                </span>
              )}
              {qaMetricEntries(graphic.qaMetrics).map(([metric, value]) => (
                <span key={metric} className="block break-all font-mono text-[11px] text-muted-foreground">
                  {metric}={value}
                </span>
              ))}
            </DetailRow>
          ))}
        </DetailSection>
      )}

      <DetailSection
        title="Preview comparison"
        action={
          request && splitProject ? (
            <button
              type="button"
              disabled={compare?.status === "loading"}
              onClick={() => void runComparison(request)}
              className="h-7 rounded-control border border-line px-2.5 text-xs font-medium text-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
            >
              {compare?.status === "loading" ? "Comparing…" : "Compare preview and render"}
            </button>
          ) : undefined
        }
      >
        {compare?.status === "loaded" && (
          <DetailRow label="Latest run">
            <span role="status">{comparisonSummary(compare.value)}</span>
          </DetailRow>
        )}
        {compare?.status === "failed" && (
          <DetailRow label="Latest run">
            <span role="alert" className="text-destructive">
              {compare.message}
            </span>
          </DetailRow>
        )}
        {comparison && comparison.comparedFrames.length > 0 ? (
          <>
            <DetailRow label="Frames">
              {comparison.comparedFrames.filter((frame) => frame.passed).length} of {comparison.comparedFrames.length} frames match
            </DetailRow>
            {comparison.comparedFrames.map((frame) => (
              <DetailRow key={`${frame.timelineSeconds}-${frame.renderedFrame}`} label={`At ${formatTimelineSeconds(frame.timelineSeconds)}`}>
                <span className="inline-flex flex-wrap items-center gap-2">
                  <StatusBadge ok={frame.passed}>{frame.passed ? "Match" : "Differs"}</StatusBadge>
                  <span className="text-xs text-muted-foreground">{formatMismatchRatio(frame.mismatchRatio)}</span>
                </span>
              </DetailRow>
            ))}
          </>
        ) : (
          compare === null && (
            <DetailRow label="Frames">{request ? "Not compared yet." : "This render has no preview comparison to run."}</DetailRow>
          )
        )}
      </DetailSection>
    </section>
  );
}
