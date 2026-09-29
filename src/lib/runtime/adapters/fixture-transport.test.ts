import { describe, expect, it, vi } from "vitest";

import { FixtureOperationUnsupportedError } from "../backend-transport";
import {
  FixtureTransport,
  type FixtureOperationHandler,
} from "./fixture-transport";

function fixtureTransport(
  handlers: Record<string, FixtureOperationHandler>,
): FixtureTransport {
  return new FixtureTransport(new Map(Object.entries(handlers)));
}

describe("FixtureTransport", () => {
  it("isolates fixture handlers from caller input and output mutation", async () => {
    const input = { nested: { value: "original" } };
    const fixtureResult = { nested: { value: "fixture" } };
    const transport = fixtureTransport({
      load_project: (received) => {
        (received.nested as { value: string }).value = "changed in handler";
        return fixtureResult;
      },
    });

    const result = await transport.request<typeof fixtureResult>(
      "load_project",
      input,
    );
    result.nested.value = "changed by caller";

    expect(input).toEqual({ nested: { value: "original" } });
    expect(fixtureResult).toEqual({ nested: { value: "fixture" } });
  });

  it("names unsupported fixture operations with a typed error", async () => {
    const transport = fixtureTransport({});

    const failure = await transport.request("missing_operation").catch(
      (error: unknown) => error,
    );

    expect(failure).toBeInstanceOf(FixtureOperationUnsupportedError);
    expect(failure).toMatchObject({
      code: "fixture_operation_unsupported",
      operation: "missing_operation",
    });
  });

  it("delivers fixture events until the selected listener is removed", async () => {
    const transport = fixtureTransport({});
    const first = vi.fn();
    const second = vi.fn();
    const unlistenFirst = await transport.listen("project-changed", first);
    await transport.listen("project-changed", second);

    transport.emitForVisualQa("project-changed", { revision: 1 });
    unlistenFirst();
    transport.emitForVisualQa("project-changed", { revision: 2 });

    expect(first).toHaveBeenCalledTimes(1);
    expect(first).toHaveBeenCalledWith({ revision: 1 });
    expect(second).toHaveBeenNthCalledWith(1, { revision: 1 });
    expect(second).toHaveBeenNthCalledWith(2, { revision: 2 });
  });

  it("creates deterministic fixture URLs without a local file URL", () => {
    const transport = fixtureTransport({});

    expect(transport.mediaUrl("/project/source clip.mp4")).toBe(
      "/__editor-fixture-media/project/source%20clip.mp4",
    );
    expect(transport.mediaUrl("/project/source clip.mp4")).not.toContain(
      "file://",
    );
  });
});
