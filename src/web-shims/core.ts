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

/** 直接调服务端的命令（不经过浏览器干预分支）。 */
async function callServer<T>(cmd: string, args: InvokeArgs): Promise<T> {
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

/**
 * 保存文件对话框 → 服务端导出路径。
 *
 * 浏览器不能给服务端选路径，于是让服务端在自己的 `<配置目录>/exports/` 里分配一个
 * 文件名；导出成功后 [`interceptBrowserCommand`] 会把该文件下载下来。
 */
async function allocateExportPath(defaultName: string): Promise<string> {
  return callServer<string>("web_allocate_export_path", { defaultName });
}

/** 打开文件对话框 → 浏览器选文件 → 上传到服务端 → 返回服务端路径。 */
function pickAndUploadFile(): Promise<string | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".sql,.zip,.skill,.json,.toml,.zip";
    input.style.display = "none";

    const cleanup = () => input.remove();

    input.onchange = async () => {
      const file = input.files?.[0];
      cleanup();
      if (!file) {
        resolve(null);
        return;
      }
      try {
        const response = await fetch(
          `/api/upload?name=${encodeURIComponent(file.name)}`,
          { method: "POST", credentials: "same-origin", body: file },
        );
        if (!response.ok) {
          throw `上传失败: HTTP ${response.status}`;
        }
        const payload = (await response.json()) as { path?: string };
        resolve(typeof payload.path === "string" ? payload.path : null);
      } catch (error) {
        reject(error);
      }
    };

    // 用户取消选择：新浏览器会发 cancel；旧的没有就把 promise 挂在那里，
    // 界面停留在"等待选择"，不会产生错误提示。
    input.oncancel = () => {
      cleanup();
      resolve(null);
    };

    document.body.appendChild(input);
    input.click();
  });
}

/** 导出到服务端路径后，触发浏览器下载同一个文件。 */
async function exportAndDownload(args: InvokeArgs): Promise<unknown> {
  const filePath = typeof args?.filePath === "string" ? args.filePath : "";
  const result = await callServer<unknown>("export_config_to_file", {
    filePath,
  });

  if (filePath) {
    const link = document.createElement("a");
    link.href = `/api/download?path=${encodeURIComponent(filePath)}`;
    link.download = "";
    link.style.display = "none";
    document.body.appendChild(link);
    link.click();
    link.remove();
  }
  return result;
}

/**
 * `stream_session_messages`：浏览器里没有 Tauri 的 `Channel`。
 *
 * 服务端一次读完整段会话（`web_session_transcript`），这里按桌面版同样的
 * `Header → Messages → Done` 顺序喂给回调，前端的分块应用逻辑因此不用改。
 */
async function streamSessionMessages(args: InvokeArgs): Promise<void> {
  const channel = args?.onChunk as Channel<unknown> | undefined;
  const providerId =
    typeof args?.providerId === "string" ? args.providerId : "";
  const sourcePath =
    typeof args?.sourcePath === "string" ? args.sourcePath : "";

  try {
    const payload = await callServer<{
      messages: unknown[];
      turns: unknown[];
      approxBytes: number;
      cached: boolean;
      parseMs: number;
    }>("web_session_transcript", { providerId, sourcePath });

    channel?.onmessage?.({
      type: "header",
      total: payload.messages.length,
      turns: payload.turns,
      cached: payload.cached,
      parseMs: payload.parseMs,
    });

    // 桌面版按字节切包；浏览器里整段已经在内存中，按条数分批即可
    const batchSize = 200;
    for (let start = 0; start < payload.messages.length; start += batchSize) {
      channel?.onmessage?.({
        type: "messages",
        start,
        messages: payload.messages.slice(start, start + batchSize),
      });
    }

    channel?.onmessage?.({ type: "done", payloadBytes: payload.approxBytes });
  } catch (error) {
    channel?.onmessage?.({ type: "error", message: String(error) });
    throw error;
  }
}

/** 浏览器能直接完成的命令不走服务端。 */
const NOT_INTERCEPTED = Symbol("not-intercepted");

function interceptBrowserCommand(cmd: string, args: InvokeArgs): unknown {
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
    // 文件对话框：换成浏览器选文件 / 服务端分配导出路径
    case "save_file_dialog": {
      const defaultName =
        typeof args?.defaultName === "string"
          ? args.defaultName
          : "cc-switch-export.sql";
      return allocateExportPath(defaultName);
    }
    case "open_file_dialog":
    case "open_zip_file_dialog": {
      return pickAndUploadFile();
    }
    case "export_config_to_file": {
      return exportAndDownload(args);
    }
    // 会话流式读取：浏览器里用一次性拉取模拟 Channel
    case "stream_session_messages": {
      return streamSessionMessages(args);
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
  return callServer<T>(cmd, args);
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
