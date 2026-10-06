# 即栈 NSIS 品牌资源

`sidebar.bmp` / `header.bmp` 由 `scripts/build-installer-branding.py` 生成，配套 PNG 在 docs 中供预览。

`SimpChinese.nsh` 基于本项目已安装的 Tauri CLI 2.11.4 内置简体中文消息，只将未展开的 `{{product_name}}` 改为 NSIS `${PRODUCTNAME}`。保持全部既有消息键名，以兼容官方默认安装器。Tauri 上游采用 MIT / Apache-2.0 双许可证，两份全文随此目录保留。后续升级 CLI 时需比对新的消息键。
