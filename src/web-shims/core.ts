// 浏览器端的 `@tauri-apps/api/core` 替身。
//
// 只在 web 构建（`CC_SWITCH_WEB=1`）下由 vite alias 生效；桌面构建仍然使用真正的
// Tauri API。语义与 Tauri 对齐：命令失败时以**纯字符串** reject（前端各处按字符串
// 处理错误），成功时返回 data。
//
// 服务端分发表见 `src-tauri/src/web/routes.rs`。

export type InvokeArgs = Record<string, unknown> | undefined;

interface InvokeResponse<T> {
  ok: boolean;
  data?: T;
  error?: string;
}

export interface ServerEnv {
  version: string;
  homeDir: string | null;
  configDir: string | null;
}

let envPromise: Promise<ServerEnv> | null = null;

/** 宿主信息（版本、home、配置目录），由服务端 `/api/env` 提供。 */
export function serverEnv(): Promise<ServerEnv> {
  envPromise ??= fetch("/api/env", { credentials: "same-origin" })
    .then(async (response) => {
      if (!response.ok) throw new Error(String(response.status));
      const raw = (await response.json()) as Record<string, unknown>;
      return {
        version: typeof raw.version === "string" ? raw.version : "",
        homeDir: typeof raw.homeDir === "string" ? raw.homeDir : null,
        configDir: typeof raw.configDir === "string" ? raw.configDir : null,
      };
    })
    .catch(() => ({ version: "", homeDir: null, configDir: null }));
  return envPromise;
}

/** 浏览器能直接完成的命令不走服务端。 */
const NOT_INTERCEPTED = Symbol("not-intercepted");

function interceptBrowserCommand(
  cmd: string,
  args: InvokeArgs,
): unknown {
  switch (cmd) {
    case "open_external": {
      const url = args?.url;
      if (typeof url === "string" && url.length > 0) {
        window.open(url, "_blank", "noopener,noreferrer");
      }
      return true;
    }
    case "copy_text_to_clipboard": {
      const text = args?.text;
      if (typeof text !== "string") {
        throw "剪贴板内容不合法";
      }
      if (!navigator.clipboard) {
        throw "浏览器不支持剪贴板写入";
      }
      return navigator.clipboard.writeText(text).then(() => true);
    }
    default:
      return NOT_INTERCEPTED;
  }
}

export async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  const intercepted = interceptBrowserCommand(cmd, args);
  if (intercepted !== NOT_INTERCEPTED) {
    return (await intercepted) as T;
  }

  const response = await fetch("/api/invoke", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ cmd, args: args ?? {} }),
  });

  if (!response.ok) {
    throw `调用 ${cmd} 失败: HTTP ${response.status}`;
  }

  const payload = (await response.json()) as InvokeResponse<T>;
  if (!payload.ok) {
    throw payload.error ?? `调用 ${cmd} 失败`;
  }
  return payload.data as T;
}

/** web 模式一律返回 false，供 `windowActivity`、窗口控件等走浏览器回退路径。 */
export function isTauri(): boolean {
  return false;
}

/**
 * `Channel` 替身。
 *
 * 服务端 PoC 未实现流式命令（`stream_session_messages`），调用会得到
 * `E_NOT_IMPLEMENTED`，前端会退回到一次性拉取。
 */
export class Channel<T> {
  onmessage?: (message: T) => void;

  toJSON(): Record<string, unknown> {
    return { __ccswitchChannel: true };
  }
}

/** 资源协议在浏览器里不存在，原样返回路径。 */
export function convertFileSrc(filePath: string): string {
  return filePath;
}
