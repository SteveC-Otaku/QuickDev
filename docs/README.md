# 项目目录与维护入口

- src/：Svelte 界面、状态与 IPC。
- src-tauri/src/：Rust 项目识别、配置、服务生命周期与托盘。
- scripts/：构建、发布与 JavaScript 回归入口。npm test 统一运行当前回归。
- docs/releases/：与 release/changelog.json 对齐的版本公告。
- docs/maintenance/2026-10-11-fixes/：当前检测回归工程与验收说明。archive/ 为本地历史草稿；evidence/ 为本地构建记录，不作为源码。
- docs/assets/screenshots/：README 使用的正式界面截图。
- docs/history/：按日期归档的历史审查文档、过程截图与 audit 材料，仅本地保留。
- demo/：界面和图标演示，包括 hairline-demo；历史演示保留为本地材料。
- hairline-demo-2026-10-11：兼容其他项目已有链接的本地目录链接，实际文件在 demo/hairline-demo-2026-10-11，不保存重复副本。
- assets/marketing/：宣传插图、导出图与小红书展示页，统一分类，仅本地保留。
- _tasks/2026-10-11-目录整理/：迁移、删除清单与历史构建记录。
- website/：官网源模板；发布脚本生成官网，不直接修改已部署站点。
- release/changelog.json：应用更新日志数据。
- release/dist/v<version>/：忽略的正式逐版本交付（github、oss）；当前保留 v0.2.0、v1.0.0、v1.0.1。正式文件以 GitHub Release 的大小和 SHA-256 为准；临时测试包、诊断程序及重复安装包已清理，构建记录归档到 _tasks。

入口文档仍保留根目录 README、PRD、FEATURES、TECH，避免破坏既有链接。真实配置在 LOCALAPPDATA 下，项目中不保存 projects.json、settings.json、密钥或日志。

## 验证

npm run check 与 npm test 验证界面和业务回归。隔离 Rust 检测、原生回归与 GUI 验收边界见 [维护说明](maintenance/2026-10-11-fixes/README.md)。正式双架构发布需要更新签名、工具链及完整产物，不能把未签名本地包写入更新清单。
