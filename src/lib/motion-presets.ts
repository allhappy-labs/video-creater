export type MotionPresetId =
  | "slide-fade-up-v1"
  | "snap-pop-v1"
  | "underline-wipe-v1"
  | "metric-count-pop-v1"
  | "vertical-reveal-v1"
  | "tracking-draw-v1"
  | "spring-pop-v2"
  | "slide-rotate-settle-v2"
  | "mask-wipe-v2"
  | "line-draw-v2"
  | "word-pop-stagger-v2"
  | "soft-depth-card-v2"
  | "pulse-emphasis-v2"
  | "exit-snap-v2";

export interface MotionPresetDefinition {
  id: MotionPresetId;
  label: string;
  description: string;
}

export const motionPresetCatalog: MotionPresetDefinition[] = [
  {
    id: "slide-fade-up-v1",
    label: "Slide Fade Up",
    description: "Lower-third entry with upward slide, quick fade, hold, and soft exit.",
  },
  {
    id: "snap-pop-v1",
    label: "Snap Pop",
    description: "Caption scale pop with a small overshoot and quick snap fade.",
  },
  {
    id: "underline-wipe-v1",
    label: "Underline Wipe",
    description: "Accent rule reveal paired with text opacity for caption emphasis.",
  },
  {
    id: "metric-count-pop-v1",
    label: "Metric Count Pop",
    description: "Metric tile pop with directional accent sweep and crisp hold.",
  },
  {
    id: "vertical-reveal-v1",
    label: "Vertical Reveal",
    description: "Chapter marker line wipe with text reveal and mask-like exit.",
  },
  {
    id: "tracking-draw-v1",
    label: "Tracking Draw",
    description: "Highlight ring draw-on with label slide and quick fade.",
  },
  {
    id: "spring-pop-v2",
    label: "Spring Pop V2",
    description: "Fast scale and opacity entry with bounded overshoot.",
  },
  {
    id: "slide-rotate-settle-v2",
    label: "Slide Rotate Settle V2",
    description: "Side entry with a subtle rotation that settles to zero.",
  },
  {
    id: "mask-wipe-v2",
    label: "Mask Wipe V2",
    description: "Rectangular clip reveal with fade-safe edges.",
  },
  {
    id: "line-draw-v2",
    label: "Line Draw V2",
    description: "Line and path draw-on motion with optional glow emphasis.",
  },
  {
    id: "word-pop-stagger-v2",
    label: "Word Pop Stagger V2",
    description: "Short word-level reveal paired with pop timing.",
  },
  {
    id: "soft-depth-card-v2",
    label: "Soft Depth Card V2",
    description: "Translucent card depth using shadow, glow, and a settled rotation.",
  },
  {
    id: "pulse-emphasis-v2",
    label: "Pulse Emphasis V2",
    description: "Restrained emphasis pulse for callouts and accent elements.",
  },
  {
    id: "exit-snap-v2",
    label: "Exit Snap V2",
    description: "Quick exit with opacity, blur, and directional movement.",
  },
];
