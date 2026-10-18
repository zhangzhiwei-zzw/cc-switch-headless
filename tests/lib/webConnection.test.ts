import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getWebConnectionStatus,
  setWebConnectionStatus,
  webFetch,
} from "@/lib/webConnection";
import { invoke } from "@/web-shims/core";

describe("Web transport connection state", () => {
  beforeEach(() => setWebConnectionStatus("ready"));
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });
  it("a command 401 invalidates the shared session", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false, status: 401 }),
    );
    await expect(invoke("get_providers")).rejects.toContain("HTTP 401");
    expect(getWebConnectionStatus()).toBe("unauthorized");
  });
  it("a business error does not pretend the server is disconnected", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ ok: false, error: "Invalid provider" }),
      }),
    );
    await expect(invoke("add_provider")).rejects.toBe("Invalid provider");
    expect(getWebConnectionStatus()).toBe("ready");
  });
  it("a late unauthorized response preserves an existing capability failure", async () => {
    setWebConnectionStatus("incompatible");
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ status: 401 }));
    await webFetch("/api/invoke");
    expect(getWebConnectionStatus()).toBe("incompatible");
  });
  it("never automatically retries a failed mutation", async () => {
    const fetch = vi.fn().mockRejectedValue(new TypeError("Disconnected"));
    vi.stubGlobal("fetch", fetch);
    await expect(invoke("add_provider", { id: "draft" })).rejects.toThrow();
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(getWebConnectionStatus()).toBe("offline");
  });
  it("times out an unresponsive capability probe", async () => {
    vi.useFakeTimers();
    vi.stubGlobal(
      "fetch",
      vi.fn(
        (_input, init: RequestInit) =>
          new Promise((_resolve, reject) =>
            init.signal?.addEventListener("abort", () =>
              reject(new DOMException("Timed out", "AbortError")),
            ),
          ),
      ),
    );
    const result = expect(webFetch("/api/capabilities")).rejects.toThrow();
    await vi.advanceTimersByTimeAsync(15_000);
    await result;
    expect(getWebConnectionStatus()).toBe("offline");
  });
});
