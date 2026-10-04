// 浏览器端的 `@tauri-apps/plugin-process` 替身。
//
// 服务端进程不归浏览器管：这里只记录日志，不做任何退出动作，
// 免得前端一个错误分支就把页面关掉。

export async function exit(code = 0): Promise<void> {
  console.warn(`[web] 忽略 exit(${code})：服务端由运维自行管理`);
}

export async function relaunch(): Promise<void> {
  window.location.reload();
}
