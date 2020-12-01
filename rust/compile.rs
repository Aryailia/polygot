// This brings the disparate parts together to do the compile pipeline.
use crate::custom_errors::ParseError;
use chrono::NaiveDateTime;
use chrono::{DateTime, TimeZone, Utc};
use std::borrow::Cow;
use std::collections::HashMap;
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::compare_mtimes;
use super::RequiredConfigs;
use crate::fileapi::{command_run, FileApi};
use crate::frontmatter::{Frontmatter, Value};
use crate::helpers::create_parent_dir;
use crate::post::Post;
use crate::traits::{ResultExt, ShellEscape, VecExt};

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
    let (api, path, post) = parse_text_to_post(config, Path::new(pathstr), text.as_str());

    // Parse some metadata
    let (out_of_date, view_sections_metadata) = parse_view_sections_metadata(config, &path, &post);
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
    )
    .or_die(1);

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
    // Number of link-cache lines
    output.push_and_check(output_targets.len().to_string());
    // The actual link-cache lines
    output.extend(output_targets.iter().map(|x| {
        let target_path = x
            .other_view_link
            .find(':')
            .map(|i| x.other_view_link.split_at(i + ':'.len_utf8()).1)
            .unwrap_or("");
        [path.stem, x.lang, target_path, x.title.as_str()].join(",")
    }));
    println!("{}", output.join("\n"));
    // Tag-cache lines
    println!("{}", tags_cache.join("\n"));
}

fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| {
        [
            "Cannot read ",
            path.to_string_lossy().escape().as_str(),
            ". ",
            err.to_string().as_str(),
        ]
        .join("")
    })
}

//fn analyze_times(cache_dir: &str) {
//    let log_loc = [cache_dir, "/updated.csv"].join("");
//    let mut log = Vec::with_capacity(len);
//    log.extend(log_str.lines().filter(|x| x.is_empty()));
//}

//run: ../make.sh build-rust test
pub fn compile2(
    config: &RequiredConfigs,
    input_list: &[PathBuf],
    _linker_loc: &str,
    _output_template: &str,
    _tags_cache_loc: &str,
    _link_cache_loc: &str,
) {
    let file_count = input_list.len();
    let mut input_paths = Vec::with_capacity(file_count);
    for x in input_list {
        input_paths.push_and_check(PathWrapper::wrap(x).or_die(1));
    }
    //let mtime_db_loc = [config.cache_dir, "/mtimes.csv"].join("");
    let mtime_db_loc = "rust/test.csv".to_string();
    let log_result = read_file(Path::new(mtime_db_loc.as_str()));
    let log_str = log_result.as_ref().map(String::as_str).unwrap_or("");
    let log = UpdateTimes::new(log_str)
        .map_err(|err| err.with_filename(Cow::Borrowed(&mtime_db_loc)))
        .or_die(1);

    //let shared = Vec::with_capacity(file_count * 3);
    let mut text_list = Vec::with_capacity(file_count);
    for path in &input_paths {
        text_list.push_and_check(read_file(path.path).or_die(1));
    }
    let (shared_metadata, post_list, lang_list, api_and_comment) =
        MetadataCache::new(config, &text_list, &input_paths);


    // HTMLify the post (i.e. run through asciidoctor, etc.)
    // Also splits the table of contents (toc) and the body (doc)
    shared_metadata.0.iter()
        .filter(|view_data| log.check_if_outdated(&input_paths[view_data.path_index]))
        .for_each(|view_data|  {
            let i = view_data.path_index;
            let toc_loc = view_data.toc_loc.as_str();
            let doc_loc = view_data.doc_loc.as_str();
            let (api, _) = api_and_comment.get(&input_paths[i].stem).unwrap();
            let view = &post_list[i].views[view_data.view_index];

            // @TODO: Create directories in building api cache (less work)
            create_parent_dir(toc_loc).or_die(1);
            create_parent_dir(doc_loc).or_die(1);
            api.compile(view.body.as_slice(), toc_loc, doc_loc).or_die(1);
            println!("Suc")
        });
    // We can drop 'text_list' and 'api_and_comment' here

    // Join the post
    //link()

    //post.views.iter().enumerate().for_each(|(i, view)| {
    //    let (lang, toc_loc, doc_loc) = &metadata[i];
    //    eprintln!(
    //        "compiling {} {} and {}",
    //        lang,
    //        toc_loc.escape(),
    //        doc_loc.escape()
    //    );
    //    create_parent_dir(toc_loc).or_die(1);
    //    create_parent_dir(doc_loc).or_die(1);
    //    api.compile(view.body.as_slice(), toc_loc, doc_loc)
    //        .or_die(1);
    //});
}

// Using this so that we can discard 'api_and_comment' and 'text_list'
// before the linker step
struct MetadataCache(Vec<ViewMetadata2>);
impl MetadataCache {
    fn new<'config, 'text, 'path>(
        config: &'config RequiredConfigs,
        text_list: &'text [String],
        input_paths: &[PathWrapper<'path>],
    ) -> (Self, Vec<Post<'text>>, Vec<String>,
    HashMap<&'path str, (FileApi<'config>, String)>
) {
        debug_assert_eq!(text_list.len(), input_paths.len());

        let len = input_paths.len();

        // Two-part builder, 'api_and_comment' is shared between both
        // Bulid 'post_list'
        let mut api_and_comment = HashMap::new();
        let mut post_list = Vec::with_capacity(len);
        let mut views_count = 0;
        for i in 0..len {
            let extension = input_paths[i].extension;
            if !api_and_comment.contains_key(extension) {
                let api = FileApi::from_filename(
                    config.api_dir,
                    extension,
                    (config.domain, config.blog_relative),
                )
                .or_die(1);
                let comment = api.comment().or_die(1);
                api_and_comment.insert(extension, (api, comment));
            }
            let (_, comment) = api_and_comment.get(extension).unwrap();
            let post = Post::new(&text_list[i], comment.as_str()).or_die(1);

            views_count += post.views.len();
            post_list.push_and_check(post);
        }

        // Build 'shared_metadata' (frontmatter)
        // This is independent of 'text_list' lifetime
        // Also
        let mut shared_metadata = Vec::with_capacity(views_count);
        let mut lang_list = Vec::with_capacity(len);
        for i in 0..len {
            let path = &input_paths[i];
            let (api, _) = api_and_comment.get(path.extension).unwrap();
            let lang_list_string = post_list[i].lang_list.join(" ");

            let mut from = 0;
            for (j, view) in post_list[i].views.iter().enumerate() {
                let frontmatter_string = api.frontmatter(view.body.as_slice()).or_die(1);
                let lang_str = view.lang.unwrap_or("");
                let lang_range = from..from + lang_str.len();
                debug_assert_eq!(lang_str, &lang_list_string[lang_range.clone()]);

                shared_metadata.push_and_check(ViewMetadata2 {
                    path_index: i,
                    view_index: j,
                    frontmatter_string,
                    lang: lang_range,
                    toc_loc: [config.cache_dir, "/toc/", lang_str, "/", path.stem, ".html"]
                        .join(""),
                    doc_loc: [config.cache_dir, "/doc/", lang_str, "/", path.stem, ".html"]
                        .join(""),
                });

                from += lang_str.len() + ' '.len_utf8();
            }
            lang_list.push_and_check(lang_list_string);
        }

        (Self(shared_metadata), post_list, lang_list, api_and_comment)
    }
}

#[derive(Debug)]
struct ViewMetadata2 {
    path_index: usize,
    view_index: usize,
    frontmatter_string: String,
    lang: std::ops::Range<usize>,
    toc_loc: String,
    doc_loc: String,
}

#[derive(Debug)]
struct CompileMetadata<'config, 'path, 'post> {
    api: FileApi<'config>,
    input: PathWrapper<'path>,
    post: Post<'post>,
}

//#[derive(Debug)]
struct SharedMetadata<'owned> {
    lang: &'owned str,
    toc_loc: String,
    doc_loc: String,
    frontmatter: Frontmatter<'owned>,
}

fn validate() {
    //let path_obj = PathWrapper::wrap(path).or_die(1);
    //let api = FileApi::from_filename(
    //    config.api_dir,
    //    path_obj.extension,
    //    (config.domain, config.blog_relative),
    //)
    //.or_die(1);
    //let comment_marker = api.comment().or_die(1);
    //let post = Post::new(text, comment_marker.as_str())
    //    .map_err(|err| err.with_filename(path.to_string_lossy().to_string()))
    //    .or_die(1);
    //CompileMetdata {
    //}
}

// run: cargo test compile -- --nocapture
#[test]
fn make() {
    let now = Utc::now();
    let str_time = now.timestamp().to_string();
    println!("{:?}", now.timestamp());
    println!("{:?}", str_time);
    println!(
        "{:?}",
        chrono::NaiveDateTime::parse_from_str(str_time.as_str(), "%s")
    );
}

#[derive(Debug)]
struct UpdateTimes<'log>(HashMap<&'log str, DateTime<Utc>>);
impl<'log> UpdateTimes<'log> {
    fn new(log_str: &'log str) -> Result<Self, ParseError> {
        let mut log = HashMap::with_capacity(log_str.lines().count());
        for (i, line) in log_str.lines().enumerate().filter(|(_, l)| !l.is_empty()) {
            // @TODO push_and_check for hash
            let (id, timestr) = line
                .rfind(',')
                .map(|delim_index| line.split_at(delim_index))
                .ok_or_else(|| -> ParseError {
                    (
                        i + 1,
                        line,
                        Cow::Borrowed("Missing a second column (comma-separated)."),
                    )
                        .into()
                })?;
            let timestamp = NaiveDateTime::parse_from_str(&timestr[','.len_utf8()..], "%s")
                .map_err(|err| ParseError::from((i + 1, line, Cow::Owned(err.to_string()))))?;
            log.insert(id, Utc.from_utc_datetime(&timestamp));
        }
        Ok(Self(log))
    }

    fn check_if_outdated(&self, id: &PathWrapper) -> bool {
        self.0
            .get(id.stem)
            .map(|log| &id.updated > log)
            .unwrap_or(false)
    }
}

/******************************************************************************/
// Parse the custom markup
#[inline]
fn parse_text_to_post<'post, 'path, 'config>(
    config: &'config RequiredConfigs,
    path: &'path Path,
    text: &'post str,
) -> (FileApi<'config>, PathWrapper<'path>, Post<'post>) {
    // @TODO check if constructor is needed
    let path_obj = PathWrapper::wrap(path).or_die(1);
    let api = FileApi::from_filename(
        config.api_dir,
        path_obj.extension,
        (config.domain, config.blog_relative),
    )
    .or_die(1);
    let comment_marker = api.comment().or_die(1);
    let post = Post::new(text, comment_marker.as_str())
        .map_err(|err| err.with_filename(path.to_string_lossy()))
        .or_die(1);
    (api, path_obj, post)
}

/******************************************************************************/
// For each view, now that we have what should be source code,
// run the parser/compiler associated with the filetype of the source
// See parse_view_sections_metadata: lang, toc_loc, doc_loc
//
// @TODO: Consider compiling langs into a single owned array
type Section = (String, String, String);

#[inline]
fn parse_view_sections_metadata(
    config: &RequiredConfigs,
    post_path: &PathWrapper,
    post: &Post,
) -> (bool, Vec<Section>) {
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

        out_of_date |= compare_mtimes(post_path.path, Path::new(toc_loc.as_str()))
            || compare_mtimes(post_path.path, Path::new(doc_loc.as_str()));
        (lang.to_string(), toc_loc, doc_loc)
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
#[derive(Debug)]
struct ViewMetadata<'compile> {
    lang: &'compile str,
    other_langs: (&'compile str, &'compile str),
    toc_loc: &'compile str,
    doc_loc: &'compile str,
    output_loc: String,
    //tags_loc: String,
    frontmatter_serialised: String,
}

#[derive(Debug)]
struct ViewLocMetadata<'compile> {
    lang: &'compile str,
    other_view_link: String,
    title: String,
}

#[inline]
fn parse_view_metadata<'post, 'compile>(
    api: &FileApi,
    post_path: &PathWrapper,
    lang_list: &'compile str,
    sections_metadata: &'compile [Section],
    post: &Post<'post>,
    path_format: &str,
) -> Result<
    (
        Vec<ViewMetadata<'compile>>,
        Vec<ViewLocMetadata<'compile>>,
        Vec<String>,
    ),
    String,
> {
    // Pre-generate the metadata for the linker
    // In particular, 'link' for all views is used by every other view
    let len = post.views.len();

    let mut loc_metadata = Vec::with_capacity(len);
    let mut views_metadata = Vec::with_capacity(len);
    let mut tags_cache = Vec::with_capacity(len);
    for (i, view) in post.views.iter().enumerate() {
        let source = api.frontmatter(view.body.as_slice()).or_die(1);
        let frontmatter = Frontmatter::new(source.as_str(), post_path.created, post_path.updated)
            // @TODO frontmatter string instead for context since
            //       frontmatter is extracted.
            //       Or perhaps make frontmatter scripts retain newlines
            //       so that this works properly?
            .map_err(|err| err.with_filename(post_path.path.to_string_lossy()))
            .or_die(1);
        let (lang, toc_loc, doc_loc) = &sections_metadata[i];
        let output_loc = frontmatter.format(path_format, post_path.stem, lang);
        let serialised = frontmatter.serialise();
        //let tags_loc = frontmatter.format(path_format, "tags", lang);
        let link = ["relative_", lang, "_view:", output_loc.as_str()].join("");
        let title = match frontmatter.lookup("title") {
            Some(Value::Utf8(s)) => s.to_string(),
            _ => "".to_string(),
        };

        tags_cache.push_and_check(frontmatter.format_to_tag_cache(post_path.stem, lang));
        loc_metadata.push_and_check(ViewLocMetadata {
            lang,
            other_view_link: link,
            title,
        });
        views_metadata.push_and_check(ViewMetadata {
            lang,
            other_langs: exclude(lang_list, lang),
            toc_loc,
            doc_loc,
            output_loc,
            //tags_loc,
            frontmatter_serialised: serialised,
        });
    }
    Ok((views_metadata, loc_metadata, tags_cache))
}

fn combine_sections_into_views(
    config: &RequiredConfigs,
    sections_metadata: &[Section],
    views_metadata: &[ViewMetadata],
    loc_metadata: &[ViewLocMetadata],
    linker_loc: &str,
) {
    // Linker step (put the ToC, doc, and disparate parts together)
    views_metadata.iter().enumerate().for_each(|(i, data)| {
        let (lang, _, _) = &sections_metadata[i];
        let target = [config.public_dir, "/", data.output_loc.as_str()].join("");
        eprintln!("linking {} {}", lang, target.escape());
        create_parent_dir(target.as_str()).or_die(1);
        let linker_stdout =
            link_view_sections(linker_loc, &config, target.as_str(), loc_metadata, data).or_die(1);
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
    loc_metadata: &[ViewLocMetadata],
    data: &ViewMetadata,
) -> Result<String, String> {
    let relative_output_loc = data.output_loc.as_str();

    let base = [
        ["domain:", config.domain].join(""),
        ["language:", data.lang].join(""),
        ["local_toc_path:", data.toc_loc].join(""),
        ["local_doc_path:", data.doc_loc].join(""),
        ["local_templates_dir:", config.templates_dir].join(""),
        ["local_output_path:", local_output_loc].join(""),
        ["relative_output_url:", relative_output_loc].join(""),
        //["relative_tags_url:", data.tags_loc.as_str()].join(""),
        ["other_view_langs:", data.other_langs.0, data.other_langs.1].join(""),
    ];
    // = base + link_list + 1 - 1 (+ 1 frontmatter, - 1 self link)
    let capacity = base.len() + loc_metadata.len();
    let mut api_keyvals = Vec::with_capacity(capacity);
    api_keyvals.push_and_check(data.frontmatter_serialised.as_str());
    api_keyvals.extend(base.iter().map(|s| s.as_str()));
    api_keyvals.extend(
        loc_metadata
            .iter()
            .filter(|x| x.lang != data.lang)
            .map(|x| x.other_view_link.as_str()),
    );
    debug_assert_eq!(capacity, api_keyvals.len());
    command_run(
        Path::new(linker_command),
        (config.domain, config.blog_relative),
        None,
        &api_keyvals,
    )
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

#[derive(Debug)]
struct PathWrapper<'path> {
    path: &'path Path,
    stem: &'path str,
    extension: &'path str,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
}

impl<'path> PathWrapper<'path> {
    fn wrap(path: &'path Path) -> Result<Self, String> {
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

        let meta = path.metadata().map_err(|err| {
            [
                "Cannot read metadata of ",
                path.to_string_lossy().escape().as_str(),
                ". ",
                err.to_string().as_str(),
            ]
            .join("")
        })?;
        let updated = to_datetime(meta.modified()).map_err(|(my_err, sys_err)| {
            [
                "The file created date of ",
                path.to_string_lossy().escape().as_str(),
                my_err,
                ". ",
                sys_err.as_str(),
            ]
            .join("")
        })?;
        let created = to_datetime(meta.created()).map_err(|(my_err, sys_err)| {
            [
                "The file last updated date metadata of ",
                path.to_string_lossy().escape().as_str(),
                my_err,
                ". ",
                sys_err.as_str(),
            ]
            .join("")
        })?;

        Ok(Self {
            path,
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
