import { invoke } from "@tauri-apps/api/core";

// Consent window — asks the user for permission before requesting UAC elevation.
//
// When the user clicks "Proceed", we tell the backend to launch the elevated
// relaunch (ShellExecuteW "runas"). The backend then exits this non-elevated
// process; the elevated process starts fresh with the setup window.
//
// When the user clicks "No thanks", we tell the backend to continue without
// elevation. The backend closes this consent window and proceeds to the
// setup window.

document.getElementById("consent-yes")!.addEventListener("click", () => {
  invoke("consent_proceed");
});

document.getElementById("consent-no")!.addEventListener("click", () => {
  invoke("consent_decline");
});
