# Quick Dev 修复材料：2026-10-11

本目录集中保存本次 Quick Dev 诊断、三批修复和验收材料。业务代码的唯一有效版本仍在仓库正常的 `src/`、`src-tauri/src/`、`scripts/` 中；没有搬动这些目录。

## 布局与交付

- `detect-tests/`：可运行的隔离 Rust 检测工程，直接引用仓库当前检测源码。
- `tools/vite-tests.rs`、`tools/vite-address-check.rs`：Vite 静态读取测试和只读诊断入口，引用仓库当前源码，不执行配置或启动服务。
- `evidence/`：三批构建证据、原生测试 manifest 和导入检查资料。产物路径已更新，旧位置保留在 `originalPath` 中用于审计。
- `archive/`：修复中间草稿、历史复现脚本、修改引用前的原件。这里不是当前源码，也不作为当前回归入口；历史脚本可能仍断言修复前的缺陷。
- `relocation-inventory.json`、`relocation-moves.json`：逐文件原位置、现位置、原始哈希及移动项。恢复时应按映射逐项核对，目标已有文件时不要覆盖。

原测试交付曾位于 `release/dist/maintenance-2026-10-11/`。2026-10-11 目录清理已删除这些非正式安装包、程序副本及诊断二进制，保留 [最终构建证据](evidence/final-fixes-build-evidence.json)。正式版本统一保留于 `release/dist/v<version>/github/`。

安装包 SHA-256：`5361CD9DA124C43918FA5DC302E7C32F744D626F9541C85E0CDD7B2179ED0C21`。
程序 SHA-256：`5FF9B897B06E110CD1013C6494EE1D922EF7F2E36886DA1884330658BEFBCEEA`。
上述哈希仅用于追溯已清理的历史测试交付，不是当前正式版本的下载校验值。

## 后续版本

本目录的旧交付为修复期间的 1.0.0 测试包。当前源码为 1.0.1，版本公告见 [1.0.1](../../releases/1.0.1.md)，正式交付在 release/dist/v1.0.1/github。旧证据和检测工程保留用于追溯。

## 验证入口

以下命令从 Quick Dev 项目根执行。检测工程的 `target` 属于可重建缓存，不作为源码迁移；新缓存统一生成在现有忽略规则覆盖的 `src-tauri/target/` 中。

```powershell
cargo test --offline --lib --manifest-path docs/maintenance/2026-10-11-fixes/detect-tests/Cargo.toml --target-dir src-tauri/target/maintenance-detect-tests
npm test
powershell -ExecutionPolicy Bypass -File scripts/test-native.ps1
```

只读检测任意项目目录（将最后的参数替换为要检查的路径）：

```powershell
cargo run --offline --manifest-path docs/maintenance/2026-10-11-fixes/detect-tests/Cargo.toml --target-dir src-tauri/target/maintenance-detect-tests -- "C:\path\to\project"
rustc --edition=2021 --test docs/maintenance/2026-10-11-fixes/tools/vite-tests.rs -o src-tauri/target/maintenance-vite-tests.exe
& .\src-tauri\target\maintenance-vite-tests.exe
```

`docs/` 和 `release/` 被检测扫描器排除。安装包、诊断二进制位于已有 Git 忽略的 `release/dist/`；另加了本目录 `archive/` 和测试 `target/` 的局部忽略规则，防止中间源码快照、缓存进入版本控制。

## 界面验收

当前界面没有路径面包屑，下面的箭头表示操作顺序。

入口：**项目 → 添加项目 → 项目目录 → 浏览… / 自动识别**。选择外层项目目录。查看每条命令时展开 **服务参数 · 目录、名称、类型与等待判断**，检查 **执行目录** 和 **网页地址（选填）**。不需要持久保存的检查结束后点 **取消**。

1. 嵌套目录：准备 `app/web + app/api` 或 `app/frontend + app/backend` 临时项目，预期分别识别前端和后端，执行目录指向子服务。
2. 根入口共存：根目录也有启动入口时，根入口及子服务都应被发现，不重复生成同一入口。
3. 重扫保护：在草稿中手改命令、端口、网址并新增方案，重复 **自动识别** 后手工字段和自定义方案应保留。旧等价方案提示 **移除重复方案：…**，只有主动移除并保存才生效。
4. 预算截断：临时项目根有有效入口，再准备超过 128 个目录或 4096 个目录项，预期提示扫描上限。超过 3 层会跳过；当前只有无候选时的错误说明深度限制，不单独弹出深度截断警告。
5. 地址和打开目标：在测试项目保存带 `/admin` 的网址，启动日志不应改写它；并行服务的运行时发现互不覆盖。已有网页+桌面混合方案的有效指定打开目标，多次编辑保存后应保持。
6. 托盘：有运行时发现地址的服务应显示打开入口，主窗口与托盘应打开同一个目标。无地址、未就绪或系统打开失败应显示错误。

没有保留可直接选择的完整 GUI 识别夹具；原生回归会创建临时夹具。第 5、6 项需要使用自己的测试项目，不能把历史诊断 EXE 当作 Quick Dev 安装包。

## 范围与验证限制

前三组运行时问题已修复，未知端口的 500ms 进程存活判断仍保留。原生测试使用编译测试 EXE 的临时副本补入 Common-Controls v6 manifest，未改系统 DLL 或正常应用构建脚本。

未安装应用、未实测桌面/托盘/系统浏览器点击，未启动真实项目；真实 `projects.json`、IELTS、其他任务均未改动。搬迁不改变最终安装包字节，不需要重新构建业务软件。
