import { Channel, invoke } from "@tauri-apps/api/core";
import type { InstalledApp, Progress, RemovalPlan, RemovalReport } from "./types";

export const scanApps = () => invoke<InstalledApp[]>("scan_apps");

export interface AppIcon {
  id: string;
  data_url: string | null;
}

/** Bundle artwork for the scanned apps, keyed by app id. */
export const appIcons = (apps: InstalledApp[]) =>
  invoke<AppIcon[]>("app_icons", { apps });

export const iconsById = (icons: AppIcon[]) =>
  new Map(icons.map((i) => [i.id, i.data_url]));

export const analyze = (app: InstalledApp) =>
  invoke<RemovalPlan>("analyze", { app });

export const defaultSelection = (plan: RemovalPlan) =>
  invoke<string[]>("default_selection", { plan });

export const uninstall = (
  plan: RemovalPlan,
  selected: string[],
  onProgress: (p: Progress) => void,
) => {
  const channel = new Channel<Progress>();
  channel.onmessage = onProgress;
  return invoke<RemovalReport>("uninstall", {
    plan,
    selected,
    onProgress: channel,
  });
};

export const exportReport = (plan: RemovalPlan, report: RemovalReport) =>
  invoke<string>("export_report", { plan, report });

export const openPath = (path: string) => invoke<void>("open_path", { path });