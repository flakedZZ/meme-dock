#!/usr/bin/env bash
set -euo pipefail

REPO="${MEMEDOCK_REPO:-flakedZZ/meme-dock}"
APP_NAME="MemeDock"
INSTALL_DIR="/Applications"
MODE="auto"

usage() {
  cat <<'EOF'
MemeDock macOS 智能安装脚本

用法:
  ./scripts/install.sh [--repo owner/name] [--from-source] [--release-only]

选项:
  --repo owner/name   指定 GitHub 仓库（默认: flakedZZ/meme-dock）
  --from-source       跳过 GitHub Release，直接本地编译安装
  --release-only      仅尝试下载 GitHub Release，不回退本地编译
  -h, --help          显示帮助
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$2"
      shift 2
      ;;
    --from-source)
      MODE="source"
      shift
      ;;
    --release-only)
      MODE="release"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "未知参数: $1"
      usage
      exit 1
      ;;
  esac
done

ARCH="$(uname -m)"
if [[ "$ARCH" == "arm64" ]]; then
  ARCH_MATCH="aarch64"
else
  ARCH_MATCH="x64"
fi

echo "==> MemeDock macOS 安装程序 (架构: $ARCH)"

install_from_dmg() {
  local dmg_path="$1"
  echo "==> 正在挂载镜像: $dmg_path"
  local mount_point
  mount_point="$(hdiutil attach "$dmg_path" -nobrowse -quiet | awk '/\/Volumes\// { $1=$2=""; print substr($0,3) }' | head -n 1)"
  if [[ -z "$mount_point" || ! -d "$mount_point" ]]; then
    mount_point="$(ls -d /Volumes/*MemeDock* 2>/dev/null | head -n 1 || true)"
  fi
  if [[ -z "$mount_point" || ! -d "$mount_point" ]]; then
    echo "挂载 DMG 失败"
    return 1
  fi

  local app_src="$mount_point/${APP_NAME}.app"
  if [[ ! -d "$app_src" ]]; then
    app_src="$(find "$mount_point" -maxdepth 2 -name "*.app" | head -n 1)"
  fi

  echo "==> 正在安装到 ${INSTALL_DIR}/${APP_NAME}.app ..."
  rm -rf "${INSTALL_DIR}/${APP_NAME}.app"
  cp -R "$app_src" "${INSTALL_DIR}/${APP_NAME}.app"
  hdiutil detach "$mount_point" -quiet || true
  xattr -dr com.apple.quarantine "${INSTALL_DIR}/${APP_NAME}.app" 2>/dev/null || true
  echo "==> 安装完成: ${INSTALL_DIR}/${APP_NAME}.app"
}

try_download_release() {
  echo "==> 正在检查 GitHub 最新 Release ($REPO)..."
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  local release_json
  if ! release_json="$(curl -fsSL "$api_url" 2>/dev/null)"; then
    echo "    未获取到公开发布的 Release。"
    return 1
  fi

  local dmg_url
  dmg_url="$(printf '%s' "$release_json" | grep -Eo "https://[^\"]+${ARCH_MATCH}[^\"]*\.dmg" | head -n 1 || true)"
  if [[ -z "$dmg_url" ]]; then
    dmg_url="$(printf '%s' "$release_json" | grep -Eo 'https://[^"]+\.dmg' | head -n 1 || true)"
  fi

  if [[ -z "$dmg_url" ]]; then
    echo "    最新 Release 中未找到适配 ${ARCH} 的 .dmg 安装包。"
    return 1
  fi

  local tmp_dmg
  tmp_dmg="$(mktemp -t memedock-XXXXXX.dmg)"
  trap 'rm -f "$tmp_dmg"' EXIT
  echo "==> 正在下载: $dmg_url"
  curl -fL --progress-bar "$dmg_url" -o "$tmp_dmg"
  install_from_dmg "$tmp_dmg"
  return 0
}

ensure_build_deps() {
  export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

  if ! xcode-select -p >/dev/null 2>&1; then
    echo "请先安装 Xcode Command Line Tools: xcode-select --install"
    exit 1
  fi

  if ! command -v node >/dev/null 2>&1; then
    if command -v brew >/dev/null 2>&1; then
      echo "==> 检测到缺少 Node.js，正在通过 Homebrew 安装..."
      brew install node
    else
      echo "缺少 Node.js，请先安装 Homebrew 或 Node.js 20+。"
      exit 1
    fi
  fi

  if ! command -v pnpm >/dev/null 2>&1; then
    echo "==> 正在启用/安装 pnpm..."
    corepack enable 2>/dev/null || npm install -g pnpm
  fi

  if ! command -v cargo >/dev/null 2>&1; then
    if command -v brew >/dev/null 2>&1; then
      echo "==> 检测到缺少 Rust 工具链，正在通过 Homebrew 安装..."
      brew install rust
    else
      echo "==> 正在通过 rustup 安装 Rust 工具链..."
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
      export PATH="$HOME/.cargo/bin:$PATH"
    fi
  fi
}

build_from_source() {
  ensure_build_deps
  local script_dir project_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  project_dir="$(cd "$script_dir/.." && pwd)"

  if [[ ! -f "$project_dir/package.json" ]]; then
    echo "未在当前目录找到 MemeDock 源码，请先克隆仓库后执行。"
    exit 1
  fi

  cd "$project_dir"
  echo "==> 正在安装前端依赖..."
  pnpm install
  echo "==> 正在构建 MemeDock macOS 应用包..."
  CI=true pnpm tauri build --bundles app

  local app_bundle="$project_dir/src-tauri/target/release/bundle/macos/${APP_NAME}.app"
  if [[ -d "$app_bundle" ]]; then
    echo "==> 正在安装到 ${INSTALL_DIR}/${APP_NAME}.app ..."
    rm -rf "${INSTALL_DIR}/${APP_NAME}.app"
    cp -R "$app_bundle" "${INSTALL_DIR}/${APP_NAME}.app"
    echo "==> 本地构建并安装完成！"
  else
    echo "未找到构建产物: $app_bundle"
    exit 1
  fi
}

if [[ "$MODE" == "release" ]]; then
  try_download_release
elif [[ "$MODE" == "source" ]]; then
  build_from_source
else
  if ! try_download_release; then
    echo "==> 回退至本地源码编译安装..."
    build_from_source
  fi
fi

cat <<'EOF'

============================================================
MemeDock 安装完成！
1. 默认全局快捷键: Option + E（召出面板）/ Option + Shift + E（剪贴板快收）
2. 首次使用自动粘贴功能，请在「系统设置 -> 隐私与安全性 -> 辅助功能」中勾选 MemeDock。
3. 可在「应用程序」中启动 MemeDock，或在终端运行: open -a MemeDock
============================================================
EOF
