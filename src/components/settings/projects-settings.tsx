import { useState, type ReactNode } from "react";

import type {
  AppPreferenceIntent,
  AppSettingsPreferences,
  NewProjectDefaults,
} from "@/lib/app-settings";

export interface ProjectsSettingsProps {
  preferences: AppSettingsPreferences;
  onChange: (patch: AppPreferenceIntent) => void;
}

const controlClassName =
  "h-8 rounded-md border border-input bg-background px-2 text-xs text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring";

function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-2 border-t py-3 md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center">
      <div>
        <h3 className="text-xs font-semibold text-foreground">{label}</h3>
        <p className="text-[11px] leading-5 text-muted-foreground">
          {description}
        </p>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-2 md:justify-end">
        {children}
      </div>
    </div>
  );
}

function presetFor(defaults: NewProjectDefaults) {
  if (defaults.width === 1280 && defaults.height === 720) return "hd";
  if (defaults.width === 1920 && defaults.height === 1080) return "fhd";
  if (defaults.width === 3840 && defaults.height === 2160) return "uhd";
  return "custom";
}

function validDimension(value: number) {
  return Number.isInteger(value) && value >= 2 && value <= 16_384 && value % 2 === 0;
}

export function ProjectsSettings({
  preferences,
  onChange,
}: ProjectsSettingsProps) {
  const defaults = preferences.newProjectDefaults;
  const [customSelected, setCustomSelected] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const selectedPreset = customSelected ? "custom" : presetFor(defaults);

  function submit(change: Partial<NewProjectDefaults>) {
    setValidationError(null);
    onChange({ newProjectDefaults: change });
  }

  function changeDimension(key: "width" | "height", rawValue: string) {
    const value = Number(rawValue);
    if (!validDimension(value)) {
      setValidationError(
        "Dimensions must be even values between 2 and 16384.",
      );
      return;
    }
    submit({ [key]: value });
  }

  return (
    <section
      data-settings-target="projects:newProjectDefaults"
      tabIndex={-1}
      aria-labelledby="new-project-defaults-heading"
      className="grid max-w-3xl text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <div className="pb-3">
        <h2 id="new-project-defaults-heading" className="text-sm font-semibold">
          New project defaults
        </h2>
        <p className="mt-1 text-[11px] text-muted-foreground">
          Copied into projects created after the accepted change. Existing projects keep their own settings.
        </p>
      </div>

      <SettingRow
        label="Format preset"
        description="Choose a standard frame size or use validated custom dimensions."
      >
        <select
          aria-label="Project format preset"
          value={selectedPreset}
          className={controlClassName}
          onChange={(event) => {
            const value = event.currentTarget.value;
            if (value === "custom") {
              setCustomSelected(true);
              return;
            }
            setCustomSelected(false);
            if (value === "hd") submit({ width: 1280, height: 720 });
            if (value === "fhd") submit({ width: 1920, height: 1080 });
            if (value === "uhd") submit({ width: 3840, height: 2160 });
          }}
        >
          <option value="hd">HD · 1280 × 720</option>
          <option value="fhd">Full HD · 1920 × 1080</option>
          <option value="uhd">UHD · 3840 × 2160</option>
          <option value="custom">Custom</option>
        </select>
        <input
          type="number"
          min={2}
          max={16384}
          step={2}
          aria-label="Project width"
          value={defaults.width}
          className={`${controlClassName} w-24 tabular-nums`}
          onChange={(event) => changeDimension("width", event.currentTarget.value)}
        />
        <span aria-hidden="true" className="text-muted-foreground">×</span>
        <input
          type="number"
          min={2}
          max={16384}
          step={2}
          aria-label="Project height"
          value={defaults.height}
          className={`${controlClassName} w-24 tabular-nums`}
          onChange={(event) => changeDimension("height", event.currentTarget.value)}
        />
      </SettingRow>

      <SettingRow
        label="Frame rate"
        description="Copied into newly created projects."
      >
        <select
          aria-label="Project frame rate"
          value={defaults.fps}
          className={controlClassName}
          onChange={(event) =>
            submit({ fps: Number(event.currentTarget.value) })
          }
        >
          {[23.976, 24, 25, 29.97, 30, 50, 59.94, 60].map((fps) => (
            <option key={fps} value={fps}>{fps} fps</option>
          ))}
        </select>
      </SettingRow>

      <SettingRow
        label="Audio loudness"
        description="Sets the target loudness for new project exports."
      >
        <div className="flex items-center gap-2">
          <input
            type="number"
            step="0.1"
            aria-label="Project loudness target"
            value={defaults.loudnessLufs}
            className={`${controlClassName} w-24 tabular-nums`}
            onChange={(event) => {
              const value = Number(event.currentTarget.value);
              if (!Number.isFinite(value)) {
                setValidationError("Loudness must be a finite LUFS value.");
                return;
              }
              submit({ loudnessLufs: value });
            }}
          />
          <span className="text-[11px] text-muted-foreground">LUFS</span>
        </div>
      </SettingRow>

      <SettingRow
        label="Caption delivery"
        description="Sets how captions are delivered by default."
      >
        <select
          aria-label="Project caption mode"
          value={defaults.captions}
          className={controlClassName}
          onChange={(event) =>
            submit({
              captions: event.currentTarget.value as NewProjectDefaults["captions"],
            })
          }
        >
          <option value="burn_in">Burn into video</option>
          <option value="mux">Mux as caption track</option>
          <option value="off">Off</option>
        </select>
      </SettingRow>

      {validationError ? (
        <p role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">
          {validationError}
        </p>
      ) : null}
    </section>
  );
}
