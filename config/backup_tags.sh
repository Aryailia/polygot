#!/usr/bin/env sh

# $1: Path to tags database
# $2: Path to blog posts home

<<EOF cat -
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">
  <title>Posts by Tags</title>
  <link rel="stylesheet" href="<!-- INSERT: prefix -->/style.css">
</head>

<body class="structure-blog">
  <header>
<!-- INSERT: navbar -->
  </header>
  <aside class="left">
EOF

space="    "
<"${1}" awk -v leading_space="${space}" -v FS=',' '
  { if (!have[$1]) tags[count += 1] = $1; }
  { have[$1] = 1; }
  END {
    for (i = 1; i <= count; ++i) {
      printf("%s", leading_space);
      printf("<div class=\"button\"><a href=\"#%s\">%s</a></div>",
        tags[i], tags[i]);
      print("");
    }
  }
'

<<EOF cat -
  </aside>
  <aside class="right">
EOF


<<EOF cat -
  </aside>
  <main class="tag-list">
EOF


# `read` terminates with error code 1 when no more lines to be read in STDIN
read_input() { IFS=, read -r n_lang n_tag name date title; }

<"${1}" sort | {
  sentinel='true'
  c_lang=''
  n_lang=''
  c_tag=''
  n_tag=''

  # To emulate a do-while loop, have to duplicate-code up front
  # and move the same code to the back
  # This is the block duplicated:
  read_input || sentinel='false'  # If "${1}" is empty, this will be false
  c_tag="${n_tag}"
  c_lang="${n_lang}"

  # The real breaking condition for these while loops are at the end
  while "${sentinel}"; do
    printf     '%s<div class="%s">\n' "${space}" "${c_lang}"
    while "${sentinel}"; do

      printf   '%s  <h2 id="%s" class="entry">%s</h2><ul>\n' \
        "${space}" "${c_tag}" "${c_tag}"
      while "${sentinel}"; do
        printf '%s    <li><span class="date">%s</span>' "${space}" "${date}"
        printf        ' - <a href="%s">%s</a></li>\n' \
          "${2}/${name}.html" "${title}"

        read_input || { sentinel='false'; break; }

        if [ "${c_lang}" != "${n_lang}" ] || [ "${c_tag}" != "${n_tag}" ]; then
          c_tag="${n_tag}"
          c_lang="${n_lang}"
          break
        fi
      done
      printf   '%s  </ul>\n' "${space}"
      [ "${c_lang}" = "${n_lang}" ] || break
    done
    printf     '%s</div>\n' "${space}"
  done
}

<<EOF cat -
  </main>
  <footer>
    sitemap
  </footer>
</body>
</html>
EOF
