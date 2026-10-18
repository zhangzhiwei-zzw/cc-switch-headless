import { useSyncExternalStore } from "react";

export type WebConnectionStatus =
  | "checking"
  | "ready"
  | "unauthorized"
  | "offline"
  | "incompatible";

let status: WebConnectionStatus = "checking";
const listeners = new Set<() => void>();

export function getWebConnectionStatus(): WebConnectionStatus {
  return status;
}

export function setWebConnectionStatus(next: WebConnectionStatus): void {
  if (status === next) return;
  status = next;
  for (const listener of listeners) listener();
}

export function useWebConnectionStatus(): WebConnectionStatus {
  return useSyncExternalStore((listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  }, getWebConnectionStatus);
}

/** Shared by the Web command, upload and capability transports. Never replay mutations. */
export async function webFetch(
  input: string,
  options: RequestInit = {},
  timeoutMs = 15_000,
): Promise<Response> {
  const controller = new AbortController();
  const timeout =
    timeoutMs > 0
      ? window.setTimeout(() => controller.abort(), timeoutMs)
      : undefined;
  try {
    const response = await fetch(input, {
      ...options,
      credentials: "same-origin",
      signal: options.signal ?? controller.signal,
    });
    if (
      response.status === 401 &&
      (status === "ready" || status === "checking")
    ) {
      setWebConnectionStatus("unauthorized");
    }
    return response;
  } catch (error) {
    if (!options.signal?.aborted) setWebConnectionStatus("offline");
    throw error;
  } finally {
    if (timeout !== undefined) window.clearTimeout(timeout);
  }
}
