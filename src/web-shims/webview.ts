// Web interface zoom follows the same saved preference as the desktop webview.
export function getCurrentWebview() {
  return {
    async setZoom(scale: number): Promise<void> {
      document.documentElement.style.setProperty("zoom", String(scale));
    },
  };
}
