import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { defaultAppPreferences } from "@/lib/app-settings";
import { ProjectsSettings } from "./projects-settings";

describe("ProjectsSettings", () => {
  it("updates validated new-project defaults", () => {
    const onChange = vi.fn();
    render(
      <ProjectsSettings
        preferences={defaultAppPreferences}
        onChange={onChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("Project frame rate"), {
      target: { value: "24" },
    });
    expect(onChange).toHaveBeenCalledWith({
      newProjectDefaults: { fps: 24 },
    });
  });

  it("applies standard presets as one validated patch", () => {
    const onChange = vi.fn();
    render(
      <ProjectsSettings
        preferences={defaultAppPreferences}
        onChange={onChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("Project format preset"), {
      target: { value: "uhd" },
    });
    expect(onChange).toHaveBeenCalledWith({
      newProjectDefaults: {
        width: 3840,
        height: 2160,
      },
    });
  });

  it("rejects odd custom dimensions before calling Rust", () => {
    const onChange = vi.fn();
    render(
      <ProjectsSettings
        preferences={defaultAppPreferences}
        onChange={onChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("Project width"), {
      target: { value: "1919" },
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Dimensions must be even values between 2 and 16384.",
    );
    expect(onChange).not.toHaveBeenCalled();
  });

  it("preserves rapid edits in the next complete native defaults patch", () => {
    const onChange = vi.fn();
    render(
      <ProjectsSettings
        preferences={defaultAppPreferences}
        onChange={onChange}
      />,
    );

    fireEvent.change(screen.getByLabelText("Project loudness target"), {
      target: { value: "-16" },
    });
    fireEvent.change(screen.getByLabelText("Project caption mode"), {
      target: { value: "mux" },
    });

    expect(onChange).toHaveBeenNthCalledWith(1, {
      newProjectDefaults: {
        loudnessLufs: -16,
      },
    });
    expect(onChange).toHaveBeenNthCalledWith(2, {
      newProjectDefaults: {
        captions: "mux",
      },
    });
  });
});
