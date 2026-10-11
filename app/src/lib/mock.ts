import type {
  InstalledApp,
  Progress,
  RemovalPlan,
  RemovalReport,
} from "./types";

/**
 * Dev-only stand-in for the Tauri IPC so the UI can be iterated on in a plain
 * browser (`npm run dev` then open `?mock=1`). Never bundled in production:
 * `main.tsx` only calls this when `import.meta.env.DEV` is true.
 */
const apps: InstalledApp[] = [
  {
    id: "app:com.spotify.client",
    name: "Spotify",
    identifier: "com.spotify.client",
    version: "1.2.42.2520",
    install_paths: ["/Applications/Spotify.app"],
    method: "AppBundle",
    platform: "Macos",
    install_bytes: 213_073_664,
  },
  {
    id: "app:com.docker.docker",
    name: "Docker",
    identifier: "com.docker.docker",
    version: "4.34.3",
    install_paths: ["/Applications/Docker.app"],
    method: "AppBundle",
    platform: "Macos",
    install_bytes: 1_073_741_824,
  },
  {
    id: "pkg:com.bitwarden.desktop",
    name: "Bitwarden",
    identifier: "com.bitwarden.desktop",
    version: "2025.6.1",
    install_paths: ["/Applications/Bitwarden.app"],
    method: "PkgReceipt",
    platform: "Macos",
    install_bytes: 402_653_184,
  },
  {
    id: "cask:firefox",
    name: "Firefox",
    identifier: "org.mozilla.firefox",
    version: "141.0",
    install_paths: ["/Applications/Firefox.app"],
    method: "Caskroom",
    platform: "Macos",
    install_bytes: 335_544_320,
  },
  {
    id: "app:com.figma.Desktop",
    name: "Figma",
    identifier: "com.figma.Desktop",
    version: "124.6.4",
    install_paths: ["/Applications/Figma.app"],
    method: "AppBundle",
    platform: "Macos",
    install_bytes: 289_406_976,
  },
  {
    id: "app:com.microsoft.VSCode",
    name: "Visual Studio Code",
    identifier: "com.microsoft.VSCode",
    version: "1.102.0",
    install_paths: ["/Applications/Visual Studio Code.app"],
    method: "AppBundle",
    platform: "Macos",
    install_bytes: 524_288_000,
  },
  {
    id: "app:com.tinyspeck.slackmacgap",
    name: "Slack",
    identifier: "com.tinyspeck.slackmacgap",
    version: "4.42.98",
    install_paths: ["/Applications/Slack.app"],
    method: "AppBundle",
    platform: "Macos",
    install_bytes: 415_236_096,
  },
];

const P = "/Users/alabbas";
const C = (path: string, category: any, confidence: any, bytes: number, reason: string) => ({
  path,
  category,
  confidence,
  bytes,
  reason,
  primary: category === "Application",
});

function plan(app: InstalledApp): RemovalPlan {
  const home = `${P}/Library`;
  return {
    app,
    candidates: [
      C(app.install_paths[0], "Application", "High", app.install_bytes, "Primary install location"),
      C(`${home}/Caches/${app.identifier}`, "Cache", "High", 1_742_845_952, "Cache directory name matches the bundle identifier"),
      C(`${home}/Caches/${app.name.toLowerCase()}`, "Cache", "Medium", 182_386_688, "Cache directory name matches the app name"),
      C(`${home}/Logs/${app.name}`, "Log", "Medium", 24_117_248, "Log directory name matches the app name"),
      C(`${home}/Preferences/${app.identifier}.plist`, "Preference", "High", 14_680_064, "Preference file name matches the bundle identifier"),
      C(`${home}/Application Support/${app.name}`, "Support", "High", 68_787_251, "Application support directory belongs to the app"),
      C(`${home}/Application Support/${app.name}/Cache`, "Support", "Medium", 91_195_392, "Nested cache inside the app support directory"),
      C(`${home}/WebKit/${app.identifier}`, "WebKit", "Medium", 61_273_088, "WebKit website data matches the bundle identifier"),
      C(`${home}/HTTPStorages/${app.identifier}`, "HttpStorage", "Medium", 32_505_856, "HTTP storage database matches the bundle identifier"),
      C(`${home}/Saved Application State/${app.identifier}.savedState`, "SavedState", "High", 4_194_304, "Saved application state matches the bundle identifier"),
      C(`${home}/Containers/com.${app.identifier.toLowerCase()}`, "Container", "Medium", 12_582_912, "Sandboxed container directory name matches the app"),
      C(`${home}/Caches/com.apple.shaderlibs`, "ShaderCache", "Low", 3_355_443, "Shared shader cache may reference this app"),
      C(`${home}/Logs/DiagnosticReports/${app.name}-2025-06-11-*.ips`, "CrashReport", "Low", 1_572_864, "Crash report names the app that produced it"),
      C(`${home}/LaunchAgents/${app.identifier}.plist`, "LaunchAgent", "Medium", 4_096, "Login item references the bundle identifier"),
      C("/Library/LaunchAgents/com.example.helper.plist", "LaunchAgent", "Low", 4_096, "Privileged helper installed by the app"),
      C(`/var/db/receipts/${app.identifier}.plist`, "Receipt", "High", 16_384, "Installer package receipt"),
    ],
    advisories: [
      {
        kind: "ShellEnv",
        summary: "PATH modified in ~/.zshrc",
        detail:
          "The installer appended an export line pointing at the app's bundled tools. Review it before removing the install, otherwise your shell may keep a dangling path.",
        command: "grep -n 'Spotify' ~/.zshrc ~/.zprofile",
        path: `${P}/.zshrc`,
      },
      {
        kind: "BrowserExtension",
        summary: "Native messaging host registered",
        detail:
          "The app registered a native messaging host so browser extensions can talk to it. Removing the host file makes the extension stop working; the extension itself must be removed from the browser.",
        command: null,
        path: `${home}/Application Support/Google/Chrome/NativeMessagingHosts/com.spotify.client.json`,
      },
      {
        kind: "LoginItem",
        summary: "Login item registered via SMAppService",
        detail:
          "The app registers itself to start at login. It is removed automatically with the app bundle on macOS 13 and newer.",
        command: null,
        path: null,
      },
    ],
    total_bytes: 2_662_030_336,
    warnings: [
      "1 candidate is shared with another application and will be skipped if still needed.",
    ],
  };
}

const report: RemovalReport = {
  removed: [
    "/Applications/Spotify.app",
    `${P}/Library/Caches/com.spotify.client`,
    `${P}/Library/Logs/Spotify`,
    `${P}/Library/Preferences/com.spotify.client.plist`,
    `${P}/Library/Application Support/Spotify`,
  ],
  bytes_freed: 2_147_483_648,
  failed: [
    {
      path: `${P}/Library/Application Support/Spotify/Session Storage`,
      message: "Permission denied (file in use by the app)",
    },
  ],
  refused: ["/Library/LaunchDaemons/com.example.shared.plist"],
  audit_log: `${P}/.perfectuninstaller/audit/uninstall-20260611-101500-abc123.jsonl`,
  report_dir: `${P}/.perfectuninstaller/reports/spotify-20260611-101500`,
  elapsed_ms: 8432,
};

export function installMock() {
  const sleep = <T,>(value: T, ms = 350): Promise<T> =>
    new Promise((r) => setTimeout(() => r(value), ms));

  const impl = async (
    cmd: string,
    args: Record<string, any> = {},
  ): Promise<unknown> => {
    switch (cmd) {
      case "scan_apps":
        return sleep(apps, 700);
      case "analyze":
        return sleep(plan(args.app), 1400);
      case "default_selection": {
        const p = args.plan as RemovalPlan;
        return sleep(
          p.candidates
            .filter((c) => c.primary || c.confidence === "High")
            .map((c) => c.path),
          120,
        );
      }
      case "uninstall": {
        const selected = args.selected as string[];
        for (let i = 0; i < selected.length; i++) {
          const channel = args.onProgress as { onmessage?: (m: Progress) => void };
          channel?.onmessage?.({
            path: selected[i],
            done: i + 1,
            total: selected.length,
          });
          await new Promise((r) => setTimeout(r, 220));
        }
        return sleep(report, 300);
      }
      case "export_report":
        return report.report_dir;
      case "open_path":
        return null;
      default:
        throw new Error(`mock: unhandled command ${cmd}`);
    }
  };

  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    value: { invoke: impl },
    writable: true,
  });
}