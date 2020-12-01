// Parsing the command line options and argument parsing
// Redirect flow to their appropriate commands
// Although I could use clap.rs, I want to keep this lean
// Also implementing manually as a learning experience

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

mod compile;
mod custom_errors;
mod fileapi;
mod frontmatter;
mod helpers;
mod post;
mod traits;

use compile::compile;
use helpers::program_name;
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

                    exit(1)
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
        @derived {
            $($d_id:ident = [$d_from:ident, $d_add:literal],)*
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
            $($d_id: String,)*
        }
        impl<'a> RequiredConfigs<'a> {
            fn unwrap(config: &'a Config) -> Self {
                Self {
                    $($o_id: config.$o_id,)*
                    $($r_id: config.$r_id.as_ref()
                        .ok_or(concat!("--", $r_long, " is a required option"))
                        .or_die(1)
                        .as_str(),
                    )*
                    $($d_id: [config.$d_from.as_ref().unwrap(), $d_add].join(""),)*
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
        "b" "blog-relative" blog_relative, // blog directory inside of public_dir
        "c" "cache-dir"     cache_dir,
        "d" "domain"        domain,        // public dir as a URL
        "l" "linker"        linker,
        "o" "output-format" output_format,
        "p" "public-dir"    public_dir,    // public dir as a path
        "t" "templates-dir" templates_dir,
    }
    @derived {
        tags_cache = [cache_dir, "/tags.csv"],
        link_cache = [cache_dir, "/link.csv"],
        changelog  = [cache_dir, "/changelog.csv"],
    }
}

fn main() {
    let (config, args) = Config::parse_env().or_die(1);

    match_subcommands!(args {
        1, "now-rfc2822" => {
            println!("{}", Local::now().to_rfc2822());
        }
        3, "is-first-newer-than" => {
            let base = Path::new(args.get(1).unwrap());
            let against = Path::new(args.get(2).unwrap());
            if compare_mtimes(base, against)  {
                exit(0)
            } else {
                exit(1)
            }
        }
        3, "sync-last-updated-of-first-to" => {
            sync_last_updated(args.get(1).unwrap(), args.get(2).unwrap());
        }

        2, "compile-markup" => {
            let source = args.get(1).unwrap();
            let unwrapped_config = RequiredConfigs::unwrap(&config);
            compile(&unwrapped_config, &[PathBuf::from(source)]);
        }

        2, "compile" => {
            let published_dir = args.get(1).unwrap();
            let unwrapped_config = RequiredConfigs::unwrap(&config);
            let input_list = shallow_walk(published_dir, unwrapped_config.verbose).or_die(1);

            compile(&unwrapped_config, input_list.as_slice());
        }
    });
}

// @TODO implement warning system (not just fatal errors) for custom_errors.rs?
//fn issue_warning() {
//    eprintln!()
//}

//run: ../build.sh build-rust clean compile-blog

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

fn compare_mtimes(source: &Path, target: &Path) -> bool {
    let source_date = source
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .map_err(|err| {
            [
                source.to_string_lossy().escape().as_str(),
                ": ",
                err.to_string().as_str(),
            ]
            .join("")
        })
        .or_die(1);

    let target_date = target
        .metadata()
        .map(|metadata| FileTime::from_last_modification_time(&metadata))
        .or_else(|err| match err.kind() {
            ErrorKind::NotFound => Ok(FileTime::zero()),
            _ => Err(err),
        })
        .map_err(|err| {
            [
                target.to_string_lossy().escape().as_str(),
                ": ",
                err.to_string().as_str(),
            ]
            .join("")
        })
        .or_die(1);

    source_date > target_date
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

fn shallow_walk(dir_loc: &str, is_verbose: bool) -> Result<Vec<PathBuf>, String> {
    // @TODO: see if a there is a good way to precalculate
    let walk_dir = fs::read_dir(dir_loc).map_err(|err| {
        [
            "Cannot read ",
            dir_loc.escape().as_str(),
            ". ",
            err.to_string().as_str(),
        ]
        .join("")
    })?;

    let mut list_of_paths = Vec::new();
    for entry in walk_dir {
        let entry = entry.map_err(|err| {
            [
                "Error while shallow walking ",
                dir_loc.escape().as_str(),
                " . ",
                err.to_string().as_str(),
            ]
            .join("")
        })?;
        let path = entry.path();
        if path.is_file() {
            list_of_paths.push(path);
        } else if is_verbose {
            return Err([
                "Skipping processing ",
                path.to_string_lossy().escape().as_str(),
                " because it is a directory.",
            ]
            .join(""));
        }
    }

    Ok(list_of_paths)
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
        let api = FileApi::from_filename("config/api/", "adoc", ("", "blog")).or_die(1);
        let frontmatter_string = api.frontmatter(view.body.as_slice()).unwrap();
        let frontmatter =
            Frontmatter::new(frontmatter_string.as_str(), Utc::now(), Utc::now()).or_die(1);
        assert!(frontmatter.lookup("date-created").is_some());
        assert!(frontmatter.lookup("date-updated").is_some());
    }
}
