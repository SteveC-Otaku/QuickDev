<div align="center">

# Quick Dev · 即栈

**A Windows tray app** to inspect listening ports and start/stop multiple project `dev` servers in one place.

**English** · [中文](README.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64%20%2F%20ARM64-0078D4.svg)](#)
[![Tauri](https://img.shields.io/badge/Tauri-2-FFC131.svg)](https://tauri.app/)
[![Release](https://img.shields.io/github/v/release/SteveC-Otaku/QuickDev)](https://github.com/SteveC-Otaku/QuickDev/releases/latest)

</div>

---

Current version **v1.0.0**. Free and open source — **no project limit**. No accounts, no cloud sync. Everything stays on your machine; the app only goes online to check for updates.

Site: [quickdev.steve-lab.cc](https://quickdev.steve-lab.cc/)

## Download

Get **v1.0.0** from the [website](https://quickdev.steve-lab.cc/#download) or [GitHub Releases](https://github.com/SteveC-Otaku/QuickDev/releases/latest):

| Installer | For |
| --- | --- |
| [`QuickDev_1.0.0_x64-setup.exe`](https://github.com/SteveC-Otaku/QuickDev/releases/download/v1.0.0/QuickDev_1.0.0_x64-setup.exe) | Most PCs (~3.3 MB) |
| [`QuickDev_1.0.0_arm64-setup.exe`](https://github.com/SteveC-Otaku/QuickDev/releases/download/v1.0.0/QuickDev_1.0.0_arm64-setup.exe) | ARM devices — Snapdragon, Surface Pro X (~3.0 MB) |

Not sure which one? Settings → System → About. If **System type** says ARM, use ARM64.

After install, open **Settings → About & updates**: the app checks for updates on launch, or you can check manually, upgrade in one click, and read the changelog.

> Windows may warn about an unknown publisher (unsigned build). Choose “Run anyway”.

## Features

- **Auto-detect** — pick a project folder; reads `package.json` / lockfiles / Tauri, classifies frontend / backend / desktop app, and generates Website / Desktop App / All launch plans
- **Plan start/stop** — start a whole plan, fill in missing services, stop everything; open the web URL or focus the app window when ready
- **Ports panel** — see what’s listening (process / PID / command line) and kill process trees
- **Conflict jump** — if `:3000` is taken, jump to the occupant, expand the process chain, or kill-and-start
- **Live logs** — Vite / Next output in the panel instead of a pile of console windows
- **System tray** — stays running after you close the window; native per-project menu; icon color is run state; global hotkey and optional tray-only auto-start
- **Settings & backup** — refresh interval, default tab, shell presets, groups; import / export JSON

## Screenshots

![Quick Dev ports panel](demo/ports-panel.png)

![Quick Dev multi-service project](demo/projects-panel.png)

![Quick Dev auto-detect](demo/auto-detect.png)

![Quick Dev tray menu](demo/tray-menu.png)

---

## Stack

- [Tauri 2](https://tauri.app/) + Rust
- SvelteKit + TypeScript + Tailwind CSS

## Development

**Requires:** Node.js 18+, Rust (`rustup`), and on Windows preferably [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the C++ desktop workload. ARM64 installers also need the ARM64 MSVC toolchain.

```bash
npm install
npm run tauri dev
```

## Build installer

```bash
# current architecture
npm run tauri build

# x64 + ARM64 installers, website, and latest.json
npm run release
```

Output:

```text
src-tauri/target/<triple>/release/bundle/nsis/
release/dist/oss/          # website + installers for OSS
release/dist/github/       # attach to the GitHub Release
```

Code signing is skipped when no certificate is configured (`scripts/sign-windows.ps1`).

## Config location

```text
%LOCALAPPDATA%\QuickDev\
  projects.json
  settings.json
  logs\
```

## Docs

- [PRD](PRD.md) (Chinese)
- [Features](FEATURES.md) (Chinese)
- [Tech notes](TECH.md) (Chinese)

## License

[MIT](LICENSE)
