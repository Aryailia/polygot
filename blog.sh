#!/usr/bin/env sh
FORBIDDEN_GLOB='a-z0-9._-'    # will be surrounded by '[!' and ']'

open_in_external_editor() {
  if require "${EDITOR}"
    then "${EDITOR}" "${1}"
    else printf %s\\n "ERROR: No editor available" >&2
  fi
}

RED='\001\033[31m\002'
GREEN='\001\033[32m\002'
YELLOW='\001\033[33m\002'
BLUE='\001\033[34m\002'
MAGENTA='\001\033[35m\002'
CYAN='\001\033[36m\002'
CLEAR='\001\033[0m\002'
NEWLINE='
'

mydir="$( dirname "${0}"; printf a )"
mydir="${mydir%${NEWLINE}a}"
MAKE="./make.sh"  # inside of ${mydir}

main() {
  [ -e "${mydir}/${MAKE}" ] || die FATAL 1 \
    "Could not find '${mydir}/${MAKE}' file for setting environment variables"

  b_args=""
  for a in "$@"; do b_args="${b_args} $( outln "${a}" | eval_escape )"; done
  set --  # clear arguments for source command
  DOMAIN='this can anything' . "${mydir}/${MAKE}" 2>/dev/null
  export DOMAIN  # required by ${MAKE}
  eval "set -- ${b_args}"

  case "$( prompt_tesT "${1}" "$( printf %s\\n \
    "Type and press return to accept" \
    "${CYAN}help${CLEAR}      - print help message" \
    '' \
    "${CYAN}new${CLEAR}       - create a new draft" \
    "${CYAN}edit${CLEAR}      - edit a existing draft" \
    "${CYAN}discard${CLEAR}   - delete a draft" \
    '' \
    "${CYAN}amend${CLEAR}     - edit a published post" \
    "${CYAN}publish${CLEAR}   - move draft to posts (builds)" \
    "${CYAN}unpublish${CLEAR} - move published post to drafts (builds)" \
    "${CYAN}trash${CLEAR}     - delete a published blog post (builds)" \
    '' \
    "${CYAN}rename${CLEAR}    - rename a draft or post" \
    "Enter one of the options: ${CYAN}" \
  )" "" false  # false for no validation, i.e. accept all input
  )"
    # To do with drafts
    in n*)
      name="$( ask_unique_filenamE "${2}" rename=false )" || exit "$?"
      mkdir -p "${DRAFTS}" || exit "$?"
      outln "Creating '${name}' ..."

      <"${POST_TEMPLATES}/template.${name##*.}" "${CONFIG}/combine.sh" \
        "author=v:${AUTHOR}" \
        "today=v:$( "${BLOG_API}" now-rfc2822 )" \
        >"${DRAFTS}/${name}" \
      || exit "$?"
      open_in_external_editor "${DRAFTS}/${name}"

    ;; d*)
      printf %b\\n "${RED}Discard${CLEAR} which ${YELLOW}draft${CLEAR}?"
      name="$( deep_list_valiD "${DRAFTS}" '*/' | pick )" || exit "$?"
      rm "${DRAFTS}/${name}" || exit "$?"

    ;; e*)
      printf %b\\n "${YELLOW}Edit${CLEAR} which ${YELLOW}draft${CLEAR}?"
      path="$( deep_list_valiD "${DRAFTS}" | pick )" || exit "$?"
      open_in_external_editor "${path}"

    # To do with publishing
    ;; a*)
      printf %b\\n "${YELLOW}Edit${CLEAR} which ${GREEN}post${CLEAR}?"
      path="$( deep_list_valiD "${PUBLISHED}" | pick )" || exit "$?"
      open_in_external_editor "${path}"
      "${MAKE}" compile-blog

    ;; p*)
      printf %b\\n "${GREEN}Publish${CLEAR} which ${YELLOW}draft${CLEAR}?"
      name="$( deep_list_valiD "${DRAFTS}" '*/' | pick )" || exit "$?"
      mkdir -p "${PUBLISHED}"
      mv "${DRAFTS}/${name}" "${PUBLISHED}/${name}"
      "${MAKE}" compile-blog

    ;; u*)
      printf %b\\n "${MAGENTA}Unpublish${CLEAR} which ${GREEN}post${CLEAR}?"
      name="$( deep_list_valiD "${PUBLISHED}" '*/' | pick )" || exit "$?"
      mkdir -p "${DRAFTS}"
      mv "${PUBLISHED}/${name}" "${DRAFTS}/${name}" || exit "$?"
      "${MAKE}" compile-blog
      #unmake_post_hunks "${name}"

    ;; t*)
      printf %b\\n "${RED}Trash${CLEAR} which ${GREEN}post${CLEAR}?"
      name="$( deep_list_valiD "${PUBLISHED}" '*/' | pick )" || exit "$?"
      rm "${PUBLISHED}/${name}" || exit "$?"  # remove markup file
      "${MAKE}" compile-blog

    # Admin stuff
    ;; r*)
      printf '%b %b\n' "${YELLOW}Rename${CLEAR} which" \
        "${YELLOW}draft${CLEAR} or ${GREEN}post${CLEAR}?"
      # Opportunity to rename
      old="$( {
        deep_list_valiD "${DRAFTS}"
        deep_list_valiD "${PUBLISHED}"
      } | pick )" || exit "$?"
      dir="${old%/*}"
      old="${old##*/}"
      new="$( ask_unique_filenamE "${old}" rename=true )" || exit "$?"
      [ "${old}" != "${new}" ] && mv "${dir}/${old}" "${dir}/${new}"

      if [ "${dir}" = "${PUBLISHED}" ]; then
        "${MAKE}" compile-blog
      fi

    ;; h*)  errln WIP; exit 0
    ;; *)   errln WIP; exit 1
  esac
}

################################################################################
# Helper functions
validate_assign_extension_to_EXT() {
  EXT="${1##*/}"
  EXT="${EXT#.}"  # SPECIAL CASE: hidden files start with '.' by convention
  if [ "${EXT##*.}" != "${EXT}" ]; then
    EXT="${EXT##*.}"
    if [ ! -e "${FILE_EXT_API}/${EXT}" ]; then
      errln "No API available for files with '${EXT}' extension." \
        "(We are expecting '${FILE_EXT_API}/${EXT}' to exist.)" \
      # END
      return 1
    else
      return 0
    fi
  else
    EXT=''
    errln "'${1}' is missing a file extension"
    return 1
  fi
}


# Since we are using atom, this should correspond with RFC 3339
# NOTE: Can also how affect tags should intepreted
date_noW() {
  date -u +'%Y-%m-%dT%H:%M:%SZ'
}

# $1: an initial input 
# $2: rename=<true/false>
ask_unique_filenamE() (
  list="${NEWLINE}$(
    deep_list_valiD "${DRAFTS}" "*/" ".*"     # remove extensions
    deep_list_valiD "${PUBLISHED}" "*/" ".*"  # remove extensions
  )${NEWLINE}"
  # If renaming then, remove it from ${list} to not trip up `prompt_tesT` check
  if "${2#rename=}"; then
    fore="${list%${NEWLINE}"${1%.*}"${NEWLINE}*}"
    back="${list#*${NEWLINE}"${1%.*}"${NEWLINE}}"
    list="${fore}${NEWLINE}${back}"
    default=""
  else
    default="${1}"
  fi

  name="$( prompt_tesT "${default}" "$(
    outln \
      "Enter valid filename with desired extension (e.g 'post.adoc')" \
      "- Must not be an existing post filename (in drafts or published)," \
      "  ignoring extension, e.g. cannot have both 'a.adoc' and 'a.md'" \
      "- Must match glob '[!${FORBIDDEN_GLOB}]' (alphanumeric + extras) " \
    # END with newline

    # Add final line
    if "${2#rename=}"; then
      out "${GREEN}Filename ${YELLOW}unchanged ${CLEAR}"
      outln "'${BLUE}${1}${CLEAR}' (empty to accept): "
    else printf %s\\n \
      "${GREEN}Filename:${CLEAR} ${BLUE}"
    fi )" \
    "" \
    is_invalid_name_or_in_list accept_blank="${2#rename=}" "${list}"
    # ^ this is validation command
  )" || return "$?"

  if "${2#rename=}" && [ -z "${name}" ]
    then printf %s\\n "${1}"
    else printf %s\\n "${name}"
  fi
)

# Only for use inside `prompt_tesT` in `ask_unique_filenamE`
# $1: accept_blank=<true/false>
# $2: list of names to check against (must have a leading and trailing newline)
# $3: Name to check (passed on by `prompt_tesT`)
is_invalid_name_or_in_list() (
  if  "${1#accept_blank=}" && [ -z "${3}" ]; then
    return 1
  elif ! validate_assign_extension_to_EXT "${3}"; then
    errln
    pc "${RED}ERROR${CLEAR}: The extension '${RED}${EXT}${CLEAR}'"
    pc " for '${BLUE}${3}${CLEAR}' is missing/unsupported.\n"
    pc "Expecting the API to be located at '${FILE_EXT_API}/${EXT}'."
    errln "" ""
    return 0
  elif [ "${3}" != "${3#*[!${FORBIDDEN_GLOB}]}" ]; then
    pc '' \
      "${RED}ERROR${CLEAR}: Invalid filename (matches [!${FORBIDDEN_GLOB}])" \
      "" \
      "This script (just manages drafts) will not let you make bad names," \
      "but the Rust program powering this blog can handle any name." \
      "" \
      "URLs in browsers work best if they do not have strange characters." \
      "WIP: This likely does not transcribe to standard-compliant URLs" \
    #
    errln "" ""
    return 0
  elif [ "${2}" != "${2#*${NEWLINE}"${3%.*}"${NEWLINE}}" ]; then
    errln
    pc "${RED}ERROR${CLEAR}:"
    pc " A post named '${BLUE}${3%.*}${CLEAR}' already exists."
    errln " (It might have a different file extension though.)"
    errln ""
    return 0
  else
    return 1
  fi
)

# $1: the initial value (e.g. provided by command line argument)
# $2: the prompt
# $3: the message to display on error, adds a newline
# ... function to validate input, input is passed as last argument
prompt_tesT() {
  pt_input="${1}"; shift 1
  pt_prompt="${1}"; shift 1
  pt_error="${1}"; shift 1

  # No initial value given, ask without error message
  if [ -z "${pt_input}" ]; then
    printf %b "${pt_prompt}" >&2
    IFS= read -r pt_input || exit "$?"
    printf %b "${CLEAR}" >&2
  fi

  # Ask until valid input
  while "$@" "${pt_input}"; do  # validaiton check is true on invalid input
    printf %b "${pt_error}" >&2
    printf %b "${pt_prompt}" >&2
    IFS= read -r pt_input || exit "$?"
    printf %b "${CLEAR}" >&2
  done

  printf %s "${pt_input}"
}

# Pick one option of many
# <&0 options to pick
pick() {
  if require "fzf"; then
    #v="$( printf %s\\n "${1}" | fzf )" || return "$?"
    v="$( <&0 fzf )" || return "$?"
    outln "${v}"
    errln "${v}"
  else
    <&0 shellscript_pick
  fi
}

shellscript_pick() {
  die WIP 1 "No non-fzf system yet"; exit 1
}

# NOTE: this is all relative to ${PWD}
# NOTE: This does not produce any
# prefix with 'dl_' to avoid namespace collisions
#
# $1: directory to list
# $2: glob to remove by greedy prefix from each entry
# $3: glob to remove by suffix from each entry
# $4: '--verbose' to display permission errors, blank to not
deep_list_valiD() {
  [ ! -e "${1}" ] && return 0
  [ -d "${1}" ] || { die FATAL 1 "'${1}' is not a directory"; exit 1; }
  dl_dirlist="${1}${NEWLINE}"
  while [ -n "${dl_dirlist}" ]; do
    dl_dir="${dl_dirlist%%${NEWLINE}*}"
    dl_dirlist="${dl_dirlist#"${dl_dir}${NEWLINE}"}"
    for i in "${dl_dir}"/* "${dl_dir}"/.[!.]* "${dl_dir}"/..?*; do
      if   [ ! -e "${i}" ]; then  # filter out literal globs
        :
      elif   [ ! -r "${i}" ]; then
        [ "${4}" = '--verbose' ] && errln "UNREADABLE: '${i}'"

      # Add '/' to this check as it is a full path (make sure to not add to end)
      elif [ "${i}" != "${i#*[!/${FORBIDDEN_GLOB}]}" ]; then
        [ "${4}" = '--verbose' ] && errln "INVALID NAME: '${i}'"
      elif [ -d "${i}" ]; then
        dl_dirlist="${dl_dirlist}${i}${NEWLINE}"
      else
        i="${i##${2}}"
        printf %s\\n "${i%${3}}"
      fi
    done
  done
}


pc() { printf %b "$@" >&2; }
out() { printf %s "$@"; }
outln() { printf %s\\n "$@"; }
err() { printf %s "$@" >&2; }
errln() { printf %s\\n "$@" >&2; }
die() { printf %s "${1}: " >&2; shift 1; printf %s\\n "$@" >&2; exit "${1}"; }
eval_escape() { <&0 sed "s/'/'\\\\''/g;1s/^/'/;\$s/\$/'/"; }
require() {
  for dir in $( printf %s "${PATH}" | tr ':' '\n' ); do
    [ -f "${dir}/${1}" ] && [ -x "${dir}/${1}" ] && return 0
  done
  return 1
}



main "$@"
