import type { ReactNode } from "react";

/**
 * The "Preview viewport" region with the canvas letterboxed to the aspect ratio. The canvas is the
 * positioning context for every layer and a container for `cqw` text sizes. `chrome` renders over
 * the viewport, outside the canvas. The region takes focus on click so preview keys keep working.
 */
export function PreviewViewport({
  width,
  height,
  chrome,
  children,
}: {
  readonly width: number;
  readonly height: number;
  readonly chrome?: ReactNode;
  readonly children: ReactNode;
}) {
  const ratio = width > 0 && height > 0 ? width / height : 16 / 9;
  return (
    <div
      role="region"
      aria-label="Preview viewport"
      tabIndex={-1}
      className="relative flex min-h-0 flex-1 items-center justify-center p-2 outline-none [container-type:size]"
    >
      <div
        data-testid="preview-canvas"
        className="relative overflow-hidden rounded-sm bg-background [container-type:inline-size]"
        style={{ aspectRatio: `${ratio}`, width: `min(100cqw, calc(100cqh * ${ratio}))` }}
      >
        {children}
      </div>
      {chrome}
    </div>
  );
}
