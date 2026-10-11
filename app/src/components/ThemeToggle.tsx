import { useTheme, type ThemeChoice } from "../lib/theme";

const NEXT: Record<ThemeChoice, ThemeChoice> = {
  system: "light",
  light: "dark",
  dark: "system",
};

const ICON: Record<ThemeChoice, string> = {
  system: "◐",
  light: "☀",
  dark: "☾",
};

const LABEL: Record<ThemeChoice, string> = {
  system: "Theme: follow system",
  light: "Theme: light",
  dark: "Theme: dark",
};

export function ThemeToggle() {
  const [choice, setChoice] = useTheme();
  return (
    <button
      className="btn btn--icon"
      title={LABEL[choice]}
      aria-label={LABEL[choice]}
      onClick={() => setChoice(NEXT[choice])}
      style={{ fontSize: 14, lineHeight: 1 }}
    >
      {ICON[choice]}
    </button>
  );
}