import { useEffect, useState } from "react";
import { Toaster } from "@/components/ui/toaster";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { AppSettingsPreferences } from "@/lib/app-settings";
import type { NativeMenuRequest, NativeMenuState } from "@/lib/native-menu";
import type { VideoProject } from "@/lib/project";
import type { AppSettingsTarget } from "@/lib/settings/target";
import { useNativeMenuBridge } from "./native-menu-bridge";
import { EditorEnvironmentProvider } from "./services/editor-environment";
import { startExportRuntime } from "./services/export-service";
import { disposeGenerationRuntime } from "./services/generation-service";
import { DesktopLayout } from "./shell/desktop-layout";
import { MobileLayout } from "./shell/mobile-layout";
import { TopBar } from "./shell/top-bar";
import { useEditorShortcuts } from "./shell/use-editor-shortcuts";
import { useLayoutMode } from "./shell/use-layout-mode";
import { useShortcutPlatform } from "./shell/use-shortcut-platform";
import { createEditorStore } from "./store/editor-store";
import { cleanLegacyEditorStorage } from "./store/legacy-storage-cleanup";
import { EditorStoreProvider, useEditorStore } from "./store/editor-store-context";

export interface EditorRootProps {
  readonly isActive: boolean;
  readonly projectDir: string;
  readonly initialProject: VideoProject;
  readonly appPreferences: AppSettingsPreferences;
  readonly configurationRefreshId: number;
  readonly transcriptionModelReady: boolean;
  readonly speechModelsReady: boolean;
  readonly runtimeReady: boolean;
  readonly nativeMenuRequest: NativeMenuRequest | null;
  readonly onNativeMenuStateChange: (state: NativeMenuState) => void;
  readonly onOpenProjectHome: () => void;
  readonly onOpenModelSettings: () => void;
  readonly onOpenSettings: (target: AppSettingsTarget, originElement: HTMLElement) => void;
  /** Receives the editor menu button so project settings can return focus to it on close. */
  readonly onOpenProjectSettings: (originElement: HTMLElement) => void;
}

export function EditorRoot(props: EditorRootProps) {
  const { initialProject, projectDir } = props;
  const [store] = useState(() => createEditorStore({ projectDir, project: initialProject }));

  // v1 editor layout, tour, rail and chat keys have no v2 reader; drop them once per profile.
  useEffect(() => cleanLegacyEditorStorage(), []);

  // Job polling belongs to this editor session: it runs while tasks are active and stops when the project closes.
  // The export runner lets a failed export's Retry re-run its stored plan. The agent loads its chat
  // sessions and registers its turn cancel with the background tasks list.
  useEffect(() => {
    store.getState().startPolling();
    const stopExports = startExportRuntime(store);
    const stopAgent = store.getState().startAgent();
    return () => {
      store.getState().stopPolling();
      disposeGenerationRuntime(store);
      stopExports();
      stopAgent();
    };
  }, [store]);

  return (
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <EditorEnvironmentProvider
          transcriptionModelReady={props.transcriptionModelReady}
          speechModelsReady={props.speechModelsReady}
          onOpenModelSettings={props.onOpenModelSettings}
          appPreferences={props.appPreferences}
          configurationRefreshId={props.configurationRefreshId}
          onOpenSettings={props.onOpenSettings}
        >
          <EditorShell {...props} />
        </EditorEnvironmentProvider>
      </EditorStoreProvider>
    </TooltipProvider>
  );
}

function EditorShell(props: EditorRootProps) {
  const { isActive, onOpenProjectHome, onOpenSettings, onOpenProjectSettings } = props;
  const mode = useLayoutMode();
  // The native menu reports and routes only while the editor is the active view, like its shortcuts.
  useNativeMenuBridge({
    isActive,
    mobile: mode === "mobile",
    request: props.nativeMenuRequest,
    onStateChange: props.onNativeMenuStateChange,
  });
  const topBar = (
    <TopBar
      compact={mode === "mobile"}
      onOpenProjectHome={onOpenProjectHome}
      onOpenSettings={(originElement) => onOpenSettings({ category: "general" }, originElement)}
      onOpenProjectSettings={onOpenProjectSettings}
    />
  );
  return (
    <>
      {/* The editor stays mounted behind Settings, so shortcuts only listen while it is the active view. */}
      {isActive && <EditorShortcuts />}
      {mode === "mobile" ? <MobileLayout topBar={topBar} /> : <DesktopLayout mode={mode} topBar={topBar} />}
      <EditorToaster />
    </>
  );
}

function EditorToaster() {
  const toasts = useEditorStore((state) => state.toasts);
  const dismissToast = useEditorStore((state) => state.dismissToast);
  return <Toaster toasts={toasts} onDismiss={dismissToast} />;
}

function EditorShortcuts() {
  useEditorShortcuts(useShortcutPlatform());
  return null;
}
