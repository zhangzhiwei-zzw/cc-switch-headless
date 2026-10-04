// 浏览器端的 `@tauri-apps/api/app` 替身：版本号来自服务端 `/api/env`。

import { serverEnv } from "./core";

export async function getVersion(): Promise<string> {
  const env = await serverEnv();
  return env.version;
}

export async function getName(): Promise<string> {
  return "CC Switch";
}

export async function getTauriVersion(): Promise<string> {
  return "";
}
