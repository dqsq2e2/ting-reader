#!/usr/bin/env bash
set -Eeuo pipefail

LANGUAGE="zh"
INPUT_FD=0
INSTALL_DIR=""

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
  answer="$(prompt "$1" "$2" "${3:-n}")"
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

is_protected_path() {
  local path="$1"
  case "$path" in
    /|/bin|/boot|/dev|/etc|/home|/lib|/lib32|/lib64|/media|/mnt|/opt|/proc|/root|/run|/sbin|/srv|/sys|/tmp|/usr|/usr/local|/var|"$HOME")
      return 0
      ;;
  esac
  [[ ${#path} -lt 6 ]]
}

canonical_path() {
  local path="$1"
  if [[ -e "$path" || -L "$path" ]]; then
    readlink -f -- "$path"
  elif [[ "$path" == /* ]]; then
    printf '%s' "$path"
  else
    return 1
  fi
}

is_install_child() {
  local path="$1"
  [[ "$path" == "$INSTALL_DIR/"* && "$path" != "$INSTALL_DIR" ]]
}

safe_remove_tree() {
  local requested_path="$1"
  local expected_scope="$2"
  local resolved_path

  [[ -n "$requested_path" ]] || return
  [[ -e "$requested_path" || -L "$requested_path" ]] || return
  if [[ -L "$requested_path" ]]; then
    say "检测到符号链接，拒绝递归删除其目标：$requested_path" "Symlink detected; refusing to recursively remove its target: $requested_path"
    return 1
  fi
  resolved_path="$(canonical_path "$requested_path")" || return

  if is_protected_path "$resolved_path"; then
    say "安全检查拒绝删除：$resolved_path" "Safety check refused to remove: $resolved_path"
    return 1
  fi

  if [[ "$expected_scope" == "install" ]] && ! is_install_child "$resolved_path"; then
    say "路径不在安装目录内，拒绝删除：$resolved_path" "Path is outside the installation directory; refusing to remove: $resolved_path"
    return 1
  fi

  if [[ "$resolved_path" == "$INSTALL_DIR" || "$INSTALL_DIR" == "$resolved_path/"* ]]; then
    say "目录是安装目录或其父目录，拒绝递归删除：$resolved_path" "Directory is the installation directory or one of its parents; refusing recursive removal: $resolved_path"
    return 1
  fi

  say "正在删除：$resolved_path" "Removing: $resolved_path"
  privileged rm -rf -- "$resolved_path"
}

safe_remove_file() {
  local path="$1"
  [[ -e "$path" || -L "$path" ]] || return
  is_install_child "$path" || { say "拒绝删除安装目录外的文件：$path" "Refusing to remove a file outside the installation directory: $path"; return 1; }
  privileged rm -f -- "$path"
}

read_toml_string() {
  local key="$1"
  sed -n "s|^${key} = \"\(.*\)\".*|\1|p" "$INSTALL_DIR/config.toml" | head -n 1
}

discover_install_dir() {
  local script_path
  local target
  local unit_line

  script_path="$(readlink -f -- "${BASH_SOURCE[0]}" 2>/dev/null || true)"
  if [[ -n "$script_path" && -f "$(dirname -- "$script_path")/config.toml" ]]; then
    dirname -- "$script_path"
    return
  fi

  for target in \
    /usr/local/bin/ting-reader-uninstall \
    /usr/local/bin/ting-reader-update \
    /usr/local/bin/ting-reader-library \
    "$HOME/.local/bin/ting-reader-uninstall" \
    "$HOME/.local/bin/ting-reader-update" \
    "$HOME/.local/bin/ting-reader-library"; do
    if [[ -L "$target" ]]; then
      script_path="$(readlink -f -- "$target" 2>/dev/null || true)"
      if [[ -n "$script_path" && -f "$(dirname -- "$script_path")/config.toml" ]]; then
        dirname -- "$script_path"
        return
      fi
    fi
  done

  if command -v systemctl >/dev/null 2>&1; then
    unit_line="$(systemctl cat ting-reader.service 2>/dev/null | sed -n 's|^ExecStart=/bin/bash "\(.*\)/run.sh"|\1|p' | head -n 1)"
    if [[ -n "$unit_line" && -f "$unit_line/config.toml" ]]; then
      printf '%s' "$unit_line"
      return
    fi
  fi

  if [[ -f "$HOME/.local/share/ting-reader/config.toml" ]]; then
    printf '%s' "$HOME/.local/share/ting-reader"
  fi
}

validate_install_dir() {
  local candidate="$1"
  local resolved

  [[ "$candidate" == /* ]] || return 1
  resolved="$(canonical_path "$candidate")" || return 1
  is_protected_path "$resolved" && return 1
  [[ -f "$resolved/config.toml" ]] || return 1
  grep -q '^\[server\]' "$resolved/config.toml" || return 1
  grep -q '^\[storage\]' "$resolved/config.toml" || return 1
  if [[ ! -f "$resolved/.ting-reader-install" ]]; then
    [[ -x "$resolved/ting-reader" && -f "$resolved/run.sh" ]] || return 1
  fi
  printf '%s' "$resolved"
}

remove_matching_symlink() {
  local link_path="$1"
  local target

  [[ -L "$link_path" ]] || return
  target="$(readlink -f -- "$link_path" 2>/dev/null || true)"
  if [[ "$target" == "$INSTALL_DIR/"* ]]; then
    privileged rm -f -- "$link_path"
  fi
}

stop_services() {
  local user_unit="$HOME/.config/systemd/user/ting-reader.service"

  if command -v systemctl >/dev/null 2>&1; then
    if systemctl cat ting-reader.service >/dev/null 2>&1 || [[ -f /etc/systemd/system/ting-reader.service ]]; then
      privileged systemctl disable --now ting-reader.service 2>/dev/null || true
      if [[ -f /etc/systemd/system/ting-reader.service ]]; then
        privileged rm -f -- /etc/systemd/system/ting-reader.service
        privileged rm -f -- /etc/systemd/system/multi-user.target.wants/ting-reader.service
        privileged systemctl daemon-reload
      fi
    fi

    if [[ -f "$user_unit" ]]; then
      systemctl --user disable --now ting-reader.service 2>/dev/null || true
      rm -f -- "$user_unit"
      rm -f -- "$HOME/.config/systemd/user/default.target.wants/ting-reader.service"
      systemctl --user daemon-reload 2>/dev/null || true
    fi
  fi
}

printf 'Select language / 选择语言:\n'
printf '  1) 简体中文\n'
printf '  2) English\n'
language_choice="$(prompt "请输入选项" "Enter choice" "1")"
[[ "$language_choice" == "2" ]] && LANGUAGE="en"

detected_dir="$(discover_install_dir)"
if [[ -z "$detected_dir" ]]; then
  say "未能自动识别 Ting Reader 安装目录。为避免误删，卸载已取消。" "Unable to detect the Ting Reader installation directory automatically. Uninstallation was cancelled to prevent accidental deletion."
  exit 1
fi

INSTALL_DIR="$(validate_install_dir "$detected_dir")" || {
  say "无法确认这是有效且安全的 Ting Reader 安装目录，已取消卸载。" "The directory could not be verified as a safe Ting Reader installation. Uninstallation cancelled."
  exit 1
}

say "已识别安装目录：$INSTALL_DIR" "Detected installation directory: $INSTALL_DIR"
if ! confirm "是否确认卸载此目录中的 Ting Reader？(y/n)" "Uninstall Ting Reader from this directory? (y/n)" "n"; then
  say "已取消。" "Cancelled."
  exit 0
fi

DATA_DIR="$(read_toml_string data_dir)"
TEMP_DIR="$(read_toml_string temp_dir)"
PLUGIN_DIR="$(read_toml_string plugin_dir)"
STORAGE_DIR="$(read_toml_string local_storage_root)"

say "即将卸载安装目录中的 Ting Reader 前后端：$INSTALL_DIR" "Ting Reader frontend and backend will be removed from: $INSTALL_DIR"
say "本地有声书存储库默认保留，额外存储库永远不会自动删除。" "Audiobook libraries are preserved by default, and additional libraries are never deleted automatically."
confirmation="$(prompt "输入 UNINSTALL 确认卸载" "Type UNINSTALL to confirm" "")"
[[ "$confirmation" == "UNINSTALL" ]] || { say "已取消。" "Cancelled."; exit 0; }

DELETE_DATA="false"
DELETE_PLUGINS="false"
DELETE_TEMP="false"
DELETE_LOGS="false"
DELETE_BACKUPS="false"
DELETE_STORAGE="false"

confirm "是否删除应用数据和数据库：$DATA_DIR？(y/n)" "Delete application data and database at $DATA_DIR? (y/n)" "n" && DELETE_DATA="true"
confirm "是否删除用户安装的插件：$PLUGIN_DIR？(y/n)" "Delete user-installed plugins at $PLUGIN_DIR? (y/n)" "n" && DELETE_PLUGINS="true"
confirm "是否删除临时缓存：$TEMP_DIR？(y/n)" "Delete temporary cache at $TEMP_DIR? (y/n)" "y" && DELETE_TEMP="true"
confirm "是否删除日志目录：$INSTALL_DIR/logs？(y/n)" "Delete logs at $INSTALL_DIR/logs? (y/n)" "y" && DELETE_LOGS="true"
confirm "是否删除升级备份：$INSTALL_DIR/backups？(y/n)" "Delete upgrade backups at $INSTALL_DIR/backups? (y/n)" "n" && DELETE_BACKUPS="true"

if confirm "是否删除默认有声书存储库：$STORAGE_DIR？此操作会删除媒体文件。(y/n)" "Delete the default audiobook library at $STORAGE_DIR? This removes media files. (y/n)" "n"; then
  media_confirmation="$(prompt "输入 DELETE MEDIA 再次确认" "Type DELETE MEDIA to confirm" "")"
  [[ "$media_confirmation" == "DELETE MEDIA" ]] && DELETE_STORAGE="true"
fi

stop_services
remove_matching_symlink /usr/local/bin/ting-reader-library
remove_matching_symlink /usr/local/bin/ting-reader-update
remove_matching_symlink /usr/local/bin/ting-reader-uninstall
remove_matching_symlink "$HOME/.local/bin/ting-reader-library"
remove_matching_symlink "$HOME/.local/bin/ting-reader-update"
remove_matching_symlink "$HOME/.local/bin/ting-reader-uninstall"

safe_remove_tree "$INSTALL_DIR/static" install
safe_remove_tree "$INSTALL_DIR/preinstalled-plugins" install
safe_remove_file "$INSTALL_DIR/ting-reader"
safe_remove_file "$INSTALL_DIR/run.sh"

[[ "$DELETE_DATA" == "true" ]] && safe_remove_tree "$DATA_DIR" data
[[ "$DELETE_PLUGINS" == "true" ]] && safe_remove_tree "$PLUGIN_DIR" data
[[ "$DELETE_TEMP" == "true" ]] && safe_remove_tree "$TEMP_DIR" data
[[ "$DELETE_LOGS" == "true" ]] && safe_remove_tree "$INSTALL_DIR/logs" install
[[ "$DELETE_BACKUPS" == "true" ]] && safe_remove_tree "$INSTALL_DIR/backups" install
[[ "$DELETE_STORAGE" == "true" ]] && safe_remove_tree "$STORAGE_DIR" data

safe_remove_file "$INSTALL_DIR/config.toml"
safe_remove_file "$INSTALL_DIR/library-roots.txt"
safe_remove_file "$INSTALL_DIR/.service-mode"
safe_remove_file "$INSTALL_DIR/.service-user"
safe_remove_file "$INSTALL_DIR/.ting-reader-install"
safe_remove_file "$INSTALL_DIR/manage-libraries.sh"
safe_remove_file "$INSTALL_DIR/update.sh"
safe_remove_file "$INSTALL_DIR/uninstall.sh"

rmdir -- "$INSTALL_DIR" 2>/dev/null || true
say "Ting Reader 前后端已卸载。未确认删除的数据和所有额外媒体存储库均已保留。" "Ting Reader frontend and backend have been removed. Unconfirmed data and all additional media libraries were preserved."
