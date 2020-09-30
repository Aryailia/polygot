#!/usr/bin/env sh

# Some of these subfunctions may not have failing conditions
# NOTE: avoid opening a read and a write to the same file
#
# &0: feed of tag database to merge into
# $1: list of input files separated so can be read by for loop
# STDOUT: the new content (direct to tag database file)
Tag_database_filteR() {
#  # Delete any records with ${1} names in them
#  while IFS= read -r _line; do
#    _name="${_line#*,*,}"; _name="${_name%%,*}"  # Get the third column
#    for _path in ${1}; do
#      _curr="${_path##*/}"; _curr="${_curr%.*}"  # Strip path and extension
#      [ "${_curr}" = "${_name}" ] && continue 2  # continue the while loop
#    done
#    printf %s\\n "${_line}"
#  done
  <&0 sed '/^[^,]*,[^*],'"${2}"',/d'
}

# This should be piped into a sort, but we need to exit on errors which means
# we cannot pipe.
#
# Some of these subfunctions may not have failing conditions
# $1: list of input files separated so can be read by for loop
tag_database_extracT() {
  # Print every line of tag information per file lang per file
  for _path in ${1}; do
    _name="${_path##*/}"; _name="${_name%.*}"  # Strip path and extension
    _ext="$( extension "${_path}" )" \
      || { die 1 "FATAL" "Unsupported file extension '${_path}'"; exit 1; }

    _lang_list="$( <"${_path}" "${_ext}"_find_available_langS )" \
      || { die 1 FATAL "Language tags malformed '${_path}'"; exit 1; }
    if [ -z "${_lang_list}" ]
      then _sections=''; _lang_list=':'  # Otherwise for loop does not run
      else _sections='frontmatter;'
    fi

    for _lang in ${_lang_list}; do
      _lang="${_lang#:}"
      _front="$( <"${_path}" "${_ext}_FrontmatteR" "${_sections}${_lang}" )" \
        || { die 1 FATAL "Frontmatter formatted incorrectly"; exit 1; }
      _tag_string="$( dehasH "${_front}" tags )"
      # space for delimiter
      [ "${_tag_string}" != "${_tag_string#[!A-Za-z ]}" ] \
        && { die 1 FATAL "Post '${_path#*/}' has invalid tags" \
                 "Tags are A-Z or a-z characters only"; exit 1; }

      for _tag in ${_tag_string}; do
        printf '%s,%s,%s,%s,%s\n' \
          "${_tag}" \
          "$( dehasH "${_front}" date-created )" \
          "${_name}" \
          "${_lang}" \
          "$( dehasH "${_front}" title )" \
        # For final backslash, for easy editing
      done
    done
  done
}

compile_post_html() {
  # $1: Path to the blog post to compile
  # $2: force=<true/false> if false, no `${_ext}_compile` unless hunks missing
  # $3: Directory path for the table-of-contents hunk
  # $3: Directory path for the body hunk
  if ! [ -f "${1}" ] || ! [ -r "${1}" ]; then
    die 1 "FATAL: cannot read post '${1}'"
    exit 1
  fi
  _name="${1##*/}"
  _name="${_name%.*}.html"
  _ext="$( extension "${1}" )" \
    || { die 1 FATAL "unsupported file extension '${1}'"; exit 1; }

  _lang_list="$( <"${1}" "${_ext}"_find_available_langS )"
  if [ -z "${_lang_list}" ]
    then _tags=''; _lang_list=':'  # Throw away colon to run for loop once
    else _tags='frontmatter;'
  fi

  for _lang in ${_lang_list}; do
    _lang="${_lang#:}"  # Does not matter if ${_lang} is blank
    #_body="${WORKING_BODY_DIR}/${_lang}/${_newname}"
    #_toc="${WORKING_TOC_DIR}/${_lang}/${_newname}"
    _body="${3}/${_lang}/${_name}"
    _toc="${4}/${_lang}/${_name}"
    mkdir -p "${3}/${_lang}" "${4}/${_lang}" || exit "$?"
    if "${2#force=}" || [ ! -f "${_body}" ] || [ ! -f "${_toc}" ]; then
      "${_ext}_compile" "${1}" "${_tags}${_lang}" "${_body}" "${_toc}" \
        || exit "$?"
    fi

    _front="$( <"${1}" "${_ext}_FrontmatteR" tags="${_tags}${_lang}" )"
    post_combiner "${_name}" "${_lang}" "${_toc}" "${_body}" "${_front}" \
      "${_lang_list}"
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


# NOTE: this is all relative to ${PWD} (which is ${PROJECT_HOME})
# NOTE: This does not produce any
# prefix with 'dl_' to avoid namespace collisions
#
# $1: directory to list
# $2: glob to remove by greedy prefix from each entry
# $3: glob to remove by suffix from each entry
# $4: '--verbose' to display permission errors, blank to not
deep_list_valiD() {
  [ -d "${1}" ] || { die 1 FATAL "'${1}' is not a directory"; exit 1; }
  dl_dirlist="${1}${NEWLINE}"
  while [ -n "${dl_dirlist}" ]; do
    dl_dir="${dl_dirlist%%${NEWLINE}*}"
    dl_dirlist="${dl_dirlist#"${dl_dir}${NEWLINE}"}"
    for i in "${dl_dir}"/* "${dl_dir}"/.[!.]* "${dl_dir}"/..?*; do
      if   [ ! -e "${i}" ]; then  # filter out literal globs
        :
      elif   [ ! -r "${i}" ]; then
        [ "${4}" = '--verbose' ] && serrln "UNREADABLE: '${i}'"
      elif [ "${i}" != "${i#*${FORBIDDEN_PATH_GLOB}}" ]; then
        [ "${4}" = '--verbose' ] && serrln "INVALID NAME: '${i}'"
      elif [ -d "${i}" ]; then
        dl_dirlist="${dl_dirlist}${i}${NEWLINE}"
      else
        i="${i##${2}}"; printf %s\\n "${i%${3}}"
      fi
    done
  done
}

serr() { printf %s "$@" >&2;  }
serrln() { printf %s\\n "$@" >&2;  }
require() {
  for dir in $( printf %s "${PATH}" | tr ':' '\n' ); do
    [ -f "${dir}/$1" ] && [ -x "${dir}/$1" ] && return 0
  done
  return 1
}

die() {
  e="${1}"
  printf '(Code %s) %s: ' "${1}" "${2}" >&2
  shift 2
  printf %s\\n "$@" >&2
  return "${e}"
}
