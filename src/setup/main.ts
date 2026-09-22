import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const setupStatus = document.getElementById("setup-status")!;
const setupBarFill = document.getElementById("setup-bar-fill")!;
let stepCount = 0;

// Total expected step emits (matches the Rust setup thread in lib.rs):
//   1. "Hiding taskbar..."
//   2. "Installing Win key handler..."
// The final setup://done event bumps the bar to 100% explicitly.
const EXPECTED_STEPS = 2;

// Load the active theme so the setup window matches the app's theme.
// Applies all theme colors as CSS variables on :root + injects the SVG
// noise tile with the theme's sand-cream-rgb values.
async function loadTheme() {
  try {
    const theme = await invoke<{ name: string; colors: Record<string, string> } | null>("get_active_theme");
    if (theme) {
      const root = document.documentElement;
      for (const [key, value] of Object.entries(theme.colors)) {
        root.style.setProperty("--" + key, value);
      }
      // Build the SVG noise tile with the theme's sand-cream RGB values
      const creamRgb = theme.colors["sand-cream-rgb"];
      if (creamRgb) {
        const parts = creamRgb.split(",").map((s) => parseFloat(s.trim()));
        if (parts.length === 3) {
          const [r, g, b] = parts;
          const rNorm = (r / 255).toFixed(3);
          const gNorm = (g / 255).toFixed(3);
          const bNorm = (b / 255).toFixed(3);
          const noiseSvg = `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 ${rNorm}  0 0 0 0 ${gNorm}  0 0 0 0 ${bNorm}  0 0 0 0.252 0'/></filter><rect width='100%25' height='100%25' filter='url(%23n)'/></svg>")`;
          root.style.setProperty("--grain-noise-svg", noiseSvg);
        }
      }
    }
  } catch {
    // Default theme (sand-cream) is baked into theme.css — no action needed
  }
}

// Load theme before showing the setup content
void loadTheme();

// UAC-declined warning: when the user said No to the launch-time UAC prompt,
// the app still runs — but the brightness dimmer (a software overlay) may not
// cover system/elevated apps. Surface that here.
listen("setup://warning", () => {
  document.getElementById("setup-warning")?.classList.add("visible");
});

listen<string>("setup://step", (event) => {
  setupStatus.classList.add("fading");
  setTimeout(() => {
    setupStatus.textContent = event.payload;
    setupStatus.classList.remove("fading");
  }, 300);
  stepCount++;
  setupBarFill.style.width = `${(stepCount / EXPECTED_STEPS) * 100}%`;
});

listen("setup://done", () => {
  setupStatus.classList.add("fading");
  setTimeout(() => {
    setupStatus.textContent = "Done!";
    setupStatus.classList.remove("fading");
  }, 300);
  setupBarFill.style.width = "100%";
  setTimeout(() => {
    document.body.classList.add("exiting");
    setTimeout(() => {
      getCurrentWindow().close();
    }, 600);
  }, 1000);
});

// Tell backend to start setup
invoke("run_setup");
