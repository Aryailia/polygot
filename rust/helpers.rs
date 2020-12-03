use crate::traits::{ShellEscape, VecExt};
use chrono::{DateTime, TimeZone, Utc};
use filetime::FileTime;
use std::{fs, io, path::Path, time::SystemTime};

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
// @FORMAT
pub fn parse_tags_and_push<'a>(
    list: &mut Vec<&'a str>,
    line: &'a str,
    ignore_list: &[&str],
    fail_on_duplicates: bool,
) -> Result<Vec<&'a str>, String> {
    let len = line.split_whitespace().count();
    let mut tags_added = Vec::with_capacity(len);
    list.reserve(len);
    for tag in line.split_whitespace().filter(|t| !ignore_list.contains(t)) {
        if tag.contains(&TAG_BLACKLIST[..]) {
            return Err(format!(
                "{} is an invalid tag. {:?} are the blacklisted characters",
                tag.escape(),
                TAG_BLACKLIST
            ));
        } else if !list.contains(&tag) {
            list.push_and_check(tag);
            tags_added.push_and_check(tag);
        } else if fail_on_duplicates {
            return Err([
                tag.escape().as_str(),
                " was already defined. Cannot have duplicates",
            ]
            .join(""));
        } else {
            // if !fail_on_duplicates
            tags_added.push_and_check(tag);
        }
    }
    Ok(tags_added)
}

pub fn program_name() -> String {
    std::env::current_exe()
        .map(|pathbuf| {
            pathbuf
                .file_name()
                .map(|p| p.to_string_lossy())
                .unwrap_or(std::borrow::Cow::Borrowed(""))
                .to_string()
        })
        .unwrap_or_else(|_| "".to_string())
}

#[derive(Debug)]
pub struct PathReadMetadata<'path> {
    pub path: &'path Path,
    pub stem: &'path str,
    pub extension: &'path str,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

impl<'path> PathReadMetadata<'path> {
    pub fn wrap_with_metadata(
        path: &'path Path,
        metadata_result: &io::Result<fs::Metadata>,
    ) -> Result<Self, String> {
        let stem_os = path.file_stem().ok_or_else(|| {
            [
                "The post path ",
                path.to_string_lossy().escape().as_str(),
                " does not is not a path to a file",
            ]
            .join("")
        })?;
        let ext_os = path.extension().ok_or_else(|| {
            [
                "The post ",
                path.to_string_lossy().escape().as_str(),
                " does not have a file extension",
            ]
            .join("")
        })?;

        let stem = stem_os.to_str().ok_or_else(|| {
            [
                "The stem ",
                stem_os.to_string_lossy().escape().as_str(),
                " in ",
                path.to_string_lossy().escape().as_str(),
                " contains invalid UTF8",
            ]
            .join("")
        })?;
        let extension = ext_os.to_str().ok_or_else(|| {
            [
                "The extension",
                ext_os.to_string_lossy().escape().as_str(),
                " in ",
                path.to_string_lossy().escape().as_str(),
                " contains invalid UTF8",
            ]
            .join("")
        })?;

        let meta = metadata_result.as_ref().map_err(|err| {
            [
                "Cannot read metadata of ",
                path.to_string_lossy().escape().as_str(),
                ". ",
                err.to_string().as_str(),
            ]
            .join("")
        })?;

        let updated = Utc.timestamp(
            FileTime::from_last_modification_time(meta).unix_seconds(),
            0,
        );
        let created = FileTime::from_creation_time(meta)
            .map(|filetime| Utc.timestamp(filetime.unix_seconds(), 0))
            .unwrap_or_else(|| Utc.timestamp(0, 0));
        // 'chrono::Utc.timestamp' errors at i64::MIN
        //.unwrap_or(Utc.timestamp(i64::MIN, 0));

        Ok(Self {
            path,
            stem,
            extension,
            created,
            updated,
        })
    }

    pub fn wrap(path: &'path Path) -> Result<Self, String> {
        Self::wrap_with_metadata(path, &path.metadata())
    }
}

fn to_datetime(
    time_result: io::Result<SystemTime>,
) -> Result<DateTime<Utc>, (&'static str, String)> {
    let system_time =
        time_result.map_err(|err| (" is not supported on this filesystem. ", err.to_string()))?;
    let time = system_time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| (" is before UNIX epoch. ", err.to_string()))?;
    let secs = time.as_nanos() / 1_000_000_000;
    let nano = time.as_nanos() % 1_000_000_000;
    if secs > i64::MAX as u128 {
        return Err((
            " is too big and is not supported by the 'chrono' crate",
            "".to_string(),
        ));
    }
    Ok(Utc.timestamp(secs as i64, nano as u32))
}

pub fn create_parent_dir(location: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(location).parent() {
        fs::create_dir_all(parent).map_err(|err| {
            [
                "Cannot create directory ",
                parent.to_string_lossy().escape().as_str(),
                ".\n",
                err.to_string().as_str(),
            ]
            .join("")
        })?;
    }
    Ok(())
}

//pub fn check_is_file(pathstr: &str) -> Result<(), String> {
//    if !Path::new(pathstr).is_file() {
//        Path::new(pathstr)
//            .metadata()
//            .map_err(|err| {
//                [
//                    pathstr.escape().as_str(),
//                    " is not a valid file.\n",
//                    err.to_string().as_str(),
//                ].join("")
//            })?;
//    }
//    Ok(())
//}
//
//pub fn check_is_dir(pathstr: &str, error_msg: &str) -> Result<(), String> {
//    if !Path::new(pathstr).is_dir() {
//        Path::new(pathstr).metadata().map_err(|err| {
//            [
//                "`",
//                error_msg,
//                " ",
//                pathstr.escape().as_str(),
//                "` is not a valid directory.\n",
//                err.to_string().as_str(),
//            ].join("")
//        })?;
//    }
//    Ok(())
//}
