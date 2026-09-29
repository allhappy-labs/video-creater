import { describe, expect, it, vi } from "vitest";
import { createTimelineFrameCoalescer } from "./timeline-interaction-frame";

describe("createTimelineFrameCoalescer", () => {
  it("evaluates only the latest scheduled value in an animation frame", () => {
    const callbacks: FrameRequestCallback[] = [];
    const evaluate = vi.fn((value: number) => value * 2);
    const deliver = vi.fn();
    const requestFrame = vi.fn((callback: FrameRequestCallback) => {
      callbacks.push(callback);
      return callbacks.length;
    });
    const cancelFrame = vi.fn();
    const coalescer = createTimelineFrameCoalescer({
      evaluate,
      deliver,
      requestFrame,
      cancelFrame,
    });

    coalescer.schedule(1);
    coalescer.schedule(2);
    coalescer.schedule(3);
    expect(requestFrame).toHaveBeenCalledTimes(1);

    callbacks[0]?.(0);

    expect(evaluate).toHaveBeenCalledTimes(1);
    expect(evaluate).toHaveBeenCalledWith(3);
    expect(deliver).toHaveBeenCalledWith(6);
  });

  it("flushes the latest value synchronously and cancels pending delivery", () => {
    const callbacks: FrameRequestCallback[] = [];
    const evaluate = vi.fn((value: number) => value * 2);
    const deliver = vi.fn();
    const cancelFrame = vi.fn();
    const coalescer = createTimelineFrameCoalescer({
      evaluate,
      deliver,
      requestFrame(callback) {
        callbacks.push(callback);
        return callbacks.length;
      },
      cancelFrame,
    });

    coalescer.schedule(4);
    expect(coalescer.flush(5)).toBe(10);
    expect(cancelFrame).toHaveBeenCalledWith(1);
    expect(deliver).toHaveBeenLastCalledWith(10);

    coalescer.schedule(6);
    const cancelledCallback = callbacks[1];
    coalescer.cancel();
    cancelledCallback?.(0);

    expect(cancelFrame).toHaveBeenLastCalledWith(2);
    expect(evaluate).toHaveBeenCalledTimes(1);
    expect(deliver).toHaveBeenCalledTimes(1);
  });
});
