// This brings the disparate parts together to do the compile pipeline.
use chrono::{DateTime, TimeZone, Utc};
use std::{fs, io, path::Path, time::SystemTime};

use super::compare_mtimes;
use super::RequiredConfigs;
use crate::fileapi::{command_run, FileApi};
use crate::frontmatter::{Value, Frontmatter};
use crate::helpers::create_parent_dir;
use crate::post::Post;
use crate::traits::{ResultExt, ShellEscape, VecExt};

//run: ../build.sh build-rust compile-blog
pub fn compile(config: &RequiredConfigs, pathstr: &str, linker_loc: &str, output_template: &str) {
    // The relative relationship is:
    // - one source text <> one 'post' <> many langs/views
    // - one lang <> one view
    // - one views <> one toc and one body (one post <> lang_num * 2 sections)
    // - one view <> one linked output file

    // C analogy: read the meta source file
    let text = fs::read_to_string(pathstr)
        .map_err(|err| {
            [
                "Cannot read ",
                pathstr.escape().as_str(),
                ". ",
                err.to_string().as_str(),
            ]
            .join("")
        })
        .or_die(1);

    // C analogy: "parse" into views
    let (api, path, post) = parse_text_to_post(config, pathstr, text.as_str());

    // Parse some metadata
    let (out_of_date, view_sections_metadata) =
        parse_view_sections_metadata(config, &path, &post);
    let lang_list = post.lang_list.join(" ");
    // Must verify frontmatter before 'htmlifying_view_sections()'
    // This prints the 
    let (views_metadata, output_targets, tags_cache) = parse_view_metadata(
        &api,
        &path,
        lang_list.as_str(),
        view_sections_metadata.as_slice(),
        &post,
        output_template,
    );

    if out_of_date {
        // C analogy: "compile" each view into even more sections ("obj")
        //            "compiling" done by external command (like Asciidoctor)
        htmlify_view_sections(&api, &post, &view_sections_metadata);

        // C analogy: "link" sections to one html per view (many "executables")
        combine_sections_into_views(
            config,
            view_sections_metadata.as_slice(),
            views_metadata.as_slice(),
            output_targets.as_slice(),
            linker_loc,
        );
    } else {
        eprintln!(
            "Skipping finished {} (use --force to not skip)",
            pathstr.escape()
        );
    }

    let mut output = Vec::with_capacity(output_targets.len() + 1);
    output.push_and_check(output_targets.len().to_string());
    output.extend(output_targets.iter().map(|x| {
        let target_path = x.1.find(':')
            .map(|i| x.1.split_at(i + ':'.len_utf8()).1)
            .unwrap_or("");
        [path.stem, x.0, target_path, x.2.as_str()].join(",")
    }));
    println!("{}", output.join("\n"));
    println!("{}", tags_cache.join("\n"));
}

/******************************************************************************/
// Parse the custom markup
#[inline]
fn parse_text_to_post<'a, 'b>(
    config: &RequiredConfigs,
    pathstr: &'b str,
    text: &'a str,
) -> (FileApi, PathWrapper<'b>, Post<'a>) {
    // @TODO check if constructor is needed
    let path = PathWrapper::wrap(pathstr).or_die(1);
    let api = FileApi::from_filename(config.api_dir, path.extension).or_die(1);
    let comment_marker = api.comment().or_die(1);
    let post = Post::new(text, comment_marker.as_str())
        .map_err(|err| err.with_filename(path.pathstr))
        .or_die(1);
    (api, path, post)
}

/******************************************************************************/
// For each view, now that we have what should be source code,
// run the parser/compiler associated with the filetype of the source
type Section<'a> = (&'a str, String, String);

#[inline]
fn parse_view_sections_metadata<'a>(
    config: &RequiredConfigs,
    post_path: &PathWrapper,
    post: &Post<'a>,
) -> (bool, Vec<Section<'a>>) {
    let mut to_html_metadata = Vec::with_capacity(post.views.len());
    let mut out_of_date = config.force; // Always recompile/etc if --force

    to_html_metadata.extend(post.views.iter().map(|view| {
        let lang = view.lang.unwrap_or("");
        let toc_loc = [
            config.cache_dir,
            "/toc/",
            lang,
            "/",
            post_path.stem,
            ".html",
        ]
        .join("");
        let doc_loc = [
            config.cache_dir,
            "/doc/",
            lang,
            "/",
            post_path.stem,
            ".html",
        ]
        .join("");

        out_of_date |= compare_mtimes(post_path.pathstr, toc_loc.as_str())
            || compare_mtimes(post_path.pathstr, doc_loc.as_str());
        (lang, toc_loc, doc_loc)
    }));
    // Compile step (makes table of contents and document itself)
    (out_of_date, to_html_metadata)
}

#[inline]
fn htmlify_view_sections(api: &FileApi, post: &Post, metadata: &[Section]) {
    post.views.iter().enumerate().for_each(|(i, view)| {
        let (lang, toc_loc, doc_loc) = &metadata[i];
        eprintln!(
            "compiling {} {} and {}",
            lang,
            toc_loc.escape(),
            doc_loc.escape()
        );
        create_parent_dir(toc_loc).or_die(1);
        create_parent_dir(doc_loc).or_die(1);
        api.compile(view.body.as_slice(), toc_loc, doc_loc)
            .or_die(1);
    });
}

/******************************************************************************/
// For each view, join the disparate sections into the final product
struct ViewMetadata<'a, 'b> {
    lang: &'a str,
    other_langs: (&'b str, &'b str),
    toc_loc: &'b str,
    doc_loc: &'b str,
    output_loc: String,
    tags_loc: String,
    frontmatter_serialised: String,
}

#[inline]
fn parse_view_metadata<'a, 'b>(
    api: &FileApi,
    post_path: &PathWrapper,
    lang_list: &'b str,
    sections_metadata: &'b [Section<'a>],
    post: &Post<'a>,
    path_format: &str,
) -> (Vec<ViewMetadata<'a, 'b>>, Vec<(&'a str, String, String)>, Vec<String>) {
    // Pre-generate the metadata for the linker
    // In particular, 'link' for all views is used by every other view
    let len = post.views.len();

    let mut link_list = Vec::with_capacity(len);
    let mut views_metadata = Vec::with_capacity(len);
    let mut tags_cache = Vec::with_capacity(len);
    views_metadata.extend(post.views.iter().enumerate().map(|(i, view)| {
        let source = api.frontmatter(view.body.as_slice()).or_die(1);
        let frontmatter = Frontmatter::new(source.as_str(), post_path.created, post_path.updated)
            // @TODO frontmatter string instead for context since
            //       frontmatter is extracted.
            //       Or perhaps make frontmatter scripts retain newlines
            //       so that this works properly?
            .map_err(|err| err.with_filename(post_path.pathstr))
            .or_die(1);
        let (lang, toc_loc, doc_loc) = &sections_metadata[i];
        let output_loc = frontmatter.format(path_format, post_path.stem, lang);
        let serialised = frontmatter.serialise();
        let tags_loc = frontmatter.format(path_format, "tags", lang);
        let link = ["relative_", lang, "_view:", output_loc.as_str()].join("");
        let title = match frontmatter.lookup("title") {
            Some(Value::Utf8(s)) => s.to_string(),
            _ => "".to_string(),
        };

        tags_cache.push_and_check(
            frontmatter.format_to_tag_cache(post_path.stem, lang)
        );
        link_list.push_and_check((*lang, link, title));
        ViewMetadata {
            lang,
            other_langs: exclude(lang_list, lang),
            toc_loc,
            doc_loc,
            output_loc,
            tags_loc,
            frontmatter_serialised: serialised,
        }
    }));
    (views_metadata, link_list, tags_cache)
}

fn combine_sections_into_views(
    config: &RequiredConfigs,
    sections_metadata: &[Section],
    views_metadata: &[ViewMetadata],
    link_list: &[(&str, String, String)],
    linker_loc: &str,
) {
    // Linker step (put the ToC, doc, and disparate parts together)
    views_metadata.iter().enumerate().for_each(|(i, data)| {
        let (lang, _, _) = &sections_metadata[i];
        let target = [config.public_dir, "/", data.output_loc.as_str()].join("");
        eprintln!("linking {} {}", lang, target.escape());
        create_parent_dir(target.as_str()).or_die(1);
        let linker_stdout =
            link_view_sections(linker_loc, &config, target.as_str(), &link_list, data).or_die(1);
        if !linker_stdout.is_empty() {
            eprintln!("{}", linker_stdout);
        }
    });
}

// 'link_view_sections()' but for a single view
// Returns the output of the command (probably just ignore Ok() case)
#[inline]
fn link_view_sections(
    linker_command: &str,
    config: &RequiredConfigs,
    local_output_loc: &str,
    link_list: &[(&str, String, String)],
    data: &ViewMetadata,
) -> Result<String, String> {
    let relative_output_loc = data.output_loc.as_str();

    let base = [
        ["domain:", config.domain].join(""),
        ["local_toc_path:", data.toc_loc].join(""),
        ["local_doc_path:", data.doc_loc].join(""),
        ["local_templates_dir:", config.templates_dir].join(""),
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
            .filter(|(l, _, _)| *l != data.lang)
            .map(|(_, other_view_link, _)| other_view_link.as_str()),
    );
    debug_assert_eq!(capacity, api_keyvals.len());
    command_run(Path::new(linker_command), None, &api_keyvals)
}

/******************************************************************************
 * Helper functions
 ******************************************************************************/
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

struct PathWrapper<'a> {
    pathstr: &'a str,
    stem: &'a str,
    extension: &'a str,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
}

impl<'a> PathWrapper<'a> {
    fn wrap(pathstr: &'a str) -> Result<Self, String> {
        let path = Path::new(pathstr);
        let stem_os = path.file_stem().ok_or_else(|| {
            [
                "The post path ",
                pathstr.escape().as_str(),
                " does not is not a path to a file",
            ]
            .join("")
        })?;
        let ext_os = path.extension().ok_or_else(|| {
            [
                "The post ",
                pathstr.escape().as_str(),
                " does not have a file extension",
            ]
            .join("")
        })?;

        let stem = stem_os.to_str().ok_or_else(|| {
            [
                "The stem ",
                stem_os.to_string_lossy().escape().as_str(),
                " in ",
                pathstr.escape().as_str(),
                " contains invalid UTF8",
            ]
            .join("")
        })?;
        let extension = ext_os.to_str().ok_or_else(|| {
            [
                "The extension",
                ext_os.to_string_lossy().escape().as_str(),
                " in ",
                pathstr.escape().as_str(),
                " contains invalid UTF8",
            ]
            .join("")
        })?;

        let meta = path.metadata().map_err(|err| {
            [
                "Cannot read metadata of ",
                pathstr.escape().as_str(),
                ". ",
                pathstr,
                err.to_string().as_str(),
            ]
            .join("")
        })?;
        let updated = to_datetime(meta.modified()).map_err(|(my_err, sys_err)| {
            [
                "The file created date of ",
                pathstr.escape().as_str(),
                my_err,
                ". ",
                sys_err.as_str(),
            ]
            .join("")
        })?;
        let created = to_datetime(meta.created()).map_err(|(my_err, sys_err)| {
            [
                "The file last updated date metadata of ",
                pathstr.escape().as_str(),
                my_err,
                ". ",
                sys_err.as_str(),
            ]
            .join("")
        })?;

        Ok(Self {
            pathstr,
            stem,
            extension,
            created,
            updated,
        })
    }
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
