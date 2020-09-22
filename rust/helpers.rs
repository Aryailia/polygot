use crate::traits::VecExt;

// @TODO test on windows
pub const TAG_BLACKLIST: [char; 4] = [
    '/',  // to not conflict compile output ('path_format')
    '\\', // same as forward slash, but for Windows (maybe unnecessary)
    ',',  // to not conflict with tag cache (csv)
    ':',  // to not conflict with 'frontmatter.serialise()'
];

// This is slightly wasteful with memory
// 'ignore_list' is expected to be small
// returns the problematic tag
pub fn parse_tags_and_push<'a>(
    list: &mut Vec<&'a str>,
    line: &'a str,
    ignore_list: &[&str],
    warn_duplicates: bool,
) -> Result<Vec<&'a str>, String> {
    let len = line.split_whitespace().count();
    let mut tags_added = Vec::with_capacity(len);
    list.reserve(len);
    for tag in line.split_whitespace().filter(|t| !ignore_list.contains(t)) {
        if tag.contains(&TAG_BLACKLIST[..]) {
            return Err(format!(
                "{:?} is an invalid tag. {:?} are the blacklisted characters",
                tag, TAG_BLACKLIST
            ));
        } else if !list.contains(&tag) {
            list.push_and_check(tag);
            tags_added.push_and_check(tag);
        } else if warn_duplicates {
            return Err(format!(
                "{:?} was already defined. Cannot have duplicates",
                tag
            ));
        }
    }
    Ok(tags_added)
}
