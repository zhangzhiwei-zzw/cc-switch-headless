// 浏览器端的 `@tauri-apps/plugin-dialog` 替身。
//
// 前端只在启动失败时用 `message` 提示（`main.tsx`）；文件选择对话框走的是后端
// 命令（`pick_directory` 等），在 web 模式下未实现，会返回明确错误。

export interface MessageDialogOptions {
  title?: string;
  kind?: string;
}

export async function message(
  text: string,
  options?: MessageDialogOptions | string,
): Promise<void> {
  const title = typeof options === "string" ? options : options?.title;
  window.alert(title ? `${title}\n\n${text}` : text);
}

export async function ask(
  text: string,
  options?: MessageDialogOptions,
): Promise<boolean> {
  return window.confirm(options?.title ? `${options.title}\n\n${text}` : text);
}

export async function confirm(
  text: string,
  options?: MessageDialogOptions,
): Promise<boolean> {
  return window.confirm(options?.title ? `${options.title}\n\n${text}` : text);
}

/** 浏览器无法给出服务端路径；返回 null 表示用户取消。 */
export async function open(): Promise<null> {
  return null;
}

export async function save(): Promise<null> {
  return null;
}
