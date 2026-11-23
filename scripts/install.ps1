param(
  [string]$Repo = "flakedZZ/meme-dock",
  [switch]$FromSource,
  [switch]$ReleaseOnly
)

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

Write-Host "==> MemeDock Windows 智能安装程序" -ForegroundColor Cyan

function Try-InstallFromRelease {
  Write-Host "==> 正在检查 GitHub 最新 Release ($Repo)..."
  $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"
  try {
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "MemeDock-Installer" }
  } catch {
    Write-Host "    未获取到公开发布的 Release: $($_.Exception.Message)" -ForegroundColor Yellow
    return $false
  }

  $asset = $release.assets | Where-Object { $_.name -match "setup\.exe$" } | Select-Object -First 1
  if (-not $asset) {
    $asset = $release.assets | Where-Object { $_.name -match "\.msi$" } | Select-Object -First 1
  }
  if (-not $asset) {
    Write-Host "    最新 Release 中未找到 Windows (.exe / .msi) 安装包。" -ForegroundColor Yellow
    return $false
  }

  $outPath = Join-Path $env:TEMP $asset.name
  Write-Host "==> 正在下载 $($asset.name) ..."
  Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $outPath -UseBasicParsing

  Write-Host "==> 正在启动安装向导..."
  if ($outPath.EndsWith(".msi")) {
    Start-Process msiexec.exe -ArgumentList "/i `"$outPath`"" -Wait
  } else {
    Start-Process $outPath -Wait
  }
  Write-Host "==> MemeDock 安装完成！" -ForegroundColor Green
  return $true
}

function Ensure-BuildDeps {
  if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
      Write-Host "==> 正在通过 winget 安装 Node.js LTS..."
      winget install -e --id OpenJS.NodeJS.LTS --accept-source-agreements --accept-package-agreements
    } else {
      throw "未检测到 Node.js，请先安装 Node.js 20+ (https://nodejs.org)。"
    }
  }

  if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    Write-Host "==> 正在安装 pnpm..."
    npm install -g pnpm
  }

  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
      Write-Host "==> 正在通过 winget 安装 Rustup..."
      winget install -e --id Rustlang.Rustup --accept-source-agreements --accept-package-agreements
      $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    } else {
      throw "未检测到 Rust 工具链，请先安装 rustup (https://rustup.rs)。"
    }
  }
}

function Build-FromSource {
  Ensure-BuildDeps
  $projectDir = Split-Path -Parent $PSScriptRoot
  if (-not (Test-Path (Join-Path $projectDir "package.json"))) {
    throw "未在当前目录找到 MemeDock 源码，请先克隆仓库后执行。"
  }

  Push-Location $projectDir
  try {
    Write-Host "==> 正在安装前端依赖..."
    pnpm install
    Write-Host "==> 正在编译 Windows 安装包 (NSIS / MSI)..."
    pnpm tauri build

    $nsisDir = Join-Path $projectDir "src-tauri\target\release\bundle\nsis"
    $installer = Get-ChildItem -Path $nsisDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($installer) {
      Write-Host "==> 构建完成，正在启动安装程序: $($installer.FullName)" -ForegroundColor Green
      Start-Process $installer.FullName -Wait
    } else {
      Write-Host "==> 构建完成，安装包位于 src-tauri\target\release\bundle\" -ForegroundColor Green
    }
  } finally {
    Pop-Location
  }
}

if ($ReleaseOnly) {
  if (-not (Try-InstallFromRelease)) {
    exit 1
  }
} elseif ($FromSource) {
  Build-FromSource
} else {
  if (-not (Try-InstallFromRelease)) {
    Write-Host "==> 回退至本地源码编译安装..." -ForegroundColor Yellow
    Build-FromSource
  }
}

Write-Host ""
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "MemeDock 已就绪！"
Write-Host "默认全局快捷键: Alt + E（召出面板）/ Alt + Shift + E（剪贴板快收）"
Write-Host "点击表情将自动以 CF_HDROP 文件格式写入剪贴板并粘贴至 QQ 输入框（保留 GIF 动图）。"
Write-Host "============================================================" -ForegroundColor Cyan
