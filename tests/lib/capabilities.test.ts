import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  initializeCapabilities,
  resetCapabilitiesForTest,
  supports,
} from "@/lib/capabilities";
import {
  getWebConnectionStatus,
  setWebConnectionStatus,
} from "@/lib/webConnection";
import contract from "@/web-shims/command-contract.json";

function payload() {
  return {
    mode: "web",
    features: Object.fromEntries(
      Object.entries(contract.features).map(([key, spec]) => [
        key,
        !("enabled" in spec && spec.enabled === false),
      ]),
    ),
    commands: [
      ...new Set([
        ...Object.values(contract.features).flatMap((spec) => spec.commands),
        ...Object.values(contract.apps).flatMap((spec) => spec.commands),
      ]),
    ],
  };
}

describe("capabilities", () => {
  beforeEach(() => {
    resetCapabilitiesForTest(null);
    setWebConnectionStatus("checking");
    delete (globalThis as { isTauri?: boolean }).isTauri;
  });
  afterEach(() => vi.unstubAllGlobals());

  it("preserves unrestricted desktop behavior without fetching", async () => {
    (globalThis as { isTauri?: boolean }).isTauri = true;
    const fetch = vi.fn();
    vi.stubGlobal("fetch", fetch);
    await initializeCapabilities();
    expect(fetch).not.toHaveBeenCalled();
    expect(supports("anything")).toBe(true);
  });
  it("only enables explicitly supported Web features", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => payload() }),
    );
    await initializeCapabilities();
    expect(supports("pageMcp")).toBe(true);
    expect(supports("pageApps")).toBe(false);
    expect(supports("unknownFeature")).toBe(false);
  });
  it("shows authentication state on 401 and never enables features", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, status: 401 }),
    );
    await expect(initializeCapabilities()).rejects.toThrow();
    expect(getWebConnectionStatus()).toBe("unauthorized");
    expect(supports("pageProviders")).toBe(false);
  });
  it("records a network failure for retry", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new TypeError("network failed")),
    );
    await expect(initializeCapabilities()).rejects.toThrow();
    expect(getWebConnectionStatus()).toBe("offline");
    expect(supports("pageMcp")).toBe(false);
  });
  it.each([{}, { features: [] }, { features: { pageProviders: "yes" } }])(
    "rejects malformed capabilities: %j",
    async (data) => {
      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue({ ok: true, json: async () => data }),
      );
      await expect(initializeCapabilities()).rejects.toThrow();
      expect(getWebConnectionStatus()).toBe("incompatible");
      expect(supports("pageProviders")).toBe(false);
    },
  );
  it("rejects a server advertising Pi support without its state command", async () => {
    const data = payload();
    data.commands = data.commands.filter(
      (cmd) => cmd !== "get_pi_current_state",
    );
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => data }),
    );
    await expect(initializeCapabilities()).rejects.toThrow();
    expect(getWebConnectionStatus()).toBe("incompatible");
    expect(supports("pageProviders")).toBe(false);
  });
  it("recovers after a failed capability request", async () => {
    const fetch = vi
      .fn()
      .mockRejectedValueOnce(new TypeError("offline"))
      .mockResolvedValue({ ok: true, json: async () => payload() });
    vi.stubGlobal("fetch", fetch);
    await expect(initializeCapabilities()).rejects.toThrow();
    await initializeCapabilities();
    expect(supports("pageProviders")).toBe(true);
  });
});
