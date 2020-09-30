#!/usr/bin/env sh

#export adoc_sourced=true

adoc_FrontmatteR() {
  # &0: Original markup post to extract from
  <&0 adoc_include_by_tag tags="${1#tags=}" \
    | sed -ne '/^:[A-Za-z0-9_-]\+:/{
      s/ *$//
      s/^://
      s/: */:/
      p
    }'
}

adoc_find_available_langS() {
  <&0 awk '
    BEGIN { count = 0; }
    /^\/\/ tag::[a-z][a-z]\[]/ {
      match($0, /::[a-z][a-z]\[/);
      name = substr($0, RSTART + 2, RLENGTH - 3);
      if (!flag[name]) lang[count += 1] = name;
      flag[name] = 1;
    }
    END {
      for (i = 1; i <= count; ++i) {
        printf("%s", lang[i]);
        if (i > 0) printf(" ");
      }
    }
  ' 
}

adoc_compile() {
  # $1: Path to original markup post
  # $2: Tags to include, use '*' to include all
  # $3: Write path to main-content file hunk
  # $4: Write path to table-of-contents file hunk

  # Probably should use `adoc_include_by_tag` instead of printing an include
  printf %s\\n "include::${1}[tags=${2}]" \
    | asciidoctor - --out-file - \
      --attribute toc --attribute toc-title="" \
      --no-header-footer \
    | adoc_Split "${3}" "${4}"
}

# If ${1} is "tags=", no tags specified, then print everything
adoc_include_by_tag() {
  <&0 awk  --posix -v tag_string="${1#tags=}" '
    BEGIN { len = split(tag_string, tags, ";"); }
    {
      bool = len ? len : 1;  # No tags means print all
      for (i = 1; i <= len; ++i) {
        if ($0 ~ "^// *tag::" tags[i] "\\[]") { flag[tags[i]] = 0; }
        if ($0 ~ "^// *end::" tags[i] "\\[]") { flag[tags[i]] = 1; print $0; }
        bool -= flag[tags[i]];
      }
      if (bool) print $0;  # or if end tag
    }
  '
}

adoc_Split() {
  # &0: Original markup post to extract from
  # $1: Write path to main-content file hunk
  # $2: Write path to table-of-contents file hunk
  _adoc_split_entry=''
  _adoc_split_count=''
  while IFS= read -r _adoc_split_line; do
    if [ "${_adoc_split_line}" != "${_adoc_split_line#<div id=\"toc\"}" ]; then
      _adoc_split_count='1'
      break
    else
      printf %s\\n "${_adoc_split_line}"
    fi
  done >"${1}"

  if [ -n "${_adoc_split_count}" ]; then
    printf %s\\n "${_adoc_split_line}"
    while IFS= read -r _adoc_split_line; do
      # Signal start for counting toc
      _adoc_split_entry="${_adoc_split_line}"
      while [ "${_adoc_split_entry}" != "${_adoc_split_entry#*<div}" ]; do
        _adoc_split_entry="${_adoc_split_line#*<div}"
        _adoc_split_count="$(( _adoc_split_count + 1 ))"
      done

      _adoc_split_entry="${_adoc_split_line}"
      while [ "${_adoc_split_entry}" != "${_adoc_split_entry#*</div}" ]; do
        _adoc_split_entry="${_adoc_split_line#*</div}"
        _adoc_split_count="$(( _adoc_split_count - 1 ))"
      done

      if [ "${_adoc_split_count}" -gt 0 ]
        then printf %s\\n "${_adoc_split_line}"
        else break
      fi
    done
  fi >"${2}"

  while IFS= read -r _adoc_split_line; do
    _adoc_split_count="$(( _adoc_split_count + 1 ))"
    printf %s\\n "${_adoc_split_line}"
  done >>"${1}"
}

#<"${1}" adoc_Split a b

#split() {
#   printf %s "${_adoc_split_line}"
#  printf %s\\n "${_adoc_split_count}"
#  _length="$( awk '
#    /^<div id="toc" .*>$/ { start = 1; }
#    (start && count >= 0) {
#      temp = $0;
#      count = count + gsub(/<div/, "", temp) - gsub(/<\/div *>/, "", temp)
#    }
#    (start && count <= 0) { print NR; exit; }
#  ' )"
#  [ -z "${_length}" ] && { printf %s\\n "No table of contents found"; exit 1; }
#  sed "${_length}q" "${1}"
#  sed "1,${_length}d" "${1}"
#}
