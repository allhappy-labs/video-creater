import { SelectField } from "../../properties/controls/select-field";
import { SwitchField } from "../../properties/controls/switch-field";
import { captionMaxWordsOptions, type CaptionBuildSettings } from "./captions-model";

interface CaptionBuildFieldsProps {
  readonly settings: CaptionBuildSettings;
  readonly disabled?: boolean;
  onChange(settings: CaptionBuildSettings): void;
}

/** Max words per line and Censor profanity, shared by Generate captions and Regenerate. */
export function CaptionBuildFields({ settings, disabled = false, onChange }: CaptionBuildFieldsProps) {
  return (
    <>
      <SelectField
        label="Max words"
        value={settings.maxWords.toString()}
        options={captionMaxWordsOptions}
        disabled={disabled}
        onChange={(value) => onChange({ ...settings, maxWords: Number(value) })}
      />
      <SwitchField
        label="Censor profanity"
        checked={settings.censorProfanity}
        disabled={disabled}
        onChange={(censorProfanity) => onChange({ ...settings, censorProfanity })}
      />
    </>
  );
}
