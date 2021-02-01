#!/usr/bin/env sh

#run: ../make.sh build-local

outln() { printf %s\\n "$@"; }

<<EOF cat -
<!DOCTYPE html>
<html lang="en">
<head>
  <meta http-equiv="Content-Type" content="text/html; charset=utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">

  <!--<meta name="keywords" content="">-->
  <!--<meta name="description" content="">-->
  <!--<meta name="author" content="">-->
  <title>Projects</title>

  <!--<link rel="icon" type="image/x-icon" href="favicon.ico">-->

  <link rel="stylesheet" href="<!-- INSERT: prefix -->/style.css">
  <!--<script type="text/javascript" src="src/app.js"></script>-->
</head>

<body><div class="structure-only-main">
  <header class="sticky" id="top">
<!-- INSERT: navbar -->
  </header>
  <main>
EOF

project() {
  # &0: labels
  # $1: title
  # $2: description
  # $3: repository

  # A masory layout via CSS Grid
  # Overview: https://css-tricks.com/piecing-together-approaches-for-a-css-masonry-layout/
  # But we have the power of compilation
  outln "<div class=\"card\" style=\"grid-row: span $(( ${#2} / 100 + 1 ));\">"

  outln "<h2 class=\"card-header\">${1}</h2>"
  outln "<div class=\"card-body\">"
  outln "<div>${2}</div>"

  #while IFS='' read -r label; do
  #  outln "<span class=\"alert-primary badge-round\"><strong>${label}</strong></span>"
  #done
  outln "</div>"

  if [ -n "${3}" ]; then
    outln "<div class=\"card-footer\">${3}</div>"
  fi
  outln "</div>"
}

github() {
  printf "<a href=\"%s\" class=\"alert-primary button\">%s GitHub</a>" \
    "${1}" \
    '<svg class="octicon" version="1.1" width="0.8rem" height="0.8rem" viewBox="0 0 16 16" aria-hidden="true" fill="white"><path fill-rule="evenodd" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0 0 16 8c0-4.42-3.58-8-8-8z"></path></svg>' \
  #
}

# Project list
outln "<h1>Projects</h1><div class=\"column-list\">"

# Pin the website to the top
outln "Rust" "POSIX Shell" "Bottom-Up Parser" "Vanilla CSS" "Resolution Responsive" \
  | project \
  "Polygot (this site)" \
  "$( outln \
    "The combination of three parts for building a static-site generator: a (simple) parser that facilitates multi-lingual posts, a build/make script for static-site generation, and HTML/CSS." \
    "Intended as a resource against the JavaScript web, a return to vanilla HTML." \
    "This was also an exercise in system admin, SSH, etc." \
  )" \
  "<div class=\"float-right\">$(
    github "https://github.com/Aryailia/polygot"
  )</div><div>2020</div>" \
#

outln "Rust" "Terminal" | project \
  "Modern Terminal Web Browser" \
  "$( outln \
    "Inspired by <a href=\"https://github.com/servo/servo\">Servo</a> and <a href=\"https://github.com/xi-editor/xi-editor\">Xi</a>, I would like a modern rewrite of w3m/lynx." \
    "Eventually support for the <a href=\"https://github.com/datprotocol/DEPs\">DAT protocol</a> would be interesting." \
    "I would consider using <a href=\"https://github.com/denoland/deno\">Deno</a> as the JavaScript engine, especially since it is written in Rust." \
    "Probably should start this without networking, just work a local file browser." \
  )" \
  "<div class=\"float-right\"></div><div>2021 <span class=\"alert-caution badge-sharp\">TODO</span></div>" \
#

# https://www.bibtex.com/g/bibtex-format/
# https://github.com/raphlinus/pulldown-cmark
outln "Rust" "Parser" | project \
  "Commonmark + Citations" \
  "$( outln \
    "I think I found <a href=\"https://github.com/trivernis/snekdown\">Snekdown</a>." \
    "<br /></br />" \
    "I think the web could do with a lot more source citation." \
    "Markdown facilitates lots of writing and is still one of the best balances between source-text clarity, syntax power, and publishing-format availability." \
    "BibTex on AsciiDoctor slows compilation down by a factor of 20~100x for me." \
    "This will also serve as a research project as my first time using <a href=\"https://github.com/pest-parser/pest\">Pest</a>, a Rust PEG parser." \
  )" \
  "<div class=\"float-right\"></div><div>2021 <span class=\"alert-caution badge-sharp\">TODO</span></div>" \
#
outln "Rust" "Bottom-Up Parser" | project \
  "Stateful Hotkeys" \
  "$( outln \
    "A small language for inputing shortcut keys that mimics." \
    "I made this to ease the transition to Wayland and other window managers." \
    "This is also a research project to explore calculating memory needed for each step by doing two passes of each compilation step." \
    "You can find my shortcuts that are processed by this project <a href=\"\">here</a>." \
  )" \
  "<div class=\"float-right\">$(
    github "https://github.com/Aryailia/stateful-hotkeys"
  )</div><div>2021</div>" \
#
outln "Node.js" "ES5 JavaScript" | project \
  "Discord Selfbot" \
  "$( outln \
    "Various tools for helping me moderator and use Discord." \
    "This was my attempt to gain practical experience with various concepts I had been researching: monoids, proto" \
    "This was also a serious attempt toInvestigating <a href=\"https://github.com/Aryailia/denotational\">my own implementation</a> of <a href=\"https://en.wikipedia.org/wiki/MapReduce\">MapReduce</a> under JavaScript, inspired by lodash's creator John-David's presentation '<a href=\"https://www.youtube.com/watch?v=cD9utLH3QOk\">Lo-Dash an JavaScript Performance Optimizations</a>'." \
    "<br /><br />" \
    "Polling how many acquaintances I had in a server (without the friends list) and how many shared servers a stranger had with me where the most useful features." \
    "I wanted to include automatic furigana before abandoning this project." \
  )" \
  "<div class=\"float-right\">$(
    github "https://github.com/Aryailia/selfbot"
  )</div><div>2017</div>" \
#
outln "</div>"



# Wishlist
outln "<h1>Wishlist</h1><div class=\"column-list\">"
outln "" | project \
  "A Decent Chinese Dictionary" \
  "$( outln \
    "I am intensely unhappy with the state of Chinese dictionaries, especially when one compares it to <a href=\"https://jisho.org\">Jisho</a>." \
    "Monolingual dictionaries are too terse and/or too frequently reuse characters of the lexeme being defined in their definitions." \
    "Many definitions would be eludicated by simply including their 對稱 (e.g. 網購 vs 實體店)." \
    "It would also be nice to be able to a full regular expression searching" \
    "In particular, I think a graph database really suits this use case." \
  )" \
  "" \
#
outln "" | project \
  "A Modern Terminal Emulator" \
  "$( outln \
    "Terminals still remain the fastest way to interface with a computer." \
    "Plan9's ACME had a lot of good ideas for mouse support integration." \
  )" \
  "<div class=\"float-right\">$(
    github "https://github.com/withoutboats/notty/"
  )</div><div>2015–2017 (Without Boats)</div>" \
#
outln "" | project \
  "Swipe IME for Terminals" \
  "$( outln \
    "Swipe/Swype input is currently the fastest way to input for the mobile device." \
    "It would be nice to have an open source solution." \
    "It would also be nice to have it be adaptable even to the use case of terminal input(i.e. Termux or SSH from a phone)." \
  )" \
  "" \
#
outln "" | project \
  "LaTeX 2.0" \
  "$( outln \
    "A modern solution to LaTeX that is performant." \
    "<a href=\"https://github.com/sile-typesetter/sile\">SILE</a> is perhaps the best and is performat, but it does not target HTML output." \
    "Alternatively, an HTML adaptor for SILE would be cool too." \
    "<a href=\"https://github.com/trivernis/snekdown\">Snekdown</a> expresses the desire to offer 'similar features to LaTeX'." \
    "" \
  )" \
  "" \
#
outln "ES5 JavaScript" "Canvas" "Graphics" | project \
  "Canvas Edit" \
  "$( outln \
    "A webapp raster-graphics editor (i.e. Photoshop/Krita) that I started in the early days of the HTML5 and canvas." \
    "Learned curve maths, vfx filter maths, and, inspired by <a href=\"https://greensock.com/\"h>GSAP</a>, I wanted to implement a game-engine loop." \
    "I was also doing proper prototype-based inheritance (the intended form of inheritance in JavaScript)." \
    "A rewrite with Svelte, Elm, or Rust WASM would be interesting." \
    "See also <a href=\"https://github.com/SVG-Edit/svgedit\">SVG-Edit</a>, a vector-graphics editor." \
  )" \
  "<div class=\"float-right\">$(
    github "https://github.com/Aryailia/archive-canvasedit-v0"
  )</div><div>2010 <span class=\"alert-warning badge-sharp\">Abandoned</span></div>" \
#
outln "</div>"


<<EOF cat -
  </main>
  <footer>
<!-- INSERT: footer -->
  </footer>
</body>
</html>

EOF
