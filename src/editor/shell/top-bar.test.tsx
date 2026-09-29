import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { TooltipProvider } from "@/components/ui/tooltip";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import {
  clearRemoteProjectAccessForTests,
  setRemoteProjectAccess,
  setRemoteTakeoverHandler,
} from "@/lib/runtime/adapters/remote-project-access";
import { createEditorStore } from "../store/editor-store";
import { EditorStoreProvider } from "../store/editor-store-context";
import { TopBar } from "./top-bar";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function renderTopBar(overrides: Partial<Parameters<typeof TopBar>[0]> = {}, project: VideoProject = fixtureProject()) {
  const store = createEditorStore({ projectDir: "/p", project });
  const props = {
    compact: false,
    onOpenProjectHome: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenProjectSettings: vi.fn(),
    ...overrides,
  };
  render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>
        <TopBar {...props} />
      </EditorStoreProvider>
    </TooltipProvider>,
  );
  return { store, props };
}

function setViewportWidth(width: number) {
  Object.defineProperty(window, "innerWidth", { configurable: true, value: width });
  window.dispatchEvent(new Event("resize"));
}

/** The sample with a long name and a running transcription, so the tasks pill shows. */
function busyProject(): VideoProject {
  return {
    ...fixtureProject(),
    name: "Edison Restoration Demo with a very long working title",
    schemaVersion: 2,
    jobs: [{ id: "transcribe-1", kind: "transcribe_media", status: "running", updatedAt: new Date().toISOString() }],
    generatedAssets: [],
    renderReports: [],
    exportArtifacts: [],
  };
}

describe("TopBar", () => {
  beforeEach(() => {
    installRuntimeMode("desktop");
    clearRemoteProjectAccessForTests();
  });
  afterEach(() => setViewportWidth(1024));

  it("pads below the status bar with the top safe-area inset", () => {
    renderTopBar();
    expect(screen.getByRole("banner")).toHaveClass("box-content", "h-12", "pt-[env(safe-area-inset-top)]");
  });

  it("at 402 px keeps Home, Undo, Redo, a spinner-only tasks pill and Export while the name truncates", () => {
    setViewportWidth(402);
    const { store } = renderTopBar({ compact: true }, busyProject());
    // Record the derived inputs first, so the progress override isn't re-derived away.
    store.getState().syncJobs();
    act(() => store.setState({ tasks: store.getState().tasks.map((task) => ({ ...task, progress: 0.62 })) }));
    const header = screen.getByRole("banner");
    const name = screen.getByTestId("project-name");
    expect(name).toHaveClass("min-w-0", "truncate");
    const fixed = [
      screen.getByRole("button", { name: "Home" }),
      screen.getByRole("button", { name: "Undo" }),
      screen.getByRole("button", { name: "Redo" }),
      screen.getByRole("button", { name: "Background tasks" }),
      screen.getByRole("button", { name: "Editor menu" }),
      screen.getByRole("button", { name: "Export" }),
    ];
    // Only the name may shrink, so the fixed controls never overflow the bar.
    for (const control of fixed) expect(control).toHaveClass("shrink-0");
    const pill = screen.getByRole("button", { name: "Background tasks" });
    expect(within(pill).getByTestId("tasks-spinner")).toBeInTheDocument();
    // Visible content is the spinner and percent; the label is only the accessible description.
    expect(Array.from(pill.querySelectorAll("span[aria-hidden]"), (span) => span.textContent)).toEqual(["62%"]);
    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
    expect(header).toHaveClass("flex", "items-center");
    expect(screen.queryByRole("button", { name: "More" })).not.toBeInTheDocument();
  });

  it("folds the gear menu into More under 380 px", async () => {
    setViewportWidth(375);
    const { props } = renderTopBar({ compact: true });
    expect(screen.queryByRole("button", { name: "Editor menu" })).not.toBeInTheDocument();
    const more = screen.getByRole("button", { name: "More" });
    fireEvent.keyDown(more, { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Project settings" }));
    expect(props.onOpenProjectSettings).toHaveBeenCalledWith(more);

    act(() => setViewportWidth(402));
    expect(screen.getByRole("button", { name: "Editor menu" })).toBeInTheDocument();
  });

  it("shows project name, save state, and disabled undo/redo on a fresh project", () => {
    renderTopBar();
    expect(screen.getByText("Edison Restoration Demo")).toBeInTheDocument();
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Redo" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Export" })).toBeEnabled();
  });

  it("hides the save state in compact mode", () => {
    renderTopBar({ compact: true });
    expect(screen.queryByText("Saved")).not.toBeInTheDocument();
  });

  it("goes home and opens settings from the gear menu", async () => {
    const { props } = renderTopBar();
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(props.onOpenProjectHome).toHaveBeenCalledTimes(1);
    const menuButton = screen.getByRole("button", { name: "Editor menu" });
    // jsdom lacks PointerEvent, so pointerDown loses `button`; open the Radix menu via keyboard instead.
    fireEvent.keyDown(menuButton, { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "App settings" }));
    expect(props.onOpenSettings).toHaveBeenCalledTimes(1);
    expect(props.onOpenSettings).toHaveBeenCalledWith(menuButton);
  });

  it("opens the Export popover from the Export button", async () => {
    const { store } = renderTopBar();
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    expect(await screen.findByRole("dialog", { name: "Export" })).toBeInTheDocument();
    expect(store.getState().exportPopover).toEqual({ preset: null });
  });

  it("shows remote ownership at the edit target and confirms takeover", async () => {
    act(() => {
      installRuntimeMode("browser");
      setRemoteProjectAccess("/p", { mode: "readOnly", editorDisplayName: "Office laptop" });
    });
    const takeover = vi.fn(async () => undefined);
    setRemoteTakeoverHandler(takeover);
    renderTopBar();

    fireEvent.click(screen.getByRole("button", { name: "View only · Take over" }));
    const dialog = await screen.findByRole("dialog", { name: "Take over editing?" });
    expect(dialog).toHaveTextContent("Office laptop will become view only");
    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Take over" }));
      await Promise.resolve();
    });
    expect(takeover).toHaveBeenCalledWith("/p");
  });
});
