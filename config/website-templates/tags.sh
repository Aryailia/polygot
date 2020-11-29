#!/usr/bin/env sh

# $1: Path to tags database
# $2: Path to blog posts home
# $3: Main language (sets the title of the entry and tag translations)

# For every available language, print all tags and all posts.
# A single post might appear under more than one tag
# If the post is not available for the language, then one will be chosen
# We are relying on rust API to auto-tag untagged posts with e.g. uncategorised

# We are specifying translations for tags in this file

TAGS_CACHE="${1}"
LINK_CACHE="${2}"
LANG="${3}"

NL='
'

TRANSLATIONS='
jp,Archive,保存
zh,Archive,存檔
jp,Japanese,日本語
zh,Japanese,日語
jp,Junk,ゴミ
zh,Junk,垃圾
jp,Linguistics,言語学
zh,Linguistics,語言學
jp,Programming,言語学
zh,Programming,編程
jp,Sinitic,華語
zh,Sinitic,華語
jp,Terminal,端末
zh,Terminal,終端
jp,Unicode,ユニコード
'
translate() {
  # $1: target language
  # $2: original tag name
  trans="${TRANSLATIONS#*"${NL}${1},${2},"}"
  trans="${trans%%${NL}*}"
  out "${trans:-"${2}"}"
}

out() { printf %s "$@"; }
outln() { printf %s\\n "$@"; }

#run: time ../../make.sh compile-blog

#LANG_LIST="$( <"${TAGS_CACHE}" cut -d ',' -f 4 | sort | uniq )"

<<EOF cat -
<!DOCTYPE html>
<html lang="en">
<head>
  <meta http-equiv="Content-Type" content="text/html; charset=utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">

  <!--<link rel="icon" type="image/x-icon" href="favicon.ico">-->

  <title>Posts by Tags</title>
  <link rel="stylesheet" type="text/css" media="screen" href="${DOMAIN}/style.css">
</head>

<body class="structure-blog">
  <header>
<!-- INSERT: navbar -->
  </header>
  <aside class="left">
    <div>Language</div>
$(
for lang in ${LANG_LIST}; do
  [ "${lang}" = "${LANG}" ] && continue
  outln "   <div><a href=\"${DOMAIN}/${BLOG_RELATIVE}/tags-${lang}.html\">${lang}</a></div>"
done
)
  </aside>
  <aside class="right">
  </aside>
  <main class="tag-list">
EOF

# Sieve through link cache for file matching id and print all associated paths
# and related data for a single <li>
select_and_format() {
  # $1: id of file to search for
  # $2: main language
  # $3: title of main version of file

  rest=''

  while IFS=',' read lc_id lc_lang lc_path _; do
    if [ "${lc_id}" = "${1}" ]; then
      if [ "${lc_lang}" = "${2}" ]; then
        # Line head
        out '<li>'
        out "<span>${time%% *}</span> "
        out "<a href=\"${DOMAIN}/${lc_path}\">${3:-Untitled}</a> [${lc_lang}]"
      else
        rest="${rest} <a href=\"${DOMAIN}/${lc_path}\">[${lc_lang}]</a>"
      fi
    fi
  done

  # Line tail
  #[ -n "${rest}" ] && errln "hello there: ${rest}"
  out "${rest}"
  out '</li>'
}


head() {
  out   "      <h1>"; translate "${LANG}" "${tag}"; outln "</h1>"
  outln "      <ul>"
}

midd() {
  out   "        "
  <"${LINK_CACHE}" select_and_format "${id}" "${lang}" "${title}"
  outln
}
foot() {
  outln "      </ul>"
  outln
}

output_all_for_lang() {
  prev_tag=''

  out   "    <div id=\"${LANG}\">"  # newline handle by inner
  outln
  while IFS=',' read -r tag time id lang title; do
    if [ "${prev_tag}" != "${tag}" ]; then
      [ -n "${prev_tag}" ] && foot
      head ""
    fi
    midd
    prev_tag="${tag}"
  done
  if [ "${prev_tag}" != "${tag}" ]; then
    midd
    foot
  fi
  outln "    </div>"
}

# Awk narrows it down to one file per entry, choosing ${main_lang} if available
<"${TAGS_CACHE}" sort | awk -v FS=',' -v lang="${LANG}" '
  function print_cache() {
    for (i = 1; i <= len; ++i) {
      print cache[i];
    }
  }

  {
    if (tag != $1) {  # If the tag ($1) changed
      print_cache();

      tag = $1;
      len = 0;
      delete seen;
    }

    if (!seen[$3]) {
      cache[++len] = $0;
      seen[$3] = len;
    } else {
      # Replace with preferred language
      if ($4 == lang) {
        cache[seen[$3]] = $0;
      }
      next;
    }
  }
  END { print_cache(); }
' | output_all_for_lang "${LANG}"

<<EOF cat -
  </main>
  <footer>
    sitemap
  </footer>
</body>
</html>
EOF
