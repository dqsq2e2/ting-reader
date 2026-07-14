#!/usr/bin/env bash
set -Eeuo pipefail

REPOSITORY="dqsq2e2/ting-reader"
LANGUAGE="zh"
INPUT_FD=0
WORK_DIR=""

if [[ -r /dev/tty ]]; then
  exec 3</dev/tty
  INPUT_FD=3
elif [[ ! -t 0 ]]; then
  printf 'Interactive terminal required. Run this script from a terminal.\n' >&2
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
  local zh_label="$1"
  local en_label="$2"
  local default_value="${3:-y}"
  local answer
  answer="$(prompt "$zh_label" "$en_label" "$default_value")"
  answer="${answer,,}"
  [[ "$answer" == "y" || "$answer" == "yes" || "$answer" == "是" ]]
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    say "缺少依赖命令：$1" "Missing required command: $1"
    exit 1
  fi
}

normalize_directory() {
  local path="$1"
  if [[ "$path" == "~" ]]; then
    path="$HOME"
  elif [[ "$path" == "~/"* ]]; then
    path="$HOME/${path#\~/}"
  elif [[ "$path" != /* ]]; then
    path="$PWD/$path"
  fi

  mkdir -p -- "$path"
  (cd -- "$path" && pwd -P)
}

toml_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '%s' "$value"
}

systemd_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//%/%%}"
  printf '%s' "$value"
}

write_config() {
  local config_path="$1"
  local install_dir="$2"
  local listen_host="$3"
  local port="$4"
  local data_dir="$5"
  local temp_dir="$6"
  local storage_dir="$7"
  local jwt_secret="$8"
  shift 8
  local additional_roots=("$@")
  local roots_toml=""
  local root

  for root in "${additional_roots[@]}"; do
    if [[ -n "$roots_toml" ]]; then
      roots_toml+=", "
    fi
    roots_toml+="\"$(toml_escape "$root")\""
  done

  cat >"$config_path" <<EOF
version = "local"

[server]
host = "$(toml_escape "$listen_host")"
port = $port
max_connections = 100
request_timeout = 30

[database]
path = "$(toml_escape "$data_dir/ting-reader.db")"
connection_pool_size = 10
busy_timeout = 30000

[plugins]
plugin_dir = "$(toml_escape "$install_dir/plugins")"
preinstalled_dir = "$(toml_escape "$install_dir/preinstalled-plugins")"
enable_hot_reload = true
max_memory_per_plugin = 536870912
max_execution_time = 300

[task_queue]
max_concurrent_tasks = 10
default_retry_count = 3
task_timeout = 600

[logging]
level = "info"
format = "json"
output = "stdout"
log_file = "$(toml_escape "$install_dir/logs/ting-reader.log")"
max_file_size = 10485760
max_backups = 5

[security]
enable_auth = false
api_key = ""
jwt_secret = "$(toml_escape "$jwt_secret")"
allowed_origins = ["*"]
rate_limit_requests = 100
rate_limit_window = 60
enable_hsts = false
hsts_max_age = 31536000

[storage]
data_dir = "$(toml_escape "$data_dir")"
temp_dir = "$(toml_escape "$temp_dir")"
local_storage_root = "$(toml_escape "$storage_dir")"
local_library_roots = [$roots_toml]
max_disk_usage = 2147483648

[audio]
cache_enabled = true
cache_size = 104857600
buffer_size = 65536
EOF
}

write_run_script() {
  local install_dir="$1"
  local data_dir="$2"
  cat >"$install_dir/run.sh" <<EOF
#!/usr/bin/env bash
set -Eeuo pipefail
cd $(printf '%q' "$install_dir")
export STATIC_DIR=$(printf '%q' "$install_dir/static")
export DATA_DIR=$(printf '%q' "$data_dir")
exec $(printf '%q' "$install_dir/ting-reader") --config $(printf '%q' "$install_dir/config.toml")
EOF
  chmod +x "$install_dir/run.sh"
}

install_systemd_service() {
  local mode="$1"
  local install_dir="$2"
  local data_dir="$3"
  local run_user
  local run_group
  local unit_file
  local service_path

  run_user="$(id -un)"
  run_group="$(id -gn)"
  unit_file="$WORK_DIR/ting-reader.service"

  cat >"$unit_file" <<EOF
[Unit]
Description=Ting Reader
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$run_user
Group=$run_group
WorkingDirectory="$(systemd_escape "$install_dir")"
ExecStart="$(systemd_escape "$install_dir/ting-reader")" --config "$(systemd_escape "$install_dir/config.toml")"
Environment="STATIC_DIR=$(systemd_escape "$install_dir/static")"
Environment="DATA_DIR=$(systemd_escape "$data_dir")"
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

  if [[ "$mode" == "system" ]]; then
    if [[ "$EUID" -eq 0 ]]; then
      install -m 644 "$unit_file" /etc/systemd/system/ting-reader.service
      systemctl daemon-reload
      systemctl enable ting-reader.service
      systemctl restart ting-reader.service
    else
      require_command sudo
      sudo install -m 644 "$unit_file" /etc/systemd/system/ting-reader.service
      sudo systemctl daemon-reload
      sudo systemctl enable ting-reader.service
      sudo systemctl restart ting-reader.service
    fi
  else
    service_path="$HOME/.config/systemd/user/ting-reader.service"
    mkdir -p "$(dirname "$service_path")"
    sed "/^User=/d; /^Group=/d; s/WantedBy=multi-user.target/WantedBy=default.target/" "$unit_file" >"$service_path"
    systemctl --user daemon-reload
    systemctl --user enable ting-reader.service
    systemctl --user restart ting-reader.service
  fi
}

printf 'Select language / 选择语言:\n'
printf '  1) 简体中文\n'
printf '  2) English\n'
language_choice="$(prompt "请输入选项" "Enter choice" "1")"
if [[ "$language_choice" == "2" ]]; then
  LANGUAGE="en"
fi

if [[ "$(uname -s)" != "Linux" ]]; then
  say "当前本地部署包仅支持 Linux。" "The native deployment packages currently support Linux only."
  exit 1
fi

case "$(uname -m)" in
  x86_64|amd64)
    ARCH="amd64"
    ;;
  aarch64|arm64)
    ARCH="arm64"
    ;;
  *)
    say "不支持的系统架构：$(uname -m)" "Unsupported system architecture: $(uname -m)"
    exit 1
    ;;
esac

require_command curl
require_command tar
say "检测到 Linux/$ARCH。" "Detected Linux/$ARCH."

DOWNLOAD_PREFIX=""
if confirm "是否启用 GitHub 镜像加速？(y/n)" "Enable GitHub download mirror? (y/n)" "n"; then
  DOWNLOAD_PREFIX="https://gh-proxy.org/"
  say "已启用镜像加速：https://gh-proxy.org/" "Download mirror enabled: https://gh-proxy.org/"
fi

printf '%s\n' "$(text "选择发布版本：" "Select release channel:")"
printf '  1) %s\n' "$(text "最新正式版" "Latest stable")"
printf '  2) %s\n' "$(text "Beta 测试版" "Beta")"
printf '  3) %s\n' "$(text "指定版本" "Specific version")"
channel_choice="$(prompt "请输入选项" "Enter choice" "1")"

case "$channel_choice" in
  2)
    RELEASE_TAG="beta"
    VERSION="beta"
    ;;
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

default_install="$HOME/.local/share/ting-reader"
install_input="$(prompt "安装目录" "Installation directory" "$default_install")"
INSTALL_DIR="$(normalize_directory "$install_input")"
if [[ "$INSTALL_DIR" == "/" ]]; then
  say "安装目录不能是根目录。" "The installation directory cannot be the filesystem root."
  exit 1
fi

printf '%s\n' "$(text "选择监听地址（Host 映射）：" "Select listen address (host binding):")"
printf '  1) 0.0.0.0  (%s)\n' "$(text "局域网可访问" "LAN accessible")"
printf '  2) 127.0.0.1 (%s)\n' "$(text "仅本机访问" "Localhost only")"
printf '  3) ::       (%s)\n' "$(text "IPv4/IPv6 双栈，需系统支持" "IPv4/IPv6 dual stack when supported")"
printf '  4) %s\n' "$(text "自定义地址" "Custom address")"
host_choice="$(prompt "请输入选项" "Enter choice" "1")"
case "$host_choice" in
  2) LISTEN_HOST="127.0.0.1" ;;
  3) LISTEN_HOST="::" ;;
  4) LISTEN_HOST="$(prompt "监听地址" "Listen address" "0.0.0.0")" ;;
  *) LISTEN_HOST="0.0.0.0" ;;
esac

while true; do
  PORT="$(prompt "服务端口" "Service port" "3000")"
  if [[ "$PORT" =~ ^[0-9]+$ ]] && (( PORT >= 1 && PORT <= 65535 )); then
    break
  fi
  say "端口必须是 1-65535 的整数。" "Port must be an integer between 1 and 65535."
done

DATA_DIR="$(normalize_directory "$(prompt "应用数据目录" "Application data directory" "$INSTALL_DIR/data")")"
TEMP_DIR="$(normalize_directory "$(prompt "临时文件目录" "Temporary files directory" "$INSTALL_DIR/temp")")"
STORAGE_DIR="$(normalize_directory "$(prompt "默认本地有声书存储库路径" "Default local audiobook library path" "$INSTALL_DIR/storage")")"
mkdir -p "$INSTALL_DIR/plugins" "$INSTALL_DIR/preinstalled-plugins" "$INSTALL_DIR/logs"

ADDITIONAL_ROOTS=()
while confirm "是否添加其他本地存储库路径？(y/n)" "Add another local library path? (y/n)" "n"; do
  root_input="$(prompt "存储库路径" "Library path" "")"
  [[ -n "$root_input" ]] || { say "路径不能为空。" "Path cannot be empty."; continue; }
  ADDITIONAL_ROOTS+=("$(normalize_directory "$root_input")")
done

if command -v openssl >/dev/null 2>&1; then
  JWT_SECRET="$(openssl rand -hex 32)"
else
  JWT_SECRET="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
fi

WORK_DIR="$(mktemp -d)"
BASE_URL="${DOWNLOAD_PREFIX}https://github.com/$REPOSITORY/releases/download/$RELEASE_TAG"
BACKEND_FILE="ting-reader-backend-linux-$ARCH-$VERSION.tar.gz"
FRONTEND_FILE="ting-reader-frontend-$VERSION.tar.gz"

say "正在下载 $RELEASE_TAG 的前后端包……" "Downloading frontend and backend packages for $RELEASE_TAG..."
curl -fL --retry 3 "$BASE_URL/$BACKEND_FILE" -o "$WORK_DIR/$BACKEND_FILE"
curl -fL --retry 3 "$BASE_URL/$FRONTEND_FILE" -o "$WORK_DIR/$FRONTEND_FILE"
mkdir -p "$WORK_DIR/backend" "$WORK_DIR/frontend"
tar -xzf "$WORK_DIR/$BACKEND_FILE" -C "$WORK_DIR/backend"
tar -xzf "$WORK_DIR/$FRONTEND_FILE" -C "$WORK_DIR/frontend"

[[ -x "$WORK_DIR/backend/ting-reader" ]] || { say "后端包缺少可执行文件。" "Backend package does not contain the executable."; exit 1; }
[[ -f "$WORK_DIR/frontend/static/index.html" ]] || { say "前端包缺少 static/index.html。" "Frontend package does not contain static/index.html."; exit 1; }

timestamp="$(date +%Y%m%d%H%M%S)"
mkdir -p "$INSTALL_DIR/backups"
if [[ -f "$INSTALL_DIR/ting-reader" ]]; then
  cp -p "$INSTALL_DIR/ting-reader" "$INSTALL_DIR/backups/ting-reader-$timestamp"
fi
if [[ -d "$INSTALL_DIR/static" ]]; then
  mv "$INSTALL_DIR/static" "$INSTALL_DIR/backups/static-$timestamp"
fi
if [[ -f "$INSTALL_DIR/config.toml" ]]; then
  cp -p "$INSTALL_DIR/config.toml" "$INSTALL_DIR/backups/config-$timestamp.toml"
fi

install -m 755 "$WORK_DIR/backend/ting-reader" "$INSTALL_DIR/ting-reader"
mv "$WORK_DIR/frontend/static" "$INSTALL_DIR/static"
if [[ -d "$WORK_DIR/backend/preinstalled-plugins" ]]; then
  cp -a "$WORK_DIR/backend/preinstalled-plugins/." "$INSTALL_DIR/preinstalled-plugins/"
fi
write_config "$INSTALL_DIR/config.toml" "$INSTALL_DIR" "$LISTEN_HOST" "$PORT" "$DATA_DIR" "$TEMP_DIR" "$STORAGE_DIR" "$JWT_SECRET" "${ADDITIONAL_ROOTS[@]}"
write_run_script "$INSTALL_DIR" "$DATA_DIR"

printf '%s\n' "$(text "选择启动方式：" "Select startup mode:")"
printf '  1) %s\n' "$(text "系统 systemd 服务（推荐，需要 sudo）" "System systemd service (recommended, requires sudo)")"
printf '  2) %s\n' "$(text "当前用户 systemd 服务" "User systemd service")"
printf '  3) %s\n' "$(text "仅生成文件，手动启动" "Files only, start manually")"
service_choice="$(prompt "请输入选项" "Enter choice" "1")"

case "$service_choice" in
  2)
    require_command systemctl
    install_systemd_service "user" "$INSTALL_DIR" "$DATA_DIR"
    START_MODE="user"
    ;;
  3)
    START_MODE="manual"
    ;;
  *)
    require_command systemctl
    install_systemd_service "system" "$INSTALL_DIR" "$DATA_DIR"
    START_MODE="system"
    ;;
esac

say "部署完成。" "Deployment completed."
say "安装目录：$INSTALL_DIR" "Installation directory: $INSTALL_DIR"
say "访问地址：http://localhost:$PORT" "Open: http://localhost:$PORT"
if [[ "$START_MODE" == "manual" ]]; then
  say "手动启动：$INSTALL_DIR/run.sh" "Start manually: $INSTALL_DIR/run.sh"
elif [[ "$START_MODE" == "user" ]]; then
  say "查看日志：journalctl --user -u ting-reader -f" "Logs: journalctl --user -u ting-reader -f"
else
  say "查看日志：sudo journalctl -u ting-reader -f" "Logs: sudo journalctl -u ting-reader -f"
fi
