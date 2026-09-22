<div align="center">
<img src="assets/banner.png" alt="Hush_UI" width="100%"/>
<p>Windows, but quieter. A custom shell replacement in one portable exe.</p>
</div>

---

Hush_UI replaces the Windows shell experience. The Win key opens the launcher instead of the Start menu. There is **no custom taskbar** — the native one is simply hidden while Hush_UI runs — and a systemless **brightness dimmer** overlays the display with a software dim. Everything runs in-process. No helpers, no child processes, no installer.

**Requires:** Windows 10/11, run as administrator.

## Keys

- **Win (tap)** — toggle the launcher. Search installed programs, run commands, take region screenshots to clipboard.
- **Win (hold 80ms default, adjustable 40-600ms)** — the pie table picker appears at the cursor. Hover a slice, release Win, that table opens:
  - **Taskbar** — your taskbar icons on a vertical line strip (wheel-scrolls, each app has a permanent name bar)
  - **Settings** — the settings panel as a movable window
  - **Widgets** — clipboard, notes, audio, system and **brightness** widgets in one window
  - **Hushlight** — the launcher itself
  - **Desktop** — desktop icons in a movable window
- **Ctrl + Win (hold)** — same, centered on screen.
- **Win + anything else** — real combos (Win+D, Win+E...) pass through untouched.
- The start menu can't open: the Win-down is swallowed before the shell ever sees it, and a background monitor kills `StartMenuExperienceHost.exe` on sight.

## Brightness dimmer (systemless)

A pure software dim: a black, topmost, fully click-through overlay window whose alpha is the dim strength. Nothing on the system is modified — no WMI/DDC brightness, no registry, no power plan. Slide the **Brightness** widget (pie → Widgets) or the **Brightness dim** slider (Settings → System) and the change is instant; exit Hush_UI (or slide back to 0%) and the display is exactly as before. The dim level persists across restarts. The overlay never goes fully opaque, so the widget stays reachable.

**Note:** on launch Hush_UI requests administrator rights via UAC. If you decline, the app still runs, but you'll get a warning that the dimming might not work on system apps (elevated windows sit above a non-elevated overlay).

## Theme

One theme: **Material 3 dark** — neutral gray surfaces, no blur, no grain, no hue tint. Fully opaque panels, soft elevation shadows. Icons can be recolored to match (Settings → Icon Recoloring). Stored in `%LOCALAPPDATA%\Hush_UI\themes.json`.

## Build

See [BUILD.md](BUILD.md). Short version: Node 18+, Rust stable with the `x86_64-pc-windows-msvc` target, VS Build Tools with the C++ workload, then:

```powershell
.\build.ps1
# -> src-tauri\target\x86_64-pc-windows-msvc\release\flatuihush.exe
```

## Settings

Everything lives in the settings table (Win hold → pie → Settings): hold duration, magnification, clock format, icon recoloring, brightness dim, widget visibility, theme. Persists across restarts.

## Exit

The exit button in settings un-hides the native taskbar, removes the dim overlay, restarts explorer and exits clean.

## License

[MIT](LICENSE)

## Reset config

Settings → Maintenance → **Reset** deletes every config file under `%LOCALAPPDATA%\Hush_UI` and restarts the app with factory defaults (Win hold 80ms, icon recoloring off, 24h clock).
