<div align="center">

<img src="assets/banner.png" alt="FlatUI" width="100%"/>



<p>Windows, but quieter. A custom shell replacement — 40px taskbar, Spotlight-style launcher, three themes, one exe.</p>

![Windows 10/11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows11&logoColor=white)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)
![MIT](https://img.shields.io/badge/license-MIT-B8835A.svg)

</div>

---

FlatUI replaces the Windows shell. The native taskbar hides, a 40px custom taskbar takes its place, and the Win key opens a fullscreen launcher instead of the Start menu. One exe, no install, three themes.

## Themes

Three pastel themes sharing the same tonal structure — only the hue changes.

| Sand Cream | Earthly Green | Silver Lining |
|---|---|---|
| Warm espresso | Dark forest | Cool steel |
| `#3A2A1A` bg | `#0F2D19` bg | `#23282E` bg |
| `#B8835A` accent | `#39AC5F` accent | `#74899E` accent |

Switch in Settings. Everything recolors instantly.


## Features

- **Launcher** — fullscreen grain-textured overlay, search installed programs
- **Taskbar** — 40px, centered icons, running indicators, peek previews
- **Apps widget** — running windows, click to focus, auto-sizes
- **Settings** — toggle widgets, switch themes, icon recoloring
- **Screenshots** — region select to clipboard
- **Run dialog** — type a command, optionally as admin
- **Exit button** — reverts everything, restarts explorer, exits
- **Crash handler** — separate window with stack trace
- **Icon recoloring** — optional CSS filter tint to match theme
- **Grain texture** — fractal noise + dot grid, no GPU-heavy blur


To exit: Win → click the exit button (top-left).

## Config

`%LOCALAPPDATA%\FlatUI\` — themes, settings, widget positions, blacklist.

## License

[MIT](LICENSE)
