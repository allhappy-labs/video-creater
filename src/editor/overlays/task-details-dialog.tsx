import { useEffect, useState, type RefObject } from "react";
import type { TaskRecord } from "@/lib/jobs/task-records";
import { getTemporalWorkerEnvironmentReport, type ProjectJobSummary, type TemporalWorkerEnvironmentReport } from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { useEditorStore } from "../store/editor-store-context";
import { OverlayDialog } from "./overlay-dialog";
import { DetailRow, DetailSection, errorMessage, StatusBadge, type LoadState } from "./task-detail-section";
import { TaskRenderReport } from "./task-render-report";

const statusLabel: Record<TaskRecord["status"], string> = {
  queued: "Queued",
  running: "Running",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  blocked: "Waiting to start",
};

/** The task whose details are open (`ui.taskDetailsId`): workflow internals, worker preflight, and render review. */
export function TaskDetailsDialog({ returnFocusRef }: { readonly returnFocusRef?: RefObject<HTMLElement | null> }) {
  const taskId = useEditorStore((state) => state.taskDetailsId);
  const record = useEditorStore((state) => (taskId === null ? null : (state.tasks.find((task) => task.id === taskId) ?? null)));
  const closeTaskDetails = useEditorStore((state) => state.closeTaskDetails);

  return (
    <OverlayDialog
      open={taskId !== null}
      onOpenChange={(open) => !open && closeTaskDetails()}
      title="Task details"
      {...(record ? { description: record.label } : {})}
      {...(returnFocusRef ? { returnFocusRef } : {})}
      size="lg"
    >
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 pb-4 pt-3">
        {record ? <TaskDetailsBody record={record} /> : <p className="text-[13px] text-muted-foreground">This task is no longer in the project.</p>}
      </div>
    </OverlayDialog>
  );
}

function TaskDetailsBody({ record }: { readonly record: TaskRecord }) {
  const job = useEditorStore((state) => state.project.jobs.find((candidate) => candidate.id === record.id) ?? null);
  const asset = useEditorStore((state) => state.project.generatedAssets.find((candidate) => candidate.id === record.id) ?? null);
  const temporal = record.workflow?.backend === "temporal";
  const serviceIssue = useEditorStore((state) => (temporal && (record.status === "queued" || record.status === "running") ? state.workflowServiceIssue : null));
  const position = job ? queuePosition(job) : null;
  const workflowId = record.workflow?.workflowId;
  const runId = record.workflow?.runId;

  return (
    <>
      <DetailSection title="Status">
        <DetailRow label="Status">{statusLabel[record.status]}</DetailRow>
        {record.failureReason && <DetailRow label="What failed">{record.failureReason}</DetailRow>}
        {serviceIssue && <DetailRow label="Workflow service">{serviceIssue}</DetailRow>}
      </DetailSection>
      <DetailSection title="Workflow">
        <DetailRow label="Runs in">{record.workflow === null ? "Not recorded" : temporal ? "Temporal workflow worker" : "Editor process"}</DetailRow>
        {workflowId && <DetailRow label="Workflow ID" mono>{workflowId}</DetailRow>}
        {runId && <DetailRow label="Run ID" mono>{runId}</DetailRow>}
        {job?.workflow?.workflowType && <DetailRow label="Workflow type" mono>{job.workflow.workflowType}</DetailRow>}
        {(job?.workflow?.taskQueue ?? job?.startRequest?.taskQueue) && (
          <DetailRow label="Task queue" mono>
            {job?.workflow?.taskQueue ?? job?.startRequest?.taskQueue}
          </DetailRow>
        )}
        {position !== null && <DetailRow label="Queue position">{position}</DetailRow>}
        {job?.providerRequest && <DetailRow label="Provider">{job.providerRequest.provider}</DetailRow>}
      </DetailSection>
      <DetailSection title="Timestamps">
        {asset && <DetailRow label="Created"><Timestamp value={asset.createdAt} /></DetailRow>}
        {job?.providerRequest && <DetailRow label="Submitted"><Timestamp value={job.providerRequest.submittedAt} /></DetailRow>}
        <DetailRow label="Last update"><Timestamp value={record.updatedAt} /></DetailRow>
      </DetailSection>
      {temporal && <WorkerPreflight />}
      {(record.kind === "render" || record.kind === "export") && <TaskRenderReport record={record} />}
    </>
  );
}

/** A queue position when the workflow metadata or its start request records one; none do yet. */
function queuePosition(job: ProjectJobSummary): number | null {
  const sources: unknown[] = [job.workflow, job.startRequest?.searchAttributes];
  for (const source of sources) {
    if (typeof source !== "object" || source === null) continue;
    const value = (source as Record<string, unknown>).queuePosition;
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return null;
}

function Timestamp({ value }: { readonly value: string }) {
  const time = Date.parse(value);
  return <time dateTime={value}>{Number.isFinite(time) ? new Date(time).toLocaleString() : value}</time>;
}

/** Temporal worker preflight, loaded each time details open for a Temporal task. */
function WorkerPreflight() {
  const [report, setReport] = useState<LoadState<TemporalWorkerEnvironmentReport>>({ status: "loading" });
  useEffect(() => {
    let cancelled = false;
    getTemporalWorkerEnvironmentReport()
      .then((value) => !cancelled && setReport({ status: "loaded", value }))
      .catch((error: unknown) => {
        if (cancelled) return;
        setReport({ status: "failed", message: isBackendUnavailableError(error) ? "Worker checks need the desktop app." : `Couldn't check the workflow worker. ${errorMessage(error)}` });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (report.status !== "loaded") {
    return (
      <DetailSection title="Worker preflight">
        <DetailRow label="Worker">{report.status === "loading" ? "Checking…" : report.message}</DetailRow>
      </DetailSection>
    );
  }
  const { value } = report;
  const ready = value.ready && value.featureEnabled;
  return (
    <DetailSection title="Worker preflight">
      <DetailRow label="Worker">
        <StatusBadge ok={ready}>{ready ? "Ready" : "Setup needed"}</StatusBadge>
      </DetailRow>
      <DetailRow label="Workflows">{value.featureEnabled ? "Turned on" : `Turned off (${value.featureName})`}</DetailRow>
      <DetailRow label="Task queue" mono>{value.taskQueue}</DetailRow>
      <DetailRow label="Local service" mono>{value.localServiceTarget}</DetailRow>
      {!ready && <DetailRow label="Start service" mono>{value.localDevCommand}</DetailRow>}
      {!ready && <DetailRow label="Start worker" mono>{value.workerRunCommand}</DetailRow>}
      {value.tools.map((tool) => (
        <DetailRow key={tool.name} label={tool.name}>
          <StatusBadge ok={tool.available}>{tool.available ? "Available" : "Missing"}</StatusBadge>
          {!tool.available && tool.installHint && <span className="mt-0.5 block text-xs text-muted-foreground">{tool.installHint}</span>}
        </DetailRow>
      ))}
    </DetailSection>
  );
}
