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

---

<div align="center">
</div>
