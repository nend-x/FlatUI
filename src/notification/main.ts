/* =========================================================================
   Hush_UI — Notification framework (frontend)
   Listens for notify://show / notify://hide and plays the slide-in/out.
   Reports back with notification_close_finished when the slide-out is done
   so the backend can hide the window.
   ========================================================================= */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const toast = document.getElementById("toast")!;
const titleEl = document.getElementById("toast-title")!;
const bodyEl = document.getElementById("toast-body")!;

listen<{ title: string; body: string }>("notify://show", (e) => {
  titleEl.textContent = e.payload.title;
  bodyEl.textContent = e.payload.body;
  // restart the slide-in animation
  toast.classList.remove("in", "out");
  void toast.offsetWidth;
  toast.classList.add("in");
});

listen("notify://hide", () => {
  if (!toast.classList.contains("in") || toast.classList.contains("out")) return;
  toast.classList.add("out");
  // report back once the slide-out finished so the backend hides the window
  setTimeout(() => {
    toast.classList.remove("in", "out");
    invoke("notification_close_finished");
  }, 380);
});