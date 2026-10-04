// 浏览器端的 `@tauri-apps/plugin-updater` 替身。
//
// 服务端由运维升级（git pull / 重新构建）；`check()` 返回 null 表示「已是最新」，
// 界面不会出现更新提示。

export interface Update {
  version: string;
  notes?: string;
  date?: string;
}

export async function check(): Promise<Update | null> {
  return null;
}
