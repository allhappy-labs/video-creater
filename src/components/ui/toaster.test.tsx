import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Toaster } from "./toaster";

describe("Toaster", () => {
  afterEach(() => vi.useRealTimers());

  it("renders each toast with its optional action and dismisses on close", () => {
    const onSelect = vi.fn();
    const onDismiss = vi.fn();
    render(
      <Toaster
        toasts={[
          { id: "toast-1", title: "Exported Edison intro" },
          { id: "toast-2", title: "Removed 3 silences", description: "Timeline 1", action: { label: "Undo", onSelect } },
        ]}
        onDismiss={onDismiss}
      />,
    );

    expect(screen.getByText("Exported Edison intro")).toBeInTheDocument();
    expect(screen.getByText("Timeline 1")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Undo" })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(onSelect).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getAllByRole("button", { name: "Dismiss" })[0] as HTMLElement);
    expect(onDismiss).toHaveBeenCalledWith("toast-1");
  });

  it("auto-dismisses each toast after 6 seconds", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(<Toaster toasts={[{ id: "toast-1", title: "Exported Edison intro" }]} onDismiss={onDismiss} />);

    act(() => void vi.advanceTimersByTime(5_900));
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => void vi.advanceTimersByTime(200));
    expect(onDismiss).toHaveBeenCalledWith("toast-1");
  });

  it("pauses the dismiss timer while a toast has focus", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(<Toaster toasts={[{ id: "toast-1", title: "Exported Edison intro" }]} onDismiss={onDismiss} />);

    act(() => screen.getByRole("button", { name: "Dismiss" }).focus());
    act(() => void vi.advanceTimersByTime(10_000));
    expect(onDismiss).not.toHaveBeenCalled();

    act(() => screen.getByRole("button", { name: "Dismiss" }).blur());
    act(() => void vi.advanceTimersByTime(6_100));
    expect(onDismiss).toHaveBeenCalledWith("toast-1");
  });
});
