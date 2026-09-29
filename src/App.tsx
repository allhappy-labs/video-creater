import { lazy, Suspense, useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { DesktopHostConnection } from "@/components/runtime/desktop-host-connection";
import { ProjectHome, type RecentProjectEntry } from "@/components/home/project-home";
import {
  loadAppPreferences,
  type AppPreferences,
  type NewProjectDefaults,
} from "@/lib/app-settings";
import {
  loadSplitProjectFromFolder,
  materializeSampleProjectMedia,
  saveSplitProjectToFolder,
  updateProjectSettingsInSplitProjectFolder,
  type UpdateProjectSettingsAction,
  type VideoProject,
} from "@/lib/project";
import { projectFolderRecoveryCopy } from "@/lib/recovery-copy";
import {
  inactiveNativeMenuState,
  listenForNativeMenuCommands,
  syncNativeMenuState,
  type NativeMenuRequest,
  type NativeMenuState,
} from "@/lib/native-menu";
import { projectSessionKey } from "@/lib/project-session";
import {
  createSampleProject,
  sampleProjectBrowserDir,
  sampleProjectDir,
} from "@/lib/sample-project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { backendRequest } from "@/lib/runtime/backend-client";
import {
  defaultSettingsReadiness,
  loadSettingsReadiness,
  sameSettingsReadiness,
  type SettingsReadiness,
} from "@/lib/settings/readiness";
import {
  type AppSettingsTarget,
  type SettingsOrigin,
} from "@/lib/settings/target";
import type { RuntimeDescriptor } from "@/lib/runtime/runtime-descriptor";

const Settings = lazy(() => import("@/components/settings/settings").then((module) => ({
  default: module.Settings,
})));
const ProjectSettings = lazy(() => import("@/components/settings/project-settings").then((module) => ({
  default: module.ProjectSettings,
})));
const SystemHealth = lazy(() => import("@/components/settings/system-health").then((module) => ({
  default: module.SystemHealth,
})));
const EditorRoot = lazy(() => import("@/editor/editor-root").then((module) => ({ default: module.EditorRoot })));

type AppView = "home" | "editor" | "settings" | "projectSettings" | "systemHealth";

interface SystemHealthOrigin {
  element: HTMLElement | null;
  returnView: "home" | "editor" | "settings" | "projectSettings";
}

interface ProjectSettingsOrigin {
  element: HTMLElement | null;
  returnView: "editor";
}

interface RemoteProjectSummary {
  readonly projectId: string;
  readonly name: string;
  readonly updatedAtMs: number;
}
/** Help › Send Feedback works in every view, so App handles it rather than the editor. */
const feedbackMailto = "mailto:feedback@video-creater.local";
const recentProjectsStorageKey = "video-creater.recentProjects";
const maxRecentProjects = 8;

function suggestedProjectParent(preferences: AppPreferences) {
  const projectLocation = preferences.projectLocation;
  return projectLocation.mode === "suggestedParent"
    ? projectLocation.parentPath
    : "";
}

const sampleProjectEntry: RecentProjectEntry = {
  id: "sample-editor-project",
  name: "Sample editor project",
  projectDir: sampleProjectDir,
  updatedAtLabel: "Sample project",
  statusLabel: "Bundled demo",
};

export interface AppProps {
  runtime: RuntimeDescriptor;
}

export default function App({ runtime }: AppProps) {
  return runtime.connection.status === "disconnected" ? (
    <DesktopHostConnection initialState={runtime.remoteSession} />
  ) : (
    <ConnectedApp
      nativeMenuEnabled={runtime.mode !== "browser"}
      remoteHostLabel={runtime.remoteSession?.kind === "connected"
        ? runtime.remoteSession.hostLabel
        : undefined}
    />
  );
}

function ConnectedApp({
  nativeMenuEnabled,
  remoteHostLabel,
}: {
  readonly nativeMenuEnabled: boolean;
  readonly remoteHostLabel?: string | undefined;
}) {
  const [appPreferences, setAppPreferences] = useState<AppPreferences | null>(null);
  const [appPreferencesError, setAppPreferencesError] = useState<string | null>(null);
  const appPreferencesRequestIdRef = useRef(0);
  const [view, setView] = useState<AppView>("home");
  const viewRef = useRef<AppView>(view);
  const [settingsTarget, setSettingsTarget] = useState<AppSettingsTarget>({
    category: "aiModels",
  });
  const [settingsOrigin, setSettingsOrigin] = useState<SettingsOrigin>({
    element: null,
    returnView: "home",
  });
  const [settingsNavigationRequestId, setSettingsNavigationRequestId] = useState(0);
  const [configurationRefreshId, setConfigurationRefreshId] = useState(0);
  const [systemHealthOrigin, setSystemHealthOrigin] = useState<SystemHealthOrigin>({
    element: null,
    returnView: "settings",
  });
  const [projectSettingsOrigin, setProjectSettingsOrigin] =
    useState<ProjectSettingsOrigin>({
      element: null,
      returnView: "editor",
    });
  const focusRestorationFrameRef = useRef<number | null>(null);
  const settingsPreferencesAcceptedRef = useRef(false);
  const [activeProjectDir, setActiveProjectDir] = useState(sampleProjectEntry.projectDir);
  const [activeProject, setActiveProject] = useState<VideoProject | null>(null);
  const [recentProjectEntries, setRecentProjectEntries] = useState<RecentProjectEntry[]>(
    () => remoteHostLabel ? [] : loadRecentProjectEntries(),
  );
  const [remoteProjectEntries, setRemoteProjectEntries] = useState<RecentProjectEntry[]>([]);
  const [loadedRemoteCatalogHost, setLoadedRemoteCatalogHost] = useState<string | null>(null);
  const remoteCatalogRequestIdRef = useRef(0);
  const [projectNavigationKind, setProjectNavigationKind] = useState<
    "open" | "create" | null
  >(null);
  const projectNavigationRequestIdRef = useRef(0);
  const [openProjectError, setOpenProjectError] = useState<string | null>(null);
  const [openProjectErrorTitle, setOpenProjectErrorTitle] = useState(
    "Project folder could not be opened",
  );
  const [settingsReadiness, setSettingsReadiness] =
    useState<SettingsReadiness>(defaultSettingsReadiness);
  // Bumped by every readiness writer so an older startup load never overwrites a newer value.
  const settingsReadinessRevisionRef = useRef(0);
  const [nativeEditorMenuRequest, setNativeEditorMenuRequest] =
    useState<NativeMenuRequest | null>(null);
  const [editorNativeMenuState, setEditorNativeMenuState] =
    useState<NativeMenuState | null>(null);
  const activeProjectRef = useRef<VideoProject | null>(activeProject);
  const settingsOriginRef = useRef<SettingsOrigin>(settingsOrigin);
  const systemHealthOriginRef = useRef<SystemHealthOrigin>(systemHealthOrigin);

  viewRef.current = view;
  activeProjectRef.current = activeProject;
  settingsOriginRef.current = settingsOrigin;
  systemHealthOriginRef.current = systemHealthOrigin;

  const loadPreferences = useCallback(() => {
    const requestId = appPreferencesRequestIdRef.current + 1;
    appPreferencesRequestIdRef.current = requestId;
    setAppPreferencesError(null);
    void loadAppPreferences()
      .then((preferences) => {
        if (appPreferencesRequestIdRef.current === requestId) {
          setAppPreferences(preferences);
        }
      })
      .catch((error: unknown) => {
        if (appPreferencesRequestIdRef.current === requestId) {
          setAppPreferencesError(
            error instanceof Error
              ? error.message
              : "Native app preferences could not be loaded.",
          );
        }
      });
  }, []);

  useEffect(() => {
    loadPreferences();
    return () => {
      appPreferencesRequestIdRef.current += 1;
    };
  }, [loadPreferences]);

  useEffect(() => {
    if (!remoteHostLabel) return;
    const requestId = remoteCatalogRequestIdRef.current + 1;
    remoteCatalogRequestIdRef.current = requestId;
    let disposed = false;
    // Remote project IDs belong to the current host catalog. Never expose a
    // desktop/local recent entry while that catalog is being refreshed.
    setRemoteProjectEntries([]);
    setLoadedRemoteCatalogHost(null);
    void backendRequest<RemoteProjectSummary[]>("remote_list_projects")
      .then((projects) => {
        if (disposed || remoteCatalogRequestIdRef.current !== requestId) return;
        setRemoteProjectEntries(dedupeRemoteProjectEntries(projects.map(remoteProjectEntry)));
        setLoadedRemoteCatalogHost(remoteHostLabel);
      })
      .catch((error: unknown) => {
        if (!disposed && remoteCatalogRequestIdRef.current === requestId) {
          setLoadedRemoteCatalogHost(remoteHostLabel);
          setOpenProjectErrorTitle("Projects could not be loaded");
          setOpenProjectError(error instanceof Error ? error.message : "The host project catalog is unavailable.");
        }
      });
    return () => {
      disposed = true;
      if (remoteCatalogRequestIdRef.current === requestId) {
        remoteCatalogRequestIdRef.current += 1;
      }
    };
  }, [remoteHostLabel]);

  function beginProjectNavigation(
    kind: "open" | "create",
    errorTitle: string,
  ) {
    const requestId = projectNavigationRequestIdRef.current + 1;
    projectNavigationRequestIdRef.current = requestId;
    setProjectNavigationKind(kind);
    setOpenProjectError(null);
    setOpenProjectErrorTitle(errorTitle);
    return requestId;
  }

  function isCurrentProjectNavigation(requestId: number) {
    return projectNavigationRequestIdRef.current === requestId;
  }

  function finishProjectNavigation(requestId: number) {
    if (isCurrentProjectNavigation(requestId)) {
      setProjectNavigationKind(null);
    }
  }

  function cancelProjectNavigation() {
    projectNavigationRequestIdRef.current += 1;
    setProjectNavigationKind(null);
  }

  useEffect(() => () => {
    if (focusRestorationFrameRef.current !== null) {
      window.cancelAnimationFrame(focusRestorationFrameRef.current);
      focusRestorationFrameRef.current = null;
    }
  }, []);

  const handleSettingsReadinessChange = useCallback(
    (next: SettingsReadiness) => {
      settingsReadinessRevisionRef.current += 1;
      setSettingsReadiness((current) =>
        sameSettingsReadiness(current, next) ? current : next,
      );
    },
    [],
  );

  const appPreferencesLoaded = appPreferences !== null;
  const activeProjectSessionKey = activeProject
    ? projectSessionKey(activeProjectDir, activeProject.id)
    : null;
  useEffect(() => {
    if (!appPreferencesLoaded) return;
    const revision = settingsReadinessRevisionRef.current + 1;
    settingsReadinessRevisionRef.current = revision;
    void loadSettingsReadiness().then((next) => {
      if (settingsReadinessRevisionRef.current !== revision) return;
      setSettingsReadiness((current) =>
        sameSettingsReadiness(current, next) ? current : next,
      );
    });
  }, [activeProjectSessionKey, appPreferencesLoaded, configurationRefreshId]);

  useEffect(() => {
    if (!nativeMenuEnabled) return;
    const state = view === "editor"
      ? editorNativeMenuState
      : inactiveNativeMenuState(
        view === "settings" || view === "projectSettings" || view === "systemHealth"
          ? "settings"
          : "home",
      );
    if (!state) return;
    void syncNativeMenuState(state).catch((error) => {
      if (!isBackendUnavailableError(error)) {
        console.error("Failed to sync native menu state", error);
      }
    });
  }, [editorNativeMenuState, nativeMenuEnabled, view]);

  useEffect(() => {
    if (!nativeMenuEnabled) return;
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listenForNativeMenuCommands((request) => {
      switch (request.command) {
        case "newProject":
        case "openProject":
          cancelProjectNavigation();
          setNativeEditorMenuRequest(null);
          setOpenProjectError(null);
          setView("home");
          return;
        case "openSettings":
          openAppSettings(
            { category: "general" },
            settingsOriginFor(viewRef.current),
          );
          return;
        case "openAdvancedSettings":
          openAppSettings(
            { category: "advanced", item: "mcp" },
            settingsOriginFor(viewRef.current),
          );
          return;
        case "openProjectSettings":
          if (activeProjectRef.current && viewRef.current === "editor") {
            openProjectSettings({
              element: activeElement(),
              returnView: "editor",
            });
          }
          return;
        case "openSystemHealth": {
          const returnView = systemHealthReturnView(viewRef.current);
          if (returnView) {
            openSystemHealth({
              element: activeElement(),
              returnView,
            });
          }
          return;
        }
        case "sendFeedback":
          window.open(feedbackMailto, "_self");
          return;
        default:
          // Everything else acts on the open editor, which ignores it unless it is the active view.
          if (viewRef.current === "editor") {
            setNativeEditorMenuRequest(request);
          }
      }
    })
      .then((registeredUnlisten) => {
        if (disposed) {
          registeredUnlisten();
        } else {
          unlisten = registeredUnlisten;
        }
      })
      .catch((error) => {
        if (!isBackendUnavailableError(error)) {
          console.error("Failed to register native menu bridge", error);
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [nativeMenuEnabled]);

  function activeElement() {
    return typeof document !== "undefined" &&
      document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  }

  function systemHealthReturnView(
    fromView: AppView,
  ): SystemHealthOrigin["returnView"] | null {
    if (fromView === "systemHealth") return null;
    return fromView;
  }

  function settingsOriginFor(fromView: AppView): SettingsOrigin {
    if (fromView === "systemHealth") {
      const origin = systemHealthOriginRef.current;
      return {
        element: origin.element,
        returnView:
          origin.returnView === "editor" ||
          origin.returnView === "projectSettings"
            ? "editor"
            : origin.returnView === "settings"
              ? settingsOriginRef.current.returnView
              : "home",
      };
    }
    return {
      element: activeElement(),
      returnView:
        fromView === "editor"
          ? "editor"
          : fromView === "projectSettings"
            ? "editor"
            : "home",
    };
  }

  function cancelFocusRestoration() {
    if (focusRestorationFrameRef.current !== null) {
      window.cancelAnimationFrame(focusRestorationFrameRef.current);
      focusRestorationFrameRef.current = null;
    }
  }

  function restoreFocus(element: HTMLElement | null) {
    cancelFocusRestoration();
    focusRestorationFrameRef.current = window.requestAnimationFrame(() => {
      focusRestorationFrameRef.current = null;
      if (element?.isConnected) {
        element.focus({ preventScroll: true });
      }
    });
  }

  function openAppSettings(
    target: AppSettingsTarget,
    origin: SettingsOrigin,
  ) {
    cancelFocusRestoration();
    cancelProjectNavigation();
    setNativeEditorMenuRequest(null);
    if (viewRef.current !== "settings") {
      setSettingsOrigin(origin);
      settingsPreferencesAcceptedRef.current = false;
    }
    setSettingsTarget(target);
    setSettingsNavigationRequestId((current) => current + 1);
    setView("settings");
  }

  function closeAppSettings() {
    const origin = settingsOrigin;
    const preserveAcceptedPreferences = settingsPreferencesAcceptedRef.current;
    cancelFocusRestoration();
    void loadAppPreferences()
      .then((preferences) => {
        if (!preserveAcceptedPreferences) setAppPreferences(preferences);
      })
      .catch((error) => {
        console.error("Failed to refresh app preferences after Settings", error);
      });
    setConfigurationRefreshId((current) => current + 1);
    setView(origin.returnView);
    restoreFocus(origin.element);
  }

  function openProjectSettings(origin: ProjectSettingsOrigin) {
    cancelFocusRestoration();
    cancelProjectNavigation();
    setNativeEditorMenuRequest(null);
    setProjectSettingsOrigin(origin);
    setView("projectSettings");
  }

  function closeProjectSettings() {
    const origin = projectSettingsOrigin;
    setView(origin.returnView);
    restoreFocus(origin.element);
  }

  function openSystemHealth(origin: SystemHealthOrigin) {
    cancelFocusRestoration();
    cancelProjectNavigation();
    setNativeEditorMenuRequest(null);
    setSystemHealthOrigin(origin);
    setView("systemHealth");
  }

  function closeSystemHealth() {
    const origin = systemHealthOrigin;
    setView(origin.returnView);
    restoreFocus(origin.element);
  }

  async function applyProjectSettings(action: UpdateProjectSettingsAction) {
    const result = await updateProjectSettingsInSplitProjectFolder({
      projectDir: activeProjectDir,
      name: action.name,
      renderSettings: action.renderSettings,
    });
    setActiveProject(result.project);
    if (remoteHostLabel) {
      setRemoteProjectEntries((current) => upsertRemoteProjectEntry(
        current,
        remoteProjectEntry({
          projectId: activeProjectDir,
          name: result.project.name,
          updatedAtMs: Date.now(),
        }),
      ));
    } else {
      rememberRecentProject(
        activeProjectDir,
        result.project,
        setRecentProjectEntries,
      );
    }
  }

  async function openSampleProject() {
    const requestId = beginProjectNavigation(
      "open",
      "Project folder could not be opened",
    );
    let project = createSampleProject();
    try {
      if (import.meta.env.DEV) {
        const {
          applySettingsVisualQaProjectFixture,
          readSettingsVisualQaFixture,
        } = await import("@/lib/settings-visual-qa-fixtures");
        project = applySettingsVisualQaProjectFixture(
          project,
          readSettingsVisualQaFixture(),
        );
      }
      if (!isCurrentProjectNavigation(requestId)) return;
      await materializeSampleProjectMedia({
        projectDir: sampleProjectEntry.projectDir,
      });
      // Opening the sample always restores its bundled state; an earlier session may already
      // have committed revisions to the sample folder, so write against the stored revision.
      const storedSample = await loadSplitProjectFromFolder({
        projectDir: sampleProjectEntry.projectDir,
      }).catch(() => null);
      const result = await saveSplitProjectToFolder({
        projectDir: sampleProjectEntry.projectDir,
        project,
        expectedRevision: storedSample?.contentRevision ?? project.contentRevision ?? 0,
      });
      if (!isCurrentProjectNavigation(requestId)) return;
      setEditorNativeMenuState(null);
      setActiveProjectDir(sampleProjectEntry.projectDir);
      setActiveProject(result.project);
      setView("editor");
    } catch (error) {
      if (!isCurrentProjectNavigation(requestId)) return;
      if (isBackendUnavailableError(error)) {
        setEditorNativeMenuState(null);
        setActiveProjectDir(sampleProjectBrowserDir);
        setActiveProject(project);
        setView("editor");
      } else {
        const copy = projectFolderRecoveryCopy(error, "open");
        setOpenProjectErrorTitle("Sample project could not be prepared");
        setOpenProjectError(copy.message);
      }
    } finally {
      finishProjectNavigation(requestId);
    }
  }

  async function openProjectFolder(projectDir: string) {
    const requestId = beginProjectNavigation(
      "open",
      "Project folder could not be opened",
    );
    try {
      const project = await loadSplitProjectFromFolder({ projectDir });
      if (!isCurrentProjectNavigation(requestId)) return;
      setEditorNativeMenuState(null);
      setActiveProjectDir(projectDir);
      setActiveProject(project);
      if (remoteHostLabel) {
        setRemoteProjectEntries((current) => upsertRemoteProjectEntry(
          current,
          remoteProjectEntry({
            projectId: projectDir,
            name: project.name,
            updatedAtMs: Date.now(),
          }),
        ));
      } else {
        rememberRecentProject(projectDir, project, setRecentProjectEntries);
      }
      setView("editor");
    } catch (error) {
      if (!isCurrentProjectNavigation(requestId)) return;
      const copy = projectFolderRecoveryCopy(error, "open");
      setOpenProjectErrorTitle(copy.title);
      setOpenProjectError(copy.message);
      if (remoteHostLabel) {
        setRemoteProjectEntries((current) =>
          current.filter((entry) => entry.projectDir !== projectDir));
      } else {
        markRecentProjectWarning(projectDir, copy.message, setRecentProjectEntries);
      }
    } finally {
      finishProjectNavigation(requestId);
    }
  }

  async function createProjectFolder(projectDir: string) {
    if (!appPreferences) {
      return;
    }
    const requestId = beginProjectNavigation(
      "create",
      "Project folder could not be created",
    );
    const project = createEmptySplitProject(
      projectDir,
      appPreferences.newProjectDefaults,
    );
    try {
      const result = await saveSplitProjectToFolder({
        projectDir,
        project,
        expectedRevision: project.contentRevision ?? 0,
      });
      if (!isCurrentProjectNavigation(requestId)) return;
      setEditorNativeMenuState(null);
      setActiveProjectDir(projectDir);
      setActiveProject(result.project);
      rememberRecentProject(projectDir, result.project, setRecentProjectEntries);
      setView("editor");
    } catch (error) {
      if (!isCurrentProjectNavigation(requestId)) return;
      const copy = projectFolderRecoveryCopy(error, "create");
      setOpenProjectErrorTitle(copy.title);
      setOpenProjectError(copy.message);
    } finally {
      finishProjectNavigation(requestId);
    }
  }

  async function createRemoteProject(name: string) {
    if (!appPreferences) return;
    const requestId = beginProjectNavigation("create", "Project could not be created");
    const project = createEmptySplitProject(name, appPreferences.newProjectDefaults);
    try {
      const result = await backendRequest<{
        catalogProjectId: string;
        project: VideoProject;
      }>("remote_create_project", { project });
      if (!isCurrentProjectNavigation(requestId)) return;
      remoteCatalogRequestIdRef.current += 1;
      setActiveProjectDir(result.catalogProjectId);
      setActiveProject(result.project);
      setRemoteProjectEntries((current) => upsertRemoteProjectEntry(
        current,
        {
          ...remoteProjectEntry({
            projectId: result.catalogProjectId,
            name: result.project.name,
            updatedAtMs: Date.now(),
          }),
          updatedAtLabel: "Created now",
        },
      ));
      setLoadedRemoteCatalogHost(remoteHostLabel ?? null);
      setView("editor");
    } catch (error) {
      if (!isCurrentProjectNavigation(requestId)) return;
      setOpenProjectError(error instanceof Error ? error.message : "The project could not be created.");
    } finally {
      finishProjectNavigation(requestId);
    }
  }

  if (!appPreferences) {
    return (
      <main
        aria-busy={appPreferencesError ? undefined : true}
        aria-label="App preferences"
        className="grid min-h-screen place-items-center bg-[#121314] p-6 text-foreground"
      >
        <div className="w-full max-w-sm rounded-md border border-white/10 bg-[#1d2021] p-4 text-sm">
          {appPreferencesError ? (
            <div role="alert">
              <div className="font-medium">Settings could not be loaded</div>
              <div className="mt-1 text-xs text-muted-foreground">
                {appPreferencesError}
              </div>
              <button
                type="button"
                className="mt-3 rounded-md border border-white/15 px-3 py-1.5 text-xs font-medium hover:bg-white/5 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                onClick={loadPreferences}
              >
                Retry loading settings
              </button>
            </div>
          ) : (
            <div role="status" className="text-muted-foreground">
              Loading settings…
            </div>
          )}
        </div>
      </main>
    );
  }

  return (
    <Suspense
      fallback={(
        <main
          role="status"
          aria-label="Loading app view"
          className="grid min-h-screen place-items-center bg-[#121314] text-sm text-muted-foreground"
        >
          Loading…
        </main>
      )}
    >
      {(view === "projectSettings" ||
        (view === "systemHealth" &&
          systemHealthOrigin.returnView === "projectSettings")) &&
      activeProject ? (
        <div hidden={view !== "projectSettings"} className="h-full min-h-0">
          <ProjectSettings
            project={activeProject}
            projectDir={activeProjectDir}
            onApply={applyProjectSettings}
            onBack={closeProjectSettings}
          />
        </div>
      ) : null}
      {view === "settings" ||
      (view === "systemHealth" && systemHealthOrigin.returnView === "settings") ? (
        <div hidden={view !== "settings"} className="h-full min-h-0">
          <Settings
            target={settingsTarget}
            navigationRequestId={settingsNavigationRequestId}
            projectRoot={activeProject ? activeProjectDir : null}
            onBack={closeAppSettings}
            preferences={appPreferences}
            onPreferencesAccepted={(preferences) => {
              settingsPreferencesAcceptedRef.current = true;
              setAppPreferences(preferences);
            }}
            onReadinessChange={handleSettingsReadinessChange}
            onOpenSystemHealth={() =>
              openSystemHealth({
                element: activeElement(),
                returnView: "settings",
              })
            }
          />
        </div>
      ) : null}
      {view === "systemHealth" ? (
        <SystemHealth
          projectRoot={activeProject ? activeProjectDir : null}
          onBack={closeSystemHealth}
        />
      ) : null}
      {view === "home" ||
      (view === "settings" && settingsOrigin.returnView === "home") ||
      (view === "systemHealth" &&
        (systemHealthOrigin.returnView === "home" ||
          (systemHealthOrigin.returnView === "settings" &&
            settingsOrigin.returnView === "home"))) ? (
        <div hidden={view !== "home"} className="h-full min-h-0">
          <ProjectHome
            key={suggestedProjectParent(appPreferences)}
            recentProjects={remoteHostLabel
              ? loadedRemoteCatalogHost === remoteHostLabel ? remoteProjectEntries : []
              : [sampleProjectEntry, ...recentProjectEntries]}
            remoteHostLabel={remoteHostLabel}
            suggestedProjectParent={suggestedProjectParent(appPreferences)}
            onOpenSample={openSampleProject}
            onOpenProject={(entry) => {
              if (entry.id === sampleProjectEntry.id) {
                openSampleProject();
                return;
              }
              void openProjectFolder(entry.projectDir);
            }}
            onOpenProjectFolder={(projectDir) => void openProjectFolder(projectDir)}
            onCreateProject={(projectDir) => void createProjectFolder(projectDir)}
            onCreateRemoteProject={(name) => void createRemoteProject(name)}
            {...(!remoteHostLabel && {
              onRemoveProject: (entry: RecentProjectEntry) =>
                removeRecentProject(entry.projectDir, setRecentProjectEntries),
            })}
            onOpenModelSettings={() =>
              openAppSettings(
                { category: "aiModels", item: "transcription" },
                settingsOriginFor("home"),
              )
            }
            isOpeningProject={projectNavigationKind === "open"}
            isCreatingProject={projectNavigationKind === "create"}
            openProjectError={openProjectError}
            openProjectErrorTitle={openProjectErrorTitle}
          />
        </div>
      ) : null}
      {view === "editor" ||
      view === "projectSettings" ||
      (view === "settings" && settingsOrigin.returnView === "editor") ||
      (view === "systemHealth" &&
        (systemHealthOrigin.returnView === "editor" ||
          systemHealthOrigin.returnView === "projectSettings" ||
          (systemHealthOrigin.returnView === "settings" &&
            settingsOrigin.returnView === "editor"))) ? (
        <div hidden={view !== "editor"} className="h-full min-h-0">
          {activeProject ? (
            <EditorRoot
              key={projectSessionKey(activeProjectDir, activeProject.id)}
              isActive={view === "editor"}
              transcriptionModelReady={settingsReadiness.transcriptionModelReady}
              speechModelsReady={settingsReadiness.speechModelsReady}
              runtimeReady={settingsReadiness.runtimeReady}
              projectDir={activeProjectDir}
              initialProject={activeProject}
              nativeMenuRequest={view === "editor" ? nativeEditorMenuRequest : null}
              onNativeMenuStateChange={setEditorNativeMenuState}
              appPreferences={appPreferences}
              configurationRefreshId={configurationRefreshId}
              onOpenProjectHome={() => {
                cancelProjectNavigation();
                setNativeEditorMenuRequest(null);
                setView("home");
              }}
              onOpenModelSettings={(target?: AppSettingsTarget) =>
                openAppSettings(
                  target ?? { category: "aiModels", item: "transcription" },
                  settingsOriginFor("editor"),
                )
              }
              onOpenSettings={(target, originElement) =>
                openAppSettings(target, {
                  element: originElement,
                  returnView: "editor",
                })
              }
              onOpenProjectSettings={(originElement) =>
                openProjectSettings({
                  element: originElement,
                  returnView: "editor",
                })
              }
            />
          ) : null}
        </div>
      ) : null}
    </Suspense>
  );
}

export function createEmptySplitProject(
  projectDir: string,
  defaults: NewProjectDefaults,
): VideoProject {
  const now = new Date().toISOString();
  return {
    schemaVersion: 2,
    id: `project-${cryptoSafeId(projectDir)}`,
    name: projectNameFromDir(projectDir),
    createdAt: now,
    updatedAt: now,
    media: [],
    generatedAssets: [],
    renderReports: [],
    transcripts: [],
    timeline: {
      durationSeconds: 0,
      tracks: [
        { id: "track-video", name: "Video", kind: "video", locked: false, enabled: true, items: [] },
        {
          id: "track-scenes",
          name: "HyperFrames",
          kind: "hyperframe_scene",
          locked: false,
          enabled: true,
          items: [],
        },
        {
          id: "track-overlays",
          name: "Overlays",
          kind: "overlay",
          locked: false,
          enabled: true,
          items: [],
        },
        {
          id: "track-captions",
          name: "Captions",
          kind: "caption",
          locked: false,
          enabled: true,
          items: [],
        },
        { id: "track-audio", name: "Audio", kind: "audio", locked: false, enabled: true, items: [] },
      ],
    },
    renderSettings: { ...defaults },
    codexThreadId: null,
    jobs: [],
  };
}

function loadRecentProjectEntries(): RecentProjectEntry[] {
  if (typeof window === "undefined") {
    return [];
  }

  try {
    const raw = window.localStorage.getItem(recentProjectsStorageKey);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .filter(isRecentProjectEntry)
      .filter((entry) => entry.id !== sampleProjectEntry.id)
      .slice(0, maxRecentProjects);
  } catch {
    return [];
  }
}

function remoteProjectEntry(project: RemoteProjectSummary): RecentProjectEntry {
  return {
    id: project.projectId,
    name: project.name,
    projectDir: project.projectId,
    updatedAtLabel: project.updatedAtMs > 0
      ? `Updated ${new Date(project.updatedAtMs).toLocaleDateString()}`
      : "On host",
    statusLabel: "Available",
  };
}

function dedupeRemoteProjectEntries(
  entries: readonly RecentProjectEntry[],
): RecentProjectEntry[] {
  const seen = new Set<string>();
  return entries.filter((entry) => {
    if (seen.has(entry.projectDir)) return false;
    seen.add(entry.projectDir);
    return true;
  });
}

function upsertRemoteProjectEntry(
  current: readonly RecentProjectEntry[],
  entry: RecentProjectEntry,
): RecentProjectEntry[] {
  return dedupeRemoteProjectEntries([
    entry,
    ...current.filter((candidate) => candidate.projectDir !== entry.projectDir),
  ]).slice(0, maxRecentProjects);
}

function rememberRecentProject(
  projectDir: string,
  project: VideoProject,
  setRecentProjectEntries: Dispatch<SetStateAction<RecentProjectEntry[]>>,
) {
  const entry: RecentProjectEntry = {
    id: `recent:${projectDir}`,
    name: project.name?.trim() || projectNameFromDir(projectDir),
    projectDir,
    updatedAtLabel: "Opened recently",
    statusLabel: "Local project",
  };

  setRecentProjectEntries((current) => {
    const next = [
      entry,
      ...current.filter((candidate) => candidate.projectDir !== projectDir),
    ].slice(0, maxRecentProjects);
    saveRecentProjectEntries(next);
    return next;
  });
}

function markRecentProjectWarning(
  projectDir: string,
  message: string,
  setRecentProjectEntries: Dispatch<SetStateAction<RecentProjectEntry[]>>,
) {
  setRecentProjectEntries((current) => {
    let changed = false;
    const next = current.map((entry) => {
      if (entry.projectDir !== projectDir) {
        return entry;
      }
      changed = true;
      return {
        ...entry,
        statusLabel: "Needs relink",
        warningLabel: message,
      };
    });
    if (changed) {
      saveRecentProjectEntries(next);
    }
    return next;
  });
}

function removeRecentProject(
  projectDir: string,
  setRecentProjectEntries: Dispatch<SetStateAction<RecentProjectEntry[]>>,
) {
  setRecentProjectEntries((current) => {
    const next = current.filter((entry) => entry.projectDir !== projectDir);
    saveRecentProjectEntries(next);
    return next;
  });
}

function saveRecentProjectEntries(entries: RecentProjectEntry[]) {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(recentProjectsStorageKey, JSON.stringify(entries));
  } catch {
    // Recent projects are a convenience cache; opening the project has already succeeded.
  }
}

function isRecentProjectEntry(value: unknown): value is RecentProjectEntry {
  if (!value || typeof value !== "object") {
    return false;
  }
  const entry = value as Partial<RecentProjectEntry>;
  return (
    typeof entry.id === "string" &&
    typeof entry.name === "string" &&
    typeof entry.projectDir === "string" &&
    typeof entry.updatedAtLabel === "string" &&
    (entry.statusLabel === undefined || typeof entry.statusLabel === "string") &&
    (entry.warningLabel === undefined || typeof entry.warningLabel === "string")
  );
}

function cryptoSafeId(value: string) {
  const sanitized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
  return sanitized || "local-project";
}

function projectNameFromDir(projectDir: string) {
  const normalized = projectDir.replace(/[/\\]+$/, "");
  const parts = normalized.split(/[/\\]/);
  return parts.at(-1) || "Local project";
}
