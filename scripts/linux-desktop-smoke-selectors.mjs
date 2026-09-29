// Accessible names of the redesigned editor (src/editor/**) and the project home, plus the small
// navigation helpers the Linux desktop smoke steps share. Helpers take the driver from
// linux-desktop-smoke-driver.mjs.

export const projectHome = "//main[@aria-label='Project home']";
export const editorWorkspace = "//main[@aria-label='Video editor workspace']";
export const exportButton = `${editorWorkspace}//header//button[normalize-space()='Export']`;
export const exportPopover = "//*[@role='dialog' and @aria-label='Export']";
export const editorMenu = `${editorWorkspace}//header//button[@aria-label='Editor menu']`;
export const backgroundTasks = `${editorWorkspace}//header//button[@aria-label='Background tasks']`;
export const settingsTab = (label) => `//button[@role='tab' and normalize-space()='${label}']`;
export const editorTab = (label) =>
  `${editorWorkspace}//*[@role='tablist' and @aria-label='Editor tools']//button[@role='tab' and normalize-space()='${label}']`;
export const exportChoice = (label) => `${exportPopover}//*[@role='radio' and normalize-space()='${label}']`;

// The AI tab (src/editor/panels/ai/**).
export const aiPanel = `${editorWorkspace}//*[@role='tabpanel' and @aria-label='AI']`;
export const composer = "//textarea[@aria-label='Describe an edit']";
export const autoApplySwitch = "//button[@role='switch' and @aria-label='Auto-apply safe edits']";
export const appliedCard = "//article[starts-with(@aria-label, 'Applied to ')]";
export const reviewCard = "//article[@aria-label='Needs your review']";
export const undoneCard = "//article[@aria-label='Undone']";
/** Shown in place of a turn's card when the Codex agent can't be reached. */
export const agentUnavailableSection = "AI agent unavailable";
export const resultPreview = "//*[@role='group' and @aria-label='Result preview']";
/** The conversation's `position`-th turn card (1-based), in document order, counting agent-unavailable sections. */
export const conversationCard = (position) =>
  `(${editorWorkspace}//ol[@aria-label='Conversation']//*[self::article or self::section[@aria-label='AI agent unavailable']])[${position}]`;

export async function waitForEditor(driver, timeoutMs = 60_000) {
  await driver.find(editorWorkspace, timeoutMs);
  await driver.poll(() => driver.execute(`return document.querySelector("main[aria-label='Project home']") === null;`), Boolean, timeoutMs);
}

export async function openSampleEditor(driver) {
  await driver.execute("window.location.reload();");
  await driver.find(projectHome, 60_000);
  await driver.click("//button[@aria-label='Open sample']");
  await waitForEditor(driver);
  await driver.find(exportButton, 60_000);
}

// The Export button opens the editor's single export popover.
export async function openExportPopover(driver) {
  if (!(await driver.execute(`return document.querySelector("[role='dialog'][aria-label='Export']") !== null;`))) {
    await driver.click(exportButton);
  }
  await driver.find(exportPopover);
  await driver.sleep(500);
}

// The top-bar tasks pill's accessible description, e.g. "Rendering · 40%" or "Export complete".
export async function backgroundTasksLabel(driver) {
  return driver.execute(`const button = document.querySelector("main[aria-label='Video editor workspace'] header button[aria-label='Background tasks']");
    const description = button && document.getElementById(button.getAttribute('aria-describedby'));
    return description ? description.textContent.trim() : null;`);
}

// Settings open from the project home ("Settings") or from the editor's gear "Editor menu" → "App settings".
export async function openSettings(driver) {
  if (await driver.execute("return [...document.querySelectorAll('button[role=tab]')].some((button) => button.textContent.trim() === 'Advanced');")) {
    return;
  }
  if (await driver.execute(`return document.querySelector("main[aria-label='Video editor workspace']") !== null;`)) {
    await driver.click(editorMenu);
    await driver.click("//*[@role='menuitem' and normalize-space()='App settings']");
  } else {
    await driver.click("//button[@aria-label='Model settings']");
  }
  await driver.find(settingsTab("Advanced"), 30_000);
}
