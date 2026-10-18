import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { initializeCapabilities } from "@/lib/capabilities";
import { isTauriRuntime } from "@/lib/platform";
import {
  getWebConnectionStatus,
  setWebConnectionStatus,
  useWebConnectionStatus,
  webFetch,
} from "@/lib/webConnection";

export function WebConnectionGate({
  children,
  onReady,
}: {
  children?: ReactNode;
  onReady?: () => void;
}) {
  const { t } = useTranslation();
  const status = useWebConnectionStatus();
  const [token, setToken] = useState("");
  const [invalidToken, setInvalidToken] = useState(false);
  const [busy, setBusy] = useState(false);
  const probing = useRef(false);
  const desktop = isTauriRuntime();

  const recover = useCallback(async () => {
    if (probing.current) return;
    probing.current = true;
    setBusy(true);
    setWebConnectionStatus("checking");
    try {
      await initializeCapabilities();
      if (onReady) {
        setWebConnectionStatus("ready");
        onReady();
      } else {
        // Reconnect from a clean query cache, environment and SSE connection.
        window.location.reload();
      }
    } catch {
      // initializeCapabilities records the actionable state; never show raw errors/tokens.
    } finally {
      probing.current = false;
      setBusy(false);
    }
  }, [onReady]);

  useEffect(() => {
    if (desktop) return;
    const probe = () => {
      if (getWebConnectionStatus() !== "ready" || probing.current) return;
      probing.current = true;
      void initializeCapabilities()
        .finally(() => {
          probing.current = false;
        })
        .catch(() => {});
    };
    const offline = () => setWebConnectionStatus("offline");
    const interval = window.setInterval(probe, 30_000);
    window.addEventListener("offline", offline);
    window.addEventListener("online", probe);
    window.addEventListener("cc-switch-web-connection-check", probe);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener("offline", offline);
      window.removeEventListener("online", probe);
      window.removeEventListener("cc-switch-web-connection-check", probe);
    };
  }, [desktop]);

  if (desktop) return children;
  if (status === "ready")
    return (
      children ?? (
        <main
          className="flex min-h-screen items-center justify-center bg-background"
          role="status"
        >
          {t("webConnection.checking.title")}
        </main>
      )
    );

  const login = async (event: React.FormEvent) => {
    event.preventDefault();
    if (busy || !token.trim()) return;
    setBusy(true);
    setInvalidToken(false);
    try {
      const response = await webFetch("/auth", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token: token.trim() }),
      });
      if (response.status === 401) {
        setInvalidToken(true);
        return;
      }
      if (!response.ok) {
        setWebConnectionStatus("incompatible");
        return;
      }
      setToken("");
      await recover();
    } catch {
      // webFetch already sets the connection state.
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-background p-6">
      <section
        className="w-full max-w-md space-y-5 rounded-xl border bg-card p-7 text-card-foreground shadow-sm"
        aria-live="polite"
      >
        <p className="text-sm text-muted-foreground">CC Switch Web</p>
        <h1 className="text-xl font-semibold">
          {t(`webConnection.${status}.title`)}
        </h1>
        <p className="text-sm text-muted-foreground">
          {t(`webConnection.${status}.description`)}
        </p>
        {status === "unauthorized" ? (
          <form onSubmit={login} className="space-y-4">
            <label className="block space-y-2 text-sm">
              <span>{t("webConnection.token")}</span>
              <input
                type="password"
                autoComplete="off"
                value={token}
                onChange={(event) => setToken(event.target.value)}
                className="w-full rounded-md border bg-background px-3 py-2"
                disabled={busy}
              />
            </label>
            {invalidToken && (
              <p role="alert" className="text-sm text-destructive">
                {t("webConnection.invalidToken")}
              </p>
            )}
            <button
              type="submit"
              disabled={busy || !token.trim()}
              className="w-full rounded-md bg-primary px-4 py-2 text-primary-foreground disabled:opacity-50"
            >
              {t("webConnection.login")}
            </button>
          </form>
        ) : status !== "checking" ? (
          <button
            type="button"
            onClick={() => void recover()}
            disabled={busy}
            className="rounded-md bg-primary px-4 py-2 text-primary-foreground disabled:opacity-50"
          >
            {t("webConnection.retry")}
          </button>
        ) : null}
      </section>
    </main>
  );
}
