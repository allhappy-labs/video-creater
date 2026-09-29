import { expect, test, type Page } from "@playwright/test";
import { openSampleEditor } from "./support/editor-fixture";

const screenshotDir = "output/settings-agent";

function agentSection(page: Page) {
  return page.locator('[data-settings-target="advanced:agent"]');
}

async function openAdvancedTab(page: Page) {
  const tabs = page.getByRole("tablist", { name: "Settings categories" });
  await expect(tabs).toBeVisible();
  await tabs.getByRole("tab", { name: "Advanced" }).click();
  const agent = agentSection(page);
  await expect(agent).toBeVisible();
  return agent;
}

/** App settings > Advanced, reached the way a user reaches it: the editor gear menu. */
async function openAdvancedSettings(page: Page) {
  await page.getByRole("button", { name: "Editor menu" }).click();
  await page.getByRole("menu").getByRole("menuitem", { name: "App settings" }).click();
  return openAdvancedTab(page);
}

/** The same section after a reload, which lands back on the project home. */
async function reopenAdvancedSettings(page: Page) {
  await page.reload();
  await expect(page.getByRole("main", { name: "Project home" })).toBeVisible();
  await page.getByRole("button", { name: "Model settings", exact: true }).click();
  return openAdvancedTab(page);
}

test("choosing an agent backend in settings survives a reload", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);

  const agent = await openAdvancedSettings(page);
  const backend = agent.getByLabel("Agent backend");
  await expect(backend).toHaveValue("automatic");
  await expect(agent.getByText("Automatic uses whichever agent is ready.")).toBeVisible();

  await agent.getByRole("button", { name: "Check agents" }).click();
  // Both agents are ready in the fixture, so this is the whole of the Automatic rule: it
  // prefers Claude, which runs on the user's own subscription with nothing bundled.
  await expect(agent.getByText("Automatic is using Claude.")).toBeVisible();
  await expect(
    agent.getByText(
      "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed.",
    ),
  ).toBeVisible();
  await expect(agent.getByText("Initialize handshake succeeded.")).toBeVisible();

  await backend.selectOption("claude");
  await expect(backend).toHaveValue("claude");
  await expect(agent.getByLabel("Claude model")).toHaveValue("sonnet");
  await expect(agent.getByText("Turns will use Claude.")).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/advanced-agent.png` });

  const reopened = await reopenAdvancedSettings(page);
  await expect(reopened.getByLabel("Agent backend")).toHaveValue("claude");

  await reopened.getByLabel("Agent backend").selectOption("automatic");
  await expect(
    (await reopenAdvancedSettings(page)).getByLabel("Agent backend"),
  ).toHaveValue("automatic");
  expect(errors).toEqual([]);
});

test("an unavailable agent sends the user to Agent settings", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const errors = await openSampleEditor(page);

  const panel = page.getByRole("tabpanel", { name: "AI" });
  await panel.getByRole("textbox", { name: "Describe an edit" }).fill("Tighten the pacing");
  await panel.getByRole("textbox", { name: "Describe an edit" }).press("Enter");

  const missing = panel.getByRole("region", { name: "AI agent unavailable" });
  await expect(missing).toContainText("No AI agent is available.");
  await missing.getByRole("button", { name: "Open Agent settings" }).click();

  await expect(page.locator('[data-settings-target="advanced:agent"]')).toBeVisible();
  await page.screenshot({ path: `${screenshotDir}/unavailable-deep-link.png` });
  expect(errors).toEqual([]);
});
