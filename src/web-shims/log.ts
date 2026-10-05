// 浏览器端的 `@tauri-apps/plugin-log` 替身：写浏览器控制台。
//
// 服务端侧日志另有自己的输出（stderr + `<配置目录>/logs/cc-switch.log`）。

type LogOptions = Record<string, unknown> | undefined;

export async function error(
  message: string,
  _options?: LogOptions,
): Promise<void> {
  console.error(message);
}

export async function warn(
  message: string,
  _options?: LogOptions,
): Promise<void> {
  console.warn(message);
}

export async function info(
  message: string,
  _options?: LogOptions,
): Promise<void> {
  console.info(message);
}

export async function debug(
  message: string,
  _options?: LogOptions,
): Promise<void> {
  console.debug(message);
}

export async function trace(
  message: string,
  _options?: LogOptions,
): Promise<void> {
  console.debug(message);
}
