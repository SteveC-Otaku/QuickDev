# Quick Dev · 即栈 — 技术方案

---

## 技术栈

| 层级 | 选型 |
|------|------|
| 桌面框架 | Tauri 2 |
| 前端 | Svelte 5 或 React + Tailwind |
| 本地存储 | SQLite（`tauri-plugin-sql`）或 JSON |
| 打包 | Tauri bundler → MSI |

---

## 架构

```
┌─────────────────────────────────────────┐
│  UI：端口列表 / 项目卡片 / 日志视图      │
└─────────────────┬───────────────────────┘
                  │ Tauri IPC
┌─────────────────▼───────────────────────┐
│  Rust Core                              │
│  ├── port/      端口扫描                │
│  ├── process/   Kill 进程树             │
│  ├── dev/       启停 + 日志管道         │
│  └── config/    SQLite 配置             │
└─────────────────┬───────────────────────┘
                  │
         Windows TCP / Process API
```

---

## Rust 依赖（建议）

| 用途 | crate / API |
|------|-------------|
| 端口枚举 | `netstat2` 或 PowerShell `Get-NetTCPConnection` |
| 进程信息 | `sysinfo` |
| 命令行 | WMI `Win32_Process.CommandLine` |
| 杀进程树 | Windows Job Object / `taskkill /T /F` |
| 子进程 + 管道 | `tokio::process::Command` |
| 数据库 | `rusqlite` |

---

## 核心流程

### 端口扫描

```
1. 枚举 State=Listen 的 TCP
2. OwningProcess → PID
3. 查进程名、exe 路径、命令行
4. 若命令行含已知项目路径 → 关联项目卡片
5. 渲染列表，dev 端口高亮
```

### Dev 启动

```
1. 读项目 cwd + command
2. 检测 port 是否占用 → 冲突则提示占用者
3. spawn: powershell -NoProfile -Command "cd 'path'; pnpm dev"
4. 异步读 stdout/stderr → ring buffer → Tauri event 推 UI
5. 记录 PID + Job Object handle
6. 状态 → running
```

### Dev 停止

```
1. TerminateJobObject（或 taskkill /T /F）
2. 确认端口释放
3. 状态 → stopped
```

---

## 目录结构（参考）

```
quick-dev/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── port/mod.rs
│   │   ├── process/kill.rs
│   │   ├── dev/runner.rs
│   │   ├── dev/job.rs
│   │   └── config/store.rs
│   └── tauri.conf.json
├── src/
│   ├── routes/
│   └── lib/
└── package.json
```

---

## 本地路径

| 用途 | 路径 |
|------|------|
| 配置与数据库 | `%LOCALAPPDATA%\QuickDev\` |
| 日志 | `%LOCALAPPDATA%\QuickDev\logs\` |

---

## License 校验（Pro）

- 用户输入激活码 → 可选联网验证一次 → 本地 `license.json`
- 免费版限制 3 个项目；Pro 解锁无限 + 分组 + 导入导出
- 无用户账号服务器

---

## 开发命令（规划）

```bash
# 初始化（待执行）
npm create tauri-app@latest quick-dev -- --template svelte-ts
cd quick-dev
npm install
npm run tauri dev
```
