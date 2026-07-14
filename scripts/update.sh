#!/usr/bin/env bash
set -Eeuo pipefail

REPOSITORY="dqsq2e2/ting-reader"
LANGUAGE="zh"
INPUT_FD=0
WORK_DIR=""
INSTALL_DIR=""
BACKUP_DIR=""
SERVICE_MODE="manual"
REPLACEMENT_STARTED="false"

if [[ -r /dev/tty ]]; then
  exec 3</dev/tty
  INPUT_FD=3
elif [[ ! -t 0 ]]; then
  printf 'Interactive terminal required.\n' >&2
  exit 1
fi

cleanup() {
  if [[ -n "$WORK_DIR" && -d "$WORK_DIR" ]]; then
    rm -rf -- "$WORK_DIR"
  fi
}
trap cleanup EXIT

text() {
  if [[ "$LANGUAGE" == "zh" ]]; then
    printf '%s' "$1"
  else
    printf '%s' "$2"
  fi
}

say() {
  text "$1" "$2"
  printf '\n'
}

prompt() {
  local zh_label="$1"
  local en_label="$2"
  local default_value="${3-}"
  local value=""

  if [[ -n "$default_value" ]]; then
    printf '%s [%s]: ' "$(text "$zh_label" "$en_label")" "$default_value" >&2
  else
    printf '%s: ' "$(text "$zh_label" "$en_label")" >&2
  fi

  IFS= read -r -u "$INPUT_FD" value || true
  printf '%s' "${value:-$default_value}"
}

confirm() {
  local answer
  answer="$(prompt "$1" "$2" "${3:-y}")"
  answer="${answer,,}"
  [[ "$answer" == "y" || "$answer" == "yes" || "$answer" == "是" ]]
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    say "缺少依赖命令：$1" "Missing required command: $1"
    exit 1
  fi
}

privileged() {
  if [[ "$EUID" -eq 0 ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

is_protected_path() {
  local path="$1"
  case "$path" in
    /|/bin|/boot|/dev|/etc|/home|/lib|/lib32|/lib64|/media|/mnt|/opt|/proc|/root|/run|/sbin|/srv|/sys|/tmp|/usr|/usr/local|/var|"$HOME")
      return 0
      ;;
  esac
  [[ ${#path} -lt 6 ]]
}

validate_install_dir() {
  local candidate="$1"
  local resolved

  [[ "$candidate" == /* ]] || return 1
  resolved="$(readlink -f -- "$candidate" 2>/dev/null || true)"
  [[ -n "$resolved" ]] || return 1
  is_protected_path "$resolved" && return 1
  [[ -f "$resolved/config.toml" && -x "$resolved/ting-reader" && -f "$resolved/run.sh" ]] || return 1
  grep -q '^\[server\]' "$resolved/config.toml" || return 1
  grep -q '^\[storage\]' "$resolved/config.toml" || return 1
  printf '%s' "$resolved"
}

discover_install_dir() {
  local script_path
  local target
  local candidate

  script_path="$(readlink -f -- "${BASH_SOURCE[0]}" 2>/dev/null || true)"
  if [[ -n "$script_path" ]]; then
    candidate="$(dirname -- "$script_path")"
    validate_install_dir "$candidate" 2>/dev/null && return
  fi

  for target in \
    /usr/local/bin/ting-reader-update \
    /usr/local/bin/ting-reader-library \
    /usr/local/bin/ting-reader-uninstall \
    "$HOME/.local/bin/ting-reader-update" \
    "$HOME/.local/bin/ting-reader-library" \
    "$HOME/.local/bin/ting-reader-uninstall"; do
    if [[ -L "$target" ]]; then
      script_path="$(readlink -f -- "$target" 2>/dev/null || true)"
      candidate="$(dirname -- "$script_path")"
      validate_install_dir "$candidate" 2>/dev/null && return
    fi
  done

  if command -v systemctl >/dev/null 2>&1; then
    candidate="$(systemctl cat ting-reader.service 2>/dev/null | sed -n 's|^ExecStart=/bin/bash "\(.*\)/run.sh"|\1|p' | head -n 1)"
    [[ -n "$candidate" ]] && validate_install_dir "$candidate" 2>/dev/null && return
  fi

  validate_install_dir "$HOME/.local/share/ting-reader" 2>/dev/null || true
}

detect_service_mode() {
  if [[ -f "$INSTALL_DIR/.service-mode" ]]; then
    cat "$INSTALL_DIR/.service-mode"
  elif [[ -f /etc/systemd/system/ting-reader.service ]]; then
    printf 'system'
  elif [[ -f "$HOME/.config/systemd/user/ting-reader.service" ]]; then
    printf 'user'
  else
    printf 'manual'
  fi
}

stop_service() {
  case "$SERVICE_MODE" in
    system) privileged systemctl stop ting-reader.service ;;
    user) systemctl --user stop ting-reader.service ;;
    *)
      say "检测到手动启动模式。" "Manual mode detected."
      confirm "是否已停止正在运行的 Ting Reader 进程？(y/n)" "Have you stopped the running Ting Reader process? (y/n)" "n" || exit 1
      ;;
  esac
}

start_service() {
  case "$SERVICE_MODE" in
    system) privileged systemctl restart ting-reader.service ;;
    user) systemctl --user restart ting-reader.service ;;
    *) say "更新完成，请使用 $INSTALL_DIR/run.sh 手动启动。" "Update complete. Start manually with $INSTALL_DIR/run.sh." ;;
  esac
}

install_command_links() {
  if [[ "$SERVICE_MODE" == "system" ]]; then
    privileged ln -sfn "$INSTALL_DIR/manage-libraries.sh" /usr/local/bin/ting-reader-library
    privileged ln -sfn "$INSTALL_DIR/update.sh" /usr/local/bin/ting-reader-update
    privileged ln -sfn "$INSTALL_DIR/uninstall.sh" /usr/local/bin/ting-reader-uninstall
  else
    mkdir -p "$HOME/.local/bin"
    ln -sfn "$INSTALL_DIR/manage-libraries.sh" "$HOME/.local/bin/ting-reader-library"
    ln -sfn "$INSTALL_DIR/update.sh" "$HOME/.local/bin/ting-reader-update"
    ln -sfn "$INSTALL_DIR/uninstall.sh" "$HOME/.local/bin/ting-reader-uninstall"
  fi
}

rollback() {
  local exit_code=$?
  trap - ERR

  if [[ "$REPLACEMENT_STARTED" == "true" && -n "$BACKUP_DIR" ]]; then
    say "更新失败，正在恢复旧版本……" "Update failed. Restoring the previous version..."
    [[ -f "$BACKUP_DIR/ting-reader" ]] && install -m 755 "$BACKUP_DIR/ting-reader" "$INSTALL_DIR/ting-reader"
    if [[ -d "$BACKUP_DIR/static" ]]; then
      rm -rf -- "$INSTALL_DIR/static"
      mv "$BACKUP_DIR/static" "$INSTALL_DIR/static"
    fi
    if [[ -d "$BACKUP_DIR/preinstalled-plugins" ]]; then
      rm -rf -- "$INSTALL_DIR/preinstalled-plugins"
      mv "$BACKUP_DIR/preinstalled-plugins" "$INSTALL_DIR/preinstalled-plugins"
    fi
    start_service || true
  fi

  exit "$exit_code"
}

printf 'Select language / 选择语言:\n'
printf '  1) 简体中文\n'
printf '  2) English\n'
language_choice="$(prompt "请输入选项" "Enter choice" "1")"
[[ "$language_choice" == "2" ]] && LANGUAGE="en"

[[ "$(uname -s)" == "Linux" ]] || { say "更新脚本仅支持 Linux。" "The update script supports Linux only."; exit 1; }
case "$(uname -m)" in
  x86_64|amd64) ARCH="amd64" ;;
  aarch64|arm64) ARCH="arm64" ;;
  *) say "不支持的系统架构：$(uname -m)" "Unsupported architecture: $(uname -m)"; exit 1 ;;
esac

require_command curl
require_command tar

detected_dir="$(discover_install_dir)"
if [[ -z "$detected_dir" ]]; then
  say "未能自动识别 Ting Reader 安装目录。为避免更新错误目录，已取消。" "Unable to detect the Ting Reader installation directory. Update cancelled to avoid modifying the wrong directory."
  exit 1
fi
INSTALL_DIR="$(validate_install_dir "$detected_dir")"
say "已识别安装目录：$INSTALL_DIR" "Detected installation directory: $INSTALL_DIR"
confirm "是否更新此安装？(y/n)" "Update this installation? (y/n)" "y" || { say "已取消。" "Cancelled."; exit 0; }

DOWNLOAD_PREFIX=""
if confirm "是否启用 GitHub 镜像加速？(y/n)" "Enable GitHub download mirror? (y/n)" "n"; then
  DOWNLOAD_PREFIX="https://gh-proxy.org/"
fi

printf '%s\n' "$(text "选择目标版本：" "Select target release:")"
printf '  1) %s\n' "$(text "最新正式版" "Latest stable")"
printf '  2) %s\n' "$(text "Beta 测试版" "Beta")"
printf '  3) %s\n' "$(text "指定版本" "Specific version")"
channel_choice="$(prompt "请输入选项" "Enter choice" "1")"

case "$channel_choice" in
  2) RELEASE_TAG="beta"; VERSION="beta" ;;
  3)
    VERSION="$(prompt "版本号（例如 1.5.4）" "Version (for example 1.5.4)" "")"
    VERSION="${VERSION#v}"
    [[ -n "$VERSION" ]] || { say "版本号不能为空。" "Version cannot be empty."; exit 1; }
    RELEASE_TAG="v$VERSION"
    ;;
  *)
    latest_url="$(curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/$REPOSITORY/releases/latest")"
    RELEASE_TAG="${latest_url##*/}"
    VERSION="${RELEASE_TAG#v}"
    [[ "$RELEASE_TAG" == v* && -n "$VERSION" ]] || { say "无法解析最新版本。" "Unable to resolve the latest release."; exit 1; }
    ;;
esac

WORK_DIR="$(mktemp -d)"
BASE_URL="${DOWNLOAD_PREFIX}https://github.com/$REPOSITORY/releases/download/$RELEASE_TAG"
LATEST_ASSET_URL="${DOWNLOAD_PREFIX}https://github.com/$REPOSITORY/releases/latest/download"
BACKEND_FILE="ting-reader-backend-linux-$ARCH-$VERSION.tar.gz"
FRONTEND_FILE="ting-reader-frontend-$VERSION.tar.gz"

say "正在下载并校验更新包……" "Downloading and validating update packages..."
curl -fL --retry 3 "$BASE_URL/$BACKEND_FILE" -o "$WORK_DIR/$BACKEND_FILE"
curl -fL --retry 3 "$BASE_URL/$FRONTEND_FILE" -o "$WORK_DIR/$FRONTEND_FILE"
for script_name in manage-libraries.sh update.sh uninstall.sh; do
  curl -fL --retry 3 "$LATEST_ASSET_URL/$script_name" -o "$WORK_DIR/$script_name"
  [[ -s "$WORK_DIR/$script_name" ]] || { say "脚本下载失败：$script_name" "Failed to download script: $script_name"; exit 1; }
done

mkdir -p "$WORK_DIR/backend" "$WORK_DIR/frontend"
tar -xzf "$WORK_DIR/$BACKEND_FILE" -C "$WORK_DIR/backend"
tar -xzf "$WORK_DIR/$FRONTEND_FILE" -C "$WORK_DIR/frontend"
[[ -x "$WORK_DIR/backend/ting-reader" ]] || { say "后端包无效。" "Invalid backend package."; exit 1; }
[[ -f "$WORK_DIR/frontend/static/index.html" ]] || { say "前端包无效。" "Invalid frontend package."; exit 1; }

SERVICE_MODE="$(detect_service_mode)"
BACKUP_DIR="$INSTALL_DIR/backups/update-$(date +%Y%m%d%H%M%S)"
mkdir -p "$BACKUP_DIR"
cp -p "$INSTALL_DIR/ting-reader" "$BACKUP_DIR/ting-reader"
[[ -f "$INSTALL_DIR/config.toml" ]] && cp -p "$INSTALL_DIR/config.toml" "$BACKUP_DIR/config.toml"

stop_service
trap rollback ERR
REPLACEMENT_STARTED="true"

if [[ -d "$INSTALL_DIR/static" ]]; then
  mv "$INSTALL_DIR/static" "$BACKUP_DIR/static"
fi
if [[ -d "$INSTALL_DIR/preinstalled-plugins" ]]; then
  mv "$INSTALL_DIR/preinstalled-plugins" "$BACKUP_DIR/preinstalled-plugins"
fi

install -m 755 "$WORK_DIR/backend/ting-reader" "$INSTALL_DIR/ting-reader"
mv "$WORK_DIR/frontend/static" "$INSTALL_DIR/static"
if [[ -d "$WORK_DIR/backend/preinstalled-plugins" ]]; then
  cp -a "$WORK_DIR/backend/preinstalled-plugins" "$INSTALL_DIR/preinstalled-plugins"
fi
install -m 755 "$WORK_DIR/manage-libraries.sh" "$INSTALL_DIR/manage-libraries.sh"
install -m 755 "$WORK_DIR/update.sh" "$INSTALL_DIR/update.sh"
install -m 755 "$WORK_DIR/uninstall.sh" "$INSTALL_DIR/uninstall.sh"
printf 'ting-reader-native\n' >"$INSTALL_DIR/.ting-reader-install"
install_command_links
start_service

REPLACEMENT_STARTED="false"
trap - ERR
say "更新完成：$RELEASE_TAG" "Update completed: $RELEASE_TAG"
say "配置、数据库、插件和媒体存储库均已保留。" "Configuration, database, plugins, and media libraries were preserved."
say "旧版本备份：$BACKUP_DIR" "Previous version backup: $BACKUP_DIR"
