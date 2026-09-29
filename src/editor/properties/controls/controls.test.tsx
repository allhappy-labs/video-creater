import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { ColorSwatches } from "./color-swatches";
import { KeyframeButton } from "./keyframe-button";
import { PresetGrid } from "./preset-grid";
import { PropertySection } from "./property-section";
import { SegmentedField } from "./segmented-field";
import { SelectField } from "./select-field";
import { SwitchField } from "./switch-field";

function renderUi(ui: ReactElement) {
  return render(<TooltipProvider>{ui}</TooltipProvider>);
}

const presets = [
  { value: "none", label: "None" },
  { value: "film", label: "Film" },
  { value: "warm", label: "Warm" },
  { value: "mono", label: "B&W" },
  { value: "cool", label: "Cool" },
] as const;

describe("KeyframeButton", () => {
  it("names and presses the toggle from the keyframe state", () => {
    const onToggle = vi.fn();
    const { rerender } = renderUi(<KeyframeButton label="Scale" active={false} onToggle={onToggle} />);
    const add = screen.getByRole("button", { name: "Add keyframe for Scale" });
    expect(add).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(add);
    expect(onToggle).toHaveBeenCalledTimes(1);

    rerender(
      <TooltipProvider>
        <KeyframeButton label="Scale" active keyframed onToggle={onToggle} />
      </TooltipProvider>,
    );
    const remove = screen.getByRole("button", { name: "Remove keyframe for Scale" });
    expect(remove).toHaveAttribute("aria-pressed", "true");
    expect(remove).toHaveClass("text-keyframe");
  });
});

describe("PresetGrid", () => {
  it("presses the selected preset and changes on click", () => {
    const onChange = vi.fn();
    renderUi(<PresetGrid label="Look" value="film" options={presets} onChange={onChange} />);
    const grid = screen.getByRole("group", { name: "Look" });
    expect(within(grid).getByRole("button", { name: "Film" })).toHaveAttribute("aria-pressed", "true");
    expect(within(grid).getByRole("button", { name: "Warm" })).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(within(grid).getByRole("button", { name: "Film" }));
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(within(grid).getByRole("button", { name: "Warm" }));
    expect(onChange).toHaveBeenCalledWith("warm");
  });

  it("keeps one tab stop and moves focus with arrow keys by item and row", () => {
    renderUi(<PresetGrid label="Look" value="film" options={presets} onChange={() => undefined} />);
    const button = (name: string) => screen.getByRole("button", { name });
    expect(button("Film")).toHaveAttribute("tabindex", "0");
    expect(button("None")).toHaveAttribute("tabindex", "-1");

    act(() => button("Film").focus());
    fireEvent.keyDown(button("Film"), { key: "ArrowRight" });
    expect(button("Warm")).toHaveFocus();
    expect(button("Warm")).toHaveAttribute("tabindex", "0");
    fireEvent.keyDown(button("Warm"), { key: "ArrowDown" });
    expect(button("Warm")).toHaveFocus();
    fireEvent.keyDown(button("Warm"), { key: "ArrowLeft" });
    fireEvent.keyDown(button("Film"), { key: "ArrowDown" });
    expect(button("Cool")).toHaveFocus();
    fireEvent.keyDown(button("Cool"), { key: "ArrowUp" });
    expect(button("Film")).toHaveFocus();
    fireEvent.keyDown(button("Film"), { key: "End" });
    expect(button("Cool")).toHaveFocus();
    fireEvent.keyDown(button("Cool"), { key: "Home" });
    expect(button("None")).toHaveFocus();
  });

  it("uses the label as the name when a tile draws a preview", () => {
    renderUi(
      <PresetGrid
        label="Caption preset"
        value={null}
        options={[{ value: "bold", label: "Bold", preview: <span aria-hidden>Aab</span> }]}
        onChange={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "Bold" })).toHaveAttribute("tabindex", "0");
  });
});

describe("ColorSwatches", () => {
  const colors = [
    { value: "#FFD84A", label: "Yellow" },
    { value: "#1fc7d4", label: "Cyan" },
  ];

  it("presses the matching swatch case-insensitively and changes on click", () => {
    const onChange = vi.fn();
    renderUi(<ColorSwatches label="Highlight" value="#ffd84a" options={colors} onChange={onChange} />);
    const group = screen.getByRole("group", { name: "Highlight" });
    expect(within(group).getByRole("button", { name: "Yellow" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(group).getByRole("button", { name: "Cyan" }));
    expect(onChange).toHaveBeenCalledWith("#1fc7d4");
    fireEvent.keyDown(within(group).getByRole("button", { name: "Yellow" }), { key: "ArrowRight" });
    expect(within(group).getByRole("button", { name: "Cyan" })).toHaveFocus();
  });
});

describe("SegmentedField", () => {
  it("labels the group and never clears the active segment", () => {
    const onChange = vi.fn();
    renderUi(
      <SegmentedField
        label="Applies to"
        labelStyle="heading"
        value="all"
        options={[
          { value: "all", label: "All captions" },
          { value: "this", label: "Only this" },
        ]}
        onChange={onChange}
      />,
    );
    const group = screen.getByRole("radiogroup", { name: "Applies to" });
    expect(within(group).getByRole("radio", { name: "All captions" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(within(group).getByRole("radio", { name: "All captions" }));
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(within(group).getByRole("radio", { name: "Only this" }));
    expect(onChange).toHaveBeenCalledWith("this");
  });
});

describe("SelectField", () => {
  beforeAll(() => {
    installPointerEventPolyfill();
    Element.prototype.hasPointerCapture = () => false;
    Element.prototype.releasePointerCapture = () => undefined;
    Element.prototype.scrollIntoView = () => undefined;
  });

  it("labels the trigger, shows the value and reports a new choice", () => {
    const onChange = vi.fn();
    renderUi(
      <SelectField
        label="Mode"
        value="over"
        options={[
          { value: "over", label: "Normal" },
          { value: "screen", label: "Screen" },
        ]}
        onChange={onChange}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Mode" });
    expect(trigger).toHaveTextContent("Normal");
    fireEvent.keyDown(trigger, { key: "Enter" });
    fireEvent.click(screen.getByRole("option", { name: "Screen" }));
    expect(onChange).toHaveBeenCalledWith("screen");
  });
});

describe("SwitchField", () => {
  it("labels the switch and toggles it", () => {
    const onChange = vi.fn();
    renderUi(<SwitchField label="Denoise" description="Ready" checked={false} onChange={onChange} />);
    const control = screen.getByRole("switch", { name: "Denoise" });
    expect(control).toHaveAttribute("aria-checked", "false");
    expect(control).toHaveAccessibleDescription("Ready");
    fireEvent.click(control);
    expect(onChange).toHaveBeenCalledWith(true);
  });
});

describe("PropertySection", () => {
  it("names the section by its title and resets on demand", () => {
    const onReset = vi.fn();
    renderUi(
      <PropertySection title="Transform" onReset={onReset} action={<button type="button">Edit on canvas</button>}>
        <p>Body</p>
      </PropertySection>,
    );
    const section = screen.getByRole("region", { name: "Transform" });
    expect(within(section).getByText("Body")).toBeInTheDocument();
    expect(within(section).getByRole("button", { name: "Edit on canvas" })).toBeInTheDocument();
    fireEvent.click(within(section).getByRole("button", { name: "Reset Transform" }));
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it("omits the reset button without a handler", () => {
    renderUi(<PropertySection title="Crop" />);
    expect(screen.queryByRole("button", { name: "Reset Crop" })).not.toBeInTheDocument();
  });
});
