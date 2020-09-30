#!/usr/bin/env sh

NAME="$( basename "$0"; printf a )"; NAME="${NAME%?a}"

show_help() {
  <<EOF cat - >&2
SYNOPSIS
  ${NAME} [SUBCOMMAND] [OPTIONS]

DESCRIPTION
  This a script that presents the UI of just editing and publishing drafts
  written in an easy-to-edit markup language of your choice and handles the
  busy work of ordering posts by date, preparing a page to sort by tags, making
  an RSS feed for updates.

  The goal is to be easily customisable via 'config.sh'.

SUBCOMMANDS
  If given no command, it will show the menu

  h, help
    Shows this help menu

  b, build
    As this program separates the process into a compile and build step (to
    runtime), this is a method to rebuild the entire blog from stratch in case
    something strange happens.

    Specifically, it deletes the public posts directory specified by
    '\${OUTPUT}/\${POSTSDIR}' as well as the intermediary build files specified
    partially by '\${BUILD}', recompiles all posts inside '\${PUBLISHED}' to
    inside '\${BUILD}', and then uses '\${SOURCE}' and the files just made in
    '\${BUILD}' to rebuilds the public directory '\${OUTPUT}'

 new
   Creates a new draft

 edit
   edit a existing draft

 discard
   delete a draft

 publish
   move draft to posts (builds)

 revise
   edit in-place a published post (builds)

 unpublish
   move published post to drafts (builds)

 trash
   delete a published blog post (builds)

OPTIONS
  --
    Special argument that prevents all following arguments from being
    intepreted as options.

  -v, --verbose
    Prints status messages to STDERR. Is mostly silent without with option.

ENVIRONMENT VARIABLES
  EDITOR
    This is the program to edit any blog posts. This has no default set.

  PROJECTHOME
    Default set to the current directory of '${NAME}' script. This is from
    where 'config.sh' will be sourced from. This variable is also available for
    use from within 'config.sh'

EOF
}

# No trailing slashes for these
if [ -z "${PROJECTHOME}" ]; then
  PROJECTHOME="$( dirname "$0"; printf a )"; PROJECTHOME="${PROJECTHOME%?a}"
fi

NEWLINE='
'

# TODO: Make sure deletion is in working order
# TODO: Build all files within '${SOURCE}'
# Make unit tests? lol
# Handles options that need arguments
main() {
  # Dependencies
  cd "${PROJECTHOME}" || die 1 'FATAL' "\`main\` - 'config.sh' not found"
  . ./config.sh

  # Constants
  RED='\001\033[31m\002'
  GREEN='\001\033[32m\002'
  YELLOW='\001\033[33m\002'
  BLUE='\001\033[34m\002'
  MAGENTA='\001\033[35m\002'
  CYAN='\001\033[36m\002'
  CLEAR='\001\033[0m\002'

  POST_BUILD_DIR="${BUILD}/temp/${POSTSDIR}"
  PUBLISHED="${BUILD}/published"
  DRAFTS="${BUILD}/drafts"

  # Option variables and their defaults
  VERBOSE='false'

  # Options processing
  args=''
  literal='false'
  while [ "$#" -gt 0 ]; do
    "${literal}" || case "$1" in
      --)  literal='true'; shift 1; continue ;;
      -h|--help)  show_help; exit 0 ;;

      -v|--verbose)  VERBOSE='true' ;;
      #-e|--example2)  soutln "-$2-"; shift 1 ;;

      *)   args="${args} $( outln "$1" | eval_escape )" ;;
    esac
    "${literal}" && args="${args} $( outln "$1" | eval_escape )"
    shift 1
  done

  eval "set -- ${args}"

  case "$( if [ "$#" = 0 ];
    then prompt_test '.*' "$( pcln \
      "${CYAN}help${CLEAR}      - print help message" \
      "${CYAN}build${CLEAR}     - rebuilds all temp files and the whole blog" \
      '' \
      "${CYAN}new${CLEAR}       - create a new draft" \
      "${CYAN}edit${CLEAR}      - edit a existing draft" \
      "${CYAN}discard${CLEAR}   - delete a draft" \
      "${CYAN}publish${CLEAR}   - move draft to posts (builds)" \
      "${CYAN}revise${CLEAR}    - edit in-place a published post (builds)" \
      "${CYAN}unpublish${CLEAR} - move published post to drafts (builds)" \
      "${CYAN}trash${CLEAR}     - delete a published blog post (builds)" \
      ; pc "Enter one of the options: ${CYAN}" )"
    else out "$1"
  fi )" in
    n*)
      name="$( ask_unique_filename )" || exit "$?"
      mkdir -p "${DRAFTS}"
      new_file "${DRAFTS}/${name}"
      "${EDITOR}" "${DRAFTS}/${name}"
      ;;
    e*)
      pcln "${YELLOW}Edit${CLEAR} which ${YELLOW}draft${CLEAR}?"
      old="$( pick "$( list_files_iN "${DRAFTS}" )" )" || exit "$?"
      new="$( ask_unique_filename "${old}" )" || return "$?"
      [ "${old}" != "${new}" ] \
        && mv "${DRAFTS}/${old}" "${DRAFTS}/${new}"
      "${EDITOR}" "${DRAFTS}/${new}" ;;
    d*)
      pcln "${RED}Discard${CLEAR} which ${YELLOW}draft${CLEAR}?"
      name="$( pick "$( list_files_iN "${DRAFTS}" )" )" || exit "$?"
      rm "${DRAFTS}/${name}" ;;
    p*)
      pcln "${GREEN}Publish${CLEAR} which ${YELLOW}draft${CLEAR}?"
      name="$( pick "$( list_files_iN "${DRAFTS}" )" )" || exit "$?"
      mkdir -p "${PUBLISHED}"
      [ -e "${PUBLISHED}/${name}" ] && die 1 FATAL \
        "'${name}' already exists in the '${PUBLISHED}' folder"
      mv "${DRAFTS}/${name}" "${PUBLISHED}/${name}"
      <"${PUBLISHED}/${name}" Compile_post "${name}" "${POST_BUILD_DIR}"
      outln "${POST_BUILD_DIR}/${name}" | Build_posts "${OUTPUT}/${POSTSDIR}"
      make_rest ;;
    r*)
      pcln "${YELLOW}Revise${CLEAR} which ${GREEN}post${CLEAR}?"
      old="$( pick "$( list_files_iN "${PUBLISHED}" )" )" || exit "$?"
      new="$( ask_unique_filename "${old}" )" || return "$?"
      [ "${old}" != "${new}" ] \
        && mv "${PUBLISHED}/${old}" "${PUBLISHED}/${new}"
      "${EDITOR}" "${PUBLISHED}/${new}"
      <"${PUBLISHED}/${new}" Compile_post "${new}" "${POST_BUILD_DIR}"
      make_rest ;;
    u*)
      pcln "${MAGENTA}Unpublish${CLEAR} which ${GREEN}post${CLEAR}?"
      name="$( pick "$( list_files_iN "${PUBLISHED}" )" )" || exit "$?"
      [ -e "${DRAFTS}/${name}" ] && die 1 FATAL \
        "'${name}' already exists in the '${DRAFTS}' folder"
      mv "${PUBLISHED}/${name}" "${DRAFTS}/${name}"
      html_name="$( html_namE "${name}" )"
      rm "${POST_BUILD_DIR}/${html_name}" "${OUTPUT}/${POSTSDIR}/${html_name}"
      make_rest ;;
    t*)
      pcln "${RED}Trash${CLEAR} which ${GREEN}post${CLEAR}?"
      name="$( pick "$( list_files_iN "${PUBLISHED}" )" )" || exit "$?"
      rm "${PUBLISHED}/${name}"
      html_name="$( html_namE "${name}" )"
      rm "${POST_BUILD_DIR}/${html_name}" "${OUTPUT}/${POSTSDIR}/${html_name}"
      make_rest ;;

    b*)  make_all ;;
    #b*)  make_rest ;;
    z*) <"${PUBLISHED}/yuio.md" Compile_post 'yuio.md' '.' | Make_html_toC
      ;;

    *)  show_help; exit 1 ;;
  esac
}

new_file() {
  _title="$( outln "${1##*/}" | sed -e 's/[-_]/ /g' )"
  case "${1##*.}" in
    adoc)  adoc_posT "${_title}" >"$1" ;;
    md)    md_posT "${_title}" >"$1"  ;;
    *)     die 1 FATAL "'${1##*/}' new_file - not a supported file type"
  esac
}


# Putting it together
make_all() {
  rm -rf "${POST_BUILD_DIR}"
  [ -z "${BUILD}" ] && die 1 FATAL "Building requires the \${BUILD} set"
  compile_all_posts_in "${PUBLISHED}"
  list_files_iN "${POST_BUILD_DIR}" \
    | Map_add_prefiX "${POST_BUILD_DIR}/" \
    | Build_posts "${OUTPUT}/${POSTSDIR}"
  make_rest
}

make_rest() {
  [ -z "${BUILD}" ] && die 1 FATAL "Building requires the \${BUILD} set"
  mkdir -p "${BUILD}"

  #list_files_iN "${PUBLISHED}/" "${PUBLISHED}" \
  #  | Validate_frontmatter || exit "$?"

  sorted="$( list_files_iN "${POST_BUILD_DIR}" \
    | Sort_by_datE "${PUBLISHED}" \
    | Map_add_prefiX "${POST_BUILD_DIR}/" \
    | tac  # Reorders to latest first
  )"

  [ -z "${sorted}" ] && die 1 FATAL \
    "No posts in '${POST_BUILD_DIR}' to build" \
    "Try running \`$0 build\` or just  \`$0 b\`"

  # TODO: Improve this check to actually check if lists are of equal content
  if [ "$( outln "${sorted}" | wc -l )" != \
    "$( list_files_iN "${PUBLISHED}" | wc -l )" ]
  then
    errln "Some posts were not built or not deleted..."
    errln "Recompiling and rebuilding posts..."
    compile_all_posts_in "${PUBLISHED}"
    outln "${sorted}" | Build_posts "${OUTPUT}/${POSTSDIR}"
  fi

  # Individual posts
  outln "${sorted}" | Build_archive "${SOURCE}" "${OUTPUT}"
  outln "${sorted}" | Build_rss
  outln "${sorted}" | Build_tagfile "${SOURCE}" "${OUTPUT}"
}

Map_add_prefiX() {
  while IFS= read -r ___line; do
    outln "$1${___line}"
  done
}

# TODO: Add validation
#Validate_frontmatter() {
#  while IFS= read -r _file; do
#    _front="$( fetch_frontmatteR "${_file}" )"
#    errln "|${_file}|"
#    outln "${_front}" | while IFS= read -r _line; do
#      # This affects markdown as `fetch_frontmatteR` filters adoc files
#      [ "${_line}" != "${line#*:*}" ] || errln "No multiline in frontmatter"
#    done
#  done
#}

Compile_post() {
  # $1: is the full filename, determines parser and 'html_filename' frontmatter
  # $2: is the output directory for the intermediary build file
  mkdir -p "$2"
  _content="$( cat - )"
  _front="$( outln "${_content}" | Fetch_frontmatteR "$1" )"
  _name="$( html_namE "${1##*/}" )"
  _target="$2/${_name}"

  _parsed="$( outln "${_content}" \
    | case "${1##*.}" in
      adoc|asc|asciidoc)  asciidoctor - --out-file '-' --no-header-footer ;;
      md)                 pandoc - ;;
      #md)                 comrak "$1" ;;
      *)                  die 1 FATAL "'$1' Not a supported file type" ;;
    esac
  )" || exit "$?"
  outln "${_parsed}" | Html_post_procesS "${_front}" >"${_target}"
  "${VERBOSE}" && errln "✔ Compiled '$1' -> '${_target}'"
  return 0
}

compile_all_posts_in() {
  # $1 is the directory which to loop over to run `Compile_post` over
  _count="0"
  for _file in $( list_files_iN "$1" | Map_add_prefiX "$1/" ); do
    # The parameter subsitution here is just to make `shellcheck` not complain
    <"${_file}" Compile_post "${_file##*/}" "${POST_BUILD_DIR}"
    _count="$(( _count + 1 ))"
  done
  "${VERBOSE}" && errln "Compiled ${_count} posts" ''
  return 0
}

Build_posts() {
  mkdir -p "$1"

  if [ ! -e "${TEMPLATE_POST}" ]; then
    "${VERBOSE}" && err "❌ Skipping making posts as \${TEMPLATE_POST} is " \
      " set to '${TEMPLATE_POST}', which is invalid${NEWLINE}"
    return 1
  elif [ -z "${POSTSDIR}" ]; then
    "${VERBOSE}" && errln "❌ Skipping making posts as \${POSTDIR} is unset"
    return 1
  else
    _count="0"
    while IFS= read -r _html; do
      {
        preprocesS 'post' "${TEMPLATE_POST}" | Before_contenT
        cat "${_html}"
        preprocesS 'post' "${TEMPLATE_POST}" | After_contenT
        _count="$(( _count + 1 ))"
      } >"$1/${_html##*/}"
      "${VERBOSE}" && errln \
        "✔ Built '$1/${_html##*/}' from '${_html}' and '${TEMPLATE_POST}'"
    done
    "${VERBOSE}" && errln "Built ${_count} posts" ''
    return 0
  fi
}

original_name() {
  ___o="${1%.*}"
  basename "${PUBLISHED}/${___o##*/}"*
}

Build_tagfile() {
  if [ -n "${TAGSFILE}" ]; then
    # Split STDIN into ${_titles} and ${_tags}
    # Not combining since this way no character restriction on ${_title}
    # Cannot pipe to awk due to scope (need both ${_titles} and ${_tags} later)
    _titles=""
    _tags=""
    while IFS= read -r _html; do
      _orig="$( original_name "${_html}" )"
      _fm="$( <"${_html}" Fetch_frontmatteR "${_orig}"  )"
      # Format titles in a way readable by `lookup_valuE`
      _titles="${_titles}${_html}:$( lookup_valuE "${_fm}" 'title' )${NEWLINE}"
      _tags="${_tags}${_html}|$( lookup_valuE "${_fm}" 'tags' )${NEWLINE}"
    done

    outln "${_tags}" | awk -v count=0 -v FS='|' '
      # Now rotate the tags
      !/^ *$/ {
        len = split($2, current_tags, " *");
        for (i = 1; i <= len; ++i) {
          if (!rotated[current_tags[i]]) {
            tags[++count] = current_tags[i];
          }
          rotated[current_tags[i]] = rotated[current_tags[i]] " " $1;
        }
        if (len == 0) {
          if (!rotated["untagged"]) tags[++count] = "#untagged";
          rotated["#untagged"] = rotated["#untagged"] " " $1;
        }
      }
      END {
        for (i = 1; i <= count; ++i) {
          print tags[i] "|" rotated[tags[i]];
        }
      }
    ' | {  # Now processing '<tag>|<file1> <file2_that_has_tag> ...'
      preprocesS 'Tags' "$1/${TAGSFILE}" | After_contenT
      while IFS= read -r _entry; do
        {  # split ${_files} by space
          for _file in ${_entry#*'|'}; do
            outln "${_file}|$( lookup_valuE "${_titles}" "${_file}" )"
          done \
        } | make_tagpage_iteM "${_entry%%'|'*}"
      done
      preprocesS 'Tags' "$1/${TAGSFILE}" | After_contenT
    } >"$2/${TAGSFILE}"
    "${VERBOSE}" && errln "✔ Built tags file '$2/${TAGSFILE}'"
    return 0
  else
    "${VERBOSE}" && errln "❌ Skipping making tags file as \${TAGSFILE} not set"
    return 1
  fi
}

# All posts on one page
Build_archive() {
  # $1: source directory
  # $2: output directory

  if [ -n "${ARCHIVE}" ] && [ -e "$1/${ARCHIVE}" ]; then
    _count="0"
    _dir="$( dirname "$2/${ARCHIVE}"; printf a )"; _dir="${_dir%?a}"
    mkdir -p "${_dir}"
    {
      preprocesS 'Blog' "$1/${ARCHIVE}" | Before_contenT
      while IFS= read -r _html; do
        cat "${_html}"
        outln
        _count="$(( _count + 1 ))"
      done
      preprocesS 'Blog' "$1/${ARCHIVE}" | After_contenT
    } >"$2/${ARCHIVE}"
    "${VERBOSE}" && err "✔ Built archive '$2/${ARCHIVE}' from " \
      "${_count} posts and '$1/${ARCHIVE}' template${NEWLINE}"
    return 0
  else
    "${VERBOSE}" && errln \
      "❌ Skipping making archive page because '\${ARCHIVE}' is invalid"
    return 1
  fi
}

# TODO: Check that `Compile_post` output does not break CDATA, might not have to as
#       the compiling software might sanitise the output for us
Build_rss() {
  # Template is done via a function
  if [ -n "${RSSOUTPUT}" ]; then
    _count="0"
    _dir="$( dirname "${RSSOUTPUT}"; printf a )"; _dir="${_dir%?a}"
    mkdir -p "${_dir}"
    {
      make_rss_feeD | Before_contenT
      while IFS= read -r _html; do
        _count="$(( _count + 1 ))"
        _orig="$( original_name "${_html##*/}" )"
        _front="$( <"${_html}" Fetch_frontmatteR "${_orig}" )"
        <"${_html}" Make_rss_iteM "${_front}"
        outln  # Add a line between <item></item> cause AeThEsTiC
      done
      make_rss_feeD | After_contenT
    } >"${RSSOUTPUT}"
    "${VERBOSE}" && errln \
      "✔ Built the rss feed '${RSSOUTPUT}' from ${_count} posts"
    return 0
  else
    "${VERBOSE}" && errln \
      "❌ Skipping making an RSS file as \${RSSOUTPUT} is not set"
    return 1
  fi
}

################################################################################
# Meta

# Do not need to consider newline names
# Consider changing to no extension though most server software knows to path
# to html files correctly (ie. both '/a.html' and '/a' map to '/a.html')
html_namE() {
  outln "${1%%.*}.html"  # Newline for sed/etc., removed by `$( )` anyway
}

#Add_headeR() {
#  while IFS= read -r ___line; do
#    if [ "${___line}" != "${___line#<!-- HEADER -->}" ]
#      then cat "$1"
#      else outln "${___line}"
#    fi
#  done
#  out "${___line}"
#}

Before_headeR() { sed -e '/^<!-- HEADER -->/,$d'; }
After_headeR() { sed -e '1,/^<!-- HEADER -->/d'; }
Before_contenT() { sed -e '/^<!-- CONTENT -->/,$d'; }
After_contenT() { sed -e '1,/^<!-- CONTENT -->/d'; }

lookup_valuE() {
  ____hash="${NEWLINE}$1"
  ____val="${____hash##*"${NEWLINE}$2":}"
  ____val="${____val%%"${NEWLINE}"*}"
  out "${____val}"
}


# Unfortunately, commited to date being editable within blog post
# `date -d` is not POSIX so have to do this manually
Sort_by_datE() {
  # $1: directory in which STDIN files are contained (for `Fetch_frontmatteR`)
  #for __path in $( cat - ); do
  while IFS= read -r __path; do
    lookup_valuE "$(
      __original="$( original_name "${__path}" )"
      <"$1/${__original}" Fetch_frontmatteR "${__original}"
    )" 'date'
    outln " ${__path}"
  done \
    | awk '  # Convert from RFC 2822 date to seconds since 1968 (not epoch)
      BEGIN {
        month["Jan"] = "01"; days["Jan"] = 31;
        month["Feb"] = "02"; days["Feb"] = 28 + days["Jan"];
        month["Mar"] = "03"; days["Mar"] = 31 + days["Feb"];
        month["Apr"] = "04"; days["Apr"] = 30 + days["Mar"];
        month["May"] = "05"; days["May"] = 31 + days["Apr"];
        month["Jun"] = "06"; days["Jun"] = 30 + days["May"];
        month["Jul"] = "07"; days["Jul"] = 31 + days["Jun"];
        month["Aug"] = "08"; days["Aug"] = 31 + days["Jul"];
        month["Sep"] = "09"; days["Sep"] = 30 + days["Aug"];
        month["Oct"] = "10"; days["Oct"] = 31 + days["Sep"];
        month["Nov"] = "11"; days["Nov"] = 30 + days["Oct"];
        month["Dec"] = "12"; days["Dec"] = 31 + days["Nov"];
      }
      {
        # eg. Thu, 01 Jan 1000 08:36:42 +0150 file-name-no-spaces.md
        split($5, hms, ":");
        split($6, tz, "");
        year = $4 - 1968; Nearly unix epoch

        count = hms[3];
        count += (hms[2] + (tz[4] tz[5])) * 60
        count += (hms[1] + (tz[2] tz[3])) * 3600
        count += $2 * 86400
        if ((year % 4 > 0) && ($3 != "Jan")) {
          count += month[$3] * (days[$3] + 1) * 86400;
        } else {
          count += month[$3] * days[$3] * 86400;
        }
        count += (year * 365 + int(year / 4)) * 86400

        print(count " " $7);
      }
    ' | sort -n -k 1 \
    | sed 's/[0-9]* //'
}

Fetch_frontmatteR() {
  case "${1##*.}" in
    adoc|asc|asciidoc)  sed -ne '/^===* /q;/^:[A-Za-z0-9_]\+: /{ s/^://p }' ;;
    md)                 sed -ne '/^---$/,/^---$/{ /^---/!p }' ;;
    *)  die 1 FATAL "\`Fetch_frontmatteR\` - '$1' is an unsupported format" ;;
  esac \
    | sed -e 's/^\([^:]*:\) */\1/;s/ *$//'  # remove leading/trailing spaces
  # Add this key and value
  outln "html_filename:$( html_namE "${1##*/}" )"  # Used for `Make_rss_iteM`
}

now() {
  # Must conform to standards
  TZ=GMT date '+%a, %d %b %Y %H:%M:%S %z'
}

################################################################################
# IO Helpers

# Filters for sane filenames (no newlines)
list_files_iN() {
  # Assumes there are no subdirectories that have to be included
  for d in "$@"; do
    for f in "$d"/* "$d"/.[!.]* "$d"/..?*; do
      [ -e "$f" ] && [ "$f" = "${f##*"${NEWLINE}"*}" ] && outln "${f##*/}"
    done
  done
}

# Same as `list_files_iN` but without file extensions
list_names_iN() {
  # Assumes there are no subdirectories that have to be included
  for _d in "$@"; do
    for _f in "${_d}"/* "${_d}"/.[!.]* "${_d}"/..?*; do
      [ -e "${_f}" ] && [ "${_f}" = "${_f##*"${NEWLINE}"*}" ] && {
        _f="${_f##*/}"
        outln "${_f%.*}"
      }
    done
  done
}
# In general we are no
# TODO: make  it check for unique filenames without file extension
ask_unique_filename() {
  _name="$( if [ -z "$1" ]
    then
      prompt_test '^[0-9a-z_.\-]\+$' "$(
        outln "Enter the filename with extension (affects url, e.g 'post.md')"
        out "${GREEN}Filename: ${CLEAR}${BLUE}"
      )" "${RED}Invalid filename${CLEAR}"
    else  # Also accept empty lines
      prompt_test '^[0-9a-z_.\-]*$' "$(
        outln "Enter the filename with extension, will also be the url"
        outln "Numbers, Lower-case letters, underscore, dash (0-9a-z_.-)"
        outln "Enter blank to leave name unchanged"
        out "${GREEN}Filename${CLEAR} ('${BLUE}$1${CLEAR}'): ${BLUE}"
      )" "${RED}Invalid filename${CLEAR}"
    fi
  )" || return "$?"

  if [ -n "$1" ] && [ -z "${_name}" ]; then
    out "$1"
  elif list_names_iN "${DRAFTS}" "${PUBLISHED}" | grep -F "${_name%.*}"; then
    pcln
    pcln "The post '${BLUE}${_name%.*}${CLEAR}' ${RED}already exists${CLEAR}"
    pcln "These are the list of posts you have:"
    list_files_iN "${DRAFTS}" "${PUBLISHED}" \
      | sed 's/^/- /' >/dev/tty  # Add aethestic dashes for list
    ask_unique_filename "$1"
  else
    out "${_name}"
  fi
}

# Do not pipe to this to retain namespace
read_from() {
  _first="$1"
  _separator="$2"
  shift 2
  # Cannot pipe here either because of namespaces
  IFS="${_separator}" read -r "$@" <<EOF
${_first}
EOF
}

prompt_test() {
  pc "$2" >/dev/tty; read -r value; pc "${CLEAR}"
  while outln "${value}" | grep -qve "$1"; do
    pcln "$3"
    pc "$2" >/dev/tty; read -r value
    pc "${CLEAR}" >/dev/tty
  done
  out "${value}"
}

pick() {
  [ -z "$1" ] && return 1
  choice="$( prompt_test "$(
      outln "$1" | awk '
        (NR == 1){ printf("^1$"); }
        (NR > 1){ printf("%s", "\\|^" NR "$"); }
      '
    )" "$(
      outln "$1" | awk '{ print "'"${CYAN}"'" NR "'"${CLEAR}"') " $0 }'
      out "Enter your choice: ${CYAN}"
    )" "${RED}Invalid option${CLEAR}"
  )"
  outln "$1" | sed -n "${choice}p"
}


################################################################################
# Semantic Shell Helpers
pc() { printf %b "$@" >/dev/tty; }
pcln() { printf %b\\n "$@" >/dev/tty; }
out() { printf %s "$@"; }
outln() { printf %s\\n "$@"; }
err() { printf %s "$@" >&2; }
errln() { printf %s\\n "$@" >&2; }
eval_escape() { <&0 sed "s/'/'\\\\''/g;1s/^/'/;\$s/\$/'/"; }
die() { c="$1"; errln "$2: '${NAME}' -- $3"; shift 3; errln "$@"; exit "$c"; }


main "$@"
