// 浏览器端的 `@tauri-apps/api/window` 替身。
//
// 浏览器里没有原生窗口控制；调用方（`WindowControls`、`windowActivity`）已经通过
// `isTauri()` 走了回退路径，这里只需要提供同形状的 no-op，避免调用即崩溃。

export class WebWindow {
  async minimize(): Promise<void> {}
  async toggleMaximize(): Promise<void> {}
  async isMaximized(): Promise<boolean> {
    return false;
  }
  async maximize(): Promise<void> {}
  async unmaximize(): Promise<void> {}
  async close(): Promise<void> {
    window.close();
  }
  async setDecorations(_decorations: boolean): Promise<void> {}
  async setFocus(): Promise<void> {}
  async show(): Promise<void> {}
  async hide(): Promise<void> {}

  async onResized(_handler: () => void): Promise<() => void> {
    return () => {};
  }

  async onFocusChanged(
    _handler: (event: { payload: boolean }) => void,
  ): Promise<() => void> {
    return () => {};
  }

  async onCloseRequested(_handler: () => void): Promise<() => void> {
    return () => {};
  }
}

export function getCurrentWindow(): WebWindow {
  return new WebWindow();
}

export function getAllWindows(): WebWindow[] {
  return [new WebWindow()];
}
