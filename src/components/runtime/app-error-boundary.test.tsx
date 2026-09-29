import "@testing-library/jest-dom/vitest";
import { useState, type ReactNode } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";

import { AppErrorBoundary } from "./app-error-boundary";

function ThrowingEditor(): ReactNode {
  throw new Error("editor render failed");
}

afterEach(() => {
  vi.restoreAllMocks();
});

it("runs app reload recovery after a child render failure", () => {
  vi.spyOn(console, "error").mockImplementation(() => undefined);
  const reloadApp = vi.fn();

  function Harness() {
    const [failed, setFailed] = useState(true);
    return (
      <AppErrorBoundary
        onRecover={() => {
          reloadApp();
          setFailed(false);
        }}
      >
        {failed ? <ThrowingEditor /> : <main>App restarted</main>}
      </AppErrorBoundary>
    );
  }

  render(<Harness />);
  expect(screen.getByRole("alert")).toHaveTextContent("The editor stopped rendering");

  fireEvent.click(screen.getByRole("button", { name: "Reload app" }));

  expect(reloadApp).toHaveBeenCalledOnce();
  expect(screen.getByText("App restarted")).toBeVisible();
});
