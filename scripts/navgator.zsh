_NAVGATOR_SCRIPT_PATH="${(%):-%N}"
_NAVGATOR_SCRIPT_DIR="${_NAVGATOR_SCRIPT_PATH:A:h}"

_navgator_bin() {
  if [[ -n "$NAVGATOR_BIN" && -x "$NAVGATOR_BIN" ]]; then
    echo "$NAVGATOR_BIN"
    return 0
  fi

  if command -v navgator >/dev/null 2>&1; then
    command -v navgator
    return 0
  fi

  if [[ -x "$_NAVGATOR_SCRIPT_DIR/../target/release/navgator" ]]; then
    echo "$_NAVGATOR_SCRIPT_DIR/../target/release/navgator"
    return 0
  fi

  if [[ -x "$_NAVGATOR_SCRIPT_DIR/../target/debug/navgator" ]]; then
    echo "$_NAVGATOR_SCRIPT_DIR/../target/debug/navgator"
    return 0
  fi

  return 1
}

navigate() {
  local bin dir tmp exit_status
  bin="$(_navgator_bin)" || { echo "navgator binary not found" >&2; return 127; }
  tmp="$(mktemp -t navgator.XXXXXX)" || return 1
  [[ -n "$ZLE" ]] && zle -I
  if command -v script >/dev/null 2>&1; then
    NAVGATOR_OUTPUT="$tmp" script -q /dev/null "$bin" navigate </dev/tty >/dev/tty 2>/dev/tty
  else
    NAVGATOR_OUTPUT="$tmp" "$bin" navigate </dev/tty >/dev/tty 2>/dev/tty
  fi
  exit_status=$?
  if [[ $exit_status -ne 0 ]]; then
    rm -f "$tmp"
    return $exit_status
  fi
  if [[ -s "$tmp" ]]; then
    dir="$(<"$tmp")"
  fi
  rm -f "$tmp"
  if [[ -n "$dir" ]]; then
    cd -- "$dir" || return $?
  fi
  zle accept-line
  BUFFER=""
}

zle -N navigate
