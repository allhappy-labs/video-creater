import { useEffect, useMemo, useRef, type CSSProperties } from "react";
import { createTemplateOverlayItem, type MotionTemplateDefinition, type MotionTemplatePreviewVariant } from "@/lib/motion-templates";
import { TemplateOverlay } from "../../preview/template-overlay";
import { useMediaQuery } from "./use-media-query";

/** Enter, hold and exit of `transform`/`opacity` over one loop, entering by `from`. */
function enterHoldExit(from: string, settle?: string): Keyframe[] {
  return [
    { offset: 0, opacity: 0, transform: from },
    ...(settle ? [{ offset: 0.14, opacity: 1, transform: settle }] : []),
    { offset: 0.22, opacity: 1, transform: "none" },
    { offset: 0.86, opacity: 1, transform: "none" },
    { offset: 1, opacity: 0, transform: "none" },
  ];
}

/** Loop keyframes approximating each template's motion preset (the canvas preview draws a still). */
const previewKeyframes: Readonly<Record<MotionTemplatePreviewVariant, Keyframe[]>> = {
  "lower-third": enterHoldExit("translateY(12%)"),
  "punchy-caption": enterHoldExit("scale(0.6)", "scale(1.08)"),
  "metric-callout": enterHoldExit("scale(0.8)", "scale(1.05)"),
  "chapter-card": [
    { offset: 0, clipPath: "inset(0% 100% 0% 0%)" },
    { offset: 0.25, clipPath: "inset(0% 0% 0% 0%)" },
    { offset: 0.8, clipPath: "inset(0% 0% 0% 0%)" },
    { offset: 1, clipPath: "inset(0% 0% 0% 100%)" },
  ],
  "tracking-highlight": enterHoldExit("scale(1.15)"),
  "holographic-logo": [
    { offset: 0, filter: "brightness(0.8)", transform: "scale(1)" },
    { offset: 0.5, filter: "brightness(1.25)", transform: "scale(1.03)" },
    { offset: 1, filter: "brightness(0.8)", transform: "scale(1)" },
  ],
  "gradient-background-loop": [
    { offset: 0, transform: "scale(1)" },
    { offset: 0.5, transform: "scale(1.05)" },
    { offset: 1, transform: "scale(1)" },
  ],
};

const stillStyle: CSSProperties = {};

/**
 * A miniature canvas drawing the template with the preview's own renderer, looping its motion
 * while `playing`. Reduced motion keeps the still frame.
 */
export function TemplatePreview({ template, playing }: { readonly template: MotionTemplateDefinition; readonly playing: boolean }) {
  const reducedMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  const motionRef = useRef<HTMLDivElement>(null);
  const item = useMemo(
    () => createTemplateOverlayItem({ templateId: template.id, itemId: `text-panel-preview-${template.id}`, startSeconds: 0 }),
    [template.id],
  );
  const animate = playing && !reducedMotion;

  useEffect(() => {
    const element = motionRef.current;
    if (!animate || !element || typeof element.animate !== "function") return;
    const animation = element.animate(previewKeyframes[template.preview.cssVariant], {
      duration: template.durationSeconds * 1000,
      easing: "ease-out",
      iterations: Infinity,
    });
    return () => animation.cancel();
  }, [animate, template]);

  return (
    <div aria-hidden className="relative isolate aspect-video w-full overflow-hidden rounded-clip bg-background [container-type:inline-size]">
      <div ref={motionRef} className="absolute inset-0">
        <TemplateOverlay item={item} style={stillStyle} />
      </div>
    </div>
  );
}
