import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { describe, expect, it, vi } from "vitest";
import { ProjectHome, type RecentProjectEntry } from "./project-home";

function renderProjectHome(props: Partial<ComponentProps<typeof ProjectHome>> = {}) {
  const defaults = {
    recentProjects: [],
    onOpenSample: vi.fn(),
    onOpenProject: vi.fn(),
    onOpenProjectFolder: vi.fn(),
    onCreateProject: vi.fn(),
  } satisfies ComponentProps<typeof ProjectHome>;
  const merged = { ...defaults, ...props };
  render(<ProjectHome {...merged} />);
  return merged;
}

const recentProject: RecentProjectEntry = {
  id: "recent-1",
  name: "Launch Cut",
  projectDir: "/Users/editor/Launch Cut",
  updatedAtLabel: "Updated today",
  statusLabel: "Saved",
};

describe("ProjectHome", () => {
  it("does not render simulated window controls", () => {
    renderProjectHome();

    expect(screen.queryByLabelText("Window controls")).not.toBeInTheDocument();
  });

  it("matches Palmier's fixed sidebar and compact project-card geometry", () => {
    renderProjectHome();

    expect(screen.getByTestId("project-home-sidebar")).toHaveClass("w-[220px]");
    expect(screen.getByTestId("project-home-main")).toBeInTheDocument();
    expect(screen.getByTestId("project-home-content")).toHaveClass("mx-6");
    expect(screen.getByRole("heading", { name: "Welcome to Video Creater" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Sample Project" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "My Projects" })).toBeVisible();
    for (const card of screen.getAllByTestId("project-card")) {
      expect(card).toHaveClass("h-[120px]", "w-[150px]");
    }
  });

  it("keeps new, open, sample, and settings actions in the sidebar", () => {
    const onOpenSample = vi.fn();
    const onOpenModelSettings = vi.fn();
    renderProjectHome({ onOpenSample, onOpenModelSettings });

    const sidebar = screen.getByTestId("project-home-sidebar");
    fireEvent.click(within(sidebar).getByRole("button", { name: "Open sample" }));
    fireEvent.click(within(sidebar).getByRole("button", { name: "Model settings" }));
    expect(onOpenSample).toHaveBeenCalledTimes(1);
    expect(onOpenModelSettings).toHaveBeenCalledTimes(1);
  });

  it("opens the bundled sample from its visual card", () => {
    const onOpenSample = vi.fn();
    renderProjectHome({
      onOpenSample,
      recentProjects: [
        {
          id: "sample-editor-project",
          name: "Palmier Sample",
          projectDir: "/tmp/video-creater-editor-project",
          updatedAtLabel: "Sample project",
        },
      ],
    });

    const samples = screen.getByRole("region", { name: "Sample projects" });
    expect(within(samples).getByLabelText("Bundled sample preview")).toHaveAttribute(
      "src",
      "/media/input.mp4",
    );
    expect(within(samples).getByText("Palmier Sample")).toBeVisible();
    fireEvent.click(within(samples).getByTestId("project-card"));
    expect(onOpenSample).toHaveBeenCalledTimes(1);
  });

  it("opens recent project cards and keeps missing-project recovery", () => {
    const onOpenProject = vi.fn();
    const onRemoveProject = vi.fn();
    renderProjectHome({
      recentProjects: [
        recentProject,
        {
          ...recentProject,
          id: "missing-1",
          name: "Missing Cut",
          projectDir: "/Volumes/missing/Missing Cut",
          warningLabel: "Folder missing",
        },
      ],
      onOpenProject,
      onRemoveProject,
    });

    const launchCard = screen.getByRole("article", { name: "Recent project Launch Cut" });
    fireEvent.click(within(launchCard).getByRole("button", { name: "Open project" }));
    expect(onOpenProject).toHaveBeenCalledWith(recentProject);
    const missing = screen.getByRole("article", { name: "Recent project Missing Cut" });
    fireEvent.click(within(missing).getByRole("button", { name: "Relink project" }));
    expect(screen.getByLabelText("Project folder path")).toHaveValue(
      "/Volumes/missing/Missing Cut",
    );
    fireEvent.click(within(missing).getByRole("button", { name: "Remove from recents" }));
    expect(onRemoveProject).toHaveBeenCalledWith(expect.objectContaining({ id: "missing-1" }));
  });

  it("opens and creates typed project paths from the native sidebar flow", () => {
    const onOpenProjectFolder = vi.fn();
    const onCreateProject = vi.fn();
    renderProjectHome({ onOpenProjectFolder, onCreateProject });

    fireEvent.click(screen.getByRole("button", { name: "Open Project" }));
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "  /Users/editor/Existing Cut  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    expect(onOpenProjectFolder).toHaveBeenCalledWith("/Users/editor/Existing Cut");

    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "" },
    });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: " /Users/editor/New Cut " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(onCreateProject).toHaveBeenCalledWith("/Users/editor/New Cut");
  });

  it("uses host-managed names and opaque project cards in browser mode", () => {
    const onCreateRemoteProject = vi.fn();
    renderProjectHome({
      remoteHostLabel: "olhapi-t3code",
      onCreateRemoteProject,
      recentProjects: [{
        id: "opaque-9f82",
        name: "Remote Cut",
        projectDir: "opaque-9f82",
        updatedAtLabel: "Updated today",
      }],
    });

    expect(screen.queryByLabelText("Project folder path")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open sample" })).not.toBeInTheDocument();
    expect(screen.getAllByText("On olhapi-t3code").length).toBeGreaterThan(0);
    expect(screen.queryByText("opaque-9f82")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: "Remote Draft" } });
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(onCreateRemoteProject).toHaveBeenCalledWith("Remote Draft");
  });

  it("uses a suggested parent only as the initial open-dialog directory", () => {
    const onOpenProjectFolder = vi.fn();
    renderProjectHome({
      suggestedProjectParent: "/Users/editor/Projects",
      onOpenProjectFolder,
    });

    expect(screen.getByLabelText("Project folder path")).toHaveValue(
      "/Users/editor/Projects",
    );
    expect(screen.getByRole("button", { name: "Open project folder" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "/Users/editor/Projects/Existing Cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));

    expect(onOpenProjectFolder).toHaveBeenCalledWith(
      "/Users/editor/Projects/Existing Cut",
    );
    expect(onOpenProjectFolder).not.toHaveBeenCalledWith("/Users/editor/Projects");
  });

  it("never creates the suggested parent as a project", () => {
    const onCreateProject = vi.fn();
    renderProjectHome({
      suggestedProjectParent: "/Users/editor/Projects",
      onCreateProject,
    });

    fireEvent.click(screen.getByRole("button", { name: "New project" }));

    expect(onCreateProject).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "/Users/editor/Projects/New Cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));

    expect(onCreateProject).toHaveBeenCalledWith("/Users/editor/Projects/New Cut");
    expect(onCreateProject).not.toHaveBeenCalledWith("/Users/editor/Projects");
  });

  it("keeps create blocked when the restored suggested parent drops its trailing separator", () => {
    const onCreateProject = vi.fn();
    renderProjectHome({
      suggestedProjectParent: "/Users/editor/Projects/",
      onCreateProject,
    });

    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "/Users/editor/Projects/New Cut" },
    });
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "  /Users/editor/Projects  " },
    });

    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(onCreateProject).not.toHaveBeenCalled();
  });

  it("keeps open blocked for an equivalent suggested parent with dot and repeated separators", () => {
    const onOpenProjectFolder = vi.fn();
    renderProjectHome({
      suggestedProjectParent: "/Users/editor/Projects",
      onOpenProjectFolder,
    });

    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "/Users/editor/Projects/Existing Cut" },
    });
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "  /../../Users//editor/./Projects/Cuts/..  " },
    });

    expect(screen.getByRole("button", { name: "Open project folder" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    expect(onOpenProjectFolder).not.toHaveBeenCalled();
  });

  it("keeps create blocked for an equivalent Windows drive parent with separator and case aliases", () => {
    const onCreateProject = vi.fn();
    renderProjectHome({
      suggestedProjectParent: "C:\\Studio\\Projects\\",
      onCreateProject,
    });

    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "C:\\Studio\\Projects\\New Cut" },
    });
    expect(screen.getByRole("button", { name: "Create project" })).toBeEnabled();
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "  c:/studio//./projects  " },
    });

    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(onCreateProject).not.toHaveBeenCalled();
  });

  it("keeps open blocked for an equivalent UNC parent with separator and case aliases", () => {
    const onOpenProjectFolder = vi.fn();
    renderProjectHome({
      suggestedProjectParent: "\\\\Server\\Share\\Projects",
      onOpenProjectFolder,
    });

    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "\\\\Server\\Share\\Projects\\Existing Cut" },
    });
    expect(screen.getByRole("button", { name: "Open project folder" })).toBeEnabled();
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "  //server//share/./projects/Cuts/..  " },
    });

    expect(screen.getByRole("button", { name: "Open project folder" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    expect(onOpenProjectFolder).not.toHaveBeenCalled();
  });

  it("shows project-open errors beside the active sidebar form", () => {
    renderProjectHome({
      suggestedProjectParent: "/Volumes/offline/Cut",
      openProjectErrorTitle: "Folder unavailable",
      openProjectError: "Choose a local split project folder.",
    });

    expect(screen.getByRole("alert")).toHaveTextContent("Folder unavailable");
    expect(screen.getByRole("alert")).toHaveTextContent("Choose a local split project folder");
  });
});
