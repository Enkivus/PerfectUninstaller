import { monogram } from "../lib/types";

type Props = {
  name: string;
  /** Bundle artwork as a data URL, when the engine could extract it. */
  icon?: string | null;
  size?: number;
};

/**
 * App artwork with a letter fallback. Icons are macOS-shaped rounded squares,
 * so we scale rather than crop and let the CSS border-radius do the rest.
 */
export function Avatar({ name, icon, size = 30 }: Props) {
  if (icon) {
    return (
      <span
        className="app-row__avatar app-row__avatar--icon"
        style={{ width: size, height: size }}
      >
        <img src={icon} alt="" aria-hidden="true" draggable={false} />
      </span>
    );
  }
  return (
    <span className="app-row__avatar" style={{ width: size, height: size }}>
      {monogram(name)}
    </span>
  );
}