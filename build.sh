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
  ${NAME} <SUBCOMMAND> [--force]

DESCRIPTION
  One of the two UIs for building the blog. Similar to makefile, you can
  specify several SUBCOMMANDs

SUBCOMMAND
 clean-public
 clean-cache
 clean-all
 compile-blog
 compile-website
 build-rust
 build             Builds both the website and the 

OPTIONS
  --help (alias: -h)
    Display this help message

  --force (alias: -f)
    Forces recompiling

EOF
}


FILES_TO_PROCESS_LIMIT=10000
NL='
'

# TODO: https://stackoverflow.com/questions/4823468/comments-in-markdown

main() {
  wd="$( dirname "${0}"; printf a )"; wd="${wd%${NL}a}"
  cd "${wd}" || exit "$?"
  PROJECT_HOME="$( pwd; printf a )"; PROJECT_HOME="${PROJECT_HOME%${NL}a}"
  CONFIG="config"
  SOURCE="source"
  PUBLIC="public"
  CACHE=".cache"

  FILE_EXT_API="${CONFIG}/api"
  BLOG_OUTPUT="${PUBLIC}/blog"
  DRAFTS="${CONFIG}/drafts"
  PUBLISHED="${CONFIG}/published"
  TEMPLATES="${CONFIG}/website-templates"

  DOMAIN="${PROJECT_HOME}/${PUBLIC}"

  # Flags
  FORCE='false'

  # Options processing
  args=''
  for a in "$@"; do case "${a}"
    in -h|--help)  show_help; exit 0
    ;; -f|--force) FORCE='true'

    ;; -*) die FATAL 1 "Invalid option '${a}'. See \`${NAME} -h\` for help"
    ;; *)  args="${args} ${a}"
  esac; done

  [ -z "${args}" ] && { show_help; exit 1; }
  eval "set -- ${args}"

  API="./$( sed -n '1,/\[\[bin\]\]/d;s/"$//;s/^name = "//p' "Cargo.toml" )"
  [ ! -x "${API}" ] && die FATAL 1 "'${API}' was not found (blog api)." \
    "Run \`${NAME} build-rust\` (though one should be provided)"

  #run: sh % build-rust build
  for subcommand in "$@"; do case "${subcommand}"
    in clean-cache)
      outln "Removing contents of '${CACHE}/'..."
      rm -rf "${CACHE}"
    ;; clean-public)
      outln "Removing contents of '${PUBLIC}/'..."
      rm -rf "${PUBLIC}"
    ;; clean-all)
      outln "Removing contents of '${CACHE}/', '${PUBLIC}/', 'target/' ..."
      require 'cargo'
      rm -rf "${CACHE}"
      rm -rf "${PUBLIC}"
      cargo clean
      rm "${API}"

    ;; compile-blog)  compile_blog
    ;; compile-website)
      errln "Building just the website (without the blog)..."
      mkdir -p "${PUBLIC}"
      do_for_each_file_in "${SOURCE}" "${SOURCE}/" compile


    ;; build-rust)    build_rust
    ;; build)
      errln "Building the website and the blog"
      mkdir -p "${PUBLIC}"
      do_for_each_file_in "${SOURCE}" "${SOURCE}/" compile
      compile_blog

    ;; test)
      errln "for testing"
      build_rust
      compile_post "${PUBLISHED}/blue.adoc"

    ;; *) die FATAL 1 "\`${NAME} '${1}'\` is an invalid subcommand."
  esac; done
}

build_rust() {
  require 'cargo' || die FATAL 1 "Could not find the executable '${API}'" \
    "And without cargo/rust installed, you cannot compile."
  cargo build --release
  ! cmp -s "target/release/${API}" "${API}" && cp "target/release/${API}" ./
}

compile_blog() {
  mkdir -p "${CACHE}" "${BLOG_OUTPUT}"
  for f in "${PUBLISHED}"/*; do
    compile_post "${f}" || exit "$?"
  done
}

compile_post() {
  # $1: path to the post to compile
  #cargo run compile-markup "${1}" "${TEMPLATES}/post.sh" \
  if "${FORCE}"
    then force_option="--force"
    else force_option=""
  fi
  "${API}" compile-markup "${1}" "${TEMPLATES}/post.sh" \
    "blog/{lang}/{year}-{month}-{day}-{file_stem}.html" \
    --api-dir "${FILE_EXT_API}" \
    --cache-dir "${CACHE}" \
    --domain "${DOMAIN}" \
    --public-dir "${PUBLIC}" \
    --templates-dir "${TEMPLATES}" \
    ${force_option} \
  # end

}



finished() {
  errln "Processed '\${SOURCE}/${1}' -> '\${PUBLIC}/${1%.*}.${2}'"
}
unchanged() {
  errln "Not updated '${1}' <> '${2}'"
}

is_forced_or_outdated() {
  "${FORCE}" || "${API}" is-first-newer-than "${1}" "${2}"
}

compile() {
  # $1: relative path to file to compile
  [ ! -f "${1}" ] || die FATAL 1 "Can only compile files, '${1}' is not a file"
  ext="${1##*/}"; ext="${ext##*.}"
  name="${1%."${ext}"}"

  from="${SOURCE}/${1}"
  neww="${PUBLIC}/${name}"

  case "${ext}"
    in sass|scss) into="${neww}.css"
      if is_forced_or_outdated "${from}" "${into}"; then
        sassc "${from}" "${into}" || exit "$?"
        "${API}" sync-last-updated-of-first-to "${from}" "${into}"
        finished "${1}" 'css'
      else
        unchanged "${1}" "${name}.css"
      fi

    ;; html)      into="${neww}.html"
      if is_forced_or_outdated "${from}" "${into}"; then
        <"${from}" "${CONFIG}/combine.sh" \
          "prefix=v:${DOMAIN}" \
          "navbar=v:$( "${TEMPLATES}/navbar.sh" "${DOMAIN}" "${1}" )" \
          "body=f:${f}" \
        >"${into}" || exit "$?"
        "${API}" sync-last-updated-of-first-to "${from}" "${into}"
        finished "${1}" 'html'
      else
        unchanged "${1}" "${name}.html"
      fi

    ;; css|js)    into="${into}.${ext}"
      if is_forced_or_outdated "${from}" "${into}"; then
        cp "${from}" "${into}" || exit "$?"
        "${API}" sync-last-updated-of-first-to "${from}" "${into}"
        finished "${1}" "${ext}"
      else
        unchanged "${1}" "${ext}"
      fi

    ;; "${1}") die FATAL 1 "'${1}' has no file extension"
    ;; *)      die FATAL 1 \
      "The extension '${_ext}' for '${1}' is unsupported. Add it?"
  esac
}

# Follows symlinks
do_for_each_file_in() {
  # $1: directory to recurse through
  # $2: prefix to remove from pathnames (typically "${1}/")
  # $3...: command to run, will add argument for file

  #[ "${1}" != "${1#/}" ] || die DEV 1 "'${1}' must be an absolute path"
  [ "${1}" != "${1#././}" ] && die DEV 1 "'${1}' must be in canonical form"
  [ -d "${1}" ] || die FATAL 1 "'${1}' is not a directory"

  # 'fe' for 'for each'
  fe_to_process="././${1}"
  fe_prefix_to_remove="${2}"
  shift 2
  fe_count=0
  while [ -n "${fe_to_process}" ]; do
    fe_dir="${fe_to_process%%././*}"
    fe_to_process="${fe_to_process#"${fe_dir}"}"
    fe_to_process="${fe_to_process#././}"
    fe_dir="${fe_dir#././}"

    if [ -n "${fe_dir}" ]; then
      for fe_node in "${fe_dir}"/* "${fe_dir}"/.[!.]* "${fe_dir}"..?*; do
        [ ! -e "${fe_node}" ] && continue
        fe_count="$(( fe_count + 1 ))"
        [ "${fe_count}" -gt "${FILES_TO_PROCESS_LIMIT}" ] && die FATAL 1 \
          "Files processed in '${1}' > '${FILES_TO_PROCESS_LIMIT}'" \
          "Increase \${FILES_TO_PROCESS_LIMIT} inside of '${NAME}'"

        if [ -d "${fe_node}" ]; then
          fe_to_process="${fe_to_process}././${fe_node}"
          continue
        fi
        "$@" "${fe_node#"${fe_prefix_to_remove}"}"
      done
    fi
  done
}

outln() { printf %s\\n "$@"; }
errln() { printf %s\\n "$@" >&2; }
die() { printf %s "${1}: " >&2; shift 1; printf %s\\n "$@" >&2; exit "${1}"; }
require() {
  for dir in $( printf %s "${PATH}" | tr ':' '\n' ); do
    [ -f "${dir}/${1}" ] && [ -x "${dir}/${1}" ] && return 0
  done
  return 1
}

main "$@"
