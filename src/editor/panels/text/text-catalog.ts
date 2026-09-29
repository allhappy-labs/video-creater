import type { MotionTemplateDefinition } from "@/lib/motion-templates";
import { formatCategory } from "@/lib/templates/template-item";

type TemplateCategory = MotionTemplateDefinition["category"];

interface TitleTemplateGroup {
  readonly category: TemplateCategory;
  readonly label: string;
  readonly templates: readonly MotionTemplateDefinition[];
}

/** Categories whose look is the type itself; the Text styles grid shows them. */
const textStyleCategories: ReadonlySet<TemplateCategory> = new Set(["text", "captions"]);
/** Group order in "Titles & lower thirds"; unlisted categories follow in catalog order. */
const titleCategoryOrder: readonly TemplateCategory[] = ["titles", "lower_thirds", "callouts"];

/** Only overlay templates on overlay tracks can be inserted (`createTemplateOverlayItem`). */
function placeable(template: MotionTemplateDefinition): boolean {
  return template.kind === "overlay" && template.placement.trackKind === "overlay";
}

export function textStyleTemplates(catalog: readonly MotionTemplateDefinition[]): MotionTemplateDefinition[] {
  return catalog.filter((template) => placeable(template) && textStyleCategories.has(template.category));
}

/** The remaining placeable templates, grouped by category. */
export function titleTemplateGroups(catalog: readonly MotionTemplateDefinition[]): TitleTemplateGroup[] {
  const titles = catalog.filter((template) => placeable(template) && !textStyleCategories.has(template.category));
  const categories = [...new Set(titles.map((template) => template.category))].sort((left, right) => rank(left) - rank(right));
  return categories.map((category) => ({
    category,
    label: formatCategory(category),
    templates: titles.filter((template) => template.category === category),
  }));
}

function rank(category: TemplateCategory): number {
  const index = titleCategoryOrder.indexOf(category);
  return index < 0 ? titleCategoryOrder.length : index;
}
