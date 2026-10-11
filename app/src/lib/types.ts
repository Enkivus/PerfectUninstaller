export type Platform = "Macos" | "Windows" | "Linux" | "Unknown";

export type InstallMethod =
  | "AppBundle"
  | "PkgReceipt"
  | "Caskroom"
  | "Msi"
  | "Appx"
  | "Store"
  | "Deb"
  | "Rpm"
  | "Flatpak"
  | "Snap"
  | "Steam"
  | "Package"
  | "Manual"
  | "Unknown";

export type Confidence = "High" | "Medium" | "Low";

export type TraceCategory =
  | "Application"
  | "Preference"
  | "Cache"
  | "Support"
  | "Log"
  | "Container"
  | "SavedState"
  | "HttpStorage"
  | "WebKit"
  | "LaunchAgent"
  | "LaunchDaemon"
  | "PrivilegedHelper"
  | "Receipt"
  | "ShaderCache"
  | "CrashReport"
  | "Registry"
  | "Service"
  | "Other";

export interface InstalledApp {
  id: string;
  name: string;
  identifier: string | null;
  version: string | null;
  install_paths: string[];
  method: InstallMethod;
  platform: Platform;
  install_bytes: number;
}

export interface TraceCandidate {
  path: string;
  category: TraceCategory;
  confidence: Confidence;
  bytes: number;
  reason: string;
  primary: boolean;
}

export type AdvisoryKind =
  | "ShellEnv"
  | "Firewall"
  | "BrowserExtension"
  | "LoginItem"
  | "ScheduledTask"
  | "FileAssociation"
  | "Permission"
  | "Other";

export interface Advisory {
  kind: AdvisoryKind;
  summary: string;
  detail: string;
  command: string | null;
  path: string | null;
}

export interface RemovalPlan {
  app: InstalledApp;
  candidates: TraceCandidate[];
  advisories: Advisory[];
  total_bytes: number;
  warnings: string[];
}

export interface RemovalFailure {
  path: string;
  message: string;
}

export interface RemovalReport {
  removed: string[];
  bytes_freed: number;
  failed: RemovalFailure[];
  refused: string[];
  audit_log: string | null;
  report_dir: string | null;
  elapsed_ms: number;
}

export interface Progress {
  path: string;
  done: number;
  total: number;
}

const CATEGORY_LABELS: Record<TraceCategory, string> = {
  Application: "Application",
  Preference: "Preferences",
  Cache: "Caches",
  Support: "Application support",
  Log: "Logs",
  Container: "Sandbox container",
  SavedState: "Saved state",
  HttpStorage: "HTTP storage",
  WebKit: "WebKit data",
  LaunchAgent: "Launch agent",
  LaunchDaemon: "Launch daemon",
  PrivilegedHelper: "Privileged helper",
  Receipt: "Install receipt",
  ShaderCache: "Shader cache",
  CrashReport: "Crash report",
  Registry: "Registry",
  Service: "Service",
  Other: "Other",
};

const METHOD_LABELS: Record<InstallMethod, string> = {
  AppBundle: "Application bundle",
  PkgReceipt: "Installer package receipt",
  Caskroom: "Homebrew cask",
  Msi: "Windows installer (MSI)",
  Appx: "Appx package",
  Store: "Store app",
  Deb: "Debian package",
  Rpm: "RPM package",
  Flatpak: "Flatpak",
  Snap: "Snap",
  Steam: "Steam",
  Package: "System package",
  Manual: "Manual install",
  Unknown: "Unknown",
};

const PLATFORM_LABELS: Record<Platform, string> = {
  Macos: "macOS",
  Windows: "Windows",
  Linux: "Linux",
  Unknown: "Unknown",
};

const ADVISORY_LABELS: Record<AdvisoryKind, string> = {
  ShellEnv: "Shell environment",
  Firewall: "Firewall rule",
  BrowserExtension: "Browser extension",
  LoginItem: "Login item",
  ScheduledTask: "Scheduled task",
  FileAssociation: "File association",
  Permission: "Permission entry",
  Other: "Other",
};

export const categoryLabel = (c: TraceCategory) => CATEGORY_LABELS[c] ?? c;
export const methodLabel = (m: InstallMethod) => METHOD_LABELS[m] ?? m;
export const platformLabel = (p: Platform) => PLATFORM_LABELS[p] ?? p;
export const advisoryLabel = (k: AdvisoryKind) => ADVISORY_LABELS[k] ?? k;

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(
    units.length - 1,
    Math.floor(Math.log(bytes) / Math.log(1024)),
  );
  const value = bytes / 1024 ** i;
  const decimals = value >= 100 || i === 0 ? 0 : 1;
  return `${value.toFixed(decimals)} ${units[i]}`;
}

export function monogram(name: string): string {
  const ch = [...name.trim()].find((c) => /[\p{L}\p{N}]/u.test(c));
  return ch ? ch.toUpperCase() : "?";
}

/**
 * Shorten a path from the middle so both the root and the meaningful tail stay
 * readable. Plain CSS ellipsis would hide the filename, and `direction: rtl`
 * would swallow the leading slash.
 */
export function shortenPath(path: string, max = 64): string {
  const chars = [...path];
  if (chars.length <= max) return path;
  const head = Math.ceil((max - 1) * 0.42);
  const tail = max - 1 - head;
  return `${chars.slice(0, head).join("")}…${chars.slice(chars.length - tail).join("")}`;
}