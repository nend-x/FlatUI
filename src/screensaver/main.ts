/* =========================================================================
   Hush_UI — Screensaver
   Fullscreen OLED surface (pure black). A soft jelly ball wanders around
   the screen slowly and bumps off the corners; the clock sits in the
   middle using the clock format from settings (24h/12h). Any key/click
   fades the whole thing out smoothly and closes the window.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Settings {
  clock_24h?: boolean;
}

const ball = document.getElementById("ball")!;
const clockEl = document.getElementById("clock")!;
const dateEl = document.getElementById("date")!;
const root = document.getElementById("root")!;

let closing = false;
let clock24 = true;
let clockTimer: number | null = null;

// ===== Clock =====
function pad(n: number) {
  return n.toString().padStart(2, "0");
}

function tick() {
  const now = new Date();
  let h = now.getHours();
  let suffix = "";
  if (!clock24) {
    suffix = h >= 12 ? " PM" : " AM";
    h = h % 12 || 12;
  }
  clockEl.textContent = `${pad(h)}:${pad(now.getMinutes())}${suffix}`;
  dateEl.textContent = now.toLocaleDateString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
  });
}

function startClock() {
  if (clockTimer) window.clearInterval(clockTimer);
  tick();
  clockTimer = window.setInterval(tick, 1000);
}

// ===== Show / hide =====
listen("screensaver://shown", () => {
  closing = false;
  root.classList.remove("fading");
  void root.offsetWidth;
  root.classList.add("awake");
  startClock();
});

listen("screensaver://hidden", () => {
  /* window is already hidden by the backend */
});

function dismiss() {
  if (closing) return;
  closing = true;
  // Smooth, relaxing fade — the ball drifts out of view, nothing snaps.
  root.classList.add("fading");
  setTimeout(() => {
    root.classList.remove("awake");
    if (clockTimer) window.clearInterval(clockTimer);
    invoke("hide_screensaver");
  }, 1200);
}

// Anything dismisses it.
window.addEventListener("keydown", dismiss);
window.addEventListener("mousedown", dismiss);
window.addEventListener("wheel", dismiss, { passive: true });

// ===== Init =====
(async function init() {
  try {
    const s = await invoke<Settings>("load_settings");
    clock24 = s.clock_24h ?? true;
  } catch {
    clock24 = true;
  }
  startClock();
})();