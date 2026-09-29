import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProjectSkillsDialog } from "./project-skills-dialog";

describe("ProjectSkillsDialog", () => {
  it("lists the three required skills with descriptions and paths", () => {
    const onOpenProjectSettings = vi.fn();
    render(<ProjectSkillsDialog open onOpenChange={vi.fn()} onOpenProjectSettings={onOpenProjectSettings} />);
    const dialog = screen.getByRole("dialog", { name: "Project skills" });

    const skills = within(dialog).getAllByRole("listitem");
    expect(skills.map((skill) => within(skill).getByRole("heading", { level: 3 }).textContent)).toEqual([
      "Editing pipeline",
      "Video graphics",
      "Editor interface",
    ]);
    expect(within(dialog).getByText(".agents/skills/video-creater-graphics/SKILL.md")).toBeInTheDocument();
    expect(within(dialog).getByText(/HyperFrames scenes, title cards/)).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "Verify in Project settings" }));
    expect(onOpenProjectSettings).toHaveBeenCalledTimes(1);
  });
});
