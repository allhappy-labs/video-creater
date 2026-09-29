import { render, type RenderResult } from "@testing-library/react";
import type { ReactElement } from "react";
import { TooltipProvider } from "@/components/ui/tooltip";
import { createEditorStore, type EditorStore } from "@/editor/store/editor-store";
import { EditorStoreProvider } from "@/editor/store/editor-store-context";
import type { VideoProject } from "@/lib/project";

/** Renders `ui` inside a fresh editor store and tooltip provider. */
export function renderWithEditorStore(
  ui: ReactElement,
  options: { readonly project: VideoProject; readonly projectDir?: string },
): RenderResult & { readonly store: EditorStore } {
  const store = createEditorStore({ projectDir: options.projectDir ?? "/p", project: options.project });
  const result = render(
    <TooltipProvider>
      <EditorStoreProvider store={store}>{ui}</EditorStoreProvider>
    </TooltipProvider>,
  );
  return { ...result, store };
}

/**
 * jsdom has no PointerEvent, so fireEvent.pointer* would drop clientX/clientY, pointerId and
 * pointerType. Install once per test file (for example in `beforeAll`).
 */
export function installPointerEventPolyfill(): void {
  if (typeof window.PointerEvent !== "undefined") return;
  class PointerEventPolyfill extends MouseEvent {
    readonly pointerId: number;
    readonly pointerType: string;
    constructor(type: string, init: PointerEventInit = {}) {
      super(type, init);
      this.pointerId = init.pointerId ?? 0;
      this.pointerType = init.pointerType ?? "mouse";
    }
  }
  window.PointerEvent = PointerEventPolyfill as unknown as typeof PointerEvent;
}

/** Stubs `getBoundingClientRect` on one element (jsdom has no layout). */
export function stubRect(element: Element, rect: { readonly left?: number; readonly top?: number; readonly width: number; readonly height?: number }): void {
  const left = rect.left ?? 0;
  const top = rect.top ?? 0;
  const height = rect.height ?? 0;
  element.getBoundingClientRect = () => ({
    x: left,
    y: top,
    left,
    top,
    width: rect.width,
    height,
    right: left + rect.width,
    bottom: top + height,
    toJSON: () => ({}),
  });
}
