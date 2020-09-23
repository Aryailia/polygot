use crate::traits::VecExt;
use std::fs;
use std::path::Path;

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

pub fn create_parent_dir(location: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(location).parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Cannot create directory {:?}. {}", parent.display(), err))?;
    }
    Ok(())
}


pub fn check_is_file(pathstr: &str) -> Result<(), String> {
    if !Path::new(pathstr).is_file() {
        Path::new(pathstr)
            .metadata()
            .map_err(|err| format!("'{:?}' is not a valid file. {}", pathstr, err))?;
    }
    Ok(())
}

pub fn check_is_dir(pathstr: &str, error_msg: &str) -> Result<(), String> {
    if !Path::new(pathstr).is_dir() {
        Path::new(pathstr)
            .metadata()
            .map_err(|err| {
                format!(
                    "`{} {:?}` is not a valid directory. {}",
                    error_msg, pathstr, err
                )
            })?;
    }
    Ok(())
}

