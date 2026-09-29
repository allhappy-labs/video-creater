# Palmier-First Foundations and Project Home Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish Palmier's neutral visual tokens and rebuild the project home as a 220px navigation shell with compact 150×120 project cards while preserving Video Creater's sample, paths, recents, and recovery behavior.

**Architecture:** Keep `ProjectHome` as a controlled component and move verbose path/recovery controls into one focused dialog owned by that component. Apply shared Palmier color/spacing variables in `index.css`, then use stable test hooks for browser geometry without introducing a second theme system.

**Tech Stack:** React 19, TypeScript, Tailwind CSS, Lucide icons, Vitest, Testing Library, Vite.

---

## File Structure

- Modify `src/index.css`: Palmier neutral background, text, border, accent, and radius variables.
- Create `src/components/workspace/project-path-dialog.tsx`: accessible New/Open/Relink path sheet with focus restoration.
- Create `src/components/workspace/project-path-dialog.test.tsx`: keyboard, validation, submit, cancel, and focus tests.
- Modify `src/components/workspace/project-home.tsx`: 220px sidebar, fluid main area, compact sample/recent cards, and dialog routing.
- Modify `src/components/workspace/project-home.test.tsx`: exact landmarks, geometry hooks, card names, and existing workflow callbacks.
- Modify `src/App.test.tsx`: end-to-end home-to-editor behavior after the shell change.
- Modify `scripts/browser-visual-qa.mjs`: stable home state and 1440×960/390×844 assertions.
- Modify `docs/parity.md`: reopen the stale home visual claim and record exact evidence.

### Task 1: Reopen the home visual claim before changing it

**Files:**
- Modify: `docs/parity.md:408-489`

- [ ] **Step 1: Record the fresh screenshot finding**

Replace broad home visual-closure wording with:

```markdown
| P1 | visual-regression | Project home | Functional sample, recent-project, create/open, relink, and recovery workflows remain, but the current centered dashboard does not match Palmier's 220px navigation shell or 150×120 project tiles. Fresh installed-app reference: `output/parity-audit-2026-07-13/reference/01-home.png`. | Reopened 2026-07-13. |
```

- [ ] **Step 2: Verify the tracker edit**

Run: `rtk git diff --check -- docs/parity.md && rtk rg -n "Project home|visual-regression|01-home" docs/parity.md`

Expected: no whitespace errors; the home row is explicitly reopened and points to the new ignored reference.

- [ ] **Step 3: Commit the tracker correction**

```bash
rtk git add docs/parity.md
rtk git commit -m "docs(parity): reopen project home visual parity"
```

### Task 2: Apply Palmier's shared neutral tokens

**Files:**
- Modify: `src/index.css:6-80`
- Modify: `src/components/workspace/project-home.test.tsx`

- [ ] **Step 1: Add a failing token-contract test**

Read `src/index.css` in the existing Vitest environment and assert exact variables:

```tsx
import { readFileSync } from "node:fs";

it("defines the approved Palmier neutral planes", () => {
  const css = readFileSync(new URL("../../index.css", import.meta.url), "utf8");
  expect(css).toContain("--background: 0 0% 3.9%");
  expect(css).toContain("--card: 0 0% 8.6%");
  expect(css).toContain("--popover: 0 0% 11.8%");
  expect(css).toContain("--muted: 0 0% 17.3%");
  expect(css).toContain("--primary: 39 49% 93%");
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx -t "Palmier neutral planes"`

Expected: FAIL because the current warm brown values remain.

- [ ] **Step 3: Replace only shared color variables**

Use HSL values equivalent to Palmier's source colors:

```css
:root {
  --background: 0 0% 3.9%;
  --foreground: 0 0% 100%;
  --card: 0 0% 8.6%;
  --card-foreground: 0 0% 100%;
  --popover: 0 0% 11.8%;
  --popover-foreground: 0 0% 100%;
  --primary: 39 49% 93%;
  --primary-foreground: 30 10% 8%;
  --secondary: 0 0% 11.8%;
  --secondary-foreground: 0 0% 80%;
  --muted: 0 0% 17.3%;
  --muted-foreground: 0 0% 62%;
  --accent: 0 0% 17.3%;
  --accent-foreground: 0 0% 100%;
  --border: 0 0% 100% / 0.16;
  --input: 0 0% 100% / 0.12;
  --ring: 39 49% 93%;
  --radius: 0.625rem;
}
```

Keep status colors and the existing semantic variable names so every current shadcn/Tailwind consumer remains compatible.

- [ ] **Step 4: Run token and broad component smoke tests**

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx src/App.test.tsx`

Expected: token test passes and existing UI tests do not regress.

- [ ] **Step 5: Commit the token change**

```bash
rtk git add src/index.css src/components/workspace/project-home.test.tsx
rtk git commit -m "style(ui): adopt Palmier neutral planes"
```

### Task 3: Create one accessible project path dialog

**Files:**
- Create: `src/components/workspace/project-path-dialog.tsx`
- Create: `src/components/workspace/project-path-dialog.test.tsx`

- [ ] **Step 1: Write the failing dialog tests**

```tsx
it("submits trimmed paths and restores focus after cancel", () => {
  const onSubmit = vi.fn();
  const onOpenChange = vi.fn();
  const trigger = document.createElement("button");
  document.body.append(trigger);
  trigger.focus();
  const returnFocusRef = { current: trigger };
  const props = {
    mode: "open" as const,
    initialPath: "  /tmp/project  ",
    busy: false,
    error: null,
    returnFocusRef,
    onOpenChange,
    onSubmit,
  };

  const { rerender } = render(<ProjectPathDialog {...props} open />);

  fireEvent.change(screen.getByRole("textbox", { name: "Project folder" }), {
    target: { value: "  /tmp/next-project  " },
  });
  fireEvent.click(screen.getByRole("button", { name: "Open Project" }));
  expect(onSubmit).toHaveBeenCalledWith("/tmp/next-project");

  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  rerender(<ProjectPathDialog {...props} open={false} />);
  expect(trigger).toHaveFocus();
});
```

Add tests for empty-path disabled state, Escape, backdrop behavior, create/open/relink labels, busy state, and inline error text.

- [ ] **Step 2: Run the tests to verify the component is missing**

Run: `rtk pnpm exec vitest run src/components/workspace/project-path-dialog.test.tsx`

Expected: FAIL because `ProjectPathDialog` does not exist.

- [ ] **Step 3: Implement the typed dialog**

```tsx
export type ProjectPathDialogMode = "create" | "open" | "relink";

export interface ProjectPathDialogProps {
  open: boolean;
  mode: ProjectPathDialogMode;
  initialPath: string;
  busy: boolean;
  error: string | null;
  returnFocusRef: RefObject<HTMLElement | null>;
  onOpenChange: (open: boolean) => void;
  onSubmit: (path: string) => void;
}
```

Use `role="dialog"`, `aria-modal="true"`, a labelled path input, Escape close, Tab/Shift+Tab focus cycling, backdrop-self close, and focus restoration. The shell must use `w-[min(440px,calc(100vw-32px))] rounded-[20px] border-white/15 bg-[#161616]/95`.

- [ ] **Step 4: Run dialog tests**

Run: `rtk pnpm exec vitest run src/components/workspace/project-path-dialog.test.tsx`

Expected: all dialog tests PASS.

- [ ] **Step 5: Commit the dialog**

```bash
rtk git add src/components/workspace/project-path-dialog.tsx src/components/workspace/project-path-dialog.test.tsx
rtk git commit -m "feat(home): add focused project path dialog"
```

### Task 4: Rebuild the desktop home shell

**Files:**
- Modify: `src/components/workspace/project-home.tsx`
- Modify: `src/components/workspace/project-home.test.tsx`

- [ ] **Step 1: Add failing landmark and card tests**

```tsx
it("uses Palmier navigation and compact project cards", () => {
  renderProjectHome();

  const sidebar = screen.getByRole("navigation", { name: "Project navigation" });
  expect(sidebar).toHaveAttribute("data-testid", "project-home-sidebar");
  expect(sidebar).toHaveClass("w-[220px]");
  expect(within(sidebar).getByRole("button", { name: "New Project" })).toBeVisible();
  expect(within(sidebar).getByRole("button", { name: "Open Project" })).toBeVisible();
  expect(within(sidebar).getByRole("button", { name: "Settings" })).toBeVisible();

  expect(screen.getByRole("heading", { name: "Welcome to Video Creater" })).toHaveClass(
    "text-[28px]",
    "font-light",
  );
  for (const card of screen.getAllByTestId("project-card")) {
    expect(card).toHaveClass("h-[120px]", "w-[150px]");
  }
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx -t "Palmier navigation"`

Expected: FAIL because the current page is a centered dashboard with a right action column.

- [ ] **Step 3: Implement the split shell**

The top-level structure must be:

```tsx
<main aria-label="Project home" className="flex min-h-screen overflow-hidden bg-[#0a0a0a] text-white">
  <nav
    aria-label="Project navigation"
    data-testid="project-home-sidebar"
    className="hidden w-[220px] shrink-0 flex-col border-r border-white/10 bg-[#1e1e1e]/80 px-3 pb-4 pt-12 md:flex"
  >
    {navigationActions}
    <div className="mt-auto">{settingsAction}</div>
  </nav>
  <section className="min-w-0 flex-1 overflow-y-auto px-6 py-8">
    <h1 className="text-[28px] font-light tracking-[-0.5px]">Welcome to Video Creater</h1>
    {sampleSection}
    {recentSection}
  </section>
</main>
```

Use Lucide `Plus`, `FolderOpen`, and `Settings` icons in 16px frames. Navigation rows use 13px labels, 8px icon gap, 8px horizontal padding, and 6px vertical padding.

- [ ] **Step 4: Replace verbose cards with accessible overlay cards**

```tsx
<button
  type="button"
  data-testid="project-card"
  aria-label={`Open project ${project.name}`}
  className="group relative h-[120px] w-[150px] overflow-hidden rounded-xl border border-white/10 bg-[#1e1e1e] text-left"
  onClick={() => onOpenProject(project)}
>
  {thumbnail}
  <span className="absolute inset-x-0 bottom-0 grid min-h-[60px] content-end bg-gradient-to-t from-black/90 to-transparent px-3 pb-2">
    <span className="truncate text-[13px] font-medium text-white">{project.name}</span>
    {projectStatus}
  </span>
</button>
```

Keep full paths, timestamps, missing status, relink, remove, and errors in the accessible name/description, context menu, missing overlay, and dialog rather than deleting them.

- [ ] **Step 5: Route sidebar actions through the dialog**

`New Project`, `Open Project`, and missing-card Relink set `{ mode, initialPath, recentProjectId }` state and open `ProjectPathDialog`. New submits to `onCreateProject(path)`. Open and Relink submit to `onOpenProjectFolder(path)`. Available recent cards continue to call `onOpenProject(entry)` directly. Settings keeps the existing `onOpenModelSettings` callback.

- [ ] **Step 6: Run all home tests**

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx`

Expected: all sample, recents, create/open, relink/remove, error, focus, and new geometry tests PASS.

- [ ] **Step 7: Commit the desktop home**

```bash
rtk git add src/components/workspace/project-home.tsx src/components/workspace/project-home.test.tsx
rtk git commit -m "feat(home): match Palmier project navigation"
```

### Task 5: Preserve narrow home behavior and App integration

**Files:**
- Modify: `src/components/workspace/project-home.tsx`
- Modify: `src/components/workspace/project-home.test.tsx`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Add failing narrow navigation tests**

```tsx
it("keeps every project action reachable through narrow navigation", () => {
  mockMatchMedia({ matches: true, media: "(max-width: 767px)" });
  renderProjectHome();

  fireEvent.click(screen.getByRole("button", { name: "Project menu" }));
  const menu = screen.getByRole("menu", { name: "Project actions" });
  expect(within(menu).getByRole("menuitem", { name: "New Project" })).toBeVisible();
  expect(within(menu).getByRole("menuitem", { name: "Open Project" })).toBeVisible();
  expect(within(menu).getByRole("menuitem", { name: "Settings" })).toBeVisible();
});
```

- [ ] **Step 2: Run the test to verify failure**

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx -t "narrow navigation"`

Expected: FAIL because the narrow project menu does not exist.

- [ ] **Step 3: Add a compact 44px narrow bar**

Below `md`, render the title and one `Project menu` button in a 44px top bar. Its menu reuses the same action descriptors as the desktop sidebar; do not duplicate callback logic. Keep cards 150×120 and allow the main grid to wrap without horizontal overflow.

- [ ] **Step 4: Run home and App workflows**

Run: `rtk pnpm exec vitest run src/components/workspace/project-home.test.tsx src/App.test.tsx`

Expected: all tests PASS, including bundled sample, persisted recents, settings navigation, open/create, and editor transition.

- [ ] **Step 5: Run lint and build**

Run: `rtk pnpm lint && rtk pnpm build && rtk git diff --check`

Expected: all commands exit 0.

- [ ] **Step 6: Commit narrow integration**

```bash
rtk git add src/components/workspace/project-home.tsx src/components/workspace/project-home.test.tsx src/App.test.tsx
rtk git commit -m "fix(home): preserve compact project actions"
```

### Task 6: Capture and verify the matched home state

**Files:**
- Modify: `scripts/browser-visual-qa.mjs`
- Modify: `docs/parity.md`
- Evidence: `output/parity-audit-2026-07-13/home/`

- [ ] **Step 1: Add exact browser geometry assertions**

Add these deterministic states to `scripts/browser-visual-qa.mjs`:

```js
{ surface: "home", viewport: "desktop", width: 1440, height: 960, state: "palmier" },
{ surface: "home", viewport: "narrow", width: 390, height: 844, state: "palmier" },
{ surface: "home", viewport: "desktop", width: 1440, height: 960, state: "missing-recent" },
```

For the desktop home scenario, assert sidebar width 220±1px, first card 150×120±1px, main left edge equals sidebar right edge, and first content starts 24±1px after the main edge. For 390×844, require zero root/page overflow and New/Open/Settings keyboard reachability through the project menu.

- [ ] **Step 2: Run the deterministic home captures**

With Vite retained at `127.0.0.1:1420`, run:

```bash
rtk pnpm visual:qa:browser -- --url http://127.0.0.1:1420 --out output/parity-audit-2026-07-13/home
```

Expected: writes non-empty desktop/narrow normal and missing-recent home screenshots and exits 0.

- [ ] **Step 3: Inspect Palmier and Video Creater together**

Run:

```bash
rtk ffmpeg -y \
  -i output/parity-audit-2026-07-13/reference/01-home.png \
  -i output/parity-audit-2026-07-13/home/home-palmier-desktop.png \
  -filter_complex '[0:v]scale=1440:960:force_original_aspect_ratio=decrease,pad=1440:960:(ow-iw)/2:(oh-ih)/2:color=black[left];[1:v]scale=1440:960:force_original_aspect_ratio=decrease,pad=1440:960:(ow-iw)/2:(oh-ih)/2:color=black[right];[left][right]hstack=inputs=2' \
  -frames:v 1 output/parity-audit-2026-07-13/home/palmier-video-creater-home-side-by-side.png
```

Expected: a non-empty 2880×960 comparison PNG. Inspect it and reject the slice if sidebar/card geometry, title hierarchy, plane separation, padding, radius, crop, or missing-project state visibly differs beyond the approved native-integration boundary. Plan 05 later incorporates the passed capture into the complete self-contained board.

- [ ] **Step 4: Update the tracker without overclaiming**

Mark the home row complete only after the comparison review records `pass`. Record Google account sign-in as a missing Palmier capability and list bundled offline sample, explicit paths, relink/remove, and missing-folder recovery as preserved Video Creater-only behavior.

- [ ] **Step 5: Commit evidence wording**

```bash
rtk git add scripts/browser-visual-qa.mjs docs/parity.md
rtk git commit -m "docs(parity): verify Palmier project home"
```
