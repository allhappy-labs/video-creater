/**
 * jsdom has no ResizeObserver. Radix primitives (the slider thumb) observe their size on mount,
 * so install an inert stub; components still read their initial size synchronously.
 */
if (typeof globalThis.ResizeObserver === "undefined") {
  class ResizeObserverStub {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
}
