import { expect, test, type Page } from "@playwright/test";
import { acceptanceViewports, clipCount, collectConsoleErrors, horizontalOverflow, mod, openAcceptanceEditor, timelineClips } from "./support/editor";

// Spec acceptance flow 5 on desktop with the keyboard alone (plan 09 Task 4): select a clip on the
// timeline, set its opacity and a keyframe in Properties, split at the playhead, then undo and redo.
// Every tab stop on the way must show a visible focus ring. The sample opens with "Opening clip"
// (0–4 s) first on Video 1.

const desktop = acceptanceViewports[0];
const screenshotDir = "output/editor-acceptance";

interface FocusState {
  readonly description: string;
  readonly focusVisible: boolean;
  readonly ringVisible: boolean;
  readonly inProperties: boolean;
  readonly inTimelineCanvas: boolean;
  readonly role: string | null;
  readonly name: string;
}

/** The focused element, whether it matches `:focus-visible`, and whether its outline or ring is actually visible. */
function focusState(page: Page): Promise<FocusState> {
  return page.evaluate(() => {
    const element = document.activeElement;
    if (!(element instanceof HTMLElement) || element === document.body) {
      return { description: "document body", focusVisible: false, ringVisible: false, inProperties: false, inTimelineCanvas: false, role: null, name: "" };
    }
    const transparent = (color: string) => color === "transparent" || /^rgba\(.*,\s*0\)$/.test(color);
    const ringed = (target: Element) => {
      const style = getComputedStyle(target);
      const outline = style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0 && !transparent(style.outlineColor);
      // Token rings are box shadows: a colored layer with a spread of at least 2 px (`ring-2`).
      const ring = Array.from(style.boxShadow.matchAll(/(rgba?\([^)]*\))\s+-?[\d.]+px\s+-?[\d.]+px\s+-?[\d.]+px\s+(-?[\d.]+)px/g)).some(
        (layer) => !transparent(layer[1] ?? "transparent") && parseFloat(layer[2] ?? "0") >= 2,
      );
      return outline || ring;
    };
    // The ring may sit on the element, on a part of it (`group-focus-visible:`), or on the box around a
    // borderless field (`focus-within:`).
    const ringVisible = ringed(element) || Array.from(element.querySelectorAll("*")).some(ringed) || (element.parentElement !== null && ringed(element.parentElement));
    const name = element.getAttribute("aria-label") ?? (element.textContent ?? "").trim().slice(0, 40);
    const role = element.getAttribute("role") ?? element.tagName.toLowerCase();
    return {
      description: `${role} “${name}”`,
      focusVisible: element.matches(":focus-visible"),
      ringVisible,
      inProperties: element.closest('[aria-label="Properties"]') !== null,
      inTimelineCanvas: element.closest('[aria-label="Timeline canvas"]') !== null,
      role: element.getAttribute("role"),
      name,
    };
  });
}

/** Presses a key and expects the element it focuses to show a visible `:focus-visible` ring. */
async function press(page: Page, key: string): Promise<FocusState> {
  await page.keyboard.press(key);
  const state = await focusState(page);
  expect.soft({ element: state.description, focusVisible: state.focusVisible, ringVisible: state.ringVisible }).toEqual({ element: state.description, focusVisible: true, ringVisible: true });
  return state;
}

/** Tabs forward until `reached` holds for the focused element. */
async function tabUntil(page: Page, reached: (state: FocusState) => boolean | Promise<boolean>, what: string): Promise<FocusState> {
  for (let presses = 0; presses < 200; presses += 1) {
    await page.keyboard.press("Tab");
    const state = await focusState(page);
    // Tab passes through the browser chrome between the last and first tab stop.
    if (state.description === "document body") continue;
    expect.soft({ element: state.description, focusVisible: state.focusVisible, ringVisible: state.ringVisible }).toEqual({ element: state.description, focusVisible: true, ringVisible: true });
    if (await reached(state)) return state;
  }
  throw new Error(`Tab never reached ${what}`);
}

test("desktop flow 5 by keyboard: select, opacity, keyframe, split, undo and redo", async ({ page }) => {
  test.slow();
  const errors = collectConsoleErrors(page);
  await openAcceptanceEditor(page, desktop);
  const clips = await clipCount(page);
  const opening = page.getByRole("region", { name: "Timeline canvas" }).getByRole("option", { name: /^Opening clip,/ });
  const playhead = page.getByTestId("playhead");

  // Tab into the timeline canvas: its first clip is the tracks' roving tab stop.
  await tabUntil(page, (state) => state.inTimelineCanvas && state.role === "option", "a timeline clip");
  await expect(opening).toBeFocused();
  await press(page, "ArrowRight");
  await expect(page.getByRole("option", { name: /^Restored Edison alternate,/ })).toBeFocused();
  await press(page, "ArrowLeft");
  await expect(opening).toBeFocused();
  await press(page, "Enter");
  await expect(opening).toHaveAttribute("aria-selected", "true");

  // On the way to Properties, the preview scrubber moves the playhead into the clip.
  await tabUntil(page, (state) => state.name === "Preview scrubber", "the preview scrubber");
  for (let steps = 0; steps < 3; steps += 1) await press(page, "Shift+ArrowRight");
  const splitSeconds = Number(await playhead.getAttribute("data-seconds"));
  expect(splitSeconds).toBeGreaterThan(0.5);
  expect(splitSeconds).toBeLessThan(3.5);

  const properties = page.getByRole("complementary", { name: "Properties" });
  const opacity = properties.getByRole("textbox", { name: "Opacity" });
  await tabUntil(page, (state) => state.inProperties, "Properties");
  await tabUntil(page, () => opacity.evaluate((element) => element === document.activeElement), "the Opacity field");
  await page.keyboard.type("50");
  await press(page, "Enter");
  await expect(opacity).toHaveValue("50%");
  await expect(page.getByRole("region", { name: "Preview viewport" }).getByLabel("Timeline video Opening clip")).toHaveCSS("opacity", "0.5");

  const keyframe = properties.getByRole("button", { name: "Add keyframe for Opacity" });
  await tabUntil(page, () => keyframe.evaluate((element) => element === document.activeElement), "the Opacity keyframe button");
  await press(page, "Space");
  await expect(properties.getByRole("button", { name: "Remove keyframe for Opacity" })).toBeVisible();

  // Back to the timeline: the selected clip is the tracks' tab stop again.
  await tabUntil(page, (state) => state.inTimelineCanvas && state.role === "option", "the timeline");
  await expect(opening).toBeFocused();
  await press(page, "s");
  await expect(timelineClips(page)).toHaveCount(clips + 1);
  await expect(opening).toHaveAttribute("aria-label", `Opening clip, 00:00:00, 00:00:0${splitSeconds.toFixed(3)}`);

  const modifier = await mod(page);
  await press(page, `${modifier}+z`);
  await expect(timelineClips(page)).toHaveCount(clips);
  await press(page, `Shift+${modifier}+z`);
  await expect(timelineClips(page)).toHaveCount(clips + 1);

  await page.screenshot({ path: `${screenshotDir}/5-edit-clip-keyboard-desktop.png`, animations: "disabled" });
  expect(await horizontalOverflow(page)).toBeLessThanOrEqual(0);
  expect(errors).toEqual([]);
});
