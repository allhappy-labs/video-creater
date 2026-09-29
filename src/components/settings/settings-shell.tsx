import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  ArrowLeft,
  Boxes,
  Cpu,
  Database,
  FolderKanban,
  Plug,
  Settings2,
  Wifi,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  appSettingsCategoryLabels,
  appSettingsTargetKey,
  appSettingsTargetLabel,
  type AppSettingsCategory,
  type AppSettingsTarget,
} from "@/lib/settings/target";

export type SettingsCategory = AppSettingsCategory;

const settingsCategories = [
  { id: "general", label: "General", icon: Settings2 },
  { id: "projects", label: "Projects", icon: FolderKanban },
  { id: "aiModels", label: "AI & Models", icon: Cpu },
  { id: "integrations", label: "Integrations", icon: Plug },
  { id: "remoteAccess", label: "Remote access", icon: Wifi },
  { id: "storage", label: "Storage", icon: Database },
  { id: "advanced", label: "Advanced", icon: Boxes },
] as const satisfies ReadonlyArray<{
  id: AppSettingsCategory;
  label: string;
  icon: LucideIcon;
}>;

function useSettingsTabOrientation() {
  const mediaQuery = "(min-width: 768px)";
  const [isDesktop, setIsDesktop] = useState(() =>
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia(mediaQuery).matches
      : false,
  );

  useEffect(() => {
    if (typeof window.matchMedia !== "function") {
      return;
    }
    const query = window.matchMedia(mediaQuery);
    const update = (event: MediaQueryListEvent) => setIsDesktop(event.matches);
    setIsDesktop(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  return isDesktop ? "vertical" : "horizontal";
}

export interface SettingsShellProps {
  initialCategory: SettingsCategory;
  navigationRequestId?: number;
  navigationTarget?: AppSettingsTarget;
  focusTargetReady?: boolean;
  onBack: () => void | Promise<void>;
  projectRoot?: string | null;
  notice?: ReactNode;
  children: (activeCategory: SettingsCategory) => ReactNode;
}

export function SettingsShell({
  initialCategory,
  navigationRequestId = 0,
  navigationTarget,
  focusTargetReady = true,
  onBack,
  projectRoot = null,
  notice = null,
  children,
}: SettingsShellProps) {
  const [activeCategory, setActiveCategory] =
    useState<SettingsCategory>(initialCategory);
  const categoryButtonRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const headingRef = useRef<HTMLHeadingElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const consumedNavigationRequestsRef = useRef(new Set<string>());
  const cancelledNavigationRequestsRef = useRef(new Set<string>());
  const [announcement, setAnnouncement] = useState("");
  const tabOrientation = useSettingsTabOrientation();
  const activeCategoryLabel = appSettingsCategoryLabels[activeCategory];
  const navigationTargetKey = navigationTarget
    ? appSettingsTargetKey(navigationTarget)
    : null;
  const navigationTargetCategory = navigationTarget?.category ?? null;
  const navigationRequestKey = navigationTargetKey
    ? `${navigationRequestId}:${navigationTargetKey}`
    : null;
  const navigationTargetAnnouncement = navigationTarget
    ? `Opened ${appSettingsTargetLabel(navigationTarget)}`
    : "";

  useEffect(() => {
    setActiveCategory(navigationTarget?.category ?? initialCategory);
  }, [initialCategory, navigationRequestId, navigationTarget?.category]);

  useEffect(() => {
    setAnnouncement("");
  }, [navigationRequestKey]);

  useEffect(() => {
    if (
      !navigationRequestKey ||
      !navigationTargetKey ||
      !focusTargetReady ||
      navigationTargetCategory !== activeCategory ||
      consumedNavigationRequestsRef.current.has(navigationRequestKey) ||
      cancelledNavigationRequestsRef.current.has(navigationRequestKey)
    ) {
      return;
    }

    const specificTarget = navigationTargetKey !== navigationTargetCategory;
    let frame: number | null = null;
    let observer: MutationObserver | null = null;

    const resolveTarget = () => {
      if (!specificTarget) return headingRef.current;
      const candidates = panelRef.current?.querySelectorAll<HTMLElement>(
        "[data-settings-target]",
      );
      return [...(candidates ?? [])].find(
        (candidate) => candidate.dataset.settingsTarget === navigationTargetKey,
      ) ?? null;
    };

    const focusResolvedTarget = () => {
      frame = null;
      if (
        consumedNavigationRequestsRef.current.has(navigationRequestKey) ||
        cancelledNavigationRequestsRef.current.has(navigationRequestKey)
      ) {
        return;
      }
      const destination = resolveTarget();
      if (!destination?.isConnected) return;
      const categoryIndex = settingsCategories.findIndex(
        (category) => category.id === navigationTargetCategory,
      );
      categoryButtonRefs.current[categoryIndex]?.scrollIntoView?.({
        block: "nearest",
        inline: "nearest",
      });
      destination.scrollIntoView?.({ block: "nearest" });
      destination.focus({ preventScroll: true });
      if (document.activeElement !== destination) return;
      consumedNavigationRequestsRef.current.add(navigationRequestKey);
      setAnnouncement(navigationTargetAnnouncement);
      observer?.disconnect();
    };

    const scheduleFocus = () => {
      if (frame !== null) return;
      frame = window.requestAnimationFrame(focusResolvedTarget);
    };

    scheduleFocus();
    if (specificTarget && panelRef.current) {
      observer = new MutationObserver(scheduleFocus);
      observer.observe(panelRef.current, { childList: true, subtree: true });
    }

    return () => {
      observer?.disconnect();
      if (frame !== null) window.cancelAnimationFrame(frame);
    };
  }, [
    activeCategory,
    focusTargetReady,
    navigationRequestKey,
    navigationTargetAnnouncement,
    navigationTargetCategory,
    navigationTargetKey,
  ]);

  function selectCategoryFromUser(category: SettingsCategory) {
    if (navigationRequestKey) {
      cancelledNavigationRequestsRef.current.add(navigationRequestKey);
    }
    setActiveCategory(category);
  }

  function moveCategoryFocus(nextIndex: number) {
    const normalizedIndex =
      (nextIndex + settingsCategories.length) % settingsCategories.length;
    const category = settingsCategories[normalizedIndex];
    if (!category) return;
    selectCategoryFromUser(category.id);
    categoryButtonRefs.current[normalizedIndex]?.focus();
  }

  return (
    <section
      data-testid="settings-shell"
      className="flex h-full min-h-0 flex-col bg-background md:flex-row"
    >
      <aside className="shrink-0 border-b bg-muted/15 md:w-[220px] md:border-b-0 md:border-r">
        <div className="flex h-12 items-center gap-2 border-b px-3">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={onBack}
            aria-label="Back"
          >
            <ArrowLeft className="h-4 w-4" aria-hidden="true" />
          </Button>
          <span className="text-sm font-semibold">Settings</span>
        </div>
        <nav
          role="tablist"
          aria-label="Settings categories"
          data-settings-acceptance-category-rail
          aria-orientation={tabOrientation}
          data-overflow-affordance="horizontal-scroll"
          className="flex gap-1 overflow-x-auto p-2 pr-8 [scrollbar-width:thin] md:flex-col md:overflow-visible md:p-3"
        >
          {settingsCategories.map((category, index) => {
            const Icon = category.icon;
            const selected = category.id === activeCategory;
            return (
              <button
                key={category.id}
                id={`settings-tab-${category.id}`}
                ref={(node) => {
                  categoryButtonRefs.current[index] = node;
                }}
                type="button"
                role="tab"
                data-settings-category-id={category.id}
                aria-selected={selected}
                aria-controls="settings-panel"
                tabIndex={selected ? 0 : -1}
                className={`flex h-9 shrink-0 items-center gap-2 rounded-md px-3 text-left text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring md:w-full ${
                  selected
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-accent/60 hover:text-foreground"
                }`}
                onClick={() => selectCategoryFromUser(category.id)}
                onKeyDown={(event) => {
                  if (
                    (tabOrientation === "vertical" &&
                      event.key === "ArrowDown") ||
                    (tabOrientation === "horizontal" &&
                      event.key === "ArrowRight")
                  ) {
                    event.preventDefault();
                    moveCategoryFocus(index + 1);
                  } else if (
                    (tabOrientation === "vertical" && event.key === "ArrowUp") ||
                    (tabOrientation === "horizontal" &&
                      event.key === "ArrowLeft")
                  ) {
                    event.preventDefault();
                    moveCategoryFocus(index - 1);
                  } else if (event.key === "Home") {
                    event.preventDefault();
                    moveCategoryFocus(0);
                  } else if (event.key === "End") {
                    event.preventDefault();
                    moveCategoryFocus(settingsCategories.length - 1);
                  }
                }}
              >
                <Icon className="h-4 w-4 shrink-0" aria-hidden="true" />
                <span className="truncate">{category.label}</span>
              </button>
            );
          })}
        </nav>
      </aside>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <header className="flex min-h-16 items-center gap-4 border-b px-5 py-3 md:px-8">
          <div className="min-w-0">
            <h1
              ref={headingRef}
              tabIndex={-1}
              className="truncate rounded-sm text-xl font-light tracking-tight outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {activeCategoryLabel}
            </h1>
            <p className="truncate text-xs text-muted-foreground">
              {projectRoot ? `Active project: ${projectRoot}` : "Global preferences · No project open"}
            </p>
          </div>
        </header>

        <span
          role="status"
          aria-live="polite"
          aria-atomic="true"
          className="sr-only"
        >
          {announcement}
        </span>

        <div
          ref={panelRef}
          id="settings-panel"
          role="tabpanel"
          aria-label={`${activeCategoryLabel} settings`}
          aria-labelledby={`settings-tab-${activeCategory}`}
          className="min-h-0 flex-1 space-y-3 overflow-auto p-5 md:px-8 md:py-6"
        >
          {notice}
          {children(activeCategory)}
        </div>
      </div>
    </section>
  );
}
