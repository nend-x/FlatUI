<div align="center">
<img src="assets/banner.png" alt="Hush_UI" width="100%"/>
<p><em>Windows, but quieter.</em></p>
<p><strong>Hush_UI</strong> is a custom shell for Windows 10/11 — one portable exe, no installer, no helper processes.<br/>The Win key opens a launcher instead of the Start menu. Hold it, and a radial table picker appears under your cursor.<br/>A systemless brightness dimmer overlays the display — nothing is modified, nothing persists after exit.</p>
<p>Run as administrator. Build with Tauri + Rust + TypeScript. See <a href="BUILD.md">BUILD.md</a>.</p>
</div>

---

## Design

Hush_UI is not a reskin of Windows — it is a bet that a desktop can be
almost silent. The design follows from that:

**Quiet by default.** Nothing animates, blinks, or demands attention.
The native taskbar is hidden; there is no replacement pinned to an edge.
The launcher only exists while you are using it. When you are not,
the screen is simply your wallpaper.

**One key.** The entire interface hangs off the Win key. A tap is the
launcher — search apps, run commands, screenshot a region. A hold is
the *tables*: a radial picker that appears at the cursor and folds away
the moment you release. Every surface (taskbar strip, settings, widgets,
Hushlight, desktop icons) is a table — the same shape, the same gesture,
learned once.

**Flat, neutral, opaque.** The `material3-dark` theme is a pure gray
ramp — `#131313` deep, `#1C1C1C` surface, `#C6C6C6` text. No hue tint,
no blur, no frost, no shadows-as-decoration. Depth comes from three
tones of gray and one pixel of outline, nothing else. Iconography is
recolored to the same desaturated ramp so nothing screams in color.

**Grain, not gloss.** The one concession to texture is a faint fractal
noise — the same `feTurbulence` grain tiled across surfaces — which
keeps large flat areas from banding. It is atmosphere at 0.25 alpha,
not a texture pack.

**Systemless.** The brightness dimmer is a click-through, topmost
overlay whose alpha *is* the dim level. No WMI, no DDC, no registry,
no power plan. Exit the app — or slide back to 0% — and the display
is exactly as it was. The same philosophy applies everywhere: Hush_UI
changes what you see, not what Windows is.

**In-process.** Every window — launcher, tables, dimmer, screensaver —
runs inside the single exe. No background services, no children to
clean up, nothing left behind.

---

<div align="center">
<sub>releases are renumbered chronologically — <code>v0.2.3-pre.4</code> is the latest</sub>
</div>
