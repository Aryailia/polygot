#!/usr/bin/env sh

# $1: Domain (local vs deployment)
# $2: Relative path from public for the file for which this header is for
# $3: Language (can be blank, probably only using for blog/index.html)

entry() {
  [ "${1}" = "${2}" ] \
    && printf '<span class="current">' \
    || printf '<span>'
  printf '<a href="%s">%s</a></span>' "${4}" "${3}"
}
s='    '
tag="${3:+"#${3}"}"  # Add the anchor if not blank


<<EOF cat -
${s}<nav id="top" class="link-hover-only-underline">
${s}  <span class="sitelogo"><a href="${1}/">Words and Semicolons</a></span><!--
${s}  -->$( entry "${2}" "projects.html" "Projects" "${1}/projects.html" )<!--
${s}  -->$( entry "${2}" "blog.html"     "Blog"     "${1}/blog${tag}" )<!--
${s}  -->$( entry "${2}" "about.html"    "About"    "${1}/about.html" )<!--
${s}--></nav>
EOF
