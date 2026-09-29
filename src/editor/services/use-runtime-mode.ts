import { useSyncExternalStore } from "react";

import type { RuntimeMode } from "@/lib/runtime/runtime-descriptor";
import { getRuntimeMode, subscribeRuntimeMode } from "@/lib/runtime/runtime-mode";

/** The active runtime (`desktop | browser | fixture`) selected by `bootstrapRuntime`. */
export function useRuntimeMode(): RuntimeMode {
  return useSyncExternalStore(subscribeRuntimeMode, getRuntimeMode, getRuntimeMode);
}
