#![allow(dead_code)]
use chrono::offset::Local;
use filetime::{set_file_mtime, FileTime};
use std::{env, fs, io::ErrorKind, path::Path, process::exit, str::Chars};

mod custom_errors;
mod helpers;
mod fileapi;
mod frontmatter;
mod post;
mod traits;
use fileapi::{command_run, FileApi};
use frontmatter::Frontmatter;
use post::Post;
use traits::*;

const NAME: &str = "blog";

macro_rules! match_subcommands {
    ($args:ident {
        $($arg_count:literal, $subcommand:literal => $block:block)*
    }) => {
        let len = $args.len();
        let first = $args.first().map(|s| s.as_str());
        match first {
            $(Some(arg) if arg == $subcommand => {
                if len != $arg_count {
                    eprintln!("`{} {}` requires {} arguments. You provided {} arguments", NAME, arg, $arg_count, len);
                    exit(1);
                }
                $block
            })*
            Some(arg) => {
                eprintln!("`{} {}` is an invalid subcommand.", NAME, arg);
                exit(1)
            }
            _ => {
                eprintln!("No subcommand given. `{} -h` for list of subcommands", NAME);
                exit(1)
            }

        }

    };
}

macro_rules! define_config {
    (
        @optional {
            $($o_short:literal $o_long:literal
               $o_id:ident: $o_type:ty = $o_default:expr => $($to_set:expr)?,)*
        }
        @to_be_required {
            $($r_short:literal $r_long:literal $r_id:ident,)*
        }
    ) => {
        //#[derive(Debug)]
        struct Config {
            $($o_id: $o_type,)*
            $($r_id: Option<String>,)*
        }
        impl Config {
            fn new() -> Self {
                Self {
                    $($o_id: $o_default,)*
                    $($r_id: None,)*
                }
            }
        }

        #[inline]
        fn parse_option(arg_iter: &mut env::Args, config: &mut Config, option: &str) -> Result<(), String> {
            match option {
                "h" | "help" => {}
                $($o_short | $o_long => config.$o_id = $o_default,)*
                $($r_short | $r_long => config.$r_id = arg_iter.next(),)*
                _ => {
                    return Err(format!(
                        "'{}' is an invalid option\nTry `{} -h` for help",
                        option, NAME,
                    ))
                }
            }
            Ok(())
        }

        // Put in a struct so that we can keep the variable names
        // the same between 'parse_option' and use in 'compile_post'
        // Naming chosen for the sentence: 'RequireConfigs::unwrap(config)'
        struct RequiredConfigs<'a> {
            $($r_id: &'a str,)*
        }
        impl<'a> RequiredConfigs<'a> {
            fn unwrap(config: &'a Config) -> Self {
                Self {
                    $($r_id: config.$r_id.as_ref()
                        .ok_or("--api-dir is a required option")
                        .or_die(1)
                        .as_str(),
                    )*
                }
            }
        }
    };
}

/******************************************************************************
 * Main entry
 ******************************************************************************/
define_config! {
    @optional {
        // "help" is special cased (see macro definition)
        // short long ident: type = default => value after option specified
        "v" "verbose" verbose: bool = false => true, // true if -v is specified
        "f" "force"   force:  bool = false => true,
    }
    @to_be_required {
        "a" "api-dir" api_dir,
        // @VOLATILE sync this with 'compile_post'
        "c" "cache-dir"     cache_dir,
        "d" "domain"        domain,        // public dir as a URL
        "p" "public-dir"    public_dir,    // public dir as a path
        "t" "templates-dir" templates_dir,
    }
}

fn main() {
    let (config, args) = Config::parse_env().or_die(1);

    // run: cargo run compare-last-updated a b
    // run: cargo run sync-last-updated-of-first-to b a

    match_subcommands!(args {
        1, "now-rfc2822" => {
            println!("{}", Local::now().to_rfc2822());
        }
        3, "compare-last-updated" => {
            if compare_mtimes(args.get(1).unwrap(), args.get(2).unwrap()) {
                exit(0);
            } else {
                exit(1);
            }
        }
        3, "sync-last-updated-of-first-to" => {
            sync_last_updated(args.get(1).unwrap(), args.get(2).unwrap());
        }

        4, "compile-markup" => {
            let source = args.get(1).unwrap();
            let post_formatter = args.get(2).unwrap();
            let path_formatter = args.get(3).unwrap();
            check_is_file_or_die(post_formatter.as_str());

            compile_post(&config, &source, &post_formatter, &path_formatter);
        }

        1, "test" => {
            let api = FileApi::from_filename("hello.adoc", "../config/api").unwrap();
            println!("{:?}", api.comment());
        }
    });
}

use chrono::{Utc, TimeZone, DateTime};
use std::io;
use std::time::SystemTime;
fn to_datetime(time_result: io::Result<SystemTime>, msg: String) -> Result<DateTime<Utc>, String> {
    let system_time = time_result.map_err(|err| format!("{} is not supported on this filesystem. {}", msg, err))?;
    let time = system_time.duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|err| format!("{} is before UNIX epoch. {}", msg, err))?;
    let secs = time.as_secs() / 1_000_000_000;
    let nano = time.as_nanos() % 1_000_000_000;
    if secs > i64::MAX as u64 {
        return Err(format!("{} is too big and is not supported by the 'chrono' crate", msg));
    }
    //println!("s {:?}", Utc.timestamp(time.as_secs(), 0));
    //println!("ns{:?}", Utc.timestamp(time.as_secs(), time.as_nanos()));

    Ok(Utc.timestamp(secs as i64, nano as u32))
}

// @TODO implement warning system (not just fatal errors) for custom_errors.rs?
//fn issue_warning() {
//    eprintln!()
//}

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
    let modified = to_datetime(metadata.modified(),
        format!("The file created date of {:?}", pathstr))?;
    let created = to_datetime(metadata.created(),
        format!("The file last modified date metadata of {:?}", pathstr))?;

    Ok((file_stem, extension, created, modified))
}

//run: ../build.sh

#[test]
fn compile_test() {
    let post = Post::new("hello", "//").or_die(1);
    let view = post.views.first().unwrap();
    let api = FileApi::from_filename("config/api/", "adoc").or_die(1);
    let frontmatter_string = api.frontmatter(view.body.as_slice()).unwrap();
    let frontmatter = Frontmatter::new(frontmatter_string.as_str(), Utc::now(),Utc::now()).or_die(1);
    assert!(frontmatter.lookup("date-created").is_some());
    assert!(frontmatter.lookup("date-updated").is_some());
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

fn compile_post(config: &Config, pathstr: &str, post_formatter: &str, path_format: &str) {
    let (stem, ext, created, modified) = analyse_path(pathstr).or_die(1);
    let text = fs::read_to_string(pathstr)
        .map_err(|err| format!("Cannot read {:?}. {}", pathstr, err))
        .or_die(1);
    let x = RequiredConfigs::unwrap(config);
    // @VOLATILE sync with 'define_config'
    check_is_dir_or_die(x.cache_dir, "--cache-dir");
    check_is_file_or_die(x.api_dir);

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

fn check_is_file_or_die(pathstr: &str) {
    if !Path::new(pathstr).is_file() {
        Path::new(pathstr)
            .metadata()
            .map_err(|err| format!("'{:?}' is not a valid file. {}", pathstr, err))
            .or_die(1);
    }
}

fn check_is_dir_or_die(pathstr: &str, error_msg: &str) {
    if !Path::new(pathstr).is_dir() {
        Path::new(pathstr)
            .metadata()
            .map_err(|err| {
                format!(
                    "`{} {:?}` is not a valid directory. {}",
                    error_msg, pathstr, err
                )
            })
            .or_die(1);
    }
}

fn sync_last_updated(first: &str, date_source: &str) -> ! {
    Path::new(date_source)
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .and_then(|filetime| set_file_mtime(Path::new(first), filetime))
        .map_err(|err| format!("'{}' {}", date_source, err))
        .or_die(1);
    exit(0)
}

fn compare_mtimes(source: &str, target: &str) -> bool {
    let source_date = Path::new(source)
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .map_err(|err| format!("'{}' {}", source, err))
        .or_die(1);

    let target_date = Path::new(target)
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .or_else(|err| match err.kind() {
            ErrorKind::NotFound => Ok(FileTime::zero()),
            _ => Err(err),
        })
        .map_err(|err| format!("'{}' {}", target, err))
        .or_die(1);

    source_date < target_date
}

// Although I could use clap.rs, I want to keep this lean
// Also implementing manually as a learning experience
impl Config {
    fn parse_env() -> Result<(Self, Vec<String>), String> {
        let mut output = Vec::with_capacity(env::args().count());
        let mut config = Config::new();
        let mut literal = false;
        //let mut stdin = false;

        let mut arg_iter = env::args();
        arg_iter.next(); // skip the 0th parameter (path to program running)
        while let Some(arg) = arg_iter.next() {
            if literal || !arg.starts_with('-') {
                output.push_and_check(arg);
            } else if arg.as_str() == "--" {
                literal = true;
            //} else if arg.as_str() == "-" {
            //    stdin = true;
            } else {
                for option in parse_option_str(arg.as_str()) {
                    parse_option(&mut arg_iter, &mut config, option)?;
                }
            }
        }
        Ok((config, output))
    }
}

enum OptionsSplitState {
    Short,
    Long,
    LongDone,
}
struct OptionsSplit<'a> {
    state: OptionsSplitState,
    iter: Chars<'a>,
}
// Does not handle '--' or '-' case
fn parse_option_str(option_str: &str) -> OptionsSplit {
    let mut iter = option_str.chars();
    debug_assert!(option_str.starts_with('-'));
    debug_assert_ne!(option_str, "-");
    debug_assert_ne!(option_str, "--");
    iter.next();

    OptionsSplit {
        state: if option_str.starts_with("--") {
            OptionsSplitState::Long
        } else {
            OptionsSplitState::Short
        },
        iter,
    }
}
impl<'a> Iterator for OptionsSplit<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        let rest = self.iter.as_str();
        let ch = self.iter.next()?;
        match self.state {
            OptionsSplitState::Long => {
                self.state = OptionsSplitState::LongDone;
                Some(self.iter.as_str())
            }
            OptionsSplitState::LongDone => None,
            OptionsSplitState::Short => Some(&rest[0..ch.len_utf8()]),
        }
    }
}

#[cfg(test)]
mod main_tests {
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
