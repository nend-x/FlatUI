/* =========================================================================
   LAUNCHER main — Spotlight + run dialog + window switcher + blacklist
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";

interface LauncherItem {
  id: string;
  name: string;
  path: string;
  icon_data_url: string | null;
  is_folder: boolean;
}

interface SearchResult {
  id: string;
  name: string;
  path: string;
  icon_data_url: string | null;
  is_folder: boolean;
}

interface WindowEntry {
  hwnd: number;
  title: string;
  icon_data_url: string | null;
}

let allItems: LauncherItem[] = [];
let filteredItems: LauncherItem[] = [];
let selectedIdx = 0;

// Spotlight results
let spotlightResults: SearchResult[] = [];
let spotlightSelectedIdx = 0;

const root = document.getElementById("launcher")!;
const expandOverlay = document.getElementById("expand-overlay")!;
const searchInput = document.getElementById("search") as HTMLInputElement;
const grid = document.getElementById("grid")!;
const spotlightResultsEl = document.getElementById("spotlight-results")!;
const actionsWrap = document.querySelector<HTMLElement>(".launcher-actions")!;
const searchWrap = document.querySelector<HTMLElement>(".launcher-search-wrap")!;
const appsWidget = document.getElementById("apps-widget")!;
const appsBody = document.getElementById("apps-body")!;
const minimizeAllBtn = document.getElementById("minimize-all-btn")!;
const runBtn = document.getElementById("run-btn")!;
const blacklistBtn = document.getElementById("blacklist-btn")!;
const winSwitcherOverlay = document.getElementById("win-switcher-overlay")!;
const winSwitcherGrid = document.getElementById("win-switcher-grid")!;
const blacklistOverlay = document.getElementById("blacklist-overlay")!;
const blacklistGrid = document.getElementById("blacklist-grid")!;
const runDialog = document.getElementById("run-dialog")!;
const runInput = document.getElementById("run-input") as HTMLInputElement;
const runOkBtn = document.getElementById("run-ok")!;
const runCancelBtn = document.getElementById("run-cancel")!;
const runAdminBtn = document.getElementById("run-admin")!;
const clipboardList = document.getElementById("clipboard-list")!;
const notesTextarea = document.getElementById("notes-textarea") as HTMLTextAreaElement;
const cpuFill = document.getElementById("cpu-fill")!;
const ramFill = document.getElementById("ram-fill")!;
const cpuVal = document.getElementById("cpu-val")!;
const ramVal = document.getElementById("ram-val")!;
const audioMasterSlider = document.getElementById("audio-master-slider") as HTMLInputElement;
const audioMasterVal = document.getElementById("audio-master-val")!;
const settingsBtn = document.getElementById("settings-btn")!;
const settingsOverlay = document.getElementById("settings-overlay")!;
const settingsClose = document.getElementById("settings-close")!;
const settingsSave = document.getElementById("settings-save")!;
const screenshotOverlay = document.getElementById("screenshot-overlay")!;
const screenshotCanvas = document.getElementById("screenshot-canvas") as HTMLCanvasElement;
const clipboardClearBtn = document.getElementById("clipboard-clear")!;

// ===== Clipboard widget =====
let clipboardItems: string[] = [];
let lastClipboardText = "";

async function loadClipboardWidget() {
  try {
    clipboardItems = await invoke<string[]>("load_clipboard");
    if (clipboardItems.length > 0) {
      lastClipboardText = clipboardItems[0];
    }
    renderClipboard();
  } catch {}
}

// Poll clipboard every 500ms via Rust backend (catches global Ctrl+C)
async function pollClipboard() {
  try {
    const text = await invoke<string | null>("get_clipboard_text");
    if (text && text !== lastClipboardText) {
      lastClipboardText = text;
      // Only add text items (not images)
      if (text.startsWith("data:image/")) {
        // It's an image data URL — add to clipboard
        clipboardItems.unshift(text);
      } else {
        clipboardItems.unshift(text);
      }
      // Deduplicate
      clipboardItems = [...new Set(clipboardItems)];
      if (clipboardItems.length > 15) clipboardItems.pop();
      renderClipboard();
      invoke("save_clipboard", { items: clipboardItems });
    }
  } catch {}
}

// Start polling
setInterval(pollClipboard, 500);

// ===== Clipboard clear button =====
clipboardClearBtn.addEventListener("click", () => {
  clipboardItems = [];
  lastClipboardText = "";
  renderClipboard();
  invoke("save_clipboard", { items: [] });
  // Clear system clipboard too
  invoke("set_clipboard_text", { text: "" });
});

function renderClipboard() {
  clipboardList.innerHTML = "";
  for (const item of clipboardItems.slice(0, 15)) {
    const el = document.createElement("div");
    el.className = "clipboard-item";
    el.textContent = item.length > 40 ? item.substring(0, 40) + "…" : item;
    el.title = item.startsWith("data:image/") ? "Screenshot (click to re-copy)" : item;
    el.addEventListener("click", () => {
      // Image items go back to the clipboard as a REAL image, not text
      if (item.startsWith("data:image/")) {
        invoke("set_clipboard_image", { dataUrl: item });
      } else {
        invoke("set_clipboard_text", { text: item });
      }
    });
    clipboardList.appendChild(el);
  }
  if (clipboardItems.length === 0) {
    const empty = document.createElement("div");
    empty.className = "clipboard-item";
    empty.style.color = "var(--sand-dim)";
    empty.textContent = "No history";
    clipboardList.appendChild(empty);
  }
}

// ===== Notes widget =====
let notesSaveTimer: number | null = null;

async function loadNotesWidget() {
  try {
    const text = await invoke<string>("load_notes");
    notesTextarea.value = text;
  } catch {}
  notesTextarea.addEventListener("input", () => {
    if (notesSaveTimer) clearTimeout(notesSaveTimer);
    notesSaveTimer = window.setTimeout(() => {
      invoke("save_notes", { text: notesTextarea.value });
    }, 500);
  });
}

// ===== Sysmon widget =====
async function updateSysmon() {
  try {
    const stats = await invoke<{ cpu_usage: number; ram_usage: number; ram_total_gb: number; ram_used_gb: number }>("get_system_stats");
    cpuFill.style.width = `${stats.cpu_usage}%`;
    ramFill.style.width = `${stats.ram_usage}%`;
    cpuVal.textContent = `${Math.round(stats.cpu_usage)}%`;
    ramVal.textContent = `${Math.round(stats.ram_usage)}%`;
  } catch {}
}

// ===== Audio widget =====
async function loadAudioWidget() {
  try {
    const vol = await invoke<number>("get_volume");
    const pct = Math.round(vol * 100);
    audioMasterSlider.value = String(pct);
    audioMasterVal.textContent = `${pct}%`;
  } catch {}
}

let audioDebounce: number | null = null;
audioMasterSlider?.addEventListener("input", () => {
  const vol = parseInt(audioMasterSlider.value, 10) / 100;
  audioMasterVal.textContent = `${audioMasterSlider.value}%`;
  // Emit cross-window sync event
  if (audioDebounce) clearTimeout(audioDebounce);
  audioDebounce = window.setTimeout(() => {
    invoke("set_volume", { volume: vol });
    // Also emit event for taskbar to sync
    emit("volume://changed", vol);
  }, 50);
});

// Listen for volume changes from taskbar
listen<number>("volume://changed", (e) => {
  const pct = Math.round(e.payload * 100);
  audioMasterSlider.value = String(pct);
  audioMasterVal.textContent = `${pct}%`;
});

// ===== Widget dragging =====
function makeDraggable(el: HTMLElement) {
  const header = el.querySelector(".widget-header") as HTMLElement | null;
  if (!header) return;

  let isDragging = false;
  let startX = 0;
  let startY = 0;
  let origLeft = 0;
  let origTop = 0;

  header.addEventListener("mousedown", (e) => {
    // Don't drag if clicking inside textarea/input
    if ((e.target as HTMLElement).tagName === "INPUT" || (e.target as HTMLElement).tagName === "TEXTAREA") return;
    isDragging = true;
    startX = e.clientX;
    startY = e.clientY;
    const rect = el.getBoundingClientRect();
    origLeft = rect.left;
    origTop = rect.top;
    // Switch from right/bottom positioning to left/top
    el.style.left = `${origLeft}px`;
    el.style.top = `${origTop}px`;
    el.style.right = "auto";
    el.style.bottom = "auto";
    e.preventDefault();
  });

  document.addEventListener("mousemove", (e) => {
    if (!isDragging) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    el.style.left = `${origLeft + dx}px`;
    el.style.top = `${origTop + dy}px`;
  });

  document.addEventListener("mouseup", () => {
    if (isDragging) {
      isDragging = false;
      // Save positions after drag
      if (widgetSaveTimer) clearTimeout(widgetSaveTimer);
      widgetSaveTimer = window.setTimeout(saveWidgetPositions, 300);
    }
  });
}

// Make all widgets draggable
function initWidgetDragging() {
  document.querySelectorAll(".widget").forEach((el) => {
    makeDraggable(el as HTMLElement);
  });
}

// Save widget positions
let widgetSaveTimer: number | null = null;
function saveWidgetPositions() {
  const positions: Record<string, [number, number]> = {};
  document.querySelectorAll(".widget").forEach((el) => {
    const widget = el as HTMLElement;
    const id = widget.id;
    if (!id) return;
    const left = parseFloat(widget.style.left || "0");
    const top = parseFloat(widget.style.top || "0");
    if (left > 0 || top > 0) {
      positions[id] = [left, top];
    }
  });
  invoke("save_widget_positions", { positions });
}

// Load widget positions
async function loadWidgetPositions() {
  try {
    const positions = await invoke<Record<string, [number, number]>>("load_widget_positions");
    for (const [id, [left, top]] of Object.entries(positions)) {
      const el = document.getElementById(id);
      if (el) {
        el.style.left = `${left}px`;
        el.style.top = `${top}px`;
        el.style.right = "auto";
        el.style.bottom = "auto";
        el.style.transform = "none";
      }
    }
  } catch {}
}

// ===== Launcher open/close animation state machine =====
//
// Why this exists (bugfix): the old implementation sequenced the open
// animation with wall-clock setTimeouts that were never cancelled. Toggling
// faster than an animation ran let stale timers from a previous open fire
// mid-animation — the background snapped to its final state and elements
// appeared before the cube finished. On top of that, the cube animation was
// started while the window was still hidden, so under heavy GPU load
// (games) the animation clock ran ahead of the first presented frame and
// the cube appeared speeded-up or fully skipped.
//
// The rules now:
//   1. Every show/hide request bumps a session token. Async continuations
//      (rAF waits, animationend waits, fallback timers) capture the token
//      and bail out silently if it went stale — spamming the Win key can
//      never leave orphaned callbacks mutating the DOM.
//   2. The cube only starts once the window is actually visible AND the
//      compositor has produced frames (2× requestAnimationFrame) — the
//      animation clock and the presented output start together, no matter
//      how loaded the system is.
//   3. Sequencing is driven by real `animationend` events, not guessed
//      milliseconds. Fallback timers exist only as a safety net and are
//      session-guarded.
//   4. Show: cube expands → finishes → content elements fade in.
//      Hide:  content elements fade out → fully gone → THEN the cube
//      collapses → the DOM snaps clean and the backend is told to hide
//      the window at that exact moment. The close is the exact mirror of
//      the open — content must never float over a moving background.

let launcherSession = 0;
let sessionTimers: number[] = [];

// NOTE: the OS `prefers-reduced-motion` setting is deliberately IGNORED.
// Windows reports it whenever system animations are disabled (common on
// gaming machines with "best performance" visual effects), and honoring
// it made the launcher pop in with no animation at all. The shell pins
// its own motion policy — the animation always runs.

function trackTimer(id: number): void {
  sessionTimers.push(id);
}

function cancelSessionTimers(): void {
  for (const id of sessionTimers) clearTimeout(id);
  sessionTimers = [];
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    trackTimer(window.setTimeout(resolve, ms));
  });
}

function nextFrame(): Promise<number> {
  return new Promise((resolve) => requestAnimationFrame(resolve));
}

// Wait until the webview reports itself visible and has painted two frames.
// This guarantees the cube animation starts together with the first
// *presented* frame instead of against a hidden window (which made the
// animation appear "sped up" or missing under GPU load in games).
async function awaitOnScreen(session: number, maxWaitMs = 1500): Promise<boolean> {
  const t0 = performance.now();
  while (document.visibilityState !== "visible") {
    if (launcherSession !== session) return false;
    if (performance.now() - t0 > maxWaitMs) return false;
    await sleep(16);
  }
  // Two rAFs: the first may coalesce with the show-paint, the second
  // guarantees the compositor is actually producing frames.
  await nextFrame();
  if (launcherSession !== session) return false;
  await nextFrame();
  if (launcherSession !== session) return false;
  return true;
}

// Resolve when the named CSS animation really ends on `el` (its own clock,
// not a wall-clock guess). A safety timeout above the CSS duration covers
// swallowed events; both paths resolve identically. `animationcancel` (the
// class being ripped off by a newer session) also resolves so no caller
// can ever dangle.
function waitAnimationEnd(el: Element, animationName: string, safetyMs: number): Promise<void> {
  return new Promise((resolve) => {
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      el.removeEventListener("animationend", onEnd);
      el.removeEventListener("animationcancel", onEnd);
      clearTimeout(safety);
      resolve();
    };
    const onEnd = (ev: Event) => {
      if ((ev as AnimationEvent).animationName === animationName) finish();
    };
    el.addEventListener("animationend", onEnd);
    el.addEventListener("animationcancel", onEnd);
    const safety = window.setTimeout(finish, safetyMs);
    trackTimer(safety);
  });
}

// Snap the launcher DOM to its clean pre-open state. Idempotent; safe from
// any session (in-flight callbacks are already session-guarded).
function resetLauncherDom(): void {
  root.classList.add("hidden");
  root.classList.remove("opening", "closing");
  expandOverlay.classList.remove("expanding", "expanded", "collapsing", "fading");
  searchInput.value = "";
  runDialog.classList.add("hidden");
  winSwitcherOverlay.classList.add("hidden");
  blacklistOverlay.classList.add("hidden");
  blacklistBtn.classList.remove("active");
  document.querySelectorAll(".context-menu").forEach((el) => el.remove());
  document.querySelectorAll(".modal-backdrop").forEach((el) => el.remove());
  applyFilter();
}

// Show sequence: window on screen → cube expands → cube DONE → elements in.
async function showLauncherSequence(): Promise<void> {
  const session = ++launcherSession;
  cancelSessionTimers();
  resetLauncherDom();

  // Gate on real visibility + painted frames. If that ever times out
  // (exotic webview states), open without the cube rather than freeze —
  // graceful degradation instead of a desynced animation.
  const onScreen = await awaitOnScreen(session);
  if (launcherSession !== session) return;

  if (onScreen) {
    const ended = waitAnimationEnd(expandOverlay, "cube-expand", 1500);
    expandOverlay.classList.add("expanding");
    await ended;
    if (launcherSession !== session) return;
  }
  expandOverlay.classList.remove("expanding");
  expandOverlay.classList.add("expanded");

  // Elements fade in strictly AFTER the background has finished.
  root.classList.remove("hidden");
  root.classList.add("opening");
  searchInput.focus({ preventScroll: true });

  // Refresh the apps widget so the running-window preview is fresh.
  // Fire-and-forget — never block the launcher open on this.
  void populateAppsWidget();

  // Drop .opening once the element cascade has finished (longest delay
  // 400ms + 400ms duration + slack). Session-guarded, so a rapid
  // re-toggle can never carry it into the next session.
  trackTimer(
    window.setTimeout(() => {
      if (launcherSession === session) root.classList.remove("opening");
    }, 1000)
  );
}

// Hide sequence — the exact mirror of the open sequence, in two strict
// phases:
//   Phase 1: elements fade out over a fully-expanded, static background.
//            (The content must never float over a collapsing background —
//            it reads as broken layering.)
//   Phase 2: elements fully gone → cube collapses over a clean screen →
//            DOM snaps clean and the backend hides the window at exactly
//            that moment (Rust delays its own hide as a fallback).
async function hideLauncherSequence(): Promise<void> {
  const session = ++launcherSession;
  cancelSessionTimers();

  // Nothing visible to animate (e.g. instant-hide paths) → snap clean.
  if (root.classList.contains("hidden")) {
    resetLauncherDom();
    invoke("launcher_close_finished");
    return;
  }

  // Phase 1 — elements out. The background stays fully expanded beneath
  // them for the whole cascade.
  root.classList.remove("opening"); // don't let opening styles fight the closing ones
  root.classList.add("closing");
  await waitElementCascadeOut();
  if (launcherSession !== session) return; // re-shown mid-close
  root.classList.add("hidden"); // content layer fully gone before the bg moves

  // Phase 2 — the cube collapses. expanded is removed and collapsing added
  // in one synchronous block, so no frame is ever painted in the base
  // (collapsed, invisible) state — the collapse animation starts FROM the
  // fullscreen appearance.
  expandOverlay.classList.remove("expanded");
  expandOverlay.classList.add("collapsing");
  await waitAnimationEnd(expandOverlay, "cube-collapse", 1500);
  if (launcherSession !== session) return; // re-shown mid-collapse
  resetLauncherDom();
  invoke("launcher_close_finished");
}

// Phase 1 helper: wait until the element fade-out cascade REALLY finished —
// `animationend` on every element actually running `launcher-element-out`.
// Elements that are display:none never start their animation, so they are
// filtered out and we never wait on an event that cannot fire.
function waitElementCascadeOut(): Promise<void> {
  const els = [actionsWrap, searchWrap, grid, spotlightResultsEl].filter(
    (el) => !el.classList.contains("hidden") && getComputedStyle(el).display !== "none"
  );
  if (els.length === 0) return Promise.resolve();
  return Promise.all(
    els.map((el) => waitAnimationEnd(el, "launcher-element-out", 1200))
  ).then(() => undefined);
}

// ===== Desktop grid render =====
function renderGrid() {
  grid.innerHTML = "";
  filteredItems.forEach((item, idx) => {
    const el = document.createElement("div");
    el.className = "launcher-item";
    if (idx === selectedIdx) el.classList.add("selected");
    el.style.animationDelay = `${Math.min(idx * 10, 160)}ms`;
    el.dataset.id = item.id;

    const iconBox = document.createElement("div");
    iconBox.className = "icon-box";
    if (item.icon_data_url) {
      const img = document.createElement("img");
      img.src = item.icon_data_url;
      img.alt = item.name;
      img.draggable = false;
      iconBox.appendChild(img);
    } else {
      const fb = document.createElement("span");
      fb.textContent = item.is_folder ? "▤" : (item.name || "?")[0].toUpperCase();
      fb.style.cssText = "font-family:var(--font-display);font-size:22px;color:var(--sand);";
      iconBox.appendChild(fb);
    }

    const label = document.createElement("div");
    label.className = "label";
    label.textContent = item.name;
    el.appendChild(iconBox);
    el.appendChild(label);

    el.addEventListener("click", (e) => {
      if (e.shiftKey) {
        // Shift+click → run as admin
        invoke("execute_run_admin", { command: item.path });
      } else {
        launch(item);
      }
    });
    el.addEventListener("mouseenter", () => {
      selectedIdx = idx;
      updateSelection();
    });
    el.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      showItemContextMenu(e.clientX, e.clientY, item);
    });

    grid.appendChild(el);
  });

  if (filteredItems.length === 0) {
    const empty = document.createElement("div");
    empty.style.cssText = "grid-column:1/-1;text-align:center;padding:40px;color:var(--sand-dim);font-size:12px;";
    empty.textContent = searchInput.value ? "Nothing found" : "Desktop folder is empty";
    grid.appendChild(empty);
  }
}

function updateSelection() {
  const items = grid.querySelectorAll(".launcher-item");
  items.forEach((el, idx) => {
    if (idx === selectedIdx) {
      el.classList.add("selected");
      el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    } else {
      el.classList.remove("selected");
    }
  });
}

function launch(item: LauncherItem | SearchResult) {
  const targetEl = findLaunchTarget(item);
  if (targetEl) {
    targetEl.classList.add("launching");
    // Stop spinning after 3 seconds
    setTimeout(() => {
      targetEl?.classList.remove("launching");
    }, 3000);
  }

  doLaunch(item);
  closeLauncher();
}

// Find the DOM element that matches this item (grid first, then spotlight).
// Items are matched by their unique id (full path) — NOT by display name.
// Matching by name made the launch animation (and with it the perceived
// click target) land on the first item that happened to share the label,
// e.g. spinning the "flatui" FOLDER tile while actually launching
// "flatui.exe". Plain loops with early return — no closure assignment, so
// the compiler keeps the narrowing honest.
function findLaunchTarget(item: LauncherItem | SearchResult): HTMLElement | null {
  for (const el of Array.from(grid.querySelectorAll(".launcher-item"))) {
    if ((el as HTMLElement).dataset.id === item.id) {
      return el as HTMLElement;
    }
  }
  const spotlightItems = Array.from(spotlightResultsEl.querySelectorAll(".spotlight-item"));
  for (let idx = 0; idx < spotlightItems.length; idx++) {
    if (spotlightResults[idx] && spotlightResults[idx].id === item.id) {
      return spotlightItems[idx] as HTMLElement;
    }
  }
  // Fallback for items without an id-based match (e.g. system shortcuts):
  // compare label text, but only when no id collision is possible.
  for (const el of Array.from(grid.querySelectorAll(".launcher-item"))) {
    const labelEl = el.querySelector(".label");
    if (labelEl && labelEl.textContent === item.name && !(el as HTMLElement).dataset.id) {
      return el as HTMLElement;
    }
  }
  return null;
}

function doLaunch(item: LauncherItem | SearchResult) {
  const isDesktopItem = allItems.some((d) => d.id === item.id);
  if (isDesktopItem) {
    invoke("launch_desktop_item", { itemId: item.id });
  } else {
    // Check for special FlatUI commands
    const path = item.path;
    if (path === "flatui:reboot") {
      invoke("reboot_system");
    } else if (path === "flatui:shutdown") {
      invoke("shutdown_system");
    } else if (path === "flatui:addstartup") {
      invoke("add_to_startup");
    } else if (path === "flatui:removestartup") {
      invoke("remove_from_startup");
    } else {
      invoke("execute_run", { command: path });
    }
  }
}

function closeLauncher() {
  invoke("close_launcher");
}

// External close (from Win key toggle, or close_launcher call) — animated.
listen("launcher://force-hidden", () => {
  void hideLauncherSequence();
});

// External show (from Win key toggle) — cube first, elements after.
listen("launcher://force-shown", () => {
  void showLauncherSequence();
});

// ===== Spotlight: when search non-empty, hide grid + show search results =====
let spotlightSearchTimer: number | null = null;

async function applyFilter() {
  const q = searchInput.value.trim().toLowerCase();

  if (q === "") {
    // Empty: show desktop grid, hide spotlight results
    // Cancel any pending search_programs call
    if (spotlightSearchTimer) {
      window.clearTimeout(spotlightSearchTimer);
      spotlightSearchTimer = null;
    }
    filteredItems = [...allItems];
    selectedIdx = 0;
    grid.classList.remove("hidden");
    spotlightResultsEl.classList.add("hidden");
    spotlightResultsEl.innerHTML = "";
    spotlightResults = [];
    renderGrid();
    return;
  }

  // Non-empty: filter desktop items locally + query backend for installed programs
  const localFiltered = allItems.filter((it) => it.name.toLowerCase().includes(q));

  // Show grid immediately with local results (so user sees something)
  filteredItems = localFiltered;
  selectedIdx = 0;
  if (localFiltered.length > 0) {
    grid.classList.remove("hidden");
    spotlightResultsEl.classList.add("hidden");
    renderGrid();
  } else {
    grid.classList.add("hidden");
  }

  // Debounced search_programs call
  if (spotlightSearchTimer) window.clearTimeout(spotlightSearchTimer);
  spotlightSearchTimer = window.setTimeout(async () => {
    // Check if search field is still non-empty (user may have cleared it during debounce)
    if (!searchInput.value.trim()) {
      return;
    }
    try {
      const remoteResults = await invoke<SearchResult[]>("search_programs", { query: q });
      // Double-check: user may have cleared the field while we were fetching
      if (!searchInput.value.trim()) {
        return;
      }
      spotlightResults = remoteResults;
      spotlightSelectedIdx = 0;
      renderSpotlightResults();
    } catch (err) {
      console.error("search_programs failed:", err);
    }
  }, 150);
}

function renderSpotlightResults() {
  spotlightResultsEl.innerHTML = "";

  if (spotlightResults.length === 0) {
    // If no results at all, show "not found" message
    if (filteredItems.length === 0) {
      grid.classList.add("hidden");
      const empty = document.createElement("div");
      empty.style.cssText = "color:var(--sand-dim);font-size:12px;padding:24px;text-align:center;";
      empty.textContent = "Nothing found";
      spotlightResultsEl.appendChild(empty);
      spotlightResultsEl.classList.remove("hidden");
    }
    return;
  }

  // Show spotlight results panel
  spotlightResultsEl.classList.remove("hidden");
  if (filteredItems.length === 0) {
    grid.classList.add("hidden");
  }

  spotlightResults.forEach((result, idx) => {
    const el = document.createElement("div");
    el.className = "spotlight-item";
    if (idx === spotlightSelectedIdx) el.classList.add("selected");
    el.dataset.id = result.id;

    const iconWrap = document.createElement("div");
    iconWrap.className = "spotlight-item-icon";
    if (result.icon_data_url) {
      const img = document.createElement("img");
      img.src = result.icon_data_url;
      img.alt = result.name;
      iconWrap.appendChild(img);
    } else {
      const placeholder = document.createElement("span");
      placeholder.textContent = (result.name || "?")[0].toUpperCase();
      placeholder.style.cssText = "font-family:var(--font-display);font-size:14px;color:var(--sand-dim);";
      iconWrap.appendChild(placeholder);
    }
    el.appendChild(iconWrap);

    const label = document.createElement("div");
    label.className = "spotlight-item-label";
    label.textContent = result.name;
    el.appendChild(label);

    // Truncated path
    const pathEl = document.createElement("div");
    pathEl.className = "spotlight-item-path";
    // Just the parent directory name as a hint
    const pathParts = result.path.split(/[\\/]/);
    const shortPath = pathParts.slice(-2, -1)[0] || pathParts.slice(-1)[0] || "";
    pathEl.textContent = shortPath;
    pathEl.title = result.path;
    el.appendChild(pathEl);

    el.addEventListener("click", (e) => {
      if (e.shiftKey) {
        invoke("execute_run_admin", { command: result.path });
        closeLauncher();
      } else {
        launch(result);
      }
    });
    el.addEventListener("mouseenter", () => {
      spotlightSelectedIdx = idx;
      updateSpotlightSelection();
    });

    spotlightResultsEl.appendChild(el);
  });
}

function updateSpotlightSelection() {
  const items = spotlightResultsEl.querySelectorAll(".spotlight-item");
  items.forEach((el, idx) => {
    if (idx === spotlightSelectedIdx) {
      el.classList.add("selected");
      el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    } else {
      el.classList.remove("selected");
    }
  });
}

// ===== Keyboard =====
searchInput.addEventListener("input", applyFilter);

document.addEventListener("keydown", (e) => {
  // If focus is in notes textarea or run input, don't process launcher shortcuts
  const target = e.target as HTMLElement;
  if (target.tagName === "TEXTAREA" || (target.tagName === "INPUT" && target.id !== "search")) {
    return;
  }

  // Run dialog has its own keyboard handling
  if (!runDialog.classList.contains("hidden")) {
    if (e.key === "Enter") {
      e.preventDefault();
      executeRunFromInput();
    } else if (e.key === "Escape") {
      e.preventDefault();
      hideRunDialog();
    }
    return;
  }

  // If blacklist overlay is open
  if (!blacklistOverlay.classList.contains("hidden")) {
    if (e.key === "Escape") {
      e.preventDefault();
      blacklistOverlay.classList.add("hidden");
      blacklistBtn.classList.remove("active");
    }
    return;
  }

  // If window switcher is open
  if (!winSwitcherOverlay.classList.contains("hidden")) {
    if (e.key === "Escape") {
      e.preventDefault();
      winSwitcherOverlay.classList.add("hidden");
    }
    return;
  }

  if (e.key === "Escape") {
    e.preventDefault();
    closeLauncher();
    return;
  }

  // Spotlight navigation
  const q = searchInput.value.trim();
  if (q && spotlightResults.length > 0) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      spotlightSelectedIdx = Math.min(spotlightSelectedIdx + 1, spotlightResults.length - 1);
      updateSpotlightSelection();
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      spotlightSelectedIdx = Math.max(spotlightSelectedIdx - 1, 0);
      updateSpotlightSelection();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (spotlightResults[spotlightSelectedIdx]) {
        launch(spotlightResults[spotlightSelectedIdx]);
      }
      return;
    }
    return; // don't process launcher grid keys when spotlight active
  }

  // Regular grid navigation
  if (filteredItems.length === 0) return;
  if (e.key === "Enter") {
    e.preventDefault();
    if (filteredItems[selectedIdx]) launch(filteredItems[selectedIdx]);
    return;
  }
  const cols = parseInt(
    getComputedStyle(document.documentElement).getPropertyValue("--launcher-grid-cols").trim(),
    10
  ) || 8;
  let newIdx = selectedIdx;
  if (e.key === "ArrowRight") newIdx = Math.min(selectedIdx + 1, filteredItems.length - 1);
  else if (e.key === "ArrowLeft") newIdx = Math.max(selectedIdx - 1, 0);
  else if (e.key === "ArrowDown") newIdx = Math.min(selectedIdx + cols, filteredItems.length - 1);
  else if (e.key === "ArrowUp") newIdx = Math.max(selectedIdx - cols, 0);
  if (newIdx !== selectedIdx) {
    e.preventDefault();
    selectedIdx = newIdx;
    updateSelection();
  }
});

// Right-click → context menu (desktop grid only)
root.addEventListener("contextmenu", (e) => {
  if (e.target === root || e.target === grid) {
    e.preventDefault();
    showBackgroundContextMenu(e.clientX, e.clientY);
  }
});

root.addEventListener("click", (e) => {
  if (e.target === root || e.target === grid) {
    document.querySelectorAll(".context-menu").forEach((el) => el.remove());
  }
});

// ===== Action buttons =====
// Apps widget: a compact preview of running windows that lives where the
// old `winselect-btn` action button used to be. Click any tile to focus
// that window; click the widget chrome (header / empty body) to open the
// full window-switcher overlay (same behavior as the old button).
async function populateAppsWidget() {
  let windows: WindowEntry[] = [];
  try {
    windows = await invoke<WindowEntry[]>("get_all_windows");
  } catch (err) {
    console.error("get_all_windows failed (apps-widget):", err);
  }

  appsBody.innerHTML = "";

  if (windows.length === 0) {
    const empty = document.createElement("div");
    empty.className = "apps-empty";
    empty.textContent = "No open windows";
    appsBody.appendChild(empty);
    return;
  }

  // Show up to 6 tiles — enough to be useful, not so many that the widget
  // grows past its compact footprint. The "+N" count badge handles overflow.
  const MAX_TILES = 6;
  const shown = windows.slice(0, MAX_TILES);

  for (const w of shown) {
    const tile = document.createElement("div");
    tile.className = "apps-tile";
    tile.title = w.title;

    if (w.icon_data_url) {
      const img = document.createElement("img");
      img.src = w.icon_data_url;
      img.alt = w.title;
      tile.appendChild(img);
    } else {
      const fb = document.createElement("span");
      fb.className = "apps-tile-fallback";
      fb.textContent = (w.title || "?").charAt(0).toUpperCase();
      tile.appendChild(fb);
    }

    tile.addEventListener("click", (e) => {
      e.stopPropagation();
      invoke("activate_window", { hwnd: w.hwnd });
      closeLauncher();
    });

    appsBody.appendChild(tile);
  }

  if (windows.length > MAX_TILES) {
    const count = document.createElement("span");
    count.className = "apps-count";
    count.textContent = `+${windows.length - MAX_TILES}`;
    appsBody.appendChild(count);
  }
}

appsWidget.addEventListener("click", (e) => {
  // Clicks on a tile are handled by the tile's own listener (stopPropagation).
  // Clicks anywhere else on the widget chrome open the full switcher.
  if ((e.target as HTMLElement).closest(".apps-tile")) return;
  showWindowSwitcher();
});

appsBody.addEventListener("click", (e) => {
  // Body click also opens the switcher unless it landed on a tile.
  if ((e.target as HTMLElement).closest(".apps-tile")) return;
  showWindowSwitcher();
});

minimizeAllBtn.addEventListener("click", () => {
  // The Rust handler will hide the launcher itself.
  invoke("minimize_all_windows");
  minimizeAllBtn.classList.add("active");
  setTimeout(() => minimizeAllBtn.classList.remove("active"), 300);
});

runBtn.addEventListener("click", () => showRunDialog());

blacklistBtn.addEventListener("click", async () => {
  if (!blacklistOverlay.classList.contains("hidden")) {
    blacklistOverlay.classList.add("hidden");
    blacklistBtn.classList.remove("active");
    return;
  }
  winSwitcherOverlay.classList.add("hidden");
  blacklistOverlay.classList.remove("hidden");
  blacklistBtn.classList.add("active");
  await showBlacklist();
});

// ===== Run dialog =====
function showRunDialog() {
  runDialog.classList.remove("hidden");
  runInput.value = "";
  setTimeout(() => runInput.focus(), 50);
}

function hideRunDialog() {
  runDialog.classList.add("hidden");
  runInput.value = "";
  searchInput.focus();
}

function executeRunFromInput() {
  const cmd = runInput.value.trim();
  if (cmd) {
    invoke("execute_run", { command: cmd });
    hideRunDialog();
    closeLauncher();
  }
}

runOkBtn.addEventListener("click", executeRunFromInput);
runCancelBtn.addEventListener("click", hideRunDialog);
runAdminBtn.addEventListener("click", () => {
  const cmd = runInput.value.trim();
  if (cmd) {
    invoke("execute_run_admin", { command: cmd });
    hideRunDialog();
    closeLauncher();
  }
});

// ===== Window switcher =====
async function showWindowSwitcher() {
  winSwitcherGrid.innerHTML = "";
  winSwitcherOverlay.classList.remove("hidden");

  let windows: WindowEntry[] = [];
  try {
    windows = await invoke<WindowEntry[]>("get_all_windows");
  } catch (err) {
    console.error("get_all_windows failed:", err);
  }

  if (windows.length === 0) {
    const empty = document.createElement("div");
    empty.style.cssText = "grid-column:1/-1;text-align:center;padding:40px;color:var(--sand-dim);font-size:12px;";
    empty.textContent = "No open windows";
    winSwitcherGrid.appendChild(empty);
    return;
  }

  for (const w of windows) {
    const card = document.createElement("div");
    card.className = "win-switcher-card";

    if (w.icon_data_url) {
      const imgWrap = document.createElement("div");
      imgWrap.className = "win-switcher-card-img";
      const img = document.createElement("img");
      img.src = w.icon_data_url;
      img.alt = w.title;
      imgWrap.appendChild(img);
      card.appendChild(imgWrap);
    } else {
      const placeholder = document.createElement("div");
      placeholder.className = "win-switcher-card-img";
      placeholder.style.cssText = "display:flex;align-items:center;justify-content:center;color:var(--sand-dim);font-size:14px;font-family:var(--font-display);";
      placeholder.textContent = (w.title || "?")[0].toUpperCase();
      card.appendChild(placeholder);
    }

    const label = document.createElement("div");
    label.className = "win-switcher-card-label";
    label.textContent = w.title;
    card.appendChild(label);

    card.addEventListener("click", () => {
      invoke("activate_window", { hwnd: w.hwnd });
      winSwitcherOverlay.classList.add("hidden");
      closeLauncher();
    });

    winSwitcherGrid.appendChild(card);
  }
}

winSwitcherOverlay.addEventListener("click", (e) => {
  if (e.target === winSwitcherOverlay) {
    winSwitcherOverlay.classList.add("hidden");
  }
});

// ===== Blacklist =====
async function showBlacklist() {
  blacklistGrid.innerHTML = "";

  let windows: WindowEntry[] = [];
  try {
    windows = await invoke<WindowEntry[]>("get_all_windows");
  } catch (err) {
    console.error("get_all_windows failed:", err);
  }

  let blacklistedEntries: { exe_path: string; title: string | null; hwnd: number }[] = [];
  try {
    blacklistedEntries = await invoke("get_blacklist");
  } catch {}

  const allWindows: { hwnd: number; title: string; icon: string | null; blacklisted: boolean }[] = [];

  for (const w of windows) {
    allWindows.push({
      hwnd: w.hwnd,
      title: w.title,
      icon: w.icon_data_url,
      blacklisted: false,
    });
  }

  // For blacklisted entries not currently open, show them with their exe_path
  for (const entry of blacklistedEntries) {
    if (!allWindows.find((w) => w.hwnd === entry.hwnd && entry.hwnd !== 0)) {
      allWindows.push({
        hwnd: entry.hwnd,
        title: entry.title || entry.exe_path.split(/[\\/]/).pop() || "Unknown",
        icon: null,
        blacklisted: true,
      });
    }
  }

  // Sort: blacklisted first
  allWindows.sort((a, b) => {
    if (a.blacklisted && !b.blacklisted) return -1;
    if (!a.blacklisted && b.blacklisted) return 1;
    return a.title.localeCompare(b.title);
  });

  for (const w of allWindows) {
    const row = document.createElement("div");
    row.className = "blacklist-row";
    if (w.blacklisted) row.classList.add("pinned");

    const checkbox = document.createElement("div");
    checkbox.className = "blacklist-checkbox";
    row.appendChild(checkbox);

    if (w.icon) {
      const iconWrap = document.createElement("div");
      iconWrap.className = "blacklist-icon";
      const img = document.createElement("img");
      img.src = w.icon;
      img.alt = w.title;
      iconWrap.appendChild(img);
      row.appendChild(iconWrap);
    } else {
      const placeholder = document.createElement("div");
      placeholder.className = "blacklist-icon";
      placeholder.style.cssText = "display:flex;align-items:center;justify-content:center;color:var(--sand-dim);font-size:11px;font-family:var(--font-display);";
      placeholder.textContent = (w.title || "?")[0].toUpperCase();
      row.appendChild(placeholder);
    }

    const label = document.createElement("div");
    label.className = "blacklist-label";
    label.textContent = w.title;
    row.appendChild(label);

    row.addEventListener("click", () => {
      const newBlacklisted = !w.blacklisted;
      w.blacklisted = newBlacklisted;
      if (newBlacklisted) {
        row.classList.add("pinned");
      } else {
        row.classList.remove("pinned");
      }
      invoke("set_blacklisted", { hwnd: w.hwnd, blacklisted: newBlacklisted });
    });

    blacklistGrid.appendChild(row);
  }

  if (allWindows.length === 0) {
    const empty = document.createElement("div");
    empty.style.cssText = "text-align:center;padding:32px;color:var(--sand-dim);font-size:12px;";
    empty.textContent = "No open windows";
    blacklistGrid.appendChild(empty);
  }
}

blacklistOverlay.addEventListener("click", (e) => {
  if (e.target === blacklistOverlay) {
    blacklistOverlay.classList.add("hidden");
    blacklistBtn.classList.remove("active");
  }
});

// ===== Context menus =====
function showBackgroundContextMenu(x: number, y: number) {
  document.querySelectorAll(".context-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  const newItem = document.createElement("div");
  newItem.className = "context-menu-item";
  newItem.innerHTML = `<span>New</span><span class="context-menu-submenu-arrow">▸</span>`;

  const submenu = document.createElement("div");
  submenu.className = "context-menu-submenu";

  const folderItem = document.createElement("div");
  folderItem.className = "context-menu-item";
  folderItem.textContent = "Folder";
  folderItem.addEventListener("click", () => {
    menu.remove();
    showNewFolderDialog();
  });

  const fileItem = document.createElement("div");
  fileItem.className = "context-menu-item";
  fileItem.textContent = "File";
  fileItem.addEventListener("click", () => {
    menu.remove();
    showNewFileDialog();
  });

  submenu.appendChild(folderItem);
  submenu.appendChild(fileItem);
  newItem.appendChild(submenu);
  menu.appendChild(newItem);

  const sep = document.createElement("div");
  sep.className = "context-menu-separator";
  menu.appendChild(sep);

  const refresh = document.createElement("div");
  refresh.className = "context-menu-item";
  refresh.textContent = "Refresh";
  refresh.addEventListener("click", () => {
    menu.remove();
    invoke("refresh_desktop");
  });
  menu.appendChild(refresh);

  document.body.appendChild(menu);

  const rect = menu.getBoundingClientRect();
  if (rect.right > window.innerWidth - 8) menu.style.left = `${x - rect.width}px`;
  if (rect.bottom > window.innerHeight - 8) menu.style.top = `${y - rect.height}px`;
}

function showItemContextMenu(x: number, y: number, item: LauncherItem) {
  document.querySelectorAll(".context-menu").forEach((el) => el.remove());

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  const open = document.createElement("div");
  open.className = "context-menu-item";
  open.textContent = "Open";
  open.addEventListener("click", () => {
    menu.remove();
    launch(item);
  });
  menu.appendChild(open);

  const sep1 = document.createElement("div");
  sep1.className = "context-menu-separator";
  menu.appendChild(sep1);

  const rename = document.createElement("div");
  rename.className = "context-menu-item";
  rename.textContent = "Rename";
  rename.addEventListener("click", () => {
    menu.remove();
    showRenameDialog(item);
  });
  menu.appendChild(rename);

  const del = document.createElement("div");
  del.className = "context-menu-item danger";
  del.textContent = "Delete";
  del.addEventListener("click", () => {
    menu.remove();
    invoke("delete_desktop_item", { itemId: item.id });
  });
  menu.appendChild(del);

  document.body.appendChild(menu);

  const rect = menu.getBoundingClientRect();
  if (rect.right > window.innerWidth - 8) menu.style.left = `${x - rect.width}px`;
  if (rect.bottom > window.innerHeight - 8) menu.style.top = `${y - rect.height}px`;
}

// ===== Dialogs =====
function showNewFolderDialog() {
  showDialog("New folder", "", "Folder name", (name) => {
    invoke("create_desktop_item", { name, isFolder: true });
  });
}

function showNewFileDialog() {
  showDialog("New file", "", "name.extension", (name) => {
    invoke("create_desktop_item", { name, isFolder: false });
  });
}

function showRenameDialog(item: LauncherItem) {
  showDialog("Rename", item.name, "New name", (name) => {
    invoke("rename_desktop_item", { itemId: item.id, newName: name });
  });
}

function showDialog(
  title: string,
  initialValue: string,
  placeholder: string,
  onConfirm: (name: string) => void
) {
  document.querySelectorAll(".modal-backdrop").forEach((el) => el.remove());

  const backdrop = document.createElement("div");
  backdrop.className = "modal-backdrop";

  const dialog = document.createElement("div");
  dialog.className = "modal-dialog";

  const titleEl = document.createElement("div");
  titleEl.className = "modal-title";
  titleEl.textContent = title;

  const input = document.createElement("input");
  input.className = "modal-input";
  input.type = "text";
  input.value = initialValue;
  input.placeholder = placeholder;
  input.autofocus = true;

  const actions = document.createElement("div");
  actions.className = "modal-actions";

  const cancel = document.createElement("button");
  cancel.className = "modal-btn";
  cancel.textContent = "Cancel";

  const confirm = document.createElement("button");
  confirm.className = "modal-btn primary";
  confirm.textContent = "OK";

  actions.appendChild(cancel);
  actions.appendChild(confirm);
  dialog.appendChild(titleEl);
  dialog.appendChild(input);
  dialog.appendChild(actions);
  backdrop.appendChild(dialog);
  document.body.appendChild(backdrop);

  setTimeout(() => input.focus(), 10);

  const close = () => backdrop.remove();
  cancel.addEventListener("click", close);
  backdrop.addEventListener("click", (e) => {
    if (e.target === backdrop) close();
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      const v = input.value.trim();
      if (v) {
        onConfirm(v);
        close();
      }
    } else if (e.key === "Escape") {
      close();
    }
  });
  confirm.addEventListener("click", () => {
    const v = input.value.trim();
    if (v) {
      onConfirm(v);
      close();
    }
  });
}

// ===== Settings =====
// ===== Screenshot =====
// Listen for screenshot events from taskbar button
listen<string>("screenshot://taken", (event) => {
  startScreenshotWithData(event.payload);
});

async function startScreenshotWithData(dataUrl: string) {
    screenshotOverlay.classList.remove("hidden");
    const ctx = screenshotCanvas.getContext("2d")!;
    const img = new Image();

    // Map viewport (CSS px) coordinates to native screenshot pixels so the
    // crop is 1:1 with the real screen regardless of DPI scaling (Lightshot-style).
    const toImgCoords = (clientX: number, clientY: number) => {
      const rect = screenshotCanvas.getBoundingClientRect();
      const sx = img.naturalWidth / rect.width;
      const sy = img.naturalHeight / rect.height;
      return { x: (clientX - rect.left) * sx, y: (clientY - rect.top) * sy };
    };

    img.onload = () => {
      // Backing store at native screenshot resolution; the canvas ELEMENT is
      // stretched over the window by CSS, so the preview stays full-screen.
      screenshotCanvas.width = img.naturalWidth;
      screenshotCanvas.height = img.naturalHeight;
      ctx.drawImage(img, 0, 0);
      // Dim the screenshot
      ctx.fillStyle = "rgba(0, 0, 0, 0.3)";
      ctx.fillRect(0, 0, screenshotCanvas.width, screenshotCanvas.height);
    };
    img.src = dataUrl;

    let isSelecting = false;
    let startX = 0, startY = 0;

    screenshotCanvas.onmousedown = (e) => {
      isSelecting = true;
      const p = toImgCoords(e.clientX, e.clientY);
      startX = p.x;
      startY = p.y;
    };

    screenshotCanvas.onmousemove = (e) => {
      if (!isSelecting) return;
      const p = toImgCoords(e.clientX, e.clientY);
      const x = Math.min(startX, p.x);
      const y = Math.min(startY, p.y);
      const w = Math.abs(p.x - startX);
      const h = Math.abs(p.y - startY);
      // Redraw
      ctx.clearRect(0, 0, screenshotCanvas.width, screenshotCanvas.height);
      ctx.drawImage(img, 0, 0);
      ctx.fillStyle = "rgba(0, 0, 0, 0.3)";
      ctx.fillRect(0, 0, screenshotCanvas.width, screenshotCanvas.height);
      // Clear selected area
      ctx.clearRect(x, y, w, h);
      ctx.drawImage(img, x, y, w, h, x, y, w, h);
      // Draw border (constant on-screen thickness regardless of scale)
      const rect = screenshotCanvas.getBoundingClientRect();
      ctx.strokeStyle = "rgba(184, 131, 90, 0.8)";
      ctx.lineWidth = 2 / (img.naturalWidth / rect.width);
      ctx.strokeRect(x, y, w, h);
    };

    screenshotCanvas.onmouseup = (e) => {
      if (!isSelecting) return;
      isSelecting = false;
      const p = toImgCoords(e.clientX, e.clientY);
      const x = Math.min(startX, p.x);
      const y = Math.min(startY, p.y);
      let w = Math.abs(p.x - startX);
      let h = Math.abs(p.y - startY);

      // Clamp to the captured image bounds
      const cx = Math.max(0, Math.min(x, img.naturalWidth - 1));
      const cy = Math.max(0, Math.min(y, img.naturalHeight - 1));
      w = Math.min(w, img.naturalWidth - cx);
      h = Math.min(h, img.naturalHeight - cy);

      if (w < 5 || h < 5) {
        // Too small — just close
        screenshotOverlay.classList.add("hidden");
        return;
      }

      // Extract selected area at NATIVE screen resolution
      const iw = Math.max(1, Math.round(w));
      const ih = Math.max(1, Math.round(h));
      const tempCanvas = document.createElement("canvas");
      tempCanvas.width = iw;
      tempCanvas.height = ih;
      const tempCtx = tempCanvas.getContext("2d")!;
      tempCtx.drawImage(img, cx, cy, w, h, 0, 0, iw, ih);
      const selectedDataUrl = tempCanvas.toDataURL("image/png");

      // Copy to system clipboard as a REAL image (CF_DIB + PNG via Win32 —
      // same approach Lightshot uses). navigator.clipboard is unreliable
      // inside WebView2 and silently fails for images.
      invoke("set_clipboard_image", { dataUrl: selectedDataUrl });

      // Save to %TEMP%\flatshot.png via backend
      invoke("save_clipboard_image", { dataUrl: selectedDataUrl });

      // Add to clipboard widget
      if (!clipboardItems.includes(selectedDataUrl)) {
        clipboardItems.unshift(selectedDataUrl);
        if (clipboardItems.length > 15) clipboardItems.pop();
        renderClipboard();
        invoke("save_clipboard", { items: clipboardItems });
      }

      screenshotOverlay.classList.add("hidden");
      // Hide launcher if it was only shown for screenshot
      invoke("close_launcher");
    };

    // Esc to cancel
    const escHandler = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        screenshotOverlay.classList.add("hidden");
        document.removeEventListener("keydown", escHandler);
        // Hide launcher if it was only shown for screenshot
        invoke("close_launcher");
      }
    };
    document.addEventListener("keydown", escHandler);
}

// ===== Init =====
async function init() {
  try {
    allItems = await invoke<LauncherItem[]>("get_desktop_items");
    applyFilter();
  } catch (err) {
    console.error("get_desktop_items failed:", err);
    allItems = [];
    applyFilter();
  }

  await listen<LauncherItem[]>("launcher://items-updated", (e) => {
    allItems = e.payload;
    applyFilter();
  });

  // Load widgets
  loadClipboardWidget();
  loadNotesWidget();
  loadAudioWidget();
  updateSysmon();
  setInterval(updateSysmon, 2000);
  initWidgetDragging();
  await loadWidgetPositions();

  // (The running-build version badge used to live in the bottom-left
  // corner of the launcher. It has been removed from the surface — the
  // backend still reports the version via Tauri APIs if a tool needs it.)
  // NOTE: no focus timer here — the launcher page loads hidden; the show
  // sequence (showLauncherSequence) focuses the search input at the right
  // moment, after the background animation has finished.
}

init();
