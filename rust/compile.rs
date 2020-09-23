use chrono::{DateTime, TimeZone, Utc};
use std::{fs, io, path::Path, time::SystemTime};

use super::{Config, RequiredConfigs};
use crate::fileapi::{command_run, FileApi};
use crate::frontmatter::Frontmatter;
use crate::helpers::{check_is_dir, check_is_file, create_parent_dir};
use crate::post::{Post, PostView};
use crate::traits::{ResultExt, VecExt};

//run: ../build.sh
pub fn compile(config: &Config, pathstr: &str, post_formatter: &str, path_format: &str) {
    let (stem, ext, created, modified) = analyse_path(pathstr).or_die(1);
    let text = fs::read_to_string(pathstr)
        .map_err(|err| format!("Cannot read {:?}. {}", pathstr, err))
        .or_die(1);
    let x = RequiredConfigs::unwrap(config);
    // @VOLATILE sync with 'define_config'
    check_is_dir(x.cache_dir, "--cache-dir").or_die(1);
    check_is_file(x.api_dir).or_die(1);

    let api = FileApi::from_filename(x.api_dir, ext).or_die(1);
    let comment_marker = api.comment().or_die(1);

    let post = Post::new(text.as_str(), comment_marker.as_str())
        .map_err(|err| err.with_filename(pathstr))
        .or_die(1);

    let len = post.views.len();
    let mut lang_toc_doc = Vec::with_capacity(len);
    lang_toc_doc.extend(post.views.iter().map(|view| {
        let lang = view.lang.unwrap_or("");
        let toc_loc = [x.cache_dir, "/toc/", lang, "/", stem, ".html"].join("");
        let doc_loc = [x.cache_dir, "/doc/", lang, "/", stem, ".html"].join("");

        // Always recompile/etc if --force

        let out_of_date = config.force || analyse_path(toc_loc.as_str())
            .and_then(|t| analyse_path(doc_loc.as_str()).map(|d| (t.3, d.3)))
            .map(|(toc_modified, doc_modified)| {
                //println!("=== {:?} ===", toc_loc);
                //println!("{:?} {:?} {:?}", modified, toc_modified, doc_modified);
                //println!("{} {}", modified > toc_modified, modified > doc_modified);
                modified > toc_modified || modified > doc_modified
            }).unwrap_or(true); // compile if they do not read file/etc.
        (out_of_date, lang, toc_loc, doc_loc)
    }));
    // Compile step (makes table of contents and document itself)
    post.views.iter().enumerate().for_each(|(i, view)| {
        let (out_of_date, _, toc_loc, doc_loc) = &lang_toc_doc[i];
        if *out_of_date {
            eprintln!("compiling {:?} and {:?}", toc_loc, doc_loc);
            make_parent(toc_loc).or_die(1);
            make_parent(doc_loc).or_die(1);
            api.compile(view.body.as_slice(), toc_loc, doc_loc).or_die(1);
        }
    });

    // Pre-generate the metadata for the linker
    // In particular, 'link' for all views is used by every other view
    let lang_list = post.lang_list.join(" ");
    let mut link_list = Vec::with_capacity(len);
    let mut output_locs = Vec::with_capacity(len);
    output_locs.extend(post.views.iter().enumerate().map(|(i, view)| {
        let source = api.frontmatter(view.body.as_slice()).or_die(1);
        let frontmatter = Frontmatter::new(source.as_str(), created, modified)
            // @TODO frontmatter string instead for context since
            //       frontmatter is extracted.
            //       Or perhaps make frontmatter scripts retain newlines
            //       so that this works properly?
            .map_err(|err| err.with_filename(pathstr))
            .or_die(1);
        let (_, lang, toc_loc, doc_loc) = &lang_toc_doc[i];
        let output_loc = frontmatter.format(path_format, stem, lang);
        let serialised = frontmatter.serialise();
        let tags_loc = frontmatter.format(path_format, "tags", lang);
        let link = ["relative_", lang, "_view:", output_loc.as_str()].join("");

        link_list.push((*lang, link));
        ExtraData {
            lang,
            other_langs: exclude(lang_list.as_str(), lang),
            toc_loc,
            doc_loc,
            output_loc,
            tags_loc,
            frontmatter_serialised: serialised,
        }
    }));

    // Linker step (put the ToC, doc, and disparate parts together)
    output_locs.iter().enumerate().for_each(|(i, data)| {
        println!("###### {:?} ######", data.lang);
        let (out_of_date, _, _, _) = &lang_toc_doc[i];
        if *out_of_date {
            let target = [x.public_dir, "/", data.output_loc.as_str()].join("");
            make_parent(target.as_str()).or_die(1);
            let linker_stdout = link_post(
                post_formatter,
                &x,
                target.as_str(),
                &link_list,
                data,
            );
            eprintln!("{}", linker_stdout);
        }

        //println!("{}", view.body.join(""));
        //println!("{}", frontmatter_string);
        //println!("{}", frontmatter.serialise());
        //println!("{}", api.frontmatter(&view.body).unwrap());
    });
}

fn make_parent(location: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(location).parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Cannot create directory {:?}. {}", parent.display(), err))?;
    }
    Ok(())
}

// Just to make passing arguments easier
struct ExtraData<'a> {
    lang: &'a str,
    other_langs: (&'a str, &'a str),
    toc_loc: &'a str,
    doc_loc: &'a str,
    output_loc: String,
    tags_loc: String,
    frontmatter_serialised: String,
}

#[inline]
// Returns the output of the command (probably just ignore this)
fn link_post(
    post_formatter: &str,
    x: &RequiredConfigs,
    local_output_loc: &str,
    link_list: &[(&str, String)],
    data: &ExtraData,
) -> String {
    let relative_output_loc = data.output_loc.as_str();

    let base = [
        ["domain:", x.domain].join(""),
        ["local_toc_path:", data.toc_loc].join(""),
        ["local_doc_path:", data.doc_loc].join(""),
        ["local_templates_dir:", x.templates_dir].join(""),
        ["local_output_path:", local_output_loc].join(""),
        ["relative_output_url:", relative_output_loc].join(""),
        ["relative_tags_url:", data.tags_loc.as_str()].join(""),
        ["other_view_langs:", data.other_langs.0, data.other_langs.1].join(""),
    ];
    // = base + link_list + 1 - 1 (+ 1 frontmatter, - 1 self link)
    let capacity = base.len() + link_list.len();
    let mut api_keyvals = Vec::with_capacity(capacity);
    api_keyvals.push_and_check(data.frontmatter_serialised.as_str());
    api_keyvals.extend(base.iter().map(|s| s.as_str()));
    api_keyvals.extend(
        link_list
            .iter()
            .filter(|(l, _)| *l != data.lang)
            .map(|(_, other_view_link)| other_view_link.as_str()),
    );
    debug_assert_eq!(capacity, api_keyvals.len());
    command_run(Path::new(post_formatter), None, &api_keyvals).or_die(1)
}

/******************************************************************************
 * Helper functions
 ******************************************************************************/
fn to_datetime(time_result: io::Result<SystemTime>, msg: String) -> Result<DateTime<Utc>, String> {
    let system_time = time_result
        .map_err(|err| format!("{} is not supported on this filesystem. {}", msg, err))?;
    let time = system_time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| format!("{} is before UNIX epoch. {}", msg, err))?;
    let secs = time.as_secs() / 1_000_000_000;
    let nano = time.as_nanos() % 1_000_000_000;
    if secs > i64::MAX as u64 {
        return Err(format!(
            "{} is too big and is not supported by the 'chrono' crate",
            msg
        ));
    }
    //println!("s {:?}", Utc.timestamp(time.as_secs(), 0));
    //println!("ns{:?}", Utc.timestamp(time.as_secs(), time.as_nanos()));

    Ok(Utc.timestamp(secs as i64, nano as u32))
}

fn analyse_path(pathstr: &str) -> Result<(&str, &str, DateTime<Utc>, DateTime<Utc>), String> {
    let path = Path::new(pathstr);
    let stem_os = path.file_stem().ok_or_else(|| {
        format!(
            "The post path {:?} does not is not a path to a file",
            pathstr
        )
    })?;
    let ext_os = path
        .extension()
        .ok_or_else(|| format!("The post {:?} does not have a file extension", pathstr))?;

    let file_stem = stem_os
        .to_str()
        .ok_or_else(|| format!("The stem {:?} in {:?} has invalid UTF8", stem_os, pathstr))?;
    let extension = ext_os.to_str().ok_or_else(|| {
        format!(
            "The extension {:?} in {:?} has invalid UTF8",
            ext_os, pathstr
        )
    })?;

    let metadata = path
        .metadata()
        .map_err(|err| format!("Cannot read metadata of {:?}. {}", pathstr, err))?;
    let modified = to_datetime(
        metadata.modified(),
        format!("The file created date of {:?}", pathstr),
    )?;
    let created = to_datetime(
        metadata.created(),
        format!("The file last modified date metadata of {:?}", pathstr),
    )?;

    Ok((file_stem, extension, created, modified))
}

// Check tests for use case
// Remove an entry of a `vec.join(" ")` preserving the correct space delimiters
fn exclude<'a>(space_delimited_str: &'a str, to_skip: &'a str) -> (&'a str, &'a str) {
    let len = space_delimited_str.len();
    let skip_len = to_skip.len();
    let before_skip = space_delimited_str.find(to_skip).unwrap_or(0);
    let left_close = if len > skip_len && (before_skip + skip_len) == len {
        before_skip - ' '.len_utf8()
    } else {
        before_skip
    };
    let right_start = if skip_len > 0 && before_skip + skip_len != len {
        before_skip + skip_len + ' '.len_utf8()
    } else {
        before_skip + skip_len
    };
    let left = &space_delimited_str[0..left_close];
    let right = &space_delimited_str[right_start..];
    (left, right)
}

/******************************************************************************
 * Tests
 ******************************************************************************/
#[cfg(test)]
mod tests {
    use super::exclude;
    fn merge(tuple: (&str, &str)) -> String {
        let mut merged = String::with_capacity(tuple.0.len() + tuple.1.len());
        merged.push_str(tuple.0);
        merged.push_str(tuple.1);
        merged
    }

    #[test]
    fn exclude_test() {
        assert_eq!(merge(exclude("en", "")), "en");
        assert_eq!(merge(exclude("en", "en")), "");
        assert_eq!(merge(exclude("en jp", "")), "en jp");
        assert_eq!(merge(exclude("en jp", "en")), "jp");
        assert_eq!(merge(exclude("en jp", "jp")), "en");
        assert_eq!(merge(exclude("en jp zh", "en")), "jp zh");
        assert_eq!(merge(exclude("en jp zh", "jp")), "en zh");
        assert_eq!(merge(exclude("en jp zh", "zh")), "en jp");
    }
}
