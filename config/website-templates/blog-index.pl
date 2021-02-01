#!/usr/bin/env perl

# The desired output will look something like:
#    <ul>
#      <li>$date - $target_lang_default [$alt_lang1] [$alt_lang2] ...</li>
#    </ul>


#run: ../../make.sh build-local
# run: PERL5LIB="$PWD/.." ./% ../../.cache/tags.csv ../../.cache/link.csv "zh"


use strict;
use warnings;
use blog_lib;

my ($tags_cache_path, $link_cache_path, $target_lang) = @ARGV;

my $DOMAIN = exists $ENV{'DOMAIN'} ? $ENV{'DOMAIN'} : '';
my $BLOG_RELATIVE = exists $ENV{'BLOG_RELATIVE'} ? $ENV{'BLOG_RELATIVE'} : '';
my $LANG_LIST = exists $ENV{'LANG_LIST'} ? $ENV{'LANG_LIST'} : '';

#TAGS_CACHE="${1}"
#LINK_CACHE="${2}"
#
#out() { printf %s "$@"; }
#outln() { printf %s\\n "$@"; }
#
print qq(
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
  <aside class="left">
);

my @list = blog_lib::parse_tags_cache($tags_cache_path, $target_lang);
my @ids = @{ $list[0] };
my %langs = %{ $list[1] };
my %ids_in_tag =  %{ $list[2] };
my %info = %{ $list[3] };
foreach my $tag (sort keys %ids_in_tag) {
  my $url = "$DOMAIN/$BLOG_RELATIVE/tags-$target_lang.html";
  print qq(    <div><a href="$url">#$tag</a></div>\n);
}

print qq(
  </aside>

  <aside class="right">
    <div><a href="#top">Back to top</a></div><br />
  </aside>
  <main>
    <ul>
);

my %links = blog_lib::parse_link_cache($link_cache_path);
foreach my $id (@ids) {
  my @chronological_langs = @{ $langs{$id} };
  my $lang = $chronological_langs[0];
  my ($date, $title) = @{ $info{$id . $lang} };
  my $url = "$DOMAIN/$links{$id . $lang}";

  if ($date =~ /^(.{4}-.{2}-.{2}) /) {
    print "      <li>";
    print "<span>$1</span> — <a href=\"$url\">$title</a>";
    foreach my $lang (@chronological_langs[1..$#chronological_langs]) {
      my $url = "$DOMAIN/$links{$id . $lang}";
      print qq( [<a href="${url}">$lang</a>]);
    }
    print "</li>\n";
  } else {
    die "Date formated improperly: $date";
  }
}

print qq)
    </ul>
  </main>
  <footer>
<!-- INSERT: footer -->
  </footer>
</div></body>
</html>
);
