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
