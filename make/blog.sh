#!/usr/bin/env sh

NAME="$( basename "$0"; printf a )"; NAME="${NAME%?a}"
PROJECT_DIR="$( dirname "${PWD}/${0}"; printf a )"
PROJECT_DIR="${PROJECT_DIR%?a}"
PROJECT_DIR="${PROJECT_DIR%/.}"  # If ${0} is '.', remove it
NEWLINE='
'

main() {
  cd "${PROJECT_DIR}" || die 1 "FATAL: could not run \`dirname\` on myself"
  . ./constants.sh

  TAGS_HUNK="${WORKING_DIR}/tags.csv"
  TAGS_BACKUP="${WORKING_DIR}/backup.csv"
  #PUBLIC_ROOT = (deployment) ? '/' : '.'
  PUBLIC_ROOT="${PROJECT_DIR}/${PUBLIC_DIR}"

  . "${MAKE_DIR}/adoc.sh"
  #. "${MAKE_DIR}/markdown.sh"
  mkdir -p "${WORKING_BODY_DIR}" "${WORKING_TOC_DIR}" "${BLOG_OUTPUT_DIR}"

  #recursive_list_eligiblE . "*/" ".*"

  printf %s\\n "chinese_tones.adoc" "stuff.adoc" \
    | Build_tag_hunk rebuild=true

  compile_post "${1}" \
    force=true \
    template="${CONFIG_DIR}/post.html"
}

FORBIDDEN_GLOB="[!a-z0-9./_-]"
recursive_list_eligiblE() {
  [ -d "${1}" ] || die 1 "FATAL: '${1}' is not a directory"
  rl_dirlist="${1}${NEWLINE}"
  while [ -n "${rl_dirlist}" ]; do
    rl_dir="${rl_dirlist%%${NEWLINE}*}"
    rl_dirlist="${rl_dirlist#"${rl_dir}${NEWLINE}"}"
    for i in "${rl_dir}"/* "${rl_dir}"/.[!.]* "${rl_dir}"/..?*; do
      [ ! -r "${i}" -o "${i}" != "${i#*${FORBIDDEN_GLOB}}" ] && continue
      if [ -d "${i}" ]; then
        rl_dirlist="${rl_dirlist}${i}${NEWLINE}"
      else
        i="${i##${2}}"
        printf %s\\n "${i%${3}}"
      fi
    done
  done

}


# If 'rebuild=true', read all of STDIN
# else read only the first line of STDIN
Build_tag_hunk() {
  _old="/dev/null"

  if ! "${1#rebuild=}"; then
    # Backup the "${TAGS_HUNK}" tags hunk file to "${_old}"
    if [ -f "${TAGS_HUNK}" ]; then
      _old="${TAGS_BACKUP}"
      mv "${TAGS_HUNK}" "${_old}" || exit "$?"
    fi
  else
    _old='/dev/null'
  fi

  while IFS= read -r f; do
    [ -f "${f}" ] || die 1 "FATAL: '${f}' is not a valid file"

    _ext="$( extension "${f}" )"
    _lang_list="$( <"${f}" "${_ext}"_find_available_langS )"
    if [ -z "${_lang_list}" ]
      then _tags=''; _lang_list='\\'  # Otherwise for loop does not run
      else _tags='frontmatter;'
    fi

    for _lang in ${_lang_list}; do
      # `{ext}_find_available_langS` should not have backslashes
      # so should be fine to ${_lang#\\}
      _lang="${_lang#\\}"
      _front="$( <"${f}" "${_ext}_FrontmatteR" tags="${_tags}${_lang}" )"
      <"${_old}" add_tagS "${f}" "${_lang}" "${_front}"
    done
    "${1#rebuild=}" || break  # only the first line if not rebuilding
  done | sort >"${TAGS_HUNK}"
}

# Using shellscript for less forking
add_tagS() {
  # &0: existing tag database
  # $1: filename
  # $2: language
  # $3: frontmatter
  __my_name="${1##*/}"
  __my_name="${__my_name%.*}"

  # Delete any lines with "${1}" in the 'name' column (third)
  while IFS=, read -r __line; do
    # Breakup ${__line} by commmas
    <<"    EOF" read -r __null __lang __tag __name __rest
      ,${__line}
    EOF
    [ "${__name}" != "${__my_name}" ] && printf %s\\n "${__line}"
  done

  # Split by ' *', culling and leading/trailing spaces
  # format: "<lang>,<tag>,<filename-without-extension>,<date-created>,<title>"
  __tag_string="$( trim_surrounding_spaces "$( dehasH "${3}" tags )" )"
  while [ -n "${__tag_string}" ]; do
    __tag="${__tag_string%% *}"
    __tag_string="$( trim_surrounding_spaces "${__tag_string#"${__tag}"}" )"
    printf '%s,%s,%s,%s,%s\n' \
      "${2}" \
      "${__tag}" \
      "${__my_name}" \
      "$( dehasH "${3}" created )" \
      "$( dehasH "${3}" title )" \
    #
  done
}

extension() {
  case "${1##*.}"
    in adoc|ad|asciidoctor)  printf 'adoc'; return 0
    #;; md|markdown)          printf 'md'; return 0
    #;; org)                  printf 'org'; return 0
  esac
  die 1 "FATAL: unsupported file format '${1##*.}"
}


compile_post() {
  # $1: Path to the blog post to compile
  # $2: force run `${_ext}_compile` (function sourced in relevent filehandler)
  ! [ -f "${1}" ] || ! [ -r "${1}" ] && die 1 "FATAL: cannot read post '${1}'"
  _basename="${1##*/}"
  _ext="$( extension "${1}" )"
  _newname="${_basename%.*}.html"

  _lang_list="$( <"${1}" "${_ext}"_find_available_langS )"
  if [ -z "${_lang_list}" ]
    then _tags=''; _lang_list=':'  # Throw away colon to run for loop once
    else _tags='frontmatter;'
  fi
  for _lang in ${_lang_list}; do
    _lang="${_lang#:}"
    _body="${WORKING_BODY_DIR}/${_lang}/${_newname}"  # Does not matter if
    _toc="${WORKING_TOC_DIR}/${_lang}/${_newname}"    # ${_lang} is blank
    _output_dir="${BLOG_OUTPUT_DIR}/${_lang}"
    mkdir -p "${_body%/*}" "${_toc%/*}" "${_output_dir}" || exit "$?"
    if "${2#force=}" || [ ! -f "${_body}" ] || [ ! -f "${_toc}" ]; then
      "${_ext}_compile" "${1}" "${_tags}${_lang}" "${_body}" "${_toc}" \
        || exit "$?"
    fi

    _front="$( <"${1}" "${_ext}_FrontmatteR" tags="${_tags}${_lang}" )"
    <"${3#template=}" "${MAKE_DIR}/combine.sh" \
      "prefix=v:${PUBLIC_ROOT}" \
      "title=v:$( dehasH "${_front}" title )" \
      "author=v:$( dehasH "${_front}" author )" \
      "date-created=v:$( dehasH "${_front}" date )" \
      "navbar=v:$( "${CONFIG_DIR}/navbar.sh" "${PUBLIC_ROOT}" "${_newname}" )" \
      "tags=v:$( "${CONFIG_DIR}/sidebar_tags.sh" \
        "${PUBLIC_ROOT}" "${TAGS_INDEX}" "$( dehasH "${_front}" tags )" )" \
      "toc=f:${_toc}" \
      "body=f:${_body}" \
      >"${_output_dir}/${_newname}" || exit "$?"
  done
}

trim_surrounding_spaces() {
  ____v="${1}"
  while [ "${____v}" != "${____v# }" ]; do ____v="${____v# }"; done
  while [ "${____v}" != "${____v% }" ]; do ____v="${____v% }"; done
  printf %s "${____v}"
}


dehasH() {
  ___hash="${NEWLINE}${1}"
  ___val="${___hash##*${NEWLINE}"${2}":}"
  ___val="${___val%%${NEWLINE}*}"
  trim_surrounding_spaces "${___val}"
}

die() { printf %s "${NAME} -- " >&2; printf %s\\n "$@" >&2; exit "${1}"; }

main "$@"
