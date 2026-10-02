import { expect, it } from "vitest";
import { BoundedRequestCache } from "./bounded-request-cache";

it("bounds entries and report bytes across repeated projects and widths", async () => {
  const cache = new BoundedRequestCache<string>(8, 64, (value) => value.length * 2);
  for (let index = 0; index < 1000; index += 1) {
    await cache.get(`project-${index}|width-${index}`, async () => "a".repeat(8));
    expect(cache.size).toBeLessThanOrEqual(8);
    expect(cache.retainedBytes).toBeLessThanOrEqual(64);
  }
  expect(cache.size).toBe(4);
});

it("coalesces pending requests and admits no extra work when all slots are running", async () => {
  const cache = new BoundedRequestCache<string>(1, 64, (value) => value.length);
  let finish!: (value: string) => void;
  let calls = 0;
  const first = cache.get("active", () => { calls += 1; return new Promise<string>((resolve) => { finish = resolve; }); });
  expect(cache.get("active", async () => { throw new Error("must coalesce"); })).toBe(first);
  await expect(cache.get("next", async () => { throw new Error("must not start"); })).resolves.toBeNull();
  await Promise.resolve();
  expect(calls).toBe(1);
  finish("done");
  await first;
  await expect(cache.get("next", async () => "new")).resolves.toBe("new");
});

it("forgets failed requests and reports larger than the entire byte budget", async () => {
  const cache = new BoundedRequestCache<string>(2, 8, (value) => value.length);
  await expect(cache.get("failed", async () => { throw new Error("decode failure"); })).resolves.toBeNull();
  expect(cache.size).toBe(0);
  await cache.get("large", async () => "a".repeat(100));
  expect(cache.size).toBe(0);
  expect(cache.retainedBytes).toBe(0);
});
