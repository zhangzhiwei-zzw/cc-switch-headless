import { afterEach, describe, expect, it, vi } from "vitest";

class TestEventSource extends EventTarget {
  static readonly CLOSED = 2;
  static instances: TestEventSource[] = [];
  readyState = 0;
  constructor(_url: string) {
    super();
    TestEventSource.instances.push(this);
  }
  close() {
    this.readyState = TestEventSource.CLOSED;
  }
}

afterEach(() => vi.unstubAllGlobals());

describe("Web events after authentication", () => {
  it("restores earlier subscriptions after a pre-login connection closes and keeps unsubscribe working", async () => {
    vi.resetModules();
    TestEventSource.instances = [];
    vi.stubGlobal("EventSource", TestEventSource);
    const { listen } = await import("@/web-shims/event");
    const usage = vi.fn();
    const index = vi.fn();
    const stopUsage = await listen("usage-log-recorded", usage);
    const rejected = TestEventSource.instances[0];
    rejected.close();
    rejected.dispatchEvent(new Event("error"));

    const stopIndex = await listen("session-index-status", index);
    const authenticated = TestEventSource.instances.at(-1)!;
    authenticated.dispatchEvent(
      new MessageEvent("usage-log-recorded", { data: "null" }),
    );
    authenticated.dispatchEvent(
      new MessageEvent("session-index-status", {
        data: '{"processed":2,"total":2,"running":false}',
      }),
    );
    expect(usage).toHaveBeenCalledTimes(1);
    expect(index).toHaveBeenCalledWith(
      expect.objectContaining({
        payload: { processed: 2, total: 2, running: false },
      }),
    );

    stopUsage();
    authenticated.dispatchEvent(
      new MessageEvent("usage-log-recorded", { data: "null" }),
    );
    expect(usage).toHaveBeenCalledTimes(1);
    stopIndex();
  });
});
