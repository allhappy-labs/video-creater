import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { Check, ChevronsUpDown, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  generationModelPreferenceId,
  normalizeGenerationModelPreferenceIds,
} from "@/lib/app-settings";
import type { ProviderCredentialStatus } from "@/lib/provider-credentials";
import { useHostPlatformCopy } from "@/lib/runtime/platform";

export interface GenerationSettingsModel {
  provider: string;
  id: string;
  kind: "image" | "video" | "audio" | "upscale";
  displayName: string;
}

export interface GenerationModelMultiselectProps {
  models: GenerationSettingsModel[];
  enabledIds: string[];
  providerStatuses: ProviderCredentialStatus[];
  onChange: (nextIds: string[]) => void;
  onConfigureProvider: (provider: string) => void;
}

const groupOrder: GenerationSettingsModel["kind"][] = [
  "image",
  "video",
  "audio",
  "upscale",
];

function groupLabel(kind: GenerationSettingsModel["kind"]) {
  return `${kind.slice(0, 1).toUpperCase()}${kind.slice(1)}`;
}

function fallbackProviderLabel(provider: string) {
  return provider
    .split(/[-_\s]+/)
    .filter(Boolean)
    .map((part) => `${part.slice(0, 1).toUpperCase()}${part.slice(1)}`)
    .join(" ");
}

function modelPreferenceId(model: GenerationSettingsModel) {
  return generationModelPreferenceId(model);
}

function searchTerms(value: string) {
  return value
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
}

function generationModelSummary(
  models: GenerationSettingsModel[],
  enabledIds: string[],
) {
  const enabled = new Set(enabledIds);
  const enabledRemoteModels = models.filter((model) => {
    const id = modelPreferenceId(model);
    return model.provider.trim().toLowerCase() !== "mock" && id && enabled.has(id);
  });
  if (enabledRemoteModels.length === 0) {
    return "No remote models enabled";
  }
  if (enabledRemoteModels.length === 1) {
    return enabledRemoteModels[0]?.displayName ?? "1 remote model enabled";
  }
  return `${enabledRemoteModels.length} remote models enabled`;
}

function focusableElements(container: HTMLElement) {
  return Array.from(
    container.querySelectorAll<HTMLElement>(
      'input:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ),
  ).filter((element) => !element.hasAttribute("hidden"));
}

export function GenerationModelMultiselect({
  models,
  enabledIds,
  providerStatuses,
  onChange,
  onConfigureProvider,
}: GenerationModelMultiselectProps) {
  const platformCopy = useHostPlatformCopy();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const triggerRef = useRef<HTMLButtonElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const enabled = useMemo(() => new Set(enabledIds), [enabledIds]);
  const statuses = useMemo(
    () =>
      new Map(
        providerStatuses.map((status) => [
          status.provider.trim().toLowerCase(),
          status,
        ]),
      ),
    [providerStatuses],
  );
  const remoteModels = useMemo(
    () =>
      models.filter(
        (model) =>
          model.provider.trim().toLowerCase() !== "mock" &&
          modelPreferenceId(model) !== null,
      ),
    [models],
  );
  const filteredModels = useMemo(() => {
    const queryTerms = searchTerms(query);
    if (queryTerms.length === 0) return remoteModels;
    return remoteModels.filter((model) =>
      [model.displayName, model.provider, model.id].some((value) =>
        queryTerms.every((term) => searchTerms(value).some((word) => word.includes(term))),
      ),
    );
  }, [query, remoteModels]);
  const groups = useMemo(
    () =>
      groupOrder.flatMap((kind) => {
        const groupModels = filteredModels.filter((model) => model.kind === kind);
        return groupModels.length > 0 ? [{ kind, models: groupModels }] : [];
      }),
    [filteredModels],
  );
  const missingProviders = useMemo(() => {
    const providers = new Map<string, string>();
    for (const model of remoteModels) {
      const provider = model.provider.trim();
      const status = statuses.get(provider.toLowerCase());
      if (status?.configured && status.source === "keychain") continue;
      providers.set(
        provider,
        status?.displayName.trim() || fallbackProviderLabel(provider),
      );
    }
    return [...providers.entries()].sort((left, right) =>
      left[1].localeCompare(right[1]),
    );
  }, [remoteModels, statuses]);

  useEffect(() => {
    if (open) {
      searchRef.current?.focus();
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function closeForOutsidePointer(event: PointerEvent) {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (
        !panelRef.current?.contains(target) &&
        !triggerRef.current?.contains(target)
      ) {
        setOpen(false);
      }
    }
    document.addEventListener("pointerdown", closeForOutsidePointer);
    return () => document.removeEventListener("pointerdown", closeForOutsidePointer);
  }, [open]);

  function closeAndRestoreFocus() {
    setOpen(false);
    setQuery("");
    triggerRef.current?.focus();
  }

  function emit(nextIds: Iterable<string>) {
    onChange(normalizeGenerationModelPreferenceIds([...nextIds]));
  }

  function toggle(model: GenerationSettingsModel) {
    const id = modelPreferenceId(model);
    if (!id) return;
    const next = new Set(enabledIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    emit(next);
  }

  function updateGroup(
    groupModels: GenerationSettingsModel[],
    shouldEnable: boolean,
  ) {
    const next = new Set(enabledIds);
    for (const model of groupModels) {
      const id = modelPreferenceId(model);
      if (!id) continue;
      if (shouldEnable) next.add(id);
      else next.delete(id);
    }
    emit(next);
  }

  function handlePanelKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeAndRestoreFocus();
      return;
    }
    if (event.key !== "Tab" || !panelRef.current) return;
    const focusable = focusableElements(panelRef.current);
    const first = focusable[0];
    const last = focusable.at(-1);
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return (
    <div className="relative max-w-xl">
      <Button
        ref={triggerRef}
        type="button"
        role="combobox"
        variant="outline"
        size="sm"
        aria-label="Enabled generation models"
        aria-expanded={open}
        aria-controls={listId}
        aria-haspopup="menu"
        className="h-8 w-full min-w-0 justify-between px-2 text-xs sm:w-80"
        onClick={() => setOpen((current) => !current)}
      >
        <span className="truncate">
          {generationModelSummary(remoteModels, enabledIds)}
        </span>
        <ChevronsUpDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
      </Button>

      {open ? (
        <div
          ref={panelRef}
          id={listId}
          role="menu"
          aria-label="Generation model choices"
          className="absolute left-0 z-50 mt-1 w-[min(26rem,calc(100vw-3rem))] rounded-md border bg-popover p-2 text-popover-foreground shadow-md"
          onKeyDown={handlePanelKeyDown}
        >
          <label className="relative block">
            <Search
              className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <input
              ref={searchRef}
              type="search"
              aria-label="Search generation models"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              className="h-8 w-full rounded-md border bg-background pl-7 pr-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          </label>

          {missingProviders.length > 0 ? (
            <div className="mt-2 grid gap-1 border-t pt-2" aria-label="Providers needing configuration">
              {missingProviders.map(([provider, displayName]) => (
                <div key={provider} className="flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
                  <span className="truncate">{displayName} needs a {platformCopy.credentialStoreShort} credential.</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 shrink-0 px-2 text-[11px]"
                    aria-label={`Configure ${displayName}`}
                    data-settings-blocked-action={`missing-${provider}-provider`}
                    data-settings-configure-target={`integrations:${provider}`}
                    onClick={() => {
                      closeAndRestoreFocus();
                      onConfigureProvider(provider);
                    }}
                  >
                    Configure
                  </Button>
                </div>
              ))}
            </div>
          ) : null}

          <div
            data-testid="generation-model-options"
            className="mt-2 max-h-80 overflow-y-auto border-t pt-1"
          >
            {groups.length === 0 ? (
              <p role="status" className="px-2 py-4 text-center text-xs text-muted-foreground">
                No generation models match this search.
              </p>
            ) : (
              groups.map((group) => {
                const label = groupLabel(group.kind);
                return (
                  <section
                    key={group.kind}
                    role="group"
                    aria-label={`${label} models`}
                    className="border-b py-1 last:border-b-0"
                  >
                    <div className="flex items-center justify-between gap-2 px-2 py-1">
                      <h3 className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                        {label}
                      </h3>
                      <div className="flex items-center gap-1">
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5 text-[10px]"
                          aria-label={`Select all ${label} models`}
                          onClick={() => updateGroup(group.models, true)}
                        >
                          Select all
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5 text-[10px]"
                          aria-label={`Clear ${label} models`}
                          onClick={() => updateGroup(group.models, false)}
                        >
                          Clear
                        </Button>
                      </div>
                    </div>
                    {group.models.map((model) => {
                      const id = modelPreferenceId(model);
                      if (!id) return null;
                      const checked = enabled.has(id);
                      const status = statuses.get(model.provider.trim().toLowerCase());
                      const providerLabel =
                        status?.displayName.trim() ||
                        fallbackProviderLabel(model.provider);
                      return (
                        <button
                          key={id}
                          type="button"
                          role="menuitemcheckbox"
                          aria-checked={checked}
                          aria-label={`${model.displayName}, ${providerLabel}`}
                          className="flex w-full min-w-0 items-center gap-2 rounded-sm px-2 py-1.5 text-left text-xs outline-none hover:bg-accent focus-visible:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
                          onClick={() => toggle(model)}
                          onKeyDown={(event) => {
                            if (event.key === " " || event.key === "Enter") {
                              event.preventDefault();
                              toggle(model);
                            }
                          }}
                        >
                          <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded-sm border border-dim">
                            {checked ? <Check className="h-3 w-3" aria-hidden="true" /> : null}
                          </span>
                          <span className="min-w-0 flex-1 truncate font-medium">
                            {model.displayName}
                          </span>
                          <span className="shrink-0 font-mono text-[10px] text-muted-foreground">
                            {model.provider}
                          </span>
                        </button>
                      );
                    })}
                  </section>
                );
              })
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
