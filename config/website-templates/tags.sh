#!/usr/bin/env sh

# $1: Path to tags database
# $2: Path to blog posts home

# For every available language, print all tags and all posts.
# A single post might appear under more than one tag
# If the post is not available for the language, then one will be chosen
# We are relying on rust API to auto-tag untagged posts with e.g. uncategorised

# We are specifying translations for tags in this file

TAGS_CACHE="${1}"
LINK_CACHE="${2}"
NL='
'

TRANSLATIONS='
jp,Junk,ゴミ
zh,Junk,垃圾
jp,Linguistics,言語学
zh,Linguistics,語言學
jp,Sinitic,華語
zh,Sinitic,華語
'
translate() {
  # $1: target language
  # $2: original tag name
  trans="${TRANSLATIONS#*"${NL}${1},${2},"}"
  trans="${trans%%${NL}*}"
  out "${trans}"
}

# NOTE: There are *three* places to update if we change the field order
# @TAGS_CACHE_ORDER
read_cache_line() {
  IFS=',' read -r tag time name lang title
}
# @TAGS_CACHE_ORDER
set_buffer() {
  eval "${1}"=\""\${${1}}${tag},${time},${name},${lang},${title}${NL}"\"
}

# Almost the same as group_by_name
# This reverse the order (newest ${time} will appear first)
partition_by_tag() {
  name_head=''
  name_prev=''
  name_buff=''

  while read_cache_line; do
    if [ -n "${time}" ]; then
      #echo "${tag}${lang}|${name}|${title}"
      tag_prev="${tag_buff}"
      set_buffer tag_buff
      if [ "${tag_head}" != "${tag}" ]; then
        if [ -n "${tag_head}" ]; then
          tag_buff="${tag_buff#"${tag_prev}"}"
          outln "${tag_prev}" | sort -r | "$@" "${tag_head}"
        fi
        tag_head="${tag}"
      fi
    fi
  done
  [ -n "${tag_buff}" ] && outln "${tag_buff}" | "$@" "${tag_head}"
}

partition_by_name() {
  name_head=''
  name_buff=''
  name_prev=''
  start_name_section "$@"
  while read_cache_line; do
    if [ -n "${time}" ]; then
      name_prev="${name_buff}"
      set_buffer name_buff
      if [ "${name_head}" != "${name}" ]; then
        if [ -n "${name_head}" ]; then
          name_buff="${name_buff#"${name_prev}"}"
          outln "${name_prev}" | "$@" "${name_head}"
        fi
        name_head="${name}"
      fi
    fi
  done
  [ -n "${name_buff}" ] && outln "${name_buff}" | "$@" "${name_head}"
  close_name_section "$@"
}

for_each() {
  while IFS=',' read -r lc_name lc_lang lc_link lc_title; do
    "$@"
  done
  [ -n "${lc_name}" ] && "$@"
}

# Attempt to combine 'partition_by_name' 'partition_by_tag'
#iterate() {
#  #echo "<table>"
#  prev=''
#  eval "${1}_head=''"
#  eval "${1}_buff=''"
#  while IFS=',' read -r lang tag time name title; do
#    [ -z "${time}" ] && continue
#
#    eval "${1}_prev"="\"\${${1}_buff}\""
#    eval "${1}_buff"="\"\${${1}_buff}${lang},${tag},${time},${name},${title}${NL}\""
#    if eval "[ \"\${${1}_head}\" != \"\${${1}}\" ]"; then
#      if eval "[ -n \"\${${1}_head}\" ]"; then
#        #outln "${prev}"
#        #tag_buff="${tag_buff#"${tag_prev}"}"
#        #eval "${1}_buff=''"
#        eval "${1}_buff"="\"\${${1}_buff#\"\${${1}_prev}\"}\""
#        outln "${lang_prev}"
#      fi
#      eval "${1}_head=\"\${lang}\""
#    fi
#  done
#  eval "prev=\"\${${1}_buff}\""
#  [ -n "${prev}" ] && outln "${prev}" | "${2}"
#  #eval "[ \"\$head{${1}_buff}\" != \"\${${1}}\" ]" \
#  #  && eval "outln \"\${${1}_buff}\" | \"${2}\""
#}

out() { printf %s "$@"; }
outln() { printf %s\\n "$@"; }


#run: time ../../build.sh build-rust build
# @TAGS_CACHE_ORDER
LANG_LIST="$( <"${TAGS_CACHE}" cut -d ',' -f 4 | sort | uniq )"

<<EOF cat -
<!DOCTYPE html>
<html lang="en">
<head>
  <meta http-equiv="Content-Type" content="text/html; charset=utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">

  <!--<link rel="icon" type="image/x-icon" href="favicon.ico">-->

  <title>Posts by Tags</title>
  <link rel="stylesheet" type="text/css" media="screen" href="<!-- INSERT: domain -->/style.css">
</head>

<body class="structure-blog">
  <header>
<!-- INSERT: navbar -->
  </header>
  <aside class="left">
    <div>Language</div>
$(
for lang in ${LANG_LIST}; do
  outln "   <div><a href=\"#${lang}\">${lang}</a></div>"
done
)
  </aside>
  <aside class="right">
  </aside>
  <main class="tag-list">
EOF

start_name_section() {
  ns_tag_translation="$( translate "${2}" "${3}" )"
  outln "<h2 id="">${ns_tag_translation:-"${3}"}</h2>"
  outln "<ul>"
}

# Checks if the lang "${2}" is available, otherwise return the first
find_lang_or_first() {
  if [ "${1}" = "${lc_name}" ]; then
    lc_first="${lc_first:-"${lc_lang}"}"
    [ "${2}" = "${lc_lang}" ] && lc_first="${2}"
  fi
}
print_title() {
  if [ "${lc_name}" = "${1}" ] && [ "${lc_lang}" = "${2}" ]; then
    out " "
    out "<a href=\"<!-- INSERT: domain -->/${lc_link}\">"
    out "${lc_title:-no title}</a>"
    out " [${lc_lang}]"
  fi
}
print_links() {
  if [ "${lc_name}" = "${1}" ] && [ "${lc_lang}" != "${2}" ]; then
    out " [<a href=\"<!-- INSERT: domain -->${lc_link}\">${lc_lang}</a>]"
  fi
}

print_file_links() {
  # $1: the language
  # $2: the file_name
  # $3: the tag
  read_cache_line
  out "<li>"
  out "<span>${time%% *}</span>"
  # ${lc_first} represents the "${1}" (the lang) or if it is not found
  # for "${name}", then the first available lang
  lc_first=''; <"${LINK_CACHE}" for_each find_lang_or_first "${name}" "${1}"
  <"${LINK_CACHE}" for_each print_title "${name}" "${lc_first}"
  <"${LINK_CACHE}" for_each print_links "${name}" "${lc_first}"
  outln "</li>"
}
close_name_section() {
  outln "</ul>"
}



# use the above functions
for l in ${LANG_LIST}; do
  outln "<h1 id="${l}">${l}</h1>"
  # NOTE: The following sets ${lang}
  <"${TAGS_CACHE}" sort |
    partition_by_tag  \
      partition_by_name \
        print_file_links "${l}"
done


<<EOF cat -
  </main>
  <footer>
    sitemap
  </footer>
</body>
</html>
EOF
