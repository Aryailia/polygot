// This brings the disparate parts together to do the compile pipeline.
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::borrow::Cow;
use std::collections::HashMap;
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::RequiredConfigs;
use crate::{
    custom_errors::ParseError,
    fileapi::{command_run, FileApi},
    frontmatter::{Frontmatter, Value},
    helpers::create_parent_dir,
    post::Post,
    traits::{BoolExt, ResultExt, ShellEscape, VecExt},
};

fn read_file(path: &Path, buffer: &mut String) -> Result<usize, String> {
    fs::File::open(path)
        .and_then(|mut file| file.read_to_string(buffer))
        .map_err(|err| {
            [
                "Cannot read ",
                path.to_string_lossy().escape().as_str(),
                ". ",
                err.to_string().as_str(),
            ]
            .join("")
        })
}

//run: ../make.sh build-rust test
pub fn compile(config: &RequiredConfigs, input_list: &[PathBuf]) {
    // The relative relationship is:
    // - one source text <> one 'post' <> many langs/views
    // - one lang <> one view
    // - one views <> one toc and one body (one post <> lang_num * 2 sections)
    // - one view <> one linked output file

    // C analogy: read the meta source file
    let file_count = input_list.len();
    let mut id_map = HashMap::with_capacity(file_count);
    let mut input_paths = Vec::with_capacity(file_count);
    for x in input_list {
        let path = PathWrapper::wrap(x).or_die(1);
        id_map.insert(path.stem, ());
        input_paths.push_and_check(path);
    }

    // C analogy: "parse" into views and metadata
    let mut text_list = Vec::with_capacity(file_count);
    for path in &input_paths {
        let mut text = String::new();
        read_file(path.path, &mut text).or_die(1);
        text_list.push_and_check(text);
    }
    let (shared_metadata, api_and_comment, post_list, lang_list) =
        analyse_metadata(config, &text_list, &input_paths);

    // C analogy: "compile" each view into even more sections ("obj")
    //            "compiling" done by external command (like Asciidoctor)
    // Though I probably should do the frontmatter validation before
    // writing to files in 'htmlify_into_partials', but decided against it.
    htmlify_into_partials(
        config,
        &input_paths,
        &shared_metadata,
        api_and_comment,
        post_list,
    );
    // We can drop 'text_list' and 'api_and_comment' here

    // C analogy: "link" sections to one html per view (many "executables")
    // We read the frontmatter here, which does some validation on the file
    // but I decided it made the lifetime dependency graph too complicated
    join_partials(config, id_map, &input_paths, &shared_metadata, &lang_list);
}

/******************************************************************************/
// Parse the custom markup

#[derive(Debug)]
struct ViewMetadata {
    path_index: usize,
    view_index: usize,
    frontmatter_string: String,
    lang: std::ops::Range<usize>,
    post_lang_count: usize,
    toc_loc: String,
    doc_loc: String,
}

#[derive(Debug)]
struct UpdateTimes<'log>(HashMap<&'log str, DateTime<Utc>>);
impl<'log> UpdateTimes<'log> {
    fn new(log_str: &'log str) -> Result<Self, ParseError> {
        let mut log = HashMap::with_capacity(log_str.lines().count());
        //eprintln!("{:?}", log_str.lines().collect::<Vec<_>>());
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
            eprintln!("{:?} {:?}", id, timestr);
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
            .unwrap_or(true)
    }
}

// Using this so that we can discard 'api_and_comment' and 'text_list'
// before the linker step
type MetadataCache = Vec<ViewMetadata>;
type ApiAndComment<'path, 'config> = HashMap<&'path str, (FileApi<'config>, String)>;
fn analyse_metadata<'config, 'text, 'path>(
    config: &'config RequiredConfigs,
    text_list: &'text [String],
    input_paths: &[PathWrapper<'path>],
) -> (
    MetadataCache,
    ApiAndComment<'path, 'config>,
    Vec<Post<'text>>,
    Vec<String>,
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
            // @TODO debug assert this
            assert_eq!(lang_str, &lang_list_string[lang_range.clone()]);

            shared_metadata.push_and_check(ViewMetadata {
                path_index: i,
                view_index: j,
                frontmatter_string,
                post_lang_count: post_list[i].lang_list.len(),
                lang: lang_range,
                toc_loc: [config.cache_dir, "/toc/", lang_str, "/", path.stem, ".html"].join(""),
                doc_loc: [config.cache_dir, "/doc/", lang_str, "/", path.stem, ".html"].join(""),
            });

            from += lang_str.len() + ' '.len_utf8();
        }
        lang_list.push_and_check(lang_list_string);
    }

    (shared_metadata, api_and_comment, post_list, lang_list)
}

/******************************************************************************/
// HTMLify the post (i.e. run through asciidoctor, etc.)
// Also splits the table of contents (toc) and the body (doc)
fn htmlify_into_partials(
    config: &RequiredConfigs,
    input_list: &[PathWrapper],
    shared_metadata: &MetadataCache,
    api_and_comment: ApiAndComment,
    post_list: Vec<Post>, // Eat this
) {
    debug_assert_eq!(input_list.len(), post_list.len());

    let mut log_result = String::new();
    let log_str = read_file(Path::new(&config.changelog), &mut log_result)
        .map(|_| log_result.as_str())
        .unwrap_or("");
    let log = UpdateTimes::new(log_str)
        .map_err(|err| err.with_filename(Cow::Borrowed(&config.changelog)))
        .or_die(1);

    // Because we flatten post views, using cursor to
    let mut cursor = 1;
    debug_assert_ne!(cursor, 0);
    let mut buffer = String::new();

    for view_data in shared_metadata {
        let i = view_data.path_index;
        let path = &input_list[i];
        let new_post = cursor != i;
        if new_post {
            cursor = i;
            buffer.clear();
            // @TODO implement non-allocating escape
            buffer.push('"');
            buffer.push_str(path.stem);
            buffer.push('.');
            buffer.push_str(path.extension);
            buffer.push('"');
        }

        if log.check_if_outdated(path) {
            let toc_loc = view_data.toc_loc.as_str();
            let doc_loc = view_data.doc_loc.as_str();
            let (api, _) = api_and_comment.get(path.extension).unwrap();
            let view = &post_list[i].views[view_data.view_index];

            // @TODO: Create directories in building api cache (less work)
            create_parent_dir(toc_loc).or_die(1);
            create_parent_dir(doc_loc).or_die(1);
            api.compile(view.body.as_slice(), toc_loc, doc_loc)
                .or_die(1);

            if config.verbose {
                eprintln!("Compiling {} to", buffer);
                eprintln!("- {}", toc_loc.escape());
                eprintln!("- {}", doc_loc.escape());
            } else {
                //if new_post {
                eprintln!("Compiling {}", buffer);
            }
        } else {
            //"Skipping finished {} (use --force to not skip)",
            //eprint!("Compiling {} to ");
            eprintln!("Skipping compiling {}", buffer);
        }
    }
}

/******************************************************************************/
// For each view, join the disparate sections into the final product
#[derive(Debug)]
struct LinkerViewMetadata<'shared, 'lang_group_list, 'frontmatter_string> {
    id: &'shared str,
    frontmatter_serialised: String,
    tags_cache_line: String,
    lang: &'lang_group_list str,
    relative_output_loc: String,
    title: &'frontmatter_string str,
    other_langs: (&'lang_group_list str, &'lang_group_list str),
}

fn join_partials(
    config: &RequiredConfigs,
    id_map: HashMap<&str, ()>,
    input_list: &[PathWrapper],
    shared_metadata: &MetadataCache,
    lang_group_list: &[String],
) {
    debug_assert_eq!(input_list.len(), lang_group_list.len());

    // Each view must know about its parent's other views to link to them
    // So first render the links into 'view_links'
    let view_count = shared_metadata.len();
    let mut linker_metadata = Vec::with_capacity(view_count);
    for view_data in shared_metadata {
        let i = view_data.path_index;
        let path = &input_list[i];
        let frontmatter = Frontmatter::new(
            view_data.frontmatter_string.as_str(),
            path.created,
            path.updated,
        )
        .map_err(|err| err.with_filename(path.path.to_string_lossy()))
        .or_die(1);
        let lang = &lang_group_list[i][view_data.lang.clone()];

        linker_metadata.push_and_check(LinkerViewMetadata {
            frontmatter_serialised: frontmatter.serialise(),
            tags_cache_line: frontmatter.format_to_tag_cache(path.stem, lang),
            lang,
            relative_output_loc: frontmatter.format(config.output_format, path.stem, lang),
            id: path.stem,
            title: match frontmatter.lookup("title") {
                Some(Value::Utf8(s)) => s,
                _ => "",
            },
            other_langs: exclude(&lang_group_list[i], lang),
        });
    }

    // Must update the cache before linking as linker uses this info
    write_caches(config, id_map, &linker_metadata);

    // Format hello
    // @TODO these should both be sorted
    // so we can do a better
    //tags.cache.lines()
    //    .filter(|line| line.split(',').get(3).unwrap_or("") == )
    //    .chain()

    // Run the linker to join the partials (toc and doc)
    let mut cursor = 1;
    debug_assert!(cursor != 0);
    let mut post_data = &linker_metadata[..];
    for i in 0..view_count {
        let shared = &shared_metadata[i];
        if cursor != shared.path_index {
            cursor = shared.path_index;
            post_data = &linker_metadata[i..i + shared.post_lang_count];
        }
        let my_data = &linker_metadata[i];
        let (target, args) = fmt_linker_args(config, &shared_metadata[i], post_data, my_data);

        let args = {
            let mut borrow: Vec<&str> = Vec::with_capacity(args.len());
            for entry in &args {
                borrow.push_and_check(entry);
            }
            borrow
        };

        create_parent_dir(target.as_str()).or_die(1);
        // @TODO Only link if out of date or final file is missing
        print!(
            "{}",
            command_run(
                Path::new(config.linker),
                (config.domain, config.blog_relative),
                None,
                &args,
            )
            .or_die(1)
        );

        eprintln!("Linking {} {}", my_data.lang, target.escape());
        if config.verbose {
            eprint!("=== Arg 1: Frontmatter ====\n{}", &args[0]);
            eprint!("=== Rest ===\n");
            for line in &args[1..] {
                eprint!("{}\n", line);
            }
            eprint!("\n");
        }
    }
}

fn write_caches(
    config: &RequiredConfigs,
    id_map: HashMap<&str, ()>,
    linker_metadata: &[LinkerViewMetadata],
) {
    debug_assert_eq!(id_map.len(), linker_metadata.len());
    let view_count = linker_metadata.len();

    fn read_and_sieve_in_old<'a>(
        id_map: &HashMap<&str, ()>,
        pathstr: &str,
        old_cache: &'a mut String,
        count: usize,
        id_index: usize,
    ) -> Vec<Cow<'a, str>> {
        let path = Path::new(pathstr);
        if let Err(err) = read_file(path, old_cache) {
            eprintln!("Generating {}...\n  {}", pathstr.escape(), err.to_string());
        }

        let mut cache = Vec::with_capacity(old_cache.lines().count() + count);
        cache.extend(old_cache.lines().filter_map(|line| {
            let id = line.split(',').nth(id_index).unwrap_or("");
            (!id_map.contains_key(id)).to_some(Cow::Borrowed(line))
        }));
        cache
    }

    let mut tags_old = String::new();
    let tags_loc = config.tags_cache.as_str();
    let mut tags = read_and_sieve_in_old(&id_map, tags_loc, &mut tags_old, view_count, 2);
    tags.extend(
        linker_metadata
            .iter()
            .flat_map(|x| x.tags_cache_line.split('\n').map(|x| Cow::Borrowed(x))),
    );
    tags.sort();
    eprintln!("Saving tags cache to {}", tags_loc.escape());
    fs::write(tags_loc, tags.join("\n")).or_die(1);
    //println!("{:#?}\n", tags);

    let mut link_old = String::new();
    let link_loc = config.link_cache.as_str();
    let mut link = read_and_sieve_in_old(&id_map, link_loc, &mut link_old, view_count, 0);
    link.extend(
        linker_metadata
            .iter()
            .map(|x| Cow::Owned([x.id, x.lang, x.relative_output_loc.as_str()].join(","))),
    );
    link.sort();
    eprintln!("Saving link cache to {}", link_loc.escape());
    fs::write(link_loc, link.join("\n")).or_die(1);
    //println!("{:#?}\n", link);
}

macro_rules! build_and_count_capacity {
    (let mut $var:ident, $capacity:ident = $base:expr,
        +
        $($entry:expr,)*
    ) => {
        let $capacity = $base + build_and_count_capacity!(@count $($entry,)*);
        let mut $var = Vec::with_capacity($capacity);
        $($var.push_and_check($entry);)*
    };
    (@count) => { 0 };
    (@count $entry:expr, $($tt:tt)*) => {
        1 + build_and_count_capacity!(@count $($tt)*);
    };
}

// 'link_view_sections()' but for a single view
// Returns the output of the command (probably just ignore Ok() case)
// Mostly separate this for the white space
fn fmt_linker_args<'shared, 'frontmatter_string>(
    config: &RequiredConfigs,
    shared: &'shared ViewMetadata,
    post_data: &[LinkerViewMetadata],
    data: &'frontmatter_string LinkerViewMetadata<'shared, '_, 'frontmatter_string>,
) -> (String, Vec<Cow<'frontmatter_string, str>>) {
    let relative_target = data.relative_output_loc.as_str();
    let local_target = [config.public_dir, "/", relative_target].join("");
    let lang_count = shared.post_lang_count;
    // ALL label is lang_count of 0, we want: min(0, lang_count - 1)
    let other_lang_count = if lang_count > 0 { lang_count - 1 } else { 0 };

    // File metadata

    // Counts the capacity for me and pushes
    build_and_count_capacity! {
        let mut api_keyvals, capacity = other_lang_count,
        +
        // User data (specified within post) pushed as first arg
        Cow::Borrowed(data.frontmatter_serialised.as_str()),

        // Remaining args are the api-calculated metadata
        Cow::Owned(["domain:", config.domain].join("")),
        Cow::Owned(["language:", data.lang].join("")),
        Cow::Owned(["local_templates_dir:", config.templates_dir].join("")),
        Cow::Owned(["local_toc_path:", shared.toc_loc.as_str()].join("")),
        Cow::Owned(["local_doc_path:", shared.doc_loc.as_str()].join("")),
        Cow::Owned(["local_output_path:", local_target.as_str()].join("")),
        Cow::Owned(["relative_output_url:", relative_target].join("")),
        //Cow::Owned(["relative_tags_url:", data.tags_loc.as_str()].join("")),
        Cow::Owned([
            "other_view_langs:",
            data.other_langs.0,
            data.other_langs.1
        ].join("")),
    }

    for i in 0..lang_count {
        if i == shared.view_index {
            continue;
        }
        api_keyvals.push_and_check(Cow::Owned(
            [
                "relative_",
                post_data[i].lang,
                "_view:",
                post_data[i].relative_output_loc.as_str(),
                //relative_target.as_str(),
            ]
            .join(""),
        ));
    }
    assert_eq!(capacity, api_keyvals.len());

    (local_target, api_keyvals)
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
