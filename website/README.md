# 官网素材与构建

`index.html` 是官网源模板，版本、安装包大小与更新日志由发布脚本填充。`img/shots/` 保留 v1.0.0 界面参考图；`img/release-<version>.png` 是当前版本的宣传卡片与社交分享图，不是应用截图。

## 构建

正式双架构安装包生成后，执行：

```powershell
npm run build:website
```

输出目录为 `release/dist/v<version>/oss/`，包含首页、图片、x64/ARM64 安装包、官网更新清单和更新日志。这个命令不编译原生安装包、不改写 GitHub 发布目录中的安装包或更新清单，也不上传文件。版本来自 Tauri 配置，更新公告来自 `release/changelog.json`；优先读取 `release/dist/v<version>/github/` 中保留的正式安装包和 `.sig` 签名，因此清理原生打包缓存后仍可构建。缺少正式签名文件时，才读取原生双架构打包目录。

`npm run build` 构建应用前端到 `build/`，不部署官网，也不生成 Windows 安装包。

## 更新宣传素材

在 Windows 上，使用已安装 Pillow 的 Python 执行：

```powershell
python scripts/build-website-materials.py
npm run build:website
```

生成器读取 `package.json` 版本，使用系统微软雅黑字体，输出 1200 × 630 PNG。更新版本时应同步调整生成器中的亮点说明，使素材与真实更新一致。

## 发布状态

2026-10-11：官网源文案与素材已同步 v1.0.1，本地官网和应用前端构建通过。GitHub 的 v1.0.1 安装包已发布；本次官网构建未部署到 OSS。
