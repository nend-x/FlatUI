/* =========================================================================
   Hush_UI — Screensaver (ribbon aurora)
   Fullscreen OLED surface. Wide monochrome light ribbons flow across the
   screen on layered sine paths — bright enough to notice from across the
   room, still flat, still quiet. A fine dot grid beneath the ribbons
   brightens as each ribbon passes over it, and the clock sits centered
   with a soft pulse halo. Any input fades it all out smoothly.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Settings {
  clock_24h?: boolean;
}

const root = document.getElementById("root")!;
const canvas = document.getElementById("field") as HTMLCanvasElement;
const ctx = canvas.getContext("2d")!;
const clockEl = document.getElementById("clock")!;
const dateEl = document.getElementById("date")!;

let closing = false;
let clock24 = true;
let clockTimer: number | null = null;
let rafId = 0;

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
    weekday: "long",
    day: "numeric",
    month: "long",
  });
}

function startClock() {
  if (clockTimer) window.clearInterval(clockTimer);
  tick();
  clockTimer = window.setInterval(tick, 1000);
}

// ===== Ribbon aurora =====
// Three wide ribbons travel horizontally on independent sine paths with
// different speeds, amplitudes and thicknesses. Beneath them a sparse dot
// grid lights up as a ribbon passes over. Everything is drawn additively
// (screen-like) so overlaps bloom brighter — noticeable, but monochrome.
interface Ribbon {
  yBase: number;      // fraction of H
  amp: number;        // px (dpr-scaled at draw time)
  freq: number;       // spatial waves across the width
  speed: number;      // phase advance per second
  thickness: number;  // px
  alpha: number;      // peak brightness
  phase: number;
  drift: number;      // slow vertical wander rate
}

let W = 0;
let H = 0;
let dpr = 1;
let ribbons: Ribbon[] = [];
let grid: { x: number; y: number }[] = [];

function seed() {
  dpr = devicePixelRatio;
  W = canvas.width = Math.round(window.innerWidth * dpr);
  H = canvas.height = Math.round(window.innerHeight * dpr);

  ribbons = [
    { yBase: 0.32, amp: H * 0.055, freq: 1.1, speed: 0.35, thickness: H * 0.05, alpha: 0.20, phase: 0.0, drift: 0.020 },
    { yBase: 0.55, amp: H * 0.080, freq: 0.8, speed: -0.24, thickness: H * 0.035, alpha: 0.30, phase: 1.8, drift: -0.014 },
    { yBase: 0.74, amp: H * 0.045, freq: 1.6, speed: 0.5, thickness: H * 0.028, alpha: 0.24, phase: 4.1, drift: 0.026 },
  ];

  // sparse dot grid
  const step = Math.round(46 * dpr);
  grid = [];
  for (let y = step; y < H; y += step) {
    for (let x = step; x < W; x += step) {
      grid.push({ x, y });
    }
  }
}

function ribbonY(r: Ribbon, x: number, t: number) {
  const wander = Math.sin(t * 0.00021 * r.speed + r.phase * 0.7) * r.drift * H;
  return (r.yBase + wander) * H + Math.sin((x / W) * Math.PI * 2 * r.freq + t * 0.001 * r.speed + r.phase) * r.amp;
}

function drawRibbon(r: Ribbon, t: number) {
  // draw as a soft vertical gradient band that follows the sine path
  const layers = 5; // inner core to outer glow
  for (let l = layers; l >= 1; l--) {
    const spread = r.thickness * l * 0.75;
    const a = r.alpha * (l === 1 ? 1.0 : 0.28 / (l - 1));
    ctx.beginPath();
    const stepX = Math.max(8 * dpr, W / 160);
    for (let x = -spread; x <= W + spread; x += stepX) {
      const y = ribbonY(r, x, t);
      const top = y - spread * 0.5;
      if (x <= 0) ctx.moveTo(x, top);
      else ctx.lineTo(x, top);
    }
    for (let x = W + spread; x >= -spread; x -= stepX) {
      const y = ribbonY(r, x, t);
      const bot = y + spread * 0.5;
      ctx.lineTo(x, bot);
    }
    ctx.closePath();
    ctx.fillStyle = `rgba(255, 255, 255, ${a.toFixed(4)})`;
    ctx.fill();
  }
}

function frame(t: number) {
  ctx.clearRect(0, 0, W, H);
  ctx.globalCompositeOperation = "lighter";

  // dot grid — lights up near ribbons
  for (const g of grid) {
    let glow = 0;
    for (const r of ribbons) {
      const ry = ribbonY(r, g.x, t);
      const d = Math.abs(g.y - ry) / (r.thickness * 3);
      if (d < 1) glow += (1 - d) * (1 - d) * r.alpha * 1.5;
    }
    if (glow > 0.01) {
      const size = 1.1 * dpr + glow * 2.2 * dpr;
      ctx.beginPath();
      ctx.arc(g.x, g.y, size, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(255, 255, 255, ${Math.min(0.5, glow).toFixed(4)})`;
      ctx.fill();
    }
  }

  for (const r of ribbons) drawRibbon(r, t);

  ctx.globalCompositeOperation = "source-over";

  rafId = requestAnimationFrame(frame);
}

// ===== Show / dismiss =====
listen("screensaver://shown", () => {
  closing = false;
  root.classList.remove("fading");
  void root.offsetWidth;
  root.classList.add("awake");
  seed();
  if (!rafId) rafId = requestAnimationFrame(frame);
  startClock();
});

listen("screensaver://hidden", () => {
  /* window already hidden by the backend */
});

function dismiss() {
  if (closing) return;
  closing = true;
  root.classList.add("fading");
  setTimeout(() => {
    root.classList.remove("awake");
    cancelAnimationFrame(rafId);
    rafId = 0;
    if (clockTimer) window.clearInterval(clockTimer);
    invoke("hide_screensaver");
  }, 1400);
}

window.addEventListener("keydown", dismiss);
window.addEventListener("mousedown", dismiss);
window.addEventListener("wheel", dismiss, { passive: true });
window.addEventListener("resize", () => {
  if (rafId) seed();
});

// ===== Init =====
(async function init() {
  try {
    const s = await invoke<Settings>("load_settings");
    clock24 = s.clock_24h ?? true;
  } catch {
    clock24 = true;
  }
  seed();
  rafId = requestAnimationFrame(frame);
  startClock();
})();