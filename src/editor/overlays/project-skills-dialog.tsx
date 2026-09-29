import { ArrowUpRight } from "lucide-react";
import type { RefObject } from "react";
import { projectSkills } from "@/lib/agent/project-skills";
import { OverlayDialog } from "./overlay-dialog";

interface ProjectSkillsDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  /** Project settings verifies and repairs the bundled skills under "Project guidance". */
  readonly onOpenProjectSettings: () => void;
  readonly returnFocusRef?: RefObject<HTMLElement | null>;
}

/** The required project skills that Codex and external agents follow when they propose edits. */
export function ProjectSkillsDialog({ open, onOpenChange, onOpenProjectSettings, returnFocusRef }: ProjectSkillsDialogProps) {
  return (
    <OverlayDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Project skills"
      description="The AI agent and connected agents follow these skills from the project's .agents/skills folder."
      {...(returnFocusRef ? { returnFocusRef } : {})}
      footer={
        <button
          type="button"
          onClick={onOpenProjectSettings}
          className="flex h-8 items-center gap-1.5 rounded-control px-3 text-[13px] font-medium text-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Verify in Project settings
          <ArrowUpRight className="h-3.5 w-3.5" aria-hidden />
        </button>
      }
    >
      <ul className="mt-3 min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        {projectSkills.map((skill) => (
          <li key={skill.name} className="border-t border-line py-2.5 first:border-t-0">
            <h3 className="text-[13px] font-medium">{skill.label}</h3>
            <p className="mt-0.5 text-[12px] text-muted-foreground">{skill.description}</p>
            <p className="mt-1 truncate font-mono text-[11px] text-dim" title={skill.path}>
              {skill.path}
            </p>
          </li>
        ))}
      </ul>
    </OverlayDialog>
  );
}
