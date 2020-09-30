#!/usr/bin/env sh

PROJECT_DIR="${PWD}/${0#"${PWD}"}"
PROJECT_DIR="${PROJECT_DIR%/*}"
PROJECT_DIR="${PROJECT_DIR%/.}"  # If ${0} is '.', remove it
cd "${PROJECT_DIR}" || exit "$?"  # ${PROJECT_DIR} only used here

# Paths relative to ${PROJECT_DIR}, that is where this script is located
 CONFIG_DIR="config"  # Website configuration, project-specific
   MAKE_DIR="make"    # Builder scripts, global
 SOURCE_DIR='src'     # Assests and content source
 PUBLIC_DIR='public'  # Compiled, public-facing content
WORKING_DIR='.blog'   # To put build files

# More customised variables
BLOG_OUTPUT_DIR="blog"  # Relative to ${PUBLIC_DIR}
    PUBLIC_ROOT="${PROJECT_DIR}/${PUBLIC_DIR}"
     TAGS_INDEX="tags.html"  # will be inside ${BLOG_OUTPUT_DIR}
# NOTE: might replace a post
# NOTE: No check in `branch_new` to make sure 'tags.html' is not overwritten
# NOTE: No check in `ask_unique_filenamE` nor publishing either


# Derived values, you probably should not customise these
   WORKING_BODY_DIR="${WORKING_DIR}/_body"
    WORKING_TOC_DIR="${WORKING_DIR}/_toc"
         DRAFTS_DIR="${WORKING_DIR}/drafts"
      PUBLISHED_DIR="${WORKING_DIR}/published"
          TAGS_HUNK="${WORKING_DIR}/tags.csv"
        TAGS_BACKUP="${WORKING_DIR}/backup.csv"
FORBIDDEN_FILE_GLOB="[!a-z0-9._-]"    # For blog post names
FORBIDDEN_PATH_GLOB="[!a-z0-9./_-]"   # Includes '/' for paths

RED='\001\033[31m\002'
GREEN='\001\033[32m\002'
YELLOW='\001\033[33m\002'
BLUE='\001\033[34m\002'
MAGENTA='\001\033[35m\002'
CYAN='\001\033[36m\002'
CLEAR='\001\033[0m\002'
NEWLINE='
'

. "${MAKE_DIR}/adoc.sh"
#. "${MAKE_DIR}/markdown.sh"
#. "${MAKE_DIR}/org-mode.sh"
. "${MAKE_DIR}/api.sh"

main() {
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
    "${CYAN}ma${CLEAR}        - remake all (blog, hunks, regular website)" \
    "${CYAN}rename${CLEAR}    - rename a draft or post" \
    "Enter one of the options: ${CYAN}" \
  )" "" false )"
    # To do with drafts
    in n*) branch_new "${2}"
    ;; d*)
      printf %b\\n "${RED}Discard${CLEAR} which ${YELLOW}draft${CLEAR}?"
      name="$( deep_list_valiD "${DRAFTS_DIR}" '*/' | pick )" || exit "$?"
      rm "${DRAFTS_DIR}/${name}" || exit "$?"
    ;; e*)
      printf %b\\n "${YELLOW}Edit${CLEAR} which ${YELLOW}draft${CLEAR}?"
      path="$( deep_list_valiD "${DRAFTS_DIR}" | pick )" || exit "$?"
      open_in_external_editor "${path}"

    # To do with publishing
    ;; a*)
      printf %b\\n "${YELLOW}Edit${CLEAR} which ${GREEN}post${CLEAR}?"
      path="$( deep_list_valiD "${PUBLISHED_DIR}" | pick )" || exit "$?"
      open_in_external_editor "${path}"
      make_post "${path}"
    ;; p*)
      printf %b\\n "${GREEN}Publish${CLEAR} which ${YELLOW}draft${CLEAR}?"
      name="$( deep_list_valiD "${DRAFTS_DIR}" '*/' | pick )" || exit "$?"
      mkdir -p "${PUBLISHED_DIR}"
      mv "${DRAFTS_DIR}/${name}" "${PUBLISHED_DIR}/${name}"
      make_post "${PUBLISHED_DIR}/${name}"
    ;; u*)
      printf %b\\n "${MAGENTA}Unpublish${CLEAR} which ${GREEN}post${CLEAR}?"
      name="$( deep_list_valiD "${PUBLISHED_DIR}" '*/' | pick )" || exit "$?"
      mkdir -p "${DRAFTS_DIR}"
      mv "${PUBLISHED_DIR}/${name}" "${DRAFTS_DIR}/${name}" || exit "$?"
      unmake_post_hunks "${name}"
    ;; t*)
      printf %b\\n "${RED}Trash${CLEAR} which ${GREEN}post${CLEAR}?"
      name="$( deep_list_valiD "${PUBLISHED_DIR}" '*/' | pick )" || exit "$?"
      rm "${PUBLISHED_DIR}/${name}" || exit "$?"  # remove markup file
      unmake_post_hunks "${name}"                       # remove hunks and public html

    # Admin stuff
    ;; r*)
      printf '%b %b\n' "${YELLOW}Rename${CLEAR} which" \
        "${YELLOW}draft${CLEAR} or ${GREEN}post${CLEAR}?"
      # Opportunity to rename
      old="$( {
        deep_list_valiD "${DRAFTS_DIR}"
        deep_list_valiD "${PUBLISHED_DIR}"
      } | pick )" || exit "$?"
      dir="${old%/*}"
      old="${old##*/}"
      new="$( ask_unique_filenamE "${old}" rename=true )" || exit "$?"
      [ "${old}" != "${new}" ] && mv "${dir}/${old}" "${dir}/${new}"
      if [ "${dir}" = "${PUBLISHED_DIR}" ]; then
        unmake_post_hunks "${old}"  # With or without ${dir} okay
        make_post "${dir}/${new}"
      fi
    ;; ld)  date_noW
    ;; ls)  status_reporT
    ;; ma)  make_entire_website
    ;; z)  unmake_post_hunks "aaaa.adoc"
    ;; bw)  build_all_nonposts
    ;; bp)  build_all_posts force-rebuild=false
    ;; bi)  build_tags_index
    ;; bt)
      update_tag_hunk --from-scratch "$( deep_list_valiD "${PUBLISHED_DIR}" )"
      build_tags_index
    ;; h*)  serrln WIP; exit 0
    ;; *)   serrln WIP; exit 1
  esac
}

################################################################################
# Customisable helper functions
open_in_external_editor() {
  if require "${EDITOR}"
    then "${EDITOR}" "${1}"
    else printf %s\\n "ERROR: No editor available" >&2
  fi
}

extension() {
  case "${1##*.}"
    in adoc|ad|asciidoctor)  printf 'adoc'
    #;; md|markdown)          printf 'md'
    #;; org)                  printf 'org'
    ;; *)                    return 1
  esac
}


# TODO: integrate with tags.sh
# FIX: index to be indepedent of compile post
# FIX: leftbar.sh
DEFAULT_LANGUAGE='en'

# This is intended to only be used within `compile_post_html` found in 'api.sh'
# Specifications for the output format can be found in  `combine.sh`
#
# $1: Filename for the output of the post
# $2: langugage of current post
# $3: Filename for the table of contents hunk of the post
# $4: Filename for the body hunk of the post
# $5: frontmatter
# $6: list of possible language versions for post
post_combiner() {
  pc_lang_dir="${BLOG_OUTPUT_DIR}${2:+"/${2}"}"
  mkdir -p "${PUBLIC_DIR}/${BLOG_OUTPUT_DIR}/${2}" \
    || { serrln "(Code $?) FATAL: error writing'"; exit "$?"; }
  <"${CONFIG_DIR}/post.html" "${MAKE_DIR}/combine.sh" \
    "prefix=v:${PUBLIC_ROOT}" \
    "title=v:$( dehasH "${5}" title )" \
    "author=v:$( dehasH "${5}" author )" \
    "date-created=v:$( dehasH "${5}" created )" \
    "navbar=v:$( "${CONFIG_DIR}/navbar.sh" \
      "${PUBLIC_ROOT}" \
      "${pc_lang_dir}/${1}"
    )" \
    "leftbar=v:$( "${CONFIG_DIR}/leftbar.sh" \
      "$( dehasH "${5}" tags )" \
      "${6}" \
      "${2}" \
      "${PUBLIC_ROOT}/${BLOG_OUTPUT_DIR}" \
      "${TAGS_INDEX}" \
      "${1}" \
    )" \
    "toc=f:${3}" \
    "body=f:${4}" \
    >"${PUBLIC_DIR}/${pc_lang_dir}/${1}" || exit "$?"
}

################################################################################
# Main operations (one level of nesting)
# Since these are all atomic actions, it does not matter if they have the
# same variable name nesting level.

status_reporT() {
  #serrln ""
  #deep_list_valiD "${SOURCE_DIR}" "" "" --verbose
  serrln '' 'Tag backup database'
  cat "${TAGS_BACKUP}"
  serrln '' 'Tag database'
  cat "${TAGS_HUNK}"

  serrln '' 'Body hunks:'
  deep_list_valiD "${WORKING_BODY_DIR}" '' '' --verbose
  serrln '' 'Table of contents hunks:'
  deep_list_valiD "${WORKING_TOC_DIR}" '' '' --verbose
  serrln '' 'Drafts:'
  deep_list_valiD "${DRAFTS_DIR}" '' '' --verbose
  serrln '' 'Published posts:'
  deep_list_valiD "${PUBLISHED_DIR}" '' '' --verbose

}

branch_new() {
  n="$( ask_unique_filenamE "${1}" rename=false )" || exit "$?"
  mkdir -p "${DRAFTS_DIR}" || exit "$?"
  ext="$( extension "${n%%}" )" \
    || { die 1 FATAL "unsupported file extension '${n}'"; exit 1; }
  case "${ext}"
    in adoc)
      <"${CONFIG_DIR}/template.adoc" "${MAKE_DIR}/combine.sh" \
        "author=v:Blogger Extrordinaire" \
        "date-created=v:$( date_noW )"
    #;; md)
    #;; org)

    ;; *)  die 2 DEV "Missing case '.${n##*.}' in \`branch_new\`"; exit 2
  esac >"${DRAFTS_DIR}/${n}" || exit "$?"
  open_in_external_editor "${DRAFTS_DIR}/${n}"
}

make_entire_website() {
  serrln "Removing public-facing directory for website"
  rm -r "${PUBLIC_DIR}" || {
    die 1 FATAL "Cannot remove current '${PUBLIC_DIR}'"
    exit 1
  }
  mkdir -p "${PUBLIC_DIR}" || exit "$?"
  build_all_nonposts
  build_all_posts force-rebuild=true
  update_tag_hunk --from-scratch "$( deep_list_valiD "${PUBLISHED_DIR}" )"
  build_tags_index
}


build_all_nonposts() {
  mkdir -p "${PUBLIC_DIR}" || {
    die 1 FATAL "Cannot create public directory '${PUBLIC_DIR}'"
    exit 1
  }

  for f in $( deep_list_valiD "${SOURCE_DIR}" ); do
    name="${f#"${SOURCE_DIR}/"}"
    serrln "Processing '${name}'"
    case "${f##*.}"
      in html)
        <"${f}" "${MAKE_DIR}/combine.sh" \
          "prefix=v:${PUBLIC_ROOT}" \
          "navbar=v:$( "${CONFIG_DIR}/navbar.sh" "${PUBLIC_ROOT}" "${name}" )" \
          "body=f:${f}" \
          >"${PUBLIC_DIR}/${name}" \
        #
      ;; scss)  sassc "${f}" "${PUBLIC_DIR}/${name%.*}.css"
      ;; *)     cp "${f}" "${PUBLIC_DIR}/${name}"
    esac
  done
}

# Separating a single make from a mass make because `rebuild_all_posts` starts
# from scratch (for speed) and `make_post` references existing files and makes
# an incremental change to be faster
build_all_posts() {
  mkdir -p "${PUBLIC_DIR}/${BLOG_OUTPUT_DIR}"
  for f in $( deep_list_valiD "${PUBLISHED_DIR}" "" "" ); do
    serrln "Processing blog post '${f}'"
    [ "${f}" != "${f#${FORBIDDEN_FILE_GLOB}}" ] && { serrln \
      "(Code 1) FATAL: Invaild filename, should match '${FORBIDDEN_FILE_GLOB}'"\
      "Did you manually add '${f}' to '${PUBLISHED_DIR}'? "
      exit 1
    }
    compile_post_html "${f}" force="${1#force-rebuild=}" \
      "${WORKING_BODY_DIR}" "${WORKING_TOC_DIR}"
  done
}

make_post() {
  serrln 'Building post...'
  compile_post_html "${1}" force=false \
    "${WORKING_BODY_DIR}" "${WORKING_TOC_DIR}"
  update_tag_hunk --add "${1}"
  build_tags_index
}

# Removes all files automatically generated by building a post (`make_post`)
# Currently does not issues errors if cleanup target does not exist
#
# $1: only need to be the extensionless filename
# NOTE: not warning in case overwrite ${TAGS_INDEX}
unmake_post_hunks() {
  n="${1##*/}"; n="${ne%.*}"

  for f in \
    $( deep_list_valiD "${WORKING_BODY_DIR}" ) \
    $( deep_list_valiD "${WORKING_TOC_DIR}" ) \
    $( deep_list_valiD "${PUBLIC_DIR}/${BLOG_OUTPUT_DIR}" )  #  languages
  do
    g="${f##*/}"; g="${g%.*}"
    if [ -f "${f}" ] && [ "${g}" = "${n}" ]; then
      serrln "Deleting hunks for '${f}'"
      rm "${f}" || exit "$?"
    fi
  done

  update_tag_hunk --remove "${ne}"
  build_tags_index  # After `update_tag_hunk` and ${BLOG_OUTPUT_DIR} 
}



# Backups up ${TAGS_HUNK} to ${TAGS_BACKUP}
# Then merges the tags from ${2} into TAG_HUNK
update_tag_hunk() {
  # $1: operation to choose: --from-scratch, --add, --remove
  # $2: list of posts to process
  serr "Building tag database '${TAGS_HUNK}'"
  serr " from $( printf %s\\n "${2}" | wc -l ) files"
  case "${1}"
    in --from-scratch)  serr " from scratch"
    ;; --add)           serr " and merging with existing database"
    ;; --remove)        serr " and removing from existing database"
    ;; *)               die 2 DEV "Typo '${1}'"; exit 2
  esac
  serr "${NEWLINE}"

  # Test for errors (if not removing)
  if [ "${1}" != '--remove' ]; then
    tag_database_extracT "${2}" >/dev/null || exit "$?"
  fi

  # Backup
  if [ -f "${TAGS_HUNK}" ]; then
    cp "${TAGS_HUNK}" "${TAGS_BACKUP}" \
      || { die "$?" FATAL "cannot backup tags database"; exit "$?"; }
  fi
  mkdir -p "${TAGS_HUNK%/*}" \
    || { die 1 FATAL "Cannot make directory for tag hunk"; exit 1; }

  {
    if [ "${1}" != '--from-scratch' ] && [ -f "${TAGS_BACKUP}" ]; then
      # TODO: this only supports one file in "${2}"
      <"${TAGS_BACKUP}" Tag_database_filteR "${2}"
    fi
    [ "${1}" != '--remove' ] && tag_database_extracT "${2}"
  } | sort >"${TAGS_HUNK}" || exit "$?"
}

# Must be after posts are created
build_tags_index() {
  mkdir -p "${TAGS_INDEX%/*}" || { serrln \
    "(Code 1) FATAL: Failed creating directory for tags index"
    exit 1;
  }
  _blogdir="${PUBLIC_DIR}/${BLOG_OUTPUT_DIR}"
  _output="$( for d in "${_blogdir}"/*; do printf ' %s' "${d}"; done )"
  _langs="$( for d in ${_output}; do printf ' %s' "${d##*/}"; done )"
  _langs="${_langs# }"
  _output="${_output:-"${_blogdir}"}"  # If no langs, set path
  _nav="$( "${CONFIG_DIR}/navbar.sh" "${PUBLIC_ROOT}" )"

  # ${l} represents directories
  # For specific languages, '' for all posts
  for d in ${_output}; do
    [ -d "${d}" ] || continue
    l="${d#"${_blogdir}"}"
    l="${l#/}"

    serrln "Processing tags index '${d}/${TAGS_INDEX}'"
    if [ "${l}" != '' ] && [ -n "${l#??}" ] && [ -n "${l%?}" ]; then
      die 1 FATAL \
        "Directory '${d}' marks an invalid language '${l}' (!= 2 characters)" \
        "Check '${d}' to see which post(s) are invalid" \
        "and edit their source files located in '${PUBLISHED_DIR}'"
      exit 1
    fi

    "${CONFIG_DIR}/tags.sh" "${TAGS_HUNK}" "${l}" "${_langs}" \
      | "${MAKE_DIR}/combine.sh" \
        prefix=v:"${PUBLIC_ROOT}" \
        dir=v:"${PUBLIC_ROOT}/${BLOG_OUTPUT_DIR}" \
        navbar=v:"${_nav}" \
      >"${d}/${TAGS_INDEX}"
  done
}



################################################################################
# Helper functions

# Since we are using atom, this should correspond with RFC 3339
# NOTE: Can also how affect tags should intepreted
date_noW() {
  date -u +'%Y-%m-%dT%H:%M:%SZ'
}

# $1: an initial input 
# $2: rename=<true/false>
ask_unique_filenamE() (
  list="${NEWLINE}$(
    deep_list_valiD "${DRAFTS_DIR}" "*/" ".*"
    deep_list_valiD "${PUBLISHED_DIR}" "*/" ".*"
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
    printf %s\\n \
      "Enter valid filename with desired extension (e.g 'post.adoc')" \
      "- Must not be an existing post filename (in drafts or published)," \
      "  ignoring extension, i.e. cannot have both 'a.adoc' and 'a.md'" \
      "- Must match glob '${FORBIDDEN_FILE_GLOB}' (alphanumeric + extras) " \
    #

    if "${2#rename=}"; then printf '%s \n' \
      "${GREEN}Filename${CLEAR} '${BLUE}${1}${CLEAR}' (empty to accept): "
    else printf %s\\n \
      "${GREEN}Filename:${CLEAR} ${BLUE}"
    fi )" \
    "" \
    is_invalid_name_or_in_list accept_blank="${2#rename=}" "${list}"
  )" || return "$?"

  if "${2#rename=}" && [ -z "${name}" ]
    then printf %s\\n "${1}"
    else printf %s\\n "${name}"
  fi
)

# Only for use inside `prompt_tesT` in `ask_unique_filenamE`
# $1: accept_blank=<true/false>
# $2: List of names to check against
# $3: Name to check (passed on by `prompt_tesT`)
is_invalid_name_or_in_list() (
  list="${NEWLINE}${2}${NEWLINE}"
  if  "${1#accept_blank=}" && [ -z "${3}" ]; then
    return 1
  elif ! extension "${3}" >/dev/null 2>&1; then
    serrln '' "ERROR: extension not supported"
    return 0
  elif [ "${3}" != "${3#*${FORBIDDEN_FILE_GLOB}}" ]; then
    serrln '' "ERROR: Invalid filename (${FORBIDDEN_FILE_GLOB})"
    return 0
  elif [ "${list}" != "${list#*${NEWLINE}"${3%.*}"${NEWLINE}}" ]; then
    serrln '' "ERROR: filename already exists"; return 0
  else
    return 1
  fi
)

# $1: the prompt
# $2: the message to display on error, adds a newline
# $3: function name that test for error, if true, ask again
prompt_tesT() {
  pt_input="${1}"; shift 1
  pt_prompt="${1}"; shift 1
  pt_error="${1}"; shift 1
  if [ -z "${pt_input}" ]; then
    printf %b "${pt_prompt}" >&2
    IFS= read -r pt_input || exit "$?"
    printf %b "${CLEAR}" >&2
  fi
  while "$@" "${pt_input}"; do
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
    printf %s\\n "${v}" | tee /dev/stderr #>/dev/stdout
  else
    <&0 shellscript_pick
  fi
}

shellscript_pick() {
  die 1 WIP "No non-fzf system yet"; exit 1
}

main "$@"
