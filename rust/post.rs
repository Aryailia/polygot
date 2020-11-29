use crate::custom_errors::ParseError;
use crate::helpers::parse_tags_and_push;
use crate::traits::{BoolExt, VecExt};
use std::borrow::Cow;

//run: cargo test -- --nocapture

const API_SET_LANGUAGE: &str = "api_set_lang:";
const ALL_LANG: Option<&str> = None;
const ALL_LANG_REPR: [&str; 2] = ["*", "ALL"]; // case-sensitive

// Originally, I wanted 'Post' to own the post and hand out views as borrows
// https://cfsamson.github.io/books-futures-explained/4_pin.html
// I cannot figure self-referntial structs in a nice way
// (Ouborous crate seems to be the newest thing)
#[derive(Debug)]
pub struct Post<'a> {
    original: &'a str,
    pub views: Vec<PostView<'a>>,
    pub lang_list: Vec<&'a str>,
}

#[derive(Debug)]
pub struct PostView<'a> {
    pub lang: Option<&'a str>,
    pub body: Vec<&'a str>,
}

impl<'a> Post<'a> {
    pub fn new(text: &'a str, comment_marker: &str) -> Result<Self, ParseError<'a>> {
        // 'lang_count' will count all duplicates (which is the common case)
        // e.g. api_set_lang: en jp
        //      api_set_lang: ALL
        //      api_set_lang: en
        // counts four
        // It might just be better to let 'unique_langs' auto size
        let (part_count, lang_max) = SplitByLabel::new(text, comment_marker).fold(
            (0, 0),
            |(sections, langs), (_, lang_label, _, _)| {
                (sections + 1, langs + lang_label.split_whitespace().count())
            },
        );

        let mut unique_langs = Vec::with_capacity(lang_max);
        let mut parts = Vec::with_capacity(part_count);
        let mut labels = Vec::with_capacity(part_count);
        {
            let mut cur_langs = ALL_LANG_REPR[0]; // start with an all label
            for (section, next_langs, line, row) in SplitByLabel::new(text, comment_marker) {
                let has_all_label = cur_langs
                    .split_whitespace()
                    .any(|t| ALL_LANG_REPR.contains(&t));

                // Build up 'unique_langs' list
                // e.g. 'api_set_lang: en ALL jp' -> 'en all jp'
                // May be both all and list languages
                let langs =
                    parse_tags_and_push(&mut unique_langs, cur_langs, &ALL_LANG_REPR, false)
                        .map_err(|err| (row, line, Cow::Owned(err)))?;

                //print!("{:?} | {:?}", cur_langs, langs);
                let label = (!has_all_label && !langs.is_empty()).to_some(langs);
                //println!(" {:?} | {:?}", label, section);

                labels.push_and_check(label);
                parts.push_and_check(section);
                cur_langs = next_langs;

                //println!("{:?}", replace(&mut label, Some(langs)));
                //println!("{:?}", has_all_label);
                //println!("{:?}\n", section);
            }
        }

        //for i in 0..part_count {
        //    println!("{:?}\n{:?}\n", labels[i], parts[i]);
        //}
        //println!();

        unique_langs.sort_unstable();

        // Transpose 'parts' from by parts-by-langs to langs-by-parts (PostView)
        let lang_count = unique_langs.len(); // bet
        let mut view_list = Vec::with_capacity(std::cmp::max(lang_count, 1));
        if unique_langs.is_empty() {
            view_list.push_and_check(PostView {
                lang: None,
                body: parts,
            });
        } else {
            for lang in &unique_langs {
                let mut view = Vec::with_capacity(part_count);

                let mut iter = labels.iter();
                view.extend(parts.iter().filter(|_| {
                    if let Some(Some(lang_labels)) = iter.next() {
                        //println!("{:?}", lang_labels);
                        //false
                        lang_labels.iter().any(|l| l == lang)
                    } else {
                        true
                    }
                }));
                //view.extend(
                //    parts
                //        .iter()
                //        .filter_map(|x| pick_lang(x, lang))
                //);

                view_list.push_and_check(PostView {
                    lang: Some(lang),
                    body: view,
                });
            }
        }

        Ok(Self {
            original: text,
            views: view_list,
            lang_list: unique_langs,
        })
    }
}

/******************************************************************************
 * Post helper functions
 ******************************************************************************/
struct SplitByLabel<'a, 'b> {
    iter: std::str::Chars<'a>,
    comment: &'b str,
    comment_len: usize,
    row: usize,
}
impl<'a, 'b> SplitByLabel<'a, 'b> {
    fn new(buffer: &'a str, comment: &'b str) -> Self {
        Self {
            iter: buffer.chars(),
            comment,
            comment_len: comment.len(),
            row: 0,
        }
    }
}

impl<'a, 'b> Iterator for SplitByLabel<'a, 'b> {
    type Item = (&'a str, &'a str, &'a str, usize);
    fn next(&mut self) -> Option<Self::Item> {
        enum State {
            Blank,
            Newline,
            Slash(usize),
        }
        let mut state = State::Newline; // \n needs not preceed first char
        let mut cursor = 0;
        let as_str = self.iter.as_str();
        while let Some(c) = self.iter.next() {
            let rest = &as_str[cursor..];
            let is_newline = c == '\n';
            if is_newline {
                self.row += 1;
            }

            match (&state, is_newline) {
                (State::Newline, _) if rest.starts_with(self.comment) => {
                    state = State::Slash(cursor); // after newline

                    // Skipping count - 1 because 'while' walk already skips one
                    for _ in 1..self.comment_len {
                        cursor += c.len_utf8();
                        self.iter.next();
                    }
                }
                (State::Slash(_), _) if c.is_whitespace() => {}
                (State::Slash(mid), _) => {
                    //let right = &as_str[*mid..];
                    //let end = right.find('\n').unwrap_or(right.len());
                    //return Some((&as_str[..*mid], &right[..end]));
                    //println!("- {:?}\n", &as_str[*mid..]);

                    if rest.starts_with(API_SET_LANGUAGE) {
                        let left = &as_str[..*mid];

                        // Skip until newline
                        let mut end = cursor + c.len_utf8();
                        while let Some(r) = self.iter.next() {
                            end += r.len_utf8();
                            if r == '\n' {
                                break;
                            }
                        }
                        let right = &as_str[cursor + API_SET_LANGUAGE.len()..end];
                        let label = &as_str[*mid..end];

                        return Some((left, right, label, self.row));
                    } else {
                        state = State::Blank;
                    }
                }
                (_, true) => state = State::Newline,
                (_, false) => state = State::Blank,
            }

            cursor += c.len_utf8();
        }
        if cursor > 0 {
            Some((&as_str[..cursor], "", "", self.row))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    const COMMENT: &str = "//";

    #[test]
    fn integration_test() {
        //let content = std::fs::read_to_string("rust/test.adoc").unwrap();
        //let post = Post::new(content.as_str(), "//").unwrap();

        //println!("{:#?}", post.views);
    }

    fn parse(buffer: &str) -> Vec<(&str, &str)> {
        SplitByLabel::new(buffer, COMMENT)
            .map(|x| (x.0, x.1))
            .collect::<Vec<_>>()
    }

    #[test]
    fn split_only_delimiter() {
        let line = &format!("{}{} hello", COMMENT, API_SET_LANGUAGE);
        assert_eq!(vec![("", " hello")], parse(line));
    }
    #[test]
    fn split_delimiter_start() {
        let line = &format!("{}{} hello\nasdf\n", COMMENT, API_SET_LANGUAGE);
        assert_eq!(vec![("", " hello\n"), ("asdf\n", "")], parse(line));
    }
    #[test]
    fn split_delimiter_end() {
        let line = &format!("stuff and things\n{}{} hello", COMMENT, API_SET_LANGUAGE);
        assert_eq!(vec![("stuff and things\n", " hello")], parse(line));
    }
    #[test]
    fn split_empty() {
        let against: Vec<(&str, &str)> = Vec::new();
        assert_eq!(against, parse(""));
    }
    #[test]
    fn split_no_delimiter() {
        let line = "lorem ipsem
// adsfkj
qwekrjl
qewrkjladjsf ldjas lf
";
        assert_eq!(vec![(line, "")], parse(line));
    }
}
