import "@testing-library/jest-dom/vitest";
import { act, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { PropertiesRegion } from "./properties-region";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

describe("PropertiesRegion", () => {
  it("renders the properties panel for the selection inside the aside", () => {
    const project = fixtureProject();
    const video = fixtureItem(project, "video");
    const { store } = renderWithEditorStore(<PropertiesRegion overlay={false} />, { project });
    const aside = screen.getByRole("complementary", { name: "Properties" });
    expect(aside).toBeEmptyDOMElement();

    act(() => store.getState().selectItems([video.id]));
    expect(within(aside).getByRole("heading", { level: 2, name: video.label })).toBeInTheDocument();
    expect(within(aside).getByRole("tablist", { name: "Property tabs" })).toBeInTheDocument();
  });

  it("overlays the aside in the narrow desktop range", () => {
    renderWithEditorStore(<PropertiesRegion overlay />, { project: fixtureProject() });
    expect(screen.getByRole("complementary", { name: "Properties" })).toHaveClass("absolute");
  });
});
