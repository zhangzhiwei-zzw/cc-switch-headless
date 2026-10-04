// 浏览器端的 `@tauri-apps/api/event` 替身：用 EventSource 订阅服务端 SSE。
//
// 事件名与桌面版完全一致，因此 App.tsx / hooks 里的 `listen(...)` 调用无需修改。

export type UnlistenFn = () => void;

export interface Event<T> {
  event: string;
  id: number;
  payload: T;
}

type Handler = (event: Event<unknown>) => void;

let source: EventSource | null = null;
let nextId = 1;
const handlers = new Map<string, Map<number, Handler>>();
const registered = new Set<string>();

function ensureSource(): EventSource {
  if (!source) {
    // EventSource 默认同源带 cookie；断线由浏览器自动重连。
    source = new EventSource("/api/events");
  }
  return source;
}

function ensureListener(name: string): void {
  const current = ensureSource();
  if (registered.has(name)) {
    return;
  }
  registered.add(name);

  current.addEventListener(name, (raw) => {
    const set = handlers.get(name);
    if (!set || set.size === 0) {
      return;
    }
    const message = raw as MessageEvent<string>;
    let payload: unknown = null;
    if (typeof message.data === "string" && message.data.length > 0) {
      try {
        payload = JSON.parse(message.data);
      } catch {
        payload = message.data;
      }
    }
    for (const [id, handler] of set) {
      handler({ event: name, id, payload });
    }
  });
}

export async function listen<T>(
  name: string,
  handler: (event: Event<T>) => void,
): Promise<UnlistenFn> {
  ensureListener(name);

  const id = nextId++;
  let set = handlers.get(name);
  if (!set) {
    set = new Map();
    handlers.set(name, set);
  }
  set.set(id, handler as Handler);

  return () => {
    handlers.get(name)?.delete(id);
  };
}

export async function once<T>(
  name: string,
  handler: (event: Event<T>) => void,
): Promise<UnlistenFn> {
  let unlisten: UnlistenFn = () => {};
  unlisten = await listen<T>(name, (event) => {
    unlisten();
    handler(event);
  });
  return unlisten;
}

/** 浏览器不能反向发事件；保留导出以免调用方编译失败。 */
export async function emit(): Promise<void> {
  // no-op
}
