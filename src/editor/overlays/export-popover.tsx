import { Upload } from "lucide-react";
import { useEffect, useId, useMemo, useState } from "react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  defaultExportChoices,
  estimatedExportSize,
  exportChoiceOptions,
  exportChoicesFromPreset,
  exportFrameRateOptions,
  exportPlan,
  type ExportChoices,
} from "@/lib/export/export-plan";
import { fallbackExportProfileAvailability } from "@/lib/export/profiles";
import type { ExportProfileAvailability } from "@/lib/project";
import { chooseExportDirectory } from "@/lib/runtime/adapters/tauri-dialog";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { cn } from "@/lib/utils";
import { loadExportProfiles, useExportService } from "../services/export-service";
import { useRuntimeMode } from "../services/use-runtime-mode";
import { useEditorStore } from "../store/editor-store-context";
import type { ExportPreset } from "../store/ui-slice";
import { ExportFooterLinks } from "./export-footer-links";
import { ExportOptions } from "./export-options";

const splitProjectReason = "Save as a schema-v2 split project to export.";
const folderNeedsDesktopReason = "Choosing a folder needs the desktop app.";

/** The Export button and its popover: the editor's single export entry point (button, ⌘E, native menu). */
export function ExportPopover({ compact = false }: { readonly compact?: boolean }) {
  const exportPopover = useEditorStore((state) => state.exportPopover);
  const openExportPopover = useEditorStore((state) => state.openExportPopover);
  const closeExportPopover = useEditorStore((state) => state.closeExportPopover);
  const preset = exportPopover?.preset ?? null;

  return (
    <Popover open={exportPopover !== null} onOpenChange={(open) => (open ? openExportPopover() : closeExportPopover())}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            "flex h-8 shrink-0 items-center gap-1.5 rounded-control bg-primary font-semibold text-primary-foreground hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
            compact ? "px-3" : "px-4",
          )}
        >
          <Upload className="h-4 w-4" aria-hidden />
          Export
        </button>
      </PopoverTrigger>
      <PopoverContent
        aria-label="Export"
        // Focus the popover itself, so the Name field doesn't open the on-screen keyboard on phones.
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          if (event.currentTarget instanceof HTMLElement) event.currentTarget.focus();
        }}
        className="max-h-[var(--radix-popover-content-available-height)] w-[340px] max-w-[calc(100vw-16px)] overflow-y-auto p-4"
      >
        {exportPopover !== null && <ExportPopoverBody key={preset?.jobId ?? "new"} preset={preset} onDone={closeExportPopover} />}
      </PopoverContent>
    </Popover>
  );
}

/** The capability report, loaded when the popover opens; profiles read as "being checked" until then. */
function useExportProfiles(): readonly ExportProfileAvailability[] {
  const [profiles, setProfiles] = useState<readonly ExportProfileAvailability[]>(fallbackExportProfileAvailability);
  useEffect(() => {
    let cancelled = false;
    void loadExportProfiles().then((loaded) => !cancelled && setProfiles(loaded));
    return () => {
      cancelled = true;
    };
  }, []);
  return profiles;
}

function ExportPopoverBody({ preset, onDone }: { readonly preset: ExportPreset | null; onDone(): void }) {
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const setLastError = useEditorStore((state) => state.setLastError);
  const runtimeMode = useRuntimeMode();
  const service = useExportService();
  const blockedId = useId();
  const profiles = useExportProfiles();
  // Until the user changes something, the choices follow the loaded report's defaults.
  const [edited, setEdited] = useState<ExportChoices | null>(null);
  const choices = useMemo(() => {
    if (edited) return edited;
    const defaults = defaultExportChoices(profiles, project.name);
    return preset ? exportChoicesFromPreset(defaults, preset) : defaults;
  }, [edited, preset, profiles, project.name]);
  const plan = useMemo(() => exportPlan({ choices, profiles, project, projectDir, jobId: "pending" }), [choices, profiles, project, projectDir]);
  const options = useMemo(() => exportChoiceOptions(profiles, choices), [choices, profiles]);
  const splitProject = project.schemaVersion >= 2 && projectDir.trim().length > 0;
  // Both backends save into the project's exports folder unless another folder is chosen.
  const projectExportsFolder = runtimeMode === "browser"
    ? "On host · Project exports"
    : splitProject ? `${projectDir.replace(/[\\/]+$/, "")}/exports` : "Project folder";
  const folderChooser = {
    disabledReason: runtimeMode === "browser" ? folderNeedsDesktopReason : null,
    choose() {
      chooseExportDirectory(choices.directory ?? (splitProject ? projectExportsFolder : undefined))
        .then((directory) => directory && setEdited({ ...choices, directory }))
        .catch((error: unknown) => setLastError(isBackendUnavailableError(error) ? folderNeedsDesktopReason : `The folder chooser couldn't open: ${String(error)}`));
    },
  };

  function exportVideo() {
    if (plan.blockedReason) return;
    void service.exportVideo(plan);
    onDone();
  }

  return (
    <div className="flex flex-col gap-4">
      <ExportOptions
        choices={choices}
        options={options}
        plan={plan}
        sizeBytes={estimatedExportSize(plan, project.timeline.durationSeconds)}
        projectExportsFolder={projectExportsFolder}
        folderChooser={folderChooser}
        frameRateOptions={exportFrameRateOptions(project.renderSettings.fps)}
        onChange={(patch) => setEdited({ ...choices, ...patch })}
      />
      <div className="flex flex-col gap-1.5">
        <button
          type="button"
          aria-disabled={plan.blockedReason !== null || undefined}
          aria-describedby={plan.blockedReason ? blockedId : undefined}
          onClick={exportVideo}
          className="h-9 rounded-control bg-primary font-semibold text-primary-foreground hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:cursor-not-allowed aria-disabled:opacity-50 aria-disabled:hover:bg-primary"
        >
          Export video
        </button>
        {plan.blockedReason && (
          <p id={blockedId} className="text-[11px] text-warning">
            {plan.blockedReason}
          </p>
        )}
      </div>
      <div className="border-t border-line pt-3">
        <ExportFooterLinks
          disabledReason={splitProject ? null : splitProjectReason}
          onExport={(target) => {
            void (target === "projectPackage" ? service.exportProjectPackage() : service.exportNleXml(target));
            onDone();
          }}
        />
      </div>
    </div>
  );
}
