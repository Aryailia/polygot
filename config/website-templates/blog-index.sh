#!/usr/bin/env sh

# $1: Path to tags database

TAGS_CACHE="${1}"
LINK_CACHE="${2}"

out() { printf %s "$@"; }
outln() { printf %s\\n "$@"; }

<<EOF cat -
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">

  <!--<link rel="icon" type="image/x-icon" href="favicon.ico">-->

  <title>Blog Index</title>
  <link rel="stylesheet" href="${DOMAIN}/style.css">
  <!--<script type="text/javascript"></script>
  <script type="text/javascript" src="src/app.js"></script>
  -->
</head>

<body><div class="structure-blog">
  <header class="sticky" id="top">
<!-- INSERT: navbar -->

  </header>
  <aside class="left"></aside>

  <aside class="right">
    <div><a href="#top">Back to top</a></div><br />
  </aside>
  <main>
    <ul>
EOF

#run: ../../../make.sh build


format() {
  # same as 'select_and_format'
  if [ "${lc_lang}" = "${2}" ]; then
    # Line head
    out '<li>'
    out "<span>${time%% *}</span> "
    out "<a href=\"${DOMAIN}/${lc_path}\">${3}</a> [${lc_lang}]"
  else
    rest="${rest} <a href=\"${DOMAIN}/${lc_path}\">[${lc_lang}]</a>"
  fi
}

select_and_format() {
  # $1: the id of the link to print
  # $2: the preferred language
  # $3: the title to print
  rest=''
  while IFS=',' read lc_id lc_lang lc_path; do
    if [ "${lc_id}" = "${1}" ]; then
      format "$@"

    fi
  done
  if [ "${lc_id}" = "${1}" ]; then
    format "$@"
  fi

  # Line tail
  out "${rest}"
  out '</li>'
}

<"${TAGS_CACHE}" cut -d ',' -f '2-' | sort -r | awk -v FS=',' -v choice='en' '
  {
    if (seen[$2]) {
      if ($3 == choice) {
        cache[len] = $0;
      }
      next;
    } else {
      seen[$2] = 1;
      cache[++len] = $0;
    }
  }

  END {
    for (i = 1; i <= len; ++i) {
      print cache[i];
    }
  }
' | {
  while IFS=',' read -r time name lang title; do
    out '      '
    <"${LINK_CACHE}" select_and_format "${name}" "${lang}" "${title}"
    outln
  done
  }

<<EOF cat -
    </ul>
  </main>
  <footer>
    sitemap
  </footer>
</div></body>
</html>
EOF
