// 运行时能力表。
//
// 桌面模式下一切可用；web 模式（`cc-switch-server`）下由服务端
// `GET /api/capabilities` 告知哪些功能没接入，界面据此**隐藏**对应入口——
// 而不是让用户点进去看报错。
//
// 取值必须是同步的（渲染期要用），所以在挂载前 [`initializeCapabilities`] 拉一次。

import contract from "@/web-shims/command-contract.json";
import { isTauriRuntime } from "./platform";
import {
  getWebConnectionStatus,
  setWebConnectionStatus,
  webFetch,
} from "./webConnection";

export type CapabilityMap = Record<string, boolean>;

let features: CapabilityMap | null = null;
let availableCommands: Set<string> | null = null;

/**
 * 拉取服务端能力表。桌面模式直接跳过（全部可用）。
 *
 * Web 拉取失败时阻止进入应用，由连接页面提供登录或重试。
 */
export async function initializeCapabilities(): Promise<void> {
  if (isTauriRuntime()) {
    return;
  }

  try {
    const response = await webFetch("/api/capabilities");
    if (!response.ok) {
      if (response.status !== 401) setWebConnectionStatus("incompatible");
      throw new Error("Web capability request failed");
    }
    const payload = (await response.json()) as {
      mode?: string;
      features?: CapabilityMap;
      commands?: unknown;
    };
    if (
      payload?.mode !== "web" ||
      !Array.isArray(payload.commands) ||
      !payload.commands.every((cmd) => typeof cmd === "string") ||
      !payload?.features ||
      typeof payload.features !== "object" ||
      Array.isArray(payload.features) ||
      !Object.values(payload.features).every(
        (value) => typeof value === "boolean",
      )
    ) {
      setWebConnectionStatus("incompatible");
      throw new Error("Invalid Web capability response");
    }
    const commands = new Set(payload.commands);
    for (const [name, spec] of Object.entries(contract.features)) {
      if (typeof payload.features[name] !== "boolean") {
        setWebConnectionStatus("incompatible");
        throw new Error("Missing Web feature");
      }
      if (!payload.features[name]) continue;
      const required = [
        ...spec.commands,
        ...Object.values(contract.apps)
          .filter((app) => app.feature === name)
          .flatMap((app) => app.commands),
      ];
      if (!required.every((cmd) => commands.has(cmd))) {
        setWebConnectionStatus("incompatible");
        throw new Error("Missing Web commands");
      }
    }
    features = payload.features;
    availableCommands = new Set(payload.commands);
  } catch {
    features = {};
    availableCommands = new Set();
    if (
      getWebConnectionStatus() === "checking" ||
      getWebConnectionStatus() === "ready"
    )
      setWebConnectionStatus("incompatible");
    throw new Error("Web capabilities unavailable");
  }
}

/** 桌面模式保持全部可用；Web 只启用服务端明确提供的能力。 */
export function supports(feature: string): boolean {
  if (!features) {
    return true;
  }
  return features[feature] === true;
}

/** Gate an individual action using the same runtime command inventory. */
export function supportsCommand(command: string): boolean {
  return (
    availableCommands === null ||
    availableCommands.has(command) ||
    contract.browserCommands.includes(command)
  );
}

/** 测试用：重置内部状态。 */
export function resetCapabilitiesForTest(
  next: CapabilityMap | null = null,
): void {
  features = next;
  availableCommands = null;
}
