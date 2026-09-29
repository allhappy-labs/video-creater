// WebDriver helpers for the Linux desktop smoke run (tauri-driver + WebKitWebDriver). The module has
// no top-level side effects: `createDriver` returns helpers bound to one WebDriver port and session.

import { writeFileSync } from "node:fs";
import { createConnection } from "node:net";
import { join } from "node:path";

export function waitForPort(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolvePort, reject) => {
    const attempt = () => {
      const socket = createConnection({ port, host: "127.0.0.1" });
      socket.once("connect", () => {
        socket.end();
        resolvePort();
      });
      socket.once("error", () => {
        socket.destroy();
        if (Date.now() > deadline) reject(new Error(`port ${port} did not open`));
        else setTimeout(attempt, 250);
      });
    };
    attempt();
  });
}

export const sleep = (ms) => new Promise((resolveSleep) => setTimeout(resolveSleep, ms));

export async function poll(read, accept, timeoutMs, intervalMs = 1000) {
  const deadline = Date.now() + timeoutMs;
  let last;
  for (;;) {
    last = await read();
    if (await accept(last)) return last;
    if (Date.now() > deadline) throw new Error(`timed out; last value ${String(JSON.stringify(last)).slice(0, 2000)}`);
    await sleep(intervalMs);
  }
}

// A webview whose main thread is blocked never answers; fail the step instead of hanging the run.
const webdriverTimeoutMs = 120_000;
const elementKey = "element-6066-11e4-a23c-4f7ab97d86b1";

// WebDriver key codes: https://www.w3.org/TR/webdriver2/#keyboard-actions
export const keys = { control: "", delete: "", enter: "", escape: "" };

export function createDriver({ port = 4444, outDir }) {
  const driver = { session: undefined, keys, poll, sleep };

  driver.webdriver = async (method, path, body) => {
    const response = await fetch(`http://127.0.0.1:${port}${path}`, {
      method,
      headers: { "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(webdriverTimeoutMs),
    });
    const payload = await response.json();
    if (!response.ok) throw new Error(`${method} ${path}: ${JSON.stringify(payload.value)}`);
    return payload.value;
  };

  driver.startSession = async (capabilities) => {
    const created = await driver.webdriver("POST", "/session", { capabilities });
    driver.session = created.sessionId;
    return driver.session;
  };

  driver.endSession = async () => {
    if (driver.session) await driver.webdriver("DELETE", `/session/${driver.session}`).catch(() => {});
  };

  driver.screenshot = async (name) => {
    const png = await driver.webdriver("GET", `/session/${driver.session}/screenshot`);
    const path = join(outDir, `${name}.png`);
    writeFileSync(path, Buffer.from(png, "base64"));
    return path;
  };

  driver.invoke = async (command, args = {}) => {
    const result = await driver.webdriver("POST", `/session/${driver.session}/execute/async`, {
      script: `const done = arguments[arguments.length - 1];
        window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1])
          .then((value) => done({ ok: true, value }), (error) => done({ ok: false, error: String(error?.message ?? JSON.stringify(error)) }));`,
      args: [command, args],
    });
    if (!result.ok) throw new Error(`${command}: ${result.error}`);
    return result.value;
  };

  driver.execute = (script, args = []) => driver.webdriver("POST", `/session/${driver.session}/execute/sync`, { script, args });

  driver.executeAsync = (script, args = []) => driver.webdriver("POST", `/session/${driver.session}/execute/async`, { script, args });

  driver.find = async (xpath, timeoutMs = 30_000) => {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      try {
        const element = await driver.webdriver("POST", `/session/${driver.session}/element`, { using: "xpath", value: xpath });
        return element[elementKey] ?? element.ELEMENT ?? Object.values(element)[0];
      } catch {
        if (Date.now() > deadline) throw new Error(`element not found: ${xpath}`);
        await sleep(500);
      }
    }
  };

  driver.clickElement = (element) => driver.webdriver("POST", `/session/${driver.session}/element/${element}/click`, {});

  driver.click = async (xpath) => driver.clickElement(await driver.find(xpath));

  driver.attribute = (element, name) => driver.webdriver("GET", `/session/${driver.session}/element/${element}/attribute/${name}`);

  // Presses the keys down in order and releases them in reverse, like a chord typed by a person.
  driver.pressKeys = async (...values) => {
    const actions = [...values.map((value) => ({ type: "keyDown", value })), ...values.toReversed().map((value) => ({ type: "keyUp", value }))];
    await driver.webdriver("POST", `/session/${driver.session}/actions`, { actions: [{ type: "key", id: "keyboard", actions }] });
    await driver.webdriver("DELETE", `/session/${driver.session}/actions`);
  };

  driver.type = async (xpath, text) => {
    const element = await driver.find(xpath);
    await driver.webdriver("POST", `/session/${driver.session}/element/${element}/clear`, {});
    await driver.webdriver("POST", `/session/${driver.session}/element/${element}/value`, { text });
  };

  /**
   * Replaces the text of a React-controlled input. WebDriver's element clear leaves the component's
   * own state behind, so the field is selected with Ctrl+A and typed over instead.
   */
  driver.replaceText = async (xpath, text) => {
    const element = await driver.find(xpath);
    await driver.clickElement(element);
    await driver.pressKeys(keys.control, "a");
    await driver.webdriver("POST", `/session/${driver.session}/element/${element}/value`, { text });
  };

  return driver;
}
