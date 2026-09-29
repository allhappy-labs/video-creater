/**
 * Quick-edit suggestions for an empty AI conversation. Choosing one fills the composer with
 * `prompt`, which the user can edit before sending; nothing is sent automatically.
 */
export interface QuickEdit {
  readonly id: string;
  readonly label: string;
  readonly prompt: string;
}

export const quickEdits: readonly QuickEdit[] = [
  {
    id: "tighten-pacing",
    label: "Tighten the pacing",
    prompt: "Tighten the pacing by trimming slow stretches and long pauses without cutting mid-sentence.",
  },
  {
    id: "remove-dead-air",
    label: "Remove dead air",
    prompt: "Remove dead air: cut silences and false starts while keeping natural breaths.",
  },
  {
    id: "add-clean-captions",
    label: "Add clean captions",
    prompt: "Add clean captions that are easy to read and timed to the speech.",
  },
  {
    id: "balance-audio",
    label: "Balance the audio",
    prompt: "Balance the audio so dialogue stays clear over music and effects.",
  },
  {
    id: "shorter-cut",
    label: "Make a shorter cut",
    prompt: "Make a shorter cut that keeps the strongest moments and the story intact.",
  },
];
