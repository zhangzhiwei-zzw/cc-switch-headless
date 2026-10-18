import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import {
  checkCoverage,
  collectInvokes,
  loadSurface,
} from "./check-command-coverage.mjs";
const contract = JSON.parse(
  fs.readFileSync(
    new URL("../../src/web-shims/command-contract.json", import.meta.url),
    "utf8",
  ),
);
test("all current frontend commands are reviewed", () =>
  assert.deepEqual(checkCoverage(contract, loadSurface()), []));
test("removing the Pi state handler fails provider coverage", () => {
  const surface = loadSurface();
  surface.handlers.delete("get_pi_current_state");
  assert.ok(
    checkCoverage(contract, surface).some((error) =>
      error.includes(
        "pageProviders: missing server command get_pi_current_state",
      ),
    ),
  );
});
test("an upstream frontend command cannot silently bypass review", () => {
  const surface = loadSurface();
  surface.calls.set("new_upstream_command", ["src/lib/api/pi.ts"]);
  assert.ok(
    checkCoverage(contract, surface).some((error) =>
      error.includes("Unclassified frontend command new_upstream_command"),
    ),
  );
});
test("browser commands must have an adapter", () => {
  const surface = loadSurface();
  surface.browser.delete("open_file_dialog");
  assert.ok(
    checkCoverage(contract, surface).some((error) =>
      error.includes("Missing browser adapter: open_file_dialog"),
    ),
  );
});

test("aliases, namespaces and template literals are audited; dynamic commands are rejected", () => {
  const found = collectInvokes(
    `import { invoke as call } from "@tauri-apps/api/core";
import * as core from "@tauri-apps/api/core";
call("get_pi_current_state"); core.invoke(\`get_settings\`); call(command);`,
    "src/example.ts",
  );
  assert.deepEqual(found.commands, ["get_pi_current_state", "get_settings"]);
  assert.equal(found.dynamic.length, 1);
});
