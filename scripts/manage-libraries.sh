#!/usr/bin/env bash
set -Eeuo pipefail

LANGUAGE="zh"
INPUT_FD=0
INSTALL_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
CONFIG_PATH="$INSTALL_DIR/config.toml"
ROOTS_FILE="$INSTALL_DIR/library-roots.txt"
SERVICE_MODE_FILE="$INSTALL_DIR/.service-mode"
SERVICE_USER_FILE="$INSTALL_DIR/.service-user"

if [[ -r /dev/tty ]]; then
  exec 3</dev/tty
  INPUT_FD=3
elif [[ ! -t 0 ]]; then
  printf 'Interactive terminal required.\n' >&2
  exit 1
fi

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

privileged() {
  if [[ "$EUID" -eq 0 ]]; then
    "$@"
  else
    sudo "$@"
  fi
}

toml_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '%s' "$value"
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

  if [[ ! -d "$path" ]]; then
    if confirm "目录不存在，是否创建？(y/n)" "Directory does not exist. Create it? (y/n)" "y"; then
      privileged mkdir -p -- "$path"
    else
      return 1
    fi
  fi

  privileged readlink -f -- "$path"
}

install_acl_tools() {
  if command -v setfacl >/dev/null 2>&1; then
    return
  fi

  if ! confirm "需要安装 ACL 工具以授权目录，是否继续？(y/n)" "ACL tools are required. Install them now? (y/n)" "y"; then
    exit 1
  fi

  if command -v apt-get >/dev/null 2>&1; then
    privileged apt-get update
    privileged apt-get install -y acl
  elif command -v dnf >/dev/null 2>&1; then
    privileged dnf install -y acl
  elif command -v yum >/dev/null 2>&1; then
    privileged yum install -y acl
  elif command -v zypper >/dev/null 2>&1; then
    privileged zypper --non-interactive install acl
  elif command -v pacman >/dev/null 2>&1; then
    privileged pacman -Sy --noconfirm acl
  else
    say "无法自动安装 ACL 工具，请先安装 setfacl。" "Unable to install ACL tools automatically. Install setfacl first."
    exit 1
  fi
}

grant_parent_access() {
  local path="$1"
  local service_user="$2"
  local parent

  parent="$(dirname -- "$path")"
  while [[ "$parent" != "/" && -n "$parent" ]]; do
    privileged setfacl -m "u:$service_user:--x" "$parent"
    parent="$(dirname -- "$parent")"
  done
}

grant_library_access() {
  local path="$1"
  local service_user="$2"
  local access_mode="$3"

  if [[ "$service_user" == "root" ]]; then
    say "服务以 root 运行，无需额外 ACL 授权。" "The service runs as root; no additional ACL is required."
    return
  fi

  install_acl_tools
  grant_parent_access "$path" "$service_user"

  if [[ "$access_mode" == "rw" ]]; then
    privileged find "$path" -type d -exec setfacl -m "u:$service_user:rwx" -m "d:u:$service_user:rwx" {} +
    privileged find "$path" -type f -exec setfacl -m "u:$service_user:rw-" {} +
  else
    privileged find "$path" -type d -exec setfacl -m "u:$service_user:r-x" -m "d:u:$service_user:r-x" {} +
    privileged find "$path" -type f -exec setfacl -m "u:$service_user:r--" {} +
  fi
}

build_roots_toml() {
  local roots_toml=""
  local root

  touch "$ROOTS_FILE"
  while IFS= read -r root || [[ -n "$root" ]]; do
    [[ -n "$root" ]] || continue
    if [[ -n "$roots_toml" ]]; then
      roots_toml+=", "
    fi
    roots_toml+="\"$(toml_escape "$root")\""
  done <"$ROOTS_FILE"

  printf '%s' "$roots_toml"
}

update_config() {
  local roots_toml
  local temp_file
  local line
  local replaced="false"

  roots_toml="$(build_roots_toml)"
  temp_file="$(mktemp)"

  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ "$line" == "local_library_roots ="* ]]; then
      printf 'local_library_roots = [%s]\n' "$roots_toml" >>"$temp_file"
      replaced="true"
    else
      printf '%s\n' "$line" >>"$temp_file"
    fi
  done <"$CONFIG_PATH"

  if [[ "$replaced" != "true" ]]; then
    rm -f "$temp_file"
    say "配置文件缺少 local_library_roots。" "The configuration file does not contain local_library_roots."
    exit 1
  fi

  if [[ -w "$CONFIG_PATH" ]]; then
    cat "$temp_file" >"$CONFIG_PATH"
  else
    privileged install -m 600 "$temp_file" "$CONFIG_PATH"
  fi
  rm -f "$temp_file"
}

restart_service() {
  local service_mode="$1"

  case "$service_mode" in
    system)
      privileged systemctl restart ting-reader.service
      ;;
    user)
      systemctl --user restart ting-reader.service
      ;;
    *)
      say "配置已更新，请手动重启 Ting Reader。" "Configuration updated. Restart Ting Reader manually."
      return
      ;;
  esac

  say "Ting Reader 服务已重启。" "The Ting Reader service has been restarted."
}

list_libraries() {
  local primary
  local root

  primary="$(sed -n 's/^local_storage_root = "\(.*\)"/\1/p' "$CONFIG_PATH" | head -n 1)"
  say "默认存储库：${primary:-未知}" "Default library: ${primary:-unknown}"
  say "其他存储库：" "Additional libraries:"

  if [[ ! -s "$ROOTS_FILE" ]]; then
    say "  暂无" "  None"
    return
  fi

  while IFS= read -r root || [[ -n "$root" ]]; do
    [[ -n "$root" ]] && printf '  - %s\n' "$root"
  done <"$ROOTS_FILE"
}

add_library() {
  local service_user="$1"
  local service_mode="$2"
  local path_input
  local library_path
  local mode_choice
  local access_mode

  path_input="$(prompt "本地存储库路径" "Local library path" "")"
  [[ -n "$path_input" ]] || { say "路径不能为空。" "Path cannot be empty."; return; }
  library_path="$(normalize_directory "$path_input")" || return

  if grep -Fqx -- "$library_path" "$ROOTS_FILE" 2>/dev/null; then
    say "该路径已经添加。" "This path has already been added."
    return
  fi

  printf '%s\n' "$(text "选择目录权限：" "Select directory access:")"
  printf '  1) %s\n' "$(text "只读（扫描和播放）" "Read-only (scan and playback)")"
  printf '  2) %s\n' "$(text "读写（允许写入 NFO、封面和元数据）" "Read-write (allow NFO, covers, and metadata)")"
  mode_choice="$(prompt "请输入选项" "Enter choice" "1")"
  [[ "$mode_choice" == "2" ]] && access_mode="rw" || access_mode="ro"

  grant_library_access "$library_path" "$service_user" "$access_mode"
  printf '%s\n' "$library_path" >>"$ROOTS_FILE"
  update_config
  restart_service "$service_mode"
  say "存储库已添加并授权：$library_path" "Library added and authorized: $library_path"
}

[[ -f "$CONFIG_PATH" ]] || { printf 'Missing config: %s\n' "$CONFIG_PATH" >&2; exit 1; }
touch "$ROOTS_FILE"

printf 'Select language / 选择语言:\n'
printf '  1) 简体中文\n'
printf '  2) English\n'
language_choice="$(prompt "请输入选项" "Enter choice" "1")"
[[ "$language_choice" == "2" ]] && LANGUAGE="en"

SERVICE_USER="$(cat "$SERVICE_USER_FILE" 2>/dev/null || id -un)"
SERVICE_MODE="$(cat "$SERVICE_MODE_FILE" 2>/dev/null || printf 'manual')"

while true; do
  printf '\n%s\n' "$(text "本地存储库管理：" "Local library management:")"
  printf '  1) %s\n' "$(text "添加并授权存储库" "Add and authorize a library")"
  printf '  2) %s\n' "$(text "查看存储库" "List libraries")"
  printf '  3) %s\n' "$(text "退出" "Exit")"
  action="$(prompt "请输入选项" "Enter choice" "1")"

  case "$action" in
    1) add_library "$SERVICE_USER" "$SERVICE_MODE" ;;
    2) list_libraries ;;
    3) exit 0 ;;
    *) say "无效选项。" "Invalid choice." ;;
  esac
done
