/* =========================================================================
   TABLES picker — the radial menu that appears while the Win key is held.
   The backend shows this window on a Win hold and reads our hover report
   (set_tables_hover) when the user releases Win over a button.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme, type ThemePayload } from "../shared/theme";

interface ShowPayload {
  x: number;
  y: number;
  center: boolean;
}

// Theme + icon-recolor values are applied together by the shared applyTheme
// — the picker must follow the active theme (startup load AND live events).

const root = document.getElementById("tables-root")!;
const buttons = Array.from(document.querySelectorAll<HTMLElement>(".table-btn"));

let hoverTask: number | null = null;

// Buttons sit on a pentagon around the anchor point (radius 84px,
// clockwise from the top). Offsets in px.
const OFFSETS: Record<string, [number, number]> = {
  flatlight: [0, -84],   // top
  desktop: [80, -26],    // top-right
  taskbar: [49, 68],     // bottom-right
  widgets: [-49, 68],    // bottom-left
  settings: [-80, -26],  // top-left
};

const TABLE_IDS: Record<string, number> = {
  taskbar: 1,
  settings: 2,
  widgets: 3,
  flatlight: 4,
  desktop: 5,
};

function reportHover(id: number) {
  if (hoverTask) window.clearTimeout(hoverTask);
  // Micro-defer: a quick pass over a button between two neighbors should
  // never leave a stale hover report racing the hook's Win-up read.
  hoverTask = window.setTimeout(() => {
    invoke("set_tables_hover", { id });
  }, 8);
}

function clearHover() {
  buttons.forEach((b) => b.classList.remove("hover"));
  reportHover(0);
}

// Hover tracking — the whole release gesture depends on this staying fresh.
for (const btn of buttons) {
  btn.addEventListener("mouseenter", () => {
    buttons.forEach((b) => b.classList.remove("hover"));
    btn.classList.add("hover");
    const table = btn.dataset.table!;
    reportHover(TABLE_IDS[table] ?? 0);
  });
  btn.addEventListener("mouseleave", () => {
    btn.classList.remove("hover");
    reportHover(0);
  });
  // Click fallback (the primary gesture is hover + release Win) — e.g. a
  // user who holds Win forever and just clicks instead.
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    invoke("open_table", { name: btn.dataset.table });
  });
}

// Click anywhere else on the overlay → dismiss.
root.addEventListener("mousedown", () => {
  invoke("close_table", { name: "tables" });
});

listen<ShowPayload>("tables://show", (e) => {
  const { x, y } = e.payload;
  // Anchor point → CSS vars for the vignette + per-button placement.
  root.style.setProperty("--pick-x", `${x}px`);
  root.style.setProperty("--pick-y", `${y}px`);
  for (const btn of buttons) {
    const [dx, dy] = OFFSETS[btn.dataset.table!] ?? [0, 0];
    btn.style.left = `${x + dx}px`;
    btn.style.top = `${y + dy}px`;
  }
  root.classList.remove("shown");
  // Force a reflow so the pop-in transition replays every open.
  void root.offsetWidth;
  root.classList.add("shown");
});

listen<ThemePayload>("theme://changed", (e) => {
  applyTheme(e.payload);
});
listen<boolean>("icon-recolor://changed", (e) => {
  document.documentElement.classList.toggle("icon-recolor", e.payload);
});

listen("tables://hide", () => {
  root.classList.remove("shown");
  buttons.forEach((b) => b.classList.remove("hover"));
});

// Apply the active theme + recolor state at startup (the picker loads
// hidden; by the time it's first shown the vars are already set).
(async function init() {
  try {
    const theme = await invoke<ThemePayload | null>("get_active_theme");
    if (theme) applyTheme(theme);
    if (await invoke<boolean>("load_icon_recolor").catch(() => false)) {
      document.documentElement.classList.add("icon-recolor");
    }
  } catch {}
})();
