#!/usr/bin/env sh

# $1: tag list
# $2: lang list
# $3: current language
# $4: blog output dir (plus prefix minus lang)
# $5: tag index filename (minus directory, same directory as me)
# $6: current post file name

spaces='    '
basename="${2%%*/}" basename="${basename#/*}"

for hashtag in ${1}; do
  printf '%s<div class="hashtag"><a href="%s">%s</a></div>\n' \
    "${spaces}" \
    "${5}#${hashtag}" \
    "${hashtag}" \
  # Because tag index ($5) is same directory as I, no need to specify directory
done


for lang in ${2}; do
  [ -z "${3}" ] || [ "${lang}" = "${3}" ] && continue
  printf '%s<div class="languagetag"><a href="%s">%s</a></div>\n' \
    "${spaces}" \
    "${4}/${lang}/${6}" \
    "${lang}" \
  # Because tag index ($5) is same directory as I, no need to specify directory
done

