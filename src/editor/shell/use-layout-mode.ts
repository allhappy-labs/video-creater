import { useSyncExternalStore } from "react";

type LayoutMode = "mobile" | "desktop-overlay" | "desktop-docked";

const mobileMaxWidth = 1023;
const dockedPropertiesMinWidth = 1280;
/** Below this width the top bar folds the gear menu into a "More" button. */
const topBarMoreMenuBelowWidth = 380;

export function layoutModeForWidth(width: number): LayoutMode {
  if (width <= mobileMaxWidth) return "mobile";
  return width >= dockedPropertiesMinWidth ? "desktop-docked" : "desktop-overlay";
}

function subscribe(onChange: () => void): () => void {
  window.addEventListener("resize", onChange);
  return () => window.removeEventListener("resize", onChange);
}

export function useLayoutMode(): LayoutMode {
  return useSyncExternalStore(subscribe, () => layoutModeForWidth(window.innerWidth), () => "desktop-docked");
}

/** True on phones narrower than 380 px (e.g. 375 px), where the top bar shows "More" instead of the gear. */
export function useNarrowTopBar(): boolean {
  return useSyncExternalStore(subscribe, () => window.innerWidth < topBarMoreMenuBelowWidth, () => false);
}
