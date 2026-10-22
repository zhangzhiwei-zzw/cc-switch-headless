import { test, expect, type Page } from "@playwright/test";
import contract from "../../src/web-shims/command-contract.json" with { type: "json" };
import { TestServer } from "./server";

const first = new TestServer(15870);
const second = new TestServer(15871);

test.beforeAll(async () => {
  test.setTimeout(180_000);
  await first.start();
  await second.start();
});
test.afterAll(async () => {
  await first.stop();
  await second.stop();
});
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("language", "en"));
  await page.route("https://**", (route) => route.abort());
});

async function login(page: Page, server = first) {
  await page.goto(server.url);
  await expect(
    page.getByRole("heading", { name: "Sign in to CC Switch" }),
  ).toBeVisible();
  await page.getByLabel("Access token").fill(server.token);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("navigation")).toBeVisible();
}

test("unauthorized users sign in without exposing a token in the URL", async ({
  page,
}) => {
  await page.goto(first.url);
  await expect(
    page.getByRole("heading", { name: "Sign in to CC Switch" }),
  ).toBeVisible();
  await expect(page.getByRole("navigation")).toHaveCount(0);
  await page.getByLabel("Access token").fill("incorrect-token");
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText("incorrect");
  await page.getByLabel("Access token").fill(first.token);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("navigation")).toBeVisible();
  expect(page.url()).not.toContain(first.token);
});

test("runtime capabilities satisfy every advertised feature and app", async ({
  request,
}) => {
  const response = await request.get(`${first.url}/api/capabilities`, {
    headers: { Authorization: `Bearer ${first.token}` },
  });
  expect(response.ok()).toBeTruthy();
  const caps = await response.json();
  for (const [feature, spec] of Object.entries(contract.features)) {
    if ("enabled" in spec && spec.enabled === false) {
      expect(caps.features[feature]).toBe(false);
      continue;
    }
    expect(caps.features[feature]).toBe(true);
    for (const cmd of spec.commands) expect(caps.commands).toContain(cmd);
  }
  for (const [app, spec] of Object.entries(contract.apps)) {
    expect(caps.apps[app]).toBe(true);
    for (const cmd of spec.commands) expect(caps.commands).toContain(cmd);
  }
});

test("Pi provider page can read native state", async ({ page }) => {
  await login(page);
  const state = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/invoke") &&
      response.request().postDataJSON()?.cmd === "get_pi_current_state",
  );
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Pi", exact: true })
    .click();
  expect((await (await state).json()).ok).toBe(true);
  await expect(
    page.getByText("E_NOT_IMPLEMENTED", { exact: false }),
  ).toHaveCount(0);
});

test("OpenCode default model can be read and updated through Web", async ({
  page,
}) => {
  await login(page);
  const invoke = async (cmd: string, args: Record<string, string> = {}) =>
    page.evaluate(
      async ({ cmd, args }) => {
        const response = await fetch("/api/invoke", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ cmd, args }),
        });
        return response.json();
      },
      { cmd, args },
    );

  const initial = await invoke("get_opencode_default_model");
  expect(initial.ok).toBe(true);
  const changed = await invoke("set_opencode_default_model", {
    model: "provider/model",
  });
  expect(changed.ok).toBe(true);
  expect((await invoke("get_opencode_default_model")).data).toBe(
    "provider/model",
  );
});

test("capability errors block application and retry recovers", async ({
  page,
}) => {
  await page.route("**/api/capabilities", (route) =>
    route.fulfill({ status: 503, body: "unavailable" }),
  );
  await page.goto(first.url);
  await expect(
    page.getByRole("heading", { name: "Cannot load server capabilities" }),
  ).toBeVisible();
  await expect(page.getByRole("navigation")).toHaveCount(0);
  await page.unroute("**/api/capabilities");
  await page.getByRole("button", { name: "Retry connection" }).click();
  await expect(
    page.getByRole("heading", { name: "Sign in to CC Switch" }),
  ).toBeVisible();
});

test("token rotation invalidates open sessions and allows signing in again", async ({
  page,
  request,
}) => {
  await login(page);
  const response = await request.post(`${first.url}/api/rotate-token`, {
    headers: { Authorization: `Bearer ${first.token}` },
    data: {},
  });
  first.token = (await response.json()).token;
  // SSE and the periodic capability probe detect the expired browser session.
  // A background request may already have removed navigation after rotation.
  await page.evaluate(() =>
    window.dispatchEvent(new Event("cc-switch-web-connection-check")),
  );
  await expect(
    page.getByRole("heading", { name: "Sign in to CC Switch" }),
  ).toBeVisible();
  await page.getByLabel("Access token").fill(first.token);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(page.getByRole("navigation")).toBeVisible();
});

test("connection loss hides cached application data and supports retry", async ({
  page,
}) => {
  await login(page);
  await page.context().setOffline(true);
  await expect(
    page.getByRole("heading", { name: "Cannot connect to server" }),
  ).toBeVisible();
  await expect(page.getByRole("navigation")).toHaveCount(0);
  await page.context().setOffline(false);
  await page.getByRole("button", { name: "Retry connection" }).click();
  await expect(page.getByRole("navigation")).toBeVisible();
});

test("two instances keep independent cookies in the same browser, including restart", async ({
  page,
  context,
}) => {
  await login(page);
  const other = await context.newPage();
  await other.addInitScript(() => localStorage.setItem("language", "en"));
  await other.route("https://**", (route) => route.abort());
  await login(other, second);
  const names = (await context.cookies()).filter((cookie) =>
    cookie.name.startsWith("ccswitch_web_token_"),
  );
  expect(new Set(names.map((cookie) => cookie.name)).size).toBe(2);
  await page.reload();
  await expect(page.getByRole("navigation")).toBeVisible();
  await first.stop();
  await first.start();
  await page.reload();
  await expect(page.getByRole("navigation")).toBeVisible();
  await other.reload();
  await expect(other.getByRole("navigation")).toBeVisible();
});

test("all application pages avoid unsupported commands and hide unavailable actions", async ({
  page,
}) => {
  test.setTimeout(120_000);
  const unsupported: string[] = [];
  const pending: Promise<void>[] = [];
  page.on("response", (response) => {
    if (!response.url().endsWith("/api/invoke")) return;
    pending.push(
      response
        .json()
        .then((body) => {
          if (String(body.error ?? "").includes("E_NOT_IMPLEMENTED"))
            unsupported.push(body.error);
        })
        .catch(() => {}),
    );
  });
  await login(page);
  for (const name of [
    "Claude Code",
    "Claude Desktop",
    "Codex",
    "Gemini CLI",
    "Grok Build",
    "OpenCode",
    "OpenClaw",
    "Hermes",
    "Pi",
    "MiniMax Code",
    "Antigravity",
  ]) {
    await page
      .getByRole("navigation")
      .getByRole("button", { name, exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Stack", exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("tab", { name: /Workspace|Memory|OpenClaw config/ }),
    ).toHaveCount(0);
    await page
      .getByRole("button", { name: "Add Provider", exact: true })
      .first()
      .click();
    await expect(
      page.getByRole("heading", { name: /Add.*provider/i }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: /^Back to (presets|providers)$/ })
      .click();
    if (
      await page.getByRole("heading", { name: /Add.*provider/i }).isVisible()
    ) {
      await page
        .getByRole("button", { name: "Back to providers", exact: true })
        .click();
    }
    await expect(
      page.getByRole("heading", { name: /Add.*provider/i }),
    ).toHaveCount(0);
  }
  await expect(page.getByRole("note")).toContainText(
    "Web can edit Antigravity",
  );
  await expect(
    page.getByRole("button", { name: /Open terminal/i }),
  ).toHaveCount(0);
  await Promise.all(pending);
  expect(unsupported).toEqual([]);
});

test("Web settings pages do not invoke desktop-only configuration commands", async ({
  page,
}) => {
  const unsupported: string[] = [];
  const pending: Promise<void>[] = [];
  page.on("response", (response) => {
    if (response.url().endsWith("/api/invoke"))
      pending.push(
        response
          .json()
          .then((body) => {
            if (String(body.error ?? "").includes("E_NOT_IMPLEMENTED"))
              unsupported.push(body.error);
          })
          .catch(() => {}),
      );
  });
  await login(page);
  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Settings", exact: true })
    .click();
  for (const name of [
    "General",
    "App config",
    "Local routing",
    "Data",
    "About",
  ]) {
    await page
      .getByRole("navigation")
      .getByRole("button", { name, exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name, exact: true }),
    ).toBeVisible();
  }
  await expect(
    page
      .getByRole("navigation")
      .getByRole("button", { name: "Network", exact: true }),
  ).toHaveCount(0);
  await Promise.all(pending);
  expect(unsupported).toEqual([]);
});
