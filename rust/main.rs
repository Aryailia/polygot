// Parsing the command line options and argument parsing
// Redirect flow to their appropriate commands
// Although I could use clap.rs, I want to keep this lean
// Also implementing manually as a learning experience

#![allow(dead_code)]
use chrono::offset::Local;
use filetime::{set_file_mtime, FileTime};
use std::{env, io::ErrorKind, path::Path, process::exit, str::Chars};

mod compile;
mod custom_errors;
mod fileapi;
mod frontmatter;
mod helpers;
mod post;
mod traits;

use compile::compile;
use helpers::{check_is_dir, check_is_file, program_name};
use traits::{ResultExt, ShellEscape, VecExt};

macro_rules! match_subcommands {
    ($args:ident {
        $($arg_count:literal, $subcommand:literal => $block:block)*
    }) => {
        let len = $args.len();
        let first = $args.first().map(|s| s.as_str());
        match first {
            $(Some(arg) if arg == $subcommand => {
                if len != $arg_count {
                    eprintln!(
                        "`{} {}` requires {} arguments. You provided {} arguments",
                        program_name(), arg, $arg_count, len
                    );

                    exit(1);
                }
                $block
            })*
            Some(arg) => {
                eprintln!("`{} {}` is an invalid subcommand.", program_name(), arg);
                exit(1)
            }
            _ => {
                eprintln!("No subcommand given. `{} -h` for list of subcommands", program_name());
                exit(1)
            }

        }

    };
}

macro_rules! define_config {
    (
        @optional {
            $($o_short:literal $o_long:literal
               $o_id:ident: $o_type:ty = $o_default:expr => $to_set:expr,)*
        }
        @to_be_required {
            $($r_short:literal $r_long:literal $r_id:ident,)*
        }
    ) => {
        #[derive(Debug)]
        pub struct Config {
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
                $($o_short | $o_long => config.$o_id = $to_set,)*
                $($r_short | $r_long => config.$r_id = arg_iter.next(),)*
                _ => {
                    return Err([
                        "'",
                        option,
                        "'",
                        " is an invalid option\nTry `",
                        program_name().as_str(),
                        " -h` for help",
                    ].join(""))
                }
            }
            Ok(())
        }

        // Put in a struct so that we can keep the variable names
        // the same between 'parse_option' and use in 'compile_post'
        // Naming chosen for the sentence: 'RequireConfigs::unwrap(config)'
        #[derive(Debug)]
        pub struct RequiredConfigs<'a> {
            $($o_id: $o_type,)*
            $($r_id: &'a str,)*
        }
        impl<'a> RequiredConfigs<'a> {
            fn unwrap(config: &'a Config) -> Self {
                Self {
                    $($o_id: config.$o_id,)*
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
            let linker_loc = args.get(2).unwrap();
            let output_template = args.get(3).unwrap();
            let unwrapped_config = RequiredConfigs::unwrap(&config);

            check_is_file(linker_loc.as_str()).or_die(1);
            // @VOLATILE sync with 'define_config'
            check_is_dir(unwrapped_config.cache_dir, "--cache-dir").or_die(1);
            check_is_file(unwrapped_config.api_dir).or_die(1);


            compile(&unwrapped_config, &source, &linker_loc, &output_template);
        }
    });
}

// @TODO implement warning system (not just fatal errors) for custom_errors.rs?
//fn issue_warning() {
//    eprintln!()
//}

//run: ../build.sh

fn sync_last_updated(first: &str, date_source: &str) -> ! {
    Path::new(date_source)
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .and_then(|filetime| set_file_mtime(Path::new(first), filetime))
        .map_err(|err| {
            [
                date_source.escape().as_str(),
                ": ",
                err.to_string().as_str(),
            ]
            .join("")
        })
        .or_die(1);
    exit(0)
}

fn compare_mtimes(source: &str, target: &str) -> bool {
    let source_date = Path::new(source)
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .map_err(|err| [source.escape().as_str(), ": ", err.to_string().as_str()].join(""))
        .or_die(1);

    let target_date = Path::new(target)
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .or_else(|err| match err.kind() {
            ErrorKind::NotFound => Ok(FileTime::zero()),
            _ => Err(err),
        })
        .map_err(|err| [target.escape().as_str(), ": ", err.to_string().as_str()].join(""))
        .or_die(1);

    source_date < target_date
}

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
mod integration_tests {
    use crate::fileapi::FileApi;
    use crate::frontmatter::Frontmatter;
    use crate::post::Post;
    use crate::traits::ResultExt;
    use chrono::Utc;
    #[test]
    fn compile_test() {
        let post = Post::new("hello", "//").or_die(1);
        let view = post.views.first().unwrap();
        let api = FileApi::from_filename("config/api/", "adoc").or_die(1);
        let frontmatter_string = api.frontmatter(view.body.as_slice()).unwrap();
        let frontmatter =
            Frontmatter::new(frontmatter_string.as_str(), Utc::now(), Utc::now()).or_die(1);
        assert!(frontmatter.lookup("date-created").is_some());
        assert!(frontmatter.lookup("date-updated").is_some());
    }
}
