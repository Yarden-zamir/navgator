navigate() {
  local dir
  dir="$(command navgator navigate)" || return $?
  if [[ -n "$dir" ]]; then
    cd -- "$dir" || return $?
  fi
  zle accept-line
  BUFFER=""
}

zle -N navigate

context() {
  local dir
  dir="$(command navgator context "$@")" || return $?
  if [[ -n "$dir" ]]; then
    cd -- "$dir" || return $?
  fi
}
