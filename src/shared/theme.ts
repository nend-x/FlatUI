/* =========================================================================
   Shared theme application for the table windows.
   The launcher and the main taskbar each have their own copy of this logic;
   the tables share ONE module so a page can never end up with themed
   colors but stale icon-recolor values (the "silver theme, sandcream
   icons" bug): applyTheme() ALWAYS sets the color vars AND the per-theme
   icon-recolor filter values together.
   ========================================================================= */

export interface ThemePayload {
  name: string;
  colors: Record<string, string>;
}

// Per-theme icon recolor hue/sat/brightness. Must match the launcher's
// THEME_ICON_RECOLOR map (computed from each theme's accent color).
export const THEME_ICON_RECOLOR: Record<string, { hue: string; sat: string; brightness: string }> = {
  // Neutral gray theme — recolor filter yields desaturated gray icons
  "material3-dark": { hue: "0deg", sat: "0.0", brightness: "0.95" },
};

export function applyThemeColors(colors: Record<string, string>): void {
  const r = document.documentElement;
  for (const [key, value] of Object.entries(colors)) {
    r.style.setProperty("--" + key, value);
  }
  const creamRgb = colors["sand-cream-rgb"];
  if (creamRgb) {
    const parts = creamRgb.split(",").map((s) => parseFloat(s.trim()));
    if (parts.length === 3) {
      const [red, green, blue] = parts;
      const noiseSvg = `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 ${(red / 255).toFixed(3)}  0 0 0 0 ${(green / 255).toFixed(3)}  0 0 0 0 ${(blue / 255).toFixed(3)}  0 0 0 0.252 0'/></filter><rect width='100%25' height='100%25' filter='url(%23n)'/></svg>")`;
      r.style.setProperty("--grain-noise-svg", noiseSvg);
    }
  }
}

/// Apply a theme fully: colors + per-theme icon-recolor values. Every table
/// page must call THIS (not applyThemeColors directly) for both the startup
/// load and live theme://changed events.
export function applyTheme(theme: ThemePayload): void {
  applyThemeColors(theme.colors);
  const recolor = THEME_ICON_RECOLOR[theme.name];
  if (recolor) {
    const r = document.documentElement;
    r.style.setProperty("--icon-recolor-hue", recolor.hue);
    r.style.setProperty("--icon-recolor-sat", recolor.sat);
    r.style.setProperty("--icon-recolor-brightness", recolor.brightness);
  }
}
