# MemeDock

轻量级跨平台（Windows + macOS）QQ 表情包管理工具，基于 **Tauri v2 + Rust + React + SQLite FTS5** 构建，突破 QQ 自带表情收藏数量限制，实现极低资源占用、秒搜与一键动图粘贴。

## 核心特性

- **极低资源占用**：原生系统 WebView + Rust 后端，托盘常驻隐藏时近零 CPU；原图按 SHA-256 自动去重，网格仅渲染轻量静态 WebP 缩略图，避免大量 GIF 同时解码。
- **别名、多标签与拼音秒搜**：本地 SQLite + FTS5 全文索引，支持中文别名、全拼、拼音首字母（如输入 `xmt` 搜索「熊猫头」）及多标签组合筛选。
- **点击自动粘贴到 QQ 输入框**：
  - 默认**仅粘贴到当前输入框**（私聊/群聊通用），设置中可开启「粘贴后自动回车发送」；
  - 按住 `Shift` 单击支持**连续粘贴不收起面板**；
  - 动图（GIF / 动图 WebP）以文件引用格式写入剪贴板（Windows `CF_HDROP` / macOS `NSURL` + `NSFilenamesPboardType`），确保粘贴到 QQ NT 完整保留动画。
- **悬浮动图预览与流畅翻页**：
  - 默认 `5×4 = 20` 个/页，支持点击、鼠标滚轮与键盘方向键翻页；
  - 鼠标悬浮约 200ms 在主窗体外侧弹出独立高清浮层，完整播放原图/GIF 动画并显示别名、标签、尺寸与文件大小。
- **灵活召出方式**：
  - **全局快捷键为主**：默认 `Alt+E`（macOS 为 `Option+E`，可自定义），召出即自动聚焦搜索框，用完失焦自动隐藏；
  - **可选 QQ 窗口边缘贴靠按钮**：在设置中开启后，仅当 QQ 位于前台时低频追踪窗口右侧并显示快捷按钮。
- **多渠道快速导入**：
  - **QQ NT 表情包一键扫描**：自动发现 Win/Mac QQ NT 表情目录（支持手动选目录），按文件头魔数识别 PNG/JPG/GIF/WebP（兼容无扩展名缓存文件）、自动去重并预览勾选批量入库；
  - **自定义添加**：多文件选择、整目录导入、直接拖拽图片/目录入窗、**剪贴板直接导入**（支持全局快捷键 `Alt+Shift+E` 一键快收）。

## 快速安装

### macOS（智能安装脚本）

```bash
chmod +x scripts/install.sh
./scripts/install.sh
```

脚本会自动检测本机架构（Apple Silicon / Intel），优先拉取最新 GitHub Release `.dmg` 并安装到 `/Applications/MemeDock.app`；如无 Release 则自动检测/补全依赖并从源码构建安装。

> **macOS 权限提示**：首次使用点击自动粘贴功能时，请在「系统设置 -> 隐私与安全性 -> 辅助功能」中勾选 `MemeDock`（设置面板内置了一键跳转按钮）。

### Windows

从 [GitHub Releases](https://github.com/flakedZZ/meme-dock/releases) 下载 `*-setup.exe`。安装到 `Program Files` 等受保护目录时，选择「为所有用户安装」并允许管理员授权。仅为自己安装时，使用向导提供的用户目录。

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

脚本会自动尝试拉取最新 GitHub Release 的 `.exe` (NSIS) / `.msi` 安装包并启动安装；若无 Release 则自动检查环境并本地编译安装。

## 本地开发与构建

前置环境：Node.js 20+、`pnpm`、Rust 稳定版工具链。

```bash
# 安装前端依赖
pnpm install

# 启动开发模式
pnpm tauri dev

# 运行 Rust 单元测试
cd src-tauri && cargo test

# 构建当前平台安装包
pnpm tauri build
```

## GitHub Actions 自动构建（Windows + macOS）

仓库已内置 `.github/workflows/build.yml`：
- 推送到 `main` 或提交 PR 时自动运行前端构建、Rust 单元测试，并分别在 `windows-latest`（产出 `.exe` / `.msi`）和 `macos-latest`（产出 Apple Silicon `arm64` `.dmg`）编译安装包并上传 Artifact；
- 推送 `v*` 标签（例如 `git tag v0.1.0 && git push origin v0.1.0`）时自动创建 GitHub Release 并附带全平台安装包。

## 默认快捷键

| 操作 | Windows | macOS |
| --- | --- | --- |
| 召出 / 隐藏主面板 | `Alt + E` | `Option + E` |
| 剪贴板一键快收 | `Alt + Shift + E` | `Option + Shift + E` |
| 连续粘贴不收起面板 | `Shift + 左键点击` | `Shift + 左键点击` |
| 编辑别名 / 标签 / 删除 | 右键点击表情 | 右键点击表情 |
| 翻页 | 滚轮 / `←` `→` | 滚轮 / `←` `→` |
| 隐藏面板 | `Esc` | `Esc` |
