import contract from "../../src/web-shims/command-contract.json" with { type: "json" };
import { spawn, type ChildProcess } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  existsSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

export class TestServer {
  readonly home = mkdtempSync(path.join(tmpdir(), "cc-switch-browser-"));
  readonly url: string;
  process?: ChildProcess;
  token = "";
  private log = "";

  constructor(
    readonly port: number,
    readonly cookieName?: string,
  ) {
    this.url = `http://127.0.0.1:${port}`;
    mkdirSync(path.join(this.home, ".cc-switch"), { recursive: true });
    mkdirSync(path.join(this.home, ".claude"), { recursive: true });
    writeFileSync(path.join(this.home, ".claude/settings.json"), "{}");
    writeFileSync(
      path.join(this.home, ".cc-switch/settings.json"),
      JSON.stringify({
        language: "en",
        firstRunNoticeConfirmed: true,
        newLayoutNoticeConfirmed: true,
        whatsNewSeenVersion: "99.0.0",
        proxyConfirmed: true,
        visibleApps: Object.fromEntries(
          Object.keys(contract.apps).map((app) => [app, true]),
        ),
        sessionAutoSyncEnabled: false,
      }),
    );
  }

  async start() {
    this.log = "";
    const env = { ...process.env };
    for (const key of Object.keys(env))
      if (
        key.startsWith("CC_SWITCH_WEB_") ||
        key === "CC_SWITCH_ALLOW_HOSTS" ||
        key === "CC_SWITCH_CONFIG_DIR"
      )
        delete env[key];
    Object.assign(env, {
      HOME: this.home,
      USERPROFILE: this.home,
      CC_SWITCH_CONFIG_DIR: path.join(this.home, ".cc-switch"),
      PI_CODING_AGENT_DIR: path.join(this.home, ".pi/agent"),
      XDG_CONFIG_HOME: path.join(this.home, ".config"),
    });
    const binary =
      process.env.CC_SWITCH_SERVER_BIN ??
      path.resolve("src-tauri/target/debug/cc-switch-server");
    const args = [
      "--port",
      String(this.port),
      "--dist",
      path.resolve("dist-web"),
    ];
    if (this.cookieName) args.push("--cookie-name", this.cookieName);
    this.process = spawn(binary, args, {
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    this.process.on("error", (error) => {
      this.log += String(error);
    });
    this.process.stdout?.on("data", (data) => {
      this.log += String(data);
    });
    this.process.stderr?.on("data", (data) => {
      this.log += String(data);
    });
    const deadline = Date.now() + 120_000;
    while (Date.now() < deadline) {
      if (this.process.exitCode !== null)
        throw new Error(`Test server exited: ${this.log}`);
      const tokenPath = path.join(this.home, ".cc-switch/web-token");
      if (existsSync(tokenPath)) {
        this.token = readFileSync(tokenPath, "utf8").trim();
        try {
          const response = await fetch(`${this.url}/api/capabilities`, {
            headers: { Authorization: `Bearer ${this.token}` },
            signal: AbortSignal.timeout(2000),
          });
          if (response.ok) return;
        } catch {
          /* Wait for the listener and database initialization. */
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 200));
    }
    throw new Error(
      `Test server startup timed out (${this.home}): ${this.log}`,
    );
  }

  async stop() {
    const child = this.process;
    if (!child || child.exitCode !== null) return;
    const exited = new Promise<void>((resolve) =>
      child.once("exit", () => resolve()),
    );
    child.kill("SIGTERM");
    const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
    await exited;
    clearTimeout(timer);
  }
}
