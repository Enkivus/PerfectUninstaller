import { useEffect, useState } from "react";

export type ThemeChoice = "system" | "light" | "dark";

const KEY = "pu-theme";
export function getChoice(): ThemeChoice {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem(KEY);
  } catch {
    // Storage can be unavailable in restricted webviews. The UI should still
    // render with the default theme in that case.
  }
  return stored === "light" || stored === "dark" || stored === "system"
    ? stored
    : "dark";
}

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function applyTheme(choice: ThemeChoice) {
  const dark = choice === "dark" || (choice === "system" && systemPrefersDark());
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

const PREVIEW = new URLSearchParams(window.location.search).get("theme");

function previewTheme(): ThemeChoice | null {
  return PREVIEW === "light" || PREVIEW === "dark" ? PREVIEW : null;
}

/** Keeps `<html data-theme>` in sync with the choice and the OS preference. */
export function useTheme(): [ThemeChoice, (c: ThemeChoice) => void] {
  const [choice, setChoice] = useState<ThemeChoice>(getChoice);
  const preview = previewTheme();

  useEffect(() => {
    if (preview) {
      applyTheme(preview);
      return;
    }
    applyTheme(choice);
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      if (getChoice() === "system") applyTheme("system");
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [choice, preview]);

  const update = (c: ThemeChoice) => {
    try {
      localStorage.setItem(KEY, c);
    } catch {
      // Keep the current session usable even when preferences cannot persist.
    }
    setChoice(c);
  };

  return [choice, update];
}
