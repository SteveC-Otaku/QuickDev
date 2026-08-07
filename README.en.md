<div align="center">

# Quick Dev · 即栈

**A Windows tray app** to inspect listening ports and start/stop multiple project `dev` servers in one place.

**English** · [中文](README.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4.svg)](#)
[![Tauri](https://img.shields.io/badge/Tauri-2-FFC131.svg)](https://tauri.app/)

</div>

---

Free and open source — **no project limit**. No accounts, no cloud sync. Everything stays on your machine.

## Features

- **Ports panel** — see what’s listening (process / PID / command line) and kill process trees in one click
- **Projects** — save cwd, start command, port, and group; start / stop with one click
- **Live logs** — follow Vite / Next output; port conflicts are flagged
- **System tray** — keep running after closing the window; global hotkey and optional auto-start (tray only)
- **Backup** — import / export config as JSON

## Screenshots

> Add screenshots under `demo/` when ready.

## Stack

- [Tauri 2](https://tauri.app/) + Rust
- SvelteKit + TypeScript + Tailwind CSS

## Development

**Requires:** Node.js 18+, Rust (`rustup`), and on Windows preferably [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the C++ desktop workload.

```bash
npm install
npm run tauri dev
```

## Build installer

```bash
npm run tauri build
```

NSIS output:

```text
src-tauri/target/release/bundle/nsis/
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
