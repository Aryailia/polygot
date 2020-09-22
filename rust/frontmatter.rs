// 'date-created', 'date-updated', and
// 'tags' has a special format
// NOTE: 'filename', 'lang' are reserved
use crate::custom_errors::ParseError;
use chrono::{Utc, DateTime, Datelike};
use std::borrow::Cow;

#[derive(Debug)]
pub enum Value<'a> {
    Utf8(&'a str),
    //DateTime(DateTime<chrono::offset::FixedOffset>),
    DateTime(DateTime<Utc>),
}

// @TODO unit test tags, date-created, date-updated always exist
//#[test]
//fn tags() {
//
//
//
//    println!("hello");
//}

const KEY_BLACKLIST: [&str; 5] = ["file_stem", "lang", "year", "month", "day"];
type FrontmatterResult<'a> = Result<Frontmatter<'a>, String>;

use crate::traits::{RangeExt, VecExt};

// This is slightly wasteful with memory
// 'ignore_list' is expected to be small
// returns the problematic tag
fn parse_tags_and_push<'a>(
    list: &mut Vec<&'a str>,
    line: &'a str,
    ignore_list: &[&str],
) -> Result<(), String> {
    list.reserve(line.split_whitespace().count());
    for tag in line.split_whitespace().filter(|t| !ignore_list.contains(t)) {
        if list.contains(&tag) {
            return Err(format!(
                "{:?} was already defined. Cannot have duplicates",
                tag
            ));

        // '/' and '\\' interfere with pathnames
        // ',' interfers with csv for tags cache
        } else if tag.contains(&['/', '\\', ','][..]) {
            return Err(format!("{:?} is an invalid tag.", tag));
        } else {
            list.push_and_check(tag);
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct Frontmatter<'a> {
    // This instead of two vec's as key and value lengths must be the same
    keys: Vec<&'a str>,
    values: Vec<Value<'a>>,
    tags: Vec<&'a str>,
}

//run: ../build.sh
// @TODO: Maybe change use a proper JSON parser?
impl<'a> Frontmatter<'a> {
    pub fn new(frontmatter: &'a str, created: DateTime<Utc>, modified: DateTime<Utc>) -> Result<Self, ParseError> {
        // + 2 for guarenteed 'date-created' and 'date-updated'
        let keyval_count = validate_and_count(frontmatter)? + 2;
        let mut key_list = Vec::with_capacity(keyval_count);
        let mut value_list = Vec::with_capacity(keyval_count);
        let mut tag_list = Vec::new();

        for (i, line) in frontmatter
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.is_empty())
        {
            let colon_index = line.find(':').unwrap();
            let key = &line[0..colon_index];
            let val_str = line[colon_index + ':'.len_utf8()..].trim();

            if KEY_BLACKLIST.iter().any(|k| *k == key) {
                error_invalid(i + 1, line, key, "is reserved")?;
            } else if key_list.contains(&key) {
                // @TODO: Change to a warning
                error_invalid(
                    i + 1,
                    line,
                    key,
                    "was already defined. Cannot have duplicates.",
                )?;
            } else if key == "tags" {
                parse_tags_and_push(&mut tag_list, val_str, &[])
                    .map_err(|err| (i + 1, line, Cow::Owned(err)))?;
            } else if key == "date-created" || key == "date-updated" {
                key_list.push_and_check(key);
                let date = DateTime::parse_from_rfc2822(val_str).map_err(|err| {
                    (
                        i + 1,
                        line,
                        Cow::Owned(
                            [
                                "Dates must conform to RFC 2822 dates (internet format).\n",
                                err.to_string().as_str(),
                            ]
                            .join(""),
                        ),
                    )
                })?;
                value_list.push_and_check(Value::DateTime(date.with_timezone(&Utc)));
            } else {
                key_list.push_and_check(key);
                value_list.push_and_check(Value::Utf8(val_str));
            }
        }
        // Default have 'date-modified' and 'date-updated'
        if !key_list.contains(&"date-created") {
            key_list.push_and_check("date-created");
            value_list.push_and_check(Value::DateTime(created));
        }
        if !key_list.contains(&"date-updated") {
            key_list.push_and_check("date-updated");
            value_list.push_and_check(Value::DateTime(modified));
        }
        debug_assert_eq!(key_list.len(), value_list.len());
        Ok(Self {
            keys: key_list,
            values: value_list,
            tags: tag_list,
        })
    }

    // Emulate hashmap lookup with a Vec<(_, _)>
    pub fn lookup(&'a self, key: &str) -> Option<&'a Value<'a>> {
        let i = self.keys.iter().position(|k| k == &key)?;
        Some(&self.values[i])
    }

    #[inline]
    fn pad_two<'b>(num: u32) -> Cow<'b, str> {
        let mut padded = String::with_capacity('0'.len_utf8() * 2);
        let tens =  num / 10;
        let ones = num % 10;
        padded.push(std::char::from_digit(tens, 10).unwrap());
        padded.push(std::char::from_digit(ones, 10).unwrap());
        Cow::Owned(padded)
    }

    pub fn format(
        &self,
        template: &str,
        file_stem: &str,
        lang: &str,
    ) -> String {
        let range = 0..template.len();
        let count = range.split_over(template, Self::find_markup).count();
        let mut output = Vec::with_capacity(count * 2);
        range.split_over(template, Self::find_markup).for_each(|x| {
            let text = x.0.of(template);
            let key = x.1.of(template);
            let key = if key.is_empty() {
                key
            } else { // remove the surrounding curly brackets
                &key['{'.len_utf8()..key.len() - '}'.len_utf8()]
            };

            // @TODO allow for date format
            // VOLATILE: Sync with key_blacklist
            let value = match key {
                "year" | "month" | "day" => match self.lookup("date-created") {
                    Some(Value::Utf8(_)) => unreachable!("Always stored as a date"),
                    Some(Value::DateTime(x)) => match key {
                        "year" => Cow::Owned(x.year().to_string()),
                        "month" => Self::pad_two(x.month()),
                        "day" => Self::pad_two(x.day()),
                        _ => unreachable!(),
                    },
                    None => todo!("Will add defaults to be taken in from file"),
                }
                "lang" => Cow::Borrowed(lang),
                "file_stem" => Cow::Borrowed(file_stem),
                _ => {
                    match self.lookup(key) {
                        Some(Value::Utf8(x)) => Cow::Borrowed(*x),
                        Some(Value::DateTime(_)) => todo!(),
                        None => Cow::Borrowed(""),
                    }
                }
            };
            output.push_and_check(Cow::Borrowed(text));
            output.push_and_check(value);
        });
        output.join("")
    }

    // post, frontmatter, lang => filename (check all filepath limits?) => filepathk
    // @TODO for split_over()
    //       change this back to Option<Range<usize>>
    //       will be useful custom_error for more robustness
    //       as we can terminate more eplicitly (maybe), also more idomatic
    pub fn find_markup(buffer: &str) -> std::ops::Range<usize> {
        buffer.find('{').and_then(|i| {
            let rest = &buffer[i..buffer.len()];
            rest.find('}').map(|end| (i..i + end + '}'.len_utf8()))
        }).unwrap_or(buffer.len()..buffer.len())
    }

    //Junk,2019-11-01,stuff,en,The Quick, brown fox jumped over the lazy doggo
    //Junk,2019-11-01,stuff,jp,これはこれはどういう意味なんだろう
    //Linguistics,2019-11-01,stuff,en,The Quick, brown fox jumped over the lazy doggo
    //Linguistics,yo,happy-times,zh,辣妹
    //Sinitic,2020-03-15,chinese_tones,en,Rusheng

    // Though we have lang, this is faster
    //pub fn format_to_tag_cache(&self, file_stem: &str, lang: &str) -> String {
    //    // @TODO: elevate to only generate once (store in frontmatter?)
    //    // probably do not want to randomly benchmark in the user-facing code
    //    let utc = chrono::offset::FixedOffset::east(0);
    //    let date = match self.lookup("date-created") {
    //        Some(Value::DateTime(dt)) => dt.with_timezone(&utc),
    //        _ => Utc::now().with_timezone(&utc),
    //    }
    //    .format("%Y-%m-%d %H:%M:%S%.3f")
    //    .to_string();
    //    let date = date.as_str();

    //    self.lookup("tags")
    //        .map(|val| match val {
    //            Value::Utf8(s) => s,
    //            _ => unreachable!(),
    //        })
    //        .unwrap_or("")
    //        .split_whitespace()
    //        .map(|tag| {
    //            let title = match self.lookup("title") {
    //                Some(Value::Utf8(s)) => s,
    //                Some(Value::DateTime(_)) => unreachable!(),
    //                None => "",
    //            };
    //            //format!("{},{},{},{},{}\n", tag, date, lang, file_stem, title)
    //            [
    //                tag,
    //                ",",
    //                date,
    //                ",",
    //                lang,
    //                ",",
    //                file_stem,
    //                ",",
    //                title,
    //                "\n",
    //            ].join("")
    //        })
    //        .collect::<Vec<String>>()
    //        .join("")
    //}

    // Serialising 'api_entries' here as well simply for code consolidation
    pub fn serialise(&self) -> String {
        let len = self.keys.len();
        assert!(len == self.values.len());

        // Four per line (key, delimiter, value, newline)
        let mut meta_keyvals = Vec::with_capacity(
            len * 4               // key, colon, value, newline
            + self.tags.len() * 2 // " " and tag
            + 1                   // 'tags:' and '\n'
            + if self.tags.is_empty() {
                1
            } else {
                0                 // - 1 for trimming leading space on tags
            },
        );

        meta_keyvals.push_and_check(Cow::Borrowed("tags:"));
        let mut not_first = false;
        for tag in &self.tags {
            if not_first {
                meta_keyvals.push_and_check(Cow::Borrowed(" "));
            }
            not_first = true;
            meta_keyvals.push_and_check(Cow::Borrowed(tag));
        }
        meta_keyvals.push_and_check(Cow::Borrowed("\n"));

        for i in 0..len {
            meta_keyvals.push_and_check(Cow::Borrowed(self.keys[i]));
            meta_keyvals.push_and_check(Cow::Borrowed(":"));
            meta_keyvals.push_and_check(match &self.values[i] {
                Value::Utf8(s) => Cow::Borrowed(s),
                // Date is the only owned entry
                Value::DateTime(datetime) => datetime.to_rfc2822().into(),
            });
            meta_keyvals.push_and_check(Cow::Borrowed("\n"));
        }

        debug_assert_eq!(meta_keyvals.len(), meta_keyvals.capacity());
        // join should be allocating the right size, probably
        meta_keyvals.join("")
    }
}

// @MARKUP_RULE
fn validate_and_count(frontmatter: &str) -> Result<usize, ParseError> {
    let mut count = 0;
    for line in frontmatter.lines().filter(|l| !l.is_empty()) {
        count += 1;
        line.find(':')
            .ok_or("No key found (i.e. no ':'). Correct syntax is 'key:value'")
            .and_then(|colon_index| {
                let key = &line[0..colon_index];
                if key.chars().any(|c| c.is_whitespace()) {
                    Err("The key cannot contain whitespaces.")
                } else {
                    Ok(())
                }
            })
            .map_err(|err| (count + 1, line, Cow::Borrowed(err)))?;
    }
    Ok(count)
}

/*******************************************************************************
 * Error messages
 ******************************************************************************/
#[inline]
fn error_invalid<'a>(
    row: usize,
    line: &'a str,
    key: &'a str,
    msg: &'a str,
) -> Result<(), ParseError<'a>> {
    Err((row, line, Cow::Owned(format!("Key {:?} {}", key, msg))).into())
}
//fn invalid_tags(
//    tags: &'a str,
//    //valid_tag: &Regex,
//    line: &'a str,
//) -> FrontmatterResult<'a> {
//    let mut error = tags
//        .split_whitespace()
//        //.filter(|tag| !valid_tag.is_match(tag))
//        .fold(
//            format!("Error in line '{}'.\nThese tags are invalid:\n", line),
//            |mut error_string, tag| {
//                error_string.push_str("- ");
//                error_string.push_str(tag);
//                error_string.push_str("\n");
//                error_string
//            },
//        );
//    //error.push_str(&format!("Must adhere to /{}/", valid_tag.as_str()));
//    Err(error)
//}

fn cannot_parse_date_tag<'a>(
    key: &'a str,
    val: &'a str,
    parse_err: String,
) -> FrontmatterResult<'a> {
    Err(format!(
        r"If the file path template requires {{day}}, {{month}}, or {{year}}.

Key:   {}
Value: {}

The value must conform to RFC 2822 dates (internet format).

Err: {}
",
        key, val, parse_err
    ))
}

// run: cargo test frontmatter::frontmatter_test -- --nocapture
//#[cfg(test)]
//mod frontmatter_test {
//    use super::*;
//    use crate::fileapi::Interface;
//    use crate::post::Post;
//    use std::ffi::OsStr;
//    use std::path::Path;
//
//    #[test]
//    fn test() {
//        let config = "config/make";
//        let input = Path::new(".blog/published");
//        let interface_cache = Interface::auto_load_apis(config, input);
//        let api = interface_cache.get_api(OsStr::new("adoc")).unwrap();
//
//        let file = std::fs::read_to_string("test/chinese_tones.adoc").unwrap();
//        let post = Post::new_multi_lang(&api, file).unwrap();
//        post.views.iter().for_each(|view| {
//            let frontmatter = Frontmatter::new(&api, &view.as_string(&post.original), true, false);
//            println!("{:?}", frontmatter);
//        });
//        //println!("{:?}", post);
//    }
//}
