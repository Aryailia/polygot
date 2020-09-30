#!/usr/bin/env sh

# A glorified version of printf, with STDIN as the format, "${MARKER}" as '%s'
# Reads STDIN and replaces instance of "${MARKER}" with "${1}", "${2}", ...
# in succession.

MARKER_FIRST="<!-- INSERT: "             # GLOB pattern
MARKER_LAST=" -->"                        # GLOB pattern
MARKER="${MARKER_FIRST}?*${MARKER_LAST}"  # GLOB pattern

lookup() {
  # Extract key (inside ${MARKER})
  key="${1#${MARKER_FIRST}}"
  key="${key%${MARKER_LAST}}"
  shift 1

  arg="$( for arg in "$@"; do
    [ "${arg}" != "${arg#"${key}="}" ] && printf %s "${arg#"${key}="}"
  done )"
  if [ -z "${arg}" ]; then
    printf %s\\n "" "----" \
      "FATAL: key '${key}' not given as a parameter. The format is:" \
      "<key>=<type>=<value>" \
      "eg. 'title=v:Cool stuff'" >&2
    exit 1
  fi
  case "${arg}"
    in v:*) printf %s "${arg#v:}"
    ;; f:*) cat "${arg#f:}"
    ;; k:*) printf %s "${MARKER_FIRST}${key}${MARKER_LAST}" >&2
  esac
}

while IFS= read -r line; do
  while [ "${line}" != "${line#*${MARKER}}" ]; do
    # line is first_middle_buffer
    buffer="${line#*${MARKER}}"
    first_middle="${line%"${buffer}"}"
    first="${first_middle%${MARKER}}"
    middle="${first_middle#"${first}"}"

    printf %s "${first}"
    #[ -n "${middle}" ] && printf %s\\n "${middle}"
    [ -n "${middle}" ] && lookup "${middle}" "$@"

    line="${buffer}"
  done
  printf %s\\n "${line}"
done
