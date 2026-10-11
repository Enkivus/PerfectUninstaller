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
/** Stand-in bundle artwork so the icon rendering path is visible in the
 *  browser preview; the real app gets this from the `app_icons` command. */
const MOCK_ICONS: Record<string, string> = {
  "app:com.spotify.client":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACNUlEQVR42u3SZVYWABQG4bsH5QMEXIjd3a1rsVuxsBO7Ewt7a9d3HzM/nnNmAVPz/x1ocdXE3/0trpr4kxCWA+AH+J0QVgbY1+KqiV8JYTkAfoCfCWFlgL0trpr4kRCWA+AHmE0Iq8Zn97S4avx7QlgOgB/gW0JYGWB3i6vGvyaE5QD4Ab4khJUBdrW4avxzQlgOgB9gJiGsGpvZ2eKqsU8JYdXYx4SwHAA/wIcdLS4HwA/wPiEsB8AP8G57i8sB8AO8TQjLAegDzHuzrcXlAPgBXieE5QD4AV5tbXE5AH6AlwlhOQB+gBdbWlwOgB/geUJYDkAfYPTZ5haXA+AHeJoQVo0+SQgrA2xqcdXo44SwHAA/wKOEsDLAxhZXjT5MCMsB8ANMJ4RVI9MbWlw18iAhLAfAD3A/ISwHwA9wb32LywHwA9xNCMsB8APcWdficgD8ALcTwnIA+gDDt9a2uBwAP8DNhLBq+EZCWBlgTYurhq8nhOUA+AGuJYSVAVa3uGr4akJYDoAfYCohrBpMrWpx1eBKQlgOgB/gckJYGWBli6sGlxLCcgD8ABcTwsoAK1pcNbiQEJYD4AeYTAirhiaXt7hq6HxCWA6AH+BcQlgZYFmLq4bOJoRVQ2cSwnIA/ACnl7a4HAA/wKmEsByAPsDck0taXA6AH+BEQlgOgB/g+OIWlwPgBziWEJYD4Ac4uqjF5QD4AY4khOUA9AHmHF7Y4nIA/ACHEsJyAPwABxe0uBwA7j/k5XJyhIbJxQAAAABJRU5ErkJggg==",
  "app:com.docker.docker":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACXElEQVR42u3SZXYQZhgF4W8dhKQhISmrwaG4uxaKraRIcHd3SEtbaAluRYJTyjYu7z5mfjznzAKmjdnyLeJqP27+P+KqAb5GXA6AH2DTfxFXDfAl4nIA/AAbP0dcNcCniMsB6AP0b/gYcTmAA3yIuFr/L+8jLgdwgHcRV+tf/zbicgAHGI64Wt+64YirBngTcdUAryOu1vdzhbBqgFcRVw3wMuJqfWsrhFUD/BtxOQB9gNFrXkRcNcDziMsB8AOsfhZx1QBPIy4HwA+w6knEVQM8jrgcgD5A78pHEZcDOMDDiKv1rngQcTmAA9yPuFrv8nsRlwM4wFDE1XqWDUVcNcDdiKsG+Cfiaj1LK4RVA/wdcdUAdyKu1rOkQlg1wO2IywHoA4xa/FfEVQP8GXE5AH6ARX9EXDXArYjLAfADLPw94nIAB/gt4qoBBiOu1r1gMOKqAW5GXDXAjYirdc+vEFYNcD3iqgGuRVyte16FsGqAqxGXA9AH+GHulYirBrgccTkAfoA5lyKuGuBixOUA+AFmX4i4HMABzkdcrWvWuYjLARzgbMTVumaeibgcwAFOR1yta0aFsGqAUxFXDXAy4mqd009GXDXAiYirBjgecbXOaRXCqgGORVw1wNGIq3X+VCGsGuBIxOUA9AFGTj0ccdUAhyIuB8APMOVgxFUDHIi4HAA/wOT9EZcDOMC+iKt1TNobcTmAA+yJuFrHxN0RlwM4wK6Iq3VMqBBWDbAz4qoBBiKuNmL8QMRVA+yIuGqA7RFXGzGuQlg1wLaIywHwA4zdGnHVAL9GXA4A9x3O8Pviq+WO0QAAAABJRU5ErkJggg==",
  "app:com.bitwarden.desktop":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACS0lEQVR42u3S1XUQUBgE4dsNwSGQAAkuwd0pDAju7u4OwZ3gLm0sfx8zD9+eLWDa5A3/Iq7Wu/5vxFUB/Im4DMAAfkdcrXfdr4jLAOgB9Kz7GXFVAD8irtazto6wKoDvEZcBGMC3iKtNWvM14jIAA/gScVUAnyOuNml1HWFVAJ8irjaxRlwVwMeIq01c9SHiMgADeB9xVQCDEVebsHIw4qoA3kVcBmAAbyOuNmHFm4jLAOgBdK94HXFVAK8irta9vI6wKoCXEVcF8CLiMgB6AF3LnkdcBmAAzyKuCuBpxNW6ltYRVgXwJOJq45c+jrgMAB/AkkcRlwEYwMOIqwIYiLjauMUDEVcF8CDiqgDuR1wGgA9g0b2IywDoAXQuuhtxVQB3Iq7WubCOsCqA2xFXBXAr4jIAegBjF9yMuAzAAG5EXBXA9YjLAPABzL8WcVUAVyOuNqZGXBXAlYirjZl3OeIyAAO4FHEZAD2A0XMvRlwVwIWIywAM4HzE1Ub3nYu4DIAewKi+sxGXAeADmHMm4qoATkdcBmAApyKuNnL2yYjLAAzgRMRlAPgAZh2PuCqAYxFXG1EjrgrgaMTVRsw8EnEZgAEcjrgqgEMRVxs+41DEVQEcjLgMwAAORFxt+PT9EZcB0AMYNn1fxFUB7I242rBpdYRVAeyJuAzAAHZHXG3o1F0RlwEYwM6IqwLYEXG1oVPqCKsC2B5xtY4acVUA2yKu1jF5a8RlAAawJeKqAPojrjaktz/iqgA2R1wGYACbIq42pGdjxGUAcP8BISPqRGjexmoAAAAASUVORK5CYII=",
  "cask:firefox":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACTElEQVR42u3Seb4IdABF8bsHj8d7XgtpkkQ0ijItkKgUpdmUkHnIlNAgGgwRQsmvu49z/viez13AzVj3xBBXHq+dGeLKf4248qgRlwegH+DfNTNDXPmnEZcHoB/gYSOuPFg9M8TlAegHuN+IK3834sq9VfOHuDwA/QB3G3HlTiOu3HmrQ1j5qxFXbjfi8gD0A9x6c/4QV/5sxJWbjbg8AP0AN1ZOD3HleiMuD0A/wLVGXPljxfQQlwegH+D3Rlz5rRFXfn1jeojLA9APcLURV6404sqV5VNDXPmlEVcuN+LyAPQD/Pz61BBXfmrE5QHoB/ixEVd+eG1qiCuXGnF5APoBLjbiyoVX5w1xeQD6Ab5vxJXzjbhy/pUOYeW7Rlw514grZxtx5ezLHcLKmUZcOd2IywPQD3DqpblDXPm2EZcHoB/gZCOunGjElePL5g5xeQD6AY414srRRlw5urRDWDnSiCuHG3F5APoBDr04OcSVg424PAD9AAcaceWbJZNDXNnfiMsD0A+wrxFX9i6eHOLyAPQDfN2IK3sacWXPC3OGuPJVI67sbsSVXY24smtRh7CysxFXdjTi8gD0A2x/fs4QV75sxOUB6Af4ohFXPl84e4grnzXi8gD0A3zaiCufPDd7iMsD0A/wcSOubGvElW0LOoSVjxpx5cNGXNnaiCtbn50Y4sqWRlz5oBGXB6Af4P1nJoa4srkRlwegH+C9Rlx59+mJIS4PQD/AO424sqkRVzY+NWuIywPQD/B2I65saMSVDU92CCvrG3H9D2Ac4LXB0ntoAAAAAElFTkSuQmCC",
  "app:com.figma.Desktop":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACd0lEQVR42u3Sd2IIBhhA8e82EpIQTZBwE3tvtUfN2lpHsDe1925LUbv23moU5Qaf7x7v/fF7J3jxrd/qFFd87ftriiv+r4irBvglxRVfKuKKzxVx1QCrUlzxqXNViiv+61yZ4nIA+gAfK+KKD50rUlwOQB/gfceKFFe861ie4nIA+gD/dixLccXbirgcgD7Amz5LU1zxuiKuGmBJiiteVcQVLyviqgEWp7jiRe/FKa54XhFXDfBziiueVcQVT3svSnE5AH2AJ70WpbhqgIUprnhcEVc86rUgxeUA9AEeVsQVD9rnp7gcgD7A/Yq44l77vBSXA9AHuNs+N8UVd9rmprgcgD7A7bafUlxxqyIuB6AP8E/bnBRX3PxhToqrBpid4oobFXHF9Yq4aoBZKa64VhFXXO05M8XlAPQBrlTEFZd7zkhxOQB9gL8r4opLrdNTXA5AH+Bi67QUlwPQB7hQEVf81To1xeUA9AHO95iS4opzFXE5AH2AP3tMTnHFHxVxOQB9gN+7/5jiirMVcdUAk1JccaYirjhdEVcNMDHFFadaJqa44mTLhBSXA9AHOFERVxxvGZ/icgD6AMeax6e44mjzuBSXA9AHONI8NsUVhyvicgD6AIeaxqS44mBFXA5AH+BA0+gUlwPQB9jfNCrFFfu6jUpxOQB9gL3dRqa4Yk9FXDXAiBRX7K6IK37rOiLFVQMMT3HFroq4YmdFXDXAsBRX7KiIK7Y3Dk1xOQB9gG0VccXWxiEpLgegD7ClcXCKKzY3DE5xOQB9gE0Ng1JcsbEiLgegD7ChYWCKK9Z3GZjiqgEGpLhiXUVcsbYirhqgf4or1lTE9R1UHlEHX5eb4QAAAABJRU5ErkJggg==",
  "app:com.microsoft.VSCode":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACHUlEQVR42u3SVVoQYBhE4W+FgKCgoKCEgoCgoLgGu7u7u7u7dT/jv49zLt5ZwHmmavO/CKwW/kZgtfAnAjMC/QCbfkdgRsAf4FcEVht/RmBGwB/gRwRW823EVfPfIzAj0A8w9y0Cq7mvEZgR6AfY8CUCMwL+AJ8jsFr/KQIzAv4AHyOwmm0jrpr9EIEZgX6AmfcRWM28i8CMQD/AurcRmBHwB3gTgdV0G3HV9OsIrKZfRWA11UZcNfUyAjMC/QCTLyKwmnwegRmBfoC1zyIwI+AP8DQCq4k24qqJJxFYTTyOwGq8jbhq/FEEZgT6AdY8jMCMgD/AgwisVt+PwIyAP8C9CKzG2oirxu5GYDV2JwKr0TbiqtHbEZgR6AcYuRWBGQF/gJsRWI3ciMBquI24avh6BGYE+gFWXYvAjIA/wNUIrFZeicCMgD/A5QishtqIq4YuRWA1dDECq8E24qrBCxGYEegHWHE+AjMC/gDnIrBafjYCMwL+AGcisBpoI64aOB2B1cCpCKz624ir+k9GYEagH2DZiQjMCPgDHI/AaumxCMwI+AMcjcCqr424qu9IBGYE+gF6D0dg1XsoAjMC/QBLDkZgRsAf4EAEVov3R2BGwB9gXwRWPW3EVT17IzAj0A/QvScCq+7dEZgR6AdYtCsCMwL+ADsjsOpqI67q2hGBVdf2CKw624irOrdFYEagH6BjawRWHVsiMCOw/QfWdR89X7oHvAAAAABJRU5ErkJggg==",
  "app:com.tinyspeck.slackmacgap":
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAIAAAACACAYAAADDPmHLAAACFUlEQVR42u3S06EYABAF0e0mtlFDbNtO3ott27ad/m62j5mPMxVMHZr5L+KqgzP+RlwO4AB/Iq4a7IirB/gdcTkAfYCB6b8iLgdwgJ8RVx3oiKsH+BFxOQB9gP3TvkdcDuAA3yKu2tcRVw/wNeJyAPoAe6d+ibgcwAE+R1y1pyOuHuBTxOUA9AF2T/kYcTmAA3yIuGpXR1w9wPuIywHoA+yc/C7icgAHeBtx1Y6OuHqANxGXA9AH2D7pdcTlAA7wKuKqbR1x9QAvIy4HoA+wdeKLiMsBHOB5xFVbOuLqAZ5FXA5AH2DzhKcRlwM4wJOIqzZ1xNUDPI64HIA+wMbxjyIuB3CAhxFXbeiIqwd4EHE5AH2A9ePuR1wO4AD3Ii4HoA+wriOuHuBuxOUA9AHWjr0TcTmAA9yOuGpNR1w9wK2IywHoA6weczPicgAHuBFx1aqOuHqA6xGXA9AHWDn6WsTlAA5wNeKqFR1x9QBXIi4HoA+wfNTliMsBHOBSxFXLOuLqAS5GXA5AH2DpyAsRlwM4wPmIq5Z0xNUDnIu4HIA+wOIRZyMuB3CAMxFXLeqIqwc4HXE5AH2AhcNPRVwO4AAnI65a0BFXD3Ai4nIA+gDzhx2PuBzAAY5FXDWvI64e4GjE5QD0AeYOPRJxOYADHI64ak5HXD3AoYjLAegDzB5yMOJyAAcYjLhqVkdcPcBAxOUAcP8B0k3IiK89SxYAAAAASUVORK5CYII=",
};

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
  const id = app.identifier ?? app.id;
  return {
    app,
    candidates: [
      C(app.install_paths[0], "Application", "High", app.install_bytes, "Primary install location"),
      C(`${home}/Caches/${id}`, "Cache", "High", 1_742_845_952, "Cache directory name matches the bundle identifier"),
      C(`${home}/Caches/${app.name.toLowerCase()}`, "Cache", "Medium", 182_386_688, "Cache directory name matches the app name"),
      C(`${home}/Logs/${app.name}`, "Log", "Medium", 24_117_248, "Log directory name matches the app name"),
      C(`${home}/Preferences/${id}.plist`, "Preference", "High", 14_680_064, "Preference file name matches the bundle identifier"),
      C(`${home}/Application Support/${app.name}`, "Support", "High", 68_787_251, "Application support directory belongs to the app"),
      C(`${home}/Application Support/${app.name}/Cache`, "Support", "Medium", 91_195_392, "Nested cache inside the app support directory"),
      C(`${home}/WebKit/${id}`, "WebKit", "Medium", 61_273_088, "WebKit website data matches the bundle identifier"),
      C(`${home}/HTTPStorages/${id}`, "HttpStorage", "Medium", 32_505_856, "HTTP storage database matches the bundle identifier"),
      C(`${home}/Saved Application State/${id}.savedState`, "SavedState", "High", 4_194_304, "Saved application state matches the bundle identifier"),
      C(`${home}/Containers/com.${id.toLowerCase()}`, "Container", "Medium", 12_582_912, "Sandboxed container directory name matches the app"),
      C(`${home}/Caches/com.apple.shaderlibs`, "ShaderCache", "Low", 3_355_443, "Shared shader cache may reference this app"),
      C(`${home}/Logs/DiagnosticReports/${app.name}-2025-06-11-*.ips`, "CrashReport", "Low", 1_572_864, "Crash report names the app that produced it"),
      C(`${home}/LaunchAgents/${id}.plist`, "LaunchAgent", "Medium", 4_096, "Login item references the bundle identifier"),
      C("/Library/LaunchAgents/com.example.helper.plist", "LaunchAgent", "Low", 4_096, "Privileged helper installed by the app"),
      C(`/var/db/receipts/${id}.plist`, "Receipt", "High", 16_384, "Installer package receipt"),
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
      case "app_icons":
        return sleep(
          (args.apps as InstalledApp[]).map((a) => ({
            id: a.id,
            data_url: MOCK_ICONS[a.id] ?? null,
          })),
          250,
        );
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

  let nextId = 1;
  const callbacks = new Map<number, (payload: unknown) => void>();

  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    value: {
      invoke: impl,
      // Tauri's Channel constructor needs this to exist before it can be
      // passed to a command; the real runtime provides it.
      transformCallback: (
        cb: (payload: unknown) => void,
        _once?: boolean,
      ) => {
        const id = nextId++;
        callbacks.set(id, cb);
        return id;
      },
    },
    writable: true,
  });
}