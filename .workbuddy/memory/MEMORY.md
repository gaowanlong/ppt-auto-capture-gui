# 项目长期记忆 — ppt-capture

## 这是什么
`gaowanlong/ppt-auto-capture-gui`：Rust + eframe/egui 桌面工具，自动截取幻灯片变化并生成 PPTX。
截图 → `CaptureWorker`（Windows DXGI/GDI、macOS xcap）→ `DetectionWorker` → `StorageWorker` → 自写 OOXML 打包（`src/pptx/`，不依赖 Office 库）。

## 发布流程（用户明确要求，必须遵守）
- **每次推送 release 必须写好完整 changelog**，不能只靠 GitHub 自动生成的 compare 链接——本项目直接向 main 提交、不走 PR，自动生成几乎是空的。
- 落点：仓库根目录 `CHANGELOG.md`（Keep a Changelog 格式，倒序，每条 `## [x.y.z] - YYYY-MM-DD`，下分 `### Added / Changed / Fixed`）。
- 发布前：`Cargo.toml` 版本号 + `CHANGELOG.md` 对应条目**同时**写。
- 流程已强制：`scripts/release-notes.sh <tag>` 抽取对应版本小节；CI preflight 里跑它，缺条目直接失败；`create-release` 用 `body_path: release-notes.md` 发布（已移除 `generate_release_notes`）。契约测试 `tests/test_release_notes.sh` 与 `tests/test_release_workflow.sh` 守着这条规则。
- 打 tag 后：`git push origin main` + `git tag -a vX.Y.Z` + push tag → `gh run watch` 等 Build and Release → `gh release view` 核对三个产物（exe / linux tar.gz / macos dmg）。

## 遗留约定
- CI 用的是 **Linux** clippy 基线（`.clippy-warning-baseline`），macOS 本地看不到 Windows/Linux 专属分支的未使用变量；新加的 `#[cfg]` 代码要在所有分支都用上参数。
- Windows 专属代码本地无法编译：用 `cargo check --target x86_64-pc-windows-msvc`，并可用"故意写错方法名"确认该 cfg 分支真的参与编译。
- 本机测试 UI：辅助功能权限未授权（不能点击），只能用 `screencapture -l <windowid>` 按窗口截图；启动进程必须在同一个 bash 调用里截完（命令结束即被杀）。
