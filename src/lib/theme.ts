// The colour theme: a light theme and a dark theme, and whether to follow the
// computer's light/dark setting or stay on one (VS Code's model, so following
// the system keeps working with any pair). Kept in localStorage on this
// computer, like the text style, and applied as `data-theme` / `data-scheme`
// on <html> before the first render so the page never flashes the wrong colours.

export type Mode = "system" | "light" | "dark";
export type LightTheme = "paper" | "white";
export type DarkTheme = "graphite" | "midnight";
export type ThemeName = LightTheme | DarkTheme;

export interface ThemePrefs {
  mode: Mode;
  light: LightTheme;
  dark: DarkTheme;
}

export const DEFAULT_THEME: ThemePrefs = { mode: "system", light: "paper", dark: "graphite" };

export const MODES: Array<{ value: Mode; label: string }> = [
  { value: "system", label: "System" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];
export const LIGHT_THEMES: Array<{ value: LightTheme; label: string }> = [
  { value: "paper", label: "Paper" },
  { value: "white", label: "White" },
];
export const DARK_THEMES: Array<{ value: DarkTheme; label: string }> = [
  { value: "graphite", label: "Graphite" },
  { value: "midnight", label: "Midnight" },
];

const KEY = "noughtpad.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

const pick = <T extends string>(v: unknown, list: Array<{ value: T }>, fallback: T): T =>
  list.some((o) => o.value === v) ? (v as T) : fallback;

/** Read saved preferences; any field that isn't a known value falls back to its default. */
export function parseTheme(raw: string | null): ThemePrefs {
  if (!raw) return DEFAULT_THEME;
  try {
    const v = JSON.parse(raw) as Partial<ThemePrefs>;
    return {
      mode: pick(v.mode, MODES, DEFAULT_THEME.mode),
      light: pick(v.light, LIGHT_THEMES, DEFAULT_THEME.light),
      dark: pick(v.dark, DARK_THEMES, DEFAULT_THEME.dark),
    };
  } catch {
    return DEFAULT_THEME;
  }
}

export function resolveTheme(p: ThemePrefs, systemDark: boolean): { name: ThemeName; scheme: "light" | "dark" } {
  const dark = p.mode === "dark" || (p.mode === "system" && systemDark);
  return dark ? { name: p.dark, scheme: "dark" } : { name: p.light, scheme: "light" };
}

export function loadTheme(): ThemePrefs {
  try {
    return parseTheme(localStorage.getItem(KEY));
  } catch {
    return DEFAULT_THEME;
  }
}

export function applyTheme(p: ThemePrefs) {
  const { name, scheme } = resolveTheme(p, window.matchMedia?.(DARK_QUERY).matches ?? false);
  const root = document.documentElement;
  root.dataset.theme = name;
  root.dataset.scheme = scheme;
  root.style.colorScheme = scheme;
}

export function saveTheme(p: ThemePrefs) {
  applyTheme(p);
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    /* storage unavailable: the choice lasts until the app closes */
  }
}

/** Re-theme live when the computer switches between light and dark. */
export function watchSystemTheme(): () => void {
  const mq = window.matchMedia?.(DARK_QUERY);
  if (!mq) return () => {};
  const onChange = () => applyTheme(loadTheme());
  mq.addEventListener("change", onChange);
  return () => mq.removeEventListener("change", onChange);
}
