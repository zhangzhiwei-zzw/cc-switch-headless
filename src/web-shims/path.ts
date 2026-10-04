// 浏览器端的 `@tauri-apps/api/path` 替身。
//
// 路径信息由服务端提供（配置目录、home 目录）——web 模式下这些路径是**服务端**的
// 路径，界面上只是展示与拼默认值用。

import { serverEnv } from "./core";

export async function homeDir(): Promise<string> {
  const env = await serverEnv();
  if (!env.homeDir) {
    throw new Error("服务端未提供 home 目录");
  }
  return env.homeDir;
}

export async function appConfigDir(): Promise<string> {
  const env = await serverEnv();
  if (!env.configDir) {
    throw new Error("服务端未提供配置目录");
  }
  return env.configDir;
}

/** 与 Tauri 的 `join` 一致：空段忽略，绝对段之后的相对段直接拼接。 */
export async function join(...parts: string[]): Promise<string> {
  const filtered = parts.filter((part) => part.length > 0);
  if (filtered.length === 0) {
    return "";
  }

  return filtered.reduce((acc, part) => {
    if (acc.length === 0) {
      return part;
    }
    if (part.startsWith("/")) {
      return part;
    }
    return acc.endsWith("/") ? `${acc}${part}` : `${acc}/${part}`;
  }, "");
}
