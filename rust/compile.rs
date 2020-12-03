// This brings the disparate parts together to do the compile pipeline.
//
// The relative relationship is:
// - one source text <> one 'post' <> many langs/views
// - one lang <> one view
// - one views <> one toc and one body (one post <> lang_num * 2 sections)
// - one view <> one linked output file
//
// - one view <> shared view metadata
// - one view <> linker view metadata

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::{borrow::Cow, collections::HashMap, fs, io::Read, path::Path};

use super::RequiredConfigs;
use crate::{
    custom_errors::ParseError,
    fileapi::{command_run, FileApi},
    frontmatter::{Frontmatter, Value},
    helpers::{create_parent_dir, PathReadMetadata},
    post::Post,
    traits::{BoolExt, ResultExt, ShellEscape, VecExt},
};

macro_rules! zip {
    ($first:ident, $second:ident) => {
        $first.iter().zip($second.iter())
    };
}

// @TODO remove println/eprintln replacing with writes to stdout/stderr
// @TODO check if we can get away with just using Utc::now() for updating
//       changelog; that we do not need to read output file updated time
// @TODO Add spacing between different compile steps, make a print vec function
// @TODO support for light and dark modes
// @TODO cli subcommands for running linker and compile step individually
// @TODO cli subcommand for verify valid url links
// @TODO validate url for output_format, post ids
// @TODO add validation that series labels do not have invalid characters
// @TODO add default language
// @TODO figure out api for labeling series

//run: ../../make.sh build-rust test
// run: cargo test compile -- --nocapture

type Shared<'config, 'path_list, 'input_path, 'shared> = (
    // Borrow RequiredConfig happens at its creation so 'config works for both
    &'config RequiredConfigs<'config>,
    &'path_list [PathReadMetadata<'input_path>],
    &'shared [ViewMetadata],
);

pub fn compile(config: &RequiredConfigs, input_list: &[PathReadMetadata]) {
    // Read the 'input_list' into 'changelog' and 'text_list'
    let mut log_owner = String::new();
    let mut changelog = {
        let log_str = read_file(Path::new(&config.changelog), &mut log_owner)
            .map(|_| log_owner.as_str())
            .unwrap_or("");
        UpdateTimes::new(log_str)
            .map_err(|err| err.with_filename(Cow::Borrowed(&config.changelog)))
            .or_die(1)
    };
    let text_list = {
        let mut text_list = Vec::with_capacity(input_list.len());
        for path in input_list {
            let mut text = String::new();
            read_file(path.path, &mut text).or_die(1);
            text_list.push_and_check(text);
        }
        text_list
    };

    // Parse into Post
    // 'text_list', 'shared_metadata', 'lang_list', 'log_owner' are owned data;
    // the rest are one-time use or borrow from these sources
    let (shared_metadata, lang_list, api_and_comment, post_list) =
        analyse_metadata(config, &text_list, &changelog, input_list);
    let shared = (config, input_list, shared_metadata.as_slice());

    // Run the markup compiler
    htmlify_into_partials(shared, &mut changelog, api_and_comment, post_list);
    // We can drop 'text_list', 'post_list', and 'api_and_comment' here

    // Parse and verify the frontmatter
    let linker_view_metadata = linker_metadata_list_new(shared, &lang_list);

    // Must update the cache before linking as linker uses this info
    write_caches(shared, &changelog, &linker_view_metadata, UPDATE);

    // Link/Join the partials into the final output
    join_partials(shared, &changelog, &linker_view_metadata);
}

pub fn delete(config: &RequiredConfigs, input_list: &[PathReadMetadata]) {
    let mut log_owner = String::new();
    let mut changelog = {
        let log_str = read_file(Path::new(&config.changelog), &mut log_owner)
            .map(|_| log_owner.as_str())
            .unwrap_or("");
        UpdateTimes::new(log_str)
            .map_err(|err| err.with_filename(Cow::Borrowed(&config.changelog)))
            .or_die(1)
    };
    let text_list = {
        let mut text_list = Vec::with_capacity(input_list.len());
        for path in input_list {
            let mut text = String::new();
            read_file(path.path, &mut text).or_die(1);
            text_list.push_and_check(text);
        }
        text_list
    };

    let (shared_metadata, lang_list, _, _) =
        analyse_metadata(config, &text_list, &changelog, input_list);
    let shared = (config, input_list, shared_metadata.as_slice());
    let linker_metadata = linker_metadata_list_new(shared, &lang_list);


    // --- From here it is meaningfully different from `compile()` ----
    // Delete toc, doc, and target
    for view_data in &shared_metadata {
        let toc_loc = view_data.toc_loc.as_str();
        let doc_loc = view_data.doc_loc.as_str();
        match delete_file(toc_loc) {
            Ok(_) => eprintln!("Deleted {}", toc_loc.escape()),
            err => err.or_eprint(()),
        }
        match delete_file(doc_loc) {
            Ok(_) => eprintln!("Deleted {}", doc_loc.escape()),
            err => err.or_eprint(()),
        }
    }
    for view_data in &linker_metadata {
        let target = output_target(config, view_data);

        // These are public facing, so fail eagerly (without --force set)
        match delete_file(target.as_str()) {
            Ok(_) => eprintln!("Deleted {}", target.escape()),
            err if !config.force => err.or_die(1),
            err => err.or_eprint(()),
        }
    }

    // Only update the logs once the public facing pages are deleted
    for path in input_list {
        changelog.remove(path);
    }
    write_caches(shared, &changelog, &linker_metadata, DELETE);
}

/******************************************************************************/
// Parse the custom markup and metadata

// Specifically contains only owned data
#[derive(Debug)]
struct ViewMetadata {
    view_index: usize,
    is_outdated: bool,
    frontmatter_string: String,
    lang: std::ops::Range<usize>,
    post_lang_count: usize,
    toc_loc: String,
    doc_loc: String,
}
type ApiAndComment<'input_path> = HashMap<&'input_path str, (FileApi, String)>;

// Using this so that we can discard 'api_and_comment' and 'text_list'
struct ViewMetadataWalker<'a> {
    index: usize,
    file_index: usize,
    start: usize,
    close: usize,
    iter: std::slice::Iter<'a, ViewMetadata>,
}
fn walk(shared_metadata: &[ViewMetadata]) -> ViewMetadataWalker {
    ViewMetadataWalker {
        index: 0,
        file_index: 0,
        start: 0,
        close: 0,
        iter: shared_metadata.iter(),
    }
}

impl<'a> Iterator for ViewMetadataWalker<'a> {
    // index, if new post subarray, post subarray, view metadata
    type Item = (usize, usize, bool, std::ops::Range<usize>, &'a ViewMetadata);
    fn next(&mut self) -> Option<Self::Item> {
        let metadata = self.iter.next()?;
        let index = self.index;
        self.index += 1;

        // Always runs first time
        let is_cross_into_new_post = index >= self.close;
        if is_cross_into_new_post {
            self.start = index;
            self.file_index += 1;
            self.close = index + metadata.post_lang_count;
        }

        Some((
            index,
            self.file_index - 1,
            is_cross_into_new_post,
            self.start..self.close,
            metadata,
        ))
    }
}

fn analyse_metadata<'config, 'text, 'input_path>(
    config: &'config RequiredConfigs,
    text_list: &'text [String],
    changelog: &UpdateTimes,
    input_paths: &[PathReadMetadata<'input_path>],
) -> (
    Vec<ViewMetadata>,
    Vec<String>,
    ApiAndComment<'input_path>,
    Vec<Post<'text>>,
) {
    debug_assert_eq!(text_list.len(), input_paths.len());

    let len = input_paths.len();

    // Two-part builder, 'api_and_comment' is shared between both
    // Bulid 'post_list'
    let mut api_and_comment = HashMap::new();
    let mut post_list = Vec::with_capacity(len);
    let mut views_count = 0;
    for (path, text) in zip!(input_paths, text_list) {
        let extension = path.extension;
        if !api_and_comment.contains_key(extension) {
            let api = FileApi::from_filename(config.api_dir, extension).or_die(1);
            let comment = api.comment().or_die(1);
            api_and_comment.insert(extension, (api, comment));
        }
        let (_, comment) = api_and_comment.get(extension).unwrap();
        let post = Post::new(text, comment.as_str())
            .map_err(|err| err.with_filename(path.path.to_string_lossy()))
            .or_die(1);

        views_count += post.views.len();
        post_list.push_and_check(post);
    }

    // Build 'shared_metadata' (referenes frontmatter)
    // This is independent of 'text_list' lifetime
    let mut shared_metadata = Vec::with_capacity(views_count);
    let mut lang_list = Vec::with_capacity(len);
    for (path, post) in zip!(input_paths, post_list) {
        let (api, _) = api_and_comment.get(path.extension).unwrap();
        let lang_list_string = post.lang_list.join(" ");

        let mut from = 0;
        for (j, view) in post.views.iter().enumerate() {
            let frontmatter_string = api.frontmatter(view.body.as_slice()).or_die(1);
            let lang_str = view.lang.unwrap_or("");
            let lang_range = from..from + lang_str.len();
            debug_assert_eq!(lang_str, &lang_list_string[lang_range.clone()]);

            shared_metadata.push_and_check(ViewMetadata {
                view_index: j,
                is_outdated: changelog.check_if_outdated(path),
                frontmatter_string,
                post_lang_count: post.lang_list.len(),
                lang: lang_range,
                toc_loc: [config.cache_dir, "/toc/", lang_str, "/", path.stem, ".html"].join(""),
                doc_loc: [config.cache_dir, "/doc/", lang_str, "/", path.stem, ".html"].join(""),
            });

            from += lang_str.len() + ' '.len_utf8();
        }
        lang_list.push_and_check(lang_list_string);
    }

    (shared_metadata, lang_list, api_and_comment, post_list)
}

/******************************************************************************/
// Compile step
// HTMLify the post (i.e. run through asciidoctor, etc.)
// Also splits the table of contents (toc) and the body (doc)

fn htmlify_into_partials<'input_path, 'log>(
    (config, input_list, shared_metadata): Shared<'_, '_, 'input_path, '_>,
    changelog: &mut UpdateTimes<'log>,
    api_and_comment: ApiAndComment,
    post_list: Vec<Post>, // Eat this
) where
    'input_path: 'log,
{
    debug_assert_eq!(input_list.len(), post_list.len());

    // Because we flatten post views, using cursor to
    let mut buffer = String::new();
    for (_, j, is_new_post, _, view_data) in walk(shared_metadata) {
        let path = &input_list[j];
        if is_new_post {
            [path.stem, ".", path.extension]
                .join("")
                .escape_to(&mut buffer);
        }

        let toc_loc = view_data.toc_loc.as_str();
        let doc_loc = view_data.doc_loc.as_str();

        if config.force
            || view_data.is_outdated
            || !Path::new(toc_loc).exists()
            || !Path::new(doc_loc).exists()
        {
            let (api, _) = api_and_comment.get(path.extension).unwrap();
            let view = &post_list[j].views[view_data.view_index];

            // @TODO: Create directories in building api cache (less work)
            create_parent_dir(toc_loc).or_die(1);
            create_parent_dir(doc_loc).or_die(1);
            api.compile(view.body.as_slice(), config.domain, toc_loc, doc_loc)
                .or_die(1);

            changelog.update(path);

            if config.verbose {
                eprintln!("Compiling {} to", buffer);
                eprintln!("- {}", toc_loc.escape());
                eprintln!("- {}", doc_loc.escape());
            } else if is_new_post {
                eprintln!("Compiling {}", buffer);
            }
        } else if is_new_post {
            eprintln!("Skipping compile of {} (use --force to not skip)", buffer);
        }
    }
}

/******************************************************************************/
// Link phase
// For each view, join the disparate sections into the final product

#[derive(Debug)]
struct LinkerViewMetadata<'input_path, 'lang_group_list, 'shared> {
    id: &'input_path str,
    frontmatter_serialised: String,
    series_cache_lines: Vec<String>,
    tags_cache_lines: Vec<String>,
    lang: &'lang_group_list str,
    relative_output_loc: String,
    title: &'shared str,
    other_langs: (&'lang_group_list str, &'lang_group_list str),
}

fn linker_metadata_list_new<'input_path, 'lang_group_list, 'shared>(
    (config, input_list, shared_metadata): Shared<'_, '_, 'input_path, 'shared>,
    lang_group_list: &'lang_group_list [String],
) -> Vec<LinkerViewMetadata<'input_path, 'lang_group_list, 'shared>> {
    debug_assert_eq!(input_list.len(), lang_group_list.len());

    // Each view must know about its parent's other views to link to them
    // So first render the links into 'view_links'
    let view_count = shared_metadata.len();
    let mut linker_metadata = Vec::with_capacity(view_count);
    for (_, j, _, _, view_data) in walk(shared_metadata) {
        let path = &input_list[j];
        let frontmatter = Frontmatter::new(
            view_data.frontmatter_string.as_str(),
            path.created,
            path.updated,
        )
        .map_err(|err| err.with_filename(path.path.to_string_lossy()))
        .or_die(1);
        let lang = &lang_group_list[j][view_data.lang.clone()];

        linker_metadata.push_and_check(LinkerViewMetadata {
            frontmatter_serialised: frontmatter.serialise(),
            tags_cache_lines: frontmatter.format_to_tag_cache(path.stem, lang),
            series_cache_lines: frontmatter.format_to_series_cache(path.stem, lang),
            lang,
            relative_output_loc: frontmatter.format(config.output_format, path.stem, lang),
            id: path.stem,
            title: match frontmatter.lookup("title") {
                Some(Value::Utf8(s)) => s,
                _ => "",
            },
            other_langs: exclude(&lang_group_list[j], lang),
        });
    }
    linker_metadata
}

fn output_target(config: &RequiredConfigs, linker_view_metadata: &LinkerViewMetadata) -> String {
    [config.public_dir, "/", linker_view_metadata.relative_output_loc.as_str()].join("")
}

fn join_partials(
    (config, input_list, shared_metadata): Shared,
    changelog: &UpdateTimes,
    linker_metadata: &[LinkerViewMetadata],
) {
    //println!("{:#?}", linker_metadata);
    //std::process::exit(0);

    // Run the linker to join the partials (toc and doc)
    for (i, j, _, post_range, shared) in walk(shared_metadata) {
        let post_data = &linker_metadata[post_range];
        let my_data = &linker_metadata[i];
        let target = output_target(config, my_data);
        let input_path_obj = &input_list[j];
        let output_path = Path::new(target.as_str());
        let is_target_missing_or_outdated = PathReadMetadata::wrap(output_path)
            .map(|_| changelog.check_if_outdated(&input_path_obj))
            .unwrap_or(true); // File is missing (or other error)

        //let path = PathReadMetadata::wrap(Path::new(target.as_str())).unwrap();
        //println!("{:?} {:?}\n{:?}\n{} {:?}\n", is_target_missing_or_outdated,
        //    path.updated,
        //    Utc::now(),
        //    path.stem, changelog.0.get(path.stem),

        //    );
        //if true {
        //} else
        if config.force || shared.is_outdated || is_target_missing_or_outdated {
            let args = fmt_linker_args(
                config,
                target.as_str(),
                &shared_metadata[i],
                post_data,
                my_data,
            );

            let args = {
                let mut borrow: Vec<&str> = Vec::with_capacity(args.len());
                for entry in &args {
                    borrow.push_and_check(entry);
                }
                borrow
            };

            create_parent_dir(target.as_str()).or_die(1);
            // @TODO Only link if out of date or final file is missing
            eprintln!("Linking {} {}", my_data.lang, target.escape());
            print!(
                "{}",
                command_run(Path::new(config.linker), None, &args).or_die(1)
            );

            if config.explicit {
                eprint!("=== Arg 1: Frontmatter ====\n{}", &args[0]);
                eprint!("=== Rest ===\n");
                for line in &args[1..] {
                    eprint!("{}\n", line);
                }
                eprint!("\n");
            }
        } else if config.verbose {
            eprintln!("Skipping linking {} {}", my_data.lang, target.escape());
        }
    }
}

const DELETE: bool = false;
const UPDATE: bool = true;

fn write_caches(
    (config, input_list, shared_metadata): Shared,
    changelog: &UpdateTimes,
    linker_metadata: &[LinkerViewMetadata],
    is_update: bool,
) {
    debug_assert_eq!(changelog.0.len(), input_list.len());
    debug_assert_eq!(shared_metadata.len(), linker_metadata.len());

    // Could not figure out lifetimes for doing this in a loop
    // 1. The read-filter step is the same for all caches
    // 2. The insert step is unique to each cache
    // 3. The sort-then-write step is the same for all caches
    fn read_old_and_sieve<'a>(
        id_map: &HashMap<&str, ()>,
        pathstr: &str,
        old_cache: &'a mut String,
        count: usize,
        id_index: usize,
    ) -> (usize, Vec<Cow<'a, str>>) {
        let path = Path::new(pathstr);
        if let Err(err) = read_file(path, old_cache) {
            eprintln!("{}.\n-> Generating {}...", err, pathstr.escape());
        }

        // This the max size (if not recompiling old posts)
        let capacity = old_cache.lines().count() + count;
        let mut cache = Vec::with_capacity(capacity);
        cache.extend(old_cache.lines().filter_map(|line| {
            let id = line.split(',').nth(id_index).unwrap_or("");
            (!id_map.contains_key(id)).to_some(Cow::Borrowed(line))
        }));
        (capacity, cache)
    }
    macro_rules! update_cache {
        (
            @id_list_to_add    $id_map:ident,
            @location          $loc:expr,
            @to_add_line_count $to_add:ident,
            @id_index_in_cache $id_index:literal,

            if $is_update:ident then
                $insert:expr;
            $msg:literal
        ) => {
            let loc = $loc;
            let mut old = String::new();
            let (capacity, mut cache) =
                read_old_and_sieve(&$id_map, loc, &mut old, $to_add, $id_index);
            if $is_update {
                cache.extend($insert);
            }
            eprintln!($msg, loc.escape());
            write_after_add(capacity, cache, loc);
        };
    }
    fn write_after_add(capacity: usize, mut cache: Vec<Cow<str>>, loc: &str) {
        debug_assert!(cache.len() <= capacity);
        cache.sort_unstable();
        write_file(loc, cache.join("\n").as_str()).or_die(1);
        //eprintln!("{:#?}\n", cache);
    }

    let mut id_map = HashMap::with_capacity(input_list.len());
    for path in input_list {
        id_map.insert(path.stem, ());
    }
    let has_any_change = shared_metadata.iter().any(|data| data.is_outdated);
    let view_count = linker_metadata.len();

    if config.force || has_any_change {
        eprintln!("Saving file update times to {}", config.changelog.escape());
        changelog.write_to(config.changelog.as_str()).or_die(1);

        // @FORMAT
        // Specifically separating path and path so csv can support
        // This means the only invalid character is newline
        // Frontmatter makes sure to format date without commas

        // @FORMAT tags cache
        // Last column is title (only column that could have commas as data)
        let tag_line_count = linker_metadata
            .iter()
            .map(|data| data.tags_cache_lines.len())
            .sum();
        update_cache! {
            @id_list_to_add    id_map,
            @location          config.tags_cache.as_str(),
            @to_add_line_count tag_line_count,
            @id_index_in_cache 2,

            if is_update then
                linker_metadata
                    .iter()
                    .flat_map(|data| data.tags_cache_lines.iter().map(String::as_str))
                    .map(Cow::Borrowed);

            "Saving tags cache to {}"
        }

        // @FORMAT link cache
        // Last column is path (only column that could have commas as data)
        update_cache! {
            @id_list_to_add    id_map,
            @location          config.link_cache.as_str(),
            @to_add_line_count view_count,
            @id_index_in_cache 0,

            if is_update then
                linker_metadata
                    .iter()
                    // @FORMAT
                    .map(|d| [d.id, d.lang, d.relative_output_loc.as_str()])
                    .map(|array| array.join(","))
                    .map(Cow::Owned);

            "Saving link cache to {}"
        }

        // @FORMAT series cache
        // Last column is title (only column that could have commas as data)
        let series_line_count = linker_metadata
            .iter()
            .map(|data| data.series_cache_lines.len())
            .sum();
        update_cache! {
            @id_list_to_add    id_map,
            @location          config.series_cache.as_str(),
            @to_add_line_count series_line_count,
            @id_index_in_cache 2,

            if is_update then
                linker_metadata
                    .iter()
                    .flat_map(|data| data.series_cache_lines.iter().map(String::as_str))
                    .map(Cow::Borrowed);

            "Saving series cache to {}"
        }

    //eprintln!("{:#?}\n", link);
    } else {
        eprintln!("No change in posts detected, caches unmodified (use --force to override)");
    }
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
    local_target: &str,
    shared: &'shared ViewMetadata,
    post_data: &[LinkerViewMetadata],
    data: &'frontmatter_string LinkerViewMetadata<'shared, '_, 'frontmatter_string>,
) -> Vec<Cow<'frontmatter_string, str>> {
    let relative_target = data.relative_output_loc.as_str();
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
        Cow::Owned(["blog_relative:", config.blog_relative].join("")),
        Cow::Owned(["link_cache:", config.link_cache.as_str()].join("")),
        Cow::Owned(["series_cache:", config.series_cache.as_str()].join("")),
        Cow::Owned(["language:", data.lang].join("")),
        Cow::Owned(["local_templates_dir:", config.templates_dir].join("")),
        Cow::Owned(["local_toc_path:", shared.toc_loc.as_str()].join("")),
        Cow::Owned(["local_doc_path:", shared.doc_loc.as_str()].join("")),
        Cow::Owned(["local_output_path:", local_target].join("")),
        Cow::Owned(["relative_output_url:", relative_target].join("")),
        Cow::Owned([
            "other_view_langs:",
            data.other_langs.0,
            data.other_langs.1
        ].join("")),
    }
    post_data
        .iter()
        .enumerate()
        .filter(|(i, _)| i != &shared.view_index)
        .map(|(_, data)| (data.lang, data.relative_output_loc.as_str()))
        .map(|(lang, loc)| ["relative_", lang, "_view:", loc].join(""))
        .for_each(|keyval| api_keyvals.push_and_check(Cow::Owned(keyval)));
    assert_eq!(capacity, api_keyvals.len());

    api_keyvals
}

/******************************************************************************
 * Helper functions
 ******************************************************************************/
#[derive(Debug)]
struct UpdateTimes<'log>(HashMap<&'log str, DateTime<Utc>>);

impl<'log> UpdateTimes<'log> {
    fn new(log_str: &'log str) -> Result<Self, ParseError> {
        let mut log = HashMap::with_capacity(log_str.lines().count());
        //eprintln!("{:?}", log_str.lines().collect::<Vec<_>>());
        for (i, line) in log_str.lines().enumerate().filter(|(_, l)| !l.is_empty()) {
            // @TODO push_and_check for hash
            let (id, timestr_with_comma) = line
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

            let timestr = &timestr_with_comma[','.len_utf8()..];
            let timestamp = NaiveDateTime::parse_from_str(timestr, "%s")
                .map_err(|err| (i + 1, line, Cow::Owned(err.to_string())))?;
            log.insert(id, Utc.from_utc_datetime(&timestamp));
        }
        Ok(Self(log))
    }

    fn check_if_outdated(&self, id: &PathReadMetadata) -> bool {
        self.0
            .get(id.stem)
            .map(|log| &id.updated > log)
            .unwrap_or(true)
    }

    fn update(&mut self, id: &PathReadMetadata<'log>) {
        self.0.insert(id.stem, Utc::now());
    }
    fn remove(&mut self, id: &PathReadMetadata) -> Option<DateTime<Utc>> {
        self.0.remove(id.stem)
    }

    const MAX_DIGITS: usize = "-9223372036854775808".len();


    fn write_to(&self, loc: &str) -> Result<(), String> {
        debug_assert_eq!(i64::MIN, -9223372036854775808);
        let capacity = self
            .0
            .iter()
            .fold(0, |sum, (key, _)| sum + key.len() + 2 + Self::MAX_DIGITS);

        let mut buffer = String::with_capacity(capacity);
        for (key, datetime) in self.0.iter() {
            buffer.push_str(key);
            buffer.push(',');
            let timestamp: i64 = datetime.timestamp();
            buffer.push_str(timestamp.to_string().as_str());
            buffer.push('\n');
        }

        write_file(loc, buffer.as_str())
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

fn write_file(loc: &str, buffer: &str) -> Result<(), String> {
    fs::write(loc, buffer).map_err(|err| {
        [
            "Cannot write to file ",
            loc.escape().as_str(),
            ". ",
            err.to_string().as_str(),
        ]
        .join("")
    })
}

fn delete_file(loc: &str) -> Result<(), String> {
    fs::remove_file(loc).map_err(|err| {
        [
            "Cannot delete file ",
            loc.escape().as_str(),
            ". ",
            err.to_string().as_str(),
        ]
        .join("")
    })

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
