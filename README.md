<div align="center">

# 即栈 · Quick Dev

**Windows 本地托盘工具**：看清端口占用，一键启停多个项目的 dev 服务。

[English](README.en.md) · **中文**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4.svg)](#)
[![Tauri](https://img.shields.io/badge/Tauri-2-FFC131.svg)](https://tauri.app/)
[![Release](https://img.shields.io/github/v/release/SteveC-Otaku/QuickDev)](https://github.com/SteveC-Otaku/QuickDev/releases/latest)

</div>

---

完全免费开源，**不限项目数量**。不收集账号、不联网同步——数据只在你本机。

## 下载

到 [Releases](https://github.com/SteveC-Otaku/QuickDev/releases/latest) 下载最新安装包（`QuickDev_x.x.x_x64-setup.exe`），双击安装即可。

> Windows 若提示「未知发布者」，属未代码签名，选择仍要运行即可。

## 能干什么

- **端口面板**：本机谁在 Listen、进程名 / PID / 命令行，一键 Kill 进程树
- **项目管理**：保存路径、启动命令、端口、分组，一键启动 / 停止
- **实时日志**：看 Vite / Next 输出，端口冲突有提示
- **托盘常驻**：关窗口不退出；支持全局热键、开机自启（仅托盘）
- **配置备份**：导入 / 导出 JSON

## 截图

![即栈端口面板](demo/ports-panel.png)

---

## 技术栈

- [Tauri 2](https://tauri.app/) + Rust
- SvelteKit + TypeScript + Tailwind CSS

## 开发

**环境：** Node.js 18+、Rust（`rustup`）、Windows 建议安装 [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（含「使用 C++ 的桌面开发」）。

```bash
npm install
npm run tauri dev
```

## 打包

```bash
npm run tauri build
```

安装包输出：

```text
src-tauri/target/release/bundle/nsis/
```

未配置代码签名时会自动跳过（见 `scripts/sign-windows.ps1`）。

## 配置目录

```text
%LOCALAPPDATA%\QuickDev\
  projects.json
  settings.json
  logs\
```

## 文档

- [产品需求 PRD](PRD.md)
- [功能说明](FEATURES.md)
- [技术方案](TECH.md)

## License

[MIT](LICENSE)
