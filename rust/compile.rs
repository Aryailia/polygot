// This brings the disparate parts together to do the compile pipeline.
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use std::{
    borrow::Cow,
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

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

// @TODO Create 'PathReadMetadata' from DirEntry walk
// @TODO 'PathReadMetadata' move to helpers
// @TODO remove println/eprintln replacing with writes to stdout/stderr
// @TODO check if we can get away with just using Utc::now() for updating
//       changelog; that we do not need to read output file updated time
// @TODO Add spacing between different compile steps, make a print vec function
// @TODO Decide on 'filetime' or rust's metadata for 'PathReadMetadata'
// @TODO delete file
// @TODO rename file

//run: ../../make.sh build -d
// run: cargo test compile -- --nocapture

// A three-major-step build procees
// 1. Analyse the metadata, process my custom markup
// 2. Run the markup's compiler (markup->HTML) (Asciidoctor, org, Pandoc, etc.)
// 3. Verify frontmatter, update the caches, then run the linker
pub fn compile(config: &RequiredConfigs, input_list: &[PathBuf]) {
    // The relative relationship is:
    // - one source text <> one 'post' <> many langs/views
    // - one lang <> one view
    // - one views <> one toc and one body (one post <> lang_num * 2 sections)
    // - one view <> one linked output file

    // @TODO Handle this in main.rs
    let file_count = input_list.len();
    let mut id_map = HashMap::with_capacity(file_count);
    let mut input_paths = Vec::with_capacity(file_count);
    for x in input_list {
        let path = PathReadMetadata::wrap(x).or_die(1);
        id_map.insert(path.stem, ());
        input_paths.push_and_check(path);
    }

    // Read the changelog to see if posts is outdated
    // Update the changelog only after htmlify and frontmatter is verified
    let mut log_result = String::new();
    let log_str = read_file(Path::new(&config.changelog), &mut log_result)
        .map(|_| log_result.as_str())
        .unwrap_or("");
    let mut changelog = UpdateTimes::new(log_str)
        .map_err(|err| err.with_filename(Cow::Borrowed(&config.changelog)))
        .or_die(1);

    // Parse into post_list, and extract relevant
    // This data is split as so to manage ownership and lifetimes
    // - 'shared_view_metadata' is for 'htmlify_into_partials' and  'join_partials'
    //   - each will then compute the relevant borrowed data
    // - 'post_list' borrows from 'text_list'
    //
    //  I separate out 'api_and_comment', 'post_list', and 'text_list'
    //  as only 'htmlify_into_partials' needs them so they could be dropped
    //  at 'join_partials'
    let mut text_list = Vec::with_capacity(file_count);
    for path in &input_paths {
        let mut text = String::new();
        read_file(path.path, &mut text).or_die(1);
        text_list.push_and_check(text);
    }
    let (shared_view_metadata, api_and_comment, post_list, lang_list) =
        analyse_metadata(config, &text_list, &changelog, &input_paths);

    // Run the markup compiler
    //
    // Though I probably should do the frontmatter validation before
    // writing to files in 'htmlify_into_partials', but decided against it
    // because it makes the lifetime dependency graph complicated
    htmlify_into_partials(
        config,
        &input_paths,
        &mut changelog,
        &shared_view_metadata,
        api_and_comment,
        post_list,
    );
    // We can drop 'text_list' and 'api_and_comment' here

    // Link/Join the partials into the final output
    // Frontmatter's lifetime depends on 'shared_view_metadata'
    join_partials(
        config,
        id_map,
        &input_paths,
        &changelog,
        &shared_view_metadata,
        &lang_list,
    );
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
type ApiAndComment<'path, 'config> = HashMap<&'path str, (FileApi<'config>, String)>;

// Using this so that we can discard 'api_and_comment' and 'text_list'
struct ViewMetadataWalker<'a> {
    index: usize,
    file_index: usize,
    start: usize,
    close: usize,
    iter: std::slice::Iter<'a, ViewMetadata>,
}
fn walk(shared_view_metadata: &[ViewMetadata]) -> ViewMetadataWalker {
    ViewMetadataWalker {
        index: 0,
        file_index: 0,
        start: 0,
        close: 0,
        iter: shared_view_metadata.iter(),
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

fn analyse_metadata<'config, 'text, 'path>(
    config: &'config RequiredConfigs,
    text_list: &'text [String],
    changelog: &UpdateTimes,
    input_paths: &[PathReadMetadata<'path>],
) -> (
    Vec<ViewMetadata>,
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
    for (path, text) in zip!(input_paths, text_list) {
        let extension = path.extension;
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
        let post = Post::new(text, comment.as_str())
            .map_err(|err| err.with_filename(path.path.to_string_lossy()))
            .or_die(1);

        views_count += post.views.len();
        post_list.push_and_check(post);
    }

    // Build 'shared_view_metadata' (referenes frontmatter)
    // This is independent of 'text_list' lifetime
    let mut shared_view_metadata = Vec::with_capacity(views_count);
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

            shared_view_metadata.push_and_check(ViewMetadata {
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

    (shared_view_metadata, api_and_comment, post_list, lang_list)
}

/******************************************************************************/
// Compile step
// HTMLify the post (i.e. run through asciidoctor, etc.)
// Also splits the table of contents (toc) and the body (doc)

fn htmlify_into_partials<'input>(
    config: &RequiredConfigs,
    input_list: &[PathReadMetadata<'input>],
    changelog: &mut UpdateTimes<'input>,
    shared_view_metadata: &[ViewMetadata],
    api_and_comment: ApiAndComment,
    post_list: Vec<Post>, // Eat this
) {
    debug_assert_eq!(input_list.len(), post_list.len());

    // Because we flatten post views, using cursor to
    let mut buffer = String::new();
    for (_, j, is_new_post, _, view_data) in walk(shared_view_metadata) {
        let path = &input_list[j];
        if is_new_post {
            buffer.clear();
            // @TODO implement non-allocating escape
            buffer.push('"');
            buffer.push_str(path.stem);
            buffer.push('.');
            buffer.push_str(path.extension);
            buffer.push('"');
        }

        let toc_loc = view_data.toc_loc.as_str();
        let doc_loc = view_data.doc_loc.as_str();

        if view_data.is_outdated || !Path::new(toc_loc).exists() || !Path::new(doc_loc).exists() {
            let (api, _) = api_and_comment.get(path.extension).unwrap();
            let view = &post_list[j].views[view_data.view_index];

            // @TODO: Create directories in building api cache (less work)
            create_parent_dir(toc_loc).or_die(1);
            create_parent_dir(doc_loc).or_die(1);
            api.compile(view.body.as_slice(), toc_loc, doc_loc)
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
            eprintln!("Skipping {} compile (use --force to not skip)", buffer);
        }
    }
}

/******************************************************************************/
// Link phase
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
    input_list: &[PathReadMetadata],
    changelog: &UpdateTimes,
    shared_view_metadata: &[ViewMetadata],
    lang_group_list: &[String],
) {
    debug_assert_eq!(input_list.len(), lang_group_list.len());

    // Each view must know about its parent's other views to link to them
    // So first render the links into 'view_links'
    let view_count = shared_view_metadata.len();
    let mut linker_metadata = Vec::with_capacity(view_count);
    for (_, j, _, _, view_data) in walk(shared_view_metadata) {
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
            tags_cache_line: frontmatter.format_to_tag_cache(path.stem, lang),
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

    // Must update the cache before linking as linker uses this info
    write_caches(
        config,
        id_map,
        &shared_view_metadata,
        changelog,
        &linker_metadata,
    );

    // Format hello
    // @TODO these should both be sorted
    // so we can do a better
    //tags.cache.lines()
    //    .filter(|line| line.split(',').get(3).unwrap_or("") == )
    //    .chain()

    // Run the linker to join the partials (toc and doc)
    for (i, j, _, post_range, shared) in walk(shared_view_metadata) {
        let post_data = &linker_metadata[post_range];
        let my_data = &linker_metadata[i];
        let target = [config.public_dir, "/", my_data.relative_output_loc.as_str()].join("");
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
        if shared.is_outdated || is_target_missing_or_outdated {
            let args = fmt_linker_args(
                config,
                target.as_str(),
                &shared_view_metadata[i],
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
                command_run(
                    Path::new(config.linker),
                    (config.domain, config.blog_relative),
                    None,
                    &args,
                )
                .or_die(1)
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

fn write_caches(
    config: &RequiredConfigs,
    id_map: HashMap<&str, ()>,
    shared_view_metadata: &[ViewMetadata],
    changelog: &UpdateTimes,
    linker_metadata: &[LinkerViewMetadata],
) {
    debug_assert_eq!(changelog.0.len(), id_map.len());
    debug_assert_eq!(shared_view_metadata.len(), linker_metadata.len());

    let has_any_change = shared_view_metadata.iter().any(|data| data.is_outdated);
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
            eprintln!("{}.\n-> Generating {}...", err, pathstr.escape());
        }

        let mut cache = Vec::with_capacity(old_cache.lines().count() + count);
        cache.extend(old_cache.lines().filter_map(|line| {
            let id = line.split(',').nth(id_index).unwrap_or("");
            (!id_map.contains_key(id)).to_some(Cow::Borrowed(line))
        }));
        cache
    }

    if has_any_change {
        eprintln!("Saving file update times to {}", config.changelog.escape());
        changelog.write_to(config.changelog.as_str()).or_die(1);

        let mut tags_old = String::new();
        let tags_loc = config.tags_cache.as_str();
        let mut tags = read_and_sieve_in_old(&id_map, tags_loc, &mut tags_old, view_count, 2);
        tags.extend(
            linker_metadata
                .iter()
                .flat_map(|x| x.tags_cache_line.split('\n').map(|x| Cow::Borrowed(x))),
        );
        tags.sort_unstable();
        eprintln!("Saving tags cache to {}", tags_loc.escape());
        write_file(tags_loc, tags.join("\n").as_str()).or_die(1);
        //eprintln!("{:#?}\n", tags);

        let mut link_old = String::new();
        let link_loc = config.link_cache.as_str();
        let mut link = read_and_sieve_in_old(&id_map, link_loc, &mut link_old, view_count, 0);
        link.extend(
            linker_metadata
                .iter()
                .map(|x| Cow::Owned([x.id, x.lang, x.relative_output_loc.as_str()].join(","))),
        );
        link.sort_unstable();
        eprintln!("Saving link cache to {}", link_loc.escape());
        write_file(link_loc, link.join("\n").as_str()).or_die(1);
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
        Cow::Owned(["language:", data.lang].join("")),
        Cow::Owned(["local_templates_dir:", config.templates_dir].join("")),
        Cow::Owned(["local_toc_path:", shared.toc_loc.as_str()].join("")),
        Cow::Owned(["local_doc_path:", shared.doc_loc.as_str()].join("")),
        Cow::Owned(["local_output_path:", local_target].join("")),
        Cow::Owned(["relative_output_url:", relative_target].join("")),
        //Cow::Owned(["relative_tags_url:", data.tags_loc.as_str()].join("")),
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

    fn check_if_outdated2(&self, id: &str, timestamp: &DateTime<Utc>) -> bool {
        self.0
            .get(id)
            .map(|logged_time| timestamp > logged_time)
            .unwrap_or(true)
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
            "Cannot write to ",
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
