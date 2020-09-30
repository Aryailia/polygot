#!/usr/bin/env sh

# A glorified version of printf, with STDIN as the format, "${MARKER}" as '%s'
# Reads STDIN and replaces instance of "${MARKER}" with "${1}", "${2}", ...
# in succession.

MARKER="<!-- REPLACE: ?* -->"  # GLOB pattern

while IFS= read -r line; do
  while [ "${line}" != "${line#*${MARKER}}" ]; do
    rest="${line#*${MARKER}}"
    part="${line%"${rest}"}"; part="${part%${MARKER}}"
    printf %s "${part}"

    [ "$#" = 0 ] && { printf "ERROR: template need more args" >&2; exit 1; }

    printf %s "${1}"
    shift 1
    line="${rest}"
  done
  printf %s\\n "${line}"
done
if [ "${line}" != "${line#${MARKER}}" ]; then
  [ "$#" = 0 ] && { printf "ERROR: template need more args" >&2; exit 1; }
  printf %s\\n "${1}"
else
  printf %s "${line}"
fi

if [ "$#" -gt 0 ]; then
  printf "ERROR: too many args" >&2
  exit 1
fi
