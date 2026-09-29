import { CheckCircle2, Loader2, OctagonAlert, PauseCircle } from "lucide-react";

import type {
  SettingsOperation,
  SettingsOperationKind,
} from "@/lib/settings/operations";

const activeStates = new Set(["queued", "running", "cancelling"]);

export function SettingsOperationProgress({
  operation,
  ariaLabel,
  formatValue = String,
  unitLabel = operation.unit ?? "units",
}: {
  operation: SettingsOperation;
  ariaLabel: string;
  formatValue?: (value: number) => string;
  unitLabel?: string;
}) {
  const total = operation.totalUnits;
  const determinate = total !== null && Number.isFinite(total) && total > 0;
  const completed = Number.isFinite(operation.completedUnits)
    ? Math.max(0, operation.completedUnits)
    : 0;
  const current = determinate ? Math.min(completed, total) : completed;
  const suffix = unitLabel ? ` ${unitLabel}` : "";
  const valueText = determinate
    ? `${formatValue(current)} / ${formatValue(total)}${suffix}`
    : `${formatValue(current)}${suffix} completed`;

  return (
    <span
      role="progressbar"
      aria-label={ariaLabel}
      aria-valuemin={determinate ? 0 : undefined}
      aria-valuenow={determinate ? current : undefined}
      aria-valuemax={determinate ? total : undefined}
      aria-valuetext={valueText}
      className="text-muted-foreground"
    >
      {valueText}
    </span>
  );
}

function operationVersion(operation: SettingsOperation) {
  const parsed = Date.parse(operation.updatedAt);
  return Number.isNaN(parsed) ? Number.NEGATIVE_INFINITY : parsed;
}

export function operationForTarget(
  operations: SettingsOperation[],
  targetId: string,
  localOperation: SettingsOperation | null = null,
) {
  const candidates = [
    ...operations.filter((operation) => operation.targetId === targetId),
    ...(localOperation?.targetId === targetId ? [localOperation] : []),
  ];
  const active = candidates.filter((operation) => activeStates.has(operation.state));
  return (active.length > 0 ? active : candidates).reduce<SettingsOperation | null>(
    (latest, operation) =>
      !latest || operationVersion(operation) >= operationVersion(latest)
        ? operation
        : latest,
    null,
  );
}

export function operationForKind(
  operations: SettingsOperation[],
  kind: SettingsOperationKind,
  localOperation: SettingsOperation | null = null,
) {
  const candidates = [
    ...operations.filter((operation) => operation.kind === kind),
    ...(localOperation ? [localOperation] : []),
  ];
  const active = candidates.filter((operation) => activeStates.has(operation.state));
  return (active.length > 0 ? active : candidates).reduce<SettingsOperation | null>(
    (latest, operation) =>
      !latest || operationVersion(operation) >= operationVersion(latest)
        ? operation
        : latest,
    null,
  );
}

function operationLabel(operation: SettingsOperation) {
  if (operation.state === "cancelled" || operation.phase === "interrupted") {
    return "Interrupted";
  }
  switch (operation.state) {
    case "queued":
      return "Queued";
    case "running":
    case "cancelling":
      return "Running";
    case "succeeded":
      return "Succeeded";
    case "failed":
      return "Failed";
  }
}

export function SettingsOperationStatus({
  operation,
  ariaLabel,
}: {
  operation: SettingsOperation | null;
  ariaLabel: string;
}) {
  if (!operation) {
    return null;
  }
  const label = operationLabel(operation);
  const active = activeStates.has(operation.state);
  const Icon = active
    ? Loader2
    : label === "Succeeded"
      ? CheckCircle2
      : label === "Interrupted"
        ? PauseCircle
        : OctagonAlert;
  const tone =
    label === "Succeeded"
      ? "text-emerald-700"
      : label === "Failed"
        ? "text-red-700"
        : label === "Interrupted"
          ? "text-amber-700"
          : "text-muted-foreground";

  return (
    <div className="grid gap-1 border-t pt-2 text-[11px]">
      <div
        role="status"
        aria-label={ariaLabel}
        aria-live="polite"
        aria-atomic="true"
        className={`flex min-w-0 items-start justify-between gap-3 ${tone}`}
      >
        <span className="min-w-0 truncate text-muted-foreground" title={operation.message}>
          {operation.message}
        </span>
        <span className="flex shrink-0 items-center gap-1 font-medium">
          <Icon
            className={`h-3.5 w-3.5 ${active ? "animate-spin motion-reduce:animate-none" : ""}`}
            aria-hidden="true"
          />
          {label}
        </span>
      </div>
      {active ? (
        <SettingsOperationProgress
          operation={operation}
          ariaLabel={`${ariaLabel} progress`}
        />
      ) : null}
      {operation.error ? (
        <div role="alert" className="grid gap-0.5 text-red-700">
          <span>
            {operation.error.code}: {operation.error.message}
          </span>
          {operation.error.detail ? <span>{operation.error.detail}</span> : null}
          {operation.error.recoveryAction ? (
            <span>{operation.error.recoveryAction}</span>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
