import type { ShaderBackgroundTemplateDefinition } from "@/lib/shader-background-templates";
import { writeAssetDragData } from "../../timeline/drag-data";
import { TileAddButton } from "./tile-add-button";

interface BackgroundTileProps {
  readonly template: ShaderBackgroundTemplateDefinition;
  onInsert(template: ShaderBackgroundTemplateDefinition): void;
}

function categoryLabel(category: string): string {
  return category.charAt(0).toLocaleUpperCase() + category.slice(1);
}

/** A shader background: its CSS preview colors, name, category and `+`; drag it onto the timeline. */
export function BackgroundTile({ template, onInsert }: BackgroundTileProps) {
  const { accentColor, secondaryColor, description } = template.preview;
  return (
    <li
      draggable
      onDragStart={(event) => writeAssetDragData(event.dataTransfer, { kind: "background", id: template.id })}
      className="group relative flex min-w-0 cursor-grab flex-col gap-1.5 rounded-control bg-raised p-1.5 transition-colors hover:bg-hover active:cursor-grabbing motion-reduce:transition-none"
    >
      {/* The template's own preview colors are content, not chrome. */}
      <div
        aria-hidden
        title={description}
        className="aspect-video rounded-md"
        style={{ backgroundImage: `radial-gradient(circle at 30% 30%, ${accentColor}, transparent 70%), linear-gradient(135deg, ${secondaryColor}, ${accentColor})` }}
      />
      <div className="min-w-0 px-0.5">
        <p className="truncate text-[12px] font-medium text-foreground" title={template.name}>
          {template.name}
        </p>
        <p className="truncate text-[11px] text-dim">{categoryLabel(template.category)}</p>
      </div>
      <TileAddButton label={`Add ${template.name} to the timeline`} reason={null} onClick={() => onInsert(template)} />
    </li>
  );
}
