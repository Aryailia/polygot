#!/usr/bin/env sh

# $1: Path to tags database
# $2: Path to blog posts home

DEFAULT_LANGUAGE='en'
SPACE="    "


TRANSLATIONS='
BEGIN {
trans["jp Junk"] = "ゴミ";
trans["zh Junk"] = "垃圾";
trans["jp Linguistics"] = "言語学";
trans["zh Linguistics"] = "語言學";
trans["zh Sinitic"] = "華語";
trans["jp Sinitic"] = "華語";
}
'

# Combine disparate language representations (several rows) into one column
#   e.g. ',,,en,' + ',,,jp,' = ',,,en jp,'
# Choose a title (preferred_lang if available, otherwise first title)
combine_posts_by_language() {
  <&0 awk -v FS=',' -v preferred_lang="${1:-"${DEFAULT_LANGUAGE}"}" '
    {
      name = $3;
      if (!flag[name]) file[count += 1] = name;
      flag[name] = 1;

      if (!match(tag[name],  " " $1)) tag[name]  = tag[name]  " " $1;
      date[name] = $2;
      if (!match(lang[name], " " $4)) {
        lang[name] = lang[name] " " $4;
      }

      if (!title[name] || $4 == preferred_lang) {
        title[name] = "";
        for (i = 5; i <= NF; ++i) {
          title[name] = title[name] FS $(i);
        }
        title[name] = substr(title[name], 2);
      }
    }
    END {
      for (i = 1; i <= count; ++i) {
        n = file[i];
        lang[n] = substr(lang[n], 2);
        tag[n] = substr(tag[n], 2);
        tag_count = split(tag[n], by_tag, " ");

        if (tag_count == 0) {
          print(tag[n] "," date[n] "," n ",," title[n]);
        } else {
          for (j = 1; j <= tag_count; ++j) {
            print(by_tag[j] "," date[n] "," n "," lang[n] "," title[n]);
          }
        }
      }
    }
  '
}


sort_by_tag_then_reverse_date() {
  <&0 sort | awk -v FS=',' '{
    sentinel = 1;
    while (sentinel) {
      count = 0;
      while (sentinel) {
        tag = $1;
        list[count += 1] = $0;
        if (getline <= 0) sentinel = 0;
        if ($1 != tag) break;
      }
      for (i = count; i >= 1; --i) {
        print list[i];
      }
    }
  }'
}

to_html() {
  # If no 
  awk -v FS=',' -v leading_space="${SPACE}" \
    -v lang_dir="${1:+"/${1}"}" \
    -v preferred_lang="${1:-"${DEFAULT_LANGUAGE}"}" \
    -v tag=1 \
    -v date=2 \
    -v name=3 \
    -v lang=4 \
    -v title=5 \
    "${TRANSLATIONS}"'
    {
      sentinel = 1;
      while (sentinel) {
        printf         "%s<h2 id=\"%s\">", leading_space, $(tag);
        translation_key = preferred_lang " " $(tag);
        printf         "%s",
          (trans[translation_key]) ? trans[translation_key] : $(tag);
        printf         "</h2>\n"
        printf         "%s<ul>\n", leading_space;

        while (sentinel) {
          curr_tag = $(tag);

          full_title = $(title);
          for (i = title + 1; i <= NF; ++i) full_title = full_title FS $(i);

          printf       "%s  <li>", leading_space;
          printf       "<span class=\"date\">%s</span> - ",
            substr($(date), 1, 10);

          # Figure out "first_lang_dir", either preferred language also
          # represented by "lang_dir" or first language
          # Swap lang_list[1] and it so we do not double print in next section
          count = split($(lang), lang_list, " ");
          found_index = 0;
          if (count > 0) {
            for (i = 1; i <= count; ++i) {
              if (lang_list[i] == preferred_lang) found_index = i;
            }
            # Swap if found
            if (found_index > 0) {
              first_lang_dir         = "/" lang_list[found_index];
              lang_list[found_index] = lang_list[1];
            } else {
              first_lang_dir         = "/" lang_list[1];
            }
          } else {
            first_lang_dir = lang_dir;
          }
          printf       "<a href=\"<!-- INSERT: dir -->%s/%s.html\">%s</a>",
            first_lang_dir, $(name), full_title;

          # Alternative languages
          # Skip the first, already represented in title text hyperlink
          for (i = 2; i <= count; ++i) {
            if (lang_list[i] != preferred_lang) {
              printf " ";
              printf "[<a href=\"<!-- INSERT: dir -->/%s/%s.html\">%s</a>]",
                lang_list[i], $(name), lang_list[i];
            }
          }
          printf "</li>\n";

          if (getline <= 0) sentinel = 0;
          if ($(tag) != curr_tag)  break;
        }
        printf "%s</ul>\n\n", leading_space;
      }
    }
  '
}

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

for l in ${3}; do
  if [ "${l}" != "${2}" ]; then
    printf '%s<a href="<!-- INSERT: dir -->/%s/tags.html"><button>' \
      "${SPACE}" "${l}"
    printf '%s</button></a>\n' "${l}"
  fi
done

<<EOF cat -
  </aside>
  <aside class="right">
EOF


<<EOF cat -
  </aside>
  <main class="tag-list">
EOF



<"${1}" combine_posts_by_language "${2}" \
  | sort_by_tag_then_reverse_date \
  | to_html "${2}"

<<EOF cat -
  </main>
  <footer>
    sitemap
  </footer>
</body>
</html>
EOF

exit
# `read` terminates with error code 1 when no more lines to be read in STDIN
read_input() { IFS=, read -r n_lang n_tag date name title; }

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

      printf '%s  ' "${SPACE}"
      printf '<h2 id="%s" class="entry">%s</h2><ul>\n' "${c_tag}" "${c_tag}"
      while "${sentinel}"; do
        printf '%s    ' "${SPACE}"
        printf '<li><span class="date">%s</span>' "${date}"
        printf ' - '
        printf '<a href="%s">%s</a></li>\n' "${2}/${name}.html" "${title}"

        read_input || { sentinel='false'; break; }

        if [ "${c_lang}" != "${n_lang}" ] || [ "${c_tag}" != "${n_tag}" ]; then
          c_tag="${n_tag}"
          c_lang="${n_lang}"
          break
        fi
      done
      printf   '%s  </ul>\n' "${SPACE}"
      [ "${c_lang}" = "${n_lang}" ] || break
    done
    printf     '%s</div>\n' "${SPACE}"
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
