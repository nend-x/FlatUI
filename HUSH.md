# Hush — concept note

> A quiet, warm-dark reimagining of FlatUI. Calm by default, quick to respond, single warm-peach accent.

## What changed

### Design philosophy

FlatUI's original Sand+Espresso palette is beautiful but heavy — espresso-brown frosted glass everywhere, sepia-washed icons, bouncy spring animations on every interaction. After extended use it starts to feel like sitting in a coffee shop at 11pm: warm, but a bit much.

**Hush** inverts that energy. The shell should disappear when you're not using it and surface only what you actually need, in the calmest possible voice.

Four pillars:

1. **Calm by default.** Soft warm-dark ink (`#0E1014`) instead of espresso. No glow pulses, no aggressive borders, no bright signal colors. The single warm-peach accent (`#E8A87C`) appears only on the foreground window indicator and on focused inputs — nothing else.
2. **Quick to respond.** All hover/transition timings are 100–220ms with `ease-out-quart`. The previous `ease-spring` overshoot is gone — spring bounces make a shell feel like a toy, not a tool. The launcher expand is 1000ms (was 1280ms) and the collapse is 600ms (was 800ms).
3. **Single accent.** Warm peach is the only accent color in the entire shell. Status indicators are flat snow-toned bars, not colored dots. Active window = accent bar. Pinned app = faint smoke dot. No more terracotta/caramel/sand tri-color system.
4. **Inter for UI, JetBrains Mono only for clock/version.** The original used JetBrains Mono everywhere — readable but heavy. Inter Variable is a proper UI sans-serif with optical sizing and ligature control. The clock, version badge, and run dialog stay monospace because that's where monospace belongs.

### Visual changes

| | Before | Hush |
|---|---|---|
| Background | espresso `rgba(58,42,26,0.55)` | ink `rgba(20,23,28,0.62)` |
| Foreground | sand `#C2B280` | snow `#F2F4F8` |
| Accent | terracotta `#B8835A` + caramel `#D4A574` | single peach `#E8A87C` |
| Font (UI) | JetBrains Mono | Inter Variable |
| Taskbar height | 40px | 38px |
| Backdrop blur | 20px / 160% sat | 14px / 120% sat |
| Expand animation | 1280ms `ease-out-expo` | 1000ms `ease-out-quart` |
| Icon hover | `scale(1.06)` spring | `translateY(-1px)` soft |
| Active indicator | 18px bar + glow | 18px flat bar, no glow |
| Action button hover | `scale(1.08)` | `translateY(-1px)` |
| Icon filter | `sepia(0.15) hue-rotate(-6deg)` | `saturate(0.92) brightness(0.98)` (truer colors) |

### Functional changes

- **Taskbar height: 40 → 38.** Three pixels less heavy. The AppBar registration, window size, and y-position math all updated to match.
- **No MSI/NSIS installers.** `bundle.active = false`, `bundle.targets = []`. The release is a single portable `.exe`, period.
- **No WebView2 runtime bundling.** The exe uses whatever WebView2 runtime is already installed on the system (Windows 11 has it built in, Windows 10 gets it via Edge updates). The `WebView2Loader` library is still statically linked into the exe — verified via `llvm-objdump`: zero `WebView2Loader.dll` in the import table.
- **Locale fix.** The taskbar clock was hard-coded to `ru-RU`. Now `en-US`.
- **Dead code removed.** Four `settings-*` element references in `launcher/main.ts` pointed at HTML elements that didn't exist. Removed.
- **Search placeholder.** Was `Flatlight` (cryptic). Now `Search apps, run commands…` (clear).

### What stayed the same

- All Tauri commands (~50 of them) — no backend behavioral changes.
- All persisted state (blacklist, clipboard history, notes) — same paths, same format.
- The expand-overlay cube grow animation — just calmer timing.
- All four action buttons (window switcher, minimize all, run, blacklist).
- The clipboard / notes / sysmon / audio widgets — same positions, same logic.
- The single-exe deployment model.
- The embedded `flatwin.exe` (AHK) and `HideTaskbar.exe` helpers.

## Build

Cross-compile from Linux to Windows x64:

```bash
# one-time toolchain setup
rustup target add x86_64-pc-windows-msvc
# cargo-xwin (binary tarball) OR standalone xwin binary
# llvm-mingw tarball (for clang-cl / lld-link / llvm-objdump)

# build (see /home/z/my-project/scripts/build-hush.sh)
./scripts/build-hush.sh
```

The exe lands at `src-tauri/target/x86_64-pc-windows-msvc/release/flatui.exe`. Expected: ~6.4 MB, GUI subsystem, x86-64, no `WebView2Loader.dll` in imports.

## Verification

```
$ llvm-objdump -p flatui.exe | grep "DLL Name"
    DLL Name: bcryptprimitives.dll
    DLL Name: ntdll.dll
    DLL Name: kernel32.dll
    DLL Name: user32.dll
    DLL Name: gdi32.dll
    DLL Name: combase.dll
    DLL Name: ole32.dll
    DLL Name: comctl32.dll
    DLL Name: shell32.dll
    DLL Name: oleaut32.dll
    DLL Name: shlwapi.dll
    DLL Name: api-ms-win-core-synch-l1-2-0.dll
    DLL Name: dwmapi.dll
    DLL Name: ADVAPI32.dll
    DLL Name: api-ms-win-core-winrt-error-l1-1-0.dll
    DLL Name: ws2_32.dll
    DLL Name: VCRUNTIME140.dll
    DLL Name: VCRUNTIME140_1.dll
    DLL Name: api-ms-win-crt-runtime-l1-1-0.dll
    DLL Name: api-ms-win-crt-stdio-l1-1-0.dll
    DLL Name: api-ms-win-crt-math-l1-1-0.dll
    DLL Name: api-ms-win-crt-heap-l1-1-0.dll
    DLL Name: api-ms-win-crt-locale-l1-1-0.dll
    DLL Name: api-ms-win-crt-convert-l1-1-0.dll
    DLL Name: api-ms-win-crt-string-l1-1-0.dll
```

25 imports, all system DLLs. No `WebView2Loader.dll`. Single portable exe.
