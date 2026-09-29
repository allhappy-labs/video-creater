import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";

describe("Tabs", () => {
  it("gives the focusable tab panel a token focus ring, like its triggers", () => {
    render(
      <Tabs defaultValue="video">
        <TabsList aria-label="Properties tabs">
          <TabsTrigger value="video">Video</TabsTrigger>
        </TabsList>
        <TabsContent value="video" className="overflow-y-auto">
          Transform
        </TabsContent>
      </Tabs>,
    );

    const panel = screen.getByRole("tabpanel", { name: "Video" });
    expect(panel).toHaveAttribute("tabindex", "0");
    expect(panel).toHaveClass("focus-visible:ring-2", "focus-visible:ring-inset", "focus-visible:ring-ring", "overflow-y-auto");
    expect(screen.getByRole("tab", { name: "Video" })).toHaveClass("focus-visible:ring-2", "focus-visible:ring-ring");
  });
});
