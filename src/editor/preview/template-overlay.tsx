import { useId, type CSSProperties } from "react";
import { builtInVPhotoLogoPaths, builtInVPhotoLogoViewBox } from "@/lib/builtin-logo-paths";
import { getMotionTemplate } from "@/lib/motion-templates";
import { getTemplateFields, gradientLoopPanelStyles, splitPreviewLines } from "@/lib/templates/template-item";
import { isTemplateTimelineItem, type TimelineItem } from "@/lib/timeline";

/**
 * A DOM approximation of a motion template at the playhead, one look per template preview variant.
 * Text sizes use container query units relative to the canvas.
 */
export function TemplateOverlay({ item, style }: { readonly item: TimelineItem; readonly style: CSSProperties }) {
  // useId output contains characters that break `url(#…)` references.
  const gradientId = `template-logo-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  if (!isTemplateTimelineItem(item)) return null;
  const template = getMotionTemplate(String(item.properties.templateId));
  if (!template) return null;
  const fields = { ...template.defaultTextFields, ...getTemplateFields(item) };
  const { headline, subline } = fields;
  const variant = template.preview.cssVariant;

  return (
    <div aria-label={`${template.name} preview`} className="pointer-events-none absolute inset-0 z-20 text-foreground" style={style}>
      {variant === "lower-third" && (
        <div className="absolute bottom-[14%] left-[9%] max-w-[46%] rounded-sm border-l-4 border-accent bg-background/85 px-[1.2cqw] py-[0.9cqw] shadow-2xl">
          <span className="block text-[length:2.6cqw] font-semibold leading-tight">{headline}</span>
          <span className="mt-[0.3cqw] block text-[length:1.7cqw] text-accent">{subline}</span>
        </div>
      )}
      {variant === "punchy-caption" && (
        <div className="absolute inset-x-[18%] bottom-[16%] text-center">
          <span className="block text-[length:4.6cqw] font-black uppercase leading-none">{headline}</span>
          <span className="mx-auto mt-[1cqw] block h-[0.5cqw] w-[20%] rounded-full bg-success" />
          {subline && <span className="mt-[0.6cqw] block text-[length:1.7cqw]">{subline}</span>}
        </div>
      )}
      {variant === "metric-callout" && (
        <div className="absolute right-[10%] top-[14%] rounded-md border border-success/70 bg-success/15 px-[1.6cqw] py-[1.2cqw] shadow-2xl">
          <span className="block text-[length:5.4cqw] font-bold leading-none">{headline}</span>
          <span className="mt-[0.6cqw] block text-[length:1.7cqw] text-success">{subline}</span>
        </div>
      )}
      {variant === "chapter-card" && (
        <div className="absolute inset-y-0 left-0 flex w-[42%] items-center gap-[1.2cqw] bg-foreground/10 px-[8%] backdrop-blur-sm">
          <span className="h-[30%] w-[0.5cqw] shrink-0 rounded-full bg-warning" />
          <span>
            <span className="block text-[length:3.4cqw] font-bold">{headline}</span>
            <span className="mt-[0.6cqw] block text-[length:1.7cqw] text-warning">{subline}</span>
          </span>
        </div>
      )}
      {variant === "tracking-highlight" && (
        <div className="absolute left-[18%] top-[24%]">
          <span className="block h-[13cqw] w-[20cqw] rounded-full border-[0.5cqw] border-accent" />
          <span className="mt-[0.9cqw] block w-fit rounded-sm bg-background/85 px-[0.9cqw] py-[0.3cqw] text-[length:1.7cqw]">{headline}</span>
          {subline && <span className="mt-[0.3cqw] block text-[length:1.4cqw] text-accent">{subline}</span>}
        </div>
      )}
      {variant === "holographic-logo" && (
        <div className="absolute inset-0 overflow-hidden bg-background">
          <span className="absolute left-[24%] top-[30%] h-[32%] w-[38%] rounded-full bg-accent/10 blur-2xl" />
          <span className="absolute right-[22%] top-[42%] h-[24%] w-[26%] rounded-full bg-accent-soft blur-2xl" />
          <svg className="absolute left-1/2 top-1/2 w-[66%] -translate-x-1/2 -translate-y-1/2" viewBox={builtInVPhotoLogoViewBox} aria-hidden>
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%" stopColor="hsl(var(--foreground))" />
                <stop offset="40%" stopColor="hsl(var(--success))" />
                <stop offset="70%" stopColor="hsl(var(--accent))" />
                <stop offset="100%" stopColor="hsl(var(--clip-text))" />
              </linearGradient>
            </defs>
            {builtInVPhotoLogoPaths.map((path) => (
              <path key={path} d={path} fill={`url(#${gradientId})`} />
            ))}
          </svg>
        </div>
      )}
      {variant === "gradient-background-loop" && (
        <div className="absolute inset-0 overflow-hidden bg-background">
          <div className="absolute inset-0 flex">
            {gradientLoopPanelStyles.map((backgroundImage, index) => (
              <span key={backgroundImage} className="h-full flex-1" style={{ backgroundImage, transform: `translateY(${index % 2 === 0 ? "-2%" : "2%"})` }} />
            ))}
          </div>
          <div className="absolute inset-x-[10%] top-1/2 -translate-y-1/2 text-center">
            {splitPreviewLines(headline).map((line, index) => (
              <span key={`${line}-${index}`} className="block break-words text-[length:12cqw] font-black leading-[0.72] [overflow-wrap:anywhere]">
                {line}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
