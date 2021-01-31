#!/usr/bin/env sh

# @TODO convert this to perl script so we can translate the tags

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


# This if else is primarily for development
# Change this to `if true` and comment out the '>' after `<<EOF cat -`
if false; then
post_hash_table="
title:平聲入去四聲是什麼：上古、中古、和現代的聲調演變
author:Aryailia
date-created:Thu, 18 Jun 2020 13:47:50 +0800
date-updated:Thu, 18 Jun 2020 13:47:50 +0800
tags:Sinitic Linguistics
series:Terminal
"
set -- \
  "domain:${HOME}/interim/bl/website/public" \
  "blog_relative:blog" \
  "link_cache:${HOME}/interim/bl/website/.cache/link.csv" \
  "series_cache:${HOME}/interim/bl/website/.cache/series.csv" \
  "language:zh" \
  "local_templates_dir:config/templates" \
  "local_toc_path:.cache/toc/zh/chinese_tones.html" \
  "local_doc_path:.cache/doc/zh/chinese_tones.html" \
  "local_output_path:public/blog/zh/chinese_tones.html" \
  "relative_output_url:blog/zh/chinese_tones.html" \
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

#run: ../../../make.sh build -f
#printf \\n%s "${post_hash_table}" >&2
#printf -- -\ %s\\n "${@}" >&2
#exit 0

# Declare before to get the exits
             domain="$( api_lookuP "domain" "$@" )" || exit 1
      blog_relative="$( api_lookuP "blog_relative" "$@" )" || exit 1
         link_cache="$( api_lookuP "link_cache" "$@" )" || exit 1
       series_cache="$( api_lookuP "series_cache" "$@" )" || exit 1
           language="$( api_lookuP "language" "$@" )" || exit 1
local_templates_dir="$( api_lookuP "local_templates_dir" "$@" )" || exit 1
     local_toc_path="$( api_lookuP "local_toc_path" "$@" )" || exit 1
     local_doc_path="$( api_lookuP "local_doc_path" "$@" )" || exit 1
  local_output_path="$( api_lookuP "local_output_path" "$@" )" || exit 1
relative_output_url="$( api_lookuP "relative_output_url" "$@" )" || exit 1
   other_view_langs="$( api_lookuP "other_view_langs" "$@" )" || exit 1


# @VOLATILE: sync with left aside on changes
# Validate existance
for lang in ${other_view_langs}; do
  api_lookuP "relative_${lang}_view" "$@"
done >/dev/null

       title="$( post_lookuP "title" )" || exit 1
      author="$( post_lookuP "author" )" || exit 1
date_created="$( post_lookuP "date-created" )" || exit 1
date_updated="$( post_lookuP "date-updated" )" || exit 1
        tags="$( post_lookuP "tags" )" || exit 1
 series_list="$( post_lookuP "series" )" || exit 1

#printf %s\\n "$@"; exit

date_created="${date_created#?????}"
date_created="${date_created%??????????????}"
date_updated="${date_updated#?????}"
date_updated="${date_updated%??????????????}"

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
$( "${local_templates_dir}/navbar.sh" \
 "${domain}" "${relative_output_url}" "${language}" )
  </header>
  <aside class="left">
    <div>${author}</div>
    <div>Posted: ${date_created}</div>
$( spaces="    "
  for hashtag in ${tags}; do
    printf '%s<div class="hashtag"><a href="%s">%s</a></div>\n' \
      "${spaces}" \
      "${domain}/${blog_relative}/tags-${language}.html#${hashtag}" \
      "#${hashtag}" \
    # end. Use 'tags.html' instead of '${domain}/${relative_tags_url}'?
  done
)
    <div>
$( spaces="      "
  if [ -n "${other_view_langs}" ]; then
    printf %s\\n "${spaces}<div>"
    printf %s\\n "${spaces}  <div><b>Other Languages:</b></div>"
    for lang in ${other_view_langs}; do
      printf '%s<div class="languagetag"><a href="%s">%s</a></div>\n' \
        "${spaces}  " \
        "${domain}/$( api_lookuP "relative_${lang}_view" "$@" )" \
        "${lang}" \
      # end
    done
    printf %s\\n "${spaces}</div>"
  fi
)
    </div>
    <div>
$( spaces="      "
  if [ -n "${series_list}" ]; then
    printf '%s%s\n' "${spaces}" "<div><b>Series:</b></div>"
    for label in ${series_list}; do
      # @FORMAT series lable, time, id, lang, title
      <"${series_cache}" awk -v FS=',' -v label="${label}" -v lang="${language}" '
        $1 == label {
          if (seen[$3]) {
            if ($4 == lang) {
              cache[len] = $0;
            }
            next;
          } else {
            senn[$3] = 1;
            cache[++len] = $0;
          }
        }

        END {
          for (i = 1; i <= len; ++i) {
            print cache[i];
          }
        }
      ' | sort | {

        printf %s\\n "${spaces}<div><p>${label}</p>"
        printf %s\\n "${spaces}  <ul>"

        while IFS=',' read -r label time id lang title; do
          # @FORMAT
          path="$( awk -v FS=',' -v id="${id}" -v lang="${lang}" '
            $1 == id && $2 == lang {
              printf "%s", $3;
              # In case title has commas, print them
              for (i = 4; i <= NF; ++i) {
                printf ",%s", $(i);
              }
            }
          ' "${link_cache}" )"

          printf %s  "${spaces}     <li>"
          if [ "${path}" = "${relative_output_url}" ]; then
            printf '<span>%s</span>' "${title}"
          else
            printf '<a href="%s">%s</a>' "${domain}/${path}" "${title}"
          fi
          printf %s\\n "</li>"
        done

        printf %s\\n "${spaces}  </ul>"
        printf %s\\n "${spaces}</div>"
      }
    done
  fi
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
