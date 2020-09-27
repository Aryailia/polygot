#!/usr/bin/env sh

NAME="$( basename "${0}"; printf a )"; NAME="${NAME%?a}"

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
  TAGS_CACHE="${CACHE}/tags.csv"
  LINK_CACHE="${CACHE}/link.csv"

  # Flags
  FORCE='false'
  API="./$( sed -n '1,/\[\[bin\]\]/d;s/"$//;s/^name = "//p' "Cargo.toml" )"

  # Options processing
  args=''
  for a in "$@"; do case "${a}"
    in -h|--help)  show_help; exit 0
    ;; -f|--force) FORCE='true'

    ;; -*) die FATAL 1 "Invalid option '${a}'. See \`${NAME} -h\` for help"
    ;; build-rust) build_rust
    ;; *)  args="${args} ${a}"
  esac; done

  [ -z "${args}" ] && { show_help; exit 1; }
  eval "set -- ${args}"

  [ ! -x "${API}" ] && die FATAL 1 "'${API}' was not found (blog api)." \
    "Run \`${NAME} build-rust\` (though one should be provided)"

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


    #;; build-rust)     build_rust
    ;; build)
      errln "Building the website and the blog"
      mkdir -p "${PUBLIC}"
      do_for_each_file_in "${SOURCE}" "${SOURCE}/" compile
      compile_blog

    ;; test)
      #errln "for testing"
      #build_rust
      #compile_post "${PUBLISHED}/blue.adoc"
      <"${TAGS_CACHE}" sieve_out_name "chinese_tones"

    ;; *) die FATAL 1 "\`${NAME} '${1}'\` is an invalid subcommand."
  esac; done
}
blah() {
  <<EOF cat - >"${TAGS_CACHE}"
Junk,2019-11-01,stuff,en,The Quick, brown fox jumped over the lazy doggo
Junk,2019-11-01,stuff,jp,これはこれはどういう意味なんだろう
Linguistics,2019-11-01,stuff,en,The Quick, brown fox jumped over the lazy doggo
Linguistics,yo,happy-times,zh,辣妹
EOF
}

#sieve_out_name() {
#  # $1: the filename to remove (no extension)
#  while IFS=',' read -r _tag _time _name _lang _title; do
#    if [ -n "${_tag}" ] && [ "${_name}" != "${1}" ]; then
#      outln "${_tag},${_time},${_name},${_lang},${_title}"
#    fi
#  done
#  if  [ -n "${_tag}" ] && [ "${_name}" != "${1}" ]; then
#    outln "${_tag},${_time},${_name},${_lang},${_title}"
#  fi
#}

build_rust() {
  # must not use FORCE, or we have to redo main
  require 'cargo' || die FATAL 1 "Could not find the executable '${API}'" \
    "And without cargo/rust installed, you cannot compile."
  cargo build --release
  ! cmp -s "target/release/${API}" "${API}" && cp "target/release/${API}" ./
}

compile_blog() {
  if "${FORCE}" || [ ! -e "${BLOG_OUTPUT}" ]
    then full_rebulid='true'
    else full_rebuild='false'
  fi
  mkdir -p "${CACHE}" "${BLOG_OUTPUT}"

  tags_cache=''
  link_cache=''
  compile_error='0'
  for file in "${PUBLISHED}"/*; do
    name="${file##*/}"
    extn="${name##*.}"
    name="${file%."${extn}"}"

    output="$( compile_post "${file}" 2>/dev/null )" || { compile_error="$?"; break; }
    if [ "${compile_error}" = 0 ]; then
      num="${output%%${NL}*}"
      output="${output#"${num}${NL}"}"
      while [ "${num}" -gt 0 ]; do
        line="${output%%${NL}*}"
        output="${output#*${NL}}"
        link_cache="${link_cache}${line}${NL}"
        num="$(( num - 1 ))"
      done
      tags_cache="${tags_cache}${output}"
    fi
  done

  if [ "${compile_error}" = 0 ]; then
    errln "Updating link cache '${LINK_CACHE}'"
    outln "${link_cache}" | sort | sed '/^$/d' >"${LINK_CACHE}"
    errln "Updating tags cache '${TAGS_CACHE}'"
    outln "${tags_cache}" | sort | sed '/^$/d' >"${TAGS_CACHE}"

    tags_output="${BLOG_OUTPUT}/tags.html"
    errln "Making tags index page '${tags_output}'"

    "${TEMPLATES}/tags.sh" "${TAGS_CACHE}" "${LINK_CACHE}" \
      | "${CONFIG}/combine.sh" \
        "domain=v:${DOMAIN}" \
        "navbar=v:$( "${TEMPLATES}/navbar.sh" "${DOMAIN}" "${tags_output#"${PUBLIC}/"}" )" \
      >"${BLOG_OUTPUT}/tags.html"
  else
    exit "${compile_error}"
  fi

}

#backup_tags() {
#  if [ -f "${TAGS_CACHE}" ]; then
#    mv TAGS_BACKUP
#  sed
#}

compile_post() {
  # $1: path to the post to compile
  #cargo run compile-markup "${1}" "${TEMPLATES}/post.sh" \
  if "${FORCE}"
    then _force_option="--force"
    else _force_option=""
  fi
  "${API}" compile-markup "${1}" "${TEMPLATES}/post.sh" \
    "blog/{lang}/{year}-{month}-{day}-{file_stem}.html" \
    --api-dir "${FILE_EXT_API}" \
    --cache-dir "${CACHE}" \
    --domain "${DOMAIN}" \
    --public-dir "${PUBLIC}" \
    --templates-dir "${TEMPLATES}" \
    ${_force_option} \
  # end

}



#run: sh % build-rust build
update() {
  filename="${1##*/}"
  parent="${1%"${filename}"}"  # has trailing '/' if not root
  filename="${filename%."${2}"}"

  from_rel="${1}"
  from="${SOURCE}/${from_rel}"
  into_rel="${parent}${filename}.${3}"
  into="${PUBLIC}/${into_rel}"
  mkdir -p "${PUBLIC}/${parent}"

  shift 3 || exit "$?"
  if "${FORCE}" || "${API}" is-first-newer-than "${from}" "${into}"; then
    "$@" "${from}" "${into}" || exit "$?"
    "${API}" sync-last-updated-of-first-to "${from}" "${into}"
    errln "Processed '\${SOURCE}/${from_rel}' -> '\${PUBLIC}/${into_rel}'"
  else
    errln "Not updated '${from_rel}' <> '${into_rel}'"
  fi
}

compile_html() {
  <"${1}" "${CONFIG}/combine.sh" \
    "prefix=v:${DOMAIN}" \
    "navbar=v:$( "${TEMPLATES}/navbar.sh" "${DOMAIN}" "${2#"${PUBLIC}/"}" )" \
  >"${2}" || exit "$?"
}

compile() {
  # $1: relative path to file to compile
  [ ! -f "${1}" ] || die FATAL 1 "Can only compile files, '${1}' is not a file"
  extension="${1##*/}"
  extension="${extension##*.}"

  case "${extension}"
    in html)      update "${1}" "${extension}" html compile_html
    ;; css|js)    update "${1}" "${extension}" html cp
    ;; sass|scss) update "${1}" "${extension}" css  sassc

    ;; "${1}") die FATAL 1 "'${1}' has no file extension"
    ;; *)      die FATAL 1 \
      "The extension '${ext}' for '${1}' is unsupported. Add it?"
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
