import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppPreferences } from "@/lib/app-settings";
import { createSampleProject } from "@/lib/sample-project";
import { EditorRoot, type EditorRootProps } from "./editor-root";

function renderProps(overrides: Partial<EditorRootProps> = {}): EditorRootProps {
  return {
    isActive: true,
    projectDir: "/tmp/project",
    initialProject: createSampleProject(),
    appPreferences: defaultAppPreferences,
    configurationRefreshId: 0,
    transcriptionModelReady: false,
    speechModelsReady: false,
    runtimeReady: true,
    nativeMenuRequest: null,
    onNativeMenuStateChange: vi.fn(),
    onOpenProjectHome: vi.fn(),
    onOpenModelSettings: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenProjectSettings: vi.fn(),
    ...overrides,
  };
}

function renderRoot(overrides: Partial<EditorRootProps> = {}) {
  const props = renderProps(overrides);
  render(<EditorRoot {...props} />);
  return props;
}

describe("EditorRoot", () => {
  beforeEach(() => window.localStorage.clear());

  it("renders the editor landmark with the project name and reports editor menu state", () => {
    const props = renderRoot();

    expect(screen.getByRole("main", { name: "Video editor workspace" })).toBeInTheDocument();
    expect(screen.getByText("Edison Restoration Demo")).toBeInTheDocument();
    expect(props.onNativeMenuStateChange).toHaveBeenCalledWith(expect.objectContaining({ view: "editor", canImport: true, canExport: true, canUndo: false }));
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(props.onOpenProjectHome).toHaveBeenCalledTimes(1);
  });

  it("clears the v1 editor storage keys on mount and keeps v2 keys", () => {
    window.localStorage.setItem("video-creater.workspace-layout.v1", "{}");
    window.localStorage.setItem("video-creater.editor.v2.layout", JSON.stringify({ activeTab: "captions", leftWidth: 320 }));

    renderRoot();

    expect(window.localStorage.getItem("video-creater.workspace-layout.v1")).toBeNull();
    expect(window.localStorage.getItem("video-creater.editor.v2.layout")).not.toBeNull();
    expect(window.localStorage.getItem("video-creater.editor.v2.legacy-cleaned")).toBe("true");
  });

  it("renders the shell regions", () => {
    renderRoot();
    expect(screen.getByRole("region", { name: "Preview panel" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Preview viewport" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Timeline canvas" })).toBeInTheDocument();
    expect(screen.getByRole("tablist", { name: "Editor tools" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
  });

  it("opens general app settings from the editor menu with the menu button as origin", async () => {
    const props = renderRoot();
    const menuButton = screen.getByRole("button", { name: "Editor menu" });
    // jsdom lacks PointerEvent, so pointerDown loses `button`; open the Radix menu via keyboard instead.
    fireEvent.keyDown(menuButton, { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "App settings" }));
    expect(props.onOpenSettings).toHaveBeenCalledWith({ category: "general" }, menuButton);
  });

  it("opens project settings from the editor menu with the menu button as origin", async () => {
    const props = renderRoot();
    const menuButton = screen.getByRole("button", { name: "Editor menu" });
    fireEvent.keyDown(menuButton, { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Project settings" }));
    expect(props.onOpenProjectSettings).toHaveBeenCalledWith(menuButton);
    expect(props.onOpenSettings).not.toHaveBeenCalled();
  });

  it("routes forwarded native menu commands through the editor store", async () => {
    const props = renderProps();
    const { rerender } = render(<EditorRoot {...props} />);
    rerender(<EditorRoot {...props} nativeMenuRequest={{ sequence: 1, command: "showTab:captions" }} />);
    expect(screen.getByRole("tab", { name: "Captions" })).toHaveAttribute("data-state", "active");

    rerender(<EditorRoot {...props} nativeMenuRequest={{ sequence: 2, command: "openShortcuts" }} />);
    expect(await screen.findByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
  });

  it("ignores native menu commands while another view is active", () => {
    const props = renderProps({ isActive: false });
    const { rerender } = render(<EditorRoot {...props} />);
    rerender(<EditorRoot {...props} nativeMenuRequest={{ sequence: 1, command: "showTab:captions" }} />);
    expect(screen.getByRole("tab", { name: "AI" })).toHaveAttribute("data-state", "active");
    expect(props.onNativeMenuStateChange).not.toHaveBeenCalled();
  });

  it("handles global shortcuts only while the editor is active", () => {
    // Without a native host the platform stays macOS, so the primary modifier is Meta.
    const { unmount } = render(<EditorRoot {...renderProps({ isActive: false })} />);
    fireEvent.keyDown(window, { key: "5", metaKey: true });
    expect(screen.getByRole("tab", { name: "AI" })).toHaveAttribute("data-state", "active");
    unmount();

    renderRoot();
    fireEvent.keyDown(window, { key: "5", metaKey: true });
    expect(screen.getByRole("tab", { name: "Captions" })).toHaveAttribute("data-state", "active");
  });
});
