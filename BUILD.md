# Building FlatUI on Windows

This buildkit contains the full FlatUI source tree (commit
`9f366731e1c26dd46bae7a55360f724d38035401`, branch
`chore/remove-ahk-use-prevent-alt-win-menu`) plus the helper scripts in this
folder. Follow the steps below to produce a single portable
`flatui.exe` (~6.6 MB, GUI subsystem, x64).

## TL;DR

```powershell
# 1. Install prerequisites (one-time, see "Prerequisites" below)
# 2. Run the build
cd FlatUI-buildkit
.\build.ps1
# 3. Find your exe here:
ls src-tauri\target\x86_64-pc-windows-msvc\release\flatui.exe
```

## Prerequisites

You need three things installed. The build script will check for them and
print a friendly error if anything is missing.

### 1. Node.js ≥ 18 (for the frontend Vite build)

- Download from <https://nodejs.org> (the LTS build is fine).
- Verify: `node --version` should print `v18.x` or higher.

### 2. Rust (stable) with the `x86_64-pc-windows-msvc` target

- Install via <https://rustup.rs> — accept the default toolchain (`stable`).
- Add the Windows MSVC target:

  ```powershell
  rustup target add x86_64-pc-windows-msvc
  ```

- Verify: `rustc --version` should print `1.85.0` or higher (the
  `prevent-alt-win-menu` dependency uses `edition = "2024"`, which needs
  Rust ≥ 1.85).

### 3. Visual Studio Build Tools (C++ workload)

- Download "Build Tools for Visual Studio" from
  <https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio>.
- In the installer, select the **Desktop development with C++** workload.
- That pulls in the MSVC compiler + Windows SDK that Rust's
  `x86_64-pc-windows-msvc` target links against.

## Build steps (manual)

If you'd rather run the commands by hand instead of using
`build.ps1`:

```powershell
# from the buildkit root
# 1. Frontend dependencies + build (produces dist\)
npm install
npm run build

# 2. Backend
cd src-tauri
cargo build --release --target x86_64-pc-windows-msvc

# 3. The portable exe is at:
#    src-tauri\target\x86_64-pc-windows-msvc\release\flatui.exe
```

## What the build produces

`src-tauri\target\x86_64-pc-windows-msvc\release\flatui.exe` — a single
~6.6 MB Windows GUI executable. It is self-contained:

- `HideTaskbar.exe` (the small MSVC tool that hides the native Windows
  taskbar) is embedded via `include_bytes!` and self-extracts to
  `%LOCALAPPDATA%\FlatUI\bin` on first run.
- The Win-key hook is **in-process** via the `prevent-alt-win-menu`
  crate — no `flatwin.exe` AutoHotkey helper, no local HTTP server.

To install: just drop `flatui.exe` anywhere and run it. The first-run
splash extracts the helper, hides the native taskbar, and installs the
FlatUI AppBar.

To revert: exit FlatUI, then run
`taskkill /f /im flatui.exe /im HideTaskbar.exe` (and restart
`explorer.exe` if the native taskbar doesn't come back on its own).

## Cross-compiling from Linux

If you'd rather build on Linux (the maintainer's CI setup), follow the
"Cross-compiling from Linux" section in `README.md`. You'll need
`cargo-xwin` and `lld-link`/`llvm-rc` from an LLVM distribution. The
buildkit does **not** ship Linux cross-compile scripts — Windows-native
builds via `build.ps1` are the recommended path for end users.

## Troubleshooting

| Symptom | Fix |
|---|---|
| `error: failed to run custom build command for flatui` | The Tauri build script needs MSVC. Re-run the VS Build Tools installer and ensure "Desktop development with C++" is checked. |
| `error[E0658]: \`edition = "2024"\` is unstable` | Your Rust is too old. `rustup update stable` and you'll get ≥ 1.85. |
| `npm install` fails on `esbuild` postinstall | Run `npm install-scripts approve esbuild` (npm 11+) or just retry — esbuild's postinstall is just a no-op download check. |
| `cargo build` complains about missing `frontendDist` | You forgot to run `npm run build` first. The `dist/` directory must exist before `cargo build`. |
| Build succeeds but the exe is huge (>10 MB) | You built `--debug` or without `--release`. Use `cargo build --release` (the `release` profile has `lto = true`, `codegen-units = 1`, `opt-level = "s"`, `strip = true`). |

## Files in this buildkit

```text
FlatUI-buildkit/
├── BUILD.md                       # this file
├── build.ps1                      # one-shot PowerShell build script
├── prereqs-check.ps1              # checks Node/Rust/MSVC are installed
├── README.md                      # project readme
├── LICENSE                        # MIT
├── package.json                   # npm deps + scripts
├── package-lock.json              # pinned deps
├── tsconfig.json
├── vite.config.ts                 # multi-page Vite build
├── assets/                        # brand banner + icon + fonts
├── scripts/                       # asset generators (Python)
├── src/                           # frontend — vanilla TypeScript
│   ├── launcher/
│   ├── taskbar/
│   ├── setup/
│   ├── shared/
│   ├── styles/
│   └── public/
└── src-tauri/
    ├── Cargo.toml                 # NOTE: prevent-alt-win-menu = "0.2.2"
    ├── Cargo.lock
    ├── build.rs
    ├── tauri.conf.json
    ├── capabilities/
    ├── icons/
    ├── resources/                 # HideTaskbar.exe (embedded) — flatwin.exe removed
    └── src/
        ├── lib.rs                # prevent-alt-win-menu::start() is wired here
        ├── main.rs
        ├── app_state.rs
        ├── crash_handler.rs
        ├── embedded.rs
        ├── persist.rs
        ├── setup.rs
        └── win32/
```

`node_modules/`, `dist/`, and `target/` are NOT included — the build
script will create them on demand.
