#!/usr/bin/env perl

# The desired output will look something like:
#    <h1>$tag</h1>
#    <ul>
#      <li>$date - $target_lang_default [$alt_lang1] [$alt_lang2] ...</li>
#    </ul>


# run: ../../make.sh build-local
#run: PERL5LIB="$PWD/.." ./% ../../.cache/tags.csv ../../.cache/link.csv "zh"

use strict;
use warnings;
use blog_lib;

my ($tags_cache_path, $link_cache_path, $default_lang) = @ARGV;

my $DOMAIN = exists $ENV{'DOMAIN'} ? $ENV{'DOMAIN'} : '';
my $BLOG_RELATIVE = exists $ENV{'BLOG_RELATIVE'} ? $ENV{'BLOG_RELATIVE'} : '';
my $LANG_LIST = exists $ENV{'LANG_LIST'} ? $ENV{'LANG_LIST'} : '';

my %tag_translations = %{ blog_lib::tag_translation_hash() };

sub print_tags {
}

print qq(<!DOCTYPE html>
<html lang="$default_lang">
<head>
  <meta http-equiv="Content-Type" content="text/html; charset=utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="X-UA-Compatible" content="ie=edge">

  <!--<link rel="icon" type="image/x-icon" href="favicon.ico">-->

  <title>Posts by Tags</title>
  <link rel="stylesheet" type="text/css" media="screen" href="$DOMAIN/style.css">
</head>

<body class="structure-blog">
  <header>
<!-- INSERT: navbar -->
  </header>
  <aside class="left">
    <div>Other Languages</div>
);

# @TODO rename it to OTHER_LANG_LIST
foreach my $lang (split /\n/, $LANG_LIST) {
  if ($lang ne $default_lang) {
    my $url = "$DOMAIN/$BLOG_RELATIVE/tags-$lang.html";
    print qq(    <div><a href="$url">${lang}</a></div>\n);
  }
}

print qq(
  </aside>

  <aside class="right">
  </aside>
  <main class="tag-list">
);

my %links = blog_lib::parse_link_cache($link_cache_path);

my @list = blog_lib::parse_tags_cache($tags_cache_path, $default_lang);
my @ids = @{ $list[0] };
my %langs = %{ $list[1] };
my %ids_for_tag =  %{ $list[2] };
my %info = %{ $list[3] };

foreach my $tag (sort keys %ids_for_tag) {

  my $translated_tag = exists $tag_translations{$default_lang . $tag}
    ? $tag_translations{$default_lang . $tag} : $tag;
  print   "    <h1>$translated_tag</h1><ul>\n";

  foreach my $id (@{ $ids_for_tag{$tag} }) {
    my @prioritised_langs = @{ $langs{$id} };
    my $lang = $prioritised_langs[0];
    my ($date, $title) =  @{ $info{$id . $lang} };
    my $url = "$DOMAIN/$links{$id . $lang}";

    print "      <li>";
    # For the first element, print the full info
    if ($date =~ /^(.{4}-.{2}-.{2}) /) {
      print "<span>$1</span> — <a href=\"$url\">$title</a>";

      # For the rest, just print "[$lang]"
      foreach my $lang (@prioritised_langs[1..$#prioritised_langs]) {
        my ($date, $title) =  @{ $info{$id . $lang} };
        my $url = "$DOMAIN/$links{$id . $lang}";
        print " [<a href=\"$url\">$lang</a>]"
      }
      print "</li>\n"
    } else {
      die "Date formated improperly: $date";
    }

  }
  print   "    </ul>\n";
}

print "
  </main>
  <footer>
<!-- INSERT: footer -->
  </footer>
</body>
</html>
";

