<div align="center">

<img src="assets/banner.png" alt="FlatUI — three themes: Sand Cream, Earthly Green, Silver Lining" width="100%"/>

<p>Windows, but quieter. A custom shell replacement — 40px taskbar, Spotlight-style launcher, three themes, one exe.</p>

*Custom taskbar · Spotlight-style launcher · Theme system · Single portable exe · Tauri 2 + Rust*

<br>

![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4?logo=windows11&logoColor=white)
![Built with](https://img.shields.io/badge/built%20with-Tauri%202-24C8DB?logo=tauri&logoColor=white)
![Language](https://img.shields.io/badge/language-Rust-DEA584?logo=rust&logoColor=white)
![Frontend](https://img.shields.io/badge/frontend-TypeScript%20%2B%20Vite-3178C6?logo=typescript&logoColor=white)
![License](https://img.shields.io/badge/license-MIT-B8835A.svg)
![PRs](https://img.shields.io/badge/PRs-welcome-B8835A.svg)

[Features](#-features) · [Themes](#-themes) · [How it works](#-how-it-works) · [Quick start](#-quick-start) · [Build from source](#️-build-from-source) · [Credits](#-credits)

</div>

---

FlatUI replaces the Windows shell. The native taskbar hides, a 40px custom taskbar takes its place, and the Win key opens a fullscreen launcher instead of the Start menu.

Everything ships as **one portable `.exe`** — no external helpers, no child processes, no installers. The taskbar-hider runs in-process, the Win-key hook runs in-process, and the start-menu killer runs in-process. Drop it anywhere, run it as administrator, done.

<br>

## ✨ Features

| | |
|---|---|
| 🎯 **Spotlight-style launcher** | Press **Win** — a fullscreen grain-textured overlay opens over a *clean desktop*. Search installed programs and system shortcuts. |
| 🎨 **Three pastel themes** | **Sand Cream** (warm brown), **Earthly Green** (dark forest), **Silver Lining** (cool gray). Switch instantly — everything recolors in realtime. |
| 🖼️ **Grain texture background** | No more GPU-heavy backdrop blur. The background uses a layered grain noise + dot-grid texture that's cheap to render and looks premium. |
| 🔄 **Icon recoloring** | Optional toggle that recolors all icons (launcher + taskbar) to match the active theme via CSS filters — no assets modified. |
| 📊 **Custom AppBar taskbar** | 40px flat taskbar — pinned & running apps, live icons, window peek previews, auto-hides when an app goes fullscreen. |
| 🧹 **Show-desktop on open** | Every time the launcher appears, all visible windows minimize automatically. Your launcher always sits on a beautiful desktop. |
| 📸 **Lightshot-style screenshots** | Region-select anywhere on screen; a **real image** lands on your clipboard — paste into Discord, Word, anywhere. |
| 🏃 **Run dialog** | Win-key launcher doubles as a run box for quick commands. |
| 🪟 **Apps widget** | Top-left widget shows running windows — click any tile to focus, click chrome to open the full switcher. Auto-sizes to fit. |
| ⚙️ **Settings overlay** | Toggle widgets on/off, switch themes, toggle icon recoloring. Everything saves to config and persists across restarts. |
| 🚪 **Exit button** | One click reverts everything — stops the start-menu killer, shows the native taskbar, restarts explorer, exits cleanly. |
| 🚫 **Start menu killer** | A background monitor kills `StartMenuExperienceHost.exe` on sight — race-free Win-key blocking that doesn't depend on hook timing. |
| 💥 **Crash handler** | If FlatUI ever crashes, a separate crash-reporter window pops up with the error type, stack trace, and version. |
| 📦 **Single-file install** | Everything is in one `.exe` — no external helpers, no child processes, no DLLs. Just run it. |

<br>

## 🎨 Themes

FlatUI ships with three pastel themes, all calibrated to share identical HSL lightness and saturation — switching themes changes only the hue family, not the contrast or character of the app.

| Sand Cream | Earthly Green | Silver Lining |
|---|---|---|
| Warm brown / tan / terracotta | Dark forest green | Cool blue-gray |
| `#4B3621` bg · `#B8835A` accent | `#113B1F` bg · `#39AC5F` accent | `#293037` bg · `#74899E` accent |

Every element recolors when you switch: backgrounds, grain texture, dot/line grids, borders, glows, shadows, slider tracks, toggle switches, text colors, and icon recolor tint. All via CSS variable overrides — no assets modified, no restart needed.

Themes persist to `themes.json` and survive restarts.

<br>
