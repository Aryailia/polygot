#![allow(dead_code)]
use chrono::offset::Local;
use filetime::{set_file_mtime, FileTime};
use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::exit,
    str::Chars,
};

mod custom_errors;
mod fileapi;
mod frontmatter;
mod post;
mod traits;
use fileapi::{FileApi, command_run};
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

#[inline]
fn parse_option(arg_iter: &mut env::Args, config: &mut Config, option: &str) -> Result<(), String> {
    match option {
        "v" | "verbose" => config.verbose = true,
        "h" | "help" => {}
        "api-dir" => config.api_dir = arg_iter.next(),
        "cache-dir" => config.cache_dir = arg_iter.next(),
        "domain" => config.domain = arg_iter.next(),
        "public-dir" => config.public_dir = arg_iter.next(),
        "templates-dir" => config.templates_dir = arg_iter.next(),
        _ => {
            return Err(format!(
                "'{}' is an invalid option\nTry `{} -h` for help",
                option, NAME,
            ))
        }
    }
    Ok(())
}

fn main() {
    let (config, args) = Config::parse_env().or_die(1);

    // run: cargo run compare-last-updated a b
    // run: cargo run sync-last-updated-of-first-to b a

    match_subcommands!(args {
        1, "date-rfc2822" => {
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

//#[test]
//fn asdf() {
//    StringPath::new(&[".cache/toc/file.adoc"]);
//}

// @TODO use error in post.rs
// @TODO: use this in fileapi.rs
fn split_name_extension(pathstr: &str) -> Result<(&str, &str), String> {
    let path = Path::new(pathstr);
    let stem_os = path.file_stem().ok_or_else(|| {
        format!("The post path {:?} does not is not a path to a file", pathstr)
    })?;
    let ext_os = path.extension().ok_or_else(|| {
        format!("The post {:?} does not have a file extension", pathstr)
    })?;

    let file_stem = stem_os.to_str().ok_or_else(|| {
        format!("The stem {:?} in {:?} has invalid UTF8", stem_os, pathstr)
    })?;
    let extension = ext_os.to_str().ok_or_else(|| {
        format!("The extension {:?} in {:?} has invalid UTF8", ext_os, pathstr)
    })?;
    //(file_stem, extension)
    Ok((file_stem, extension))

}


macro_rules! join {
    ($($t:expr),*) => {
        [$($t),*].join("").as_str()
    };
}

// Lexically hide the owned value
macro_rules! borrow {
    (let $lhs:ident = $rhs:expr) => {
        let $lhs = $rhs;
        let $lhs = &$lhs;
    };
}
//run: ../build.sh
fn compile_post(config: &Config, pathstr: &str, post_formatter: &str, path_format: &str) {
    let (stem, ext) = split_name_extension(pathstr).or_die(1);
    // @TODO: Macroify/const function this
    let text = fs::read_to_string(pathstr)
        .map_err(|err| format!("Cannot read {:?}. {}", pathstr, err))
        .or_die(1);
    let api_dir = config.api_dir.as_ref()
        .ok_or("--api-dir is a required option")
        .or_die(1);
    let cache_dir = config.cache_dir.as_ref()
        .ok_or("--cache-dir is a required option")
        .or_die(1);
    let domain = config.domain.as_ref()
        .ok_or("--domain is a required option")
        .or_die(1);
    let public_dir = config.public_dir.as_ref()
        .ok_or("--public_dir is a required option")
        .or_die(1);
    let templates_dir = config.templates_dir.as_ref()
        .ok_or("--templates-dir is a required option")
        .or_die(1);

    check_is_dir_or_die(cache_dir.as_str(), "--cache-dir");
    check_is_file_or_die(api_dir.as_str());

    let api = FileApi::from_filename(pathstr, api_dir.as_str()).or_die(1);

    //let asdf = "// api_set_lang: yo/ \nasdf\nasdf\n// api_set_lang:try *\nyo";
    //println!("{}", text);
    //println!("########\n# Done #\n########\n");
    let post = match Post::new(text.as_str(), &api) {
        Ok(x) => x,
        Err(err) => {
            eprintln!("{}{}", pathstr, err);
            exit(1);
        }
    };
    post.views.iter().for_each(|view| {
        println!("###### {} ######", view.lang.unwrap_or("All"));
        let frontmatter_string = api.frontmatter(view.body.as_slice()).or_die(1);
        let frontmatter = Frontmatter::new(frontmatter_string.as_str())
            .map_err(|err| err.with_filename(pathstr))
            .or_die(1);
        let lang = view.lang.unwrap_or("");
        borrow!(let output_loc = frontmatter.format(path_format, stem, lang));
        borrow!(let tags_loc = frontmatter.format(path_format, "tags", lang));
        command_run(
            Path::new(post_formatter),
            None,
            &[
                frontmatter.serialise().as_str(),
                join!("domain:", domain),
                join!("local_toc_path:", cache_dir, "/toc/", output_loc),
                join!("local_body_path:", cache_dir, "/body/", output_loc),
                join!("local_templates_dir:", templates_dir),
                join!("local_output_path:", public_dir, "/blog/", output_loc),
                join!("relative_output_url:", output_loc),
                join!("relative_tags_url:", tags_loc),
                join!("lang_list:", post.lang_list.join(" ").as_str()),
            ]
        ).or_die(1);

        //println!("{}", view.body.join(""));
        //println!("{}", frontmatter_string);
        //println!("{}", frontmatter.serialise());
        //println!("{}", api.frontmatter(&view.body).unwrap());
    });
}

fn stringpath_join(paths: &[&str]) -> String {
    paths.iter().for_each(|part| {
        debug_assert!(!part.contains("/"));
    });
    paths.join("")
}

fn check_is_file_or_die(pathstr: &str) {
    if !Path::new(pathstr).is_file() {
        Path::new(pathstr).metadata()
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
#[derive(Debug)]
struct Config {
    arg: String,
    stdin: bool,
    verbose: bool,

    api_dir: Option<String>,
    cache_dir: Option<String>,
    domain: Option<String>, // public dir as a URL
    public_dir: Option<String>, // public dir as a path
    templates_dir: Option<String>,
}
impl Config {
    fn parse_env() -> Result<(Self, Vec<String>), String> {
        let mut output = Vec::with_capacity(env::args().count());
        let mut config = Self {
            // Defaults
            arg: String::new(),
            stdin: false,
            api_dir: None,
            cache_dir: None,
            domain: None,
            public_dir: None,
            templates_dir: None,
            verbose: false,
        };
        let mut literal = false;
        let mut arg_iter = env::args();
        arg_iter.next(); // skip the 0th parameter (path to program running)
        while let Some(arg) = arg_iter.next() {
            if literal || !arg.starts_with('-') {
                output.push_and_check(arg);
            } else if arg.as_str() == "--" {
                literal = true;
            } else if arg.as_str() == "-" {
                config.stdin = true;
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
