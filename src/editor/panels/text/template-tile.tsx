import { Plus } from "lucide-react";
import { useState } from "react";
import type { MotionTemplateDefinition } from "@/lib/motion-templates";
import { cn } from "@/lib/utils";
import { writeAssetDragData } from "../../timeline/drag-data";
import { TemplatePreview } from "./template-preview";

interface TemplateTileProps {
  readonly template: MotionTemplateDefinition;
  /** False on touch-first devices (`(hover: none)`): no hover preview, `+` always visible. */
  readonly canHover: boolean;
  /** The tap (or keyboard) preview is on for this tile. */
  readonly previewing: boolean;
  onTogglePreview(templateId: string): void;
  onInsert(templateId: string): void;
}

/** A motion template tile: animated on hover or tap, inserted with `+` or by dragging onto the timeline. */
export function TemplateTile({ template, canHover, previewing, onTogglePreview, onInsert }: TemplateTileProps) {
  const [hovered, setHovered] = useState(false);

  return (
    <li
      draggable
      data-template-id={template.id}
      className="group relative min-w-0"
      onDragStart={(event) => writeAssetDragData(event.dataTransfer, { kind: "template", id: template.id })}
      onPointerEnter={(event) => {
        if (canHover && event.pointerType !== "touch") setHovered(true);
      }}
      onPointerLeave={() => setHovered(false)}
    >
      <button
        type="button"
        aria-label={`Preview ${template.name}`}
        aria-pressed={previewing}
        onClick={() => onTogglePreview(template.id)}
        className={cn(
          "flex w-full min-w-0 flex-col gap-1.5 rounded-control bg-raised p-1.5 text-left transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
          previewing && "ring-2 ring-primary",
        )}
      >
        <TemplatePreview template={template} playing={previewing || hovered} />
        <span className="truncate px-0.5 text-[12px] text-foreground">{template.name}</span>
      </button>
      <button
        type="button"
        aria-label={`Add ${template.name}`}
        onClick={() => onInsert(template.id)}
        className={cn(
          "absolute right-2.5 top-2.5 flex h-7 w-7 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg transition-opacity hover:bg-primary/90 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring group-focus-within:opacity-100 group-hover:opacity-100 motion-reduce:transition-none",
          canHover ? "opacity-0" : "opacity-100",
        )}
      >
        <Plus className="h-4 w-4" aria-hidden />
      </button>
    </li>
  );
}
