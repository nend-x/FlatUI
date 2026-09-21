/* =========================================================================
   TASKBAR main — Win11 style centered icons + window switcher flyout
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen, emit, type UnlistenFn } from "@tauri-apps/api/event";

interface TaskbarApp {
  id: string;
  name: string;
  icon_data_url: string | null;
  running: boolean;
  pinned: boolean;
  is_foreground: boolean;
}

// Per-theme icon recolor hue/sat values. Must match the launcher's
// THEME_ICON_RECOLOR map. Used when applying a theme so the taskbar's
// icon recolor filter matches the launcher's.
const THEME_ICON_RECOLOR: Record<string, { hue: string; sat: string; brightness: string }> = {
  "material3-dark": { hue: "218deg", sat: "1.0", brightness: "0.95" },
};

interface WindowPreview {
  hwnd: number;
  title: string;
  icon_data_url: string | null;
}

// ===== State =====
let apps: TaskbarApp[] = [];
let lastAppsSignature = "";

// ===== DOM refs =====
const zoneCenter = document.getElementById("zone-center")!;
const clockTime = document.getElementById("clock-time")!;
const clockDate = document.getElementById("clock-date")!;
const clockWrap = document.getElementById("clock")!;
const switcherEl = document.getElementById("win-switcher")!;
const volumeSlider = document.getElementById("volume-slider") as HTMLInputElement;
const screenshotBtn = document.getElementById("screenshot-btn")!;
const langIndicator = document.getElementById("lang-indicator")!;

let switcherHideTimer: number | null = null;

// ===== Screenshot button =====
screenshotBtn.addEventListener("click", async () => {
  try {
    const dataUrl = await invoke<string | null>("take_screenshot");
    if (!dataUrl) return;
    // Show launcher window so screenshot overlay is visible
    await invoke("show_launcher_for_screenshot");
    // Small delay to let the window appear
    setTimeout(() => {
      emit("screenshot://taken", dataUrl);
    }, 100);
  } catch (err) {
    console.error("screenshot failed:", err);
  }
});

// ===== Language indicator =====
async function updateLanguage() {
  try {
    const lang = await invoke<string>("get_language");
    const isCaps = lang === lang.toUpperCase() && lang !== "??";
    langIndicator.textContent = lang.toLowerCase();
    if (isCaps) {
      langIndicator.classList.add("caps");
    } else {
      langIndicator.classList.remove("caps");
    }
  } catch {}
}

// ===== Volume slider =====
async function loadVolume() {
  try {
    const vol = await invoke<number>("get_volume");
    const pct = Math.round(vol * 100);
    volumeSlider.value = String(pct);
  } catch {}
}

let volumeDebounce: number | null = null;
volumeSlider?.addEventListener("input", () => {
  const vol = parseInt(volumeSlider.value, 10) / 100;
  if (volumeDebounce) clearTimeout(volumeDebounce);
  volumeDebounce = window.setTimeout(() => {
    invoke("set_volume", { volume: vol });
    // Emit cross-window sync for launcher
    emit("volume://changed", vol);
  }, 50);
});

// Listen for volume changes from launcher (cross-window sync)
listen<number>("volume://changed", (e) => {
  const pct = Math.round(e.payload * 100);
  volumeSlider.value = String(pct);
});

// Load volume on init
loadVolume();

// ===== Taskbar icon context menu =====
function showTaskbarContextMenu(x: number, y: number, app: TaskbarApp) {
  // Remove existing menus
  document.querySelectorAll(".tb-context-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "tb-context-menu";

  const endTask = document.createElement("div");
  endTask.className = "tb-context-menu-item danger";
  endTask.textContent = "End task";
  endTask.addEventListener("click", () => {
    menu.remove();
    invoke("end_task", { appId: app.id });
  });
  menu.appendChild(endTask);

  document.body.appendChild(menu);
  menu.style.left = `${x}px`;
  menu.style.top = `${Math.max(0, y - menu.offsetHeight - 4)}px`;

  // Close on outside click
  setTimeout(() => {
    const handler = (ev: MouseEvent) => {
      if (!menu.contains(ev.target as Node)) {
        menu.remove();
        document.removeEventListener("mousedown", handler);
      }
    };
    document.addEventListener("mousedown", handler);
  }, 0);
}

// Close context menu on Escape
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    document.querySelectorAll(".tb-context-menu").forEach((el) => el.remove());
  }
});

// ===== Render icons (centered) — only re-render if list actually changed =====
function maybeRenderIcons() {
  const sig = apps
    .map((a) => `${a.id}|${a.name}|${a.running ? "r" : "-"}|${a.pinned ? "p" : "-"}|${a.is_foreground ? "f" : "-"}|${a.icon_data_url ? "i" : "-"}`)
    .join(";");
  if (sig === lastAppsSignature) return;
  lastAppsSignature = sig;
  renderIcons();
}

function renderIcons() {
  zoneCenter.innerHTML = "";
  for (const app of apps) {
    const el = document.createElement("div");
    el.className = "taskbar-icon";
    if (app.running) el.classList.add("running");
    if (app.pinned) el.classList.add("pinned");
    if (app.is_foreground) el.classList.add("foreground");
    el.dataset.appId = app.id;

    if (app.icon_data_url) {
      const img = document.createElement("img");
      img.src = app.icon_data_url;
      img.alt = app.name;
      img.draggable = false;
      el.appendChild(img);
    } else {
      const letter = document.createElement("span");
      letter.textContent = (app.name || "?")[0].toUpperCase();
      letter.style.cssText =
        "font-family:var(--font-display);font-size:12px;color:var(--sand);font-weight:500;";
      el.appendChild(letter);
    }

    const dot = document.createElement("div");
    dot.className = "indicator";
    el.appendChild(dot);

    const tip = document.createElement("div");
    tip.className = "taskbar-tooltip";
    tip.textContent = app.name;
    el.appendChild(tip);

    el.addEventListener("click", () => {
      invoke("activate_app", { appId: app.id });
      el.style.animation = "icon-pop 280ms var(--ease-spring)";
      setTimeout(() => (el.style.animation = ""), 280);
    });

    el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      showTaskbarContextMenu(e.clientX, e.clientY, app);
    });

    zoneCenter.appendChild(el);
  }
}

// ===== Window switcher flyout (replaces Aero Peek) =====
async function showSwitcher(app: TaskbarApp, anchor: HTMLElement) {
  if (!app.running) return;

  let previews: WindowPreview[] = [];
  try {
    previews = await invoke<WindowPreview[]>("get_app_windows", { appId: app.id });
  } catch (err) {
    console.error("get_app_windows failed:", err);
    return;
  }

  if (previews.length === 0) return;

  // Position flyout above the icon, centered on it
  const rect = anchor.getBoundingClientRect();
  const flyoutWidth = previews.length * 186 + (previews.length - 1) * 6 + 16;
  const flyoutLeft = Math.max(
    8,
    Math.min(
      window.innerWidth - flyoutWidth - 8,
      rect.left + rect.width / 2 - flyoutWidth / 2
    )
  );
  switcherEl.style.left = `${flyoutLeft}px`;
  switcherEl.innerHTML = "";

  for (const p of previews) {
    const thumb = document.createElement("div");
    thumb.className = "win-switcher-thumb";

    if (p.icon_data_url) {
      const iconWrap = document.createElement("div");
      iconWrap.className = "win-switcher-icon-wrap";
      const img = document.createElement("img");
      img.src = p.icon_data_url;
      img.alt = p.title;
      iconWrap.appendChild(img);
      thumb.appendChild(iconWrap);
    }

    const label = document.createElement("div");
    label.className = "win-switcher-label";
    label.textContent = p.title;
    thumb.appendChild(label);

    thumb.addEventListener("click", () => {
      invoke("activate_window", { hwnd: p.hwnd });
      hideSwitcher();
    });

    switcherEl.appendChild(thumb);
  }

  switcherEl.classList.add("visible");
}

function hideSwitcher() {
  switcherEl.classList.remove("visible");
  switcherEl.innerHTML = "";
}

function scheduleHideSwitcher() {
  if (switcherHideTimer) window.clearTimeout(switcherHideTimer);
  switcherHideTimer = window.setTimeout(() => {
    if (!switcherEl.matches(":hover")) hideSwitcher();
  }, 300);
}

switcherEl.addEventListener("mouseenter", () => {
  if (switcherHideTimer) {
    window.clearTimeout(switcherHideTimer);
    switcherHideTimer = null;
  }
});

switcherEl.addEventListener("mouseleave", () => {
  hideSwitcher();
});

// ===== Clock =====
// 0.2: format honors the settings-table segmented control (24h / 12h),
// loaded at boot and updated live via settings://changed.
let clock24h = true;

async function loadClockFormat() {
  try {
    const s = await invoke<{ clock_24h?: boolean }>("load_settings");
    clock24h = s.clock_24h ?? true;
  } catch {}
}

listen<{ clock_24h?: boolean }>("settings://changed", (e) => {
  if (typeof e.payload.clock_24h === "boolean") {
    clock24h = e.payload.clock_24h;
    updateClock();
  }
});

function updateClock() {
  const now = new Date();
  let hh: number;
  let suffix = "";
  if (clock24h) {
    hh = now.getHours();
  } else {
    hh = now.getHours() % 12;
    if (hh === 0) hh = 12;
    suffix = now.getHours() < 12 ? " AM" : " PM";
  }
  const mm = now.getMinutes().toString().padStart(2, "0");
  const hhStr = clock24h ? hh.toString().padStart(2, "0") : String(hh);
  clockTime.textContent = `${hhStr}:${mm}${suffix}`;
  const dayShort = now.toLocaleDateString("ru-RU", { weekday: "short" });
  const dayNum = now.getDate().toString().padStart(2, "0");
  const monthShort = now.toLocaleDateString("ru-RU", { month: "short" });
  clockDate.textContent = `${dayShort} ${dayNum} ${monthShort}`;
}

// ===== Listeners =====
const unlistenFns: UnlistenFn[] = [];

async function setupListeners() {
  unlistenFns.push(
    await listen<TaskbarApp[]>("taskbar://apps-updated", (e) => {
      apps = e.payload;
      maybeRenderIcons();
    })
  );

  // Icon recolor toggle — applies the .icon-recolor class to the taskbar root
  unlistenFns.push(
    await listen<boolean>("icon-recolor://changed", (e) => {
      const taskbarRoot = document.querySelector(".taskbar-root");
      if (taskbarRoot) {
        if (e.payload) {
          taskbarRoot.classList.add("icon-recolor");
        } else {
          taskbarRoot.classList.remove("icon-recolor");
        }
      }
    })
  );

  // Theme change — applies the theme's colors as CSS variables on :root
  unlistenFns.push(
    await listen<{ name: string; colors: Record<string, string> }>("theme://changed", (e) => {
      const root = document.documentElement;
      const colors = e.payload.colors;
      // Map the theme colors to CSS variables (keys match the serde-renamed
      // ThemeColors field names with -- prefix)
      for (const [key, value] of Object.entries(colors)) {
        root.style.setProperty("--" + key, value);
      }
      // Build the SVG noise tile with the theme's sand-cream RGB values
      const creamRgb = colors["sand-cream-rgb"];
      if (creamRgb) {
        const parts = creamRgb.split(",").map((s) => parseFloat(s.trim()));
        if (parts.length === 3) {
          const [r, g, b] = parts;
          const rNorm = (r / 255).toFixed(3);
          const gNorm = (g / 255).toFixed(3);
          const bNorm = (b / 255).toFixed(3);
          // Launcher-level grain (high visibility — 3 octaves, 0.28 alpha)
          const noiseSvg = `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 ${rNorm}  0 0 0 0 ${gNorm}  0 0 0 0 ${bNorm}  0 0 0 0.252 0'/></filter><rect width='100%25' height='100%25' filter='url(%23n)'/></svg>")`;
          root.style.setProperty("--grain-noise-svg", noiseSvg);
          // Taskbar-level grain (LOW visibility — 2 octaves, 0.13 alpha.
          // The increased grain looks good on the fullscreen launcher but
          // is too busy on the thin 40px taskbar strip.)
          const taskbarNoiseSvg = `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 ${rNorm}  0 0 0 0 ${gNorm}  0 0 0 0 ${bNorm}  0 0 0 0.13 0'/></filter><rect width='100%25' height='100%25' filter='url(%23n)'/></svg>")`;
          root.style.setProperty("--taskbar-grain-noise-svg", taskbarNoiseSvg);
        }
      }
      // Set per-theme icon recolor values (must match launcher's THEME_ICON_RECOLOR)
      const recolor = THEME_ICON_RECOLOR[e.payload.name];
      if (recolor) {
        root.style.setProperty("--icon-recolor-hue", recolor.hue);
        root.style.setProperty("--icon-recolor-sat", recolor.sat);
        root.style.setProperty("--icon-recolor-brightness", recolor.brightness);
      }
    })
  );
}

clockWrap.addEventListener("click", () => invoke("toggle_calendar_flyout"));

// ===== Init =====
async function init() {
  updateClock();
  setInterval(updateClock, 1000);
  void loadClockFormat();
  updateLanguage();
  setInterval(updateLanguage, 500);

  try {
    apps = await invoke<TaskbarApp[]>("get_taskbar_apps");
    maybeRenderIcons();
  } catch (err) {
    console.error("get_taskbar_apps failed:", err);
  }

  await setupListeners();

  // Load icon recolor state on startup (in case the launcher isn't open yet)
  try {
    const iconRecolorEnabled = await invoke<boolean>("load_icon_recolor");
    const taskbarRoot = document.querySelector(".taskbar-root");
    if (taskbarRoot && iconRecolorEnabled) {
      taskbarRoot.classList.add("icon-recolor");
    }
  } catch {}

  // Load active theme on startup (in case the launcher isn't open yet)
  try {
    const theme = await invoke<{ name: string; colors: Record<string, string> } | null>("get_active_theme");
    if (theme) {
      const root = document.documentElement;
      for (const [key, value] of Object.entries(theme.colors)) {
        root.style.setProperty("--" + key, value);
      }
      // Build the SVG noise tile
      const creamRgb = theme.colors["sand-cream-rgb"];
      if (creamRgb) {
        const parts = creamRgb.split(",").map((s) => parseFloat(s.trim()));
        if (parts.length === 3) {
          const [r, g, b] = parts;
          const rNorm = (r / 255).toFixed(3);
          const gNorm = (g / 255).toFixed(3);
          const bNorm = (b / 255).toFixed(3);
          // Launcher-level grain (high visibility)
          const noiseSvg = `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.85' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 ${rNorm}  0 0 0 0 ${gNorm}  0 0 0 0 ${bNorm}  0 0 0 0.252 0'/></filter><rect width='100%25' height='100%25' filter='url(%23n)'/></svg>")`;
          root.style.setProperty("--grain-noise-svg", noiseSvg);
          // Taskbar-level grain (low visibility — less busy on the thin strip)
          const taskbarNoiseSvg = `url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 ${rNorm}  0 0 0 0 ${gNorm}  0 0 0 0 ${bNorm}  0 0 0 0.13 0'/></filter><rect width='100%25' height='100%25' filter='url(%23n)'/></svg>")`;
          root.style.setProperty("--taskbar-grain-noise-svg", taskbarNoiseSvg);
        }
      }
      // Set per-theme icon recolor values
      const recolor = THEME_ICON_RECOLOR[theme.name];
      if (recolor) {
        root.style.setProperty("--icon-recolor-hue", recolor.hue);
        root.style.setProperty("--icon-recolor-sat", recolor.sat);
        root.style.setProperty("--icon-recolor-brightness", recolor.brightness);
      }
    }
  } catch {}

  setInterval(async () => {
    try {
      apps = await invoke<TaskbarApp[]>("get_taskbar_apps");
      maybeRenderIcons();
    } catch {}
  }, 3000);
}

init();

window.addEventListener("beforeunload", () => {
  unlistenFns.forEach((fn) => fn());
});
