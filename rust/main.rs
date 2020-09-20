#![allow(dead_code)]
use chrono::offset::Local;
use filetime::{set_file_mtime, FileTime};
use std::{env, fs, io::ErrorKind, path::Path, process::exit, str::Chars};

mod fileapi;
mod post;
mod traits;
use fileapi::FileApi;
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

fn main() {
    let (_config, args) = Config::parse_env().or_die(1);

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
            let cache_dir = args.get(2).unwrap();
            let api_dir = args.get(3).unwrap();

            let file = fs::read_to_string(source)
                .map_err(|err| format!("{:?} {}", source, err))
                .or_die(1);
            if !Path::new(cache_dir).is_dir() {
                Path::new(cache_dir).metadata()
                    .map_err(|err| format!("'{}' {}", cache_dir, err))
                    .or_die(1);
            }
            if !Path::new(api_dir).is_file() {
                Path::new(api_dir).metadata()
                    .map_err(|err| format!("'{}' {}", api_dir, err))
                    .or_die(1);
            }

            let api = FileApi::from_filename(source.as_str(), api_dir.as_str())
                .or_die(1);
            let _stem = Path::new(source).file_stem();
            compile_post(file, &api, source.as_str());
        }

        1, "test" => {
            let api = FileApi::from_filename("hello.adoc", "../config/api").unwrap();
            println!("{:?}", api.comment());
        }
    });
}

fn compile_post(text: String, api: &FileApi, filename: &str) {
    //let post = Post::new_multi_lang(text.as_str(), &api).or_die(1);
    let asdf = "// api_set_lang: yo/ \nasdf\nasdf\n// api_set_lang:try *\nyo";
    println!("{}", asdf);
    println!("########\n# Done #\n########\n");
    let post = match Post::new(asdf, &api) {
        Ok(x) => x,
        Err(err) => {
            eprintln!("{}{}", filename, err);
            exit(1);
        }
    };
    post.views.iter().for_each(|view| {
        println!("###### {} ######", view.lang.unwrap_or("All"));
        println!("{}", view.body.join(""));
        //    //println!("{}", api.frontmatter(&view.body).unwrap());
    });
}

fn sync_last_updated(first: &String, date_source: &String) -> ! {
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
}
impl Config {
    fn parse_env() -> Result<(Self, Vec<String>), String> {
        let mut output = Vec::with_capacity(env::args().count());
        let mut config = Self {
            // Defaults
            arg: String::new(),
            stdin: false,
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
                    match option {
                        "v" | "verbose" => config.verbose = true,
                        "h" | "help" => {}
                        "a" | "arg" => {
                            if let Some(s) = arg_iter.next() {
                                config.arg = s;
                            }
                        }
                        _ => {
                            return Err(format!(
                                "'{}' is an invalid option\nTry `{} -h` for help",
                                option, NAME,
                            ))
                        }
                    }
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
    println!("{:?}", option_str);
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
