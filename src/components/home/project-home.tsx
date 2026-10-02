import {
  AlertTriangle,
  Film,
  FolderOpen,
  FolderPlus,
  Play,
  Settings,
} from "lucide-react";
import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { normalizeAbsoluteProjectPathForComparison } from "@/lib/app-settings";

export interface RecentProjectEntry {
  id: string;
  name: string;
  projectDir: string;
  updatedAtLabel: string;
  statusLabel?: string;
  warningLabel?: string;
}

interface ProjectHomeProps {
  recentProjects: readonly RecentProjectEntry[];
  suggestedProjectParent?: string;
  onOpenSample: () => void;
  onOpenProject: (entry: RecentProjectEntry) => void;
  onOpenProjectFolder: (projectDir: string) => void;
  onCreateProject: (projectDir: string) => void;
  onCreateRemoteProject?: (name: string) => void;
  remoteHostLabel?: string | undefined;
  remoteCreationUnconfirmed?: boolean;
  onRefreshRemoteProjects?: () => void;
  onRemoveProject?: (entry: RecentProjectEntry) => void;
  onOpenModelSettings?: () => void;
  openProjectError?: string | null;
  openProjectErrorTitle?: string;
  isOpeningProject?: boolean;
  isCreatingProject?: boolean;
}

type StartMode = "open" | "create" | null;

export function ProjectHome({
  recentProjects,
  suggestedProjectParent = "",
  onOpenSample,
  onOpenProject,
  onOpenProjectFolder,
  onCreateProject,
  onCreateRemoteProject,
  remoteHostLabel,
  remoteCreationUnconfirmed = false,
  onRefreshRemoteProjects,
  onRemoveProject,
  onOpenModelSettings,
  openProjectError = null,
  openProjectErrorTitle = "Project folder could not be opened",
  isOpeningProject = false,
  isCreatingProject = false,
}: ProjectHomeProps) {
  const isRemote = Boolean(remoteHostLabel);
  const [projectDir, setProjectDir] = useState(suggestedProjectParent);
  const [hasExplicitProjectDir, setHasExplicitProjectDir] = useState(
    suggestedProjectParent.trim().length === 0,
  );
  const [startMode, setStartMode] = useState<StartMode>(isRemote ? null : "open");
  const trimmedProjectDir = projectDir.trim();
  const normalizedSuggestedProjectParent =
    normalizeAbsoluteProjectPathForComparison(suggestedProjectParent);
  const matchesSuggestedProjectParent =
    normalizedSuggestedProjectParent !== null &&
    normalizeAbsoluteProjectPathForComparison(trimmedProjectDir) ===
      normalizedSuggestedProjectParent;
  const canUseProjectDir =
    Boolean(trimmedProjectDir) &&
    hasExplicitProjectDir &&
    !matchesSuggestedProjectParent;
  const bundledSample = recentProjects.find((entry) => entry.id === "sample-editor-project");
  const localProjects = recentProjects.filter((entry) => entry.id !== "sample-editor-project");

  function handleOpenProjectFolder(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isRemote) {
      if (startMode === "create" && trimmedProjectDir && !isCreatingProject && !remoteCreationUnconfirmed) {
        onCreateRemoteProject?.(trimmedProjectDir);
      }
      return;
    }
    if (!canUseProjectDir || isOpeningProject) return;
    if (startMode === "create") onCreateProject(trimmedProjectDir);
    else onOpenProjectFolder(trimmedProjectDir);
  }

  return (
    <main
      aria-label="Project home"
      className="min-h-screen bg-[#121314] text-foreground"
    >
      <div className="grid min-h-screen grid-cols-[220px_minmax(0,1fr)] max-md:grid-cols-1">
        <aside
          data-testid="project-home-sidebar"
          aria-label="Project home actions"
          className="flex min-h-screen w-[220px] flex-col border-r border-white/5 bg-[#1d2021] px-3 pb-4 pt-3 max-md:min-h-0 max-md:w-full max-md:border-b max-md:border-r-0"
        >
          <nav aria-label="Project actions" className="space-y-1">
            <Button
              type="button"
              aria-label="New project"
              variant="ghost"
              disabled={remoteCreationUnconfirmed}
              className="h-9 w-full justify-start gap-3 px-2 text-[13px] font-medium"
              onClick={() => {
                if (canUseProjectDir && !isCreatingProject) {
                  onCreateProject(trimmedProjectDir);
                } else {
                  setStartMode("create");
                }
              }}
            >
              <FolderPlus className="h-4 w-4" aria-hidden="true" />
              New Project
            </Button>
            <Button
              type="button"
              aria-label="Open Project"
              variant="ghost"
              className="h-9 w-full justify-start gap-3 px-2 text-[13px] font-medium"
              onClick={() => setStartMode(isRemote ? null : "open")}
            >
              <FolderOpen className="h-4 w-4" aria-hidden="true" />
              Open Project
            </Button>
            {!isRemote ? <Button
              type="button"
              aria-label="Open sample"
              variant="ghost"
              className="h-9 w-full justify-start gap-3 px-2 text-[13px] font-medium"
              onClick={onOpenSample}
            >
              <Play className="h-4 w-4" aria-hidden="true" />
              Open Sample
            </Button> : null}
          </nav>

          {startMode ? (
            <form className="mt-4 space-y-2 border-t border-white/10 px-1 pt-4" onSubmit={handleOpenProjectFolder}>
              <label htmlFor="project-folder-path" className="text-[11px] font-medium text-white/55">
                {isRemote ? "Project name" : "Project folder path"}
              </label>
              <input
                id="project-folder-path"
                className="h-8 w-full rounded-md border border-white/10 bg-black/20 px-2 font-mono text-[11px] text-foreground outline-none focus-visible:ring-1 focus-visible:ring-ring"
                placeholder={isRemote ? "Untitled project" : "/path/to/project.palmier"}
                value={projectDir}
                onChange={(event) => {
                  setProjectDir(event.target.value);
                  setHasExplicitProjectDir(true);
                }}
              />
              <Button
                type="submit"
                aria-label={startMode === "create" ? "Create project" : "Open project folder"}
                size="sm"
                className="h-8 w-full"
                disabled={
                  (!isRemote && !canUseProjectDir) ||
                  (isRemote && !trimmedProjectDir) ||
                  isOpeningProject ||
                  isCreatingProject ||
                  (startMode === "create" && remoteCreationUnconfirmed)
                }
              >
                {startMode === "create"
                  ? isCreatingProject ? "Creating project..." : "Create Project"
                  : isOpeningProject ? "Opening project..." : "Open Project Folder"}
              </Button>
              {openProjectError && !remoteCreationUnconfirmed ? (
                <div role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-[11px]">
                  <div className="font-medium">{openProjectErrorTitle}</div>
                  <div className="mt-1 text-muted-foreground">{openProjectError}</div>
                </div>
              ) : null}
            </form>
          ) : null}

          {remoteCreationUnconfirmed ? (
            <div role="alert" className="mt-4 rounded-md border border-destructive/30 bg-destructive/10 p-2 text-[11px]">
              <div className="font-medium">Project creation unconfirmed</div>
              <p className="mt-1 text-muted-foreground">The project may already exist. Refresh host projects and open it from My Projects if it appears. New project creation stays paused while this request is unconfirmed.</p>
              {onRefreshRemoteProjects ? <Button type="button" variant="outline" size="sm" className="mt-2 w-full" onClick={onRefreshRemoteProjects}>Refresh host projects</Button> : null}
            </div>
          ) : null}

          {onOpenModelSettings ? (
            <Button
              type="button"
              aria-label="Model settings"
              variant="ghost"
              className="mt-auto h-9 w-full justify-start gap-3 px-2 text-[13px] font-medium max-md:mt-4"
              onClick={onOpenModelSettings}
            >
              <Settings className="h-4 w-4" aria-hidden="true" />
              Settings
            </Button>
          ) : null}
        </aside>

        <section data-testid="project-home-main" className="min-w-0 overflow-y-auto">
          <div data-testid="project-home-content" className="mx-6 py-12">
            <h1 className="text-[32px] font-light tracking-[-0.04em]">Welcome to Video Creater</h1>
            {isRemote ? (
              <p className="mt-1 text-xs text-muted-foreground">On {remoteHostLabel}</p>
            ) : null}

            {!isRemote ? <section aria-label="Sample projects" className="mt-8">
              <h2 className="mb-3 text-sm font-semibold">Sample Project</h2>
              <button
                type="button"
                aria-label="Open sample project"
                data-testid="project-card"
                className="group relative h-[120px] w-[150px] overflow-hidden rounded-xl border border-white/10 bg-black text-left shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onClick={onOpenSample}
              >
                <video
                  aria-label="Bundled sample preview"
                  className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-[1.025] motion-reduce:transform-none"
                  src="/media/input.mp4"
                  muted
                  playsInline
                  preload="metadata"
                />
                <span className="pointer-events-none absolute inset-0 bg-gradient-to-t from-black/85 via-black/5 to-transparent" />
                <span className="pointer-events-none absolute inset-x-3 bottom-2 truncate text-[13px] font-semibold text-white">
                  {bundledSample?.name ?? "Video Creater Sample"}
                </span>
              </button>
            </section> : null}

            <section aria-label="Recent projects" className="mt-8">
              <span className="sr-only">Recent projects</span>
              <h2 className="mb-3 text-sm font-semibold">My Projects</h2>
              {localProjects.length === 0 ? (
                <button
                  type="button"
                  data-testid="project-card"
                  className="grid h-[120px] w-[150px] place-items-center rounded-xl border border-white/10 bg-gradient-to-b from-white/[0.04] to-black/60 text-white/40 hover:text-white/65 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  disabled={remoteCreationUnconfirmed}
                  onClick={() => setStartMode("create")}
                >
                  <FolderPlus className="h-7 w-7" aria-hidden="true" />
                  <span className="sr-only">Create first project</span>
                </button>
              ) : (
                <div className="flex flex-wrap gap-4">
                  {localProjects.map((entry) => (
                    <article key={entry.id} aria-label={`Recent project ${entry.name}`} className="w-[150px]">
                      <button
                        type="button"
                        data-testid="project-card"
                        aria-label="Open project"
                        className="group relative h-[120px] w-[150px] overflow-hidden rounded-xl border border-white/10 bg-gradient-to-br from-[#282a2b] to-black text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        disabled={Boolean(entry.warningLabel)}
                        onClick={() => onOpenProject(entry)}
                      >
                        <Film className="absolute left-1/2 top-1/2 h-8 w-8 -translate-x-1/2 -translate-y-1/2 text-white/20" aria-hidden="true" />
                        <span className="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-black to-transparent" />
                        <span className="absolute inset-x-3 bottom-2 truncate text-[13px] font-semibold text-white">{entry.name}</span>
                        {entry.warningLabel ? (
                          <span className="absolute inset-0 grid place-items-center bg-black/75 p-3 text-center text-[11px] text-amber-200">
                            <span><AlertTriangle className="mx-auto mb-1 h-5 w-5" aria-hidden="true" />{entry.warningLabel}</span>
                          </span>
                        ) : null}
                      </button>
                      <div className="mt-1 flex items-center justify-between gap-1 text-[10px] text-muted-foreground">
                        <span className="min-w-0 truncate">
                          <span className="block truncate">{entry.statusLabel ?? entry.updatedAtLabel}</span>
                          <span className="block truncate">
                            {isRemote ? `On ${remoteHostLabel}` : entry.projectDir}
                          </span>
                        </span>
                        {entry.warningLabel ? (
                          <button
                            type="button"
                            aria-label="Relink project"
                            className="shrink-0 underline"
                            onClick={() => {
                              setProjectDir(entry.projectDir);
                              setHasExplicitProjectDir(true);
                              setStartMode("open");
                            }}
                          >
                            Relink
                          </button>
                        ) : null}
                      </div>
                      {entry.warningLabel && onRemoveProject ? (
                        <button
                          type="button"
                          aria-label="Remove from recents"
                          className="mt-1 text-[10px] text-muted-foreground underline"
                          onClick={() => onRemoveProject(entry)}
                        >
                          Remove
                        </button>
                      ) : null}
                    </article>
                  ))}
                </div>
              )}
            </section>
          </div>
        </section>
      </div>
    </main>
  );
}
