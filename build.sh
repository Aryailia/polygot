#!/usr/bin/env sh

NAME="$( basename "${0}"; printf a )"; NAME="${NAME%?a}"


show_help() {
  <<EOF cat - >&2
SYNOPSIS
  ${NAME}

DESCRIPTION
  

OPTIONS
  -
    Special argument that says read from STDIN

  --
    Special argument that prevents all following arguments from being
    intepreted as options.
EOF
}
show_help() {
  <<EOF cat - >&2
SYNOPSIS
  ${NAME}

DESCRIPTION
  

OPTIONS
  -
    Special argument that says read from STDIN

  --
    Special argument that prevents all following arguments from being
    intepreted as options.
EOF
}

NL='
'

# https://stackoverflow.com/questions/4823468/comments-in-markdown

#run: time sh %
main() {
  wd="$( pwd; printf a )"; wd="${wd%${NL}a}"
  CONFIG="${wd}/config"
  SOURCE="${wd}/source"
  PUBLIC="${wd}/public"
  DOMAIN="${PUBLIC}"
  TEMPLATES="${CONFIG}/website-templates"

  clean
  mkdir -p "${SOURCE}"
  do_for_each_file_in 'source' 'source/' compile || exit "$?"
}


FILES_TO_PROCESS_LIMIT=10000

clean() {
  rm -rf "${PUBLIC}"
}

finished() {
  errln "Processed '\${SOURCE}/${1}' -> '\${PUBLIC}/${1%.*}.${2}'"
}
compile() {
  # $1: relative path to file to compile
  [ ! -f "${1}" ] || die FATAL 1 "Can only compile files, '${1}' is not a file"
  mkdir -p "${PUBLIC}"
  case "${1##*.}"
    in sass) finished "${1}" 'sass'; sassc "${1}" "${PUBLIC}/${1%.*}.css"
    ;; html) finished "${1}" 'html'
      <"${SOURCE}/${1}" "${CONFIG}/combine.sh" \
        "prefix=v:${DOMAIN}" \
        "navbar=v:$( "${TEMPLATES}/navbar.sh" "${DOMAIN}" "${1}" )" \
        "body=f:${f}" \
      >"${PUBLIC}/${1%.*}.html"
      #
    ;; css|js)
      # -s silent
      if ! cmp -s "${SOURCE}/${1}" "${PUBLIC}/${1}"; then
        cp "${SOURCE}/${1}" "${PUBLIC}/${1}" || exit "$?"
        errln "Processed '${1}'"
      else
        errln "Skipped '${1}'"
      fi
    ;; "${1}") die FATAL 1 "'${1}' has no file extension"
  esac
}


# Follows symlinks
do_for_each_file_in() {
  # $1: directory to recurse through
  # $2...: command to run, will add argument for file 

  #[ "${1}" != "${1#/}" ] || die DEV 1 "'${1}' must be an absolute path"
  [ "${1}" != "${1#././}" ] && die DEV 1 "'${1}' must be in canonical form"
  [ -d "${1}" ] || die FATAL 1 "'${1}' is not a directory"

  lfe_to_process="././${1}"
  lfe_prefix_to_remove="${2}"
  shift 2
  lfe_count=0
  while [ -n "${lfe_to_process}" ]; do
    lfe_dir="${lfe_to_process%%././*}"
    lfe_to_process="${lfe_to_process#"${lfe_dir}"}"
    lfe_to_process="${lfe_to_process#././}"
    lfe_dir="${lfe_dir#././}"

    [ -n "${lfe_dir}" ] && for lfe_node in "${lfe_dir}"/*; do
      lfe_count="$(( lfe_count + 1 ))"
      [ "${lfe_count}" -gt "${FILES_TO_PROCESS_LIMIT}" ] && \
        die FATAL 1 "Files processed in '${1}' > '${FILES_TO_PROCESS_LIMIT}'" \
         "Increase \${FILES_TO_PROCESS_LIMIT} inside of '${NAME}'"

      if [ -d "${lfe_node}" ]; then
        lfe_to_process="${lfe_to_process}././${lfe_node}"
        continue
      fi
      "$@" "${lfe_node#"${lfe_prefix_to_remove}"}"
    done
  done
}

outln() { printf %s\\n "$@"; }
errln() { printf %s\\n "$@" >&2; }
die() { printf %s "${1}: " >&2; shift 1; printf %s\\n "$@" >&2; exit "${1}"; }

main "$@"
