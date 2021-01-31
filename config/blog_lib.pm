package blog_lib;

use strict;
use warnings;

our @EXPORT = qw(tag_translation_hash parse_link_cache parse_tags_cache);

sub tag_translation_hash {
  my %tag_translations = (
    "jp" . "Archive" => "保存",
    "zh" . "Archive" => "存檔",
    "jp" . "Japanese" => "日本語",
    "zh" . "Japanese" => "日語",
    "jp" . "Junk" => "ゴミ",
    "zh" . "Junk" => "垃圾",
    "jp" . "Linguistics" => "言語学",
    "zh" . "Linguistics" => "語言學",
    "jp" . "Programming" => "言語学",
    "zh" . "Programming" => "編程",
    "jp" . "Self-Hosting" => "自己ホスト",
    "zh" . "Self-Hosting" => "自託管",
    "jp" . "Sinitic" => "華語",
    "zh" . "Sinitic" => "華語",
    "jp" . "Terminal" => "端末",
    "zh" . "Terminal" => "終端",
    "jp" . "Unicode" => "ユニコード",
    "zh" . "Unicode" => "Unicode (國際碼)",
  );
  return \%tag_translations;
}

sub parse_link_cache {
  my $link_cache_path = shift;
  open(my $link_handle, "<", $link_cache_path)
    or die "Can't open \"$link_cache_path\"";

  my %links;
  my $row = 0;
  while (<$link_handle>) {
    $row += 1;
    if ($_ =~ /^([^,]*),([^,]*),(.*)$/) {
      $links{$1 . $2} = $3;
    } else {
      die "Link cache has invalid line\n$row: $_\n";
    }
  }
  return %links;
}

sub parse_tags_cache {
  my ($tags_cache_path, $default_lang) = @_;
  open(my $tags_handle, "<", $tags_cache_path)
    or die "Can't open \"$tags_cache_path\"";

  # Sorting guarentees $tags are together and ids per tag are together
  # (because $tag (and $date) and $id are first columns)
  my @lines = sort <$tags_handle>;
  my ($is_first, $prev_tag) = (1, "");
  my (%seen_id, %langs, %ids_in_tag, %info, %seen_id_for_tag, @id_cache);
  my $row = 0;
  my $count = 0;
  foreach my $line (@lines) {
    $row += 1;
    if ($line =~ /^([^,]*),([^,]*),([^,]*),([^,]*),(.*)$/) {
      my ($tag, $date, $id, $lang, $title) = ($1, $2, $3, $4, $5);

      if ($is_first) {
        $prev_tag = $tag;
        $is_first = 0;
      }

      if ($tag ne $prev_tag) {
        # outer sort orders these by date
        my @new_allocation_of_ids = reverse @id_cache;
        $ids_in_tag{$prev_tag} = \@new_allocation_of_ids;
        undef %seen_id_for_tag;
        undef @id_cache;
        $prev_tag = $tag;
      }

      if ($lang eq $default_lang) {
        if (!exists($info{$id . $lang})) { unshift @{ $langs{$id} }, $lang; }
        unshift @id_cache, $id;
        $seen_id{$id} = $date;
      } else {
        if (!exists($info{$id . $lang})) { push @{ $langs{$id} }, $lang; }
        if (!exists $seen_id{$id}) { $seen_id{$id} = $date; }
        if (!exists $seen_id_for_tag{$id}) { push @id_cache, $id; }
      }
      $info{$id . $lang} = [$date, $title];
      $seen_id_for_tag{$id} = 1;
    } else {
      die "Tags cache has invalid line\n$row: $_\n";
    }
  }
  @id_cache = reverse @id_cache;
  $ids_in_tag{$prev_tag} = \@id_cache;


  my @ids = reverse sort { $seen_id{$a} cmp $seen_id{$b} } keys %seen_id;
  return (\@ids, \%langs, \%ids_in_tag, \%info);
}

1;
