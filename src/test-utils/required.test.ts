import { describe, expect, it } from "vitest";
import { requiredAt, requiredValue } from "./required";

describe("requiredAt", () => {
  it("returns an existing fixture value", () => {
    expect(requiredAt(["first"], 0, "fixture")).toBe("first");
  });

  it("fails with fixture context when the expected value is absent", () => {
    expect(() => requiredAt([], 0, "fixture")).toThrow(
      "Expected fixture at index 0.",
    );
  });

  it("requires keyed fixture values", () => {
    expect(requiredValue("present", "keyed fixture")).toBe("present");
    expect(() => requiredValue(undefined, "keyed fixture")).toThrow(
      "Expected keyed fixture.",
    );
  });
});
