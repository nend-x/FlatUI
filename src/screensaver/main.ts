/* =========================================================================
   Hush_UI — Screensaver (canvas drift field)
   Fullscreen OLED surface. A slow constellation of particles drifts across
   the screen, linking up when they pass close; two large soft orbs wander
   behind everything. The clock sits centered on top, using the time format
   from settings. Any input fades it all out smoothly.
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

// ===== Drift field =====
// Small monochrome particles with individual sinusoidal drift; when two get
// close, a faint link line fades in. Two big soft orbs wander behind. All
// speeds are tiny — the field is meant to be noticed slowly, not watched.
interface P {
  x: number; y: number;
  vx: number; vy: number;
  r: number;
  phase: number; freq: number; amp: number;
}

let W = 0;
let H = 0;
let particles: P[] = [];
let orbs: { x: number; y: number; dx: number; dy: number; r: number; a: number }[] = [];

function seed() {
  W = canvas.width = window.innerWidth * devicePixelRatio;
  H = canvas.height = window.innerHeight * devicePixelRatio;

  const count = Math.min(90, Math.max(40, Math.round((W * H) / 38000)));
  particles = Array.from({ length: count }, () => ({
    x: Math.random() * W,
    y: Math.random() * H,
    vx: (Math.random() - 0.5) * 0.14,
    vy: (Math.random() - 0.5) * 0.14,
    r: 0.9 + Math.random() * 1.7,
    phase: Math.random() * Math.PI * 2,
    freq: 0.00012 + Math.random() * 0.0001,
    amp: 12 + Math.random() * 30,
  }));

  orbs = [
    { x: W * 0.22, y: H * 0.3, dx: 0.05, dy: -0.03, r: Math.min(W, H) * 0.38, a: 0.05 },
    { x: W * 0.78, y: H * 0.72, dx: -0.04, dy: 0.045, r: Math.min(W, H) * 0.3, a: 0.04 },
  ];
}

const dprScale = () => devicePixelRatio;

function frame(t: number) {
  const dpr = dprScale();
  ctx.clearRect(0, 0, W, H);

  // orbs — big, soft, slow
  for (const o of orbs) {
    o.x += o.dx * dpr; o.y += o.dy * dpr;
    if (o.x < -o.r || o.x > W + o.r) o.dx *= -1;
    if (o.y < -o.r || o.y > H + o.r) o.dy *= -1;
    const g = ctx.createRadialGradient(o.x, o.y, 0, o.x, o.y, o.r);
    g.addColorStop(0, `rgba(200, 200, 205, ${o.a})`);
    g.addColorStop(1, "rgba(200, 200, 205, 0)");
    ctx.fillStyle = g;
    ctx.fillRect(o.x - o.r, o.y - o.r, o.r * 2, o.r * 2);
  }

  // particles
  const link = 150 * dpr;
  for (const p of particles) {
    p.x += p.vx * dpr;
    p.y += p.vy * dpr;
    const wob = Math.sin(t * p.freq + p.phase) * p.a * dpr;
    if (p.x < 0) p.x = W; else if (p.x > W) p.x = 0;
    if (p.y < 0) p.y = H; else if (p.y > H) p.y = 0;

    const px = p.x + wob;
    const py = p.y + Math.cos(t * p.freq + p.phase) * p.a * dpr;
    p._px = px; p._py = py;

    ctx.beginPath();
    ctx.arc(px, py, p.r * dpr, 0, Math.PI * 2);
    ctx.fillStyle = "rgba(220, 220, 225, 0.55)";
    ctx.fill();
  }

  // links
  ctx.lineWidth = 0.6 * dpr;
  for (let i = 0; i < particles.length; i++) {
    const a = particles[i] as unknown as { _px: number; _py: number };
    for (let j = i + 1; j < particles.length; j++) {
      const b = particles[j] as unknown as { _px: number; _py: number };
      const dx = a._px - b._px;
      const dy = a._py - b._py;
      const d2 = dx * dx + dy * dy;
      if (d2 < link * link) {
        const alpha = 0.14 * (1 - Math.sqrt(d2) / link);
        ctx.beginPath();
        ctx.moveTo(a._px, a._py);
        ctx.lineTo(b._px, b._py);
        ctx.strokeStyle = `rgba(210, 210, 215, ${alpha})`;
        ctx.stroke();
      }
    }
  }

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