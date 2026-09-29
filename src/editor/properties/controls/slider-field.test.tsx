import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { installPointerEventPolyfill, stubRect } from "@/test-utils/editor-render";
import { SliderField, type SliderFieldProps } from "./slider-field";

const percent = (value: number) => `${Math.round(value).toString()}%`;

function renderField(props: Partial<SliderFieldProps> = {}) {
  const onPreview = vi.fn();
  const onCommit = vi.fn();
  const view = render(
    <TooltipProvider>
      <SliderField label="Opacity" value={50} min={0} max={100} step={1} format={percent} onPreview={onPreview} onCommit={onCommit} {...props} />
    </TooltipProvider>,
  );
  return { ...view, onPreview, onCommit };
}

function sliderRoot(): HTMLElement {
  const root = screen.getByRole("slider", { name: "Opacity" }).closest<HTMLElement>("[data-orientation]:not([role])");
  if (!root) throw new Error("slider root not found");
  return root;
}

describe("SliderField", () => {
  beforeAll(() => {
    installPointerEventPolyfill();
    Element.prototype.hasPointerCapture = () => true;
    Element.prototype.setPointerCapture = () => undefined;
    Element.prototype.releasePointerCapture = () => undefined;
  });

  it("previews while dragging and commits exactly once on release", () => {
    const { onPreview, onCommit } = renderField();
    const root = sliderRoot();
    stubRect(root, { width: 100, height: 20 });

    fireEvent.pointerDown(root, { clientX: 20, pointerId: 1, button: 0 });
    fireEvent.pointerMove(root, { clientX: 30, pointerId: 1 });
    fireEvent.pointerMove(root, { clientX: 40, pointerId: 1 });

    expect(onPreview.mock.calls).toEqual([[20], [30], [40]]);
    expect(onCommit).not.toHaveBeenCalled();
    expect(screen.getByRole("slider", { name: "Opacity" })).toHaveAttribute("aria-valuetext", "40%");
    expect(screen.getByRole("textbox", { name: "Opacity" })).toHaveValue("40%");

    fireEvent.pointerUp(root, { clientX: 40, pointerId: 1 });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith(40);
  });

  it("commits each keyboard step once without a preview", () => {
    const { onPreview, onCommit } = renderField();
    const slider = screen.getByRole("slider", { name: "Opacity" });
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    fireEvent.keyUp(slider, { key: "ArrowRight" });
    expect(onPreview).not.toHaveBeenCalled();
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith(51);
  });

  it("keeps the dragged value until an async commit settles", async () => {
    let resolve: () => void = () => undefined;
    const pending = new Promise<void>((done) => {
      resolve = done;
    });
    renderField({ onCommit: () => pending });
    const slider = screen.getByRole("slider", { name: "Opacity" });
    fireEvent.keyDown(slider, { key: "End" });
    expect(slider).toHaveAttribute("aria-valuetext", "100%");
    await act(async () => {
      resolve();
      await pending;
    });
    expect(slider).toHaveAttribute("aria-valuetext", "50%");
  });

  it("commits the numeric field once on Enter and not again on blur", () => {
    const { onCommit } = renderField();
    const input = screen.getByRole("textbox", { name: "Opacity" });
    fireEvent.change(input, { target: { value: "75" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith(75);
  });

  it("commits the numeric field on blur and parses unit suffixes", () => {
    const { onCommit } = renderField();
    const input = screen.getByRole("textbox", { name: "Opacity" });
    fireEvent.change(input, { target: { value: "25%" } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledWith(25);
  });

  it("rejects invalid numeric input inline without committing", () => {
    const { onCommit } = renderField();
    const input = screen.getByRole("textbox", { name: "Opacity" });

    fireEvent.change(input, { target: { value: "abc" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Enter a number.");

    fireEvent.change(input, { target: { value: "150" } });
    fireEvent.blur(input);
    expect(input).toHaveAccessibleDescription("Enter a value from 0% to 100%.");
    expect(onCommit).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Escape" });
    expect(input).toHaveAttribute("aria-invalid", "false");
    expect(input).toHaveValue("50%");
    expect(screen.queryByText("Enter a value from 0% to 100%.")).not.toBeInTheDocument();
  });

  it("does not commit an unchanged numeric value", () => {
    const { onCommit } = renderField();
    const input = screen.getByRole("textbox", { name: "Opacity" });
    fireEvent.change(input, { target: { value: "50" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("shows the keyframe toggle state and forwards clicks", () => {
    const onToggle = vi.fn();
    function Harness() {
      const [active, setActive] = useState(false);
      return (
        <SliderField
          label="Opacity"
          value={50}
          min={0}
          max={100}
          format={percent}
          onCommit={() => undefined}
          keyframe={{
            active,
            onToggle: () => {
              onToggle();
              setActive((current) => !current);
            },
          }}
        />
      );
    }
    render(
      <TooltipProvider>
        <Harness />
      </TooltipProvider>,
    );
    const add = screen.getByRole("button", { name: "Add keyframe for Opacity" });
    expect(add).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(add);
    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Remove keyframe for Opacity" })).toHaveAttribute("aria-pressed", "true");
  });

  it("shows Mixed until the value changes", () => {
    const { onCommit } = renderField({ mixed: true });
    const slider = screen.getByRole("slider", { name: "Opacity" });
    const input = screen.getByRole("textbox", { name: "Opacity" });
    expect(input).toHaveValue("Mixed");
    expect(slider).toHaveAttribute("aria-valuetext", "Mixed");
    fireEvent.blur(input);
    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "30" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith(30);
  });

  it("commits a mixed value even when it equals the shown value", () => {
    const { onCommit } = renderField({ mixed: true });
    const input = screen.getByRole("textbox", { name: "Opacity" });
    fireEvent.change(input, { target: { value: "50" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onCommit).toHaveBeenCalledWith(50);
  });

  it("clears through onBlank when the numeric field is emptied", () => {
    const onBlank = vi.fn();
    const { onCommit } = renderField({ onBlank });
    const input = screen.getByRole("textbox", { name: "Opacity" });
    fireEvent.change(input, { target: { value: "  " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onBlank).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
    expect(input).toHaveAttribute("aria-invalid", "false");
  });
});
