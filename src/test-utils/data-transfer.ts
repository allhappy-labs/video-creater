/** jsdom has no DataTransfer; this fake keeps the parts drag-and-drop handlers read and write. */
export function fakeDataTransfer(entries: Record<string, string> = {}): DataTransfer {
  const data = new Map(Object.entries(entries));
  return {
    get types() {
      return [...data.keys()];
    },
    getData: (type: string) => data.get(type) ?? "",
    setData: (type: string, value: string) => void data.set(type, value),
    dropEffect: "none",
    effectAllowed: "all",
  } as unknown as DataTransfer;
}

/**
 * jsdom has no DragEvent, so fireEvent.drag* would build a plain Event and drop clientX/clientY.
 * Install once per test file; Testing Library still attaches the `dataTransfer` init value.
 */
export function installDragEventPolyfill(): void {
  if (typeof window.DragEvent !== "undefined") return;
  class DragEventPolyfill extends MouseEvent {}
  window.DragEvent = DragEventPolyfill as unknown as typeof DragEvent;
}
