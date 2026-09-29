/** A bundled skill that Codex and external agents load from the project's `.agents/skills` folder. */
interface ProjectSkill {
  /** The skill id, which is also its folder name. */
  readonly name: string;
  /** The plain-words name Project settings verifies it under. */
  readonly label: string;
  readonly path: string;
  readonly description: string;
}

/** The three skills every project requires; Project settings verifies and repairs them. */
export const projectSkills: readonly ProjectSkill[] = [
  {
    name: "video-creater-video-pipeline",
    label: "Editing pipeline",
    path: ".agents/skills/video-creater-video-pipeline/SKILL.md",
    description: "EDL rough cuts, transcript-to-timeline planning, renders, and draft MP4 review.",
  },
  {
    name: "video-creater-graphics",
    label: "Video graphics",
    path: ".agents/skills/video-creater-graphics/SKILL.md",
    description: "HyperFrames scenes, title cards, overlays, lower thirds, captions, and callouts.",
  },
  {
    name: "video-creater-visuals",
    label: "Editor interface",
    path: ".agents/skills/video-creater-visuals/SKILL.md",
    description: "Editor chrome, preview surfaces, timeline controls, panels, and visual QA.",
  },
];
