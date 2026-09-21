/* =========================================================================
   TABLE 3 — widgets table. The flatlight launcher's widgets in a small
   non-fullscreen movable window (position persists to tables.json).
   Same design language: grain panels, launcher.css widget styles.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { applyTheme, type ThemePayload } from "../shared/theme";

const root = document.getElementById("mt-root")!;
const greeting = document.getElementById("mt-greeting")!;
const clipboardList = document.getElementById("clipboard-list")!;
const notesTextarea = document.getElementById("notes-textarea") as HTMLTextAreaElement;
const cpuFill = document.getElementById("cpu-fill")!;
const ramFill = document.getElementById("ram-fill")!;
const cpuVal = document.getElementById("cpu-val")!;
const ramVal = document.getElementById("ram-val")!;
const audioSlider = document.getElementById("audio-master-slider") as HTMLInputElement;
const audioVal = document.getElementById("audio-master-val")!;
const clipboardClearBtn = document.getElementById("clipboard-clear")!;

// ===== Show / hide =====
let closing = false;

listen("table://widgets-shown", () => {
  closing = false;
  root.classList.remove("shown");
  void root.offsetWidth;
  root.classList.add("shown");
  // Fresh data on every open (window positions/toggles may have changed).
  void applyVisibility();
  void updateSysmon();
  void loadAudio();
  void renderGreeting();
});

function close() {
  if (closing) return;
  closing = true;
  root.classList.remove("shown");
  // Let the pop-out play before the backend hides the window.
  setTimeout(() => invoke("close_table", { name: "widgets" }), 240);
}

document.getElementById("mt-close")!.addEventListener("click", close);
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") close();
});

// ===== Greeting (user name from settings — the new text-input setting) =====
async function renderGreeting() {
  try {
    const s = await invoke<{ user_name?: string }>("load_settings");
    const name = (s.user_name || "").trim();
    greeting.innerHTML = name
      ? `Hush table — <b>${escapeHtml(name)}</b>`
      : "Hush table";
  } catch {
    greeting.textContent = "Hush table";
  }
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c] || c)
  );
}

// ===== Widget visibility (shared config with the launcher + settings table)
const WIDGET_IDS = [
  "clipboard-widget",
  "notes-widget",
  "sysmon-widget",
  "audio-widget",
] as const;

async function applyVisibility() {
  try {
    const visibility = await invoke<Record<string, boolean>>("load_widget_visibility");
    for (const id of WIDGET_IDS) {
      const el = document.getElementById(id);
      if (el) el.style.display = (visibility[id] ?? true) ? "" : "none";
    }
  } catch {}
}

// table-settings broadcasts changes so an open widgets table updates live.
listen<Record<string, boolean>>("widgets://visibility", (e) => {
  for (const [id, visible] of Object.entries(e.payload)) {
    const el = document.getElementById(id);
    if (el) el.style.display = visible ? "" : "none";
  }
});

// ===== Sysmon widget (visibility-gated — same idle-hang guard as the launcher)
async function updateSysmon() {
  if (document.visibilityState !== "visible") return;
  if (document.getElementById("sysmon-widget")?.style.display === "none") return;
  try {
    const stats = await invoke<{ cpu_usage: number; ram_usage: number }>("get_system_stats");
    cpuFill.style.width = `${stats.cpu_usage}%`;
    ramFill.style.width = `${stats.ram_usage}%`;
    cpuVal.textContent = `${Math.round(stats.cpu_usage)}%`;
    ramVal.textContent = `${Math.round(stats.ram_usage)}%`;
  } catch {}
}

setInterval(updateSysmon, 2000);

// ===== Audio widget =====
async function loadAudio() {
  if (document.getElementById("audio-widget")?.style.display === "none") return;
  try {
    const vol = await invoke<number>("get_volume");
    const pct = Math.round(vol * 100);
    audioSlider.value = String(pct);
    audioVal.textContent = `${pct}%`;
  } catch {}
}

let audioDebounce: number | null = null;
audioSlider.addEventListener("input", () => {
  const vol = parseInt(audioSlider.value, 10) / 100;
  audioVal.textContent = `${audioSlider.value}%`;
  if (audioDebounce) window.clearTimeout(audioDebounce);
  audioDebounce = window.setTimeout(() => {
    invoke("set_volume", { volume: vol });
    void emit("volume://changed", vol);
  }, 50);
});

listen<number>("volume://changed", (e) => {
  const pct = Math.round(e.payload * 100);
  audioSlider.value = String(pct);
  audioVal.textContent = `${pct}%`;
});

// ===== Clipboard widget =====
let clipboardItems: string[] = [];
let lastClipboardText = "";

function renderClipboard() {
  clipboardList.innerHTML = "";
  for (const item of clipboardItems.slice(0, 15)) {
    const el = document.createElement("div");
    el.className = "clipboard-item";
    el.textContent = item.length > 40 ? item.substring(0, 40) + "…" : item;
    el.title = item.startsWith("data:image/") ? "Screenshot (click to re-copy)" : item;
    el.addEventListener("click", () => {
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

async function pollClipboard() {
  if (document.visibilityState !== "visible") return;
  if (document.getElementById("clipboard-widget")?.style.display === "none") return;
  try {
    const text = await invoke<string | null>("get_clipboard_text");
    if (text && text !== lastClipboardText) {
      lastClipboardText = text;
      clipboardItems.unshift(text);
      clipboardItems = [...new Set(clipboardItems)];
      if (clipboardItems.length > 15) clipboardItems.pop();
      renderClipboard();
      invoke("save_clipboard", { items: clipboardItems });
    }
  } catch {}
}

setInterval(pollClipboard, 500);

clipboardClearBtn.addEventListener("click", () => {
  clipboardItems = [];
  lastClipboardText = "";
  renderClipboard();
  invoke("save_clipboard", { items: [] });
  invoke("set_clipboard_text", { text: "" });
});

// ===== Notes widget =====
let notesSaveTimer: number | null = null;

async function loadNotes() {
  if (document.getElementById("notes-widget")?.style.display === "none") return;
  try {
    notesTextarea.value = await invoke<string>("load_notes");
  } catch {}
}

notesTextarea.addEventListener("input", () => {
  if (notesSaveTimer) window.clearTimeout(notesSaveTimer);
  notesSaveTimer = window.setTimeout(() => {
    invoke("save_notes", { text: notesTextarea.value });
  }, 500);
});

// Theme + icon recolor come from the shared module — colors and per-theme
// icon-recolor values are always applied together.

listen<ThemePayload>("theme://changed", (e) => {
  applyTheme(e.payload);
});

listen<boolean>("icon-recolor://changed", (e) => {
  document.documentElement.classList.toggle("icon-recolor", e.payload);
});

// ===== Init =====
(async function init() {
  try {
    const theme = await invoke<ThemePayload | null>("get_active_theme");
    if (theme) applyTheme(theme);
  } catch {}
  try {
    if (await invoke<boolean>("load_icon_recolor").catch(() => false)) {
      document.documentElement.classList.add("icon-recolor");
    }
  } catch {}
  await applyVisibility();
  await renderGreeting();
  await loadNotes();
  await loadAudio();
  renderClipboard();
  try {
    const items = await invoke<string[]>("load_clipboard");
    clipboardItems = items;
    if (items.length > 0) lastClipboardText = items[0];
    renderClipboard();
  } catch {}
})();
