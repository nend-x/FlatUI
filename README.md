<div align="center">
<img src="assets/banner.png" alt="FlatUI Hush" width="100%"/>
<p>Windows, but quieter. A custom shell replacement in one portable exe.</p>
</div>

---

FlatUI Hush replaces the Windows shell. The native taskbar is hidden, a 40px custom taskbar takes its place, and the Win key opens the launcher instead of the Start menu. Everything — taskbar hider, keyboard hook, start-menu killer — runs in-process. No helpers, no child processes, no installer.

**Requires:** Windows 10/11, run as administrator.

## Keys

- **Win (tap)** — toggle the launcher. Search installed programs, run commands, take region screenshots to clipboard.
- **Win (hold ~220ms)** — the pie table picker appears at the cursor. Hover a slice, release Win, that table opens:
  - **Taskbar** — your taskbar icons on a vertical line strip (wheel-scrolls, each app has a permanent name bar)
  - **Settings** — the settings panel as a movable window
  - **Widgets** — clipboard, notes, audio and apps widgets in one window
  - **Flatlight** — the launcher itself
  - **Desktop** — desktop icons in a movable window
- **Ctrl + Win (hold)** — same, centered on screen.
- **Win + anything else** — real combos (Win+D, Win+E...) pass through untouched.
- The start menu can't open: the Win-down is swallowed before the shell ever sees it, and a background monitor kills `StartMenuExperienceHost.exe` on sight.

## The taskbar

A flat 40px bar: pinned and running apps with permanent name labels, live window previews, clock, volume, keyboard layout. Auto-hides when an app goes fullscreen and comes back when it exits.

The bottom bar can be turned off entirely (Settings → Shell taskbar) if you only want the taskbar table from the pie.

## Theme

One theme: **Material 3 dark** — neutral gray surfaces, no blur, no grain, no hue tint. Fully opaque panels, soft elevation shadows. Icons can be recolored to match (Settings → Icon Recoloring). Stored in `%LOCALAPPDATA%\FlatUIHush\themes.json`.

## Build

See [BUILD.md](BUILD.md). Short version: Node 18+, Rust stable with the `x86_64-pc-windows-msvc` target, VS Build Tools with the C++ workload, then:

```powershell
.\build.ps1
# -> src-tauri\target\x86_64-pc-windows-msvc\release\flatuihush.exe
```

## Settings

Everything lives in the settings table (Win hold → pie → Settings): hold duration, magnification, clock format, icon recoloring, shell taskbar on/off, widget visibility, theme. Persists across restarts.

## Exit

The exit button in settings reverts everything: native taskbar back, explorer restarted, clean exit.

## License

[MIT](LICENSE)
