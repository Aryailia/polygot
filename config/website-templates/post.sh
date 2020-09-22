#!/usr/bin/env sh

NEWLINE='
'

exit_error() { printf %s\\n "Key '${1}' not provided" >&2; exit 1; }

dehasH() {
  ___hash="${NEWLINE}${1}"
  [ "${___hash}" != "${___hash##*${NEWLINE}"${2}":}" ]  || exit_error "${2}"
  ___val="${___hash##*${NEWLINE}"${2}":}"
  ___val="${___val%%${NEWLINE}*}"
  printf %s "${___val}"
}

api_lookuP() {
  __key="${1}"
  shift 1
  for __keyval in "$@"; do
    if [ "${__keyval}" != "${__keyval#"${__key}":}" ]; then
      printf %s "${__keyval#"${__key}":}"
      return 0
    fi
  done
  exit_error "${__key}"
}
post_lookuP() { dehasH "${post_hash_table}" "${1}";  }

if false; then
post_hash_table="
author:Aryailia
date-created:Thu, 18 Jun 2020 13:47:50 +0800
date-updated:Thu, 18 Jun 2020 13:47:50 +0800
tags:Sinitic Linguistics
title:平聲入去四聲是什麼：上古、中古、和現代的聲調演變
"
set -- \
  "domain:"~/interim/b/public \
  "local_make_dir:config/make" \
  "local_templates_dir:config/templates" \
  "local_toc_path:.blog/toc/zh/chinese_tones.html" \
  "local_doc_path:.blog/doc/zh/chinese_tones.html" \
  "local_output_path:public/blog/zh/chinese_tones.html" \
  "relative_output_url:blog/zh/chinese_tones.html" \
  "relative_tags_url:blog/zh/tags.html" \
  "other_view_langs:en jp stuff" \
  "relative_en_view:blog/en/chinese_tones.html" \
  "relative_jp_view:blog/jp/chinese_tones.html" \
  "relative_stuff_view:blog/stuff/chinese_tones.html" \
# end
else
  post_hash_table="${1}"
  [ "$#" -gt 1 ] || { printf %s\\n "Not enough arguments" >&2; exit 1; }
  shift 1
fi

NEWLINE='
'

#printf \\n%s "${post_hash_table}" >&2
#printf -- -\ %s\\n "${@}" >&2
#exit 0

# Declare before to get the exits
             domain="$( api_lookuP "domain" "$@" )" || exit 1
local_templates_dir="$( api_lookuP "local_templates_dir" "$@" )" || exit 1
     local_toc_path="$( api_lookuP "local_toc_path" "$@" )" || exit 1
     local_doc_path="$( api_lookuP "local_doc_path" "$@" )" || exit 1
  local_output_path="$( api_lookuP "local_output_path" "$@" )" || exit 1
relative_output_url="$( api_lookuP "relative_output_url" "$@" )" || exit 1
  relative_tags_url="$( api_lookuP "relative_tags_url" "$@" )" || exit 1
   other_view_langs="$( api_lookuP "other_view_langs" "$@" )" || exit 1

# @VOLATILE: sync with left aside on changes
# Validate existance
for lang in ${lang}; do
  api_lookuP "relative_${lang}_view" "$@"
done >/dev/null

      author="$( post_lookuP "author" )" || exit 1
date_created="$( post_lookuP "date-created" )" || exit 1
date_updated="$( post_lookuP "date-updated" )" || exit 1
        tags="$( post_lookuP "tags" )" || exit 1
       title="$( post_lookuP "title" )" || exit 1

<<EOF cat - >"${local_output_path}"
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">
  <title>${title}</title>
  <link rel="stylesheet" href="${domain}/style.css">
  <!--<script type="text/javascript"></script>
  <script type="text/javascript" src="src/app.js"></script>
  -->
</head>

<body><div class="structure-blog">
  <header class="sticky" id="top">
$( "${local_templates_dir}/navbar.sh" "${domain}" "${relative_output_url}" )
  </header>
  <aside class="left">
    <div>${author}</div>
    <div>Posted: ${date_created}</div>
$( spaces="    "
  for hashtag in ${tags}; do
    printf '%s<div class="hashtag"><a href="%s">%s</a></div>\n' \
      "${spaces}" \
      "${domain}/${relative_tags_url}#${hashtag}" \
      "#${hashtag}" \
    # end. Use 'tags.html' instead of '${domain}/${relative_tags_url}'?
  done
)

    <div>
      <b>Other Languages:</b>
$( spaces="      "
  for lang in ${other_view_langs}; do
    printf '%s<div class="languagetag"><a href="%s">%s</a></div>\n' \
      "${spaces}" \
      "${domain}/$( api_lookuP "relative_${lang}_view" "$@" )" \
      "${lang}" \
    # end
  done
)
    </div>
  </aside>
  <aside class="right">
    <div><a href="#top">Back to top</a></div><br />
    <div class="test spoiler default-hide large-screen-default-show" id="toc">
      <input class="toggle" id="toc-toggle" type="checkbox" />
      <label for="toc-toggle">
        <span>Table of Contents</span>
        <span class="display-on-hide indicate-clickable">Hide ^</span>
        <span class="display-on-show indicate-clickable">Show v</span>
      </label>
      <div class="display-on-hide">
$( cat "${local_toc_path}" )
      </div>
    </div>
  </aside>
  <main>
    <h1>${title}</h1>
    <div>Last Updated: ${date_updated}</div>
$( cat "${local_doc_path}" )
  </main>
  <footer>
    sitemap
  </footer>
</div></body>
</html>
EOF
