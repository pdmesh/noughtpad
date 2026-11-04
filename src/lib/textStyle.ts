// The note text's font and size: the system's by default, changeable in
// Settings → Writing. Kept in localStorage on this computer (like the other
// layout preferences) and applied as CSS variables (`--font-text`,
// `--text-size`) on <html>, before the first render so the text never jumps.

export type FontChoice = "system" | "serif" | "rounded" | "mono" | "custom";

export interface TextStyle {
  font: FontChoice;
  /** The font's name when `font` is "custom". */
  custom: string;
  /** px; `null` is the system's default size. */
  size: number | null;
}

export const FONTS: Array<{ value: FontChoice; label: string }> = [
  { value: "system", label: "System" },
  { value: "serif", label: "Serif" },
  { value: "rounded", label: "Rounded" },
  { value: "mono", label: "Monospace" },
  { value: "custom", label: "Other…" },
];

export const SIZES = [12, 13, 14, 15, 16, 17, 18, 20, 22, 24, 28];

const STACKS: Record<Exclude<FontChoice, "custom">, string> = {
  system: `system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", sans-serif`,
  serif: `ui-serif, "New York", "Iowan Old Style", Georgia, "Times New Roman", serif`,
  rounded: `ui-rounded, "SF Pro Rounded", system-ui, sans-serif`,
  mono: `ui-monospace, "SF Mono", Menlo, Consolas, monospace`,
};

export const DEFAULT_STYLE: TextStyle = { font: "system", custom: "", size: null };

const KEY = "noughtpad.textStyle";

/** The CSS font-family for a style (a custom name falls back to the system font). */
export function fontFamily(s: TextStyle): string {
  if (s.font !== "custom") return STACKS[s.font];
  const name = s.custom.trim().replace(/["\\]/g, "");
  return name ? `"${name}", ${STACKS.system}` : STACKS.system;
}

/** Read a saved style, ignoring anything malformed. */
export function parseStyle(raw: string | null): TextStyle {
  if (!raw) return DEFAULT_STYLE;
  try {
    const v = JSON.parse(raw) as Partial<TextStyle>;
    const font = FONTS.some((f) => f.value === v.font) ? (v.font as FontChoice) : "system";
    const size = typeof v.size === "number" && v.size >= 8 && v.size <= 48 ? v.size : null;
    return { font, custom: typeof v.custom === "string" ? v.custom : "", size };
  } catch {
    return DEFAULT_STYLE;
  }
}

export function loadStyle(): TextStyle {
  try {
    return parseStyle(localStorage.getItem(KEY));
  } catch {
    return DEFAULT_STYLE;
  }
}

export function applyStyle(s: TextStyle) {
  const root = document.documentElement.style;
  root.setProperty("--font-text", fontFamily(s));
  if (s.size === null) root.removeProperty("--text-size");
  else root.setProperty("--text-size", `${s.size}px`);
}

export function saveStyle(s: TextStyle) {
  applyStyle(s);
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    /* storage unavailable: the choice lasts until the app closes */
  }
}
