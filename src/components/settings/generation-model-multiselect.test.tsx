import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";

import { normalizeGenerationModelPreferenceIds } from "@/lib/app-settings";
import type { ProviderCredentialStatus } from "@/lib/provider-credentials";
import {
  GenerationModelMultiselect,
  type GenerationSettingsModel,
} from "./generation-model-multiselect";

const models: GenerationSettingsModel[] = [
  {
    provider: "replicate",
    id: "seedance-1.5-pro",
    kind: "video",
    displayName: "Seedance 1.5 Pro",
  },
  {
    provider: "openai",
    id: "gpt-image-2",
    kind: "image",
    displayName: "GPT-image-2",
  },
  {
    provider: "elevenlabs",
    id: "eleven-v3",
    kind: "audio",
    displayName: "Eleven v3",
  },
  {
    provider: "replicate",
    id: "flux-schnell",
    kind: "image",
    displayName: "Flux Schnell",
  },
];

function providerStatus(
  provider: string,
  displayName: string,
  configured = true,
): ProviderCredentialStatus {
  return {
    provider,
    displayName,
    configured,
    source: configured ? "keychain" : "missing",
  };
}

const statuses: ProviderCredentialStatus[] = [
  providerStatus("openai", "OpenAI"),
  providerStatus("replicate", "Replicate"),
  providerStatus("elevenlabs", "ElevenLabs"),
];

function renderMultiselect({ configured = true } = {}) {
  const onChange = vi.fn();
  const onConfigureProvider = vi.fn();
  const result = render(
    <GenerationModelMultiselect
      models={models}
      enabledIds={[]}
      providerStatuses={statuses.map((status) =>
        status.provider === "openai"
          ? { ...status, configured, source: configured ? "keychain" : "missing" }
          : status,
      )}
      onChange={onChange}
      onConfigureProvider={onConfigureProvider}
    />,
  );
  return { ...result, onChange, onConfigureProvider };
}

it("searches grouped models and toggles checkboxes from the keyboard", () => {
  const { onChange } = renderMultiselect();
  const trigger = screen.getByRole("combobox", {
    name: "Enabled generation models",
  });
  expect(trigger).toHaveTextContent("No remote models enabled");
  fireEvent.click(trigger);
  fireEvent.change(
    screen.getByRole("searchbox", { name: "Search generation models" }),
    { target: { value: "gpt-image" } },
  );
  const option = screen.getByRole("menuitemcheckbox", {
    name: /GPT-image-2/,
  });
  option.focus();
  fireEvent.keyDown(option, { key: " " });
  expect(onChange).toHaveBeenCalledWith(["openai:gpt-image-2"]);
});

it("matches a space-separated query against hyphenated model names", () => {
  renderMultiselect();
  fireEvent.click(
    screen.getByRole("combobox", { name: "Enabled generation models" }),
  );
  fireEvent.change(
    screen.getByRole("searchbox", { name: "Search generation models" }),
    { target: { value: "gpt image" } },
  );
  expect(
    screen.getByRole("menuitemcheckbox", { name: /GPT-image-2/ }),
  ).toBeInTheDocument();
});

it("keeps the popover bounded and links missing providers", () => {
  const { onConfigureProvider } = renderMultiselect({ configured: false });
  fireEvent.click(
    screen.getByRole("combobox", { name: "Enabled generation models" }),
  );
  expect(screen.getByTestId("generation-model-options")).toHaveClass(
    "max-h-80",
    "overflow-y-auto",
  );
  fireEvent.click(screen.getByRole("button", { name: "Configure OpenAI" }));
  expect(onConfigureProvider).toHaveBeenCalledWith("openai");
});

it("groups models with sorted select-all and clear actions", () => {
  const onChange = vi.fn();
  render(
    <GenerationModelMultiselect
      models={models}
      enabledIds={["replicate:seedance-1.5-pro"]}
      providerStatuses={statuses}
      onChange={onChange}
      onConfigureProvider={vi.fn()}
    />,
  );

  const trigger = screen.getByRole("combobox", {
    name: "Enabled generation models",
  });
  expect(trigger).toHaveTextContent("Seedance 1.5 Pro");
  fireEvent.click(trigger);
  expect(screen.getByRole("group", { name: "Image models" })).toBeInTheDocument();
  expect(screen.getByRole("group", { name: "Video models" })).toBeInTheDocument();
  expect(screen.getByRole("group", { name: "Audio models" })).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Select all Image models" }));
  expect(onChange).toHaveBeenLastCalledWith([
    "openai:gpt-image-2",
    "replicate:flux-schnell",
    "replicate:seedance-1.5-pro",
  ]);

  fireEvent.click(screen.getByRole("button", { name: "Clear Video models" }));
  expect(onChange).toHaveBeenLastCalledWith([]);
});

it("closes on Escape and returns focus to the trigger", () => {
  renderMultiselect();
  const trigger = screen.getByRole("combobox", {
    name: "Enabled generation models",
  });
  fireEvent.click(trigger);
  const search = screen.getByRole("searchbox", {
    name: "Search generation models",
  });
  expect(search).toHaveFocus();
  fireEvent.keyDown(search, { key: "Escape" });
  expect(
    screen.queryByRole("searchbox", { name: "Search generation models" }),
  ).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
});

it("normalizes provider model IDs before persistence", () => {
  expect(
    normalizeGenerationModelPreferenceIds([
      " replicate:seedance-1.5-pro ",
      "openai:gpt-image-2",
      "openai:gpt-image-2",
      "missing-separator",
      ":missing-provider",
    ]),
  ).toEqual(["openai:gpt-image-2", "replicate:seedance-1.5-pro"]);
});
