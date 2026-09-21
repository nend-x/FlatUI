/* =========================================================================
   TABLE 1 — taskbar table. The taskbar's own icons on a small vertical
   line: max 5 on screen, wheel-scrolls with a smooth animation, same
   click / right-click behavior as the real taskbar, macOS-style hover
   magnification (hovered icon only).

   The strip is PERSISTENT once opened: moving the cursor away never
   closes it (Esc or launching an app does). Backend app updates are
   reconciled INCREMENTALLY — existing icons keep their position and
   scroll offset, new icons append to the end.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { applyTheme, type ThemePayload } from "../shared/theme";

interface TaskbarApp {
  id: string;
  name: string;
  icon_data_url: string | null;
  running: boolean;
  pinned: boolean;
  is_foreground: boolean;
}

const root = document.getElementById("tt-root")!;
const scroll = document.getElementById("tt-scroll")!;
const tooltip = document.getElementById("tt-tooltip")!;

// Theme + icon-recolor values are applied together by the shared applyTheme
// — startup load AND live theme://changed events both go through it.

async function loadInitialTheme() {
  try {
    const theme = await invoke<ThemePayload | null>("get_active_theme");
    if (theme) applyTheme(theme);
    if (await invoke<boolean>("load_icon_recolor").catch(() => false)) {
      document.documentElement.classList.add("icon-recolor");
    }
  } catch {}
}

listen<ThemePayload>("theme://changed", (e) => {
  applyTheme(e.payload);
});
listen<boolean>("icon-recolor://changed", (e) => {
  document.documentElement.classList.toggle("icon-recolor", e.payload);
});

// ===== Magnification strength (settings) — hovered icon only =====
let magnify = 1.45;
async function loadMagnify() {
  try {
    const s = await invoke<{ table_icon_magnify?: number }>("load_settings");
    if (typeof s.table_icon_magnify === "number") magnify = s.table_icon_magnify;
  } catch {}
}
listen<{ table_icon_magnify?: number }>("settings://changed", (e) => {
  if (typeof e.payload.table_icon_magnify === "number") {
    magnify = e.payload.table_icon_magnify;
    if (hoveredIdx >= 0) {
      const el = iconEls()[hoveredIdx];
      if (el) el.style.setProperty("--mag", magnify.toFixed(3));
    }
  }
});

// ===== App data + keyed icon reconciliation =====
// DOM order is append-only; the payload's order is IGNORED so a refresh can
// never yank the icons the user is currently looking at. New apps append to
// the end, gone apps are removed, live apps are updated in place.
let apps: TaskbarApp[] = [];
let appById = new Map<string, TaskbarApp>();

function appFor(el: HTMLElement): TaskbarApp | undefined {
  return appById.get(el.dataset.appId!);
}

function buildIcon(app: TaskbarApp): HTMLElement {
  const slot = document.createElement("div");
  slot.className = "tt-icon";
  slot.dataset.appId = app.id;

  // Permanent horizontal name bar LEFT of the icon — always visible.
  const name = document.createElement("div");
  name.className = "tt-name";
  name.textContent = app.name || "?";
  slot.appendChild(name);

  const box = document.createElement("div");
  box.className = "tt-icon-box";
  slot.appendChild(box);

  const indicator = document.createElement("div");
  indicator.className = "indicator";
  slot.appendChild(indicator);

  slot.addEventListener("click", () => {
    const a = appFor(slot);
    if (!a) return;
    invoke("activate_app", { appId: a.id });
    box.style.animation = "icon-pop 280ms var(--ease-spring)";
    setTimeout(() => (box.style.animation = ""), 280);
    closeStrip();
  });

  slot.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    const a = appFor(slot);
    if (a) showContextMenu(slot, a);
  });

  slot.addEventListener("mouseenter", () => {
    hoveredIdx = iconEls().indexOf(slot);
    slot.classList.add("hover");
    slot.style.setProperty("--mag", magnify.toFixed(3));
    showTooltip(slot);
  });

  slot.addEventListener("mouseleave", () => {
    if (iconEls().indexOf(slot) === hoveredIdx) {
      hoveredIdx = -1;
      slot.classList.remove("hover");
      slot.style.removeProperty("--mag");
      hideTooltip();
    }
  });

  updateIconContent(slot, app);
  return slot;
}

function updateIconContent(slot: HTMLElement, app: TaskbarApp) {
  slot.classList.toggle("running", app.running);
  slot.classList.toggle("pinned", app.pinned);
  slot.classList.toggle("foreground", app.is_foreground);

  const name = slot.querySelector(".tt-name") as HTMLElement;
  if (name && name.textContent !== (app.name || "?")) name.textContent = app.name || "?";

  const box = slot.querySelector(".tt-icon-box") as HTMLElement;
  const img = box.querySelector("img");
  const letter = box.querySelector(".fallback");

  if (app.icon_data_url) {
    if (img) {
      if (img.src !== app.icon_data_url) img.src = app.icon_data_url;
    } else {
      box.innerHTML = "";
      const el = document.createElement("img");
      el.src = app.icon_data_url;
      el.alt = app.name;
      el.draggable = false;
      box.appendChild(el);
    }
  } else if (!letter) {
    box.innerHTML = "";
    const el = document.createElement("span");
    el.className = "fallback";
    el.textContent = (app.name || "?")[0].toUpperCase();
    box.appendChild(el);
  }
}

function reconcile() {
  appById = new Map(apps.map((a) => [a.id, a]));

  // Remove icons whose app vanished.
  for (const el of Array.from(scroll.children) as HTMLElement[]) {
    if (!appById.has(el.dataset.appId!)) {
      if (el.classList.contains("hover")) {
        hoveredIdx = -1;
        hideTooltip();
      }
      el.remove();
    }
  }

  // Append new apps at the END (payload order never reorders existing icons).
  const present = new Set(
    Array.from(scroll.children).map((el) => (el as HTMLElement).dataset.appId)
  );
  for (const app of apps) {
    if (!present.has(app.id)) {
      scroll.appendChild(buildIcon(app));
    }
  }

  // Update live icons in place (running/pinned/foreground/icons/names).
  for (const el of Array.from(scroll.children) as HTMLElement[]) {
    const app = appById.get(el.dataset.appId!);
    if (app) updateIconContent(el, app);
  }

  // Clamp the scroll target into the new range without moving it otherwise.
  target = Math.max(0, Math.min(maxOffset(), target));
}

function iconEls(): HTMLElement[] {
  return Array.from(scroll.querySelectorAll<HTMLElement>(".tt-icon"));
}

let hoveredIdx = -1;

function showTooltip(el: HTMLElement) {
  const app = appFor(el);
  if (!app) return;
  tooltip.textContent = app.name;
  // Icon center is offset from the container top by the scroll offset.
  const top = el.offsetTop + el.offsetHeight / 2 - offset;
  tooltip.style.top = `${top}px`;
  tooltip.classList.add("visible");
}

function hideTooltip() {
  tooltip.classList.remove("visible");
}

// ===== Smooth wheel scrolling (max 5 icons visible) =====
const SLOT = 62;
const VISIBLE = 5;

let offset = 0;   // current (animated) offset
let target = 0;   // scroll target
let raf = 0;

function maxOffset(): number {
  const total = Math.max(0, iconEls().length * SLOT);
  const visible = VISIBLE * SLOT;
  return Math.max(0, total - visible);
}

function applyTransform() {
  scroll.style.transform = `translateY(${-offset}px)`;
}

function animate() {
  // Critically-damped-ish lerp — smooth, never overshoots the edges.
  offset += (target - offset) * 0.22;
  if (Math.abs(target - offset) < 0.4) {
    offset = target;
    applyTransform();
    raf = 0;
    return;
  }
  applyTransform();
  raf = requestAnimationFrame(animate);
}

window.addEventListener("wheel", (e) => {
  e.preventDefault();
  target = Math.max(0, Math.min(maxOffset(), target + e.deltaY * 0.9));
  if (!raf) raf = requestAnimationFrame(animate);
  // Keep the tooltip glued to its icon while scrolling.
  const hovered = iconEls()[hoveredIdx];
  if (hovered) showTooltip(hovered);
}, { passive: false });

// ===== Right-click context menu (End task — same as the taskbar) =====
function showContextMenu(anchor: HTMLElement, app: TaskbarApp) {
  document.querySelectorAll(".tt-context-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "tt-context-menu";

  const endTask = document.createElement("div");
  endTask.className = "tt-context-menu-item danger";
  endTask.textContent = "End task";
  endTask.addEventListener("click", () => {
    menu.remove();
    invoke("end_task", { appId: app.id });
    closeStrip();
  });
  menu.appendChild(endTask);

  document.body.appendChild(menu);
  // Window is 260px wide, rail 62 — the menu lives in the transparent zone.
  const rect = anchor.getBoundingClientRect();
  menu.style.left = `74px`;
  menu.style.top = `${Math.max(4, Math.min(window.innerHeight - menu.offsetHeight - 4, rect.top + rect.height / 2 - menu.offsetHeight / 2))}px`;
}

// ===== Dismissal =====
// The strip is persistent: leaving it with the cursor NEVER closes it.
// Esc, an outside click, or activating an app closes it — all with the
// pop-out animation before the window actually hides.
let closing = false;

function closeStrip() {
  if (closing) return;
  closing = true;
  root.classList.remove("shown");
  hideTooltip();
  // Let the pop-out transition (220ms) finish before the backend hides the
  // window (close_table also tears down the outside-click watcher).
  setTimeout(() => invoke("close_table", { name: "taskbar" }), 230);
}

// Backend: click anywhere outside the strip → pop out.
listen("table://taskbar-outside", () => {
  closeStrip();
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    document.querySelectorAll(".tt-context-menu").forEach((el) => el.remove());
    closeStrip();
  }
});

// Click outside the context menu closes just the menu.
document.addEventListener("mousedown", (e) => {
  const menu = document.querySelector(".tt-context-menu");
  if (menu && !menu.contains(e.target as Node)) {
    menu.remove();
  }
});

// ===== Listeners + init =====
listen<TaskbarApp[]>("taskbar://apps-updated", (e) => {
  apps = e.payload;
  reconcile();
});

listen("table://taskbar-shown", () => {
  closing = false;
  // Replay the pop-in even when the previous close skipped the animation.
  root.classList.remove("shown");
  void root.offsetWidth;
  root.classList.add("shown");
  // Fresh data — the periodic backend refresh may be up to 2s behind.
  // Reconcile keeps scroll + icons exactly where they are.
  invoke<TaskbarApp[]>("get_taskbar_apps")
    .then((a) => {
      apps = a;
      reconcile();
    })
    .catch(() => {});
});

(async function init() {
  await loadInitialTheme();
  await loadMagnify();
  try {
    apps = await invoke<TaskbarApp[]>("get_taskbar_apps");
  } catch {}
  reconcile();
})();
