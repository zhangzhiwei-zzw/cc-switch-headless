// 运行时能力表。
//
// 桌面模式下一切可用；web 模式（`cc-switch-server`）下由服务端
// `GET /api/capabilities` 告知哪些功能没接入，界面据此**隐藏**对应入口——
// 而不是让用户点进去看报错。
//
// 取值必须是同步的（渲染期要用），所以在挂载前 [`initializeCapabilities`] 拉一次。

export type CapabilityMap = Record<string, boolean>;

let features: CapabilityMap | null = null;

/**
 * 拉取服务端能力表。桌面模式直接跳过（全部可用）。
 *
 * 拉取失败时同样保持"全部可用"：这是最保守的降级——真调不通时命令层还会报错。
 */
export async function initializeCapabilities(): Promise<void> {
  const isDesktop =
    typeof window !== "undefined" &&
    Boolean((globalThis as { isTauri?: boolean }).isTauri);
  if (isDesktop) {
    return;
  }

  try {
    const response = await fetch("/api/capabilities", {
      credentials: "same-origin",
    });
    if (!response.ok) {
      return;
    }
    const payload = (await response.json()) as { features?: CapabilityMap };
    if (payload?.features && typeof payload.features === "object") {
      features = payload.features;
    }
  } catch {
    // 保持 null：不隐藏任何东西
  }
}

/** 某个能力是否可用；未初始化（桌面模式/拉取失败）时为 `true`。 */
export function supports(feature: string): boolean {
  if (!features) {
    return true;
  }
  return features[feature] !== false;
}

/** 测试用：重置内部状态。 */
export function resetCapabilitiesForTest(
  next: CapabilityMap | null = null,
): void {
  features = next;
}
