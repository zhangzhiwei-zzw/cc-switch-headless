// Runs before React mounts; keep startup CSP-compatible without an inline script.
try {
  const theme = localStorage.getItem("cc-switch-theme") || "system";
  const dark =
    theme === "dark" ||
    (theme === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.classList.toggle("dark", dark);
} catch {
  // Storage can be unavailable in restricted browser contexts.
}
