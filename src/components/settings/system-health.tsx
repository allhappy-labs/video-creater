import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  CheckCircle2,
  CircleDashed,
  Loader2,
  RefreshCw,
  TriangleAlert,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  getSystemHealthSnapshot,
  mergeSystemHealthSection,
  refreshSystemHealthSection,
  type SystemHealthSection,
  type SystemHealthSnapshot,
  type SystemHealthState,
} from "@/lib/settings/health";
import { useSettingsOperations } from "@/lib/settings/use-settings-operations";
import {
  operationForKind,
  SettingsOperationStatus,
} from "./settings-operation-status";

export interface SystemHealthProps {
  projectRoot: string | null;
  onBack: () => void;
}

const sectionOrder = [
  "rendering",
  "localAi",
  "agent",
  "project",
  "environment",
] as const;

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error
  ) {
    return String(error.message);
  }
  return typeof error === "string" ? error : String(error);
}

function stateLabel(state: SystemHealthState) {
  switch (state) {
    case "ready":
      return "Ready";
    case "needsAction":
      return "Needs action";
    case "failed":
      return "Failed";
    case "checking":
      return "Checking";
    case "notConfigured":
      return "Not configured";
    case "notEnabled":
      return "Not enabled";
    case "notApplicable":
      return "Not applicable";
    case "unavailable":
      return "Unavailable";
  }
}

function stateTone(state: SystemHealthState) {
  if (state === "ready") return "text-emerald-700";
  if (state === "failed") return "text-red-700";
  if (state === "needsAction") return "text-amber-800";
  return "text-muted-foreground";
}

function StateIcon({ state }: { state: SystemHealthState }) {
  if (state === "checking") {
    return (
      <Loader2
        className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none"
        aria-hidden="true"
      />
    );
  }
  if (state === "ready") {
    return <CheckCircle2 className="h-3.5 w-3.5" aria-hidden="true" />;
  }
  if (state === "failed" || state === "needsAction") {
    return <TriangleAlert className="h-3.5 w-3.5" aria-hidden="true" />;
  }
  return <CircleDashed className="h-3.5 w-3.5" aria-hidden="true" />;
}

function orderedSections(snapshot: SystemHealthSnapshot) {
  const known = sectionOrder.flatMap((id) =>
    snapshot.sections[id] ? [snapshot.sections[id]] : [],
  );
  const knownIds = new Set(sectionOrder);
  const additional = Object.values(snapshot.sections).filter(
    ({ id }) => !knownIds.has(id as (typeof sectionOrder)[number]),
  );
  return [...known, ...additional];
}

export function SystemHealth({ projectRoot, onBack }: SystemHealthProps) {
  const [snapshot, setSnapshot] = useState<SystemHealthSnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [sectionErrors, setSectionErrors] = useState<Record<string, string>>({});
  const [refreshingSectionIds, setRefreshingSectionIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const projectRootRef = useRef(projectRoot);
  const projectGenerationRef = useRef(0);
  const sectionRequestIdsRef = useRef(new Map<string, number>());
  const loadRequestIdRef = useRef(0);
  const operationSync = useSettingsOperations();

  if (projectRootRef.current !== projectRoot) {
    projectRootRef.current = projectRoot;
    projectGenerationRef.current += 1;
    sectionRequestIdsRef.current.clear();
  }

  useEffect(() => {
    const requestRoot = projectRoot;
    const requestGeneration = projectGenerationRef.current;
    const requestId = ++loadRequestIdRef.current;
    let cancelled = false;
    setSnapshot(null);
    setLoadError(null);
    setSectionErrors({});
    setRefreshingSectionIds(new Set());

    void getSystemHealthSnapshot(requestRoot)
      .then((nextSnapshot) => {
        if (
          !cancelled &&
          projectRootRef.current === requestRoot &&
          projectGenerationRef.current === requestGeneration &&
          loadRequestIdRef.current === requestId
        ) {
          setSnapshot(nextSnapshot);
        }
      })
      .catch((error) => {
        if (
          !cancelled &&
          projectRootRef.current === requestRoot &&
          projectGenerationRef.current === requestGeneration &&
          loadRequestIdRef.current === requestId
        ) {
          setLoadError(errorMessage(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [projectRoot]);

  async function refreshSection(section: SystemHealthSection) {
    const requestRoot = projectRoot;
    const requestGeneration = projectGenerationRef.current;
    const requestId = (sectionRequestIdsRef.current.get(section.id) ?? 0) + 1;
    sectionRequestIdsRef.current.set(section.id, requestId);
    const isCurrent = () =>
      projectRootRef.current === requestRoot &&
      projectGenerationRef.current === requestGeneration &&
      sectionRequestIdsRef.current.get(section.id) === requestId;

    setSectionErrors((current) => {
      const next = { ...current };
      delete next[section.id];
      return next;
    });
    setRefreshingSectionIds((current) => new Set([...current, section.id]));
    try {
      const refreshed = await refreshSystemHealthSection(section.id, requestRoot);
      if (!isCurrent()) return;
      setSnapshot((current) =>
        current ? mergeSystemHealthSection(current, refreshed) : current,
      );
    } catch (error) {
      if (!isCurrent()) return;
      setSectionErrors((current) => ({
        ...current,
        [section.id]: errorMessage(error),
      }));
    } finally {
      if (isCurrent()) {
        setRefreshingSectionIds((current) => {
          const next = new Set(current);
          next.delete(section.id);
          return next;
        });
      }
    }
  }

  const latestHealthOperation = operationForKind(
    operationSync.operations,
    "healthCheck",
  );

  return (
    <main className="flex h-full min-h-0 flex-col bg-background" aria-label="System Health">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b px-3">
        <Button type="button" variant="ghost" size="icon" onClick={onBack} aria-label="Back to Settings">
          <ArrowLeft className="h-4 w-4" aria-hidden="true" />
        </Button>
        <div className="min-w-0">
          <h1 className="text-sm font-semibold">System Health</h1>
          <p className="text-[11px] text-muted-foreground">
            Runtime and project diagnostics. Optional tools do not lower overall readiness.
          </p>
        </div>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4 md:px-6">
        <div className="mx-auto grid max-w-4xl gap-4">
          {loadError ? (
            <section role="alert" className="border-l-2 border-red-500 pl-3 text-xs text-red-700">
              <p className="font-medium">System Health could not be loaded.</p>
              <p>{loadError}</p>
            </section>
          ) : !snapshot ? (
            <p role="status" className="flex items-center gap-2 text-xs text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin motion-reduce:animate-none" aria-hidden="true" />
              Checking system health
            </p>
          ) : (
            <>
              <section aria-label="Overall readiness" className="flex items-center justify-between border-b pb-3">
                <div>
                  <h2 className="text-xs font-semibold">Overall readiness</h2>
                  <p className="text-[11px] text-muted-foreground">Based only on required sections.</p>
                </div>
                <span className={`flex items-center gap-1.5 text-xs font-semibold ${stateTone(snapshot.overall)}`}>
                  <StateIcon state={snapshot.overall} />
                  {stateLabel(snapshot.overall)}
                </span>
              </section>

              <div className="grid gap-4">
                {orderedSections(snapshot).map((section) => {
                  const refreshing = refreshingSectionIds.has(section.id);
                  return (
                    <section key={section.id} aria-labelledby={`system-health-${section.id}`} className="grid gap-3 border-b pb-4">
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div>
                          <h2 id={`system-health-${section.id}`} className="text-xs font-semibold">{section.label}</h2>
                          <p className="text-[11px] text-muted-foreground">{section.required ? "Required for overall readiness" : "Optional or context-specific"}</p>
                        </div>
                        <div className="flex items-center gap-2">
                          <span className={`flex items-center gap-1 text-[11px] font-medium ${stateTone(refreshing ? "checking" : section.state)}`}>
                            <StateIcon state={refreshing ? "checking" : section.state} />
                            {stateLabel(refreshing ? "checking" : section.state)}
                          </span>
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            className="h-7 px-2 text-[11px]"
                            disabled={refreshing}
                            onClick={() => void refreshSection(section)}
                            aria-label={`Retry ${section.label}`}
                          >
                            <RefreshCw className={`h-3.5 w-3.5 ${refreshing ? "animate-spin motion-reduce:animate-none" : ""}`} aria-hidden="true" />
                            Retry
                          </Button>
                        </div>
                      </div>

                      {sectionErrors[section.id] ? (
                        <p role="alert" className="border-l-2 border-red-500 pl-2 text-[11px] text-red-700">
                          {sectionErrors[section.id]}
                        </p>
                      ) : null}

                      {section.items.length > 0 ? (
                        <ul className="grid gap-2">
                          {section.items.map((item) => (
                            <li key={item.id} className="grid gap-1 border-t pt-2 text-[11px] md:grid-cols-[minmax(11rem,0.8fr)_minmax(16rem,1.2fr)]">
                              <div className="flex min-w-0 items-center gap-2 font-medium text-foreground">
                                <StateIcon state={item.state} />
                                <span>{item.label}</span>
                              </div>
                              <div className="min-w-0 text-muted-foreground md:text-right">
                                <p>{item.summary}</p>
                                {item.diagnosticCode ? <code className="font-mono text-[10px]">{item.diagnosticCode}</code> : null}
                              </div>
                            </li>
                          ))}
                        </ul>
                      ) : (
                        <p className="text-[11px] text-muted-foreground">No diagnostics are available for this context.</p>
                      )}
                    </section>
                  );
                })}
              </div>
            </>
          )}

          {operationSync.error ? (
            <p role="alert" className="border-l-2 border-amber-500 pl-2 text-[11px] text-amber-800">
              {operationSync.error.message} {operationSync.error.detail}
            </p>
          ) : null}
          <SettingsOperationStatus operation={latestHealthOperation} ariaLabel="Latest diagnostic operation" />
        </div>
      </div>
    </main>
  );
}
