import { useEffect, useMemo, useRef, useState } from "react";
import {
  CheckCircle2,
  KeyRound,
  Loader2,
  RotateCw,
  Unplug,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { generationModelPreferenceId } from "@/lib/app-settings";
import {
  deleteProviderCredential,
  listProviderCredentialStatuses,
  setProviderCredential,
  type ProviderCredentialStatus,
  type ProviderCredentialSource,
} from "@/lib/provider-credentials";
import {
  getHostPlatform,
  hostPlatformCopy,
  useHostPlatformCopy,
  type HostPlatformCopy,
} from "@/lib/runtime/platform";
import type { SettingsOperation } from "@/lib/settings/operations";
import {
  getProviderHealth,
  providerIntegrationGroup,
  refreshProviderHealth,
  type ProviderHealth,
  type ProviderIntegrationGroup,
} from "@/lib/settings/providers";
import { operationForTarget, SettingsOperationStatus } from "./settings-operation-status";

export interface ProviderGenerationModel {
  provider: string;
  id: string;
  kind: "image" | "video" | "audio" | "upscale";
  displayName: string;
}

export interface ProvidersSettingsProps {
  disabledGenerationModelIds: string[];
  generationModels: ProviderGenerationModel[];
  onCredentialStatusChange?: (status: ProviderCredentialStatus) => void;
  operations?: SettingsOperation[];
}

type CredentialValidation = {
  draftGeneration: number;
};
type RefreshAuthority = {
  generation: number;
  providerIds: string[];
  allProviders: boolean;
};
type RefreshRequest = {
  targetId: string;
  provider: string | null;
  requestGeneration: number;
  disabledModelIds: string[];
  disabledModelKey: string;
  focusKey: string | null;
  authority: RefreshAuthority;
};
type PendingRefresh = {
  operationId: string;
  request: RefreshRequest;
  adopted: boolean;
};

const terminalStates = new Set(["succeeded", "failed", "cancelled"]);

function sourceLabel(source: ProviderCredentialSource, copy: HostPlatformCopy) {
  switch (source) {
    case "keychain":
      return copy.credentialSavedLabel;
    case "unavailable":
      return "Credential source unavailable";
    case "missing":
      return "Not configured";
  }
}

function safeActionError(action: "saved" | "removed") {
  return action === "saved"
    ? "Credential could not be saved. Check the value and try again."
    : "Credential could not be removed. Try again.";
}

function validationError(reason: "notAccepted" | "notStarted" | "notCompleted") {
  switch (reason) {
    case "notAccepted":
      return "Credential was saved, but provider validation did not accept it.";
    case "notStarted":
      return "Credential was saved, but provider validation could not be started.";
    case "notCompleted":
      return "Credential was saved, but provider validation could not be completed.";
  }
}

function validationAccepted(provider: ProviderHealth | undefined) {
  return Boolean(
    provider?.configured &&
      (provider.validationState === "available" ||
        provider.validationState === "balanceUnavailable"),
  );
}

function dependencyLabel(
  modelId: string,
  generationModels: ProviderGenerationModel[],
) {
  const model = generationModels.find((candidate) => {
    const preferenceId = generationModelPreferenceId(candidate);
    return preferenceId === modelId || candidate.id === modelId;
  });
  return model?.displayName ?? modelId;
}

function providerIssue(provider: ProviderHealth) {
  if (!provider.configured) return null;
  if (provider.validationState === "rejected") {
    return "Credential rejected. Update the key or validate again.";
  }
  if (provider.validationState === "unavailable") {
    return "Provider validation unavailable. Validate again when the service is reachable.";
  }
  if (provider.validationState === "missing") {
    return "The saved credential could not be found. Update the key or disconnect.";
  }
  return null;
}

function ProviderRow({
  provider,
  credentialStatus,
  generationModels,
  draft,
  mutationError,
  mutating,
  refreshing,
  operation,
  inputRef,
  disconnectButtonRef,
  onDraftChange,
  onSave,
  onDisconnect,
  onValidate,
}: {
  provider: ProviderHealth;
  credentialStatus: ProviderCredentialStatus | null;
  generationModels: ProviderGenerationModel[];
  draft: string;
  mutationError: string | null;
  mutating: boolean;
  refreshing: boolean;
  operation: SettingsOperation | null;
  inputRef: (node: HTMLInputElement | null) => void;
  disconnectButtonRef: (node: HTMLButtonElement | null) => void;
  onDraftChange: (value: string) => void;
  onSave: () => void;
  onDisconnect: () => void;
  onValidate: () => void;
}) {
  const platformCopy = useHostPlatformCopy();
  const credentialSource = credentialStatus?.source ?? provider.credentialSource;
  const configured = providerIntegrationGroup(provider, credentialStatus) === "configured";
  const issue = providerIssue({ ...provider, configured });
  const dependencyLabels = provider.dependentModelIds.map((modelId) =>
    dependencyLabel(modelId, generationModels),
  );
  const saveActionLabel = configured
    ? `Update ${provider.displayName} key`
    : `Connect ${provider.displayName}`;

  return (
    <form
      aria-label={`${provider.displayName} credential`}
      data-settings-target={`integrations:${provider.provider}`}
      data-settings-credential-control
      data-settings-credential-storage="keychain"
      tabIndex={-1}
      className="grid min-w-0 gap-2 border-t py-3 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring lg:grid-cols-[minmax(11rem,0.8fr)_minmax(15rem,1.35fr)_auto] lg:items-center"
      onSubmit={(event) => {
        event.preventDefault();
        onSave();
      }}
    >
      <div className="min-w-0">
        <div className="flex min-w-0 items-center gap-2">
          <KeyRound className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
          <span className="truncate font-medium text-foreground">{provider.displayName}</span>
          <span
            className={`shrink-0 text-[10px] font-medium ${
              configured
                ? "text-emerald-700"
                : "text-muted-foreground"
            }`}
          >
            {sourceLabel(credentialSource, platformCopy)}
          </span>
        </div>
        {provider.accountLabel || provider.balanceLabel ? (
          <div className="mt-1 flex min-w-0 flex-wrap gap-x-2 text-[11px] text-muted-foreground">
            {provider.accountLabel ? <span className="truncate">{provider.accountLabel}</span> : null}
            {provider.balanceLabel ? <span className="font-medium tabular-nums text-foreground">{provider.balanceLabel}</span> : null}
          </div>
        ) : null}
        {dependencyLabels.length > 0 ? (
          <div className="mt-1 truncate text-[11px] text-muted-foreground" title={dependencyLabels.join(", ")}>
            Used by {dependencyLabels.join(", ")}
          </div>
        ) : null}
      </div>

      <div className="grid min-w-0 gap-1.5">
        <input
          ref={inputRef}
          type="password"
          aria-label={`New ${provider.displayName} key`}
          autoComplete="off"
          spellCheck={false}
          value={draft}
          placeholder={
            configured ? "Enter a replacement key" : "Enter a key"
          }
          disabled={mutating}
          onChange={(event) => onDraftChange(event.target.value)}
          className="h-7 min-w-0 rounded-md border bg-background px-2 font-mono text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
        />
        {issue ? (
          <span className={provider.validationState === "rejected" || provider.validationState === "unavailable" ? "text-[11px] text-amber-700" : "text-[11px] text-muted-foreground"}>
            {issue}
          </span>
        ) : null}
        {mutationError ? (
          <span role="alert" className="text-[11px] text-red-700">
            {mutationError}
          </span>
        ) : null}
        {issue && provider.diagnosticCode ? (
          <code className="font-mono text-[10px] text-muted-foreground">{provider.diagnosticCode}</code>
        ) : null}
      </div>

      <div className="grid gap-1.5 lg:justify-items-end">
        <div className="flex flex-wrap items-center gap-1.5 lg:justify-end">
          <Button
            type="submit"
            size="sm"
            className="h-7 px-2 text-xs"
            aria-label={saveActionLabel}
            disabled={mutating || !draft.trim()}
          >
            {mutating ? <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" /> : null}
            {configured ? "Update key" : "Connect"}
          </Button>
          {configured ? (
            <>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 px-2 text-xs"
                aria-label={`Validate ${provider.displayName}`}
                disabled={refreshing}
                onClick={onValidate}
              >
                <RotateCw className={`h-3.5 w-3.5 ${refreshing ? "animate-spin motion-reduce:animate-none" : ""}`} aria-hidden="true" />
                Validate
              </Button>
              <Button
                ref={disconnectButtonRef}
                type="button"
                variant="outline"
                size="sm"
                className="h-7 px-2 text-xs text-red-700 hover:text-red-800"
                aria-label={`Disconnect ${provider.displayName}`}
                disabled={mutating}
                onClick={onDisconnect}
              >
                Disconnect
              </Button>
            </>
          ) : null}
        </div>
        <SettingsOperationStatus operation={operation} ariaLabel={`${provider.displayName} refresh status`} />
      </div>
    </form>
  );
}

export function ProvidersSettings({
  disabledGenerationModelIds,
  generationModels,
  onCredentialStatusChange,
  operations = [],
}: ProvidersSettingsProps) {
  const [health, setHealth] = useState<ProviderHealth[]>([]);
  const [credentialStatuses, setCredentialStatuses] = useState<Record<string, ProviderCredentialStatus>>({});
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [mutationErrors, setMutationErrors] = useState<Record<string, string>>({});
  const [mutatingProviders, setMutatingProviders] = useState<ReadonlySet<string>>(new Set());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [credentialLoadError, setCredentialLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const platformCopy = useHostPlatformCopy();
  const [refreshingTargets, setRefreshingTargets] = useState<ReadonlySet<string>>(new Set());
  const [localOperations, setLocalOperations] = useState<Record<string, SettingsOperation>>({});
  const [disconnectProvider, setDisconnectProvider] = useState<ProviderHealth | null>(null);
  const [focusVersion, setFocusVersion] = useState(0);
  const mountedRef = useRef(true);
  const operationsRef = useRef(operations);
  operationsRef.current = operations;
  const disabledModelIdsRef = useRef(disabledGenerationModelIds);
  disabledModelIdsRef.current = disabledGenerationModelIds;
  const healthRequestRef = useRef(0);
  const credentialStatusRequestRef = useRef(0);
  const credentialMutationGenerationRef = useRef<Record<string, number>>({});
  const pendingCredentialValidationsRef = useRef<Record<string, CredentialValidation>>({});
  const providerIdsRef = useRef(new Set<string>());
  const authorityClockRef = useRef(0);
  const latestRefreshAuthorityRef = useRef(0);
  const latestAllAuthorityRef = useRef(0);
  const providerAuthorityRef = useRef<Record<string, number>>({});
  const refreshRequestRef = useRef<Record<string, number>>({});
  const refreshStartingRef = useRef(new Set<string>());
  const mutationStartingRef = useRef(new Set<string>());
  const pendingRefreshesRef = useRef(new Map<string, PendingRefresh>());
  const desiredRefreshesRef = useRef(new Map<string, RefreshRequest>());
  const observedOperationIdsRef = useRef(new Set<string>());
  const deferredOperationEventsRef = useRef(new Map<string, SettingsOperation>());
  const draftGenerationRef = useRef<Record<string, number>>({});
  const inputRefs = useRef<Record<string, HTMLInputElement | null>>({});
  const disconnectButtonRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const disconnectCancelRef = useRef<HTMLButtonElement | null>(null);
  const disconnectConfirmRef = useRef<HTMLButtonElement | null>(null);
  const restoreDisconnectFocusRef = useRef<string | null>(null);
  const pendingFocusRef = useRef<string | null>(null);

  function restoreFocus(focusKey: string | null) {
    if (!focusKey) return;
    pendingFocusRef.current = focusKey;
    setFocusVersion((current) => current + 1);
  }

  useEffect(() => {
    const focusKey = pendingFocusRef.current;
    if (!focusKey) return;
    const provider = focusKey.slice("provider:".length);
    const target = inputRefs.current[provider];
    target?.focus();
    if (target) pendingFocusRef.current = null;
  }, [focusVersion, health]);

  useEffect(() => {
    if (disconnectProvider) {
      disconnectCancelRef.current?.focus();
      return;
    }
    const provider = restoreDisconnectFocusRef.current;
    if (!provider) return;
    restoreDisconnectFocusRef.current = null;
    disconnectButtonRefs.current[provider]?.focus();
  }, [disconnectProvider]);

  function closeDisconnectDialog(restoreFocus = true) {
    if (restoreFocus && disconnectProvider) {
      restoreDisconnectFocusRef.current = disconnectProvider.provider;
    }
    setDisconnectProvider(null);
  }

  function handleDisconnectDialogKeyDown(
    event: React.KeyboardEvent<HTMLElement>,
  ) {
    if (event.key === "Escape") {
      event.preventDefault();
      closeDisconnectDialog();
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = [
      disconnectCancelRef.current,
      disconnectConfirmRef.current,
    ].filter((element): element is HTMLButtonElement => element !== null);
    if (focusable.length === 0) return;
    const currentIndex = focusable.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex = event.shiftKey
      ? currentIndex <= 0
        ? focusable.length - 1
        : currentIndex - 1
      : currentIndex === focusable.length - 1
        ? 0
        : currentIndex + 1;
    event.preventDefault();
    focusable[nextIndex]?.focus();
  }

  function authoritativeProviderIds(
    authority: RefreshAuthority,
    discoveredProviders: string[] = [],
  ) {
    if (
      authority.allProviders &&
      latestAllAuthorityRef.current === authority.generation
    ) {
      for (const provider of discoveredProviders) {
        const currentAuthority = providerAuthorityRef.current[provider];
        if (
          currentAuthority === undefined ||
          currentAuthority < authority.generation
        ) {
          providerAuthorityRef.current[provider] = authority.generation;
        }
      }
    }
    return Array.from(new Set([...authority.providerIds, ...discoveredProviders])).filter(
      (provider) => providerAuthorityRef.current[provider] === authority.generation,
    );
  }

  function controlsLoadError(authority: RefreshAuthority | null) {
    return latestRefreshAuthorityRef.current === (authority?.generation ?? 0);
  }

  function reconcileCredentialValidations(
    authority: RefreshAuthority,
    nextHealth: ProviderHealth[],
  ) {
    for (const provider of authoritativeProviderIds(
      authority,
      nextHealth.map((provider) => provider.provider),
    )) {
      const validation = pendingCredentialValidationsRef.current[provider];
      if (!validation) continue;
      if (draftGenerationRef.current[provider] !== validation.draftGeneration) {
        delete pendingCredentialValidationsRef.current[provider];
        continue;
      }
      if (validationAccepted(
        nextHealth.find((candidate) => candidate.provider === provider),
      )) {
        delete pendingCredentialValidationsRef.current[provider];
        setDrafts((current) => ({ ...current, [provider]: "" }));
        setMutationErrors((current) => ({ ...current, [provider]: "" }));
      } else {
        setMutationErrors((current) => ({
          ...current,
          [provider]: validationError("notAccepted"),
        }));
      }
    }
  }

  function failCredentialValidations(
    authority: RefreshAuthority,
    reason: "notStarted" | "notCompleted",
  ) {
    for (const provider of authoritativeProviderIds(authority)) {
      const validation = pendingCredentialValidationsRef.current[provider];
      if (!validation) continue;
      if (draftGenerationRef.current[provider] !== validation.draftGeneration) {
        delete pendingCredentialValidationsRef.current[provider];
        continue;
      }
      setMutationErrors((current) => ({
        ...current,
        [provider]: validationError(reason),
      }));
    }
  }

  function mergeAuthoritativeHealth(
    nextHealth: ProviderHealth[],
    authoritativeProviders: string[],
  ) {
    const replacements = new Map(
      nextHealth
        .filter((provider) => authoritativeProviders.includes(provider.provider))
        .map((provider) => [provider.provider, provider]),
    );
    setHealth((current) => {
      const merged = current.map((provider) => replacements.get(provider.provider) ?? provider);
      const currentIds = new Set(current.map((provider) => provider.provider));
      for (const provider of replacements.values()) {
        if (!currentIds.has(provider.provider)) merged.push(provider);
      }
      return merged;
    });
  }

  function loadHealth(
    focusKey: string | null = null,
    authority: RefreshAuthority | null = null,
  ) {
    const requestId = ++healthRequestRef.current;
    const refreshAuthorityAtStart = latestRefreshAuthorityRef.current;
    if (controlsLoadError(authority)) setLoadError(null);
    return getProviderHealth()
      .then((nextHealth) => {
        if (!mountedRef.current) return null;
        if (
          !authority &&
          latestRefreshAuthorityRef.current !== refreshAuthorityAtStart
        ) {
          return nextHealth;
        }
        for (const provider of nextHealth) providerIdsRef.current.add(provider.provider);
        if (authority) {
          const authoritativeProviders = authoritativeProviderIds(
            authority,
            nextHealth.map((provider) => provider.provider),
          );
          reconcileCredentialValidations(authority, nextHealth);
          if (authoritativeProviders.length > 0) {
            mergeAuthoritativeHealth(nextHealth, authoritativeProviders);
          }
          restoreFocus(focusKey);
          return nextHealth;
        }
        if (healthRequestRef.current !== requestId) return nextHealth;
        setHealth(nextHealth);
        restoreFocus(focusKey);
        const hasActiveRefresh = operationsRef.current.some(
          (operation) =>
            operation.kind === "providerRefresh" &&
            !terminalStates.has(operation.state),
        );
        if (
          nextHealth.length === 0 &&
          !hasActiveRefresh &&
          pendingRefreshesRef.current.size === 0
        ) {
          requestRefresh(null, disabledModelIdsRef.current, null);
        }
        return nextHealth;
      })
      .catch(() => {
        if (!mountedRef.current) return null;
        if (
          !authority &&
          latestRefreshAuthorityRef.current !== refreshAuthorityAtStart
        ) {
          return null;
        }
        if (authority) {
          failCredentialValidations(authority, "notCompleted");
          if (
            controlsLoadError(authority) &&
            authoritativeProviderIds(authority).length > 0
          ) {
            setLoadError("Provider health could not be loaded. Retry the health check.");
          }
          restoreFocus(focusKey);
          return null;
        }
        if (healthRequestRef.current !== requestId) return null;
        setLoadError("Provider health could not be loaded. Retry the health check.");
        restoreFocus(focusKey);
        return null;
      });
  }

  useEffect(() => {
    mountedRef.current = true;
    setLoading(true);
    const credentialStatusRequestId = ++credentialStatusRequestRef.current;
    const credentialMutationGenerations = {
      ...credentialMutationGenerationRef.current,
    };
    void Promise.all([
      loadHealth(),
      listProviderCredentialStatuses()
        .then((statuses) => {
          if (
            mountedRef.current &&
            credentialStatusRequestRef.current === credentialStatusRequestId
          ) {
            setCredentialStatuses((current) => {
              const next = { ...current };
              for (const status of statuses) {
                if (
                  (credentialMutationGenerationRef.current[status.provider] ?? 0) ===
                  (credentialMutationGenerations[status.provider] ?? 0)
                ) {
                  next[status.provider] = status;
                }
              }
              return next;
            });
          }
        })
        .catch(() => {
          if (
            mountedRef.current &&
            credentialStatusRequestRef.current === credentialStatusRequestId
          ) {
            setCredentialLoadError(
              `Credential sources could not be loaded. ${
                hostPlatformCopy(getHostPlatform()).credentialValuesHidden
              }`,
            );
          }
        }),
    ]).finally(() => {
      if (mountedRef.current) setLoading(false);
    });
    return () => {
      mountedRef.current = false;
      healthRequestRef.current += 1;
      credentialStatusRequestRef.current += 1;
      pendingRefreshesRef.current.clear();
      desiredRefreshesRef.current.clear();
      observedOperationIdsRef.current.clear();
      deferredOperationEventsRef.current.clear();
      refreshStartingRef.current.clear();
      mutationStartingRef.current.clear();
    };
  }, []);

  function completeRefresh(operation: SettingsOperation, pending: PendingRefresh) {
    if (!terminalStates.has(operation.state)) return false;
    pendingRefreshesRef.current.delete(operation.id);
    setRefreshingTargets((current) => {
      const next = new Set(current);
      next.delete(pending.request.targetId);
      return next;
    });
    setLocalOperations((current) => {
      const next = { ...current };
      delete next[pending.request.targetId];
      return next;
    });

    const desired = desiredRefreshesRef.current.get(pending.request.targetId);
    const requestIsCurrent = Boolean(
      !pending.adopted &&
        desired &&
        desired.requestGeneration === pending.request.requestGeneration &&
        desired.disabledModelKey === pending.request.disabledModelKey,
    );
    const mustLaunchLatest = Boolean(
      pending.adopted ||
        (desired &&
          (desired.requestGeneration > pending.request.requestGeneration ||
            desired.disabledModelKey !== pending.request.disabledModelKey)),
    );

    if (mustLaunchLatest) {
      void launchDesiredRefresh(pending.request.targetId);
      return true;
    }

    if (requestIsCurrent) {
      desiredRefreshesRef.current.delete(pending.request.targetId);
      if (operation.state === "succeeded") {
        void loadHealth(
          pending.request.focusKey,
          pending.request.authority,
        );
      } else {
        failCredentialValidations(pending.request.authority, "notCompleted");
        if (controlsLoadError(pending.request.authority)) {
          setLoadError("Provider refresh did not complete. Retry the health check.");
        }
        restoreFocus(pending.request.focusKey);
      }
    }
    return true;
  }

  function adoptActiveRefresh(operation: SettingsOperation) {
    if (
      operation.kind !== "providerRefresh" ||
      terminalStates.has(operation.state) ||
      pendingRefreshesRef.current.has(operation.id) ||
      observedOperationIdsRef.current.has(operation.id)
    ) {
      return false;
    }
    const targetId = operation.targetId;
    observedOperationIdsRef.current.add(operation.id);
    const unknownRequest: RefreshRequest = {
      targetId,
      provider: targetId === "providers" ? null : targetId,
      requestGeneration: 0,
      disabledModelIds: [],
      disabledModelKey: "unknown",
      focusKey: null,
      authority: { generation: 0, providerIds: [], allProviders: false },
    };
    pendingRefreshesRef.current.set(operation.id, {
      operationId: operation.id,
      request: unknownRequest,
      adopted: true,
    });
    setRefreshingTargets((current) => new Set(current).add(targetId));
    setLocalOperations((current) => ({ ...current, [targetId]: operation }));
    requestRefresh(
      targetId === "providers" ? null : targetId,
      disabledModelIdsRef.current,
      null,
    );
    return true;
  }

  useEffect(() => {
    for (const operation of operations) {
      if (refreshStartingRef.current.has(operation.targetId)) {
        if (
          operation.kind === "providerRefresh" &&
          !pendingRefreshesRef.current.has(operation.id) &&
          !observedOperationIdsRef.current.has(operation.id)
        ) {
          const deferred = deferredOperationEventsRef.current.get(operation.targetId);
          if (!deferred || deferred.id === operation.id) {
            deferredOperationEventsRef.current.set(operation.targetId, operation);
          }
        }
        continue;
      }
      adoptActiveRefresh(operation);
    }
  }, [operations]);

  useEffect(() => {
    for (const [operationId, pending] of pendingRefreshesRef.current) {
      const terminal = operations.find(
        (candidate) => candidate.id === operationId && terminalStates.has(candidate.state),
      );
      if (terminal) completeRefresh(terminal, pending);
    }
  }, [operations]);

  function requestRefresh(
    provider: string | null,
    nextDisabledModelIds: string[],
    focusKey: string | null,
  ) {
    const targetId = provider ?? "providers";
    const requestGeneration = (refreshRequestRef.current[targetId] ?? 0) + 1;
    refreshRequestRef.current[targetId] = requestGeneration;
    const authorityGeneration = ++authorityClockRef.current;
    latestRefreshAuthorityRef.current = authorityGeneration;
    if (provider === null) latestAllAuthorityRef.current = authorityGeneration;
    const authorityProviderIds = provider
      ? [provider]
      : Array.from(new Set([
          ...providerIdsRef.current,
          ...Object.keys(pendingCredentialValidationsRef.current),
        ]));
    for (const providerId of authorityProviderIds) {
      providerAuthorityRef.current[providerId] = authorityGeneration;
    }
    desiredRefreshesRef.current.set(targetId, {
      targetId,
      provider,
      requestGeneration,
      disabledModelIds: [...nextDisabledModelIds],
      disabledModelKey: JSON.stringify(nextDisabledModelIds),
      focusKey,
      authority: {
        generation: authorityGeneration,
        providerIds: authorityProviderIds,
        allProviders: provider === null,
      },
    });
    void launchDesiredRefresh(targetId);
  }

  async function launchDesiredRefresh(targetId: string) {
    if (
      refreshStartingRef.current.has(targetId) ||
      Array.from(pendingRefreshesRef.current.values()).some(
        (pending) => pending.request.targetId === targetId,
      )
    ) {
      return;
    }
    const request = desiredRefreshesRef.current.get(targetId);
    if (!request) return;
    refreshStartingRef.current.add(targetId);
    setRefreshingTargets((current) => new Set(current).add(targetId));
    if (controlsLoadError(request.authority)) setLoadError(null);
    try {
      const queued = await refreshProviderHealth(
        request.provider,
        request.disabledModelIds,
      );
      refreshStartingRef.current.delete(targetId);
      if (!mountedRef.current) return;
      const deferred = deferredOperationEventsRef.current.get(targetId);
      if (deferred?.id === queued.id) deferredOperationEventsRef.current.delete(targetId);
      observedOperationIdsRef.current.add(queued.id);
      const pending: PendingRefresh = {
        operationId: queued.id,
        request,
        adopted: false,
      };
      pendingRefreshesRef.current.set(queued.id, pending);
      setLocalOperations((current) => ({ ...current, [targetId]: queued }));
      const observed = operationsRef.current.find((candidate) => candidate.id === queued.id);
      if (!completeRefresh(observed ?? queued, pending)) restoreFocus(request.focusKey);
    } catch {
      refreshStartingRef.current.delete(targetId);
      if (mountedRef.current) {
        const deferred = deferredOperationEventsRef.current.get(targetId);
        deferredOperationEventsRef.current.delete(targetId);
        const activeOperation = operationsRef.current.find(
          (operation) =>
            operation.kind === "providerRefresh" &&
            operation.targetId === targetId &&
            !terminalStates.has(operation.state) &&
            !pendingRefreshesRef.current.has(operation.id) &&
            !observedOperationIdsRef.current.has(operation.id),
        );
        if (activeOperation && adoptActiveRefresh(activeOperation)) return;
        if (
          deferred &&
          deferred.kind === "providerRefresh" &&
          !observedOperationIdsRef.current.has(deferred.id)
        ) {
          observedOperationIdsRef.current.add(deferred.id);
          requestRefresh(
            request.provider,
            disabledModelIdsRef.current,
            request.focusKey,
          );
          return;
        }
        const desired = desiredRefreshesRef.current.get(targetId);
        const requestIsCurrent = desired?.requestGeneration === request.requestGeneration;
        if (requestIsCurrent) desiredRefreshesRef.current.delete(targetId);
        if (controlsLoadError(request.authority)) {
          setLoadError("Provider refresh could not be started. Retry the health check.");
        }
        setRefreshingTargets((current) => {
          const next = new Set(current);
          next.delete(targetId);
          return next;
        });
        setLocalOperations((current) => {
          const next = { ...current };
          delete next[targetId];
          return next;
        });
        failCredentialValidations(request.authority, "notStarted");
        restoreFocus(request.focusKey);
        if (!requestIsCurrent) void launchDesiredRefresh(targetId);
      }
    }
  }

  async function saveCredential(provider: ProviderHealth) {
    const draft = drafts[provider.provider]?.trim() ?? "";
    if (!draft || mutationStartingRef.current.has(provider.provider)) return;
    const draftGeneration = draftGenerationRef.current[provider.provider] ?? 0;
    mutationStartingRef.current.add(provider.provider);
    setMutatingProviders((current) => new Set(current).add(provider.provider));
    setMutationErrors((current) => ({ ...current, [provider.provider]: "" }));
    try {
      const status = await setProviderCredential(provider.provider, draft);
      if (!mountedRef.current) return;
      credentialMutationGenerationRef.current[provider.provider] =
        (credentialMutationGenerationRef.current[provider.provider] ?? 0) + 1;
      setCredentialStatuses((current) => ({ ...current, [status.provider]: status }));
      onCredentialStatusChange?.(status);
      setHealth((current) =>
        current.map((candidate) =>
          candidate.provider === provider.provider
            ? {
                ...candidate,
                credentialSource: status.source,
                configured: status.configured,
                validationState: status.configured ? "notChecked" : "missing",
                accountLabel: null,
                balanceLabel: null,
                lastCheckedAt: null,
                diagnosticCode: null,
              }
            : candidate,
        ),
      );
      pendingCredentialValidationsRef.current[provider.provider] = {
        draftGeneration,
      };
      setDrafts((current) => ({ ...current, [provider.provider]: "" }));
      restoreFocus(`provider:${provider.provider}`);
      requestRefresh(
        provider.provider,
        disabledModelIdsRef.current,
        `provider:${provider.provider}`,
      );
    } catch {
      if (mountedRef.current) {
        setMutationErrors((current) => ({
          ...current,
          [provider.provider]: safeActionError("saved"),
        }));
        restoreFocus(`provider:${provider.provider}`);
      }
    } finally {
      mutationStartingRef.current.delete(provider.provider);
      if (mountedRef.current) {
        setMutatingProviders((current) => {
          const next = new Set(current);
          next.delete(provider.provider);
          return next;
        });
      }
    }
  }

  async function removeCredential(provider: ProviderHealth) {
    if (mutationStartingRef.current.has(provider.provider)) return;
    mutationStartingRef.current.add(provider.provider);
    setMutatingProviders((current) => new Set(current).add(provider.provider));
    setMutationErrors((current) => ({ ...current, [provider.provider]: "" }));
    try {
      const status = await deleteProviderCredential(provider.provider);
      if (!mountedRef.current) return;
      credentialMutationGenerationRef.current[provider.provider] =
        (credentialMutationGenerationRef.current[provider.provider] ?? 0) + 1;
      setCredentialStatuses((current) => ({ ...current, [status.provider]: status }));
      onCredentialStatusChange?.(status);
      setHealth((current) =>
        current.map((candidate) =>
          candidate.provider === provider.provider
            ? {
                ...candidate,
                credentialSource: status.source,
                configured: status.configured,
                validationState: "missing",
                accountLabel: null,
                balanceLabel: null,
                lastCheckedAt: null,
                diagnosticCode: null,
              }
            : candidate,
        ),
      );
      delete pendingCredentialValidationsRef.current[provider.provider];
      draftGenerationRef.current[provider.provider] =
        (draftGenerationRef.current[provider.provider] ?? 0) + 1;
      setDrafts((current) => ({ ...current, [provider.provider]: "" }));
      restoreFocus(`provider:${provider.provider}`);
      requestRefresh(
        provider.provider,
        disabledModelIdsRef.current,
        `provider:${provider.provider}`,
      );
    } catch {
      if (mountedRef.current) {
        setMutationErrors((current) => ({
          ...current,
          [provider.provider]: safeActionError("removed"),
        }));
        restoreFocus(`provider:${provider.provider}`);
      }
    } finally {
      mutationStartingRef.current.delete(provider.provider);
      if (mountedRef.current) {
        setMutatingProviders((current) => {
          const next = new Set(current);
          next.delete(provider.provider);
          return next;
        });
      }
    }
  }

  const grouped = useMemo(() => {
    const result: Record<ProviderIntegrationGroup, ProviderHealth[]> = {
      configured: [],
      available: [],
    };
    for (const provider of health) {
      result[
        providerIntegrationGroup(
          provider,
          credentialStatuses[provider.provider] ?? null,
        )
      ].push(provider);
    }
    return result;
  }, [credentialStatuses, health]);

  const sections = [
    {
      id: "configured" as const,
      title: "Configured integrations",
      detail: `Keys are stored in ${platformCopy.credentialStore}. Validation details stay within each integration.`,
      icon: CheckCircle2,
    },
    {
      id: "available" as const,
      title: "Available integrations",
      detail: "Connect an integration when you want to use its models.",
      icon: Unplug,
    },
  ];

  return (
    <div
      data-testid="providers-settings-page"
      data-settings-credential-boundary="keychain-only"
      className="grid gap-5 text-xs"
    >
      <section aria-label="Integration status" className="grid gap-2 border-t pt-3">
        <div className="flex min-w-0 items-start justify-between gap-3">
          <div>
            <h2 className="text-sm font-semibold tracking-normal">Integration status</h2>
            <p className="mt-1 text-muted-foreground">
              Keys are write-only and stay in {platformCopy.credentialStore}.
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            aria-label="Refresh all providers"
            disabled={refreshingTargets.has("providers")}
            onClick={() => requestRefresh(null, disabledGenerationModelIds, null)}
          >
            <RotateCw className={`h-3.5 w-3.5 ${refreshingTargets.has("providers") ? "animate-spin motion-reduce:animate-none" : ""}`} aria-hidden="true" />
            Refresh
          </Button>
        </div>
        {loading ? <span role="status" className="text-muted-foreground">Loading provider readiness…</span> : null}
        {loadError ? <div role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">{loadError}</div> : null}
        {credentialLoadError ? <div role="alert" className="border-l-2 border-amber-500 pl-2 text-amber-700">{credentialLoadError}</div> : null}
        <SettingsOperationStatus
          operation={operationForTarget(operations, "providers", localOperations.providers ?? null)}
          ariaLabel="Provider refresh status"
        />
      </section>

      {sections.map((section) => {
        const Icon = section.icon;
        return (
          <section key={section.id} aria-label={section.title} className="grid gap-1">
            <div className="flex min-w-0 items-start gap-2">
              <Icon className={`mt-0.5 h-3.5 w-3.5 shrink-0 ${section.id === "configured" ? "text-emerald-700" : "text-muted-foreground"}`} aria-hidden="true" />
              <div className="min-w-0">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-foreground">
                  {section.title}{" "}
                  <span aria-hidden="true" className="tabular-nums text-muted-foreground">{grouped[section.id].length}</span>
                </h3>
                <p className="text-[11px] text-muted-foreground">{section.detail}</p>
              </div>
            </div>
            {grouped[section.id].length === 0 ? (
              <p className="border-t py-3 text-[11px] text-muted-foreground">No providers in this group.</p>
            ) : grouped[section.id].map((provider) => (
              <ProviderRow
                key={provider.provider}
                provider={provider}
                credentialStatus={credentialStatuses[provider.provider] ?? null}
                generationModels={generationModels}
                draft={drafts[provider.provider] ?? ""}
                mutationError={mutationErrors[provider.provider] || null}
                mutating={mutatingProviders.has(provider.provider)}
                refreshing={refreshingTargets.has(provider.provider)}
                operation={operationForTarget(operations, provider.provider, localOperations[provider.provider] ?? null)}
                inputRef={(node) => { inputRefs.current[provider.provider] = node; }}
                disconnectButtonRef={(node) => { disconnectButtonRefs.current[provider.provider] = node; }}
                onDraftChange={(value) => {
                  draftGenerationRef.current[provider.provider] =
                    (draftGenerationRef.current[provider.provider] ?? 0) + 1;
                  delete pendingCredentialValidationsRef.current[provider.provider];
                  setDrafts((current) => ({ ...current, [provider.provider]: value }));
                  setMutationErrors((current) => ({ ...current, [provider.provider]: "" }));
                }}
                onSave={() => void saveCredential(provider)}
                onDisconnect={() => {
                  if (provider.dependentModelIds.length > 0) {
                    setDisconnectProvider(provider);
                  } else {
                    void removeCredential(provider);
                  }
                }}
                onValidate={() => requestRefresh(provider.provider, disabledGenerationModelIds, `provider:${provider.provider}`)}
              />
            ))}
          </section>
        );
      })}

      {disconnectProvider ? (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4">
          <section
            role="dialog"
            aria-modal="true"
            aria-labelledby="disconnect-integration-title"
            className="grid w-full max-w-md gap-3 rounded-md border bg-background p-4 text-xs shadow-xl"
            onKeyDown={handleDisconnectDialogKeyDown}
          >
            <div>
              <h2 id="disconnect-integration-title" className="text-sm font-semibold">
                Disconnect {disconnectProvider.displayName}
              </h2>
              <p className="mt-1 text-muted-foreground">
                {platformCopy.credentialRemovalDetail}
              </p>
            </div>
            <ul className="grid gap-1 text-foreground">
              {disconnectProvider.dependentModelIds.map((modelId) => (
                <li key={modelId}>
                  {dependencyLabel(modelId, generationModels)} will become unavailable.
                </li>
              ))}
            </ul>
            <div className="flex justify-end gap-2">
              <Button
                ref={disconnectCancelRef}
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => closeDisconnectDialog()}
              >
                Cancel
              </Button>
              <Button
                ref={disconnectConfirmRef}
                type="button"
                variant="outline"
                size="sm"
                className="border-red-300 text-red-700 hover:bg-red-50 hover:text-red-800"
                onClick={() => {
                  const provider = disconnectProvider;
                  closeDisconnectDialog(false);
                  void removeCredential(provider);
                }}
              >
                Disconnect {disconnectProvider.displayName}
              </Button>
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
