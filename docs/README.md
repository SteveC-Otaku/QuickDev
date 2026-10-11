# 项目目录与维护入口

- src/：Svelte 界面、状态与 IPC。
- src-tauri/src/：Rust 项目识别、配置、服务生命周期与托盘。
- scripts/：构建、发布与 JavaScript 回归入口。npm test 统一运行当前回归。
- docs/releases/：与 release/changelog.json 对齐的版本公告。
- docs/maintenance/2026-10-11-fixes/：当前检测回归工程与验收说明。archive/ 为本地历史草稿；evidence/ 为本地构建记录，不作为源码。
- docs/audit/ 及 docs 中日期材料：既有设计审查记录，保持原位置。
- demo/、assets/、宣传图/：既有演示、品牌和宣传材料，保持原位置以保护引用；本次不提交无关草稿。
- website/：官网源模板；发布脚本生成官网，不直接修改已部署站点。
- release/changelog.json：应用更新日志数据。
- release/dist/v<version>/：忽略的逐版本交付（oss、github 或 local）；旧批次交付另存于 maintenance-2026-10-11，不覆盖。

入口文档仍保留根目录 README、PRD、FEATURES、TECH，避免破坏既有链接。真实配置在 LOCALAPPDATA 下，项目中不保存 projects.json、settings.json、密钥或日志。

## 验证

npm run check 与 npm test 验证界面和业务回归。隔离 Rust 检测、原生回归与 GUI 验收边界见 [维护说明](maintenance/2026-10-11-fixes/README.md)。正式双架构发布需要更新签名、工具链及完整产物，不能把未签名本地包写入更新清单。
