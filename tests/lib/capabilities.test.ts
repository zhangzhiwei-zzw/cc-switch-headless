import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  initializeCapabilities,
  resetCapabilitiesForTest,
  supports,
} from "@/lib/capabilities";

describe("capabilities", () => {
  beforeEach(() => {
    resetCapabilitiesForTest(null);
    delete (globalThis as { isTauri?: boolean }).isTauri;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("未初始化时一切可用（桌面模式的行为）", () => {
    expect(supports("pageUsage")).toBe(true);
    expect(supports("随便一个键")).toBe(true);
  });

  it("服务端说不可用就是不可用；没列出的键按可用处理", () => {
    resetCapabilitiesForTest({ pageUsage: false, pageSessions: true });

    expect(supports("pageUsage")).toBe(false);
    expect(supports("pageSessions")).toBe(true);
    expect(supports("服务端没提到的键")).toBe(true);
  });

  it("桌面模式（isTauri）不拉取也不限制", async () => {
    (globalThis as { isTauri?: boolean }).isTauri = true;
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);

    await initializeCapabilities();

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(supports("pageUsage")).toBe(true);
  });

  it("从 /api/capabilities 读取 features", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ features: { pageMcp: false, pageSessions: true } }),
      }),
    );

    await initializeCapabilities();

    expect(supports("pageMcp")).toBe(false);
    expect(supports("pageSessions")).toBe(true);
  });

  it("拉取失败时保持全部可用（最保守的降级）", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("boom")));

    await initializeCapabilities();

    expect(supports("pageMcp")).toBe(true);
  });

  it("响应格式不对时不启用任何限制", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: true, json: async () => ({}) }),
    );

    await initializeCapabilities();

    expect(supports("pageMcp")).toBe(true);
  });
});
